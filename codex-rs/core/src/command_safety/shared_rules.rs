//! Shared safelist logic for commands that are allowed on both Unix and Windows.
//! Keep these helpers read-only and conservative.

/// Ensures the ripgrep invocation does not contain flags that could run
/// arbitrary commands.
pub(crate) fn is_safe_ripgrep_command<S: AsRef<str>>(words: &[S]) -> bool {
    const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &[
        // Takes an arbitrary command that is executed for each match.
        "--pre",
        // Takes a command that can be used to obtain the local hostname.
        "--hostname-bin",
    ];
    const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &[
        // Calls out to other decompression tools, so do not auto-approve out of
        // an abundance of caution.
        "--search-zip",
        "-z",
    ];

    !words.iter().skip(1).any(|arg| {
        let arg_lc = arg.as_ref().to_ascii_lowercase();
        UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg_lc.as_str())
            || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                .iter()
                .any(|opt| arg_lc == *opt || arg_lc.starts_with(&format!("{opt}=")))
    })
}

/// Ensures a Git command sticks to whitelisted read-only subcommands and flags.
pub(crate) fn is_safe_git_command<S: AsRef<str>>(words: &[S]) -> bool {
    const SAFE_SUBCOMMANDS: &[&str] = &["status", "log", "show", "diff", "cat-file"];

    let mut iter = words.iter().skip(1);
    while let Some(arg) = iter.next() {
        let arg = arg.as_ref();
        let arg_lc = arg.to_ascii_lowercase();

        if arg.starts_with('-') {
            if arg.eq_ignore_ascii_case("-c") || arg.eq_ignore_ascii_case("--config") {
                if iter.next().is_none() {
                    return false;
                }
                continue;
            }

            if arg_lc.starts_with("-c=")
                || arg_lc.starts_with("--config=")
                || arg_lc.starts_with("--git-dir=")
                || arg_lc.starts_with("--work-tree=")
            {
                continue;
            }

            if arg.eq_ignore_ascii_case("--git-dir") || arg.eq_ignore_ascii_case("--work-tree") {
                if iter.next().is_none() {
                    return false;
                }
                continue;
            }

            continue;
        }

        return SAFE_SUBCOMMANDS.contains(&arg_lc.as_str());
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIPGREP_UNSAFE_NO_ARG: &[&[&str]] =
        &[&["rg", "--search-zip", "files"], &["rg", "-z", "files"]];
    const RIPGREP_UNSAFE_WITH_ARG: &[&[&str]] = &[
        &["rg", "--pre", "pwned", "files"],
        &["rg", "--pre=pwned", "files"],
        &["rg", "--hostname-bin", "pwned", "files"],
        &["rg", "--hostname-bin=pwned", "files"],
    ];

    #[test]
    fn ripgrep_rules() {
        // Safe ripgrep invocations – none of the unsafe flags are present.
        assert!(is_safe_ripgrep_command(&["rg", "Cargo.toml", "-n"]));

        // Unsafe flags that do not take an argument (present verbatim).
        for args in RIPGREP_UNSAFE_NO_ARG {
            assert!(
                !is_safe_ripgrep_command(args),
                "expected {args:?} to be considered unsafe due to zip-search flag",
            );
        }

        // Unsafe flags that expect a value, provided in both split and = forms.
        for args in RIPGREP_UNSAFE_WITH_ARG {
            assert!(
                !is_safe_ripgrep_command(args),
                "expected {args:?} to be considered unsafe due to external-command flag",
            );
        }
    }

    #[test]
    fn git_rules() {
        assert!(is_safe_git_command(&["git", "status"]));
        assert!(is_safe_git_command(&[
            "git",
            "-c",
            "core.pager=cat",
            "show",
            "HEAD:foo.rs"
        ]));
        assert!(is_safe_git_command(&[
            "git",
            "--work-tree=.",
            "--git-dir=.git",
            "diff"
        ]));

        assert!(!is_safe_git_command(&["git"]));
        assert!(!is_safe_git_command(&["git", "branch"]));
        assert!(!is_safe_git_command(&["git", "fetch"]));
        assert!(!is_safe_git_command(&["git", "-c"]));
        assert!(!is_safe_git_command(&["git", "--config"]));
        assert!(!is_safe_git_command(&["git", "--git-dir"]));
        assert!(!is_safe_git_command(&["git", "--work-tree"]));
    }
}
