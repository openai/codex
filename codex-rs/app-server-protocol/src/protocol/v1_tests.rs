use super::AuthMode;
use crate::protocol::common::AuthMode as ApiAuthMode;
use pretty_assertions::assert_eq;

#[test]
fn personal_access_token_reports_chatgpt_auth_mode_on_v1() {
    assert_eq!(
        AuthMode::from(ApiAuthMode::PersonalAccessToken),
        AuthMode::Chatgpt
    );
}
