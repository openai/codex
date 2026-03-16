use std::collections::VecDeque;
use std::path::PathBuf;

use async_trait::async_trait;
use codex_utils_string::take_bytes_at_char_boundary;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ReadFileHandler;

const MAX_LINE_LENGTH: usize = 500;
const TAB_WIDTH: usize = 4;

// TODO(jif) add support for block comments
const COMMENT_PREFIXES: &[&str] = &["#", "//", "--"];

/// JSON arguments accepted by the `read_file` tool handler.
#[derive(Deserialize)]
struct ReadFileArgs {
    /// Absolute path to the file that will be read.
    file_path: String,
    /// 1-indexed line number to start reading from; defaults to 1.
    #[serde(default = "defaults::offset")]
    offset: usize,
    /// Maximum number of lines to return; defaults to 2000.
    #[serde(default = "defaults::limit")]
    limit: usize,
    /// Determines whether the handler reads a simple slice or indentation-aware block.
    #[serde(default)]
    mode: ReadMode,
    /// Optional indentation configuration used when `mode` is `Indentation`.
    #[serde(default)]
    indentation: Option<IndentationArgs>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum ReadMode {
    #[default]
    Slice,
    Indentation,
}
/// Additional configuration for indentation-aware reads.
#[derive(Deserialize, Clone)]
struct IndentationArgs {
    /// Optional explicit anchor line; defaults to `offset` when omitted.
    #[serde(default)]
    anchor_line: Option<usize>,
    /// Maximum indentation depth to collect; `0` means unlimited.
    #[serde(default = "defaults::max_levels")]
    max_levels: usize,
    /// Whether to include sibling blocks at the same indentation level.
    #[serde(default = "defaults::include_siblings")]
    include_siblings: bool,
    /// Whether to include header lines above the anchor block. This made on a best effort basis.
    #[serde(default = "defaults::include_header")]
    include_header: bool,
    /// Optional hard cap on returned lines; defaults to the global `limit`.
    #[serde(default)]
    max_lines: Option<usize>,
}

#[derive(Clone, Debug)]
struct LineRecord {
    number: usize,
    raw: String,
    display: String,
    indent: usize,
}

impl LineRecord {
    fn trimmed(&self) -> &str {
        self.raw.trim_start()
    }

    fn is_blank(&self) -> bool {
        self.trimmed().is_empty()
    }

    fn is_comment(&self) -> bool {
        COMMENT_PREFIXES
            .iter()
            .any(|prefix| self.raw.trim().starts_with(prefix))
    }
}

#[async_trait]
impl ToolHandler for ReadFileHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "read_file handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ReadFileArgs = parse_arguments(&arguments)?;

        let ReadFileArgs {
            file_path,
            offset,
            limit,
            mode,
            indentation,
        } = args;

        if offset == 0 {
            return Err(FunctionCallError::RespondToModel(
                "offset must be a 1-indexed line number".to_string(),
            ));
        }

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let path = PathBuf::from(&file_path);
        if !path.is_absolute() {
            return Err(FunctionCallError::RespondToModel(
                "file_path must be an absolute path".to_string(),
            ));
        }

        let collected = match mode {
            ReadMode::Slice => slice::read(&path, offset, limit).await?,
            ReadMode::Indentation => {
                let indentation = indentation.unwrap_or_default();
                indentation::read_block(&path, offset, limit, indentation).await?
            }
        };
        Ok(FunctionToolOutput::from_text(
            collected.join("\n"),
            Some(true),
        ))
    }
}

mod slice {
    use crate::function_tool::FunctionCallError;
    use crate::tools::handlers::read_file::format_line;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::BufReader;

    pub async fn read(
        path: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>, FunctionCallError> {
        let file = File::open(path).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to read file: {err}"))
        })?;

        let mut reader = BufReader::new(file);
        let mut collected = Vec::new();
        let mut seen = 0usize;
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer).await.map_err(|err| {
                FunctionCallError::RespondToModel(format!("failed to read file: {err}"))
            })?;

            if bytes_read == 0 {
                break;
            }

            if buffer.last() == Some(&b'\n') {
                buffer.pop();
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
            }

            seen += 1;

            if seen < offset {
                continue;
            }

            if collected.len() == limit {
                break;
            }

            let formatted = format_line(&buffer);
            collected.push(format!("L{seen}: {formatted}"));

            if collected.len() == limit {
                break;
            }
        }

        if seen < offset {
            return Err(FunctionCallError::RespondToModel(
                "offset exceeds file length".to_string(),
            ));
        }

        Ok(collected)
    }
}

mod indentation {
    use crate::function_tool::FunctionCallError;
    use crate::tools::handlers::read_file::IndentationArgs;
    use crate::tools::handlers::read_file::LineRecord;
    use crate::tools::handlers::read_file::TAB_WIDTH;
    use crate::tools::handlers::read_file::format_line;
    use crate::tools::handlers::read_file::trim_empty_lines;
    use std::collections::VecDeque;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::BufReader;

