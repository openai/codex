use std::collections::BTreeMap;

use ratatui::text::Line;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum StatusSurfacePreviewItem {
    AppName,
    ProjectName,
    CurrentDir,
    Status,
    ThreadTitle,
    GitBranch,
    ContextRemaining,
    ContextUsed,
    FiveHourLimit,
    WeeklyLimit,
    CodexVersion,
    ContextWindowSize,
    UsedTokens,
    TotalInputTokens,
    TotalOutputTokens,
    SessionId,
    FastMode,
    Model,
    ModelWithReasoning,
    TaskProgress,
}

impl StatusSurfacePreviewItem {
    fn placeholder(self) -> &'static str {
        match self {
            StatusSurfacePreviewItem::AppName => "codex",
            StatusSurfacePreviewItem::ProjectName => "my-project",
            StatusSurfacePreviewItem::CurrentDir => "~/my-project/subdir",
            StatusSurfacePreviewItem::Status => "Working",
            StatusSurfacePreviewItem::ThreadTitle => "thread title",
            StatusSurfacePreviewItem::GitBranch => "feat/awesome-feature",
            StatusSurfacePreviewItem::ContextRemaining => "Context 0% left",
            StatusSurfacePreviewItem::ContextUsed => "Context 0% used",
            StatusSurfacePreviewItem::FiveHourLimit => "5h 0%",
            StatusSurfacePreviewItem::WeeklyLimit => "weekly 0%",
            StatusSurfacePreviewItem::CodexVersion => "0.0.0",
            StatusSurfacePreviewItem::ContextWindowSize => "0 window",
            StatusSurfacePreviewItem::UsedTokens => "0 used",
            StatusSurfacePreviewItem::TotalInputTokens => "0 in",
            StatusSurfacePreviewItem::TotalOutputTokens => "0 out",
            StatusSurfacePreviewItem::SessionId => "550e8400-e29b-41d4",
            StatusSurfacePreviewItem::FastMode => "Fast on",
            StatusSurfacePreviewItem::Model => "gpt-5.2-codex",
            StatusSurfacePreviewItem::ModelWithReasoning => "gpt-5.2-codex medium",
            StatusSurfacePreviewItem::TaskProgress => "Tasks 0/0",
        }
    }

    pub(crate) fn iter() -> impl Iterator<Item = Self> {
        [
            Self::AppName,
            Self::ProjectName,
            Self::CurrentDir,
            Self::Status,
            Self::ThreadTitle,
            Self::GitBranch,
            Self::ContextRemaining,
            Self::ContextUsed,
            Self::FiveHourLimit,
            Self::WeeklyLimit,
            Self::CodexVersion,
            Self::ContextWindowSize,
            Self::UsedTokens,
            Self::TotalInputTokens,
            Self::TotalOutputTokens,
            Self::SessionId,
            Self::FastMode,
            Self::Model,
            Self::ModelWithReasoning,
            Self::TaskProgress,
        ]
        .into_iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreviewValue {
    text: String,
    is_placeholder: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatusSurfacePreviewData {
    values: BTreeMap<StatusSurfacePreviewItem, PreviewValue>,
}

impl Default for StatusSurfacePreviewData {
    fn default() -> Self {
        let mut data = Self {
            values: BTreeMap::new(),
        };
        for item in StatusSurfacePreviewItem::iter() {
            data.set_placeholder(item, item.placeholder());
        }
        data
    }
}

impl StatusSurfacePreviewData {
    pub(crate) fn from_iter<I, V>(values: I) -> Self
    where
        I: IntoIterator<Item = (StatusSurfacePreviewItem, V)>,
        V: Into<String>,
    {
        let mut data = Self::default();
        for (item, value) in values {
            data.set_live(item, value);
        }
        data
    }

    pub(crate) fn set_live<V>(&mut self, item: StatusSurfacePreviewItem, value: V)
    where
        V: Into<String>,
    {
        self.values.insert(
            item,
            PreviewValue {
                text: value.into(),
                is_placeholder: false,
            },
        );
    }

    pub(crate) fn set_placeholder<V>(&mut self, item: StatusSurfacePreviewItem, value: V)
    where
        V: Into<String>,
    {
        if self
            .values
            .get(&item)
            .is_some_and(|value| !value.is_placeholder)
        {
            return;
        }
        self.values.insert(
            item,
            PreviewValue {
                text: value.into(),
                is_placeholder: true,
            },
        );
    }

    pub(crate) fn value_for(&self, item: StatusSurfacePreviewItem) -> Option<&str> {
        self.values.get(&item).map(|value| value.text.as_str())
    }

    pub(crate) fn line_for_items<I>(&self, items: I) -> Option<Line<'static>>
    where
        I: IntoIterator<Item = StatusSurfacePreviewItem>,
    {
        let preview = items
            .into_iter()
            .filter_map(|item| self.value_for(item))
            .collect::<Vec<_>>()
            .join(" · ");
        if preview.is_empty() {
            None
        } else {
            Some(Line::from(preview))
        }
    }
}
