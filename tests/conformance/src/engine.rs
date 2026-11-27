//! One engine (with its output process and the real CEF renderer) for the conformance and pixel
//! runs, driven over the engine's own loopback control API exactly as `noplayout-engine ctl` is.

use noplayout_core::{client, ctl};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TOKEN: &str = "conformance";

pub struct Engine {
    child: Child,
    port: u16,
    pub log: PathBuf,
}

pub struct Opts<'a> {
    pub bin: &'a Path,
    pub host: &'a Path,
    pub port: u16,
    pub dir: PathBuf,
    /// Extra environment for the engine (inherited by output and renderer).
    pub env: Vec<(String, String)>,
}

impl Engine {
    pub fn start(o: Opts) -> Result<Engine, String> {
        let _ = std::fs::remove_dir_all(&o.dir);
        std::fs::create_dir_all(&o.dir).map_err(|e| format!("{}: {e}", o.dir.display()))?;
        let log = o.dir.join("engine.log");
        let out = std::fs::File::create(&log).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let exe = o
            .bin
            .join(format!("noplayout-engine{}", std::env::consts::EXE_SUFFIX));
        let mut cmd = Command::new(&exe);
        cmd.arg("--port")
            .arg(o.port.to_string())
            .env("NOPLAYOUT_DATA_DIR", o.dir.join("data"))
            .env("NOPLAYOUT_UI_TOKEN", TOKEN)
            .env("NOPLAYOUT_HOST_DIR", o.host)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err);
        for (k, v) in &o.env {
            cmd.env(k, v);
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
        let mut e = Engine {
            child,
            port: o.port,
            log,
        };
        e.wait_ready()?;
        Ok(e)
    }

    /// Ready = the program page has registered on the host bus (a command sent earlier is lost,
    /// STATUS.md), plus one heartbeat.
    fn wait_ready(&mut self) -> Result<(), String> {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(30) {
            if let Ok(Some(st)) = self.child.try_wait() {
                return Err(format!(
                    "engine exited during start-up ({st}); see {}",
                    self.log.display()
                ));
            }
            if let Ok(v) = self.ctl(&["inspect"])
                && v["pages"].as_array().is_some_and(|p| !p.is_empty())
            {
                std::thread::sleep(Duration::from_millis(500));
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!(
            "no program page after 30 s; see {}",
            self.log.display()
        ))
    }

    pub fn ctl(&self, args: &[&str]) -> Result<Value, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (_, req) = ctl::parse(&args)?;
        let body = client::post_ctl(self.port, TOKEN, &req, Duration::from_secs(20))
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&body).map_err(|e| format!("{body}: {e}"))
    }

    /// `import` → the manifest URL.
    pub fn import(&self, dir: &Path) -> Result<String, String> {
        let v = self.ctl(&["import", &dir.to_string_lossy()])?;
        v["manifestUrl"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("import {}: {v}", dir.display()))
    }

    /// Bytes of the engine log so far, to cut out one graphic's console messages.
    pub fn log_len(&self) -> u64 {
        std::fs::metadata(&self.log).map(|m| m.len()).unwrap_or(0)
    }

    /// Console errors the program page logged since `from` (a [`Engine::log_len`]).
    pub fn console_errors(&self, from: u64) -> Vec<String> {
        let text = std::fs::read(&self.log).unwrap_or_default();
        let from = (from as usize).min(text.len());
        String::from_utf8_lossy(&text[from..])
            .lines()
            .filter_map(|l| l.split_once("console error: ").map(|(_, m)| m.to_owned()))
            .collect()
    }

    pub fn stop(mut self) -> Result<(), String> {
        let _ = client::post_shutdown(self.port, TOKEN, Duration::from_secs(5));
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(15) {
            if let Ok(Some(st)) = self.child.try_wait() {
                return if st.success() {
                    Ok(())
                } else {
                    Err(format!("engine exited with {st}"))
                };
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        Err("engine did not stop within 15 s".into())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = client::post_shutdown(self.port, TOKEN, Duration::from_secs(2));
            std::thread::sleep(Duration::from_secs(2));
            let _ = self.child.kill();
        }
    }
}
