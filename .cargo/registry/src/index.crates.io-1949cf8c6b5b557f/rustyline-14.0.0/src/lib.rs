//! Readline for Rust
//!
//! This implementation is based on [Antirez's
//! Linenoise](https://github.com/antirez/linenoise)
//!
//! # Example
//!
//! Usage
//!
//! ```
//! let mut rl = rustyline::DefaultEditor::new()?;
//! let readline = rl.readline(">> ");
//! match readline {
//!     Ok(line) => println!("Line: {:?}", line),
//!     Err(_) => println!("No input"),
//! }
//! # Ok::<(), rustyline::error::ReadlineError>(())
//! ```
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "custom-bindings")]
mod binding;
mod command;
pub mod completion;
pub mod config;
mod edit;
pub mod error;
pub mod highlight;
pub mod hint;
pub mod history;
mod keymap;
mod keys;
mod kill_ring;
mod layout;
pub mod line_buffer;
#[cfg(feature = "with-sqlite-history")]
pub mod sqlite_history;
mod tty;
mod undo;
pub mod validate;

use std::fmt;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::result;

use log::debug;
#[cfg(feature = "derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use rustyline_derive::{Completer, Helper, Highlighter, Hinter, Validator};
use unicode_width::UnicodeWidthStr;

use crate::tty::{Buffer, RawMode, RawReader, Renderer, Term, Terminal};

#[cfg(feature = "custom-bindings")]
pub use crate::binding::{ConditionalEventHandler, Event, EventContext, EventHandler};
use crate::completion::{longest_common_prefix, Candidate, Completer};
pub use crate::config::{Behavior, ColorMode, CompletionType, Config, EditMode, HistoryDuplicates};
use crate::edit::State;
use crate::error::ReadlineError;
use crate::highlight::Highlighter;
use crate::hint::Hinter;
use crate::history::{DefaultHistory, History, SearchDirection};
pub use crate::keymap::{Anchor, At, CharSearch, Cmd, InputMode, Movement, RepeatCount, Word};
use crate::keymap::{Bindings, InputState, Refresher};
pub use crate::keys::{KeyCode, KeyEvent, Modifiers};
use crate::kill_ring::KillRing;
pub use crate::tty::ExternalPrinter;
pub use crate::undo::Changeset;
use crate::validate::Validator;

/// The error type for I/O and Linux Syscalls (Errno)
pub type Result<T> = result::Result<T, ReadlineError>;

