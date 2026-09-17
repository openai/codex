mod common;
#[cfg(target_os = "linux")]
#[path = "common/fake_bwrap.rs"]
mod fake_bwrap;

#[cfg(target_os = "linux")]
use anyhow::Context as _;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_exec_server::CAPABILITY_ROOTS_DISCOVER_METHOD;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::CapabilityRootsDiscoverParams;
use codex_exec_server::CapabilityRootsDiscoverResponse;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::WindowsSandboxSelection;
use codex_exec_server_protocol::CapabilityRootDiscoverRequest;
use codex_exec_server_protocol::EXEC_METHOD;
#[cfg(unix)]
use codex_exec_server_protocol::EXEC_READ_METHOD;
use codex_exec_server_protocol::FS_OPEN_METHOD;
#[cfg(unix)]
use codex_exec_server_protocol::FS_READ_BLOCK_METHOD;
use codex_exec_server_protocol::FS_READ_FILE_METHOD;
#[cfg(unix)]
use codex_exec_server_protocol::FsReadBlockResponse;
use codex_exec_server_protocol::FsReadFileResponse;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
#[cfg(unix)]
use codex_exec_server_protocol::ReadResponse;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
#[cfg(target_os = "linux")]
use common::exec_server::exec_server_with_env;
#[cfg(target_os = "linux")]
use fake_bwrap::write_fake_bwrap;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_a_complete_capability_bundle_in_one_request() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    write_file(
        &root.path().join(".codex-plugin/plugin.json"),
        r#"{
  "name": "demo",
  "interface": {"displayName": "Demo Plugin"},
  "mcpServers": "./config/mcp.json",
  "apps": "./config/apps.json"
}"#,
    )?;
    write_file(
        &root.path().join(".claude-plugin/plugin.json"),
        r#"{"name":"lower-priority-claude"}"#,
    )?;
    write_file(
        &root.path().join(".cursor-plugin/plugin.json"),
        r#"{"name":"lower-priority-cursor"}"#,
    )?;
    write_file(
        &root.path().join("config/mcp.json"),
        r#"{"mcpServers":{"demo":{"command":"demo-server"}}}"#,
    )?;
    write_file(
        &root.path().join("config/apps.json"),
        r#"{"apps":{"demo":{"connector_id":"connector-demo"}}}"#,
    )?;
    write_file(
        &root.path().join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Deploy the service.\n---\n\nDeploy instructions.\n",
    )?;
    write_file(
        &root.path().join("skills/deploy/agents/openai.yaml"),
        "policy:\n  allow_implicit_invocation: false\n",
    )?;
    write_file(
        &root.path().join("nested/.claude-plugin/plugin.json"),
        r#"{"name":"nested"}"#,
    )?;
    write_file(
        &root.path().join("nested/skills/audit/SKILL.md"),
        "---\nname: audit\ndescription: Audit the service.\n---\n",
    )?;
    write_file(
        &root.path().join("nested-cursor/.cursor-plugin/plugin.json"),
        r#"{"name":"cursor-nested"}"#,
    )?;
    write_file(
        &root.path().join("nested-cursor/skills/review/SKILL.md"),
        "---\nname: review\ndescription: Review the service.\n---\n",
    )?;

    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let root_uri = PathUri::from_host_native_path(root.path())?;
    let discovery = discover_root(&mut server, "demo@1", root_uri.clone()).await?;

    assert_eq!(discovery.id, "demo@1");
    assert_eq!(discovery.path, root_uri);
    assert_eq!(discovery.error, None);
    assert_eq!(discovery.warnings, Vec::<String>::new());
    let plugin = discovery.plugin.as_ref().expect("root plugin");
    assert_eq!(
        plugin.manifest.path,
        root_uri.join(".codex-plugin/plugin.json")?
    );
    assert!(plugin.manifest.contents.contains("Demo Plugin"));
    assert_eq!(
        plugin.mcp_config.as_ref().map(|file| &file.path),
        Some(&root_uri.join("config/mcp.json")?)
    );
    assert_eq!(
        plugin.apps_config.as_ref().map(|file| &file.path),
        Some(&root_uri.join("config/apps.json")?)
    );
    assert_eq!(
        discovery
            .namespace_manifests
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
        vec![
            root_uri.join(".codex-plugin/plugin.json")?,
            root_uri.join("nested/.claude-plugin/plugin.json")?,
            root_uri.join("nested-cursor/.cursor-plugin/plugin.json")?,
        ]
    );
    assert_eq!(
        discovery
            .skills
            .iter()
            .map(|skill| (
                skill.instructions.path.clone(),
                skill
                    .metadata
                    .as_ref()
                    .map(|metadata| metadata.path.clone()),
            ))
            .collect::<Vec<_>>(),
        vec![
            (root_uri.join("nested-cursor/skills/review/SKILL.md")?, None,),
            (root_uri.join("nested/skills/audit/SKILL.md")?, None,),
            (
                root_uri.join("skills/deploy/SKILL.md")?,
                Some(root_uri.join("skills/deploy/agents/openai.yaml")?),
            ),
        ]
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_cursor_plugin_without_reading_default_mcp_for_inline_servers()
-> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    write_file(
        &root.path().join(".cursor-plugin/plugin.json"),
        r#"{"name":"cursor-demo","mcpServers":{"inline":{"command":"inline"}}}"#,
    )?;
    write_file(
        &root.path().join(".mcp.json"),
        r#"{"mcpServers":{"should-not-load":{"command":"wrong"}}}"#,
    )?;

    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let root_uri = PathUri::from_host_native_path(root.path())?;
    let discovery = discover_root(&mut server, "cursor@1", root_uri.clone()).await?;

    assert_eq!(discovery.error, None);
    assert_eq!(discovery.warnings, Vec::<String>::new());
    let plugin = discovery.plugin.expect("cursor plugin");
    assert_eq!(
        plugin.manifest.path,
        root_uri.join(".cursor-plugin/plugin.json")?
    );
    assert_eq!(plugin.mcp_config, None);
    assert_eq!(
        discovery
            .namespace_manifests
            .iter()
            .map(|manifest| manifest.path.clone())
            .collect::<Vec<_>>(),
        vec![root_uri.join(".cursor-plugin/plugin.json")?]
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn executor_legacy_missing_or_null_sandbox_cwd_keeps_absolute_requests_working()
-> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(workspace.path().join("note.txt"), b"contents")?;
    write_file(
        &workspace.path().join("skills/demo/SKILL.md"),
        "---\nname: demo\ndescription: Demo.\n---\n",
    )?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let path = cwd.join("note.txt")?;
    let program = std::env::current_exe()?.to_string_lossy().into_owned();
    // A process with sandbox intent needs an enforceable profile and an enabled Windows backend.
    let process_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Path { path: cwd.clone() },
            FileSystemAccessMode::Write,
        ),
    ]);
    let mut process_sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&process_policy, NetworkSandboxPolicy::Enabled),
        cwd.clone(),
    );
    process_sandbox.windows_sandbox_selection = WindowsSandboxSelection::RestrictedToken;
    let sandbox =
        FileSystemSandboxContext::from_permission_profile(PermissionProfile::Disabled, cwd.clone());
    let requests = [
        (
            EXEC_METHOD,
            serde_json::json!({
                "processId": "sandbox-cwd", "argv": [program, "--help"], "cwd": cwd,
                "env": {}, "tty": false, "sandbox": process_sandbox,
            }),
            "/sandbox",
        ),
        (
            FS_READ_FILE_METHOD,
            serde_json::json!({ "path": path, "sandbox": sandbox }),
            "/sandbox",
        ),
        (
            CAPABILITY_ROOTS_DISCOVER_METHOD,
            serde_json::json!({ "roots": [{ "id": "root", "path": cwd, "sandbox": sandbox }] }),
            "/roots/0/sandbox",
        ),
    ];
    let mut server = exec_server().await?;
    initialize(&mut server).await?;

    for (method, request, sandbox_pointer) in requests {
        for (index, legacy_cwd) in [None, Some(serde_json::Value::Null)]
            .into_iter()
            .enumerate()
        {
            let mut request = request.clone();
            if method == EXEC_METHOD {
                request["processId"] = serde_json::json!(format!("sandbox-cwd-{index}"));
            }
            let sandbox = request
                .pointer_mut(sandbox_pointer)
                .and_then(serde_json::Value::as_object_mut)
                .expect("sandbox request");
            if let Some(legacy_cwd) = legacy_cwd {
                sandbox.insert("cwd".to_string(), legacy_cwd);
            } else {
                sandbox.remove("cwd");
            }
            sandbox.remove("workspaceRoots");
            let request_id = server.send_request(method, request).await?;
            let response = server
                .wait_for_event(|event| match event {
                    JSONRPCMessage::Response(response) => response.id == request_id,
                    JSONRPCMessage::Error(error) => error.id == request_id,
                    JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
                })
                .await?;
            let JSONRPCMessage::Response(response) = response else {
                anyhow::bail!("expected legacy {method} to succeed, got {response:?}");
            };
            if method == FS_READ_FILE_METHOD {
                let read: FsReadFileResponse = serde_json::from_value(response.result)?;
                assert_eq!(STANDARD.decode(read.data_base64)?, b"contents");
            } else if method == CAPABILITY_ROOTS_DISCOVER_METHOD {
                let discovery: CapabilityRootsDiscoverResponse =
                    serde_json::from_value(response.result)?;
                assert_eq!(
                    discovery.roots[0]
                        .skills
                        .iter()
                        .map(|skill| &skill.instructions.path)
                        .collect::<Vec<_>>(),
                    vec![&cwd.join("skills/demo/SKILL.md")?],
                );
            }
        }
    }

    server.shutdown().await?;
    Ok(())
}

