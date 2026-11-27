//! §2.14 / §2.13.2 / §4.3 cases on the pure channel state.

use super::onair;
use super::state::*;
use super::wire::*;
use crate::time::TimeSource;
use serde_json::json;

fn ch() -> ChannelState {
    ChannelState::new("1", TimeSource::with_run("run-a".into()))
}

fn reg(role: Role, inc: Option<u64>, token: Option<&str>) -> Register {
    Register {
        channel: "1".into(),
        role,
        host_version: "0.0.1".into(),
        last_seq: None,
        inc,
        token: token.map(Into::into),
    }
}

fn load(s: &mut ChannelState, layer: u32) -> Dispatched {
    s.dispatch(Command::Load {
        layer,
        graphic_url: "/library/x/g.ograf.json".into(),
        data: Some(json!({"name": "A"})),
        cued: false,
    })
    .unwrap()
    .0
}

fn result(d: &Dispatched, seq: u64, step: Option<u64>) -> HostUp {
    HostUp::Result(PageResult {
        seq,
        instance_id: d.instance_id.clone(),
        generation: d.generation,
        status_code: 200,
        status_message: None,
        current_step: step,
        payload: None,
    })
}

fn play(s: &mut ChannelState, layer: u32) -> u64 {
    s.dispatch(Command::Play {
        target: Target::Layer(layer),
        delta: None,
        goto: None,
        skip_animation: false,
    })
    .unwrap()
    .0
    .seq
}

/// An active renderer with its program page registered as `conn`.
fn active(s: &mut ChannelState, conn: ConnId) -> u64 {
    let inc = s.new_incarnation("tok".into());
    s.register(conn, true, &reg(Role::Program, Some(inc), Some("tok")))
        .unwrap();
    s.promote(inc);
    inc
}

#[test]
fn every_action_carries_ts_non_decreasing_in_seq_order_and_fans_out() {
    let mut s = ch();
    s.register(7, false, &reg(Role::WebFill, None, None))
        .unwrap();
    let d = load(&mut s, 10);
    let mut last = None;
    for _ in 0..50 {
        let (_, out) = s
            .dispatch(Command::Update {
                target: Target::Layer(10),
                data: json!({"n": 1}),
                skip_animation: false,
            })
            .unwrap();
        let HostDown::Action(a) = &out.sends[0].1 else {
            panic!()
        };
        assert_eq!(out.sends[0].0, 7);
        assert_eq!(a.instance_id, d.instance_id);
        if let Some((seq, utc, mono)) = last {
            assert!(a.seq == seq + 1 && a.ts.utc_us >= utc && a.ts.mono.us >= mono);
        }
        last = Some((a.seq, a.ts.utc_us, a.ts.mono.us));
    }
    assert_eq!(s.journal(&d.instance_id).unwrap().len(), 50);
}

#[test]
fn reversed_result_completion_commits_state_in_seq_order() {
    let mut s = ch();
    active(&mut s, 1);
    let d = load(&mut s, 10);
    s.page(1, result(&d, d.seq, None));
    let p1 = play(&mut s, 10);
    let p2 = play(&mut s, 10);
    // play #2 resolves first: its result is reported at once but state waits for #1.
    let out = s.page(1, result(&d, p2, Some(1)));
    assert_eq!(out.results.len(), 1);
    let v = &s.instances()[0];
    assert_eq!((v.current_step, v.pending), (None, 2));
    s.page(1, result(&d, p1, Some(0)));
    let v = &s.instances()[0];
    assert_eq!((v.current_step, v.at_end, v.pending), (Some(1), false, 0));
    assert_eq!(v.phase, Phase::Loaded);
}

#[test]
fn current_step_undefined_means_at_end() {
    let mut s = ch();
    active(&mut s, 1);
    let d = load(&mut s, 10);
    s.page(1, result(&d, d.seq, None));
    let p = play(&mut s, 10);
    s.page(1, result(&d, p, None));
    assert!(s.instances()[0].at_end);
}

#[test]
fn late_result_after_dispose_is_discarded_and_never_recreates() {
    let mut s = ch();
    active(&mut s, 1);
    let d = load(&mut s, 10);
    let p = play(&mut s, 10);
    s.dispatch(Command::Dispose {
        target: Target::Layer(10),
    })
    .unwrap();
    assert!(s.instances().is_empty());
    let out = s.page(1, result(&d, p, Some(0)));
    assert!(out.results.is_empty() && s.instances().is_empty());
    assert_eq!(s.counters.dropped_results, 1);
}

