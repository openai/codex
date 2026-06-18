pub const PROD_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const PROD_ISSUER: &str = "https://auth.openai.com";

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

    pub fn token_url(&self) -> String {
        self.oauth_endpoint("token")
    }

    pub fn revoke_url(&self) -> String {
        self.oauth_endpoint("revoke")
    }

    fn prod() -> Self {
        Self::new(PROD_CLIENT_ID.to_string(), PROD_ISSUER.to_string())
    }

    fn oauth_endpoint(&self, endpoint: &str) -> String {
        format!("{}/oauth/{endpoint}", self.issuer.trim_end_matches('/'))
    }
}
