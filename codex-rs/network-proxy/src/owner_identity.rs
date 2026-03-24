use rama_core::extensions::Extensions;
use rama_core::extensions::ExtensionsRef;
use rama_http::headers::authorization::AuthoritySync;
use rama_net::user::Basic;
use rama_net::user::UserId;
use rama_net::user::authority::AuthorizeResult;
use rama_net::user::authority::Authorizer;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProxyParentToolItemAuthorizer;

impl AuthoritySync<Basic, ()> for ProxyParentToolItemAuthorizer {
    fn authorized(&self, ext: &mut Extensions, credentials: &Basic) -> bool {
        ext.insert(UserId::Username(credentials.username().to_string()));
        true
    }
}

impl Authorizer<Basic> for ProxyParentToolItemAuthorizer {
    type Error = std::convert::Infallible;

    async fn authorize(&self, credentials: Basic) -> AuthorizeResult<Basic, Self::Error> {
        let mut extensions = Extensions::new();
        extensions.insert(UserId::Username(credentials.username().to_string()));
        AuthorizeResult {
            credentials,
            result: Ok(Some(extensions)),
        }
    }
}

/// Extract the parent tool item id from proxy auth/user extensions. Managed
/// proxy callbacks carry the parent tool item id as the proxy auth username so
/// core can attribute blocked requests back to the originating tool call.
pub(crate) fn extract_parent_tool_item_id<T: ExtensionsRef>(input: &T) -> Option<String> {
    match input.extensions().get::<UserId>() {
        Some(UserId::Username(username)) => Some(username.clone()),
        Some(UserId::Token(token)) => String::from_utf8(token.clone()).ok(),
        Some(UserId::Anonymous) | None => None,
    }
}