    pub async fn read_block(
        path: &Path,
        offset: usize,
        limit: usize,
        options: IndentationArgs,
    ) -> Result<Vec<String>, FunctionCallError> {
        let anchor_line = options.anchor_line.unwrap_or(offset);
        if anchor_line == 0 {
            return Err(FunctionCallError::RespondToModel(
                "anchor_line must be a 1-indexed line number".to_string(),
            ));
        }

        let guard_limit = options.max_lines.unwrap_or(limit);
        if guard_limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "max_lines must be greater than zero".to_string(),
            ));
        }

        let collected = collect_file_lines(path).await?;
        if collected.is_empty() || anchor_line > collected.len() {
            return Err(FunctionCallError::RespondToModel(
                "anchor_line exceeds file length".to_string(),
            ));
        }

        let anchor_index = anchor_line - 1;
        let effective_indents = compute_effective_indents(&collected);
        let anchor_indent = effective_indents[anchor_index];

        // Compute the min indent
        let min_indent = if options.max_levels == 0 {
            0
        } else {
            anchor_indent.saturating_sub(options.max_levels * TAB_WIDTH)
        };

        // Cap requested lines by guard_limit and file length
        let final_limit = limit.min(guard_limit).min(collected.len());

        if final_limit == 1 {
            return Ok(vec![format!(
                "L{}: {}",
                collected[anchor_index].number, collected[anchor_index].display
            )]);
        }

        // Cursors
        let mut i: isize = anchor_index as isize - 1; // up (inclusive)
        let mut j: usize = anchor_index + 1; // down (inclusive)
        let mut i_counter_min_indent = 0;
        let mut j_counter_min_indent = 0;

        let mut out = VecDeque::with_capacity(limit);
        out.push_back(&collected[anchor_index]);

        while out.len() < final_limit {
            let mut progressed = 0;

            // Up.
            if i >= 0 {
                let iu = i as usize;
                if effective_indents[iu] >= min_indent {
                    out.push_front(&collected[iu]);
                    progressed += 1;
                    i -= 1;

                    // We do not include the siblings (not applied to comments).
                    if effective_indents[iu] == min_indent && !options.include_siblings {
                        let allow_header_comment =
                            options.include_header && collected[iu].is_comment();
                        let can_take_line = allow_header_comment || i_counter_min_indent == 0;

                        if can_take_line {
                            i_counter_min_indent += 1;
                        } else {
                            // This line shouldn't have been taken.
                            out.pop_front();
                            progressed -= 1;
                            i = -1; // consider using Option<usize> or a control flag instead of a sentinel
                        }
                    }

                    // Short-cut.
                    if out.len() >= final_limit {
                        break;
                    }
                } else {
                    // Stop moving up.
                    i = -1;
                }
            }

            // Down.
            if j < collected.len() {
                let ju = j;
                if effective_indents[ju] >= min_indent {
                    out.push_back(&collected[ju]);
                    progressed += 1;
                    j += 1;

                    // We do not include the siblings (applied to comments).
                    if effective_indents[ju] == min_indent && !options.include_siblings {
                        if j_counter_min_indent > 0 {
                            // This line shouldn't have been taken.
                            out.pop_back();
                            progressed -= 1;
                            j = collected.len();
                        }
                        j_counter_min_indent += 1;
                    }
                } else {
                    // Stop moving down.
                    j = collected.len();
                }
            }

            if progressed == 0 {
                break;
            }
        }

        // Trim empty lines
        trim_empty_lines(&mut out);

        Ok(out
            .into_iter()
            .map(|record| format!("L{}: {}", record.number, record.display))
            .collect())
    }

    async fn collect_file_lines(path: &Path) -> Result<Vec<LineRecord>, FunctionCallError> {
        let file = File::open(path).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to read file: {err}"))
        })?;

        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        let mut lines = Vec::new();
        let mut number = 0usize;

        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer).await.map_err(|err| {
                FunctionCallError::RespondToModel(format!("failed to read file: {err}"))
            })?;

            if bytes_read == 0 {
                break;
            }

            if buffer.last() == Some(&b'\n') {
                buffer.pop();
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
            }

            number += 1;
            let raw = String::from_utf8_lossy(&buffer).into_owned();
            let indent = measure_indent(&raw);
            let display = format_line(&buffer);
            lines.push(LineRecord {
                number,
                raw,
                display,
                indent,
            });
        }

        Ok(lines)
    }

    fn compute_effective_indents(records: &[LineRecord]) -> Vec<usize> {
        let mut effective = Vec::with_capacity(records.len());
        let mut previous_indent = 0usize;
        for record in records {
            if record.is_blank() {
                effective.push(previous_indent);
            } else {
                previous_indent = record.indent;
                effective.push(previous_indent);
            }
        }
        effective
    }

    fn measure_indent(line: &str) -> usize {
        line.chars()
            .take_while(|c| matches!(c, ' ' | '\t'))
            .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
            .sum()
    }
}

fn format_line(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    if decoded.len() > MAX_LINE_LENGTH {
        take_bytes_at_char_boundary(&decoded, MAX_LINE_LENGTH).to_string()
    } else {
        decoded.into_owned()
    }
}

fn trim_empty_lines(out: &mut VecDeque<&LineRecord>) {
    while matches!(out.front(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_front();
    }
    while matches!(out.back(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_back();
    }
}

mod defaults {
    use super::*;

    impl Default for IndentationArgs {
        fn default() -> Self {
            Self {
                anchor_line: None,
                max_levels: max_levels(),
                include_siblings: include_siblings(),
                include_header: include_header(),
                max_lines: None,
            }
        }
    }

    pub fn offset() -> usize {
        1
    }

    pub fn limit() -> usize {
        2000
    }

    pub fn max_levels() -> usize {
        0
    }

    pub fn include_siblings() -> bool {
        false
    }

    pub fn include_header() -> bool {
        true
    }
}

#[cfg(test)]
#[path = "read_file_tests.rs"]
mod tests;
