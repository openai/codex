use std::{path::PathBuf, process::Command};

#[test]
fn workspace_deps_working() {
    let manifest_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/workspace_deps/Cargo.toml");

    assert!(Command::new("cargo")
        .arg("build")
        .arg("--all")
        .arg(format!("--manifest-path={}", manifest_dir.display()))
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success());
}
