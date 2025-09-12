// this file is only used for e2e tests which are currently disabled on windows
#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used)]

use anyhow::Context;
use assert_cmd::prelude::*;
use core_test_support::load_sse_fixture_with_id_from_str;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::matchers::method;
use wiremock::matchers::path;

use wiremock::Respond;

struct SeqResponder {
    num_calls: AtomicUsize,
    responses: Vec<String>,
}

impl Respond for SeqResponder {
    fn respond(&self, _: &wiremock::Request) -> wiremock::ResponseTemplate {
        let call_num = self.num_calls.fetch_add(1, Ordering::SeqCst);
        match self.responses.get(call_num) {
            Some(body) => wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(
                    load_sse_fixture_with_id_from_str(body, &format!("request_{call_num}")),
                    "text/event-stream",
                ),
            None => panic!("no response for {call_num}"),
        }
    }
}

/// Helper function to run an E2E test of a codex-exec call. Starts a wiremock
/// server, and returns the response_streams in order for each api call. Runs
/// the codex-exec command with the wiremock server as the model server.
pub(crate) async fn run_e2e_exec_test(cwd: &Path, response_streams: Vec<String>) {
    let server = MockServer::start().await;

    let num_calls = response_streams.len();
    let seq_responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses: response_streams,
    };

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(seq_responder)
        .expect(num_calls as u64)
        .mount(&server)
        .await;

    let cwd = cwd.to_path_buf();
    let uri = server.uri();
    Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")
        .expect("should find binary for codex-exec")
        .current_dir(cwd.clone())
        .env("CODEX_HOME", cwd)
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{uri}/v1"))
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("danger-full-access")
        .arg("foo")
        .assert()
        .success();
}
