use super::{EnterRuntime, CONTEXT};

/// Returns true if in a runtime context.
pub(crate) fn current_enter_context() -> EnterRuntime {
    CONTEXT.with(|c| c.runtime.get())
}

/// Forces the current "entered" state to be cleared while the closure
/// is executed.
pub(crate) fn exit_runtime<F: FnOnce() -> R, R>(f: F) -> R {
    // Reset in case the closure panics
    struct Reset(EnterRuntime);

    impl Drop for Reset {
        fn drop(&mut self) {
            CONTEXT.with(|c| {
                assert!(
                    !c.runtime.get().is_entered(),
                    "closure claimed permanent executor"
                );
                c.runtime.set(self.0);
            });
        }
    }

    let was = CONTEXT.with(|c| {
        let e = c.runtime.get();
        assert!(e.is_entered(), "asked to exit when not entered");
        c.runtime.set(EnterRuntime::NotEntered);
        e
    });

    let _reset = Reset(was);
    // dropping _reset after f() will reset ENTERED
    f()
}
