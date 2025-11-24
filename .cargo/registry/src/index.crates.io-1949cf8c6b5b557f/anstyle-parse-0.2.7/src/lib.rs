//! Parser for implementing virtual terminal emulators
//!
//! [`Parser`] is implemented according to [Paul Williams' ANSI parser
//! state machine]. The state machine doesn't assign meaning to the parsed data
//! and is thus not itself sufficient for writing a terminal emulator. Instead,
//! it is expected that an implementation of [`Perform`] is provided which does
//! something useful with the parsed data. The [`Parser`] handles the book
//! keeping, and the [`Perform`] gets to simply handle actions.
//!
//! # Examples
//!
//! For an example of using the [`Parser`] please see the examples folder. The example included
//! there simply logs all the actions [`Perform`] does. One quick thing to see it in action is to
//! pipe `vim` into it
//!
//! ```sh
//! cargo build --release --example parselog
//! vim | target/release/examples/parselog
//! ```
//!
//! Just type `:q` to exit.
//!
//! # Differences from original state machine description
//!
//! * UTF-8 Support for Input
//! * OSC Strings can be terminated by 0x07
//! * Only supports 7-bit codes. Some 8-bit codes are still supported, but they no longer work in
//!   all states.
//!
//! [Paul Williams' ANSI parser state machine]: https://vt100.net/emu/dec_ansi_parser
#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![allow(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(not(feature = "core"))]
extern crate alloc;

use core::mem::MaybeUninit;

#[cfg(feature = "core")]
use arrayvec::ArrayVec;
#[cfg(feature = "utf8")]
use utf8parse as utf8;

mod params;
pub mod state;

pub use params::{Params, ParamsIter};

use state::{state_change, Action, State};

const MAX_INTERMEDIATES: usize = 2;
const MAX_OSC_PARAMS: usize = 16;
#[cfg(feature = "core")]
const MAX_OSC_RAW: usize = 1024;

/// Parser for raw _VTE_ protocol which delegates actions to a [`Perform`]
#[allow(unused_qualifications)]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Parser<C = DefaultCharAccumulator> {
    state: State,
    intermediates: [u8; MAX_INTERMEDIATES],
    intermediate_idx: usize,
    params: Params,
    param: u16,
    #[cfg(feature = "core")]
    osc_raw: ArrayVec<u8, MAX_OSC_RAW>,
    #[cfg(not(feature = "core"))]
    osc_raw: alloc::vec::Vec<u8>,
    osc_params: [(usize, usize); MAX_OSC_PARAMS],
    osc_num_params: usize,
    ignoring: bool,
    utf8_parser: C,
}

