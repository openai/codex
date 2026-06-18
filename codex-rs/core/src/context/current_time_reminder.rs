use chrono::DateTime;
use chrono::Utc;

use super::ContextualUserFragment;

pub(crate) struct CurrentTimeReminder {
    current_time: DateTime<Utc>,
}

impl CurrentTimeReminder {
    pub(crate) fn new(current_time: DateTime<Utc>) -> Self {
        Self { current_time }
    }
}

impl ContextualUserFragment for CurrentTimeReminder {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        format!(
            "It is {}.",
            self.current_time.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}
