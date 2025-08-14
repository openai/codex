use std::path::PathBuf;

use codex_login::ServerOptions;
use codex_login::run_server_blocking;

fn main() {
    let codex_home = match std::env::var("CODEX_HOME") {
        Ok(v) => PathBuf::from(v),
        Err(_) => {
            eprintln!("ERROR: CODEX_HOME environment variable is not set");
            std::process::exit(1);
        }
    };

    let client_id = std::env::var("CODEX_CLIENT_ID").unwrap_or_else(|_| {
        // Mirror existing default
        codex_login::CLIENT_ID.to_string()
    });

    let opts = ServerOptions::new(&codex_home, &client_id);
    match run_server_blocking(opts) {
        Ok(()) => std::process::exit(0),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => std::process::exit(13),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1)
        }
    }
}
