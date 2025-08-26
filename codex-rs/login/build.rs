use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Always rerun if the Swift helper changes.
    println!("cargo:rerun-if-changed=src/native_browser_helper.swift");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let helper_out = out_dir.join("codex-auth-helper");
    let swift_src = PathBuf::from("src/native_browser_helper.swift");

    // Attempt to compile with various invocations to maximize compatibility.
    let candidates: &[&[&str]] = &[
        &["swiftc", "-O", "-framework", "Cocoa", "-framework", "WebKit"],
        &["/usr/bin/swiftc", "-O", "-framework", "Cocoa", "-framework", "WebKit"],
        &["xcrun", "swiftc", "-O", "-framework", "Cocoa", "-framework", "WebKit"],
    ];

    let mut built = false;
    for base in candidates {
        let mut cmd = Command::new(base[0]);
        cmd.args(&base[1..])
            .arg(swift_src.as_os_str())
            .arg("-o")
            .arg(&helper_out);
        match cmd.status() {
            Ok(status) if status.success() => {
                built = true;
                break;
            }
            _ => {}
        }
    }

    if !built {
        // Ensure an empty placeholder exists so include_bytes! compiles; runtime will fallback.
        let _ = fs::write(&helper_out, &[] as &[u8]);
        println!("cargo:warning=Failed to compile Swift helper; runtime will attempt on-demand compile.");
    }
}

