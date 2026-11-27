//! Offline OGraf package ingestion and immutable content-addressed assets (E-024).
//!
//! Imports are snapshots: copy into a private staging directory, validate there, hash sorted
//! paths and bytes, then publish with a same-filesystem rename. Never serve staging files.
//! TODO(M1): host runtime and dev CLI consumers remain separate tasks; see STATUS.md.
//! Library storage is engine-owned; callers must not mutate it or source folders during import.

use anyhow::{Context, Result, bail, ensure};
use jsonschema::{Retrieve, Uri};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA: &str =
    "https://ograf.ebu.io/v1/specification/json-schemas/graphics/schema.json";
const SCHEMA_PREFIX: &str = "https://ograf.ebu.io/v1/specification/json-schemas/";
const MAX_FILES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

macro_rules! schema {
    ($path:literal) => {
        (
            $path,
            include_str!(concat!(
                "../../../third_party/ograf/v1/specification/json-schemas/",
                $path
            )),
        )
    };
}
const SCHEMAS: &[(&str, &str)] = &[
    schema!("graphics/schema.json"),
    schema!("gdd/basic-types.json"),
    schema!("gdd/gdd-types.json"),
    schema!("gdd/object.json"),
    schema!("gdd/playout-options.json"),
    schema!("lib/action.json"),
    schema!("lib/constraints/boolean.json"),
    schema!("lib/constraints/number.json"),
    schema!("lib/constraints/string.json"),
];

fn url_path(path: &str) -> String {
    let mut result = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            result.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").expect("writing a string cannot fail");
        }
    }
    result
}

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PackageVersion {
    pub content_hash: String,
    pub manifest_path: String,
    pub manifest: Value,
}

impl PackageVersion {
    pub fn manifest_url(&self) -> String {
        format!(
            "/library/{}/{}",
            self.content_hash,
            url_path(&self.manifest_path)
        )
    }

    pub fn main_url(&self) -> String {
        let parent = Path::new(&self.manifest_path).parent().unwrap();
        let main = self.manifest["main"].as_str().expect("validated main");
        format!(
            "/library/{}/{}",
            self.content_hash,
            url_path(&parent.join(main).to_string_lossy().replace('\\', "/"))
        )
    }
}

