use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, StandardDeviceAuthorizationResponse, TokenUrl,
};

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = BasicClient::new(ClientId::new("client_id".to_string()))
        .set_auth_uri(AuthUrl::new(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
        )?)
        .set_token_uri(TokenUrl::new(
            "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
        )?)
        .set_device_authorization_url(DeviceAuthorizationUrl::new(
            "https://login.microsoftonline.com/common/oauth2/v2.0/devicecode".to_string(),
        )?);

    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let details: StandardDeviceAuthorizationResponse = client
        .exchange_device_code()
        .add_scope(Scope::new("read".to_string()))
        .request_async(&http_client)
        .await?;

    eprintln!(
        "Open this URL in your browser:\n{}\nand enter the code: {}",
        details.verification_uri(),
        details.user_code().secret(),
    );

    let token_result = client
        .exchange_device_access_token(&details)
        .request_async(&http_client, tokio::time::sleep, None)
        .await;

    eprintln!("Token:{token_result:?}");

    Ok(())
}
