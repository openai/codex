#![cfg(target_os = "windows")]

use codex_windows_sandbox::log_note;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::os::windows::fs::MetadataExt as _;
use std::os::windows::process::CommandExt as _;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

fn junction_name_for_path(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn junction_root_for_userprofile(userprofile: &str) -> PathBuf {
    PathBuf::from(userprofile)
        .join(".codex")
        .join(".sandbox")
        .join("cwd")
}

pub fn create_cwd_junction(requested_cwd: &Path, log_dir: Option<&Path>) -> Option<PathBuf> {
    let userprofile = std::env::var("USERPROFILE").ok()?;
    let junction_root = junction_root_for_userprofile(&userprofile);
    if let Err(err) = std::fs::create_dir_all(&junction_root) {
        log_note(
            &format!(
                "junction: failed to create {}: {err}",
                junction_root.display()
            ),
            log_dir,
        );
        return None;
    }

    let junction_path = junction_root.join(junction_name_for_path(requested_cwd));
    if junction_path.exists() {
        // Reuse an existing junction if it looks like a reparse point; this keeps the hot path
        // cheap and avoids repeatedly shelling out to mklink.
        match std::fs::symlink_metadata(&junction_path) {
            Ok(md) if (md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 => {
                log_note(
                    &format!("junction: reusing existing {}", junction_path.display()),
                    log_dir,
                );
                return Some(junction_path);
            }
            Ok(_) => {
                // Unexpected: something else exists at this path (regular directory/file). We'll
                // try to remove it if possible and recreate the junction.
                log_note(
                    &format!(
                        "junction: existing path is not a reparse point, recreating {}",
                        junction_path.display()
                    ),
                    log_dir,
                );
            }
            Err(err) => {
                log_note(
                    &format!(
                        "junction: failed to stat existing {}: {err}",
                        junction_path.display()
                    ),
                    log_dir,
                );
                return None;
            }
        }

        if let Err(err) = std::fs::remove_dir(&junction_path) {
            log_note(
                &format!(
                    "junction: failed to remove existing {}: {err}",
                    junction_path.display()
                ),
                log_dir,
            );
            return None;
        }
    }

    let link = junction_path.to_string_lossy().to_string();
    let target = requested_cwd.to_string_lossy().to_string();

    // Use `cmd /c` so we can call `mklink` (a cmd builtin). We must quote paths so CWDs
    // containing spaces work reliably.
    //
    // IMPORTANT: `std::process::Command::args()` will apply Windows quoting/escaping rules when
    // constructing the command line. Passing a single argument that itself contains quotes can
    // confuse `cmd.exe` and cause mklink to fail with "syntax is incorrect". We avoid that by
    // using `raw_arg` to pass the tokens exactly as `cmd.exe` expects to parse them.
    //
    // Paths cannot contain quotes on Windows, so no extra escaping is needed here.
    let link_quoted = format!("\"{link}\"");
    let target_quoted = format!("\"{target}\"");
    log_note(
        &format!("junction: creating via cmd /c mklink /J {link_quoted} {target_quoted}"),
        log_dir,
    );
    let output = match std::process::Command::new("cmd")
        .raw_arg("/c")
        .raw_arg("mklink")
        .raw_arg("/J")
        .raw_arg(&link_quoted)
        .raw_arg(&target_quoted)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            log_note(&format!("junction: mklink failed to run: {err}"), log_dir);
            return None;
        }
    };
    if output.status.success() && junction_path.exists() {
        log_note(
            &format!(
                "junction: created {} -> {}",
                junction_path.display(),
                requested_cwd.display()
            ),
            log_dir,
        );
        return Some(junction_path);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    log_note(
        &format!(
            "junction: mklink failed status={:?} stdout={} stderr={}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ),
        log_dir,
    );
    None
}
