//! Extended methods for StreamingClient.
//!
//! Provides interceptor support without modifying streaming.rs significantly.

use crate::auth::AuthProvider;
use crate::interceptors::InterceptorContext;
use crate::interceptors::apply_interceptors;
use codex_client::HttpTransport;
use codex_client::Request;

use super::streaming::StreamingClient;

impl<T: HttpTransport, A: AuthProvider> StreamingClient<T, A> {
    /// Build InterceptorContext from current state.
    pub(crate) fn build_interceptor_context(
        &self,
        model: Option<&str>,
        conversation_id: Option<&str>,
    ) -> InterceptorContext {
        InterceptorContext {
            conversation_id: conversation_id.map(String::from),
            model: model.map(String::from),
            provider_name: Some(self.provider().name.clone()),
        }
    }

    /// Apply interceptors to a request.
    pub(crate) fn apply_interceptors_to_request(
        &self,
        request: &mut Request,
        ctx: &InterceptorContext,
    ) {
        apply_interceptors(request, ctx, &self.provider().interceptors);
    }
}
