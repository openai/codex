pub use self::{
    grapheme::{decode_grapheme, GraphemeIndices, Graphemes},
    sentence::{SentenceIndices, Sentences},
    whitespace::{whitespace_len_fwd, whitespace_len_rev},
    word::{WordIndices, Words, WordsWithBreakIndices, WordsWithBreaks},
};

mod fsm;
mod grapheme;
mod sentence;
mod whitespace;
mod word;
