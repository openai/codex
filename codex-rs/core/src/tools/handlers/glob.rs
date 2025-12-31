use async_trait::async_trait;
use globset::Glob;
use globset::GlobSetBuilder;
use serde::Deserialize;
use walkdir::WalkDir;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct GlobHandler;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 2000;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[async_trait]
impl ToolHandler for GlobHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "glob handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GlobArgs = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse function arguments: {err:?}"
            ))
        })?;

        let pattern = args.pattern.trim();
        if pattern.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "pattern must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit.min(MAX_LIMIT);
        let search_path = turn.resolve_path(args.path.clone());

        verify_path_exists(&search_path).await?;

        let matches = collect_matches(pattern, &search_path, limit)?;

        if matches.is_empty() {
            Ok(ToolOutput::Function {
                content: "No matches found.".to_string(),
                content_items: None,
                success: Some(false),
            })
        } else {
            Ok(ToolOutput::Function {
                content: matches.join("\n"),
                content_items: None,
                success: Some(true),
            })
        }
    }
}

async fn verify_path_exists(path: &std::path::Path) -> Result<(), FunctionCallError> {
    tokio::fs::metadata(path).await.map_err(|err| {
        let display_path = path.display();
        FunctionCallError::RespondToModel(format!("unable to access `{display_path}`: {err}"))
    })?;
    Ok(())
}

fn collect_matches(
    pattern: &str,
    search_path: &std::path::Path,
    limit: usize,
) -> Result<Vec<String>, FunctionCallError> {
    let glob = Glob::new(pattern)
        .map_err(|err| FunctionCallError::RespondToModel(format!("invalid glob pattern: {err}")))?;
    let globset = GlobSetBuilder::new()
        .add(glob)
        .build()
        .map_err(|err| FunctionCallError::RespondToModel(format!("invalid glob pattern: {err}")))?;
    let match_absolute = std::path::Path::new(pattern).is_absolute();

    let mut results = Vec::new();
    for entry in WalkDir::new(search_path).follow_links(true) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "failed to walk directory: {err}"
                )));
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let candidate = if match_absolute {
            path
        } else {
            path.strip_prefix(search_path).unwrap_or(path)
        };
        if globset.is_match(candidate) {
            results.push(path.to_string_lossy().into_owned());
            if results.len() == limit {
                break;
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn collects_matches_with_relative_pattern() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let root = temp.path();
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::write(root.join("src/lib.rs"), "lib")?;
        std::fs::write(root.join("src/main.rs"), "main")?;
        std::fs::write(root.join("README.md"), "readme")?;

        let matches = collect_matches("src/*.rs", root, 10)?;
        assert_eq!(matches.len(), 2);
        assert_eq!(matches.iter().all(|path| path.ends_with(".rs")), true);
        Ok(())
    }

    #[test]
    fn respects_limit() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let root = temp.path();
        std::fs::write(root.join("a.txt"), "a")?;
        std::fs::write(root.join("b.txt"), "b")?;
        std::fs::write(root.join("c.txt"), "c")?;

        let matches = collect_matches("*.txt", root, 2)?;
        assert_eq!(matches.len(), 2);
        Ok(())
    }
}
