use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_cargo_bin::repo_root;
use codex_utils_cargo_bin::runfiles_available;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

const STABLE_EXPORT_BIN: &str = "codex-app-server-protocol-stable-export";

#[test]
fn stable_ts_types_match_generation() -> Result<()> {
    let repo_root = repo_root()?;
    let codex_rs = repo_root.join("codex-rs");
    let hint = "run: just write-app-server-protocol-stable";
    let expected_dir = codex_utils_cargo_bin::find_resource!("stable").with_context(|| {
        "failed to resolve stable TypeScript artifacts in this package; ".to_owned() + hint
    })?;

    let temp_dir = TempDir::new()?;
    let out_dir = temp_dir.path().join("generated");

    let output = if option_env!("BAZEL_PACKAGE").is_some() || runfiles_available() {
        let stable_export_bin = cargo_bin(STABLE_EXPORT_BIN)
            .context("failed to resolve stable-export binary via cargo_bin")?;
        Command::new(stable_export_bin)
            .current_dir(&codex_rs)
            .args(["--out"])
            .arg(&out_dir)
            .output()
            .context("failed to run stable-export binary to generate artifacts")?
    } else {
        // Use a separate cargo invocation with an isolated target dir to avoid
        // workspace feature unification (notably `codex-experimental-api`).
        let target_dir = temp_dir.path().join("cargo-target");
        Command::new("cargo")
            .current_dir(&codex_rs)
            .env("CARGO_TARGET_DIR", &target_dir)
            .args(["run", "-p", STABLE_EXPORT_BIN, "--", "--out"])
            .arg(&out_dir)
            .output()
            .context("failed to run cargo to generate stable app-server protocol artifacts")?
    };

    if !output.status.success() {
        let status = output.status;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("cargo run failed with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    let expected = collect_files(&expected_dir, "ts")?;
    let generated = collect_files(&out_dir, "ts")?;
    let diffs = compare_maps(&expected, &generated, "ts");

    let assert_hint = "If this fails, run: just write-app-server-protocol-stable";
    assert_eq!(diffs, Vec::<String>::new(), "{assert_hint}");
    Ok(())
}

fn compare_maps(
    expected: &BTreeMap<PathBuf, Vec<u8>>,
    generated: &BTreeMap<PathBuf, Vec<u8>>,
    label: &str,
) -> Vec<String> {
    let expected_paths: BTreeSet<PathBuf> = expected.keys().cloned().collect();
    let generated_paths: BTreeSet<PathBuf> = generated.keys().cloned().collect();

    let mut diffs = Vec::new();
    for missing in expected_paths.difference(&generated_paths) {
        let missing = missing.display();
        diffs.push(format!("missing generated {label} file: {missing}"));
    }
    for extra in generated_paths.difference(&expected_paths) {
        let extra = extra.display();
        diffs.push(format!("unexpected generated {label} file: {extra}"));
    }
    for path in expected_paths.intersection(&generated_paths) {
        let Some(expected_bytes) = expected.get(path) else {
            let path = path.display();
            diffs.push(format!(
                "expected {label} artifacts missing intersection file: {path}"
            ));
            continue;
        };
        let Some(generated_bytes) = generated.get(path) else {
            let path = path.display();
            diffs.push(format!(
                "generated {label} artifacts missing intersection file: {path}"
            ));
            continue;
        };
        if expected_bytes != generated_bytes {
            let path = path.display();
            diffs.push(format!("generated {label} file contents differ: {path}"));
        }
    }

    diffs
}

fn collect_files(root: &Path, extension: &str) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path
                .extension()
                .is_some_and(|ext| ext == OsStr::new(extension))
            {
                let rel_path = path.strip_prefix(root)?;
                let bytes = fs::read(&path)?;
                files.insert(rel_path.to_path_buf(), bytes);
            }
        }
    }
    Ok(files)
}
