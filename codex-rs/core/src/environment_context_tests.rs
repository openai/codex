use crate::shell::ShellType;

use super::*;
use core_test_support::test_path_buf;
use pretty_assertions::assert_eq;

fn fake_shell() -> Shell {
    Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    }
}

#[test]
fn serialize_workspace_write_environment_context() {
    let cwd = test_path_buf("/repo");
    let context = EnvironmentContext::new(
        Some(cwd.clone()),
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
        None,
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

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_environment_context_with_network() {
    let network = NetworkContext {
        allowed_domains: vec!["api.example.com".to_string(), "*.openai.com".to_string()],
        denied_domains: vec!["blocked.example.com".to_string()],
    };
    let context = EnvironmentContext::new(
        Some(test_path_buf("/repo")),
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        Some(network),
        Vec::new(),
        None,
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

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_read_only_environment_context() {
    let context = EnvironmentContext::new(
        None,
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
        None,
    );

    let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_external_sandbox_environment_context() {
    let context = EnvironmentContext::new(
        None,
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
        None,
    );

    let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_external_sandbox_with_restricted_network_environment_context() {
    let context = EnvironmentContext::new(
        None,
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
        None,
    );

    let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_full_access_environment_context() {
    let context = EnvironmentContext::new(
        None,
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
        None,
    );

    let expected = r#"<environment_context>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
</environment_context>"#;

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn equals_except_shell_compares_cwd() {
    let context1 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    let context2 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    assert!(context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_ignores_sandbox_policy() {
    let context1 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    let context2 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );

    assert!(context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_compares_cwd_differences() {
    let context1 = EnvironmentContext::new(
        Some(PathBuf::from("/repo1")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    let context2 = EnvironmentContext::new(
        Some(PathBuf::from("/repo2")),
        fake_shell(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );

    assert!(!context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_compares_deny_read_patterns() {
    let context1 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        vec!["/repo/.gitconfig".to_string()],
        None,
    );
    let context2 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        fake_shell(),
        None,
        None,
        None,
        vec!["/repo/.ssh".to_string()],
        None,
    );

    assert!(!context1.equals_except_shell(&context2));
}

#[test]
fn equals_except_shell_ignores_shell() {
    let context1 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        Shell {
            shell_type: ShellType::Bash,
            shell_path: "/bin/bash".into(),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        },
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    let context2 = EnvironmentContext::new(
        Some(PathBuf::from("/repo")),
        Shell {
            shell_type: ShellType::Zsh,
            shell_path: "/bin/zsh".into(),
            shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
        },
        None,
        None,
        None,
        Vec::new(),
        None,
    );

    assert!(context1.equals_except_shell(&context2));
}

#[test]
fn serialize_environment_context_with_deny_read_patterns() {
    let denied = vec!["/repo/.gitconfig".to_string(), "/repo/.ssh".to_string()];
    let context = EnvironmentContext::new(
        Some(test_path_buf("/repo")),
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        denied,
        None,
    );

    let expected = format!(
        r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <current_date>2026-02-26</current_date>
  <timezone>America/Los_Angeles</timezone>
  <deny_read_patterns>
    <pattern>/repo/.gitconfig</pattern>
    <pattern>/repo/.ssh</pattern>
  </deny_read_patterns>
</environment_context>"#,
        test_path_buf("/repo").display()
    );

    assert_eq!(context.serialize_to_xml(), expected);
}

#[test]
fn serialize_environment_context_with_subagents() {
    let context = EnvironmentContext::new(
        Some(test_path_buf("/repo")),
        fake_shell(),
        Some("2026-02-26".to_string()),
        Some("America/Los_Angeles".to_string()),
        None,
        Vec::new(),
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

    assert_eq!(context.serialize_to_xml(), expected);
}
