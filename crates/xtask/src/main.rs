//! `cargo xtask <command>` (PLAN §2.2, §7 M0 task 6).
//!
//! ```text
//! cargo xtask cef                export the pinned CEF for this machine into $CEF_PATH
//! cargo xtask web                npm ci (when needed) + npm run build in web/
//! cargo xtask bundle [--release] build the renderer; on macOS make its dev app bundle
//! cargo xtask self-test [--release]  build everything headless and run `noplayout-engine --self-test`
//! cargo xtask run [--release]    build everything and open the noPlayout app
//! cargo xtask preflight          run the local checks a CI lane cannot, write target/preflight/*.json
//! ```

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The `cef` / `export-cef-dir` crate pin (PLAN §3). A test checks it against Cargo.toml.
const CEF_CRATE_VERSION: &str = "156.7.0+156.0.31";
/// The CEF release inside that crate version (the build metadata), used in directory names.
const CEF_VERSION: &str = "156.0.31";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let release = args.iter().any(|a| a == "--release");
    match args.first().map(String::as_str) {
        Some("cef") => cef(),
        Some("web") => web(),
        Some("bundle") => bundle(release).map(|_| ()),
        Some("self-test") => self_test(release).map(|_| ()),
        Some("run") => run(release),
        Some("preflight") => preflight(),
        _ => {
            eprintln!(
                "usage: cargo xtask <cef | web | bundle | self-test | run | preflight> [--release]"
            );
            std::process::exit(2);
        }
    }
}

fn root() -> PathBuf {
    // crates/xtask → workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask lives two levels below the workspace root")
        .to_path_buf()
}

fn host_target() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else {
        "unsupported"
    }
}

/// cef-dll-sys's directory name for a target (`cef_<os>_<arch>`).
fn cef_os_arch(target: &str) -> Result<&'static str> {
    Ok(match target {
        "aarch64-apple-darwin" => "cef_macos_aarch64",
        "x86_64-apple-darwin" => "cef_macos_x86_64",
        "x86_64-pc-windows-msvc" => "cef_windows_x86_64",
        "x86_64-unknown-linux-gnu" => "cef_linux_x86_64",
        other => bail!("no CEF build for target {other}"),
    })
}

/// `$CEF_PATH`, else `<root>/target/cef` (the same default as `.cargo/config.toml`).
fn cef_root() -> PathBuf {
    std::env::var_os("CEF_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target").join("cef"))
}

fn cef_dir() -> Result<PathBuf> {
    Ok(cef_root()
        .join(CEF_VERSION)
        .join(cef_os_arch(host_target())?))
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    c.current_dir(root());
    c
}

fn npm() -> Command {
    let mut c = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
    c.current_dir(root().join("web"));
    c
}

fn status(mut cmd: Command) -> Result<()> {
    eprintln!("xtask: {cmd:?}");
    let st = cmd
        .status()
        .with_context(|| format!("could not run {cmd:?}"))?;
    if !st.success() {
        bail!("{cmd:?} failed with {st}");
    }
    Ok(())
}

fn profile_dir(release: bool) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target"));
    target.join(if release { "release" } else { "debug" })
}

/// Export the pinned CEF into `$CEF_PATH/<version>/<os_arch>` with export-cef-dir (installed
/// into target/tools on first use), the layout cef-dll-sys's build script looks for. CI caches
/// that directory.
fn cef() -> Result<()> {
    let dir = cef_dir()?;
    if dir.join("archive.json").exists() {
        eprintln!("xtask: CEF {CEF_VERSION} already at {}", dir.display());
        return Ok(());
    }
    let tools = root().join("target").join("tools");
    let exe = tools
        .join("bin")
        .join(format!("export-cef-dir{}", std::env::consts::EXE_SUFFIX));
    if !exe.exists() {
        let mut install = cargo();
        install.args(["install", "--locked", "--root"]);
        install.arg(&tools);
        install.args([
            "--version",
            &format!("={CEF_CRATE_VERSION}"),
            "export-cef-dir",
        ]);
        status(install)?;
    }
    std::fs::create_dir_all(dir.parent().expect("versioned dir has a parent"))?;
    let mut export = Command::new(&exe);
    export.args(["--force", "--target", host_target()]);
    export.arg(&dir);
    status(export)?;
    if !dir.join("archive.json").exists() {
        bail!(
            "export-cef-dir finished but {} has no archive.json",
            dir.display()
        );
    }
    eprintln!("xtask: CEF {CEF_VERSION} exported to {}", dir.display());
    Ok(())
}

/// Build the Svelte UI into web/dist/ui (the app embeds it at compile time).
fn web() -> Result<()> {
    if !root().join("web").join("node_modules").exists() {
        let mut ci = npm();
        ci.arg("ci");
        status(ci)?;
    }
    let mut build = npm();
    build.args(["run", "build"]);
    status(build)
}

/// Build the renderer and, on macOS, assemble `noplayout-renderer.app` with the CEF framework
/// and the four helper apps next to the other binaries (spike S5). Returns the renderer path.
fn bundle(release: bool) -> Result<PathBuf> {
    let mut build = cargo();
    build.args(["build", "--package", "noplayout-renderer"]);
    if release {
        build.arg("--release");
    }
    status(build)?;
    let dir = profile_dir(release);
    bundle_platform(&dir)
}

