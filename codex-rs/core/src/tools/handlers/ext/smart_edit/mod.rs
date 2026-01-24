//! Smart Edit Handler - Instruction-based code editing with intelligent matching
//!
//! This module provides the SmartEditHandler which implements instruction-based
//! file editing with three-tier matching strategies and LLM-powered correction.
//!
//! ## Features
//! - Three-tier matching: Exact → Flexible → Regex
//! - Semantic LLM correction with instruction context
//! - Concurrent modification detection (SHA256)
//! - Line ending preservation (CRLF/LF)
//! - Indentation preservation

pub(crate) mod common;
mod correction;
mod strategies;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use common::count_non_overlapping_occurrences;
use common::detect_line_ending;
use common::hash_content;
use common::unescape_string_for_llm_bug;
use correction::attempt_llm_correction;
use correction::correct_new_string_escaping;
use correction::is_potentially_over_escaped;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use strategies::ReplacementResult;
use strategies::trim_pair_if_possible;
use strategies::try_all_strategies;

/// Smart Edit tool arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartEditArgs {
    pub file_path: String,
    pub instruction: String, // Key differentiator: semantic context
    pub old_string: String,
    pub new_string: String,

    #[serde(default = "default_expected")]
    pub expected_replacements: i32,
}

fn default_expected() -> i32 {
    1
}

/// Smart Edit Handler (stateless)
///
/// Uses ModelClient from turn context - no configuration needed.
pub struct SmartEditHandler;

#[async_trait]
impl ToolHandler for SmartEditHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse and validate arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for smart_edit".to_string(),
                ));
            }
        };

        let args: SmartEditArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        validate_args(&args)?;

        // 2. Resolve file path
        let file_path = invocation.turn.resolve_path(Some(args.file_path));

        // 3. Handle file creation (empty old_string)
        if args.old_string.is_empty() {
            // Check if file already exists - reject to prevent accidental overwrite
            if file_path.exists() {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Cannot create file: {} already exists. Use non-empty old_string to edit existing files.",
                    file_path.display()
                )));
            }
            return create_new_file(&file_path, &args.new_string);
        }

        // 4. Read file
        let (content, line_ending) = read_file_with_line_ending(&file_path)?;

        // 5. Normalize and compute hash
        let normalized = content.replace("\r\n", "\n");
        let initial_hash = hash_content(&normalized);

        // 6. PRE-CORRECTION: Check if simple unescape can fix the issue
        //    This avoids unnecessary LLM calls for common over-escaping issues
        let (working_old, working_new) = pre_correct_escaping(
            &args.old_string,
            &args.new_string,
            &normalized,
            args.expected_replacements,
        );

        // 7. Try three-tier strategies with (potentially corrected) strings
        let result = try_all_strategies(&working_old, &working_new, &normalized);

        if check_success(&result, args.expected_replacements) {
            // Success! Write file and return
            return write_file_and_respond(&file_path, &result, &line_ending);
        }

        // 8. FALLBACK: Try trim_pair_if_possible
        if let Some((trimmed_old, trimmed_new)) = trim_pair_if_possible(
            &working_old,
            &working_new,
            &normalized,
            args.expected_replacements,
        ) {
            let trim_result = try_all_strategies(&trimmed_old, &trimmed_new, &normalized);
            if check_success(&trim_result, args.expected_replacements) {
                tracing::info!("Smart edit: trim_pair_if_possible succeeded");
                return write_file_and_respond(&file_path, &trim_result, &line_ending);
            }
        }

        // 9. Detect concurrent modifications
        let (content_for_llm, error_msg) =
            detect_concurrent_modification(&file_path, &normalized, &initial_hash, &result)?;

        // 10. LLM correction with instruction context
        let corrected = attempt_llm_correction(
            &invocation.turn.client, // Use existing ModelClient
            &args.instruction,
            &working_old,
            &working_new,
            &content_for_llm,
            &error_msg,
        )
        .await
        .map_err(|e| FunctionCallError::RespondToModel(format!("LLM correction failed: {e}")))?;

        // Check if no changes required
        if corrected.no_changes_required {
            return Ok(ToolOutput::Function {
                content: format!("No changes needed: {}", corrected.explanation),
                content_items: None,
                success: Some(true),
            });
        }

        // 11. Retry with corrected parameters
        let retry_result =
            try_all_strategies(&corrected.search, &corrected.replace, &content_for_llm);

        if check_success(&retry_result, args.expected_replacements) {
            write_file_with_explanation(
                &file_path,
                &retry_result,
                &line_ending,
                &corrected.explanation,
            )
        } else {
            Err(FunctionCallError::RespondToModel(format!(
                "Edit failed after LLM correction. {}\n\
                 LLM explanation: {}\n\
                 Found {} occurrences (expected {}).",
                error_msg,
                corrected.explanation,
                retry_result.occurrences,
                args.expected_replacements
            )))
        }
    }
}

