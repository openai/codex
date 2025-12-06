//! State like [`ListState`], [`TableState`] and [`ScrollbarState`] can be serialized and
//! deserialized through serde. This allows saving your entire state to disk when the user exits the
//! the app, and restore it again upon re-opening the app.
//! This way, they get right back to where they were, without having to re-seek to their previous
//! position, if that's applicable for the app at hand.
//!
//! **Note**: For this pattern to work easily, you need to have some toplevel struct which stores
//! _only_ state and not any draw commands.
//!
//! **Note**: For many applications, it might be beneficial to instead keep your own state and
//! instead construct the state for widgets on the fly instead, if that allows you to express you
//! the semantic meaning of your state better or only fetch part of a dataset.

// not too happy about the redundancy in these tests,
// but if that helps readability then it's ok i guess /shrug

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, List, ListState, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
    TableState,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AppState {
    list: ListState,
    table: TableState,
    scrollbar: ScrollbarState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            list: ListState::default(),
            table: TableState::default(),
            scrollbar: ScrollbarState::new(10),
        }
    }
}
impl AppState {
    const fn select(&mut self, index: usize) {
        self.list.select(Some(index));
        self.table.select_cell(Some((index, index)));
        self.scrollbar = self.scrollbar.position(index);
    }
}

/// Renders the list to a `TestBackend` and asserts that the result matches the expected buffer.
#[track_caller]
fn assert_buffer<'line, Lines>(state: &mut AppState, expected: Lines)
where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(21, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let items = [
                "awa", "banana", "Cats!!", "d20", "Echo", "Foxtrot", "Golf", "Hotel", "IwI",
                "Juliett",
            ];

            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(1),
                ])
                .split(f.area());
            let list = List::new(items)
                .highlight_symbol(">>")
                .block(Block::new().borders(Borders::RIGHT));
            f.render_stateful_widget(list, layout[0], &mut state.list);

            let table = Table::new(
                items.into_iter().map(|i| Row::new(vec![i])),
                [Constraint::Length(10); 1],
            )
            .highlight_symbol(">>");
            f.render_stateful_widget(table, layout[1], &mut state.table);

            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            f.render_stateful_widget(scrollbar, layout[2], &mut state.scrollbar);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

const DEFAULT_STATE_BUFFER: [&str; 5] = [
    "awa      │awa       ▲",
    "banana   │banana    █",
    "Cats!!   │Cats!!    ║",
    "d20      │d20       ║",
    "Echo     │Echo      ▼",
];

const DEFAULT_STATE_REPR: &str = r#"{
  "list": {
    "offset": 0,
    "selected": null
  },
  "table": {
    "offset": 0,
    "selected": null,
    "selected_column": null
  },
  "scrollbar": {
    "content_length": 10,
    "position": 0,
    "viewport_content_length": 0
  }
}"#;

#[test]
fn default_state_serialize() {
    let mut state = AppState::default();
    assert_buffer(&mut state, DEFAULT_STATE_BUFFER);
    let state = serde_json::to_string_pretty(&state).unwrap();
    assert_eq!(state, DEFAULT_STATE_REPR);
}

#[test]
fn default_state_deserialize() {
    let mut state: AppState = serde_json::from_str(DEFAULT_STATE_REPR).unwrap();
    assert_buffer(&mut state, DEFAULT_STATE_BUFFER);
}

const SELECTED_STATE_BUFFER: [&str; 5] = [
    "  awa    │  awa     ▲",
    ">>banana │>>banana  █",
    "  Cats!! │  Cats!!  ║",
    "  d20    │  d20     ║",
    "  Echo   │  Echo    ▼",
];
const SELECTED_STATE_REPR: &str = r#"{
  "list": {
    "offset": 0,
    "selected": 1
  },
  "table": {
    "offset": 0,
    "selected": 1,
    "selected_column": 0
  },
  "scrollbar": {
    "content_length": 10,
    "position": 1,
    "viewport_content_length": 0
  }
}"#;

#[test]
fn selected_state_serialize() {
    let mut state = AppState::default();
    state.select(1);
    assert_buffer(&mut state, SELECTED_STATE_BUFFER);
    let state = serde_json::to_string_pretty(&state).unwrap();
    assert_eq!(state, SELECTED_STATE_REPR);
}

#[test]
fn selected_state_deserialize() {
    let mut state: AppState = serde_json::from_str(SELECTED_STATE_REPR).unwrap();
    assert_buffer(&mut state, SELECTED_STATE_BUFFER);
}

const SCROLLED_STATE_BUFFER: [&str; 5] = [
    "  Echo   │  Echo    ▲",
    "  Foxtrot│  Foxtrot ║",
    "  Golf   │  Golf    ║",
    "  Hotel  │  Hotel   █",
    ">>IwI    │>>IwI     ▼",
];

const SCROLLED_STATE_REPR: &str = r#"{
  "list": {
    "offset": 4,
    "selected": 8
  },
  "table": {
    "offset": 4,
    "selected": 8,
    "selected_column": 0
  },
  "scrollbar": {
    "content_length": 10,
    "position": 8,
    "viewport_content_length": 0
  }
}"#;

#[test]
fn scrolled_state_serialize() {
    let mut state = AppState::default();
    state.select(8);
    assert_buffer(&mut state, SCROLLED_STATE_BUFFER);
    let state = serde_json::to_string_pretty(&state).unwrap();
    assert_eq!(state, SCROLLED_STATE_REPR);
}

#[test]
fn scrolled_state_deserialize() {
    let mut state: AppState = serde_json::from_str(SCROLLED_STATE_REPR).unwrap();
    assert_buffer(&mut state, SCROLLED_STATE_BUFFER);
}

// For backwards compatibility these fields should be enough to deserialize the state.
const OLD_TABLE_DESERIALIZE: &str = r#"{
    "offset": 0,
    "selected": 1
}"#;

const NEW_TABLE_DESERIALIZE: &str = r#"{
    "offset": 0,
    "selected": 1,
    "selected_column": null
}"#;

// This test is to check for backwards compatibility with the old states.
#[test]
fn table_state_backwards_compatibility() {
    let old_state: TableState = serde_json::from_str(OLD_TABLE_DESERIALIZE).unwrap();
    let new_state: TableState = serde_json::from_str(NEW_TABLE_DESERIALIZE).unwrap();
    assert_eq!(old_state, new_state);
}
