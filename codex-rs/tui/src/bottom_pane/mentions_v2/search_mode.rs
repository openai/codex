use super::candidate::MentionType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SearchMode {
    Results,
    FilesystemOnly,
    Tools,
}

impl SearchMode {
    pub(super) fn previous(self) -> Self {
        match self {
            Self::Results => Self::Tools,
            Self::FilesystemOnly => Self::Results,
            Self::Tools => Self::FilesystemOnly,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Results => Self::FilesystemOnly,
            Self::FilesystemOnly => Self::Tools,
            Self::Tools => Self::Results,
        }
    }

    pub(super) fn accepts(self, mention_type: MentionType) -> bool {
        match self {
            Self::Results => true,
            Self::FilesystemOnly => {
                matches!(mention_type, MentionType::File | MentionType::Directory)
            }
            Self::Tools => {
                matches!(
                    mention_type,
                    MentionType::Plugin | MentionType::App | MentionType::Skill
                )
            }
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Results => "All Results",
            Self::FilesystemOnly => "Filesystem Only",
            Self::Tools => "Plugins",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SearchModeSettings {
    favorite_search_mode: Option<SearchMode>,
}

impl SearchModeSettings {
    pub(super) fn favorite_search_mode(self) -> SearchMode {
        self.favorite_search_mode.unwrap_or(SearchMode::Results)
    }

    pub(super) fn toggle_favorite_search_mode(&mut self, search_mode: SearchMode) {
        if self.favorite_search_mode == Some(search_mode) {
            self.favorite_search_mode = None;
        } else {
            self.favorite_search_mode = Some(search_mode);
        }
    }
}
