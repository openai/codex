//! Customize line editor
use crate::Result;
use std::default::Default;

/// User preferences
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Maximum number of entries in History.
    max_history_size: usize, // history_max_entries
    history_duplicates: HistoryDuplicates,
    history_ignore_space: bool,
    completion_type: CompletionType,
    /// When listing completion alternatives, only display
    /// one screen of possibilities at a time.
    completion_prompt_limit: usize,
    /// Duration (milliseconds) Rustyline will wait for a character when
    /// reading an ambiguous key sequence.
    keyseq_timeout: Option<u16>,
    /// Emacs or Vi mode
    edit_mode: EditMode,
    /// If true, each nonblank line returned by `readline` will be
    /// automatically added to the history.
    auto_add_history: bool,
    /// Beep or Flash or nothing
    bell_style: BellStyle,
    /// if colors should be enabled.
    color_mode: ColorMode,
    /// Whether to use stdio or not
    behavior: Behavior,
    /// Horizontal space taken by a tab.
    tab_stop: usize,
    /// Indentation size for indent/dedent commands
    indent_size: usize,
    /// Check if cursor position is at leftmost before displaying prompt
    check_cursor_position: bool,
    /// Bracketed paste on unix platform
    enable_bracketed_paste: bool,
    /// Whether to disable or not the signals in termios
    enable_signals: bool,
}

impl Config {
    /// Returns a `Config` builder.
    #[must_use]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Tell the maximum length (i.e. number of entries) for the history.
    #[must_use]
    pub fn max_history_size(&self) -> usize {
        self.max_history_size
    }

    pub(crate) fn set_max_history_size(&mut self, max_size: usize) {
        self.max_history_size = max_size;
    }

    /// Tell if lines which match the previous history entry are saved or not
    /// in the history list.
    ///
    /// By default, they are ignored.
    #[must_use]
    pub fn history_duplicates(&self) -> HistoryDuplicates {
        self.history_duplicates
    }

    pub(crate) fn set_history_ignore_dups(&mut self, yes: bool) {
        self.history_duplicates = if yes {
            HistoryDuplicates::IgnoreConsecutive
        } else {
            HistoryDuplicates::AlwaysAdd
        };
    }

    /// Tell if lines which begin with a space character are saved or not in
    /// the history list.
    ///
    /// By default, they are saved.
    #[must_use]
    pub fn history_ignore_space(&self) -> bool {
        self.history_ignore_space
    }

    pub(crate) fn set_history_ignore_space(&mut self, yes: bool) {
        self.history_ignore_space = yes;
    }

    /// Completion behaviour.
    ///
    /// By default, `CompletionType::Circular`.
    #[must_use]
    pub fn completion_type(&self) -> CompletionType {
        self.completion_type
    }

    /// When listing completion alternatives, only display
    /// one screen of possibilities at a time (used for `CompletionType::List`
    /// mode).
    #[must_use]
    pub fn completion_prompt_limit(&self) -> usize {
        self.completion_prompt_limit
    }

    /// Duration (milliseconds) Rustyline will wait for a character when
    /// reading an ambiguous key sequence (used for `EditMode::Vi` mode on unix
    /// platform).
    ///
    /// By default, no timeout (-1) or 500ms if `EditMode::Vi` is activated.
    #[must_use]
    pub fn keyseq_timeout(&self) -> Option<u16> {
        self.keyseq_timeout
    }

    /// Emacs or Vi mode
    #[must_use]
    pub fn edit_mode(&self) -> EditMode {
        self.edit_mode
    }

    /// Tell if lines are automatically added to the history.
    ///
    /// By default, they are not.
    #[must_use]
    pub fn auto_add_history(&self) -> bool {
        self.auto_add_history
    }

    /// Bell style: beep, flash or nothing.
    #[must_use]
    pub fn bell_style(&self) -> BellStyle {
        self.bell_style
    }

    /// Tell if colors should be enabled.
    ///
    /// By default, they are except if stdout is not a TTY.
    #[must_use]
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    pub(crate) fn set_color_mode(&mut self, color_mode: ColorMode) {
        self.color_mode = color_mode;
    }

