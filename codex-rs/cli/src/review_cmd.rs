//! Code review CLI commands

use anyhow::Result;
use clap::Parser;
use codex_code_review::{
    ContinuousReviewer, ReviewConfig, ReviewTask, ReviewType,
};
use std::path::PathBuf;
use tokio::signal;
use tracing::info;

/// Code review commands
#[derive(Debug, Parser)]
pub struct ReviewCli {
    #[command(subcommand)]
    pub command: ReviewCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum ReviewCommand {
    /// Start continuous code review
    Start(StartCommand),

    /// Analyze a specific file or directory
    Analyze(AnalyzeCommand),

    /// Show review statistics
    Stats(StatsCommand),

    /// List review sessions
    Sessions(SessionsCommand),

    /// Configure continuous review
    Configure(ConfigureCommand),
}

#[derive(Debug, Parser)]
pub struct StartCommand {
    /// Directory to watch (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Duration to run (in hours)
    #[arg(short = 't', long)]
    pub duration: Option<u64>,

    /// Run indefinitely (until interrupted)
    #[arg(short = 'i', long)]
    pub indefinite: bool,

    /// Session directory (defaults to ~/.codex/review-sessions)
    #[arg(long)]
    pub session_dir: Option<PathBuf>,

    /// Configuration file
    #[arg(long)]
    pub config_file: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct AnalyzeCommand {
    /// File or directory to analyze
    pub target: PathBuf,

    /// Review type: incremental, full, or quick
    #[arg(short, long, default_value = "full")]
    pub review_type: String,

    /// Output format: text or json
    #[arg(short, long, default_value = "text")]
    pub output: String,

    /// Configuration file
    #[arg(long)]
    pub config_file: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct StatsCommand {
    /// Session ID (defaults to current session)
    #[arg(short, long)]
    pub session: Option<String>,

    /// Session directory (defaults to ~/.codex/review-sessions)
    #[arg(long)]
    pub session_dir: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct SessionsCommand {
    /// Session directory (defaults to ~/.codex/review-sessions)
    #[arg(long)]
    pub session_dir: Option<PathBuf>,

    /// Show detailed information
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Parser)]
pub struct ConfigureCommand {
    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Initialize default configuration
    #[arg(long)]
    pub init: bool,

    /// Configuration file path
    #[arg(long)]
    pub config_file: Option<PathBuf>,
}

/// Run the review CLI command
pub async fn run_review_command(cli: ReviewCli) -> Result<()> {
    match cli.command {
        ReviewCommand::Start(cmd) => run_start_command(cmd).await,
        ReviewCommand::Analyze(cmd) => run_analyze_command(cmd).await,
        ReviewCommand::Stats(cmd) => run_stats_command(cmd).await,
        ReviewCommand::Sessions(cmd) => run_sessions_command(cmd).await,
        ReviewCommand::Configure(cmd) => run_configure_command(cmd).await,
    }
}

async fn run_start_command(cmd: StartCommand) -> Result<()> {
    println!("🚀 Starting continuous code review...");
    println!("   Watching: {:?}", cmd.dir.canonicalize()?);

    // Load configuration
    let config = if let Some(config_path) = cmd.config_file {
        load_config_from_file(&config_path).await?
    } else {
        ReviewConfig::default()
    };

    // Determine session directory
    let session_dir = cmd.session_dir.unwrap_or_else(|| {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".codex/review-sessions")
    });

    // Create reviewer
    let reviewer = ContinuousReviewer::new(config, session_dir, cmd.dir)?;

    // Start in background
    let reviewer_handle = tokio::spawn(async move {
        reviewer.start().await
    });

    println!("✓ Continuous review started");
    println!("  Press Ctrl+C to stop");

    // Wait for signal or duration
    if cmd.indefinite || cmd.duration.is_none() {
        signal::ctrl_c().await?;
        println!("\n⏸  Stopping continuous review...");
    } else if let Some(hours) = cmd.duration {
        let duration = tokio::time::Duration::from_secs(hours * 3600);
        tokio::select! {
            _ = tokio::time::sleep(duration) => {
                println!("\n⏱  Duration completed, stopping...");
            }
            _ = signal::ctrl_c() => {
                println!("\n⏸  Interrupted, stopping...");
            }
        }
    }

    // Wait for reviewer to finish
    reviewer_handle.abort();
    println!("✓ Continuous review stopped");

    Ok(())
}

async fn run_analyze_command(cmd: AnalyzeCommand) -> Result<()> {
    println!("🔍 Analyzing: {:?}", cmd.target);

    // Load configuration
    let config = if let Some(config_path) = cmd.config_file {
        load_config_from_file(&config_path).await?
    } else {
        ReviewConfig::default()
    };

    // Create analyzer
    let analyzer = codex_code_review::CodeAnalyzer::new(config.analysis);

    // Analyze target
    if cmd.target.is_file() {
        let result = analyzer.analyze_file(&cmd.target).await?;

        if cmd.output == "json" {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            print_analysis_result(&result);
        }
    } else if cmd.target.is_dir() {
        println!("Directory analysis not yet implemented");
        // Implement directory scanning
    } else {
        anyhow::bail!("Target must be a file or directory");
    }

    Ok(())
}

async fn run_stats_command(cmd: StatsCommand) -> Result<()> {
    let session_dir = cmd.session_dir.unwrap_or_else(|| {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".codex/review-sessions")
    });

