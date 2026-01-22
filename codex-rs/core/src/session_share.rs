use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use aws_config::BehaviorVersion;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use azure_core::auth::TokenCredential;
use azure_identity::AzureCliCredential;
use codex_protocol::ThreadId;
use reqwest::StatusCode;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

const SHARE_OBJECT_PREFIX: &str = "sessions";
const SHARE_OBJECT_SUFFIX: &str = ".jsonl";
const SHARE_META_SUFFIX: &str = ".meta.json";
const AZURE_STORAGE_SCOPE: &str = "https://storage.azure.com/.default";

#[derive(Debug, Clone)]
pub struct SessionShareResult {
    pub remote_id: ThreadId,
    pub object_url: Url,
}

#[derive(Debug, Clone)]
enum SessionObjectStore {
    Http(HttpObjectStore),
    S3(S3ObjectStore),
    Azure(AzureObjectStore),
}

#[derive(Debug, Clone)]
struct HttpObjectStore {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
struct S3ObjectStore {
    client: S3Client,
    bucket: String,
    prefix: String,
}

#[derive(Debug, Clone)]
struct AzureObjectStore {
    endpoint: Url,
    container: String,
    prefix: String,
    sas_query: Option<String>,
    client: reqwest::Client,
    credential: Option<Arc<dyn TokenCredential>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionShareMeta {
    owner: String,
    created_at: i64,
    updated_at: i64,
}

impl SessionObjectStore {
    pub async fn new(base_url: &str) -> anyhow::Result<Self> {
        let mut url = Url::parse(base_url)
            .with_context(|| format!("invalid session_object_storage_url: {base_url}"))?;
        match url.scheme() {
            "s3" => Ok(SessionObjectStore::S3(S3ObjectStore::new(&url).await?)),
            "az" => Ok(SessionObjectStore::Azure(AzureObjectStore::new_from_az(
                &url,
            )?)),
            "http" | "https" => {
                if is_azure_blob_url(&url) {
                    Ok(SessionObjectStore::Azure(AzureObjectStore::new(&url)?))
                } else {
                    ensure_trailing_slash(&mut url);
                    Ok(SessionObjectStore::Http(HttpObjectStore {
                        base_url: url,
                        client: reqwest::Client::new(),
                    }))
                }
            }
            other => Err(anyhow::anyhow!(
                "unsupported session_object_storage_url scheme {other}"
            )),
        }
    }

    fn object_url(&self, key: &str) -> anyhow::Result<Url> {
        match self {
            SessionObjectStore::Http(store) => store.object_url(key),
            SessionObjectStore::S3(store) => store.object_url(key),
            SessionObjectStore::Azure(store) => store.object_url(key),
        }
    }

    async fn object_exists(&self, key: &str) -> anyhow::Result<bool> {
        match self {
            SessionObjectStore::Http(store) => store.object_exists(key).await,
            SessionObjectStore::S3(store) => store.object_exists(key).await,
            SessionObjectStore::Azure(store) => store.object_exists(key).await,
        }
    }

    async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> anyhow::Result<()> {
        match self {
            SessionObjectStore::Http(store) => store.put_object(key, data, content_type).await,
            SessionObjectStore::S3(store) => store.put_object(key, data, content_type).await,
            SessionObjectStore::Azure(store) => store.put_object(key, data, content_type).await,
        }
    }