/// Old filesystem clients cannot replace the original cwd for rules whose meaning depends on it.
#[tokio::test]
async fn executor_legacy_filesystem_rejects_missing_cwd_for_dynamic_permissions()
-> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let mut server = exec_server().await?;
    initialize(&mut server).await?;

    for path in [
        FileSystemPath::Special {
            value: FileSystemSpecialPath::ProjectRoots { subpath: None },
        },
        FileSystemPath::GlobPattern {
            pattern: "secrets/**".to_string(),
        },
    ] {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            path,
            FileSystemAccessMode::Deny,
        )]);
        let sandbox = FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
            cwd.clone(),
        );
        for legacy_cwd in [None, Some(serde_json::Value::Null)] {
            let mut sandbox = serde_json::to_value(&sandbox)?;
            let fields = sandbox.as_object_mut().expect("sandbox object");
            if let Some(legacy_cwd) = legacy_cwd {
                fields.insert("cwd".to_string(), legacy_cwd);
            } else {
                fields.remove("cwd");
            }
            fields.remove("workspaceRoots");
            for (method, request) in [
                (
                    FS_READ_FILE_METHOD,
                    serde_json::json!({ "path": cwd, "sandbox": sandbox }),
                ),
                (
                    FS_OPEN_METHOD,
                    serde_json::json!({ "handleId": "denied", "path": cwd, "sandbox": sandbox }),
                ),
                (
                    CAPABILITY_ROOTS_DISCOVER_METHOD,
                    serde_json::json!({ "roots": [{ "id": "root", "path": cwd, "sandbox": sandbox }] }),
                ),
            ] {
                let request_id = server.send_request(method, request).await?;
                let response = server
                    .wait_for_event(|event| match event {
                        JSONRPCMessage::Response(response) => response.id == request_id,
                        JSONRPCMessage::Error(error) => error.id == request_id,
                        JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
                    })
                    .await?;
                let JSONRPCMessage::Error(error) = response else {
                    anyhow::bail!(
                        "expected legacy {method} to reject dynamic permissions, got {response:?}"
                    );
                };
                assert_eq!(
                    (error.error.code, error.error.message.as_str()),
                    (
                        -32602,
                        "file system sandbox context with dynamic permissions requires cwd"
                    ),
                    "{method}",
                );
            }
        }
    }

    server.shutdown().await?;
    Ok(())
}

