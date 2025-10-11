use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;

fn file_hash(path: &str) -> u64 {
    match fs::read_to_string(path) {
        Ok(s) => {
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            hasher.finish()
        }
        Err(_) => 0,
    }
}

fn main() {
    // Ensure Cargo rebuilds this crate when compact templates change.
    const FILES: &[&str] = &[
        "templates/compact/prompt.md",
        "templates/compact/history_bridge.md",
    ];

    for f in FILES {
        println!("cargo:rerun-if-changed={f}");
    }

    // Also feed a hash into rustc env to guarantee recompilation when content changes.
    // This avoids relying solely on rerun-if-changed behavior.
    let combined_hash: u64 = FILES.iter().map(|f| file_hash(f)).fold(0, |acc, h| acc ^ h);
    println!("cargo:rustc-env=COMPACT_TEMPLATES_HASH={combined_hash}");

    // Provide an absolute path env for debugging if useful.
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let abs: Vec<String> = FILES
            .iter()
            .map(|f| PathBuf::from(&manifest_dir).join(f).display().to_string())
            .collect();
        println!("cargo:rustc-env=COMPACT_TEMPLATES_PATHS={}", abs.join(";"));
    }
}
