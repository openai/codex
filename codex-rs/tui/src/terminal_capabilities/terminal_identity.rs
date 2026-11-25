//! Recognize Terminal.app's device attributes without interpreting pasted text as a reply.

/// A complete pair is required; missing or incomplete replies remain unknown.
pub(super) fn is_apple_terminal(input: &[u8]) -> Option<bool> {
    let primary = device_attributes(input, b"[?")?;
    let secondary = device_attributes(input, b"[>")?;
    Some(primary == b"1;2" && secondary == b"1;95;0")
}

fn device_attributes<'a>(input: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    let mut inside_paste = false;
    for sequence in input.split(|byte| *byte == b'\x1b').skip(/*n*/ 1) {
        match sequence {
            [b'[', b'2', b'0', b'0', b'~', ..] => inside_paste = true,
            [b'[', b'2', b'0', b'1', b'~', ..] => inside_paste = false,
            _ if !inside_paste => {
                if let Some(payload) = sequence.strip_prefix(prefix)
                    && let Some(end) = payload
                        .iter()
                        .take(/*n*/ 64)
                        .position(|byte| (0x40..=0x7e).contains(byte))
                    && payload[end] == b'c'
                {
                    return Some(&payload[..end]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "terminal_identity_tests.rs"]
mod tests;
