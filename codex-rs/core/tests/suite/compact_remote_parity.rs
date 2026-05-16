#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const FIXED_CWD: &str = "/tmp/codex_remote_compaction_parity_workspace";
const IMAGE_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
const SUMMARY: &str = "REMOTE_COMPACTION_PARITY_ENCRYPTED_SUMMARY";
const DUMMY_FUNCTION_NAME: &str = "test_tool";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Legacy,
    V2,
}

#[derive(Clone, Copy, Debug)]
enum AuthCase {
    ChatGpt,
    ApiKey,
}

impl AuthCase {
    fn build(self) -> CodexAuth {
        match self {
            AuthCase::ChatGpt => CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            AuthCase::ApiKey => CodexAuth::from_api_key("dummy"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RunSettings {
    auth: AuthCase,
    service_tier_fast: bool,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            auth: AuthCase::ChatGpt,
            service_tier_fast: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Assistant,
    ReasoningAssistant,
    FunctionTool,
    ShellTool,
    ImageAssistant,
    WebSearchAssistant,
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Step::Assistant => "assistant",
            Step::ReasoningAssistant => "reasoning_assistant",
            Step::FunctionTool => "function_tool",
            Step::ShellTool => "shell_tool",
            Step::ImageAssistant => "image_assistant",
            Step::WebSearchAssistant => "web_search_assistant",
        }
    }
}

#[derive(Debug)]
struct Scenario {
    name: &'static str,
    steps: &'static [Step],
}

#[derive(Debug)]
struct Capture {
    compact_body: Value,
    follow_up_body: Value,
    replacement_history: Value,
    normal_response_requests: usize,
    compact_requests: usize,
}

const ASSISTANT_ONLY: &[Step] = &[Step::Assistant];
const REASONING_IMAGE: &[Step] = &[Step::ReasoningAssistant, Step::ImageAssistant];
const TOOL_MIX: &[Step] = &[Step::Assistant, Step::FunctionTool, Step::ShellTool];
const FULL_MIX: &[Step] = &[
    Step::ReasoningAssistant,
    Step::FunctionTool,
    Step::ImageAssistant,
    Step::ShellTool,
    Step::WebSearchAssistant,
    Step::Assistant,
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compaction_parity_manual_transcripts() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let scenarios = [
        Scenario {
            name: "assistant_only",
            steps: ASSISTANT_ONLY,
        },
        Scenario {
            name: "reasoning_image",
            steps: REASONING_IMAGE,
        },
        Scenario {
            name: "tool_mix",
            steps: TOOL_MIX,
        },
        Scenario {
            name: "full_mix",
            steps: FULL_MIX,
        },
    ];

    for scenario in scenarios {
        compare_manual_scenario(&scenario, RunSettings::default()).await?;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compaction_parity_v2_api_key_sends_service_tier_upgrade() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let scenario = Scenario {
        name: "api_key_service_tier",
        steps: TOOL_MIX,
    };
    let settings = RunSettings {
        auth: AuthCase::ApiKey,
        service_tier_fast: true,
    };
    let legacy = run_manual_session(&scenario, Mode::Legacy, settings).await?;
    let v2 = run_manual_session(&scenario, Mode::V2, settings).await?;

    assert_eq!(
        legacy.compact_body.get("service_tier"),
        None,
        "legacy /responses/compact should continue omitting service_tier for API-key auth"
    );
    assert_eq!(
        v2.compact_body.get("service_tier").and_then(Value::as_str),
        Some(ServiceTier::Fast.request_value()),
        "v2 compaction should send service_tier through /responses for API-key auth"
    );

    assert_compact_requests_eq_except_v2_service_tier("api-key service tier", &legacy, &v2);
    assert_follow_up_and_history_eq("api-key service tier", &legacy, &v2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compaction_parity_manual_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let legacy = run_manual_hook_session(Mode::Legacy).await?;
    let v2 = run_manual_hook_session(Mode::V2).await?;
    assert_json_eq("manual compact hook payload parity mismatch", &legacy, &v2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compaction_parity_pre_turn_auto() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let legacy = run_pre_turn_auto_session(Mode::Legacy).await?;
    let v2 = run_pre_turn_auto_session(Mode::V2).await?;
    assert_capture_eq("pre-turn auto", &legacy, &v2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compaction_parity_mid_turn_auto() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let legacy = run_mid_turn_auto_session(Mode::Legacy).await?;
    let v2 = run_mid_turn_auto_session(Mode::V2).await?;
    assert_capture_eq("mid-turn auto", &legacy, &v2);
    Ok(())
}

async fn compare_manual_scenario(scenario: &Scenario, settings: RunSettings) -> Result<()> {
    let legacy = run_manual_session(scenario, Mode::Legacy, settings).await?;
    let v2 = run_manual_session(scenario, Mode::V2, settings).await?;
    assert_capture_eq(scenario.name, &legacy, &v2);
    Ok(())
}

fn assert_capture_eq(label: &str, legacy: &Capture, v2: &Capture) {
    assert_eq!(
        legacy.compact_requests, 1,
        "legacy compact endpoint should be called exactly once for {label}",
    );
    assert_eq!(
        v2.compact_requests, 0,
        "v2 should not call /responses/compact for {label}",
    );

    let legacy_compact = compact_request_view(&legacy.compact_body, Mode::Legacy);
    let v2_compact = compact_request_view(&v2.compact_body, Mode::V2);
    assert_json_eq(
        &format!("compact request parity mismatch for {label}"),
        &legacy_compact,
        &v2_compact,
    );

    let legacy_follow_up = follow_up_request_view(&legacy.follow_up_body);
    let v2_follow_up = follow_up_request_view(&v2.follow_up_body);
    assert_json_eq(
        &format!("post-compact follow-up request parity mismatch for {label}"),
        &legacy_follow_up,
        &v2_follow_up,
    );

    assert_json_eq(
        &format!("replacement history parity mismatch for {label}"),
        &legacy.replacement_history,
        &v2.replacement_history,
    );

    println!(
        "PARITY_OK scenario={} normal_response_requests={} compact_input_items={} replacement_history_items={} follow_up_input_items={}",
        label,
        legacy.normal_response_requests,
        compact_input_len(&legacy.compact_body, Mode::Legacy),
        replacement_history_len(&legacy.replacement_history),
        follow_up_input_len(&legacy.follow_up_body)
    );
}

fn assert_compact_requests_eq_except_v2_service_tier(label: &str, legacy: &Capture, v2: &Capture) {
    assert_eq!(
        legacy.compact_requests, 1,
        "legacy compact endpoint should be called exactly once for {label}",
    );
    assert_eq!(
        v2.compact_requests, 0,
        "v2 should not call /responses/compact for {label}",
    );

    let legacy_compact = compact_request_view(&legacy.compact_body, Mode::Legacy);
    let mut v2_compact = compact_request_view(&v2.compact_body, Mode::V2);
    remove_object_field(&mut v2_compact, "service_tier");
    assert_json_eq(
        &format!("compact request parity mismatch for {label} after service_tier upgrade"),
        &legacy_compact,
        &v2_compact,
    );
}

fn assert_follow_up_and_history_eq(label: &str, legacy: &Capture, v2: &Capture) {
    let legacy_follow_up = follow_up_request_view(&legacy.follow_up_body);
    let v2_follow_up = follow_up_request_view(&v2.follow_up_body);
    assert_json_eq(
        &format!("post-compact follow-up request parity mismatch for {label}"),
        &legacy_follow_up,
        &v2_follow_up,
    );

    assert_json_eq(
        &format!("replacement history parity mismatch for {label}"),
        &legacy.replacement_history,
        &v2.replacement_history,
    );
}

async fn run_manual_session(
    scenario: &Scenario,
    mode: Mode,
    settings: RunSettings,
) -> Result<Capture> {
    let mut response_bodies = response_bodies_for_scenario(scenario);
    if mode == Mode::V2 {
        response_bodies.push(compaction_v2_response_body());
    }
    response_bodies.push(after_compact_response_body(scenario.name));

    let harness = build_harness(mode, settings, /*hooks*/ false).await?;
    let rollout_path = rollout_path(&harness);
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(harness.server(), response_bodies).await;
    let compact_mock = mount_legacy_compact_if_needed(&harness, mode).await;

    for (idx, step) in scenario.steps.iter().enumerate() {
        submit_user_input(&codex, user_input_for_step(scenario.name, idx, *step)).await?;
    }

    codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&codex).await;

    submit_user_input(
        &codex,
        vec![UserInput::Text {
            text: format!("{}_AFTER_COMPACT_USER", scenario.name),
            text_elements: Vec::new(),
        }],
    )
    .await?;

    capture_from_requests(
        mode,
        &codex,
        &rollout_path,
        &responses_mock,
        compact_mock.as_ref(),
        follow_up_index(responses_mock.requests().len()),
    )
    .await
}

async fn run_pre_turn_auto_session(mode: Mode) -> Result<Capture> {
    let response_bodies = match mode {
        Mode::Legacy => vec![
            responses::sse(vec![
                responses::ev_assistant_message("pre-turn-first-message", "PRE_TURN_FIRST_REPLY"),
                responses::ev_completed_with_tokens(
                    "pre-turn-first-response",
                    /*total_tokens*/ 500,
                ),
            ]),
            after_compact_response_body("pre_turn_auto"),
        ],
        Mode::V2 => vec![
            responses::sse(vec![
                responses::ev_assistant_message("pre-turn-first-message", "PRE_TURN_FIRST_REPLY"),
                responses::ev_completed_with_tokens(
                    "pre-turn-first-response",
                    /*total_tokens*/ 500,
                ),
            ]),
            compaction_v2_response_body(),
            after_compact_response_body("pre_turn_auto"),
        ],
    };
    let harness = build_auto_harness(mode).await?;
    let rollout_path = rollout_path(&harness);
    let codex = harness.test().codex.clone();
    let responses_mock = responses::mount_sse_sequence(harness.server(), response_bodies).await;
    let compact_mock = mount_legacy_compact_if_needed(&harness, mode).await;

    submit_user_input(
        &codex,
        vec![UserInput::Text {
            text: "pre_turn_auto_before".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await?;
    submit_user_input(
        &codex,
        vec![UserInput::Text {
            text: "pre_turn_auto_after".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await?;

    capture_from_requests(
        mode,
        &codex,
        &rollout_path,
        &responses_mock,
        compact_mock.as_ref(),
        follow_up_index(responses_mock.requests().len()),
    )
    .await
}

async fn run_mid_turn_auto_session(mode: Mode) -> Result<Capture> {
    let response_bodies = match mode {
        Mode::Legacy => vec![
            responses::sse(vec![
                responses::ev_function_call("mid-turn-call", DUMMY_FUNCTION_NAME, "{}"),
                responses::ev_completed_with_tokens(
                    "mid-turn-call-response",
                    /*total_tokens*/ 500,
                ),
            ]),
            after_compact_response_body("mid_turn_auto"),
        ],
        Mode::V2 => vec![
            responses::sse(vec![
                responses::ev_function_call("mid-turn-call", DUMMY_FUNCTION_NAME, "{}"),
                responses::ev_completed_with_tokens(
                    "mid-turn-call-response",
                    /*total_tokens*/ 500,
                ),
            ]),
            compaction_v2_response_body(),
            after_compact_response_body("mid_turn_auto"),
        ],
    };
    let harness = build_auto_harness(mode).await?;
    let rollout_path = rollout_path(&harness);
    let codex = harness.test().codex.clone();
    let responses_mock = responses::mount_sse_sequence(harness.server(), response_bodies).await;
    let compact_mock = mount_legacy_compact_if_needed(&harness, mode).await;

    submit_user_input(
        &codex,
        vec![UserInput::Text {
            text: "mid_turn_auto_user".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await?;

    capture_from_requests(
        mode,
        &codex,
        &rollout_path,
        &responses_mock,
        compact_mock.as_ref(),
        follow_up_index(responses_mock.requests().len()),
    )
    .await
}

async fn run_manual_hook_session(mode: Mode) -> Result<Value> {
    let response_bodies = match mode {
        Mode::Legacy => vec![responses::sse(vec![
            responses::ev_assistant_message("hook-first-message", "HOOK_FIRST_REPLY"),
            responses::ev_completed("hook-first-response"),
        ])],
        Mode::V2 => vec![
            responses::sse(vec![
                responses::ev_assistant_message("hook-first-message", "HOOK_FIRST_REPLY"),
                responses::ev_completed("hook-first-response"),
            ]),
            compaction_v2_response_body(),
        ],
    };
    let harness = build_harness(mode, RunSettings::default(), /*hooks*/ true).await?;
    let codex = harness.test().codex.clone();
    responses::mount_sse_sequence(harness.server(), response_bodies).await;
    let compact_mock = mount_legacy_compact_if_needed(&harness, mode).await;

    submit_user_input(
        &codex,
        vec![UserInput::Text {
            text: "manual_hooks_before".to_string(),
            text_elements: Vec::new(),
        }],
    )
    .await?;
    codex.submit(Op::Compact).await?;
    wait_for_turn_complete(&codex).await;

    if let Some(compact_mock) = compact_mock {
        assert_eq!(compact_mock.requests().len(), 1);
    }

    let home = harness.test().codex_home_path();
    let pre = hook_log_view(&home.join("pre_compact_manual_log.jsonl"))?;
    let post = hook_log_view(&home.join("post_compact_manual_log.jsonl"))?;
    Ok(json!({
        "pre": pre,
        "post": post,
    }))
}

async fn build_auto_harness(mode: Mode) -> Result<TestCodexHarness> {
    build_harness_inner(
        mode,
        RunSettings::default(),
        /*hooks*/ false,
        Some(200),
    )
    .await
}

async fn build_harness(mode: Mode, settings: RunSettings, hooks: bool) -> Result<TestCodexHarness> {
    build_harness_inner(mode, settings, hooks, /*auto_compact_limit*/ None).await
}

async fn build_harness_inner(
    mode: Mode,
    settings: RunSettings,
    hooks: bool,
    auto_compact_limit: Option<i64>,
) -> Result<TestCodexHarness> {
    fs::create_dir_all(FIXED_CWD)?;
    let mut builder = test_codex().with_auth(settings.auth.build());
    if hooks {
        builder = builder.with_pre_build_hook(write_manual_compact_hooks);
    }
    TestCodexHarness::with_builder(builder.with_config(move |config| {
        config.cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(PathBuf::from(
            FIXED_CWD,
        ))
        .expect("fixed cwd should be absolute");
        config.user_instructions = Some("PARITY_USER_INSTRUCTIONS".to_string());
        config.developer_instructions = Some("PARITY_DEVELOPER_INSTRUCTIONS".to_string());
        if settings.service_tier_fast {
            config.service_tier = Some(ServiceTier::Fast.request_value().to_string());
        }
        config.model_auto_compact_token_limit = auto_compact_limit;
        if hooks {
            trust_discovered_hooks(config);
        }
        if mode == Mode::V2 {
            let _ = config.features.enable(Feature::RemoteCompactionV2);
        }
    }))
    .await
}

fn rollout_path(harness: &TestCodexHarness) -> PathBuf {
    harness
        .test()
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path")
}

async fn mount_legacy_compact_if_needed(
    harness: &TestCodexHarness,
    mode: Mode,
) -> Option<ResponseMock> {
    match mode {
        Mode::Legacy => Some(
            responses::mount_compact_user_history_with_summary_once(harness.server(), SUMMARY)
                .await,
        ),
        Mode::V2 => None,
    }
}

fn follow_up_index(request_count: usize) -> usize {
    request_count.checked_sub(1).expect("follow-up request")
}

async fn capture_from_requests(
    mode: Mode,
    codex: &codex_core::CodexThread,
    rollout_path: &Path,
    responses_mock: &ResponseMock,
    compact_mock: Option<&ResponseMock>,
    follow_up_index: usize,
) -> Result<Capture> {
    let response_requests = responses_mock.requests();
    let follow_up_body = response_requests
        .get(follow_up_index)
        .expect("follow-up request should be present")
        .body_json();

    let (compact_body, compact_requests) = match (mode, compact_mock) {
        (Mode::Legacy, Some(compact_mock)) => {
            let compact_requests = compact_mock.requests().len();
            (compact_mock.single_request().body_json(), compact_requests)
        }
        (Mode::V2, None) => {
            let compact_index = follow_up_index
                .checked_sub(1)
                .expect("v2 compact request should precede follow-up");
            (response_requests[compact_index].body_json(), 0)
        }
        (Mode::Legacy, None) | (Mode::V2, Some(_)) => panic!("unexpected compact mock state"),
    };

    codex.submit(Op::Shutdown).await?;
    wait_for_event(codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    Ok(Capture {
        compact_body,
        follow_up_body,
        replacement_history: replacement_history_from_rollout(rollout_path)?,
        normal_response_requests: response_requests.len(),
        compact_requests,
    })
}

async fn submit_user_input(codex: &codex_core::CodexThread, items: Vec<UserInput>) -> Result<()> {
    codex
        .submit(Op::UserInput {
            environments: None,
            items,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;
    wait_for_turn_complete(codex).await;
    Ok(())
}

async fn wait_for_turn_complete(codex: &codex_core::CodexThread) {
    wait_for_event(codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

fn user_input_for_step(scenario_name: &str, idx: usize, step: Step) -> Vec<UserInput> {
    let mut items = Vec::new();
    if matches!(step, Step::ImageAssistant) {
        items.push(UserInput::Image {
            image_url: IMAGE_URL.to_string(),
            detail: None,
        });
    }
    items.push(UserInput::Text {
        text: format!("{}_USER_TURN_{}_{}", scenario_name, idx, step.label()),
        text_elements: Vec::new(),
    });
    items
}

fn response_bodies_for_scenario(scenario: &Scenario) -> Vec<String> {
    scenario
        .steps
        .iter()
        .enumerate()
        .flat_map(|(idx, step)| response_bodies_for_step(scenario.name, idx, *step))
        .collect()
}

fn response_bodies_for_step(scenario_name: &str, idx: usize, step: Step) -> Vec<String> {
    let response_id = format!("{scenario_name}-{idx}-{}", step.label());
    match step {
        Step::Assistant => vec![responses::sse(vec![
            responses::ev_assistant_message(
                &format!("{response_id}-message"),
                &format!("{scenario_name} assistant reply {idx}"),
            ),
            responses::ev_completed(&format!("{response_id}-response")),
        ])],
        Step::ReasoningAssistant => vec![responses::sse(vec![
            responses::ev_reasoning_item(
                &format!("{response_id}-reasoning"),
                &["PARITY_REASONING_SUMMARY"],
                &["parity raw reasoning content"],
            ),
            responses::ev_assistant_message(
                &format!("{response_id}-message"),
                &format!("{scenario_name} reasoning reply {idx}"),
            ),
            responses::ev_completed(&format!("{response_id}-response")),
        ])],
        Step::FunctionTool => vec![
            responses::sse(vec![
                responses::ev_function_call(
                    &format!("{response_id}-call"),
                    DUMMY_FUNCTION_NAME,
                    r#"{"case":"parity"}"#,
                ),
                responses::ev_completed(&format!("{response_id}-tool-response")),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message(
                    &format!("{response_id}-final-message"),
                    &format!("{scenario_name} function follow-up {idx}"),
                ),
                responses::ev_completed(&format!("{response_id}-final-response")),
            ]),
        ],
        Step::ShellTool => vec![
            responses::sse(vec![
                responses::ev_shell_command_call(
                    &format!("{response_id}-shell-call"),
                    &format!("echo {scenario_name}_{idx}_SHELL_TOOL"),
                ),
                responses::ev_completed(&format!("{response_id}-shell-response")),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message(
                    &format!("{response_id}-final-message"),
                    &format!("{scenario_name} shell follow-up {idx}"),
                ),
                responses::ev_completed(&format!("{response_id}-final-response")),
            ]),
        ],
        Step::ImageAssistant => vec![responses::sse(vec![
            responses::ev_assistant_message(
                &format!("{response_id}-message"),
                &format!("{scenario_name} image reply {idx}"),
            ),
            responses::ev_completed(&format!("{response_id}-response")),
        ])],
        Step::WebSearchAssistant => vec![responses::sse(vec![
            responses::ev_web_search_call_done(
                &format!("{response_id}-web-search"),
                "completed",
                &format!("{scenario_name} parity query"),
            ),
            responses::ev_assistant_message(
                &format!("{response_id}-message"),
                &format!("{scenario_name} web search reply {idx}"),
            ),
            responses::ev_completed(&format!("{response_id}-response")),
        ])],
    }
}

fn compaction_v2_response_body() -> String {
    responses::sse(vec![
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "compaction",
                "encrypted_content": SUMMARY,
            }
        }),
        responses::ev_completed("remote-compaction-v2-response"),
    ])
}

fn after_compact_response_body(scenario_name: &str) -> String {
    responses::sse(vec![
        responses::ev_assistant_message(
            &format!("{scenario_name}-after-compact-message"),
            &format!("{scenario_name} after compact reply"),
        ),
        responses::ev_completed(&format!("{scenario_name}-after-compact-response")),
    ])
}

fn compact_request_view(body: &Value, mode: Mode) -> Value {
    let mut input = body
        .get("input")
        .and_then(Value::as_array)
        .cloned()
        .expect("compact request should include input");
    if mode == Mode::V2 {
        let trigger = input
            .pop()
            .expect("v2 compact input should end with trigger");
        assert_eq!(
            trigger,
            json!({"type": "compaction_trigger"}),
            "v2 compact input should append exactly one compaction_trigger"
        );
    }

    let mut selected = selected_request_fields(body, SelectedFieldsMode::Compact);
    selected["input"] = normalize_value(Value::Array(input));
    canonical_json(&normalize_value(selected))
}

fn follow_up_request_view(body: &Value) -> Value {
    let mut selected = selected_request_fields(body, SelectedFieldsMode::FollowUp);
    selected["input"] = normalize_value(
        body.get("input")
            .cloned()
            .expect("follow-up request should include input"),
    );
    canonical_json(&normalize_value(selected))
}

fn replacement_history_from_rollout(path: &Path) -> Result<Value> {
    let rollout_text = fs::read_to_string(path)?;
    let mut replacement_history = None;
    for line in rollout_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(entry) = serde_json::from_str::<RolloutLine>(line) else {
            continue;
        };
        if let RolloutItem::Compacted(compacted) = entry.item
            && compacted.message.is_empty()
            && let Some(items) = compacted.replacement_history
        {
            let values = items
                .into_iter()
                .map(|item| serde_json::to_value(item).expect("serialize replacement item"))
                .collect::<Vec<_>>();
            replacement_history = Some(Value::Array(values));
        }
    }
    let replacement_history =
        replacement_history.expect("expected compacted rollout replacement history");
    Ok(canonical_json(&normalize_value(replacement_history)))
}

fn write_manual_compact_hooks(home: &Path) {
    write_hook_script(
        &home.join("pre_compact_manual.py"),
        &home.join("pre_compact_manual_log.jsonl"),
    );
    write_hook_script(
        &home.join("post_compact_manual.py"),
        &home.join("post_compact_manual_log.jsonl"),
    );
    let hooks = json!({
        "hooks": {
            "PreCompact": [{
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": python_hook_command(&home.join("pre_compact_manual.py")),
                }]
            }],
            "PostCompact": [{
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": python_hook_command(&home.join("post_compact_manual.py")),
                }]
            }]
        }
    });
    fs::write(home.join("hooks.json"), hooks.to_string()).expect("write hooks.json");
}

fn write_hook_script(script_path: &Path, log_path: &Path) {
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload, sort_keys=True) + "\n")
"#,
        log_path = log_path.display(),
    );
    fs::write(script_path, script).expect("write compact hook script");
}

fn python_hook_command(script_path: &Path) -> String {
    format!("python3 \"{}\"", script_path.display())
}

fn hook_log_view(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)?;
    let values = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let payload: Value = serde_json::from_str(line).expect("parse compact hook payload");
            json!({
                "hook_event_name": payload["hook_event_name"].clone(),
                "trigger": payload["trigger"].clone(),
                "model": payload["model"].clone(),
                "has_reason": payload.get("reason").is_some(),
                "has_phase": payload.get("phase").is_some(),
                "has_implementation": payload.get("implementation").is_some(),
                "has_status": payload.get("status").is_some(),
                "has_error": payload.get("error").is_some(),
            })
        })
        .collect::<Vec<_>>();
    Ok(Value::Array(values))
}

#[derive(Clone, Copy)]
enum SelectedFieldsMode {
    Compact,
    FollowUp,
}

fn selected_request_fields(body: &Value, mode: SelectedFieldsMode) -> Value {
    let mut selected = serde_json::Map::new();
    let fields: &[&str] = match mode {
        SelectedFieldsMode::Compact => &[
            "model",
            "instructions",
            "parallel_tool_calls",
            "reasoning",
            "service_tier",
            "prompt_cache_key",
            "text",
            "tools",
            "previous_response_id",
        ],
        SelectedFieldsMode::FollowUp => &[
            "model",
            "instructions",
            "parallel_tool_calls",
            "reasoning",
            "service_tier",
            "prompt_cache_key",
            "text",
            "tools",
            "tool_choice",
            "previous_response_id",
            "store",
            "stream",
            "include",
        ],
    };
    for field in fields {
        if let Some(value) = body.get(field) {
            selected.insert((*field).to_string(), normalize_value(value.clone()));
        }
    }
    Value::Object(selected)
}

fn normalize_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(normalize_string(&text)),
        Value::Array(values) => Value::Array(values.into_iter().map(normalize_value).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, normalize_value(value)))
                .collect(),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) => value,
    }
}

fn normalize_string(value: &str) -> String {
    if is_uuid_like(value) {
        return "<UUID>".to_string();
    }

    let mut text = value.to_string();
    normalize_tmp_prefix_before_marker(&mut text, "/skills/");
    normalize_tmp_prefix_before_marker(&mut text, "\\skills\\");

    let mut search_start = 0;
    let wall_time_prefix = "Wall time: ";
    let wall_time_suffix = " seconds";
    while let Some(relative_start) = text[search_start..].find(wall_time_prefix) {
        let value_start = search_start + relative_start + wall_time_prefix.len();
        let Some(relative_end) = text[value_start..].find(wall_time_suffix) else {
            break;
        };
        let value_end = value_start + relative_end;
        let value = &text[value_start..value_end];
        if !value.is_empty()
            && value.chars().any(|ch| ch.is_ascii_digit())
            && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
        {
            text.replace_range(value_start..value_end, "<WALL_TIME>");
            search_start = value_start + "<WALL_TIME>".len() + wall_time_suffix.len();
        } else {
            search_start = value_end + wall_time_suffix.len();
        }
    }
    text
}

fn is_uuid_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|idx| bytes[*idx] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| [8, 13, 18, 23].contains(&idx) || byte.is_ascii_hexdigit())
}