/// Completes the line/word
fn complete_line<H: Helper>(
    rdr: &mut <Terminal as Term>::Reader,
    s: &mut State<'_, '_, H>,
    input_state: &mut InputState,
    config: &Config,
) -> Result<Option<Cmd>> {
    #[cfg(all(unix, feature = "with-fuzzy"))]
    use skim::prelude::{
        unbounded, Skim, SkimItem, SkimItemReceiver, SkimItemSender, SkimOptionsBuilder,
    };

    let completer = s.helper.unwrap();
    // get a list of completions
    let (start, candidates) = completer.complete(&s.line, s.line.pos(), &s.ctx)?;
    // if no completions, we are done
    if candidates.is_empty() {
        s.out.beep()?;
        Ok(None)
    } else if CompletionType::Circular == config.completion_type() {
        let mark = s.changes.begin();
        // Save the current edited line before overwriting it
        let backup = s.line.as_str().to_owned();
        let backup_pos = s.line.pos();
        let mut cmd;
        let mut i = 0;
        loop {
            // Show completion or original buffer
            if i < candidates.len() {
                let candidate = candidates[i].replacement();
                // TODO we can't highlight the line buffer directly
                /*let candidate = if let Some(highlighter) = s.highlighter {
                    highlighter.highlight_candidate(candidate, CompletionType::Circular)
                } else {
                    Borrowed(candidate)
                };*/
                completer.update(&mut s.line, start, candidate, &mut s.changes);
            } else {
                // Restore current edited line
                s.line.update(&backup, backup_pos, &mut s.changes);
            }
            s.refresh_line()?;

            cmd = s.next_cmd(input_state, rdr, true, true)?;
            match cmd {
                Cmd::Complete => {
                    i = (i + 1) % (candidates.len() + 1); // Circular
                    if i == candidates.len() {
                        s.out.beep()?;
                    }
                }
                Cmd::CompleteBackward => {
                    if i == 0 {
                        i = candidates.len(); // Circular
                        s.out.beep()?;
                    } else {
                        i = (i - 1) % (candidates.len() + 1); // Circular
                    }
                }
                Cmd::Abort => {
                    // Re-show original buffer
                    if i < candidates.len() {
                        s.line.update(&backup, backup_pos, &mut s.changes);
                        s.refresh_line()?;
                    }
                    s.changes.truncate(mark);
                    return Ok(None);
                }
                _ => {
                    s.changes.end();
                    break;
                }
            }
        }
        Ok(Some(cmd))
    } else if CompletionType::List == config.completion_type() {
        if let Some(lcp) = longest_common_prefix(&candidates) {
            // if we can extend the item, extend it
            if lcp.len() > s.line.pos() - start || candidates.len() == 1 {
                completer.update(&mut s.line, start, lcp, &mut s.changes);
                s.refresh_line()?;
            }
        }
        // beep if ambiguous
        if candidates.len() > 1 {
            s.out.beep()?;
        } else {
            return Ok(None);
        }
        // we can't complete any further, wait for second tab
        let mut cmd = s.next_cmd(input_state, rdr, true, true)?;
        // if any character other than tab, pass it to the main loop
        if cmd != Cmd::Complete {
            return Ok(Some(cmd));
        }
        // move cursor to EOL to avoid overwriting the command line
        let save_pos = s.line.pos();
        s.edit_move_end()?;
        s.line.set_pos(save_pos);
        // we got a second tab, maybe show list of possible completions
        let show_completions = if candidates.len() > config.completion_prompt_limit() {
            let msg = format!("\nDisplay all {} possibilities? (y or n)", candidates.len());
            s.out.write_and_flush(msg.as_str())?;
            s.layout.end.row += 1;
            while cmd != Cmd::SelfInsert(1, 'y')
                && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
            {
                cmd = s.next_cmd(input_state, rdr, false, true)?;
            }
            matches!(cmd, Cmd::SelfInsert(1, 'y' | 'Y'))
        } else {
            true
        };
        if show_completions {
            page_completions(rdr, s, input_state, &candidates)
        } else {
            s.refresh_line()?;
            Ok(None)
        }
    } else {
        // if fuzzy feature is enabled and on unix based systems check for the
        // corresponding completion_type
        #[cfg(all(unix, feature = "with-fuzzy"))]
        {
            use std::borrow::Cow;
            if CompletionType::Fuzzy == config.completion_type() {
                struct Candidate {
                    index: usize,
                    text: String,
                }
                impl SkimItem for Candidate {
                    fn text(&self) -> Cow<str> {
                        Cow::Borrowed(&self.text)
                    }
                }

                let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();

                candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| Candidate {
                        index: i,
                        text: c.display().to_owned(),
                    })
                    .for_each(|c| {
                        let _ = tx_item.send(std::sync::Arc::new(c));
                    });
                drop(tx_item); // so that skim could know when to stop waiting for more items.

                // setup skim and run with input options
                // will display UI for fuzzy search and return selected results
                // by default skim multi select is off so only expect one selection

                let options = SkimOptionsBuilder::default()
                    .prompt(Some("? "))
                    .reverse(true)
                    .build()
                    .unwrap();

                let selected_items = Skim::run_with(&options, Some(rx_item))
                    .map(|out| out.selected_items)
                    .unwrap_or_default();

                // match the first (and only) returned option with the candidate and update the
                // line otherwise only refresh line to clear the skim UI changes
                if let Some(item) = selected_items.first() {
                    let item: &Candidate = (*item).as_any() // cast to Any
                        .downcast_ref::<Candidate>() // downcast to concrete type
                        .expect("something wrong with downcast");
                    if let Some(candidate) = candidates.get(item.index) {
                        completer.update(
                            &mut s.line,
                            start,
                            candidate.replacement(),
                            &mut s.changes,
                        );
                    }
                }
                s.refresh_line()?;
            }
        };
        Ok(None)
    }
}

