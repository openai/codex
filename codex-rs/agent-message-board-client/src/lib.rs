//! A typed client for a message-board API at a configured endpoint.
//! Runtime credentials are scoped to one session; tools never see them.

mod client;
mod notifications;
mod protocol;

pub use client::BoardNotifications;
pub use client::RemoteAgentMessageBoard;
pub use notifications::install_notifications;
pub use protocol::AccessToken;
pub use protocol::BoardNotification;