impl Library {
    /// `root` is engine-owned storage, not the folder being imported.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        ensure!(
            !fs::symlink_metadata(&root)?.file_type().is_symlink(),
            "library root is a symlink"
        );
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    pub fn import_folder(&self, source: &Path) -> Result<PackageVersion> {
        ensure!(
            fs::symlink_metadata(source)?.file_type().is_dir(),
            "package must be a folder, not a link"
        );
        let source = source.canonicalize()?;
        ensure!(
            !self.root.starts_with(&source),
            "source contains library storage"
        );
        let stage = self.stage()?;
        let mut budget = Budget::default();
        copy_folder(&source, &source, &stage.0, &mut budget)?;
        self.publish(&stage)
    }

    /// Stored/deflated ZIPs only. Header limits are checked before extraction; actual decoded
    /// lengths and CRCs are checked while reading. No links, aliases or overwrites.
    pub fn import_zip(&self, source: &Path) -> Result<PackageVersion> {
        let file = fs::File::open(source)?;
        ensure!(
            file.metadata()?.len() <= MAX_TOTAL_BYTES,
            "zip exceeds compressed size limit"
        );
        let mut archive = zip::ZipArchive::new(file).context("invalid zip archive")?;
        ensure!(archive.len() <= MAX_FILES, "zip exceeds entry limit");
        archive
            .validate_header_consistency()
            .context("zip header consistency check failed")?;
        let stage = self.stage()?;
        let mut budget = Budget::default();
        let mut names = std::collections::BTreeSet::new();
        // Complete preflight: a bad late entry must not leave partially published content.
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            budget.entry()?;
            ensure!(
                !entry.encrypted() && !entry.is_symlink(),
                "encrypted zip entries and links are not allowed"
            );
            let raw = entry.name()?;
            let raw = raw.trim_end_matches('/');
            ensure!(!raw.split('/').any(|p| p == "."), "zip path alias");
            safe_relative(raw)?;
            budget.path(raw)?;
            ensure!(entry.enclosed_name().is_some(), "zip path escapes package");
            ensure!(
                names.insert(raw.to_lowercase()),
                "duplicate or case-colliding zip path"
            );
            let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
            ensure!(
                kind == 0 || kind == 0o100000 || (kind == 0o040000 && entry.is_dir()),
                "zip special files are not allowed"
            );
            if !entry.is_dir() {
                budget.bytes(entry.size())?;
                ensure!(
                    entry.size() <= entry.compressed_size().max(1).saturating_mul(200),
                    "zip compression ratio exceeds limit"
                );
            }
        }
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let enclosed = entry.enclosed_name().context("zip path escapes package")?;
            let destination = stage.0.join(enclosed);
            if entry.is_dir() {
                fs::create_dir_all(destination)?;
            } else {
                fs::create_dir_all(destination.parent().unwrap())?;
                let size = entry.size();
                let mut data = Vec::new();
                (&mut entry).take(size + 1).read_to_end(&mut data)?;
                ensure!(
                    data.len() as u64 == size,
                    "zip decoded size differs from declared size"
                );
                fs::File::create_new(destination)?.write_all(&data)?;
            }
        }
        self.publish(&stage)
    }

    /// Read a regular asset only from a published version; no directory index or fallback.
    pub fn read_asset(&self, hash: &str, asset: &str) -> Result<Vec<u8>> {
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid content hash"
        );
        let version_root = self.root.join(hash);
        ensure!(
            fs::symlink_metadata(&version_root)?.is_dir(),
            "version is not a regular directory"
        );
        let path = contained_file(&version_root, asset)?;
        read_limited(&path)
    }

    fn stage(&self) -> Result<Stage> {
        // A random sibling keeps the final rename on the same filesystem; no shared staging root.
        let path = self.root.join(format!(".staging-{}", crate::new_token()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&path)?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&path)?;
        Ok(Stage(path))
    }

    fn publish(&self, stage: &Stage) -> Result<PackageVersion> {
        let files = inventory(&stage.0)?;
        let manifests: Vec<_> = files
            .keys()
            .filter(|p| p.ends_with(".ograf.json"))
            .collect();
        ensure!(!manifests.is_empty(), "package has no .ograf.json manifest");
        ensure!(
            manifests.len() == 1,
            "package has multiple manifests; import each graphic folder separately"
        );
        let manifest_path = manifests[0].to_string();
        let manifest: Value = serde_json::from_slice(&read_limited(&stage.0.join(&manifest_path))?)
            .context("manifest is not valid JSON")?;
        validate_manifest(&stage.0, &manifest_path, &manifest)?;
        let mut hasher = Sha256::new();
        hasher.update(b"noplayout-package-v1\0");
        for (path, size) in &files {
            hasher.update((path.len() as u64).to_be_bytes());
            hasher.update(path.as_bytes());
            hasher.update(size.to_be_bytes());
            hasher.update(read_limited(&stage.0.join(path))?);
        }
        let content_hash: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let destination = self.root.join(&content_hash);
        if destination.exists() {
            ensure!(
                inventory(&destination)? == files,
                "stored version differs from its content hash"
            );
            for path in files.keys() {
                ensure!(
                    contained_file(&destination, path)?.is_file(),
                    "stored asset is not regular"
                );
                ensure!(
                    read_limited(&destination.join(path))? == read_limited(&stage.0.join(path))?,
                    "stored version content differs"
                );
            }
        } else if let Err(error) = fs::rename(&stage.0, &destination) {
            // Another importer may have published the same immutable version concurrently.
            if !destination.is_dir() {
                return Err(error.into());
            }
            ensure!(
                inventory(&destination)? == files,
                "concurrent version differs"
            );
            for path in files.keys() {
                ensure!(
                    read_limited(&contained_file(&destination, path)?)?
                        == read_limited(&stage.0.join(path))?,
                    "concurrent version content differs"
                );
            }
        }
        Ok(PackageVersion {
            content_hash,
            manifest_path,
            manifest,
        })
    }
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Budget {
    entries: usize,
    bytes: u64,
    paths: BTreeMap<String, String>,
}
impl Budget {
    fn path(&mut self, path: &str) -> Result<()> {
        let mut prefix = String::new();
        for part in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let key = prefix.to_lowercase();
            if let Some(previous) = self.paths.get(&key) {
                ensure!(previous == &prefix, "case-colliding package path");
            } else {
                self.paths.insert(key, prefix.clone());
            }
        }
        Ok(())
    }

    fn entry(&mut self) -> Result<()> {
        self.entries += 1;
        ensure!(self.entries <= MAX_FILES, "package exceeds entry limit");
        Ok(())
    }
    fn bytes(&mut self, bytes: u64) -> Result<()> {
        ensure!(bytes <= MAX_FILE_BYTES, "asset exceeds file size limit");
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .context("package size overflow")?;
        ensure!(
            self.bytes <= MAX_TOTAL_BYTES,
            "package exceeds total size limit"
        );
        Ok(())
    }
}

