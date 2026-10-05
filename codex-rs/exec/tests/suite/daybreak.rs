use anyhow::Context;
use core_test_support::responses;
use core_test_support::test_codex_exec::TestCodexExecBuilder;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn started_thread(output: std::process::Output) -> anyhow::Result<String> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["type"] == "thread.started")
        .and_then(|event| event["thread_id"].as_str().map(str::to_owned))
        .context("exec should report its thread ID")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_rejects_daybreak_for_api_keys_and_allows_an_override() -> anyhow::Result<()> {
    let (test, server) = configured_daybreak_exec().await?;
    std::fs::remove_file(test.home_path().join("auth.json"))?;
    let response = responses::mount_sse_once(&server, responses::sse_completed("response")).await;

    test.cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .arg("--skip-git-repo-check")
        .arg("hello")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Daybreak requires a signed-in ChatGPT account",
        ));
    assert!(response.requests().is_empty());

    test.cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .arg("--skip-git-repo-check")
        .arg("-c")
        .arg("daybreak=false")
        .arg("hello")
        .assert()
        .success();
    assert_eq!(
        response.single_request().body_json().get("access_programs"),
        None
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_rejects_daybreak_for_non_openai_provider() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    std::fs::write(
        test.home_path().join("config.toml"),
        "daybreak = true\nfeatures.cli_daybreak = true\n",
    )?;
    let response = responses::mount_sse_once(&server, responses::sse_completed("response")).await;

    test.cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .args([
            "--oss",
            "--local-provider",
            "ollama",
            "--skip-git-repo-check",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Daybreak requires the OpenAI model provider",
        ));
    assert!(response.requests().is_empty());
    Ok(())
}

