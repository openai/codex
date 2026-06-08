use anyhow::Context;
use codex_config::CONFIG_TOML_FILE;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::exec::ExecCapturePolicy;
use codex_core::exec::ExecParams;
use codex_core::exec::process_exec_tool_call;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::PathExt;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

enum TestCodexHome {
    Persistent(PathBuf),
    Temporary(TempDir),
}

impl TestCodexHome {
    fn path(&self) -> &Path {
        match self {
            Self::Persistent(path) => path.as_path(),
            Self::Temporary(temp_dir) => temp_dir.path(),
        }
    }
}

fn codex_home_for_windows_sandbox_test(name: &str) -> anyhow::Result<TestCodexHome> {
    if let Some(test_tmpdir) = std::env::var_os("TEST_TMPDIR") {
        // The elevated backend provisions machine-local sandbox users. Bazel
        // retries run in the same Windows VM, so keep CODEX_HOME stable within
        // the test temp root and let setup reconcile its persisted ACL state.
        let codex_home = PathBuf::from(test_tmpdir).join(name);
        std::fs::create_dir_all(&codex_home)
            .with_context(|| format!("create stable test CODEX_HOME {}", codex_home.display()))?;
        return Ok(TestCodexHome::Persistent(codex_home));
    }

    Ok(TestCodexHome::Temporary(TempDir::new()?))
}

