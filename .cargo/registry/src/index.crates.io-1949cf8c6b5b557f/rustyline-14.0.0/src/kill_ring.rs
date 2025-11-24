//! Kill Ring management
use crate::line_buffer::{DeleteListener, Direction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Kill,
    Yank(usize),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Append,
    Prepend,
}

pub struct KillRing {
    slots: Vec<String>,
    // where we are in the kill ring
    index: usize,
    // whether or not the last command was a kill or a yank
    last_action: Action,
    killing: bool,
}

impl KillRing {
    /// Create a new kill-ring of the given `size`.
    pub fn new(size: usize) -> Self {
        Self {
            slots: Vec::with_capacity(size),
            index: 0,
            last_action: Action::Other,
            killing: false,
        }
    }

    /// Reset `last_action` state.
    pub fn reset(&mut self) {
        self.last_action = Action::Other;
    }

    /// Add `text` to the kill-ring.
    pub fn kill(&mut self, text: &str, dir: Mode) {
        if let Action::Kill = self.last_action {
            if self.slots.capacity() == 0 {
                // disabled
                return;
            }
            match dir {
                Mode::Append => self.slots[self.index].push_str(text),
                Mode::Prepend => self.slots[self.index].insert_str(0, text),
            };
        } else {
            self.last_action = Action::Kill;
            if self.slots.capacity() == 0 {
                // disabled
                return;
            }
            if self.index == self.slots.capacity() - 1 {
                // full
                self.index = 0;
            } else if !self.slots.is_empty() {
                self.index += 1;
            }
            if self.index == self.slots.len() {
                self.slots.push(String::from(text));
            } else {
                self.slots[self.index] = String::from(text);
            }
        }
    }

    /// Yank previously killed text.
    /// Return `None` when kill-ring is empty.
    pub fn yank(&mut self) -> Option<&String> {
        if self.slots.is_empty() {
            None
        } else {
            self.last_action = Action::Yank(self.slots[self.index].len());
            Some(&self.slots[self.index])
        }
    }

    /// Yank killed text stored in previous slot.
    /// Return `None` when the previous command was not a yank.
    pub fn yank_pop(&mut self) -> Option<(usize, &String)> {
        match self.last_action {
            Action::Yank(yank_size) => {
                if self.slots.is_empty() {
                    return None;
                }
                if self.index == 0 {
                    self.index = self.slots.len() - 1;
                } else {
                    self.index -= 1;
                }
                self.last_action = Action::Yank(self.slots[self.index].len());
                Some((yank_size, &self.slots[self.index]))
            }
            _ => None,
        }
    }
}

impl DeleteListener for KillRing {
    fn start_killing(&mut self) {
        self.killing = true;
    }

    fn delete(&mut self, _: usize, string: &str, dir: Direction) {
        if !self.killing {
            return;
        }
        let mode = match dir {
            Direction::Forward => Mode::Append,
            Direction::Backward => Mode::Prepend,
        };
        self.kill(string, mode);
    }

    fn stop_killing(&mut self) {
        self.killing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, KillRing, Mode};

    #[test]
    fn disabled() {
        let mut kill_ring = KillRing::new(0);
        kill_ring.kill("text", Mode::Append);
        assert!(kill_ring.slots.is_empty());
        assert_eq!(0, kill_ring.index);
        assert_eq!(Action::Kill, kill_ring.last_action);

        assert_eq!(None, kill_ring.yank());
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn one_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word1", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_append() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        kill_ring.kill(" word2", Mode::Append);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word1 word2", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_backward() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Prepend);
        kill_ring.kill("word2 ", Mode::Prepend);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word2 word1", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_other_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("word2", Mode::Append);
        assert_eq!(1, kill_ring.index);
        assert_eq!(2, kill_ring.slots.len());
        assert_eq!("word1", kill_ring.slots[0]);
        assert_eq!("word2", kill_ring.slots[1]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn many_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("word2", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("word3", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("word4", Mode::Append);
        assert_eq!(1, kill_ring.index);
        assert_eq!(2, kill_ring.slots.len());
        assert_eq!("word3", kill_ring.slots[0]);
        assert_eq!("word4", kill_ring.slots[1]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn yank() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("word2", Mode::Append);

        assert_eq!(Some(&"word2".to_owned()), kill_ring.yank());
        assert_eq!(Action::Yank(5), kill_ring.last_action);
        assert_eq!(Some(&"word2".to_owned()), kill_ring.yank());
        assert_eq!(Action::Yank(5), kill_ring.last_action);
    }

    #[test]
    fn yank_pop() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", Mode::Append);
        kill_ring.reset();
        kill_ring.kill("longword2", Mode::Append);

        assert_eq!(None, kill_ring.yank_pop());
        kill_ring.yank();
        assert_eq!(Some((9, &"word1".to_owned())), kill_ring.yank_pop());
        assert_eq!(Some((5, &"longword2".to_owned())), kill_ring.yank_pop());
        assert_eq!(Some((9, &"word1".to_owned())), kill_ring.yank_pop());
    }
}
