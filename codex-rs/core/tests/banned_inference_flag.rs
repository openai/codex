use std::path::Path;

use walkdir::DirEntry;
use walkdir::WalkDir;

#[test]
fn chatgpt_code_inference_safety_flag_is_absent() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("expected workspace root to be discoverable");
    const BANNED_FLAG: [u8; 38] = [
        99, 104, 97, 116, 103, 112, 116, 95, 99, 111, 100, 101, 95, 116, 117, 114, 110, 95, 111,
        102, 102, 95, 105, 110, 102, 101, 114, 101, 110, 99, 101, 95, 115, 97, 102, 101, 116, 121,
    ];
    let skip_dirs = [
        ".git",
        "node_modules",
        ".pnpm-store",
        "target",
        "dist",
        "build",
        "v8-compile-cache-0",
    ];
    let mut offenders = Vec::new();

    for entry in WalkDir::new(repo_root)
        .into_iter()
        .filter_entry(|entry| should_descend(entry, &skip_dirs))
    {
        let entry = entry.expect("failed to walk repository tree");
        if !entry.file_type().is_file() {
            continue;
        }

        let data = std::fs::read(entry.path()).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", entry.path().display());
        });

        if data
            .windows(BANNED_FLAG.len())
            .any(|window| window == BANNED_FLAG)
        {
            let rel = entry
                .path()
                .strip_prefix(repo_root)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| entry.path().display().to_string());
            offenders.push(rel);
        }
    }

    if !offenders.is_empty() {
        panic!(
            "found deprecated ChatGPT Code inference-safety override in: {}",
            offenders.join(", ")
        );
    }
}

fn should_descend(entry: &DirEntry, skip_dirs: &[&str]) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }

    let name = entry.file_name().to_string_lossy();
    !skip_dirs.iter().any(|candidate| candidate == &name)
}