/// Completes the current hint
fn complete_hint_line<H: Helper>(s: &mut State<'_, '_, H>) -> Result<()> {
    let hint = match s.hint.as_ref() {
        Some(hint) => hint,
        None => return Ok(()),
    };
    s.line.move_end();
    if let Some(text) = hint.completion() {
        if s.line.yank(text, 1, &mut s.changes).is_none() {
            s.out.beep()?;
        }
    } else {
        s.out.beep()?;
    }
    s.refresh_line()
}

fn page_completions<C: Candidate, H: Helper>(
    rdr: &mut <Terminal as Term>::Reader,
    s: &mut State<'_, '_, H>,
    input_state: &mut InputState,
    candidates: &[C],
) -> Result<Option<Cmd>> {
    use std::cmp;

    let min_col_pad = 2;
    let cols = s.out.get_columns();
    let max_width = cmp::min(
        cols,
        candidates
            .iter()
            .map(|s| s.display().width())
            .max()
            .unwrap()
            + min_col_pad,
    );
    let num_cols = cols / max_width;

    let mut pause_row = s.out.get_rows() - 1;
    let num_rows = (candidates.len() + num_cols - 1) / num_cols;
    let mut ab = String::new();
    for row in 0..num_rows {
        if row == pause_row {
            s.out.write_and_flush("\n--More--")?;
            let mut cmd = Cmd::Noop;
            while cmd != Cmd::SelfInsert(1, 'y')
                && cmd != Cmd::SelfInsert(1, 'Y')
                && cmd != Cmd::SelfInsert(1, 'n')
                && cmd != Cmd::SelfInsert(1, 'N')
                && cmd != Cmd::SelfInsert(1, 'q')
                && cmd != Cmd::SelfInsert(1, 'Q')
                && cmd != Cmd::SelfInsert(1, ' ')
                && cmd != Cmd::Kill(Movement::BackwardChar(1))
                && cmd != Cmd::AcceptLine
                && cmd != Cmd::Newline
                && !matches!(cmd, Cmd::AcceptOrInsertLine { .. })
            {
                cmd = s.next_cmd(input_state, rdr, false, true)?;
            }
            match cmd {
                Cmd::SelfInsert(1, 'y' | 'Y' | ' ') => {
                    pause_row += s.out.get_rows() - 1;
                }
                Cmd::AcceptLine | Cmd::Newline | Cmd::AcceptOrInsertLine { .. } => {
                    pause_row += 1;
                }
                _ => break,
            }
        }
        s.out.write_and_flush("\n")?;
        ab.clear();
        for col in 0..num_cols {
            let i = (col * num_rows) + row;
            if i < candidates.len() {
                let candidate = &candidates[i].display();
                let width = candidate.width();
                if let Some(highlighter) = s.highlighter() {
                    ab.push_str(&highlighter.highlight_candidate(candidate, CompletionType::List));
                } else {
                    ab.push_str(candidate);
                }
                if ((col + 1) * num_rows) + row < candidates.len() {
                    for _ in width..max_width {
                        ab.push(' ');
                    }
                }
            }
        }
        s.out.write_and_flush(ab.as_str())?;
    }
    s.out.write_and_flush("\n")?;
    s.layout.end.row = 0; // dirty way to make clear_old_rows do nothing
    s.layout.cursor.row = 0;
    s.refresh_line()?;
    Ok(None)
}

