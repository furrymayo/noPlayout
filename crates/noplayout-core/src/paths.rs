//! Where the engine keeps its files.

use std::path::PathBuf;

/// Environment override for the data directory (tests, portable installs).
pub const ENV_DATA_DIR: &str = "NOPLAYOUT_DATA_DIR";

/// The engine's data directory: `$NOPLAYOUT_DATA_DIR`, else the per-user local data directory
/// (`~/Library/Application Support/noPlayout`, `%LOCALAPPDATA%\noPlayout\data`,
/// `~/.local/share/noplayout`).
pub fn data_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(ENV_DATA_DIR) {
        return Ok(PathBuf::from(dir));
    }
    let dirs = directories::ProjectDirs::from("", "", "noPlayout").ok_or_else(|| {
        anyhow::anyhow!("no home directory found; set {ENV_DATA_DIR} to choose a data directory")
    })?;
    Ok(dirs.data_local_dir().to_path_buf())
}

/// Log directory inside the data directory.
pub fn log_dir(data: &std::path::Path) -> PathBuf {
    data.join("logs")
}

/// CEF cache root for one renderer slot (`root_cache_path` must be unique per running CEF).
pub fn cef_cache_dir(data: &std::path::Path, channel: &str, slot: &str) -> PathBuf {
    data.join("cef-cache").join(format!("{channel}-{slot}"))
}

/// Environment override for the built `web/host` directory.
pub const ENV_HOST_DIR: &str = "NOPLAYOUT_HOST_DIR";

/// The built host runtime (`renderer.html`, `ograf-host.js`) the engine serves at `/host/`:
/// `$NOPLAYOUT_HOST_DIR`, else `host/` next to the running binary (placed by `cargo xtask`).
pub fn host_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(ENV_HOST_DIR) {
        return Some(PathBuf::from(dir));
    }
    let dir = std::env::current_exe().ok()?.parent()?.join("host");
    dir.join("renderer.html").is_file().then_some(dir)
}