fn stage_windows_sandbox_helpers() -> anyhow::Result<()> {
    let test_exe = std::env::current_exe().context("resolve current Windows test executable")?;
    let test_exe_dir = test_exe
        .parent()
        .context("Windows test executable should have a parent directory")?;
    let resources_dir = test_exe_dir.join("codex-resources");
    match std::fs::create_dir_all(&resources_dir) {
        Ok(()) => {}
        Err(err)
            if err.kind() == std::io::ErrorKind::PermissionDenied && resources_dir.is_dir() => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("create resources dir {}", resources_dir.display()));
        }
    }
    for helper_name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
        let helper = codex_utils_cargo_bin::cargo_bin(helper_name)?;
        let file_name = Path::new(helper_name).with_extension("exe");
        let destination = resources_dir.join(file_name);
        if let Err(err) = std::fs::copy(&helper, &destination) {
            // A sandbox helper can briefly remain alive after the sandboxed
            // command exits. Bazel may retry the test while that process still
            // has the staged executable open, so keep the already-staged copy.
            if err.kind() == std::io::ErrorKind::PermissionDenied && destination.exists() {
                continue;
            }
            return Err(err).with_context(|| {
                format!(
                    "stage Windows sandbox helper {} at {}",
                    helper.display(),
                    destination.display()
                )
            });
        }
    }
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_restricted_token_rejects_exact_and_glob_deny_read_policy() -> anyhow::Result<()> {
    let codex_home =
        codex_home_for_windows_sandbox_test("windows-restricted-token-deny-read-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let secret = cwd.join("secret.env");
    let future_secret = cwd.join("future.env");
    let public = cwd.join("public.txt");
    std::fs::write(&secret, "glob secret\n")?;
    std::fs::write(&public, "public ok\n")?;

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: future_secret,
            },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let err = process_exec_tool_call(
        ExecParams {
            command: vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "type secret.env >NUL 2>NUL & echo exact secret 1>future.env 2>NUL & type future.env 2>NUL & type public.txt & exit /B 0"
                    .to_string(),
            ],
            cwd: cwd.clone(),
            expiration: 10_000.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            env: HashMap::new(),
            network: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
            windows_sandbox_private_desktop: false,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        std::slice::from_ref(&cwd),
        &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await
    .expect_err("restricted-token sandbox should reject deny-read restrictions");

    assert_eq!(
        err.to_string(),
        "unsupported operation: windows unelevated restricted-token sandbox cannot enforce deny-read restrictions directly; refusing to run unsandboxed"
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_enforces_exact_and_glob_deny_read_policy() -> anyhow::Result<()> {
    let codex_home = codex_home_for_windows_sandbox_test("windows-elevated-deny-read-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let glob_secret = cwd.join("secret.env");
    let exact_secret = cwd.join("exact-secret.txt");
    let public = cwd.join("public.txt");
    std::fs::write(&glob_secret, "glob secret\n")?;
    std::fs::write(&exact_secret, "exact secret\n")?;
    std::fs::write(&public, "public ok\n")?;

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: exact_secret },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let ExecToolCallOutput {
        exit_code,
        stdout,
        stderr,
        ..
    } = process_exec_tool_call(
        ExecParams {
            command: vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "(type secret.env 1>NUL 2>NUL && echo GLOB-READ || echo GLOB-DENIED) & (type exact-secret.txt 1>NUL 2>NUL && echo EXACT-READ || echo EXACT-DENIED) & type public.txt".to_string(),
            ],
            cwd: cwd.clone(),
            expiration: 10_000.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            env: HashMap::new(),
            network: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            windows_sandbox_private_desktop: false,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        std::slice::from_ref(&cwd),
        &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await?;

    assert_eq!(exit_code, 0, "sandboxed command should complete");
    assert!(
        stdout.text.contains("GLOB-DENIED"),
        "glob deny-read should block the secret: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("GLOB-READ"),
        "glob deny-read should not allow the secret: {stdout:?}"
    );
    assert!(
        stdout.text.contains("EXACT-DENIED"),
        "exact deny-read should block the secret: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("EXACT-READ"),
        "exact deny-read should not allow the secret: {stdout:?}"
    );
    assert!(
        stdout.text.contains("public ok"),
        "allowed reads should still work: {stdout:?}"
    );
    assert_eq!(stderr.text, "");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn windows_elevated_enforces_managed_deny_read_for_shell_subprocess() -> anyhow::Result<()> {
    stage_windows_sandbox_helpers()?;
    let codex_home = Arc::new(TempDir::new()?);
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"sandbox_mode = "workspace-write"

[windows]
sandbox = "elevated"
"#,
    )?;

    let protected_dir = TempDir::new()?;
    let denied_path = protected_dir.path().join("secret.env");
    std::fs::write(&denied_path, "managed secret\n")?;
    let denied_path = dunce::canonicalize(denied_path)?.abs();
    let denied_path_toml = toml::Value::String(denied_path.to_string_lossy().into()).to_string();
    let requirements = format!(
        r#"[permissions.filesystem]
deny_read = [{denied_path_toml}]

[windows]
allowed_sandbox_implementations = ["elevated"]
"#
    );

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
        )
        .with_windows_cmd_shell()
        .with_config(|config| {
            config.permissions.windows_sandbox_private_desktop = false;
        });
    let fixture = builder.build(&server).await?;

    assert_eq!(
        fixture.config.permissions.windows_sandbox_mode,
        Some(codex_config::types::WindowsSandboxModeToml::Elevated)
    );
    assert!(
        fixture
            .session_configured
            .permission_profile
            .file_system_sandbox_policy()
            .entries
            .iter()
            .any(|entry| {
                entry.access == FileSystemAccessMode::Deny
                    && matches!(
                        &entry.path,
                        FileSystemPath::Path { path } if path == &denied_path
                    )
            }),
        "managed deny-read path should reach the configured session"
    );

    let call_id = "managed-deny-read";
    let command = format!(
        "type \"{}\" 1>NUL 2>NUL && echo SECRET-READ || echo SECRET-DENIED",
        denied_path.display()
    );
    let args = json!({
        "command": command,
        "login": false,
        "timeout_ms": 10_000,
    });
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    fixture
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "read the fixture files".into(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&fixture.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output = request_log
        .function_call_output_text(call_id)
        .context("shell output present")?;
    assert!(
        output.contains("SECRET-DENIED"),
        "managed deny-read should block the subprocess read: {output}"
    );
    assert!(
        !output.contains("SECRET-READ"),
        "managed deny-read must not allow the subprocess read: {output}"
    );
    assert!(
        !output.contains("managed secret"),
        "denied file contents leaked into shell output: {output}"
    );
    Ok(())
}
