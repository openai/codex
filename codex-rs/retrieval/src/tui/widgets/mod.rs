//! Reusable UI widgets for the retrieval TUI.
//!
//! These widgets follow ratatui patterns and can be composed in views.

pub mod event_log;
pub mod progress_bar;
pub mod result_list;
pub mod search_input;
pub mod search_pipeline;
pub mod stats_panel;

pub use event_log::EventLog;
pub use event_log::EventLogState;
pub use progress_bar::ProgressBar;
pub use progress_bar::ProgressBarState;
pub use result_list::ResultList;
pub use result_list::ResultListState;
pub use search_input::SearchInput;
pub use search_input::SearchInputState;
pub use search_pipeline::SearchPipeline;
pub use stats_panel::StatsPanel;
pub use stats_panel::StatsPanelState;
