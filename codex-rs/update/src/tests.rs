use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sources(server: &MockServer) -> UpdateSources {
    let base_url = server.uri();
    UpdateSources {
        homebrew_cask_api_url: format!("{base_url}/brew"),
        latest_release_url: format!("{base_url}/release"),
        npm_package_url: format!("{base_url}/npm"),
    }
}

#[test]
fn maps_install_context_to_update_action() {
    let native_release_dir =
        AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join("native-release"))
            .expect("temp dir path should be absolute");
    let cases = [
        (InstallMethod::Other, None),
        (InstallMethod::Npm, Some(UpdateAction::NpmGlobalLatest)),
        (InstallMethod::Bun, Some(UpdateAction::BunGlobalLatest)),
        (InstallMethod::Brew, Some(UpdateAction::BrewUpgrade)),
        (
            InstallMethod::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: native_release_dir.clone(),
                resources_dir: None,
            },
            Some(UpdateAction::StandaloneUnix),
        ),
        (
            InstallMethod::Standalone {
                platform: StandalonePlatform::Windows,
                release_dir: native_release_dir,
                resources_dir: None,
            },
            Some(UpdateAction::StandaloneWindows),
        ),
    ];

    for (method, expected) in cases {
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method,
                package_layout: None,
            }),
            expected
        );
    }
}

#[test]
fn compares_plain_versions_and_rejects_prereleases() {
    assert_eq!(is_newer("0.11.1", "0.11.0"), Some(true));
    assert_eq!(is_newer("0.11.0", "0.11.1"), Some(false));
    assert_eq!(is_newer("1.0.0", "0.9.9"), Some(true));
    assert_eq!(is_newer("0.11.0-beta.1", "0.11.0"), None);
    assert!(is_source_build_version("0.0.0"));
}

#[tokio::test]
async fn brew_uses_the_cask_version() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/brew"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "version": "1.2.3"
        })))
        .mount(&server)
        .await;

    let latest = latest_version_from_sources(Some(UpdateAction::BrewUpgrade), &sources(&server))
        .await
        .expect("brew version should load");

    assert_eq!(latest, "1.2.3");
}

#[tokio::test]
async fn npm_requires_the_release_to_be_installable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/release"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tag_name": "rust-v1.2.3"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/npm"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "dist-tags": { "latest": "1.2.3" },
            "versions": {
                "1.2.3": {
                    "dist": {
                        "tarball": "https://registry.npmjs.org/codex-1.2.3.tgz",
                        "integrity": "sha512-test"
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let latest =
        latest_version_from_sources(Some(UpdateAction::NpmGlobalLatest), &sources(&server))
            .await
            .expect("npm version should be ready");

    assert_eq!(latest, "1.2.3");
}

#[tokio::test]
async fn npm_rejects_a_stale_latest_tag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/release"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tag_name": "rust-v1.2.3"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/npm"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "dist-tags": { "latest": "1.2.2" },
            "versions": {}
        })))
        .mount(&server)
        .await;

    let error = latest_version_from_sources(Some(UpdateAction::NpmGlobalLatest), &sources(&server))
        .await
        .expect_err("stale npm latest tag should fail");

    assert!(error.to_string().contains("latest dist-tag"));
}

#[tokio::test]
async fn other_installations_use_the_latest_github_release() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/release"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tag_name": "rust-v1.2.3"
        })))
        .mount(&server)
        .await;

    let latest = latest_version_from_sources(/*action*/ None, &sources(&server))
        .await
        .expect("GitHub version should load");

    assert_eq!(latest, "1.2.3");
}
