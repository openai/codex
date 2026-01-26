//! Retrieval CLI/TUI - Testing tool for the retrieval system.
//!
//! Provides both an interactive TUI (default) and CLI commands for testing
//! indexing and search capabilities.
//!
//! ## Usage
//!
//! ```bash
//! # TUI mode (default)
//! retrieval_cli [workdir]
//!
//! # CLI mode
//! retrieval_cli --no-tui search "query"
//! retrieval_cli --no-tui build --clean
//!
//! # Event stream mode (JSON-lines)
//! retrieval_cli --events search "query"
//! ```
//!
//! ## Logging
//!
//! Logs are written to `~/.codex/log/retrieval.log` by default.
//! Use `-v` flags to control verbosity in CLI mode.

use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use clap::Subcommand;
use tracing_appender::non_blocking::WorkerGuard;

use cocode_retrieval::EventConsumer;
use cocode_retrieval::FacadeBuilder;
use cocode_retrieval::JsonLinesConsumer;
use cocode_retrieval::LoggingConsumer;
use cocode_retrieval::RebuildMode;
use cocode_retrieval::RepoMapRequest;
use cocode_retrieval::RetrievalConfig;
use cocode_retrieval::RetrievalFacade;
use cocode_retrieval::SnippetStorage;
use cocode_retrieval::SqliteStore;
use cocode_retrieval::SymbolQuery;
use cocode_retrieval::event_emitter;
use cocode_retrieval::events::LogLevel;
use cocode_retrieval::indexing::IndexStatus;
use tokio_util::sync::CancellationToken;

/// Extract workspace name from a directory path.
fn workspace_name(workdir: &Path) -> &str {
    workdir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("default")
}

/// Create default features for BM25-only search.
fn bm25_features() -> cocode_retrieval::RetrievalFeatures {
    cocode_retrieval::RetrievalFeatures::STANDARD
}

/// Create features for hybrid search (BM25 + vector if available).
fn hybrid_features() -> cocode_retrieval::RetrievalFeatures {
    cocode_retrieval::RetrievalFeatures::FULL
}

#[derive(Parser)]
#[command(name = "retrieval")]
#[command(about = "Retrieval system TUI/CLI - search, index, and explore code")]
#[command(version)]
struct Cli {
    /// Working directory to index/search
    #[arg(default_value = ".")]
    workdir: PathBuf,

    /// Path to config file (default: {workdir}/.codex/retrieval.toml or ~/.codex/retrieval.toml)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Disable TUI and run in CLI mode
    #[arg(long)]
    no_tui: bool,

    /// Output structured events as JSON-lines (implies --no-tui)
    #[arg(long)]
    events: bool,

    /// Verbosity level (-v: info, -vv: debug, -vvv: trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Run a single CLI command (requires --no-tui or --events)
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Show index status
    Status,

    /// Build/rebuild the index
    Build {
        /// Clean all existing data before rebuilding
        #[arg(long)]
        clean: bool,
    },

    /// Watch for file changes and auto-index
    Watch,