    /// Whether to use stdio or not
    ///
    /// By default, stdio is used.
    #[must_use]
    pub fn behavior(&self) -> Behavior {
        self.behavior
    }

    pub(crate) fn set_behavior(&mut self, behavior: Behavior) {
        self.behavior = behavior;
    }

    /// Horizontal space taken by a tab.
    ///
    /// By default, 8.
    #[must_use]
    pub fn tab_stop(&self) -> usize {
        self.tab_stop
    }

    pub(crate) fn set_tab_stop(&mut self, tab_stop: usize) {
        self.tab_stop = tab_stop;
    }

    /// Check if cursor position is at leftmost before displaying prompt.
    ///
    /// By default, we don't check.
    #[must_use]
    pub fn check_cursor_position(&self) -> bool {
        self.check_cursor_position
    }

    /// Indentation size used by indentation commands
    ///
    /// By default, 2.
    #[must_use]
    pub fn indent_size(&self) -> usize {
        self.indent_size
    }

    pub(crate) fn set_indent_size(&mut self, indent_size: usize) {
        self.indent_size = indent_size;
    }

    /// Bracketed paste on unix platform
    ///
    /// By default, it's enabled.
    #[must_use]
    pub fn enable_bracketed_paste(&self) -> bool {
        self.enable_bracketed_paste
    }

    /// Enable or disable signals in termios
    ///
    /// By default, it's disabled.
    #[must_use]
    pub fn enable_signals(&self) -> bool {
        self.enable_signals
    }

    pub(crate) fn set_enable_signals(&mut self, enable_signals: bool) {
        self.enable_signals = enable_signals;
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_history_size: 100,
            history_duplicates: HistoryDuplicates::IgnoreConsecutive,
            history_ignore_space: false,
            completion_type: CompletionType::Circular, // TODO Validate
            completion_prompt_limit: 100,
            keyseq_timeout: None,
            edit_mode: EditMode::Emacs,
            auto_add_history: false,
            bell_style: BellStyle::default(),
            color_mode: ColorMode::Enabled,
            behavior: Behavior::default(),
            tab_stop: 8,
            indent_size: 2,
            check_cursor_position: false,
            enable_bracketed_paste: true,
            enable_signals: false,
        }
    }
}

/// Beep or flash or nothing
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BellStyle {
    /// Beep
    Audible,
    /// Silent
    None,
    /// Flash screen (not supported)
    Visible,
}

/// `Audible` by default on unix (overridden by current Terminal settings).
/// `None` on windows.
impl Default for BellStyle {
    #[cfg(any(windows, target_arch = "wasm32"))]
    fn default() -> Self {
        BellStyle::None
    }

    #[cfg(unix)]
    fn default() -> Self {
        BellStyle::Audible
    }
}

/// History filter
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDuplicates {
    /// No filter
    AlwaysAdd,
    /// a line will not be added to the history if it matches the previous entry
    IgnoreConsecutive,
}

/// Tab completion style
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionType {
    /// Complete the next full match (like in Vim by default)
    Circular,
    /// Complete till longest match.
    /// When more than one match, list all matches
    /// (like in Bash/Readline).
    List,

    /// Complete the match using fuzzy search and selection
    /// (like fzf and plugins)
    /// Currently only available for unix platforms as dependency on
    /// skim->tuikit Compile with `--features=fuzzy` to enable
    #[cfg(all(unix, feature = "with-fuzzy"))]
    Fuzzy,
}

/// Style of editing / Standard keymaps
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditMode {
    /// Emacs keymap
    Emacs,
    /// Vi keymap
    Vi,
}

/// Colorization mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorMode {
    /// Activate highlighting if platform/terminal is supported.
    Enabled,
    /// Activate highlighting even if platform is not supported (windows < 10).
    Forced,
    /// Deactivate highlighting even if platform/terminal is supported.
    Disabled,
}

/// Should the editor use stdio
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Behavior {
    /// Use stdin / stdout
    #[default]
    Stdio,
    /// Use terminal-style interaction whenever possible, even if 'stdin' and/or
    /// 'stdout' are not terminals.
    PreferTerm,
    // TODO
    // Use file-style interaction, reading input from the given file.
    // useFile
}

