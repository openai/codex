pub mod auth;
pub mod chat;
mod client;
// Legacy payload decoding has been removed; wire decoding lives in decode_wire
mod decode_wire;
pub mod error;
// payload building lives in codex-core now
pub mod responses;
pub mod routed_client;
pub mod stream;

pub use crate::auth::AuthContext;
pub use crate::auth::AuthProvider;
pub use crate::chat::ChatCompletionsApiClient;
pub use crate::chat::ChatCompletionsApiClientConfig;
pub use crate::error::Error;
pub use crate::error::Result;
pub use crate::responses::ResponsesApiClient;
pub use crate::responses::ResponsesApiClientConfig;
pub use crate::routed_client::RoutedApiClient;
pub use crate::routed_client::RoutedApiClientConfig;
pub use crate::stream::EventStream;
pub use crate::stream::Reasoning;
pub use crate::stream::ResponseEvent;
pub use crate::stream::ResponseStream;
pub use crate::stream::TextControls;
pub use crate::stream::TextFormat;
pub use crate::stream::TextFormatType;
pub use crate::stream::WireEvent;
pub use crate::stream::WireResponseStream;
pub use codex_provider_config::ModelProviderInfo;
pub use codex_provider_config::WireApi;
