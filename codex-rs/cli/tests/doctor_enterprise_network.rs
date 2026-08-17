#[cfg(target_os = "macos")]
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context as _;
use anyhow::Result;
#[cfg(target_os = "macos")]
use pretty_assertions::assert_eq;
use serde_json::Value;
#[cfg(target_os = "macos")]
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_custom_ca_falls_back_to_system_roots() -> Result<()> {
    let server = MockServer::start().await;
    for (request_method, request_path) in [("HEAD", "/v1/responses"), ("GET", "/v1/models")] {
        Mock::given(method(request_method))
            .and(path(request_path))
            .respond_with(ResponseTemplate::new(200))
            .expect(2)
            .mount(&server)
            .await;
    }

    let codex_home = TempDir::new()?;
    let certificate = codex_home.path().join("invalid-ca.pem");
    std::fs::write(&certificate, "not a certificate")?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "model_provider = \"local\"\n[model_providers.local]\nname = \"local\"\nbase_url = \"{}/v1\"\nwire_api = \"responses\"\n",
            server.uri()
        ),
    )?;
    for sandbox in [None, Some("seatbelt")] {
        let mut command = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
        command
            .args(["doctor", "--json"])
            .env("CODEX_HOME", codex_home.path())
            .env("CODEX_CA_CERTIFICATE", &certificate)
            .stdin(Stdio::null());
        if let Some(sandbox) = sandbox {
            command
                .env("CODEX_SANDBOX", sandbox)
                .env("HTTP_PROXY", "http://127.0.0.1:1")
                .env("http_proxy", "http://127.0.0.1:1")
                .env("HTTPS_PROXY", "http://127.0.0.1:1")
                .env("https_proxy", "http://127.0.0.1:1")
                .env("NO_PROXY", "")
                .env("no_proxy", "");
        } else {
            command
                .env("NO_PROXY", "127.0.0.1,localhost")
                .env("no_proxy", "127.0.0.1,localhost");
        }
        let output = command
            .output()
            .context("failed to run the doctor with an invalid custom CA")?;
        let report: Value = serde_json::from_slice(&output.stdout)?;

        assert!(
            report["checks"]["network.provider_reachability"]["details"]["local API inference URL"]
                .as_str()
                .is_some_and(|detail| detail.ends_with("reachable (HTTP 200)"))
        );
    }
    server.verify().await;

    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn doctor_reports_macos_system_proxy_configuration_and_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let report = doctor_report(codex_home.path())?;
    let details = &report["checks"]["network.env"]["details"];

    assert_eq!(details["respect system proxy"], json!("disabled"));
    assert!(matches!(
        details["system proxy"].as_str(),
        Some("automatic (PAC)" | "manual" | "direct" | "unavailable")
    ));

    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nrespect_system_proxy = true\n",
    )?;
    let report = doctor_report(codex_home.path())?;
    assert_eq!(
        report["checks"]["network.env"]["details"]["respect system proxy"],
        json!("enabled")
    );

    Ok(())
}

#[cfg(target_os = "macos")]
fn doctor_report(codex_home: &Path) -> Result<Value> {
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["doctor", "--json"])
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::null())
        .output()
        .context("failed to run the doctor")?;

    serde_json::from_slice(&output.stdout).context("doctor did not emit a valid json report")
}
