//! The engine side of the host protocol (PLAN §4.3, §2.13.2, §2.14): one actor per channel
//! owning a [`state::ChannelState`], the `/ws/host/<channel>` socket handler, and the on-air
//! journal file.

pub mod onair;
pub mod state;
pub mod wire;

#[cfg(test)]
mod tests;

use crate::time::TimeSource;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use state::{ChannelState, CmdError, Command, ConnId, Counters, Dispatched, InstanceView, Out};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use wire::{HostDown, HostUp, PageResult, Register, close};

/// Per-page outbound queue depth. A page this far behind is closed and resyncs by snapshot.
const PAGE_QUEUE: usize = 4096;
/// `TimeSync` period (§4.3).
const TIME_SYNC: Duration = Duration::from_secs(5);
/// A page must send `Register` this soon after connecting.
const REGISTER_TIMEOUT: Duration = Duration::from_secs(5);
/// How often the actor checks per-action deadlines (state.rs `DEADLINE_*`).
const DEADLINE_TICK: Duration = Duration::from_millis(250);

/// Events for the engine's supervisor (renderer kills, injected faults).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusEvent {
    /// A superseded incarnation tried to register: kill that renderer if it is alive (§2.14).
    KillIncarnation { channel: String, inc: u64 },
}

enum Outbound {
    Down(Arc<str>),
    Close(u16, String),
}

enum Msg {
    Command(
        Command,
        oneshot::Sender<Result<(Dispatched, oneshot::Receiver<PageResult>), CmdError>>,
    ),
    Register {
        conn: ConnId,
        loopback: bool,
        reg: Register,
        tx: mpsc::Sender<Outbound>,
        reply: oneshot::Sender<bool>,
    },
    Up(ConnId, HostUp),
    Gone(ConnId),
    NewIncarnation(String, oneshot::Sender<u64>),
    RemoveIncarnation(u64),
    Promote(u64),
    Checkpoint(String),
    SlotClock(Option<crate::time::SlotClock>),
    Inspect(oneshot::Sender<ChannelView>),
}

/// Health/test view of a channel.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelView {
    pub last_seq: u64,
    pub active_inc: Option<u64>,
    pub barrier: u64,
    pub instances: Vec<InstanceView>,
    pub pages: Vec<state::PageView>,
    pub counters: Counters,
}

/// Handle to one channel's actor. Cheap to clone.
#[derive(Clone, Debug)]
pub struct ChannelHandle {
    tx: mpsc::Sender<Msg>,
}

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("{0}")]
    Command(#[from] CmdError),
    #[error("the channel actor stopped")]
    Stopped,
}

impl ChannelHandle {
    async fn ask<T>(&self, f: impl FnOnce(oneshot::Sender<T>) -> Msg) -> Result<T, BusError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(f(tx)).await.map_err(|_| BusError::Stopped)?;
        rx.await.map_err(|_| BusError::Stopped)
    }

    /// Dispatch a command. Returns the dispatch identity and a receiver for the authoritative
    /// page's result, delivered as soon as it arrives (§2.14; state commit stays seq-ordered).
    pub async fn command(
        &self,
        cmd: Command,
    ) -> Result<(Dispatched, oneshot::Receiver<PageResult>), BusError> {
        Ok(self.ask(|r| Msg::Command(cmd, r)).await??)
    }

    pub async fn new_incarnation(&self, token: String) -> Result<u64, BusError> {
        self.ask(|r| Msg::NewIncarnation(token, r)).await
    }
    pub async fn remove_incarnation(&self, inc: u64) {
        let _ = self.tx.send(Msg::RemoveIncarnation(inc)).await;
    }
    pub async fn promote(&self, inc: u64) {
        let _ = self.tx.send(Msg::Promote(inc)).await;
    }
    pub async fn request_checkpoint(&self, instance_id: String) {
        let _ = self.tx.send(Msg::Checkpoint(instance_id)).await;
    }
    /// The channel's output clock, so every action's `ts` carries `slot` (§4.3); `None`
    /// while no output process is up.
    pub async fn set_slot_clock(&self, clock: Option<crate::time::SlotClock>) {
        let _ = self.tx.send(Msg::SlotClock(clock)).await;
    }
    pub async fn inspect(&self) -> Result<ChannelView, BusError> {
        self.ask(Msg::Inspect).await
    }
}

