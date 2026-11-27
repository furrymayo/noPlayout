fn main() {
    // The UI is embedded at compile time from web/dist/ui (`cargo xtask web`). On a fresh clone
    // without a Node toolchain, give `cargo build --workspace` a placeholder page instead of a
    // hard error, and say so.
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist/ui");
    let index = dist.join("index.html");
    println!("cargo::rerun-if-changed={}", index.display());
    if !index.exists() {
        println!(
            "cargo::warning=web/dist/ui is missing; embedding a placeholder page. \
             Run `cargo xtask web` (or `npm ci && npm run build` in web/) for the real UI."
        );
        std::fs::create_dir_all(&dist).expect("create web/dist/ui");
        std::fs::write(
            &index,
            "<!doctype html><title>noPlayout</title>\
             <p>The noPlayout UI was not built. Run <code>cargo xtask web</code>, then rebuild.</p>\n",
        )
        .expect("write placeholder index.html");
    }
    tauri_build::build();
}
