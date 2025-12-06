use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::{Line, ToLine};
use ratatui_core::widgets::{StatefulWidget, Widget};

use crate::block::BlockExt;
use crate::list::{List, ListDirection, ListState};

impl Widget for List<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &List<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

impl StatefulWidget for List<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buf, state);
    }
}

impl StatefulWidget for &List<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        self.block.as_ref().render(area, buf);
        let list_area = self.block.inner_if_some(area);

        if list_area.is_empty() {
            return;
        }

        if self.items.is_empty() {
            state.select(None);
            return;
        }

        // If the selected index is out of bounds, set it to the last item
        if state.selected.is_some_and(|s| s >= self.items.len()) {
            state.select(Some(self.items.len().saturating_sub(1)));
        }

        let list_height = list_area.height as usize;

        let (first_visible_index, last_visible_index) =
            self.get_items_bounds(state.selected, state.offset, list_height);

        // Important: this changes the state's offset to be the beginning of the now viewable items
        state.offset = first_visible_index;

        // Get our set highlighted symbol (if one was set)
        let default_highlight_symbol = Line::default();
        let highlight_symbol = self
            .highlight_symbol
            .as_ref()
            .unwrap_or(&default_highlight_symbol);
        let highlight_symbol_width = highlight_symbol.width() as u16;
        let empty_symbol = " ".repeat(highlight_symbol_width as usize);
        let empty_symbol = empty_symbol.to_line();

        let mut current_height = 0;
        let selection_spacing = self.highlight_spacing.should_add(state.selected.is_some());
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(last_visible_index - first_visible_index)
        {
            let (x, y) = if self.direction == ListDirection::BottomToTop {
                current_height += item.height() as u16;
                (list_area.left(), list_area.bottom() - current_height)
            } else {
                let pos = (list_area.left(), list_area.top() + current_height);
                current_height += item.height() as u16;
                pos
            };

            let row_area = Rect::new(x, y, list_area.width, item.height() as u16);

            let item_style = self.style.patch(item.style);
            buf.set_style(row_area, item_style);

            let is_selected = state.selected == Some(i);

            let item_area = if selection_spacing {
                Rect {
                    x: row_area.x + highlight_symbol_width,
                    width: row_area.width.saturating_sub(highlight_symbol_width),
                    ..row_area
                }
            } else {
                row_area
            };
            Widget::render(&item.content, item_area, buf);

            if is_selected {
                buf.set_style(row_area, self.highlight_style);
            }
            if selection_spacing {
                for j in 0..item.content.height() {
                    // if the item is selected, we need to display the highlight symbol:
                    // - either for the first line of the item only,
                    // - or for each line of the item if the appropriate option is set
                    let line = if is_selected && (j == 0 || self.repeat_highlight_symbol) {
                        highlight_symbol
                    } else {
                        &empty_symbol
                    };
                    let highlight_area = Rect::new(x, y + j as u16, highlight_symbol_width, 1);
                    line.render(highlight_area, buf);
                }
            }
        }
    }
}

