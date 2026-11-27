//! Host protocol wire types (PLAN §4.3). JSON over a WebSocket at `/ws/host/<channel>`; the
//! TypeScript side is `web/host/protocol.ts`. `type` discriminates messages.

use crate::time::Ts;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Page role. A role string never grants authority (§2.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Program,
    Preview,
    WebProgram,
    WebFill,
    WebKey,
    Editor,
}

/// OGraf methods the engine sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    Load,
    PlayAction,
    StopAction,
    UpdateAction,
    CustomAction,
    Dispose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub seq: u64,
    pub ts: Ts,
    pub method: Method,
    pub params: Value,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quarantined: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub revision: u64,
    pub seq: u64,
    pub ts: Ts,
    pub instance_id: String,
    pub generation: u64,
    pub method: Method,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointRec {
    pub state: Value,
    pub frontier_seq: u64,
    pub cut_ts: Ts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLayer {
    pub layer: u32,
    pub instance_id: String,
    pub generation: u64,
    pub graphic_url: String,
    pub data_at_load: Value,
    pub render_characteristics: Value,
    pub load_ts: Ts,
    pub journal: Vec<JournalEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<CheckpointRec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub last_seq: u64,
    pub server_now: Ts,
    pub layers: Vec<SnapshotLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cued: Option<SnapshotLayer>,
    pub flags: Vec<Value>,
}

/// Engine → page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum HostDown {
    Snapshot(Box<Snapshot>),
    Action(Action),
    TimeSync {
        server_now: Ts,
    },
    Checkpoint {
        instance_id: String,
        generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult {
    pub seq: u64,
    pub instance_id: String,
    pub generation: u64,
    pub status_code: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_step: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// The page ran this action from a `Snapshot` journal or checkpoint tail (§2.14). The engine
    /// also classifies by how it fed the page; either side saying replay makes it a replay.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replay: bool,
}

impl PageResult {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
    pub fn class(&self) -> u16 {
        self.status_code / 100
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Register {
    pub channel: String,
    pub role: Role,
    pub host_version: String,
    #[serde(default)]
    pub last_seq: Option<u64>,
    #[serde(default)]
    pub inc: Option<u64>,
    #[serde(default)]
    pub token: Option<String>,
}

/// Page → engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum HostUp {
    Register(Register),
    Result(PageResult),
    Beat {
        n: u64,
        raf_count: u64,
        lag_ms: f64,
        last_applied_seq: u64,
    },
    Synced {
        seq: u64,
    },
    PageError {
        text: String,
    },
    CheckpointResult {
        instance_id: String,
        generation: u64,
        frontier_seq: u64,
        cut_ts: Ts,
        state: Value,
    },
    CheckpointDeferred {
        instance_id: String,
    },
}

/// WebSocket close codes (§2.14).
pub mod close {
    /// Bad first message, wrong channel or malformed Register.
    pub const BAD_REGISTER: u16 = 4400;
    /// Program registration refused: not loopback, bad incarnation/token, or already held.
    pub const FORBIDDEN: u16 = 4403;
    /// Unknown channel.
    pub const NO_CHANNEL: u16 = 4404;
    /// A page of a superseded renderer incarnation.
    pub const SUPERSEDED: u16 = 4410;
    /// The page fell too far behind the live stream; it re-registers and resyncs.
    pub const TOO_SLOW: u16 = 4429;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_every_page_message_shape_from_protocol_ts() {
        let ts = json!({"utcUs": 1, "mono": {"run": "r", "us": 2}});
        for v in [
            json!({"type": "Register", "channel": "1", "role": "web-fill", "hostVersion": "0.0.1"}),
            json!({"type": "Register", "channel": "1", "role": "program", "hostVersion": "0.0.1", "inc": 3, "token": "t"}),
            json!({"type": "Result", "seq": 4, "instanceId": "i", "generation": 1, "statusCode": 200, "currentStep": 0, "payload": {"x": 1}}),
            json!({"type": "Beat", "n": 1, "rafCount": 9, "lagMs": 1.5, "lastAppliedSeq": 4}),
            json!({"type": "Synced", "seq": 4}),
            json!({"type": "PageError", "text": "x"}),
            json!({"type": "CheckpointResult", "instanceId": "i", "generation": 1, "frontierSeq": 3, "cutTs": ts, "state": {}}),
            json!({"type": "CheckpointDeferred", "instanceId": "i"}),
        ] {
            let m: HostUp =
                serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
            assert_eq!(serde_json::to_value(&m).unwrap()["type"], v["type"]);
        }
    }

    #[test]
    fn action_and_timesync_match_the_page_shape() {
        let ts = crate::time::TimeSource::with_run("r".into()).now();
        let a = HostDown::Action(Action {
            revision: 1,
            seq: 2,
            ts: ts.clone(),
            instance_id: "i".into(),
            generation: 3,
            method: Method::PlayAction,
            params: json!({}),
        });
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["type"], "Action");
        assert_eq!(v["instanceId"], "i");
        assert_eq!(v["method"], "playAction");
        assert!(v["ts"]["mono"]["us"].is_u64());
        let t = serde_json::to_value(HostDown::TimeSync { server_now: ts }).unwrap();
        assert!(t["serverNow"]["utcUs"].is_i64());
    }
}
