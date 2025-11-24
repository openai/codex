use crate::runtime::{Builder, Handle};

#[test]
fn join_handle_cancel_on_shutdown() {
    let mut builder = loom::model::Builder::new();
    builder.preemption_bound = Some(2);
    builder.check(|| {
        use futures::future::FutureExt;

        let rt = Builder::new_multi_thread()
            .worker_threads(2)
            .build()
            .unwrap();

        let handle = rt.block_on(async move { Handle::current() });

        let jh1 = handle.spawn(futures::future::pending::<()>());

        drop(rt);

        let jh2 = handle.spawn(futures::future::pending::<()>());

        let err1 = jh1.now_or_never().unwrap().unwrap_err();
        let err2 = jh2.now_or_never().unwrap().unwrap_err();
        assert!(err1.is_cancelled());
        assert!(err2.is_cancelled());
    });
}
