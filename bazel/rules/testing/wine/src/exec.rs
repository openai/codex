use anyhow::Context;
use anyhow::Result;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let executable = args
        .next()
        .context("usage: wine-test-exec <windows-executable> [args...]")?;
    let status = wine_test_support::ambient_wine_command(executable)?
        .args(args)
        .status()
        .context("run Windows command in shared Wine test prefix")?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
