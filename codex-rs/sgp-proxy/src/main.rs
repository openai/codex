use clap::Parser;
use codex_sgp_proxy::Args;

#[ctor::ctor]
fn pre_main() {
    codex_process_hardening::pre_main_hardening();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    codex_sgp_proxy::run_main(args).await
}