impl List<'_> {
    /// Given an offset, calculate which items can fit in a given area
    fn get_items_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: usize,
    ) -> (usize, usize) {
        let offset = offset.min(self.items.len().saturating_sub(1));

        // Note: visible here implies visible in the given area
        let mut first_visible_index = offset;
        let mut last_visible_index = offset;

        // Current height of all items in the list to render, beginning at the offset
        let mut height_from_offset = 0;

        // Calculate the last visible index and total height of the items
        // that will fit in the available space
        for item in self.items.iter().skip(offset) {
            if height_from_offset + item.height() > max_height {
                break;
            }

            height_from_offset += item.height();

            last_visible_index += 1;
        }

        // Get the selected index and apply scroll_padding to it, but still honor the offset if
        // nothing is selected. This allows for the list to stay at a position after select()ing
        // None.
        let index_to_display = self
            .apply_scroll_padding_to_selected_index(
                selected,
                max_height,
                first_visible_index,
                last_visible_index,
            )
            .unwrap_or(offset);

        // Recall that last_visible_index is the index of what we
        // can render up to in the given space after the offset
        // If we have an item selected that is out of the viewable area (or
        // the offset is still set), we still need to show this item
        while index_to_display >= last_visible_index {
            height_from_offset =
                height_from_offset.saturating_add(self.items[last_visible_index].height());

            last_visible_index += 1;

            // Now we need to hide previous items since we didn't have space
            // for the selected/offset item
            while height_from_offset > max_height {
                height_from_offset =
                    height_from_offset.saturating_sub(self.items[first_visible_index].height());

                // Remove this item to view by starting at the next item index
                first_visible_index += 1;
            }
        }

        // Here we're doing something similar to what we just did above
        // If the selected item index is not in the viewable area, let's try to show the item
        while index_to_display < first_visible_index {
            first_visible_index -= 1;

            height_from_offset =
                height_from_offset.saturating_add(self.items[first_visible_index].height());

            // Don't show an item if it is beyond our viewable height
            while height_from_offset > max_height {
                last_visible_index -= 1;

                height_from_offset =
                    height_from_offset.saturating_sub(self.items[last_visible_index].height());
            }
        }

        (first_visible_index, last_visible_index)
    }

    /// Applies scroll padding to the selected index, reducing the padding value to keep the
    /// selected item on screen even with items of inconsistent sizes
    ///
    /// This function is sensitive to how the bounds checking function handles item height
    fn apply_scroll_padding_to_selected_index(
        &self,
        selected: Option<usize>,
        max_height: usize,
        first_visible_index: usize,
        last_visible_index: usize,
    ) -> Option<usize> {
        let last_valid_index = self.items.len().saturating_sub(1);
        let selected = selected?.min(last_valid_index);

        // The bellow loop handles situations where the list item sizes may not be consistent,
        // where the offset would have excluded some items that we want to include, or could
        // cause the offset value to be set to an inconsistent value each time we render.
        // The padding value will be reduced in case any of these issues would occur
        let mut scroll_padding = self.scroll_padding;
        while scroll_padding > 0 {
            let mut height_around_selected = 0;
            for index in selected.saturating_sub(scroll_padding)
                ..=selected
                    .saturating_add(scroll_padding)
                    .min(last_valid_index)
            {
                height_around_selected += self.items[index].height();
            }
            if height_around_selected <= max_height {
                break;
            }
            scroll_padding -= 1;
        }

        Some(
            if (selected + scroll_padding).min(last_valid_index) >= last_visible_index {
                selected + scroll_padding
            } else if selected.saturating_sub(scroll_padding) < first_visible_index {
                selected.saturating_sub(scroll_padding)
            } else {
                selected
            }
            .min(last_valid_index),
        )
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::ToOwned;
    use alloc::vec;
    use alloc::vec::Vec;

    use pretty_assertions::assert_eq;
    use ratatui_core::layout::{Alignment, Rect};
    use ratatui_core::style::{Color, Modifier, Style, Stylize};
    use ratatui_core::text::Line;
    use ratatui_core::widgets::{StatefulWidget, Widget};
    use rstest::{fixture, rstest};

    use super::*;
    use crate::block::Block;
    use crate::list::ListItem;
    use crate::table::HighlightSpacing;

    #[fixture]
    fn single_line_buf() -> Buffer {
        Buffer::empty(Rect::new(0, 0, 10, 1))
    }

    #[rstest]
    fn empty_list(mut single_line_buf: Buffer) {
        let mut state = ListState::default();

        let items: Vec<ListItem> = Vec::new();
        let list = List::new(items);
        state.select_first();
        StatefulWidget::render(list, single_line_buf.area, &mut single_line_buf, &mut state);
        assert_eq!(state.selected, None);
    }

    #[rstest]
    fn single_item(mut single_line_buf: Buffer) {
        let mut state = ListState::default();

        let items = vec![ListItem::new("Item 1")];
        let list = List::new(items);
        state.select_first();
        StatefulWidget::render(
            &list,
            single_line_buf.area,
            &mut single_line_buf,
            &mut state,
        );
        assert_eq!(state.selected, Some(0));

        state.select_last();
        StatefulWidget::render(
            &list,
            single_line_buf.area,
            &mut single_line_buf,
            &mut state,
        );
        assert_eq!(state.selected, Some(0));

        state.select_previous();
        StatefulWidget::render(
            &list,
            single_line_buf.area,
            &mut single_line_buf,
            &mut state,
        );
        assert_eq!(state.selected, Some(0));

        state.select_next();
        StatefulWidget::render(
            &list,
            single_line_buf.area,
            &mut single_line_buf,
            &mut state,
        );
        assert_eq!(state.selected, Some(0));
    }

    /// helper method to render a widget to an empty buffer with the default state
    fn widget(widget: List<'_>, width: u16, height: u16) -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        Widget::render(widget, buffer.area, &mut buffer);
        buffer
    }

    /// helper method to render a widget to an empty buffer with a given state
    fn stateful_widget(widget: List<'_>, state: &mut ListState, width: u16, height: u16) -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
        StatefulWidget::render(widget, buffer.area, &mut buffer, state);
        buffer
    }

    #[test]
    fn does_not_render_in_small_space() {
        let items = vec!["Item 0", "Item 1", "Item 2"];
        let list = List::new(items.clone()).highlight_symbol(">>");
        let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));

        // attempt to render into an area of the buffer with 0 width
        Widget::render(list.clone(), Rect::new(0, 0, 0, 3), &mut buffer);
        assert_eq!(&buffer, &Buffer::empty(buffer.area));

        // attempt to render into an area of the buffer with 0 height
        Widget::render(list.clone(), Rect::new(0, 0, 15, 0), &mut buffer);
        assert_eq!(&buffer, &Buffer::empty(buffer.area));

        let list = List::new(items)
            .highlight_symbol(">>")
            .block(Block::bordered());
        // attempt to render into an area of the buffer with zero height after
        // setting the block borders
        Widget::render(list, Rect::new(0, 0, 15, 2), &mut buffer);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "┌─────────────┐",
            "└─────────────┘",
            "               ",
        ]);
        assert_eq!(buffer, expected,);
    }

    #[expect(clippy::too_many_lines)]
    #[test]
    fn combinations() {
        #[track_caller]
        fn test_case_render<'line, Lines>(items: &[ListItem], expected: Lines)
        where
            Lines: IntoIterator,
            Lines::Item: Into<Line<'line>>,
        {
            let list = List::new(items.to_owned()).highlight_symbol(">>");
            let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
            Widget::render(list, buffer.area, &mut buffer);
            assert_eq!(buffer, Buffer::with_lines(expected));
        }

        #[track_caller]
        fn test_case_render_stateful<'line, Lines>(
            items: &[ListItem],
            selected: Option<usize>,
            expected: Lines,
        ) where
            Lines: IntoIterator,
            Lines::Item: Into<Line<'line>>,
        {
            let list = List::new(items.to_owned()).highlight_symbol(">>");
            let mut state = ListState::default().with_selected(selected);
            let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
            StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
            assert_eq!(buffer, Buffer::with_lines(expected));
        }

        let empty_items = Vec::new();
        let single_item = vec!["Item 0".into()];
        let multiple_items = vec!["Item 0".into(), "Item 1".into(), "Item 2".into()];
        let multi_line_items = vec!["Item 0\nLine 2".into(), "Item 1".into(), "Item 2".into()];

        // empty list
        test_case_render(
            &empty_items,
            [
                "          ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &empty_items,
            None,
            [
                "          ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &empty_items,
            Some(0),
            [
                "          ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );

        // single item
        test_case_render(
            &single_item,
            [
                "Item 0    ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &single_item,
            None,
            [
                "Item 0    ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &single_item,
            Some(0),
            [
                ">>Item 0  ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &single_item,
            Some(1),
            [
                ">>Item 0  ",
                "          ",
                "          ",
                "          ",
                "          ",
            ],
        );

        // multiple items
        test_case_render(
            &multiple_items,
            [
                "Item 0    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multiple_items,
            None,
            [
                "Item 0    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multiple_items,
            Some(0),
            [
                ">>Item 0  ",
                "  Item 1  ",
                "  Item 2  ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multiple_items,
            Some(1),
            [
                "  Item 0  ",
                ">>Item 1  ",
                "  Item 2  ",
                "          ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multiple_items,
            Some(3),
            [
                "  Item 0  ",
                "  Item 1  ",
                ">>Item 2  ",
                "          ",
                "          ",
            ],
        );

        // multi line items
        test_case_render(
            &multi_line_items,
            [
                "Item 0    ",
                "Line 2    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multi_line_items,
            None,
            [
                "Item 0    ",
                "Line 2    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multi_line_items,
            Some(0),
            [
                ">>Item 0  ",
                "  Line 2  ",
                "  Item 1  ",
                "  Item 2  ",
                "          ",
            ],
        );
        test_case_render_stateful(
            &multi_line_items,
            Some(1),
            [
                "  Item 0  ",
                "  Line 2  ",
                ">>Item 1  ",
                "  Item 2  ",
                "          ",
            ],
        );
    }

    #[test]
    fn items() {
        let list = List::default().items(["Item 0", "Item 1", "Item 2"]);
        let buffer = widget(list, 10, 5);
        let expected = Buffer::with_lines([
            "Item 0    ",
            "Item 1    ",
            "Item 2    ",
            "          ",
            "          ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn empty_strings() {
        let list = List::new(["Item 0", "", "", "Item 1", "Item 2"])
            .block(Block::bordered().title("List"));
        let buffer = widget(list, 10, 7);
        let expected = Buffer::with_lines([
            "┌List────┐",
            "│Item 0  │",
            "│        │",
            "│        │",
            "│Item 1  │",
            "│Item 2  │",
            "└────────┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn block() {
        let list = List::new(["Item 0", "Item 1", "Item 2"]).block(Block::bordered().title("List"));
        let buffer = widget(list, 10, 7);
        let expected = Buffer::with_lines([
            "┌List────┐",
            "│Item 0  │",
            "│Item 1  │",
            "│Item 2  │",
            "│        │",
            "│        │",
            "└────────┘",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn style() {
        let list = List::new(["Item 0", "Item 1", "Item 2"]).style(Style::default().fg(Color::Red));
        let buffer = widget(list, 10, 5);
        let expected = Buffer::with_lines([
            "Item 0    ".red(),
            "Item 1    ".red(),
            "Item 2    ".red(),
            "          ".red(),
            "          ".red(),
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn highlight_symbol_and_style() {
        let list = List::new(["Item 0", "Item 1", "Item 2"])
            .highlight_symbol(">>")
            .highlight_style(Style::default().fg(Color::Yellow));
        let mut state = ListState::default();
        state.select(Some(1));
        let buffer = stateful_widget(list, &mut state, 10, 5);
        let expected = Buffer::with_lines([
            "  Item 0  ".into(),
            ">>Item 1  ".yellow(),
            "  Item 2  ".into(),
            "          ".into(),
            "          ".into(),
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn highlight_symbol_style_and_style() {
        let list = List::new(["Item 0", "Item 1", "Item 2"])
            .highlight_symbol(Line::from(">>").red().bold())
            .highlight_style(Style::default().fg(Color::Yellow));
        let mut state = ListState::default();
        state.select(Some(1));
        let buffer = stateful_widget(list, &mut state, 10, 5);
        let mut expected = Buffer::with_lines([
            "  Item 0  ".into(),
            ">>Item 1  ".yellow(),
            "  Item 2  ".into(),
            "          ".into(),
            "          ".into(),
        ]);
        expected.set_style(Rect::new(0, 1, 2, 1), Style::new().red().bold());
        assert_eq!(buffer, expected);
    }

    #[test]
    fn highlight_spacing_default_when_selected() {
        // when not selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"]).highlight_symbol(">>");
            let mut state = ListState::default();
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "Item 0    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }

        // when selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"]).highlight_symbol(">>");
            let mut state = ListState::default();
            state.select(Some(1));
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "  Item 0  ",
                ">>Item 1  ",
                "  Item 2  ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }
    }

    #[test]
    fn highlight_spacing_default_always() {
        // when not selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"])
                .highlight_symbol(">>")
                .highlight_spacing(HighlightSpacing::Always);
            let mut state = ListState::default();
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "  Item 0  ",
                "  Item 1  ",
                "  Item 2  ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }

        // when selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"])
                .highlight_symbol(">>")
                .highlight_spacing(HighlightSpacing::Always);
            let mut state = ListState::default();
            state.select(Some(1));
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "  Item 0  ",
                ">>Item 1  ",
                "  Item 2  ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }
    }

    #[test]
    fn highlight_spacing_default_never() {
        // when not selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"])
                .highlight_symbol(">>")
                .highlight_spacing(HighlightSpacing::Never);
            let mut state = ListState::default();
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "Item 0    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }

        // when selected
        {
            let list = List::new(["Item 0", "Item 1", "Item 2"])
                .highlight_symbol(">>")
                .highlight_spacing(HighlightSpacing::Never);
            let mut state = ListState::default();
            state.select(Some(1));
            let buffer = stateful_widget(list, &mut state, 10, 5);
            let expected = Buffer::with_lines([
                "Item 0    ",
                "Item 1    ",
                "Item 2    ",
                "          ",
                "          ",
            ]);
            assert_eq!(buffer, expected);
        }
    }

    #[test]
    fn repeat_highlight_symbol() {
        let list = List::new(["Item 0\nLine 2", "Item 1", "Item 2"])
            .highlight_symbol(Line::from(">>").red().bold())
            .highlight_style(Style::default().fg(Color::Yellow))
            .repeat_highlight_symbol(true);
        let mut state = ListState::default();
        state.select(Some(0));
        let buffer = stateful_widget(list, &mut state, 10, 5);
        let mut expected = Buffer::with_lines([
            ">>Item 0  ".yellow(),
            ">>Line 2  ".yellow(),
            "  Item 1  ".into(),
            "  Item 2  ".into(),
            "          ".into(),
        ]);
        expected.set_style(Rect::new(0, 0, 2, 2), Style::new().red().bold());
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case::top_to_bottom(ListDirection::TopToBottom, [
        "Item 0    ",
        "Item 1    ",
        "Item 2    ",
        "          ",
    ])]
    #[case::top_to_bottom(ListDirection::BottomToTop, [
        "          ",
        "Item 2    ",
        "Item 1    ",
        "Item 0    ",
    ])]
    fn list_direction<'line, Lines>(#[case] direction: ListDirection, #[case] expected: Lines)
    where
        Lines: IntoIterator,
        Lines::Item: Into<Line<'line>>,
    {
        let list = List::new(["Item 0", "Item 1", "Item 2"]).direction(direction);
        let buffer = widget(list, 10, 4);
        assert_eq!(buffer, Buffer::with_lines(expected));
    }

    #[test]
    fn truncate_items() {
        let list = List::new(["Item 0", "Item 1", "Item 2", "Item 3", "Item 4"]);
        let buffer = widget(list, 10, 3);
        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "Item 0    ",
            "Item 1    ",
            "Item 2    ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn offset_renders_shifted() {
        let list = List::new([
            "Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6",
        ]);
        let mut state = ListState::default().with_offset(3);
        let buffer = stateful_widget(list, &mut state, 6, 3);

        let expected = Buffer::with_lines(["Item 3", "Item 4", "Item 5"]);
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case(None, [
        "Item 0 with a v",
        "Item 1         ",
        "Item 2         ",
    ])]
    #[case(Some(0), [
        ">>Item 0 with a",
        "  Item 1       ",
        "  Item 2       ",
    ])]
    fn long_lines<'line, Lines>(#[case] selected: Option<usize>, #[case] expected: Lines)
    where
        Lines: IntoIterator,
        Lines::Item: Into<Line<'line>>,
    {
        let items = [
            "Item 0 with a very long line that will be truncated",
            "Item 1",
            "Item 2",
        ];
        let list = List::new(items).highlight_symbol(">>");
        let mut state = ListState::default().with_selected(selected);
        let buffer = stateful_widget(list, &mut state, 15, 3);
        assert_eq!(buffer, Buffer::with_lines(expected));
    }

    #[test]
    fn selected_item_ensures_selected_item_is_visible_when_offset_is_before_visible_range() {
        let items = [
            "Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6",
        ];
        let list = List::new(items).highlight_symbol(">>");
        // Set the initial visible range to items 3, 4, and 5
        let mut state = ListState::default().with_selected(Some(1)).with_offset(3);
        let buffer = stateful_widget(list, &mut state, 10, 3);

        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            ">>Item 1  ",
            "  Item 2  ",
            "  Item 3  ",
        ]);

        assert_eq!(buffer, expected);
        assert_eq!(state.selected, Some(1));
        assert_eq!(
            state.offset, 1,
            "did not scroll the selected item into view"
        );
    }

    #[test]
    fn selected_item_ensures_selected_item_is_visible_when_offset_is_after_visible_range() {
        let items = [
            "Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6",
        ];
        let list = List::new(items).highlight_symbol(">>");
        // Set the initial visible range to items 3, 4, and 5
        let mut state = ListState::default().with_selected(Some(6)).with_offset(3);
        let buffer = stateful_widget(list, &mut state, 10, 3);

        #[rustfmt::skip]
        let expected = Buffer::with_lines([
            "  Item 4  ",
            "  Item 5  ",
            ">>Item 6  ",
        ]);

        assert_eq!(buffer, expected);
        assert_eq!(state.selected, Some(6));
        assert_eq!(
            state.offset, 4,
            "did not scroll the selected item into view"
        );
    }

    #[test]
    fn can_be_stylized() {
        assert_eq!(
            List::new::<Vec<&str>>(vec![])
                .black()
                .on_white()
                .bold()
                .not_dim()
                .style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn with_alignment() {
        let list = List::new([
            Line::from("Left").alignment(Alignment::Left),
            Line::from("Center").alignment(Alignment::Center),
            Line::from("Right").alignment(Alignment::Right),
        ]);
        let buffer = widget(list, 10, 4);
        let expected = Buffer::with_lines(["Left      ", "  Center  ", "     Right", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_odd_line_odd_area() {
        let list = List::new([
            Line::from("Odd").alignment(Alignment::Left),
            Line::from("Even").alignment(Alignment::Center),
            Line::from("Width").alignment(Alignment::Right),
        ]);
        let buffer = widget(list, 7, 4);
        let expected = Buffer::with_lines(["Odd    ", " Even  ", "  Width", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_even_line_even_area() {
        let list = List::new([
            Line::from("Odd").alignment(Alignment::Left),
            Line::from("Even").alignment(Alignment::Center),
            Line::from("Width").alignment(Alignment::Right),
        ]);
        let buffer = widget(list, 6, 4);
        let expected = Buffer::with_lines(["Odd   ", " Even ", " Width", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_odd_line_even_area() {
        let list = List::new([
            Line::from("Odd").alignment(Alignment::Left),
            Line::from("Even").alignment(Alignment::Center),
            Line::from("Width").alignment(Alignment::Right),
        ]);
        let buffer = widget(list, 8, 4);
        let expected = Buffer::with_lines(["Odd     ", "  Even  ", "   Width", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_even_line_odd_area() {
        let list = List::new([
            Line::from("Odd").alignment(Alignment::Left),
            Line::from("Even").alignment(Alignment::Center),
            Line::from("Width").alignment(Alignment::Right),
        ]);
        let buffer = widget(list, 6, 4);
        let expected = Buffer::with_lines(["Odd   ", " Even ", " Width", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_zero_line_width() {
        let list = List::new([Line::from("This line has zero width").alignment(Alignment::Center)]);
        let buffer = widget(list, 0, 2);
        assert_eq!(buffer, Buffer::with_lines([""; 2]));
    }

    #[test]
    fn alignment_zero_area_width() {
        let list = List::new([Line::from("Text").alignment(Alignment::Left)]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));
        Widget::render(list, Rect::new(0, 0, 4, 0), &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["    "]));
    }

    #[test]
    fn alignment_line_less_than_width() {
        let list = List::new([Line::from("Small").alignment(Alignment::Center)]);
        let buffer = widget(list, 10, 2);
        let expected = Buffer::with_lines(["  Small   ", ""]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn alignment_line_equal_to_width() {
        let list = List::new([Line::from("Exact").alignment(Alignment::Left)]);
        let buffer = widget(list, 5, 2);
        assert_eq!(buffer, Buffer::with_lines(["Exact", ""]));
    }

    #[test]
    fn alignment_line_greater_than_width() {
        let list = List::new([Line::from("Large line").alignment(Alignment::Left)]);
        let buffer = widget(list, 5, 2);
        assert_eq!(buffer, Buffer::with_lines(["Large", ""]));
    }

    #[rstest]
    #[case::no_padding(
        4,
        2, // Offset
        0, // Padding
        Some(2), // Selected
        [
            ">> Item 2 ",
            "   Item 3 ",
            "   Item 4 ",
            "   Item 5 ",
        ]
    )]
    #[case::one_before(
        4,
        2, // Offset
        1, // Padding
        Some(2), // Selected
        [
            "   Item 1 ",
            ">> Item 2 ",
            "   Item 3 ",
            "   Item 4 ",
        ]
    )]
    #[case::one_after(
        4,
        1, // Offset
        1, // Padding
        Some(4), // Selected
        [
            "   Item 2 ",
            "   Item 3 ",
            ">> Item 4 ",
            "   Item 5 ",
        ]
    )]
    #[case::check_padding_overflow(
        4,
        1, // Offset
        2, // Padding
        Some(4), // Selected
        [
            "   Item 2 ",
            "   Item 3 ",
            ">> Item 4 ",
            "   Item 5 ",
        ]
    )]
    #[case::no_padding_offset_behavior(
        5, // Render Area Height
        2, // Offset
        0, // Padding
        Some(3), // Selected
        [
            "   Item 2 ",
            ">> Item 3 ",
            "   Item 4 ",
            "   Item 5 ",
            "          ",
        ]
    )]
    #[case::two_before(
        5, // Render Area Height
        2, // Offset
        2, // Padding
        Some(3), // Selected
        [
            "   Item 1 ",
            "   Item 2 ",
            ">> Item 3 ",
            "   Item 4 ",
            "   Item 5 ",
        ]
    )]
    #[case::keep_selected_visible(
        4,
        0, // Offset
        4, // Padding
        Some(1), // Selected
        [
            "   Item 0 ",
            ">> Item 1 ",
            "   Item 2 ",
            "   Item 3 ",
        ]
    )]
    fn with_padding<'line, Lines>(
        #[case] render_height: u16,
        #[case] offset: usize,
        #[case] padding: usize,
        #[case] selected: Option<usize>,
        #[case] expected: Lines,
    ) where
        Lines: IntoIterator,
        Lines::Item: Into<Line<'line>>,
    {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, render_height));
        let mut state = ListState::default();

        *state.offset_mut() = offset;
        state.select(selected);

        let list = List::new(["Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5"])
            .scroll_padding(padding)
            .highlight_symbol(">> ");
        StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines(expected));
    }

    /// If there isn't enough room for the selected item and the requested padding the list can jump
    /// up and down every frame if something isn't done about it. This code tests to make sure that
    /// isn't currently happening
    #[test]
    fn padding_flicker() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let mut state = ListState::default();

        *state.offset_mut() = 2;
        state.select(Some(4));

        let items = [
            "Item 0", "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6", "Item 7",
        ];
        let list = List::new(items).scroll_padding(3).highlight_symbol(">> ");

        StatefulWidget::render(&list, buffer.area, &mut buffer, &mut state);

        let offset_after_render = state.offset();

        StatefulWidget::render(&list, buffer.area, &mut buffer, &mut state);

        // Offset after rendering twice should remain the same as after once
        assert_eq!(offset_after_render, state.offset());
    }

    #[test]
    fn padding_inconsistent_item_sizes() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        let mut state = ListState::default().with_offset(0).with_selected(Some(3));

        let items = [
            ListItem::new("Item 0"),
            ListItem::new("Item 1"),
            ListItem::new("Item 2"),
            ListItem::new("Item 3"),
            ListItem::new("Item 4\nTest\nTest"),
            ListItem::new("Item 5"),
        ];
        let list = List::new(items).scroll_padding(1).highlight_symbol(">> ");

        StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);

        #[rustfmt::skip]
        let expected = [
            "   Item 1 ",
            "   Item 2 ",
            ">> Item 3 ",
        ];
        assert_eq!(buffer, Buffer::with_lines(expected));
    }

    // Tests to make sure when it's pushing back the first visible index value that it doesnt
    // include an item that's too large
    #[test]
    fn padding_offset_pushback_break() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 4));
        let mut state = ListState::default();

        *state.offset_mut() = 1;
        state.select(Some(2));

        let items = [
            ListItem::new("Item 0\nTest\nTest"),
            ListItem::new("Item 1"),
            ListItem::new("Item 2"),
            ListItem::new("Item 3"),
        ];
        let list = List::new(items).scroll_padding(2).highlight_symbol(">> ");

        StatefulWidget::render(list, buffer.area, &mut buffer, &mut state);
        #[rustfmt::skip]
        assert_eq!(
            buffer,
            Buffer::with_lines([
                "   Item 1 ",
                ">> Item 2 ",
                "   Item 3 ",
                "          "])
        );
    }

    /// Regression test for a bug where highlight symbol being greater than width caused a panic due
    /// to subtraction with underflow.
    ///
    /// See [#949](https://github.com/ratatui/ratatui/pull/949) for details
    #[rstest]
    #[case::under(">>>>", "Item1", ">>>>Item1 ")] // enough space to render the highlight symbol
    #[case::exact(">>>>>", "Item1", ">>>>>Item1")] // exact space to render the highlight symbol
    #[case::overflow(">>>>>>", "Item1", ">>>>>>Item")] // not enough space
    fn highlight_symbol_overflow(
        #[case] highlight_symbol: &str,
        #[case] item: &str,
        #[case] expected: &str,
        mut single_line_buf: Buffer,
    ) {
        let list = List::new([item]).highlight_symbol(highlight_symbol);
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, single_line_buf.area, &mut single_line_buf, &mut state);
        assert_eq!(single_line_buf, Buffer::with_lines([expected]));
    }
}
