fn main() {
    // `cargo xtask bundle` puts libcef.so and its resources next to the binaries on Linux; the
    // loader only searches the executable's own directory if the binary says so.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo::rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
}
