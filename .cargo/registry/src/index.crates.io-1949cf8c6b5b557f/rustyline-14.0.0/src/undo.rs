//! Undo API
use std::fmt::Debug;

use crate::keymap::RepeatCount;
use crate::line_buffer::{ChangeListener, DeleteListener, Direction, LineBuffer, NoListener};
use log::debug;
use unicode_segmentation::UnicodeSegmentation;

enum Change {
    Begin,
    End,
    Insert {
        idx: usize,
        text: String,
    }, // QuotedInsert, SelfInsert, Yank
    Delete {
        idx: usize,
        text: String,
    }, /* BackwardDeleteChar, BackwardKillWord, DeleteChar,
        * KillLine, KillWholeLine, KillWord,
        * UnixLikeDiscard, ViDeleteTo */
    Replace {
        idx: usize,
        old: String,
        new: String,
    }, /* CapitalizeWord, Complete, DowncaseWord, Replace, TransposeChars, TransposeWords,
        * UpcaseWord, YankPop */
}

impl Change {
    fn undo(&self, line: &mut LineBuffer) {
        match *self {
            Change::Begin | Change::End => {
                unreachable!();
            }
            Change::Insert { idx, ref text } => {
                line.delete_range(idx..idx + text.len(), &mut NoListener);
            }
            Change::Delete { idx, ref text } => {
                line.insert_str(idx, text, &mut NoListener);
                line.set_pos(idx + text.len());
            }
            Change::Replace {
                idx,
                ref old,
                ref new,
            } => {
                line.replace(idx..idx + new.len(), old, &mut NoListener);
            }
        }
    }

    #[cfg(test)]
    fn redo(&self, line: &mut LineBuffer) {
        match *self {
            Change::Begin | Change::End => {
                unreachable!();
            }
            Change::Insert { idx, ref text } => {
                line.insert_str(idx, text, &mut NoListener);
            }
            Change::Delete { idx, ref text } => {
                line.delete_range(idx..idx + text.len(), &mut NoListener);
            }
            Change::Replace {
                idx,
                ref old,
                ref new,
            } => {
                line.replace(idx..idx + old.len(), new, &mut NoListener);
            }
        }
    }

    fn insert_seq(&self, indx: usize) -> bool {
        if let Change::Insert { idx, ref text } = *self {
            idx + text.len() == indx
        } else {
            false
        }
    }

    fn delete_seq(&self, indx: usize, len: usize) -> bool {
        if let Change::Delete { idx, .. } = *self {
            // delete or backspace
            idx == indx || idx == indx + len
        } else {
            false
        }
    }

    fn replace_seq(&self, indx: usize) -> bool {
        if let Change::Replace { idx, ref new, .. } = *self {
            idx + new.len() == indx
        } else {
            false
        }
    }
}

/// Undo manager
pub struct Changeset {
    undo_group_level: u32,
    undos: Vec<Change>, // undoable changes
    redos: Vec<Change>, // undone changes, redoable
}

