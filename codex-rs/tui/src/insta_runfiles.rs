use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;

#[ctor::ctor]
fn init_insta_runfiles() {
    sync_snapshots("_main/codex-rs/tui/");
}

fn sync_snapshots(prefix: &str) {
    let Some(manifest) = std::env::var_os("RUNFILES_MANIFEST_FILE") else {
        return;
    };
    let manifest = PathBuf::from(manifest);
    let Some(runfiles_root) = manifest.parent() else {
        return;
    };
    let Ok(file) = fs::File::open(&manifest) else {
        return;
    };
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        if !key.starts_with(prefix) || !key.ends_with(".snap") {
            continue;
        }

        let dest = runfiles_root.join(key);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent()
            && fs::create_dir_all(parent).is_err()
        {
            continue;
        }
        let _ = fs::copy(value, &dest);
    }
}