    /// Hybrid search (BM25 + vector + snippet)
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: i32,
    },

    /// BM25 full-text search only
    Bm25 {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: i32,
    },

    /// Vector similarity search only
    Vector {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: i32,
    },

    /// Symbol-based search (functions, classes, etc.)
    Snippet {
        /// Search query (e.g., "fn:handle" or "type:struct name:Config")
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: i32,
    },

    /// Generate and display repo map
    Repomap {
        /// Maximum tokens in output
        #[arg(short = 't', long, default_value = "1024")]
        max_tokens: i32,

        /// Files to focus on (chat context files)
        #[arg(short, long)]
        focus: Vec<PathBuf>,
    },

    /// Show current configuration
    Config,

    /// Interactive REPL mode (legacy)
    Repl,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Determine mode
    let events_mode = cli.events;
    let cli_mode = cli.no_tui || events_mode || cli.command.is_some();

    // Canonicalize workdir early for config loading
    let workdir = cli.workdir.canonicalize().unwrap_or(cli.workdir.clone());

    // Load config from specified file or default locations
    // Config is loaded before tracing so we can use config.logging
    let config = if let Some(config_path) = &cli.config {
        // Use explicit config file - returns default if not found (with warning)
        RetrievalConfig::load_with_config_file(config_path)?
    } else {
        RetrievalConfig::load(&workdir)?
    };

    // Initialize tracing with config.logging (keep guard alive for logging)
    // Priority: RUST_LOG > CLI -v flags > config.logging.level > fallback "info"
    let _log_guard = init_tracing(&config.logging, cli.verbose);

    // Register LoggingConsumer to sync events with tracing logs
    event_emitter::EventEmitter::register_consumer(Arc::new(std::sync::RwLock::new(
        LoggingConsumer::new(LogLevel::Debug),
    )));

    // Log startup info
    tracing::info!(
        target: "codex_retrieval",
        cli_mode = cli_mode,
        events_mode = events_mode,
        "Retrieval CLI starting"
    );

    // Set up event consumer for --events mode
    let _event_guard = if events_mode {
        let writer: Box<dyn std::io::Write + Send + Sync> = Box::new(std::io::stdout());
        let consumer = JsonLinesConsumer::new(writer);
        Some(setup_event_consumer(consumer))
    } else {
        None
    };

    // Route to appropriate mode
    if cli_mode {
        // CLI mode
        if !config.enabled {
            print_not_enabled(&workdir, cli.config.as_ref());
            return Ok(());
        }

        // Show which config is being used
        if let Some(config_path) = &cli.config {
            eprintln!("Using config: {}", config_path.display());
        }

        match cli.command {
            Some(Command::Repl) => run_repl(&workdir, &config, cli.config.as_ref()).await?,
            Some(cmd) => run_command(cmd, &workdir, &config).await?,
            None => {
                // No command in CLI mode - show help
                eprintln!("No command specified. Use --help for available commands.");
                eprintln!("Or run without --no-tui to launch the interactive TUI.");
            }
        }
    } else {
        // TUI mode (default)
        // Update config with workdir for TUI
        let mut tui_config = config.clone();
        tui_config.workdir = Some(workdir.clone());

        // Create service if retrieval is enabled - use with_workspace for operations
        let (service, not_enabled_reason) = if config.enabled {
            match FacadeBuilder::new(config)
                .features(hybrid_features())
                .workspace(workdir.clone())
                .build()
                .await
            {
                Ok(svc) => (Some(Arc::new(svc)), None),
                Err(e) => {
                    tracing::warn!("Could not initialize retrieval service: {}", e);
                    (None, Some(format!("Service error: {}", e)))
                }
            }
        } else {
            (
                None,
                Some(get_not_enabled_message(&workdir, cli.config.as_ref())),
            )
        };

        // Set error banner in TUI if not enabled
        let mut app = cocode_retrieval::tui::App::new(tui_config, service);
        if let Some(reason) = not_enabled_reason {
            app.error_banner = Some(reason);
        }

        // Initialize terminal and run
        let mut terminal = cocode_retrieval::tui::init_terminal()?;
        let result = app.run(&mut terminal).await;
        cocode_retrieval::tui::restore_terminal()?;
        result?;
    }

    // Explicitly drop the log guard to flush remaining logs
    drop(_log_guard);

    Ok(())
}

/// Get user-friendly message when retrieval is not enabled.
fn get_not_enabled_message(workdir: &Path, config_path: Option<&PathBuf>) -> String {
    if config_path.is_some() {
        "Retrieval not enabled. Set 'enabled = true' in your config file.".to_string()
    } else {
        format!(
            "Retrieval not enabled. Create config at {}/.codex/retrieval.toml or ~/.codex/retrieval.toml",
            workdir.display()
        )
    }
}

/// Initialize tracing with file-based logging.
///
/// Logs are written to `~/.codex/log/retrieval.log`.
/// Returns a guard that must be kept alive for the duration of logging.
///
/// Priority chain for log level:
/// 1. RUST_LOG env var (highest, handled by configure_fmt_layer!)
/// 2. CLI -v flags (if verbose > 0, overrides config)
/// 3. config.logging.level (from retrieval.toml)
/// 4. fallback "codex_retrieval=info" (lowest)
fn init_tracing(
    config_logging: &cocode_utils_common::LoggingConfig,
    verbose: u8,
) -> Option<WorkerGuard> {
    use tracing_appender::non_blocking;
    use tracing_subscriber::prelude::*;

    // Determine log directory: codex_home/log/ (respects CODEX_HOME)
    let log_dir = cocode_retrieval::config::find_codex_home()
        .map(|h| h.join("log"))
        .unwrap_or_else(|| PathBuf::from(".codex/log"));

    // Create log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Could not create log directory: {e}");
        return init_tracing_stderr(config_logging, verbose);
    }

    // Open log file with append mode
    let log_path = log_dir.join("retrieval.log");
    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_file = match log_file_opts.open(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Could not open log file: {e}");
            return init_tracing_stderr(config_logging, verbose);
        }
    };

    let (non_blocking, guard) = non_blocking(log_file);

    // CLI -v flags override config level when specified
    let logging_config = if verbose > 0 {
        let level = match verbose {
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        cocode_utils_common::LoggingConfig::with_level(level)
    } else {
        config_logging.clone()
    };

    // Use cocode-utils logging infrastructure for timezone-aware timestamps
    let file_layer = cocode_utils_common::configure_fmt_layer!(
        tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false),
        &logging_config,
        "cocode_retrieval=info"
    );

    let _ = tracing_subscriber::registry().with(file_layer).try_init();

    Some(guard)
}

