//! M1 task 9: each graphic, unmodified, through load → play (each step) → update → each custom
//! action → stop → dispose in the real CEF renderer. Asserted: every result arrives with
//! `statusCode` 200; zero console errors; `currentStep` semantics (O6/O7: a result's
//! `currentStep` is `step − 1` for the `step`-th `delta: 1`, absent means "at end", and the
//! engine's mirror agrees); `stepCount: -1` graphics are played until they report the end.

use crate::engine::Engine;
use serde_json::{Map, Value, json};
use std::path::Path;

/// The most `delta: 1` plays a `stepCount: -1` graphic gets before it must have reported its end.
const UNBOUNDED_CAP: usize = 32;

pub struct Outcome {
    pub name: String,
    pub actions: usize,
    pub failures: Vec<String>,
}

/// A value that satisfies a JSON schema: its `default`, else the first `enum` value, else a
/// placeholder of the right type (objects recursively).
pub fn sample(schema: &Value) -> Value {
    if let Some(d) = schema.get("default") {
        return d.clone();
    }
    if let Some(first) = schema.get("enum").and_then(|e| e.get(0)) {
        return first.clone();
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") | None if schema.get("properties").is_some() => {
            let mut m = Map::new();
            if let Some(props) = schema["properties"].as_object() {
                for (k, s) in props {
                    m.insert(k.clone(), sample(s));
                }
            }
            Value::Object(m)
        }
        Some("integer" | "number") => schema.get("minimum").cloned().unwrap_or(json!(1)),
        Some("boolean") => json!(false),
        Some("array") => json!([]),
        Some("string") => json!(""),
        _ => json!({}),
    }
}

fn manifest(dir: &Path) -> Result<Value, String> {
    let m = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with(".ograf.json"))
        .ok_or_else(|| format!("{}: no .ograf.json", dir.display()))?;
    let text = std::fs::read_to_string(&m).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", m.display()))
}

/// The instance on `layer` in the engine's mirror (`inspect`).
fn mirror(e: &Engine, layer: &str) -> Option<Value> {
    let v = e.ctl(&["inspect"]).ok()?;
    v["instances"]
        .as_array()?
        .iter()
        .find(|i| i["layer"].as_u64().is_some_and(|l| l.to_string() == layer))
        .cloned()
}

pub fn run(e: &Engine, dir: &Path, layer: u32, load_data: Option<&Value>) -> Outcome {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut o = Outcome {
        name,
        actions: 0,
        failures: Vec::new(),
    };
    let from = e.log_len();
    if let Err(f) = sequence(e, dir, &layer.to_string(), load_data, &mut o) {
        o.failures.push(f);
    }
    for c in e.console_errors(from) {
        o.failures.push(format!("console error: {c}"));
    }
    if !o.failures.is_empty() {
        o.failures.push(e.diagnose());
    }
    o
}

fn sequence(
    e: &Engine,
    dir: &Path,
    layer: &str,
    load_data: Option<&Value>,
    o: &mut Outcome,
) -> Result<(), String> {
    let m = manifest(dir)?;
    let url = e.import(dir)?;
    // One action: send, require a result with statusCode 200, return it.
    let act = |o: &mut Outcome, args: &[&str]| -> Result<Value, String> {
        o.actions += 1;
        let v = e.ctl(args)?;
        let r = &v["result"];
        if r.is_null() {
            return Err(format!("{}: no result (timed out)", args.join(" ")));
        }
        if r["statusCode"] != json!(200) {
            o.failures.push(format!("{}: {r}", args.join(" ")));
        }
        Ok(r.clone())
    };
    let data;
    let mut load = vec!["load", layer, url.as_str()];
    if let Some(d) = load_data {
        data = d.to_string();
        load.extend(["--data", data.as_str()]);
    }
    act(o, &load)?;

    // Steps (O6/O7).
    let steps = m["stepCount"].as_i64().unwrap_or(1);
    let plays = if steps < 0 {
        UNBOUNDED_CAP
    } else {
        steps as usize
    };
    let mut ended = false;
    for i in 0..plays {
        let r = act(o, &["play", layer])?;
        let step = r.get("currentStep").and_then(Value::as_i64);
        match step {
            Some(s) if s != i as i64 => o
                .failures
                .push(format!("play #{}: currentStep {s}, expected {i}", i + 1)),
            Some(s) if steps > 0 && s >= steps => o.failures.push(format!(
                "play #{}: currentStep {s} ≥ stepCount {steps}",
                i + 1
            )),
            _ => {}
        }
        // The mirror must agree with the result, absent = at end (O7).
        if let Some(mi) = mirror(e, layer) {
            let want = step.map_or(Value::Null, |s| json!(s));
            if mi["currentStep"] != want || mi["atEnd"] != json!(step.is_none()) {
                o.failures.push(format!(
                    "play #{}: mirror currentStep {} atEnd {}, result {want}",
                    i + 1,
                    mi["currentStep"],
                    mi["atEnd"]
                ));
            }
        }
        if step.is_none() {
            ended = true;
            if steps > 0 && i + 1 < plays {
                o.failures
                    .push(format!("play #{}: at end before stepCount {steps}", i + 1));
            }
            break;
        }
    }
    if steps < 0 && !ended {
        o.failures.push(format!(
            "stepCount -1: no end reported within {UNBOUNDED_CAP} plays"
        ));
    }

    // Update with schema-valid data, then every custom action with schema-valid params.
    let upd = m
        .get("schema")
        .map(sample)
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
        .to_string();
    act(o, &["update", layer, &upd])?;
    if let Some(list) = m["customActions"].as_array() {
        for a in list {
            let id = a["id"].as_str().unwrap_or_default();
            let params = a
                .get("schema")
                .filter(|s| !s.is_null())
                .map(sample)
                .unwrap_or_else(|| json!({}))
                .to_string();
            act(o, &["custom", layer, id, &params])?;
        }
    }
    act(o, &["stop", layer])?;

    // Dispose: the instance must leave the mirror.
    o.actions += 1;
    e.ctl(&["dispose", layer])?;
    for _ in 0..50 {
        if mirror(e, layer).is_none() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("dispose: the instance is still in the engine's mirror after 5 s".into())
}