    async fn get_object_bytes(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        match self {
            SessionObjectStore::Http(store) => store.get_object_bytes(key).await,
            SessionObjectStore::S3(store) => store.get_object_bytes(key).await,
            SessionObjectStore::Azure(store) => store.get_object_bytes(key).await,
        }
    }
}

pub async fn upload_rollout_with_owner(
    base_url: &str,
    session_id: ThreadId,
    owner: &str,
    rollout_path: &Path,
) -> anyhow::Result<SessionShareResult> {
    let data = tokio::fs::read(rollout_path)
        .await
        .with_context(|| format!("failed to read rollout at {}", rollout_path.display()))?;
    let store = SessionObjectStore::new(base_url).await?;
    let key = object_key(session_id);
    let meta_key = meta_key(session_id);
    let exists = store.object_exists(&key).await?;
    let now = OffsetDateTime::now_utc().unix_timestamp();

    if exists {
        let meta = fetch_meta(&store, &meta_key).await?;
        let Some(meta) = meta else {
            return Err(anyhow::anyhow!(
                "remote session already exists without metadata; refusing to overwrite"
            ));
        };
        if meta.owner != owner {
            return Err(anyhow::anyhow!(
                "remote session already exists and belongs to another user"
            ));
        }
        store
            .put_object(&key, data, "application/x-ndjson")
            .await
            .with_context(|| format!("failed to upload rollout for id {session_id}"))?;
        let updated = SessionShareMeta {
            owner: meta.owner,
            created_at: meta.created_at,
            updated_at: now,
        };
        upload_meta(&store, &meta_key, &updated).await?;
    } else {
        store
            .put_object(&key, data, "application/x-ndjson")
            .await
            .with_context(|| format!("failed to upload rollout for id {session_id}"))?;
        let meta = SessionShareMeta {
            owner: owner.to_string(),
            created_at: now,
            updated_at: now,
        };
        upload_meta(&store, &meta_key, &meta).await?;
    }

    let object_url = store.object_url(&key)?;
    Ok(SessionShareResult {
        remote_id: session_id,
        object_url,
    })
}

fn object_key(id: ThreadId) -> String {
    format!("{SHARE_OBJECT_PREFIX}/{id}{SHARE_OBJECT_SUFFIX}")
}

fn meta_key(id: ThreadId) -> String {
    format!("{SHARE_OBJECT_PREFIX}/{id}{SHARE_META_SUFFIX}")
}

async fn fetch_meta(
    store: &SessionObjectStore,
    key: &str,
) -> anyhow::Result<Option<SessionShareMeta>> {
    let Some(bytes) = store.get_object_bytes(key).await? else {
        return Ok(None);
    };
    let meta: SessionShareMeta =
        serde_json::from_slice(&bytes).with_context(|| "failed to parse session share metadata")?;
    Ok(Some(meta))
}

async fn upload_meta(
    store: &SessionObjectStore,
    key: &str,
    meta: &SessionShareMeta,
) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(meta).with_context(|| "failed to serialize metadata")?;
    store.put_object(key, payload, "application/json").await?;
    Ok(())
}

impl HttpObjectStore {
    fn object_url(&self, key: &str) -> anyhow::Result<Url> {
        self.base_url
            .join(key)
            .with_context(|| format!("failed to build object URL for key {key}"))
    }

    async fn object_exists(&self, key: &str) -> anyhow::Result<bool> {
        let url = self.object_url(key)?;
        let response = self.client.head(url).send().await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(false),
            StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED => {
                self.object_exists_via_get(key).await
            }
            status if status.is_success() => Ok(true),
            status => Err(anyhow::anyhow!(
                "object store HEAD failed with status {status}"
            )),
        }
    }

    async fn object_exists_via_get(&self, key: &str) -> anyhow::Result<bool> {
        let url = self.object_url(key)?;
        let response = self
            .client
            .get(url)
            .header(reqwest::header::RANGE, "bytes=0-0")
            .send()
            .await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(false),
            StatusCode::PARTIAL_CONTENT | StatusCode::OK => Ok(true),
            status => Err(anyhow::anyhow!(
                "object store GET probe failed with status {status}"
            )),
        }
    }

    async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> anyhow::Result<()> {
        let url = self.object_url(key)?;
        let response = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(data)
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "object store PUT failed with status {}",
                response.status()
            ))
        }
    }

    async fn get_object_bytes(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let url = self.object_url(key)?;
        let response = self.client.get(url).send().await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => {
                let bytes = response.bytes().await?;
                Ok(Some(bytes.to_vec()))
            }
            status => Err(anyhow::anyhow!(
                "object store GET failed with status {status}"
            )),
        }
    }
}

