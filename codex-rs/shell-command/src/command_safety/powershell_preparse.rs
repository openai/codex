/// Returns whether a PowerShell source must be rejected before calling `Parser::ParseInput`.
///
/// `ParseInput` is not a side-effect-free syntax parser. Its semantic passes can resolve `using`
/// directives and initialize DSC configuration keywords, which may inspect or load a
/// source-selected module path. Keep the pre-parser deliberately coarse and auditable: any raw
/// mention of those language forms, or any backtick that could obscure spelling, is handled by
/// the outer-command policy without sending the source to System.Management.Automation.
pub(super) fn requires_preparse_rejection(source: &str) -> bool {
    source.contains('`')
        || ["using", "configuration", "import-dscresource"]
            .into_iter()
            .any(|keyword| contains_ignore_ascii_case(source, keyword))
}

fn contains_ignore_ascii_case(source: &str, needle: &str) -> bool {
    source
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
#[path = "powershell_preparse_tests.rs"]
mod tests;
