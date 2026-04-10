//! This module is responsible for parsing & validating a patch into a list of "hunks".
//! (It does not attempt to actually check that the patch can be applied to the filesystem.)
//!
//! The official Lark grammar for the apply-patch format is:
//!
//! start: begin_patch hunk+ end_patch
//! begin_patch: "*** Begin Patch" LF
//! end_patch: "*** End Patch" LF?
//!
//! hunk: add_hunk | delete_hunk | update_hunk
//! add_hunk: "*** Add File: " filename LF add_line+
//! delete_hunk: "*** Delete File: " filename LF
//! update_hunk: "*** Update File: " filename LF change_move? change?
//! filename: /(.+)/
//! add_line: "+" /(.+)/ LF -> line
//!
//! change_move: "*** Move to: " filename LF
//! change: (change_context | change_line)+ eof_line?
//! change_context: ("@@" | "@@ " /(.+)/) LF
//! change_line: ("+" | "-" | " ") /(.+)/ LF
//! eof_line: "*** End of File" LF
//!
//! The parser below is a little more lenient than the explicit spec and allows for
//! leading/trailing whitespace around patch markers.
use crate::ApplyPatchArgs;
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use codex_utils_absolute_path::test_support::PathBufExt;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

pub(crate) const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
pub(crate) const END_PATCH_MARKER: &str = "*** End Patch";
pub(crate) const ADD_FILE_MARKER: &str = "*** Add File: ";
pub(crate) const DELETE_FILE_MARKER: &str = "*** Delete File: ";
pub(crate) const UPDATE_FILE_MARKER: &str = "*** Update File: ";
pub(crate) const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

/// Currently, the only OpenAI model that knowingly requires lenient parsing is
/// gpt-4.1. While we could try to require everyone to pass in a strictness
/// param when invoking apply_patch, it is a pain to thread it through all of
/// the call sites, so we resign ourselves allowing lenient parsing for all
/// models. See [`ParseMode::Lenient`] for details on the exceptions we make for
/// gpt-4.1.
const PARSE_IN_STRICT_MODE: bool = false;

#[derive(Debug, PartialEq, Error, Clone)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatchError(String),
    #[error("invalid hunk at line {line_number}, {message}")]
    InvalidHunkError { message: String, line_number: usize },
}
use ParseError::*;

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,

        /// Chunks should be in order, i.e. the `change_context` of one chunk
        /// should occur later in the file than the previous chunk.
        chunks: Vec<UpdateFileChunk>,
    },
}

impl Hunk {
    pub fn resolve_path(&self, cwd: &AbsolutePathBuf) -> AbsolutePathBuf {
        let path = match self {
            Hunk::UpdateFile { path, .. } => path,
            Hunk::AddFile { .. } | Hunk::DeleteFile { .. } => self.path(),
        };
        AbsolutePathBuf::resolve_path_against_base(path, cwd)
    }

    /// Returns the path affected by this hunk, using the move destination for rename hunks.
    pub fn path(&self) -> &Path {
        match self {
            Hunk::AddFile { path, .. } => path,
            Hunk::DeleteFile { path } => path,
            Hunk::UpdateFile {
                move_path: Some(path),
                ..
            } => path,
            Hunk::UpdateFile {
                path,
                move_path: None,
                ..
            } => path,
        }
    }
}

use Hunk::*;

#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    /// A single line of context used to narrow down the position of the chunk
    /// (this is usually a class, method, or function definition.)
    pub change_context: Option<String>,

    /// A contiguous block of lines that should be replaced with `new_lines`.
    /// `old_lines` must occur strictly after `change_context`.
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,

    /// If set to true, `old_lines` must occur at the end of the source file.
    /// (Tolerance around trailing newlines should be encouraged.)
    pub is_end_of_file: bool,
}

pub fn parse_patch(patch: &str) -> Result<ApplyPatchArgs, ParseError> {
    let mode = if PARSE_IN_STRICT_MODE {
        ParseMode::Strict
    } else {
        ParseMode::Lenient
    };
    parse_patch_text(patch, mode)
}

enum ParseMode {
    /// Parse the patch text argument as is.
    Strict,

    /// GPT-4.1 is known to formulate the `command` array for the `local_shell`
    /// tool call for `apply_patch` call using something like the following:
    ///
    /// ```json
    /// [
    ///   "apply_patch",
    ///   "<<'EOF'\n*** Begin Patch\n*** Update File: README.md\n@@...\n*** End Patch\nEOF\n",
    /// ]
    /// ```
    ///
    /// This is a problem because `local_shell` is a bit of a misnomer: the
    /// `command` is not invoked by passing the arguments to a shell like Bash,
    /// but are invoked using something akin to `execvpe(3)`.
    ///
    /// This is significant in this case because where a shell would interpret
    /// `<<'EOF'...` as a heredoc and pass the contents via stdin (which is
    /// fine, as `apply_patch` is specified to read from stdin if no argument is
    /// passed), `execvpe(3)` interprets the heredoc as a literal string. To get
    /// the `local_shell` tool to run a command the way shell would, the
    /// `command` array must be something like:
    ///
    /// ```json
    /// [
    ///   "bash",
    ///   "-lc",
    ///   "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: README.md\n@@...\n*** End Patch\nEOF\n",
    /// ]
    /// ```
    ///
    /// In lenient mode, we check if the argument to `apply_patch` starts with
    /// `<<'EOF'` and ends with `EOF\n`. If so, we strip off these markers,
    /// trim() the result, and treat what is left as the patch text.
    Lenient,

