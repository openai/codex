//! TUI constants and configuration values.
//!
//! Centralizes magic numbers for easier maintenance and documentation.

/// Search debounce interval in milliseconds.
/// Prevents rapid-fire searches when user types quickly.
pub const SEARCH_DEBOUNCE_MS: u64 = 200;

/// Search timeout in seconds.
/// Cancels long-running searches to prevent UI blocking.
pub const SEARCH_TIMEOUT_SECS: u64 = 30;

/// Maximum number of history entries to keep.
pub const MAX_HISTORY_ENTRIES: usize = 50;

/// Maximum number of events to keep in the event log.
pub const MAX_EVENT_LOG_ENTRIES: usize = 100;

/// UI tick interval in milliseconds.
/// Controls animation and update refresh rate.
pub const TICK_INTERVAL_MS: u64 = 100;

/// Circuit breaker failure threshold.
/// Number of consecutive failures before opening the circuit.
pub const CIRCUIT_BREAKER_THRESHOLD: i32 = 5;

/// Circuit breaker reset timeout in seconds.
/// Time to wait before attempting recovery after circuit opens.
pub const CIRCUIT_BREAKER_RESET_SECS: i32 = 60;

/// Default token budget for RepoMap generation.
pub const DEFAULT_REPOMAP_TOKENS: i32 = 1024;

/// Minimum token budget for RepoMap.
pub const MIN_REPOMAP_TOKENS: i32 = 256;

/// Maximum token budget for RepoMap.
pub const MAX_REPOMAP_TOKENS: i32 = 8192;

/// Token budget adjustment step for RepoMap.
pub const REPOMAP_TOKEN_STEP: i32 = 256;

/// Help overlay width in characters.
pub const HELP_OVERLAY_WIDTH: u16 = 42;

/// Help overlay height in lines.
pub const HELP_OVERLAY_HEIGHT: u16 = 50;

/// Page navigation step (number of items to move).
pub const PAGE_SCROLL_STEP: usize = 10;
