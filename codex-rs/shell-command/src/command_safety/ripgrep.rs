pub(crate) fn is_safe_ripgrep_command(command: &[String]) -> bool {
    !command
        .iter()
        .skip(1)
        .map(String::as_str)
        .any(is_unsafe_ripgrep_arg)
}

pub(crate) fn ripgrep_command_can_execute_arbitrary_command(command: &[String]) -> bool {
    command
        .iter()
        .skip(1)
        .map(String::as_str)
        .any(ripgrep_arg_can_execute_arbitrary_command)
}

fn is_unsafe_ripgrep_arg(arg: &str) -> bool {
    if ripgrep_arg_can_execute_arbitrary_command(arg) {
        return true;
    }

    match arg {
        // Calls out to other decompression tools, so do not auto-approve
        // out of an abundance of caution.
        "--search-zip" => true,
        "-z" => true,
        _ => false,
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
            vec_str(&["rg", "-z", "files"]),
        ] {
            assert!(
                !is_safe_ripgrep_command(&args),
                "expected {args:?} to be unsafe",
            );
        }
    }
}
