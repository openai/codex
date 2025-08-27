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

    let out_dir = match env::var("OUT_DIR") {
        Ok(v) => PathBuf::from(v),
        Err(_) => {
            println!("cargo:warning=OUT_DIR not set; skipping Swift helper embed");
            return;
        }
    };
    let helper_out = out_dir.join("codex-auth-helper");
    let swift_src = PathBuf::from("src/native_browser_helper.swift");

    // Resolve SDK path when available (xcrun)
    let sdk_path = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    // Helper to compile an arch-specific binary
    let compile_arch = |arch: &str, out: &PathBuf| -> bool {
        // Prefer `xcrun swiftc`, then `swiftc`, then explicit path
        let candidates: &[&[&str]] = &[&["xcrun", "swiftc"], &["swiftc"], &["/usr/bin/swiftc"]];
        for base in candidates {
            let mut cmd = Command::new(base[0]);
            cmd.args(&base[1..]);
            cmd.arg("-target").arg(format!("{}-apple-macos12.0", arch));
            if let Some(ref sdk) = sdk_path {
                cmd.arg("-sdk").arg(sdk);
            }
            cmd.arg("-O")
                .arg("-framework")
                .arg("Cocoa")
                .arg("-framework")
                .arg("WebKit")
                .arg(&swift_src)
                .arg("-o")
                .arg(out);
            if let Ok(status) = cmd.status()
                && status.success()
            {
                return true;
            }
        }
        false
    };

    let arm_out = out_dir.join("codex-auth-helper-arm64");
    let x86_out = out_dir.join("codex-auth-helper-x86_64");
    let mut embedded_ok = false;

    let arm_ok = compile_arch("arm64", &arm_out);
    let x86_ok = compile_arch("x86_64", &x86_out);

    if arm_ok && x86_ok {
        // lipo into a universal binary
        let status = Command::new("xcrun")
            .args(["lipo", "-create", "-output"])
            .arg(&helper_out)
            .arg(&arm_out)
            .arg(&x86_out)
            .status()
            .ok();
        if matches!(status, Some(s) if s.success()) {
            embedded_ok = true;
        }
    }
    if !embedded_ok {
        // Fall back to single-arch success
        if arm_ok {
            let _ = fs::copy(&arm_out, &helper_out);
            embedded_ok = helper_out.exists();
        } else if x86_ok {
            let _ = fs::copy(&x86_out, &helper_out);
            embedded_ok = helper_out.exists();
        }
    }
    if !embedded_ok {
        // Ensure an empty placeholder exists so include_bytes! compiles; runtime will fallback.
        let _ = fs::write(&helper_out, &[] as &[u8]);
        println!(
            "cargo:warning=Failed to compile Swift helper; runtime will attempt on-demand compile."
        );
    }
}
