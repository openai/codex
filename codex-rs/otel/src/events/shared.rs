use chrono::SecondsFormat;
use chrono::Utc;

macro_rules! log_event {
    ($self:expr, $($fields:tt)*) => {{
        tracing::event!(
            target: $crate::targets::OTEL_LOG_ONLY_TARGET,
            tracing::Level::INFO,
            $($fields)*
            event.timestamp = %$crate::events::shared::timestamp(),
            conversation.id = %$self.metadata.conversation_id,
            app.version = %$self.metadata.app_version,
            auth_mode = $self.metadata.auth_mode,
            originator = %$self.metadata.originator,
            user.account_id = $self.metadata.account_id,
            user.email = $self.metadata.account_email,
            terminal.type = %$self.metadata.terminal_type,
            model = %$self.metadata.model,
            slug = %$self.metadata.slug,
        );
    }};
}

macro_rules! trace_event {
    ($self:expr, $($fields:tt)*) => {{
        tracing::event!(
            target: $crate::targets::OTEL_TRACE_SAFE_TARGET,
            tracing::Level::INFO,
            $($fields)*
            event.timestamp = %$crate::events::shared::timestamp(),
            conversation.id = %$self.metadata.conversation_id,
            app.version = %$self.metadata.app_version,
            auth_mode = $self.metadata.auth_mode,
            originator = %$self.metadata.originator,
            terminal.type = %$self.metadata.terminal_type,
            model = %$self.metadata.model,
            slug = %$self.metadata.slug,
        );
    }};
}

macro_rules! log_and_trace_event {
    (
        $self:expr,
        common: { $($common:tt)* },
        log: { $($log:tt)* },
        trace: { $($trace:tt)* },
    ) => {{
        log_event!($self, $($common)* $($log)*);
        trace_event!($self, $($common)* $($trace)*);
    }};
}

pub(crate) use log_and_trace_event;
pub(crate) use log_event;
pub(crate) use trace_event;

pub(crate) fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