impl S3ObjectStore {
    async fn new(url: &Url) -> anyhow::Result<Self> {
        let bucket = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("s3 url missing bucket"))?
            .to_string();
        let prefix = url
            .path()
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string();
        let region_provider = RegionProviderChain::default_provider();
        let shared_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        if shared_config.region().is_none() {
            return Err(anyhow::anyhow!(
                "AWS region not set. Set AWS_REGION or configure a default region."
            ));
        }
        let client = S3Client::new(&shared_config);
        Ok(Self {
            client,
            bucket,
            prefix,
        })
    }

    fn object_url(&self, key: &str) -> anyhow::Result<Url> {
        let full_key = join_prefix(&self.prefix, key);
        Url::parse(&format!("s3://{}/{}", self.bucket, full_key))
            .with_context(|| format!("failed to build s3 url for key {full_key}"))
    }

    async fn object_exists(&self, key: &str) -> anyhow::Result<bool> {
        let full_key = join_prefix(&self.prefix, key);
        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await;
        match response {
            Ok(_) => Ok(true),
            Err(err) => match err {
                SdkError::ServiceError(service) => {
                    let code = service.err().code();
                    if matches!(code, Some("NotFound") | Some("NoSuchKey")) {
                        Ok(false)
                    } else {
                        Err(anyhow::anyhow!(
                            "object store HEAD failed with status {code:?}"
                        ))
                    }
                }
                other => Err(anyhow::anyhow!("object store HEAD failed: {other}")),
            },
        }
    }

    async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> anyhow::Result<()> {
        let full_key = join_prefix(&self.prefix, key);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .content_type(content_type)
            .body(ByteStream::from(data))
            .send()
            .await
            .with_context(|| format!("failed to upload s3://{}/{}", self.bucket, full_key))?;
        Ok(())
    }

    async fn get_object_bytes(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let full_key = join_prefix(&self.prefix, key);
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await;
        match response {
            Ok(output) => {
                let bytes = output.body.collect().await?;
                Ok(Some(bytes.into_bytes().to_vec()))
            }
            Err(err) => match err {
                SdkError::ServiceError(service) => {
                    let code = service.err().code();
                    if matches!(code, Some("NotFound") | Some("NoSuchKey")) {
                        Ok(None)
                    } else {
                        Err(anyhow::anyhow!(
                            "object store GET failed with status {code:?}"
                        ))
                    }
                }
                other => Err(anyhow::anyhow!("object store GET failed: {other}")),
            },
        }
    }
}

impl AzureObjectStore {
    fn new(url: &Url) -> anyhow::Result<Self> {
        let endpoint = azure_endpoint(url)?;
        let (container, prefix) = azure_container_and_prefix(url)?;
        let sas_query = url.query().map(str::to_string);
        let credential = if sas_query.is_some() {
            None
        } else {
            let credential: Arc<dyn TokenCredential> = Arc::new(AzureCliCredential::new());
            Some(credential)
        };
        Ok(Self {
            endpoint,
            container,
            prefix,
            sas_query,
            client: reqwest::Client::new(),
            credential,
        })
    }

