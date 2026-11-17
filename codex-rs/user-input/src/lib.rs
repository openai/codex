//! Captures user attachments and renders resolved input for model requests.

mod local_media;
mod message;

pub use local_media::audio_mime_for_path;
pub use local_media::snapshot_local_user_input;
pub use message::LocalImagePreparation;
pub use message::from_user_input;

#[cfg(test)]
#[path = "message_tests.rs"]
mod message_tests;