/// Incremental search
fn reverse_incremental_search<H: Helper, I: History>(
    rdr: &mut <Terminal as Term>::Reader,
    s: &mut State<'_, '_, H>,
    input_state: &mut InputState,
    history: &I,
) -> Result<Option<Cmd>> {
    if history.is_empty() {
        return Ok(None);
    }
    let mark = s.changes.begin();
    // Save the current edited line (and cursor position) before overwriting it
    let backup = s.line.as_str().to_owned();
    let backup_pos = s.line.pos();

    let mut search_buf = String::new();
    let mut history_idx = history.len() - 1;
    let mut direction = SearchDirection::Reverse;
    let mut success = true;

    let mut cmd;
    // Display the reverse-i-search prompt and process chars
    loop {
        let prompt = if success {
            format!("(reverse-i-search)`{search_buf}': ")
        } else {
            format!("(failed reverse-i-search)`{search_buf}': ")
        };
        s.refresh_prompt_and_line(&prompt)?;

        cmd = s.next_cmd(input_state, rdr, true, true)?;
        if let Cmd::SelfInsert(_, c) = cmd {
            search_buf.push(c);
        } else {
            match cmd {
                Cmd::Kill(Movement::BackwardChar(_)) => {
                    search_buf.pop();
                    continue;
                }
                Cmd::ReverseSearchHistory => {
                    direction = SearchDirection::Reverse;
                    if history_idx > 0 {
                        history_idx -= 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::ForwardSearchHistory => {
                    direction = SearchDirection::Forward;
                    if history_idx < history.len() - 1 {
                        history_idx += 1;
                    } else {
                        success = false;
                        continue;
                    }
                }
                Cmd::Abort => {
                    // Restore current edited line (before search)
                    s.line.update(&backup, backup_pos, &mut s.changes);
                    s.refresh_line()?;
                    s.changes.truncate(mark);
                    return Ok(None);
                }
                Cmd::Move(_) => {
                    s.refresh_line()?; // restore prompt
                    break;
                }
                _ => break,
            }
        }
        success = match history.search(&search_buf, history_idx, direction)? {
            Some(sr) => {
                history_idx = sr.idx;
                s.line.update(&sr.entry, sr.pos, &mut s.changes);
                true
            }
            _ => false,
        };
    }
    s.changes.end();
    Ok(Some(cmd))
}

struct Guard<'m>(&'m tty::Mode);

#[allow(unused_must_use)]
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let Guard(mode) = *self;
        mode.disable_raw_mode();
    }
}

// Helper to handle backspace characters in a direct input
fn apply_backspace_direct(input: &str) -> String {
    // Setup the output buffer
    // No '\b' in the input in the common case, so set the capacity to the input
    // length
    let mut out = String::with_capacity(input.len());

    // Keep track of the size of each grapheme from the input
    // As many graphemes as input bytes in the common case
    let mut grapheme_sizes: Vec<u8> = Vec::with_capacity(input.len());

    for g in unicode_segmentation::UnicodeSegmentation::graphemes(input, true) {
        if g == "\u{0008}" {
            // backspace char
            if let Some(n) = grapheme_sizes.pop() {
                // Remove the last grapheme
                out.truncate(out.len() - n as usize);
            }
        } else {
            out.push_str(g);
            grapheme_sizes.push(g.len() as u8);
        }
    }

    out
}

