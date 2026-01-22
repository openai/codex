use anyhow::Result;
pub use codex_protocol::protocol::SandboxPolicy;

pub fn parse_policy(value: &str) -> Result<SandboxPolicy> {
    match value {
        "read-only" => Ok(SandboxPolicy::ReadOnly),
        "workspace-write" => Ok(SandboxPolicy::new_workspace_write_policy()),
        "danger-full-access" | "external-sandbox" => anyhow::bail!(
            "DangerFullAccess and ExternalSandbox are not supported for sandboxing"
        ),
        other => {
            let parsed: SandboxPolicy = serde_json::from_str(other)?;
            if matches!(
                parsed,
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
            ) {
                anyhow::bail!(
                    "DangerFullAccess and ExternalSandbox are not supported for sandboxing"
                );
            }
            Ok(parsed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn rejects_external_sandbox_preset() {
        let err = parse_policy("external-sandbox").unwrap_err();
        assert!(err
            .to_string()
            .contains("DangerFullAccess and ExternalSandbox are not supported"));
    }

    #[test]
    fn rejects_external_sandbox_json() {
        let payload = serde_json::to_string(
            &codex_protocol::protocol::SandboxPolicy::ExternalSandbox {
                network_access: codex_protocol::protocol::NetworkAccess::Enabled,
            },
        )
        .unwrap();
        let err = parse_policy(&payload).unwrap_err();
        assert!(err
            .to_string()
            .contains("DangerFullAccess and ExternalSandbox are not supported"));
    }

    #[test]
    fn parses_read_only_policy() {
        assert_eq!(parse_policy("read-only").unwrap(), SandboxPolicy::ReadOnly);
    }
}
