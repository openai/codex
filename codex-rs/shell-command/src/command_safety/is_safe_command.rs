use crate::bash::parse_shell_lc_plain_commands;
use crate::command_safety::is_dangerous_command::executable_name_lookup_key;
#[cfg(windows)]
use crate::command_safety::windows_safe_commands::is_safe_command_windows;
#[cfg(windows)]
use crate::command_safety::windows_safe_commands::is_safe_powershell_words as is_safe_powershell_words_windows;

pub fn is_known_safe_command(command: &[String]) -> bool {
    let command: Vec<String> = command
        .iter()
        .map(|s| {
            if s == "zsh" {
                "bash".to_string()
            } else {
                s.clone()
            }
        })
        .collect();

    #[cfg(windows)]
    {
        if is_safe_command_windows(&command) {
            return true;
        }
    }

    if is_safe_to_call_with_exec(&command) {
        return true;
    }

    // Support `bash -lc "..."` where the script consists solely of one or
    // more "plain" commands (only bare words / quoted strings) combined with
    // a conservative allow‑list of shell operators that themselves do not
    // introduce side effects ( "&&", "||", ";", and "|" ). If every
    // individual command in the script is itself a known‑safe command, then
    // the composite expression is considered safe.
    if let Some(all_commands) = parse_shell_lc_plain_commands(&command)
        && !all_commands.is_empty()
        && all_commands
            .iter()
            .all(|cmd| is_safe_to_call_with_exec(cmd))
    {
        return true;
    }
    false
}

/// Returns whether already-tokenized PowerShell words are read-only enough to
/// be auto-approved by the Windows safelist.
pub fn is_safe_powershell_words(command: &[String]) -> bool {
    #[cfg(windows)]
    {
        is_safe_powershell_words_windows(command)
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        false
    }
}

fn is_safe_to_call_with_exec(command: &[String]) -> bool {
    let Some(cmd0) = command.first().map(String::as_str) else {
        return false;
    };
    if std::path::Path::new(cmd0).components().count() != 1 {
        // A workspace executable can impersonate an allowlisted utility by
        // reusing its basename. Only bare names resolved through the trusted
        // process PATH are eligible for generic safe-command classification.
        return false;
    }

    match executable_name_lookup_key(cmd0).as_deref() {
        Some(cmd) if cfg!(target_os = "linux") && matches!(cmd, "numfmt" | "tac") => true,

        #[rustfmt::skip]
        Some(
            "cat" |
            "cd" |
            "cut" |
            "echo" |
            "expr" |
            "false" |
            "grep" |
            "head" |
            "id" |
            "ls" |
            "nl" |
            "paste" |
            "pwd" |
            "rev" |
            "seq" |
            "stat" |
            "tail" |
            "tr" |
            "true" |
            "uname" |
            "uniq" |
            "wc" |
            "which" |
            "whoami") => {
            true
        },

        Some("base64") => {
            const UNSAFE_BASE64_OPTIONS: &[&str] = &["-o", "--output"];

            !command.iter().skip(1).any(|arg| {
                UNSAFE_BASE64_OPTIONS.contains(&arg.as_str())
                    || arg.starts_with("--output=")
                    || (arg.starts_with("-o") && arg != "-o")
            })
        }

        Some("find") => {
            // Certain options to `find` can delete files, write to files, or
            // execute arbitrary commands, so we cannot auto-approve the
            // invocation of `find` in such cases.
            #[rustfmt::skip]
            const UNSAFE_FIND_OPTIONS: &[&str] = &[
                // Options that can execute arbitrary commands.
                "-exec", "-execdir", "-ok", "-okdir",
                // Option that deletes matching files.
                "-delete",
                // Options that write pathnames to a file.
                "-fls", "-fprint", "-fprint0", "-fprintf",
            ];

            !command
                .iter()
                .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
        }

        // Ripgrep
        Some("rg") => {
            const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &[
                // Takes an arbitrary command that is executed for each match.
                "--pre",
                // Takes a command that can be used to obtain the local hostname.
                "--hostname-bin",
            ];
            const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &[
                // Calls out to other decompression tools, so do not auto-approve
                // out of an abundance of caution.
                "--search-zip",
                "-z",
            ];

            !command.iter().any(|arg| {
                UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg.as_str())
                    || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                        .iter()
                        .any(|&opt| arg == opt || arg.starts_with(&format!("{opt}=")))
            })
        }

        // Git
        Some("git") => is_safe_git_command(command),

        // Special-case `sed -n {N|M,N}p`
        Some("sed")
            if {
                command.len() <= 4
                    && command.get(1).map(String::as_str) == Some("-n")
                    && is_valid_sed_n_arg(command.get(2).map(String::as_str))
            } =>
        {
            true
        }

        // ── anything else ─────────────────────────────────────────────────
        _ => false,
    }
}

