use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    println!("cargo:rerun-if-env-changed=SDKROOT");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    println!("cargo:rustc-link-lib=framework=IOKit");
    let sdk_path = macos_sdk_path();

    let bindings = bindgen::Builder::default()
        .header_contents("iokit_wrapper.h", "#include <IOKit/pwr_mgt/IOPMLib.h>\n")
        .clang_arg("-isysroot")
        .clang_arg(&sdk_path)
        .clang_arg("-F")
        .clang_arg(format!("{sdk_path}/System/Library/Frameworks"))
        .allowlist_function("IOPMAssertionCreateWithName")
        .allowlist_function("IOPMAssertionRelease")
        .allowlist_type("IOPMAssertionID")
        .allowlist_type("IOPMAssertionLevel")
        .allowlist_type("IOReturn")
        .allowlist_type("CFStringRef")
        .allowlist_type("__CFString")
        .allowlist_var("kIOPMAssertionLevelOn")
        .allowlist_var("kIOReturnSuccess")
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("failed to generate IOKit bindings");

    let out_dir = PathBuf::from(
        env::var("OUT_DIR").expect("Cargo should always provide the OUT_DIR env var"),
    );
    bindings
        .write_to_file(out_dir.join("iokit_bindings.rs"))
        .expect("failed to write generated IOKit bindings");
}

fn macos_sdk_path() -> String {
    if let Ok(sdk_path) = env::var("SDKROOT")
        && !sdk_path.is_empty()
    {
        return sdk_path;
    }

    let output = Command::new("xcrun")
        .arg("--show-sdk-path")
        .output()
        .expect("failed to run xcrun --show-sdk-path");
    if !output.status.success() {
        panic!(
            "xcrun --show-sdk-path failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout)
        .expect("xcrun --show-sdk-path returned non-UTF8 output")
        .trim()
        .to_owned()
}
