fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match os.as_str() {
        "linux" | "android" | "freebsd" => println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libtinyusd.so"),
        "macos" | "ios" => println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libtinyusd.dylib"),
        _ => {}
    }
}
