use anyhow::Context;
use anyhow::Result;
use std::ffi::OsStr;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let executable = args
        .next()
        .context("usage: wine-test-exec [--powershell | <windows-executable>] [args...]")?;
    if executable.as_os_str() == OsStr::new("--powershell") {
        let args = args
            .map(|arg| {
                arg.into_string()
                    .map_err(|arg| anyhow::anyhow!("PowerShell argument is not UTF-8: {arg:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let output = wine_test_support::run_ambient_powershell(&args).await?;
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&output.stdout)?;
        stdout.flush()?;
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(&output.stderr)?;
        stderr.flush()?;
        if output.exit_code != 0 {
            std::process::exit(output.exit_code);
        }
    } else {
        let status = wine_test_support::ambient_wine_command(executable)?
            .args(args)
            .status()
            .context("run Windows command in shared Wine test prefix")?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    }
    Ok(())
}
