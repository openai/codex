//! `clap`-native completion system
//!
//! See [`complete()`]

mod candidate;
mod complete;
mod custom;

pub use candidate::CompletionCandidate;
pub use complete::complete;
pub use custom::ArgValueCandidates;
pub use custom::ArgValueCompleter;
pub use custom::PathCompleter;
pub use custom::SubcommandCandidates;
pub use custom::ValueCandidates;
pub use custom::ValueCompleter;