    /// Parse partial patch text for progress reporting while the model is
    /// still streaming tool input. This mode requires a begin marker but does
    /// not require an end marker, and its output must not be used to apply a
    /// patch.
    Streaming,
}

fn parse_patch_text(patch: &str, mode: ParseMode) -> Result<ApplyPatchArgs, ParseError> {
    let lines: Vec<&str> = patch.trim().lines().collect();
    let lines: &[&str] = match check_patch_boundaries_strict(&lines) {
        Ok(()) => &lines,
        Err(e) => match mode {
            ParseMode::Strict => {
                return Err(e);
            }
            ParseMode::Lenient => check_patch_boundaries_lenient(&lines, e)?,
            ParseMode::Streaming => check_patch_boundaries_streaming(&lines, e)?,
        },
    };

    let mut hunks: Vec<Hunk> = Vec::new();
    // The above checks ensure that lines.len() >= 2 for strict/lenient mode.
    // Streaming mode may omit the end marker, so its body extends through the
    // final line unless the final line is an explicit end marker.
    let body_end_index = if matches!(mode, ParseMode::Streaming)
        && lines
            .last()
            .is_some_and(|line| line.trim() != END_PATCH_MARKER)
    {
        lines.len()
    } else {
        lines.len().saturating_sub(1)
    };
    let mut remaining_lines = &lines[1..body_end_index];
    let mut line_number = 2;
    while !remaining_lines.is_empty() {
        let parsed_hunk = parse_one_hunk(remaining_lines, line_number);
        let (hunk, hunk_lines) = match parsed_hunk {
            Ok(parsed) => parsed,
            Err(err) if matches!(mode, ParseMode::Streaming) => {
                if hunks.is_empty() {
                    return Err(err);
                }
                break;
            }
            Err(err) => return Err(err),
        };
        hunks.push(hunk);
        line_number += hunk_lines;
        remaining_lines = &remaining_lines[hunk_lines..]
    }
    let patch = lines.join("\n");
    Ok(ApplyPatchArgs {
        hunks,
        patch,
        workdir: None,
    })
}

fn check_patch_boundaries_streaming<'a>(
    original_lines: &'a [&'a str],
    original_parse_error: ParseError,
) -> Result<&'a [&'a str], ParseError> {
    match original_lines {
        [first, ..] if first.trim() == BEGIN_PATCH_MARKER => Ok(original_lines),
        _ => Err(original_parse_error),
    }
}

/// Checks the start and end lines of the patch text for `apply_patch`,
/// returning an error if they do not match the expected markers.
fn check_patch_boundaries_strict(lines: &[&str]) -> Result<(), ParseError> {
    let (first_line, last_line) = match lines {
        [] => (None, None),
        [first] => (Some(first), Some(first)),
        [first, .., last] => (Some(first), Some(last)),
    };
    check_start_and_end_lines_strict(first_line, last_line)
}

/// If we are in lenient mode, we check if the first line starts with `<<EOF`
/// (possibly quoted) and the last line ends with `EOF`. There must be at least
/// 4 lines total because the heredoc markers take up 2 lines and the patch text
/// must have at least 2 lines.
///
/// If successful, returns the lines of the patch text that contain the patch
/// contents, excluding the heredoc markers.
fn check_patch_boundaries_lenient<'a>(
    original_lines: &'a [&'a str],
    original_parse_error: ParseError,
) -> Result<&'a [&'a str], ParseError> {
    match original_lines {
        [first, .., last] => {
            if (first == &"<<EOF" || first == &"<<'EOF'" || first == &"<<\"EOF\"")
                && last.ends_with("EOF")
                && original_lines.len() >= 4
            {
                let inner_lines = &original_lines[1..original_lines.len() - 1];
                match check_patch_boundaries_strict(inner_lines) {
                    Ok(()) => Ok(inner_lines),
                    Err(e) => Err(e),
                }
            } else {
                Err(original_parse_error)
            }
        }
        _ => Err(original_parse_error),
    }
}

