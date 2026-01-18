use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use clap::ValueEnum;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_lsp::DiagnosticEntry;
use codex_lsp::LanguageServerId;
use codex_lsp::LspManager;
use codex_lsp::SeverityFilter;
use serde::Serialize;

#[derive(Debug, clap::Parser)]
pub struct LspCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: LspSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum LspSubcommand {
    Status(StatusArgs),
    Diagnostics(DiagnosticsArgs),
    Install(InstallArgs),
}

#[derive(Debug, clap::Parser)]
pub struct StatusArgs {
    /// Output status as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct DiagnosticsArgs {
    /// Optional file path to query diagnostics for.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Filter by severity (errors, warnings, all).
    #[arg(long, value_enum, default_value_t = DiagnosticsSeverity::Errors)]
    pub severity: DiagnosticsSeverity,

    /// Output diagnostics as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum DiagnosticsSeverity {
    Errors,
    Warnings,
    All,
}

impl DiagnosticsSeverity {
    fn filter(self) -> SeverityFilter {
        match self {
            DiagnosticsSeverity::Errors => SeverityFilter::Errors,
            DiagnosticsSeverity::Warnings => SeverityFilter::ErrorsAndWarnings,
            DiagnosticsSeverity::All => SeverityFilter::All,
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct InstallArgs {
    /// Optional language server id to install.
    pub server_id: Option<String>,
}

impl LspCli {
    pub async fn run(self) -> Result<()> {
        let overrides = self
            .config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;
        let config = Config::load_with_cli_overrides(overrides)
            .await
            .context("failed to load configuration")?;
        let manager = LspManager::new(config.lsp.clone(), config.cwd.clone());

        match self.subcommand {
            LspSubcommand::Status(args) => run_status(manager, args).await,
            LspSubcommand::Diagnostics(args) => run_diagnostics(manager, args).await,
            LspSubcommand::Install(args) => run_install(manager, args).await,
        }
    }
}

#[derive(Serialize)]
struct StatusEntryJson {
    id: String,
    enabled: bool,
    detected: bool,
    running: bool,
    installed: bool,
    root: Option<String>,
    command: Vec<String>,
}

async fn run_status(manager: LspManager, args: StatusArgs) -> Result<()> {
    let status = manager.status().await?;
    if args.json {
        let entries: Vec<StatusEntryJson> = status
            .entries
            .into_iter()
            .map(|entry| StatusEntryJson {
                id: entry.id.as_str().to_string(),
                enabled: entry.enabled,
                detected: entry.detected,
                running: entry.running,
                installed: entry.installed,
                root: entry.root.map(|path| path.display().to_string()),
                command: entry.command,
            })
            .collect();
        let payload = serde_json::to_string_pretty(&entries)?;
        println!("{payload}");
        return Ok(());
    }

    for entry in status.entries {
        let root = entry
            .root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string());
        let command = if entry.command.is_empty() {
            "-".to_string()
        } else {
            entry.command.join(" ")
        };
        let id = entry.id.as_str();
        let enabled = entry.enabled;
        let detected = entry.detected;
        let running = entry.running;
        let installed = entry.installed;
        println!(
            "{id}\tenabled={enabled}\tdetected={detected}\trunning={running}\tinstalled={installed}\troot={root}\tcommand={command}"
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct DiagnosticJson {
    file: String,
    line: u32,
    character: u32,
    severity: String,
    code: Option<String>,
    message: String,
    source: Option<String>,
}

async fn run_diagnostics(manager: LspManager, args: DiagnosticsArgs) -> Result<()> {
    let filter = args.severity.filter();
    let path = args.file.map(resolve_cli_path).transpose()?;
    let wait = path.as_ref().map(|_| Duration::from_millis(500));
    let diagnostics = manager.diagnostics_for(path, filter, wait).await?;

    if args.json {
        let items: Vec<DiagnosticJson> = diagnostics
            .into_iter()
            .map(|entry| diagnostic_json(entry))
            .collect();
        let payload = serde_json::to_string_pretty(&items)?;
        println!("{payload}");
        return Ok(());
    }

    let content = render_diagnostics(&diagnostics);
    println!("{content}");
    Ok(())
}

async fn run_install(manager: LspManager, args: InstallArgs) -> Result<()> {
    let id = args
        .server_id
        .map(|id| LanguageServerId::from_str(&id))
        .transpose()
        .map_err(|e| anyhow::anyhow!("invalid language server id: {e}"))?;
    let installed = manager.install(id).await?;
    if installed.is_empty() {
        println!("No language servers installed.");
        return Ok(());
    }
    let names = installed
        .into_iter()
        .map(|id| id.as_str().to_string())
        .collect::<Vec<_>>();
    let list = names.join(", ");
    println!("Installed: {list}");
    Ok(())
}

fn resolve_cli_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().context("resolve current directory")?;
    Ok(cwd.join(path))
}

fn diagnostic_json(entry: DiagnosticEntry) -> DiagnosticJson {
    let diagnostic = entry.diagnostic;
    let severity = diagnostic
        .severity
        .map(|severity| format!("{severity:?}").to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
    let code = diagnostic.code.map(|code| format!("{code:?}"));
    DiagnosticJson {
        file: entry.path.display().to_string(),
        line: diagnostic.range.start.line + 1,
        character: diagnostic.range.start.character + 1,
        severity,
        code,
        message: diagnostic.message,
        source: diagnostic.source,
    }
}

fn render_diagnostics(diagnostics: &[DiagnosticEntry]) -> String {
    if diagnostics.is_empty() {
        return "No diagnostics.".to_string();
    }
    let mut out = String::new();
    for entry in diagnostics {
        let diagnostic = &entry.diagnostic;
        let severity = diagnostic
            .severity
            .map(|severity| format!("{severity:?}").to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let line = diagnostic.range.start.line + 1;
        let character = diagnostic.range.start.character + 1;
        let path_display = entry.path.display();
        let message = diagnostic.message.trim();
        if let Some(source) = diagnostic.source.as_deref() {
            out.push_str(&format!(
                "- {path_display}:{line}:{character} [{severity}] {message} ({source})\n"
            ));
        } else {
            out.push_str(&format!(
                "- {path_display}:{line}:{character} [{severity}] {message}\n"
            ));
        }
    }
    out
}