/// Providing cwd lets project roots and relative rules be enforced against the original directory.
#[cfg(unix)]
#[tokio::test]
async fn executor_legacy_filesystem_accepts_cwd_for_dynamic_permissions() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    write_file(&workspace.path().join("allowed.txt"), "allowed")?;
    write_file(&workspace.path().join("secret.txt"), "secret")?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::ProjectRoots { subpath: None },
            },
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: "secret*".to_string(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let request_id = server
        .send_request(
            FS_READ_FILE_METHOD,
            serde_json::json!({"path": cwd.join("secret.txt")?}),
        )
        .await?;
    let response = server
        .wait_for_event(|event| match event {
            JSONRPCMessage::Response(response) => response.id == request_id,
            JSONRPCMessage::Error(error) => error.id == request_id,
            JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
        })
        .await?;
    let JSONRPCMessage::Response(response) = response else {
        anyhow::bail!(
            "expected the denied path to be readable without a sandbox, got {response:?}"
        );
    };
    let read: FsReadFileResponse = serde_json::from_value(response.result)?;
    assert_eq!(STANDARD.decode(read.data_base64)?, b"secret");

    for (name, permitted) in [("allowed.txt", true), ("secret.txt", false)] {
        let request_id = server
            .send_request(
                FS_READ_FILE_METHOD,
                serde_json::json!({
                    "path": cwd.join(name)?, "sandbox": sandbox,
                }),
            )
            .await?;
        let response = server
            .wait_for_event(|event| match event {
                JSONRPCMessage::Response(response) => response.id == request_id,
                JSONRPCMessage::Error(error) => error.id == request_id,
                JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
            })
            .await?;
        match response {
            JSONRPCMessage::Response(response) if permitted => {
                let read: FsReadFileResponse = serde_json::from_value(response.result)?;
                assert_eq!(STANDARD.decode(read.data_base64)?, b"allowed");
            }
            JSONRPCMessage::Error(error) if !permitted => {
                assert_eq!(error.error.code, -32600, "{error:?}");
                assert!(
                    matches!(
                        error.error.message.as_str(),
                        "Permission denied (os error 13)" | "Operation not permitted (os error 1)"
                    ),
                    "expected a filesystem permission denial: {error:?}"
                );
            }
            response => anyhow::bail!("unexpected filesystem result for {name}: {response:?}"),
        }
    }
    server.shutdown().await?;
    Ok(())
}

