//! Blueprint persistence
//!
//! Handles writing blueprints to both Markdown (human-readable) and JSON (machine-readable) formats.

use super::schema::BlueprintBlock;
use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Blueprint persistence manager
pub struct BlueprintPersister {
    /// Base directory for markdown exports
    markdown_dir: PathBuf,
    /// Base directory for JSON logs
    json_dir: PathBuf,
}

impl BlueprintPersister {
    /// Create a new persister with default directories
    pub fn new() -> Result<Self> {
        let markdown_dir = PathBuf::from("docs/blueprints");
        let json_dir = PathBuf::from("logs/blueprint");

        // Ensure directories exist
        fs::create_dir_all(&markdown_dir)?;
        fs::create_dir_all(&json_dir)?;

        Ok(Self {
            markdown_dir,
            json_dir,
        })
    }

    /// Create a persister with custom directories
    pub fn with_dirs(markdown_dir: PathBuf, json_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&markdown_dir)?;
        fs::create_dir_all(&json_dir)?;

        Ok(Self {
            markdown_dir,
            json_dir,
        })
    }

    /// Save blueprint as markdown
    pub fn save_markdown(&self, blueprint: &BlueprintBlock) -> Result<PathBuf> {
        let filename = format!(
            "{}_{}.md",
            blueprint.created_at.format("%Y-%m-%d"),
            blueprint.title.to_lowercase().replace(' ', "-")
        );
        let path = self.markdown_dir.join(&filename);

        let content = self.to_markdown(blueprint);
        fs::write(&path, content)
            .with_context(|| format!("Failed to write markdown to {}", path.display()))?;

        Ok(path)
    }

    /// Save blueprint as JSON
    pub fn save_json(&self, blueprint: &BlueprintBlock) -> Result<PathBuf> {
        let filename = format!("{}.json", blueprint.id);
        let path = self.json_dir.join(&filename);

        let content = serde_json::to_string_pretty(blueprint)
            .context("Failed to serialize blueprint to JSON")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write JSON to {}", path.display()))?;

        Ok(path)
    }

    /// Load blueprint from JSON
    pub fn load_json(&self, id: &str) -> Result<BlueprintBlock> {
        let filename = format!("{}.json", id);
        let path = self.json_dir.join(&filename);

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read JSON from {}", path.display()))?;
        let blueprint: BlueprintBlock =
            serde_json::from_str(&content).context("Failed to deserialize blueprint from JSON")?;

        Ok(blueprint)
    }

    /// Export blueprint (saves both formats)
    pub fn export(&self, blueprint: &BlueprintBlock) -> Result<(PathBuf, PathBuf)> {
        let md_path = self.save_markdown(blueprint)?;
        let json_path = self.save_json(blueprint)?;
        Ok((md_path, json_path))
    }

    /// List all blueprint IDs
    pub fn list_blueprints(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();

        if !self.json_dir.exists() {
            return Ok(ids);
        }

        for entry in fs::read_dir(&self.json_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }

        Ok(ids)
    }

    /// Convert blueprint to markdown format
    fn to_markdown(&self, bp: &BlueprintBlock) -> String {
        let mut md = String::new();

        // Header
        md.push_str(&format!("# {}\n\n", bp.title));
        md.push_str(&format!("**Blueprint ID**: `{}`  \n", bp.id));
        md.push_str(&format!("**Status**: {}  \n", bp.state));
        md.push_str(&format!("**Mode**: {}  \n", bp.mode));
        md.push_str(&format!(
            "**Created**: {}  \n",
            bp.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!(
            "**Updated**: {}  \n\n",
            bp.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Goal
        md.push_str("## Goal\n\n");
        md.push_str(&format!("{}\n\n", bp.goal));

        // Assumptions
        if !bp.assumptions.is_empty() {
            md.push_str("## Assumptions\n\n");
            for assumption in &bp.assumptions {
                md.push_str(&format!("- {}\n", assumption));
            }
            md.push('\n');
        }

        // Clarifying Questions
        if !bp.clarifying_questions.is_empty() {
            md.push_str("## Clarifying Questions\n\n");
            for question in &bp.clarifying_questions {
                md.push_str(&format!("- {}\n", question));
            }
            md.push('\n');
        }

        // Approach
        if !bp.approach.is_empty() {
            md.push_str("## Approach\n\n");
            md.push_str(&format!("{}\n\n", bp.approach));
        }

        // Work Items
        if !bp.work_items.is_empty() {
            md.push_str("## Work Items\n\n");
            for item in &bp.work_items {
                md.push_str(&format!("### {}\n\n", item.name));
                md.push_str(&format!("**Files**: {}\n", item.files_touched.join(", ")));
                md.push_str(&format!("**Diff Contract**: {}\n", item.diff_contract));
                if !item.tests.is_empty() {
                    md.push_str(&format!("**Tests**: {}\n", item.tests.join(", ")));
                }
                md.push('\n');
            }
        }

        // Risks
        if !bp.risks.is_empty() {
            md.push_str("## Risks & Mitigations\n\n");
            for risk in &bp.risks {
                md.push_str(&format!("**Risk**: {}\n", risk.item));
                md.push_str(&format!("**Mitigation**: {}\n\n", risk.mitigation));
            }
        }

        // Evaluation Criteria
        md.push_str("## Evaluation Criteria\n\n");
        if !bp.eval.tests.is_empty() {
            md.push_str("**Tests**:\n");
            for test in &bp.eval.tests {
                md.push_str(&format!("- {}\n", test));
            }
            md.push('\n');
        }
        if !bp.eval.metrics.is_empty() {
            md.push_str("**Metrics**:\n");
            for (key, value) in &bp.eval.metrics {
                md.push_str(&format!("- {}: {}\n", key, value));
            }
            md.push('\n');
        }

        // Budget
        md.push_str("## Budget\n\n");
        if let Some(max_step) = bp.budget.max_step {
            md.push_str(&format!("- Max tokens per step: {}\n", max_step));
        }
        if let Some(session_cap) = bp.budget.session_cap {
            md.push_str(&format!("- Session token cap: {}\n", session_cap));
        }
        if let Some(estimate) = bp.budget.estimate_min {
            md.push_str(&format!("- Time estimate: {} minutes\n", estimate));
        }
        if let Some(cap) = bp.budget.cap_min {
            md.push_str(&format!("- Time cap: {} minutes\n", cap));
        }
        md.push('\n');

        // Rollback Plan
        if !bp.rollback.is_empty() {
            md.push_str("## Rollback Plan\n\n");
            md.push_str(&format!("{}\n\n", bp.rollback));
        }

        // Research Results
        if let Some(research) = &bp.research {
            md.push_str("## Research Results\n\n");
            md.push_str(&format!("**Query**: {}\n", research.query));
            md.push_str(&format!("**Depth**: {}\n", research.depth));
            md.push_str(&format!("**Strategy**: {}\n", research.strategy));
            md.push_str(&format!("**Confidence**: {:.2}\n\n", research.confidence));

            if !research.sources.is_empty() {
                md.push_str("### Sources\n\n");
                for source in &research.sources {
                    md.push_str(&format!("- [{}]({})\n", source.title, source.url));
                    md.push_str(&format!("  - Date: {}\n", source.date));
                    md.push_str(&format!("  - Finding: {}\n", source.key_finding));
                    md.push_str(&format!("  - Confidence: {:.2}\n\n", source.confidence));
                }
            }

            md.push_str("### Synthesis\n\n");
            md.push_str(&format!("{}\n\n", research.synthesis));
        }

        // Artifacts
        if !bp.artifacts.is_empty() {
            md.push_str("## Artifacts\n\n");
            for artifact in &bp.artifacts {
                md.push_str(&format!("- {}\n", artifact));
            }
            md.push('\n');
        }

        md
    }
}

impl Default for BlueprintPersister {
    fn default() -> Self {
        Self::new().expect("Failed to create default BlueprintPersister")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_blueprint() -> BlueprintBlock {
        let mut bp = BlueprintBlock::new("Test blueprint".to_string(), "test-bp".to_string());
        bp.assumptions.push("Test assumption".to_string());
        bp.approach = "Test approach".to_string();
        bp
    }

    #[test]
    fn test_save_and_load_json() {
        let temp_dir = TempDir::new().unwrap();
        let markdown_dir = temp_dir.path().join("markdown");
        let json_dir = temp_dir.path().join("json");

        let persister = BlueprintPersister::with_dirs(markdown_dir, json_dir).unwrap();
        let bp = create_test_blueprint();

        // Save
        let json_path = persister.save_json(&bp).unwrap();
        assert!(json_path.exists());

        // Load
        let loaded = persister.load_json(&bp.id).unwrap();
        assert_eq!(loaded.id, bp.id);
        assert_eq!(loaded.title, bp.title);
    }

    #[test]
    fn test_save_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let markdown_dir = temp_dir.path().join("markdown");
        let json_dir = temp_dir.path().join("json");

        let persister = BlueprintPersister::with_dirs(markdown_dir, json_dir).unwrap();
        let bp = create_test_blueprint();

        let md_path = persister.save_markdown(&bp).unwrap();
        assert!(md_path.exists());

        let content = fs::read_to_string(md_path).unwrap();
        assert!(content.contains(&bp.title));
        assert!(content.contains(&bp.goal));
    }

    #[test]
    fn test_list_blueprints() {
        let temp_dir = TempDir::new().unwrap();
        let markdown_dir = temp_dir.path().join("markdown");
        let json_dir = temp_dir.path().join("json");

        let persister = BlueprintPersister::with_dirs(markdown_dir, json_dir).unwrap();

        // Initially empty
        assert_eq!(persister.list_blueprints().unwrap().len(), 0);

        // Save a blueprint
        let bp = create_test_blueprint();
        persister.save_json(&bp).unwrap();

        // Should list one blueprint
        let ids = persister.list_blueprints().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], bp.id);
    }
}
