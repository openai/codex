use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Tell rustc/clippy that this is an expected cfg value.
    println!("cargo:rustc-check-cfg=cfg(vendored_bwrap_available)");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "linux" {
        return;
    }

    // Opt-in: do not attempt to fetch/compile bwrap unless explicitly enabled.
    let enable_ffi = matches!(env::var("CODEX_BWRAP_ENABLE_FFI"), Ok(value) if value == "1");
    if !enable_ffi {
        return;
    }

    if let Err(err) = try_build_vendored_bwrap() {
        // Keep normal builds working even if the experiment fails.
        println!("cargo:warning=build-time bubblewrap disabled: {err}");
    }
}

fn try_build_vendored_bwrap() -> Result<(), String> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let src_dir = resolve_bwrap_source_dir(&out_dir)?;

    let libcap = pkg_config::Config::new()
        .probe("libcap")
        .map_err(|err| format!("libcap not available via pkg-config: {err}"))?;

    let config_h = out_dir.join("config.h");
    std::fs::write(
        &config_h,
        "#pragma once\n#define PACKAGE_STRING \"bubblewrap built at codex build-time\"\n",
    )
    .map_err(|err| format!("failed to write {}: {err}", config_h.display()))?;

    let mut build = cc::Build::new();
    build
        .file(src_dir.join("bubblewrap.c"))
        .file(src_dir.join("bind-mount.c"))
        .file(src_dir.join("network.c"))
        .file(src_dir.join("utils.c"))
        .include(&out_dir)
        .include(&src_dir)
        .define("_GNU_SOURCE", None)
        // Rename `main` so we can call it via FFI.
        .define("main", Some("bwrap_main"));

    for include_path in libcap.include_paths {
        build.include(include_path);
    }

    build.compile("build_time_bwrap");
    println!("cargo:rustc-cfg=vendored_bwrap_available");
    Ok(())
}

/// Resolve the bubblewrap source directory used for build-time compilation.
///
/// Priority:
/// 1. `CODEX_BWRAP_SOURCE_DIR` points at an existing bubblewrap checkout.
/// 2. `CODEX_BWRAP_FETCH=1` triggers a build-time shallow git clone into
///    `OUT_DIR`.
fn resolve_bwrap_source_dir(out_dir: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("CODEX_BWRAP_SOURCE_DIR") {
        let src_dir = PathBuf::from(path);
        if src_dir.exists() {
            return Ok(src_dir);
        }
        return Err(format!(
            "CODEX_BWRAP_SOURCE_DIR was set but does not exist: {}",
            src_dir.display()
        ));
    }

    let fetch = matches!(env::var("CODEX_BWRAP_FETCH"), Ok(value) if value == "1");
    if !fetch {
        return Err(
            "no bwrap source available: set CODEX_BWRAP_SOURCE_DIR or CODEX_BWRAP_FETCH=1"
                .to_string(),
        );
    }

    let fetch_ref = env::var("CODEX_BWRAP_FETCH_REF").unwrap_or_else(|_| "v0.11.0".to_string());
    let src_dir = out_dir.join("bubblewrap-src");
    if src_dir.exists() {
        return Ok(src_dir);
    }

    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--branch")
        .arg(&fetch_ref)
        .arg("https://github.com/containers/bubblewrap")
        .arg(&src_dir)
        .status()
        .map_err(|err| format!("failed to spawn git clone: {err}"))?;
    if status.success() {
        return Ok(src_dir);
    }

    Err(format!(
        "git clone bubblewrap ({fetch_ref}) failed with status: {status}"
    ))
}