async fn configured_daybreak_exec() -> anyhow::Result<(TestCodexExecBuilder, wiremock::MockServer)>
{
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let token = concat!(
        "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.",
        "eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJlbnRlcnByaXNlIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoid29ya3NwYWNlIn19.",
        "c2lnbmF0dXJl",
    );
    std::fs::write(
        test.home_path().join("auth.json"),
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt",
            "tokens": {"id_token": token, "access_token": "test-token",
                       "refresh_token": "refresh-token", "account_id": "workspace"},
            "last_refresh": "2099-01-01T00:00:00Z"
        }))?,
    )?;
    let catalog = test.home_path().join("catalog.json");
    std::fs::write(
        &catalog,
        serde_json::to_vec(&json!({"models": [{
            "slug": "gpt-test", "display_name": "Test", "base_instructions": "Test instructions", "supported_reasoning_levels": [],
            "shell_type": "disabled", "visibility": "list", "supported_in_api": true,
            "priority": 0, "support_verbosity": false,
            "truncation_policy": {"mode": "tokens", "limit": 10000},
            "experimental_supported_tools": [],
            "available_access_programs": {"cyber": ["standard", "daybreak_red", "daybreak_blue"]}
        }]}))?,
    )?;
    std::fs::write(
        test.home_path().join("config.toml"),
        format!(
            "cli_auth_credentials_store = 'file'\nchatgpt_base_url = '{}/backend-api'\nmodel_catalog_json = '{}'\nmodel = 'gpt-test'\ndaybreak = true\nfeatures.cli_daybreak = true\n",
            server.uri(),
            catalog.display()
        ),
    )?;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/config/bundle"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "config_toml": {"enterprise_managed": []}
        })))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/accounts/check"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "accounts": [{"id": "workspace", "workspace_backend_origin": "https://chatgpt.com",
                "account_routing_override": "NO_CONSTRAINT"}]
        })))
        .mount(&server)
        .await;
    Ok((test, server))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resumes_and_forks_the_saved_daybreak_choice() -> anyhow::Result<()> {
    let (test, server) = configured_daybreak_exec().await?;
    let response = responses::mount_sse_once(&server, responses::sse_completed("started")).await;
    let output = test
        .cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .args(["--skip-git-repo-check", "--json", "hello"])
        .output()?;
    let thread_id = started_thread(output)?;
    assert_eq!(
        response.single_request().body_json()["access_programs"],
        json!({"cyber": "daybreak_blue"})
    );

    // The saved choice wins over a changed default; a CLI override affects only its invocation.
    std::fs::write(
        test.home_path().join("config.toml"),
        format!(
            "cli_auth_credentials_store = 'file'\nchatgpt_base_url = '{}/backend-api'\nmodel_catalog_json = '{}'\nmodel = 'gpt-test'\nfeatures.cli_daybreak = true\n",
            server.uri(),
            test.home_path().join("catalog.json").display()
        ),
    )?;
    for (args, expected) in [
        (
            vec![
                "-c",
                "daybreak=false",
                "resume",
                thread_id.as_str(),
                "continue",
            ],
            Some("standard"),
        ),
        (
            vec!["resume", thread_id.as_str(), "continue"],
            Some("daybreak_blue"),
        ),
        (
            vec!["fork", thread_id.as_str(), "continue"],
            Some("daybreak_blue"),
        ),
        (
            vec![
                "-c",
                "features.cli_daybreak=false",
                "--ephemeral",
                "fork",
                thread_id.as_str(),
                "continue",
            ],
            None,
        ),
        (
            vec![
                "--ephemeral",
                "-c",
                "daybreak=false",
                "fork",
                thread_id.as_str(),
                "continue",
            ],
            Some("standard"),
        ),
    ] {
        let response =
            responses::mount_sse_once(&server, responses::sse_completed("continued")).await;
        test.cmd_with_server(&server)
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .arg("--skip-git-repo-check")
            .args(args)
            .assert()
            .success();
        assert_eq!(
            response.single_request().body_json().get("access_programs"),
            expected.map(|program| json!({"cyber": program})).as_ref()
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_explicit_cyber_program_overrides_daybreak_for_one_turn() -> anyhow::Result<()> {
    let (test, server) = configured_daybreak_exec().await?;
    let response =
        responses::mount_sse_once(&server, responses::sse_completed("explicit-program")).await;
    let output = test
        .cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .args([
            "--skip-git-repo-check",
            "--json",
            "-c",
            "features.cli_daybreak=false",
            "--cyber-access-program",
            "daybreak_red",
            "hello",
        ])
        .output()?;
    let thread_id = started_thread(output)?;
    assert_eq!(
        response.single_request().body_json()["access_programs"],
        json!({"cyber": "daybreak_red"})
    );

    let response =
        responses::mount_sse_once(&server, responses::sse_completed("saved-daybreak")).await;
    test.cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .args(["--skip-git-repo-check", "resume", &thread_id, "continue"])
        .assert()
        .success();
    assert_eq!(
        response.single_request().body_json()["access_programs"],
        json!({"cyber": "daybreak_blue"})
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_override_does_not_change_a_standard_thread() -> anyhow::Result<()> {
    let (test, server) = configured_daybreak_exec().await?;
    let response =
        responses::mount_sse_once(&server, responses::sse_completed("started-standard")).await;
    let output = test
        .cmd_with_server(&server)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .args([
            "--skip-git-repo-check",
            "--json",
            "-c",
            "features.cli_daybreak=false",
            "-c",
            "daybreak=false",
            "hello",
        ])
        .output()?;
    let thread_id = started_thread(output)?;
    assert_eq!(
        response.single_request().body_json().get("access_programs"),
        None
    );
    for (args, expected) in [
        (
            vec![
                "-c",
                "daybreak=true",
                "resume",
                thread_id.as_str(),
                "continue",
            ],
            "daybreak_blue",
        ),
        (vec!["resume", thread_id.as_str(), "continue"], "standard"),
    ] {
        let response =
            responses::mount_sse_once(&server, responses::sse_completed("continued-standard"))
                .await;
        test.cmd_with_server(&server)
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .arg("--skip-git-repo-check")
            .args(args)
            .assert()
            .success();
        assert_eq!(
            response.single_request().body_json()["access_programs"],
            json!({"cyber": expected})
        );
    }
    Ok(())
}
