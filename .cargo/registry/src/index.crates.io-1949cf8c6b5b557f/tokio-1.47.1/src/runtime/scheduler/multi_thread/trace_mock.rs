pub(super) struct TraceStatus {}

impl TraceStatus {
    pub(super) fn new(_: usize) -> Self {
        Self {}
    }

    pub(super) fn trace_requested(&self) -> bool {
        false
    }
}
