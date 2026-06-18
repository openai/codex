use pretty_assertions::assert_eq;

use super::*;

#[test]
fn uses_staging_oauth_for_staging_chatgpt_base_url() {
    assert_eq!(
        ChatgptOAuthConfig::for_chatgpt_base_url(Some("https://chatgpt-staging.com/backend-api/")),
        ChatgptOAuthConfig::staging()
    );
}

#[test]
fn uses_staging_oauth_for_staging_chatgpt_subdomain() {
    assert_eq!(
        ChatgptOAuthConfig::for_chatgpt_base_url(Some(
            "https://dev-foo.chatgpt-staging.com/backend-api/"
        )),
        ChatgptOAuthConfig::staging()
    );
}

#[test]
fn leaves_non_staging_chatgpt_base_urls_on_prod_oauth() {
    assert_eq!(
        ChatgptOAuthConfig::for_chatgpt_base_url(Some("https://chatgpt.com/backend-api/")),
        ChatgptOAuthConfig::prod()
    );
    assert_eq!(
        ChatgptOAuthConfig::for_chatgpt_base_url(Some("not a url")),
        ChatgptOAuthConfig::prod()
    );
}
