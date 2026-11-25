//! Size and place flex items in terminal cells. Each wrapped band distributes its
//! own free space. Cross-axis stretch uses intrinsic heights before viewport clipping.

use super::AvailableWidth;
use super::Fragment;
use super::Height;
use super::NodeKind;
use super::PreparedNode;
use super::Visit;
use super::layout_node;
use super::prepare::RowPaint;
use crate::ui_tree::RowJustify;
use crate::ui_tree::RowProps;
use crate::ui_tree::RowWrap;

pub(super) fn column<'a>(
    children: &'a [PreparedNode],
    gap: u64,
    spacers: usize,
    width: AvailableWidth,
    limit: usize,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let mut growth = Shares::new(width.height.extra(), spacers);
    let measuring = matches!(width.height, Height::Measure);
    let width = AvailableWidth {
        height: width.height.with_extra(/*extra*/ 0),
        ..width
    };
    let gap = gap.min(limit as u64) as usize;
    let mut y: usize = 0;
    for (index, child) in children.iter().enumerate() {
        let mut blank = if index > 0 { gap } else { 0 };
        if matches!(child.kind, NodeKind::Spacer) {
            blank = blank.saturating_add(growth.take());
        }
        let end = y.saturating_add(blank).min(limit);
        if !measuring {
            for row in y..end {
                visit(row, Fragment::BlankLine);
            }
        }
        y = end;
        if y == limit {
            break;
        }
        y += layout_node(child, width, limit - y, &mut |row, part| {
            visit(y + row, part)
        });
    }
    y
}

pub(super) fn row<'a>(
    children: &'a [PreparedNode],
    props: &RowProps,
    paint: &'a RowPaint,
    width: AvailableWidth,
    limit: usize,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let gap = props.gap.min((width.cells + 1) as u64) as usize;
    let mut rest = children;
    let bands = std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut used = 0;
        let mut count = 0;
        for child in rest {
            let preferred = child.widths.preferred.min((width.cells + 1) as u64) as usize;
            let next = used + if count > 0 { gap } else { 0 } + preferred;
            if props.wrap == RowWrap::Wrap && count > 0 && next > width.cells {
                break;
            }
            used = next;
            count += 1;
        }
        let (band, tail) = rest.split_at(count);
        rest = tail;
        Some((band, width.cells.saturating_sub(used)))
    });
    let extra = width.height.extra();
    let mut growth = Shares::new(extra, if extra > 0 { bands.clone().count() } else { 0 });
    let mut y = 0;
    for (band, spare) in bands {
        let width = AvailableWidth {
            height: width.height.with_extra(growth.take()),
            ..width
        };
        y += layout_band(
            band,
            width,
            limit - y,
            props,
            spare,
            paint,
            &mut |row, part| visit(y + row, part),
        );
        if y == limit {
            break;
        }
    }
    y
}

// Round cumulative positions so the last share always lands at the far edge.
#[derive(Clone, Copy)]
struct Shares {
    space: usize,
    count: usize,
    used: usize,
}
impl Shares {
    fn new(space: usize, count: usize) -> Self {
        Self {
            space,
            count,
            used: 0,
        }
    }

    fn take(&mut self) -> usize {
        if self.space == 0 || self.count == 0 {
            return 0;
        }
        let position = |i: usize| {
            ((i as u128 * self.space as u128 + (self.count / 2) as u128) / self.count as u128)
                as usize
        };
        self.used += 1;
        position(self.used) - position(self.used - 1)
    }
}

