//! M1 task 10: pixel tests. A graphic is played in the real CEF renderer, the output process
//! dumps settled slots (`NOPLAYOUT_DUMP_AFTER`), and the last slot's FILL (straight BGRA) and
//! KEY are joined into a straight-alpha RGBA image and compared with a PNG golden. The
//! alpha-ramp golden (`goldens/shape/`) serves every OS; `gradient-shape` and the Basketball
//! goldens are per OS (`goldens/<os>-<arch>/`, ADR 0064). Colour is compared only where alpha ≥ 16 in both images
//! (below that, un-premultiplied colour is quantisation noise). Also done-when (b): the
//! alpha-ramp's 16 patches within ±1 LSB of 17·i, and the scorebug's paint sequence animates
//! with a transparent background and antialiased edges.

use crate::engine::{Engine, Opts};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const W: usize = 1920;
pub const H: usize = 1080;

/// Straight-alpha RGBA, `W × H`.
pub struct Image(pub Vec<u8>);

pub struct Capture {
    pub frames: Vec<Image>,
}

pub struct Case<'a> {
    pub name: &'a str,
    pub dir: PathBuf,
    pub data: Option<Value>,
    /// Sent as an `update` after `play` (before the settle), e.g. a changed score.
    pub update: Option<Value>,
    /// How long after `play` the dump starts (lets entry animations finish).
    pub settle: Duration,
    pub frames: usize,
    /// Open the dump gate after `load`, before `play` (to capture an entry animation: `play`
    /// resolves when the animation has finished).
    pub before_play: bool,
}

/// Run one case in a fresh engine (a dump ends the output's pacing, so one capture per engine).
pub fn capture(bin: &Path, host: &Path, work: &Path, c: &Case) -> Result<Capture, String> {
    let dir = work.join(format!("pixels-{}", c.name));
    let gate = dir.join("go");
    let dump = dir.join("dump");
    let e = Engine::start(Opts {
        bin,
        host,
        port: 18731,
        dir: dir.clone(),
        env: vec![
            (
                "NOPLAYOUT_DUMP_PAINTS".into(),
                dump.to_string_lossy().into(),
            ),
            ("NOPLAYOUT_DUMP_FRAMES".into(), c.frames.to_string()),
            ("NOPLAYOUT_DUMP_AFTER".into(), gate.to_string_lossy().into()),
        ],
    })?;
    let url = e.import(&c.dir)?;
    let data = c.data.as_ref().map(Value::to_string);
    let mut load = vec!["load", "10", url.as_str()];
    if let Some(d) = &data {
        load.extend(["--data", d.as_str()]);
    }
    let update = c.update.as_ref().map(Value::to_string);
    let mut steps = vec![load, vec!["play", "10"]];
    if let Some(u) = &update {
        steps.push(vec!["update", "10", u.as_str()]);
    }
    for args in steps {
        if c.before_play && args[0] == "play" {
            std::thread::sleep(c.settle);
            std::fs::write(&gate, b"").map_err(|e| e.to_string())?;
        }
        let v = e.ctl(&args)?;
        if v["result"]["statusCode"] != serde_json::json!(200) {
            return Err(format!("{}: {v}", args.join(" ")));
        }
    }
    if !c.before_play {
        std::thread::sleep(c.settle);
        std::fs::write(&gate, b"").map_err(|e| e.to_string())?;
    }
    let t0 = Instant::now();
    let lines = |p: &str| {
        std::fs::read_to_string(dump.join(format!("{p}.jsonl")))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    };
    // The dump offers `c.frames` slots; a lane still writing the previous one drops a slot, as
    // a real-time output must (a hosted runner's disk can't always take 1080p at 59.94). So
    // accept the dump once it has stopped growing with at least 3/4 of the slots on both planes.
    let enough = (c.frames * 3 / 4).max(1);
    let (mut last, mut since) = ((0, 0), Instant::now());
    loop {
        let now = (lines("fill"), lines("key"));
        if now.0 >= c.frames && now.1 >= c.frames {
            break;
        }
        if now != last {
            (last, since) = (now, Instant::now());
        } else if since.elapsed() > Duration::from_secs(3) && now.0 >= enough && now.1 >= enough {
            break;
        }
        if t0.elapsed() > Duration::from_secs(30) {
            return Err(format!(
                "{}: dump incomplete after 30 s ({} fill, {} key lines); {}",
                c.name,
                now.0,
                now.1,
                e.diagnose()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(200));
    let errors = e.console_errors(0);
    e.stop()?;
    if !errors.is_empty() {
        return Err(format!("{}: console errors: {errors:?}", c.name));
    }
    let mut frames = Vec::new();
    let index = std::fs::read_to_string(dump.join("fill.jsonl")).map_err(|e| e.to_string())?;
    for line in index.lines() {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let k = v["k"].as_u64().ok_or("dump line without k")?;
        let (w, h) = (v["width"].as_u64(), v["height"].as_u64());
        if (w, h) != (Some(W as u64), Some(H as u64)) {
            return Err(format!(
                "{}: slot {k} is {w:?}×{h:?}, expected {W}×{H}",
                c.name
            ));
        }
        let read = |p: &str| std::fs::read(dump.join(p).join(format!("{k:08}.raw")));
        let fill = read("fill").map_err(|e| e.to_string())?;
        // A slot the KEY lane dropped (see above) is skipped, never paired with another.
        let Ok(key) = read("key") else { continue };
        if fill.len() != W * H * 4 || key.len() != W * H {
            return Err(format!("{}: slot {k}: short dump", c.name));
        }
        let mut rgba = vec![0u8; W * H * 4];
        for i in 0..W * H {
            rgba[i * 4] = fill[i * 4 + 2];
            rgba[i * 4 + 1] = fill[i * 4 + 1];
            rgba[i * 4 + 2] = fill[i * 4];
            rgba[i * 4 + 3] = key[i];
        }
        frames.push(Image(rgba));
    }
    let _ = std::fs::remove_dir_all(&dump);
    Ok(Capture { frames })
}

pub fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), W as u32, H as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&img.0).map_err(|e| e.to_string())
}

