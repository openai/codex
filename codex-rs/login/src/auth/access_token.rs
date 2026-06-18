const PERSONAL_ACCESS_TOKEN_PREFIX: &str = "at-";

pub(super) enum CodexAccessToken<'a> {
    PersonalAccessToken(&'a str),
    AgentIdentityJwt(&'a str),
}

pub(super) fn classify_codex_access_token(access_token: &str) -> CodexAccessToken<'_> {
    if access_token.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX) {
        CodexAccessToken::PersonalAccessToken(access_token)
    } else {
        CodexAccessToken::AgentIdentityJwt(access_token)
    }
}

#[cfg(test)]
#[path = "access_token_tests.rs"]
mod tests;