impl<C> Parser<C>
where
    C: CharAccumulator,
{
    /// Create a new Parser
    pub fn new() -> Parser {
        Parser::default()
    }

    #[inline]
    fn params(&self) -> &Params {
        &self.params
    }

    #[inline]
    fn intermediates(&self) -> &[u8] {
        &self.intermediates[..self.intermediate_idx]
    }

    /// Advance the parser state
    ///
    /// Requires a [`Perform`] in case `byte` triggers an action
    #[inline]
    pub fn advance<P: Perform>(&mut self, performer: &mut P, byte: u8) {
        // Utf8 characters are handled out-of-band.
        if let State::Utf8 = self.state {
            self.process_utf8(performer, byte);
            return;
        }

        let (state, action) = state_change(self.state, byte);
        self.perform_state_change(performer, state, action, byte);
    }

    #[inline]
    fn process_utf8<P>(&mut self, performer: &mut P, byte: u8)
    where
        P: Perform,
    {
        if let Some(c) = self.utf8_parser.add(byte) {
            performer.print(c);
            self.state = State::Ground;
        }
    }

    #[inline]
    fn perform_state_change<P>(&mut self, performer: &mut P, state: State, action: Action, byte: u8)
    where
        P: Perform,
    {
        match state {
            State::Anywhere => {
                // Just run the action
                self.perform_action(performer, action, byte);
            }
            state => {
                match self.state {
                    State::DcsPassthrough => {
                        self.perform_action(performer, Action::Unhook, byte);
                    }
                    State::OscString => {
                        self.perform_action(performer, Action::OscEnd, byte);
                    }
                    _ => (),
                }

                match action {
                    Action::Nop => (),
                    action => {
                        self.perform_action(performer, action, byte);
                    }
                }

                match state {
                    State::CsiEntry | State::DcsEntry | State::Escape => {
                        self.perform_action(performer, Action::Clear, byte);
                    }
                    State::DcsPassthrough => {
                        self.perform_action(performer, Action::Hook, byte);
                    }
                    State::OscString => {
                        self.perform_action(performer, Action::OscStart, byte);
                    }
                    _ => (),
                }

                // Assume the new state
                self.state = state;
            }
        }
    }

    /// Separate method for `osc_dispatch` that borrows self as read-only
    ///
    /// The aliasing is needed here for multiple slices into `self.osc_raw`
    #[inline]
    fn osc_dispatch<P: Perform>(&self, performer: &mut P, byte: u8) {
        let mut slices: [MaybeUninit<&[u8]>; MAX_OSC_PARAMS] =
            unsafe { MaybeUninit::uninit().assume_init() };

        for (i, slice) in slices.iter_mut().enumerate().take(self.osc_num_params) {
            let indices = self.osc_params[i];
            *slice = MaybeUninit::new(&self.osc_raw[indices.0..indices.1]);
        }

        unsafe {
            let num_params = self.osc_num_params;
            let params = &slices[..num_params] as *const [MaybeUninit<&[u8]>] as *const [&[u8]];
            performer.osc_dispatch(&*params, byte == 0x07);
        }
    }

    #[inline]
    fn perform_action<P: Perform>(&mut self, performer: &mut P, action: Action, byte: u8) {
        match action {
            Action::Print => performer.print(byte as char),
            Action::Execute => performer.execute(byte),
            Action::Hook => {
                if self.params.is_full() {
                    self.ignoring = true;
                } else {
                    self.params.push(self.param);
                }

                performer.hook(self.params(), self.intermediates(), self.ignoring, byte);
            }
            Action::Put => performer.put(byte),
            Action::OscStart => {
                self.osc_raw.clear();
                self.osc_num_params = 0;
            }
            Action::OscPut => {
                #[cfg(feature = "core")]
                {
                    if self.osc_raw.is_full() {
                        return;
                    }
                }

                let idx = self.osc_raw.len();

                // Param separator
                if byte == b';' {
                    let param_idx = self.osc_num_params;
                    match param_idx {
                        // Only process up to MAX_OSC_PARAMS
                        MAX_OSC_PARAMS => return,

                        // First param is special - 0 to current byte index
                        0 => {
                            self.osc_params[param_idx] = (0, idx);
                        }

                        // All other params depend on previous indexing
                        _ => {
                            let prev = self.osc_params[param_idx - 1];
                            let begin = prev.1;
                            self.osc_params[param_idx] = (begin, idx);
                        }
                    }

                    self.osc_num_params += 1;
                } else {
                    self.osc_raw.push(byte);
                }
            }
            Action::OscEnd => {
                let param_idx = self.osc_num_params;
                let idx = self.osc_raw.len();

                match param_idx {
                    // Finish last parameter if not already maxed
                    MAX_OSC_PARAMS => (),

                    // First param is special - 0 to current byte index
                    0 => {
                        self.osc_params[param_idx] = (0, idx);
                        self.osc_num_params += 1;
                    }

                    // All other params depend on previous indexing
                    _ => {
                        let prev = self.osc_params[param_idx - 1];
                        let begin = prev.1;
                        self.osc_params[param_idx] = (begin, idx);
                        self.osc_num_params += 1;
                    }
                }
                self.osc_dispatch(performer, byte);
            }
            Action::Unhook => performer.unhook(),
            Action::CsiDispatch => {
                if self.params.is_full() {
                    self.ignoring = true;
                } else {
                    self.params.push(self.param);
                }

                performer.csi_dispatch(self.params(), self.intermediates(), self.ignoring, byte);
            }
            Action::EscDispatch => {
                performer.esc_dispatch(self.intermediates(), self.ignoring, byte);
            }
            Action::Collect => {
                if self.intermediate_idx == MAX_INTERMEDIATES {
                    self.ignoring = true;
                } else {
                    self.intermediates[self.intermediate_idx] = byte;
                    self.intermediate_idx += 1;
                }
            }
            Action::Param => {
                if self.params.is_full() {
                    self.ignoring = true;
                    return;
                }

                if byte == b';' {
                    self.params.push(self.param);
                    self.param = 0;
                } else if byte == b':' {
                    self.params.extend(self.param);
                    self.param = 0;
                } else {
                    // Continue collecting bytes into param
                    self.param = self.param.saturating_mul(10);
                    self.param = self.param.saturating_add((byte - b'0') as u16);
                }
            }
            Action::Clear => {
                // Reset everything on ESC/CSI/DCS entry
                self.intermediate_idx = 0;
                self.ignoring = false;
                self.param = 0;

                self.params.clear();
            }
            Action::BeginUtf8 => self.process_utf8(performer, byte),
            Action::Ignore => (),
            Action::Nop => (),
        }
    }
}

