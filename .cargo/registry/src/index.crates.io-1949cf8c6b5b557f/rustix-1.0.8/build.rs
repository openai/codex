use std::env::var;
use std::io::Write as _;

/// The directory for inline asm.
const ASM_PATH: &str = "src/backend/linux_raw/arch";

fn main() {
    // Don't rerun this on changes other than build.rs, as we only depend on
    // the rustc version.
    println!("cargo:rerun-if-changed=build.rs");

    // Gather target information.
    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();
    let env = var("CARGO_CFG_TARGET_ENV").unwrap();
    let abi = var("CARGO_CFG_TARGET_ABI");
    let inline_asm_name = format!("{}/{}.rs", ASM_PATH, arch);
    let inline_asm_name_present = std::fs::metadata(inline_asm_name).is_ok();
    let os = var("CARGO_CFG_TARGET_OS").unwrap();
    let pointer_width = var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap();
    let endian = var("CARGO_CFG_TARGET_ENDIAN").unwrap();

    // Check for special target variants.
    let is_x32 = arch == "x86_64" && pointer_width == "32";
    let is_arm64_ilp32 = arch == "aarch64" && pointer_width == "32";
    let is_powerpc64be = arch == "powerpc64" && endian == "big";
    let is_mipseb = (arch == "mips" || arch == "mips32r6") && endian == "big";
    let is_mips64eb = arch.contains("mips64") && endian == "big";
    let is_unsupported_abi = is_x32 || is_arm64_ilp32 || is_powerpc64be || is_mipseb || is_mips64eb;

    // Check for `--features=use-libc`. This allows crate users to enable the
    // libc backend.
    let feature_use_libc = var("CARGO_FEATURE_USE_LIBC").is_ok();

    // Check for `RUSTFLAGS=--cfg=rustix_use_libc`. This allows end users to
    // enable the libc backend even if rustix is depended on transitively.
    let cfg_use_libc = var("CARGO_CFG_RUSTIX_USE_LIBC").is_ok();

    // Check for `--features=rustc-dep-of-std`.
    let rustc_dep_of_std = var("CARGO_FEATURE_RUSTC_DEP_OF_STD").is_ok();

    // Check for eg. `RUSTFLAGS=--cfg=rustix_use_experimental_features`. This
    // is a rustc flag rather than a cargo feature flag because it's
    // experimental and not something we want accidentally enabled via
    // `--all-features`.
    let rustix_use_experimental_features =
        var("CARGO_CFG_RUSTIX_USE_EXPERIMENTAL_FEATURES").is_ok();

    // Check for eg. `RUSTFLAGS=--cfg=rustix_use_experimental_asm`. This is a
    // rustc flag rather than a cargo feature flag because it's experimental
    // and not something we want accidentally enabled via `--all-features`.
    let rustix_use_experimental_asm = var("CARGO_CFG_RUSTIX_USE_EXPERIMENTAL_ASM").is_ok();

    // Miri doesn't support inline asm, and has builtin support for recognizing
    // libc FFI calls, so if we're running under miri, use the libc backend.
    let miri = var("CARGO_CFG_MIRI").is_ok();

    // If experimental features are enabled, auto-detect and use available
    // features.
    if rustc_dep_of_std {
        use_feature("rustc_attrs");
        use_feature("core_intrinsics");
    } else if rustix_use_experimental_features {
        use_feature_or_nothing("rustc_attrs");
        use_feature_or_nothing("core_intrinsics");
    }

    // Features needed only in no-std configurations.
    #[cfg(not(feature = "std"))]
    {
        use_feature_or_nothing("core_c_str");
        use_feature_or_nothing("core_ffi_c");
        use_feature_or_nothing("alloc_c_string");
        use_feature_or_nothing("alloc_ffi");
        use_feature_or_nothing("error_in_core");
    }

    // Feature needed for testing.
    if use_static_assertions() {
        use_feature("static_assertions");
    }

    // WASI support can utilize wasi_ext if present.
    if os == "wasi" {
        use_feature_or_nothing("wasi_ext");
        use_feature_or_nothing("wasip2");
    }

    // If the libc backend is requested, or if we're not on a platform for
    // which we have linux_raw support, use the libc backend.
    //
    // For now Android uses the libc backend; in theory it could use the
    // linux_raw backend, but to do that we'll need to figure out how to
    // install the toolchain for it.
    let libc = feature_use_libc
        || cfg_use_libc
        || os != "linux"
        || !inline_asm_name_present
        || is_unsupported_abi
        || miri
        || ((arch == "powerpc"
            || arch == "powerpc64"
            || arch == "s390x"
            || arch.starts_with("mips"))
            && !rustix_use_experimental_asm);
    if libc {
        // Use the libc backend.
        use_feature("libc");
    } else {
        // Use the linux_raw backend.
        use_feature("linux_raw");
        if rustix_use_experimental_asm {
            use_feature("asm_experimental_arch");
        }
    }

    // Detect whether the compiler requires us to use thumb mode on ARM.
    if arch == "arm" && use_thumb_mode() {
        use_feature("thumb_mode");
    }

    // Rust's libc crate groups some OS's together which have similar APIs;
    // create similarly-named features to make `cfg` tests more concise.
    let freebsdlike = os == "freebsd" || os == "dragonfly";
    if freebsdlike {
        use_feature("freebsdlike");
    }
    let netbsdlike = os == "openbsd" || os == "netbsd";
    if netbsdlike {
        use_feature("netbsdlike");
    }
    let apple = os == "macos" || os == "ios" || os == "tvos" || os == "visionos" || os == "watchos";
    if apple {
        use_feature("apple");
    }
    if os == "linux" || os == "l4re" || os == "android" || os == "emscripten" {
        use_feature("linux_like");
    }
    if os == "solaris" || os == "illumos" {
        use_feature("solarish");
    }
    if apple || freebsdlike || netbsdlike {
        use_feature("bsd");
    }

    // Add some additional common target combinations.

    // Android and "regular" Linux both use the Linux kernel.
    if os == "android" || os == "linux" {
        use_feature("linux_kernel");
    }

    // These platforms have a 32-bit `time_t`.
    if libc
        && (arch == "arm"
            || arch == "powerpc"
            || arch == "mips"
            || arch == "sparc"
            || arch == "x86"
            || (arch == "aarch64" && os == "linux" && abi == Ok("ilp32".to_string())))
        && (apple
            || os == "android"
            || (os == "freebsd" && arch == "x86")
            || os == "haiku"
            || env == "gnu"
            || (env == "musl" && arch == "x86")
            || (arch == "aarch64" && os == "linux" && abi == Ok("ilp32".to_string())))
    {
        use_feature("fix_y2038");
    }

    println!("cargo:rerun-if-env-changed=CARGO_CFG_RUSTIX_USE_EXPERIMENTAL_ASM");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_RUSTIX_USE_LIBC");

    // Rerun this script if any of our features or configuration flags change,
    // or if the toolchain we used for feature detection changes.
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_USE_LIBC");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_RUSTC_DEP_OF_STD");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_MIRI");
}