fn check_start_and_end_lines_strict(
    first_line: Option<&&str>,
    last_line: Option<&&str>,
) -> Result<(), ParseError> {
    let first_line = first_line.map(|line| line.trim());
    let last_line = last_line.map(|line| line.trim());

    match (first_line, last_line) {
        (Some(first), Some(last)) if first == BEGIN_PATCH_MARKER && last == END_PATCH_MARKER => {
            Ok(())
        }
        (Some(first), _) if first != BEGIN_PATCH_MARKER => Err(InvalidPatchError(String::from(
            "The first line of the patch must be '*** Begin Patch'",
        ))),
        _ => Err(InvalidPatchError(String::from(
            "The last line of the patch must be '*** End Patch'",
        ))),
    }
}

/// Attempts to parse a single hunk from the start of lines.
/// Returns the parsed hunk and the number of lines parsed (or a ParseError).
fn parse_one_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), ParseError> {
    // Be tolerant of case mismatches and extra padding around marker strings.
    let first_line = lines[0].trim();
    if let Some(path) = first_line.strip_prefix(ADD_FILE_MARKER) {
        // Add File
        let mut contents = String::new();
        let mut parsed_lines = 1;
        for add_line in &lines[1..] {
            if let Some(line_to_add) = add_line.strip_prefix('+') {
                contents.push_str(line_to_add);
                contents.push('\n');
                parsed_lines += 1;
            } else {
                break;
            }
        }
        return Ok((
            AddFile {
                path: PathBuf::from(path),
                contents,
            },
            parsed_lines,
        ));
    } else if let Some(path) = first_line.strip_prefix(DELETE_FILE_MARKER) {
        // Delete File
        return Ok((
            DeleteFile {
                path: PathBuf::from(path),
            },
            1,
        ));
    } else if let Some(path) = first_line.strip_prefix(UPDATE_FILE_MARKER) {
        // Update File
        let mut remaining_lines = &lines[1..];
        let mut parsed_lines = 1;

        // Optional: move file line
        let move_path = remaining_lines
            .first()
            .and_then(|x| x.strip_prefix(MOVE_TO_MARKER));

        if move_path.is_some() {
            remaining_lines = &remaining_lines[1..];
            parsed_lines += 1;
        }

        let mut chunks = Vec::new();
        // NOTE: we need to know to stop once we reach the next special marker header.
        while !remaining_lines.is_empty() {
            // Skip over any completely blank lines that may separate chunks.
            if remaining_lines[0].trim().is_empty() {
                parsed_lines += 1;
                remaining_lines = &remaining_lines[1..];
                continue;
            }

            if remaining_lines[0].starts_with("***") {
                break;
            }

            let (chunk, chunk_lines) = parse_update_file_chunk(
                remaining_lines,
                line_number + parsed_lines,
                chunks.is_empty(),
            )?;
            chunks.push(chunk);
            parsed_lines += chunk_lines;
            remaining_lines = &remaining_lines[chunk_lines..]
        }

        if chunks.is_empty() {
            return Err(InvalidHunkError {
                message: format!("Update file hunk for path '{path}' is empty"),
                line_number,
            });
        }

        return Ok((
            UpdateFile {
                path: PathBuf::from(path),
                move_path: move_path.map(PathBuf::from),
                chunks,
            },
            parsed_lines,
        ));
    }

    Err(InvalidHunkError {
        message: format!(
            "'{first_line}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
        ),
        line_number,
    })
}

