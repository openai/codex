use std::future::Future;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::task::Context;
use std::task::Waker;

use codex_model_provider_info::AwsAuthRefreshConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::provider::ModelProvider;
use crate::provider::ProviderUnauthorizedRecovery;

use super::super::AmazonBedrockModelProvider;
use super::super::AwsAuthRecovery;
use super::*;

const SECRET: &str = "credential-export-secret";
const VALID_OUTPUT: &[u8] = br#"{"AccessKeyId":"exported","SecretAccessKey":"credential-export-secret","SessionToken":"credential-export-secret"}"#;

fn exporter_fixture(
    output: &[u8],
    mode: &str,
) -> io::Result<(tempfile::TempDir, AwsCredentialExport)> {
    let directory = tempfile::tempdir()?;
    let output_path = directory.path().join("output.json");
    std::fs::write(&output_path, output)?;

    #[cfg(unix)]
    let (command, mut args) = {
        let script = directory.path().join("export.sh");
        std::fs::write(
            &script,
            r#"printf 'invoked\n' >> "$1"
printf '%s\n' "$4" >&2
cat "$2"
case "$3" in
  fail) exit 7 ;;
  wait) while :; do :; done ;;
esac
"#,
        )?;
        (
            "sh".to_string(),
            vec![script.to_string_lossy().into_owned()],
        )
    };
    #[cfg(windows)]
    let (command, mut args) = {
        let script = directory.path().join("export.cmd");
        std::fs::write(
            &script,
            "@echo off\r\n>> \"%~1\" echo invoked\r\n>&2 echo %~4\r\ntype \"%~2\"\r\nif \"%~3\"==\"fail\" exit /b 7\r\nif \"%~3\"==\"wait\" goto wait\r\nexit /b 0\r\n:wait\r\ngoto wait\r\n",
        )?;
        (
            "cmd.exe".to_string(),
            vec![
                "/D".to_string(),
                "/Q".to_string(),
                "/C".to_string(),
                script.to_string_lossy().into_owned(),
            ],
        )
    };
    args.extend([
        directory
            .path()
            .join("invocations")
            .to_string_lossy()
            .into_owned(),
        output_path.to_string_lossy().into_owned(),
        mode.to_string(),
        SECRET.to_string(),
    ]);
    Ok((
        directory,
        AwsCredentialExport::new(AwsCredentialExportConfig {
            command,
            args: args.into_iter().map(Into::into).collect(),
            timeout_ms: NonZeroU64::new(5_000).expect("non-zero timeout"),
        }),
    ))
}

#[test]
fn credential_output_formats_expiration_and_sanitized_errors() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    for (optional_fields, refresh_after) in [
        (json!({}), 3_600),
        (json!({"SessionToken": null, "Expiration": null}), 3_600),
        (
            json!({"SessionToken": SECRET, "Expiration": "2023-11-14T23:13:20+00:00"}),
            3_300,
        ),
        (
            json!({"SessionToken": SECRET, "Expiration": "2023-11-14T22:18:20Z"}),
            0,
        ),
        (json!({"Expiration": "2023-11-14T22:15:20Z"}), 0),
    ] {
        let mut credentials = json!({"AccessKeyId": "exported", "SecretAccessKey": SECRET});
        credentials
            .as_object_mut()
            .unwrap()
            .extend(optional_fields.as_object().unwrap().clone());
        let mut process_output = credentials.clone();
        process_output["Version"] = json!(1);
        for output in [
            process_output,
            json!({"Credentials": credentials, "AssumedRoleUser": {"Arn": "test-role"}}),
        ] {
            let cached = CachedCredentials::from_output(output.to_string().as_bytes(), now)
                .expect("valid credentials should parse");
            assert_eq!(
                (cached.access_keys, cached.refresh_at),
                (
                    AwsAccessKeys {
                        access_key_id: "exported".to_string(),
                        secret_access_key: SECRET.to_string(),
                        session_token: optional_fields
                            .get("SessionToken")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                    },
                    now + Duration::from_secs(refresh_after)
                )
            );
        }
    }

    let mut invalid_outputs = vec![(
        format!("{{\"SecretAccessKey\":\"{SECRET}\""),
        io::ErrorKind::InvalidData,
    )];
    for (field, value) in [
        ("AccessKeyId", None),
        ("SecretAccessKey", None),
        ("AccessKeyId", Some(json!(" "))),
        ("SecretAccessKey", Some(json!(" "))),
        ("AccessKeyId", Some(json!([SECRET]))),
        ("SecretAccessKey", Some(json!([SECRET]))),
        ("SessionToken", Some(json!([SECRET]))),
        ("Expiration", Some(json!([SECRET]))),
        ("Expiration", Some(json!(SECRET))),
    ] {
        let mut credentials: serde_json::Value = serde_json::from_slice(VALID_OUTPUT).unwrap();
        match value {
            Some(value) => {
                credentials[field] = value;
            }
            None => {
                credentials.as_object_mut().unwrap().remove(field);
            }
        }
        invalid_outputs.extend(
            [
                credentials.to_string(),
                json!({"Credentials": credentials}).to_string(),
            ]
            .map(|output| (output, io::ErrorKind::InvalidData)),
        );
    }
    for expiration in ["2023-11-14T22:13:20Z", "2023-11-14T22:00:00Z"] {
        let credentials =
            json!({"AccessKeyId": "exported", "SecretAccessKey": SECRET, "Expiration": expiration});
        invalid_outputs.extend(
            [
                credentials.to_string(),
                json!({"Credentials": credentials}).to_string(),
            ]
            .map(|output| (output, io::ErrorKind::Other)),
        );
    }
    for (output, expected_kind) in invalid_outputs {
        let error = CachedCredentials::from_output(output.as_bytes(), now)
            .expect_err("invalid credentials should be rejected");
        assert_eq!(error.kind(), expected_kind);
        assert!(!error.to_string().contains(SECRET));
    }
}

