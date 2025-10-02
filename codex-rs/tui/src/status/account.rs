#[derive(Debug, Clone)]
pub enum StatusAccountDisplay {
    ChatGpt {
        email: Option<String>,
        plan: Option<String>,
    },
    ApiKey,
}
