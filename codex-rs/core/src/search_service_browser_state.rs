use codex_protocol::mcp::CallToolResult;
use serde_json::Map;
use serde_json::Value;

pub(crate) const SEARCH_SERVICE_CONNECTOR_ID: &str = "connector_openai_search_service";

const SEARCH_SERVICE_BROWSER_STATE_META_KEY: &str = "search_service_browser_state";

pub(crate) fn augment_request_meta(meta: Option<Value>, state: Option<&Value>) -> Option<Value> {
    match state.and_then(normalize_browser_state) {
        Some(state) => Some(insert_browser_state(meta, state)),
        None => meta,
    }
}

fn insert_browser_state(meta: Option<Value>, state: Value) -> Value {
    match meta {
        Some(Value::Object(mut map)) => {
            map.insert(SEARCH_SERVICE_BROWSER_STATE_META_KEY.to_string(), state);
            Value::Object(map)
        }
        None => {
            let mut map = Map::new();
            map.insert(SEARCH_SERVICE_BROWSER_STATE_META_KEY.to_string(), state);
            Value::Object(map)
        }
        Some(other) => other,
    }
}

pub(crate) fn browser_state_from_tool_result(result: &CallToolResult) -> Option<Value> {
    let meta = result.meta.as_ref()?.as_object()?;
    if let Some(state) = meta
        .get(SEARCH_SERVICE_BROWSER_STATE_META_KEY)
        .and_then(normalize_browser_state)
    {
        return Some(state);
    }
    let raw_messages = meta.get("raw_messages")?.as_array()?;
    browser_state_from_raw_messages(raw_messages)
}

fn browser_state_from_raw_messages(raw_messages: &[Value]) -> Option<Value> {
    let mut state = empty_browser_state();
    for raw_message in raw_messages {
        let Some(raw_message) = raw_message.as_object() else {
            continue;
        };
        merge_raw_message_into_browser_state(&mut state, raw_message);
    }
    non_empty_browser_state(state)
}

fn normalize_browser_state(value: &Value) -> Option<Value> {
    let value = value.as_object()?;
    let mut state = empty_browser_state();

    copy_latest_string(value, &mut state, "sonic_thread_id");
    copy_latest_string(value, &mut state, "sonic_user_id");
    copy_latest_i64(value, &mut state, "sonic_turn_index");

    if let Some(precise_location_metadata) = value.get("precise_location_metadata") {
        state.insert(
            "precise_location_metadata".to_string(),
            precise_location_metadata.clone(),
        );
    }

    if let Some(external_browser_state) = value
        .get("external_browser_state")
        .and_then(Value::as_object)
    {
        if let Some(content) = external_browser_state
            .get("content")
            .and_then(Value::as_object)
        {
            external_browser_state_object_mut(&mut state, "content").extend(content.clone());
        }
        if let Some(link_urls) = external_browser_state
            .get("link_urls")
            .and_then(Value::as_object)
        {
            external_browser_state_object_mut(&mut state, "link_urls").extend(link_urls.clone());
        }
    }

    for key in [
        "external_urls",
        "product_lookup_keys_by_ref_id",
        "refid_randomizer_state",
        "randomized_to_canonical_ref_ids",
    ] {
        if let Some(object) = value.get(key).and_then(Value::as_object) {
            state.insert(key.to_string(), Value::Object(object.clone()));
        }
    }

    for key in [
        "num_image_queries",
        "num_product_queries",
        "elapsed_tokens",
        "elapsed_queries",
    ] {
        copy_latest_i64(value, &mut state, key);
    }

    if let Some(product_queries) = value.get("product_queries").and_then(Value::as_array) {
        state.insert(
            "product_queries".to_string(),
            Value::Array(product_queries.clone()),
        );
    }

    for key in ["commerce_blocked_canvas", "image_group_shown"] {
        if let Some(value) = value.get(key).and_then(Value::as_bool) {
            state.insert(key.to_string(), Value::Bool(value));
        }
    }

    let aliases = randomized_ref_aliases(
        state
            .get("refid_randomizer_state")
            .and_then(Value::as_object),
    );
    object_mut(&mut state, "randomized_to_canonical_ref_ids").extend(aliases);
    non_empty_browser_state(state)
}

