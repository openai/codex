//! File edit conflict resolution for multi-agent orchestration.
//!
//! Provides mechanisms to track and resolve conflicts when multiple agents
//! attempt to edit the same files concurrently.

use anyhow::Context;
use anyhow::Result;
use dashmap::DashMap;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Strategy for resolving edit conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Execute edits sequentially (safe but slower)
    Sequential,
    /// Attempt three-way merge (faster but may fail)
    ThreeWayMerge,
    /// Last write wins (fast but risky)
    LastWriteWins,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::Sequential
    }
}

/// Token representing permission to edit a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditToken {
    /// File path being edited
    pub file_path: PathBuf,
    /// Agent that requested the edit
    pub agent_name: String,
    /// Unique edit ID
    pub edit_id: uuid::Uuid,
}

/// A single edit operation on a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOperation {
    /// Agent performing the edit
    pub agent_name: String,
    /// Original content (before edit)
    pub original_content: Option<String>,
    /// New content (after edit)
    pub new_content: String,
    /// Edit timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Edit ID
    pub edit_id: uuid::Uuid,
}

/// Merged content from multiple edits.
#[derive(Debug, Clone)]
pub struct MergedContent {
    /// Final merged content
    pub content: String,
    /// Whether conflicts were detected
    pub had_conflicts: bool,
    /// Agents that contributed to the merge
    pub contributors: Vec<String>,
}

/// Result of a three-way merge operation.
#[derive(Debug, Clone)]
struct ThreeWayMergeResult {
    /// Merged content (may contain conflict markers)
    merged_content: String,
    /// Whether conflicts were detected
    has_conflicts: bool,
}

/// Tracks file edits and manages conflicts.
pub struct FileEditTracker {
    /// Map of file paths to their edit queues
    file_edits: DashMap<PathBuf, Arc<RwLock<Vec<EditOperation>>>>,
    /// Default merge strategy
    strategy: MergeStrategy,
}

impl FileEditTracker {
    /// Create a new file edit tracker.
    pub fn new(strategy: MergeStrategy) -> Self {
        Self {
            file_edits: DashMap::new(),
            strategy,
        }
    }

    /// Request permission to edit a file.
    ///
    /// Returns an `EditToken` that must be used to commit the edit.
    pub async fn request_edit(&self, file: PathBuf, agent: String) -> EditToken {
        let edit_id = uuid::Uuid::new_v4();

        // Ensure the file has an edit queue
        self.file_edits
            .entry(file.clone())
            .or_insert_with(|| Arc::new(RwLock::new(Vec::new())));

        debug!(
            "Agent '{}' requested edit permission for {:?} (ID: {})",
            agent, file, edit_id
        );

        EditToken {
            file_path: file,
            agent_name: agent,
            edit_id,
        }
    }

    /// Commit an edit using the provided token.
    ///
    /// This adds the edit to the queue and waits if sequential execution is required.
    pub async fn commit_edit(
        &self,
        token: EditToken,
        original_content: Option<String>,
        new_content: String,
    ) -> Result<()> {
        let edit_op = EditOperation {
            agent_name: token.agent_name.clone(),
            original_content,
            new_content,
            timestamp: chrono::Utc::now(),
            edit_id: token.edit_id,
        };

        if let Some(edit_queue) = self.file_edits.get(&token.file_path) {
            let mut queue = edit_queue.write().await;
            queue.push(edit_op);

            info!(
                "Agent '{}' committed edit to {:?} (ID: {}, queue length: {})",
                token.agent_name,
                token.file_path,
                token.edit_id,
                queue.len()
            );
        } else {
            anyhow::bail!("Edit token for non-existent file: {:?}", token.file_path);
        }

        Ok(())
    }