#[test]
fn program_registration_requires_loopback_token_and_live_incarnation() {
    let mut s = ch();
    let inc = s.new_incarnation("secret".into());
    let deny = |r: Result<Snapshot, (u16, String)>| r.unwrap_err().0;
    assert_eq!(
        deny(s.register(1, false, &reg(Role::Program, Some(inc), Some("secret")))),
        close::FORBIDDEN
    );
    assert_eq!(
        deny(s.register(2, true, &reg(Role::Program, None, None))),
        close::FORBIDDEN
    );
    assert_eq!(
        deny(s.register(3, true, &reg(Role::Program, Some(inc), Some("wrong!")))),
        close::FORBIDDEN
    );
    assert_eq!(
        deny(s.register(4, true, &reg(Role::Program, Some(inc + 5), Some("secret")))),
        close::FORBIDDEN
    );
    s.register(5, true, &reg(Role::Program, Some(inc), Some("secret")))
        .unwrap();
    assert_eq!(
        deny(s.register(6, true, &reg(Role::Program, Some(inc), Some("secret")))),
        close::FORBIDDEN,
        "one program page per incarnation"
    );
    let mut wrong = reg(Role::WebKey, None, None);
    wrong.channel = "2".into();
    assert_eq!(deny(s.register(7, false, &wrong)), close::NO_CHANNEL);
    // Mirror roles need no token, from anywhere (access scope arrives with §2.12).
    s.register(8, false, &reg(Role::Preview, None, None))
        .unwrap();
    assert_eq!(s.counters.refused_registrations, 6);
}

#[test]
fn old_page_reconnecting_after_replacement_is_superseded() {
    let mut s = ch();
    let old = active(&mut s, 1);
    let new = s.new_incarnation("tok2".into());
    let out = s.remove_incarnation(old);
    assert_eq!(
        out.close,
        vec![(1, close::SUPERSEDED, "renderer superseded".into())]
    );
    s.promote(new);
    let r = reg(Role::Program, Some(old), Some("tok"));
    assert_eq!(s.register(9, true, &r).unwrap_err().0, close::SUPERSEDED);
    assert_eq!(s.superseded_kill(&r), Some(old));
}

#[test]
fn promotion_takes_held_standby_results_for_pending_seqs() {
    let mut s = ch();
    let a = active(&mut s, 1);
    let b = s.new_incarnation("tb".into());
    s.register(2, true, &reg(Role::Program, Some(b), Some("tb")))
        .unwrap();
    let d = load(&mut s, 10);
    s.page(1, result(&d, d.seq, None));
    s.page(2, result(&d, d.seq, None));
    let p = play(&mut s, 10);
    // The active dies before answering; the standby already executed it.
    s.page(2, result(&d, p, Some(0)));
    assert_eq!(s.instances()[0].pending, 1, "standby results never commit");
    s.remove_incarnation(a);
    let out = s.promote(b);
    assert_eq!(s.barrier(), p);
    assert_eq!(out.results.len(), 1);
    assert_eq!(s.instances()[0].current_step, Some(0));
    // A late answer from the superseded incarnation's connection is ignored.
    assert!(s.page(1, result(&d, p, Some(5))).results.is_empty());
}

#[test]
fn mirror_disagreement_is_counted() {
    let mut s = ch();
    active(&mut s, 1);
    s.register(3, false, &reg(Role::WebProgram, None, None))
        .unwrap();
    let d = load(&mut s, 10);
    s.page(1, result(&d, d.seq, None));
    let p = play(&mut s, 10);
    s.page(1, result(&d, p, Some(2)));
    s.page(3, result(&d, p, Some(1)));
    assert_eq!(s.counters.mirror_divergence, 1);
}

