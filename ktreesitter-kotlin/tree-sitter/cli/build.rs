use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

fn main() {
    if let Some(git_sha) = read_git_sha() {
        println!("cargo:rustc-env=BUILD_SHA={git_sha}");
    }

    println!("cargo:rustc-check-cfg=cfg(sanitizing)");
    println!("cargo:rustc-check-cfg=cfg(TREE_SITTER_EMBED_WASM_BINDING)");

    if web_playground_files_present() {
        println!("cargo:rustc-cfg=TREE_SITTER_EMBED_WASM_BINDING");
    }

    let build_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    println!("cargo:rustc-env=BUILD_TIME={build_time}");

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("dynamic-symbols.txt");
        std::fs::write(
            &out_dir,
            "{
                ts_current_malloc;
                ts_current_calloc;
                ts_current_realloc;
                ts_current_free;
            };",
        )
        .unwrap();
        println!(
            "cargo:rustc-link-arg=-Wl,--dynamic-list={}",
            out_dir.display()
        );
    }
}

fn web_playground_files_present() -> bool {
    let paths = [
        "../docs/src/assets/js/playground.js",
        "../lib/binding_web/tree-sitter.js",
        "../lib/binding_web/tree-sitter.wasm",
    ];

    paths.iter().all(|p| Path::new(p).exists())
}

fn read_git_sha() -> Option<String> {
    let crate_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    if !crate_path.parent().is_some_and(|p| p.join(".git").exists()) {
        return None;
    }

    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(crate_path)
        .output()
        .map_or(None, |output| {
            if !output.status.success() {
                return None;
            }
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        })
}
