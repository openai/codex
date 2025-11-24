//! An example showing off the usage of `Deserialize` to automatically decode
//! TOML into a Rust `struct`

#![deny(warnings)]
#![allow(dead_code)]

use serde::Deserialize;

/// This is what we're going to decode into. Each field is optional, meaning
/// that it doesn't have to be present in TOML.
#[derive(Debug, Deserialize)]
struct Config {
    global_string: Option<String>,
    global_integer: Option<u64>,
    server: Option<ServerConfig>,
    peers: Option<Vec<PeerConfig>>,
}

/// Sub-structs are decoded from tables, so this will decode from the `[server]`
/// table.
///
/// Again, each field is optional, meaning they don't have to be present.
#[derive(Debug, Deserialize)]
struct ServerConfig {
    ip: Option<String>,
    port: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PeerConfig {
    ip: Option<String>,
    port: Option<u64>,
}

fn main() {
    let toml_str = r#"
        global_string = "test"
        global_integer = 5

        [server]
        ip = "127.0.0.1"
        port = 80

        [[peers]]
        ip = "127.0.0.1"
        port = 8080

        [[peers]]
        ip = "127.0.0.1"
    "#;

    let decoded: Config = toml::from_str(toml_str).unwrap();
    println!("{decoded:#?}");
}
