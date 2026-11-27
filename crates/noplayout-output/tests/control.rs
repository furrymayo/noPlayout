//! Drives the real `noplayout-output` binary over its control socket.

use interprocess::local_socket::{ListenerOptions, prelude::*};
use noplayout_proto::{self as proto, Down, Hello, Up};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn spawn(tag: &str, token: &str) -> (interprocess::local_socket::Listener, Child) {
    let name = format!("noplayout-test-out-{tag}-{}.sock", std::process::id());
    let listener = ListenerOptions::new()
        .name(proto::socket_name(&name).unwrap())
        .create_sync()
        .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_noplayout-output"))
        .args(["--channel", "t"])
        .env(proto::ENV_IPC_NAME, &name)
        .env(proto::ENV_IPC_TOKEN, token)
        .spawn()
        .unwrap();
    (listener, child)
}

fn wait_exit(child: &mut Child, within: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            panic!("noplayout-output did not exit within {within:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn hello_ping_shutdown() {
    let (listener, mut child) = spawn("hps", "tok-1");
    let stream = listener.accept().unwrap();
    let (mut rx, mut tx) = (&stream, &stream);

    let Up::Hello(hello) = proto::read_frame::<_, Up>(&mut rx).unwrap() else {
        panic!("first message must be Hello");
    };
    hello
        .check("tok-1")
        .expect("hello passes the engine's check");
    let Hello::Output(h) = hello else {
        panic!("output process must send an output hello");
    };
    assert_eq!(h.pid, child.id());
    assert!(h.epoch.utc_100ns > 0, "epoch carries wall-clock UTC");

    for seq in [1, 2, 3] {
        proto::write_frame(&mut tx, &Down::Ping { seq }).unwrap();
        match proto::read_frame::<_, Up>(&mut rx).unwrap() {
            Up::Pong(p) => assert_eq!(p.seq, seq),
            other => panic!("expected Pong, got {other:?}"),
        }
    }
    proto::write_frame(&mut tx, &Down::Shutdown).unwrap();
    let status = wait_exit(&mut child, Duration::from_secs(5));
    assert!(status.success(), "clean exit on Shutdown: {status}");
}

#[test]
fn orphaned_output_exits_after_grace() {
    let (listener, mut child) = spawn("orphan", "tok-2");
    let stream = listener.accept().unwrap();
    let _hello = proto::read_frame::<_, Up>(&mut &stream).unwrap();
    let closed = Instant::now();
    drop(stream);
    drop(listener);
    let status = wait_exit(&mut child, Duration::from_secs(10));
    assert_eq!(status.code(), Some(4), "orphan exit status: {status}");
    assert!(
        closed.elapsed() >= Duration::from_millis(2900),
        "held for the 3 s grace, took {:?}",
        closed.elapsed()
    );
}
