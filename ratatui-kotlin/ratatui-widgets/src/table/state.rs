/// State of a [`Table`] widget
///
/// This state can be used to scroll through the rows and select one of them. When the table is
/// rendered as a stateful widget, the selected row, column and cell will be highlighted and the
/// table will be shifted to ensure that the selected row is visible. This will modify the
/// [`TableState`] object passed to the `Frame::render_stateful_widget` method.
///
/// The state consists of two fields:
/// - [`offset`]: the index of the first row to be displayed
/// - [`selected`]: the index of the selected row, which can be `None` if no row is selected
/// - [`selected_column`]: the index of the selected column, which can be `None` if no column is
///   selected
///
/// [`offset`]: TableState::offset()
/// [`selected`]: TableState::selected()
/// [`selected_column`]: TableState::selected_column()
///
/// See the `table` example and the `recipe` and `traceroute` tabs in the demo2 example in the
/// [Examples] directory for a more in depth example of the various configuration options and for
/// how to handle state.
///
/// [Examples]: https://github.com/ratatui/ratatui/blob/master/examples/README.md
///
/// # Example
///
/// ```rust
/// use ratatui::Frame;
/// use ratatui::layout::{Constraint, Rect};
/// use ratatui::widgets::{Row, Table, TableState};
///
/// # fn ui(frame: &mut Frame) {
/// # let area = Rect::default();
/// let rows = [Row::new(vec!["Cell1", "Cell2"])];
/// let widths = [Constraint::Length(5), Constraint::Length(5)];
/// let table = Table::new(rows, widths).widths(widths);
///
/// // Note: TableState should be stored in your application state (not constructed in your render
/// // method) so that the selected row is preserved across renders
/// let mut table_state = TableState::default();
/// *table_state.offset_mut() = 1; // display the second row and onwards
/// table_state.select(Some(3)); // select the forth row (0-indexed)
/// table_state.select_column(Some(2)); // select the third column (0-indexed)
///
/// frame.render_stateful_widget(table, area, &mut table_state);
/// # }
/// ```
///
/// Note that if [`Table::widths`] is not called before rendering, the rendered columns will have
/// equal width.
///
/// [`Table`]: super::Table
/// [`Table::widths`]: crate::table::Table::widths
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableState {
    pub(crate) offset: usize,
    pub(crate) selected: Option<usize>,
    pub(crate) selected_column: Option<usize>,
}

