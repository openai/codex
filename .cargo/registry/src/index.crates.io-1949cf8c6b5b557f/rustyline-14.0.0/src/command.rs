use crate::complete_hint_line;
use crate::config::Config;
use crate::edit::State;
use crate::error;
use crate::history::SearchDirection;
use crate::keymap::{Anchor, At, Cmd, Movement, Word};
use crate::keymap::{InputState, Refresher};
use crate::kill_ring::{KillRing, Mode};
use crate::line_buffer::WordAction;
use crate::{Helper, Result};

pub enum Status {
    Proceed,
    Submit,
}

pub fn execute<H: Helper>(
    cmd: Cmd,
    s: &mut State<'_, '_, H>,
    input_state: &InputState,
    kill_ring: &mut KillRing,
    config: &Config,
) -> Result<Status> {
    use Status::{Proceed, Submit};

    match cmd {
        Cmd::EndOfFile | Cmd::AcceptLine | Cmd::AcceptOrInsertLine { .. } | Cmd::Newline => {
            if s.has_hint() || !s.is_default_prompt() || s.highlight_char {
                // Force a refresh without hints to leave the previous
                // line as the user typed it after a newline.
                s.forced_refresh = true;
                s.refresh_line_with_msg(None)?;
                s.forced_refresh = false;
            }
        }
        _ => {}
    };
    match cmd {
        Cmd::CompleteHint => {
            complete_hint_line(s)?;
        }
        Cmd::SelfInsert(n, c) => {
            s.edit_insert(c, n)?;
        }
        Cmd::Insert(n, text) => {
            s.edit_yank(input_state, &text, Anchor::Before, n)?;
        }
        Cmd::Move(Movement::BeginningOfLine) => {
            // Move to the beginning of line.
            s.edit_move_home()?;
        }
        Cmd::Move(Movement::ViFirstPrint) => {
            s.edit_move_home()?;
            if s.line.starts_with(char::is_whitespace) {
                s.edit_move_to_next_word(At::Start, Word::Big, 1)?;
            }
        }
        Cmd::Move(Movement::BackwardChar(n)) => {
            // Move back a character.
            s.edit_move_backward(n)?;
        }
        Cmd::ReplaceChar(n, c) => s.edit_replace_char(c, n)?,
        Cmd::Replace(mvt, text) => {
            s.edit_kill(&mvt, kill_ring)?;
            if let Some(text) = text {
                s.edit_insert_text(&text)?;
            }
        }
        Cmd::Overwrite(c) => {
            s.edit_overwrite_char(c)?;
        }
        Cmd::EndOfFile => {
            if s.line.is_empty() {
                return Err(error::ReadlineError::Eof);
            } else if !input_state.is_emacs_mode() {
                return Ok(Submit);
            }
        }
        Cmd::Move(Movement::EndOfLine) => {
            // Move to the end of line.
            s.edit_move_end()?;
        }
        Cmd::Move(Movement::ForwardChar(n)) => {
            // Move forward a character.
            s.edit_move_forward(n)?;
        }
        Cmd::ClearScreen => {
            // Clear the screen leaving the current line at the top of the screen.
            s.clear_screen()?;
            s.refresh_line()?;
        }
        Cmd::NextHistory => {
            // Fetch the next command from the history list.
            s.edit_history_next(false)?;
        }
        Cmd::PreviousHistory => {
            // Fetch the previous command from the history list.
            s.edit_history_next(true)?;
        }
        Cmd::LineUpOrPreviousHistory(n) => {
            if !s.edit_move_line_up(n)? {
                s.edit_history_next(true)?;
            }
        }
        Cmd::LineDownOrNextHistory(n) => {
            if !s.edit_move_line_down(n)? {
                s.edit_history_next(false)?;
            }
        }
        Cmd::HistorySearchBackward => s.edit_history_search(SearchDirection::Reverse)?,
        Cmd::HistorySearchForward => s.edit_history_search(SearchDirection::Forward)?,
        Cmd::TransposeChars => {
            // Exchange the char before cursor with the character at cursor.
            s.edit_transpose_chars()?;
        }
        Cmd::Yank(n, anchor) => {
            // retrieve (yank) last item killed
            if let Some(text) = kill_ring.yank() {
                s.edit_yank(input_state, text, anchor, n)?;
            }
        }
        Cmd::ViYankTo(ref mvt) => {
            if let Some(text) = s.line.copy(mvt) {
                kill_ring.kill(&text, Mode::Append);
            }
        }
        Cmd::Newline => {
            s.edit_insert('\n', 1)?;
        }
        Cmd::Repaint => {
            s.refresh_line()?;
        }
        Cmd::AcceptLine | Cmd::AcceptOrInsertLine { .. } => {
            let validation_result = s.validate()?;
            let valid = validation_result.is_valid();
            let end = s.line.is_end_of_input();
            match (cmd, valid, end) {
                (Cmd::AcceptLine, ..)
                | (Cmd::AcceptOrInsertLine { .. }, true, true)
                | (
                    Cmd::AcceptOrInsertLine {
                        accept_in_the_middle: true,
                    },
                    true,
                    _,
                ) => {
                    return Ok(Submit);
                }
                (Cmd::AcceptOrInsertLine { .. }, false, _)
                | (Cmd::AcceptOrInsertLine { .. }, true, false) => {
                    if valid || !validation_result.has_message() {
                        s.edit_insert('\n', 1)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Cmd::BeginningOfHistory => {
            // move to first entry in history
            s.edit_history(true)?;
        }
        Cmd::EndOfHistory => {
            // move to last entry in history
            s.edit_history(false)?;
        }
        Cmd::Move(Movement::BackwardWord(n, word_def)) => {
            // move backwards one word
            s.edit_move_to_prev_word(word_def, n)?;
        }
        Cmd::CapitalizeWord => {
            // capitalize word after point
            s.edit_word(WordAction::Capitalize)?;
        }
        Cmd::Kill(ref mvt) => {
            s.edit_kill(mvt, kill_ring)?;
        }
        Cmd::Move(Movement::ForwardWord(n, at, word_def)) => {
            // move forwards one word
            s.edit_move_to_next_word(at, word_def, n)?;
        }
        Cmd::Move(Movement::LineUp(n)) => {
            s.edit_move_line_up(n)?;
        }
        Cmd::Move(Movement::LineDown(n)) => {
            s.edit_move_line_down(n)?;
        }
        Cmd::Move(Movement::BeginningOfBuffer) => {
            // Move to the start of the buffer.
            s.edit_move_buffer_start()?;
        }
        Cmd::Move(Movement::EndOfBuffer) => {
            // Move to the end of the buffer.
            s.edit_move_buffer_end()?;
        }
        Cmd::DowncaseWord => {
            // lowercase word after point
            s.edit_word(WordAction::Lowercase)?;
        }
        Cmd::TransposeWords(n) => {
            // transpose words
            s.edit_transpose_words(n)?;
        }
        Cmd::UpcaseWord => {
            // uppercase word after point
            s.edit_word(WordAction::Uppercase)?;
        }
        Cmd::YankPop => {
            // yank-pop
            if let Some((yank_size, text)) = kill_ring.yank_pop() {
                s.edit_yank_pop(yank_size, text)?;
            }
        }
        Cmd::Move(Movement::ViCharSearch(n, cs)) => s.edit_move_to(cs, n)?,
        Cmd::Undo(n) => {
            if s.changes.undo(&mut s.line, n) {
                s.refresh_line()?;
            }
        }
        Cmd::Dedent(mvt) => {
            s.edit_indent(&mvt, config.indent_size(), true)?;
        }
        Cmd::Indent(mvt) => {
            s.edit_indent(&mvt, config.indent_size(), false)?;
        }
        Cmd::Interrupt => {
            // Move to end, in case cursor was in the middle of the
            // line, so that next thing application prints goes after
            // the input
            s.move_cursor_to_end()?;
            return Err(error::ReadlineError::Interrupted);
        }
        _ => {
            // Ignore the character typed.
        }
    }
    Ok(Proceed)
}
