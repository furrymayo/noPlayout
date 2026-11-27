use noplayout_core::library::Library;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use zip::write::SimpleFileOptions;

async fn request(port: u16, method: &str, path: &str) -> (String, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut socket = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    socket
        .write_all(
            format!("{method} {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).await.unwrap();
    let split = bytes.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    (
        String::from_utf8(bytes[..split].to_vec()).unwrap(),
        bytes[split + 4..].to_vec(),
    )
}

#[tokio::test]
async fn actual_http_routes_serve_manifest_modules_and_assets_but_not_escapes() {
    use noplayout_core::{
        health::HealthState,
        http::{self, ApiState},
    };
    use std::sync::Arc;
    use tokio::sync::Notify;
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/ograf/v1/examples/l3rd-name");
    let version = library.import_folder(&example).unwrap();
    let listener = http::bind(0).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(Notify::new());
    let state = Arc::new(ApiState {
        health: HealthState::new(),
        ui_token: None,
        shutdown: stop.clone(),
        library: Some(library),
    });
    let server = tokio::spawn(http::serve(listener, state, stop.clone()));
    for (path, mime) in [
        ("l3rd.ograf.json", "application/json"),
        ("graphic.mjs", "text/javascript"),
        ("lib/gsap-core.js", "text/javascript"),
        ("lib/ograf-logo-app.svg", "image/svg+xml"),
    ] {
        let (headers, body) = request(
            port,
            "GET",
            &format!("/library/{}/{path}", version.content_hash),
        )
        .await;
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers}");
        assert!(headers.contains(mime), "{headers}");
        assert!(headers.contains("immutable"));
        assert!(headers.contains("nosniff"));
        assert_eq!(body, std::fs::read(example.join(path)).unwrap());
    }
    let (headers, body) = request(port, "HEAD", &version.main_url()).await;
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert!(body.is_empty());
    for suffix in [
        "../escape",
        "%2e%2e/escape",
        "%252e%252e/escape",
        "%2fabsolute",
        "lib%5c..%5cescape",
        "missing",
        "lib",
        "C:%5cescape",
    ] {
        let (headers, _) = request(
            port,
            "GET",
            &format!("/library/{}/{suffix}", version.content_hash),
        )
        .await;
        assert!(headers.starts_with("HTTP/1.1 404"), "{suffix}: {headers}");
    }
    for path in [
        "/library/.staging-hidden/graphic.mjs",
        "/library/not-a-hash/graphic.mjs",
    ] {
        assert!(
            request(port, "GET", path)
                .await
                .0
                .starts_with("HTTP/1.1 404")
        );
    }
    assert!(
        request(port, "POST", &version.main_url())
            .await
            .0
            .starts_with("HTTP/1.1 405")
    );
    stop.notify_one();
    server.await.unwrap().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

fn zip_bytes(entries: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        zip.start_file(
            *name,
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn zip_import_is_immutable_and_rejects_escape_links_and_bombs_without_publication() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/ograf/v1/examples/minimal");
    let folder = library.import_folder(&example).unwrap();
    let manifest = std::fs::read(example.join("minimal.ograf.json")).unwrap();
    let main = std::fs::read(example.join("graphic.mjs")).unwrap();
    let thumbnail = std::fs::read(example.join("thumbnails/1280x720.png")).unwrap();
    let readme = std::fs::read(example.join("README.md")).unwrap();
    let package = zip_bytes(
        &[
            ("minimal.ograf.json", &manifest),
            ("graphic.mjs", &main),
            ("thumbnails/1280x720.png", &thumbnail),
            ("README.md", &readme),
        ],
        zip::CompressionMethod::Deflated,
    );
    let archive = root.join("package.zip");
    std::fs::write(&archive, package).unwrap();
    assert_eq!(
        library.import_zip(&archive).unwrap().content_hash,
        folder.content_hash
    );
    for path in [
        "../escape",
        "/absolute",
        "C:/escape",
        "nested/../../escape",
        "nested\\escape",
        "a/../alias",
        "asset%2fescape",
        "NUL.txt",
    ] {
        std::fs::write(
            &archive,
            zip_bytes(&[(path, b"bad")], zip::CompressionMethod::Stored),
        )
        .unwrap();
        assert!(library.import_zip(&archive).is_err(), "accepted {path}");
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_symlink("link", "../outside", SimpleFileOptions::default())
        .unwrap();
    std::fs::write(&archive, zip.finish().unwrap().into_inner()).unwrap();
    assert!(library.import_zip(&archive).is_err());
    std::fs::write(
        &archive,
        zip_bytes(
            &[("bomb", &vec![0; 1024 * 1024])],
            zip::CompressionMethod::Deflated,
        ),
    )
    .unwrap();
    assert!(
        library
            .import_zip(&archive)
            .unwrap_err()
            .to_string()
            .contains("ratio")
    );
    assert_eq!(
        std::fs::read_dir(root.join("library")).unwrap().count(),
        1,
        "failed imports leaked staging/published files"
    );
    assert!(!root.join("escape").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zip_header_inconsistency_and_duplicate_names_are_rejected() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let archive = root.join("package.zip");
    let manifest = serde_json::to_vec(&basic_manifest()).unwrap();
    let valid = zip_bytes(
        &[
            ("test.ograf.json", &manifest),
            ("graphic.mjs", b"module"),
            ("another.mjs", b"module"),
        ],
        zip::CompressionMethod::Stored,
    );
    let mut inconsistent = valid.clone();
    let position = inconsistent
        .windows(11)
        .rposition(|b| b == b"graphic.mjs")
        .unwrap();
    inconsistent[position] = b'x';
    std::fs::write(&archive, inconsistent).unwrap();
    assert!(
        library
            .import_zip(&archive)
            .unwrap_err()
            .to_string()
            .contains("header")
    );
    let mut duplicates = valid;
    for position in 0..duplicates.len() - 11 {
        if &duplicates[position..position + 11] == b"another.mjs" {
            duplicates[position..position + 11].copy_from_slice(b"graphic.mjs");
        }
    }
    std::fs::write(&archive, duplicates).unwrap();
    assert!(
        library
            .import_zip(&archive)
            .unwrap_err()
            .to_string()
            .contains("header")
    );
    assert_eq!(std::fs::read_dir(root.join("library")).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn depth_boundary_assets_and_nested_manifests_with_spaces_remain_usable() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let archive = root.join("package.zip");
    let manifest = serde_json::to_vec(&basic_manifest()).unwrap();
    let prefix = "a/".repeat(63);
    let manifest_path = format!("{prefix}test.ograf.json");
    let main = format!("{prefix}graphic.mjs");
    std::fs::write(
        &archive,
        zip_bytes(
            &[(&manifest_path, &manifest), (&main, b"module")],
            zip::CompressionMethod::Stored,
        ),
    )
    .unwrap();
    let version = library.import_zip(&archive).unwrap();
    assert_eq!(
        library.read_asset(&version.content_hash, &main).unwrap(),
        b"module"
    );
    assert_eq!(
        library.import_zip(&archive).unwrap().content_hash,
        version.content_hash
    );
    let source = root.join("source");
    let mut manifest = basic_manifest();
    manifest["schema"] = serde_json::json!({"type": "object", "properties": {"n": {"type": "integer", "$ref": "payload.json"}}});
    write_package(&source.join("with spaces"), &manifest);
    std::fs::write(
        source.join("with spaces/payload.json"),
        r#"{"type":"integer"}"#,
    )
    .unwrap();
    let version = library.import_folder(&source).unwrap();
    assert!(version.manifest_url().contains("with%20spaces"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn zip_limits_and_case_aliases_fail_before_publication() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let archive = root.join("package.zip");
    let manifest = serde_json::to_vec(&basic_manifest()).unwrap();
    let prefix = "a/".repeat(65);
    let deep_manifest = format!("{prefix}test.ograf.json");
    let deep_main = format!("{prefix}graphic.mjs");
    std::fs::write(
        &archive,
        zip_bytes(
            &[(&deep_manifest, &manifest), (&deep_main, b"module")],
            zip::CompressionMethod::Stored,
        ),
    )
    .unwrap();
    assert!(
        library
            .import_zip(&archive)
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
    for entries in [
        vec![("A/one", b"a".as_slice()), ("a/two", b"b".as_slice())],
        vec![("same", b"a".as_slice()), ("SAME", b"b".as_slice())],
    ] {
        std::fs::write(
            &archive,
            zip_bytes(&entries, zip::CompressionMethod::Stored),
        )
        .unwrap();
        assert!(
            library
                .import_zip(&archive)
                .unwrap_err()
                .to_string()
                .contains("collid")
        );
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..10_001 {
        zip.start_file(
            format!("entry-{index}"),
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
    }
    std::fs::write(&archive, zip.finish().unwrap().into_inner()).unwrap();
    assert!(
        library
            .import_zip(&archive)
            .unwrap_err()
            .to_string()
            .contains("entry limit")
    );
    assert_eq!(std::fs::read_dir(root.join("library")).unwrap().count(), 0);
    std::fs::remove_dir_all(root).unwrap();
}

fn write_package(root: &std::path::Path, manifest: &serde_json::Value) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("test.ograf.json"),
        serde_json::to_vec(manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("graphic.mjs"), "export default class {}").unwrap();
}

fn basic_manifest() -> serde_json::Value {
    serde_json::json!({"$schema": noplayout_core::library::MANIFEST_SCHEMA, "id": "test", "name": "Test", "main": "graphic.mjs", "supportsRealTime": true, "supportsNonRealTime": false})
}

#[test]
fn urls_encode_spaces_and_imports_preserve_old_versions_after_source_edits() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let source = root.join("source");
    let mut manifest = basic_manifest();
    manifest["main"] = serde_json::json!("main module.mjs");
    write_package(&source, &manifest);
    std::fs::rename(source.join("graphic.mjs"), source.join("main module.mjs")).unwrap();
    let old = library.import_folder(&source).unwrap();
    assert!(old.main_url().ends_with("/main%20module.mjs"));
    std::fs::write(source.join("main module.mjs"), "changed").unwrap();
    let new = library.import_folder(&source).unwrap();
    assert_ne!(old.content_hash, new.content_hash);
    assert_eq!(
        library
            .read_asset(&old.content_hash, "main module.mjs")
            .unwrap(),
        b"export default class {}"
    );
    assert_eq!(
        library
            .read_asset(&new.content_hash, "main module.mjs")
            .unwrap(),
        b"changed"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn validates_every_upstream_example_and_each_basketball_graphic_without_modifying_fixtures() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut counts = Vec::new();
    for parent in [
        repo.join("third_party/ograf/v1/examples"),
        repo.join("fixtures/basketball-ograf-package"),
    ] {
        let mut count = 0;
        for entry in std::fs::read_dir(parent).unwrap() {
            let entry = entry.unwrap();
            if !entry.path().is_dir() {
                continue;
            }
            let has_manifest = std::fs::read_dir(entry.path()).unwrap().any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".ograf.json")
            });
            if has_manifest {
                library
                    .import_folder(&entry.path())
                    .unwrap_or_else(|e| panic!("{}: {e:#}", entry.file_name().to_string_lossy()));
                count += 1;
            }
        }
        counts.push(count);
    }
    assert_eq!(counts, vec![9, 6]);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn package_schemas_resolve_only_contained_local_files_and_never_contact_a_server() {
    let root = scratch();
    let source = root.join("source");
    let library = Library::open(root.join("library")).unwrap();
    let mut manifest = basic_manifest();
    manifest["schema"] = serde_json::json!({"type": "object", "properties": {"value": {"type": "integer", "$ref": "payload.json"}}});
    write_package(&source, &manifest);
    std::fs::write(source.join("payload.json"), r#"{"type":"integer"}"#).unwrap();
    library.import_folder(&source).unwrap();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let remote = format!("http://{}/payload.json", listener.local_addr().unwrap());
    for reference in [
        remote.as_str(),
        &remote.replacen("http:", "https:", 1),
        "file:///outside.json",
        "../outside.json",
        "/outside.json",
        "payload%2foutside.json",
    ] {
        manifest["schema"]["properties"]["value"]["$ref"] = serde_json::json!(reference);
        write_package(&source, &manifest);
        assert!(
            library.import_folder(&source).is_err(),
            "accepted {reference}"
        );
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
            .await
            .is_err(),
        "validation attempted a network connection"
    );
    manifest["schema"]["properties"]["value"]["$ref"] = serde_json::json!("payload.json");
    write_package(&source, &manifest);
    std::fs::write(
        source.join("payload.json"),
        serde_json::to_vec(&serde_json::json!({"$ref": remote})).unwrap(),
    )
    .unwrap();
    assert!(library.import_folder(&source).is_err());
    std::fs::remove_file(source.join("payload.json")).unwrap();
    assert!(library.import_folder(&source).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn folder_and_published_assets_reject_symlinks() {
    use std::os::unix::fs::symlink;
    let root = scratch();
    let source = root.join("source");
    let library = Library::open(root.join("library")).unwrap();
    write_package(&source, &basic_manifest());
    std::fs::write(root.join("outside"), "private").unwrap();
    symlink(root.join("outside"), source.join("link")).unwrap();
    assert!(library.import_folder(&source).is_err());
    std::fs::remove_file(source.join("link")).unwrap();
    let version = library.import_folder(&source).unwrap();
    symlink(
        root.join("outside"),
        root.join("library")
            .join(&version.content_hash)
            .join("link"),
    )
    .unwrap();
    assert!(library.read_asset(&version.content_hash, "link").is_err());
    symlink(&source, root.join("alias")).unwrap();
    assert!(library.import_folder(&root.join("alias")).is_err());
    assert!(library.import_folder(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_manifests_missing_main_and_non_realtime_packages_never_publish() {
    let root = scratch();
    let source = root.join("source");
    let library = Library::open(root.join("library")).unwrap();
    for (key, value) in [
        ("$schema", serde_json::json!("other")),
        ("supportsRealTime", serde_json::json!(false)),
        ("main", serde_json::json!("../escape.mjs")),
        ("main", serde_json::json!("missing.mjs")),
        ("unexpected", serde_json::json!(true)),
        (
            "customActions",
            serde_json::json!([{"id":"bad", "schema":null}]),
        ),
    ] {
        let mut manifest = basic_manifest();
        manifest[key] = value;
        write_package(&source, &manifest);
        assert!(library.import_folder(&source).is_err(), "accepted {key}");
        assert_eq!(std::fs::read_dir(root.join("library")).unwrap().count(), 0);
    }
    write_package(&source, &basic_manifest());
    std::fs::copy(
        source.join("test.ograf.json"),
        source.join("second.ograf.json"),
    )
    .unwrap();
    assert!(library.import_folder(&source).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!("library-test-{}", noplayout_core::new_token()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn imports_the_unmodified_renderer_example_and_validates_the_actual_schema() {
    let root = scratch();
    let library = Library::open(root.join("library")).unwrap();
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/ograf/v1/examples/renderer-test");
    let version = library.import_folder(&example).unwrap();
    assert_eq!(version.manifest["id"], "renderer-test");
    assert_eq!(version.content_hash.len(), 64);
    assert_eq!(
        library.import_folder(&example).unwrap().content_hash,
        version.content_hash
    );
    assert_eq!(
        library
            .read_asset(&version.content_hash, "graphic.mjs")
            .unwrap(),
        std::fs::read(example.join("graphic.mjs")).unwrap()
    );
    let invalid = root.join("invalid");
    std::fs::create_dir(&invalid).unwrap();
    let mut manifest = version.manifest;
    manifest["stepCount"] = serde_json::json!(-2);
    std::fs::write(
        invalid.join("test.ograf.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(invalid.join("graphic.mjs"), "export default class {}").unwrap();
    assert!(
        library
            .import_folder(&invalid)
            .unwrap_err()
            .to_string()
            .contains("stepCount")
    );
    std::fs::remove_dir_all(root).unwrap();
}