/// Fallback: stderr-only logging when file logging fails.
fn init_tracing_stderr(
    config_logging: &cocode_utils_common::LoggingConfig,
    verbose: u8,
) -> Option<WorkerGuard> {
    use tracing_subscriber::prelude::*;

    // CLI -v flags override config level when specified
    let logging_config = if verbose > 0 {
        let level = match verbose {
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        cocode_utils_common::LoggingConfig::with_level(level)
    } else {
        config_logging.clone()
    };

    // Use cocode-utils logging infrastructure for consistent formatting
    let stderr_layer = cocode_utils_common::configure_fmt_layer!(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true),
        &logging_config,
        "cocode_retrieval=info"
    );

    let _ = tracing_subscriber::registry().with(stderr_layer).try_init();

    None
}

fn setup_event_consumer(
    consumer: JsonLinesConsumer<Box<dyn std::io::Write + Send + Sync>>,
) -> impl Drop {
    // Subscribe to events and forward to consumer
    let mut rx = event_emitter::subscribe();
    let consumer = Arc::new(std::sync::Mutex::new(consumer));
    let consumer_clone = consumer.clone();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(mut c) = consumer_clone.lock() {
                c.on_event(&event);
            }
        }
    });

    // Return guard that flushes on drop
    struct Guard(Arc<std::sync::Mutex<JsonLinesConsumer<Box<dyn std::io::Write + Send + Sync>>>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Ok(mut c) = self.0.lock() {
                c.flush();
            }
        }
    }
    Guard(consumer)
}

fn print_not_enabled(workdir: &Path, config_path: Option<&PathBuf>) {
    println!("Retrieval is not enabled.");
    if config_path.is_some() {
        println!("Set 'enabled = true' in your config file.");
    } else {
        println!(
            "Create a config file at: {}/.codex/retrieval.toml",
            workdir.display()
        );
        println!("\nExample config:");
        println!("[retrieval]");
        println!("enabled = true");
    }
}

async fn run_command(
    cmd: Command,
    workdir: &PathBuf,
    config: &RetrievalConfig,
) -> anyhow::Result<()> {
    // Create service with workdir set for operations commands
    let mut service_config = config.clone();
    service_config.workdir = Some(workdir.clone());

    // Commands that need service - use with_workspace to set workspace_root
    let service = match &cmd {
        Command::Status | Command::Build { .. } | Command::Watch | Command::Repomap { .. } => {
            Some(Arc::new(
                FacadeBuilder::new(service_config.clone())
                    .features(hybrid_features())
                    .workspace(workdir.clone())
                    .build()
                    .await?,
            ))
        }
        _ => None,
    };

    match cmd {
        Command::Status => cmd_status(service.unwrap()).await?,
        Command::Build { clean } => cmd_build(service.unwrap(), clean).await?,
        Command::Watch => cmd_watch(service.unwrap()).await?,
        Command::Search { query, limit } => cmd_search(config, &query, limit).await?,
        Command::Bm25 { query, limit } => cmd_bm25(config, &query, limit).await?,
        Command::Vector { query, limit } => cmd_vector(config, &query, limit).await?,
        Command::Snippet { query, limit } => cmd_snippet(workdir, config, &query, limit).await?,
        Command::Repomap { max_tokens, focus } => {
            cmd_repomap(service.unwrap(), max_tokens, &focus).await?
        }
        Command::Config => cmd_config(config)?,
        Command::Repl => unreachable!(), // Handled in main
    }
    Ok(())
}

