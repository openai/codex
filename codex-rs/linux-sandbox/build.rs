use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    // Tell rustc/clippy that this is an expected cfg value.
    println!("cargo:rustc-check-cfg=cfg(vendored_bwrap_available)");
    println!("cargo:rerun-if-env-changed=CODEX_BWRAP_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_ALLOW_CROSS");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR");

    // Rebuild if the vendored bwrap sources change.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let vendor_dir = manifest_dir.join("../vendor/bubblewrap");
    println!(
        "cargo:rerun-if-changed={}",
        vendor_dir.join("bubblewrap.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        vendor_dir.join("bind-mount.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        vendor_dir.join("network.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        vendor_dir.join("utils.c").display()
    );

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "linux" {
        return;
    }

    if let Err(err) = try_build_vendored_bwrap() {
        panic!("failed to compile vendored bubblewrap for Linux target: {err}");
    }
}

fn try_build_vendored_bwrap() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|err| err.to_string())?);
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let src_dir = resolve_bwrap_source_dir(&manifest_dir)?;

    let config_h = out_dir.join("config.h");
    std::fs::write(
        &config_h,
        r#"#pragma once
#define PACKAGE_STRING "bubblewrap built at codex build-time"
"#,
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
    match pkg_config::Config::new().probe("libcap") {
        Ok(libcap) => {
            for include_path in libcap.include_paths {
                build.include(include_path);
            }
        }
        Err(err) => {
            let running_under_bazel = env::var_os("RUNFILES_MANIFEST_ONLY").is_some();
            if !running_under_bazel {
                return Err(format!("libcap not available via pkg-config: {err}"));
            }

            // Bazel/RBE sandboxes may not provide pkg-config or a target libcap.pc,
            // even though the build otherwise has enough headers to compile bwrap.
            // We only need libcap for parsing --cap-add/--cap-drop names (cap_from_name).
            // Provide a tiny compatibility implementation for that symbol.
            let shim_include_dir = out_dir.join("sys");
            fs::create_dir_all(&shim_include_dir).map_err(|create_err| {
                format!(
                    "failed to create libcap shim include dir {}: {create_err}",
                    shim_include_dir.display()
                )
            })?;
            let shim_header = shim_include_dir.join("capability.h");
            fs::write(
                &shim_header,
                r#"#pragma once
#include <linux/capability.h>

typedef int cap_value_t;
int cap_from_name(const char *name, cap_value_t *cap);
"#,
            )
            .map_err(|write_err| {
                format!(
                    "failed to write libcap shim header {}: {write_err}",
                    shim_header.display()
                )
            })?;
            let shim_source = out_dir.join("libcap_compat.c");
            fs::write(
                &shim_source,
                r#"#include <errno.h>
#include <stddef.h>
#include <sys/capability.h>

int cap_from_name(const char *name, cap_value_t *cap) {
  (void)name;
  if (cap != NULL) {
    *cap = -1;
  }
  errno = EINVAL;
  return -1;
}
"#,
            )
            .map_err(|write_err| {
                format!(
                    "failed to write libcap shim source {}: {write_err}",
                    shim_source.display()
                )
            })?;
            build.file(&shim_source);
            println!(
                "cargo:warning=libcap pkg-config unavailable in Bazel build ({err}); using cap_from_name compatibility shim"
            );
        }
    }

    build.compile("build_time_bwrap");
    println!("cargo:rustc-cfg=vendored_bwrap_available");
    Ok(())
}

/// Resolve the bubblewrap source directory used for build-time compilation.
///
/// Priority:
/// 1. `CODEX_BWRAP_SOURCE_DIR` points at an existing bubblewrap checkout.
/// 2. The vendored bubblewrap tree under `codex-rs/vendor/bubblewrap`.
fn resolve_bwrap_source_dir(manifest_dir: &Path) -> Result<PathBuf, String> {
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

    let vendor_dir = manifest_dir.join("../vendor/bubblewrap");
    if vendor_dir.exists() {
        return Ok(vendor_dir);
    }

    Err(format!(
        "expected vendored bubblewrap at {}, but it was not found.\n\
Set CODEX_BWRAP_SOURCE_DIR to an existing checkout or vendor bubblewrap under codex-rs/vendor.",
        vendor_dir.display()
    ))
}
