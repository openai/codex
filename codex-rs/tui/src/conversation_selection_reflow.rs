//! Source-backed endpoint bookmarks for selection-preserving conversation reflow.

use std::ops::Range;

use crate::conversation_selection::CellSelectionProjection;
use crate::conversation_selection::ConversationSelection;
use crate::conversation_selection::SelectionCellLayout;
use crate::conversation_selection::SelectionPoint;

#[derive(Clone, Copy, Eq, PartialEq)]
enum SelectionEndpointEdge {
    Start,
    End,
}

pub(crate) struct SelectionBookmarks {
    anchor: SelectionBookmark,
    focus: SelectionBookmark,
    focus_screen_row: usize,
}

pub(crate) struct RestoredSelection {
    pub(crate) anchor: SelectionPoint,
    pub(crate) focus: SelectionPoint,
    pub(crate) scroll_offset: usize,
}

enum SelectionBookmark {
    Projected {
        cell: usize,
        source: Range<usize>,
        edge: SelectionEndpointEdge,
        fallback_row: usize,
        fallback_column: u16,
    },
    CellRow {
        cell: usize,
        row: usize,
        column: u16,
    },
    BeforeCell {
        cell: usize,
        column: u16,
    },
    AfterContent {
        column: u16,
    },
}

impl SelectionBookmarks {
    pub(crate) fn capture(
        selection: &ConversationSelection,
        projections: &[Option<CellSelectionProjection>],
        layout: &[SelectionCellLayout],
        scroll_offset: usize,
    ) -> Option<Self> {
        let (anchor, focus) = selection.endpoints()?;
        let (anchor_edge, focus_edge) = if anchor <= focus {
            (SelectionEndpointEdge::Start, SelectionEndpointEdge::End)
        } else {
            (SelectionEndpointEdge::End, SelectionEndpointEdge::Start)
        };
        Some(Self {
            anchor: SelectionBookmark::capture(anchor, anchor_edge, projections, layout),
            focus: SelectionBookmark::capture(focus, focus_edge, projections, layout),
            focus_screen_row: focus.row.saturating_sub(scroll_offset),
        })
    }

    pub(crate) fn projected_cells(&self) -> impl Iterator<Item = usize> + '_ {
        [self.anchor.projected_cell(), self.focus.projected_cell()]
            .into_iter()
            .flatten()
    }

    pub(crate) fn restore(
        self,
        projections: &[Option<CellSelectionProjection>],
        layout: &[SelectionCellLayout],
        width: u16,
    ) -> Option<RestoredSelection> {
        let anchor = self.anchor.restore(projections, layout, width)?;
        let focus = self.focus.restore(projections, layout, width)?;
        Some(RestoredSelection {
            anchor,
            focus,
            scroll_offset: focus.row.saturating_sub(self.focus_screen_row),
        })
    }
}

impl SelectionBookmark {
    fn capture(
        point: SelectionPoint,
        edge: SelectionEndpointEdge,
        projections: &[Option<CellSelectionProjection>],
        layout: &[SelectionCellLayout],
    ) -> Self {
        let containing_cell = layout.iter().copied().enumerate().find(|(_, layout)| {
            layout.height > 0
                && point.row >= layout.top
                && point.row < layout.top.saturating_add(layout.height)
        });
        if let Some((cell, cell_layout)) = containing_cell {
            let row = point.row.saturating_sub(cell_layout.top);
            return if let Some(projection) = projections.get(cell).and_then(Option::as_ref) {
                Self::Projected {
                    cell,
                    source: projection.point_range(row, point.column),
                    edge,
                    fallback_row: row,
                    fallback_column: point.column,
                }
            } else {
                Self::CellRow {
                    cell,
                    row,
                    column: point.column,
                }
            };
        }

        if let Some(cell) = layout.iter().position(|layout| layout.top > point.row) {
            Self::BeforeCell {
                cell,
                column: point.column,
            }
        } else {
            Self::AfterContent {
                column: point.column,
            }
        }
    }

    fn projected_cell(&self) -> Option<usize> {
        match self {
            Self::Projected { cell, .. } => Some(*cell),
            Self::CellRow { .. } | Self::BeforeCell { .. } | Self::AfterContent { .. } => None,
        }
    }

    fn restore(
        self,
        projections: &[Option<CellSelectionProjection>],
        layout: &[SelectionCellLayout],
        width: u16,
    ) -> Option<SelectionPoint> {
        let max_column = width.saturating_sub(1);
        match self {
            Self::Projected {
                cell,
                source,
                edge,
                fallback_row,
                fallback_column,
            } => {
                let cell_layout = *layout.get(cell)?;
                let (row, column) = projections
                    .get(cell)
                    .and_then(Option::as_ref)
                    .and_then(|projection| {
                        point_for_source_range(projection, &source, edge, width, fallback_row)
                    })
                    .unwrap_or((
                        fallback_row.min(cell_layout.height.saturating_sub(1)),
                        fallback_column.min(max_column),
                    ));
                Some(SelectionPoint {
                    row: cell_layout.top.saturating_add(row),
                    column,
                })
            }
            Self::CellRow { cell, row, column } => {
                let cell_layout = *layout.get(cell)?;
                Some(SelectionPoint {
                    row: cell_layout
                        .top
                        .saturating_add(row.min(cell_layout.height.saturating_sub(1))),
                    column: column.min(max_column),
                })
            }
            Self::BeforeCell { cell, column } => Some(SelectionPoint {
                row: layout.get(cell)?.top.saturating_sub(1),
                column: column.min(max_column),
            }),
            Self::AfterContent { column } => Some(SelectionPoint {
                row: layout
                    .last()
                    .map(|layout| layout.top.saturating_add(layout.height))
                    .unwrap_or_default(),
                column: column.min(max_column),
            }),
        }
    }
}

fn point_for_source_range(
    projection: &CellSelectionProjection,
    source: &Range<usize>,
    edge: SelectionEndpointEdge,
    width: u16,
    preferred_row: usize,
) -> Option<(usize, u16)> {
    if !source.is_empty() {
        return projection
            .rows()
            .iter()
            .enumerate()
            .find_map(|(row, selection_row)| {
                selection_row.segments.iter().find_map(|segment| {
                    (segment.bytes.start <= source.start && segment.bytes.end >= source.end)
                        .then_some((row, segment.columns.start))
                })
            });
    }

    let boundary = source.start;
    let mut rows = (0..projection.rows().len()).collect::<Vec<_>>();
    rows.sort_by_key(|row| row.abs_diff(preferred_row));
    for row in rows {
        let mut columns = vec![0, width.saturating_sub(1)];
        for pair in projection.rows()[row].segments.windows(2) {
            let [left, right] = pair else {
                continue;
            };
            columns.push(left.columns.end);
            columns.push(right.columns.start.saturating_sub(1));
        }
        columns.sort_unstable();
        columns.dedup();
        if edge == SelectionEndpointEdge::End {
            columns.reverse();
        }
        if let Some(column) = columns.into_iter().find(|column| {
            let range = projection.point_range(row, *column);
            match edge {
                SelectionEndpointEdge::Start => range.start == boundary,
                SelectionEndpointEdge::End => range.end == boundary,
            }
        }) {
            return Some((row, column));
        }
    }
    None
}