    fn new_from_az(url: &Url) -> anyhow::Result<Self> {
        let account = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("az url missing account name"))?;
        let endpoint = azure_endpoint_for_account(account)?;
        let (container, prefix) = azure_container_and_prefix(url)?;
        let sas_query = url.query().map(str::to_string);
        let credential = if sas_query.is_some() {
            None
        } else {
            let credential: Arc<dyn TokenCredential> = Arc::new(AzureCliCredential::new());
            Some(credential)
        };
        Ok(Self {
            endpoint,
            container,
            prefix,
            sas_query,
            client: reqwest::Client::new(),
            credential,
        })
    }

    fn object_url(&self, key: &str) -> anyhow::Result<Url> {
        let full_key = join_prefix(&self.prefix, key);
        let mut url = self.endpoint.clone();
        if full_key.is_empty() {
            url.set_path(&format!("/{}", self.container));
        } else {
            url.set_path(&format!("/{}/{}", self.container, full_key));
        }
        if let Some(query) = &self.sas_query {
            url.set_query(Some(query));
        }
        Ok(url)
    }

    async fn object_exists(&self, key: &str) -> anyhow::Result<bool> {
        let url = self.object_url(key)?;
        let response = self
            .authorized_request(self.client.head(url))
            .await?
            .send()
            .await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(false),
            status if status.is_success() => Ok(true),
            status => Err(anyhow::anyhow!(
                "azure blob HEAD failed with status {status}{}",
                azure_response_context(response.headers())
            )),
        }
    }

    async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> anyhow::Result<()> {
        let url = self.object_url(key)?;
        let response = self
            .authorized_request(
                self.client
                    .put(url)
                    .header("x-ms-blob-type", "BlockBlob")
                    .header(reqwest::header::CONTENT_TYPE, content_type)
                    .body(data),
            )
            .await?
            .send()
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let headers = azure_response_context(response.headers());
            let body = response.text().await.unwrap_or_default();
            let body_snippet = azure_response_body_snippet(&body);
            Err(anyhow::anyhow!(
                "azure blob PUT failed with status {status}{headers}{body_snippet}"
            ))
        }
    }

    async fn get_object_bytes(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let url = self.object_url(key)?;
        let response = self
            .authorized_request(self.client.get(url))
            .await?
            .send()
            .await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => {
                let bytes = response.bytes().await?;
                Ok(Some(bytes.to_vec()))
            }
            status => Err(anyhow::anyhow!(
                "azure blob GET failed with status {status}{}",
                azure_response_context(response.headers())
            )),
        }
    }
}

fn ensure_trailing_slash(url: &mut Url) {
    let path = url.path();
    if path.ends_with('/') {
        return;
    }
    let trimmed = path.trim_end_matches('/');
    let new_path = if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("{trimmed}/")
    };
    url.set_path(&new_path);
}

fn join_prefix(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}/{key}")
    }
}

fn is_azure_blob_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    host.ends_with(".blob.core.windows.net")
}

fn azure_endpoint(url: &Url) -> anyhow::Result<Url> {
    let mut endpoint = url.clone();
    endpoint.set_path("/");
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    Ok(endpoint)
}

fn azure_container_and_prefix(url: &Url) -> anyhow::Result<(String, String)> {
    let segments = url
        .path_segments()
        .map(|iter| {
            iter.filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    azure_container_and_prefix_from_segments(&segments)
}

fn azure_container_and_prefix_from_segments(segments: &[&str]) -> anyhow::Result<(String, String)> {
    if segments.is_empty() {
        return Err(anyhow::anyhow!(
            "azure blob url must include a container name"
        ));
    }
    let container = segments[0].to_string();
    let prefix = segments[1..].join("/");
    Ok((container, prefix))
}

fn azure_request(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header("x-ms-version", "2021-08-06")
}

fn azure_endpoint_for_account(account: &str) -> anyhow::Result<Url> {
    let endpoint = format!("https://{account}.blob.core.windows.net/");
    Url::parse(&endpoint).with_context(|| "failed to build azure blob endpoint")
}

impl AzureObjectStore {
    async fn authorized_request(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> anyhow::Result<reqwest::RequestBuilder> {
        let builder = azure_request(builder);
        let Some(credential) = &self.credential else {
            return Ok(builder);
        };
        let token = credential
            .get_token(&[AZURE_STORAGE_SCOPE])
            .await
            .with_context(|| "failed to acquire azure blob access token")?;
        Ok(builder.bearer_auth(token.token.secret()))
    }
}

fn azure_response_context(headers: &reqwest::header::HeaderMap) -> String {
    let mut parts = Vec::new();
    if let Some(value) = azure_header_value(headers, "x-ms-error-code") {
        parts.push(format!("x-ms-error-code={value}"));
    }
    if let Some(value) = azure_header_value(headers, "x-ms-request-id") {
        parts.push(format!("x-ms-request-id={value}"));
    }
    if let Some(value) = azure_header_value(headers, "www-authenticate") {
        parts.push(format!("www-authenticate={value}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

fn azure_header_value(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn azure_response_body_snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let snippet = if trimmed.len() <= 512 {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(512).collect();
        format!("{truncated}...")
    };
    format!(" (body={snippet})")
}
