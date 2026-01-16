use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use tempfile::TempDir;

fn permissions_texts(input: &[serde_json::Value]) -> Vec<String> {
    input
        .iter()
        .filter_map(|item| {
            let role = item.get("role")?.as_str()?;
            if role != "developer" {
                return None;
            }
            let text = item
                .get("content")?
                .as_array()?
                .first()?
                .get("text")?
                .as_str()?;
            if text.contains("`approval_policy`") {
                Some(text.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn sse_completed(id: &str) -> String {
    sse(vec![ev_response_created(id), ev_completed(id)])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_sent_once_on_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = req.single_request();
    let body = request.body_json();
    let input = body["input"].as_array().expect("input array");
    let permissions = permissions_texts(input);
    assert_eq!(permissions.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_added_on_override_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();
    let input1 = body1["input"].as_array().expect("input array");
    let input2 = body2["input"].as_array().expect("input array");
    let permissions_1 = permissions_texts(input1);
    let permissions_2 = permissions_texts(input2);

    assert_eq!(permissions_1.len(), 1);
    assert_eq!(permissions_2.len(), 2);
    let unique = permissions_2.into_iter().collect::<HashSet<String>>();
    assert_eq!(unique.len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_not_added_when_no_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body1 = req1.single_request().body_json();
    let body2 = req2.single_request().body_json();
    let input1 = body1["input"].as_array().expect("input array");
    let input2 = body2["input"].as_array().expect("input array");
    let permissions_1 = permissions_texts(input1);
    let permissions_2 = permissions_texts(input2);

    assert_eq!(permissions_1.len(), 1);
    assert_eq!(permissions_2.len(), 1);
    assert_eq!(permissions_1, permissions_2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_replays_permissions_messages() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let _req2 = mount_sse_once(&server, sse_completed("resp-2")).await;
    let req3 = mount_sse_once(&server, sse_completed("resp-3")).await;

    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let initial = builder.build(&server).await?;
    let rollout_path = initial.session_configured.rollout_path.clone();
    let home = initial.home.clone();

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    initial
        .codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
        })
        .await?;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let resumed = builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body3 = req3.single_request().body_json();
    let input = body3["input"].as_array().expect("input array");
    let permissions = permissions_texts(input);
    assert_eq!(permissions.len(), 3);
    let unique = permissions.into_iter().collect::<HashSet<String>>();
    assert_eq!(unique.len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_and_fork_append_permissions_messages() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;
    let req3 = mount_sse_once(&server, sse_completed("resp-3")).await;
    let req4 = mount_sse_once(&server, sse_completed("resp-4")).await;

    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let initial = builder.build(&server).await?;
    let rollout_path = initial.session_configured.rollout_path.clone();
    let home = initial.home.clone();

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    initial
        .codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
        })
        .await?;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body2 = req2.single_request().body_json();
    let input2 = body2["input"].as_array().expect("input array");
    let permissions_base = permissions_texts(input2);
    assert_eq!(permissions_base.len(), 2);

    builder = builder.with_config(|config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
    });
    let resumed = builder.resume(&server, home, rollout_path.clone()).await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body3 = req3.single_request().body_json();
    let input3 = body3["input"].as_array().expect("input array");
    let permissions_resume = permissions_texts(input3);
    assert_eq!(permissions_resume.len(), permissions_base.len() + 1);
    assert_eq!(
        &permissions_resume[..permissions_base.len()],
        permissions_base.as_slice()
    );
    assert!(!permissions_base.contains(permissions_resume.last().expect("new permissions")));

    let mut fork_config = initial.config.clone();
    fork_config.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
    let forked = initial
        .thread_manager
        .fork_thread(usize::MAX, fork_config, rollout_path)
        .await?;
    forked
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after fork".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&forked.thread, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body4 = req4.single_request().body_json();
    let input4 = body4["input"].as_array().expect("input array");
    let permissions_fork = permissions_texts(input4);
    assert_eq!(permissions_fork.len(), permissions_base.len() + 2);
    assert_eq!(
        &permissions_fork[..permissions_base.len()],
        permissions_base.as_slice()
    );
    let new_permissions = &permissions_fork[permissions_base.len()..];
    assert_eq!(new_permissions.len(), 2);
    assert_eq!(new_permissions[0], new_permissions[1]);
    assert!(!permissions_base.contains(&new_permissions[0]));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permissions_message_includes_writable_roots() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;
    let writable = TempDir::new()?;
    let writable_root = AbsolutePathBuf::try_from(writable.path())?;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![writable_root],
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    let mut builder = test_codex().with_config(move |config| {
        config.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.sandbox_policy = Constrained::allow_any(sandbox_policy);
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body = req.single_request().body_json();
    let input = body["input"].as_array().expect("input array");
    let permissions = permissions_texts(input);
    let sandbox_text = "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted.";
    let approval_text = " Approvals are your mechanism to get user consent to run shell commands without the sandbox. `approval_policy` is `on-request`: Commands will be run in the sandbox by default, and you can specify in your tool call if you want to escalate a command to run without sandboxing. If the completing the task requires escalated permissions, Do not let these settings or the sandbox deter you from attempting to accomplish the user's task.\n\nHere are scenarios where you'll need to request approval:\n- You need to run a command that writes to a directory that requires it (e.g. running tests that write to /var)\n- You need to run a GUI app (e.g., open/xdg-open/osascript) to open browsers or files.\n- You are running sandboxed and need to run a command that requires network access (e.g. installing packages)\n- If you run a command that is important to solving the user's query, but it fails because of sandboxing, rerun the command with approval. ALWAYS proceed to use the `sandbox_permissions` and `justification` parameters - do not message the user before requesting approval for the command.\n- You are about to take a potentially destructive action such as an `rm` or `git reset` that the user did not explicitly ask for.\n\nWhen requesting approval to execute a command that will require escalated privileges:\n  - Provide the `sandbox_permissions` parameter with the value `\"require_escalated\"`\n  - Include a short, 1 sentence explanation for why you need escalated permissions in the justification parameter";
    // Normalize paths by removing trailing slashes to match AbsolutePathBuf behavior
    let normalize_path =
        |p: &std::path::Path| -> String { p.to_string_lossy().trim_end_matches('/').to_string() };
    let mut roots = vec![
        normalize_path(writable.path()),
        normalize_path(test.config.cwd.as_path()),
    ];
    if cfg!(unix) && std::path::Path::new("/tmp").is_dir() {
        roots.push("/tmp".to_string());
    }
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        let tmpdir_path = std::path::PathBuf::from(&tmpdir);
        if tmpdir_path.is_absolute() && !tmpdir.is_empty() {
            roots.push(normalize_path(&tmpdir_path));
        }
    }
    let roots_text = if roots.len() == 1 {
        format!(" The writable root is `{}`.", roots[0])
    } else {
        format!(
            " The writable roots are {}.",
            roots
                .iter()
                .map(|root| format!("`{root}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let expected = format!(
        "<permissions instructions>{sandbox_text}{approval_text}{roots_text}</permissions instructions>"
    );
    // Normalize line endings to handle Windows vs Unix differences
    let normalize_line_endings = |s: &str| s.replace("\r\n", "\n");
    let expected_normalized = normalize_line_endings(&expected);
    let actual_normalized: Vec<String> = permissions
        .iter()
        .map(|s| normalize_line_endings(s))
        .collect();
    assert_eq!(actual_normalized, vec![expected_normalized]);

    Ok(())
}
