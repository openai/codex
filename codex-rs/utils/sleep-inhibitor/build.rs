use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    println!("cargo:rerun-if-env-changed=SDKROOT");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return Ok(());
    }

    println!("cargo:rustc-link-lib=framework=IOKit");
    let sdk_path = macos_sdk_path()?;
    let framework_path = format!("{sdk_path}/System/Library/Frameworks");

    let bindings = bindgen::Builder::default()
        .header_contents("iokit_wrapper.h", "#include <IOKit/pwr_mgt/IOPMLib.h>\n")
        .clang_arg("-isysroot")
        .clang_arg(sdk_path.as_str())
        .clang_arg("-F")
        .clang_arg(framework_path)
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
        .generate()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    bindings.write_to_file(out_dir.join("iokit_bindings.rs"))?;

    Ok(())
}

fn macos_sdk_path() -> Result<String, Box<dyn Error>> {
    if let Ok(sdk_path) = env::var("SDKROOT")
        && !sdk_path.is_empty()
    {
        return Ok(sdk_path);
    }

    let output = Command::new("xcrun").arg("--show-sdk-path").output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("xcrun --show-sdk-path failed: {stderr}").into());
    }

    let sdk_path = String::from_utf8(output.stdout)?;
    Ok(sdk_path.trim().to_owned())
}
