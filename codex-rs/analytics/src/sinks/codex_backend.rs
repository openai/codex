use crate::events::TrackEventRequest;
use crate::events::TrackEventsRequest;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use std::time::Duration;

const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) async fn write(
    auth_manager: &AuthManager,
    base_url: &str,
    events: &[TrackEventRequest],
) {
    if events.is_empty() {
        return;
    }

    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    if !auth.uses_codex_backend() {
        return;
    }

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    for events in track_event_request_batches(events) {
        send_track_events_request(&auth, &url, events).await;
    }
}

fn track_event_request_batches(events: &[TrackEventRequest]) -> Vec<&[TrackEventRequest]> {
    let mut batches = Vec::new();
    let mut current_batch_start = 0;

    for (index, event) in events.iter().enumerate() {
        if event.should_send_in_isolated_request() {
            if current_batch_start < index {
                batches.push(&events[current_batch_start..index]);
            }
            batches.push(&events[index..=index]);
            current_batch_start = index + 1;
        }
    }

    if current_batch_start < events.len() {
        batches.push(&events[current_batch_start..]);
    }

    batches
}

async fn send_track_events_request(auth: &CodexAuth, url: &str, events: &[TrackEventRequest]) {
    let payload = TrackEventsRequest { events };

    let response = create_client()
        .post(url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .headers(codex_model_provider::auth_provider_from_auth(auth).to_auth_headers())
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("events failed with status {status}: {body}");
        }
        Err(err) => {
            tracing::warn!("failed to send events request: {err}");
        }
    }
}

#[cfg(test)]
#[path = "codex_backend_tests.rs"]
mod tests;
