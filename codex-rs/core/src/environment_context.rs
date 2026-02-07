use crate::codex::TurnContext;
use crate::config_loader::NetworkConstraints;
use crate::shell::Shell;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_CLOSE_TAG;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub shell: Shell,
    pub network: Option<NetworkConstraints>,
}

impl EnvironmentContext {
    pub fn new(cwd: Option<PathBuf>, shell: Shell, network: Option<NetworkConstraints>) -> Self {
        Self {
            cwd,
            shell,
            network,
        }
    }

    /// Compares two environment contexts, ignoring the shell. Useful when
    /// comparing turn to turn, since the initial environment_context will
    /// include the shell, and then it is not configurable from turn to turn.
    pub fn equals_except_shell(&self, other: &EnvironmentContext) -> bool {
        self.cwd == other.cwd && self.network == other.network
    }

    pub fn diff(before: &TurnContext, after: &TurnContext, shell: &Shell) -> Self {
        let before_network = Self::network_from_turn_context(before);
        let after_network = Self::network_from_turn_context(after);
        let cwd = if before.cwd != after.cwd {
            Some(after.cwd.clone())
        } else {
            None
        };
        let network = if before_network != after_network {
            after_network
        } else {
            None
        };
        EnvironmentContext::new(cwd, shell.clone(), network)
    }

    pub fn from_turn_context(turn_context: &TurnContext, shell: &Shell) -> Self {
        Self::new(
            Some(turn_context.cwd.clone()),
            shell.clone(),
            Self::network_from_turn_context(turn_context),
        )
    }

    fn network_from_turn_context(turn_context: &TurnContext) -> Option<NetworkConstraints> {
        turn_context
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()
            .map(|network| network.value.clone())
    }
}

impl EnvironmentContext {
    /// Serializes the environment context to XML. Libraries like `quick-xml`
    /// require custom macros to handle Enums with newtypes, so we just do it
    /// manually, to keep things simple. Output looks like:
    ///
    /// ```xml
    /// <environment_context>
    ///   <cwd>...</cwd>
    ///   <shell>...</shell>
    /// </environment_context>
    /// ```
    pub fn serialize_to_xml(self) -> String {
        let mut lines = vec![ENVIRONMENT_CONTEXT_OPEN_TAG.to_string()];
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }

        let shell_name = self.shell.name();
        lines.push(format!("  <shell>{shell_name}</shell>"));
        if let Some(network) = self.network {
            lines.push("  <network>".to_string());
            if let Some(enabled) = network.enabled {
                lines.push(format!("    <enabled>{enabled}</enabled>"));
            }
            if let Some(http_port) = network.http_port {
                lines.push(format!("    <http_port>{http_port}</http_port>"));
            }
            if let Some(socks_port) = network.socks_port {
                lines.push(format!("    <socks_port>{socks_port}</socks_port>"));
            }
            if let Some(allow_upstream_proxy) = network.allow_upstream_proxy {
                lines.push(format!(
                    "    <allow_upstream_proxy>{allow_upstream_proxy}</allow_upstream_proxy>"
                ));
            }
            if let Some(dangerously_allow_non_loopback_proxy) =
                network.dangerously_allow_non_loopback_proxy
            {
                lines.push(format!(
                    "    <dangerously_allow_non_loopback_proxy>{dangerously_allow_non_loopback_proxy}</dangerously_allow_non_loopback_proxy>"
                ));
            }
            if let Some(dangerously_allow_non_loopback_admin) =
                network.dangerously_allow_non_loopback_admin
            {
                lines.push(format!(
                    "    <dangerously_allow_non_loopback_admin>{dangerously_allow_non_loopback_admin}</dangerously_allow_non_loopback_admin>"
                ));
            }
            if let Some(allowed_domains) = network.allowed_domains {
                lines.push(format!(
                    "    <allowed_domains>{}</allowed_domains>",
                    allowed_domains.join(", ")
                ));
            }
            if let Some(denied_domains) = network.denied_domains {
                lines.push(format!(
                    "    <denied_domains>{}</denied_domains>",
                    denied_domains.join(", ")
                ));
            }
            if let Some(allow_unix_sockets) = network.allow_unix_sockets {
                lines.push(format!(
                    "    <allow_unix_sockets>{}</allow_unix_sockets>",
                    allow_unix_sockets.join(", ")
                ));
            }
            if let Some(allow_local_binding) = network.allow_local_binding {
                lines.push(format!(
                    "    <allow_local_binding>{allow_local_binding}</allow_local_binding>"
                ));
            }
            lines.push("  </network>".to_string());
        }
        lines.push(ENVIRONMENT_CONTEXT_CLOSE_TAG.to_string());
        lines.join("\n")
    }
}

