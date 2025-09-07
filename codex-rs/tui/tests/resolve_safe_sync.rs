use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

fn mktempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("safe-sync-resolver-")
        .tempdir()
        .expect("temp dir")
}

fn find_resolver_from_manifest() -> PathBuf {
    // Start from this crate's manifest dir and walk up to find the workspace root
    // containing the canonical resolver at <workspace>/scripts/resolve_safe_sync.sh.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..5 {
        let candidate = dir.join("scripts/resolve_safe_sync.sh");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() { break; }
    }
    panic!("could not locate scripts/resolve_safe_sync.sh from manifest dir: {}", env!("CARGO_MANIFEST_DIR"));
}

fn run_resolver(root: &std::path::Path) -> String {
    // Run the resolver with the provided root path; capture stdout.
    let resolver = find_resolver_from_manifest();
    let mut cmd = Command::new("bash");
    cmd.arg(&resolver);
    cmd.arg("--root");
    cmd.arg(root);
    cmd.env("LC_ALL", "C");
    cmd.env("LANG", "C");
    let out = cmd.output().expect("run resolver");
    assert!(out.status.success(), "resolver failed: status={:?} stderr={}", out.status, String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn parse_var(s: &str, key: &str) -> Option<String> {
    s.lines()
        .find(|l| l.starts_with(&format!("{}=", key)))
        .map(|l| l[key.len() + 1..].to_string())
}

#[test]
fn resolver_prefers_codex_rs_when_both_exist() {
    let tmp = mktempdir();
    let root = tmp.path();

    // Create both paths: root/scripts and codex-rs/scripts
    let codex = root.join("codex-rs/scripts");
    let root_scripts = root.join("scripts");
    fs::create_dir_all(&codex).unwrap();
    fs::create_dir_all(&root_scripts).unwrap();
    let codex_target = codex.join("safe_sync_merge.sh");
    let root_target = root_scripts.join("safe_sync_merge.sh");
    fs::write(&codex_target, b"#!/usr/bin/env bash\nexit 0\n").unwrap();
    fs::write(&root_target, b"#!/usr/bin/env bash\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&codex_target, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&root_target, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let out = run_resolver(root);
    let safe_sync = parse_var(&out, "SAFE_SYNC").expect("SAFE_SYNC");
    assert_eq!(PathBuf::from(safe_sync), codex_target);
}

#[test]
fn resolver_falls_back_to_root_scripts() {
    let tmp = mktempdir();
    let root = tmp.path();

    let root_scripts = root.join("scripts");
    fs::create_dir_all(&root_scripts).unwrap();
    let root_target = root_scripts.join("safe_sync_merge.sh");
    fs::write(&root_target, b"#!/usr/bin/env bash\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root_target, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let out = run_resolver(root);
    let safe_sync = parse_var(&out, "SAFE_SYNC").expect("SAFE_SYNC");
    assert_eq!(PathBuf::from(safe_sync), root_target);
}

#[cfg(unix)]
#[test]
fn resolver_handles_detached_like_layout_with_symlink() {
    let tmp = mktempdir();
    let real = tmp.path().join("real");
    let altroot = tmp.path().join("altroot");
    std::fs::create_dir_all(real.join("codex-rs/scripts")).unwrap();
    std::fs::create_dir_all(&altroot).unwrap();
    // Create the target under real
    let codex_target = real.join("codex-rs/scripts/safe_sync_merge.sh");
    std::fs::write(&codex_target, b"#!/usr/bin/env bash\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&codex_target, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Symlink codex-rs into altroot to simulate a detached worktree-like layout
    std::os::unix::fs::symlink(real.join("codex-rs"), altroot.join("codex-rs")).unwrap();
    let out = run_resolver(&altroot);
    let safe_sync = parse_var(&out, "SAFE_SYNC").expect("SAFE_SYNC");
    assert_eq!(PathBuf::from(safe_sync), altroot.join("codex-rs/scripts/safe_sync_merge.sh"));
}

#[test]
fn resolver_exit_codes() {
    let tmp = mktempdir();
    let root = tmp.path();
    // Not-found under valid root → exit 2
    let resolver = find_resolver_from_manifest();
    let status_nf = Command::new("bash")
        .arg(&resolver)
        .arg("--root")
        .arg(root)
        .status()
        .expect("run");
    assert_eq!(status_nf.code(), Some(2));

    // Invalid root → exit 3
    let status_bad = Command::new("bash")
        .arg(&resolver)
        .arg("--root")
        .arg("/definitely/not/there")
        .status()
        .expect("run");
    assert_eq!(status_bad.code(), Some(3));
}
