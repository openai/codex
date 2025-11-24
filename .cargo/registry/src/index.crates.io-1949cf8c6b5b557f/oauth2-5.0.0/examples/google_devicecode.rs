//!
//! This example showcases the Google OAuth2 process for requesting access to the Google Calendar features
//! and the user's profile.
//!
//! Before running it, you'll need to generate your own Google OAuth2 credentials.
//!
//! In order to run the example call:
//!
//! ```sh
//! GOOGLE_CLIENT_ID=xxx GOOGLE_CLIENT_SECRET=yyy cargo run --example google
//! ```
//!
//! ...and follow the instructions.
//!

use oauth2::basic::BasicClient;
use oauth2::reqwest;
use oauth2::{
    AuthType, AuthUrl, ClientId, ClientSecret, DeviceAuthorizationResponse, DeviceAuthorizationUrl,
    ExtraDeviceAuthorizationFields, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct StoringFields(HashMap<String, serde_json::Value>);

impl ExtraDeviceAuthorizationFields for StoringFields {}
type StoringDeviceAuthorizationResponse = DeviceAuthorizationResponse<StoringFields>;

fn main() {
    let google_client_id = ClientId::new(
        env::var("GOOGLE_CLIENT_ID").expect("Missing the GOOGLE_CLIENT_ID environment variable."),
    );
    let google_client_secret = ClientSecret::new(
        env::var("GOOGLE_CLIENT_SECRET")
            .expect("Missing the GOOGLE_CLIENT_SECRET environment variable."),
    );
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .expect("Invalid token endpoint URL");
    let device_auth_url =
        DeviceAuthorizationUrl::new("https://oauth2.googleapis.com/device/code".to_string())
            .expect("Invalid device authorization endpoint URL");

    // Set up the config for the Google OAuth2 process.
    //
    // Google's OAuth endpoint expects the client_id to be in the request body,
    // so ensure that option is set.
    let device_client = BasicClient::new(google_client_id)
        .set_client_secret(google_client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_device_authorization_url(device_auth_url)
        .set_auth_type(AuthType::RequestBody);

    let http_client = reqwest::blocking::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Request the set of codes from the Device Authorization endpoint.
    let details: StoringDeviceAuthorizationResponse = device_client
        .exchange_device_code()
        .add_scope(Scope::new("profile".to_string()))
        .request(&http_client)
        .expect("Failed to request codes from device auth endpoint");

    // Display the URL and user-code.
    println!(
        "Open this URL in your browser:\n{}\nand enter the code: {}",
        details.verification_uri(),
        details.user_code().secret(),
    );

    // Now poll for the token
    let token = device_client
        .exchange_device_access_token(&details)
        .request(&http_client, std::thread::sleep, None)
        .expect("Failed to get token");

    println!("Google returned the following token:\n{token:?}\n");
}