fn normalize_tmp_prefix_before_marker(text: &mut String, marker: &str) {
    let mut search_start = 0;
    while let Some(relative_marker_index) = text[search_start..].find(marker) {
        let marker_index = search_start + relative_marker_index;
        let prefix = &text[..marker_index];
        let windows_appdata_temp_start = prefix
            .rfind("/AppData/Local/Temp/.tmp")
            .and_then(|temp_index| prefix[..temp_index].rfind(":/Users/"))
            .and_then(|colon_index| colon_index.checked_sub(1))
            .or_else(|| {
                prefix
                    .rfind("\\AppData\\Local\\Temp\\.tmp")
                    .and_then(|temp_index| prefix[..temp_index].rfind(":\\Users\\"))
                    .and_then(|colon_index| colon_index.checked_sub(1))
            });
        let start = prefix
            .rfind("/private/var/folders/")
            .or_else(|| prefix.rfind("/var/folders/"))
            .or_else(|| prefix.rfind("/private/tmp/.tmp"))
            .or_else(|| prefix.rfind("/tmp/.tmp"))
            .or(windows_appdata_temp_start);
        if let Some(start_index) = start {
            text.replace_range(start_index..marker_index, "<CODEX_HOME>");
            search_start = start_index + "<CODEX_HOME>".len() + marker.len();
        } else {
            search_start = marker_index + marker.len();
        }
    }
}

