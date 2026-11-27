//! Dev control (`noplayout-engine ctl …`, PLAN §7 M1 task 3): drives the host bus before the
//! rundown exists (M4). Loopback-only, behind the per-launch internal token (E-034).

use crate::hostbus::state::Command;
use serde::{Deserialize, Serialize};

/// One dev-control request (`POST /api/v1/internal/ctl`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Ctl {
    /// Import a package folder or `.zip` on the engine host into the library.
    Import { path: String },
    /// Sequence one OGraf action on a channel.
    Command { channel: String, command: Command },
    /// Fault K1: kill the channel's active renderer process with no grace.
    KillRenderer { channel: String },
    /// Fault K3: hang the active program page's main thread for `ms`.
    Hang { channel: String, ms: u32 },
    /// The channel's current state (instances, seq, authority, counters).
    Inspect { channel: String },
}

/// Faults the engine's supervisor carries out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    KillRenderer { channel: String },
    Hang { channel: String, ms: u32 },
}

const USAGE: &str = "usage: noplayout-engine ctl [--port N] <op> [args]
  import <folder|zip>
  load <layer> <manifest-url> [--data JSON] [--cued]
  play <layer|cued> [--delta N | --goto N] [--skip]
  update <layer|cued> <JSON> [--skip]
  stop <layer|cued> [--skip]
  custom <layer|cued> <id> [JSON] [--skip]
  dispose <layer|cued>
  kill-renderer
  hang <ms>
  inspect
  (--channel C selects the channel, default 1; the token comes from NOPLAYOUT_UI_TOKEN)";

/// Parse `ctl` arguments (after `ctl`). Returns the port override and the request.
pub fn parse(args: &[String]) -> Result<(Option<u16>, Ctl), String> {
    use crate::hostbus::state::Target;
    let mut port = None;
    let mut channel = "1".to_string();
    let mut flags = std::collections::BTreeMap::<&str, Option<String>>::new();
    let mut pos: Vec<&str> = vec![];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => {
                let v = it.next().ok_or("--port needs a value")?;
                port = Some(v.parse().map_err(|_| format!("--port {v}: not a port"))?);
            }
            "--channel" => channel = it.next().ok_or("--channel needs a value")?.clone(),
            "--data" | "--delta" | "--goto" => {
                let v = it.next().ok_or(format!("{a} needs a value"))?;
                flags.insert(a.as_str(), Some(v.clone()));
            }
            "--cued" | "--skip" => {
                flags.insert(a.as_str(), None);
            }
            "--help" | "-h" => return Err(USAGE.into()),
            s if s.starts_with("--") => return Err(format!("unknown flag {s}\n{USAGE}")),
            s => pos.push(s),
        }
    }
    let json =
        |s: &str| serde_json::from_str::<serde_json::Value>(s).map_err(|e| format!("{s}: {e}"));
    let target = |s: Option<&&str>| -> Result<Target, String> {
        match s {
            Some(&"cued") => Ok(Target::Cued),
            Some(l) => l
                .parse()
                .map(Target::Layer)
                .map_err(|_| format!("{l}: not a layer")),
            None => Err(format!("missing layer\n{USAGE}")),
        }
    };
    let skip = flags.contains_key("--skip");
    let num = |k: &str| -> Result<Option<i64>, String> {
        flags
            .get(k)
            .cloned()
            .flatten()
            .map(|v| v.parse().map_err(|_| format!("{k} {v}: not a number")))
            .transpose()
    };
    let cmd = |command| Ctl::Command {
        channel: channel.clone(),
        command,
    };
    let op = pos.first().copied().ok_or(USAGE)?;
    let req = match op {
        "import" => Ctl::Import {
            path: pos.get(1).ok_or(USAGE)?.to_string(),
        },
        "load" => cmd(Command::Load {
            layer: pos
                .get(1)
                .and_then(|l| l.parse().ok())
                .ok_or(format!("load needs a layer number\n{USAGE}"))?,
            graphic_url: pos.get(2).ok_or(USAGE)?.to_string(),
            data: flags
                .get("--data")
                .cloned()
                .flatten()
                .map(|d| json(&d))
                .transpose()?,
            cued: flags.contains_key("--cued"),
        }),
        "play" => cmd(Command::Play {
            target: target(pos.get(1))?,
            delta: num("--delta")?,
            goto: num("--goto")?
                .map(|g| u64::try_from(g).map_err(|_| "--goto must be ≥ 0".to_string()))
                .transpose()?,
            skip_animation: skip,
        }),
        "update" => cmd(Command::Update {
            target: target(pos.get(1))?,
            data: json(pos.get(2).ok_or(USAGE)?)?,
            skip_animation: skip,
        }),
        "stop" => cmd(Command::Stop {
            target: target(pos.get(1))?,
            skip_animation: skip,
        }),
        "custom" => cmd(Command::Custom {
            target: target(pos.get(1))?,
            id: pos.get(2).ok_or(USAGE)?.to_string(),
            payload: pos.get(3).map(|p| json(p)).transpose()?,
            skip_animation: skip,
        }),
        "dispose" => cmd(Command::Dispose {
            target: target(pos.get(1))?,
        }),
        "kill-renderer" => Ctl::KillRenderer { channel },
        "hang" => Ctl::Hang {
            channel,
            ms: pos
                .get(1)
                .and_then(|m| m.parse().ok())
                .ok_or(format!("hang needs milliseconds\n{USAGE}"))?,
        },
        "inspect" => Ctl::Inspect { channel },
        other => return Err(format!("unknown ctl op {other}\n{USAGE}")),
    };
    Ok((port, req))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hostbus::state::Target;

    fn p(s: &str) -> Result<(Option<u16>, Ctl), String> {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn parses_the_dev_cli() {
        assert_eq!(
            p("--port 9001 load 10 /library/h/x.ograf.json --cued").unwrap(),
            (
                Some(9001),
                Ctl::Command {
                    channel: "1".into(),
                    command: Command::Load {
                        layer: 10,
                        graphic_url: "/library/h/x.ograf.json".into(),
                        data: None,
                        cued: true
                    }
                }
            )
        );
        assert!(matches!(
            p("play cued --goto 2 --skip").unwrap().1,
            Ctl::Command {
                command: Command::Play {
                    target: Target::Cued,
                    goto: Some(2),
                    skip_animation: true,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            p("--channel 2 custom 5 addPoints {\"team\":\"home\"}").unwrap().1,
            Ctl::Command { channel, command: Command::Custom { target: Target::Layer(5), .. } } if channel == "2"
        ));
        assert_eq!(
            p("hang 3000").unwrap().1,
            Ctl::Hang {
                channel: "1".into(),
                ms: 3000
            }
        );
        assert!(p("load x url").is_err());
        assert!(p("update 1 notjson").is_err());
        assert!(p("frobnicate").is_err());
        assert!(p("").is_err());
    }

    #[test]
    fn wire_shape_is_tagged() {
        let v = serde_json::to_value(Ctl::KillRenderer {
            channel: "1".into(),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"op": "kill-renderer", "channel": "1"})
        );
        let v = serde_json::to_value(p("stop 3").unwrap().1).unwrap();
        assert_eq!(v["command"]["cmd"], "stop");
        assert_eq!(v["command"]["target"], serde_json::json!({"layer": 3}));
    }
}