impl From<EnvironmentContext> for ResponseItem {
    fn from(ec: EnvironmentContext) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: ec.serialize_to_xml(),
            }],
            end_turn: None,
            phase: None,
        }
    }
}

#[cfg(test)]
mod tests {
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
        let context = EnvironmentContext::new(Some(cwd.clone()), fake_shell(), None);

        let expected = format!(
            r#"<environment_context>
  <cwd>{cwd}</cwd>
  <shell>bash</shell>
</environment_context>"#,
            cwd = cwd.display(),
        );

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_environment_context_with_network() {
        let network = NetworkConstraints {
            enabled: Some(true),
            http_port: Some(3128),
            socks_port: Some(1080),
            allow_upstream_proxy: Some(false),
            dangerously_allow_non_loopback_proxy: Some(false),
            dangerously_allow_non_loopback_admin: Some(true),
            allowed_domains: Some(vec![
                "api.example.com".to_string(),
                "*.openai.com".to_string(),
            ]),
            denied_domains: Some(vec!["blocked.example.com".to_string()]),
            allow_unix_sockets: Some(vec!["/tmp/example.sock".to_string()]),
            allow_local_binding: Some(true),
        };
        let context =
            EnvironmentContext::new(Some(test_path_buf("/repo")), fake_shell(), Some(network));

        let expected = format!(
            r#"<environment_context>
  <cwd>{}</cwd>
  <shell>bash</shell>
  <network>
    <enabled>true</enabled>
    <http_port>3128</http_port>
    <socks_port>1080</socks_port>
    <allow_upstream_proxy>false</allow_upstream_proxy>
    <dangerously_allow_non_loopback_proxy>false</dangerously_allow_non_loopback_proxy>
    <dangerously_allow_non_loopback_admin>true</dangerously_allow_non_loopback_admin>
    <allowed_domains>api.example.com, *.openai.com</allowed_domains>
    <denied_domains>blocked.example.com</denied_domains>
    <allow_unix_sockets>/tmp/example.sock</allow_unix_sockets>
    <allow_local_binding>true</allow_local_binding>
  </network>
</environment_context>"#,
            test_path_buf("/repo").display()
        );

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_read_only_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_external_sandbox_with_restricted_network_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn serialize_full_access_environment_context() {
        let context = EnvironmentContext::new(None, fake_shell(), None);

        let expected = r#"<environment_context>
  <shell>bash</shell>
</environment_context>"#;

        assert_eq!(context.serialize_to_xml(), expected);
    }

    #[test]
    fn equals_except_shell_compares_cwd() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_ignores_sandbox_policy() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo")), fake_shell(), None);

        assert!(context1.equals_except_shell(&context2));
    }

    #[test]
    fn equals_except_shell_compares_cwd_differences() {
        let context1 = EnvironmentContext::new(Some(PathBuf::from("/repo1")), fake_shell(), None);
        let context2 = EnvironmentContext::new(Some(PathBuf::from("/repo2")), fake_shell(), None);

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
        );
        let context2 = EnvironmentContext::new(
            Some(PathBuf::from("/repo")),
            Shell {
                shell_type: ShellType::Zsh,
                shell_path: "/bin/zsh".into(),
                shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
            },
            None,
        );

        assert!(context1.equals_except_shell(&context2));
    }
}
