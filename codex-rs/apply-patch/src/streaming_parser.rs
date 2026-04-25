use std::path::PathBuf;

use crate::parser::ADD_FILE_MARKER;
use crate::parser::BEGIN_PATCH_MARKER;
use crate::parser::CHANGE_CONTEXT_MARKER;
use crate::parser::DELETE_FILE_MARKER;
use crate::parser::EMPTY_CHANGE_CONTEXT_MARKER;
use crate::parser::END_PATCH_MARKER;
use crate::parser::EOF_MARKER;
use crate::parser::Hunk;
use crate::parser::MOVE_TO_MARKER;
use crate::parser::ParseError;
use crate::parser::UPDATE_FILE_MARKER;
use crate::parser::UpdateFileChunk;

use Hunk::*;
use ParseError::*;

#[derive(Debug, Default, Clone)]
pub struct StreamingPatchParser {
    line_buffer: String,
    state: StreamingParserState,
    line_number: usize,
}

#[derive(Debug, Default, Clone)]
struct StreamingParserState {
    mode: StreamingParserMode,
    hunks: Vec<Hunk>,
}

#[derive(Debug, Default, Clone)]
enum StreamingParserMode {
    #[default]
    NotStarted,
    StartedPatch,
    AddFile,
    DeleteFile,
    UpdateFile,
    EndedPatch,
}

fn handle_hunk_headers_and_end_patch(
    trimmed: &str,
    hunks: &mut Vec<Hunk>,
) -> Option<StreamingParserMode> {
    if trimmed == END_PATCH_MARKER {
        return Some(StreamingParserMode::EndedPatch);
    }
    if let Some(path) = trimmed.strip_prefix(ADD_FILE_MARKER) {
        hunks.push(AddFile {
            path: PathBuf::from(path),
            contents: String::new(),
        });
        return Some(StreamingParserMode::AddFile);
    }
    if let Some(path) = trimmed.strip_prefix(DELETE_FILE_MARKER) {
        hunks.push(DeleteFile {
            path: PathBuf::from(path),
        });
        return Some(StreamingParserMode::DeleteFile);
    }
    if let Some(path) = trimmed.strip_prefix(UPDATE_FILE_MARKER) {
        hunks.push(UpdateFile {
            path: PathBuf::from(path),
            move_path: None,
            chunks: Vec::new(),
        });
        return Some(StreamingParserMode::UpdateFile);
    }
    None
}

impl StreamingPatchParser {
    pub fn push_delta(&mut self, delta: &str) -> Result<Option<Vec<Hunk>>, ParseError> {
        for ch in delta.chars() {
            if ch == '\n' {
                let line = std::mem::take(&mut self.line_buffer);
                let state = std::mem::take(&mut self.state);
                self.line_number += 1;
                self.state =
                    Self::process_line(state, line.trim_end_matches('\r'), self.line_number)?;
            } else {
                self.line_buffer.push(ch);
            }
        }

        let hunks = self.state.hunks.clone();
        Ok(if hunks.is_empty() { None } else { Some(hunks) })
    }