pub(crate) fn merge_browser_states(
    previous_state: Option<&Value>,
    new_state: Option<&Value>,
) -> Option<Value> {
    let mut merged = empty_browser_state();
    for state in [
        previous_state.and_then(normalize_browser_state),
        new_state.and_then(normalize_browser_state),
    ]
    .into_iter()
    .flatten()
    {
        merge_browser_state(&mut merged, state.as_object()?);
    }
    non_empty_browser_state(merged)
}

fn merge_raw_message_into_browser_state(
    state: &mut Map<String, Value>,
    raw_message: &Map<String, Value>,
) {
    let internal = raw_message
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("__internal"))
        .and_then(Value::as_object);

    let Some(internal) = internal else {
        merge_webpage_content_into_external_urls(state, raw_message);
        return;
    };

    let browser_state = internal
        .get("sonic_browser_tool")
        .and_then(Value::as_object);
    let Some(browser_state) = browser_state else {
        merge_webpage_content_into_external_urls(state, raw_message);
        return;
    };

    copy_latest_string(internal, state, "sonic_thread_id");
    copy_latest_string(internal, state, "sonic_user_id");
    copy_latest_i64(internal, state, "sonic_turn_index");
    if let Some(precise_location_metadata) = internal.get("precise_location_metadata") {
        state.insert(
            "precise_location_metadata".to_string(),
            precise_location_metadata.clone(),
        );
    }

    for key in ["summary_content", "content", "content_to_persist"] {
        if let Some(content) = browser_state.get(key).and_then(Value::as_object) {
            external_browser_state_object_mut(state, "content").extend(content.clone());
        }
    }
    if let Some(link_urls) = browser_state.get("link_urls").and_then(Value::as_object) {
        external_browser_state_object_mut(state, "link_urls").extend(link_urls.clone());
    }
    if let Some(product_lookup_keys) = browser_state
        .get("product_lookup_keys_by_ref_id")
        .and_then(Value::as_object)
    {
        object_mut(state, "product_lookup_keys_by_ref_id").extend(product_lookup_keys.clone());
    }
    if let Some(refid_randomizer_state) = browser_state
        .get("refid_randomizer_state")
        .and_then(Value::as_object)
    {
        state.insert(
            "refid_randomizer_state".to_string(),
            Value::Object(refid_randomizer_state.clone()),
        );
        object_mut(state, "randomized_to_canonical_ref_ids")
            .extend(randomized_ref_aliases(Some(refid_randomizer_state)));
    }

    for key in [
        "num_image_queries",
        "num_product_queries",
        "elapsed_tokens",
        "elapsed_queries",
    ] {
        copy_latest_i64(browser_state, state, key);
    }
    if let Some(product_queries) = browser_state
        .get("product_queries")
        .and_then(Value::as_array)
    {
        state.insert(
            "product_queries".to_string(),
            Value::Array(product_queries.clone()),
        );
    }
    for key in ["commerce_blocked_canvas", "image_group_shown"] {
        if let Some(value) = browser_state.get(key).and_then(Value::as_bool) {
            state.insert(key.to_string(), Value::Bool(value));
        }
    }

    refresh_external_urls_from_content(state);
    merge_webpage_content_into_external_urls(state, raw_message);
}

fn merge_browser_state(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    copy_latest_string(source, target, "sonic_thread_id");
    copy_latest_string(source, target, "sonic_user_id");
    copy_latest_i64(source, target, "sonic_turn_index");
    if let Some(precise_location_metadata) = source.get("precise_location_metadata") {
        target.insert(
            "precise_location_metadata".to_string(),
            precise_location_metadata.clone(),
        );
    }

    if let Some(source_browser_state) = source
        .get("external_browser_state")
        .and_then(Value::as_object)
    {
        if let Some(content) = source_browser_state
            .get("content")
            .and_then(Value::as_object)
        {
            external_browser_state_object_mut(target, "content").extend(content.clone());
        }
        if let Some(link_urls) = source_browser_state
            .get("link_urls")
            .and_then(Value::as_object)
        {
            external_browser_state_object_mut(target, "link_urls").extend(link_urls.clone());
        }
    }

    for key in [
        "external_urls",
        "product_lookup_keys_by_ref_id",
        "randomized_to_canonical_ref_ids",
    ] {
        if let Some(source_object) = source.get(key).and_then(Value::as_object) {
            object_mut(target, key).extend(source_object.clone());
        }
    }
    if let Some(refid_randomizer_state) = source
        .get("refid_randomizer_state")
        .and_then(Value::as_object)
        .filter(|state| !state.is_empty())
    {
        target.insert(
            "refid_randomizer_state".to_string(),
            Value::Object(refid_randomizer_state.clone()),
        );
    }

    for key in [
        "num_image_queries",
        "num_product_queries",
        "elapsed_tokens",
        "elapsed_queries",
    ] {
        copy_latest_i64(source, target, key);
    }
    if let Some(product_queries) = source.get("product_queries").and_then(Value::as_array) {
        target.insert(
            "product_queries".to_string(),
            Value::Array(product_queries.clone()),
        );
    }
    for key in ["commerce_blocked_canvas", "image_group_shown"] {
        if let Some(value) = source.get(key).and_then(Value::as_bool) {
            target.insert(key.to_string(), Value::Bool(value));
        }
    }
}

