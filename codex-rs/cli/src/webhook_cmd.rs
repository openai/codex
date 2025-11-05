//! Webhook command for triggering external integrations.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::integrations::WebhookClient;
use codex_core::integrations::WebhookPayload;
use codex_core::integrations::WebhookService;
use serde_json::Value;
use std::io::Read;

/// Send webhook notifications to external services (GitHub, Slack, Custom)
#[derive(Debug, Parser)]
pub struct WebhookCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: WebhookSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum WebhookSubcommand {
    /// Send a GitHub API request
    Github(GithubArgs),

    /// Send a Slack webhook notification
    Slack(SlackArgs),

    /// Send a custom webhook
    Custom(CustomArgs),
}

#[derive(Debug, Parser)]
pub struct GithubArgs {
    /// GitHub API endpoint (e.g., "repos/owner/repo/issues")
    #[arg(long, short)]
    pub endpoint: String,

    /// JSON payload (or use stdin with -)
    #[arg(long, short)]
    pub data: Option<String>,

    /// Read payload from stdin
    #[arg(long)]
    pub stdin: bool,
}

#[derive(Debug, Parser)]
pub struct SlackArgs {
    /// Slack message text
    #[arg(long, short)]
    pub text: String,

    /// Optional channel override
    #[arg(long, short)]
    pub channel: Option<String>,

    /// Additional JSON data
    #[arg(long, short)]
    pub data: Option<String>,
}

#[derive(Debug, Parser)]
pub struct CustomArgs {
    /// Custom webhook URL
    #[arg(long, short)]
    pub url: String,

    /// JSON payload (or use stdin with -)
    #[arg(long, short)]
    pub data: Option<String>,

    /// Read payload from stdin
    #[arg(long)]
    pub stdin: bool,

    /// Custom headers in format "Key: Value" (repeatable)
    #[arg(long = "header", short = 'H')]
    pub headers: Vec<String>,
}

pub async fn run(cli: WebhookCli) -> Result<()> {
    cli.config_overrides
        .parse_overrides()
        .map_err(|e| anyhow::anyhow!(e))?;

    match cli.subcommand {
        WebhookSubcommand::Github(args) => run_github(args).await,
        WebhookSubcommand::Slack(args) => run_slack(args).await,
        WebhookSubcommand::Custom(args) => run_custom(args).await,
    }
}

async fn run_github(args: GithubArgs) -> Result<()> {
    // Verify GITHUB_TOKEN is set (WebhookClient will use it)
    std::env::var("GITHUB_TOKEN")
        .context("GITHUB_TOKEN environment variable not set. Please set it with your GitHub Personal Access Token.")?;

    println!("🔗 Sending GitHub API request to: {}", args.endpoint);

    let data: Value = if args.stdin {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        serde_json::from_str(&buffer).context("Invalid JSON from stdin")?
    } else if let Some(data_str) = args.data {
        serde_json::from_str(&data_str).context("Invalid JSON data")?
    } else {
        bail!("Either --data or --stdin must be provided");
    };

    let payload = WebhookPayload {
        service: WebhookService::GitHub,
        action: args.endpoint,
        data,
        headers: None,
    };

    let client = WebhookClient::new();
    let response = client.execute(payload).await?;

    if response.success {
        println!("✅ GitHub API call succeeded (status: {})", response.status);
        if let Some(body) = response.body {
            println!("\nResponse:");
            println!("{}", serde_json::to_string_pretty(&body)?);
        }
    } else {
        eprintln!("❌ GitHub API call failed (status: {})", response.status);
        eprintln!("Response: {}", response.text);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_slack(args: SlackArgs) -> Result<()> {
    let webhook_url = std::env::var("SLACK_WEBHOOK_URL")
        .context("SLACK_WEBHOOK_URL environment variable not set. Please set it with your Slack Incoming Webhook URL.")?;

    println!("📢 Sending Slack notification...");

    let mut data = serde_json::json!({
        "text": args.text,
    });

    if let Some(channel) = args.channel {
        data["channel"] = Value::String(channel);
    }

    if let Some(extra_data) = args.data {
        let extra: Value = serde_json::from_str(&extra_data).context("Invalid JSON data")?;
        if let (Some(obj), Some(extra_obj)) = (data.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }

    let payload = WebhookPayload {
        service: WebhookService::Slack,
        action: webhook_url,
        data,
        headers: None,
    };

    let client = WebhookClient::new();
    let response = client.execute(payload).await?;

    if response.success {
        println!("✅ Slack notification sent successfully");
    } else {
        eprintln!("❌ Slack notification failed (status: {})", response.status);
        eprintln!("Response: {}", response.text);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_custom(args: CustomArgs) -> Result<()> {
    println!("🔗 Sending custom webhook to: {}", args.url);

    let data: Value = if args.stdin {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        serde_json::from_str(&buffer).context("Invalid JSON from stdin")?
    } else if let Some(data_str) = args.data {
        serde_json::from_str(&data_str).context("Invalid JSON data")?
    } else {
        bail!("Either --data or --stdin must be provided");
    };

    // Parse custom headers
    let mut headers_map = serde_json::Map::new();
    for header in args.headers {
        if let Some((key, value)) = header.split_once(':') {
            headers_map.insert(
                key.trim().to_string(),
                Value::String(value.trim().to_string()),
            );
        }
    }

    let payload = WebhookPayload {
        service: WebhookService::Custom,
        action: args.url,
        data,
        headers: if headers_map.is_empty() {
            None
        } else {
            Some(headers_map)
        },
    };

    let client = WebhookClient::new();
    let response = client.execute(payload).await?;

    if response.success {
        println!("✅ Custom webhook succeeded (status: {})", response.status);
        if !response.text.is_empty() {
            println!("\nResponse:");
            println!("{}", response.text);
        }
    } else {
        eprintln!("❌ Custom webhook failed (status: {})", response.status);
        eprintln!("Response: {}", response.text);
        std::process::exit(1);
    }

    Ok(())
}
