//! Plan file management utilities.
//!
//! Plan file naming follows Claude Code convention:
//! - Format: `{adjective}-{action}-{noun}.md` (e.g., "bright-exploring-aurora.md")
//! - Location: `~/.codex/plans/`
//! - Key: Uses **session-to-slug caching** - same session = same file always

use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use codex_protocol::ConversationId;
use dashmap::DashMap;
use rand::prelude::IndexedRandom;

use super::wordlist::ACTIONS;
use super::wordlist::ADJECTIVES;
use super::wordlist::NOUNS;
use crate::error::CodexErr;

// =============================================================================
// Plan Slug Caching (aligned with Claude Code chunks.88.mjs:790-803)
// =============================================================================

/// Global cache: conversation_id -> plan_slug
/// Same session ALWAYS gets the same plan file regardless of how many times /plan is called.
static PLAN_SLUG_CACHE: LazyLock<DashMap<ConversationId, String>> = LazyLock::new(DashMap::new);

/// Generate a random plan slug.
/// Format: `{adjective}-{action}-{noun}` (e.g., "bright-exploring-aurora")
///
/// Uses word lists from `wordlist.rs` (aligned with Claude Code).
/// Total combinations: ~8.5 million (220 adj × 110 act × 350 noun)
fn generate_random_plan_slug() -> String {
    let mut rng = rand::rng();
    let adj = ADJECTIVES.choose(&mut rng).unwrap_or(&"unknown");
    let act = ACTIONS.choose(&mut rng).unwrap_or(&"planning");
    let noun = NOUNS.choose(&mut rng).unwrap_or(&"task");
    format!("{adj}-{act}-{noun}")
}

/// Get or generate plan slug (CACHED per conversation_id).
///
/// Same session always returns the same slug, enabling:
/// 1. Re-entry detection (same file exists check)
/// 2. No path mismatch between TUI and Core
pub fn get_plan_slug(conversation_id: &ConversationId) -> String {
    if let Some(slug) = PLAN_SLUG_CACHE.get(conversation_id) {
        return slug.clone();
    }
    let slug = generate_random_plan_slug();
    PLAN_SLUG_CACHE.insert(*conversation_id, slug.clone());
    slug
}

/// Clean up slug cache entry for a conversation.
/// Called when session ends to free memory.
pub fn cleanup_plan_slug(conversation_id: &ConversationId) {
    PLAN_SLUG_CACHE.remove(conversation_id);
}

// =============================================================================
// Plan File Path Management
// =============================================================================

/// Get the plans directory path.
///
/// Returns ~/.codex/plans/, creating the directory if it doesn't exist.
pub fn get_plans_directory() -> Result<PathBuf, CodexErr> {
    let plans_dir = dirs::home_dir()
        .ok_or_else(|| CodexErr::Fatal("Unable to get home directory".to_string()))?
        .join(".codex")
        .join("plans");

    if !plans_dir.exists() {
        std::fs::create_dir_all(&plans_dir)
            .map_err(|e| CodexErr::Fatal(format!("Unable to create plans directory: {e}")))?;
    }

    Ok(plans_dir)
}

/// Get the full plan file path using cached slug.
///
/// # Arguments
/// * `conversation_id` - Conversation ID
///
/// # Returns
/// Full path, e.g., `~/.codex/plans/bright-exploring-aurora.md`
///
/// # Important
/// Same conversation_id ALWAYS returns the same path (slug is cached).
/// This enables proper re-entry detection.
pub fn get_plan_file_path(conversation_id: &ConversationId) -> Result<PathBuf, CodexErr> {
    let slug = get_plan_slug(conversation_id);
    Ok(get_plans_directory()?.join(format!("{slug}.md")))
}

/// Read plan file content.
///
/// # Arguments
/// * `path` - Plan file path
///
/// # Returns
/// File content, or None if file doesn't exist or read fails
pub fn read_plan_file(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(e) => {
            tracing::warn!("Failed to read plan file {}: {e}", path.display());
            None
        }
    }
}

/// Check if plan file exists.
pub fn plan_file_exists(path: &Path) -> bool {
    path.exists()
}
