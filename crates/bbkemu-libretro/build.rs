fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "gnu" {
        // Link Winsock 2 library needed by Rust std for older MinGW
        // toolchains used by the libretro buildbot (provides GetHostNameW).
        println!("cargo:rustc-link-lib=ws2_32");
    }
}
