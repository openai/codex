//! Global override of color control

#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

use core::sync::atomic::{AtomicUsize, Ordering};

/// Selection for overriding color output
#[allow(clippy::exhaustive_enums)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorChoice {
    /// Use colors if the output device appears to support them
    Auto,
    /// Like `Always`, except it never tries to use anything other than emitting ANSI
    /// color codes.
    AlwaysAnsi,
    /// Try very hard to emit colors.
    ///
    /// This includes emitting ANSI colors on Windows if the console API is unavailable.
    Always,
    /// Never emit colors.
    Never,
}

impl ColorChoice {
    /// Get the current [`ColorChoice`] state
    pub fn global() -> Self {
        USER.get()
    }

    /// Override the detected [`ColorChoice`]
    pub fn write_global(self) {
        USER.set(self);
    }
}

impl Default for ColorChoice {
    fn default() -> Self {
        Self::Auto
    }
}

static USER: AtomicChoice = AtomicChoice::new();

#[derive(Debug)]
pub(crate) struct AtomicChoice(AtomicUsize);

impl AtomicChoice {
    pub(crate) const fn new() -> Self {
        Self(AtomicUsize::new(Self::from_choice(ColorChoice::Auto)))
    }

    pub(crate) fn get(&self) -> ColorChoice {
        let choice = self.0.load(Ordering::SeqCst);
        Self::to_choice(choice).expect("Only `ColorChoice` values can be `set`")
    }

    pub(crate) fn set(&self, choice: ColorChoice) {
        let choice = Self::from_choice(choice);
        self.0.store(choice, Ordering::SeqCst);
    }

    const fn from_choice(choice: ColorChoice) -> usize {
        match choice {
            ColorChoice::Auto => 0,
            ColorChoice::AlwaysAnsi => 1,
            ColorChoice::Always => 2,
            ColorChoice::Never => 3,
        }
    }

    const fn to_choice(choice: usize) -> Option<ColorChoice> {
        match choice {
            0 => Some(ColorChoice::Auto),
            1 => Some(ColorChoice::AlwaysAnsi),
            2 => Some(ColorChoice::Always),
            3 => Some(ColorChoice::Never),
            _ => None,
        }
    }
}

impl Default for AtomicChoice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn choice_serialization() {
        let expected = vec![
            ColorChoice::Auto,
            ColorChoice::AlwaysAnsi,
            ColorChoice::Always,
            ColorChoice::Never,
        ];
        let values: Vec<_> = expected
            .iter()
            .cloned()
            .map(AtomicChoice::from_choice)
            .collect();
        let actual: Vec<_> = values
            .iter()
            .cloned()
            .filter_map(AtomicChoice::to_choice)
            .collect();
        assert_eq!(expected, actual);
    }
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
