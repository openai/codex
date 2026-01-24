//! TUI-specific events for the retrieval application.
//!
//! These events are internal to the TUI and separate from the retrieval system
//! events in `events.rs`. They handle UI interactions and view navigation.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

use crate::events::RetrievalEvent;
use crate::types::SearchResult;

/// TUI application events.
///
/// These events drive the TUI state machine and handle:
/// - User input (keyboard, mouse)
/// - View navigation
/// - Async operation results
/// - Retrieval system events
#[derive(Debug, Clone)]
pub enum AppEvent {
    // ========================================================================
    // Input Events
    // ========================================================================
    /// Keyboard input.
    Key(KeyEvent),

    /// Text pasted from clipboard.
    Paste(String),

    /// Terminal resize.
    Resize { width: u16, height: u16 },

    // ========================================================================
    // Navigation Events
    // ========================================================================
    /// Switch to a different view.
    SwitchView(ViewMode),

    /// Navigate to next tab.
    NextTab,

    /// Navigate to previous tab.
    PrevTab,

    /// Go back (close modal, etc.).
    GoBack,

    /// Request to quit the application.
    Quit,

    // ========================================================================
    // Search Events
    // ========================================================================
    /// Search query changed.
    SearchQueryChanged(String),

    /// Search mode changed.
    SearchModeChanged(crate::events::SearchMode),

    /// Search results received.
    SearchResults {
        query_id: String,
        results: Vec<SearchResult>,
        duration_ms: i64,
    },

    /// Search error occurred.
    SearchError { query_id: String, error: String },

    /// Select a search result.
    SelectResult(usize),

    /// Open selected result in editor.
    OpenResult,

    // ========================================================================
    // Index Events
    // ========================================================================
    /// Start index build.
    StartBuild { clean: bool },

    /// Stop index build.
    StopBuild,

    /// Index build error occurred.
    BuildError { error: String },

    /// Index build was cancelled by user.
    BuildCancelled,

    /// Start file watching.
    StartWatch,

    /// Stop file watching.
    StopWatch,

    /// File watcher error occurred.
    WatchError { error: String },

    // ========================================================================
    // RepoMap Events
    // ========================================================================
    /// Generate repo map.
    GenerateRepoMap,

    /// Refresh repo map.
    RefreshRepoMap,

    /// Change token budget.
    ChangeTokenBudget(i32),

    /// RepoMap generated successfully.
    RepoMapGenerated {
        content: String,
        tokens: i32,
        files: i32,
        duration_ms: i64,
    },

    /// RepoMap generation error occurred.
    RepoMapError { error: String },

    // ========================================================================
    // Retrieval System Events
    // ========================================================================
    /// Retrieval system event received.
    RetrievalEvent(RetrievalEvent),

    // ========================================================================
    // UI Control Events
    // ========================================================================
    /// Request a redraw.
    Redraw,

    /// Tick for animations/updates.
    Tick,

    /// Show help overlay.
    ShowHelp,

    /// Hide help overlay.
    HideHelp,
}

/// View modes for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Search view (main view).
    #[default]
    Search,

    /// Index management view.
    Index,

    /// RepoMap explorer view.
    RepoMap,

    /// File watch view.
    Watch,

    /// Debug/pipeline view.
    Debug,
}

impl ViewMode {
    /// Get all view modes in tab order.
    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::Search,
            ViewMode::Index,
            ViewMode::RepoMap,
            ViewMode::Watch,
            ViewMode::Debug,
        ]
    }

    /// Get the display name for the view.
    pub fn name(&self) -> &'static str {
        match self {
            ViewMode::Search => "Search",
            ViewMode::Index => "Index",
            ViewMode::RepoMap => "RepoMap",
            ViewMode::Watch => "Watch",
            ViewMode::Debug => "Debug",
        }
    }

    /// Get the tab index.
    pub fn index(&self) -> usize {
        match self {
            ViewMode::Search => 0,
            ViewMode::Index => 1,
            ViewMode::RepoMap => 2,
            ViewMode::Watch => 3,
            ViewMode::Debug => 4,
        }
    }

    /// Get view mode from tab index.
    pub fn from_index(index: usize) -> Option<ViewMode> {
        Self::all().get(index).copied()
    }

    /// Get the next view mode.
    pub fn next(&self) -> ViewMode {
        let idx = (self.index() + 1) % Self::all().len();
        Self::from_index(idx).unwrap_or_default()
    }

    /// Get the previous view mode.
    pub fn prev(&self) -> ViewMode {
        let idx = if self.index() == 0 {
            Self::all().len() - 1
        } else {
            self.index() - 1
        };
        Self::from_index(idx).unwrap_or_default()
    }
}

/// Key binding helper functions.
pub mod keybindings {
    use super::*;

    /// Check if key is quit (q or Ctrl+C).
    pub fn is_quit(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('q'))
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c')))
    }

    /// Check if key is help (?).
    pub fn is_help(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('?'))
    }

    /// Check if key is escape.
    pub fn is_escape(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Esc)
    }

    /// Check if key is enter.
    pub fn is_enter(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Enter)
    }

    /// Check if key is tab (next).
    pub fn is_tab(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Tab) && !key.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if key is shift+tab (prev).
    pub fn is_shift_tab(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::BackTab)
            || (matches!(key.code, KeyCode::Tab) && key.modifiers.contains(KeyModifiers::SHIFT))
    }

    /// Check if key is up arrow.
    pub fn is_up(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Up)
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('p')))
    }

    /// Check if key is down arrow.
    pub fn is_down(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Down)
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('n')))
    }

    /// Check if key is page up.
    pub fn is_page_up(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::PageUp)
    }

    /// Check if key is page down.
    pub fn is_page_down(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::PageDown)
    }

    /// Check if key is home.
    pub fn is_home(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Home)
    }

    /// Check if key is end.
    pub fn is_end(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::End)
    }

    /// Get number key (1-5) for tab switching.
    pub fn get_number_key(key: &KeyEvent) -> Option<usize> {
        match key.code {
            KeyCode::Char('1') => Some(0),
            KeyCode::Char('2') => Some(1),
            KeyCode::Char('3') => Some(2),
            KeyCode::Char('4') => Some(3),
            KeyCode::Char('5') => Some(4),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_navigation() {
        assert_eq!(ViewMode::Search.next(), ViewMode::Index);
        assert_eq!(ViewMode::Debug.next(), ViewMode::Search);
        assert_eq!(ViewMode::Search.prev(), ViewMode::Debug);
        assert_eq!(ViewMode::Index.prev(), ViewMode::Search);
    }

    #[test]
    fn test_view_mode_index() {
        assert_eq!(ViewMode::Search.index(), 0);
        assert_eq!(ViewMode::Debug.index(), 4);
        assert_eq!(ViewMode::from_index(0), Some(ViewMode::Search));
        assert_eq!(ViewMode::from_index(4), Some(ViewMode::Debug));
        assert_eq!(ViewMode::from_index(5), None);
    }

    #[test]
    fn test_keybindings() {
        let quit_key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(keybindings::is_quit(&quit_key));

        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(keybindings::is_quit(&ctrl_c));

        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert!(keybindings::is_tab(&tab_key));

        let shift_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
        assert!(keybindings::is_shift_tab(&shift_tab));
    }
}
