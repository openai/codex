//! Selection contracts and projection builders shared by history cells.

use crate::conversation_selection::CellSelectionProjection;
use ratatui::text::Line;

/// Describes whether a history cell contributes semantic text to mouse selection.
///
/// Selectable cells map their rendered rows back to canonical copy text. Transparent cells are
/// presentation-only and may be crossed without contributing text to the clipboard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SelectionContribution {
    Selectable(CellSelectionProjection),
    Transparent,
}

/// Source-backed semantic projection for one emitted or mutable streaming fragment.
///
/// The controller slices this from its full source projection using rendered-line boundaries.
/// Cells can reuse the exact mapping at the original body width or realign the same canonical text
/// after a resize without ever deriving clipboard text from joined terminal rows.
#[derive(Clone, Debug)]
pub(crate) struct StreamSelectionFragment {
    projection: CellSelectionProjection,
    body_width: u16,
}

impl StreamSelectionFragment {
    pub(crate) fn new(projection: CellSelectionProjection, body_width: u16) -> Self {
        Self {
            projection,
            body_width,
        }
    }

    pub(crate) fn projection_for_display(
        &self,
        body_width: u16,
        display_lines: Vec<Line<'static>>,
        display_width: u16,
        outer_prefix_columns: u16,
    ) -> Option<CellSelectionProjection> {
        Some(if self.body_width == body_width {
            self.projection
                .clone()
                .with_column_offset(outer_prefix_columns)
        } else {
            CellSelectionProjection::from_display_lines(
                self.projection.text().to_string(),
                display_lines,
                display_width,
                outer_prefix_columns,
            )?
        })
    }

    pub(crate) fn text(&self) -> &str {
        self.projection.text()
    }
}

impl SelectionContribution {
    pub(crate) fn into_projection(self) -> Option<CellSelectionProjection> {
        match self {
            Self::Selectable(projection) => Some(projection),
            Self::Transparent => None,
        }
    }
}

pub(crate) fn selection_text_from_lines(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds a selectable contribution by aligning rendered cells with canonical semantic text.
pub(crate) fn selection_contribution_from_semantic_text(
    text: String,
    lines: Vec<Line<'static>>,
    width: u16,
    first_row_prefix_columns: u16,
) -> SelectionContribution {
    match CellSelectionProjection::from_display_lines(text, lines, width, first_row_prefix_columns)
    {
        Some(projection) => SelectionContribution::Selectable(projection),
        None => SelectionContribution::Transparent,
    }
}

/// Builds a selectable contribution with presentation-prefix widths specified per logical line.
pub(crate) fn selection_contribution_from_semantic_rows(
    text: String,
    lines: Vec<Line<'static>>,
    width: u16,
    first_row_prefix_columns: &[u16],
) -> SelectionContribution {
    match CellSelectionProjection::from_display_lines_with_prefixes(
        text,
        lines,
        width,
        first_row_prefix_columns,
    ) {
        Some(projection) => SelectionContribution::Selectable(projection),
        None => SelectionContribution::Transparent,
    }
}

/// Builds a contribution whose rendered text is also its canonical clipboard text.
pub(crate) fn selection_contribution_from_display_lines(
    lines: Vec<Line<'static>>,
    width: u16,
) -> SelectionContribution {
    let text = selection_text_from_lines(&lines);
    selection_contribution_from_semantic_text(
        text, lines, width, /*first_row_prefix_columns*/ 0,
    )
}
