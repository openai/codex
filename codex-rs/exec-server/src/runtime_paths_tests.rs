use super::ExecServerRuntimePaths;
use super::prepend_package_path;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

#[test]
fn discovers_package_path_from_codex_executable() {
    let temp_dir = TempDir::new().expect("temp dir");
    let package_dir = temp_dir.path();
    let bin_dir = package_dir.join("bin");
    let package_path_dir = package_dir.join("codex-path");
    fs::create_dir(&bin_dir).expect("bin dir");
    fs::create_dir(&package_path_dir).expect("package path dir");
    fs::write(package_dir.join("codex-package.json"), "{}").expect("metadata");
    let codex_exe = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
    fs::write(&codex_exe, "codex").expect("codex executable");

    let runtime_paths =
        ExecServerRuntimePaths::new(codex_exe, /*codex_linux_sandbox_exe*/ None)
            .expect("runtime paths");
    let package_path_dir = fs::canonicalize(package_path_dir).expect("canonical package path");

    assert_eq!(
        runtime_paths
            .package_path_dir
            .as_ref()
            .map(|path| path.as_path()),
        Some(package_path_dir.as_path())
    );
}

#[test]
fn package_path_is_prepended_and_deduplicated() {
    let package_path = std::path::PathBuf::from("package-bin");
    let other_path = std::path::PathBuf::from("system-bin");
    let initial_path = std::env::join_paths([
        other_path.as_path(),
        package_path.as_path(),
        package_path.as_path(),
    ])
    .expect("initial path");
    let mut env = HashMap::from([(
        "PATH".to_string(),
        initial_path.to_string_lossy().into_owned(),
    )]);

    prepend_package_path(&mut env, &package_path);

    let path = env.get("PATH").expect("PATH");
    assert_eq!(
        std::env::split_paths(path).collect::<Vec<_>>(),
        vec![package_path, other_path]
    );
}

#[test]
fn package_path_preserves_missing_path() {
    let mut env = HashMap::from([("ONLY_THIS".to_string(), "1".to_string())]);

    prepend_package_path(&mut env, std::path::Path::new("package-bin"));

    assert_eq!(
        env,
        HashMap::from([("ONLY_THIS".to_string(), "1".to_string())])
    );
}
