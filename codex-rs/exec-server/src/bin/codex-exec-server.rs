use clap::Parser;

#[derive(Debug, Parser)]
struct ExecServerArgs {
    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default).
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = codex_exec_server::DEFAULT_LISTEN_URL
    )]
    listen: String,
    // Hidden entrypoint used by LocalFileSystem to re-run the same fs logic
    // in a one-shot helper process after sandbox transformation.
    #[arg(long = "internal-fs-op", hide = true)]
    internal_fs_op: bool,
}

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let args = ExecServerArgs::parse();
        if args.internal_fs_op {
            // The helper mode reads a single structured fs request from stdin,
            // executes it, writes the response to stdout, and exits.
            codex_exec_server::run_internal_fs_op().await?;
        } else {
            codex_exec_server::run_main_with_listen_url(&args.listen)
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        }
        Ok(())
    })
}