pub(crate) fn is_safe_git_command(_command: &[String]) -> bool {
    // Git behavior depends on repository config, attributes, the discovered
    // repository, environment, and TTY state. This argv-only classifier cannot
    // prove that any Git command avoids repository-selected executables.
    false
}

// (bash parsing helpers implemented in crate::bash)

/* ----------------------------------------------------------
Example
---------------------------------------------------------- */

/// Returns true if `arg` matches /^(\d+,)?\d+p$/
fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    // unwrap or bail
    let s = match arg {
        Some(s) => s,
        None => return false,
    };

    // must end with 'p', strip it
    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };

    // split on ',' and ensure 1 or 2 numeric parts
    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        // single number, e.g. "10"
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),

        // two numbers, e.g. "1,5"
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }

        // anything else (more than one comma) is invalid
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn known_safe_examples() {
        assert!(is_safe_to_call_with_exec(&vec_str(&["ls"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&["base64"])));
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "1,5p", "file.txt"
        ])));
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "nl",
            "-nrz",
            "Cargo.toml"
        ])));

        // Safe `find` command (no unsafe options).
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "find", ".", "-name", "file.txt"
        ])));

        if cfg!(target_os = "linux") {
            assert!(is_safe_to_call_with_exec(&vec_str(&["numfmt", "1000"])));
            assert!(is_safe_to_call_with_exec(&vec_str(&["tac", "Cargo.toml"])));
        } else {
            assert!(!is_safe_to_call_with_exec(&vec_str(&["numfmt", "1000"])));
            assert!(!is_safe_to_call_with_exec(&vec_str(&["tac", "Cargo.toml"])));
        }
    }

    #[test]
    fn path_qualified_safe_command_names_require_approval() {
        let absolute_cat = if cfg!(windows) {
            r"C:\workspace\cat.exe"
        } else {
            "/tmp/workspace/cat"
        };
        let parent_relative_cat = if cfg!(windows) {
            r"..\cat.exe"
        } else {
            "../cat"
        };

        for args in [
            vec_str(&["./cat", "Cargo.toml"]),
            vec_str(&[parent_relative_cat, "Cargo.toml"]),
            vec_str(&[absolute_cat, "Cargo.toml"]),
            vec_str(&["bash", "-lc", "./cat Cargo.toml"]),
        ] {
            assert!(
                !is_known_safe_command(&args),
                "expected path-qualified executable {args:?} to require approval",
            );
        }

        let bare_cat = if cfg!(windows) { "cat.exe" } else { "cat" };
        assert!(is_known_safe_command(&vec_str(&[bare_cat, "Cargo.toml"])));
    }

    #[test]
    fn git_commands_require_approval() {
        for args in [
            vec_str(&["git", "status"]),
            vec_str(&["git", "log", "-1"]),
            vec_str(&["git", "diff"]),
            vec_str(&["git", "show", "HEAD"]),
            vec_str(&["git", "branch"]),
            vec_str(&["git", "branch", "--show-current"]),
            vec_str(&["bash", "-lc", "git status"]),
            vec_str(&["bash", "-lc", "git log -1"]),
            vec_str(&["bash", "-lc", "git diff"]),
            vec_str(&["bash", "-lc", "git show HEAD"]),
            vec_str(&["bash", "-lc", "git branch"]),
        ] {
            assert!(
                !is_known_safe_command(&args),
                "expected {args:?} to require approval because Git may invoke repository-configured helpers",
            );
        }
    }

    #[test]
    fn git_branch_mutating_flags_are_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "branch", "-d", "feature"
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "branch",
            "new-branch"
        ])));
    }

    #[test]
    fn git_branch_read_only_flags_still_require_approval() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "branch",
            "--show-current",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "branch", "-d", "feature",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git branch -d feature",
        ])));
    }

    #[test]
    fn git_first_positional_is_the_subcommand() {
        // In git, the first non-option token is the subcommand. Later positional
        // args (like branch names) must not be treated as subcommands.
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "checkout", "status",
        ])));
    }

    #[test]
    fn git_output_flags_are_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "log",
            "--output=/tmp/git-log-out-test",
            "-n",
            "1",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "diff",
            "--output",
            "/tmp/git-diff-out-test",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "show",
            "--output=/tmp/git-show-out-test",
            "HEAD",
        ])));
    }

    #[test]
    fn git_global_pagination_flags_are_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "--paginate",
            "log",
            "-1",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "-p", "log", "-1",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git --paginate log -1",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git -p log -1",
        ])));
    }

    #[test]
    fn git_patch_display_subcommands_require_approval() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "log", "-p", "-1"
        ])));
        assert!(!is_known_safe_command(&vec_str(&["git", "diff", "-p"])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "show", "-p", "HEAD",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git log -p -1",
        ])));
    }

    #[test]
    fn git_global_override_flags_are_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&[
            "git", "-C", ".", "status",
        ])));
        assert!(!is_known_safe_command(&vec_str(&["git", "-C.", "status",])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "-c",
            "core.pager=cat",
            "log",
            "-n",
            "1",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "git",
            "-ccore.pager=cat",
            "status",
        ])));

        for args in [
            vec_str(&["git", "--config-env", "core.pager=PAGER", "show", "HEAD"]),
            vec_str(&["git", "--config-env=core.pager=PAGER", "show", "HEAD"]),
            vec_str(&["git", "--git-dir", ".evil-git", "diff", "HEAD~1..HEAD"]),
            vec_str(&["git", "--git-dir=.evil-git", "diff", "HEAD~1..HEAD"]),
            vec_str(&["git", "--work-tree", ".", "status"]),
            vec_str(&["git", "--work-tree=.", "status"]),
            vec_str(&["git", "--exec-path", ".git/helpers", "show", "HEAD"]),
            vec_str(&["git", "--exec-path=.git/helpers", "show", "HEAD"]),
            vec_str(&["git", "--namespace", "attacker", "show", "HEAD"]),
            vec_str(&["git", "--namespace=attacker", "show", "HEAD"]),
            vec_str(&["git", "--super-prefix", "attacker/", "show", "HEAD"]),
            vec_str(&["git", "--super-prefix=attacker/", "show", "HEAD"]),
        ] {
            assert!(
                !is_known_safe_command(&args),
                "expected {args:?} to require approval due to unsafe git global option",
            );
        }

        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git -C .project-deps/test-fixtures status",
        ])));
        assert!(!is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "git --git-dir=.evil-git diff HEAD~1..HEAD",
        ])));
    }

    #[test]
    fn cargo_check_is_not_safe() {
        assert!(!is_known_safe_command(&vec_str(&["cargo", "check"])));
    }

    #[test]
    fn zsh_lc_safe_command_sequence() {
        assert!(is_known_safe_command(&vec_str(&["zsh", "-lc", "ls"])));
    }

    #[test]
    fn unknown_or_partial() {
        assert!(!is_safe_to_call_with_exec(&vec_str(&["foo"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&["git", "fetch"])));
        assert!(!is_safe_to_call_with_exec(&vec_str(&[
            "sed", "-n", "xp", "file.txt"
        ])));

        // Unsafe `find` commands.
        for args in [
            vec_str(&["find", ".", "-name", "file.txt", "-exec", "rm", "{}", ";"]),
            vec_str(&[
                "find", ".", "-name", "*.py", "-execdir", "python3", "{}", ";",
            ]),
            vec_str(&["find", ".", "-name", "file.txt", "-ok", "rm", "{}", ";"]),
            vec_str(&["find", ".", "-name", "*.py", "-okdir", "python3", "{}", ";"]),
            vec_str(&["find", ".", "-delete", "-name", "file.txt"]),
            vec_str(&["find", ".", "-fls", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprint0", "/etc/passwd"]),
            vec_str(&["find", ".", "-fprintf", "/root/suid.txt", "%#m %u %p\n"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be unsafe"
            );
        }
    }

    #[test]
    fn base64_output_options_are_unsafe() {
        for args in [
            vec_str(&["base64", "-o", "out.bin"]),
            vec_str(&["base64", "--output", "out.bin"]),
            vec_str(&["base64", "--output=out.bin"]),
            vec_str(&["base64", "-ob64.txt"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be considered unsafe due to output option"
            );
        }
    }

    #[test]
    fn ripgrep_rules() {
        // Safe ripgrep invocations – none of the unsafe flags are present.
        assert!(is_safe_to_call_with_exec(&vec_str(&[
            "rg",
            "Cargo.toml",
            "-n"
        ])));

        // Unsafe flags that do not take an argument (present verbatim).
        for args in [
            vec_str(&["rg", "--search-zip", "files"]),
            vec_str(&["rg", "-z", "files"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be considered unsafe due to zip-search flag",
            );
        }

        // Unsafe flags that expect a value, provided in both split and = forms.
        for args in [
            vec_str(&["rg", "--pre", "pwned", "files"]),
            vec_str(&["rg", "--pre=pwned", "files"]),
            vec_str(&["rg", "--hostname-bin", "pwned", "files"]),
            vec_str(&["rg", "--hostname-bin=pwned", "files"]),
        ] {
            assert!(
                !is_safe_to_call_with_exec(&args),
                "expected {args:?} to be considered unsafe due to external-command flag",
            );
        }
    }

    #[test]
    fn windows_powershell_full_path_is_safe() {
        if !cfg!(windows) {
            // Windows only because on Linux path splitting doesn't handle `/` separators properly
            return;
        }

        let Some(powershell) = crate::powershell::try_find_pwsh_executable_blocking()
            .or_else(crate::powershell::try_find_powershell_executable_blocking)
        else {
            return;
        };
        let powershell = powershell.as_path().to_str().unwrap();

        assert!(is_known_safe_command(&vec_str(&[
            powershell,
            "-Command",
            "Get-Location",
        ])));
    }

    #[test]
    fn windows_git_full_path_requires_approval() {
        if !cfg!(windows) {
            return;
        }

        assert!(!is_known_safe_command(&vec_str(&[
            r"C:\Program Files\Git\cmd\git.exe",
            "status",
        ])));
    }

    #[test]
    fn bash_lc_safe_examples() {
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls"])));
        assert!(is_known_safe_command(&vec_str(&["bash", "-lc", "ls -1"])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep -R \"Cargo.toml\" -n"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n 1,5p file.txt"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "sed -n '1,5p' file.txt"
        ])));

        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "find . -name file.txt"
        ])));
    }

    #[test]
    fn bash_lc_safe_examples_with_operators() {
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "grep -R \"Cargo.toml\" -n || true"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls && pwd"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "echo 'hi' ; ls"
        ])));
        assert!(is_known_safe_command(&vec_str(&[
            "bash",
            "-lc",
            "ls | wc -l"
        ])));
    }

    #[test]
    fn bash_lc_unsafe_examples() {
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "git", "status"])),
            "Four arg version is not known to be safe."
        );
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "'git status'"])),
            "The extra quoting around 'git status' makes it a program named 'git status' and is therefore unsafe."
        );

        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "find . -name file.txt -delete"])),
            "Unsafe find option should not be auto-approved."
        );

        // Disallowed because of unsafe command in sequence.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls && rm -rf /"])),
            "Sequence containing unsafe command must be rejected"
        );

        // Disallowed because of parentheses / subshell.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "(ls)"])),
            "Parentheses (subshell) are not provably safe with the current parser"
        );
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls || (pwd && echo hi)"])),
            "Nested parentheses are not provably safe with the current parser"
        );

        // Disallowed redirection.
        assert!(
            !is_known_safe_command(&vec_str(&["bash", "-lc", "ls > out.txt"])),
            "> redirection should be rejected"
        );
    }

    #[test]
    fn direct_powershell_words_use_windows_safelist() {
        let command = vec_str(&["Get-Content", "Cargo.toml"]);

        if cfg!(windows) {
            assert!(is_safe_powershell_words(&command));
        } else {
            assert!(!is_safe_powershell_words(&command));
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_windows_safe_classification_does_not_spawn_repo_powershell_path() {
        use std::fs;
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::time::SystemTime;
        use std::time::UNIX_EPOCH;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "codex-safe-command-pwsh-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&temp_dir).expect("create temp dir for fake pwsh");
        let fake_pwsh = temp_dir.join("pwsh");
        let marker = temp_dir.join("marker");
        let quoted_marker = marker.to_string_lossy().replace('\'', "'\\''");

        let mut script = fs::File::create(&fake_pwsh).expect("create fake pwsh");
        writeln!(
            script,
            "#!/bin/sh\nprintf spawned > '{quoted_marker}'\nexit 0"
        )
        .expect("write fake pwsh");
        let mut permissions = fs::metadata(&fake_pwsh)
            .expect("stat fake pwsh")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_pwsh, permissions).expect("make fake pwsh executable");

        assert!(!is_known_safe_command(&[
            fake_pwsh.to_string_lossy().into_owned(),
            "-Command".to_string(),
            "Get-ChildItem".to_string(),
        ]));
        assert!(
            !marker.exists(),
            "non-Windows safety classification must not spawn a PowerShell-looking path"
        );

        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }
}
