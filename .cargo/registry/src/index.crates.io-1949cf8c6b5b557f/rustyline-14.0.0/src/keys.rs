//! Key constants

/// Input key pressed and modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent(pub KeyCode, pub Modifiers);

impl KeyEvent {
    /// Constant value representing an unmodified press of `KeyCode::Backspace`.
    pub(crate) const BACKSPACE: Self = Self(KeyCode::Backspace, Modifiers::NONE);
    /// Constant value representing an unmodified press of `KeyCode::Enter`.
    pub(crate) const ENTER: Self = Self(KeyCode::Enter, Modifiers::NONE);
    /// Constant value representing an unmodified press of `KeyCode::Esc`.
    pub(crate) const ESC: Self = Self(KeyCode::Esc, Modifiers::NONE);

    /// Constructor from `char` and modifiers
    #[must_use]
    pub fn new(c: char, mut mods: Modifiers) -> Self {
        use {KeyCode as K, KeyEvent as E, Modifiers as M};

        if !c.is_control() {
            if !mods.is_empty() {
                mods.remove(M::SHIFT); // TODO Validate: no SHIFT even if
                                       // `c` is uppercase
            }
            return E(K::Char(c), mods);
        }
        #[allow(clippy::match_same_arms)]
        match c {
            '\x00' => E(K::Char('@'), mods | M::CTRL), // '\0'
            '\x01' => E(K::Char('A'), mods | M::CTRL),
            '\x02' => E(K::Char('B'), mods | M::CTRL),
            '\x03' => E(K::Char('C'), mods | M::CTRL),
            '\x04' => E(K::Char('D'), mods | M::CTRL),
            '\x05' => E(K::Char('E'), mods | M::CTRL),
            '\x06' => E(K::Char('F'), mods | M::CTRL),
            '\x07' => E(K::Char('G'), mods | M::CTRL), // '\a'
            #[cfg(unix)]
            '\x08' => E(K::Backspace, mods), // '\b'
            #[cfg(windows)]
            '\x08' => E(K::Char('H'), mods | M::CTRL),
            #[cfg(unix)]
            '\x09' => {
                // '\t'
                if mods.contains(M::SHIFT) {
                    mods.remove(M::SHIFT);
                    E(K::BackTab, mods)
                } else {
                    E(K::Tab, mods)
                }
            }
            #[cfg(windows)]
            '\x09' => E(K::Char('I'), mods | M::CTRL),
            '\x0a' => E(K::Char('J'), mods | M::CTRL), // '\n' (10)
            '\x0b' => E(K::Char('K'), mods | M::CTRL),
            '\x0c' => E(K::Char('L'), mods | M::CTRL),
            #[cfg(unix)]
            '\x0d' => E(K::Enter, mods), // '\r' (13)
            #[cfg(windows)]
            '\x0d' => E(K::Char('M'), mods | M::CTRL),
            '\x0e' => E(K::Char('N'), mods | M::CTRL),
            '\x0f' => E(K::Char('O'), mods | M::CTRL),
            '\x10' => E(K::Char('P'), mods | M::CTRL),
            '\x11' => E(K::Char('Q'), mods | M::CTRL),
            '\x12' => E(K::Char('R'), mods | M::CTRL),
            '\x13' => E(K::Char('S'), mods | M::CTRL),
            '\x14' => E(K::Char('T'), mods | M::CTRL),
            '\x15' => E(K::Char('U'), mods | M::CTRL),
            '\x16' => E(K::Char('V'), mods | M::CTRL),
            '\x17' => E(K::Char('W'), mods | M::CTRL),
            '\x18' => E(K::Char('X'), mods | M::CTRL),
            '\x19' => E(K::Char('Y'), mods | M::CTRL),
            '\x1a' => E(K::Char('Z'), mods | M::CTRL),
            '\x1b' => E(K::Esc, mods), // Ctrl-[, '\e'
            '\x1c' => E(K::Char('\\'), mods | M::CTRL),
            '\x1d' => E(K::Char(']'), mods | M::CTRL),
            '\x1e' => E(K::Char('^'), mods | M::CTRL),
            '\x1f' => E(K::Char('_'), mods | M::CTRL),
            '\x7f' => E(K::Backspace, mods), // Rubout, Ctrl-?
            '\u{9b}' => E(K::Esc, mods | M::SHIFT),
            _ => E(K::Null, mods),
        }
    }

