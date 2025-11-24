//! A TOML push [parser][parse_document]
//!
//! This takes TOML [tokens][crate::lexer::Token] and [emits][EventReceiver] [events][Event].

mod document;
mod event;

pub use document::parse_document;
pub use document::parse_key;
pub use document::parse_simple_key;
pub use document::parse_value;
pub use event::Event;
pub use event::EventKind;
pub use event::EventReceiver;
pub use event::RecursionGuard;
pub use event::ValidateWhitespace;