/// Configuration builder
#[derive(Clone, Debug, Default)]
pub struct Builder {
    p: Config,
}

impl Builder {
    /// Returns a `Config` builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            p: Config::default(),
        }
    }

    /// Set the maximum length for the history.
    pub fn max_history_size(mut self, max_size: usize) -> Result<Self> {
        self.set_max_history_size(max_size)?;
        Ok(self)
    }

    /// Tell if lines which match the previous history entry are saved or not
    /// in the history list.
    ///
    /// By default, they are ignored.
    pub fn history_ignore_dups(mut self, yes: bool) -> Result<Self> {
        self.set_history_ignore_dups(yes)?;
        Ok(self)
    }

    /// Tell if lines which begin with a space character are saved or not in
    /// the history list.
    ///
    /// By default, they are saved.
    #[must_use]
    pub fn history_ignore_space(mut self, yes: bool) -> Self {
        self.set_history_ignore_space(yes);
        self
    }

    /// Set `completion_type`.
    #[must_use]
    pub fn completion_type(mut self, completion_type: CompletionType) -> Self {
        self.set_completion_type(completion_type);
        self
    }

    /// The number of possible completions that determines when the user is
    /// asked whether the list of possibilities should be displayed.
    #[must_use]
    pub fn completion_prompt_limit(mut self, completion_prompt_limit: usize) -> Self {
        self.set_completion_prompt_limit(completion_prompt_limit);
        self
    }

    /// Timeout for ambiguous key sequences in milliseconds.
    /// Currently, it is used only to distinguish a single ESC from an ESC
    /// sequence.
    /// After seeing an ESC key, wait at most `keyseq_timeout_ms` for another
    /// byte.
    #[must_use]
    pub fn keyseq_timeout(mut self, keyseq_timeout_ms: Option<u16>) -> Self {
        self.set_keyseq_timeout(keyseq_timeout_ms);
        self
    }

    /// Choose between Emacs or Vi mode.
    #[must_use]
    pub fn edit_mode(mut self, edit_mode: EditMode) -> Self {
        self.set_edit_mode(edit_mode);
        self
    }

    /// Tell if lines are automatically added to the history.
    ///
    /// By default, they are not.
    #[must_use]
    pub fn auto_add_history(mut self, yes: bool) -> Self {
        self.set_auto_add_history(yes);
        self
    }

    /// Set bell style: beep, flash or nothing.
    #[must_use]
    pub fn bell_style(mut self, bell_style: BellStyle) -> Self {
        self.set_bell_style(bell_style);
        self
    }

    /// Forces colorization on or off.
    ///
    /// By default, colorization is on except if stdout is not a TTY.
    #[must_use]
    pub fn color_mode(mut self, color_mode: ColorMode) -> Self {
        self.set_color_mode(color_mode);
        self
    }

    /// Whether to use stdio or not
    ///
    /// By default, stdio is used.
    #[must_use]
    pub fn behavior(mut self, behavior: Behavior) -> Self {
        self.set_behavior(behavior);
        self
    }

    /// Horizontal space taken by a tab.
    ///
    /// By default, `8`
    #[must_use]
    pub fn tab_stop(mut self, tab_stop: usize) -> Self {
        self.set_tab_stop(tab_stop);
        self
    }

    /// Check if cursor position is at leftmost before displaying prompt.
    ///
    /// By default, we don't check.
    #[must_use]
    pub fn check_cursor_position(mut self, yes: bool) -> Self {
        self.set_check_cursor_position(yes);
        self
    }

    /// Indentation size
    ///
    /// By default, `2`
    #[must_use]
    pub fn indent_size(mut self, indent_size: usize) -> Self {
        self.set_indent_size(indent_size);
        self
    }

    /// Enable or disable bracketed paste on unix platform
    ///
    /// By default, it's enabled.
    #[must_use]
    pub fn bracketed_paste(mut self, enabled: bool) -> Self {
        self.enable_bracketed_paste(enabled);
        self
    }

    /// Enable or disable signals in termios
    ///
    /// By default, it's disabled.
    #[must_use]
    pub fn enable_signals(mut self, enable_signals: bool) -> Self {
        self.p.set_enable_signals(enable_signals);
        self
    }

    /// Builds a `Config` with the settings specified so far.
    #[must_use]
    pub fn build(self) -> Config {
        self.p
    }
}

