use anyhow::Result;
use futures::future::BoxFuture;

pub mod director;
pub mod solver;
pub mod verifier;
pub mod verifier_pool;

pub trait Role<Req, Resp> {
    fn call<'a>(&'a self, req: &'a Req) -> BoxFuture<'a, Result<Resp>>;
}

// Shared helpers used by role implementations
use anyhow::Context as _;
use anyhow::anyhow;
use std::any::type_name;

pub(crate) fn strip_json_code_fence(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.strip_suffix("```").map(str::trim);
    }
    if let Some(rest) = trimmed.strip_prefix("```JSON") {
        return rest.strip_suffix("```").map(str::trim);
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.strip_suffix("```").map(str::trim);
    }
    None
}

pub(crate) fn parse_json_struct<T>(message: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("message was empty"));
    }

    serde_json::from_str(trimmed)
        .or_else(|err| {
            strip_json_code_fence(trimmed)
                .map(|inner| serde_json::from_str(inner))
                .unwrap_or_else(|| Err(err))
        })
        .map_err(|err| anyhow!(err))
        .with_context(|| format!("failed to parse message as {}", type_name::<T>()))
}
