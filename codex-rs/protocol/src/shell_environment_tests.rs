use std::collections::HashMap;
use std::process::Command;

use pretty_assertions::assert_eq;

use super::*;

const CHILD_MODE_ENV_VAR: &str = "CODEX_SHELL_ENVIRONMENT_SCRUBBER_TEST_MODE";
const TEST_NAME: &str =
    "shell_environment::tests::command_scrubber_removes_names_from_real_child_environment";

#[test]
fn non_inheritable_environment_is_removed_after_policy_overrides() {
    let vars = [
        ("SAFE".to_string(), "inherited".to_string()),
        (
            "openai_federation_rule_id".to_string(),
            "inherited-rule".to_string(),
        ),
        (
            "OPENAI_WORKLOAD_IDENTITY_CONTEXT".to_string(),
            r#"{"instance_id":"box-one"}"#.to_string(),
        ),
        (
            "codex_exec_server_noise_auth_token".to_string(),
            "inherited-noise-token".to_string(),
        ),
    ];
    let policy = ShellEnvironmentPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: true,
        r#set: HashMap::from([
            ("SAFE".to_string(), "override".to_string()),
            (
                "OpenAI_Identity_Token_File".to_string(),
                "/run/identity-token".to_string(),
            ),
            (
                "Codex_Exec_Server_Noise_Auth_Token".to_string(),
                "configured-noise-token".to_string(),
            ),
        ]),
        ..Default::default()
    };

    assert_eq!(
        populate_env(vars, &policy, /*thread_id*/ None),
        HashMap::from([("SAFE".to_string(), "override".to_string())])
    );
}

#[test]
fn command_scrubber_removes_names_from_real_child_environment() {
    if std::env::var_os(CHILD_MODE_ENV_VAR).is_none() {
        let output = Command::new(std::env::current_exe().expect("locate current test binary"))
            .args([TEST_NAME, "--exact", "--nocapture"])
            .env(CHILD_MODE_ENV_VAR, "1")
            .env("OpenAI_Federation_Rule_Id", "inherited-rule")
            .env(
                "OpenAI_Workload_Identity_Context",
                r#"{"instance_id":"box-one"}"#,
            )
            .env(
                "Codex_Exec_Server_Noise_Auth_Token",
                "inherited-noise-token",
            )
            .output()
            .expect("run inherited-environment test process");
        assert!(
            output.status.success(),
            "child failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    let mut command = environment_command();
    command
        .env("openai_identity_token_file", "/run/identity-token")
        .env(
            CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR,
            "configured-noise-token",
        )
        .env("SAFE", "value");
    scrub_non_inheritable_env_vars(&mut command);
    let output = command.output().expect("read child environment");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let restricted_names = stdout
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name))
        .filter(|name| is_non_inheritable_env_var(name))
        .collect::<Vec<_>>();
    assert_eq!(restricted_names, Vec::<&str>::new());
}

#[cfg(windows)]
fn environment_command() -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/D", "/C", "set"]);
    command
}

#[cfg(not(windows))]
fn environment_command() -> Command {
    Command::new("env")
}