impl Changeset {
    pub(crate) fn new() -> Self {
        Self {
            undo_group_level: 0,
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    pub(crate) fn begin(&mut self) -> usize {
        debug!(target: "rustyline", "Changeset::begin");
        self.redos.clear();
        let mark = self.undos.len();
        self.undos.push(Change::Begin);
        self.undo_group_level += 1;
        mark
    }

    /// Returns `true` when changes happen between the last call to `begin` and
    /// this `end`.
    pub(crate) fn end(&mut self) -> bool {
        debug!(target: "rustyline", "Changeset::end");
        self.redos.clear();
        let mut touched = false;
        while self.undo_group_level > 0 {
            self.undo_group_level -= 1;
            if let Some(&Change::Begin) = self.undos.last() {
                // empty Begin..End
                self.undos.pop();
            } else {
                self.undos.push(Change::End);
                touched = true;
            }
        }
        touched
    }

    fn insert_char(idx: usize, c: char) -> Change {
        let mut text = String::new();
        text.push(c);
        Change::Insert { idx, text }
    }

    pub(crate) fn insert(&mut self, idx: usize, c: char) {
        debug!(target: "rustyline", "Changeset::insert({}, {:?})", idx, c);
        self.redos.clear();
        if !c.is_alphanumeric() || !self.undos.last().map_or(false, |lc| lc.insert_seq(idx)) {
            self.undos.push(Self::insert_char(idx, c));
            return;
        }
        // merge consecutive char insertions when char is alphanumeric
        let mut last_change = self.undos.pop().unwrap();
        if let Change::Insert { ref mut text, .. } = last_change {
            text.push(c);
        } else {
            unreachable!();
        }
        self.undos.push(last_change);
    }

    pub(crate) fn insert_str<S: AsRef<str> + Into<String> + Debug>(
        &mut self,
        idx: usize,
        string: S,
    ) {
        debug!(target: "rustyline", "Changeset::insert_str({}, {:?})", idx, string);
        self.redos.clear();
        if string.as_ref().is_empty() {
            return;
        }
        self.undos.push(Change::Insert {
            idx,
            text: string.into(),
        });
    }

    pub(crate) fn delete<S: AsRef<str> + Into<String> + Debug>(&mut self, indx: usize, string: S) {
        debug!(target: "rustyline", "Changeset::delete({}, {:?})", indx, string);
        self.redos.clear();
        if string.as_ref().is_empty() {
            return;
        }

        if !Self::single_char(string.as_ref())
            || !self
                .undos
                .last()
                .map_or(false, |lc| lc.delete_seq(indx, string.as_ref().len()))
        {
            self.undos.push(Change::Delete {
                idx: indx,
                text: string.into(),
            });
            return;
        }
        // merge consecutive char deletions when char is alphanumeric
        let mut last_change = self.undos.pop().unwrap();
        if let Change::Delete {
            ref mut idx,
            ref mut text,
        } = last_change
        {
            if *idx == indx {
                text.push_str(string.as_ref());
            } else {
                text.insert_str(0, string.as_ref());
                *idx = indx;
            }
        } else {
            unreachable!();
        }
        self.undos.push(last_change);
    }

    fn single_char(s: &str) -> bool {
        let mut graphemes = s.graphemes(true);
        graphemes.next().map_or(false, |grapheme| {
            grapheme.chars().all(char::is_alphanumeric)
        }) && graphemes.next().is_none()
    }

    pub(crate) fn replace<S: AsRef<str> + Into<String> + Debug>(
        &mut self,
        indx: usize,
        old_: S,
        new_: S,
    ) {
        debug!(target: "rustyline", "Changeset::replace({}, {:?}, {:?})", indx, old_, new_);
        self.redos.clear();

        if !self.undos.last().map_or(false, |lc| lc.replace_seq(indx)) {
            self.undos.push(Change::Replace {
                idx: indx,
                old: old_.into(),
                new: new_.into(),
            });
            return;
        }

        // merge consecutive char replacements
        let mut last_change = self.undos.pop().unwrap();
        if let Change::Replace {
            ref mut old,
            ref mut new,
            ..
        } = last_change
        {
            old.push_str(old_.as_ref());
            new.push_str(new_.as_ref());
        } else {
            unreachable!();
        }
        self.undos.push(last_change);
    }

    pub(crate) fn undo(&mut self, line: &mut LineBuffer, n: RepeatCount) -> bool {
        debug!(target: "rustyline", "Changeset::undo");
        let mut count = 0;
        let mut waiting_for_begin = 0;
        let mut undone = false;
        while let Some(change) = self.undos.pop() {
            match change {
                Change::Begin => {
                    waiting_for_begin -= 1;
                }
                Change::End => {
                    waiting_for_begin += 1;
                }
                _ => {
                    change.undo(line);
                    undone = true;
                }
            };
            self.redos.push(change);
            if waiting_for_begin <= 0 {
                count += 1;
                if count >= n {
                    break;
                }
            }
        }
        undone
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        debug!(target: "rustyline", "Changeset::truncate({})", len);
        self.undos.truncate(len);
    }

    #[cfg(test)]
    pub(crate) fn redo(&mut self, line: &mut LineBuffer) -> bool {
        let mut waiting_for_end = 0;
        let mut redone = false;
        while let Some(change) = self.redos.pop() {
            match change {
                Change::Begin => {
                    waiting_for_end += 1;
                }
                Change::End => {
                    waiting_for_end -= 1;
                }
                _ => {
                    change.redo(line);
                    redone = true;
                }
            };
            self.undos.push(change);
            if waiting_for_end <= 0 {
                break;
            }
        }
        redone
    }

    pub(crate) fn last_insert(&self) -> Option<String> {
        for change in self.undos.iter().rev() {
            match change {
                Change::Insert { ref text, .. } => return Some(text.clone()),
                Change::Replace { ref new, .. } => return Some(new.clone()),
                Change::End => {
                    continue;
                }
                _ => {
                    return None;
                }
            }
        }
        None
    }
}

impl DeleteListener for Changeset {
    fn delete(&mut self, idx: usize, string: &str, _: Direction) {
        self.delete(idx, string);
    }
}
impl ChangeListener for Changeset {
    fn insert_char(&mut self, idx: usize, c: char) {
        self.insert(idx, c);
    }

    fn insert_str(&mut self, idx: usize, string: &str) {
        self.insert_str(idx, string);
    }

    fn replace(&mut self, idx: usize, old: &str, new: &str) {
        self.replace(idx, old, new);
    }
}

#[cfg(test)]
mod tests {
    use super::Changeset;
    use crate::line_buffer::{LineBuffer, NoListener};

    #[test]
    fn test_insert_chars() {
        let mut cs = Changeset::new();
        cs.insert(0, 'H');
        cs.insert(1, 'i');
        assert_eq!(1, cs.undos.len());
        assert_eq!(0, cs.redos.len());
        cs.insert(0, ' ');
        assert_eq!(2, cs.undos.len());
    }

    #[test]
    fn test_insert_strings() {
        let mut cs = Changeset::new();
        cs.insert_str(0, "Hello");
        cs.insert_str(5, ", ");
        assert_eq!(2, cs.undos.len());
        assert_eq!(0, cs.redos.len());
    }

    #[test]
    fn test_undo_insert() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hello", &mut NoListener);
        buf.insert_str(5, ", world!", &mut NoListener);
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.insert_str(5, ", world!");

        cs.undo(&mut buf, 1);
        assert_eq!(0, cs.undos.len());
        assert_eq!(1, cs.redos.len());
        assert_eq!(buf.as_str(), "Hello");

        cs.redo(&mut buf);
        assert_eq!(1, cs.undos.len());
        assert_eq!(0, cs.redos.len());
        assert_eq!(buf.as_str(), "Hello, world!");
    }

    #[test]
    fn test_undo_delete() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hello", &mut NoListener);
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello");

        cs.delete(5, ", world!");

        cs.undo(&mut buf, 1);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_delete_chars() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hlo", &mut NoListener);

        let mut cs = Changeset::new();
        cs.delete(1, "e");
        cs.delete(1, "l");
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf, 1);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_backspace_chars() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hlo", &mut NoListener);

        let mut cs = Changeset::new();
        cs.delete(2, "l");
        cs.delete(1, "e");
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf, 1);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_undo_replace() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hello, world!", &mut NoListener);
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello, world!");

        buf.replace(1..5, "i", &mut NoListener);
        assert_eq!(buf.as_str(), "Hi, world!");
        cs.replace(1, "ello", "i");

        cs.undo(&mut buf, 1);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hi, world!");
    }

    #[test]
    fn test_last_insert() {
        let mut cs = Changeset::new();
        cs.begin();
        cs.delete(0, "Hello");
        cs.insert_str(0, "Bye");
        cs.end();
        let insert = cs.last_insert();
        assert_eq!(Some("Bye".to_owned()), insert);
    }

    #[test]
    fn test_end() {
        let mut cs = Changeset::new();
        cs.begin();
        assert!(!cs.end());
        cs.begin();
        cs.insert_str(0, "Hi");
        assert!(cs.end());
    }
}
