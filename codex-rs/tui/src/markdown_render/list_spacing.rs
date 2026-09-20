//! List spacing is a layout policy, not a streaming boundary. Completed redrawable lists use
//! uniform sibling separators; streaming lists stay compact until source-backed consolidation.

use crate::terminal_hyperlinks::HyperlinkLine;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ListSpacing {
    /// Preserve the historical spacing for terminal-owned scrollback.
    #[default]
    AfterMultiline,
    /// Do not add separators between siblings while an owned transcript streams.
    Compact,
    /// Separate every sibling if any item occupies multiple rendered rows.
    Uniform,
}

#[derive(Default)]
pub(super) struct UniformList {
    pub(super) has_item: bool,
    pub(super) multiline: bool,
    pub(super) separators: Vec<usize>,
}

impl UniformList {
    pub(super) fn finish(self, lines: &mut Vec<HyperlinkLine>) {
        if self.multiline {
            return;
        }
        let mut separators = self.separators.into_iter().peekable();
        let Some(&start) = separators.peek() else {
            return;
        };
        // Only remove this list's provisional separators, preserving source and link metadata.
        let tail = lines.split_off(start);
        lines.extend(tail.into_iter().enumerate().filter_map(|(offset, line)| {
            if separators.peek() == Some(&(start + offset)) {
                separators.next();
                None
            } else {
                Some(line)
            }
        }));
    }
}
