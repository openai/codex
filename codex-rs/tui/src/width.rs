//! Terminal display-width helpers and guards for fixed prefix columns.
//!
//! Several rendering paths reserve a fixed number of columns for bullets,
//! gutters, or labels before laying out content.  When the terminal is very
//! narrow, those reserved columns can consume the entire width, leaving zero
//! or negative space for content.
//!
//! The display-width helpers match Ratatui's terminal-cell semantics while retaining `usize`
//! precision for long lines. The guards centralise subtraction and enforce a strict-positive
//! contract: they return `Some(n)` where `n > 0`, or `None` when no usable
//! content width remains.  Callers treat `None` as "render prefix-only
//! fallback" rather than attempting wrapped rendering at zero width, which
//! would produce empty or unstable output.

use ratatui::buffer::CellWidth;
use ratatui::text::Line;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Returns the display width Ratatui uses for terminal text without its `u16` limit.
pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
        + text
            .chars()
            .filter(|ch| matches!(ch, '\u{FF9E}' | '\u{FF9F}'))
            .count()
}

/// Counts rows occupied by a line soft-wrapped at a nonzero terminal width.
/// A wide character can leave the last column unused when the terminal wraps.
pub(crate) fn physical_row_count(line: &Line<'_>, wrap_width: usize) -> usize {
    let mut cells = 0;
    for grapheme in line.styled_graphemes(line.style) {
        let width = usize::from(grapheme.symbol.cell_width());
        // A cluster such as ｶﾞ can straddle the edge. Count a gap only when
        // its first character is itself wide, retaining Ratatui's cluster width.
        if width > 1
            && wrap_width > 1
            && cells % wrap_width == wrap_width - 1
            && grapheme
                .symbol
                .chars()
                .next()
                .and_then(UnicodeWidthChar::width)
                == Some(2)
        {
            cells += 1;
        }
        cells += width;
    }
    cells.max(1).div_ceil(wrap_width)
}

/// Returns usable content width after reserving fixed columns.
///
/// Guarantees a strict positive width (`Some(n)` where `n > 0`) or `None` when
/// the reserved columns consume the full width.
///
/// Treat `None` as "render prefix-only fallback". Coercing it to `0` and still
/// attempting wrapped rendering often produces empty or unstable output at very
/// narrow terminal widths.
pub(crate) fn usable_content_width(total_width: usize, reserved_cols: usize) -> Option<usize> {
    total_width
        .checked_sub(reserved_cols)
        .filter(|remaining| *remaining > 0)
}

/// `u16` convenience wrapper around [`usable_content_width`].
///
/// This keeps width math at callsites that receive terminal dimensions as
/// `u16` while preserving the same `None` contract for exhausted width.
pub(crate) fn usable_content_width_u16(total_width: u16, reserved_cols: u16) -> Option<usize> {
    usable_content_width(usize::from(total_width), usize::from(reserved_cols))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_truncation::line_width;
    use pretty_assertions::assert_eq;

    #[test]
    fn display_width_matches_ratatui_halfwidth_sound_marks_without_overflow() {
        assert_eq!(display_width("ｶﾞﾊﾟ"), 4);
        assert_eq!(display_width("ｶﾞﾞ"), 3);
        assert_eq!(display_width("界ﾞ"), 3);

        let text = "a".repeat(65_536);
        assert_eq!(display_width(&text), 65_536);
        assert_eq!(line_width(&Line::from(text)), 65_536);
    }

    #[test]
    fn narrow_characters_in_a_cluster_can_straddle_the_right_edge() {
        for cluster in ["ｶﾞ", "क्ष"] {
            let line = Line::from(format!("https://example.co/{cluster}1234567890123456789"));
            assert_eq!(physical_row_count(&line, /*wrap_width*/ 20), 2);
        }
    }

    #[test]
    fn usable_content_width_returns_none_when_reserved_exhausts_width() {
        assert_eq!(
            usable_content_width(/*total_width*/ 0, /*reserved_cols*/ 0),
            None
        );
        assert_eq!(
            usable_content_width(/*total_width*/ 2, /*reserved_cols*/ 2),
            None
        );
        assert_eq!(
            usable_content_width(/*total_width*/ 3, /*reserved_cols*/ 4),
            None
        );
        assert_eq!(
            usable_content_width(/*total_width*/ 5, /*reserved_cols*/ 4),
            Some(1)
        );
    }

    #[test]
    fn usable_content_width_u16_matches_usize_variant() {
        assert_eq!(
            usable_content_width_u16(/*total_width*/ 2, /*reserved_cols*/ 2),
            None
        );
        assert_eq!(
            usable_content_width_u16(/*total_width*/ 5, /*reserved_cols*/ 4),
            Some(1)
        );
    }
}
