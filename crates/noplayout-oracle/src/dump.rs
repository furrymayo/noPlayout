//! Reading an output-process paint dump (`DumpSink`): `<plane>.jsonl` lines name each frame's
//! file and timecode; the pixels are decoded here. The `k`, paint and `held` fields the dump
//! also records are deliberately not read (the oracle judges pixels and timecodes only).

use crate::analyze::Observed;
use crate::decode::{self, Plane};
use crate::layout;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::Path;

struct Line {
    file: String,
    tc: i64,
    width: u32,
    height: u32,
    bpp: u32,
    crops: Vec<[u32; 4]>,
}

fn lines(dir: &Path, plane: &str) -> io::Result<Vec<Line>> {
    let f = std::fs::File::open(dir.join(format!("{plane}.jsonl")))?;
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{plane}: {what}"));
    let mut out = Vec::new();
    for l in io::BufReader::new(f).lines() {
        let l = l?;
        if l.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&l).map_err(|e| bad(&e.to_string()))?;
        let num = |k: &str| v[k].as_i64().ok_or_else(|| bad(&format!("no {k}")));
        let crops = v["crop"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        let r: Vec<u32> = r
                            .as_array()?
                            .iter()
                            .filter_map(|x| x.as_u64().map(|x| x as u32))
                            .collect();
                        <[u32; 4]>::try_from(r).ok()
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Line {
            file: format!("{:08}.raw", num("k")?),
            tc: num("tc")?,
            width: num("width")? as u32,
            height: num("height")? as u32,
            bpp: num("bpp")? as u32,
            crops,
        });
    }
    Ok(out)
}

/// Decode a dump directory: FILL frames in order (with the KEY of the same timecode) and the
/// KEY plane's timecodes in order.
pub fn read(dir: &Path) -> io::Result<(Vec<Observed>, Vec<i64>)> {
    let fills = lines(dir, "fill")?;
    let keys = lines(dir, "key").unwrap_or_default();
    let mut key_by_tc = HashMap::new();
    for k in &keys {
        let data = std::fs::read(dir.join("key").join(&k.file))?;
        let p = Plane {
            width: k.width,
            height: k.height,
            bpp: k.bpp,
            data: &data,
            crops: &k.crops,
        };
        key_by_tc.insert(k.tc, (decode::read_alpha(&p), decode::ramp_errors(&p)));
    }
    let mut frames = Vec::with_capacity(fills.len());
    for f in &fills {
        let data = std::fs::read(dir.join("fill").join(&f.file))?;
        let p = Plane {
            width: f.width,
            height: f.height,
            bpp: f.bpp,
            data: &data,
            crops: &f.crops,
        };
        frames.push(Observed {
            tc_100ns: f.tc,
            a: decode::read_patch(&p, layout::patch_a()),
            b: decode::read_patch(&p, layout::patch_b(f.width, f.height)),
            key: key_by_tc.get(&f.tc).copied(),
        });
    }
    Ok((frames, keys.iter().map(|k| k.tc).collect()))
}
