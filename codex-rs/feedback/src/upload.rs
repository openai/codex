use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use sentry::ClientOptions;
use sentry::protocol::Envelope;
use sentry::protocol::EnvelopeItem;
use sentry::protocol::Event;
use sentry::protocol::Exception;
use sentry::protocol::Level;
use sentry::protocol::Values;
use sentry::types::Dsn;

use crate::FeedbackSnapshot;
use crate::FeedbackUploadOptions;
use crate::SENTRY_DSN;
use crate::UPLOAD_TIMEOUT_SECS;
use crate::display_classification;

/// Serialized feedback ready to upload without performing additional filesystem reads.
pub struct PreparedFeedbackUpload {
    endpoint: String,
    authorization: String,
    body: Vec<u8>,
}

impl PreparedFeedbackUpload {
    /// Sends the prepared envelope using the application's configured outbound proxy policy.
    pub async fn send(self, http_client_factory: &HttpClientFactory) -> Result<()> {
        let client = http_client_factory
            .build_client(&self.endpoint, ClientRouteClass::Other)
            .context("failed to build Sentry feedback HTTP client")?;

        client
            .post(&self.endpoint)
            .header("X-Sentry-Auth", self.authorization)
            .timeout(Duration::from_secs(UPLOAD_TIMEOUT_SECS))
            .body(self.body)
            .send()
            .await
            .context("failed to send Sentry feedback envelope")?
            .error_for_status()
            .context("Sentry feedback upload returned an unsuccessful HTTP status")?;

        Ok(())
    }
}

impl FeedbackSnapshot {
    /// Builds a feedback envelope and synchronously reads any file-backed attachments.
    ///
    /// Call this from a blocking task, then send the resulting upload asynchronously with the
    /// application's configured [`HttpClientFactory`].
    pub fn prepare_feedback_upload(
        &self,
        options: FeedbackUploadOptions<'_>,
    ) -> Result<PreparedFeedbackUpload> {
        let dsn = SENTRY_DSN.parse::<Dsn>().context("invalid Sentry DSN")?;
        self.prepare_feedback_upload_with_dsn(options, &dsn)
    }

    fn prepare_feedback_upload_with_dsn(
        &self,
        options: FeedbackUploadOptions<'_>,
        dsn: &Dsn,
    ) -> Result<PreparedFeedbackUpload> {
        let tags = self.upload_tags(
            options.classification,
            options.reason,
            options.tags,
            options.session_source.as_ref(),
        );

        let level = match options.classification {
            "bug" | "bad_result" | "safety_check" => Level::Error,
            _ => Level::Info,
        };

        let mut envelope = Envelope::new();
        let title = format!(
            "[{}]: Codex session {}",
            display_classification(options.classification),
            self.thread_id
        );

        let mut event = Event {
            level,
            message: Some(title.clone()),
            tags,
            ..Default::default()
        };
        if let Some(reason) = options.reason {
            event.exception = Values::from(vec![Exception {
                ty: title,
                value: Some(reason.to_string()),
                ..Default::default()
            }]);
        }
        envelope.add_item(EnvelopeItem::Event(event));

        for attachment in self.feedback_attachments(
            options.include_logs,
            options.extra_attachments,
            options.extra_attachment_paths,
            options.logs_override,
        ) {
            envelope.add_item(EnvelopeItem::Attachment(attachment));
        }

        let mut body = Vec::new();
        envelope
            .to_writer(&mut body)
            .context("failed to serialize Sentry feedback envelope")?;

        let user_agent = ClientOptions::default().user_agent;
        Ok(PreparedFeedbackUpload {
            endpoint: dsn.envelope_api_url().to_string(),
            authorization: dsn.to_auth(Some(user_agent.as_ref())).to_string(),
            body,
        })
    }
}

#[cfg(test)]
#[path = "upload_tests.rs"]
mod tests;