/// All channels of the engine.
#[derive(Clone, Debug)]
pub struct HostBus {
    channels: Arc<HashMap<String, ChannelHandle>>,
    next_conn: Arc<AtomicU64>,
}

impl HostBus {
    /// Start one actor per channel. `onair` receives journal records; `events` supervisor events.
    pub fn start(
        channels: &[&str],
        time: TimeSource,
        onair: Option<onair::OnAirLog>,
        events: Option<mpsc::UnboundedSender<BusEvent>>,
    ) -> Self {
        let mut map = HashMap::new();
        for ch in channels {
            let (tx, rx) = mpsc::channel(1024);
            let actor = Actor {
                state: ChannelState::new(*ch, time.clone()),
                pages: HashMap::new(),
                waiters: HashMap::new(),
                onair: onair.clone(),
                events: events.clone(),
            };
            tokio::spawn(actor.run(rx));
            map.insert((*ch).to_string(), ChannelHandle { tx });
        }
        Self {
            channels: Arc::new(map),
            next_conn: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn channel(&self, ch: &str) -> Option<ChannelHandle> {
        self.channels.get(ch).cloned()
    }

    /// Serve one page socket: `Register` first, then up messages until it closes.
    pub async fn serve_socket(self, channel: String, peer: SocketAddr, mut socket: WebSocket) {
        let first = tokio::time::timeout(REGISTER_TIMEOUT, socket.recv()).await;
        let reg = match first {
            Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<HostUp>(t.as_str()) {
                Ok(HostUp::Register(r)) => r,
                _ => return close_socket(socket, close::BAD_REGISTER, "expected Register").await,
            },
            _ => return close_socket(socket, close::BAD_REGISTER, "expected Register").await,
        };
        let Some(handle) = self.channel(&channel) else {
            return close_socket(socket, close::NO_CHANNEL, "unknown channel").await;
        };
        let conn = self.next_conn.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::channel(PAGE_QUEUE);
        let (reply, accepted) = oneshot::channel();
        let msg = Msg::Register {
            conn,
            loopback: peer.ip().is_loopback(),
            reg,
            tx,
            reply,
        };
        if handle.tx.send(msg).await.is_err() || !accepted.await.unwrap_or(false) {
            // The actor queued the close frame on `tx`; deliver it.
            if let Some(Outbound::Close(code, why)) = rx.recv().await {
                tracing::info!(%peer, code, "host registration refused: {why}");
                return close_socket(socket, code, &why).await;
            }
            return;
        }
        loop {
            tokio::select! {
                out = rx.recv() => match out {
                    Some(Outbound::Down(text)) => {
                        if socket.send(Message::Text(text.as_ref().into())).await.is_err() { break; }
                    }
                    Some(Outbound::Close(code, why)) => { close_socket(socket, code, &why).await; break; }
                    None => break,
                },
                inbound = socket.recv() => match inbound {
                    Some(Ok(Message::Text(t))) => match serde_json::from_str::<HostUp>(t.as_str()) {
                        Ok(m) => { if handle.tx.send(Msg::Up(conn, m)).await.is_err() { break; } }
                        Err(e) => tracing::debug!(conn, "unparseable page message: {e}"),
                    },
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                },
            }
        }
        let _ = handle.tx.send(Msg::Gone(conn)).await;
    }
}

async fn close_socket(mut socket: WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

struct Actor {
    state: ChannelState,
    pages: HashMap<ConnId, mpsc::Sender<Outbound>>,
    waiters: HashMap<u64, oneshot::Sender<PageResult>>,
    onair: Option<onair::OnAirLog>,
    events: Option<mpsc::UnboundedSender<BusEvent>>,
}

impl Actor {
    async fn run(mut self, mut rx: mpsc::Receiver<Msg>) {
        let mut sync = tokio::time::interval(TIME_SYNC);
        sync.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut deadlines = tokio::time::interval(DEADLINE_TICK);
        deadlines.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(m) => self.handle(m),
                    None => break,
                },
                _ = sync.tick() => { let out = self.state.time_sync(); self.apply(out); }
                _ = deadlines.tick() => {
                    let out = self.state.expire(std::time::Instant::now());
                    self.apply(out);
                }
            }
        }
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Command(cmd, reply) => match self.state.dispatch(cmd) {
                Ok((d, out)) => {
                    let (tx, rx) = oneshot::channel();
                    self.waiters.insert(d.seq, tx);
                    self.apply(out);
                    let _ = reply.send(Ok((d, rx)));
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            },
            Msg::Register {
                conn,
                loopback,
                reg,
                tx,
                reply,
            } => match self.state.register(conn, loopback, &reg) {
                Ok(snapshot) => {
                    self.pages.insert(conn, tx);
                    // Snapshot at S and subscription from S + 1 happen in this one step; a
                    // late authoritative page then gets its backlog live (§2.14, ADR 0062).
                    self.send(conn, &HostDown::Snapshot(Box::new(snapshot)));
                    let out = self.state.flush_backlog(conn);
                    self.apply(out);
                    let _ = reply.send(true);
                }
                Err((code, why)) => {
                    if code == close::SUPERSEDED
                        && let (Some(inc), Some(ev)) =
                            (self.state.superseded_kill(&reg), &self.events)
                    {
                        let _ = ev.send(BusEvent::KillIncarnation {
                            channel: self.state.channel().to_string(),
                            inc,
                        });
                    }
                    let _ = tx.try_send(Outbound::Close(code, why));
                    let _ = reply.send(false);
                }
            },
            Msg::Up(conn, m) => {
                let out = self.state.page(conn, m);
                self.apply(out);
            }
            Msg::Gone(conn) => {
                self.pages.remove(&conn);
                self.state.unregister(conn);
            }
            Msg::NewIncarnation(token, reply) => {
                let _ = reply.send(self.state.new_incarnation(token));
            }
            Msg::RemoveIncarnation(inc) => {
                let out = self.state.remove_incarnation(inc);
                self.apply(out);
            }
            Msg::Promote(inc) => {
                let out = self.state.promote(inc);
                self.apply(out);
            }
            Msg::Checkpoint(id) => {
                let out = self.state.request_checkpoint(&id);
                self.apply(out);
            }
            Msg::SlotClock(c) => self.state.set_slot_clock(c),
            Msg::Inspect(reply) => {
                let _ = reply.send(ChannelView {
                    last_seq: self.state.last_seq(),
                    active_inc: self.state.active_inc(),
                    barrier: self.state.barrier(),
                    instances: self.state.instances(),
                    pages: self.state.pages(),
                    counters: self.state.counters.clone(),
                });
            }
        }
    }

