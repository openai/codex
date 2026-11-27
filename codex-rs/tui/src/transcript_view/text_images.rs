//! Substitute previews for labels in the painted rows, retaining logical text for copy and Find.

use super::LogicalLine;
use super::TextLayout;
use crate::history_cell::HistoryCell;
use crate::pets::ImageProtocol;
use crate::terminal_hyperlinks::project_hyperlink_columns;
use crate::terminal_hyperlinks::source_text_width;
use crate::transcript_image::TranscriptImage;
use crate::transcript_view::markdown_images::MarkdownImages;
use crate::transcript_view::markdown_images::Request;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct InlineImage {
    label: Range<usize>,
    request: Request,
    image: Option<Arc<TranscriptImage>>,
    // First row measured from the end, so prepending a spacer preserves the placement.
    region: Option<(usize, u16)>,
}

pub(in crate::transcript_view) fn retained_bytes(layout: &TextLayout) -> usize {
    layout
        .inline_images
        .iter()
        .filter_map(|i| i.image.as_ref())
        .map(|i| i.pixels.len())
        .sum()
}

pub(in crate::transcript_view) fn attach(
    layout: &mut TextLayout,
    reply: &Arc<dyn HistoryCell>,
    protocol: ImageProtocol,
    cell_size: Option<(u32, u32)>,
    cache: &MarkdownImages,
) {
    let mut images: Vec<InlineImage> = Vec::new();
    for ((start, source), line) in layout.logical_sources().zip(&layout.logical) {
        let Some(copy) = &source.copy else { continue };
        for marker in &copy.images {
            let offset = |byte: usize| {
                start + (byte.clamp(source.range.start, source.range.end) - source.range.start)
            };
            let label = offset(marker.label.start)..offset(marker.label.end);
            // Fragments of the same multiline label share the original allocation.
            if let Some(last) = images.last_mut()
                && Arc::ptr_eq(&last.request.path, &marker.path)
            {
                last.label.end = label.end;
                continue;
            }
            let request = Request {
                reply: Arc::downgrade(reply),
                path: Arc::clone(&marker.path),
                columns: available_width(line, layout.width),
                protocol,
                cell_size,
            };
            images.push(InlineImage {
                label,
                image: cache.get(&request).cloned().flatten(),
                request,
                region: None,
            });
        }
    }
    layout.has_markdown_images = !images.is_empty();
    place(layout, images);
}

pub(in crate::transcript_view) fn visible_requests(
    layout: &TextLayout,
    rows: Range<usize>,
) -> impl Iterator<Item = &Request> {
    let visible = layout.rows.get(rows).unwrap_or_default();
    let start = visible.first().map_or(usize::MAX, |r| r.source.start);
    let end = visible
        .last()
        .map_or(/*default*/ 0, |r| r.line_end.unwrap_or(r.source.end));
    let first = layout
        .inline_images
        .partition_point(|i| i.label.end < start);
    layout.inline_images[first..]
        .iter()
        .take_while(move |image| image.label.start <= end)
        .map(|image| &image.request)
}

pub(in crate::transcript_view) fn regions(
    layout: &TextLayout,
) -> impl Iterator<Item = (usize, u16, &Arc<TranscriptImage>)> {
    let trailing = layout.visible_image().map(|image| {
        let indent = if layout.image_left_aligned {
            2
        } else {
            (layout.width - image.columns) / 2
        };
        (layout.rows.len(), indent, image)
    });
    layout
        .inline_images
        .iter()
        .filter_map(|inline| {
            let (distance, indent) = inline.region?;
            let row = layout.rows.len().checked_sub(distance)?;
            Some((row, indent, inline.image.as_ref()?))
        })
        .chain(trailing)
}

/// Dragging past a preview's first row includes its label, even for a one-row image.
pub(super) fn label_end_at(layout: &TextLayout, row: usize, column: u16) -> Option<usize> {
    layout.inline_images.iter().find_map(|inline| {
        let (distance, indent) = inline.region?;
        let image = inline.image.as_ref()?;
        (layout.rows.len().checked_sub(distance) == Some(row)
            && usize::from(column) >= usize::from(indent) + usize::from(image.columns))
        .then_some(inline.label.end)
    })
}

pub(super) fn place(layout: &mut TextLayout, mut images: Vec<InlineImage>) {
    if !images.iter().any(|i| i.image.is_some()) {
        layout.inline_images = images;
        return;
    }
    let lines = std::mem::take(&mut layout.logical);
    let mut original = std::mem::take(&mut layout.rows).into_iter().peekable();
    let mut start = 0;
    let mut resume = None;
    let mut pending = images.iter_mut().peekable();
    for line in &lines {
        let end = start + line.origin.range.len();
        let first = layout.rows.len();
        while let Some(inline) = pending.next_if(|i| i.label.start <= end) {
            inline.region = None;
            let Some(image) = &inline.image else { continue };
            if !image.fits(
                available_width(line, layout.width),
                inline.request.cell_size,
            ) || layout.text.get(inline.label.clone()).is_none()
            {
                continue;
            }
            append_fragment(layout, line, start, resume, inline.label.start);
            let mut prefix = if layout.rows.len() == first
                && resume.is_none()
                && line.initial_indent.width() > 0
            {
                &line.initial_indent
            } else {
                &line.subsequent_indent
            };
            let indent = prefix.width();
            inline.region = Some((layout.rows.len(), indent as u16));
            let mut offset = inline.label.start;
            for _ in 0..image.rows {
                layout.insert_presentation_row(layout.rows.len(), offset, indent, prefix.clone());
                prefix = &line.subsequent_indent;
                offset = inline.label.end;
            }
            resume = Some(inline.label.end);
        }
        while let Some(row) = original.next_if(|r| r.source.start <= end) {
            if resume.is_none() {
                layout.rows.push(row);
            }
        }
        if resume.is_some_and(|offset| offset <= end) {
            append_fragment(layout, line, start, resume.take(), end);
            if let Some(row) = layout.rows.last_mut() {
                row.line_end = Some(end);
            }
        }
        start = end + 1;
    }
    layout.logical = lines;
    for image in &mut images {
        if let Some((row, _)) = &mut image.region {
            *row = layout.rows.len() - *row;
        }
    }
    layout.inline_images = images;
}

fn append_fragment(
    layout: &mut TextLayout,
    line: &LogicalLine,
    start: usize,
    resume: Option<usize>,
    end: usize,
) {
    let offset = resume.unwrap_or(start);
    let Some(text) = layout.text.get(offset..end) else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    let mut fragment = line.clone();
    let mut left = offset;
    if resume.is_some() {
        fragment.initial_indent = line.subsequent_indent.clone();
        left += text.len() - text.trim_start().len();
    }
    let right = offset + text.trim_end().len();
    fragment.line.line = super::logical::slice_line(&line.line.line, left - start..right - start);
    if !fragment.line.hyperlinks.is_empty() {
        let column = |x| source_text_width(&layout.text[start..x]);
        fragment.line.hyperlinks = project_hyperlink_columns(
            &line.line.hyperlinks,
            column(left)..column(right),
            /*prefix*/ 0,
        );
    }
    fragment.origin.range.start += left - start;
    layout
        .rows
        .extend(super::layout_line(&fragment, left, layout.width));
}

fn available_width(line: &LogicalLine, width: u16) -> u16 {
    let indent = line
        .initial_indent
        .width()
        .max(line.subsequent_indent.width());
    usize::from(width).saturating_sub(indent) as u16
}
