//! Remote credential masking and its effects on model-visible context.

use super::*;
use anyhow::Context;
use codex_protocol::models::PermissionProfile;
use core_test_support::exec_server::ExecServerProcess;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ev_function_call;
use core_test_support::skip_if_sandbox;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;

#[test_case::test_case(false, false, None; "builtin_providers")]
#[test_case::test_case(true, false, None; "configured_provider")]
#[test_case::test_case(true, true, None; "configured_provider_login_fallback")]
#[test_case::test_case(true, false, Some("BASH_ENV"); "configured_provider_executor_startup_fallback")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_credential_masking_request_history(
    configured_provider: bool,
    use_login_shell: bool,
    executor_startup_env: Option<&'static str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    let shell = codex_core::shell::default_user_shell();
    if !matches!(shell.name(), "bash" | "zsh") {
        eprintln!("skipping credential masking: this scenario requires a Bash or Zsh executor");
        return Ok(());
    }
    let expect_masking = !use_login_shell && executor_startup_env.is_none();
    let use_zsh_snapshot = shell.name() == "zsh" && expect_masking;
    let home = Arc::new(TempDir::new()?);
    if let Some(warning) =
        codex_sandboxing::system_bwrap_warning(&PermissionProfile::workspace_write(), home.path())
    {
        eprintln!("skipping sandbox test: {warning}");
        return Ok(());
    }
    let (credential_env, credential, prefix) = if configured_provider {
        ("VENDOR_TOKEN", "vendor_abcdefgh", "vendor_")
    } else {
        (
            "OPENAI_API_KEY",
            "sk-proj-executor-only-0000000000000000000000000000000000",
            "sk-proj-",
        )
    };
    const CALL_ID: &str = "print-credential";
    // Use our own native executor so only its environment contains the credential.
    let mut executor_env = vec![
        (credential_env, credential),
        ("OPENAI_BASE_URL", "https://api.openai.com/v1"),
    ];
    executor_env.extend(executor_startup_env.map(|key| (key, "/dev/null")));
    let executor = ExecServerProcess::start_with_env(&executor_env).await?;
    let server = start_mock_server().await;
    let test = test_codex()
        .with_home(home)
        .with_exec_server_url(executor.websocket_url.clone())
        .with_cloud_config_bundle(managed_network_requirements_loader())
        .with_pre_build_hook(move |home| {
            let provider_config = if configured_provider {
                r#"[features.network_proxy.credentials.vendor]
env = ["VENDOR_TOKEN"]
patterns = ["vendor_[a-z]{8}"]
url_prefixes = ["https://api.vendor.example"]
auth = ["bearer"]
"#
            } else {
                ""
            };
            let startup_env = executor_startup_env
                .map(|key| format!(", \"{key}\""))
                .unwrap_or_default();
            fs::write(
                home.join("config.toml"),
                format!(
                    r#"default_permissions = "brokered"

[permissions.brokered]
extends = ":workspace"

[permissions.brokered.network]
enabled = true
mode = "full"
allow_local_binding = true

[shell_environment_policy]
ignore_default_excludes = true
include_only = ["HOME", "PATH", "{credential_env}", "OPENAI_BASE_URL"{startup_env}]

[features]
unified_exec = true
credential_masking = true
{provider_config}"#
                ),
            )
            .expect("write credential masking config");
        })
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.project_doc_max_bytes = 0;
            if use_zsh_snapshot {
                config
                    .features
                    .enable(Feature::ShellSnapshotV2)
                    .expect("enable shell snapshots");
            }
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model.use_responses_lite = false;
        })
        .build(&server)
        .await?;
    let mut command = if !expect_masking {
        // Keep the real credential out of the model-visible command text.
        fs::write(test.workspace_path("expected-api-key"), credential)?;
        format!(
            "test \"${credential_env}\" = \"$(<expected-api-key)\" && printf 'unmasked fallback'"
        )
    } else {
        format!("printf '%s' \"${credential_env}\"")
    };
    if let Some(key) = executor_startup_env {
        command = format!("test -n \"${key}\" && {command}");
    }
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("credential-response"),
                ev_function_call(
                    CALL_ID,
                    "exec_command",
                    &json!({
                        "cmd": command,
                        "environment_id": "remote",
                        "login": use_login_shell || use_zsh_snapshot,
                        "yield_time_ms": 1000,
                    })
                    .to_string(),
                ),
                ev_completed("credential-response"),
            ]),
            sse(vec![
                ev_response_created("followup-response"),
                ev_assistant_message("followup-message", "Done."),
                ev_completed("followup-response"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(&format!(
            "Print {credential_env} from the executor environment.",
        ))]))
        .await?;
    if use_login_shell || shell.name() == "zsh" && !use_zsh_snapshot {
        let warning = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::Warning(warning) if warning.message.starts_with("Credential masking is unavailable"))
        })
        .await;
        let EventMsg::Warning(warning) = warning else {
            unreachable!("waited for a credential masking warning")
        };
        insta::assert_snapshot!(warning.message, @"Credential masking is unavailable for this shell startup on environment `remote`. This command will run with unmasked credentials.");
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    // Check complete, raw requests before any snapshot rewriting or normalization.
    for request in &requests {
        assert!(!request.body_json().to_string().contains(credential));
    }
    let output = requests[1]
        .function_call_output_text(CALL_ID)
        .context("model follow-up should include the executor output")?;
    assert!(output.contains("Process exited with code 0"), "{output}");
    if !expect_masking {
        assert!(output.ends_with("Output:\nunmasked fallback"), "{output}");
        return Ok(());
    }
    let (_, placeholder) = output
        .split_once("Output:\n")
        .context("executor output should contain a credential placeholder")?;
    assert!(placeholder.starts_with(prefix));
    assert_eq!(placeholder.len(), credential.len());
    assert_ne!(placeholder, credential);

    // Share the full-history snapshots after normalizing the provider-specific variable name.
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        "A remote executor masks its credential before the command output reaches the model.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    )
    .replace(placeholder, "<CREDENTIAL_PLACEHOLDER>")
    .replace(credential_env, "<CREDENTIAL_ENV>");
    for (pattern, replacement) in [
        (r"Chunk ID: [a-z0-9]+", "Chunk ID: <CHUNK_ID>"),
        (
            r"Wall time: [0-9]+(?:\.[0-9]+)? seconds",
            "Wall time: <DURATION> seconds",
        ),
        (
            r"Original token count: [0-9]+",
            "Original token count: <TOKEN_COUNT>",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    let snapshot_name = if use_zsh_snapshot {
        "remote_credential_masking_zsh"
    } else {
        "remote_credential_masking"
    };
    insta::assert_snapshot!(snapshot_name, snapshot);
    Ok(())
}