#[tokio::test]
async fn credential_export_process_errors_are_bounded_and_sanitized() -> io::Result<()> {
    let oversized = format!(
        "{SECRET}{}",
        "x".repeat(MAX_CREDENTIAL_OUTPUT_BYTES as usize)
    );
    for (mode, output, timeout_ms, expected_kind) in [
        ("missing", VALID_OUTPUT, 5_000, io::ErrorKind::NotFound),
        ("relative", VALID_OUTPUT, 5_000, io::ErrorKind::InvalidInput),
        (
            "normalized-relative",
            VALID_OUTPUT,
            5_000,
            io::ErrorKind::InvalidInput,
        ),
        ("fail", VALID_OUTPUT, 5_000, io::ErrorKind::Other),
        ("wait", VALID_OUTPUT, 100, io::ErrorKind::TimedOut),
        (
            "wait",
            oversized.as_bytes(),
            5_000,
            io::ErrorKind::InvalidData,
        ),
    ] {
        let (directory, mut exporter) = exporter_fixture(output, mode)?;
        exporter.config.timeout_ms = NonZeroU64::new(timeout_ms).expect("non-zero timeout");
        if mode == "missing" {
            exporter.config.command = directory
                .path()
                .join("missing")
                .to_string_lossy()
                .into_owned();
        } else if mode == "relative" {
            exporter.config.command = std::path::Path::new(".")
                .join("export-credentials")
                .to_string_lossy()
                .into_owned();
        } else if mode == "normalized-relative" {
            exporter.config.command = std::path::Path::new("scripts")
                .join(".")
                .to_string_lossy()
                .into_owned();
        }
        let error = exporter
            .credentials()
            .await
            .expect_err("export should fail");
        assert_eq!(error.kind(), expected_kind);
        assert!(!error.to_string().contains(SECRET));
        assert!(!format!("{exporter:?}").contains(SECRET));
        assert!(exporter.cached.lock().await.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn credential_export_reuses_cache_and_coalesces_refreshes() -> io::Result<()> {
    let mut output = VALID_OUTPUT.to_vec();
    output.resize(MAX_CREDENTIAL_OUTPUT_BYTES as usize, b' ');
    let (directory, exporter) = exporter_fixture(&output, "success")?;
    let original = AwsAccessKeys {
        access_key_id: "exported".to_string(),
        secret_access_key: SECRET.to_string(),
        session_token: Some(SECRET.to_string()),
    };
    let (first, second) = tokio::join!(exporter.credentials(), exporter.credentials());
    assert_eq!([first?, second?], [original.clone(), original.clone()]);
    let rotated = json!({"AccessKeyId": "rotated", "SecretAccessKey": SECRET});
    std::fs::write(directory.path().join("output.json"), rotated.to_string())?;
    assert_eq!(exporter.credentials().await?, original);

    exporter
        .cached
        .lock()
        .await
        .as_mut()
        .expect("populated cache")
        .refresh_at = SystemTime::UNIX_EPOCH;
    let rotated = AwsAccessKeys {
        access_key_id: "rotated".to_string(),
        secret_access_key: SECRET.to_string(),
        session_token: None,
    };
    assert_eq!(exporter.credentials().await?, rotated);

    let mut provider = AmazonBedrockModelProvider::new(
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: Some("us-west-2".to_string()),
            credential_export: Some(exporter.config.clone()),
            auth_refresh: None,
        })),
        /*auth_manager*/ None,
    );
    let exporter = Arc::new(exporter);
    provider.credential_export = Some(exporter.clone());
    let controller = codex_http_client::NetworkPolicyController::default();
    let policy = controller.policy();
    provider.http_client_factory = provider
        .http_client_factory
        .clone()
        .with_network_policy(policy.clone());
    controller.publish(
        policy.revision(),
        codex_http_client::DestinationPolicy::Restricted {
            allowed_hosts: Default::default(),
        },
    );
    let denied = provider
        .recover_from_unauthorized()
        .await
        .expect_err("restricted recovery must not launch the exporter");
    assert_eq!(denied.retry_delay(/*retry_count*/ 1), None);
    assert_eq!(
        denied.to_string(),
        "Fatal error: this SDK transport is disabled by application network restrictions"
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("invocations"))?
            .lines()
            .collect::<Vec<_>>(),
        vec!["invoked", "invoked"]
    );
    controller.publish(
        policy.revision(),
        codex_http_client::DestinationPolicy::Unrestricted,
    );
    {
        let first = provider.recover_from_unauthorized();
        let second = provider.recover_from_unauthorized();
        tokio::pin!(first, second);
        {
            // Both callers must observe the same generation before either can export.
            let _cached = exporter.cached.lock().await;
            let mut context = Context::from_waker(Waker::noop());
            assert!(first.as_mut().poll(&mut context).is_pending());
            assert!(second.as_mut().poll(&mut context).is_pending());
        }
        let (first, second) = tokio::join!(first, second);
        assert_eq!(
            [
                first.expect("first recovery"),
                second.expect("second recovery")
            ],
            [ProviderUnauthorizedRecovery::Recovered; 2],
        );
    }
    assert_eq!(exporter.credentials().await?, rotated);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("invocations"))?
            .lines()
            .collect::<Vec<_>>(),
        vec!["invoked", "invoked", "invoked"]
    );
    assert!(!format!("{exporter:?}").contains(SECRET));

    provider.auth_recovery = Some(Arc::new(AwsAuthRecovery::new(AwsAuthRefreshConfig {
        command: "not-aws".to_string(),
        args: Vec::new(),
        timeout_ms: NonZeroU64::new(5_000).expect("non-zero timeout"),
    })));
    let credentials = exporter.credentials();
    let refresh = provider.recover_from_unauthorized();
    tokio::pin!(credentials, refresh);
    {
        // A normal export completing must not make a queued login recovery look completed.
        let mut cached = exporter.cached.lock().await;
        cached.as_mut().expect("populated cache").refresh_at = SystemTime::UNIX_EPOCH;
        let mut context = Context::from_waker(Waker::noop());
        assert!(credentials.as_mut().poll(&mut context).is_pending());
        assert!(refresh.as_mut().poll(&mut context).is_pending());
    }
    let (credentials, refresh) = tokio::join!(credentials, refresh);
    assert_eq!(credentials?, rotated);
    assert_eq!(
        refresh
            .expect_err("queued login recovery must still run")
            .retry_delay(/*retry_count*/ 1),
        None
    );
    assert!(exporter.cached.lock().await.is_none());

    // Cancellation also leaves the invalidated cache empty and releases the guard.
    exporter.credentials().await?;
    let refresh = exporter.begin_refresh().await.expect("new recovery");
    let credentials = exporter.credentials();
    tokio::pin!(credentials);
    assert!(
        credentials
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(refresh);
    assert_eq!(credentials.await?, rotated);
    Ok(())
}
