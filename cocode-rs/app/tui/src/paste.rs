//! Paste management for TUI.
//!
//! This module handles large text and image paste operations,
//! displaying compact pills in the input field and resolving
//! them to full content on submit.
//!
//! ## Features
//!
//! - Text paste with pill display: `[Pasted text #1 +421 lines]`
//! - Image paste with pill display: `[Image #1]`
//! - Two-tier storage: inline for small content, disk cache for large
//! - Content resolution on submit
//!
//! ## Storage Strategy
//!
//! - Content ≤ 1KB: stored inline in memory
//! - Content > 1KB: stored on disk at `~/.cocode/paste-cache/`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

use hyper_sdk::ContentBlock;
use hyper_sdk::ImageSource;
use regex::Regex;

/// Threshold for inline vs disk storage (1KB).
const INLINE_THRESHOLD_BYTES: usize = 1024;

/// Maximum number of inline cache entries before LRU eviction.
const MAX_INLINE_ENTRIES: usize = 100;

/// Type of pasted content.
#[derive(Debug, Clone)]
pub enum PasteKind {
    /// Plain text content.
    Text,
    /// Image content with format.
    Image {
        /// MIME type (e.g., "image/png", "image/jpeg").
        media_type: String,
    },
}

/// Storage location for pasted content.
#[derive(Debug, Clone)]
pub enum PasteStorage {
    /// Content stored inline in memory.
    Inline(String),
    /// Content stored on disk at the given path.
    Disk(PathBuf),
}

/// A single paste entry.
#[derive(Debug, Clone)]
pub struct PasteEntry {
    /// Unique ID for this paste.
    pub id: i32,
    /// Type of content.
    pub kind: PasteKind,
    /// Where the content is stored.
    pub storage: PasteStorage,
    /// Number of lines in the content (for text).
    pub line_count: i32,
    /// The pill text to display in input.
    pub pill: String,
}

/// Manager for paste operations.
///
/// Tracks pasted content and resolves pills to actual content
/// when the user submits their input.
pub struct PasteManager {
    /// Next ID to assign.
    next_id: AtomicI32,
    /// All paste entries by ID.
    entries: HashMap<i32, PasteEntry>,
    /// Inline content cache (small content only).
    inline_cache: HashMap<i32, Vec<u8>>,
    /// Order of inline entries for LRU eviction.
    inline_order: Vec<i32>,
    /// Directory for disk cache.
    cache_dir: PathBuf,
    /// Regex for matching pill patterns.
    pill_regex: Regex,
}

impl Default for PasteManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteManager {
    /// Create a new paste manager.
    ///
    /// Uses `~/.cocode/paste-cache/` for disk storage.
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cocode")
            .join("paste-cache");