    /// Constructor from `char` with Ctrl modifier
    #[must_use]
    pub fn ctrl(c: char) -> Self {
        Self::new(c, Modifiers::CTRL)
    }

    /// Constructor from `char` with Alt modifier
    #[must_use]
    pub fn alt(c: char) -> Self {
        Self::new(c, Modifiers::ALT)
    }

    /// ctrl-a => ctrl-A (uppercase)
    /// shift-A => A (no SHIFT modifier)
    /// shift-Tab => `BackTab`
    #[must_use]
    pub fn normalize(e: Self) -> Self {
        use {KeyCode as K, KeyEvent as E, Modifiers as M};

        match e {
            E(K::Char(c), m) if c.is_ascii_control() => Self::new(c, m),
            E(K::Char(c), m) if c.is_ascii_lowercase() && m.contains(M::CTRL) => {
                E(K::Char(c.to_ascii_uppercase()), m)
            }
            E(K::Char(c), m) if c.is_ascii_uppercase() && m.contains(M::SHIFT) => {
                E(K::Char(c), m ^ M::SHIFT)
            }
            E(K::Tab, m) if m.contains(M::SHIFT) => E(K::BackTab, m ^ M::SHIFT),
            _ => e,
        }
    }
}

impl From<char> for KeyEvent {
    fn from(c: char) -> Self {
        Self::new(c, Modifiers::NONE)
    }
}

/// Input key pressed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// Unsupported escape sequence (on unix platform)
    UnknownEscSeq,
    /// ⌫ or Ctrl-H
    Backspace,
    /// ⇤ (usually Shift-Tab)
    BackTab,
    /// Paste (on unix platform)
    BracketedPasteStart,
    /// Paste (on unix platform)
    BracketedPasteEnd,
    /// Single char
    Char(char),
    /// ⌦
    Delete,
    /// ↓ arrow key
    Down,
    /// ⇲
    End,
    /// ↵ or Ctrl-M
    Enter,
    /// Escape or Ctrl-[
    Esc,
    /// Function key
    F(u8),
    /// ⇱
    Home,
    /// Insert key
    Insert,
    /// ← arrow key
    Left,
    /// \0
    Null,
    /// ⇟
    PageDown,
    /// ⇞
    PageUp,
    /// → arrow key
    Right,
    /// ⇥ or Ctrl-I
    Tab,
    /// ↑ arrow key
    Up,
}

bitflags::bitflags! {
    /// The set of modifier keys that were triggered along with a key press.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct Modifiers: u8 {
        /// Control modifier
        const CTRL  = 1<<3;
        /// Escape or Alt modifier
        const ALT  = 1<<2;
        /// Shift modifier
        const SHIFT = 1<<1;

        /// No modifier
        const NONE = 0;
        /// Ctrl + Shift
        const CTRL_SHIFT = Self::CTRL.bits() | Self::SHIFT.bits();
        /// Alt + Shift
        const ALT_SHIFT = Self::ALT.bits() | Self::SHIFT.bits();
        /// Ctrl + Alt
        const CTRL_ALT = Self::CTRL.bits() | Self::ALT.bits();
        /// Ctrl + Alt + Shift
        const CTRL_ALT_SHIFT = Self::CTRL.bits() | Self::ALT.bits() | Self::SHIFT.bits();
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyCode as K, KeyEvent as E, Modifiers as M};

    #[test]
    fn new() {
        assert_eq!(E::ESC, E::new('\x1b', M::NONE));
    }

    #[test]
    #[cfg(unix)]
    fn from() {
        assert_eq!(E(K::Tab, M::NONE), E::from('\t'));
    }

    #[test]
    #[cfg(windows)]
    fn from() {
        assert_eq!(E(K::Char('I'), M::CTRL), E::from('\t'));
    }

    #[test]
    fn normalize() {
        assert_eq!(E::ctrl('A'), E::normalize(E(K::Char('\x01'), M::NONE)));
        assert_eq!(E::ctrl('A'), E::normalize(E::ctrl('a')));
        assert_eq!(E::from('A'), E::normalize(E(K::Char('A'), M::SHIFT)));
        assert_eq!(E(K::BackTab, M::NONE), E::normalize(E(K::Tab, M::SHIFT)));
    }
}
