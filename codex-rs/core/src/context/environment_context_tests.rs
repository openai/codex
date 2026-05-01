use crate::shell::ShellType;

use super::*;
use codex_utils_absolute_path::test_support::PathBufExt;
use core_test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn fake_shell_name() -> String {
    let shell = crate::shell::Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };
    shell.name().to_string()
}

fn test_abs_path(unix_path: &str) -> AbsolutePathBuf {
    test_path_buf(unix_path).abs()
}

#[test]
fn serialize_workspace_write_environment_context() {
    let cwd = test_path_buf("/repo");
    let context = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: cwd.abs(),
            shell: fake_shell_name(),
        }],
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        /*network*/ None,
        /*subagents*/ None,
    );

    let expected = format!(
        r#"<environment_context>
  <cwd>{cwd}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#,
        cwd = cwd.display(),
    );

    assert_eq!(context.render(), expected);
}

#[test]
fn serialize_environment_context_with_network() {
    let network = NetworkContext::new(
        vec!["api.example.com".to_string(), "*.openai.com".to_string()],
        vec!["blocked.example.com".to_string()],
    );
    let context = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_path_buf("/repo").abs(),
            shell: fake_shell_name(),
        }],
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        Some(network),
        /*subagents*/ None,
    );

    let expected = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <network enabled="true">
    <allowed>api.example.com</allowed>
    <allowed>*.openai.com</allowed>
    <denied>blocked.example.com</denied>
  </network>
</environment_context>"#,
        test_path_buf("/repo").display()
    );

    assert_eq!(context.render(), expected);
}

#[test]
fn serialize_read_only_environment_context() {
    let context = EnvironmentContext::new(
        Vec::new(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        /*network*/ None,
        /*subagents*/ None,
    );

    let expected = r#"<environment_context>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

    assert_eq!(context.render(), expected);
}

#[test]
fn equals_except_shell_compares_cwd() {
    let context1 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_abs_path("/repo"),
            shell: fake_shell_name(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );
    let context2 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_abs_path("/repo"),
            shell: fake_shell_name(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );
    assert!(context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_compares_cwd_differences() {
    let context1 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_abs_path("/repo1"),
            shell: fake_shell_name(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );
    let context2 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_abs_path("/repo2"),
            shell: fake_shell_name(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );

    assert!(!context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_ignores_shell() {
    let context1 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_abs_path("/repo"),
            shell: "bash".to_string(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );
    let context2 = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "other".to_string(),
            cwd: test_abs_path("/repo"),
            shell: "zsh".to_string(),
        }],
        /*current_date*/ None,
        /*timezone*/ None,
        /*network*/ None,
        /*subagents*/ None,
    );

    assert!(context1.equals_except_shell(&context2));
}

#[test]
fn serialize_environment_context_with_subagents() {
    let context = EnvironmentContext::new(
        vec![EnvironmentContextEnvironment {
            id: "local".to_string(),
            cwd: test_path_buf("/repo").abs(),
            shell: fake_shell_name(),
        }],
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        /*network*/ None,
        Some("- agent-1: atlas\n- agent-2".to_string()),
    );

    let expected = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <subagents>
    - agent-1: atlas
    - agent-2
  </subagents>
</environment_context>"#,
        test_path_buf("/repo").display()
    );

    assert_eq!(context.render(), expected);
}

#[test]
fn serialize_environment_context_with_multiple_selected_environments() {
    let local_cwd = test_path_buf("/repo/local");
    let remote_cwd = test_path_buf("/repo/remote");
    let context = EnvironmentContext::new(
        vec![
            EnvironmentContextEnvironment {
                id: "local".to_string(),
                cwd: local_cwd.abs(),
                shell: "bash".to_string(),
            },
            EnvironmentContextEnvironment {
                id: "remote".to_string(),
                cwd: remote_cwd.abs(),
                shell: "bash".to_string(),
            },
        ],
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        /*network*/ None,
        /*subagents*/ None,
    );

    let expected = format!(
        r#"<environment_context>
  <environments>
    <environment id="local">
      <cwd>{}</cwd>
      <shell>bash</shell>
    </environment>
    <environment id="remote">
      <cwd>{}</cwd>
      <shell>bash</shell>
    </environment>
  </environments>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#,
        local_cwd.display(),
        remote_cwd.display()
    );

    assert_eq!(context.render(), expected);
}