fn readline_direct(
    mut reader: impl BufRead,
    mut writer: impl Write,
    validator: &Option<impl Validator>,
) -> Result<String> {
    let mut input = String::new();

    loop {
        if reader.read_line(&mut input)? == 0 {
            return Err(ReadlineError::Eof);
        }
        // Remove trailing newline
        let trailing_n = input.ends_with('\n');
        let trailing_r;

        if trailing_n {
            input.pop();
            trailing_r = input.ends_with('\r');
            if trailing_r {
                input.pop();
            }
        } else {
            trailing_r = false;
        }

        input = apply_backspace_direct(&input);

        match validator.as_ref() {
            None => return Ok(input),
            Some(v) => {
                let mut ctx = input.as_str();
                let mut ctx = validate::ValidationContext::new(&mut ctx);

                match v.validate(&mut ctx)? {
                    validate::ValidationResult::Valid(msg) => {
                        if let Some(msg) = msg {
                            writer.write_all(msg.as_bytes())?;
                        }
                        return Ok(input);
                    }
                    validate::ValidationResult::Invalid(Some(msg)) => {
                        writer.write_all(msg.as_bytes())?;
                    }
                    validate::ValidationResult::Incomplete => {
                        // Add newline and keep on taking input
                        if trailing_r {
                            input.push('\r');
                        }
                        if trailing_n {
                            input.push('\n');
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Syntax specific helper.
///
/// TODO Tokenizer/parser used for both completion, suggestion, highlighting.
/// (parse current line once)
pub trait Helper
where
    Self: Completer + Hinter + Highlighter + Validator,
{
}

impl Helper for () {}

impl<'h, H: ?Sized + Helper> Helper for &'h H {}

/// Completion/suggestion context
pub struct Context<'h> {
    history: &'h dyn History,
    history_index: usize,
}

impl<'h> Context<'h> {
    /// Constructor. Visible for testing.
    #[must_use]
    pub fn new(history: &'h dyn History) -> Self {
        Context {
            history,
            history_index: history.len(),
        }
    }

    /// Return an immutable reference to the history object.
    #[must_use]
    pub fn history(&self) -> &dyn History {
        self.history
    }

    /// The history index we are currently editing
    #[must_use]
    pub fn history_index(&self) -> usize {
        self.history_index
    }
}

/// Line editor
#[must_use]
pub struct Editor<H: Helper, I: History> {
    term: Terminal,
    buffer: Option<Buffer>,
    history: I,
    helper: Option<H>,
    kill_ring: KillRing,
    config: Config,
    custom_bindings: Bindings,
}

/// Default editor with no helper and `DefaultHistory`
pub type DefaultEditor = Editor<(), DefaultHistory>;

#[allow(clippy::new_without_default)]
impl<H: Helper> Editor<H, DefaultHistory> {
    /// Create an editor with the default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(Config::default())
    }

    /// Create an editor with a specific configuration.
    pub fn with_config(config: Config) -> Result<Self> {
        Self::with_history(config, DefaultHistory::with_config(config))
    }
}

impl<H: Helper, I: History> Editor<H, I> {
    /// Create an editor with a custom history impl.
    pub fn with_history(config: Config, history: I) -> Result<Self> {
        let term = Terminal::new(
            config.color_mode(),
            config.behavior(),
            config.tab_stop(),
            config.bell_style(),
            config.enable_bracketed_paste(),
            config.enable_signals(),
        )?;
        Ok(Self {
            term,
            buffer: None,
            history,
            helper: None,
            kill_ring: KillRing::new(60),
            config,
            custom_bindings: Bindings::new(),
        })
    }

    /// This method will read a line from STDIN and will display a `prompt`.
    ///
    /// It uses terminal-style interaction if `stdin` is connected to a
    /// terminal.
    /// Otherwise (e.g., if `stdin` is a pipe or the terminal is not supported),
    /// it uses file-style interaction.
    pub fn readline(&mut self, prompt: &str) -> Result<String> {
        self.readline_with(prompt, None)
    }

    /// This function behaves in the exact same manner as `readline`, except
    /// that it pre-populates the input area.
    ///
    /// The text that resides in the input area is given as a 2-tuple.
    /// The string on the left of the tuple is what will appear to the left of
    /// the cursor and the string on the right is what will appear to the
    /// right of the cursor.
    pub fn readline_with_initial(&mut self, prompt: &str, initial: (&str, &str)) -> Result<String> {
        self.readline_with(prompt, Some(initial))
    }

    fn readline_with(&mut self, prompt: &str, initial: Option<(&str, &str)>) -> Result<String> {
        if self.term.is_unsupported() {
            debug!(target: "rustyline", "unsupported terminal");
            // Write prompt and flush it to stdout
            let mut stdout = io::stdout();
            stdout.write_all(prompt.as_bytes())?;
            stdout.flush()?;

            readline_direct(io::stdin().lock(), io::stderr(), &self.helper)
        } else if self.term.is_input_tty() {
            let (original_mode, term_key_map) = self.term.enable_raw_mode()?;
            let guard = Guard(&original_mode);
            let user_input = self.readline_edit(prompt, initial, &original_mode, term_key_map);
            if self.config.auto_add_history() {
                if let Ok(ref line) = user_input {
                    self.add_history_entry(line.as_str())?;
                }
            }
            drop(guard); // disable_raw_mode(original_mode)?;
            self.term.writeln()?;
            user_input
        } else {
            debug!(target: "rustyline", "stdin is not a tty");
            // Not a tty: read from file / pipe.
            readline_direct(io::stdin().lock(), io::stderr(), &self.helper)
        }
    }

    /// Handles reading and editing the readline buffer.
    /// It will also handle special inputs in an appropriate fashion
    /// (e.g., C-c will exit readline)
    fn readline_edit(
        &mut self,
        prompt: &str,
        initial: Option<(&str, &str)>,
        original_mode: &tty::Mode,
        term_key_map: tty::KeyMap,
    ) -> Result<String> {
        let mut stdout = self.term.create_writer();

        self.kill_ring.reset(); // TODO recreate a new kill ring vs reset
        let ctx = Context::new(&self.history);
        let mut s = State::new(&mut stdout, prompt, self.helper.as_ref(), ctx);

        let mut input_state = InputState::new(&self.config, &self.custom_bindings);

        if let Some((left, right)) = initial {
            s.line.update(
                (left.to_owned() + right).as_ref(),
                left.len(),
                &mut s.changes,
            );
        }

        let mut rdr = self
            .term
            .create_reader(self.buffer.take(), &self.config, term_key_map);
        if self.term.is_output_tty() && self.config.check_cursor_position() {
            if let Err(e) = s.move_cursor_at_leftmost(&mut rdr) {
                if let ReadlineError::WindowResized = e {
                    s.out.update_size();
                } else {
                    return Err(e);
                }
            }
        }
        s.refresh_line()?;

        loop {
            let mut cmd = s.next_cmd(&mut input_state, &mut rdr, false, false)?;

            if cmd.should_reset_kill_ring() {
                self.kill_ring.reset();
            }

            // First trigger commands that need extra input

            if cmd == Cmd::Complete && s.helper.is_some() {
                let next = complete_line(&mut rdr, &mut s, &mut input_state, &self.config)?;
                if let Some(next) = next {
                    cmd = next;
                } else {
                    continue;
                }
            }

            if cmd == Cmd::ReverseSearchHistory {
                // Search history backward
                let next =
                    reverse_incremental_search(&mut rdr, &mut s, &mut input_state, &self.history)?;
                if let Some(next) = next {
                    cmd = next;
                } else {
                    continue;
                }
            }

            #[cfg(unix)]
            if cmd == Cmd::Suspend {
                original_mode.disable_raw_mode()?;
                tty::suspend()?;
                let _ = self.term.enable_raw_mode()?; // TODO original_mode may have changed
                s.out.update_size(); // window may have been resized
                s.refresh_line()?;
                continue;
            }

            #[cfg(unix)]
            if cmd == Cmd::QuotedInsert {
                // Quoted insert
                let c = rdr.next_char()?;
                s.edit_insert(c, 1)?;
                continue;
            }

            #[cfg(windows)]
            if cmd == Cmd::PasteFromClipboard {
                let clipboard = rdr.read_pasted_text()?;
                s.edit_yank(&input_state, &clipboard[..], Anchor::Before, 1)?;
            }

            // Tiny test quirk
            #[cfg(test)]
            if matches!(
                cmd,
                Cmd::AcceptLine | Cmd::Newline | Cmd::AcceptOrInsertLine { .. }
            ) {
                self.term.cursor = s.layout.cursor.col;
            }

            // Execute things can be done solely on a state object
            match command::execute(cmd, &mut s, &input_state, &mut self.kill_ring, &self.config)? {
                command::Status::Proceed => continue,
                command::Status::Submit => break,
            }
        }

        // Move to end, in case cursor was in the middle of the line, so that
        // next thing application prints goes after the input
        s.forced_refresh = true;
        s.edit_move_buffer_end()?;
        s.forced_refresh = false;

        if cfg!(windows) {
            let _ = original_mode; // silent warning
        }
        self.buffer = rdr.unbuffer();
        Ok(s.line.into_string())
    }

    /// Load the history from the specified file.
    pub fn load_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        self.history.load(path.as_ref())
    }

    /// Save the history in the specified file.
    pub fn save_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        self.history.save(path.as_ref())
    }

    /// Append new entries in the specified file.
    pub fn append_history<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        self.history.append(path.as_ref())
    }

    /// Add a new entry in the history.
    pub fn add_history_entry<S: AsRef<str> + Into<String>>(&mut self, line: S) -> Result<bool> {
        self.history.add(line.as_ref())
    }

    /// Clear history.
    pub fn clear_history(&mut self) -> Result<()> {
        self.history.clear()
    }

    /// Return a mutable reference to the history object.
    pub fn history_mut(&mut self) -> &mut I {
        &mut self.history
    }

    /// Return an immutable reference to the history object.
    pub fn history(&self) -> &I {
        &self.history
    }

    /// Register a callback function to be called for tab-completion
    /// or to show hints to the user at the right of the prompt.
    pub fn set_helper(&mut self, helper: Option<H>) {
        self.helper = helper;
    }

    /// Return a mutable reference to the helper.
    pub fn helper_mut(&mut self) -> Option<&mut H> {
        self.helper.as_mut()
    }

    /// Return an immutable reference to the helper.
    pub fn helper(&self) -> Option<&H> {
        self.helper.as_ref()
    }

    /// Bind a sequence to a command.
    #[cfg(feature = "custom-bindings")]
    #[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
    pub fn bind_sequence<E: Into<Event>, R: Into<EventHandler>>(
        &mut self,
        key_seq: E,
        handler: R,
    ) -> Option<EventHandler> {
        self.custom_bindings
            .insert(Event::normalize(key_seq.into()), handler.into())
    }

    /// Remove a binding for the given sequence.
    #[cfg(feature = "custom-bindings")]
    #[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
    pub fn unbind_sequence<E: Into<Event>>(&mut self, key_seq: E) -> Option<EventHandler> {
        self.custom_bindings
            .remove(&Event::normalize(key_seq.into()))
    }

    /// Returns an iterator over edited lines.
    /// Iterator ends at [EOF](ReadlineError::Eof).
    /// ```
    /// let mut rl = rustyline::DefaultEditor::new()?;
    /// for readline in rl.iter("> ") {
    ///     match readline {
    ///         Ok(line) => {
    ///             println!("Line: {}", line);
    ///         }
    ///         Err(err) => {
    ///             println!("Error: {:?}", err);
    ///             break;
    ///         }
    ///     }
    /// }
    /// # Ok::<(), rustyline::error::ReadlineError>(())
    /// ```
    pub fn iter<'a>(&'a mut self, prompt: &'a str) -> impl Iterator<Item = Result<String>> + 'a {
        Iter {
            editor: self,
            prompt,
        }
    }

    /// If output stream is a tty, this function returns its width and height as
    /// a number of characters.
    pub fn dimensions(&mut self) -> Option<(usize, usize)> {
        if self.term.is_output_tty() {
            let out = self.term.create_writer();
            Some((out.get_columns(), out.get_rows()))
        } else {
            None
        }
    }

    /// Clear the screen.
    pub fn clear_screen(&mut self) -> Result<()> {
        if self.term.is_output_tty() {
            let mut out = self.term.create_writer();
            out.clear_screen()
        } else {
            Ok(())
        }
    }

    /// Create an external printer
    pub fn create_external_printer(&mut self) -> Result<<Terminal as Term>::ExternalPrinter> {
        self.term.create_external_printer()
    }

    /// Change cursor visibility
    pub fn set_cursor_visibility(
        &mut self,
        visible: bool,
    ) -> Result<Option<<Terminal as Term>::CursorGuard>> {
        self.term.set_cursor_visibility(visible)
    }
}

impl<H: Helper, I: History> config::Configurer for Editor<H, I> {
    fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    fn set_max_history_size(&mut self, max_size: usize) -> Result<()> {
        self.config_mut().set_max_history_size(max_size);
        self.history.set_max_len(max_size)
    }

    fn set_history_ignore_dups(&mut self, yes: bool) -> Result<()> {
        self.config_mut().set_history_ignore_dups(yes);
        self.history.ignore_dups(yes)
    }

    fn set_history_ignore_space(&mut self, yes: bool) {
        self.config_mut().set_history_ignore_space(yes);
        self.history.ignore_space(yes);
    }

    fn set_color_mode(&mut self, color_mode: ColorMode) {
        self.config_mut().set_color_mode(color_mode);
        self.term.color_mode = color_mode;
    }
}

impl<H: Helper, I: History> fmt::Debug for Editor<H, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Editor")
            .field("term", &self.term)
            .field("config", &self.config)
            .finish()
    }
}

struct Iter<'a, H: Helper, I: History> {
    editor: &'a mut Editor<H, I>,
    prompt: &'a str,
}

impl<'a, H: Helper, I: History> Iterator for Iter<'a, H, I> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let readline = self.editor.readline(self.prompt);
        match readline {
            Ok(l) => Some(Ok(l)),
            Err(ReadlineError::Eof) => None,
            e @ Err(_) => Some(e),
        }
    }
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;
#[cfg(test)]
mod test;

#[cfg(doctest)]
doc_comment::doctest!("../README.md");
