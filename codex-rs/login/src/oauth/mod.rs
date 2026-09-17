//! OAuth protocol operations shared by independent credential managers.
//!
//! Callers supply an HTTP client with the application's proxy and CA policy. They retain
//! ownership of credential storage, refresh scheduling, account validation, and login UX.

mod authorization;
mod client;
mod diagnostics;
mod error;
mod pkce;

pub(crate) use authorization::AuthorizationRequest;
pub(crate) use authorization::CallbackError;
pub(crate) use authorization::CallbackParameters;
pub(crate) use authorization::build_authorization_url;
pub(crate) use authorization::generate_state;
pub(crate) use client::AuthorizationCodeGrant;
pub(crate) use client::OAuthClient;
pub(crate) use client::RefreshTokenGrant;
pub(crate) use client::TokenEncoding;
pub(crate) use client::TokenEndpoint;
pub(crate) use diagnostics::sanitize_url_for_logging;
pub(crate) use error::ErrorBodyLimit;
pub(crate) use error::OAuthError;
pub(crate) use error::TokenErrorDetail;
pub(crate) use error::TokenRejection;
pub(crate) use pkce::PkceCodes;
pub(crate) use pkce::generate_pkce;
