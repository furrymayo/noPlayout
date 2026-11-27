//! Host protocol end to end over a real socket (PLAN §4.3, §2.14): registration and refusal
//! codes, snapshot-then-live ordering, dev `ctl` through the internal route, and the
//! authoritative result reaching the caller.

use futures_util::{SinkExt, StreamExt};
use noplayout_core::client;
use noplayout_core::ctl::Ctl;
use noplayout_core::health::HealthState;
use noplayout_core::hostbus::HostBus;
use noplayout_core::hostbus::state::Command;
use noplayout_core::http::{self, ApiState};
use noplayout_core::time::TimeSource;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn open(port: u16, register: Value) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws/host/1"))
        .await
        .unwrap();
    ws.send(Message::Text(register.to_string().into()))
        .await
        .unwrap();
    ws
}

async fn next(ws: &mut Ws) -> Message {
    tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("message within 5 s")
        .expect("stream open")
        .expect("no socket error")
}

async fn next_json(ws: &mut Ws) -> Value {
    loop {
        if let Message::Text(t) = next(ws).await {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["type"] != "TimeSync" {
                return v;
            }
        }
    }
}

async fn close_code(ws: &mut Ws) -> u16 {
    match next(ws).await {
        Message::Close(Some(f)) => f.code.into(),
        other => panic!("expected a close frame, got {other:?}"),
    }
}

fn ctl(port: u16, req: Ctl) -> tokio::task::JoinHandle<Value> {
    tokio::task::spawn_blocking(move || {
        let body = client::post_ctl(port, "ui-token", &req, Duration::from_secs(20)).unwrap();
        serde_json::from_str(&body).unwrap()
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn registration_snapshot_live_actions_and_ctl_results() {
    let bus = HostBus::start(&["1"], TimeSource::new(), None, None);
    let listener = http::bind(0).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(Notify::new());
    let state = Arc::new(ApiState {
        health: HealthState::new(),
        library: None,
        ui_token: Some("ui-token".into()),
        shutdown: stop.clone(),
        hostbus: Some(bus.clone()),
        host_dir: None,
        faults: None,
    });
    let server = tokio::spawn(http::serve(listener, state, stop.clone()));
    let ch = bus.channel("1").unwrap();
    let inc = ch.new_incarnation("launch-token".into()).await.unwrap();
    ch.promote(inc).await;

    let program = |token: &str| {
        json!({"type": "Register", "channel": "1", "role": "program", "hostVersion": "0.0.1",
               "inc": inc, "token": token})
    };
    // A token-less or wrong-token program registration is refused with 4403, never downgraded.
    let mut bad = open(port, program("nope")).await;
    assert_eq!(close_code(&mut bad).await, 4403);
    let mut bad = open(
        port,
        json!({"type": "Register", "channel": "1", "role": "program", "hostVersion": "0.0.1"}),
    )
    .await;
    assert_eq!(close_code(&mut bad).await, 4403);
    let mut bad = open(port, json!({"type": "Beat"})).await;
    assert_eq!(close_code(&mut bad).await, 4400);

    let mut page = open(port, program("launch-token")).await;
    let snap = next_json(&mut page).await;
    assert_eq!(
        (snap["type"].as_str(), snap["lastSeq"].as_u64()),
        (Some("Snapshot"), Some(0))
    );
    // The incarnation's program registration is single-use.
    let mut dup = open(port, program("launch-token")).await;
    assert_eq!(close_code(&mut dup).await, 4403);

    let load = ctl(
        port,
        Ctl::Command {
            channel: "1".into(),
            command: Command::Load {
                layer: 10,
                graphic_url: "/library/h/g.ograf.json".into(),
                data: Some(json!({"name": "A"})),
                cued: false,
            },
        },
    );
    let action = next_json(&mut page).await;
    assert_eq!(action["type"], "Action");
    assert_eq!(action["method"], "load");
    assert_eq!(action["seq"], 1);
    assert!(action["ts"]["utcUs"].is_i64() && action["ts"]["mono"]["us"].is_u64());
    assert_eq!(
        action["params"]["renderCharacteristics"]["resolution"]["height"],
        1080
    );
    let result = json!({"type": "Result", "seq": 1, "instanceId": action["instanceId"],
                        "generation": action["generation"], "statusCode": 200});
    page.send(Message::Text(result.to_string().into()))
        .await
        .unwrap();
    let reply = load.await.unwrap();
    assert_eq!(reply["dispatched"]["seq"], 1);
    assert_eq!(reply["result"]["statusCode"], 200);

    // A late-joining mirror gets the load in its snapshot, with its original ts, then live.
    let mut mirror = open(
        port,
        json!({"type": "Register", "channel": "1", "role": "web-fill", "hostVersion": "0.0.1"}),
    )
    .await;
    let snap = next_json(&mut mirror).await;
    assert_eq!(snap["layers"][0]["loadTs"], action["ts"]);
    assert_eq!(snap["layers"][0]["dataAtLoad"], json!({"name": "A"}));
    let upd = ctl(
        port,
        Ctl::Command {
            channel: "1".into(),
            command: Command::Update {
                target: noplayout_core::hostbus::state::Target::Layer(10),
                data: json!({"name": "B"}),
                skip_animation: false,
            },
        },
    );
    for ws in [&mut page, &mut mirror] {
        let a = next_json(ws).await;
        assert_eq!(
            (a["seq"].as_u64(), a["method"].as_str()),
            (Some(2), Some("updateAction"))
        );
    }
    let r = json!({"type": "Result", "seq": 2, "instanceId": action["instanceId"],
                   "generation": action["generation"], "statusCode": 200});
    page.send(Message::Text(r.to_string().into()))
        .await
        .unwrap();
    assert_eq!(upd.await.unwrap()["result"]["statusCode"], 200);

    let view = ctl(
        port,
        Ctl::Inspect {
            channel: "1".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(view["lastSeq"], 2);
    assert_eq!(view["activeInc"], inc);
    assert_eq!(view["instances"][0]["phase"], "loaded");
    assert_eq!(view["instances"][0]["data"], json!({"name": "B"}));
    assert_eq!(
        view["counters"]["refusedRegistrations"], 3,
        "4400 is refused before the actor"
    );

    // Faults need a supervisor; without one the route says so instead of pretending.
    let err = tokio::task::spawn_blocking(move || {
        client::post_ctl(
            port,
            "ui-token",
            &Ctl::KillRenderer {
                channel: "1".into(),
            },
            Duration::from_secs(5),
        )
    })
    .await
    .unwrap()
    .unwrap_err();
    assert!(err.to_string().contains("503"), "{err}");
    let err = tokio::task::spawn_blocking(move || {
        client::post_ctl(
            port,
            "wrong",
            &Ctl::Inspect {
                channel: "1".into(),
            },
            Duration::from_secs(5),
        )
    })
    .await
    .unwrap()
    .unwrap_err();
    assert!(err.to_string().contains("403"), "{err}");

    stop.notify_one();
    let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
}
