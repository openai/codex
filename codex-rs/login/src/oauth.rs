use url::Url;

pub const PROD_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const STAGING_CLIENT_ID: &str = "app_WWpKUzlOnCTqf9WmuzvqovoW";

const PROD_ISSUER: &str = "https://auth.openai.com";
const STAGING_ISSUER: &str = "https://auth.api.openai.org";
const STAGING_CHATGPT_HOST: &str = "chatgpt-staging.com";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatgptOAuthConfig {
    pub client_id: String,
    pub issuer: String,
}

impl Default for ChatgptOAuthConfig {
    fn default() -> Self {
        Self::prod()
    }
}

impl ChatgptOAuthConfig {
    pub fn new(client_id: String, issuer: String) -> Self {
        Self { client_id, issuer }
    }

    pub fn for_chatgpt_base_url(chatgpt_base_url: Option<&str>) -> Self {
        if chatgpt_base_url.is_some_and(is_staging_chatgpt_base_url) {
            return Self::staging();
        }

        Self::prod()
    }

    pub fn token_url(&self) -> String {
        self.oauth_endpoint("token")
    }

    pub fn revoke_url(&self) -> String {
        self.oauth_endpoint("revoke")
    }

    fn prod() -> Self {
        Self::new(PROD_CLIENT_ID.to_string(), PROD_ISSUER.to_string())
    }

    fn staging() -> Self {
        Self::new(STAGING_CLIENT_ID.to_string(), STAGING_ISSUER.to_string())
    }

    fn oauth_endpoint(&self, endpoint: &str) -> String {
        format!("{}/oauth/{endpoint}", self.issuer.trim_end_matches('/'))
    }
}

fn is_staging_chatgpt_base_url(chatgpt_base_url: &str) -> bool {
    let Ok(url) = Url::parse(chatgpt_base_url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };

    host == STAGING_CHATGPT_HOST || host.ends_with(&format!(".{STAGING_CHATGPT_HOST}"))
}

#[cfg(test)]
#[path = "oauth_tests.rs"]
mod tests;
