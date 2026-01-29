//! Read-only command detection for safe execution without sandbox.

/// Known safe read-only commands that do not modify the system.
const READ_ONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "wc", "grep", "rg", "find", "which", "whoami", "pwd", "echo",
    "date", "env", "printenv", "uname", "hostname", "df", "du", "file", "stat", "type", "git",
];

/// Shell operators that may cause side effects (piping to commands, chaining, redirects).
const UNSAFE_OPERATORS: &[&str] = &["&&", "||", ";", "|", ">", "<"];

/// Git subcommands that are purely read-only.
const GIT_READ_ONLY_SUBCOMMANDS: &[&str] =
    &["status", "log", "diff", "show", "branch", "tag", "remote"];

/// Returns true if the command is a known read-only command.
///
/// A command is considered read-only if:
/// 1. Its first word is in the safe command list
/// 2. It does not contain shell operators (&&, ||, ;, |, >, <)
///
/// For `git` commands, further checks are applied via [`is_git_read_only`].
pub fn is_read_only_command(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Reject commands containing unsafe shell operators
    for op in UNSAFE_OPERATORS {
        if trimmed.contains(op) {
            return false;
        }
    }

    // Extract the first word (the command name)
    let first_word = match trimmed.split_whitespace().next() {
        Some(word) => word,
        None => return false,
    };

    // Check if it is a known safe command
    if !READ_ONLY_COMMANDS.contains(&first_word) {
        return false;
    }

    // For git commands, additionally verify the subcommand
    if first_word == "git" {
        return is_git_read_only(trimmed);
    }

    true
}

/// Returns true if the git command is a read-only subcommand.
///
/// Checks the second word of the command against the known read-only
/// git subcommands (status, log, diff, show, branch, tag, remote).
pub fn is_git_read_only(command: &str) -> bool {
    let trimmed = command.trim();
    let mut words = trimmed.split_whitespace();

    // Skip "git"
    match words.next() {
        Some("git") => {}
        _ => return false,
    }

    // Check subcommand
    match words.next() {
        Some(subcommand) => GIT_READ_ONLY_SUBCOMMANDS.contains(&subcommand),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_read_only_commands() {
        assert!(is_read_only_command("ls"));
        assert!(is_read_only_command("ls -la"));
        assert!(is_read_only_command("cat foo.txt"));
        assert!(is_read_only_command("head -n 10 file.rs"));
        assert!(is_read_only_command("tail -f log.txt"));
        assert!(is_read_only_command("wc -l foo"));
        assert!(is_read_only_command("grep pattern file"));
        assert!(is_read_only_command("rg pattern"));
        assert!(is_read_only_command("find . -name '*.rs'"));
        assert!(is_read_only_command("which cargo"));
        assert!(is_read_only_command("whoami"));
        assert!(is_read_only_command("pwd"));
        assert!(is_read_only_command("echo hello"));
        assert!(is_read_only_command("date"));
        assert!(is_read_only_command("env"));
        assert!(is_read_only_command("printenv HOME"));
        assert!(is_read_only_command("uname -a"));
        assert!(is_read_only_command("hostname"));
        assert!(is_read_only_command("df -h"));
        assert!(is_read_only_command("du -sh ."));
        assert!(is_read_only_command("file foo.txt"));
        assert!(is_read_only_command("stat foo.txt"));
        assert!(is_read_only_command("type ls"));
    }

    #[test]
    fn test_non_read_only_commands() {
        assert!(!is_read_only_command("rm -rf /"));
        assert!(!is_read_only_command("mkdir foo"));
        assert!(!is_read_only_command("cp a b"));
        assert!(!is_read_only_command("mv a b"));
        assert!(!is_read_only_command("cargo build"));
        assert!(!is_read_only_command("npm install"));
        assert!(!is_read_only_command("python script.py"));
    }

    #[test]
    fn test_commands_with_unsafe_operators() {
        assert!(!is_read_only_command("ls && rm foo"));
        assert!(!is_read_only_command("ls || echo fail"));
        assert!(!is_read_only_command("ls; rm foo"));
        assert!(!is_read_only_command("ls | grep foo"));
        assert!(!is_read_only_command("echo hello > file.txt"));
        assert!(!is_read_only_command("cat < file.txt"));
    }

    #[test]
    fn test_git_read_only() {
        assert!(is_read_only_command("git status"));
        assert!(is_read_only_command("git log --oneline"));
        assert!(is_read_only_command("git diff HEAD"));
        assert!(is_read_only_command("git show abc123"));
        assert!(is_read_only_command("git branch -a"));
        assert!(is_read_only_command("git tag"));
        assert!(is_read_only_command("git remote -v"));
    }

    #[test]
    fn test_git_non_read_only() {
        assert!(!is_read_only_command("git commit -m 'msg'"));
        assert!(!is_read_only_command("git push"));
        assert!(!is_read_only_command("git pull"));
        assert!(!is_read_only_command("git checkout main"));
        assert!(!is_read_only_command("git add ."));
        assert!(!is_read_only_command("git reset --hard"));
        assert!(!is_read_only_command("git merge feature"));
        assert!(!is_read_only_command("git rebase main"));
    }

    #[test]
    fn test_git_bare_command() {
        // "git" alone is not read-only (no subcommand)
        assert!(!is_read_only_command("git"));
    }

    #[test]
    fn test_empty_and_whitespace() {
        assert!(!is_read_only_command(""));
        assert!(!is_read_only_command("   "));
    }

    #[test]
    fn test_leading_trailing_whitespace() {
        assert!(is_read_only_command("  ls -la  "));
        assert!(is_read_only_command("  git status  "));
    }

    #[test]
    fn test_is_git_read_only_direct() {
        assert!(is_git_read_only("git status"));
        assert!(is_git_read_only("git log"));
        assert!(is_git_read_only("git diff"));
        assert!(is_git_read_only("git show"));
        assert!(is_git_read_only("git branch"));
        assert!(is_git_read_only("git tag"));
        assert!(is_git_read_only("git remote"));
        assert!(!is_git_read_only("git push"));
        assert!(!is_git_read_only("git commit"));
        assert!(!is_git_read_only("not-git status"));
        assert!(!is_git_read_only("git"));
    }
}
