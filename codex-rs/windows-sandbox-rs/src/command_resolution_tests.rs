use super::resolve_windows_executable;
use super::resolve_windows_launch;
use crate::process::WindowsProcessLaunch;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn unresolved(program: &str) -> WindowsProcessLaunch {
    WindowsProcessLaunch {
        application_path: None,
        command: vec![program.to_string(), "argument".to_string()],
    }
}

#[test]
fn resolves_with_case_insensitive_child_path_and_pathext() {
    let temp = TempDir::new().expect("tempdir");
    let cwd = temp.path().join("cwd");
    let tools = temp.path().join("tools");
    fs::create_dir_all(&cwd).expect("create cwd");
    fs::create_dir_all(&tools).expect("create tools");
    let executable = tools.join("child-only.BIN");
    fs::write(&executable, b"fixture").expect("write executable fixture");
    let env = HashMap::from([
        ("Path".to_string(), tools.to_string_lossy().into_owned()),
        ("PathExt".to_string(), ".BIN".to_string()),
    ]);

    let launch = resolve_windows_launch(unresolved("child-only"), &cwd, &env)
        .expect("resolve from child environment");

    assert_eq!(
        launch,
        WindowsProcessLaunch {
            application_path: Some(executable),
            command: vec!["child-only".to_string(), "argument".to_string()],
        }
    );
}

#[test]
fn requested_cwd_precedes_child_path() {
    let temp = TempDir::new().expect("tempdir");
    let cwd = temp.path().join("cwd");
    let tools = temp.path().join("tools");
    fs::create_dir_all(&cwd).expect("create cwd");
    fs::create_dir_all(&tools).expect("create tools");
    let cwd_executable = cwd.join("tool.EXE");
    fs::write(&cwd_executable, b"cwd").expect("write cwd fixture");
    fs::write(tools.join("tool.EXE"), b"path").expect("write PATH fixture");
    let env = HashMap::from([("PATH".to_string(), tools.to_string_lossy().into_owned())]);

    assert_eq!(
        resolve_windows_executable(&["tool".to_string()], &cwd, &env).expect("resolve executable"),
        cwd_executable
    );
}

#[test]
fn keeps_extended_length_paths_unchanged() {
    let executable = dunce::canonicalize(std::env::current_exe().expect("current executable"))
        .expect("canonical current executable");
    let executable_text = executable.to_string_lossy();
    let extended = if executable_text.starts_with(r"\\?\") {
        executable
    } else if let Some(unc) = executable_text.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{unc}"))
    } else {
        PathBuf::from(format!(r"\\?\{executable_text}"))
    };
    let cwd = std::env::current_dir().expect("current dir");

    assert_eq!(
        resolve_windows_executable(
            &[extended.to_string_lossy().into_owned()],
            &cwd,
            &HashMap::new(),
        )
        .expect("resolve extended-length executable"),
        extended
    );
}

#[test]
fn does_not_fall_back_to_the_parent_path() {
    let temp = TempDir::new().expect("tempdir");
    let cwd = temp.path();
    let env = HashMap::from([
        ("PATH".to_string(), String::new()),
        ("PATHEXT".to_string(), ".EXE".to_string()),
    ]);

    let err = resolve_windows_executable(&["cmd".to_string()], cwd, &env)
        .expect_err("parent PATH must not be consulted");

    assert!(err.to_string().contains("child PATH and PATHEXT"));
}

#[test]
fn rejects_batch_files_instead_of_skipping_to_a_later_candidate() {
    let temp = TempDir::new().expect("tempdir");
    let cwd = temp.path().join("cwd");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    for directory in [&cwd, &first, &second] {
        fs::create_dir_all(directory).expect("create directory");
    }
    fs::write(first.join("tool.CMD"), b"@exit /b 0\r\n").expect("write batch fixture");
    fs::write(second.join("tool.EXE"), b"fixture").expect("write executable fixture");
    let env = HashMap::from([
        (
            "PATH".to_string(),
            std::env::join_paths([first, second])
                .expect("join PATH")
                .to_string_lossy()
                .into_owned(),
        ),
        ("PATHEXT".to_string(), ".CMD;.EXE".to_string()),
    ]);

    let err = resolve_windows_executable(&["tool".to_string()], &cwd, &env)
        .expect_err("batch candidate should fail closed");

    assert!(err.to_string().contains("must be launched through cmd.exe"));
}