#[test]
fn normalize_string_rewrites_linux_temp_skill_paths() {
    let text = normalize_string(
        "file: /tmp/.tmp5YYdK3/skills/.system/imagegen/SKILL.md and \
         /private/tmp/.tmpw3wqF9/skills/custom/SKILL.md",
    );

    assert_eq!(
        text,
        "file: <CODEX_HOME>/skills/.system/imagegen/SKILL.md and \
         <CODEX_HOME>/skills/custom/SKILL.md"
    );
}

#[test]
fn normalize_string_rewrites_windows_temp_skill_paths() {
    let text = normalize_string(
        "file: C:/Users/runneradmin/AppData/Local/Temp/.tmpDuYxa3/skills/.system/imagegen/SKILL.md and \
         C:\\Users\\runneradmin\\AppData\\Local\\Temp\\.tmpiP36Yr\\skills\\custom\\SKILL.md",
    );

    assert_eq!(
        text,
        "file: <CODEX_HOME>/skills/.system/imagegen/SKILL.md and \
         <CODEX_HOME>\\skills\\custom\\SKILL.md"
    );
}

#[test]
fn normalize_string_rewrites_shell_wall_times() {
    let text = normalize_string(
        "Exit code: 0\nWall time: 0 seconds\nOutput:\nok\n\
         Exit code: 0\nWall time: 0.1 seconds\nOutput:\nok",
    );

    assert_eq!(
        text,
        "Exit code: 0\nWall time: <WALL_TIME> seconds\nOutput:\nok\n\
         Exit code: 0\nWall time: <WALL_TIME> seconds\nOutput:\nok"
    );
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonical_json(value)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

fn remove_object_field(value: &mut Value, field: &str) {
    if let Value::Object(map) = value {
        map.remove(field);
    }
}

fn assert_json_eq(label: &str, left: &Value, right: &Value) {
    if left != right {
        panic!("{}\n{}", label, first_json_diff(left, right, "$"));
    }
}

fn first_json_diff(left: &Value, right: &Value, path: &str) -> String {
    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let mut keys = left_map.keys().chain(right_map.keys()).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            for key in keys {
                match (left_map.get(key), right_map.get(key)) {
                    (Some(left_value), Some(right_value)) if left_value != right_value => {
                        return first_json_diff(left_value, right_value, &format!("{path}.{key}"));
                    }
                    (None, Some(right_value)) => {
                        return format!(
                            "{path}.{key}: missing on left, right={}",
                            short_json(right_value)
                        );
                    }
                    (Some(left_value), None) => {
                        return format!(
                            "{path}.{key}: left={}, missing on right",
                            short_json(left_value)
                        );
                    }
                    (Some(_), Some(_)) | (None, None) => {}
                }
            }
            format!("{path}: object mismatch")
        }
        (Value::Array(left_values), Value::Array(right_values)) => {
            let len = left_values.len().min(right_values.len());
            for idx in 0..len {
                if left_values[idx] != right_values[idx] {
                    return first_json_diff(
                        &left_values[idx],
                        &right_values[idx],
                        &format!("{path}[{idx}]"),
                    );
                }
            }
            if left_values.len() != right_values.len() {
                return format!(
                    "{path}: array len left={} right={}",
                    left_values.len(),
                    right_values.len()
                );
            }
            format!("{path}: array mismatch")
        }
        _ => format!(
            "{path}: left={} right={}",
            short_json(left),
            short_json(right)
        ),
    }
}

fn short_json(value: &Value) -> String {
    let text = serde_json::to_string(value).expect("serialize short json value");
    const MAX_LEN: usize = 1000;
    if text.len() <= MAX_LEN {
        text
    } else {
        let prefix = text.chars().take(MAX_LEN).collect::<String>();
        format!("{prefix}...<{} chars>", text.len())
    }
}

fn compact_input_len(body: &Value, mode: Mode) -> usize {
    let len = body
        .get("input")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    match mode {
        Mode::Legacy => len,
        Mode::V2 => len.saturating_sub(1),
    }
}

fn follow_up_input_len(body: &Value) -> usize {
    body.get("input")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn replacement_history_len(body: &Value) -> usize {
    body.as_array().map(Vec::len).unwrap_or_default()
}
