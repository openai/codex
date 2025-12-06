/// State of the [`List`] widget
///
/// This state can be used to scroll through items and select one. When the list is rendered as a
/// stateful widget, the selected item will be highlighted and the list will be shifted to ensure
/// that the selected item is visible. This will modify the [`ListState`] object passed to the
/// `Frame::render_stateful_widget` method.
///
/// The state consists of two fields:
/// - [`offset`]: the index of the first item to be displayed
/// - [`selected`]: the index of the selected item, which can be `None` if no item is selected
///
/// [`offset`]: ListState::offset()
/// [`selected`]: ListState::selected()
///
/// See the list in the [Examples] directory for a more in depth example of the various
/// configuration options and for how to handle state.
///
/// [Examples]: https://github.com/ratatui/ratatui/blob/main/examples/README.md
///
/// # Example
///
/// ```rust
/// use ratatui::Frame;
/// use ratatui::layout::Rect;
/// use ratatui::widgets::{List, ListState};
///
/// # fn ui(frame: &mut Frame) {
/// # let area = Rect::default();
/// let items = ["Item 1"];
/// let list = List::new(items);
///
/// // This should be stored outside of the function in your application state.
/// let mut state = ListState::default();
///
/// *state.offset_mut() = 1; // display the second item and onwards
/// state.select(Some(3)); // select the forth item (0-indexed)
///
/// frame.render_stateful_widget(list, area, &mut state);
/// # }
/// ```
///
/// [`List`]: super::List
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListState {
    pub(crate) offset: usize,
    pub(crate) selected: Option<usize>,
}

impl ListState {
    /// Sets the index of the first item to be displayed
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let state = ListState::default().with_offset(1);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the index of the selected item
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let state = ListState::default().with_selected(Some(1));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Index of the first item to be displayed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let state = ListState::default();
    /// assert_eq!(state.offset(), 0);
    /// ```
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Mutable reference to the index of the first item to be displayed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// *state.offset_mut() = 1;
    /// ```
    pub const fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Index of the selected item
    ///
    /// Returns `None` if no item is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let state = ListState::default();
    /// assert_eq!(state.selected(), None);
    /// ```
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Mutable reference to the index of the selected item
    ///
    /// Returns `None` if no item is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// *state.selected_mut() = Some(1);
    /// ```
    pub const fn selected_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected
    }

    /// Sets the index of the selected item
    ///
    /// Set to `None` if no item is selected. This will also reset the offset to `0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.select(Some(1));
    /// ```
    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    /// Selects the next item or the first one if no item is selected
    ///
    /// Note: until the list is rendered, the number of items is not known, so the index is set to
    /// `0` and will be corrected when the list is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.select_next();
    /// ```
    pub fn select_next(&mut self) {
        let next = self.selected.map_or(0, |i| i.saturating_add(1));
        self.select(Some(next));
    }

    /// Selects the previous item or the last one if no item is selected
    ///
    /// Note: until the list is rendered, the number of items is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the list is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.select_previous();
    /// ```
    pub fn select_previous(&mut self) {
        let previous = self.selected.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select(Some(previous));
    }

    /// Selects the first item
    ///
    /// Note: until the list is rendered, the number of items is not known, so the index is set to
    /// `0` and will be corrected when the list is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.select_first();
    /// ```
    pub const fn select_first(&mut self) {
        self.select(Some(0));
    }

    /// Selects the last item
    ///
    /// Note: until the list is rendered, the number of items is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the list is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.select_last();
    /// ```
    pub const fn select_last(&mut self) {
        self.select(Some(usize::MAX));
    }

    /// Scrolls down by a specified `amount` in the list.
    ///
    /// This method updates the selected index by moving it down by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., if the index is greater than
    /// the length of the list), the last item in the list will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.scroll_down_by(4);
    /// ```
    pub fn scroll_down_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_add(amount as usize)));
    }

    /// Scrolls up by a specified `amount` in the list.
    ///
    /// This method updates the selected index by moving it up by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., less than zero),
    /// the first item in the list will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::ListState;
    ///
    /// let mut state = ListState::default();
    /// state.scroll_up_by(4);
    /// ```
    pub fn scroll_up_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_sub(amount as usize)));
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::list::ListState;

    #[test]
    fn selected() {
        let mut state = ListState::default();
        assert_eq!(state.selected(), None);

        state.select(Some(1));
        assert_eq!(state.selected(), Some(1));

        state.select(None);
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn select() {
        let mut state = ListState::default();
        assert_eq!(state.selected, None);
        assert_eq!(state.offset, 0);

        state.select(Some(2));
        assert_eq!(state.selected, Some(2));
        assert_eq!(state.offset, 0);

        state.select(None);
        assert_eq!(state.selected, None);
        assert_eq!(state.offset, 0);
    }

    #[test]
    fn state_navigation() {
        let mut state = ListState::default();
        state.select_first();
        assert_eq!(state.selected, Some(0));

        state.select_previous(); // should not go below 0
        assert_eq!(state.selected, Some(0));

        state.select_next();
        assert_eq!(state.selected, Some(1));

        state.select_previous();
        assert_eq!(state.selected, Some(0));

        state.select_last();
        assert_eq!(state.selected, Some(usize::MAX));

        state.select_next(); // should not go above usize::MAX
        assert_eq!(state.selected, Some(usize::MAX));

        state.select_previous();
        assert_eq!(state.selected, Some(usize::MAX - 1));

        state.select_next();
        assert_eq!(state.selected, Some(usize::MAX));

        let mut state = ListState::default();
        state.select_next();
        assert_eq!(state.selected, Some(0));

        let mut state = ListState::default();
        state.select_previous();
        assert_eq!(state.selected, Some(usize::MAX));

        let mut state = ListState::default();
        state.select(Some(2));
        state.scroll_down_by(4);
        assert_eq!(state.selected, Some(6));

        let mut state = ListState::default();
        state.scroll_up_by(3);
        assert_eq!(state.selected, Some(0));

        state.select(Some(6));
        state.scroll_up_by(4);
        assert_eq!(state.selected, Some(2));

        state.scroll_up_by(4);
        assert_eq!(state.selected, Some(0));
    }
}