    fn process_line(
        state: StreamingParserState,
        line: &str,
        line_number: usize,
    ) -> Result<StreamingParserState, ParseError> {
        let trimmed = line.trim();
        let StreamingParserState {
            mut mode,
            mut hunks,
        } = state;
        mode = match mode {
            StreamingParserMode::NotStarted => {
                if trimmed == BEGIN_PATCH_MARKER {
                    return Ok(StreamingParserState {
                        mode: StreamingParserMode::StartedPatch,
                        hunks,
                    });
                }
                return Err(InvalidPatchError(
                    "The first line of the patch must be '*** Begin Patch'".to_string(),
                ));
            }
            StreamingParserMode::StartedPatch => {
                if let Some(mode) = handle_hunk_headers_and_end_patch(trimmed, &mut hunks) {
                    return Ok(StreamingParserState { mode, hunks });
                }
                return Err(InvalidHunkError {
                    message: format!(
                        "'{trimmed}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
                    ),
                    line_number,
                });
            }
            StreamingParserMode::AddFile => {
                if let Some(mode) = handle_hunk_headers_and_end_patch(trimmed, &mut hunks) {
                    return Ok(StreamingParserState { mode, hunks });
                }
                if let Some(line_to_add) = line.strip_prefix('+')
                    && let Some(AddFile { contents, .. }) = hunks.last_mut()
                {
                    contents.push_str(line_to_add);
                    contents.push('\n');
                    return Ok(StreamingParserState {
                        mode: StreamingParserMode::AddFile,
                        hunks,
                    });
                }
                return Err(InvalidHunkError {
                    message: format!(
                        "Unexpected line found in add file hunk: '{line}'. Every line should start with '+'"
                    ),
                    line_number,
                });
            }
            StreamingParserMode::DeleteFile => {
                if let Some(mode) = handle_hunk_headers_and_end_patch(trimmed, &mut hunks) {
                    return Ok(StreamingParserState { mode, hunks });
                }
                return Err(InvalidHunkError {
                    message: format!(
                        "'{trimmed}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
                    ),
                    line_number,
                });
            }
            StreamingParserMode::UpdateFile => {
                if let Some(mode) = handle_hunk_headers_and_end_patch(trimmed, &mut hunks) {
                    return Ok(StreamingParserState { mode, hunks });
                }

                if let Some(UpdateFile {
                    move_path, chunks, ..
                }) = hunks.last_mut()
                {
                    if chunks.is_empty()
                        && move_path.is_none()
                        && let Some(move_to_path) = line.trim().strip_prefix(MOVE_TO_MARKER)
                    {
                        *move_path = Some(PathBuf::from(move_to_path));
                        return Ok(StreamingParserState {
                            mode: StreamingParserMode::UpdateFile,
                            hunks,
                        });
                    }

                    match line.trim() {
                        EMPTY_CHANGE_CONTEXT_MARKER => {
                            chunks.push(UpdateFileChunk {
                                change_context: None,
                                old_lines: Vec::new(),
                                new_lines: Vec::new(),
                                is_end_of_file: false,
                            });
                            return Ok(StreamingParserState {
                                mode: StreamingParserMode::UpdateFile,
                                hunks,
                            });
                        }
                        line => {
                            if let Some(change_context) = line.strip_prefix(CHANGE_CONTEXT_MARKER) {
                                chunks.push(UpdateFileChunk {
                                    change_context: Some(change_context.to_string()),
                                    old_lines: Vec::new(),
                                    new_lines: Vec::new(),
                                    is_end_of_file: false,
                                });
                                return Ok(StreamingParserState {
                                    mode: StreamingParserMode::UpdateFile,
                                    hunks,
                                });
                            }
                        }
                    }

                    if trimmed == EOF_MARKER {
                        if let Some(chunk) = chunks.last_mut() {
                            chunk.is_end_of_file = true;
                        }
                        return Ok(StreamingParserState {
                            mode: StreamingParserMode::UpdateFile,
                            hunks,
                        });
                    }

                    let chunk = if chunks.is_empty() {
                        chunks.push(UpdateFileChunk {
                            change_context: None,
                            old_lines: Vec::new(),
                            new_lines: Vec::new(),
                            is_end_of_file: false,
                        });
                        chunks.last_mut()
                    } else {
                        chunks.last_mut()
                    };
                    if let Some(chunk) = chunk {
                        let parsed_update_line = match line.chars().next() {
                            None => {
                                chunk.old_lines.push(String::new());
                                chunk.new_lines.push(String::new());
                                true
                            }
                            Some(' ') => {
                                chunk.old_lines.push(line[1..].to_string());
                                chunk.new_lines.push(line[1..].to_string());
                                true
                            }
                            Some('+') => {
                                chunk.new_lines.push(line[1..].to_string());
                                true
                            }
                            Some('-') => {
                                chunk.old_lines.push(line[1..].to_string());
                                true
                            }
                            Some(_) => false,
                        };
                        if parsed_update_line {
                            return Ok(StreamingParserState {
                                mode: StreamingParserMode::UpdateFile,
                                hunks,
                            });
                        }
                    }
                }
                return Err(InvalidHunkError {
                    message: format!(
                        "Unexpected line found in update hunk: '{line}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)"
                    ),
                    line_number,
                });
            }
            StreamingParserMode::EndedPatch => mode,
        };
        Ok(StreamingParserState { mode, hunks })
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_streaming_patch_parser_streams_complete_lines_before_end_patch() {
        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta("*** Begin Patch\n*** Add File: src/hello.txt\n+hello\n+wor"),
            Ok(Some(vec![AddFile {
                path: PathBuf::from("src/hello.txt"),
                contents: "hello\n".to_string(),
            }]))
        );
        assert_eq!(
            parser.push_delta("ld\n"),
            Ok(Some(vec![AddFile {
                path: PathBuf::from("src/hello.txt"),
                contents: "hello\nworld\n".to_string(),
            }]))
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta(
                "*** Begin Patch\n*** Update File: src/old.rs\n*** Move to: src/new.rs\n@@\n-old\n+new\n",
            ),
            Ok(Some(vec![UpdateFile {
                path: PathBuf::from("src/old.rs"),
                move_path: Some(PathBuf::from("src/new.rs")),
                chunks: vec![UpdateFileChunk {
                    change_context: None,
                    old_lines: vec!["old".to_string()],
                    new_lines: vec!["new".to_string()],
                    is_end_of_file: false,
                }],
            }]))
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta("*** Begin Patch\n*** Delete File: gone.txt"),
            Ok(None)
        );
        assert_eq!(
            parser.push_delta("\n"),
            Ok(Some(vec![DeleteFile {
                path: PathBuf::from("gone.txt"),
            }]))
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta(
                "*** Begin Patch\n*** Add File: src/one.txt\n+one\n*** Delete File: src/two.txt\n",
            ),
            Ok(Some(vec![
                AddFile {
                    path: PathBuf::from("src/one.txt"),
                    contents: "one\n".to_string(),
                },
                DeleteFile {
                    path: PathBuf::from("src/two.txt"),
                },
            ]))
        );
    }

