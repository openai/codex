//! External editor support.
//!
//! This module provides functionality to open the current input
//! in an external text editor (like vim, nano, etc.).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use crate::terminal::restore_terminal;
use crate::terminal::setup_terminal;

/// Get the preferred editor from environment variables.
///
/// Checks VISUAL first, then EDITOR, defaulting to vim.
pub fn get_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vim".to_string())
}

/// Result of editing in an external editor.
#[derive(Debug)]
pub struct EditResult {
    /// The edited content (may be same as original if unchanged).
    pub content: String,
    /// Whether the content was modified.
    pub modified: bool,
}

/// Open the given content in an external editor and return the edited content.
///
/// This function:
/// 1. Writes the content to a temporary file
/// 2. Suspends the TUI (restores terminal to normal mode)
/// 3. Opens the editor with the temp file
/// 4. Waits for the editor to close
/// 5. Reads the modified content
/// 6. Re-initializes the TUI
/// 7. Returns the edited content
///
/// # Arguments
///
/// * `content` - The initial content to edit
///
/// # Returns
///
/// The edited content, or an error if the operation failed.
///
/// # Errors
///
/// Returns an error if:
/// - Creating the temp file fails
/// - Spawning the editor fails
/// - Reading the edited content fails
/// - Terminal restoration fails
pub fn edit_in_external_editor(content: &str) -> io::Result<EditResult> {
    // Create a temp file with the content
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("cocode_edit_{}.md", std::process::id()));

    // Write content to temp file
    fs::write(&temp_path, content)?;

    // Get the editor command
    let editor = get_editor();

    // Restore terminal to normal mode before spawning editor
    restore_terminal()?;

    // Spawn the editor
    let result = spawn_editor(&editor, &temp_path);

    // Re-setup terminal regardless of editor result
    // Note: We ignore errors here because we may have already restored
    let _terminal = setup_terminal();

    // Handle editor result
    result?;

    // Read back the edited content
    let edited_content = fs::read_to_string(&temp_path)?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_path);

    let modified = edited_content != content;

    Ok(EditResult {
        content: edited_content,
        modified,
    })
}

/// Spawn the editor process and wait for it to complete.
fn spawn_editor(editor: &str, file_path: &PathBuf) -> io::Result<()> {
    // Parse the editor command (might have arguments like "vim -c 'set ft=markdown'")
    let parts: Vec<&str> = editor.split_whitespace().collect();
    if parts.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty editor command",
        ));
    }

    let program = parts[0];
    let args = &parts[1..];

    let mut cmd = Command::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg(file_path);

    // Spawn and wait
    let status = cmd.status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Editor exited with status: {status}"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_editor_returns_value() {
        // Just test that get_editor returns a non-empty value
        // (either from env or the default "vim")
        let editor = get_editor();
        assert!(!editor.is_empty());
    }

    #[test]
    fn test_edit_result_struct() {
        let result = EditResult {
            content: "test content".to_string(),
            modified: true,
        };
        assert!(result.modified);
        assert_eq!(result.content, "test content");
    }
}