async fn run_repl(
    workdir: &PathBuf,
    config: &RetrievalConfig,
    config_path: Option<&PathBuf>,
) -> anyhow::Result<()> {
    println!("Retrieval CLI v0.1");
    if let Some(path) = config_path {
        println!("Config: {}", path.display());
    } else {
        println!(
            "Config: {}/.codex/retrieval.toml (or ~/.codex/retrieval.toml)",
            workdir.display()
        );
    }
    println!("Data: {}", config.data_dir.display());
    println!(
        "\nCommands: status, build [--clean], watch, search <query>, bm25 <query>, vector <query>, snippet <query>, repomap, config, quit"
    );
    println!();

    // Create service for operations commands - use with_workspace for build/watch support
    let mut service_config = config.clone();
    service_config.workdir = Some(workdir.clone());
    let service = Arc::new(
        FacadeBuilder::new(service_config)
            .features(hybrid_features())
            .workspace(workdir.clone())
            .build()
            .await?,
    );

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts.first().unwrap_or(&"");

        let result = match *cmd {
            "quit" | "exit" | "q" => break,
            "status" => cmd_status(Arc::clone(&service)).await,
            "build" => {
                let clean = parts.get(1).map(|s| *s == "--clean").unwrap_or(false);
                cmd_build(Arc::clone(&service), clean).await
            }
            "watch" => cmd_watch(Arc::clone(&service)).await,
            "search" => {
                let query = parts[1..].join(" ");
                if query.is_empty() {
                    println!("Usage: search <query>");
                    continue;
                }
                cmd_search(config, &query, 10).await
            }
            "bm25" => {
                let query = parts[1..].join(" ");
                if query.is_empty() {
                    println!("Usage: bm25 <query>");
                    continue;
                }
                cmd_bm25(config, &query, 10).await
            }
            "vector" => {
                let query = parts[1..].join(" ");
                if query.is_empty() {
                    println!("Usage: vector <query>");
                    continue;
                }
                cmd_vector(config, &query, 10).await
            }
            "snippet" => {
                let query = parts[1..].join(" ");
                if query.is_empty() {
                    println!("Usage: snippet <query>");
                    continue;
                }
                cmd_snippet(workdir, config, &query, 10).await
            }
            "repomap" => cmd_repomap(Arc::clone(&service), 1024, &[]).await,
            "config" => cmd_config(config),
            "help" | "?" => {
                println!("Commands:");
                println!("  status         - Show index status");
                println!("  build [--clean] - Build index (--clean for full rebuild)");
                println!("  watch          - Watch for file changes");
                println!("  search <query> - Hybrid search");
                println!("  bm25 <query>   - BM25 full-text search");
                println!("  vector <query> - Vector similarity search");
                println!("  snippet <query> - Symbol-based search");
                println!("  repomap        - Generate repo map");
                println!("  config         - Show configuration");
                println!("  quit           - Exit");
                continue;
            }
            _ => {
                println!(
                    "Unknown command: {}. Type 'help' for available commands.",
                    cmd
                );
                continue;
            }
        };

        if let Err(e) = result {
            println!("Error: {e}");
        }
    }

    Ok(())
}

async fn cmd_status(service: Arc<RetrievalFacade>) -> anyhow::Result<()> {
    let stats = service.index_service().get_status().await?;

    if stats.file_count == 0 && stats.last_indexed.is_none() {
        println!("Index not found. Run 'build' to create it.");
        return Ok(());
    }

    println!("Files indexed: {}", stats.file_count);
    println!("Total chunks: {}", stats.chunk_count);
    if let Some(ts) = stats.last_indexed {
        let dt = chrono::DateTime::from_timestamp(ts, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("Last indexed: {}", dt);
    } else {
        println!("Last indexed: never");
    }

    Ok(())
}

async fn cmd_build(service: Arc<RetrievalFacade>, clean: bool) -> anyhow::Result<()> {
    let mode = if clean {
        println!("[Clean] Deleting old index...");
        RebuildMode::Clean
    } else {
        println!("[Incremental] Scanning for changes...");
        RebuildMode::Incremental
    };

    let cancel = CancellationToken::new();
    let mut rx = service.build_index(mode, cancel).await?;

    // Process progress updates
    while let Some(progress) = rx.recv().await {
        match progress.status {
            IndexStatus::Loading => {
                println!("{}", progress.description);
            }
            IndexStatus::Indexing => {
                let pct = (progress.progress * 100.0) as i32;
                println!("[{:3}%] {}", pct, progress.description);
            }
            IndexStatus::Done => {
                println!("Done: {}", progress.description);
            }
            IndexStatus::Failed => {
                println!("Failed: {}", progress.description);
            }
            _ => {}
        }
    }

    Ok(())
}

async fn cmd_watch(service: Arc<RetrievalFacade>) -> anyhow::Result<()> {
    println!("[Watch] Watching for changes (Ctrl+C to stop)...");

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Set up signal handler for graceful shutdown
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            cancel_clone.cancel();
        }
    });

    let mut rx = service.index_service().start_watch(cancel.clone()).await?;

    // Process watch events from service
    while let Some(event) = rx.recv().await {
        // Check file existence to determine actual change type
        let kind = if event.path.exists() {
            "changed"
        } else {
            "deleted"
        };
        println!("[Change] {} {}", event.path.display(), kind);
    }

    println!("\n[Watch] Stopped watching.");
    Ok(())
}

