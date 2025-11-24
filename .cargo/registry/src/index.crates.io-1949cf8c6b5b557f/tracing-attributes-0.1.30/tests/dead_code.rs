use tracing_attributes::instrument;

#[deny(unfulfilled_lint_expectations)]
#[expect(dead_code)]
#[instrument]
fn unused() {}

#[expect(dead_code)]
#[instrument]
async fn unused_async() {}