impl Configurer for Builder {
    fn config_mut(&mut self) -> &mut Config {
        &mut self.p
    }
}

/// Trait for component that holds a `Config`.
pub trait Configurer {
    /// `Config` accessor.
    fn config_mut(&mut self) -> &mut Config;

    /// Set the maximum length for the history.
    fn set_max_history_size(&mut self, max_size: usize) -> Result<()> {
        self.config_mut().set_max_history_size(max_size);
        Ok(())
    }

    /// Tell if lines which match the previous history entry are saved or not
    /// in the history list.
    ///
    /// By default, they are ignored.
    fn set_history_ignore_dups(&mut self, yes: bool) -> Result<()> {
        self.config_mut().set_history_ignore_dups(yes);
        Ok(())
    }

    /// Tell if lines which begin with a space character are saved or not in
    /// the history list.
    ///
    /// By default, they are saved.
    fn set_history_ignore_space(&mut self, yes: bool) {
        self.config_mut().set_history_ignore_space(yes);
    }
    /// Set `completion_type`.
    fn set_completion_type(&mut self, completion_type: CompletionType) {
        self.config_mut().completion_type = completion_type;
    }

    /// The number of possible completions that determines when the user is
    /// asked whether the list of possibilities should be displayed.
    fn set_completion_prompt_limit(&mut self, completion_prompt_limit: usize) {
        self.config_mut().completion_prompt_limit = completion_prompt_limit;
    }

    /// Timeout for ambiguous key sequences in milliseconds.
    fn set_keyseq_timeout(&mut self, keyseq_timeout_ms: Option<u16>) {
        self.config_mut().keyseq_timeout = keyseq_timeout_ms;
    }

    /// Choose between Emacs or Vi mode.
    fn set_edit_mode(&mut self, edit_mode: EditMode) {
        self.config_mut().edit_mode = edit_mode;
        match edit_mode {
            EditMode::Emacs => self.set_keyseq_timeout(None), // no timeout
            EditMode::Vi => self.set_keyseq_timeout(Some(500)),
        }
    }

    /// Tell if lines are automatically added to the history.
    ///
    /// By default, they are not.
    fn set_auto_add_history(&mut self, yes: bool) {
        self.config_mut().auto_add_history = yes;
    }

    /// Set bell style: beep, flash or nothing.
    fn set_bell_style(&mut self, bell_style: BellStyle) {
        self.config_mut().bell_style = bell_style;
    }

    /// Forces colorization on or off.
    ///
    /// By default, colorization is on except if stdout is not a TTY.
    fn set_color_mode(&mut self, color_mode: ColorMode) {
        self.config_mut().set_color_mode(color_mode);
    }

    /// Whether to use stdio or not
    ///
    /// By default, stdio is used.
    fn set_behavior(&mut self, behavior: Behavior) {
        self.config_mut().set_behavior(behavior);
    }

    /// Horizontal space taken by a tab.
    ///
    /// By default, `8`
    fn set_tab_stop(&mut self, tab_stop: usize) {
        self.config_mut().set_tab_stop(tab_stop);
    }

    /// Check if cursor position is at leftmost before displaying prompt.
    ///
    /// By default, we don't check.
    fn set_check_cursor_position(&mut self, yes: bool) {
        self.config_mut().check_cursor_position = yes;
    }
    /// Indentation size for indent/dedent commands
    ///
    /// By default, `2`
    fn set_indent_size(&mut self, size: usize) {
        self.config_mut().set_indent_size(size);
    }

    /// Enable or disable bracketed paste on unix platform
    ///
    /// By default, it's enabled.
    fn enable_bracketed_paste(&mut self, enabled: bool) {
        self.config_mut().enable_bracketed_paste = enabled;
    }

    /// Enable or disable signals in termios
    ///
    /// By default, it's disabled.
    fn set_enable_signals(&mut self, enable_signals: bool) {
        self.config_mut().set_enable_signals(enable_signals);
    }
}
