//!
//! This example showcases the Microsoft Graph OAuth2 process for requesting access to Microsoft
//! services using PKCE.
//!
//! Before running it, you'll need to generate your own Microsoft OAuth2 credentials. See
//! https://docs.microsoft.com/azure/active-directory/develop/quickstart-register-app
//! * Register a `Web` application with a `Redirect URI` of `http://localhost:3003/redirect`.
//! * In the left menu select `Overview`. Copy the `Application (client) ID` as the MSGRAPH_CLIENT_ID.
//! * In the left menu select `Certificates & secrets` and add a new client secret. Copy the secret value
//!   as MSGRAPH_CLIENT_SECRET.
//! * In the left menu select `API permissions` and add a permission. Select Microsoft Graph and
//!   `Delegated permissions`. Add the `Files.Read` permission.
//!
//! In order to run the example call:
//!
//! ```sh
//! MSGRAPH_CLIENT_ID=xxx MSGRAPH_CLIENT_SECRET=yyy cargo run --example msgraph
//! ```
//!
//! ...and follow the instructions.
//!

use oauth2::basic::BasicClient;
use oauth2::reqwest;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenUrl,
};
use url::Url;

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

fn main() {
    let graph_client_id = ClientId::new(
        env::var("MSGRAPH_CLIENT_ID").expect("Missing the MSGRAPH_CLIENT_ID environment variable."),
    );
    let graph_client_secret = ClientSecret::new(
        env::var("MSGRAPH_CLIENT_SECRET")
            .expect("Missing the MSGRAPH_CLIENT_SECRET environment variable."),
    );
    let auth_url =
        AuthUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string())
            .expect("Invalid authorization endpoint URL");
    let token_url =
        TokenUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string())
            .expect("Invalid token endpoint URL");

    // Set up the config for the Microsoft Graph OAuth2 process.
    let client = BasicClient::new(graph_client_id)
        .set_client_secret(graph_client_secret)
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        // Microsoft Graph requires client_id and client_secret in URL rather than
        // using Basic authentication.
        .set_auth_type(AuthType::RequestBody)
        // This example will be running its own server at localhost:3003.
        // See below for the server implementation.
        .set_redirect_uri(
            RedirectUrl::new("http://localhost:3003/redirect".to_string())
                .expect("Invalid redirect URL"),
        );

    let http_client = reqwest::blocking::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Microsoft Graph supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        // This example requests read access to OneDrive.
        .add_scope(Scope::new(
            "https://graph.microsoft.com/Files.Read".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    println!("Open this URL in your browser:\n{authorize_url}\n");

    let (code, state) = {
        // A very naive implementation of the redirect server.
        let listener = TcpListener::bind("127.0.0.1:3003").unwrap();

        // The server will terminate itself after collecting the first code.
        let Some(mut stream) = listener.incoming().flatten().next() else {
            panic!("listener terminated without accepting a connection");
        };

        let mut reader = BufReader::new(&stream);

        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();

        let redirect_url = request_line.split_whitespace().nth(1).unwrap();
        let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();

        let code = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .map(|(_, code)| AuthorizationCode::new(code.into_owned()))
            .unwrap();

        let state = url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, state)| CsrfToken::new(state.into_owned()))
            .unwrap();

        let message = "Go back to your terminal :)";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
            message.len(),
            message
        );
        stream.write_all(response.as_bytes()).unwrap();

        (code, state)
    };

    println!("MS Graph returned the following code:\n{}\n", code.secret());
    println!(
        "MS Graph returned the following state:\n{} (expected `{}`)\n",
        state.secret(),
        csrf_state.secret()
    );

    // Exchange the code with a token.
    let token = client
        .exchange_code(code)
        // Send the PKCE code verifier in the token request
        .set_pkce_verifier(pkce_code_verifier)
        .request(&http_client);

    println!("MS Graph returned the following token:\n{token:?}\n");
}
