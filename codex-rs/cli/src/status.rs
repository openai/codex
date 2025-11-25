use clap::Args;
use codex_status::fetch_codex_health;
use codex_status::ComponentHealth;
use std::io::Write;
use std::io::{self};

#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Emit the Codex-only status as prettified JSON instead of a human summary.
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

pub async fn run_status(cmd: StatusCommand) -> anyhow::Result<()> {
    let component_health = fetch_codex_health().await?;

    if cmd.json {
        let json = serde_json::to_string_pretty(&component_health)?;
        println!("{json}");
    } else {
        write_human(component_health, &mut io::stdout())?;
    }

    Ok(())
}

fn write_human<W: Write>(status: ComponentHealth, writer: &mut W) -> anyhow::Result<()> {
    writeln!(writer, "Codex status: {status}")?;
    writeln!(writer, "operational: {}", status.is_operational())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_output_handles_absent_affected_entry() -> anyhow::Result<()> {
        let mut buffer = Vec::new();
        write_human(ComponentHealth::Operational, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("Codex status: operational"));
        assert!(output.contains("operational: true"));
        Ok(())
    }

    #[test]
    fn human_output_shows_affected_status() -> anyhow::Result<()> {
        let mut buffer = Vec::new();
        write_human(ComponentHealth::DegradedPerformance, &mut buffer)?;

        let output = String::from_utf8(buffer).expect("utf8");
        assert!(output.contains("Codex status: degraded_performance"));
        assert!(output.contains("operational: false"));
        Ok(())
    }
}