        Self::with_cache_dir(cache_dir)
    }

    /// Create a new paste manager with a custom cache directory.
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        // Create cache directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!(error = %e, "Failed to create paste cache directory");
        }

        // Pattern matches: [Pasted text #1], [Pasted text #1 +421 lines], [Image #1]
        let pill_regex =
            Regex::new(r"\[(Pasted text|Image|\.\.\.Truncated text) #(\d+)(?: \+(\d+) lines?)?\]")
                .expect("Invalid pill regex");

        Self {
            next_id: AtomicI32::new(1),
            entries: HashMap::new(),
            inline_cache: HashMap::new(),
            inline_order: Vec::new(),
            cache_dir,
            pill_regex,
        }
    }

    /// Process pasted text content.
    ///
    /// For small content (≤1KB), returns the original text.
    /// For large content, stores it and returns a pill.
    pub fn process_text(&mut self, text: String) -> String {
        let bytes = text.as_bytes();

        // Small content: return as-is (no pill needed)
        if bytes.len() <= INLINE_THRESHOLD_BYTES {
            return text;
        }

        // Large content: create pill
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let line_count = count_lines(&text);
        let pill = generate_pill(id, &PasteKind::Text, line_count);

        // Store content
        let storage = if bytes.len() <= INLINE_THRESHOLD_BYTES * 4 {
            // Medium content: store inline
            self.store_inline(id, bytes.to_vec());
            PasteStorage::Inline(text.clone())
        } else {
            // Large content: store on disk
            match self.store_on_disk(id, bytes) {
                Ok(path) => PasteStorage::Disk(path),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to store paste on disk, using inline");
                    self.store_inline(id, bytes.to_vec());
                    PasteStorage::Inline(text.clone())
                }
            }
        };

        let entry = PasteEntry {
            id,
            kind: PasteKind::Text,
            storage,
            line_count,
            pill: pill.clone(),
        };

        self.entries.insert(id, entry);
        pill
    }

    /// Process pasted image content.
    ///
    /// Stores the image and returns a pill.
    pub fn process_image(&mut self, data: Vec<u8>, media_type: String) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let pill = generate_pill(
            id,
            &PasteKind::Image {
                media_type: media_type.clone(),
            },
            0,
        );

        // Store image data
        let storage = if data.len() <= INLINE_THRESHOLD_BYTES * 4 {
            self.store_inline(id, data);
            // For inline storage, we need to track the data differently
            PasteStorage::Inline(String::new()) // Empty string marker for images
        } else {
            match self.store_on_disk(id, &data) {
                Ok(path) => PasteStorage::Disk(path),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to store image on disk, using inline");
                    self.store_inline(id, data);
                    PasteStorage::Inline(String::new())
                }
            }
        };

        let entry = PasteEntry {
            id,
            kind: PasteKind::Image { media_type },
            storage,
            line_count: 0,
            pill: pill.clone(),
        };

        self.entries.insert(id, entry);
        pill
    }

    /// Resolve paste pills in the input text to content blocks.
    ///
    /// Returns a vector of content blocks suitable for sending to the API.
    /// Text between pills becomes text blocks, pills become their resolved content.
    pub fn resolve_to_blocks(&self, text: &str) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();
        let mut last_end = 0;

        for captures in self.pill_regex.captures_iter(text) {
            let full_match = captures.get(0).expect("Full match should exist");

            // Add text before this pill
            if full_match.start() > last_end {
                let text_before = &text[last_end..full_match.start()];
                if !text_before.is_empty() {
                    blocks.push(ContentBlock::text(text_before));
                }
            }

            // Resolve pill to content
            if let Some(id_match) = captures.get(2) {
                if let Ok(id) = id_match.as_str().parse::<i32>() {
                    if let Some(entry) = self.entries.get(&id) {
                        match &entry.kind {
                            PasteKind::Text => {
                                if let Some(content) = self.get_text_content(entry) {
                                    blocks.push(ContentBlock::text(content));
                                }
                            }
                            PasteKind::Image { media_type } => {
                                if let Some(data) = self.get_image_data(entry) {
                                    let base64_data = base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        &data,
                                    );
                                    blocks.push(ContentBlock::Image {
                                        source: ImageSource::Base64 {
                                            data: base64_data,
                                            media_type: media_type.clone(),
                                        },
                                        detail: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            last_end = full_match.end();
        }

        // Add remaining text after last pill
        if last_end < text.len() {
            let remaining = &text[last_end..];
            if !remaining.is_empty() {
                blocks.push(ContentBlock::text(remaining));
            }
        }

        // If no pills found, just return the original text
        if blocks.is_empty() && !text.is_empty() {
            blocks.push(ContentBlock::text(text));
        }

        blocks
    }

    /// Resolve paste pills to plain text (for display/history).
    ///
    /// Returns the text with pills replaced by their actual content.
    pub fn resolve_pills(&self, text: &str) -> String {
        let mut result = String::new();
        let mut last_end = 0;

        for captures in self.pill_regex.captures_iter(text) {
            let full_match = captures.get(0).expect("Full match should exist");

            // Add text before this pill
            result.push_str(&text[last_end..full_match.start()]);

            // Resolve pill to content
            if let Some(id_match) = captures.get(2) {
                if let Ok(id) = id_match.as_str().parse::<i32>() {
                    if let Some(entry) = self.entries.get(&id) {
                        match &entry.kind {
                            PasteKind::Text => {
                                if let Some(content) = self.get_text_content(entry) {
                                    result.push_str(&content);
                                } else {
                                    // Fallback: keep the pill
                                    result.push_str(full_match.as_str());
                                }
                            }
                            PasteKind::Image { .. } => {
                                // Images can't be resolved to text, keep pill
                                result.push_str(full_match.as_str());
                            }
                        }
                    } else {
                        // Unknown ID, keep the pill
                        result.push_str(full_match.as_str());
                    }
                } else {
                    result.push_str(full_match.as_str());
                }
            } else {
                result.push_str(full_match.as_str());
            }

            last_end = full_match.end();
        }

        // Add remaining text
        result.push_str(&text[last_end..]);
        result
    }

    /// Check if text contains any paste pills.
    pub fn has_pills(&self, text: &str) -> bool {
        self.pill_regex.is_match(text)
    }

    /// Clear all paste entries and cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.inline_cache.clear();
        self.inline_order.clear();
        // Don't reset next_id to avoid ID collision with any lingering pills
    }

    /// Store content inline.
    fn store_inline(&mut self, id: i32, data: Vec<u8>) {
        // LRU eviction if at capacity
        while self.inline_cache.len() >= MAX_INLINE_ENTRIES {
            if let Some(oldest_id) = self.inline_order.first().copied() {
                self.inline_cache.remove(&oldest_id);
                self.inline_order.remove(0);
            } else {
                break;
            }
        }

        self.inline_cache.insert(id, data);
        self.inline_order.push(id);
    }

    /// Store content on disk.
    fn store_on_disk(&self, id: i32, data: &[u8]) -> std::io::Result<PathBuf> {
        let hash = content_hash(data);
        let filename = format!("{id}-{hash}.bin");
        let path = self.cache_dir.join(filename);

        std::fs::write(&path, data)?;
        Ok(path)
    }

    /// Get text content from an entry.
    fn get_text_content(&self, entry: &PasteEntry) -> Option<String> {
        match &entry.storage {
            PasteStorage::Inline(text) => Some(text.clone()),
            PasteStorage::Disk(path) => std::fs::read_to_string(path).ok(),
        }
    }

    /// Get binary data from an entry (for images).
    fn get_image_data(&self, entry: &PasteEntry) -> Option<Vec<u8>> {
        match &entry.storage {
            PasteStorage::Inline(_) => self.inline_cache.get(&entry.id).cloned(),
            PasteStorage::Disk(path) => std::fs::read(path).ok(),
        }
    }
}

/// Count lines in text (handles various line endings).
fn count_lines(text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }

    let mut count = 1_i32;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' {
            count += 1;
            // Handle \r\n as single newline
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else if c == '\n' {
            count += 1;
        }
    }

    count
}

/// Generate a pill string for display.
fn generate_pill(id: i32, kind: &PasteKind, line_count: i32) -> String {
    match kind {
        PasteKind::Text => {
            if line_count > 1 {
                format!("[Pasted text #{id} +{} lines]", line_count - 1)
            } else {
                format!("[Pasted text #{id}]")
            }
        }
        PasteKind::Image { .. } => format!("[Image #{id}]"),
    }
}

/// Generate a content hash for disk storage filename.
fn content_hash(content: &[u8]) -> String {
    use sha1::Digest;
    use sha1::Sha1;

    let mut hasher = Sha1::new();
    hasher.update(content);
    let result = hasher.finalize();

    // Take first 8 bytes for a shorter hash
    hex::encode(&result[..8])
}

/// Check if the pill regex matches a specific pattern.
pub fn is_paste_pill(text: &str) -> bool {
    // Simple check without full regex for performance
    text.starts_with("[Pasted text #")
        || text.starts_with("[Image #")
        || text.starts_with("[...Truncated text #")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_lines_empty() {
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn test_count_lines_single() {
        assert_eq!(count_lines("hello"), 1);
    }

    #[test]
    fn test_count_lines_multiple() {
        assert_eq!(count_lines("hello\nworld"), 2);
        assert_eq!(count_lines("a\nb\nc"), 3);
    }

    #[test]
    fn test_count_lines_crlf() {
        assert_eq!(count_lines("hello\r\nworld"), 2);
    }

    #[test]
    fn test_count_lines_cr() {
        assert_eq!(count_lines("hello\rworld"), 2);
    }

    #[test]
    fn test_generate_pill_text_single_line() {
        let pill = generate_pill(1, &PasteKind::Text, 1);
        assert_eq!(pill, "[Pasted text #1]");
    }

    #[test]
    fn test_generate_pill_text_multi_line() {
        let pill = generate_pill(2, &PasteKind::Text, 421);
        assert_eq!(pill, "[Pasted text #2 +420 lines]");
    }

    #[test]
    fn test_generate_pill_image() {
        let pill = generate_pill(
            3,
            &PasteKind::Image {
                media_type: "image/png".to_string(),
            },
            0,
        );
        assert_eq!(pill, "[Image #3]");
    }

    #[test]
    fn test_is_paste_pill() {
        assert!(is_paste_pill("[Pasted text #1]"));
        assert!(is_paste_pill("[Pasted text #1 +420 lines]"));
        assert!(is_paste_pill("[Image #1]"));
        assert!(is_paste_pill("[...Truncated text #1]"));
        assert!(!is_paste_pill("hello world"));
        assert!(!is_paste_pill("[Some other bracket]"));
    }

    fn temp_cache_dir() -> PathBuf {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().to_path_buf();
        // Keep the tempdir alive by forgetting it (prevents cleanup)
        std::mem::forget(dir);
        path
    }

    #[test]
    fn test_process_small_text() {
        let mut manager = PasteManager::with_cache_dir(temp_cache_dir());
        let small_text = "hello world";
        let result = manager.process_text(small_text.to_string());

        // Small text should be returned as-is
        assert_eq!(result, small_text);
        assert!(manager.entries.is_empty());
    }

    #[test]
    fn test_process_large_text() {
        let mut manager = PasteManager::with_cache_dir(temp_cache_dir());
        let large_text = "x".repeat(2000);
        let result = manager.process_text(large_text);

        // Should return a pill
        assert!(result.starts_with("[Pasted text #"));
        assert_eq!(manager.entries.len(), 1);
    }

    #[test]
    fn test_resolve_pills_no_pills() {
        let manager = PasteManager::with_cache_dir(temp_cache_dir());
        let text = "hello world";
        let resolved = manager.resolve_pills(text);
        assert_eq!(resolved, text);
    }

    #[test]
    fn test_resolve_to_blocks_no_pills() {
        let manager = PasteManager::with_cache_dir(temp_cache_dir());
        let text = "hello world";
        let blocks = manager.resolve_to_blocks(text);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].as_text(), Some("hello world"));
    }

    #[test]
    fn test_process_and_resolve_text() {
        let mut manager = PasteManager::with_cache_dir(temp_cache_dir());
        let content = "line1\nline2\nline3\n".repeat(100); // Make it large enough
        let pill = manager.process_text(content.clone());

        // Verify it's a pill
        assert!(pill.starts_with("[Pasted text #"));

        // Resolve and verify
        let resolved = manager.resolve_pills(&pill);
        assert_eq!(resolved, content);
    }

    #[test]
    fn test_mixed_text_and_pill() {
        let mut manager = PasteManager::with_cache_dir(temp_cache_dir());
        let content = "x".repeat(2000);
        let pill = manager.process_text(content.clone());

        let input = format!("Please analyze this: {pill} and tell me what it means.");
        let resolved = manager.resolve_pills(&input);

        assert!(resolved.starts_with("Please analyze this: "));
        assert!(resolved.contains(&content));
        assert!(resolved.ends_with(" and tell me what it means."));
    }

    #[test]
    fn test_has_pills() {
        let manager = PasteManager::with_cache_dir(temp_cache_dir());

        assert!(!manager.has_pills("hello world"));
        assert!(manager.has_pills("[Pasted text #1]"));
        assert!(manager.has_pills("[Pasted text #1 +420 lines]"));
        assert!(manager.has_pills("[Image #1]"));
        assert!(manager.has_pills("Before [Pasted text #1] after"));
    }

    #[test]
    fn test_content_hash() {
        let hash1 = content_hash(b"hello");
        let hash2 = content_hash(b"hello");
        let hash3 = content_hash(b"world");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // 8 bytes = 16 hex chars
    }
}