    /// Resolve conflicts for a file and return merged content.
    pub async fn resolve_conflicts(&self, file: &Path) -> Result<MergedContent> {
        let edit_queue = self
            .file_edits
            .get(file)
            .context("No edits found for file")?;

        let queue = edit_queue.read().await;

        if queue.is_empty() {
            anyhow::bail!("No edits to resolve for {file:?}");
        }

        // If only one edit, no conflict
        if queue.len() == 1 {
            return Ok(MergedContent {
                content: queue[0].new_content.clone(),
                had_conflicts: false,
                contributors: vec![queue[0].agent_name.clone()],
            });
        }

        // Multiple edits - resolve based on strategy
        match self.strategy {
            MergeStrategy::Sequential => self.resolve_sequential(&queue).await,
            MergeStrategy::ThreeWayMerge => self.resolve_three_way(&queue).await,
            MergeStrategy::LastWriteWins => self.resolve_last_write_wins(&queue).await,
        }
    }

    /// Resolve conflicts using sequential strategy (last edit wins).
    async fn resolve_sequential(&self, queue: &[EditOperation]) -> Result<MergedContent> {
        let last_edit = queue.last().context("Empty edit queue")?;

        info!(
            "Resolving {} edits sequentially, using last edit from '{}'",
            queue.len(),
            last_edit.agent_name
        );

        Ok(MergedContent {
            content: last_edit.new_content.clone(),
            had_conflicts: queue.len() > 1,
            contributors: queue.iter().map(|e| e.agent_name.clone()).collect(),
        })
    }

    /// Resolve conflicts using three-way merge strategy.
    ///
    /// Implements Git-style 3-way merge:
    /// - Base: Original content (common ancestor)
    /// - Ours: First agent's changes
    /// - Theirs: Subsequent agents' changes
    ///
    /// Automatically merges non-conflicting changes and inserts conflict markers
    /// for conflicting sections.
    async fn resolve_three_way(&self, queue: &[EditOperation]) -> Result<MergedContent> {
        if queue.len() < 2 {
            debug!("Less than 2 edits, using sequential strategy");
            return self.resolve_sequential(queue).await;
        }

        info!("🔀 Starting ThreeWayMerge for {} edits", queue.len());

        // 1. Determine base content (common ancestor)
        let base = queue[0]
            .original_content
            .as_deref()
            .unwrap_or("")
            .to_string();

        // 2. Start with first edit as current content
        let mut current_content = queue[0].new_content.clone();
        let mut had_conflicts = false;
        let mut contributors = vec![queue[0].agent_name.clone()];

        // 3. Merge each subsequent edit
        for (i, edit) in queue.iter().enumerate().skip(1) {
            contributors.push(edit.agent_name.clone());

            debug!(
                "  Merging edit {}/{}: '{}' -> '{}'",
                i + 1,
                queue.len(),
                queue[i - 1].agent_name,
                edit.agent_name
            );

            // Perform 3-way diff: base vs current vs new_edit
            let merge_result = self.three_way_diff(
                &base,
                &current_content,
                &edit.new_content,
                &queue[i - 1].agent_name,
                &edit.agent_name,
            );

            if merge_result.has_conflicts {
                had_conflicts = true;
                warn!(
                    "⚠️  Conflicts detected between '{}' and '{}'",
                    queue[i - 1].agent_name,
                    edit.agent_name
                );
            }

            current_content = merge_result.merged_content;
        }

        info!(
            "✅ ThreeWayMerge completed: {} edits, conflicts: {}",
            queue.len(),
            had_conflicts
        );

        Ok(MergedContent {
            content: current_content,
            had_conflicts,
            contributors,
        })
    }