fn empty_browser_state() -> Map<String, Value> {
    Map::from_iter([
        ("sonic_thread_id".to_string(), Value::Null),
        ("sonic_user_id".to_string(), Value::Null),
        ("sonic_turn_index".to_string(), Value::Number(0.into())),
        ("precise_location_metadata".to_string(), Value::Null),
        (
            "external_browser_state".to_string(),
            Value::Object(Map::from_iter([
                ("content".to_string(), Value::Object(Map::new())),
                ("link_urls".to_string(), Value::Object(Map::new())),
            ])),
        ),
        ("external_urls".to_string(), Value::Object(Map::new())),
        (
            "product_lookup_keys_by_ref_id".to_string(),
            Value::Object(Map::new()),
        ),
        (
            "refid_randomizer_state".to_string(),
            Value::Object(Map::new()),
        ),
        (
            "randomized_to_canonical_ref_ids".to_string(),
            Value::Object(Map::new()),
        ),
        ("num_image_queries".to_string(), Value::Number(0.into())),
        ("num_product_queries".to_string(), Value::Number(0.into())),
        ("product_queries".to_string(), Value::Array(Vec::new())),
        ("commerce_blocked_canvas".to_string(), Value::Bool(false)),
        ("image_group_shown".to_string(), Value::Bool(false)),
        ("elapsed_tokens".to_string(), Value::Number(0.into())),
        ("elapsed_queries".to_string(), Value::Number(0.into())),
    ])
}

fn non_empty_browser_state(state: Map<String, Value>) -> Option<Value> {
    let external_browser_state = state
        .get("external_browser_state")
        .and_then(Value::as_object);
    let content = external_browser_state
        .and_then(|state| state.get("content"))
        .and_then(Value::as_object);
    let link_urls = external_browser_state
        .and_then(|state| state.get("link_urls"))
        .and_then(Value::as_object);

    let is_empty = [
        content.is_none_or(Map::is_empty),
        link_urls.is_none_or(Map::is_empty),
        state
            .get("external_urls")
            .and_then(Value::as_object)
            .is_none_or(Map::is_empty),
        state
            .get("product_lookup_keys_by_ref_id")
            .and_then(Value::as_object)
            .is_none_or(Map::is_empty),
        state
            .get("refid_randomizer_state")
            .and_then(Value::as_object)
            .is_none_or(Map::is_empty),
        state
            .get("randomized_to_canonical_ref_ids")
            .and_then(Value::as_object)
            .is_none_or(Map::is_empty),
        state
            .get("sonic_thread_id")
            .and_then(Value::as_str)
            .is_none(),
    ]
    .into_iter()
    .all(|empty| empty);

    (!is_empty).then_some(Value::Object(state))
}

fn randomized_ref_aliases(state: Option<&Map<String, Value>>) -> Map<String, Value> {
    let Some(refid_to_randomized_id) = state
        .and_then(|state| state.get("refid_to_randomized_id"))
        .and_then(Value::as_object)
    else {
        return Map::new();
    };

    refid_to_randomized_id
        .iter()
        .filter_map(|(canonical_ref_id, randomized_ref_id)| {
            randomized_ref_id
                .as_str()
                .map(|randomized_ref_id| (randomized_ref_id.to_string(), canonical_ref_id.clone()))
        })
        .map(|(randomized_ref_id, canonical_ref_id)| {
            (randomized_ref_id, Value::String(canonical_ref_id))
        })
        .collect()
}

