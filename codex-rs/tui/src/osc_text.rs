//! Shared sanitization helpers for untrusted text placed inside OSC sequences.
//!
//! Several emitters (the terminal title in `terminal_title.rs`, the OSC 21337
//! tab-status detail in `tab_status.rs`) assemble display strings from
//! untrusted sources such as model output, command argv, MCP server messages,
//! thread names, and project paths. Before that text goes into an OSC payload
//! it has to be stripped of two distinct hazards, and both modules need the
//! exact same rule so they cannot drift:
//!
//! - Control characters that could terminate or reshape the escape sequence
//!   (BEL, ESC, the C1 controls, etc.).
//! - Bidi/invisible formatting codepoints that can visually reorder or hide
//!   text (the family of issues described in the Trojan Source writeups). These
//!   are not `char::is_control()`, so they have to be enumerated.

/// Returns whether `ch` must be dropped from any OSC display payload.
///
/// Covers both plain control characters and a curated set of invisible
/// formatting codepoints. The bidi entries cover the Trojan-Source-style
/// text-reordering controls that can make a string render misleadingly relative
/// to its underlying byte sequence.
pub(crate) fn is_disallowed_osc_text_char(ch: char) -> bool {
    if ch.is_control() {
        return true;
    }

    matches!(
        ch,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

#[cfg(test)]
#[path = "osc_text_tests.rs"]
mod tests;
