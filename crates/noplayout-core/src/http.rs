//! The engine's HTTP server: health, immutable validated library assets (M1), and the
//! loopback-only internal shutdown route (E-034: internal routes need the per-launch token).

use crate::TOKEN_HEADER;
use crate::health::{Health, HealthState};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;

/// State shared by the handlers.
#[derive(Debug)]
pub struct ApiState {
    pub health: HealthState,
    /// Validated package snapshots; `None` is used by health-only test servers.
    pub library: Option<crate::library::Library>,
    /// Token a supervising app must present to the internal routes; `None` disables them.
    pub ui_token: Option<String>,
    /// Notified when an authorised shutdown request arrives.
    pub shutdown: Arc<Notify>,
    /// The host protocol (`/ws/host/<channel>`) and dev control; `None` in health-only servers.
    pub hostbus: Option<crate::hostbus::HostBus>,
    /// Built `web/host` (`renderer.html`, `ograf-host.js`) served at `/host/`.
    pub host_dir: Option<std::path::PathBuf>,
    /// Where dev-control faults go (the engine's supervisor).
    pub faults: Option<tokio::sync::mpsc::UnboundedSender<crate::ctl::Fault>>,
}

pub fn router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/library/{hash}/{*asset}", get(library_asset))
        .route("/host/{*asset}", get(host_asset))
        .route("/ws/host/{channel}", get(host_ws))
        .route("/api/v1/internal/shutdown", post(shutdown))
        .route("/api/v1/internal/ctl", post(ctl))
        .with_state(state)
}

async fn host_ws(
    State(state): State<Arc<ApiState>>,
    axum::extract::Path(channel): axum::extract::Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> Result<axum::response::Response, StatusCode> {
    let bus = state.hostbus.clone().ok_or(StatusCode::NOT_FOUND)?;
    Ok(ws
        .max_message_size(noplayout_proto::MAX_FRAME)
        .on_upgrade(move |socket| bus.serve_socket(channel, peer, socket)))
}

async fn host_asset(
    State(state): State<Arc<ApiState>>,
    axum::extract::Path(asset): axum::extract::Path<String>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::response::IntoResponse;
    let dir = state.host_dir.clone().ok_or(StatusCode::NOT_FOUND)?;
    let rel = std::path::Path::new(&asset);
    if !rel
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = dir.join(rel);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        [
            ("content-type", asset_mime(&asset)),
            ("cache-control", "no-cache"),
            ("x-content-type-options", "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

/// Loopback + per-launch token gate for internal routes (E-034).
fn authorize(
    state: &ApiState,
    peer: SocketAddr,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, String)> {
    if !peer.ip().is_loopback() {
        tracing::warn!(%peer, "refused an internal request from a non-loopback address");
        return Err((
            StatusCode::FORBIDDEN,
            "internal routes answer loopback only\n".into(),
        ));
    }
    let Some(expected) = state.ui_token.as_deref() else {
        return Err((
            StatusCode::FORBIDDEN,
            "this engine was started without a launch token (set NOPLAYOUT_UI_TOKEN)\n".into(),
        ));
    };
    let presented = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok());
    if !presented.is_some_and(|p| crate::hostbus::state::token_eq(p, expected)) {
        tracing::warn!(%peer, "refused an internal request with a missing or wrong token");
        return Err((
            StatusCode::FORBIDDEN,
            "wrong or missing launch token\n".into(),
        ));
    }
    Ok(())
}

/// How long `ctl` waits for the authoritative page's result. The engine itself resolves every
/// action by then: the page times out a `load` at 10 s, and the bus commits a 504 at 12 s
/// (others 7 s) if no result arrived (ADR 0062). Commands sent before the program page has
/// registered are delivered to it live at registration.
const CTL_RESULT_WAIT: std::time::Duration = std::time::Duration::from_secs(14);

async fn ctl(
    State(state): State<Arc<ApiState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<crate::ctl::Ctl>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::ctl::{Ctl, Fault};
    use serde_json::json;
    authorize(&state, peer, &headers)?;
    let bad = |e: String| (StatusCode::BAD_REQUEST, format!("{e}\n"));
    let channel = |ch: &str| {
        state
            .hostbus
            .as_ref()
            .and_then(|b| b.channel(ch))
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no channel {ch}\n")))
    };
    let fault = |f: Fault| {
        state
            .faults
            .as_ref()
            .and_then(|tx| tx.send(f).ok())
            .map(|_| Json(json!({"accepted": true})))
            .ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "no supervisor\n".to_string(),
                )
            })
    };
    match req {
        Ctl::Import { path } => {
            let lib = state
                .library
                .clone()
                .ok_or_else(|| (StatusCode::NOT_FOUND, "no library\n".to_string()))?;
            let v = tokio::task::spawn_blocking(move || {
                let p = std::path::Path::new(&path);
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
                    lib.import_zip(p)
                } else {
                    lib.import_folder(p)
                }
            })
            .await
            .map_err(|e| bad(e.to_string()))?
            .map_err(|e| bad(format!("{e:#}")))?;
            Ok(Json(
                json!({"contentHash": v.content_hash, "manifestUrl": v.manifest_url()}),
            ))
        }
        Ctl::Command {
            channel: ch,
            command,
        } => {
            let (d, result) = channel(&ch)?
                .command(command)
                .await
                .map_err(|e| bad(e.to_string()))?;
            let result = tokio::time::timeout(CTL_RESULT_WAIT, result)
                .await
                .ok()
                .and_then(Result::ok);
            Ok(Json(json!({"dispatched": d, "result": result})))
        }
        Ctl::Inspect { channel: ch } => {
            let v = channel(&ch)?
                .inspect()
                .await
                .map_err(|e| bad(e.to_string()))?;
            Ok(Json(serde_json::to_value(v).unwrap_or_default()))
        }
        Ctl::KillRenderer { channel: ch } => {
            channel(&ch)?;
            fault(Fault::KillRenderer { channel: ch })
        }
        Ctl::Hang { channel: ch, ms } => {
            channel(&ch)?;
            fault(Fault::Hang { channel: ch, ms })
        }
    }
}

