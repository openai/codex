use std::path::Path;
use std::path::PathBuf;

use codex_core::parse_command::extract_shell_command;
use dirs::home_dir;
use shlex::try_join;

const APPROVAL_LABEL_MAX_LEN: usize = 80;
const APPROVAL_LABEL_TRUNCATED_SUFFIX: &str = "... (truncated)";

pub(crate) fn escape_command(command: &[String]) -> String {
    try_join(command.iter().map(String::as_str)).unwrap_or_else(|_| command.join(" "))
}

pub(crate) fn strip_bash_lc_and_escape(command: &[String]) -> String {
    if let Some((_, script)) = extract_shell_command(command) {
        return script.to_string();
    }
    escape_command(command)
}

pub(crate) fn render_for_approval_prefix_label(command: &[String]) -> String {
    let rendered = strip_bash_lc_and_escape(command);

    // Approval choices are rendered inline in a list; ensure we never introduce
    // multi-line labels due to heredocs or embedded newlines.
    let collapsed = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let prefix: String = chars.by_ref().take(APPROVAL_LABEL_MAX_LEN).collect();
    if chars.next().is_some() {
        format!("{}{APPROVAL_LABEL_TRUNCATED_SUFFIX}", prefix.trim_end())
    } else {
        prefix
    }
}

/// If `path` is absolute and inside $HOME, return the part *after* the home
/// directory; otherwise, return the path as-is. Note if `path` is the homedir,
/// this will return and empty path.
pub(crate) fn relativize_to_home<P>(path: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    if !path.is_absolute() {
        // If the path is not absolute, we can’t do anything with it.
        return None;
    }

    let home_dir = home_dir()?;
    let rel = path.strip_prefix(&home_dir).ok()?;
    Some(rel.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_command() {
        let args = vec!["foo".into(), "bar baz".into(), "weird&stuff".into()];
        let cmdline = escape_command(&args);
        assert_eq!(cmdline, "foo 'bar baz' 'weird&stuff'");
    }

    #[test]
    fn test_strip_bash_lc_and_escape() {
        // Test bash
        let args = vec!["bash".into(), "-lc".into(), "echo hello".into()];
        let cmdline = strip_bash_lc_and_escape(&args);
        assert_eq!(cmdline, "echo hello");

        // Test zsh
        let args = vec!["zsh".into(), "-lc".into(), "echo hello".into()];
        let cmdline = strip_bash_lc_and_escape(&args);
        assert_eq!(cmdline, "echo hello");

        // Test absolute path to zsh
        let args = vec!["/usr/bin/zsh".into(), "-lc".into(), "echo hello".into()];
        let cmdline = strip_bash_lc_and_escape(&args);
        assert_eq!(cmdline, "echo hello");

        // Test absolute path to bash
        let args = vec!["/bin/bash".into(), "-lc".into(), "echo hello".into()];
        let cmdline = strip_bash_lc_and_escape(&args);
        assert_eq!(cmdline, "echo hello");
    }

    #[test]
    fn approval_prefix_label_is_single_line_and_truncated() {
        let long = format!("python - <<'PY'\n{}\nPY\n", "x".repeat(500));
        let args = vec!["bash".into(), "-lc".into(), long];
        let label = render_for_approval_prefix_label(&args);
        assert!(!label.contains('\n'));
        assert!(label.ends_with(APPROVAL_LABEL_TRUNCATED_SUFFIX));
    }
}