    println!("📊 Review Statistics");
    println!("   Session directory: {:?}", session_dir);

    // Load session and display stats
    // This would integrate with SessionManager
    println!("   (Stats feature to be implemented)");

    Ok(())
}

async fn run_sessions_command(cmd: SessionsCommand) -> Result<()> {
    let session_dir = cmd.session_dir.unwrap_or_else(|| {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".codex/review-sessions")
    });

    println!("📋 Review Sessions");
    println!("   Session directory: {:?}", session_dir);

    if session_dir.exists() {
        let sessions = codex_code_review::ReviewSession::list_sessions(&session_dir).await?;

        if sessions.is_empty() {
            println!("   No sessions found");
        } else {
            println!("   Found {} session(s):", sessions.len());
            for session_id in sessions {
                println!("   • {}", session_id);

                if cmd.verbose {
                    if let Ok(session) = codex_code_review::ReviewSession::load(&session_id, &session_dir).await {
                        println!("     Started: {}", session.started_at);
                        println!("     State: {:?}", session.state);
                        println!("     Files analyzed: {}", session.statistics.files_analyzed);
                        println!("     Issues found: {}", session.statistics.total_issues);
                        println!("     Improvements: {}", session.statistics.improvements_applied);
                    }
                }
            }
        }
    } else {
        println!("   Session directory does not exist");
    }

    Ok(())
}

async fn run_configure_command(cmd: ConfigureCommand) -> Result<()> {
    if cmd.show {
        println!("📝 Current Configuration");
        let config = ReviewConfig::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else if cmd.init {
        let config_path = cmd.config_file.unwrap_or_else(|| PathBuf::from("codex-review.toml"));
        println!("Initializing configuration at: {:?}", config_path);

        let config = ReviewConfig::default();
        let toml = toml::to_string_pretty(&config)?;
        tokio::fs::write(&config_path, toml).await?;

        println!("✓ Configuration file created");
    } else {
        println!("Use --show to display current configuration");
        println!("Use --init to create a default configuration file");
    }

    Ok(())
}

// Helper functions

async fn load_config_from_file(path: &PathBuf) -> Result<ReviewConfig> {
    let content = tokio::fs::read_to_string(path).await?;
    let config: ReviewConfig = toml::from_str(&content)?;
    Ok(config)
}

fn print_analysis_result(result: &codex_code_review::AnalysisResult) {
    println!("\n📊 Analysis Results for {:?}", result.file_path);
    println!("   Duration: {:?}", result.duration);
    println!();

    // Print issues
    if result.issues.is_empty() {
        println!("✓ No issues found");
    } else {
        println!("⚠  Issues ({}):", result.issues.len());
        for issue in &result.issues {
            let severity_icon = match issue.severity {
                codex_code_review::IssueSeverity::Critical => "🔴",
                codex_code_review::IssueSeverity::Error => "🟠",
                codex_code_review::IssueSeverity::Warning => "🟡",
                codex_code_review::IssueSeverity::Info => "🔵",
            };

            print!("   {} {:?}: {}", severity_icon, issue.category, issue.description);
            if let Some(line) = issue.line {
                print!(" (line {})", line);
            }
            println!();
        }
    }

    println!();

    // Print suggestions
    if result.suggestions.is_empty() {
        println!("✓ No suggestions");
    } else {
        println!("💡 Suggestions ({}):", result.suggestions.len());
        for suggestion in &result.suggestions {
            println!(
                "   • {} (confidence: {:.0}%)",
                suggestion.description,
                suggestion.confidence * 100.0
            );
            println!("     Rationale: {}", suggestion.rationale);
        }
    }

    println!();

    // Print metrics
    println!("📈 Metrics:");
    println!("   Lines of code: {}", result.metrics.loc);
    println!("   Complexity: {}", result.metrics.complexity);
    println!("   Functions: {}", result.metrics.num_functions);
    println!("   Comments: {}", result.metrics.num_comments);
    println!("   Doc coverage: {:.1}%", result.metrics.doc_coverage * 100.0);
}
