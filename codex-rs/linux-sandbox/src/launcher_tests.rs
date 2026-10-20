//! Verify system bubblewrap selection and legacy fd mount translation.

use super::*;
use pretty_assertions::assert_eq;
use tempfile::NamedTempFile;

#[test]
fn prefers_system_bwrap_when_help_lists_argv0() {
    let fake_bwrap = NamedTempFile::new().expect("temp file");
    let fake_bwrap_path = fake_bwrap.path();
    let expected = AbsolutePathBuf::from_absolute_path(fake_bwrap_path).expect("absolute");

    assert_eq!(
        system_bwrap_launcher_for_path_with_probe(fake_bwrap_path, |_| {
            Some(SystemBwrapCapabilities {
                supports_argv0: true,
                supports_perms: true,
                supports_ro_bind_fd: true,
            })
        }),
        Some(SystemBwrapLauncher {
            program: expected,
            supports_argv0: true,
            supports_ro_bind_fd: true,
        })
    );
}

#[test]
fn prefers_system_bwrap_when_system_bwrap_lacks_argv0() {
    let fake_bwrap = NamedTempFile::new().expect("temp file");
    let fake_bwrap_path = fake_bwrap.path();

    assert_eq!(
        system_bwrap_launcher_for_path_with_probe(fake_bwrap_path, |_| {
            Some(SystemBwrapCapabilities {
                supports_argv0: false,
                supports_perms: true,
                supports_ro_bind_fd: false,
            })
        }),
        Some(SystemBwrapLauncher {
            program: AbsolutePathBuf::from_absolute_path(fake_bwrap_path).expect("absolute"),
            supports_argv0: false,
            supports_ro_bind_fd: false,
        })
    );
}

#[test]
fn ignores_system_bwrap_when_system_bwrap_lacks_perms() {
    let fake_bwrap = NamedTempFile::new().expect("temp file");

    assert_eq!(
        system_bwrap_launcher_for_path_with_probe(fake_bwrap.path(), |_| {
            Some(SystemBwrapCapabilities {
                supports_argv0: false,
                supports_perms: false,
                supports_ro_bind_fd: false,
            })
        }),
        None
    );
}

#[test]
fn detects_fd_backed_read_only_mount_support_in_system_bwrap_help() {
    let temp_dir = tempfile::tempdir().expect("temp directory");
    let fake_bwrap_path = temp_dir.path().join("bwrap");
    codex_utils_cargo_bin::write_executable(
        &fake_bwrap_path,
        "#!/bin/sh\nprintf '%s\\n' '--as-pid-1' '--perms' '--argv0' '--ro-bind-fd'\n",
    )
    .expect("write fake bubblewrap");

    assert_eq!(
        system_bwrap_capabilities(&fake_bwrap_path),
        Some(SystemBwrapCapabilities {
            supports_argv0: true,
            supports_perms: true,
            supports_ro_bind_fd: true,
        })
    );
}

#[test]
fn translates_fd_mounts_for_legacy_system_bubblewrap() {
    let mut argv = vec![
        "bwrap".to_string(),
        "--ro-bind-fd".to_string(),
        "7".to_string(),
        "/tmp/socket-root".to_string(),
        "--".to_string(),
        "/usr/bin/codex-linux-sandbox".to_string(),
        "--apply-seccomp-then-exec".to_string(),
        "--".to_string(),
        "echo".to_string(),
        "--ro-bind-fd".to_string(),
    ];

    translate_legacy_bwrap_fd_mounts(&mut argv).expect("fd mount should translate");

    assert_eq!(
        argv,
        vec![
            "bwrap".to_string(),
            "--ro-bind".to_string(),
            "/proc/self/fd/7".to_string(),
            "/tmp/socket-root".to_string(),
            "--".to_string(),
            "/usr/bin/codex-linux-sandbox".to_string(),
            "--verify-fd-mount".to_string(),
            "7:/tmp/socket-root".to_string(),
            "--apply-seccomp-then-exec".to_string(),
            "--".to_string(),
            "echo".to_string(),
            "--ro-bind-fd".to_string(),
        ]
    );
}

#[test]
fn translates_multiple_fd_mounts_for_legacy_system_bubblewrap() {
    let mut argv = vec![
        "bwrap".to_string(),
        "--ro-bind-fd".to_string(),
        "7".to_string(),
        "/tmp/first".to_string(),
        "--ro-bind-fd".to_string(),
        "9".to_string(),
        "/tmp/second:with-colon".to_string(),
        "--".to_string(),
        "codex-linux-sandbox".to_string(),
        "--apply-seccomp-then-exec".to_string(),
        "--".to_string(),
        "true".to_string(),
    ];

    translate_legacy_bwrap_fd_mounts(&mut argv).expect("fd mounts should translate");

    assert_eq!(
        argv,
        vec![
            "bwrap".to_string(),
            "--ro-bind".to_string(),
            "/proc/self/fd/7".to_string(),
            "/tmp/first".to_string(),
            "--ro-bind".to_string(),
            "/proc/self/fd/9".to_string(),
            "/tmp/second:with-colon".to_string(),
            "--".to_string(),
            "codex-linux-sandbox".to_string(),
            "--verify-fd-mount".to_string(),
            "7:/tmp/first".to_string(),
            "--verify-fd-mount".to_string(),
            "9:/tmp/second:with-colon".to_string(),
            "--apply-seccomp-then-exec".to_string(),
            "--".to_string(),
            "true".to_string(),
        ]
    );
}

#[test]
fn rejects_malformed_legacy_bubblewrap_fd_mounts() {
    for (fd, destination) in [
        ("invalid", "/tmp/socket-root"),
        ("2", "/tmp/socket-root"),
        ("7", "relative/socket-root"),
    ] {
        let mut argv = vec![
            "bwrap".to_string(),
            "--ro-bind-fd".to_string(),
            fd.to_string(),
            destination.to_string(),
            "--".to_string(),
            "codex-linux-sandbox".to_string(),
        ];

        assert!(translate_legacy_bwrap_fd_mounts(&mut argv).is_err());
    }
}

#[test]
fn rejects_fd_mounts_without_the_trusted_inner_sandbox_stage() {
    let mut argv = vec![
        "bwrap".to_string(),
        "--ro-bind-fd".to_string(),
        "7".to_string(),
        "/tmp/socket-root".to_string(),
        "--".to_string(),
        "/bin/true".to_string(),
        "--".to_string(),
        "--apply-seccomp-then-exec".to_string(),
    ];

    assert!(translate_legacy_bwrap_fd_mounts(&mut argv).is_err());
}

#[test]
fn rejects_duplicate_legacy_bubblewrap_fd_mounts() {
    let mut argv = vec![
        "bwrap".to_string(),
        "--ro-bind-fd".to_string(),
        "7".to_string(),
        "/tmp/first".to_string(),
        "--ro-bind-fd".to_string(),
        "7".to_string(),
        "/tmp/second".to_string(),
        "--".to_string(),
        "codex-linux-sandbox".to_string(),
    ];

    assert!(translate_legacy_bwrap_fd_mounts(&mut argv).is_err());
}

#[test]
fn ignores_system_bwrap_when_system_bwrap_is_missing() {
    assert_eq!(
        system_bwrap_launcher_for_path(Path::new("/definitely/not/a/bwrap")),
        None
    );
}
