#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::CellId;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::ExecuteRequest;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::ExecuteToPendingOutcome;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::FunctionCallOutputContentItem;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::InProcessCodeModeSession;
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
use codex_code_mode::RuntimeResponse;

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let provider = parse_provider()?;
    install_provider(provider)?;
    exercise_provider(provider)?;

    let session = InProcessCodeModeSession::new();
    let outcome = session
        .execute_to_pending(ExecuteRequest {
            tool_call_id: "ci_runtime_smoke".to_string(),
            enabled_tools: Vec::new(),
            source: r#"text("runtime smoke ok");"#.to_string(),
            yield_time_ms: Some(60_000),
            max_output_tokens: Some(1_000),
        })
        .await?;
    session.shutdown().await?;

    let expected = ExecuteToPendingOutcome::Completed(RuntimeResponse::Result {
        cell_id: CellId::new("1".to_string()),
        content_items: vec![FunctionCallOutputContentItem::InputText {
            text: "runtime smoke ok".to_string(),
        }],
        error_text: None,
    });
    if outcome != expected {
        return Err(format!("unexpected code-mode response: {outcome:?}"));
    }

    println!("code-mode runtime smoke passed with {provider}");
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
#[derive(Clone, Copy)]
enum Provider {
    AwsLc,
    Ring,
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
impl std::fmt::Display for Provider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AwsLc => formatter.write_str("aws-lc"),
            Self::Ring => formatter.write_str("ring"),
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn parse_provider() -> Result<Provider, String> {
    let mut args = std::env::args().skip(1);
    let provider = match args.next().as_deref() {
        Some("aws-lc") => Provider::AwsLc,
        Some("ring") => Provider::Ring,
        Some(provider) => return Err(format!("unsupported rustls provider: {provider}")),
        None => return Err("expected rustls provider argument: aws-lc or ring".to_string()),
    };
    if args.next().is_some() {
        return Err("expected exactly one rustls provider argument".to_string());
    }
    Ok(provider)
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn install_provider(provider: Provider) -> Result<(), String> {
    match provider {
        Provider::AwsLc => {
            codex_utils_rustls_provider::ensure_rustls_crypto_provider();
            Ok(())
        }
        Provider::Ring => rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| "failed to install ring rustls provider".to_string()),
    }
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn exercise_provider(provider: Provider) -> Result<(), String> {
    let installed = rustls::crypto::CryptoProvider::get_default()
        .ok_or_else(|| format!("{provider} rustls provider was not installed"))?;
    let key_exchange_group = installed
        .kx_groups
        .first()
        .ok_or_else(|| format!("{provider} rustls provider has no key exchange groups"))?;
    key_exchange_group
        .start()
        .map(|_| ())
        .map_err(|error| format!("{provider} key exchange initialization failed: {error}"))
}

#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
fn main() {
    eprintln!("code-mode runtime smoke is only supported on Intel macOS");
    std::process::exit(2);
}
