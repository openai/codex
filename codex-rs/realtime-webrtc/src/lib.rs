mod client;
mod helper_exit;
#[cfg(any(target_os = "linux", test))]
mod linux_alsa;
mod message_reader;
mod protocol;
mod session;

pub use client::ConnectionError;
pub use client::VoiceHost;
pub use helper_exit::HelperExitStage;
pub use protocol::AudioControls;
pub use protocol::AudioState;
pub use protocol::MAX_FRAME_BYTES;
pub use protocol::Message;
pub use protocol::RUNTIME_ENVIRONMENT;
pub use protocol::SessionDescription;
pub use protocol::decode_frame;
pub use protocol::encode_frame;
pub use protocol::read_message;
pub use session::RealtimeWebrtcSession;
pub use session::RealtimeWebrtcSessionHandle;
pub use session::StartedRealtimeWebrtcSession;
