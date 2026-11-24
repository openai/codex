#![cfg(test)]

use super::*;
use crate::test_support::auth_headers;
use crate::test_support::fixture;
use crate::test_support::key_request;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn signed_fixture() -> (Vec<u8>, Value) {
    let fixture = fixture();
    let archive = STANDARD
        .decode(fixture["archive_base64"].as_str().expect("archive base64"))
        .expect("valid archive base64");
    (archive, fixture)
}

fn download(signing: &Value) -> RemotePluginArchiveDownload {
    serde_json::from_value(json!({"signing": signing})).unwrap()
}

async fn unexpected_key_request(
    url: String,
) -> Result<(RequestBuilder, HeaderMap), ArchiveVerificationFailure> {
    panic!("unexpected signing-key request to {url}");
}

async fn server_with_keys(keys: Value) -> (MockServer, String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(keys))
        .expect(1)
        .mount(&server)
        .await;
    let base_url = format!("{}/backend-api", server.uri());
    (server, base_url)
}

#[tokio::test]
async fn verifies_production_compatible_archive_and_rejects_tampering() {
    let cache = SigningKeyCache::default();
    let (archive, fixture) = signed_fixture();
    // Legacy servers included eval keys in the same list as marketplace keys for all callers.
    let mut keys = fixture["signing_keys_response"].clone();
    let mut eval_key = keys["keys"][0].clone();
    eval_key["key_id"] = json!("old-eval-key");
    eval_key["purpose"] = json!("EVAL_PLUGIN_RELEASE_ARCHIVE_SIGNING");
    keys["keys"].as_array_mut().unwrap().push(eval_key.clone());
    eval_key["key_id"] = json!("scoped-eval-key");
    keys["eval_keys"] = json!([eval_key]);
    let (_server, base_url) = server_with_keys(keys).await;
    let auth = auth_headers("user-token", "account");
    let metadata = download(&fixture["signing"]);

    assert_eq!(
        metadata
            .verify(
                archive.clone(),
                &base_url,
                &cache,
                key_request(auth.clone())
            )
            .await,
        Ok((archive.clone(), PluginArchiveVerification::Verified))
    );
    let redirector = MockServer::start().await;
    let (_target, target_url) = server_with_keys(fixture["signing_keys_response"].clone()).await;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/signing-keys"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{target_url}/ps/plugins/signing-keys")),
        )
        .expect(1)
        .mount(&redirector)
        .await;
    // The redirected server returns the exact key that verifies this archive.
    assert_eq!(
        metadata
            .verify(
                archive.clone(),
                &redirector.uri(),
                &cache,
                key_request(auth.clone())
            )
            .await,
        Err(ArchiveVerificationFailure::SigningKeysUnavailable)
    );

    let mut eval_signing = fixture["signing"].clone();
    eval_signing["key_id"] = json!("old-eval-key");
    assert_eq!(
        download(&eval_signing)
            .verify(
                archive.clone(),
                &base_url,
                &cache,
                key_request(auth.clone())
            )
            .await,
        Err(ArchiveVerificationFailure::UnsupportedKeyPurpose)
    );
    eval_signing["key_id"] = json!("scoped-eval-key");
    assert_eq!(
        download(&eval_signing)
            .verify(
                archive.clone(),
                &base_url,
                &cache,
                key_request(auth.clone())
            )
            .await,
        Ok((archive.clone(), PluginArchiveVerification::Verified))
    );
    let mut altered_archive = archive.clone();
    altered_archive[0] ^= 1;
    assert_eq!(
        metadata
            .verify(
                altered_archive.clone(),
                &base_url,
                &cache,
                unexpected_key_request
            )
            .await,
        Err(ArchiveVerificationFailure::DigestMismatch)
    );
    let mut altered_signing = fixture["signing"].clone();
    altered_signing["sha256"] = json!(hex::encode(digest(&SHA256, &altered_archive)));
    assert_eq!(
        download(&altered_signing)
            .verify(
                altered_archive,
                &base_url,
                &cache,
                key_request(auth.clone())
            )
            .await,
        Err(ArchiveVerificationFailure::SignatureMismatch)
    );
}