/// Pre-correct escaping issues before trying strategies
///
/// This function implements the pre-correction flow from gemini-cli's `ensureCorrectEdit()`:
/// 1. Check if old_string has the expected occurrence count
/// 2. If occurrences == 0, try unescaping old_string
/// 3. If unescaping helps, also unescape new_string
/// 4. If new_string appears over-escaped, correct it
///
/// Returns (working_old_string, working_new_string) - may be unchanged or corrected.
fn pre_correct_escaping(
    old_string: &str,
    new_string: &str,
    content: &str,
    expected: i32,
) -> (String, String) {
    // Check current occurrence count
    let occurrences = count_non_overlapping_occurrences(content, old_string);

    // If matches expected, check if new_string needs escaping correction
    if occurrences == expected {
        if is_potentially_over_escaped(new_string) {
            let corrected_new = correct_new_string_escaping(new_string);
            return (old_string.to_string(), corrected_new);
        }
        return (old_string.to_string(), new_string.to_string());
    }

    // If no match, try unescaping old_string
    if occurrences == 0 {
        let unescaped_old = unescape_string_for_llm_bug(old_string);
        let unescaped_occurrences = count_non_overlapping_occurrences(content, &unescaped_old);

        if unescaped_occurrences == expected {
            tracing::info!("Smart edit: pre-correction unescape fixed old_string match");
            // Unescaping old_string worked - also unescape new_string for consistency
            let unescaped_new = unescape_string_for_llm_bug(new_string);
            return (unescaped_old, unescaped_new);
        }
    }

    // No pre-correction helped - return original strings
    (old_string.to_string(), new_string.to_string())
}

/// Validate arguments
fn validate_args(args: &SmartEditArgs) -> Result<(), FunctionCallError> {
    if args.expected_replacements < 1 {
        return Err(FunctionCallError::RespondToModel(
            "expected_replacements must be at least 1".to_string(),
        ));
    }

    if !args.old_string.is_empty() && args.old_string == args.new_string {
        return Err(FunctionCallError::RespondToModel(
            "old_string and new_string cannot be identical (no change would occur)".to_string(),
        ));
    }

    Ok(())
}

/// Check if replacement result matches expected count
fn check_success(result: &ReplacementResult, expected: i32) -> bool {
    result.occurrences == expected
}

/// Create a new file with the given content
///
/// Automatically creates parent directories if they don't exist,
/// matching gemini-cli's ensureParentDirectoriesExist() behavior.
fn create_new_file(
    file_path: &std::path::Path,
    content: &str,
) -> Result<ToolOutput, FunctionCallError> {
    // Ensure parent directories exist
    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| {
                FunctionCallError::RespondToModel(format!(
                    "Failed to create parent directories for {}: {e}",
                    file_path.display()
                ))
            })?;
        }
    }

    fs::write(file_path, content).map_err(|e| {
        FunctionCallError::RespondToModel(format!(
            "Failed to create file {}: {e}",
            file_path.display()
        ))
    })?;

    Ok(ToolOutput::Function {
        content: format!("Created new file: {}", file_path.display()),
        content_items: None,
        success: Some(true),
    })
}

/// Read file and detect line ending
fn read_file_with_line_ending(
    file_path: &std::path::Path,
) -> Result<(String, &'static str), FunctionCallError> {
    let content = fs::read_to_string(file_path).map_err(|e| {
        FunctionCallError::RespondToModel(format!(
            "Failed to read file {}: {e}",
            file_path.display()
        ))
    })?;

    let line_ending = detect_line_ending(&content);
    Ok((content, line_ending))
}

/// Write file and return success response
fn write_file_and_respond(
    file_path: &std::path::Path,
    result: &ReplacementResult,
    original_line_ending: &str,
) -> Result<ToolOutput, FunctionCallError> {
    // Restore line ending
    let final_content = if original_line_ending == "\r\n" {
        result.new_content.replace('\n', "\r\n")
    } else {
        result.new_content.clone()
    };

    // Write file
    fs::write(file_path, &final_content).map_err(|e| {
        FunctionCallError::RespondToModel(format!(
            "Failed to write file {}: {e}",
            file_path.display()
        ))
    })?;

    Ok(ToolOutput::Function {
        content: format!(
            "Successfully edited {} using {} strategy ({} occurrence{})",
            file_path.display(),
            result.strategy,
            result.occurrences,
            if result.occurrences == 1 { "" } else { "s" }
        ),
        content_items: None,
        success: Some(true),
    })
}

