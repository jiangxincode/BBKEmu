fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "gnu" {
        // Rust std references GetHostNameW, which older MinGW linkers do not
        // pull in automatically when producing a Windows cdylib.
        println!("cargo:rustc-link-lib=secur32");
    }
}