async fn cmd_search(config: &RetrievalConfig, query: &str, limit: i32) -> anyhow::Result<()> {
    use cocode_retrieval::SearchRequest;

    let service = FacadeBuilder::new(config.clone())
        .features(hybrid_features())
        .build()
        .await?;
    let request = SearchRequest::new(query).hybrid().limit(limit);
    let output = service.search_service().execute(request).await?;

    // Display filter info if configured
    if let Some(filter) = &output.filter {
        println!("[Index Filter: {}]\n", filter.to_display_string());
    }

    println!("[Hybrid] Found {} results:\n", output.results.len());

    for (i, result) in output.results.iter().enumerate() {
        println!(
            "{}. {}:{}-{} (score: {:.3}, type: {:?})",
            i + 1,
            result.chunk.filepath,
            result.chunk.start_line,
            result.chunk.end_line,
            result.score,
            result.score_type
        );
        // Show first 2 lines of content
        let lines: Vec<&str> = result.chunk.content.lines().take(2).collect();
        for line in lines {
            println!("   {}", line.trim());
        }
        println!();
    }

    Ok(())
}

async fn cmd_bm25(config: &RetrievalConfig, query: &str, limit: i32) -> anyhow::Result<()> {
    use cocode_retrieval::SearchRequest;

    let service = FacadeBuilder::new(config.clone())
        .features(bm25_features())
        .build()
        .await?;
    let request = SearchRequest::new(query).bm25().limit(limit);
    let output = service.search_service().execute(request).await?;

    // Display filter info if configured
    if let Some(filter) = &output.filter {
        println!("[Index Filter: {}]\n", filter.to_display_string());
    }

    println!("[BM25] Found {} results:\n", output.results.len());

    for (i, result) in output.results.iter().enumerate() {
        println!(
            "{}. {}:{}-{} (score: {:.3})",
            i + 1,
            result.chunk.filepath,
            result.chunk.start_line,
            result.chunk.end_line,
            result.score
        );
        let lines: Vec<&str> = result.chunk.content.lines().take(2).collect();
        for line in lines {
            println!("   {}", line.trim());
        }
        println!();
    }

    Ok(())
}

async fn cmd_vector(config: &RetrievalConfig, query: &str, limit: i32) -> anyhow::Result<()> {
    use cocode_retrieval::SearchRequest;

    let service = FacadeBuilder::new(config.clone())
        .features(hybrid_features())
        .build()
        .await?;

    if !service.search_service().has_vector_search() {
        println!("[Vector] Vector search not available (embeddings not configured)");
        return Ok(());
    }

    let request = SearchRequest::new(query).vector().limit(limit);
    let output = service.search_service().execute(request).await?;

    // Display filter info if configured
    if let Some(filter) = &output.filter {
        println!("[Index Filter: {}]\n", filter.to_display_string());
    }

    println!("[Vector] Found {} results:\n", output.results.len());

    for (i, result) in output.results.iter().enumerate() {
        println!(
            "{}. {}:{}-{} (score: {:.3})",
            i + 1,
            result.chunk.filepath,
            result.chunk.start_line,
            result.chunk.end_line,
            result.score
        );
        let lines: Vec<&str> = result.chunk.content.lines().take(2).collect();
        for line in lines {
            println!("   {}", line.trim());
        }
        println!();
    }

    Ok(())
}

