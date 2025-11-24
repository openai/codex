use std::ffi::OsStr;
use std::ffi::OsString;

use clap::builder::StyledStr;

/// A shell-agnostic completion candidate
#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionCandidate {
    value: OsString,
    help: Option<StyledStr>,
    id: Option<String>,
    tag: Option<StyledStr>,
    display_order: Option<usize>,
    hidden: bool,
}

impl CompletionCandidate {
    /// Create a new completion candidate
    pub fn new(value: impl Into<OsString>) -> Self {
        let value = value.into();
        Self {
            value,
            ..Default::default()
        }
    }

    /// Set the help message of the completion candidate
    pub fn help(mut self, help: Option<StyledStr>) -> Self {
        self.help = help;
        self
    }

    /// Only first for a given Id is shown
    ///
    /// To reduce the risk of conflicts, this should likely contain a namespace.
    pub fn id(mut self, id: Option<String>) -> Self {
        self.id = id;
        self
    }

    /// Group candidates by tag
    ///
    /// Future: these may become user-visible
    pub fn tag(mut self, tag: Option<StyledStr>) -> Self {
        self.tag = tag;
        self
    }

    /// Sort weight within a [`CompletionCandidate::tag`]
    pub fn display_order(mut self, order: Option<usize>) -> Self {
        self.display_order = order;
        self
    }

    /// Set the visibility of the completion candidate
    ///
    /// Only shown when there is no visible candidate for completing the current argument.
    pub fn hide(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Add a prefix to the value of completion candidate
    ///
    /// This is generally used for post-process by [`complete`][crate::engine::complete()] for
    /// things like pre-pending flags, merging delimiter-separated values, etc.
    pub fn add_prefix(mut self, prefix: impl Into<OsString>) -> Self {
        let suffix = self.value;
        let mut value = prefix.into();
        value.push(&suffix);
        self.value = value;
        self
    }
}

/// Reflection API
impl CompletionCandidate {
    /// Get the literal value being proposed for completion
    pub fn get_value(&self) -> &OsStr {
        &self.value
    }

    /// Get the help message of the completion candidate
    pub fn get_help(&self) -> Option<&StyledStr> {
        self.help.as_ref()
    }

    /// Get the id used for de-duplicating
    pub fn get_id(&self) -> Option<&String> {
        self.id.as_ref()
    }

    /// Get the grouping tag
    pub fn get_tag(&self) -> Option<&StyledStr> {
        self.tag.as_ref()
    }

    /// Get the grouping tag
    pub fn get_display_order(&self) -> Option<usize> {
        self.display_order
    }

    /// Get the visibility of the completion candidate
    pub fn is_hide_set(&self) -> bool {
        self.hidden
    }
}

impl<S: Into<OsString>> From<S> for CompletionCandidate {
    fn from(s: S) -> Self {
        CompletionCandidate::new(s.into())
    }
}
