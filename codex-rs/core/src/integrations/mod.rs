//! External service integrations for webhooks and APIs.

pub mod webhook_client;

pub use webhook_client::WebhookClient;
pub use webhook_client::WebhookPayload;
pub use webhook_client::WebhookService;
