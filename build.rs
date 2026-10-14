/// Linker script pieces required by `cortex-m-rt`.
///
/// Two files are copied next to the rest of the link arguments emitted by
/// `cortex-m-rt` itself (`link.x`): `device.x` (weak aliases for every NVIC
/// handler) and `memory.x` (flash / ram layout).
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    for file in ["device.x", "memory.x"] {
        fs::copy(file, out.join(file)).unwrap();
        println!("cargo:rerun-if-changed={}", file);
    }

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=build.rs");
}