    #[test]
    fn test_streaming_patch_parser_large_patch_split_by_character() {
        let patch = "\
*** Begin Patch
*** Add File: docs/release-notes.md
+# Release notes
+
+## CLI
+- Surface apply_patch progress while arguments stream.
+- Keep final patch application gated on the completed tool call.
+- Include file summaries in the progress event payload.
*** Update File: src/config.rs
@@ impl Config
-    pub apply_patch_progress: bool,
+    pub stream_apply_patch_progress: bool,
     pub include_diagnostics: bool,
@@ fn default_progress_interval()
-    Duration::from_millis(500)
+    Duration::from_millis(250)
*** Delete File: src/legacy_patch_progress.rs
*** Update File: crates/cli/src/main.rs
*** Move to: crates/cli/src/bin/codex.rs
@@ fn run()
-    let args = Args::parse();
-    dispatch(args)
+    let cli = Cli::parse();
+    dispatch(cli)
*** Add File: tests/fixtures/apply_patch_progress.json
+{
+  \"type\": \"apply_patch_progress\",
+  \"hunks\": [
+    { \"operation\": \"add\", \"path\": \"docs/release-notes.md\" },
+    { \"operation\": \"update\", \"path\": \"src/config.rs\" }
+  ]
+}
*** Update File: README.md
@@ Development workflow
 Build the Rust workspace before opening a pull request.
+When touching streamed tool calls, include parser coverage for partial input.
+Prefer tests that exercise the exact event payload shape.
*** Delete File: docs/old-apply-patch-progress.md
*** End Patch";

        let mut parser = StreamingPatchParser::default();
        let mut max_hunk_count = 0;
        let mut saw_hunk_counts = Vec::new();
        let mut hunks = Vec::new();
        for ch in patch.chars() {
            if let Some(updated_hunks) = parser.push_delta(&ch.to_string()).unwrap() {
                let hunk_count = updated_hunks.len();
                assert!(
                    hunk_count >= max_hunk_count,
                    "hunk count should never decrease while streaming: {hunk_count} < {max_hunk_count}",
                );
                if hunk_count > max_hunk_count {
                    saw_hunk_counts.push(hunk_count);
                    max_hunk_count = hunk_count;
                }
                hunks = updated_hunks;
            }
        }

        assert_eq!(saw_hunk_counts, vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(hunks.len(), 7);
        assert_eq!(
            hunks
                .iter()
                .map(|hunk| match hunk {
                    AddFile { .. } => "add",
                    DeleteFile { .. } => "delete",
                    UpdateFile {
                        move_path: Some(_), ..
                    } => "move-update",
                    UpdateFile {
                        move_path: None, ..
                    } => "update",
                })
                .collect::<Vec<_>>(),
            vec![
                "add",
                "update",
                "delete",
                "move-update",
                "add",
                "update",
                "delete"
            ]
        );
    }

    #[test]
    fn test_streaming_patch_parser_returns_errors() {
        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta("bad\n"),
            Err(InvalidPatchError(
                "The first line of the patch must be '*** Begin Patch'".to_string(),
            ))
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(parser.push_delta("*** Begin Patch\n"), Ok(None));
        assert_eq!(
            parser.push_delta("bad\n"),
            Err(InvalidHunkError {
                message: "'bad' is not a valid hunk header. Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'"
                    .to_string(),
                line_number: 2,
            })
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta("*** Begin Patch\n*** Add File: file.txt\nbad\n"),
            Err(InvalidHunkError {
                message:
                    "Unexpected line found in add file hunk: 'bad'. Every line should start with '+'"
                        .to_string(),
                line_number: 3,
            })
        );

        let mut parser = StreamingPatchParser::default();
        assert_eq!(
            parser.push_delta("*** Begin Patch\n*** Delete File: file.txt\nbad\n"),
            Err(InvalidHunkError {
                message: "'bad' is not a valid hunk header. Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'"
                    .to_string(),
                line_number: 3,
            })
        );
    }
}
