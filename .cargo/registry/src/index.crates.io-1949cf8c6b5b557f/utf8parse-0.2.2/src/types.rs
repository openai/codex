//! Types supporting the UTF-8 parser

/// Action to take when receiving a byte
#[derive(Debug, Copy, Clone)]
pub enum Action {
    /// Unexpected byte; sequence is invalid
    InvalidSequence = 0,
    /// Received valid 7-bit ASCII byte which can be directly emitted.
    EmitByte = 1,
    /// Set the bottom continuation byte
    SetByte1 = 2,
    /// Set the 2nd-from-last continuation byte
    SetByte2 = 3,
    /// Set the 2nd-from-last byte which is part of a two byte sequence
    SetByte2Top = 4,
    /// Set the 3rd-from-last continuation byte
    SetByte3 = 5,
    /// Set the 3rd-from-last byte which is part of a three byte sequence
    SetByte3Top = 6,
    /// Set the top byte of a four byte sequence.
    SetByte4 = 7,
}

/// States the parser can be in.
///
/// There is a state for each initial input of the 3 and 4 byte sequences since
/// the following bytes are subject to different conditions than a tail byte.
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum State {
    /// Ground state; expect anything
    #[default]
    Ground = 0,
    /// 3 tail bytes
    Tail3 = 1,
    /// 2 tail bytes
    Tail2 = 2,
    /// 1 tail byte
    Tail1 = 3,
    /// UTF8-3 starting with E0
    U3_2_e0 = 4,
    /// UTF8-3 starting with ED
    U3_2_ed = 5,
    /// UTF8-4 starting with F0
    Utf8_4_3_f0 = 6,
    /// UTF8-4 starting with F4
    Utf8_4_3_f4 = 7,
}

impl State {
    /// Advance the parser state.
    ///
    /// This takes the current state and input byte into consideration, to determine the next state
    /// and any action that should be taken.
    #[inline]
    pub fn advance(self, byte: u8) -> (State, Action) {
        match self {
            State::Ground => match byte {
                0x00..=0x7f => (State::Ground, Action::EmitByte),
                0xc2..=0xdf => (State::Tail1, Action::SetByte2Top),
                0xe0 => (State::U3_2_e0, Action::SetByte3Top),
                0xe1..=0xec => (State::Tail2, Action::SetByte3Top),
                0xed => (State::U3_2_ed, Action::SetByte3Top),
                0xee..=0xef => (State::Tail2, Action::SetByte3Top),
                0xf0 => (State::Utf8_4_3_f0, Action::SetByte4),
                0xf1..=0xf3 => (State::Tail3, Action::SetByte4),
                0xf4 => (State::Utf8_4_3_f4, Action::SetByte4),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::U3_2_e0 => match byte {
                0xa0..=0xbf => (State::Tail1, Action::SetByte2),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::U3_2_ed => match byte {
                0x80..=0x9f => (State::Tail1, Action::SetByte2),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::Utf8_4_3_f0 => match byte {
                0x90..=0xbf => (State::Tail2, Action::SetByte3),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::Utf8_4_3_f4 => match byte {
                0x80..=0x8f => (State::Tail2, Action::SetByte3),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::Tail3 => match byte {
                0x80..=0xbf => (State::Tail2, Action::SetByte3),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::Tail2 => match byte {
                0x80..=0xbf => (State::Tail1, Action::SetByte2),
                _ => (State::Ground, Action::InvalidSequence),
            },
            State::Tail1 => match byte {
                0x80..=0xbf => (State::Ground, Action::SetByte1),
                _ => (State::Ground, Action::InvalidSequence),
            },
        }
    }
}
