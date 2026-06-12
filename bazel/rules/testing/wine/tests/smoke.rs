#[cfg(not(target_os = "linux"))]
compile_error!("the Wine smoke test can only run on Linux");

use anyhow::Result;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use wine_test_support::WineTestCommand;

#[tokio::test]
async fn runs_basic_windows_executable_under_wine() -> Result<()> {
    let executable = codex_utils_cargo_bin::cargo_bin("wine-smoke")?;
    let mut process = WineTestCommand::new(executable).spawn()?;
    let mut stdout = process.take_stdout();

    process
        .scope(async move {
            let mut output = String::new();
            stdout.read_to_string(&mut output).await?;
            assert_eq!(output.trim(), "WINE_TEST_READY");
            Ok(())
        })
        .await
}
