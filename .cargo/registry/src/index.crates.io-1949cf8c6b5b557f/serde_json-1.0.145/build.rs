use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-check-cfg=cfg(fast_arithmetic, values(\"32\", \"64\"))");

    // Decide ideal limb width for arithmetic in the float parser and string
    // parser.
    let target_arch = env::var_os("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_pointer_width = env::var_os("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap();
    if target_arch == "aarch64"
        || target_arch == "loongarch64"
        || target_arch == "mips64"
        || target_arch == "powerpc64"
        || target_arch == "wasm32"
        || target_arch == "x86_64"
        || target_pointer_width == "64"
    {
        // The above list of architectures are ones that have native support for
        // 64-bit arithmetic, but which have some targets using a smaller
        // pointer width. Examples include aarch64-unknown-linux-gnu_ilp32 and
        // x86_64-unknown-linux-gnux32. So our choice of limb width is not
        // equivalent to using usize everywhere.
        println!("cargo:rustc-cfg=fast_arithmetic=\"64\"");
    } else {
        println!("cargo:rustc-cfg=fast_arithmetic=\"32\"");
    }
}
