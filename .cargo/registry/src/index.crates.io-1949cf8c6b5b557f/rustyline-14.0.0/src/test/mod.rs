use std::vec::IntoIter;

use crate::completion::Completer;
use crate::config::{CompletionType, Config, EditMode};
use crate::edit::init_state;
use crate::highlight::Highlighter;
use crate::hint::Hinter;
use crate::history::History;
use crate::keymap::{Bindings, Cmd, InputState};
use crate::keys::{KeyCode as K, KeyEvent, KeyEvent as E, Modifiers as M};
use crate::tty::Sink;
use crate::validate::Validator;
use crate::{apply_backspace_direct, readline_direct, Context, DefaultEditor, Helper, Result};

mod common;
mod emacs;
mod history;
mod vi_cmd;
mod vi_insert;

fn init_editor(mode: EditMode, keys: &[KeyEvent]) -> DefaultEditor {
    let config = Config::builder().edit_mode(mode).build();
    let mut editor = DefaultEditor::with_config(config).unwrap();
    editor.term.keys.extend(keys.iter().cloned());
    editor
}

struct SimpleCompleter;
impl Completer for SimpleCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<String>)> {
        Ok((
            0,
            if line == "rus" {
                vec![line.to_owned() + "t"]
            } else if line == "\\hbar" {
                vec!["ℏ".to_owned()]
            } else {
                vec![]
            },
        ))
    }
}
impl Hinter for SimpleCompleter {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        None
    }
}

impl Helper for SimpleCompleter {}
impl Highlighter for SimpleCompleter {}
impl Validator for SimpleCompleter {}

#[test]
fn complete_line() {
    let mut out = Sink::default();
    let history = crate::history::DefaultHistory::new();
    let helper = Some(SimpleCompleter);
    let mut s = init_state(&mut out, "rus", 3, helper.as_ref(), &history);
    let config = Config::default();
    let bindings = Bindings::new();
    let mut input_state = InputState::new(&config, &bindings);
    let keys = vec![E::ENTER];
    let mut rdr: IntoIter<KeyEvent> = keys.into_iter();
    let cmd = super::complete_line(&mut rdr, &mut s, &mut input_state, &config).unwrap();
    assert_eq!(
        Some(Cmd::AcceptOrInsertLine {
            accept_in_the_middle: true
        }),
        cmd
    );
    assert_eq!("rust", s.line.as_str());
    assert_eq!(4, s.line.pos());
}

#[test]
fn complete_symbol() {
    let mut out = Sink::default();
    let history = crate::history::DefaultHistory::new();
    let helper = Some(SimpleCompleter);
    let mut s = init_state(&mut out, "\\hbar", 5, helper.as_ref(), &history);
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let bindings = Bindings::new();
    let mut input_state = InputState::new(&config, &bindings);
    let keys = vec![E::ENTER];
    let mut rdr: IntoIter<KeyEvent> = keys.into_iter();
    let cmd = super::complete_line(&mut rdr, &mut s, &mut input_state, &config).unwrap();
    assert_eq!(None, cmd);
    assert_eq!("ℏ", s.line.as_str());
    assert_eq!(3, s.line.pos());
}

// `keys`: keys to press
// `expected_line`: line after enter key
fn assert_line(mode: EditMode, keys: &[KeyEvent], expected_line: &str) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline(">>").unwrap();
    assert_eq!(expected_line, actual_line);
}

// `initial`: line status before `keys` pressed: strings before and after cursor
// `keys`: keys to press
// `expected_line`: line after enter key
fn assert_line_with_initial(
    mode: EditMode,
    initial: (&str, &str),
    keys: &[KeyEvent],
    expected_line: &str,
) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline_with_initial(">>", initial).unwrap();
    assert_eq!(expected_line, actual_line);
}

// `initial`: line status before `keys` pressed: strings before and after cursor
// `keys`: keys to press
// `expected`: line status before enter key: strings before and after cursor
fn assert_cursor(mode: EditMode, initial: (&str, &str), keys: &[KeyEvent], expected: (&str, &str)) {
    let mut editor = init_editor(mode, keys);
    let actual_line = editor.readline_with_initial("", initial).unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    assert_eq!(expected.0.len(), editor.term.cursor);
}

// `entries`: history entries before `keys` pressed
// `keys`: keys to press
// `expected`: line status before enter key: strings before and after cursor
fn assert_history(
    mode: EditMode,
    entries: &[&str],
    keys: &[KeyEvent],
    prompt: &str,
    expected: (&str, &str),
) {
    let mut editor = init_editor(mode, keys);
    for entry in entries {
        editor.history.add(entry).unwrap();
    }
    let actual_line = editor.readline(prompt).unwrap();
    assert_eq!(expected.0.to_owned() + expected.1, actual_line);
    if prompt.is_empty() {
        assert_eq!(expected.0.len(), editor.term.cursor);
    }
}

#[test]
fn unknown_esc_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[E(K::UnknownEscSeq, M::NONE), E::ENTER], "");
    }
}

#[test]
fn test_send() {
    fn assert_send<T: Send>() {}
    assert_send::<DefaultEditor>();
}

#[test]
fn test_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<DefaultEditor>();
}

#[test]
fn test_apply_backspace_direct() {
    assert_eq!(
        &apply_backspace_direct("Hel\u{0008}\u{0008}el\u{0008}llo ☹\u{0008}☺"),
        "Hello ☺"
    );
}

#[test]
fn test_readline_direct() {
    use std::io::Cursor;

    let mut write_buf = vec![];
    let output = readline_direct(
        Cursor::new("([)\n\u{0008}\n\n\r\n])".as_bytes()),
        Cursor::new(&mut write_buf),
        &Some(crate::validate::MatchingBracketValidator::new()),
    );

    assert_eq!(
        &write_buf,
        b"Mismatched brackets: '[' is not properly closed"
    );
    assert_eq!(&output.unwrap(), "([\n\n\r\n])");
}
