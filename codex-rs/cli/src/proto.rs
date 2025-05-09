use std::io::IsTerminal;
use std::sync::Arc;

use clap::Parser;
use codex_core::Codex;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::Submission;
use codex_core::util::notify_on_sigint;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tracing::error;
use tracing::info;

#[derive(Debug, Parser)]
pub struct ProtoCli {}

pub async fn run_main(_opts: ProtoCli) -> anyhow::Result<()> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("Protocol mode expects stdin to be a pipe, not a terminal");
    }

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let config = Config::load_with_overrides(ConfigOverrides::default())?;
    let ctrl_c = notify_on_sigint();
    let (codex, _init_id) = Codex::spawn(config, ctrl_c.clone()).await?;
    let codex = Arc::new(codex);

    // Task that reads JSON lines from stdin and forwards to Submission Queue
    let sq_fut = {
        let codex = codex.clone();
        let ctrl_c = ctrl_c.clone();
        async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            loop {
                let result = tokio::select! {
                    _ = ctrl_c.notified() => {
                        info!("Interrupted, exiting");
                        break
                    },
                    res = lines.next_line() => res,
                };

                match result {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Submission>(line) {
                            Ok(sub) => {
                                if let Err(e) = codex.submit_with_id(sub).await {
                                    error!("{e:#}");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("invalid submission: {e}");
                            }
                        }
                    }
                    _ => {
                        info!("Submission queue closed");
                        break;
                    }
                }
            }
        }
    };

    // Task that reads events from the agent and prints them as JSON lines to stdout
    let eq_fut = async move {
        loop {
            let event = tokio::select! {
                _ = ctrl_c.notified() => break,
                event = codex.next_event() => event,
            };
            match event {
                Ok(event) => {
                    let event_str = match serde_json::to_string(&event) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize event: {e}");
                            continue;
                        }
                    };
                    println!("{event_str}");
                }
                Err(e) => {
                    error!("{e:#}");
                    break;
                }
            }
        }
        info!("Event queue closed");
    };

    tokio::join!(sq_fut, eq_fut);
    Ok(())
}
