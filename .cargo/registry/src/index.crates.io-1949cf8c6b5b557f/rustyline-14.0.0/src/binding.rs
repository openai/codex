/// Custom event handlers
use crate::{
    Cmd, EditMode, InputMode, InputState, KeyCode, KeyEvent, Modifiers, Refresher, RepeatCount,
};

use radix_trie::TrieKey;

/// Input event
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
pub enum Event {
    /// Wildcard.
    /// Useful if you want to filter out some keys.
    Any,
    /// Key sequence
    KeySeq(Vec<KeyEvent>),
    /// TODO Mouse event
    Mouse(),
}

impl Event {
    /// See [`KeyEvent::normalize`]
    pub(crate) fn normalize(mut self) -> Self {
        if let Event::KeySeq(ref mut keys) = self {
            for key in keys.iter_mut() {
                *key = KeyEvent::normalize(*key);
            }
        }
        self
    }

    /// Return `i`th key event
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&KeyEvent> {
        if let Event::KeySeq(ref ks) = self {
            ks.get(i)
        } else {
            None
        }
    }
}

impl From<KeyEvent> for Event {
    fn from(k: KeyEvent) -> Event {
        Event::KeySeq(vec![k])
    }
}

const BASE: u32 = 0x0010_ffff + 1;
const BASE_CONTROL: u32 = 0x0200_0000;
const BASE_META: u32 = 0x0400_0000;
const BASE_SHIFT: u32 = 0x0100_0000;
const ESCAPE: u32 = 27;
const PAGE_UP: u32 = BASE + 1;
const PAGE_DOWN: u32 = PAGE_UP + 1;
const DOWN: u32 = PAGE_DOWN + 1;
const UP: u32 = DOWN + 1;
const LEFT: u32 = UP + 1;
const RIGHT: u32 = LEFT + 1;
const HOME: u32 = RIGHT + 1;
const END: u32 = HOME + 1;
const DELETE: u32 = END + 1;
const INSERT: u32 = DELETE + 1;
//const F1: u32 = INSERT + 1;
const MOUSE: u32 = /* F24 + 1 */ INSERT + 25;
const PASTE_START: u32 = MOUSE + 1;
const PASTE_FINISH: u32 = PASTE_START + 1;
const ANY: u32 = PASTE_FINISH + 1;

impl KeyEvent {
    fn encode(&self) -> u32 {
        let mut u = match self.0 {
            KeyCode::UnknownEscSeq | KeyCode::Null => 0,
            KeyCode::Backspace => u32::from('\x7f'),
            KeyCode::BackTab => u32::from('\t') | BASE_SHIFT,
            KeyCode::BracketedPasteStart => PASTE_START,
            KeyCode::BracketedPasteEnd => PASTE_FINISH,
            KeyCode::Char(c) => u32::from(c),
            KeyCode::Delete => DELETE,
            KeyCode::Down => DOWN,
            KeyCode::End => END,
            KeyCode::Enter => u32::from('\r'),
            KeyCode::F(i) => INSERT + u32::from(i),
            KeyCode::Esc => ESCAPE,
            KeyCode::Home => HOME,
            KeyCode::Insert => INSERT,
            KeyCode::Left => LEFT,
            KeyCode::PageDown => PAGE_DOWN,
            KeyCode::PageUp => PAGE_UP,
            KeyCode::Right => RIGHT,
            KeyCode::Tab => u32::from('\t'),
            KeyCode::Up => UP,
        };
        if self.1.contains(Modifiers::CTRL) {
            u |= BASE_CONTROL;
        }
        if self.1.contains(Modifiers::ALT) {
            u |= BASE_META;
        }
        if self.1.contains(Modifiers::SHIFT) {
            u |= BASE_SHIFT;
        }
        u
    }
}

impl TrieKey for Event {
    fn encode_bytes(&self) -> Vec<u8> {
        match self {
            Event::Any => ANY.to_be_bytes().to_vec(),
            Event::KeySeq(keys) => {
                let mut dst = Vec::with_capacity(keys.len() * 4);
                for key in keys {
                    dst.extend_from_slice(&key.encode().to_be_bytes());
                }
                dst
            }
            Event::Mouse() => MOUSE.to_be_bytes().to_vec(),
        }
    }
}

