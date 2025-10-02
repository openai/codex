use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use keepawake::KeepAwake;
use tracing::debug;

static PREVENT_SLEEP_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_prevent_sleep_enabled(enabled: bool) {
    PREVENT_SLEEP_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_prevent_sleep_enabled() -> bool {
    PREVENT_SLEEP_ENABLED.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy)]
pub enum ActivityKind {
    RemoteApi,
    LocalTool,
}

impl ActivityKind {
    fn reason_prefix(self) -> &'static str {
        match self {
            ActivityKind::RemoteApi => "Codex remote API call",
            ActivityKind::LocalTool => "Codex local tool execution",
        }
    }
}

#[must_use]
#[derive(Default)]
pub struct Guard {
    inner: Option<KeepAwake>,
}

impl Guard {
    pub fn for_activity(activity: ActivityKind, detail: impl AsRef<str>) -> Self {
        if !is_prevent_sleep_enabled() {
            return Self::default();
        }

        let detail = detail.as_ref().trim();
        let reason = if detail.is_empty() {
            activity.reason_prefix().to_string()
        } else {
            format!("{} ({detail})", activity.reason_prefix())
        };

        match keepawake::Builder::default()
            .display(false)
            .idle(true)
            .sleep(true)
            .reason(reason.clone())
            .app_name("codex")
            .app_reverse_domain("com.openai.codex")
            .create()
        {
            Ok(inner) => Self { inner: Some(inner) },
            Err(err) => {
                debug!(%err, reason, "failed to acquire keepawake guard");
                Self { inner: None }
            }
        }
    }

    pub fn remote_api(detail: impl AsRef<str>) -> Self {
        Self::for_activity(ActivityKind::RemoteApi, detail)
    }

    pub fn local_tool(detail: impl AsRef<str>) -> Self {
        Self::for_activity(ActivityKind::LocalTool, detail)
    }

    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }
}
