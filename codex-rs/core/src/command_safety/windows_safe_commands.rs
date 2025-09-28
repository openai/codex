pub fn is_safe_command_windows(command: &[String]) -> bool {
    let cmd0 = command.first().map(String::as_str);

    match cmd0 {
        #[rustfmt::skip]
        Some(
            "cat" |
            "cd" |
            "echo" |
            "false" |
            "grep" |
            "head" |
            "ls" |
            "nl" |
            "pwd" |
            "tail" |
            "true" |
            "wc" |
            "which") => true,

        Some("find") => {
            #[rustfmt::skip]
            const UNSAFE_FIND_OPTIONS: &[&str] = &[
                "-exec", "-execdir", "-ok", "-okdir",
                "-delete",
                "-fls", "-fprint", "-fprint0", "-fprintf",
            ];

            !command
                .iter()
                .any(|arg| UNSAFE_FIND_OPTIONS.contains(&arg.as_str()))
        }

        Some("rg") => {
            const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &["--pre", "--hostname-bin"];
            const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &["--search-zip", "-z"];

            !command.iter().any(|arg| {
                UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg.as_str())
                    || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                        .iter()
                        .any(|&opt| arg == opt || arg.starts_with(&format!("{opt}=")))
            })
        }

        Some("git") => matches!(
            command.get(1).map(String::as_str),
            Some("branch" | "status" | "log" | "diff" | "show"),
        ),

        Some("cargo") if command.get(1).map(String::as_str) == Some("check") => true,

        Some("sed")
            if {
                command.len() == 4
                    && command.get(1).map(String::as_str) == Some("-n")
                    && is_valid_sed_n_arg(command.get(2).map(String::as_str))
                    && command.get(3).map(String::is_empty) == Some(false)
            } =>
        {
            true
        }

        _ => false,
    }
}

fn is_valid_sed_n_arg(arg: Option<&str>) -> bool {
    let s = match arg {
        Some(s) => s,
        None => return false,
    };

    let core = match s.strip_suffix('p') {
        Some(rest) => rest,
        None => return false,
    };

    let parts: Vec<&str> = core.split(',').collect();
    match parts.as_slice() {
        [num] => !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()),
        [a, b] => {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_safe_command_windows;
    use std::string::ToString;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn known_safe_examples() {
        assert!(is_safe_command_windows(&vec_str(&["ls"])));
        assert!(is_safe_command_windows(&vec_str(&["git", "status"])));
        assert!(is_safe_command_windows(&vec_str(&[
            "sed", "-n", "1,5p", "file.txt"
        ])));
        assert!(is_safe_command_windows(&vec_str(&[
            "nl",
            "-nrz",
            "Cargo.toml"
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "find", ".", "-name", "file.txt"
        ])));
    }

    #[test]
    fn unknown_or_partial() {
        assert!(!is_safe_command_windows(&vec_str(&["foo"])));
        assert!(!is_safe_command_windows(&vec_str(&["git", "fetch"])));
        assert!(!is_safe_command_windows(&vec_str(&[
            "sed", "-n", "xp", "file.txt"
        ])));

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
                !is_safe_command_windows(&args),
                "expected {args:?} to be unsafe"
            );
        }
    }

    #[test]
    fn ripgrep_rules() {
        assert!(is_safe_command_windows(&vec_str(&[
            "rg",
            "Cargo.toml",
            "-n"
        ])));

        for args in [
            vec_str(&["rg", "--search-zip", "files"]),
            vec_str(&["rg", "-z", "files"]),
        ] {
            assert!(
                !is_safe_command_windows(&args),
                "expected {args:?} to be considered unsafe due to zip-search flag",
            );
        }

        for args in [
            vec_str(&["rg", "--pre", "pwned", "files"]),
            vec_str(&["rg", "--pre=pwned", "files"]),
            vec_str(&["rg", "--hostname-bin", "pwned", "files"]),
            vec_str(&["rg", "--hostname-bin=pwned", "files"]),
        ] {
            assert!(
                !is_safe_command_windows(&args),
                "expected {args:?} to be considered unsafe due to external-command flag",
            );
        }
    }
}