/// Event handler
#[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
pub enum EventHandler {
    /// unconditional command
    Simple(Cmd),
    /// handler behaviour depends on input state
    Conditional(Box<dyn ConditionalEventHandler>),
    /* invoke multiple actions
     * TODO Macro(), */
}

impl From<Cmd> for EventHandler {
    fn from(c: Cmd) -> EventHandler {
        EventHandler::Simple(c)
    }
}

/// Give access to user input.
#[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
pub struct EventContext<'r> {
    mode: EditMode,
    input_mode: InputMode,
    wrt: &'r dyn Refresher,
}

impl<'r> EventContext<'r> {
    pub(crate) fn new(is: &InputState, wrt: &'r dyn Refresher) -> Self {
        EventContext {
            mode: is.mode,
            input_mode: is.input_mode,
            wrt,
        }
    }

    /// emacs or vi mode
    #[must_use]
    pub fn mode(&self) -> EditMode {
        self.mode
    }

    /// vi input mode
    #[must_use]
    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    /// Returns `true` if there is a hint displayed.
    #[must_use]
    pub fn has_hint(&self) -> bool {
        self.wrt.has_hint()
    }

    /// Returns the hint text that is shown after the current cursor position.
    #[must_use]
    pub fn hint_text(&self) -> Option<&str> {
        self.wrt.hint_text()
    }

    /// currently edited line
    #[must_use]
    pub fn line(&self) -> &str {
        self.wrt.line()
    }

    /// Current cursor position (byte position)
    #[must_use]
    pub fn pos(&self) -> usize {
        self.wrt.pos()
    }
}

/// May behave differently depending on:
///  * edit mode (emacs vs vi)
///  * vi input mode (insert vs replace vs command modes)
///  * empty line
///  * cursor position
///  * repeat count
///  * original key pressed (when same command is bound to different key)
///  * hint
///  * ...
#[cfg_attr(docsrs, doc(cfg(feature = "custom-bindings")))]
pub trait ConditionalEventHandler: Send + Sync {
    /// Takes the current input state and
    /// returns the command to be performed or `None` to perform the default
    /// one.
    fn handle(
        &self,
        evt: &Event,
        n: RepeatCount,
        positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd>;
}

#[cfg(test)]
mod test {
    use super::{Event, EventHandler};
    use crate::{Cmd, KeyCode, KeyEvent, Modifiers};
    use radix_trie::Trie;

    #[test]
    fn encode() {
        let mut trie = Trie::new();
        let evt = Event::KeySeq(vec![KeyEvent::ctrl('X'), KeyEvent::ctrl('E')]);
        trie.insert(evt.clone(), EventHandler::from(Cmd::Noop));
        let prefix = Event::from(KeyEvent::ctrl('X'));
        let subtrie = trie.get_raw_descendant(&prefix);
        assert!(subtrie.is_some());
        let subtrie = subtrie.unwrap();
        let sub_result = subtrie.get(&evt);
        assert!(sub_result.unwrap().is_some());
        let prefix = Event::from(KeyEvent::ctrl('O'));
        let subtrie = trie.get_raw_descendant(&prefix);
        assert!(subtrie.is_none())
    }

    #[test]
    fn no_collision() {
        use {Event as E, EventHandler as H, KeyCode as C, KeyEvent as K, Modifiers as M};
        let mut trie = Trie::new();
        trie.insert(E::from(K(C::Backspace, M::NONE)), H::from(Cmd::Noop));
        trie.insert(E::from(K(C::Enter, M::NONE)), H::from(Cmd::Noop));
        trie.insert(E::from(K(C::Tab, M::NONE)), H::from(Cmd::Noop));
        trie.insert(E::from(K(C::Backspace, M::CTRL)), H::from(Cmd::Noop));
        trie.insert(E::from(K(C::Enter, M::CTRL)), H::from(Cmd::Noop));
        trie.insert(E::from(K(C::Tab, M::CTRL)), H::from(Cmd::Noop));
    }

    #[test]
    #[ignore]
    #[cfg(target_arch = "x86_64")]
    fn size_of_event() {
        use core::mem::size_of;
        assert_eq!(size_of::<Event>(), 32);
    }
}
