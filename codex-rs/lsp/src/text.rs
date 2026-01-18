use lsp_types::Position;
use lsp_types::PositionEncodingKind;

pub fn offset_for_position(
    text: &str,
    position: Position,
    encoding: &PositionEncodingKind,
) -> Option<usize> {
    let target_line = position.line as usize;
    let target_character = position.character as usize;
    let (line_start, line_text) = line_start_and_text(text, target_line)?;

    let offset_in_line = if encoding == &PositionEncodingKind::UTF8 {
        offset_for_utf8(line_text, target_character)?
    } else if encoding == &PositionEncodingKind::UTF32 {
        offset_for_utf32(line_text, target_character)?
    } else {
        offset_for_utf16(line_text, target_character)?
    };
    Some(line_start + offset_in_line)
}

pub fn position_for_offset(
    text: &str,
    offset: usize,
    encoding: &PositionEncodingKind,
) -> Option<Position> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let mut line = 0usize;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }

    let line_text = text.get(line_start..)?;
    let relative = offset.saturating_sub(line_start);
    let character = if encoding == &PositionEncodingKind::UTF8 {
        relative as u32
    } else if encoding == &PositionEncodingKind::UTF32 {
        utf32_units_for_prefix(line_text, relative)?
    } else {
        utf16_units_for_prefix(line_text, relative)?
    };

    Some(Position {
        line: line as u32,
        character,
    })
}

fn line_start_and_text(text: &str, target_line: usize) -> Option<(usize, &str)> {
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if line == target_line {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    if line != target_line {
        return None;
    }
    let line_text = text.get(line_start..)?.split('\n').next().unwrap_or("");
    Some((line_start, line_text))
}

fn offset_for_utf8(line_text: &str, target: usize) -> Option<usize> {
    if target > line_text.len() {
        return None;
    }
    if !line_text.is_char_boundary(target) {
        return None;
    }
    Some(target)
}

fn offset_for_utf16(line_text: &str, target: usize) -> Option<usize> {
    let mut units = 0usize;
    for (idx, ch) in line_text.char_indices() {
        if units == target {
            return Some(idx);
        }
        let next_units = units + ch.len_utf16();
        if next_units > target {
            return None;
        }
        units = next_units;
    }
    if units == target {
        Some(line_text.len())
    } else {
        None
    }
}

fn offset_for_utf32(line_text: &str, target: usize) -> Option<usize> {
    let mut count = 0usize;
    for (idx, _ch) in line_text.char_indices() {
        if count == target {
            return Some(idx);
        }
        count += 1;
    }
    if count == target {
        Some(line_text.len())
    } else {
        None
    }
}

fn utf16_units_for_prefix(text: &str, byte_len: usize) -> Option<u32> {
    if byte_len > text.len() || !text.is_char_boundary(byte_len) {
        return None;
    }
    let mut units = 0u32;
    for ch in text[..byte_len].chars() {
        units += ch.len_utf16() as u32;
    }
    Some(units)
}

fn utf32_units_for_prefix(text: &str, byte_len: usize) -> Option<u32> {
    if byte_len > text.len() || !text.is_char_boundary(byte_len) {
        return None;
    }
    Some(text[..byte_len].chars().count() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::PositionEncodingKind;
    use pretty_assertions::assert_eq;

    #[test]
    fn utf16_offset_roundtrip_with_emoji() {
        let text = "a🦀b\nc";
        let pos_a = Position {
            line: 0,
            character: 1,
        };
        let pos_emoji_end = Position {
            line: 0,
            character: 3,
        };
        let offset_a = offset_for_position(text, pos_a, &PositionEncodingKind::UTF16).unwrap();
        let offset_emoji =
            offset_for_position(text, pos_emoji_end, &PositionEncodingKind::UTF16).unwrap();
        assert_eq!(offset_a, 1);
        assert_eq!(offset_emoji, 1 + "🦀".len());

        let roundtrip = position_for_offset(text, offset_emoji, &PositionEncodingKind::UTF16)
            .expect("position");
        assert_eq!(roundtrip, pos_emoji_end);
    }

    #[test]
    fn utf8_offset_counts_bytes() {
        let text = "a🦀b";
        let pos = Position {
            line: 0,
            character: 1 + "🦀".len() as u32,
        };
        let offset = offset_for_position(text, pos, &PositionEncodingKind::UTF8).unwrap();
        assert_eq!(offset, 1 + "🦀".len());
    }

    #[test]
    fn utf32_offset_counts_scalars() {
        let text = "a🦀b";
        let pos = Position {
            line: 0,
            character: 2,
        };
        let offset = offset_for_position(text, pos, &PositionEncodingKind::UTF32).unwrap();
        assert_eq!(offset, 1 + "🦀".len());
    }
}