fn refresh_external_urls_from_content(state: &mut Map<String, Value>) {
    let content = state
        .get("external_browser_state")
        .and_then(Value::as_object)
        .and_then(|state| state.get("content"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (ref_id, page) in content {
        let Some(url) = page
            .as_object()
            .and_then(|page| page.get("url"))
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty())
        else {
            continue;
        };
        object_mut(state, "external_urls").insert(ref_id, Value::String(url.to_string()));
    }
}

fn merge_webpage_content_into_external_urls(
    state: &mut Map<String, Value>,
    raw_message: &Map<String, Value>,
) {
    let content = raw_message.get("content").and_then(Value::as_object);
    let Some((ref_id, url)) = content
        .and_then(|content| {
            Some((
                content.get("ref_id")?.as_str()?,
                content.get("url")?.as_str()?,
            ))
        })
        .filter(|(_, url)| !url.is_empty())
    else {
        return;
    };
    object_mut(state, "external_urls").insert(ref_id.to_string(), Value::String(url.to_string()));
}

fn copy_latest_string(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_str) {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn copy_latest_i64(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_i64) {
        target.insert(key.to_string(), Value::Number(value.into()));
    }
}

fn external_browser_state_object_mut<'a>(
    state: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let external_browser_state = state
        .entry("external_browser_state".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !external_browser_state.is_object() {
        *external_browser_state = Value::Object(Map::new());
    }
    let Value::Object(external_browser_state) = external_browser_state else {
        unreachable!("external_browser_state was normalized to an object");
    };

    let child = external_browser_state
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !child.is_object() {
        *child = Value::Object(Map::new());
    }
    let Value::Object(child) = child else {
        unreachable!("external_browser_state child was normalized to an object");
    };
    child
}

fn object_mut<'a>(state: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let value = state
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    let Value::Object(value) = value else {
        unreachable!("browser state entry was normalized to an object");
    };
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn stores_browser_state_from_raw_messages_and_attaches_to_request_meta() {
        let browser_state = browser_state_from_tool_result(&CallToolResult {
            content: Vec::new(),
            structured_content: None,
            is_error: None,
            meta: Some(json!({
                "raw_messages": [{
                    "metadata": {
                        "__internal": {
                            "sonic_browser_tool": {
                                "content": {
                                    "turn0search0": { "url": "https://example.com/" }
                                },
                                "link_urls": {
                                    "turn0search0": {
                                        "1": "https://example.com/read-more"
                                    }
                                },
                                "refid_randomizer_state": {
                                    "refid_to_randomized_id": {
                                        "turn0search0": "turn123search0"
                                    }
                                }
                            },
                            "sonic_thread_id": "thread-123",
                            "sonic_turn_index": 1,
                            "sonic_user_id": "user-123"
                        }
                    }
                }]
            })),
        });
        let browser_state = merge_browser_states(None, browser_state.as_ref());

        assert_eq!(
            augment_request_meta(
                Some(json!({ "threadId": "thread-live" })),
                browser_state.as_ref(),
            ),
            Some(json!({
                "threadId": "thread-live",
                "search_service_browser_state": {
                    "sonic_thread_id": "thread-123",
                    "sonic_user_id": "user-123",
                    "sonic_turn_index": 1,
                    "precise_location_metadata": null,
                    "external_browser_state": {
                        "content": {
                            "turn0search0": { "url": "https://example.com/" }
                        },
                        "link_urls": {
                            "turn0search0": {
                                "1": "https://example.com/read-more"
                            }
                        }
                    },
                    "external_urls": { "turn0search0": "https://example.com/" },
                    "product_lookup_keys_by_ref_id": {},
                    "refid_randomizer_state": {
                        "refid_to_randomized_id": {
                            "turn0search0": "turn123search0"
                        }
                    },
                    "randomized_to_canonical_ref_ids": {
                        "turn123search0": "turn0search0"
                    },
                    "num_image_queries": 0,
                    "num_product_queries": 0,
                    "product_queries": [],
                    "commerce_blocked_canvas": false,
                    "image_group_shown": false,
                    "elapsed_tokens": 0,
                    "elapsed_queries": 0
                }
            }))
        );
    }

    #[test]
    fn preserves_request_meta_when_browser_state_is_missing() {
        let request_meta = Some(json!({ "threadId": "thread-live" }));

        assert_eq!(
            augment_request_meta(request_meta.clone(), None),
            request_meta
        );
    }
}
