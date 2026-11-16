//! Seeks a history cursor and supplies the pager with each segment and its newer overrides.

use super::super::rollout_lineage::LineageWalk;
use super::super::rollout_lineage::RolloutLineageSegment;
use super::read::HistoryCursor;
use super::read::invalid_cursor;
use crate::SortDirection;
use crate::ThreadStoreResult;

pub(super) struct PageSegments<'a, 'store> {
    lineage: &'a mut LineageWalk<'store>,
    step: isize,
    next_depth: Option<usize>,
    cursor: Option<HistoryCursor>,
}

pub(super) struct PageSegment<'a> {
    pub segment: &'a RolloutLineageSegment,
    pub newer: &'a [RolloutLineageSegment],
    pub cursor: Option<HistoryCursor>,
}

impl<'a, 'store> PageSegments<'a, 'store> {
    pub(super) async fn from_cursor(
        lineage: &'a mut LineageWalk<'store>,
        direction: SortDirection,
        cursor: Option<HistoryCursor>,
    ) -> ThreadStoreResult<Self> {
        // Ascending reads must discover the oldest segment up front. Descending reads start
        // at the leaf and seek only as far as the cursor, retaining all newer shadowing turns.
        let (start, step) = match direction {
            SortDirection::Asc => (lineage.resolve_all().await?.len().checked_sub(1), -1),
            SortDirection::Desc => (Some(0), 1),
        };
        let mut next_depth = start;
        if let Some(cursor) = &cursor {
            loop {
                let missing = || invalid_cursor("position outside thread lineage");
                let depth = next_depth.ok_or_else(missing)?;
                let (segment, _) = lineage.at_depth(depth).await?.ok_or_else(missing)?;
                if segment.contains(cursor.rollout_ordinal) {
                    break;
                }
                next_depth = depth.checked_add_signed(step);
            }
        }
        Ok(Self {
            lineage,
            step,
            next_depth,
            cursor,
        })
    }

    pub(super) async fn next(&mut self) -> ThreadStoreResult<Option<PageSegment<'_>>> {
        let Some(depth) = self.next_depth else {
            return Ok(None);
        };
        let segment = self.lineage.at_depth(depth).await?;
        self.next_depth = depth.checked_add_signed(self.step);
        Ok(segment.map(|(segment, newer)| PageSegment {
            segment,
            newer,
            cursor: self.cursor.take(),
        }))
    }
}
