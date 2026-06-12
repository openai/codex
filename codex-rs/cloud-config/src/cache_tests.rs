use super::*;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

fn test_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: vec![CloudConfigFragment {
                id: "cfg_1".to_string(),
                name: "Base config".to_string(),
                contents: "model = \"gpt-5\"".to_string(),
            }],
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![CloudRequirementsFragment {
                id: "req_1".to_string(),
                name: "Base requirements".to_string(),
                contents: "allowed_approval_policies = [\"never\"]".to_string(),
            }],
        },
    }
}

fn signed_cache_file(
    signed_payload: CloudConfigBundleCacheSignedPayload,
) -> CloudConfigBundleCacheFile {
    let payload_bytes = cache_payload_bytes(&signed_payload).expect("payload bytes");
    CloudConfigBundleCacheFile {
        signature: sign_cache_payload(&payload_bytes).expect("signature"),
        signed_payload,
    }
}

fn valid_signed_payload() -> CloudConfigBundleCacheSignedPayload {
    let cached_at = Utc::now();
    CloudConfigBundleCacheSignedPayload {
        version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
        cached_at,
        expires_at: cached_at + ChronoDuration::minutes(15),
        chatgpt_user_id: Some("user-12345".to_string()),
        account_id: Some("account-12345".to_string()),
        bundle: test_bundle(),
    }
}

fn write_cache_file(cache: &CloudConfigBundleCache, cache_file: &CloudConfigBundleCacheFile) {
    std::fs::write(
        cache.path(),
        serde_json::to_vec_pretty(cache_file).expect("serialize cache"),
    )
    .expect("write cache");
}

fn create_test_cache(codex_home: &Path) -> CloudConfigBundleCache {
    CloudConfigBundleCache::new(AbsolutePathBuf::resolve_path_against_base(codex_home, "/"))
}

#[tokio::test]
async fn save_writes_signed_payload_and_loads_for_matching_identity() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let bundle = test_bundle();

    cache
        .save(
            Some("user-12345".to_string()),
            Some("account-12345".to_string()),
            bundle.clone(),
        )
        .await
        .expect("save cache");

    let cache_file: CloudConfigBundleCacheFile =
        serde_json::from_slice(&std::fs::read(cache.path()).expect("read cache"))
            .expect("parse cache");
    assert!(
        cache_file.signed_payload.expires_at
            <= cache_file.signed_payload.cached_at + ChronoDuration::minutes(15)
    );
    assert!(cache_file.signed_payload.expires_at > cache_file.signed_payload.cached_at);
    assert_eq!(
        cache_file,
        signed_cache_file(CloudConfigBundleCacheSignedPayload {
            version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
            cached_at: cache_file.signed_payload.cached_at,
            expires_at: cache_file.signed_payload.expires_at,
            chatgpt_user_id: Some("user-12345".to_string()),
            account_id: Some("account-12345".to_string()),
            bundle,
        })
    );

    let loaded_cache = cache
        .load(Some("user-12345"), Some("account-12345"))
        .await
        .expect("load cache");
    assert_eq!(loaded_cache.signed_payload, cache_file.signed_payload);
    assert!(loaded_cache.refresh_in <= CLOUD_CONFIG_BUNDLE_CACHE_TTL);
}

#[tokio::test]
async fn load_rejects_missing_request_identity_before_reading_cache_file() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());

    assert_eq!(
        cache
            .load(/*chatgpt_user_id*/ None, Some("account-12345"))
            .await,
        Err(CacheLoadStatus::AuthIdentityIncomplete)
    );
    assert_eq!(
        cache.load(Some("user-12345"), /*account_id*/ None).await,
        Err(CacheLoadStatus::AuthIdentityIncomplete)
    );
}

#[tokio::test]
async fn load_reports_missing_and_malformed_cache_files() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheFileNotFound)
    );

    std::fs::write(cache.path(), "{").expect("write malformed cache");
    assert!(matches!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheParseFailed(_))
    ));
}

#[tokio::test]
async fn load_rejects_tampered_payload() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let mut cache_file = signed_cache_file(valid_signed_payload());
    cache_file
        .signed_payload
        .bundle
        .requirements_toml
        .enterprise_managed[0]
        .contents = "allowed_approval_policies = [\"on-request\"]".to_string();
    write_cache_file(&cache, &cache_file);

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheSignatureInvalid)
    );
}

