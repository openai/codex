use clap::Args;
use codex_status::CodexStatus;
use codex_status::StatusClient;
use std::io::Write;
use std::io::{self};

#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Emit the Codex-only status as prettified JSON instead of a human summary.
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

pub async fn run_status(cmd: StatusCommand) -> anyhow::Result<()> {
    let client = StatusClient::new()?;
    let codex_status = client.fetch_codex_status().await?;

    if cmd.json {
        let json = serde_json::to_string_pretty(&codex_status)?;
        println!("{json}");
    } else {
        write_human(&codex_status, &mut io::stdout())?;
    }

    Ok(())
}

fn write_human<W: Write>(status: &CodexStatus, writer: &mut W) -> anyhow::Result<()> {
    writeln!(writer, "Codex status: {}", status.status)?;
    writeln!(writer, "operational: {}", status.is_operational)?;
    writeln!(writer, "component_id: {}", status.component_id)?;

    if let Some(affected) = &status.raw_affected {
        writeln!(writer, "affected status: {}", affected.status)?;
    } else {
        writeln!(writer, "affected status: none")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_status::AffectedComponent;
    use codex_status::ComponentHealth;

    #[test]
    fn human_output_handles_absent_affected_entry() -> anyhow::Result<()> {
        let status = CodexStatus {
            component_id: "cmp-1".to_string(),
            name: "Codex".to_string(),
            status: ComponentHealth::Operational,
            is_operational: true,
            raw_affected: None,
        };

        let mut buffer = Vec::new();
        write_human(&status, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("Codex status: operational"));
        assert!(output.contains("operational: true"));
        assert!(output.contains("component_id: cmp-1"));
        assert!(output.contains("affected status: none"));
        Ok(())
    }

    #[test]
    fn human_output_shows_affected_status() -> anyhow::Result<()> {
        let status = CodexStatus {
            component_id: "cmp-1".to_string(),
            name: "Codex".to_string(),
            status: ComponentHealth::DegradedPerformance,
            is_operational: false,
            raw_affected: Some(AffectedComponent {
                component_id: "cmp-1".to_string(),
                status: ComponentHealth::DegradedPerformance,
            }),
        };

        let mut buffer = Vec::new();
        write_human(&status, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("Codex status: degraded_performance"));
        assert!(output.contains("operational: false"));
        assert!(output.contains("affected status: degraded_performance"));
        Ok(())
    }
}
