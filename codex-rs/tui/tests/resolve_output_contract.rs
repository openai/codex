use std::env;
use std::path::PathBuf;
use std::process::Command;

fn find_resolver_from_manifest() -> PathBuf {
    // Walk up from this crate towards the workspace root and locate scripts/resolve_safe_sync.sh.
    // Stop if we reach a directory containing a .git folder and we still couldn't find it.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        let cand = dir.join("scripts/resolve_safe_sync.sh");
        if cand.exists() {
            return cand;
        }
        // Bound the walk by .git or Cargo.toml typical of a workspace root
        if dir.join(".git").exists() || dir.join("Cargo.toml").exists() {
            // If not found at this level, break so the panic provides guidance
            break;
        }
        if !dir.pop() { break; }
    }
    panic!(
        "could not locate scripts/resolve_safe_sync.sh from {}; run tests via the workspace root (cargo test --workspace)",
        env!("CARGO_MANIFEST_DIR")
    );
}

#[test]
fn resolver_output_contract_exact_keys() {
    // Create a minimal layout with root-only scripts so resolver emits paths.
    let tmp = tempfile::Builder::new()
        .prefix("safe-sync-contract-")
        .tempdir()
        .expect("tmp");
    let root = tmp.path();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    let target = root.join("scripts/safe_sync_merge.sh");
    std::fs::write(&target, b"#!/usr/bin/env bash\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let resolver = find_resolver_from_manifest();
    let out = Command::new("bash")
        .arg(&resolver)
        .arg("--root")
        .arg(root)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("run resolver");
    assert!(out.status.success(), "resolver failed: {:?} stderr={}", out.status, String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Collect keys and assert contract (no extra keys; no trailing spaces)
    let mut keys = vec![];
    for line in stdout.lines() {
        assert!(!line.ends_with(' ') && !line.ends_with('\t'), "trailing whitespace in line: {:?}", line);
        let mut parts = line.splitn(2, '=');
        let k = parts.next().unwrap();
        let _v = parts.next().unwrap_or("");
        keys.push(k.to_string());
    }
    keys.sort();
    assert_eq!(keys, vec![
        "HAS_CODEX_RS".to_string(),
        "SAFE_SYNC".to_string(),
        "TEST_SCRIPT".to_string(),
        "WORKSPACE_PRESENT".to_string(),
    ]);
}
