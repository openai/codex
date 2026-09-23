//! Compose request-scoped response interceptors before tracing and response bookkeeping.
use codex_extension_api::ModelRequestContributor;
use codex_extension_api::ModelRequestInput;
use codex_extension_api::ModelRequestKind;
use codex_extension_api::ModelResponseInterceptor;
use codex_extension_api::ModelResponseStream;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn prepare(
    contributors: &[Arc<dyn ModelRequestContributor>],
    thread_id: &str,
    model: &str,
    kind: ModelRequestKind,
    metadata: &mut Option<HashMap<String, String>>,
) -> Vec<Box<dyn ModelResponseInterceptor>> {
    contributors
        .iter()
        .filter_map(|contributor| {
            let mut additions = None;
            let interceptor = contributor.request(ModelRequestInput {
                kind,
                thread_id,
                client_metadata: &mut additions,
                model,
            });
            if let Some(additions) = additions {
                for (key, value) in crate::responses_metadata::filter_extra_metadata(additions) {
                    if key != "parent_response_id" {
                        metadata.get_or_insert_default().entry(key).or_insert(value);
                    }
                }
            }
            interceptor
        })
        .collect()
}

pub(crate) fn intercept_stream(
    mut stream: ModelResponseStream,
    interceptors: Vec<Box<dyn ModelResponseInterceptor>>,
) -> ModelResponseStream {
    for interceptor in interceptors {
        stream = interceptor.intercept(stream);
    }
    stream
}

#[cfg(test)]
#[path = "model_request_tests.rs"]
mod tests;