fn parse_update_file_chunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<(UpdateFileChunk, usize), ParseError> {
    if lines.is_empty() {
        return Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number,
        });
    }
    // If we see an explicit context marker @@ or @@ <context>, consume it; otherwise, optionally
    // allow treating the chunk as starting directly with diff lines.
    let (change_context, start_index) = if lines[0] == EMPTY_CHANGE_CONTEXT_MARKER {
        (None, 1)
    } else if let Some(context) = lines[0].strip_prefix(CHANGE_CONTEXT_MARKER) {
        (Some(context.to_string()), 1)
    } else {
        if !allow_missing_context {
            return Err(InvalidHunkError {
                message: format!(
                    "Expected update hunk to start with a @@ context marker, got: '{}'",
                    lines[0]
                ),
                line_number,
            });
        }
        (None, 0)
    };
    if start_index >= lines.len() {
        return Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number: line_number + 1,
        });
    }
    let mut chunk = UpdateFileChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };
    let mut parsed_lines = 0;
    for line in &lines[start_index..] {
        match *line {
            EOF_MARKER => {
                if parsed_lines == 0 {
                    return Err(InvalidHunkError {
                        message: "Update hunk does not contain any lines".to_string(),
                        line_number: line_number + 1,
                    });
                }
                chunk.is_end_of_file = true;
                parsed_lines += 1;
                break;
            }
            line_contents => {
                match line_contents.chars().next() {
                    None => {
                        // Interpret this as an empty line.
                        chunk.old_lines.push(String::new());
                        chunk.new_lines.push(String::new());
                    }
                    Some(' ') => {
                        chunk.old_lines.push(line_contents[1..].to_string());
                        chunk.new_lines.push(line_contents[1..].to_string());
                    }
                    Some('+') => {
                        chunk.new_lines.push(line_contents[1..].to_string());
                    }
                    Some('-') => {
                        chunk.old_lines.push(line_contents[1..].to_string());
                    }
                    _ => {
                        if parsed_lines == 0 {
                            return Err(InvalidHunkError {
                                message: format!(
                                    "Unexpected line found in update hunk: '{line_contents}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)"
                                ),
                                line_number: line_number + 1,
                            });
                        }
                        // Assume this is the start of the next hunk.
                        break;
                    }
                }
                parsed_lines += 1;
            }
        }
    }

    Ok((chunk, parsed_lines + start_index))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplyPatchProgress {
    pub changes: HashMap<PathBuf, ApplyPatchProgressChange>,
    pub active_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApplyPatchProgressChange {
    Add {
        content: String,
    },
    Delete,
    Update {
        unified_diff: String,
        move_path: Option<PathBuf>,
    },
}

/// Parses streamed `apply_patch` tool input into best-effort structured progress.
///
/// Unlike [`parse_patch`], this parser intentionally accepts incomplete input
/// via [`ParseMode::Streaming`] so callers can report progress while the model
/// is still writing the patch. The returned changes are not authoritative and
/// must not be used to apply a patch.
pub fn parse_apply_patch_tool_input_progress(input: &str) -> Option<ApplyPatchProgress> {
    let patch = extract_patch_text(input)?;
    let progress = parse_patch_progress_text(&patch, ParseMode::Streaming)?;
    if progress.changes.is_empty() && progress.active_path.is_none() {
        return None;
    }
    Some(progress)
}

fn extract_patch_text(input: &str) -> Option<String> {
    if let Some(patch) = extract_json_input_prefix(input)
        && patch.contains(BEGIN_PATCH_MARKER)
    {
        return Some(patch);
    }

    let start = input.find(BEGIN_PATCH_MARKER)?;
    Some(input[start..].to_string())
}

fn extract_json_input_prefix(input: &str) -> Option<String> {
    let key = "\"input\"";
    let mut search_start = 0;
    while let Some(offset) = input[search_start..].find(key) {
        let key_start = search_start + offset;
        let mut index = key_start + key.len();
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b':') {
            search_start = key_start + key.len();
            continue;
        }
        index += 1;
        index = skip_json_whitespace(input, index);
        if input.as_bytes().get(index) != Some(&b'"') {
            search_start = key_start + key.len();
            continue;
        }
        return Some(decode_json_string_prefix(&input[index + 1..]));
    }
    None
}

fn skip_json_whitespace(input: &str, mut index: usize) -> usize {
    while let Some(byte) = input.as_bytes().get(index)
        && byte.is_ascii_whitespace()
    {
        index += 1;
    }
    index
}

fn decode_json_string_prefix(input: &str) -> String {
    let mut decoded = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,
            '\\' => {
                let Some(escaped) = chars.next() else {
                    break;
                };
                match escaped {
                    '"' => decoded.push('"'),
                    '\\' => decoded.push('\\'),
                    '/' => decoded.push('/'),
                    'b' => decoded.push('\u{0008}'),
                    'f' => decoded.push('\u{000c}'),
                    'n' => decoded.push('\n'),
                    'r' => decoded.push('\r'),
                    't' => decoded.push('\t'),
                    'u' => {
                        let mut digits = String::with_capacity(4);
                        for _ in 0..4 {
                            let Some(digit) = chars.next() else {
                                return decoded;
                            };
                            digits.push(digit);
                        }
                        if let Ok(value) = u16::from_str_radix(&digits, 16)
                            && let Some(ch) = char::from_u32(u32::from(value))
                        {
                            decoded.push(ch);
                        }
                    }
                    other => decoded.push(other),
                }
            }
            other => decoded.push(other),
        }
    }
    decoded
}

fn parse_patch_progress_text(patch: &str, mode: ParseMode) -> Option<ApplyPatchProgress> {
    let ParseMode::Streaming = mode else {
        return None;
    };

    let segments = patch_line_segments(patch);
    let lines: Vec<&str> = segments.iter().map(|segment| segment.line).collect();
    check_patch_boundaries_streaming(
        &lines,
        InvalidPatchError(String::from(
            "The first line of the patch must be '*** Begin Patch'",
        )),
    )
    .ok()?;
    let body_end_index = if lines
        .last()
        .is_some_and(|line| line.trim() == END_PATCH_MARKER)
    {
        segments.len().saturating_sub(1)
    } else {
        segments.len()
    };
    let body_segments = &segments[1..body_end_index];
    let body_lines: Vec<&str> = body_segments.iter().map(|segment| segment.line).collect();
    let mut changes = HashMap::new();
    let mut active_path: Option<PathBuf> = None;

    let mut offset = 0;
    let mut line_number = 2;
    while offset < body_lines.len() {
        match parse_one_hunk(&body_lines[offset..], line_number) {
            Ok((hunk, hunk_lines)) => {
                let hunk_segments = &body_segments[offset..offset + hunk_lines];
                active_path = Some(hunk.path().to_path_buf());
                insert_progress_hunk(&mut changes, &hunk, hunk_segments);
                offset += hunk_lines;
                line_number += hunk_lines;
            }
            Err(_) => {
                if let Some((path, change, hunk_active_path)) =
                    parse_partial_progress_hunk(&body_segments[offset..])
                {
                    active_path = hunk_active_path.or_else(|| Some(path.clone()));
                    changes.insert(path, change);
                }
                break;
            }
        }
    }

    Some(ApplyPatchProgress {
        changes,
        active_path,
    })
}

