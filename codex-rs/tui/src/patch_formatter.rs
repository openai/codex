use codex_core::protocol::FileChange;
use std::collections::HashMap;
use std::path::PathBuf;

/// Format patch changes for display in Omnara dashboard
pub fn format_patch_details(changes: &HashMap<PathBuf, FileChange>) -> (String, usize, usize) {
    let mut patch_details = String::new();
    let mut added_lines = 0;
    let mut removed_lines = 0;
    
    for (path, change) in changes {
        let path_str = path.display().to_string();
        
        // Add spacing between files if not the first one
        if !patch_details.is_empty() {
            patch_details.push_str("\n");
        }
        
        match change {
            FileChange::Add { content } => {
                added_lines += content.lines().count();
                patch_details.push_str(&format!("**New file: {}**\n```diff\n", path_str));
                // Show first 10 lines of new file
                let preview_lines: Vec<&str> = content.lines().take(10).collect();
                for line in preview_lines {
                    patch_details.push_str(&format!("+{}\n", line));
                }
                if content.lines().count() > 10 {
                    patch_details.push_str(&format!("... ({} more lines)\n", content.lines().count() - 10));
                }
                patch_details.push_str("```\n");
            }
            FileChange::Update { unified_diff, .. } => {
                patch_details.push_str(&format!("**{}**\n```diff\n", path_str));
                // Include the actual diff
                patch_details.push_str(unified_diff);
                patch_details.push_str("\n```\n");
                
                for line in unified_diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        added_lines += 1;
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        removed_lines += 1;
                    }
                }
            }
            FileChange::Delete => {
                removed_lines += 1; // Placeholder
                patch_details.push_str(&format!("**Delete file: {}**\n", path_str));
            }
        }
    }
    
    (patch_details, added_lines, removed_lines)
}