fn copy_folder(root: &Path, source: &Path, target: &Path, budget: &mut Budget) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        budget.entry()?;
        let relative = portable_path(entry.path().strip_prefix(root)?)?;
        safe_relative(&relative)?;
        budget.path(&relative)?;
        let kind = fs::symlink_metadata(entry.path())?.file_type();
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir(&destination)?;
            copy_folder(root, &entry.path(), &destination, budget)?;
        } else {
            ensure!(kind.is_file(), "links and special files are not allowed");
            let data = read_limited(&entry.path())?;
            budget.bytes(data.len() as u64)?;
            fs::File::create_new(destination)?.write_all(&data)?;
        }
    }
    Ok(())
}

fn portable_path(path: &Path) -> Result<String> {
    path.components()
        .map(|p| p.as_os_str().to_str().context("non-UTF8 package path"))
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

fn inventory(root: &Path) -> Result<BTreeMap<String, u64>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, u64>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "stored package contains a link"
            );
            if metadata.is_dir() {
                walk(root, &entry.path(), files)?;
            } else {
                ensure!(metadata.is_file(), "package contains a special file");
                let path = portable_path(entry.path().strip_prefix(root)?)?;
                safe_relative(&path)?;
                files.insert(path, metadata.len());
            }
        }
        Ok(())
    }
    ensure!(
        fs::symlink_metadata(root)?.is_dir(),
        "package root is not a regular directory"
    );
    let mut files = BTreeMap::new();
    walk(root, root, &mut files)?;
    Ok(files)
}

/// Conservative, platform-independent relative paths; the same package must be safe on Windows.
fn safe_relative(path: &str) -> Result<PathBuf> {
    ensure!(
        !path.is_empty() && !path.starts_with('/'),
        "asset path is not relative"
    );
    ensure!(
        path.split('/').count() <= 64,
        "asset path exceeds depth limit"
    );
    let mut result = PathBuf::new();
    for part in path.split('/') {
        if part == "." {
            continue;
        }
        ensure!(!part.is_empty() && part != "..", "asset path traversal");
        ensure!(
            !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:%?#<>\"|*".contains(c)),
            "unsafe asset path"
        );
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            ![
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9"
            ]
            .contains(&stem.as_str()),
            "reserved asset name"
        );
        result.push(part);
    }
    ensure!(!result.as_os_str().is_empty(), "empty asset path");
    Ok(result)
}

fn contained_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = safe_relative(relative)?;
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "asset contains a link"
        );
    }
    let path = path.canonicalize()?;
    ensure!(
        path.starts_with(root) && path.is_file(),
        "asset escapes package or is not a file"
    );
    Ok(path)
}

