/// Returns whether `git config --null --get core.fsmonitor` reported canonical
/// boolean `true`.
pub fn is_canonical_fsmonitor_true(output: &[u8]) -> bool {
    output == b"true\0"
}

/// Returns whether `git version --build-options` advertises the built-in
/// fsmonitor daemon.
pub fn supports_builtin_fsmonitor(output: &[u8]) -> bool {
    output
        .split(|byte| *byte == b'\n')
        .any(|line| line.trim_ascii() == b"feature: fsmonitor--daemon")
}
