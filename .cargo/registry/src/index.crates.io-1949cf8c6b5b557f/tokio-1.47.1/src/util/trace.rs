cfg_rt! {
    use std::marker::PhantomData;

    #[derive(Copy, Clone)]
    pub(crate) struct SpawnMeta<'a> {
        /// The name of the task
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        pub(crate) name: Option<&'a str>,
        /// The original size of the future or function being spawned
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        pub(crate) original_size: usize,
        /// The source code location where the task was spawned.
        ///
        /// This is wrapped in a type that may be empty when `tokio_unstable` is
        /// not enabled.
        pub(crate) spawned_at: crate::runtime::task::SpawnLocation,
        _pd: PhantomData<&'a ()>,
    }

    impl<'a> SpawnMeta<'a> {
        /// Create new spawn meta with a name and original size (before possible auto-boxing)
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        #[track_caller]
        pub(crate) fn new(name: Option<&'a str>, original_size: usize) -> Self {
            Self {
                name,
                original_size,
                spawned_at: crate::runtime::task::SpawnLocation::capture(),
                _pd: PhantomData,
            }
        }

        /// Create a new unnamed spawn meta with the original size (before possible auto-boxing)
        #[track_caller]
        pub(crate) fn new_unnamed(original_size: usize) -> Self {
            #[cfg(not(all(tokio_unstable, feature = "tracing")))]
            let _original_size = original_size;

            Self {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                name: None,
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                original_size,
                spawned_at: crate::runtime::task::SpawnLocation::capture(),
                _pd: PhantomData,
            }
        }
    }

    cfg_trace! {
        use core::{
            pin::Pin,
            task::{Context, Poll},
        };
        use pin_project_lite::pin_project;
        use std::mem;
        use std::future::Future;
        use tracing::instrument::Instrument;
        pub(crate) use tracing::instrument::Instrumented;

        #[inline]
        pub(crate) fn task<F>(task: F, kind: &'static str, meta: SpawnMeta<'_>, id: u64) -> Instrumented<F> {
            fn get_span(kind: &'static str, spawn_meta: SpawnMeta<'_>, id: u64, task_size: usize) -> tracing::Span {
                let original_size = if spawn_meta.original_size != task_size {
                    Some(spawn_meta.original_size)
                } else {
                    None
                };
                tracing::trace_span!(
                    target: "tokio::task",
                    parent: None,
                    "runtime.spawn",
                    %kind,
                    task.name = %spawn_meta.name.unwrap_or_default(),
                    task.id = id,
                    original_size.bytes = original_size,
                    size.bytes = task_size,
                    loc.file = spawn_meta.spawned_at.0.file(),
                    loc.line = spawn_meta.spawned_at.0.line(),
                    loc.col = spawn_meta.spawned_at.0.column(),
                )
            }
            use tracing::instrument::Instrument;
            let span = get_span(kind, meta, id, mem::size_of::<F>());
            task.instrument(span)
        }

        #[inline]
        pub(crate) fn blocking_task<Fn, Fut>(task: Fut, spawn_meta: SpawnMeta<'_>, id: u64) -> Instrumented<Fut> {
            let fn_size = mem::size_of::<Fn>();
            let original_size = if spawn_meta.original_size != fn_size {
                Some(spawn_meta.original_size)
            } else {
                None
            };

            let span = tracing::trace_span!(
                target: "tokio::task::blocking",
                "runtime.spawn",
                kind = %"blocking",
                task.name = %spawn_meta.name.unwrap_or_default(),
                task.id = id,
                "fn" = %std::any::type_name::<Fn>(),
                original_size.bytes = original_size,
                size.bytes = fn_size,
                loc.file = spawn_meta.spawned_at.0.file(),
                loc.line = spawn_meta.spawned_at.0.line(),
                loc.col = spawn_meta.spawned_at.0.column(),
            );
            task.instrument(span)

        }

        pub(crate) fn async_op<P,F>(inner: P, resource_span: tracing::Span, source: &str, poll_op_name: &'static str, inherits_child_attrs: bool) -> InstrumentedAsyncOp<F>
        where P: FnOnce() -> F {
            resource_span.in_scope(|| {
                let async_op_span = tracing::trace_span!("runtime.resource.async_op", source = source, inherits_child_attrs = inherits_child_attrs);
                let enter = async_op_span.enter();
                let async_op_poll_span = tracing::trace_span!("runtime.resource.async_op.poll");
                let inner = inner();
                drop(enter);
                let tracing_ctx = AsyncOpTracingCtx {
                    async_op_span,
                    async_op_poll_span,
                    resource_span: resource_span.clone(),
                };
                InstrumentedAsyncOp {
                    inner,
                    tracing_ctx,
                    poll_op_name,
                }
            })
        }

        #[derive(Debug, Clone)]
        pub(crate) struct AsyncOpTracingCtx {
            pub(crate) async_op_span: tracing::Span,
            pub(crate) async_op_poll_span: tracing::Span,
            pub(crate) resource_span: tracing::Span,
        }


        pin_project! {
            #[derive(Debug, Clone)]
            pub(crate) struct InstrumentedAsyncOp<F> {
                #[pin]
                pub(crate) inner: F,
                pub(crate) tracing_ctx: AsyncOpTracingCtx,
                pub(crate) poll_op_name: &'static str
            }
        }

        impl<F: Future> Future for InstrumentedAsyncOp<F> {
            type Output = F::Output;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let this = self.project();
                let poll_op_name = &*this.poll_op_name;
                let _res_enter = this.tracing_ctx.resource_span.enter();
                let _async_op_enter = this.tracing_ctx.async_op_span.enter();
                let _async_op_poll_enter = this.tracing_ctx.async_op_poll_span.enter();
                trace_poll_op!(poll_op_name, this.inner.poll(cx))
            }
        }
    }

    cfg_not_trace! {
        #[inline]
        pub(crate) fn task<F>(task: F, _kind: &'static str, _meta: SpawnMeta<'_>, _id: u64) -> F {
            // nop
            task
        }

        #[inline]
        pub(crate) fn blocking_task<Fn, Fut>(task: Fut, _spawn_meta: SpawnMeta<'_>, _id: u64) -> Fut {
            let _ = PhantomData::<&Fn>;
            // nop
            task
        }
    }
}

cfg_time! {
    #[track_caller]
    pub(crate) fn caller_location() -> Option<&'static std::panic::Location<'static>> {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        return Some(std::panic::Location::caller());
        #[cfg(not(all(tokio_unstable, feature = "tracing")))]
        None
    }
}
