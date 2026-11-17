//! Captures user attachments and renders resolved input for model requests.

mod local_media;
mod resolved_user_input;

pub use local_media::audio_mime_for_path;
pub use local_media::snapshot_local_user_input;
pub use resolved_user_input::LocalImagePreparation;
pub use resolved_user_input::ResolvedUserInput;

#[cfg(test)]
#[path = "message_tests.rs"]
mod message_tests;
