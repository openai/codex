#[derive(Debug, Clone)]
pub(crate) enum StatusAccountDisplay {
    ChatGpt {
        email: Option<String>,
        plan: Option<String>,
    },
    ApiKey,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StatusAccountMetadata {
    pub(crate) account_display_name: Option<String>,
    pub(crate) account_group_names: Option<Vec<String>>,
}
