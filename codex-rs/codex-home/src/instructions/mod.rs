use std::io;

use codex_extension_api::UserInstructions;
use codex_extension_api::UserInstructionsLoadFuture;
use codex_extension_api::UserInstructionsLoadOutcome;
use codex_extension_api::UserInstructionsProvider;
use codex_utils_absolute_path::AbsolutePathBuf;

const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";

/// Loads user instructions from a Codex home directory.
#[derive(Clone, Debug)]
pub struct CodexHomeUserInstructionsProvider {
    codex_home: AbsolutePathBuf,
}

impl CodexHomeUserInstructionsProvider {
    /// Creates a provider rooted at the supplied absolute Codex home directory.
    pub fn new(codex_home: AbsolutePathBuf) -> Self {
        Self { codex_home }
    }

    async fn load_from_codex_home(&self) -> UserInstructionsLoadOutcome {
        let mut warnings = Vec::new();
        for candidate in [LOCAL_AGENTS_MD_FILENAME, DEFAULT_AGENTS_MD_FILENAME] {
            let path = self.codex_home.join(candidate);
            let data = match tokio::fs::read(path.as_path()).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) if err.kind() == io::ErrorKind::IsADirectory => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global AGENTS.md instructions from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            };
            if let Err(err) = std::str::from_utf8(&data) {
                warnings.push(format!(
                    "Global AGENTS.md instructions from `{}` contain invalid UTF-8: {err}. Invalid byte sequences were replaced.",
                    path.display()
                ));
            }
            let contents = String::from_utf8_lossy(&data);
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return UserInstructionsLoadOutcome {
                    instructions: Some(UserInstructions {
                        text: trimmed.to_string(),
                        source: path,
                    }),
                    warnings,
                };
            }
        }
        UserInstructionsLoadOutcome {
            instructions: None,
            warnings,
        }
    }
}

impl UserInstructionsProvider for CodexHomeUserInstructionsProvider {
    fn load(&self) -> UserInstructionsLoadFuture<'_> {
        Box::pin(self.load_from_codex_home())
    }
}

#[cfg(test)]
mod tests;
