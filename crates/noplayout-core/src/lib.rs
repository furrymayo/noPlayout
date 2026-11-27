//! noPlayout engine library (`noplayout-core`). The headless `noplayout-engine` binary is a thin
//! wrapper over this crate (E-001, E-002).
//!
//! M0 scope (PLAN §7): single-instance lock, rolling logs, `GET /api/v1/health` on loopback, and
//! launching one output process plus one renderer over the control protocol to prove the process
//! model end to end. Shows, playout, settings and the OGraf APIs arrive in later milestones.

pub mod children;
pub mod client;
pub mod ctl;
pub mod health;
pub mod hostbus;
pub mod http;
pub mod instance;
pub mod library;
pub mod logging;
pub mod paths;
pub mod selftest;
pub mod time;

/// Default HTTP port (IDEA §4.2: web output port, default 8686).
pub const DEFAULT_PORT: u16 = 8686;

/// Header carrying the per-launch internal token on loopback-only routes (E-034).
pub const TOKEN_HEADER: &str = "x-noplayout-token";

/// Environment variable through which a supervising app hands the engine its internal token.
pub const ENV_UI_TOKEN: &str = "NOPLAYOUT_UI_TOKEN";

/// A fresh per-launch token. UUIDv7 carries 74 random bits; it guards loopback-only routes and
/// child control sockets against accidental cross-talk, not against a local attacker.
pub fn new_token() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}