/// A present policy context can omit cwd: legacy cwd still anchors denials, and only static
/// filesystem policies can use the executor default when both cwd fields are absent.
#[tokio::test]
async fn executor_legacy_nested_policy_context_cwd_uses_available_fallbacks() -> anyhow::Result<()>
{
    let workspace = tempfile::tempdir()?;
    write_file(&workspace.path().join("allowed.txt"), "allowed")?;
    write_file(&workspace.path().join("secret.txt"), "secret")?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let static_permissions = if cfg!(unix) {
        PermissionProfile::read_only()
    } else {
        // The direct Windows test fixture has no enabled platform sandbox.
        PermissionProfile::Disabled
    };
    let static_sandbox =
        FileSystemSandboxContext::from_permission_profile(static_permissions, cwd.clone());
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: "secret*".to_string(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let dynamic_sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    let mut server = exec_server().await?;
    initialize(&mut server).await?;

    for (shape, policy_context) in [
        ("missing", serde_json::json!({"workspaceRoots": [cwd]})),
        (
            "null",
            serde_json::json!({"cwd": null, "workspaceRoots": [cwd]}),
        ),
    ] {
        for (case, sandbox, legacy_cwd, name, expected) in [
            (
                "static allowed without legacy cwd",
                &static_sandbox,
                None::<&PathUri>,
                "allowed.txt",
                Ok(b"allowed".as_slice()),
            ),
            (
                "static secret without legacy cwd",
                &static_sandbox,
                None,
                "secret.txt",
                Ok(b"secret".as_slice()),
            ),
            (
                "dynamic without legacy cwd",
                &dynamic_sandbox,
                None,
                "allowed.txt",
                Err(-32602),
            ),
            // The default Windows test backend cannot enforce read denials at runtime.
            #[cfg(unix)]
            (
                "dynamic allowed with legacy cwd",
                &dynamic_sandbox,
                Some(&cwd),
                "allowed.txt",
                Ok(b"allowed".as_slice()),
            ),
            #[cfg(unix)]
            (
                "dynamic secret with legacy cwd",
                &dynamic_sandbox,
                Some(&cwd),
                "secret.txt",
                Err(-32600),
            ),
        ] {
            let mut sandbox = serde_json::to_value(sandbox)?;
            let fields = sandbox.as_object_mut().expect("sandbox object");
            fields.insert("policyContext".to_string(), policy_context.clone());
            if let Some(legacy_cwd) = legacy_cwd {
                fields.insert("cwd".to_string(), serde_json::to_value(legacy_cwd)?);
            } else {
                fields.remove("cwd");
            }
            fields.remove("workspaceRoots");
            let request_id = server
                .send_request(
                    FS_READ_FILE_METHOD,
                    serde_json::json!({"path": cwd.join(name)?, "sandbox": sandbox}),
                )
                .await?;
            let response = server
                .wait_for_event(|event| match event {
                    JSONRPCMessage::Response(response) => response.id == request_id,
                    JSONRPCMessage::Error(error) => error.id == request_id,
                    JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
                })
                .await?;
            match (response, expected) {
                (JSONRPCMessage::Response(response), Ok(contents)) => {
                    let read: FsReadFileResponse = serde_json::from_value(response.result)?;
                    assert_eq!(
                        STANDARD.decode(read.data_base64)?,
                        contents,
                        "{shape}: {case}"
                    );
                }
                (JSONRPCMessage::Error(error), Err(-32602)) => assert_eq!(
                    (error.error.code, error.error.message.as_str()),
                    (
                        -32602,
                        "file system sandbox context with dynamic permissions requires cwd"
                    ),
                    "{shape}: {case}",
                ),
                (JSONRPCMessage::Error(error), Err(-32600)) => {
                    assert_eq!(error.error.code, -32600, "{shape}: {case}: {error:?}");
                    assert!(
                        matches!(
                            error.error.message.as_str(),
                            "Permission denied (os error 13)"
                                | "Operation not permitted (os error 1)"
                        ),
                        "{shape}: {case}: {error:?}",
                    );
                }
                (response, expected) => anyhow::bail!(
                    "unexpected {shape} nested cwd result for {case}: expected {expected:?}, got {response:?}"
                ),
            }
        }
    }

    server.shutdown().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn executor_legacy_exec_uses_process_cwd_for_relative_denials() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(workspace.path().join("allowed.txt"), b"allowed")?;
    std::fs::write(workspace.path().join("secret.txt"), b"secret")?;
    let cwd = PathUri::from_host_native_path(workspace.path())?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::GlobPattern {
                pattern: "secret*".to_string(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    let process_env =
        std::collections::HashMap::from([("PATH".to_string(), std::env::var("PATH")?)]);
    #[cfg(target_os = "linux")]
    let process_env = {
        let mut process_env = process_env;
        // Bazel-provided bubblewrap lives in runfiles, not necessarily on the process PATH.
        for name in [
            "CARGO_BIN_EXE_bwrap",
            "RUNFILES_DIR",
            "RUNFILES_MANIFEST_FILE",
            "TEST_SRCDIR",
            "TEST_WORKSPACE",
        ] {
            if let Ok(value) = std::env::var(name) {
                process_env.insert(name.to_string(), value);
            }
        }
        process_env
    };
    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    for (index, legacy_cwd) in [None, Some(serde_json::Value::Null)]
        .into_iter()
        .enumerate()
    {
        let mut sandbox = serde_json::to_value(&sandbox)?;
        let fields = sandbox.as_object_mut().expect("sandbox object");
        if let Some(legacy_cwd) = legacy_cwd {
            fields.insert("cwd".to_string(), legacy_cwd);
        } else {
            fields.remove("cwd");
        }
        let process = format!("legacy-relative-{index}");
        let request_id = server.send_request(EXEC_METHOD, serde_json::json!({
            "processId": process, "argv": ["/bin/sh", "-c", "cat allowed.txt && if cat secret.txt 2>/dev/null; then exit 9; fi"], "cwd": cwd,
            "env": process_env, "tty": false, "sandbox": sandbox,
        })).await?;
        let start = server
            .wait_for_event(|event| match event {
                JSONRPCMessage::Response(response) => response.id == request_id,
                JSONRPCMessage::Error(error) => error.id == request_id,
                JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
            })
            .await?;
        if !matches!(start, JSONRPCMessage::Response(_)) {
            anyhow::bail!("expected legacy process to start, got {start:?}");
        }
        let mut after_seq = None;
        let mut output = Vec::new();
        let (exit_code, failure, sandbox_denied) = loop {
            let request_id = server
                .send_request(
                    EXEC_READ_METHOD,
                    serde_json::json!({
                        "processId": process, "afterSeq": after_seq, "waitMs": 1000,
                    }),
                )
                .await?;
            let response = server
                .wait_for_event(|event| match event {
                    JSONRPCMessage::Response(response) => response.id == request_id,
                    JSONRPCMessage::Error(error) => error.id == request_id,
                    JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
                })
                .await?;
            let JSONRPCMessage::Response(response) = response else {
                anyhow::bail!("expected legacy process output, got {response:?}");
            };
            let response: ReadResponse = serde_json::from_value(response.result)?;
            output.extend(response.chunks.into_iter().flat_map(|chunk| chunk.chunk.0));
            after_seq = response.next_seq.checked_sub(1).or(after_seq);
            if response.closed {
                break (
                    response.exit_code,
                    response.failure,
                    response.sandbox_denied,
                );
            }
        };
        assert_eq!(
            (exit_code, String::from_utf8(output)?),
            (Some(0), "allowed".to_string()),
            "legacy process failed: {failure:?}; sandbox denied: {sandbox_denied}"
        );
    }
    server.shutdown().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn executor_legacy_filesystem_cwd_keeps_absolute_read_allow_and_deny_rules()
-> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let allowed = workspace.path().join("allowed");
    let denied = workspace.path().join("denied");
    for (root, name) in [(&allowed, "allowed"), (&denied, "denied")] {
        write_file(&root.join("note.txt"), "contents")?;
        write_file(
            &root.join(format!("skills/{name}/SKILL.md")),
            &format!("---\nname: {name}\ndescription: {name}.\n---\n"),
        )?;
    }
    let allowed = PathUri::from_host_native_path(allowed)?;
    let denied = PathUri::from_host_native_path(denied)?;
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Path {
                path: denied.clone(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        PathUri::from_host_native_path(workspace.path())?,
    );
    let mut server = exec_server().await?;
    initialize(&mut server).await?;

    let request_id = server
        .send_request(
            FS_READ_FILE_METHOD,
            serde_json::json!({"path": denied.join("note.txt")?}),
        )
        .await?;
    let response = server
        .wait_for_event(|event| match event {
            JSONRPCMessage::Response(response) => response.id == request_id,
            JSONRPCMessage::Error(error) => error.id == request_id,
            JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
        })
        .await?;
    let JSONRPCMessage::Response(response) = response else {
        anyhow::bail!(
            "expected the denied path to be readable without a sandbox, got {response:?}"
        );
    };
    let read: FsReadFileResponse = serde_json::from_value(response.result)?;
    assert_eq!(STANDARD.decode(read.data_base64)?, b"contents");

    for (index, legacy_cwd) in [None, Some(serde_json::Value::Null)]
        .into_iter()
        .enumerate()
    {
        let mut sandbox = serde_json::to_value(&sandbox)?;
        let fields = sandbox.as_object_mut().expect("sandbox object");
        if let Some(legacy_cwd) = legacy_cwd {
            fields.insert("cwd".to_string(), legacy_cwd);
        } else {
            fields.remove("cwd");
        }
        fields.remove("workspaceRoots");
        for (root, permitted) in [(&allowed, true), (&denied, false)] {
            let handle = format!("legacy-{index}-{permitted}");
            for (method, request) in [
                (
                    FS_READ_FILE_METHOD,
                    serde_json::json!({ "path": root.join("note.txt")?, "sandbox": sandbox }),
                ),
                (
                    FS_OPEN_METHOD,
                    serde_json::json!({ "handleId": handle, "path": root.join("note.txt")?, "sandbox": sandbox }),
                ),
            ] {
                let request_id = server.send_request(method, request).await?;
                let response = server
                    .wait_for_event(|event| match event {
                        JSONRPCMessage::Response(response) => response.id == request_id,
                        JSONRPCMessage::Error(error) => error.id == request_id,
                        JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
                    })
                    .await?;
                match response {
                    JSONRPCMessage::Response(response) if permitted => {
                        if method == FS_READ_FILE_METHOD {
                            let read: FsReadFileResponse = serde_json::from_value(response.result)?;
                            assert_eq!(STANDARD.decode(read.data_base64)?, b"contents");
                        } else {
                            let request_id = server.send_request(FS_READ_BLOCK_METHOD, serde_json::json!({ "handleId": handle, "offset": 0, "len": 16 })).await?;
                            let block = server
                                .wait_for_event(|event| match event {
                                    JSONRPCMessage::Response(response) => response.id == request_id,
                                    JSONRPCMessage::Error(error) => error.id == request_id,
                                    JSONRPCMessage::Request(_)
                                    | JSONRPCMessage::Notification(_) => false,
                                })
                                .await?;
                            let JSONRPCMessage::Response(block) = block else {
                                anyhow::bail!("expected to read the legacy stream, got {block:?}");
                            };
                            let block: FsReadBlockResponse = serde_json::from_value(block.result)?;
                            assert_eq!(block.chunk.0, b"contents");
                        }
                    }
                    JSONRPCMessage::Error(error) if !permitted => {
                        assert_eq!(error.error.code, -32600, "{method}: {error:?}");
                        assert!(
                            matches!(
                                error.error.message.as_str(),
                                "Permission denied (os error 13)"
                                    | "Operation not permitted (os error 1)"
                            ),
                            "expected a filesystem permission denial for {method}: {error:?}"
                        );
                    }
                    response => {
                        anyhow::bail!("unexpected legacy {method} result for {root}: {response:?}")
                    }
                }
            }
        }
        let request_id = server
            .send_request(
                CAPABILITY_ROOTS_DISCOVER_METHOD,
                serde_json::json!({ "roots": [
            { "id": "allowed", "path": allowed, "sandbox": sandbox },
            { "id": "denied", "path": denied, "sandbox": sandbox },
        ] }),
            )
            .await?;
        let discovery = server
            .wait_for_event(|event| match event {
                JSONRPCMessage::Response(response) => response.id == request_id,
                JSONRPCMessage::Error(error) => error.id == request_id,
                JSONRPCMessage::Request(_) | JSONRPCMessage::Notification(_) => false,
            })
            .await?;
        let JSONRPCMessage::Response(discovery) = discovery else {
            anyhow::bail!("expected legacy capability discovery, got {discovery:?}");
        };
        let discovery: CapabilityRootsDiscoverResponse = serde_json::from_value(discovery.result)?;
        assert_eq!(
            discovery
                .roots
                .iter()
                .map(|root| (
                    root.id.as_str(),
                    root.skills
                        .iter()
                        .map(|skill| &skill.instructions.path)
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("allowed", vec![&allowed.join("skills/allowed/SKILL.md")?]),
                ("denied", vec![]),
            ]
        );
    }

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandboxed_discovery_batches_roots_without_combining_different_permissions()
-> anyhow::Result<()> {
    #[cfg(windows)]
    crate::skip_if_mxc_unavailable!(Ok(()));
    let workspace = tempfile::tempdir()?;
    let first_root = workspace.path().join("first");
    let second_root = workspace.path().join("second");
    write_file(
        &first_root.join("skills/first/SKILL.md"),
        "---\nname: first\ndescription: First skill.\n---\n",
    )?;
    write_file(
        &second_root.join("skills/second/SKILL.md"),
        "---\nname: second\ndescription: Second skill.\n---\n",
    )?;

    let workspace_uri = PathUri::from_host_native_path(workspace.path())?;
    let first_uri = PathUri::from_host_native_path(&first_root)?;
    let second_uri = PathUri::from_host_native_path(&second_root)?;
    let read_workspace = FileSystemSandboxEntry::new(
        AbsolutePathBuf::from_absolute_path(workspace.path())?.into(),
        FileSystemAccessMode::Read,
    );
    let policy = FileSystemSandboxPolicy::restricted(vec![read_workspace]);
    let shared_sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
        workspace_uri,
    );
    let shared_sandbox = with_native_sandbox(shared_sandbox);

    #[cfg(target_os = "linux")]
    let fake_bwrap_directory = tempfile::tempdir()?;
    #[cfg(target_os = "linux")]
    let (mut server, fake_bwrap) = {
        let fake_bin_dir = fake_bwrap_directory.path().to_path_buf();
        let fake_bwrap = write_fake_bwrap(&fake_bin_dir)?;
        let mut path_entries = vec![fake_bin_dir];
        if let Some(path) = std::env::var_os("PATH") {
            path_entries.extend(std::env::split_paths(&path));
        }
        let helper_path = std::env::join_paths(path_entries)?;
        (
            exec_server_with_env([("PATH", helper_path.as_os_str())], &[]).await?,
            fake_bwrap,
        )
    };
    #[cfg(not(target_os = "linux"))]
    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let response = discover_roots(
        &mut server,
        vec![
            CapabilityRootDiscoverRequest {
                id: "first".to_string(),
                path: first_uri.clone(),
                sandbox: Some(shared_sandbox.clone()),
            },
            CapabilityRootDiscoverRequest {
                id: "second".to_string(),
                path: second_uri.clone(),
                sandbox: Some(shared_sandbox.clone()),
            },
        ],
    )
    .await?;
    assert_eq!(
        response
            .roots
            .into_iter()
            .map(|root| (
                root.id,
                root.path,
                root.skills
                    .into_iter()
                    .map(|skill| skill.instructions.path)
                    .collect::<Vec<_>>(),
                root.error,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "first".to_string(),
                first_uri.clone(),
                vec![first_uri.join("skills/first/SKILL.md")?],
                None,
            ),
            (
                "second".to_string(),
                second_uri.clone(),
                vec![second_uri.join("skills/second/SKILL.md")?],
                None,
            ),
        ]
    );

    #[cfg(target_os = "linux")]
    {
        let launch_log = fake_bwrap.with_file_name("bwrap.log");
        let launch_count = std::fs::read_to_string(&launch_log)
            .with_context(|| format!("expected fake bwrap launch log at {}", launch_log.display()))?
            .lines()
            .count();
        assert_eq!(launch_count, 1);

        std::fs::write(fake_bwrap.with_file_name("bwrap.fail-once"), "")?;
        let fallback = discover_roots(
            &mut server,
            vec![
                CapabilityRootDiscoverRequest {
                    id: "fallback-first".to_string(),
                    path: first_uri.clone(),
                    sandbox: Some(shared_sandbox.clone()),
                },
                CapabilityRootDiscoverRequest {
                    id: "fallback-second".to_string(),
                    path: second_uri.clone(),
                    sandbox: Some(shared_sandbox.clone()),
                },
            ],
        )
        .await?;
        assert_eq!(
            fallback
                .roots
                .into_iter()
                .map(|root| (root.id, root.skills.len(), root.error))
                .collect::<Vec<_>>(),
            vec![
                ("fallback-first".to_string(), 1, None),
                ("fallback-second".to_string(), 1, None),
            ]
        );
        assert!(std::fs::read_to_string(&launch_log)?.lines().count() > 2);

        server.shutdown().await?;
        server = exec_server().await?;
        initialize(&mut server).await?;
    }

    let read_first_root = FileSystemSandboxEntry::new(
        AbsolutePathBuf::from_absolute_path(&first_root)?.into(),
        FileSystemAccessMode::Read,
    );
    let first_policy = FileSystemSandboxPolicy::restricted(vec![read_first_root]);
    let first_only_sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &first_policy,
            NetworkSandboxPolicy::Restricted,
        ),
        first_uri.clone(),
    );
    let first_only_sandbox = with_native_sandbox(first_only_sandbox);
    let read_second_root = FileSystemSandboxEntry::new(
        AbsolutePathBuf::from_absolute_path(&second_root)?.into(),
        FileSystemAccessMode::Read,
    );
    let second_policy = FileSystemSandboxPolicy::restricted(vec![read_second_root]);
    let second_only_sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &second_policy,
            NetworkSandboxPolicy::Restricted,
        ),
        second_uri.clone(),
    );
    let second_only_sandbox = with_native_sandbox(second_only_sandbox);
    let response = discover_roots(
        &mut server,
        vec![
            CapabilityRootDiscoverRequest {
                id: "first-isolated".to_string(),
                path: first_uri,
                sandbox: Some(first_only_sandbox),
            },
            CapabilityRootDiscoverRequest {
                id: "second-isolated".to_string(),
                path: second_uri,
                sandbox: Some(second_only_sandbox),
            },
        ],
    )
    .await?;
    assert_eq!(
        response
            .roots
            .into_iter()
            .map(|root| (root.id, root.skills.len(), root.error))
            .collect::<Vec<_>>(),
        vec![
            ("first-isolated".to_string(), 1, None),
            ("second-isolated".to_string(), 1, None),
        ]
    );

    server.shutdown().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandboxed_discovery_follows_only_permitted_external_symlinks() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let external = tempfile::tempdir()?;
    write_file(
        &root.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"linked-plugin","mcpServers":"./external-mcp.json"}"#,
    )?;
    write_file(
        &external.path().join("mcp.json"),
        r#"{"mcpServers":{"linked":{"command":"linked-server"}}}"#,
    )?;
    write_file(
        &external.path().join("skill/SKILL.md"),
        "---\nname: linked\ndescription: Linked external skill.\n---\n",
    )?;
    std::fs::create_dir_all(root.path().join("skills"))?;
    std::os::unix::fs::symlink(
        external.path().join("skill"),
        root.path().join("skills/linked"),
    )?;
    std::os::unix::fs::symlink(
        external.path().join("mcp.json"),
        root.path().join("external-mcp.json"),
    )?;

    let mut server = exec_server().await?;
    initialize(&mut server).await?;
    let root_uri = PathUri::from_host_native_path(root.path())?;
    let root_path = AbsolutePathBuf::from_absolute_path(root.path())?;
    let external_root = AbsolutePathBuf::from_absolute_path(external.path())?;
    let path_entry =
        |path: AbsolutePathBuf, access| FileSystemSandboxEntry::new(path.into(), access);
    let read_root = path_entry(root_path, FileSystemAccessMode::Read);
    let read_external = path_entry(external_root.clone(), FileSystemAccessMode::Read);
    let deny_external_skill = path_entry(external_root.join("skill"), FileSystemAccessMode::Deny);
    let cases = [
        (
            "permitted symlinks",
            vec![read_root.clone(), read_external.clone()],
            true,
            true,
        ),
        (
            "denied external root",
            vec![read_root.clone()],
            false,
            false,
        ),
        (
            "denied external skill",
            vec![read_root, read_external, deny_external_skill],
            false,
            true,
        ),
    ];

    for (scenario, entries, has_skill, has_mcp) in cases {
        let policy = FileSystemSandboxPolicy::restricted(entries);
        let sandbox = FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
            root_uri.clone(),
        );
        let discovery =
            discover_root_with_sandbox(&mut server, "linked", root_uri.clone(), Some(sandbox))
                .await?;

        assert_eq!(discovery.error, None, "{scenario}");
        assert_eq!(discovery.skills.len(), usize::from(has_skill), "{scenario}");
        assert_eq!(
            discovery
                .plugin
                .and_then(|plugin| plugin.mcp_config)
                .is_some_and(|config| config.contents.contains("linked-server")),
            has_mcp,
            "{scenario}"
        );
    }

    server.shutdown().await?;
    Ok(())
}