fn read_limited(path: &Path) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= MAX_FILE_BYTES,
        "asset exceeds file size limit"
    );
    Ok(data)
}

struct OfflineSchemas {
    package: Option<PathBuf>,
}
impl Retrieve for OfflineSchemas {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let uri = uri.as_str();
        if let Some(name) = uri.strip_prefix(SCHEMA_PREFIX)
            && let Some((_, bytes)) = SCHEMAS.iter().find(|(path, _)| *path == name)
        {
            return Ok(serde_json::from_str(bytes)?);
        }
        if let (Some(root), Some(path)) = (&self.package, uri.strip_prefix("noplayout:/")) {
            let bytes = read_limited(&contained_file(root, path)?)?;
            let value = serde_json::from_slice(&bytes)?;
            check_references(&value, false)?;
            return Ok(value);
        }
        Err(format!("schema reference is not available offline: {uri}").into())
    }
}

fn check_references(value: &Value, manifest_root: bool) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "$ref" | "$dynamicRef" | "$recursiveRef" | "$schema"
                ) {
                    let uri = value
                        .as_str()
                        .context("schema reference must be a string")?;
                    if key == "$schema"
                        && ((manifest_root && uri == MANIFEST_SCHEMA)
                            || uri == "https://json-schema.org/draft/2020-12/schema")
                    {
                        continue;
                    }
                    let path = uri.split('#').next().unwrap();
                    if !path.is_empty() {
                        safe_relative(path).context("remote or escaping schema reference")?;
                    }
                }
                check_references(value, false)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                check_references(value, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_manifest(root: &Path, path: &str, manifest: &Value) -> Result<()> {
    let schema: Value = serde_json::from_str(SCHEMAS[0].1)?;
    let validator = jsonschema::options()
        .with_retriever(OfflineSchemas { package: None })
        .build(&schema)
        .map_err(|e| anyhow::anyhow!("vendored manifest schema: {e}"))?;
    if let Some(error) = validator.iter_errors(manifest).next() {
        bail!("manifest {}: {}", error.instance_path(), error);
    }
    ensure!(
        manifest["supportsRealTime"] == true,
        "supportsRealTime must be true; non-real-time-only graphics are not supported"
    );
    check_references(manifest, true)?;
    let parent = Path::new(path).parent().unwrap();
    let main = safe_relative(manifest["main"].as_str().unwrap())?;
    let main = parent.join(main).to_string_lossy().replace('\\', "/");
    contained_file(root, &main).context("main module is missing or outside the package")?;
    if let Some(schema) = manifest.get("schema") {
        compile_package_schema(root, path, schema)?;
    }
    if let Some(actions) = manifest["customActions"].as_array() {
        for action in actions {
            if let Some(schema) = action.get("schema").filter(|s| !s.is_null()) {
                compile_package_schema(root, path, schema)?;
            }
        }
    }
    Ok(())
}

fn compile_package_schema(root: &Path, path: &str, schema: &Value) -> Result<()> {
    let schema_root = root.join(Path::new(path).parent().context("manifest has no parent")?);
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_base_uri("noplayout:/manifest.json")
        .with_retriever(OfflineSchemas {
            package: Some(schema_root),
        })
        .build(schema)
        .map_err(|e| anyhow::anyhow!("package schema is invalid or unavailable offline: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_enforce_file_total_and_entry_boundaries_without_large_allocations() {
        let mut budget = Budget::default();
        assert!(budget.bytes(MAX_FILE_BYTES + 1).is_err());
        let mut budget = Budget::default();
        for _ in 0..4 {
            budget.bytes(MAX_FILE_BYTES).unwrap();
        }
        assert_eq!(budget.bytes, MAX_TOTAL_BYTES);
        assert!(budget.bytes(1).is_err());
        let mut budget = Budget {
            entries: MAX_FILES - 1,
            ..Budget::default()
        };
        budget.entry().unwrap();
        assert!(budget.entry().is_err());
        assert!(safe_relative(&"x/".repeat(65)).is_err());
    }
}
