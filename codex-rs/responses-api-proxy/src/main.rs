use anyhow::Context;
use clap::Parser;
use codex_arg0::arg0_dispatch_or_else;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;

pub fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|_codex_linux_sandbox_exe| async move {
        let args = ResponsesApiProxyArgs::parse();
        tokio::task::spawn_blocking(move || codex_responses_api_proxy::run_main(args))
            .await
            .context("responses-api-proxy blocking task panicked")??;
        Ok(())
    })
}
