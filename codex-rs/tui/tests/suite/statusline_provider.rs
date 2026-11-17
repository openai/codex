#![cfg(test)]
//! A real CLI, app server and stdio MCP provider exercise the selected resource end to end.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

fn test_home(cwd: &Path) -> Result<TempDir> {
    let home = tempfile::tempdir_in("/tmp")?;
    write_test_config(home.path(), cwd)?;
    let config = home.path().join("config.toml");
    let existing = fs::read_to_string(&config)?;
    fs::write(
        config,
        format!("features.tui_extensions_statusline = true\n{existing}"),
    )?;
    Ok(home)
}

fn register_provider(config: &Path, receipt: &Path, barrier: &Path) -> Result<()> {
    let registration = json!({"mcp_servers":{"fixture":{
        "command":core_test_support::stdio_server_bin()?,
        "env":{"MCP_TEST_STATUSLINE_RECORD":receipt, "MCP_TEST_INITIALIZE_BARRIER_FILE":barrier}
    }}});
    writeln!(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(config)?,
        "\n{}",
        toml::to_string(&registration)?
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setup {
    Embedded,
    Remote,
    ProjectShadow,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn statusline_reads_the_selected_stdio_resource_on_the_displayed_thread() -> Result<()> {
    for mode in [Setup::Embedded, Setup::Remote, Setup::ProjectShadow] {
        let work = tempfile::tempdir_in("/tmp")?;
        let cwd = work.path().canonicalize()?;
        fs::create_dir(cwd.join(".git"))?;
        let client = test_home(&cwd)?;
        let registration_home = test_home(&cwd)?;
        let host = if mode == Setup::Remote {
            registration_home.path()
        } else {
            client.path()
        };
        let receipt = cwd.join("resource-read.jsonl");
        let shadow = cwd.join("project-read.jsonl");
        let barrier = cwd.join("provider-ready");
        register_provider(&host.join("config.toml"), &receipt, &barrier)?;
        let preference = "[tui]\ndisable_paste_burst = true\nstatus_line = []\nstatus_line_provider = { server='fixture', tool='sandbox_meta' }";
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(client.path().join("config.toml"))?,
            "\n{preference}"
        )?;
        if mode == Setup::ProjectShadow {
            let project = cwd.join(".codex");
            fs::create_dir(&project)?;
            register_provider(&project.join("config.toml"), &shadow, &barrier)?;
        }
        let mut daemon = None;
        let endpoint;
        let args = if mode == Setup::Remote {
            let (child, socket) = super::unix_daemon::start(host, &[]).await?;
            daemon = Some(child);
            endpoint = socket;
            vec!["--remote", endpoint.as_str()]
        } else {
            vec!["--no-daemon"]
        };
        let mut terminal = PtyCodex::start_cli(&cwd, client, &args)?;
        terminal
            .wait_for_screen("Initializing…")
            .with_context(|| format!("{mode:?} initialization"))?;
        assert!(!receipt.exists());
        if mode == Setup::Embedded {
            terminal.write_input(b"/raw on\r")?;
            terminal.wait_for_screen("Raw output mode on")?;
        }
        fs::write(&barrier, "")?;
        if mode == Setup::ProjectShadow {
            terminal.wait_for_screen("Error loading statusline contents from sandbox_meta")?;
            assert!(!receipt.exists());
            assert!(!shadow.exists());
        } else {
            terminal
                .wait_for_screen("Provider thread ")
                .with_context(|| format!("{mode:?} resource read"))?;
            let receipts = fs::read_to_string(&receipt)?;
            let line = receipts
                .lines()
                .next()
                .context("missing first resource read")?;
            let state: Value = serde_json::from_str(line)?;
            let thread = state["thread"]["id"]
                .as_str()
                .context("missing displayed thread ID")?;
            terminal.wait_for_screen(&format!("Provider thread {thread}"))?;
            terminal.write_input(b"/status\r")?;
            terminal.wait_for_screen("Session:")?;
            let status = terminal.screen_contents();
            let session = status
                .lines()
                .find(|line| line.contains("Session:"))
                .context("missing Session row")?;
            assert!(
                session.contains(thread),
                "{session} should identify {thread}"
            );
            if mode == Setup::Embedded {
                assert_eq!(state["app"]["display_mode"], "raw");
                terminal.write_input(b"/raw off\r")?;
                terminal.wait_for_screen("Raw output mode off")?;
                terminal.wait_for_screen(&format!("Provider thread {thread} (standard)"))?;
            }
            assert_eq!(state["thread"]["workspace"], json!({"cwd": cwd}));
        }
        drop(terminal);
        if let Some(mut child) = daemon {
            child.kill().await?;
            child.wait().await?;
        }
    }
    Ok(())
}
