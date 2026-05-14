use codex_login::default_client::ClientIdentity;

#[derive(Debug, Clone)]
pub struct AppServerClientInfo {
    pub name: String,
    pub version: String,
    pub client_identity: ClientIdentity,
}
