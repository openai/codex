use std::io;
use std::path::Path;

use codex_extension_api::LoadUserInstructionsFuture;
use codex_extension_api::LoadedUserInstructions;
use codex_extension_api::UserInstructions;
use codex_extension_api::UserInstructionsProvider;
use codex_utils_absolute_path::AbsolutePathBuf;

const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";
const INSTRUCTIONS_DIR_NAME: &str = "instructions";
const INSTRUCTIONS_FILE_EXTENSION: &str = "md";

#[derive(Clone, Debug)]
struct HomeInstructionFile {
    text: String,
    source: AbsolutePathBuf,
}

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

    async fn load_from_codex_home(&self) -> LoadedUserInstructions {
        let mut warnings = Vec::new();

        if let Some(instructions) = self.load_agents_md(&mut warnings).await {
            return LoadedUserInstructions {
                instructions: Some(UserInstructions {
                    text: instructions.text,
                    source: instructions.source,
                }),
                warnings,
            };
        }

        let Some(instructions) = self.load_instructions_dir(&mut warnings).await else {
            return LoadedUserInstructions {
                instructions: None,
                warnings,
            };
        };

        LoadedUserInstructions {
            instructions: Some(UserInstructions {
                text: instructions.text,
                source: instructions.source,
            }),
            warnings,
        }
    }

    async fn load_agents_md(&self, warnings: &mut Vec<String>) -> Option<HomeInstructionFile> {
        for candidate in [LOCAL_AGENTS_MD_FILENAME, DEFAULT_AGENTS_MD_FILENAME] {
            let path = self.codex_home.join(candidate);
            match tokio::fs::metadata(path.as_path()).await {
                Ok(metadata) if !metadata.is_file() => continue,
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global AGENTS.md instructions from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            }
            let data = match tokio::fs::read(path.as_path()).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global AGENTS.md instructions from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            };
            let contents = String::from_utf8_lossy(&data);
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return Some(HomeInstructionFile {
                    text: trimmed.to_string(),
                    source: path,
                });
            }
        }

        None
    }

    async fn load_instructions_dir(
        &self,
        warnings: &mut Vec<String>,
    ) -> Option<HomeInstructionFile> {
        let instructions_dir = self.codex_home.join(INSTRUCTIONS_DIR_NAME);
        match tokio::fs::symlink_metadata(instructions_dir.as_path()).await {
            Ok(metadata) if !metadata.is_dir() => return None,
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
            Err(err) => {
                warnings.push(format!(
                    "Failed to read global instructions directory from `{}`: {err}",
                    instructions_dir.display()
                ));
                return None;
            }
        }

        let mut pending_dirs = vec![instructions_dir.as_path().to_path_buf()];
        let mut candidates = Vec::new();
        while let Some(dir) = pending_dirs.pop() {
            let mut read_dir = match tokio::fs::read_dir(&dir).await {
                Ok(read_dir) => read_dir,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global instructions directory from `{}`: {err}",
                        dir.display()
                    ));
                    continue;
                }
            };

            loop {
                match read_dir.next_entry().await {
                    Ok(Some(entry)) => {
                        let path = entry.path();
                        let metadata = match tokio::fs::symlink_metadata(&path).await {
                            Ok(metadata) => metadata,
                            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                            Err(err) => {
                                warnings.push(format!(
                                    "Failed to read global instruction path from `{}`: {err}",
                                    path.display()
                                ));
                                continue;
                            }
                        };
                        let file_type = metadata.file_type();
                        if file_type.is_symlink() {
                            continue;
                        }
                        if file_type.is_dir() {
                            pending_dirs.push(path);
                        } else if file_type.is_file()
                            && is_markdown_file(&path)
                            && let Ok(relative_path) = path
                                .strip_prefix(instructions_dir.as_path())
                                .map(Path::to_path_buf)
                        {
                            candidates.push((relative_path, path));
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        warnings.push(format!(
                            "Failed to read global instructions directory entry from `{}`: {err}",
                            dir.display()
                        ));
                        break;
                    }
                }
            }
        }
        candidates.sort_by(|(left, _), (right, _)| left.cmp(right));

        let mut files = Vec::new();
        for (_relative_path, path) in candidates {
            let Ok(source) = AbsolutePathBuf::try_from(path.clone()) else {
                warnings.push(format!(
                    "Failed to read global instruction file from `{}`: path is not absolute",
                    path.display()
                ));
                continue;
            };

            match tokio::fs::metadata(&path).await {
                Ok(metadata) if !metadata.is_file() => continue,
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global instruction file from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            }

            let data = match tokio::fs::read(&path).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => {
                    warnings.push(format!(
                        "Failed to read global instruction file from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            };
            let contents = String::from_utf8_lossy(&data);
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                files.push(HomeInstructionFile {
                    text: trimmed.to_string(),
                    source,
                });
            }
        }

        self.combine_instruction_files(files)
    }

    fn combine_instruction_files(
        &self,
        files: Vec<HomeInstructionFile>,
    ) -> Option<HomeInstructionFile> {
        let mut files = files.into_iter();
        let first = files.next()?;
        let mut combined = first.text;
        let mut source = first.source;
        let mut has_multiple_sources = false;

        for file in files {
            combined.push_str("\n\n");
            combined.push_str(&file.text);
            has_multiple_sources = true;
        }

        if has_multiple_sources {
            source = self.codex_home.join(INSTRUCTIONS_DIR_NAME);
        }

        Some(HomeInstructionFile {
            text: combined,
            source,
        })
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some(INSTRUCTIONS_FILE_EXTENSION)
}

impl UserInstructionsProvider for CodexHomeUserInstructionsProvider {
    fn load_user_instructions(&self) -> LoadUserInstructionsFuture<'_> {
        Box::pin(self.load_from_codex_home())
    }
}

#[cfg(test)]
mod tests;