fn use_static_assertions() -> bool {
    // `offset_from` was made const in Rust 1.65.
    can_compile("const unsafe fn foo(p: *const u8) -> isize { p.offset_from(p) }")
}

fn use_thumb_mode() -> bool {
    // In thumb mode, r7 is reserved.
    !can_compile("pub unsafe fn f() { core::arch::asm!(\"udf #16\", in(\"r7\") 0); }")
}

fn use_feature_or_nothing(feature: &str) {
    if has_feature(feature) {
        use_feature(feature);
    }
}

fn use_feature(feature: &str) {
    println!("cargo:rustc-cfg={}", feature);
}

/// Test whether the rustc at `var("RUSTC")` supports the given feature.
fn has_feature(feature: &str) -> bool {
    can_compile(format!(
        "#![allow(stable_features)]\n#![feature({})]",
        feature
    ))
}

/// Test whether the rustc at `var("RUSTC")` can compile the given code.
fn can_compile<T: AsRef<str>>(test: T) -> bool {
    use std::process::Stdio;

    let rustc = var("RUSTC").unwrap();
    let target = var("TARGET").unwrap();

    // Use `RUSTC_WRAPPER` if it's set, unless it's set to an empty string, as
    // documented [here].
    // [here]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-reads
    let wrapper = var("RUSTC_WRAPPER")
        .ok()
        .and_then(|w| if w.is_empty() { None } else { Some(w) });

    let mut cmd = if let Some(wrapper) = wrapper {
        let mut cmd = std::process::Command::new(wrapper);
        // The wrapper's first argument is supposed to be the path to rustc.
        cmd.arg(rustc);
        cmd
    } else {
        std::process::Command::new(rustc)
    };

    cmd.arg("--crate-type=rlib") // Don't require `main`.
        .arg("--emit=metadata") // Do as little as possible but still parse.
        .arg("--target")
        .arg(target)
        .arg("-o")
        .arg("-")
        .stdout(Stdio::null()); // We don't care about the output (only whether it builds or not)

    // If Cargo wants to set RUSTFLAGS, use that.
    if let Ok(rustflags) = var("CARGO_ENCODED_RUSTFLAGS") {
        if !rustflags.is_empty() {
            for arg in rustflags.split('\x1f') {
                cmd.arg(arg);
            }
        }
    }

    let mut child = cmd
        .arg("-") // Read from stdin.
        .stdin(Stdio::piped()) // Stdin is a pipe.
        .stderr(Stdio::null()) // Errors from feature detection aren't interesting and can be confusing.
        .spawn()
        .unwrap();

    writeln!(child.stdin.take().unwrap(), "{}", test.as_ref()).unwrap();

    child.wait().unwrap().success()
}
