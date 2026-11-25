//! Visual order for message prose on terminals that leave ordering to the application. Resolve direction on
//! the logical paragraph, then permute graphemes only after wrapping. The terminal
//! still supplies glyph shaping. Never use the visual line as a copy or reflow source.

use super::HyperlinkLine;
use super::line_text;
use super::push_link_range;
use super::push_styled_content;
use crate::text_formatting::contains_rtl_or_arabic_number;
use crate::width::display_width;
use icu_properties::CodePointMapData;
use icu_properties::props::BidiMirroringGlyph;
use ratatui::text::Line;
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use unicode_bidi::BidiClass;
use unicode_bidi::Level;
use unicode_bidi::ParagraphBidiInfo;
use unicode_bidi::bidi_class;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

pub(crate) fn prepare_message(lines: &mut [HyperlinkLine]) {
    for source in lines.iter_mut().filter_map(|line| line.source.as_mut()) {
        source.message = true;
    }
}

pub(crate) fn session_lines(mut lines: Vec<HyperlinkLine>, enabled: bool) -> Vec<HyperlinkLine> {
    if enabled {
        resolve_paragraphs(&mut lines);
    }
    lines
}

pub(crate) fn resolve_paragraphs(lines: &mut [HyperlinkLine]) {
    for group in lines.chunk_by_mut(|a, b| {
        matches!(
            (&a.source, &b.source),
            (Some(a), Some(b)) if Arc::ptr_eq(&a.text, &b.text)
        )
    }) {
        let Some(source) = &group[0].source else {
            continue;
        };
        // Invisible direction changes can disguise executable syntax. Allow zero-width
        // nonspacing marks and boundary neutrals (e.g. joiners), which set no direction.
        if !source.message || source
            .copy
            .as_ref()
            .is_some_and(|copy| copy.code || copy.table.is_some() || copy.code_source.is_some())
            // Do not move text across shell operators or substitution markers.
            || source.text.contains(['\t', ';', '|', '<', '>', '$', '`'])
            || !contains_rtl_or_arabic_number(&source.text)
            || source.text.chars().any(|c| {
                c.width() == Some(0) && !matches!(bidi_class(c), BidiClass::NSM | BidiClass::BN)
            })
        {
            continue;
        }
        // Treat URLs and literals as strong LTR tokens without changing byte offsets.
        let mut text = Cow::Borrowed(source.text.as_ref());
        for (range, _) in super::web_link_ranges(&source.text) {
            text.to_mut()
                .replace_range(range.clone(), &"a".repeat(range.len()));
        }
        // Exempt URL fragments and query separators, but keep checking literals.
        if text.contains(['#', '&']) {
            continue;
        }
        for range in source.copy.iter().flat_map(|copy| &copy.literal_ranges) {
            if text.get(range.clone()).is_some() {
                text.to_mut()
                    .replace_range(range.clone(), &"a".repeat(range.len()));
            }
        }
        // An unquoted URL's & is also a shell operator. An LTR base keeps the
        // masked URL at level zero, so no text can move across that boundary.
        let base = source.text.contains('&').then_some(Level::ltr());
        let levels: Arc<[Level]> = ParagraphBidiInfo::new(&text, base).levels.into();
        for source in group.iter_mut().filter_map(|line| line.source.as_mut()) {
            source.bidi = Some(Arc::clone(&levels));
        }
    }
}

/// A projected row and the logical body bytes under each of its visible body cells.
pub(crate) struct VisualRow {
    pub(crate) line: HyperlinkLine,
    pub(crate) offsets: Vec<Range<usize>>,
}

pub(crate) fn paint_rows(lines: &mut [HyperlinkLine], width: usize) {
    resolve_paragraphs(lines);
    for line in lines {
        if let Some(source) = &line.source
            && let Some(levels) = &source.bidi
            && let Some(levels) = levels.get(source.range.clone())
            && let Some(visual) = project(line, levels, width)
        {
            *line = visual.line;
        }
    }
}

pub(crate) fn project(line: &HyperlinkLine, levels: &[Level], width: usize) -> Option<VisualRow> {
    let source = line.source.as_ref()?;
    // Never reorder across terminal autowrap boundaries (e.g. an oversized URL).
    if line.width() > width || levels.len() != source.range.len() {
        return None;
    }
    let text = line_text(&line.line);
    let start = source.prefix_bytes;
    let body = text.get(start..start + levels.len())?;
    if Some(body) != source.text.get(source.range.clone()) {
        return None;
    }
    // Keep synthetic gutters and trailing wrap padding outside the bidi run.
    let end = start + body.trim_end().len();
    let mut spans = line.line.spans.iter();
    let mut span = spans.next()?;
    let mut span_end = span.content.len();
    let mut column = 0;
    let mut links = line.hyperlinks.iter().peekable();
    let mut graphemes = Vec::new();
    let mut row_levels = Vec::new();
    for (byte, grapheme) in text.grapheme_indices(/*is_extended*/ true) {
        while byte >= span_end {
            span = spans.next()?;
            span_end += span.content.len();
        }
        let level = if (start..end).contains(&byte) {
            levels[byte - start]
        } else {
            Level::ltr()
        };
        row_levels.push(level);
        let cells = display_width(grapheme);
        while links.peek().is_some_and(|link| link.columns.end <= column) {
            links.next();
        }
        let link = links
            .peek()
            .copied()
            .filter(|link| link.columns.contains(&column));
        graphemes.push((byte, grapheme, cells, span.style, link));
        column += cells;
    }
    let mut visual = HyperlinkLine::new(Line::default().style(line.line.style));
    visual.line.alignment = line.line.alignment;
    let mut offsets = Vec::new();
    column = 0;
    for index in ParagraphBidiInfo::reorder_visual(&row_levels) {
        let (byte, grapheme, cells, style, link) = graphemes[index];
        if cells == 0 {
            continue;
        }
        let mut symbol = Cow::Borrowed(grapheme);
        if row_levels[index].is_rtl()
            && let Some(base) = grapheme.chars().next()
            && let Some(mirror) = CodePointMapData::<BidiMirroringGlyph>::new()
                .get(base)
                .mirroring_glyph
        {
            symbol
                .to_mut()
                .replace_range(..base.len_utf8(), mirror.encode_utf8(&mut [0; 4]));
        }
        push_styled_content(&mut visual.line.spans, &symbol, style);
        if let Some(link) = link {
            push_link_range(&mut visual, column..column + cells, link);
        }
        if (start..start + body.len()).contains(&byte) {
            let offset = byte - start;
            offsets.resize(offsets.len() + cells, offset..offset + grapheme.len());
        }
        column += cells;
    }
    Some(VisualRow {
        line: visual,
        offsets,
    })
}