fn insert_progress_hunk(
    changes: &mut HashMap<PathBuf, ApplyPatchProgressChange>,
    hunk: &Hunk,
    segments: &[PatchLineSegment<'_>],
) {
    match hunk {
        AddFile { path, .. } => {
            changes.insert(
                path.clone(),
                ApplyPatchProgressChange::Add {
                    content: add_content_from_segments(segments),
                },
            );
        }
        DeleteFile { path } => {
            changes.insert(path.clone(), ApplyPatchProgressChange::Delete);
        }
        UpdateFile {
            path, move_path, ..
        } => {
            changes.insert(
                path.clone(),
                ApplyPatchProgressChange::Update {
                    unified_diff: update_diff_from_segments(segments),
                    move_path: move_path.clone(),
                },
            );
        }
    }
}

fn parse_partial_progress_hunk(
    segments: &[PatchLineSegment<'_>],
) -> Option<(PathBuf, ApplyPatchProgressChange, Option<PathBuf>)> {
    let first = segments.first()?;
    if let Some(path) = marker_path(first.line, ADD_FILE_MARKER) {
        return Some((
            path,
            ApplyPatchProgressChange::Add {
                content: add_content_from_segments(segments),
            },
            None,
        ));
    }
    if let Some(path) = marker_path(first.line, DELETE_FILE_MARKER) {
        return Some((path, ApplyPatchProgressChange::Delete, None));
    }
    if let Some(path) = marker_path(first.line, UPDATE_FILE_MARKER) {
        let move_path = segments
            .get(1)
            .and_then(|segment| marker_path(segment.line, MOVE_TO_MARKER));
        let active_path = move_path.clone();
        return Some((
            path,
            ApplyPatchProgressChange::Update {
                unified_diff: update_diff_from_segments(segments),
                move_path,
            },
            active_path,
        ));
    }
    None
}

fn add_content_from_segments(segments: &[PatchLineSegment<'_>]) -> String {
    let mut content = String::new();
    for segment in &segments[1..] {
        if let Some(added_line) = segment.raw.strip_prefix('+') {
            content.push_str(added_line);
        } else {
            break;
        }
    }
    content
}

fn update_diff_from_segments(segments: &[PatchLineSegment<'_>]) -> String {
    let mut diff = String::new();
    let mut index = 1;
    if segments
        .get(index)
        .is_some_and(|segment| marker_path(segment.line, MOVE_TO_MARKER).is_some())
    {
        index += 1;
    }
    for segment in &segments[index..] {
        diff.push_str(segment.raw);
    }
    diff
}

fn marker_path(line: &str, marker: &str) -> Option<PathBuf> {
    let path = line.trim().strip_prefix(marker)?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[derive(Clone, Copy)]
struct PatchLineSegment<'a> {
    raw: &'a str,
    line: &'a str,
}

fn patch_line_segments(patch: &str) -> Vec<PatchLineSegment<'_>> {
    patch
        .split_inclusive('\n')
        .map(|raw| PatchLineSegment {
            raw,
            line: raw.trim_end_matches('\n').trim_end_matches('\r'),
        })
        .collect()
}

#[test]
fn test_parse_apply_patch_tool_input_progress_direct_add() {
    let progress = parse_apply_patch_tool_input_progress(
        "*** Begin Patch\n*** Add File: src/hello.txt\n+hello\n+wor",
    )
    .expect("patch progress");

    assert_eq!(progress.active_path, Some(PathBuf::from("src/hello.txt")));
    assert_eq!(
        progress.changes,
        HashMap::from([(
            PathBuf::from("src/hello.txt"),
            ApplyPatchProgressChange::Add {
                content: "hello\nwor".to_string(),
            },
        )])
    );
}

#[test]
fn test_parse_apply_patch_tool_input_progress_partial_json_arguments() {
    let progress = parse_apply_patch_tool_input_progress(
        r#"{"input":"*** Begin Patch\n*** Add File: src/hello.txt\n+hello\n+wor"#,
    )
    .expect("patch progress");

    assert_eq!(progress.active_path, Some(PathBuf::from("src/hello.txt")));
    assert_eq!(
        progress.changes,
        HashMap::from([(
            PathBuf::from("src/hello.txt"),
            ApplyPatchProgressChange::Add {
                content: "hello\nwor".to_string(),
            },
        )])
    );
}

#[test]
fn test_parse_apply_patch_tool_input_progress_update_and_move_path() {
    let progress = parse_apply_patch_tool_input_progress(
        "*** Begin Patch\n*** Update File: src/old.rs\n*** Move to: src/new.rs\n@@\n-old\n+new",
    )
    .expect("patch progress");

    assert_eq!(progress.active_path, Some(PathBuf::from("src/new.rs")));
    assert_eq!(
        progress.changes,
        HashMap::from([(
            PathBuf::from("src/old.rs"),
            ApplyPatchProgressChange::Update {
                unified_diff: "@@\n-old\n+new".to_string(),
                move_path: Some(PathBuf::from("src/new.rs")),
            },
        )])
    );
}

#[test]
fn test_parse_patch_text_streaming_mode_omits_end_marker() {
    assert!(
        parse_patch_text(
            "*** Begin Patch\n*** Delete File: gone.txt",
            ParseMode::Streaming
        )
        .is_ok()
    );
    assert!(
        parse_patch_text(
            "*** Begin Patch\n*** Delete File: gone.txt",
            ParseMode::Strict
        )
        .is_err()
    );
}

#[test]
fn test_parse_apply_patch_tool_input_progress_complete_hunk_before_partial_hunk() {
    let progress = parse_apply_patch_tool_input_progress(
        "*** Begin Patch\n*** Add File: src/one.txt\n+one\n*** Update File: src/two.txt\n",
    )
    .expect("patch progress");

    assert_eq!(progress.active_path, Some(PathBuf::from("src/two.txt")));
    assert_eq!(
        progress.changes,
        HashMap::from([
            (
                PathBuf::from("src/one.txt"),
                ApplyPatchProgressChange::Add {
                    content: "one\n".to_string(),
                },
            ),
            (
                PathBuf::from("src/two.txt"),
                ApplyPatchProgressChange::Update {
                    unified_diff: String::new(),
                    move_path: None,
                },
            ),
        ])
    );
}

#[test]
fn test_parse_patch() {
    assert_eq!(
        parse_patch_text("bad", ParseMode::Strict),
        Err(InvalidPatchError(
            "The first line of the patch must be '*** Begin Patch'".to_string()
        ))
    );
    assert_eq!(
        parse_patch_text("*** Begin Patch\nbad", ParseMode::Strict),
        Err(InvalidPatchError(
            "The last line of the patch must be '*** End Patch'".to_string()
        ))
    );

    assert_eq!(
        parse_patch_text(
            concat!(
                "*** Begin Patch",
                " ",
                "\n*** Add File: foo\n+hi\n",
                " ",
                "*** End Patch"
            ),
            ParseMode::Strict
        )
        .unwrap()
        .hunks,
        vec![AddFile {
            path: PathBuf::from("foo"),
            contents: "hi\n".to_string()
        }]
    );
    assert_eq!(
        parse_patch_text(
            "*** Begin Patch\n\
             *** Update File: test.py\n\
             *** End Patch",
            ParseMode::Strict
        ),
        Err(InvalidHunkError {
            message: "Update file hunk for path 'test.py' is empty".to_string(),
            line_number: 2,
        })
    );
    assert_eq!(
        parse_patch_text(
            "*** Begin Patch\n\
             *** End Patch",
            ParseMode::Strict
        )
        .unwrap()
        .hunks,
        Vec::new()
    );
    assert_eq!(
        parse_patch_text(
            "*** Begin Patch\n\
             *** Add File: path/add.py\n\
             +abc\n\
             +def\n\
             *** Delete File: path/delete.py\n\
             *** Update File: path/update.py\n\
             *** Move to: path/update2.py\n\
             @@ def f():\n\
             -    pass\n\
             +    return 123\n\
             *** End Patch",
            ParseMode::Strict
        )
        .unwrap()
        .hunks,
        vec![
            AddFile {
                path: PathBuf::from("path/add.py"),
                contents: "abc\ndef\n".to_string()
            },
            DeleteFile {
                path: PathBuf::from("path/delete.py")
            },
            UpdateFile {
                path: PathBuf::from("path/update.py"),
                move_path: Some(PathBuf::from("path/update2.py")),
                chunks: vec![UpdateFileChunk {
                    change_context: Some("def f():".to_string()),
                    old_lines: vec!["    pass".to_string()],
                    new_lines: vec!["    return 123".to_string()],
                    is_end_of_file: false
                }]
            }
        ]
    );
    // Update hunk followed by another hunk (Add File).
    assert_eq!(
        parse_patch_text(
            "*** Begin Patch\n\
             *** Update File: file.py\n\
             @@\n\
             +line\n\
             *** Add File: other.py\n\
             +content\n\
             *** End Patch",
            ParseMode::Strict
        )
        .unwrap()
        .hunks,
        vec![
            UpdateFile {
                path: PathBuf::from("file.py"),
                move_path: None,
                chunks: vec![UpdateFileChunk {
                    change_context: None,
                    old_lines: vec![],
                    new_lines: vec!["line".to_string()],
                    is_end_of_file: false
                }],
            },
            AddFile {
                path: PathBuf::from("other.py"),
                contents: "content\n".to_string()
            }
        ]
    );

    // Update hunk without an explicit @@ header for the first chunk should parse.
    // Use a raw string to preserve the leading space diff marker on the context line.
    assert_eq!(
        parse_patch_text(
            r#"*** Begin Patch
*** Update File: file2.py
 import foo
+bar
*** End Patch"#,
            ParseMode::Strict
        )
        .unwrap()
        .hunks,
        vec![UpdateFile {
            path: PathBuf::from("file2.py"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["import foo".to_string()],
                new_lines: vec!["import foo".to_string(), "bar".to_string()],
                is_end_of_file: false,
            }],
        }]
    );
}

#[test]
fn test_parse_patch_accepts_relative_and_absolute_hunk_paths() {
    let dir = tempfile::tempdir().unwrap();
    let absolute_delete = dir.path().join("absolute-delete.py").abs();
    let absolute_update = dir.path().join("absolute-update.py").abs();
    let patch_text = format!(
        r#"*** Begin Patch
*** Add File: relative-add.py
+content
*** Delete File: {}
*** Update File: {}
@@
-old
+new
*** End Patch"#,
        absolute_delete.display(),
        absolute_update.display()
    );

    assert_eq!(
        parse_patch_text(&patch_text, ParseMode::Strict)
            .unwrap()
            .hunks,
        vec![
            AddFile {
                path: PathBuf::from("relative-add.py"),
                contents: "content\n".to_string()
            },
            DeleteFile {
                path: absolute_delete.to_path_buf()
            },
            UpdateFile {
                path: absolute_update.to_path_buf(),
                move_path: None,
                chunks: vec![UpdateFileChunk {
                    change_context: None,
                    old_lines: vec!["old".to_string()],
                    new_lines: vec!["new".to_string()],
                    is_end_of_file: false
                }]
            },
        ]
    );
}

#[test]
fn test_hunk_resolve_path_accepts_relative_and_absolute_paths() {
    let cwd_dir = tempfile::tempdir().unwrap();
    let cwd = cwd_dir.path().to_path_buf().abs();
    let absolute_dir = tempfile::tempdir().unwrap();
    let absolute_add = absolute_dir.path().join("absolute-add.py").abs();
    let absolute_delete = absolute_dir.path().join("absolute-delete.py").abs();
    let absolute_update = absolute_dir.path().join("absolute-update.py").abs();

    for (hunk, expected_path) in [
        (
            AddFile {
                path: PathBuf::from("relative-add.py"),
                contents: String::new(),
            },
            cwd.join("relative-add.py"),
        ),
        (
            DeleteFile {
                path: PathBuf::from("relative-delete.py"),
            },
            cwd.join("relative-delete.py"),
        ),
        (
            UpdateFile {
                path: PathBuf::from("relative-update.py"),
                move_path: None,
                chunks: Vec::new(),
            },
            cwd.join("relative-update.py"),
        ),
        (
            AddFile {
                path: absolute_add.to_path_buf(),
                contents: String::new(),
            },
            absolute_add,
        ),
        (
            DeleteFile {
                path: absolute_delete.to_path_buf(),
            },
            absolute_delete,
        ),
        (
            UpdateFile {
                path: absolute_update.to_path_buf(),
                move_path: None,
                chunks: Vec::new(),
            },
            absolute_update,
        ),
    ] {
        assert_eq!(hunk.resolve_path(&cwd), expected_path);
    }
}

#[test]
fn test_parse_patch_lenient() {
    let patch_text = r#"*** Begin Patch
*** Update File: file2.py
 import foo
+bar
*** End Patch"#;
    let expected_patch = vec![UpdateFile {
        path: PathBuf::from("file2.py"),
        move_path: None,
        chunks: vec![UpdateFileChunk {
            change_context: None,
            old_lines: vec!["import foo".to_string()],
            new_lines: vec!["import foo".to_string(), "bar".to_string()],
            is_end_of_file: false,
        }],
    }];
    let expected_error =
        InvalidPatchError("The first line of the patch must be '*** Begin Patch'".to_string());

    let patch_text_in_heredoc = format!("<<EOF\n{patch_text}\nEOF\n");
    assert_eq!(
        parse_patch_text(&patch_text_in_heredoc, ParseMode::Strict),
        Err(expected_error.clone())
    );
    assert_eq!(
        parse_patch_text(&patch_text_in_heredoc, ParseMode::Lenient),
        Ok(ApplyPatchArgs {
            hunks: expected_patch.clone(),
            patch: patch_text.to_string(),
            workdir: None,
        })
    );

    let patch_text_in_single_quoted_heredoc = format!("<<'EOF'\n{patch_text}\nEOF\n");
    assert_eq!(
        parse_patch_text(&patch_text_in_single_quoted_heredoc, ParseMode::Strict),
        Err(expected_error.clone())
    );
    assert_eq!(
        parse_patch_text(&patch_text_in_single_quoted_heredoc, ParseMode::Lenient),
        Ok(ApplyPatchArgs {
            hunks: expected_patch.clone(),
            patch: patch_text.to_string(),
            workdir: None,
        })
    );

    let patch_text_in_double_quoted_heredoc = format!("<<\"EOF\"\n{patch_text}\nEOF\n");
    assert_eq!(
        parse_patch_text(&patch_text_in_double_quoted_heredoc, ParseMode::Strict),
        Err(expected_error.clone())
    );
    assert_eq!(
        parse_patch_text(&patch_text_in_double_quoted_heredoc, ParseMode::Lenient),
        Ok(ApplyPatchArgs {
            hunks: expected_patch,
            patch: patch_text.to_string(),
            workdir: None,
        })
    );

    let patch_text_in_mismatched_quotes_heredoc = format!("<<\"EOF'\n{patch_text}\nEOF\n");
    assert_eq!(
        parse_patch_text(&patch_text_in_mismatched_quotes_heredoc, ParseMode::Strict),
        Err(expected_error.clone())
    );
    assert_eq!(
        parse_patch_text(&patch_text_in_mismatched_quotes_heredoc, ParseMode::Lenient),
        Err(expected_error.clone())
    );

    let patch_text_with_missing_closing_heredoc =
        "<<EOF\n*** Begin Patch\n*** Update File: file2.py\nEOF\n".to_string();
    assert_eq!(
        parse_patch_text(&patch_text_with_missing_closing_heredoc, ParseMode::Strict),
        Err(expected_error)
    );
    assert_eq!(
        parse_patch_text(&patch_text_with_missing_closing_heredoc, ParseMode::Lenient),
        Err(InvalidPatchError(
            "The last line of the patch must be '*** End Patch'".to_string()
        ))
    );
}

#[test]
fn test_parse_one_hunk() {
    assert_eq!(
        parse_one_hunk(&["bad"], /*line_number*/ 234),
        Err(InvalidHunkError {
            message: "'bad' is not a valid hunk header. \
            Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'".to_string(),
            line_number: 234
        })
    );
    // Other edge cases are already covered by tests above/below.
}

#[test]
fn test_update_file_chunk() {
    assert_eq!(
        parse_update_file_chunk(
            &["bad"],
            /*line_number*/ 123,
            /*allow_missing_context*/ false
        ),
        Err(InvalidHunkError {
            message: "Expected update hunk to start with a @@ context marker, got: 'bad'"
                .to_string(),
            line_number: 123
        })
    );
    assert_eq!(
        parse_update_file_chunk(
            &["@@"],
            /*line_number*/ 123,
            /*allow_missing_context*/ false
        ),
        Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number: 124
        })
    );
    assert_eq!(
        parse_update_file_chunk(&["@@", "bad"], /*line_number*/ 123, /*allow_missing_context*/ false),
        Err(InvalidHunkError {
            message:  "Unexpected line found in update hunk: 'bad'. \
                       Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)".to_string(),
            line_number: 124
        })
    );
    assert_eq!(
        parse_update_file_chunk(
            &["@@", "*** End of File"],
            /*line_number*/ 123,
            /*allow_missing_context*/ false
        ),
        Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number: 124
        })
    );
    assert_eq!(
        parse_update_file_chunk(
            &[
                "@@ change_context",
                "",
                " context",
                "-remove",
                "+add",
                " context2",
                "*** End Patch",
            ],
            /*line_number*/ 123,
            /*allow_missing_context*/ false
        ),
        Ok((
            (UpdateFileChunk {
                change_context: Some("change_context".to_string()),
                old_lines: vec![
                    "".to_string(),
                    "context".to_string(),
                    "remove".to_string(),
                    "context2".to_string()
                ],
                new_lines: vec![
                    "".to_string(),
                    "context".to_string(),
                    "add".to_string(),
                    "context2".to_string()
                ],
                is_end_of_file: false
            }),
            6
        ))
    );
    assert_eq!(
        parse_update_file_chunk(
            &["@@", "+line", "*** End of File"],
            /*line_number*/ 123,
            /*allow_missing_context*/ false
        ),
        Ok((
            (UpdateFileChunk {
                change_context: None,
                old_lines: vec![],
                new_lines: vec!["line".to_string()],
                is_end_of_file: true
            }),
            3
        ))
    );
}
