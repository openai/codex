use std::path::PathBuf;

use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Span;

const TAG_WIDTH: usize = "[Plugin]".len();

#[derive(Clone, Debug)]
pub(crate) enum Selection {
    File(PathBuf),
    Tool {
        insert_text: String,
        path: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum MentionType {
    Plugin,
    App,
    Skill,
    File,
    Directory,
}

impl MentionType {
    pub(super) fn is_filesystem(self) -> bool {
        matches!(self, Self::File | Self::Directory)
    }

    pub(super) fn span(self, base_style: Style) -> Span<'static> {
        let style = match self {
            Self::Plugin | Self::App => base_style.magenta(),
            Self::Skill => base_style.dark_gray(),
            Self::File => base_style.cyan(),
            Self::Directory => base_style,
        };
        format!("{:<width$}", self.label(), width = TAG_WIDTH).set_style(style)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Plugin => "[Plugin]",
            Self::App => "[App]",
            Self::Skill => "[Skill]",
            Self::File => "[File]",
            Self::Directory => "[Dir]",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(super) display_name: String,
    pub(super) description: Option<String>,
    pub(super) search_terms: Vec<String>,
    pub(super) mention_type: MentionType,
    pub(super) selection: Selection,
}

#[derive(Clone, Debug)]
pub(super) struct SearchResult {
    pub(super) display_name: String,
    pub(super) description: Option<String>,
    pub(super) mention_type: MentionType,
    pub(super) selection: Selection,
    pub(super) match_indices: Option<Vec<usize>>,
    pub(super) score: i32,
}

impl Candidate {
    pub(super) fn to_result(&self, match_indices: Option<Vec<usize>>, score: i32) -> SearchResult {
        SearchResult {
            display_name: self.display_name.clone(),
            description: self.description.clone(),
            mention_type: self.mention_type,
            selection: self.selection.clone(),
            match_indices,
            score,
        }
    }
}
