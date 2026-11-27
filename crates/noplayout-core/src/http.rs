//! The engine's HTTP server. M0 serves only `GET /api/v1/health` and the loopback-only
//! `POST /api/v1/internal/shutdown` (E-034: internal routes need the per-launch token).

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
    /// Token a supervising app must present to the internal routes; `None` disables them.
    pub ui_token: Option<String>,
    /// Notified when an authorised shutdown request arrives.
    pub shutdown: Arc<Notify>,
}

pub fn router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/internal/shutdown", post(shutdown))
        .with_state(state)
}

async fn health(State(state): State<Arc<ApiState>>) -> Json<Health> {
    Json(state.health.snapshot())
}

async fn shutdown(
    State(state): State<Arc<ApiState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> (StatusCode, &'static str) {
    if !peer.ip().is_loopback() {
        tracing::warn!(%peer, "refused shutdown from a non-loopback address");
        return (
            StatusCode::FORBIDDEN,
            "internal routes answer loopback only\n",
        );
    }
    let Some(expected) = state.ui_token.as_deref() else {
        return (
            StatusCode::FORBIDDEN,
            "this engine was started without a launch token; stop it with Ctrl+C\n",
        );
    };
    let presented = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok());
    if presented != Some(expected) {
        tracing::warn!(%peer, "refused shutdown with a missing or wrong token");
        return (StatusCode::FORBIDDEN, "wrong or missing launch token\n");
    }
    tracing::info!(%peer, "shutdown requested over the API");
    state.shutdown.notify_one();
    (StatusCode::ACCEPTED, "shutting down\n")
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
            ui_token: ui_token.map(str::to_owned),
            shutdown: Arc::new(Notify::new()),
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