    /// Perform three-way diff and merge.
    fn three_way_diff(
        &self,
        base: &str,
        ours: &str,
        theirs: &str,
        ours_name: &str,
        theirs_name: &str,
    ) -> ThreeWayMergeResult {
        let mut merged = String::new();
        let mut has_conflicts = false;

        let base_lines: Vec<&str> = base.lines().collect();
        let ours_lines: Vec<&str> = ours.lines().collect();
        let theirs_lines: Vec<&str> = theirs.lines().collect();

        let mut base_idx = 0;
        let mut ours_idx = 0;
        let mut theirs_idx = 0;

        // Simple line-by-line 3-way merge
        while base_idx < base_lines.len()
            || ours_idx < ours_lines.len()
            || theirs_idx < theirs_lines.len()
        {
            let base_line = base_lines.get(base_idx);
            let ours_line = ours_lines.get(ours_idx);
            let theirs_line = theirs_lines.get(theirs_idx);

            match (base_line, ours_line, theirs_line) {
                // All three are the same - no changes
                (Some(b), Some(o), Some(t)) if b == o && o == t => {
                    merged.push_str(b);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // Ours changed, theirs unchanged - use ours
                (Some(b), Some(o), Some(t)) if b == t && b != o => {
                    merged.push_str(o);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // Theirs changed, ours unchanged - use theirs
                (Some(b), Some(o), Some(t)) if b == o && b != t => {
                    merged.push_str(t);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // Both changed to the same thing - use either
                (Some(_b), Some(o), Some(t)) if o == t => {
                    merged.push_str(o);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // Both changed differently - CONFLICT!
                (Some(_b), Some(o), Some(t)) => {
                    has_conflicts = true;
                    merged.push_str("<<<<<<< Agent: ");
                    merged.push_str(ours_name);
                    merged.push('\n');
                    merged.push_str(o);
                    merged.push('\n');
                    merged.push_str("=======\n");
                    merged.push_str(t);
                    merged.push('\n');
                    merged.push_str(">>>>>>> Agent: ");
                    merged.push_str(theirs_name);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                (None, Some(o), None) => {
                    merged.push_str(o);
                    merged.push('\n');
                    ours_idx += 1;
                }
                // Theirs only (no base, no ours)
                (None, None, Some(t)) => {
                    merged.push_str(t);
                    merged.push('\n');
                    theirs_idx += 1;
                }
                // Base only (both deleted)
                (Some(_b), None, None) => {
                    base_idx += 1;
                }
                // Ours and base, no theirs
                (Some(_b), Some(o), None) => {
                    merged.push_str(o);
                    merged.push('\n');
                    base_idx += 1;
                    ours_idx += 1;
                }
                // Theirs and base, no ours
                (Some(_b), None, Some(t)) => {
                    merged.push_str(t);
                    merged.push('\n');
                    base_idx += 1;
                    theirs_idx += 1;
                }
                // Both sides added, same content
                (None, Some(o), Some(t)) if o == t => {
                    merged.push_str(o);
                    merged.push('\n');
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // Conflicting additions
                (None, Some(o), Some(t)) => {
                    has_conflicts = true;
                    merged.push_str("<<<<<<< Agent: ");
                    merged.push_str(ours_name);
                    merged.push_str(" (added)\n");
                    merged.push_str(o);
                    merged.push('\n');
                    merged.push_str("=======\n");
                    merged.push_str(t);
                    merged.push('\n');
                    merged.push_str(">>>>>>> Agent: ");
                    merged.push_str(theirs_name);
                    merged.push_str(" (added)\n");
                    ours_idx += 1;
                    theirs_idx += 1;
                }
                // All None - shouldn't happen
                (None, None, None) => {
                    break;
                }
            }
        }

        ThreeWayMergeResult {
            merged_content: merged,
            has_conflicts,
        }
    }

    /// Resolve conflicts using last-write-wins strategy.
    async fn resolve_last_write_wins(&self, queue: &[EditOperation]) -> Result<MergedContent> {
        // Sort by timestamp and take the latest
        let mut sorted = queue.to_vec();
        sorted.sort_by_key(|e| e.timestamp);

        let latest = sorted.last().context("Empty edit queue")?;

        info!(
            "Resolving {} edits using last-write-wins, winner: '{}'",
            queue.len(),
            latest.agent_name
        );

        Ok(MergedContent {
            content: latest.new_content.clone(),
            had_conflicts: queue.len() > 1,
            contributors: queue.iter().map(|e| e.agent_name.clone()).collect(),
        })
    }

    /// Get the number of pending edits for a file.
    pub async fn pending_edits_count(&self, file: &Path) -> usize {
        if let Some(edit_queue) = self.file_edits.get(file) {
            edit_queue.read().await.len()
        } else {
            0
        }
    }

    /// Clear all edits for a file.
    pub async fn clear_edits(&self, file: &Path) {
        if let Some(edit_queue) = self.file_edits.get(file) {
            edit_queue.write().await.clear();
            debug!("Cleared all edits for {:?}", file);
        }
    }
}

/// Conflict resolver that integrates with the orchestrator.
pub struct ConflictResolver {
    /// File edit tracker
    tracker: Arc<FileEditTracker>,
}

impl ConflictResolver {
    /// Create a new conflict resolver with the specified strategy.
    pub fn new(strategy: MergeStrategy) -> Self {
        Self {
            tracker: Arc::new(FileEditTracker::new(strategy)),
        }
    }

    /// Get a reference to the file edit tracker.
    pub fn tracker(&self) -> Arc<FileEditTracker> {
        self.tracker.clone()
    }

    /// Resolve all pending conflicts and return a map of file paths to merged content.
    pub async fn resolve_all(&self) -> Result<Vec<(PathBuf, MergedContent)>> {
        let mut results = Vec::new();

        for entry in self.tracker.file_edits.iter() {
            let file_path = entry.key().clone();
            match self.tracker.resolve_conflicts(&file_path).await {
                Ok(merged) => {
                    results.push((file_path.clone(), merged));
                }
                Err(e) => {
                    warn!("Failed to resolve conflicts for {:?}: {}", file_path, e);
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_edit_no_conflict() {
        let tracker = FileEditTracker::new(MergeStrategy::Sequential);
        let file = PathBuf::from("test.txt");

        let token = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        tracker
            .commit_edit(token, None, "Hello, world!".to_string())
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();
        assert_eq!(merged.content, "Hello, world!");
        assert!(!merged.had_conflicts);
        assert_eq!(merged.contributors, vec!["agent1"]);
    }

    #[tokio::test]
    async fn test_multiple_edits_sequential() {
        let tracker = FileEditTracker::new(MergeStrategy::Sequential);
        let file = PathBuf::from("test.txt");

        let token1 = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        let token2 = tracker
            .request_edit(file.clone(), "agent2".to_string())
            .await;

        tracker
            .commit_edit(token1, None, "First edit".to_string())
            .await
            .unwrap();
        tracker
            .commit_edit(token2, None, "Second edit".to_string())
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();
        assert_eq!(merged.content, "Second edit");
        assert!(merged.had_conflicts);
        assert_eq!(merged.contributors.len(), 2);
    }

    #[tokio::test]
    async fn test_last_write_wins() {
        let tracker = FileEditTracker::new(MergeStrategy::LastWriteWins);
        let file = PathBuf::from("test.txt");

        let token1 = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let token2 = tracker
            .request_edit(file.clone(), "agent2".to_string())
            .await;

        tracker
            .commit_edit(token1, None, "Older edit".to_string())
            .await
            .unwrap();
        tracker
            .commit_edit(token2, None, "Newer edit".to_string())
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();
        assert_eq!(merged.content, "Newer edit");
        assert!(merged.had_conflicts);
    }

    #[tokio::test]
    async fn test_three_way_merge_no_conflict() {
        let tracker = FileEditTracker::new(MergeStrategy::ThreeWayMerge);
        let file = PathBuf::from("test.txt");

        let base_content = "Line 1\nLine 2\nLine 3\n";

        let token1 = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        let token2 = tracker
            .request_edit(file.clone(), "agent2".to_string())
            .await;

        // Agent1: Change line 1
        tracker
            .commit_edit(
                token1,
                Some(base_content.to_string()),
                "Line 1 (modified by agent1)\nLine 2\nLine 3\n".to_string(),
            )
            .await
            .unwrap();

        // Agent2: Change line 3 (non-conflicting)
        tracker
            .commit_edit(
                token2,
                Some(base_content.to_string()),
                "Line 1\nLine 2\nLine 3 (modified by agent2)\n".to_string(),
            )
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();

        // Should merge both changes without conflict
        assert!(!merged.had_conflicts, "Expected no conflicts");
        assert!(merged.content.contains("modified by agent1"));
        assert!(merged.content.contains("modified by agent2"));
        assert_eq!(merged.contributors.len(), 2);
    }

    #[tokio::test]
    async fn test_three_way_merge_with_conflict() {
        let tracker = FileEditTracker::new(MergeStrategy::ThreeWayMerge);
        let file = PathBuf::from("test.txt");

        let base_content = "Line 1\nLine 2\nLine 3\n";

        let token1 = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        let token2 = tracker
            .request_edit(file.clone(), "agent2".to_string())
            .await;

        // Agent1: Change line 2
        tracker
            .commit_edit(
                token1,
                Some(base_content.to_string()),
                "Line 1\nLine 2 (agent1 version)\nLine 3\n".to_string(),
            )
            .await
            .unwrap();

        // Agent2: Also change line 2 (CONFLICT!)
        tracker
            .commit_edit(
                token2,
                Some(base_content.to_string()),
                "Line 1\nLine 2 (agent2 version)\nLine 3\n".to_string(),
            )
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();

        // Should detect conflict
        assert!(merged.had_conflicts, "Expected conflicts to be detected");
        assert!(
            merged.content.contains("<<<<<<<"),
            "Expected conflict markers"
        );
        assert!(
            merged.content.contains("======="),
            "Expected conflict markers"
        );
        assert!(
            merged.content.contains(">>>>>>>"),
            "Expected conflict markers"
        );
        assert!(merged.content.contains("agent1"));
        assert!(merged.content.contains("agent2"));
    }

    #[tokio::test]
    async fn test_three_way_merge_multiple_agents() {
        let tracker = FileEditTracker::new(MergeStrategy::ThreeWayMerge);
        let file = PathBuf::from("test.txt");

        let base_content = "Line 1\nLine 2\nLine 3\nLine 4\n";

        let token1 = tracker
            .request_edit(file.clone(), "agent1".to_string())
            .await;
        let token2 = tracker
            .request_edit(file.clone(), "agent2".to_string())
            .await;
        let token3 = tracker
            .request_edit(file.clone(), "agent3".to_string())
            .await;

        // Agent1: Change line 1
        tracker
            .commit_edit(
                token1,
                Some(base_content.to_string()),
                "Line 1 (by agent1)\nLine 2\nLine 3\nLine 4\n".to_string(),
            )
            .await
            .unwrap();

        // Agent2: Change line 2
        tracker
            .commit_edit(
                token2,
                Some(base_content.to_string()),
                "Line 1\nLine 2 (by agent2)\nLine 3\nLine 4\n".to_string(),
            )
            .await
            .unwrap();

        // Agent3: Change line 3
        tracker
            .commit_edit(
                token3,
                Some(base_content.to_string()),
                "Line 1\nLine 2\nLine 3 (by agent3)\nLine 4\n".to_string(),
            )
            .await
            .unwrap();

        let merged = tracker.resolve_conflicts(&file).await.unwrap();

        // All three changes should be merged successfully
        assert_eq!(merged.contributors.len(), 3);
        assert!(
            merged.content.contains("agent1")
                || merged.content.contains("agent2")
                || merged.content.contains("agent3"),
            "Expected all agents' changes to be included"
        );
    }
}
