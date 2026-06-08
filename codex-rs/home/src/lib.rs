//! Global instruction contributor backed by files in the Codex home directory.

use std::io;

use codex_extension_api::GlobalInstruction;
use codex_extension_api::GlobalInstructions;
use codex_extension_api::GlobalInstructionsContributor;
use codex_extension_api::GlobalInstructionsFuture;
use codex_utils_absolute_path::AbsolutePathBuf;

pub const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
pub const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";

/// Loads global instructions from a configured Codex home directory.
pub struct CodexHomeInstructionsContributor {
    codex_home: AbsolutePathBuf,
}

impl CodexHomeInstructionsContributor {
    pub fn new(codex_home: AbsolutePathBuf) -> Self {
        Self { codex_home }
    }

    async fn load(&self) -> Result<GlobalInstructions, String> {
        let mut warnings = Vec::new();
        let mut read_errors = Vec::new();
        for candidate in [LOCAL_AGENTS_MD_FILENAME, DEFAULT_AGENTS_MD_FILENAME] {
            let path = self.codex_home.join(candidate);
            let data = match tokio::fs::read(path.as_path()).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) if err.kind() == io::ErrorKind::IsADirectory => continue,
                Err(err) => {
                    let warning = format!(
                        "Failed to read global AGENTS.md instructions from `{}`: {err}",
                        path.display()
                    );
                    warnings.push(warning.clone());
                    read_errors.push(warning);
                    continue;
                }
            };
            if let Err(err) = std::str::from_utf8(&data) {
                warnings.push(format!(
                    "Global AGENTS.md instructions from `{}` contain invalid UTF-8: {err}. Invalid byte sequences were replaced.",
                    path.display(),
                ));
            }
            let contents = String::from_utf8_lossy(&data);
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return Ok(GlobalInstructions {
                    instructions: vec![GlobalInstruction::new(trimmed.to_string(), Some(path))],
                    warnings,
                });
            }
        }

        if !read_errors.is_empty() {
            return Err(read_errors.join("; "));
        }

        Ok(GlobalInstructions {
            instructions: Vec::new(),
            warnings,
        })
    }
}

impl GlobalInstructionsContributor for CodexHomeInstructionsContributor {
    fn contribute(&self) -> GlobalInstructionsFuture<'_> {
        Box::pin(self.load())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