async fn discover_root(
    server: &mut common::exec_server::ExecServerHarness,
    id: &str,
    path: PathUri,
) -> anyhow::Result<CapabilityRootDiscovery> {
    discover_root_with_sandbox(server, id, path, /*sandbox*/ None).await
}

async fn discover_root_with_sandbox(
    server: &mut common::exec_server::ExecServerHarness,
    id: &str,
    path: PathUri,
    sandbox: Option<FileSystemSandboxContext>,
) -> anyhow::Result<CapabilityRootDiscovery> {
    let response = discover_roots(
        server,
        vec![CapabilityRootDiscoverRequest {
            id: id.to_string(),
            path,
            sandbox,
        }],
    )
    .await?;
    let [discovery] = response.roots.as_slice() else {
        anyhow::bail!("expected exactly one discovered root");
    };
    Ok(discovery.clone())
}

async fn discover_roots(
    server: &mut common::exec_server::ExecServerHarness,
    roots: Vec<CapabilityRootDiscoverRequest>,
) -> anyhow::Result<CapabilityRootsDiscoverResponse> {
    let request_id = server
        .send_request(
            CAPABILITY_ROOTS_DISCOVER_METHOD,
            serde_json::to_value(CapabilityRootsDiscoverParams { roots })?,
        )
        .await?;
    let response = server.next_event().await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        anyhow::bail!("expected discovery response, received {response:?}");
    };
    assert_eq!(id, request_id);
    Ok(serde_json::from_value(result)?)
}

async fn initialize(server: &mut common::exec_server::ExecServerHarness) -> anyhow::Result<()> {
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "capability-discovery-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let response = server
        .wait_for_event(|event| {
            matches!(event, JSONRPCMessage::Response(response) if response.id == initialize_id)
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        unreachable!("wait predicate only accepts a response");
    };
    let _: InitializeResponse = serde_json::from_value(result)?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    Ok(())
}

fn write_file(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("test file should have a parent"))?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, contents)?;
    Ok(())
}

fn with_native_sandbox(mut sandbox: FileSystemSandboxContext) -> FileSystemSandboxContext {
    if cfg!(windows) {
        sandbox.windows_sandbox_selection = WindowsSandboxSelection::Mxc;
    }
    sandbox
}