#[tokio::test]
async fn load_rejects_cache_for_incomplete_or_different_identity() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let cache_file = signed_cache_file(valid_signed_payload());
    write_cache_file(&cache, &cache_file);

    assert_eq!(
        cache.load(Some("user-99999"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheIdentityMismatch)
    );

    let mut signed_payload = valid_signed_payload();
    signed_payload.chatgpt_user_id = None;
    write_cache_file(&cache, &signed_cache_file(signed_payload));

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheIdentityIncomplete)
    );
}

#[tokio::test]
async fn load_rejects_expired_cache() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let mut signed_payload = valid_signed_payload();
    signed_payload.expires_at = Utc::now() - ChronoDuration::seconds(1);
    write_cache_file(&cache, &signed_cache_file(signed_payload));

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheExpired)
    );
}

#[tokio::test]
async fn load_rejects_cache_older_than_ttl_even_when_expiry_is_later() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let mut signed_payload = valid_signed_payload();
    signed_payload.cached_at =
        Utc::now() - ChronoDuration::minutes(15) - ChronoDuration::seconds(1);
    signed_payload.expires_at = Utc::now() + ChronoDuration::minutes(15);
    write_cache_file(&cache, &signed_cache_file(signed_payload));

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheExpired)
    );
}

#[tokio::test]
async fn load_uses_ttl_cap_for_refresh_delay() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let now = Utc::now();
    let mut signed_payload = valid_signed_payload();
    signed_payload.cached_at = now - ChronoDuration::minutes(5);
    signed_payload.expires_at = now + ChronoDuration::minutes(30);
    write_cache_file(&cache, &signed_cache_file(signed_payload));

    let loaded_cache = cache
        .load(Some("user-12345"), Some("account-12345"))
        .await
        .expect("load cache");

    assert!(loaded_cache.refresh_in <= Duration::from_secs(10 * 60));
    assert!(loaded_cache.refresh_in >= Duration::from_secs(10 * 60 - 5));
}

#[tokio::test]
async fn load_rejects_unsupported_cache_version() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let mut signed_payload = valid_signed_payload();
    signed_payload.version = 2;
    write_cache_file(&cache, &signed_cache_file(signed_payload));

    assert_eq!(
        cache.load(Some("user-12345"), Some("account-12345")).await,
        Err(CacheLoadStatus::CacheVersionUnsupported(2))
    );
}

#[tokio::test]
async fn cache_lock_reuses_persistent_lock_file() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let lock_path = codex_home
        .path()
        .join(CLOUD_CONFIG_BUNDLE_CACHE_LOCK_FILENAME);
    std::fs::write(&lock_path, []).expect("create persistent lock file");

    let first_lock = cache
        .try_acquire_lock()
        .await
        .expect("acquire first cache lock");
    let CacheLockAttempt::Acquired(first_lock) = first_lock else {
        panic!("first cache lock should be acquired");
    };
    drop(first_lock);
    assert!(lock_path.exists());

    let second_lock = cache
        .try_acquire_lock()
        .await
        .expect("reacquire cache lock from persistent file");
    assert!(matches!(second_lock, CacheLockAttempt::Acquired(_)));
}

#[tokio::test]
async fn contended_cache_lock_returns_without_waiting() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    let first_lock = cache
        .try_acquire_lock()
        .await
        .expect("acquire first cache lock");
    let CacheLockAttempt::Acquired(first_lock) = first_lock else {
        panic!("first cache lock should be acquired");
    };

    let second_lock = timeout(Duration::from_millis(100), cache.try_acquire_lock())
        .await
        .expect("contended cache lock should return without waiting")
        .expect("contended cache lock should not error");
    assert!(matches!(second_lock, CacheLockAttempt::Contended));

    drop(first_lock);
    let third_lock = cache
        .try_acquire_lock()
        .await
        .expect("acquire cache lock after first owner exits");
    assert!(
        matches!(third_lock, CacheLockAttempt::Acquired(_)),
        "next owner should acquire after the first owner exits"
    );
}