/// Write file with LLM explanation
fn write_file_with_explanation(
    file_path: &std::path::Path,
    result: &ReplacementResult,
    original_line_ending: &str,
    explanation: &str,
) -> Result<ToolOutput, FunctionCallError> {
    // Restore line ending
    let final_content = if original_line_ending == "\r\n" {
        result.new_content.replace('\n', "\r\n")
    } else {
        result.new_content.clone()
    };

    // Write file
    fs::write(file_path, &final_content).map_err(|e| {
        FunctionCallError::RespondToModel(format!(
            "Failed to write file {}: {e}",
            file_path.display()
        ))
    })?;

    Ok(ToolOutput::Function {
        content: format!(
            "Successfully edited {} using {} strategy after LLM correction.\n\
             Occurrences: {}\n\
             Correction: {}",
            file_path.display(),
            result.strategy,
            result.occurrences,
            explanation
        ),
        content_items: None,
        success: Some(true),
    })
}

/// Detect concurrent modifications
fn detect_concurrent_modification(
    file_path: &std::path::Path,
    original_content: &str,
    initial_hash: &str,
    result: &ReplacementResult,
) -> Result<(String, String), FunctionCallError> {
    let error_msg = format!(
        "Found {} occurrences (expected different count or no match)",
        result.occurrences
    );

    // Re-read file from disk
    let on_disk_content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => {
            // File disappeared - use original content
            return Ok((original_content.to_string(), error_msg));
        }
    };

    let on_disk_normalized = on_disk_content.replace("\r\n", "\n");
    let on_disk_hash = hash_content(&on_disk_normalized);

    if initial_hash != on_disk_hash {
        // File was modified externally → use latest version for LLM correction
        Ok((
            on_disk_normalized,
            format!(
                "File modified externally. Using latest version. Original error: {}",
                error_msg
            ),
        ))
    } else {
        // File unchanged → use original content
        Ok((original_content.to_string(), error_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_args_valid() {
        let valid = SmartEditArgs {
            file_path: "test.rs".into(),
            instruction: "Update value".into(),
            old_string: "old".into(),
            new_string: "new".into(),
            expected_replacements: 1,
        };
        assert!(validate_args(&valid).is_ok());
    }

    #[test]
    fn test_validate_args_invalid_count() {
        let invalid = SmartEditArgs {
            file_path: "test.rs".into(),
            instruction: "Update".into(),
            old_string: "old".into(),
            new_string: "new".into(),
            expected_replacements: 0,
        };
        assert!(validate_args(&invalid).is_err());
    }

    #[test]
    fn test_validate_args_same_strings() {
        let invalid = SmartEditArgs {
            file_path: "test.rs".into(),
            instruction: "Update".into(),
            old_string: "same".into(),
            new_string: "same".into(),
            expected_replacements: 1,
        };
        assert!(validate_args(&invalid).is_err());
    }

    #[test]
    fn test_validate_args_empty_old_string_allowed() {
        let valid = SmartEditArgs {
            file_path: "test.rs".into(),
            instruction: "Create file".into(),
            old_string: "".into(),
            new_string: "new content".into(),
            expected_replacements: 1,
        };
        // Empty old_string is allowed (creates new file)
        assert!(validate_args(&valid).is_ok());
    }

    #[test]
    fn test_check_success() {
        let result = ReplacementResult {
            new_content: "updated".to_string(),
            occurrences: 2,
            strategy: "exact".to_string(),
        };

        assert!(check_success(&result, 2));
        assert!(!check_success(&result, 1));
        assert!(!check_success(&result, 3));
    }

    // Tests for pre_correct_escaping

    #[test]
    fn test_pre_correct_no_change_needed() {
        // old_string already matches, no escaping issues
        let content = "hello world";
        let (old, new) = pre_correct_escaping("hello", "hi", content, 1);
        assert_eq!(old, "hello");
        assert_eq!(new, "hi");
    }

    #[test]
    fn test_pre_correct_unescape_fixes_match() {
        // old_string doesn't match, but unescaping fixes it
        let content = "line1\nline2";
        let (old, new) = pre_correct_escaping("line1\\nline2", "line1\\nupdated", content, 1);
        assert_eq!(old, "line1\nline2");
        assert_eq!(new, "line1\nupdated");
    }

    #[test]
    fn test_pre_correct_new_string_escaping() {
        // old_string matches, but new_string is over-escaped
        let content = "hello world";
        let (old, new) = pre_correct_escaping("hello", "hi\\nthere", content, 1);
        assert_eq!(old, "hello");
        assert_eq!(new, "hi\nthere");
    }

    #[test]
    fn test_pre_correct_no_help() {
        // Neither escaping correction helps - return original
        let content = "hello world";
        let (old, new) = pre_correct_escaping("notfound", "replacement", content, 1);
        assert_eq!(old, "notfound");
        assert_eq!(new, "replacement");
    }
}
