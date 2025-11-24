//! Hints (suggestions at the right of the prompt as you type).

use crate::history::SearchDirection;
use crate::Context;

/// A hint returned by Hinter
pub trait Hint {
    /// Text to display when hint is active
    fn display(&self) -> &str;
    /// Text to insert in line when right arrow is pressed
    fn completion(&self) -> Option<&str>;
}

impl Hint for String {
    fn display(&self) -> &str {
        self.as_str()
    }

    fn completion(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

/// Hints provider
pub trait Hinter {
    /// Specific hint type
    type Hint: Hint + 'static;

    /// Takes the currently edited `line` with the cursor `pos`ition and
    /// returns the string that should be displayed or `None`
    /// if no hint is available for the text the user currently typed.
    // TODO Validate: called while editing line but not while moving cursor.
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        let _ = (line, pos, ctx);
        None
    }
}

impl Hinter for () {
    type Hint = String;
}

impl<'r, H: ?Sized + Hinter> Hinter for &'r H {
    type Hint = H::Hint;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        (**self).hint(line, pos, ctx)
    }
}

/// Add suggestion based on previous history entries matching current user
/// input.
#[derive(Default)]
pub struct HistoryHinter {}

impl HistoryHinter {
    /// Create a new `HistoryHinter`
    pub fn new() -> HistoryHinter {
        HistoryHinter::default()
    }
}

impl Hinter for HistoryHinter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if line.is_empty() || pos < line.len() {
            return None;
        }
        let start = if ctx.history_index() == ctx.history().len() {
            ctx.history_index().saturating_sub(1)
        } else {
            ctx.history_index()
        };
        if let Some(sr) = ctx
            .history
            .starts_with(line, start, SearchDirection::Reverse)
            .unwrap_or(None)
        {
            if sr.entry == line {
                return None;
            }
            return Some(sr.entry[pos..].to_owned());
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::{Hinter, HistoryHinter};
    use crate::history::DefaultHistory;
    use crate::Context;

    #[test]
    pub fn empty_history() {
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);
        let hinter = HistoryHinter {};
        let hint = hinter.hint("test", 4, &ctx);
        assert_eq!(None, hint);
    }
}
