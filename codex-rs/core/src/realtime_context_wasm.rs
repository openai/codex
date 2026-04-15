use crate::codex::Session;

pub(crate) async fn build_realtime_startup_context(
    _sess: &Session,
    _budget_tokens: usize,
) -> Option<String> {
    None
}
