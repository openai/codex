use clap::Args;
use codex_status::CodexStatusReport;
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
    let report = client.fetch_codex_status().await?;

    if cmd.json {
        let json = serde_json::to_string_pretty(&report)?;
        println!("{json}");
    } else {
        write_human(&report, &mut io::stdout())?;
    }

    Ok(())
}

fn write_human<W: Write>(report: &CodexStatusReport, writer: &mut W) -> anyhow::Result<()> {
    writeln!(
        writer,
        "overall: {} ({})",
        report.overall_description, report.overall_indicator
    )?;
    writeln!(writer, "updated_at: {}", report.updated_at)?;

    writeln!(writer, "codex components:")?;
    if report.components.is_empty() {
        writeln!(writer, "  none")?;
    } else {
        for component in &report.components {
            writeln!(writer, "  {}: {}", component.name, component.status)?;
        }
    }

    writeln!(writer, "codex incidents:")?;
    if report.incidents.is_empty() {
        writeln!(writer, "  none")?;
    } else {
        for incident in &report.incidents {
            writeln!(
                writer,
                "  {}: status={} impact={} updated={}",
                incident.name, incident.status, incident.impact, incident.updated_at
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_status::ComponentStatus;
    use codex_status::IncidentStatus;

    #[test]
    fn human_output_handles_empty_lists() -> anyhow::Result<()> {
        let report = CodexStatusReport {
            overall_description: "All Systems Operational".to_string(),
            overall_indicator: "none".to_string(),
            updated_at: "2025-11-07T21:55:20Z".to_string(),
            components: Vec::new(),
            incidents: Vec::new(),
        };

        let mut buffer = Vec::new();
        write_human(&report, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("overall: All Systems Operational (none)"));
        assert!(output.contains("updated_at: 2025-11-07T21:55:20Z"));
        assert!(output.contains("codex components:\n  none"));
        assert!(output.contains("codex incidents:\n  none"));
        Ok(())
    }

    #[test]
    fn human_output_lists_components_and_incidents() -> anyhow::Result<()> {
        let report = CodexStatusReport {
            overall_description: "Degraded".to_string(),
            overall_indicator: "minor".to_string(),
            updated_at: "2025-11-07T21:55:20Z".to_string(),
            components: vec![ComponentStatus {
                name: "Codex".to_string(),
                status: "degraded_performance".to_string(),
            }],
            incidents: vec![IncidentStatus {
                name: "Codex degraded".to_string(),
                status: "investigating".to_string(),
                impact: "minor".to_string(),
                updated_at: "2025-11-07T21:45:00Z".to_string(),
            }],
        };

        let mut buffer = Vec::new();
        write_human(&report, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("codex components:\n  Codex: degraded_performance"));
        assert!(output.contains(
            "codex incidents:\n  Codex degraded: status=investigating impact=minor updated=2025-11-07T21:45:00Z"
        ));
        Ok(())
    }
}