    fn send(&mut self, conn: ConnId, msg: &HostDown) {
        let Ok(text) = serde_json::to_string(msg) else {
            return;
        };
        self.send_text(conn, text.into());
    }

    fn send_text(&mut self, conn: ConnId, text: Arc<str>) {
        let Some(tx) = self.pages.get(&conn) else {
            return;
        };
        if tx.try_send(Outbound::Down(text)).is_err() {
            // Too far behind (or gone): close; the page re-registers and resyncs by snapshot.
            if let Some(tx) = self.pages.remove(&conn) {
                let _ = tx.try_send(Outbound::Close(close::TOO_SLOW, "page fell behind".into()));
            }
            self.state.unregister(conn);
        }
    }

    fn apply(&mut self, out: Out) {
        if let Some(log) = &self.onair {
            for r in out.log {
                log.append(r);
            }
        }
        // Serialize each distinct message once (an Action fans out to every page).
        let mut last: Option<(&HostDown, Arc<str>)> = None;
        for (conn, msg) in &out.sends {
            let text = match &last {
                Some((p, t)) if *p == msg => t.clone(),
                _ => match serde_json::to_string(msg) {
                    Ok(t) => {
                        let t: Arc<str> = t.into();
                        last = Some((msg, t.clone()));
                        t
                    }
                    Err(_) => continue,
                },
            };
            self.send_text(*conn, text);
        }
        for (conn, code, why) in out.close {
            if let Some(tx) = self.pages.remove(&conn) {
                let _ = tx.try_send(Outbound::Close(code, why));
            }
        }
        for r in out.results {
            if let Some(w) = self.waiters.remove(&r.seq) {
                let _ = w.send(r);
            }
        }
        for inc in out.kill {
            if let Some(ev) = &self.events {
                let _ = ev.send(BusEvent::KillIncarnation {
                    channel: self.state.channel().to_string(),
                    inc,
                });
            }
        }
        // Waiters whose instance went away never get a result; drop them (callers time out).
        self.waiters.retain(|_, w| !w.is_closed());
    }
}