fn layout_band<'a>(
    children: &'a [PreparedNode],
    width: AvailableWidth,
    limit: usize,
    props: &RowProps,
    spare: usize,
    paint: &'a RowPaint,
    visit: &mut Visit<'a, '_>,
) -> usize {
    let bounded = |cells: u64| cells.min((width.cells + 1) as u64) as usize;
    let gap = bounded(props.gap);
    // Flex growth consumes free space before justification, separately on each band.
    let spacers = children
        .iter()
        .filter(|c| matches!(c.kind, NodeKind::Spacer))
        .count();
    let growth = if spacers > 0 { spare } else { 0 };
    let spare = spare - growth;
    let (leading, between) = match props.justify.unwrap_or_default() {
        RowJustify::Start => (0, 0),
        RowJustify::Center => (spare / 2, 0),
        RowJustify::End => (spare, 0),
        RowJustify::Between => (0, spare),
    };
    let gaps = children.len().saturating_sub(1);
    let between = if gaps == 0 { 0 } else { between };
    let mut gap_shares = Shares::new(between, gaps);
    let mut spacer_shares = Shares::new(growth, spacers);
    let mut reserved = children
        .iter()
        .map(|c| bounded(c.widths.minimum))
        .sum::<usize>()
        + gap * gaps
        + between;
    let mut offset = leading;
    let aligned =
        spacers == 0 && matches!(props.justify, Some(RowJustify::Center | RowJustify::End));
    let background_width = if width.fill_background && props.background.is_some() {
        width.cells.min(width.visible)
    } else {
        0
    };
    let suppress_blank = aligned && !width.in_aligned_row && background_width == 0;
    let placements = children.iter().map(move |child| {
        let minimum = bounded(child.widths.minimum);
        reserved -= minimum;
        let available = if matches!(child.kind, NodeKind::Spacer) {
            spacer_shares.take()
        } else {
            bounded(child.widths.preferred)
                .min(width.cells.saturating_sub(offset + reserved))
                .max(minimum)
        };
        let child_width = AvailableWidth {
            cells: available.min(width.cells),
            visible: width.visible.saturating_sub(offset),
            in_aligned_row: width.in_aligned_row || aligned,
            fill_background: true,
            height: Height::Grow(0),
        };
        let placement = (offset, available, child, child_width);
        let next_gap = gap + gap_shares.take();
        offset += available + next_gap;
        reserved = reserved.saturating_sub(next_gap);
        placement
    });
    let intrinsic = |cap| {
        placements.clone().map(move |(_, _, child, mut w)| {
            w.height = Height::Measure;
            layout_node(child, w, cap, &mut |_, _| {})
        })
    };
    if matches!(width.height, Height::Measure) {
        return intrinsic(limit).max().unwrap_or_default();
    }
    // A column with spacers can stretch to the tallest sibling in its band.
    // Measure before viewport clipping so offscreen lines still affect positioning.
    let extra = width.height.extra();
    let stretch = (children.len() > 1 || extra > 0) && children.iter().any(|c| c.stretches);
    let heights: Vec<_> = if stretch {
        intrinsic(usize::MAX).collect()
    } else {
        Vec::new()
    };
    let band_height = if stretch {
        heights.iter().copied().max().unwrap_or_default()
    } else if spacers > 0 || extra > 0 {
        intrinsic(limit).max().unwrap_or_default()
    } else {
        0
    };
    let band_height = band_height.saturating_add(extra);
    let mut lines = Vec::new();
    for (i, (offset, available, child, mut child_width)) in placements.enumerate() {
        let mut place = |y: usize, part: Fragment<'a>| {
            if y >= lines.len() {
                lines.resize_with(y + 1, || (0, suppress_blank.then(Vec::new)));
            }
            if matches!(part, Fragment::BlankLine) {
                visit(y, part);
                return;
            }
            // Unpainted alignment and gaps alone do not make a blank line visible.
            let (end, pending) = &mut lines[y];
            if part.width() > 0
                && !matches!(part, Fragment::Padding(..) | Fragment::Gap(..))
                && let Some(gaps) = pending.take()
            {
                for gap in gaps {
                    visit(y, gap);
                }
            }
            let mut emit = |fragment: Fragment<'a>| {
                if let Some(gaps) = pending
                    && fragment.width() > 0
                {
                    gaps.push(fragment);
                } else {
                    visit(y, fragment);
                }
            };
            // A gap before an empty fragment can paint to the edge, but
            // only content beyond that edge should trigger overflow.
            let start = if part.width() == 0 {
                offset.min(width.visible).min(width.cells)
            } else {
                offset.min(width.cells + 1)
            };
            if *end == 0 && leading > 0 && start > 0 {
                let padding = leading.min(start);
                emit(if padding == background_width {
                    Fragment::Background(padding, &paint.leading)
                } else {
                    Fragment::Padding(padding, &paint.leading)
                });
                *end = padding;
            }
            if start > *end {
                emit(if background_width > 0 {
                    Fragment::Background(start - *end, &paint.gap)
                } else {
                    Fragment::Gap(start - *end, &paint.gap)
                });
                *end = start;
            }
            // Forward blank lines so ancestors count them toward the height limit.
            emit(part);
            *end += part.width();
        };
        if matches!(child.kind, NodeKind::Spacer) {
            for y in 0..band_height.min(limit) {
                place(y, Fragment::Padding(available, &paint.leading));
            }
        } else {
            if stretch && child.stretches {
                child_width.height = Height::Grow(band_height - heights[i]);
            }
            layout_node(child, child_width, limit, &mut place);
        }
    }
    // Fill the rest of each visible line of an explicitly colored flex item,
    // including blank continuations. Never take cells from the next sibling.
    if background_width > 0 {
        for (y, (end, _)) in lines.iter().enumerate() {
            if *end < background_width {
                visit(
                    y,
                    Fragment::Background(background_width - end, &paint.leading),
                );
            }
        }
    }
    lines.len().max(band_height.min(limit))
}
