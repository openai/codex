use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

pub(crate) struct CountingCloneableState {
    state: Arc<InnerState>,
}

struct InnerState {
    setup_done: AtomicBool,
    count: AtomicUsize,
}

impl CountingCloneableState {
    pub(crate) fn new() -> Self {
        let inner_state = InnerState {
            setup_done: AtomicBool::new(false),
            count: AtomicUsize::new(0),
        };
        CountingCloneableState {
            state: Arc::new(inner_state),
        }
    }

    pub(crate) fn setup_done(&self) {
        self.state.setup_done.store(true, Ordering::SeqCst);
    }

    pub(crate) fn count(&self) -> usize {
        self.state.count.load(Ordering::SeqCst)
    }
}

impl Clone for CountingCloneableState {
    fn clone(&self) -> Self {
        let state = self.state.clone();
        if state.setup_done.load(Ordering::SeqCst) {
            let bt = std::backtrace::Backtrace::force_capture();
            let bt = bt
                .to_string()
                .lines()
                .filter(|line| line.contains("axum") || line.contains("./src"))
                .collect::<Vec<_>>()
                .join("\n");
            println!("AppState::Clone:\n===============\n{bt}\n");
            state.count.fetch_add(1, Ordering::SeqCst);
        }

        CountingCloneableState { state }
    }
}