#[test]
fn snapshot_carries_original_ts_cued_slot_and_checkpoint_tail_only() {
    let mut s = ch();
    active(&mut s, 1);
    let d = load(&mut s, 10);
    let u = s
        .dispatch(Command::Update {
            target: Target::Layer(10),
            data: json!({"name": "B"}),
            skip_animation: false,
        })
        .unwrap()
        .1;
    let HostDown::Action(ua) = &u.sends[0].1 else {
        panic!()
    };
    let p = play(&mut s, 10);
    s.dispatch(Command::Load {
        layer: 20,
        graphic_url: "/library/y/c.ograf.json".into(),
        data: None,
        cued: true,
    })
    .unwrap();
    let snap = s.snapshot();
    assert_eq!(snap.last_seq, s.last_seq());
    assert_eq!(snap.layers.len(), 1);
    let l = &snap.layers[0];
    assert_eq!(l.data_at_load, json!({"name": "A"}));
    assert_eq!(l.journal.len(), 2);
    assert_eq!(l.journal[0].ts, ua.ts, "journal keeps the original ts");
    assert_eq!(l.journal[0].replay, Some(true));
    assert_eq!(snap.cued.as_ref().unwrap().layer, 20);
    assert_eq!(
        s.instances().iter().find(|i| !i.cued).unwrap().data,
        json!({"name": "B"})
    );
    // A checkpoint at the update trims the snapshot journal to the tail after it.
    let out = s.page(
        1,
        HostUp::CheckpointResult {
            instance_id: d.instance_id.clone(),
            generation: d.generation,
            frontier_seq: ua.seq,
            cut_ts: s.now(),
            state: json!({"clock": 1}),
        },
    );
    assert_eq!(out.log.len(), 1);
    let l = &s.snapshot().layers[0];
    assert_eq!(l.journal.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![p]);
    assert_eq!(l.checkpoint.as_ref().unwrap().frontier_seq, ua.seq);
    assert_eq!(
        s.journal(&d.instance_id).unwrap().len(),
        2,
        "the head stays"
    );
}

#[test]
fn playing_a_cue_promotes_it_and_retires_the_layer_holder() {
    let mut s = ch();
    load(&mut s, 10);
    s.dispatch(Command::Load {
        layer: 10,
        graphic_url: "/library/y/c.ograf.json".into(),
        data: None,
        cued: true,
    })
    .unwrap();
    let (_, out) = s
        .dispatch(Command::Play {
            target: Target::Cued,
            delta: None,
            goto: None,
            skip_animation: false,
        })
        .unwrap();
    assert!(matches!(out.log[0], OnAirRecord::Dispose { .. }));
    let v = s.instances();
    assert_eq!(v.len(), 1);
    assert!(!v[0].cued && v[0].layer == 10);
    assert!(matches!(
        s.dispatch(Command::Stop {
            target: Target::Cued,
            skip_animation: false
        }),
        Err(CmdError::NoInstance(Target::Cued))
    ));
}

#[test]
fn onair_file_round_trips_and_drops_disposed_instances() {
    let dir = std::env::temp_dir().join(format!("np-onair-{}", crate::new_token()));
    let path = dir.join("shows/dev.onair.jsonl");
    let (log, thread) = onair::OnAirLog::open(&path).unwrap();
    // Each state step returns its journal records; feed them to the writer.
    let mut t = ch();
    active(&mut t, 1);
    let mut all = vec![];
    let (d2, o) = t
        .dispatch(Command::Load {
            layer: 10,
            graphic_url: "/library/x/g.ograf.json".into(),
            data: Some(json!({"name": "A"})),
            cued: false,
        })
        .unwrap();
    all.extend(o.log);
    all.extend(t.page(1, result(&d2, d2.seq, None)).log);
    let (pd, o) = t
        .dispatch(Command::Play {
            target: Target::Layer(10),
            delta: None,
            goto: None,
            skip_animation: false,
        })
        .unwrap();
    all.extend(o.log);
    all.extend(t.page(1, result(&d2, pd.seq, Some(0))).log);
    all.extend(
        t.dispatch(Command::Load {
            layer: 11,
            graphic_url: "u".into(),
            data: None,
            cued: false,
        })
        .unwrap()
        .1
        .log,
    );
    all.extend(
        t.dispatch(Command::Dispose {
            target: Target::Layer(11),
        })
        .unwrap()
        .1
        .log,
    );
    for r in all {
        log.append(r);
    }
    drop(log);
    thread.join().unwrap();
    let back = onair::read(&path).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].instance_id, d2.instance_id);
    assert_eq!(back[0].journal.len(), 1);
    assert_eq!(back[0].journal[0].1.as_ref().unwrap().current_step, Some(0));
    // A torn final line is tolerated.
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"{\"kind\":\"act"))
        .unwrap();
    assert_eq!(onair::read(&path).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn token_compare_is_exact() {
    assert!(token_eq("abc", "abc"));
    assert!(!token_eq("abc", "abd"));
    assert!(!token_eq("abc", "abcd"));
}
