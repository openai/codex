use anyhow::Result;
pub use codex_protocol::protocol::SandboxPolicy;

pub fn parse_policy(value: &str) -> Result<SandboxPolicy> {
    match value {
        "read-only" => Ok(SandboxPolicy::ReadOnly),
        "workspace-write" => Ok(SandboxPolicy::new_workspace_write_policy()),
        "danger-full-access" => anyhow::bail!("DangerFullAccess is not supported for sandboxing"),
        other => {
            let parsed: SandboxPolicy = serde_json::from_str(other)?;
            if matches!(parsed, SandboxPolicy::DangerFullAccess) {
                anyhow::bail!("DangerFullAccess is not supported for sandboxing");
            }
            Ok(parsed)
        }
    }
}