#[tokio::test]
async fn rejects_malformed_and_unsupported_signing_metadata() {
    let cache = SigningKeyCache::default();
    let (archive, fixture) = signed_fixture();
    for (field, value, failure) in [
        (
            "sha256",
            json!("A".repeat(64)),
            ArchiveVerificationFailure::MalformedMetadata,
        ),
        (
            "signature",
            json!("not base64"),
            ArchiveVerificationFailure::MalformedMetadata,
        ),
        (
            "key_id",
            json!(""),
            ArchiveVerificationFailure::MalformedMetadata,
        ),
        (
            "algorithm",
            json!("RSA_PSS_SHA256"),
            ArchiveVerificationFailure::UnsupportedArchiveAlgorithm,
        ),
    ] {
        let mut signing = fixture["signing"].clone();
        signing[field] = value;
        assert_eq!(
            download(&signing)
                .verify(archive.clone(), "", &cache, unexpected_key_request)
                .await,
            Err(failure)
        );
    }
}

#[tokio::test]
async fn applies_release_signing_presence_at_download() {
    let cache = SigningKeyCache::default();
    let (archive, _) = signed_fixture();
    for (raw, expected) in [
        (
            r#"{}"#,
            Ok((archive.clone(), PluginArchiveVerification::NotRequired)),
        ),
        (
            r#"{"signing":null}"#,
            Ok((archive.clone(), PluginArchiveVerification::NotRequired)),
        ),
        (
            r#"{"signing":{}}"#,
            Err(ArchiveVerificationFailure::MalformedMetadata),
        ),
        (
            r#"{"signing":true}"#,
            Err(ArchiveVerificationFailure::MalformedMetadata),
        ),
    ] {
        let download: RemotePluginArchiveDownload =
            serde_json::from_str(raw).expect("verification failures are deferred until download");
        assert_eq!(
            download
                .verify(archive.clone(), "", &cache, unexpected_key_request)
                .await,
            expected
        );
    }
}

#[tokio::test]
async fn rejects_keys_with_unsupported_purpose_algorithm_or_invalid_material() {
    let (archive, fixture) = signed_fixture();
    let auth = auth_headers("user-token", "account");
    for (field, value, failure) in [
        (
            "purpose",
            "OTHER_PURPOSE",
            ArchiveVerificationFailure::UnsupportedKeyPurpose,
        ),
        (
            "algorithm",
            "RSA_PSS_SHA256",
            ArchiveVerificationFailure::UnsupportedKeyAlgorithm,
        ),
        (
            "public_key_spki_der_base64",
            "invalid",
            ArchiveVerificationFailure::InvalidPublicKey,
        ),
    ] {
        let mut keys = fixture["signing_keys_response"].clone();
        keys["keys"][0][field] = json!(value);
        let (_server, base_url) = server_with_keys(keys).await;
        let cache = SigningKeyCache::default();
        assert_eq!(
            download(&fixture["signing"])
                .verify(
                    archive.clone(),
                    &base_url,
                    &cache,
                    key_request(auth.clone())
                )
                .await,
            Err(failure)
        );
    }
}

#[test]
fn retains_signing_metadata_only_for_download() {
    let url = "https://example.com/archive";
    let signing = json!({"sha256": "digest", "key_id": "key", "signature": "signature", "algorithm": "algorithm"});
    let release = json!({"bundle_download_url": url, "signing": signing});
    let download: RemotePluginArchiveDownload = serde_json::from_value(release).unwrap();
    assert!(matches!(download.signing, ArchiveVerification::Present(_)));
    assert_eq!(
        serde_json::to_value(&download).unwrap(),
        json!({"bundle_download_url": url})
    );
    let malformed: RemotePluginArchiveDownload =
        serde_json::from_value(json!({"signing": {}})).unwrap();
    assert_eq!(malformed.signing, ArchiveVerification::Malformed);
    let unsigned: RemotePluginArchiveDownload = serde_json::from_value(json!({})).unwrap();
    assert_eq!(unsigned.signing, ArchiveVerification::Absent);
}
