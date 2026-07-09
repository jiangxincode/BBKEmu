fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "gnu" {
        // Keep secur32 after Rust static libraries for older MinGW linkers.
        println!("cargo:rustc-link-arg=-lsecur32");
    }
}
