/// Returns whether a PowerShell source must be rejected before calling `Parser::ParseInput`.
///
/// `ParseInput` is not a side-effect-free syntax parser. Its semantic passes can resolve `using`
/// directives, initialize DSC configuration keywords, and resolve assembly-qualified type names,
/// any of which may inspect or load a source-selected path. Keep the pre-parser deliberately
/// coarse and auditable: suspicious raw forms are handled by the outer-command policy without
/// sending the source to System.Management.Automation.
pub(super) fn requires_preparse_rejection(source: &str) -> bool {
    source.contains('`')
        || contains_bracket_before_later_comma(source)
        || ["using", "configuration", "import-dscresource"]
            .into_iter()
            .any(|keyword| contains_ignore_ascii_case(source, keyword))
}

/// Assembly-qualified PowerShell type syntax contains `[` before a later `,`. Once an opening
/// bracket is seen, deliberately never reset on `]`: comments and strings can contain a closing
/// bracket before the real assembly delimiter, while `ParseInput` still resolves the assembly.
fn contains_bracket_before_later_comma(source: &str) -> bool {
    let mut saw_bracket = false;
    for byte in source.bytes() {
        match byte {
            b'[' => saw_bracket = true,
            b',' if saw_bracket => return true,
            _ => {}
        }
    }
    false
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