/// Build a `char` out of bytes
pub trait CharAccumulator: Default {
    /// Build a `char` out of bytes
    ///
    /// Return `None` when more data is needed
    fn add(&mut self, byte: u8) -> Option<char>;
}

/// Most flexible [`CharAccumulator`] for [`Parser`] based on active features
#[cfg(feature = "utf8")]
pub type DefaultCharAccumulator = Utf8Parser;
#[cfg(not(feature = "utf8"))]
pub type DefaultCharAccumulator = AsciiParser;

/// Only allow parsing 7-bit ASCII
#[allow(clippy::exhaustive_structs)]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct AsciiParser;

impl CharAccumulator for AsciiParser {
    fn add(&mut self, _byte: u8) -> Option<char> {
        unreachable!("multi-byte UTF8 characters are unsupported")
    }
}

/// Allow parsing UTF-8
#[cfg(feature = "utf8")]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Utf8Parser {
    utf8_parser: utf8::Parser,
}

#[cfg(feature = "utf8")]
impl CharAccumulator for Utf8Parser {
    fn add(&mut self, byte: u8) -> Option<char> {
        let mut c = None;
        let mut receiver = VtUtf8Receiver(&mut c);
        self.utf8_parser.advance(&mut receiver, byte);
        c
    }
}

#[cfg(feature = "utf8")]
struct VtUtf8Receiver<'a>(&'a mut Option<char>);

#[cfg(feature = "utf8")]
impl utf8::Receiver for VtUtf8Receiver<'_> {
    fn codepoint(&mut self, c: char) {
        *self.0 = Some(c);
    }

    fn invalid_sequence(&mut self) {
        *self.0 = Some('�');
    }
}

/// Performs actions requested by the [`Parser`]
///
/// Actions in this case mean, for example, handling a CSI escape sequence describing cursor
/// movement, or simply printing characters to the screen.
///
/// The methods on this type correspond to actions described in
/// <http://vt100.net/emu/dec_ansi_parser>. I've done my best to describe them in
/// a useful way in my own words for completeness, but the site should be
/// referenced if something isn't clear. If the site disappears at some point in
/// the future, consider checking archive.org.
pub trait Perform {
    /// Draw a character to the screen and update states.
    fn print(&mut self, _c: char) {}

    /// Execute a C0 or C1 control function.
    fn execute(&mut self, _byte: u8) {}

    /// Invoked when a final character arrives in first part of device control string.
    ///
    /// The control function should be determined from the private marker, final character, and
    /// execute with a parameter list. A handler should be selected for remaining characters in the
    /// string; the handler function should subsequently be called by `put` for every character in
    /// the control string.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: u8) {}

    /// Pass bytes as part of a device control string to the handle chosen in `hook`. C0 controls
    /// will also be passed to the handler.
    fn put(&mut self, _byte: u8) {}

    /// Called when a device control string is terminated.
    ///
    /// The previously selected handler should be notified that the DCS has
    /// terminated.
    fn unhook(&mut self) {}

    /// Dispatch an operating system command.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    /// A final character has arrived for a CSI sequence
    ///
    /// The `ignore` flag indicates that either more than two intermediates arrived
    /// or the number of parameters exceeded the maximum supported length,
    /// and subsequent characters were ignored.
    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: u8,
    ) {
    }

    /// The final character of an escape sequence has arrived.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
