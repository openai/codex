use anyhow::Result;
use codex_features::Feature;
use core_test_support::PathBufExt;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

const CONFIGURED_CONTEXT_WINDOW: i64 = 128_000;
const EFFECTIVE_CONTEXT_WINDOW: i64 = CONFIGURED_CONTEXT_WINDOW * 95 / 100;

fn token_budget_texts(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<token_budget>"))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_context_is_only_emitted_with_full_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;

    let second_cwd = test.workspace_path("second-cwd");
    std::fs::create_dir_all(&second_cwd)?;
    test.submit_turn_with_environments("second turn", Some(vec![local(second_cwd.abs())]))
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);

    let expected = vec![format!(
        "<token_budget>\nCurrent context window 0, window size {EFFECTIVE_CONTEXT_WINDOW} tokens\n</token_budget>"
    )];
    assert_eq!(token_budget_texts(&requests[0]), expected);
    assert_eq!(token_budget_texts(&requests[1]), expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_remaining_context_emits_on_first_threshold_crossing() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 2_500),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_completed_with_tokens("resp-2", /*total_tokens*/ 3_000),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_completed_with_tokens("resp-3", /*total_tokens*/ 5_000),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_completed_with_tokens("resp-4", /*total_tokens*/ 8_000),
            ]),
            sse(vec![ev_response_created("resp-5"), ev_completed("resp-5")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    for turn in 1..=5 {
        test.submit_turn(&format!("turn {turn}")).await?;
    }

    let requests = responses.requests();
    assert_eq!(requests.len(), 5);

    let full_context =
        "<token_budget>\nCurrent context window 0, window size 9500 tokens\n</token_budget>"
            .to_string();
    let threshold_25 =
        "<token_budget>\n7000 tokens left in the current window\n</token_budget>".to_string();
    let threshold_50 =
        "<token_budget>\n4500 tokens left in the current window\n</token_budget>".to_string();
    let threshold_75 =
        "<token_budget>\n1500 tokens left in the current window\n</token_budget>".to_string();

    assert_eq!(token_budget_texts(&requests[0]), vec![full_context.clone()]);
    assert_eq!(
        token_budget_texts(&requests[1]),
        vec![full_context.clone(), threshold_25.clone()]
    );
    assert_eq!(
        token_budget_texts(&requests[2]),
        vec![full_context.clone(), threshold_25.clone()]
    );
    assert_eq!(
        token_budget_texts(&requests[3]),
        vec![
            full_context.clone(),
            threshold_25.clone(),
            threshold_50.clone()
        ]
    );
    assert_eq!(
        token_budget_texts(&requests[4]),
        vec![full_context, threshold_25, threshold_50, threshold_75]
    );

    Ok(())
}