async fn cmd_snippet(
    workdir: &PathBuf,
    config: &RetrievalConfig,
    query: &str,
    limit: i32,
) -> anyhow::Result<()> {
    let db_path = config.data_dir.join("retrieval.db");

    if !db_path.exists() {
        println!("[Snippet] Index not found. Run 'build' first.");
        return Ok(());
    }

    let store = Arc::new(SqliteStore::open(&db_path)?);
    let snippet_store = SnippetStorage::new(store);

    let workspace = workspace_name(workdir);

    // Parse symbol query (e.g., "type:function name:handle")
    let symbol_query = SymbolQuery::parse(query);

    let results = snippet_store
        .search_fts(workspace, &symbol_query, limit)
        .await?;

    println!("[Snippet] Found {} symbols:\n", results.len());

    for (i, snippet) in results.iter().enumerate() {
        println!(
            "{}. {} {} ({}:{}-{})",
            i + 1,
            snippet.syntax_type,
            snippet.name,
            snippet.filepath,
            snippet.start_line,
            snippet.end_line
        );
        if let Some(sig) = &snippet.signature {
            println!("   {}", sig);
        }
    }

    Ok(())
}

async fn cmd_repomap(
    service: Arc<RetrievalFacade>,
    max_tokens: i32,
    focus_files: &[PathBuf],
) -> anyhow::Result<()> {
    let request = RepoMapRequest {
        chat_files: focus_files.to_vec(),
        max_tokens,
        ..Default::default()
    };

    println!("[RepoMap] Generating with max {} tokens...\n", max_tokens);

    let result = service.generate_repomap(request).await?;

    println!(
        "=== Repo Map ({} tokens, {} files) ===\n",
        result.tokens, result.files_included
    );
    println!("{}", result.content);

    println!("\n[RepoMap] Generated in {}ms", result.generation_time_ms);

    Ok(())
}

fn cmd_config(config: &RetrievalConfig) -> anyhow::Result<()> {
    println!("Configuration:");
    println!("  enabled: {}", config.enabled);
    println!("  data_dir: {}", config.data_dir.display());
    println!();
    println!("Indexing:");
    println!("  max_file_size_mb: {}", config.indexing.max_file_size_mb);
    println!("  batch_size: {}", config.indexing.batch_size);
    println!("  watch_enabled: {}", config.indexing.watch_enabled);
    println!("  watch_debounce_ms: {}", config.indexing.watch_debounce_ms);
    println!();
    println!("File Filtering:");
    if config.indexing.include_dirs.is_empty()
        && config.indexing.exclude_dirs.is_empty()
        && config.indexing.include_extensions.is_empty()
        && config.indexing.exclude_extensions.is_empty()
    {
        println!("  (default: all directories, standard text file extensions)");
    } else {
        if !config.indexing.include_dirs.is_empty() {
            println!("  include_dirs: {:?}", config.indexing.include_dirs);
        }
        if !config.indexing.exclude_dirs.is_empty() {
            println!("  exclude_dirs: {:?}", config.indexing.exclude_dirs);
        }
        if !config.indexing.include_extensions.is_empty() {
            println!(
                "  include_extensions: {:?}",
                config.indexing.include_extensions
            );
        }
        if !config.indexing.exclude_extensions.is_empty() {
            println!(
                "  exclude_extensions: {:?}",
                config.indexing.exclude_extensions
            );
        }
    }
    println!();
    println!("Chunking:");
    println!("  max_tokens: {}", config.chunking.max_tokens);
    println!("  overlap_tokens: {}", config.chunking.overlap_tokens);
    println!();
    println!("Search:");
    println!("  n_final: {}", config.search.n_final);
    println!("  bm25_weight: {}", config.search.bm25_weight);
    println!("  vector_weight: {}", config.search.vector_weight);
    println!("  snippet_weight: {}", config.search.snippet_weight);
    println!();
    println!(
        "Embedding: {}",
        if config.embedding.is_some() {
            "configured"
        } else {
            "not configured"
        }
    );
    println!(
        "Query Rewrite: {}",
        if config.query_rewrite.is_some() {
            "configured"
        } else {
            "not configured"
        }
    );
    println!();
    println!("RepoMap:");
    if let Some(ref repo_map) = config.repo_map {
        println!("  enabled: {}", repo_map.enabled);
        println!("  map_tokens: {}", repo_map.map_tokens);
        println!("  max_iterations: {}", repo_map.max_iterations);
    } else {
        println!("  not configured");
    }

    Ok(())
}