impl TableState {
    /// Creates a new [`TableState`]
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let state = TableState::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            offset: 0,
            selected: None,
            selected_column: None,
        }
    }

    /// Sets the index of the first row to be displayed
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let state = TableState::new().with_offset(1);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the index of the selected row
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let state = TableState::new().with_selected(Some(1));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_selected<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<usize>>,
    {
        self.selected = selected.into();
        self
    }

    /// Sets the index of the selected column
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let state = TableState::new().with_selected_column(Some(1));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_selected_column<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<usize>>,
    {
        self.selected_column = selected.into();
        self
    }

    /// Sets the indexes of the selected cell
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let state = TableState::new().with_selected_cell(Some((1, 5)));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_selected_cell<T>(mut self, selected: T) -> Self
    where
        T: Into<Option<(usize, usize)>>,
    {
        if let Some((r, c)) = selected.into() {
            self.selected = Some(r);
            self.selected_column = Some(c);
        } else {
            self.selected = None;
            self.selected_column = None;
        }

        self
    }

    /// Index of the first row to be displayed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let state = TableState::new();
    /// assert_eq!(state.offset(), 0);
    /// ```
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Mutable reference to the index of the first row to be displayed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// *state.offset_mut() = 1;
    /// ```
    pub const fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Index of the selected row
    ///
    /// Returns `None` if no row is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let state = TableState::new();
    /// assert_eq!(state.selected(), None);
    /// ```
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Index of the selected column
    ///
    /// Returns `None` if no column is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let state = TableState::new();
    /// assert_eq!(state.selected_column(), None);
    /// ```
    pub const fn selected_column(&self) -> Option<usize> {
        self.selected_column
    }

    /// Indexes of the selected cell
    ///
    /// Returns `None` if no cell is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let state = TableState::new();
    /// assert_eq!(state.selected_cell(), None);
    /// ```
    pub const fn selected_cell(&self) -> Option<(usize, usize)> {
        if let (Some(r), Some(c)) = (self.selected, self.selected_column) {
            return Some((r, c));
        }
        None
    }

    /// Mutable reference to the index of the selected row
    ///
    /// Returns `None` if no row is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// *state.selected_mut() = Some(1);
    /// ```
    pub const fn selected_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected
    }

    /// Mutable reference to the index of the selected column
    ///
    /// Returns `None` if no column is selected
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// *state.selected_column_mut() = Some(1);
    /// ```
    pub const fn selected_column_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected_column
    }

    /// Sets the index of the selected row
    ///
    /// Set to `None` if no row is selected. This will also reset the offset to `0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.select(Some(1));
    /// ```
    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    /// Sets the index of the selected column
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_column(Some(1));
    /// ```
    pub const fn select_column(&mut self, index: Option<usize>) {
        self.selected_column = index;
    }

    /// Sets the indexes of the selected cell
    ///
    /// Set to `None` if no cell is selected. This will also reset the row offset to `0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_cell(Some((1, 5)));
    /// ```
    pub const fn select_cell(&mut self, indexes: Option<(usize, usize)>) {
        if let Some((r, c)) = indexes {
            self.selected = Some(r);
            self.selected_column = Some(c);
        } else {
            self.offset = 0;
            self.selected = None;
            self.selected_column = None;
        }
    }

    /// Selects the next row or the first one if no row is selected
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `0` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.select_next();
    /// ```
    pub fn select_next(&mut self) {
        let next = self.selected.map_or(0, |i| i.saturating_add(1));
        self.select(Some(next));
    }

    /// Selects the next column or the first one if no column is selected
    ///
    /// Note: until the table is rendered, the number of columns is not known, so the index is set
    /// to `0` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_next_column();
    /// ```
    pub fn select_next_column(&mut self) {
        let next = self.selected_column.map_or(0, |i| i.saturating_add(1));
        self.select_column(Some(next));
    }

    /// Selects the previous row or the last one if no item is selected
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.select_previous();
    /// ```
    pub fn select_previous(&mut self) {
        let previous = self.selected.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select(Some(previous));
    }

    /// Selects the previous column or the last one if no column is selected
    ///
    /// Note: until the table is rendered, the number of columns is not known, so the index is set
    /// to `usize::MAX` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_previous_column();
    /// ```
    pub fn select_previous_column(&mut self) {
        let previous = self
            .selected_column
            .map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select_column(Some(previous));
    }

    /// Selects the first row
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `0` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.select_first();
    /// ```
    pub const fn select_first(&mut self) {
        self.select(Some(0));
    }

    /// Selects the first column
    ///
    /// Note: until the table is rendered, the number of columns is not known, so the index is set
    /// to `0` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_first_column();
    /// ```
    pub const fn select_first_column(&mut self) {
        self.select_column(Some(0));
    }

    /// Selects the last row
    ///
    /// Note: until the table is rendered, the number of rows is not known, so the index is set to
    /// `usize::MAX` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.select_last();
    /// ```
    pub const fn select_last(&mut self) {
        self.select(Some(usize::MAX));
    }

    /// Selects the last column
    ///
    /// Note: until the table is rendered, the number of columns is not known, so the index is set
    /// to `usize::MAX` and will be corrected when the table is rendered
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.select_last();
    /// ```
    pub const fn select_last_column(&mut self) {
        self.select_column(Some(usize::MAX));
    }

    /// Scrolls down by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it down by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., if the index is greater than
    /// the number of rows in the table), the last row in the table will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.scroll_down_by(4);
    /// ```
    pub fn scroll_down_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_add(amount as usize)));
    }

    /// Scrolls up by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it up by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., less than zero),
    /// the first row in the table will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::TableState;
    ///
    /// let mut state = TableState::default();
    /// state.scroll_up_by(4);
    /// ```
    pub fn scroll_up_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_sub(amount as usize)));
    }

    /// Scrolls right by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it right by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., if the index is greater than
    /// the number of columns in the table), the last column in the table will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.scroll_right_by(4);
    /// ```
    pub fn scroll_right_by(&mut self, amount: u16) {
        let selected = self.selected_column.unwrap_or_default();
        self.select_column(Some(selected.saturating_add(amount as usize)));
    }

    /// Scrolls left by a specified `amount` in the table.
    ///
    /// This method updates the selected index by moving it left by the given `amount`.
    /// If the `amount` causes the index to go out of bounds (i.e., less than zero),
    /// the first item in the table will be selected.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use ratatui::widgets::{TableState};
    /// let mut state = TableState::default();
    /// state.scroll_left_by(4);
    /// ```
    pub fn scroll_left_by(&mut self, amount: u16) {
        let selected = self.selected_column.unwrap_or_default();
        self.select_column(Some(selected.saturating_sub(amount as usize)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let state = TableState::new();
        assert_eq!(state.offset, 0);
        assert_eq!(state.selected, None);
        assert_eq!(state.selected_column, None);
    }

    #[test]
    fn with_offset() {
        let state = TableState::new().with_offset(1);
        assert_eq!(state.offset, 1);
    }

    #[test]
    fn with_selected() {
        let state = TableState::new().with_selected(Some(1));
        assert_eq!(state.selected, Some(1));
    }

    #[test]
    fn with_selected_column() {
        let state = TableState::new().with_selected_column(Some(1));
        assert_eq!(state.selected_column, Some(1));
    }

    #[test]
    fn with_selected_cell_none() {
        let state = TableState::new().with_selected_cell(None);
        assert_eq!(state.selected, None);
        assert_eq!(state.selected_column, None);
    }

    #[test]
    fn offset() {
        let state = TableState::new();
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn offset_mut() {
        let mut state = TableState::new();
        *state.offset_mut() = 1;
        assert_eq!(state.offset, 1);
    }

    #[test]
    fn selected() {
        let state = TableState::new();
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn selected_column() {
        let state = TableState::new();
        assert_eq!(state.selected_column(), None);
    }

    #[test]
    fn selected_cell() {
        let state = TableState::new();
        assert_eq!(state.selected_cell(), None);
    }

    #[test]
    fn selected_mut() {
        let mut state = TableState::new();
        *state.selected_mut() = Some(1);
        assert_eq!(state.selected, Some(1));
    }

    #[test]
    fn selected_column_mut() {
        let mut state = TableState::new();
        *state.selected_column_mut() = Some(1);
        assert_eq!(state.selected_column, Some(1));
    }

    #[test]
    fn select() {
        let mut state = TableState::new();
        state.select(Some(1));
        assert_eq!(state.selected, Some(1));
    }

    #[test]
    fn select_none() {
        let mut state = TableState::new().with_selected(Some(1));
        state.select(None);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn select_column() {
        let mut state = TableState::new();
        state.select_column(Some(1));
        assert_eq!(state.selected_column, Some(1));
    }

    #[test]
    fn select_column_none() {
        let mut state = TableState::new().with_selected_column(Some(1));
        state.select_column(None);
        assert_eq!(state.selected_column, None);
    }

    #[test]
    fn select_cell() {
        let mut state = TableState::new();
        state.select_cell(Some((1, 5)));
        assert_eq!(state.selected_cell(), Some((1, 5)));
    }

    #[test]
    fn select_cell_none() {
        let mut state = TableState::new().with_selected_cell(Some((1, 5)));
        state.select_cell(None);
        assert_eq!(state.selected, None);
        assert_eq!(state.selected_column, None);
        assert_eq!(state.selected_cell(), None);
    }

    #[test]
    fn test_table_state_navigation() {
        let mut state = TableState::default();
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

        let mut state = TableState::default();
        state.select_next();
        assert_eq!(state.selected, Some(0));

        let mut state = TableState::default();
        state.select_previous();
        assert_eq!(state.selected, Some(usize::MAX));

        let mut state = TableState::default();
        state.select(Some(2));
        state.scroll_down_by(4);
        assert_eq!(state.selected, Some(6));

        let mut state = TableState::default();
        state.scroll_up_by(3);
        assert_eq!(state.selected, Some(0));

        state.select(Some(6));
        state.scroll_up_by(4);
        assert_eq!(state.selected, Some(2));

        state.scroll_up_by(4);
        assert_eq!(state.selected, Some(0));

        let mut state = TableState::default();
        state.select_first_column();
        assert_eq!(state.selected_column, Some(0));

        state.select_previous_column();
        assert_eq!(state.selected_column, Some(0));

        state.select_next_column();
        assert_eq!(state.selected_column, Some(1));

        state.select_previous_column();
        assert_eq!(state.selected_column, Some(0));

        state.select_last_column();
        assert_eq!(state.selected_column, Some(usize::MAX));

        state.select_previous_column();
        assert_eq!(state.selected_column, Some(usize::MAX - 1));

        let mut state = TableState::default().with_selected_column(Some(12));
        state.scroll_right_by(4);
        assert_eq!(state.selected_column, Some(16));

        state.scroll_left_by(20);
        assert_eq!(state.selected_column, Some(0));

        state.scroll_right_by(100);
        assert_eq!(state.selected_column, Some(100));

        state.scroll_left_by(20);
        assert_eq!(state.selected_column, Some(80));
    }
}
