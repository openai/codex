//! Sanitization for untrusted terminal-title and tab-status OSC text.

/// Whether a control or invisible formatting character is unsafe in OSC text.
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
