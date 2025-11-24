cfg_rt! {
    pub(crate) use crate::runtime::spawn_blocking;

    cfg_fs! {
        #[allow(unused_imports)]
        pub(crate) use crate::runtime::spawn_mandatory_blocking;
    }

    pub(crate) use crate::task::JoinHandle;
}

cfg_not_rt! {
    use std::fmt;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    pub(crate) fn spawn_blocking<F, R>(_f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        assert_send_sync::<JoinHandle<std::cell::Cell<()>>>();
        panic!("requires the `rt` Tokio feature flag")
    }

    cfg_fs! {
        pub(crate) fn spawn_mandatory_blocking<F, R>(_f: F) -> Option<JoinHandle<R>>
        where
            F: FnOnce() -> R + Send + 'static,
            R: Send + 'static,
        {
            panic!("requires the `rt` Tokio feature flag")
        }
    }

    pub(crate) struct JoinHandle<R> {
        _p: std::marker::PhantomData<R>,
    }

    unsafe impl<T: Send> Send for JoinHandle<T> {}
    unsafe impl<T: Send> Sync for JoinHandle<T> {}

    impl<R> Future for JoinHandle<R> {
        type Output = Result<R, std::io::Error>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            unreachable!()
        }
    }

    impl<T> fmt::Debug for JoinHandle<T>
    where
        T: fmt::Debug,
    {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt.debug_struct("JoinHandle").finish()
        }
    }

    fn assert_send_sync<T: Send + Sync>() {
    }
}