#[cfg(target_os = "macos")]
fn bundle_platform(dir: &Path) -> Result<PathBuf> {
    use cef::build_util::mac::{BundleInfo, bundle};
    let version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let info = BundleInfo::new(
        "noplayout-renderer",
        "org.noplayout.renderer",
        "noPlayout Renderer",
        "English",
        version,
    );
    let app = bundle(
        dir,
        dir,
        "noplayout-renderer",
        "noplayout-renderer",
        None,
        info,
    )
    .map_err(|e| anyhow::anyhow!("bundling the renderer failed: {e:?}"))?;
    let exe = app
        .join("Contents")
        .join("MacOS")
        .join("noplayout-renderer");
    eprintln!("xtask: renderer bundle at {}", app.display());
    Ok(exe)
}

#[cfg(not(target_os = "macos"))]
fn bundle_platform(dir: &Path) -> Result<PathBuf> {
    // On Windows and Linux cef-dll-sys copies the CEF runtime next to the binaries itself.
    Ok(dir.join(format!(
        "noplayout-renderer{}",
        std::env::consts::EXE_SUFFIX
    )))
}

fn build_headless(release: bool) -> Result<PathBuf> {
    let mut build = cargo();
    build.args([
        "build",
        "--package",
        "noplayout-core",
        "--package",
        "noplayout-output",
    ]);
    if release {
        build.arg("--release");
    }
    status(build)?;
    bundle(release)?;
    Ok(profile_dir(release).join(format!("noplayout-engine{}", std::env::consts::EXE_SUFFIX)))
}

/// Build engine, output and renderer, then run the engine's headless self-test.
fn self_test(release: bool) -> Result<String> {
    let engine = build_headless(release)?;
    eprintln!("xtask: {} --self-test", engine.display());
    let out = Command::new(&engine)
        .arg("--self-test")
        .output()
        .with_context(|| format!("could not run {}", engine.display()))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    print!("{text}");
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        bail!("self-test failed ({})", out.status);
    }
    Ok(text)
}

/// Build everything and open the app; the app starts (or attaches to) the engine.
fn run(release: bool) -> Result<()> {
    web()?;
    build_headless(release)?;
    let mut build = cargo();
    build.args(["build", "--package", "noplayout-app"]);
    if release {
        build.arg("--release");
    }
    status(build)?;
    let app = profile_dir(release).join(format!("noplayout-app{}", std::env::consts::EXE_SUFFIX));
    status(Command::new(app))
}

#[derive(Serialize)]
struct PreflightCheck {
    name: &'static str,
    /// `pass`, `fail` or `skipped`.
    result: &'static str,
    detail: String,
}

#[derive(Serialize)]
struct Preflight {
    os: &'static str,
    arch: &'static str,
    target: &'static str,
    commit: String,
    dirty: bool,
    cef: &'static str,
    created_unix: u64,
    checks: Vec<PreflightCheck>,
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Run the local checks a hosted lane cannot (R13, PLAN §8.2) and record them.
fn preflight() -> Result<()> {
    let commit = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    let mut checks = Vec::new();

    let st = self_test(true);
    checks.push(PreflightCheck {
        name: "cef-self-test",
        result: if st.is_ok() { "pass" } else { "fail" },
        detail: match &st {
            Ok(text) => text
                .lines()
                .filter(|l| l.contains("PASS") || l.contains("FAIL"))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("{e:#}"),
        },
    });
    // TODO(M1): CEF conformance + pixel goldens. TODO(M2): NDI loopback. TODO(M1): S1 timing.
    for (name, milestone) in [
        ("cef-conformance", "M1"),
        ("pixel-goldens", "M1"),
        ("ndi-loopback", "M2"),
        ("timing-s1", "M1"),
    ] {
        checks.push(PreflightCheck {
            name,
            result: "skipped",
            detail: format!("not built yet; arrives in {milestone}"),
        });
    }

    let report = Preflight {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        target: host_target(),
        commit: commit.clone(),
        dirty,
        cef: CEF_VERSION,
        created_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        checks,
    };
    let dir = root().join("target").join("preflight");
    std::fs::create_dir_all(&dir)?;
    let short = commit.get(..12).unwrap_or(&commit);
    let path = dir.join(format!(
        "{}-{}-{short}.json",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    std::fs::write(&path, serde_json::to_string_pretty(&report)? + "\n")?;
    println!("xtask: preflight written to {}", path.display());
    if report.checks.iter().any(|c| c.result == "fail") {
        bail!("preflight has failing checks");
    }
    if dirty {
        eprintln!("xtask: warning: the working tree is dirty; the file names HEAD {short}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cef_pins_match_the_workspace_manifest_and_lock() {
        let (semver, build) = CEF_CRATE_VERSION.split_once('+').unwrap();
        assert_eq!(build, CEF_VERSION);
        let manifest = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
        let pin = format!("cef = {{ version = \"={semver}\" }}");
        assert!(manifest.contains(&pin), "Cargo.toml must pin {pin}");
        let lock = std::fs::read_to_string(root().join("Cargo.lock")).unwrap();
        for krate in ["cef", "cef-dll-sys"] {
            let entry = format!("name = \"{krate}\"\nversion = \"{CEF_CRATE_VERSION}\"");
            assert!(
                lock.contains(&entry),
                "Cargo.lock must hold {krate} {CEF_CRATE_VERSION}"
            );
        }
    }

    #[test]
    fn cef_directory_names_follow_cef_dll_sys() {
        assert_eq!(
            cef_os_arch("aarch64-apple-darwin").unwrap(),
            "cef_macos_aarch64"
        );
        assert_eq!(
            cef_os_arch("x86_64-pc-windows-msvc").unwrap(),
            "cef_windows_x86_64"
        );
        assert!(cef_os_arch("riscv64gc-unknown-linux-gnu").is_err());
    }
}
