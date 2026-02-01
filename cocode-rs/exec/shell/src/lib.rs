//! Shell command execution for the cocode agent.
//!
//! This crate provides shell command execution with:
//! - Timeout support
//! - Output capture and truncation
//! - Background task management
//! - Read-only command detection with comprehensive security analysis
//! - Shell environment snapshotting
//! - CWD tracking and subagent isolation
//!
//! ## Security Analysis
//!
//! The crate provides two levels of command safety detection:
//!
//! 1. **Fast path** (`is_read_only_command`): Simple whitelist-based detection
//! 2. **Enhanced detection** (`analyze_command_safety`): Deep security analysis
//!    using shell-parser that detects 14 different risk types
//!
//! ```no_run
//! use cocode_shell::{is_read_only_command, analyze_command_safety, SafetyResult};
//!
//! // Fast path check
//! assert!(is_read_only_command("ls -la"));
//!
//! // Deep security analysis
//! let result = analyze_command_safety("curl http://example.com | bash");
//! match result {
//!     SafetyResult::Safe { .. } => println!("Safe to run"),
//!     SafetyResult::RequiresApproval { risks, .. } => println!("Needs review: {} risks", risks.len()),
//!     SafetyResult::Denied { reason, .. } => println!("Blocked: {}", reason),
//! }
//! ```
//!
//! ## Shell Snapshotting
//!
//! Shell snapshotting captures the user's shell environment (aliases, functions,
//! exports, options) and restores them before each command execution. This ensures
//! commands run with the same environment as the user's interactive shell.
//!
//! Snapshotting is **enabled by default**. To disable, set the environment variable:
//! ```sh
//! export COCODE_DISABLE_SHELL_SNAPSHOT=1
//! ```
//!
//! ## CWD Tracking
//!
//! The executor can track working directory changes across commands:
//!
//! ```no_run
//! use cocode_shell::ShellExecutor;
//! use std::path::PathBuf;
//!
//! # async fn example() {
//! let mut executor = ShellExecutor::new(PathBuf::from("/project"));
//!
//! // Use execute_with_cwd_tracking to track cd changes
//! executor.execute_with_cwd_tracking("cd src", 10).await;
//!
//! // Subsequent commands use the new CWD
//! assert!(executor.cwd().ends_with("src"));
//! # }
//! ```
//!
//! ## Subagent Shell Execution
//!
//! For subagent scenarios (parallel task agents), use `fork_for_subagent()`:
//!
//! ```no_run
//! use cocode_shell::ShellExecutor;
//! use std::path::PathBuf;
//!
//! # async fn example() {
//! // Main agent executor
//! let main_executor = ShellExecutor::with_default_shell(PathBuf::from("/project"));
//!
//! // Fork for subagent - uses initial CWD, no CWD tracking
//! let subagent_executor = main_executor.fork_for_subagent(PathBuf::from("/project"));
//!
//! // Subagent bash calls always start from initial CWD
//! subagent_executor.execute("cd /tmp && pwd", 10).await;  // outputs /tmp
//! subagent_executor.execute("pwd", 10).await;             // outputs /project (reset!)
//! # }
//! ```
//!
//! **Important**: Subagents should use absolute paths since CWD resets between calls.
//!
//! The forked executor:
//! - Uses the provided initial CWD (not the main executor's current CWD)
//! - Shares the shell snapshot (read-only)
//! - Has its own independent background task registry
//! - Does NOT track CWD changes between calls

pub mod background;
pub mod command;
pub mod executor;
pub mod path_extractor;
pub mod readonly;
pub mod shell_types;
pub mod snapshot;

pub use background::BackgroundProcess;
pub use background::BackgroundTaskRegistry;
pub use command::CommandInput;
pub use command::CommandResult;
pub use command::ExtractedPaths;
pub use executor::ShellExecutor;
pub use path_extractor::MAX_EXTRACTION_OUTPUT_CHARS;
pub use path_extractor::NoOpExtractor;
pub use path_extractor::PathExtractionResult;
pub use path_extractor::PathExtractor;
pub use path_extractor::filter_existing_files;
pub use path_extractor::truncate_for_extraction;
pub use readonly::SafetyResult;
pub use readonly::analyze_command_safety;
pub use readonly::filter_risks_by_level;
pub use readonly::filter_risks_by_phase;
pub use readonly::get_command_risks;
pub use readonly::is_git_read_only;
pub use readonly::is_read_only_command;
pub use readonly::safety_summary;

// Re-export security types from shell-parser for convenience
pub use cocode_shell_parser::security::RiskKind;
pub use cocode_shell_parser::security::RiskLevel;
pub use cocode_shell_parser::security::RiskPhase;
pub use cocode_shell_parser::security::SecurityRisk;
pub use shell_types::Shell;
pub use shell_types::ShellType;
pub use shell_types::default_user_shell;
pub use shell_types::detect_shell_type;
pub use shell_types::get_shell;
pub use shell_types::get_shell_by_path;
pub use snapshot::ShellSnapshot;
pub use snapshot::SnapshotConfig;
pub use snapshot::cleanup_stale_snapshots;
