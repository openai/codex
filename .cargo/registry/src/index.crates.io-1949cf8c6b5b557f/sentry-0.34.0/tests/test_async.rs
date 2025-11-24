// See https://github.com/getsentry/sentry-rust/issues/184
#[tokio::test]
async fn test_nested_async_runtimes() {
    let _guard = sentry::init("https://public@example.com/42");
}
