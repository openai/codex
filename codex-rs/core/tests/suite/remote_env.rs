use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::RemoveOptions;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::PathBufExt;
use core_test_support::get_remote_test_env;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_abs = remote_test_file_path().abs();
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_abs, payload.clone(), /*sandbox*/ None)
        .await?;
    let actual = file_system
        .read_file(&file_path_abs, /*sandbox*/ None)
        .await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_abs,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
    match AbsolutePathBuf::try_from(path) {
        Ok(path) => path,
        Err(error) => panic!("path should be absolute: {error}"),
    }
}

fn read_only_sandbox(readable_root: PathBuf) -> FileSystemSandboxContext {
    let readable_root = absolute_path(readable_root);
    // The policy is evaluated in the remote container, so use a container path
    // for cwd instead of capturing the local test runner cwd.
    FileSystemSandboxContext::from_legacy_sandbox_policy(
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![readable_root.clone()],
            },
            network_access: false,
        },
        readable_root,
    )
}

fn workspace_write_sandbox(writable_root: PathBuf) -> FileSystemSandboxContext {
    let writable_root = absolute_path(writable_root);
    // The policy is evaluated in the remote container, so use a container path
    // for cwd instead of capturing the local test runner cwd.
    FileSystemSandboxContext::from_legacy_sandbox_policy(
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![writable_root.clone()],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![],
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
        writable_root,
    )
}

fn assert_normalized_path_rejected(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected not-found message: {error}",
        ),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected rejection message: {message}",
            );
        }
        other => panic!("unexpected normalized-path error kind: {other:?}: {error:?}"),
    }
}

fn remote_exec(script: &str) -> Result<()> {
    let remote_env = get_remote_test_env().context("remote env should be configured")?;
    let output = Command::new("docker")
        .args(["exec", &remote_env.container_name, "sh", "-lc", script])
        .output()?;
    assert!(
        output.status.success(),
        "remote exec failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_allows_readable_root() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let allowed_dir = PathBuf::from(format!("/tmp/codex-remote-readable-{}", std::process::id()));
    let file_path = allowed_dir.join("note.txt");
    file_system
        .create_directory(
            &absolute_path(allowed_dir.clone()),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    file_system
        .write_file(
            &absolute_path(file_path.clone()),
            b"sandboxed hello".to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let sandbox = read_only_sandbox(allowed_dir.clone());
    let contents = file_system
        .read_file(&absolute_path(file_path.clone()), Some(&sandbox))
        .await?;
    assert_eq!(contents, b"sandboxed hello");

    file_system
        .remove(
            &absolute_path(allowed_dir),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_rejects_symlink_parent_dotdot_escape() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!("/tmp/codex-remote-dotdot-{}", std::process::id()));
    let allowed_dir = root.join("allowed");
    let outside_dir = root.join("outside");
    let secret_path = root.join("secret.txt");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside}; printf nope > {secret}; ln -s {outside} {allowed}/link",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside = outside_dir.display(),
        secret = secret_path.display(),
    ))?;

    let requested_path = absolute_path(allowed_dir.join("link").join("..").join("secret.txt"));
    let sandbox = read_only_sandbox(allowed_dir.clone());
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    assert_normalized_path_rejected(&error);

    remote_exec(&format!("rm -rf {}", root.display()))?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_remove_removes_symlink_not_target() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-remove-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("keep.txt");
    let symlink_path = allowed_dir.join("link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {symlink}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = absolute_path(
            outside_file
                .parent()
                .context("outside parent should exist")?
                .to_path_buf(),
        )
        .display(),
        outside = outside_file.display(),
        symlink = symlink_path.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .remove(
            &absolute_path(symlink_path.clone()),
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await?;

    let symlink_exists = file_system
        .get_metadata(&absolute_path(symlink_path), /*sandbox*/ None)
        .await
        .is_ok();
    assert!(!symlink_exists);
    let outside = file_system
        .read_file_text(&absolute_path(outside_file.clone()), /*sandbox*/ None)
        .await?;
    assert_eq!(outside, "outside");

    file_system
        .remove(
            &absolute_path(root),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_copy_preserves_symlink_source() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-copy-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("outside.txt");
    let source_symlink = allowed_dir.join("link");
    let copied_symlink = allowed_dir.join("copied-link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {source}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = outside_file.parent().expect("outside parent").display(),
        outside = outside_file.display(),
        source = source_symlink.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .copy(
            &absolute_path(source_symlink),
            &absolute_path(copied_symlink.clone()),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await?;

    let link_target = Command::new("docker")
        .args([
            "exec",
            &get_remote_test_env()
                .context("remote env should still be configured")?
                .container_name,
            "readlink",
            copied_symlink
                .to_str()
                .context("copied symlink path should be utf-8")?,
        ])
        .output()?;
    assert!(
        link_target.status.success(),
        "readlink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&link_target.stdout).trim(),
        String::from_utf8_lossy(&link_target.stderr).trim(),
    );
    assert_eq!(
        String::from_utf8_lossy(&link_target.stdout).trim(),
        outside_file.to_string_lossy()
    );

    file_system
        .remove(
            &absolute_path(root),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

fn remote_test_file_path() -> PathBuf {
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    PathBuf::from(format!(
        "/tmp/codex-remote-test-env-{}-{nanos}.txt",
        std::process::id()
    ))
}
