use anyhow::Result;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::{
    chat_completions::stream_chat_completions,
    client_common::{Prompt, ResponseEvent},
    config::Config,
    model_family,
};
use codex_protocol::models::{ContentItem, ResponseItem};
use futures::StreamExt;
use std::process::Command;
use std::{fs, thread, time::Duration};

#[derive(Debug, Parser)]
pub struct AmbientCommand {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,
}

pub async fn run_main(cmd: AmbientCommand) -> Result<()> {
    println!("Ambient agent started. Monitoring for changes... (Press Ctrl+C to stop)");

    // Load the codex config
    let cli_overrides = cmd.config_overrides.parse_overrides().map_err(|e| anyhow::anyhow!(e))?;
    let config = Config::load_with_cli_overrides(cli_overrides, Default::default())?;

    let client = reqwest::Client::new();

    // Get the Ollama provider info from the config
    let provider = config
        .model_providers
        .get(&config.model_provider_id)
        .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", config.model_provider_id))?;

    loop {
        // Execute git status --porcelain
        let output = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .output()?;

        if !output.stdout.is_empty() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.trim().lines().collect();

            println!("[{}] Found {} changed file(s).", chrono::Local::now().to_rfc2822(), lines.len());

            if let Some(first_line) = lines.first() {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() == 2 {
                    let file_path = parts[1];
                    println!("Analyzing: {file_path}");

                    match fs::read_to_string(file_path) {
                        Ok(content) => {
                            let model_family = model_family::find_family_for_model(&config.model)
                                .ok_or_else(|| anyhow::anyhow!("Model family not found for: {}", config.model))?;

                            let prompt_text = format!(
                                "You are an ambient AI assistant. Briefly summarize the change in this file:\n\n---\n\n{content}"
                            );

                            let user_message = ResponseItem::Message {
                                id: None,
                                role: "user".to_string(),
                                content: vec![ContentItem::InputText { text: prompt_text }],
                            };

                            let prompt = Prompt {
                                input: vec![user_message],
                                store: false,
                                tools: vec![],
                                base_instructions_override: None,
                            };

                            let mut stream = stream_chat_completions(&prompt, &model_family, &client, provider).await?;

                            println!("AI Insight:");
                            while let Some(event) = stream.next().await {
                                match event {
                                    Ok(ResponseEvent::OutputTextDelta(delta)) => {
                                        print!("{delta}");
                                    }
                                    Ok(ResponseEvent::Completed { .. }) => {
                                        println!("\n---");
                                        break;
                                    }
                                    Err(e) => {
                                        eprintln!("\nError processing stream: {e:?}");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read file {file_path}: {e}");
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_secs(10));
    }
}
