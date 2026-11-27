fn main() {
    // cef-dll-sys copies libcef.so and its resources next to the binaries on Linux, but the
    // loader doesn't search the executable's own directory unless the binary says so.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo::rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
}
