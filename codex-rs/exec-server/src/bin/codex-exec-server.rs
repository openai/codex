#[tokio::main]
async fn main() {
    if let Err(err) = codex_exec_server::run_main().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
