//! Extension state management for compact_v2.
//!
//! This module provides session-level state storage for compact operations
//! using a global map keyed by conversation_id. This follows the extension
//! pattern to minimize modifications to existing session.rs.

use std::sync::OnceLock;

use codex_protocol::ConversationId;
use dashmap::DashMap;

use crate::compact_v2::CompactState;
use crate::compact_v2::ReadFileEntry;

/// Global storage for CompactState keyed by conversation_id.
static COMPACT_STATES: OnceLock<DashMap<ConversationId, CompactState>> = OnceLock::new();

/// Global storage for ReadFileState keyed by conversation_id.
static READ_FILE_STATES: OnceLock<DashMap<ConversationId, ReadFileState>> = OnceLock::new();

/// Session-level read file state for context restoration.
///
/// Tracks recently read files with their timestamps and token counts
/// for restoration after compaction.
#[derive(Debug, Clone, Default)]
pub struct ReadFileState {
    /// Recently read files, sorted by timestamp (most recent last)
    pub files: Vec<ReadFileEntry>,
}

/// Get the global CompactState storage.
fn get_compact_states() -> &'static DashMap<ConversationId, CompactState> {
    COMPACT_STATES.get_or_init(DashMap::new)
}

/// Get the global ReadFileState storage.
fn get_read_file_states() -> &'static DashMap<ConversationId, ReadFileState> {
    READ_FILE_STATES.get_or_init(DashMap::new)
}

/// Get or create CompactState for a conversation.
///
/// Returns a mutable reference guard that can be used to modify the state.
pub fn get_compact_state_mut(
    conversation_id: ConversationId,
) -> dashmap::mapref::one::RefMut<'static, ConversationId, CompactState> {
    let states = get_compact_states();
    states.entry(conversation_id).or_default()
}

/// Get CompactState for a conversation (read-only).
///
/// Returns None if no state exists for this conversation.
#[allow(dead_code)] // Reserved for state management
pub fn get_compact_state(
    conversation_id: ConversationId,
) -> Option<dashmap::mapref::one::Ref<'static, ConversationId, CompactState>> {
    let states = get_compact_states();
    states.get(&conversation_id)
}

/// Clear CompactState for a conversation (for testing or session cleanup).
#[allow(dead_code)]
pub fn clear_compact_state(conversation_id: ConversationId) {
    let states = get_compact_states();
    states.remove(&conversation_id);
}

/// Record a file read for context restoration.
///
/// Updates the read file state for the given conversation, replacing
/// any existing entry for the same filename with updated timestamp/tokens.
pub fn record_file_read(
    conversation_id: ConversationId,
    filename: String,
    timestamp: i64,
    token_count: i64,
) {
    let states = get_read_file_states();
    let mut state = states.entry(conversation_id).or_default();

    // Remove existing entry for this file (if any)
    state.files.retain(|f| f.filename != filename);

    // Add new entry
    state.files.push(ReadFileEntry {
        filename,
        timestamp,
        token_count,
    });
}

/// Get read files for a conversation (for context restoration).
///
/// Returns files sorted by timestamp (most recent first).
pub fn get_read_files(conversation_id: ConversationId) -> Vec<ReadFileEntry> {
    let states = get_read_file_states();
    match states.get(&conversation_id) {
        Some(state) => {
            let mut files = state.files.clone();
            // Sort by timestamp descending (most recent first)
            files.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            files
        }
        None => Vec::new(),
    }
}

/// Clear ReadFileState for a conversation (for testing or session cleanup).
#[allow(dead_code)]
pub fn clear_read_file_state(conversation_id: ConversationId) {
    let states = get_read_file_states();
    states.remove(&conversation_id);
}

/// Clear all state for a conversation (both CompactState and ReadFileState).
#[allow(dead_code)]
pub fn clear_all_state(conversation_id: ConversationId) {
    clear_compact_state(conversation_id);
    clear_read_file_state(conversation_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn compact_state_persistence() {
        let conv_id = ConversationId::new();

        // Initially empty
        assert!(get_compact_state(conv_id).is_none());

        // Create and modify state
        {
            let mut state = get_compact_state_mut(conv_id);
            state.compacted_tool_ids.insert("tool-1".to_string());
            state.tool_token_cache.insert("tool-1".to_string(), 1000);
        }

        // Verify persistence
        {
            let state = get_compact_state(conv_id).unwrap();
            assert!(state.compacted_tool_ids.contains("tool-1"));
            assert_eq!(state.tool_token_cache.get("tool-1"), Some(&1000));
        }

        // Cleanup
        clear_compact_state(conv_id);
        assert!(get_compact_state(conv_id).is_none());
    }

    #[test]
    fn read_file_state_recording() {
        let conv_id = ConversationId::new();

        // Record some files
        record_file_read(conv_id, "file1.rs".to_string(), 100, 500);
        record_file_read(conv_id, "file2.rs".to_string(), 200, 300);
        record_file_read(conv_id, "file3.rs".to_string(), 150, 400);

        // Get files (should be sorted by timestamp descending)
        let files = get_read_files(conv_id);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].filename, "file2.rs"); // timestamp 200
        assert_eq!(files[1].filename, "file3.rs"); // timestamp 150
        assert_eq!(files[2].filename, "file1.rs"); // timestamp 100

        // Cleanup
        clear_read_file_state(conv_id);
        assert!(get_read_files(conv_id).is_empty());
    }

    #[test]
    fn read_file_state_updates_existing() {
        let conv_id = ConversationId::new();

        // Record file
        record_file_read(conv_id, "file.rs".to_string(), 100, 500);

        // Re-read same file with new timestamp
        record_file_read(conv_id, "file.rs".to_string(), 200, 600);

        // Should only have one entry with updated values
        let files = get_read_files(conv_id);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].timestamp, 200);
        assert_eq!(files[0].token_count, 600);

        // Cleanup
        clear_read_file_state(conv_id);
    }

    #[test]
    fn clear_all_state_works() {
        let conv_id = ConversationId::new();

        // Setup both states
        {
            let mut state = get_compact_state_mut(conv_id);
            state.compacted_tool_ids.insert("tool-1".to_string());
        }
        record_file_read(conv_id, "file.rs".to_string(), 100, 500);

        // Verify both exist
        assert!(get_compact_state(conv_id).is_some());
        assert!(!get_read_files(conv_id).is_empty());

        // Clear all
        clear_all_state(conv_id);

        // Verify both cleared
        assert!(get_compact_state(conv_id).is_none());
        assert!(get_read_files(conv_id).is_empty());
    }
}