async fn library_asset(
    State(state): State<Arc<ApiState>>,
    axum::extract::Path((hash, asset)): axum::extract::Path<(String, String)>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::response::IntoResponse;
    let library = state.library.clone().ok_or(StatusCode::NOT_FOUND)?;
    let mime = asset_mime(&asset);
    // Disk I/O must not block the async engine's control handlers.
    let bytes = tokio::task::spawn_blocking(move || library.read_asset(&hash, &asset))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((
        [
            ("content-type", mime),
            ("cache-control", "public, max-age=31536000, immutable"),
            ("x-content-type-options", "nosniff"),
        ],
        bytes,
    )
        .into_response())
}

fn asset_mime(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mjs" | "js" => "text/javascript",
        "json" | "map" => "application/json",
        "css" => "text/css",
        "html" | "htm" => "text/html",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<Health> {
    Json(state.health.snapshot())
}

async fn shutdown(
    State(state): State<Arc<ApiState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> (StatusCode, String) {
    if let Err(e) = authorize(&state, peer, &headers) {
        return e;
    }
    tracing::info!(%peer, "shutdown requested over the API");
    state.shutdown.notify_one();
    (StatusCode::ACCEPTED, "shutting down\n".into())
}

/// Bind the HTTP listener on loopback. A taken port gets a plain message (IDEA §6).
pub async fn bind(port: u16) -> anyhow::Result<tokio::net::TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "port {port} on 127.0.0.1 is already in use by another program; \
                 start the engine with --port <free port>"
            )
        } else {
            anyhow::anyhow!("could not listen on {addr}: {e}")
        }
    })
}

/// Serve until `shutdown` is notified.
pub async fn serve(
    listener: tokio::net::TcpListener,
    state: Arc<ApiState>,
    stop: Arc<Notify>,
) -> std::io::Result<()> {
    let app = router(state).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { stop.notified().await })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use std::time::Duration;

    async fn start(ui_token: Option<&str>) -> (u16, Arc<ApiState>, Arc<Notify>) {
        let listener = bind(0).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(ApiState {
            health: HealthState::new(),
            library: None,
            ui_token: ui_token.map(str::to_owned),
            shutdown: Arc::new(Notify::new()),
            hostbus: None,
            host_dir: None,
            faults: None,
        });
        let stop = Arc::new(Notify::new());
        tokio::spawn(serve(listener, state.clone(), stop.clone()));
        (port, state, stop)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn health_answers_with_engine_running() {
        let (port, _state, stop) = start(None).await;
        let h =
            tokio::task::spawn_blocking(move || client::get_health(port, Duration::from_secs(5)))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(h.engine.state, "running");
        assert_eq!(h.engine.pid, std::process::id());
        stop.notify_one();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shutdown_needs_the_right_token() {
        let (port, state, stop) = start(Some("right")).await;
        let wrong = tokio::task::spawn_blocking(move || {
            client::post_shutdown(port, "wrong", Duration::from_secs(5))
        })
        .await
        .unwrap();
        assert!(
            matches!(&wrong, Err(client::ClientError::Status(403, body)) if body.contains("wrong")),
            "{wrong:?}"
        );
        let notified = state.shutdown.clone();
        let waiter = tokio::spawn(async move { notified.notified().await });
        tokio::task::spawn_blocking(move || {
            client::post_shutdown(port, "right", Duration::from_secs(5))
        })
        .await
        .unwrap()
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("shutdown notified")
            .unwrap();
        stop.notify_one();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shutdown_is_disabled_without_a_token() {
        let (port, _state, stop) = start(None).await;
        let r = tokio::task::spawn_blocking(move || {
            client::post_shutdown(port, "anything", Duration::from_secs(5))
        })
        .await
        .unwrap();
        assert!(
            matches!(r, Err(client::ClientError::Status(403, _))),
            "{r:?}"
        );
        stop.notify_one();
    }

    #[tokio::test]
    async fn taken_port_gets_a_plain_message() {
        let first = bind(0).await.unwrap();
        let port = first.local_addr().unwrap().port();
        let err = bind(port).await.unwrap_err().to_string();
        assert!(err.contains("already in use"), "{err}");
    }
}
