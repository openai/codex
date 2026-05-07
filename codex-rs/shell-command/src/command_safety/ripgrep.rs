use std::borrow::Cow;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum RipgrepArgCase {
    Sensitive,
    AsciiInsensitive,
}

pub(crate) fn is_safe_ripgrep_command(command: &[String], arg_case: RipgrepArgCase) -> bool {
    !command
        .iter()
        .skip(1)
        .map(String::as_str)
        .any(|arg| is_unsafe_ripgrep_arg(arg, arg_case))
}

pub(crate) fn ripgrep_command_can_execute_arbitrary_command(
    command: &[String],
    arg_case: RipgrepArgCase,
) -> bool {
    command.iter().skip(1).map(String::as_str).any(|arg| {
        let normalized = normalized_long_arg(arg, arg_case);
        ripgrep_arg_can_execute_arbitrary_command(normalized.as_ref())
    })
}

fn is_unsafe_ripgrep_arg(arg: &str, arg_case: RipgrepArgCase) -> bool {
    let normalized = normalized_long_arg(arg, arg_case);
    if ripgrep_arg_can_execute_arbitrary_command(normalized.as_ref()) {
        return true;
    }

    match normalized.as_ref() {
        // Calls out to other decompression tools, so do not auto-approve
        // out of an abundance of caution.
        "--search-zip" => true,
        _ => {
            normalized.starts_with("--search-zip=")
                || ripgrep_short_options_contain_search_zip(arg, arg_case)
        }
    }
}

fn ripgrep_arg_can_execute_arbitrary_command(normalized_arg: &str) -> bool {
    matches!(
        normalized_arg,
        // Takes an arbitrary command that is executed for each match.
        "--pre"
        // Takes a command that can be used to obtain the local hostname.
        | "--hostname-bin"
    ) || normalized_arg.starts_with("--pre=")
        || normalized_arg.starts_with("--hostname-bin=")
}

fn normalized_long_arg(arg: &str, arg_case: RipgrepArgCase) -> Cow<'_, str> {
    if !arg.starts_with("--") {
        return Cow::Borrowed(arg);
    }

    match arg_case {
        RipgrepArgCase::Sensitive => Cow::Borrowed(arg),
        RipgrepArgCase::AsciiInsensitive => Cow::Owned(arg.to_ascii_lowercase()),
    }
}

fn ripgrep_short_options_contain_search_zip(arg: &str, arg_case: RipgrepArgCase) -> bool {
    let Some(short_options) = arg.strip_prefix('-') else {
        return false;
    };
    if short_options.is_empty() || short_options.starts_with('-') {
        return false;
    }

    for option in short_options.chars() {
        if option == 'z' || (arg_case == RipgrepArgCase::AsciiInsensitive && option == 'Z') {
            return true;
        }
        if ripgrep_short_option_takes_value(option) {
            return false;
        }
    }

    false
}

fn ripgrep_short_option_takes_value(option: char) -> bool {
    matches!(
        option,
        'A' | 'B' | 'C' | 'E' | 'M' | 'T' | 'd' | 'e' | 'f' | 'g' | 'j' | 'm' | 'r' | 't'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn rejects_ripgrep_options_that_can_spawn_processes() {
        for args in [
            vec_str(&["rg", "--pre", "pwned", "files"]),
            vec_str(&["rg", "--pre=pwned", "files"]),
            vec_str(&["rg", "--hostname-bin", "pwned", "files"]),
            vec_str(&["rg", "--hostname-bin=pwned", "files"]),
            vec_str(&["rg", "--search-zip", "files"]),
            vec_str(&["rg", "--search-zip=true", "files"]),
            vec_str(&["rg", "-z", "files"]),
            vec_str(&["rg", "-zn", "files"]),
            vec_str(&["rg", "-nz", "files"]),
        ] {
            assert!(
                !is_safe_ripgrep_command(&args, RipgrepArgCase::Sensitive),
                "expected {args:?} to be unsafe",
            );
        }
    }

    #[test]
    fn case_insensitive_matching_preserves_short_option_value_shape() {
        for args in [
            vec_str(&["rg", "--PRE", "pwned", "files"]),
            vec_str(&["rg", "--HOSTNAME-BIN=pwned", "files"]),
            vec_str(&["rg", "--SEARCH-ZIP", "files"]),
            vec_str(&["rg", "-Z", "files"]),
            vec_str(&["rg", "-Fz", "needle", "."]),
        ] {
            assert!(
                !is_safe_ripgrep_command(&args, RipgrepArgCase::AsciiInsensitive),
                "expected {args:?} to be unsafe with case-insensitive matching",
            );
        }

        let args = vec_str(&["rg", "-fz", "needle", "."]);
        assert!(
            is_safe_ripgrep_command(&args, RipgrepArgCase::AsciiInsensitive),
            "expected lowercase -f to consume z as its pattern-file value",
        );
    }
}
