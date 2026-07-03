/// Canonicalize command argv for approval-cache matching.
///
/// Approval identity preserves the literal executable, wrapper arguments, and
/// script text. Each affects the authorization boundary, so even semantically
/// similar argv vectors remain distinct cache entries.
pub(crate) fn canonicalize_command_for_approval(command: &[String]) -> Vec<String> {
    command.to_vec()
}

#[cfg(test)]
#[path = "command_canonicalization_tests.rs"]
mod tests;