pub fn read_png(path: &Path) -> Result<Image, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut r = png::Decoder::new(std::io::BufReader::new(f))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("png too large")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if (info.width, info.height, info.color_type) != (W as u32, H as u32, png::ColorType::Rgba) {
        return Err(format!("{}: not a {W}×{H} RGBA PNG", path.display()));
    }
    buf.truncate(info.buffer_size());
    Ok(Image(buf))
}

/// Pixels outside ±`tol` and the largest difference seen. Colour is compared premultiplied
/// (c·α/255), as a keyer composites it: un-premultiplied colour under low alpha only carries
/// 8-bit quantisation, amplified by 255/α.
pub fn diff(a: &Image, b: &Image, tol: u8) -> (usize, u8) {
    let (mut bad, mut worst) = (0, 0u8);
    let pm = |c: u8, a: u8| (u16::from(c) * u16::from(a) + 127) / 255;
    for (p, q) in a.0.as_chunks::<4>().0.iter().zip(b.0.as_chunks::<4>().0) {
        let mut d = p[3].abs_diff(q[3]);
        for c in 0..3 {
            d = d.max(pm(p[c], p[3]).abs_diff(pm(q[c], q[3])) as u8);
        }
        worst = worst.max(d);
        if d > tol {
            bad += 1;
        }
    }
    (bad, worst)
}

/// Done-when (b), alpha-ramp half: patch `i` centre alpha within ±1 of 17·i, colour white.
pub fn check_ramp(img: &Image) -> Vec<String> {
    let mut f = Vec::new();
    for i in 0..16usize {
        let (x, y) = (64 + i * 80 + 32, 64 + 32);
        let p = &img.0[(y * W + x) * 4..(y * W + x) * 4 + 4];
        let want = (17 * i) as u8;
        if p[3].abs_diff(want) > 1 {
            f.push(format!(
                "ramp patch {i}: alpha {}, expected {want} ± 1",
                p[3]
            ));
        }
        if p[3] >= 16 && p[..3].iter().any(|&c| c < 253) {
            f.push(format!(
                "ramp patch {i}: colour {:?}, expected white",
                &p[..3]
            ));
        }
    }
    f
}

/// Done-when (b), scorebug half: over the paint sequence the graphic animates (frames differ),
/// the background stays alpha 0 (corners and most of the frame), something is opaque, and the
/// edges are antialiased (alpha values strictly between 0 and 255 exist).
pub fn check_scorebug(frames: &[Image]) -> Vec<String> {
    let mut f = Vec::new();
    let distinct = frames.windows(2).filter(|w| w[0].0 != w[1].0).count();
    if distinct == 0 {
        f.push(format!(
            "scorebug: {} frames, no change between any two",
            frames.len()
        ));
    }
    for (n, img) in frames.iter().enumerate() {
        for (x, y) in [(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)] {
            let a = img.0[(y * W + x) * 4 + 3];
            if a != 0 {
                f.push(format!(
                    "scorebug frame {n}: corner ({x},{y}) alpha {a}, expected 0"
                ));
            }
        }
    }
    if let Some(last) = frames.last() {
        let mut hist = [0usize; 256];
        for p in last.0.as_chunks::<4>().0 {
            hist[p[3] as usize] += 1;
        }
        let total = W * H;
        if hist[0] * 2 < total {
            f.push(format!(
                "scorebug: only {} of {total} pixels transparent",
                hist[0]
            ));
        }
        if hist[255] < total / 200 {
            f.push(format!("scorebug: only {} opaque pixels", hist[255]));
        }
        let edges: usize = hist[1..255].iter().sum();
        if edges < 100 {
            f.push(format!(
                "scorebug: {edges} pixels with alpha 1..254 (no antialiasing?)"
            ));
        }
    }
    f
}
