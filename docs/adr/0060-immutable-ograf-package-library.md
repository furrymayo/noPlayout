# 0060. Immutable OGraf package snapshots and offline import boundaries

- Status: proposed (M1 task 1 implementation; parent review pending)
- Date: 2026-11-27
- Related: E-024, PLAN §2.3 / §7 M1 task 1, card t_77d0cf14

## Context

E-024 requires a pinned upstream schema and offline manifest validation. The package library
must not expose partially extracted files, follow package links, fetch schema references from
the network, or change bytes behind a URL already used by an on-air instance. This task's
implementation contract chooses `library.rs` and `/library/<content-hash>/...` URLs instead
of PLAN §2.3's sketched graphic-id / shortened-hash URL.

## Decision

- Store one graphic per import. Discover exactly one `.ograf.json` anywhere in the supplied
  folder or ZIP, preserving its position and all package-relative assets. For a multi-graphic
  collection, import each graphic folder separately. Do not use the manifest's arbitrary id
  as a filesystem path.
- Copy/extract into a random engine-owned staging sibling, inaccessible through HTTP. Validate
  the staged manifest against the byte-identical EBU schema and its vendored dependencies,
  require real-time support, check `main` is a contained regular file, and compile data/action
  schemas with an offline-only retriever. Publish only after all checks via a same-filesystem
  directory rename; clean staging on both success and errors.
- Embed the vendored schema bytes. Their canonical identifiers are an in-memory allowlist,
  not network destinations. Compile package schemas under a virtual package URI; relative
  references may access regular JSON files in staging only. Reject network, absolute-file,
  traversal and encoded-separator references, including indirect references. Keep jsonschema
  defaults disabled and only the E-024 `resolve-file` feature; replace its default retriever.
- Use full SHA-256 over a versioned domain separator followed by sorted UTF-8 file paths and
  their bytes, with big-endian u64 path lengths and byte lengths. Include every file, including
  recovery sidecars; exclude timestamps, directory entries and ZIP metadata. Duplicate imports
  reuse a verified identical version. Different source bytes get a new URL; never update an
  existing package directory.
- Apply import caps: 10,000 explicit entries, 64 path components, 128 MiB per file, 512 MiB total
  unpacked bytes, 512 MiB ZIP input, and 200:1 per-entry expansion ratio. Check declared ZIP
  budgets before extraction, then bound actual reads and verify decoded lengths and CRCs.
  Support Stored and Deflate only (pure-Rust flate2 backend), with encryption disabled.
- Require ZIP header consistency, `enclosed_name`, and safe portable relative names. Reject
  links, special files, traversal, drive/UNC/ADS paths, percent-encoded aliases, duplicate names,
  case-colliding path prefixes and Windows reserved filenames. Folder imports apply the same
  portable path rules and entry/byte limits; reject symlinks and special files.
- Serve only GET/HEAD regular assets from a full lowercase hex hash beneath `/library/`.
  Decode URLs once through Axum, then revalidate containment. No directory listing, redirects,
  staging access or fallback documents. Supply explicit module/image/font/media MIME types,
  `nosniff`, and immutable one-year caching. Do blocking file I/O outside async worker threads.

## Consequences and limits

Packages are application data, not a security sandbox for hostile JavaScript. Library storage
is engine-owned and must not be edited out of band; concurrent hostile filesystem mutation by
another process with the same account is outside this path-based containment boundary. Do not
modify source folders during an import. No file watcher, package deletion/GC, import UI, upload
API, OGraf host execution or later-milestone playout behavior is introduced here.

Imports have bounded memory per file but not a global concurrent-request memory budget. Staging
is unpublished, including after an interrupted import; crash-left staging cleanup and power-loss
filesystem durability are not claimed by this task. Existing versions remain usable after engine
restart because asset lookup needs only the content hash and package path.

## Verification

Focused integration tests import all nine EBU examples and all six Basketball graphics without
rewriting fixtures, check the actual vendored schema constraints, exercise real HTTP routes and
asset MIME types, preserve old versions after source edits, and cover malformed imports, links,
ZIP aliases/bombs/entry/depth limits, header inconsistencies, local references and no-network
negative controls. See the worker evidence report for exact commands and results.
