use std::fmt;
use std::future::Future;
use std::panic;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::runtime::task::AbortHandle;
use crate::runtime::Builder;
use crate::sync::oneshot;
use crate::task::JoinHandle;

use futures::future::FutureExt;

// Enums for each option in the combinations being tested

#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiRuntime {
    CurrentThread,
    Multi1,
    Multi2,
}
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiLocalSet {
    Yes,
    No,
}
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiTask {
    PanicOnRun,
    PanicOnDrop,
    PanicOnRunAndDrop,
    NoPanic,
}
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiOutput {
    PanicOnDrop,
    NoPanic,
}
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiJoinInterest {
    Polled,
    NotPolled,
}
#[allow(clippy::enum_variant_names)] // we aren't using glob imports
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiJoinHandle {
    DropImmediately = 1,
    DropFirstPoll = 2,
    DropAfterNoConsume = 3,
    DropAfterConsume = 4,
}
#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiAbort {
    NotAborted = 0,
    AbortedImmediately = 1,
    AbortedFirstPoll = 2,
    AbortedAfterFinish = 3,
    AbortedAfterConsumeOutput = 4,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum CombiAbortSource {
    JoinHandle,
    AbortHandle,
}

#[test]
#[cfg_attr(panic = "abort", ignore)]
fn test_combinations() {
    let mut rt = &[
        CombiRuntime::CurrentThread,
        CombiRuntime::Multi1,
        CombiRuntime::Multi2,
    ][..];

    if cfg!(miri) {
        rt = &[CombiRuntime::CurrentThread];
    }

    let ls = [CombiLocalSet::Yes, CombiLocalSet::No];
    let task = [
        CombiTask::NoPanic,
        CombiTask::PanicOnRun,
        CombiTask::PanicOnDrop,
        CombiTask::PanicOnRunAndDrop,
    ];
    let output = [CombiOutput::NoPanic, CombiOutput::PanicOnDrop];
    let ji = [CombiJoinInterest::Polled, CombiJoinInterest::NotPolled];
    let jh = [
        CombiJoinHandle::DropImmediately,
        CombiJoinHandle::DropFirstPoll,
        CombiJoinHandle::DropAfterNoConsume,
        CombiJoinHandle::DropAfterConsume,
    ];
    let abort = [
        CombiAbort::NotAborted,
        CombiAbort::AbortedImmediately,
        CombiAbort::AbortedFirstPoll,
        CombiAbort::AbortedAfterFinish,
        CombiAbort::AbortedAfterConsumeOutput,
    ];
    let ah = [
        None,
        Some(CombiJoinHandle::DropImmediately),
        Some(CombiJoinHandle::DropFirstPoll),
        Some(CombiJoinHandle::DropAfterNoConsume),
        Some(CombiJoinHandle::DropAfterConsume),
    ];

    for rt in rt.iter().copied() {
        for ls in ls.iter().copied() {
            for task in task.iter().copied() {
                for output in output.iter().copied() {
                    for ji in ji.iter().copied() {
                        for jh in jh.iter().copied() {
                            for abort in abort.iter().copied() {
                                // abort via join handle --- abort  handles
                                // may be dropped at any point
                                for ah in ah.iter().copied() {
                                    test_combination(
                                        rt,
                                        ls,
                                        task,
                                        output,
                                        ji,
                                        jh,
                                        ah,
                                        abort,
                                        CombiAbortSource::JoinHandle,
                                    );
                                }
                                // if aborting via AbortHandle, it will
                                // never be dropped.
                                test_combination(
                                    rt,
                                    ls,
                                    task,
                                    output,
                                    ji,
                                    jh,
                                    None,
                                    abort,
                                    CombiAbortSource::AbortHandle,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn is_debug<T: fmt::Debug>(_: &T) {}

#[allow(clippy::too_many_arguments)]
fn test_combination(
    rt: CombiRuntime,
    ls: CombiLocalSet,
    task: CombiTask,
    output: CombiOutput,
    ji: CombiJoinInterest,
    jh: CombiJoinHandle,
    ah: Option<CombiJoinHandle>,
    abort: CombiAbort,
    abort_src: CombiAbortSource,
) {
    match (abort_src, ah) {
        (CombiAbortSource::JoinHandle, _) if (jh as usize) < (abort as usize) => {
            // join handle dropped prior to abort
            return;
        }
        (CombiAbortSource::AbortHandle, Some(_)) => {
            // abort handle dropped, we can't abort through the
            // abort handle
            return;
        }

        _ => {}
    }

    if (task == CombiTask::PanicOnDrop) && (output == CombiOutput::PanicOnDrop) {
        // this causes double panic
        return;
    }
    if (task == CombiTask::PanicOnRunAndDrop) && (abort != CombiAbort::AbortedImmediately) {
        // this causes double panic
        return;
    }

    is_debug(&rt);
    is_debug(&ls);
    is_debug(&task);
    is_debug(&output);
    is_debug(&ji);
    is_debug(&jh);
    is_debug(&ah);
    is_debug(&abort);
    is_debug(&abort_src);

    // A runtime optionally with a LocalSet
    struct Rt {
        rt: crate::runtime::Runtime,
        ls: Option<crate::task::LocalSet>,
    }
    impl Rt {
        fn new(rt: CombiRuntime, ls: CombiLocalSet) -> Self {
            let rt = match rt {
                CombiRuntime::CurrentThread => Builder::new_current_thread().build().unwrap(),
                CombiRuntime::Multi1 => Builder::new_multi_thread()
                    .worker_threads(1)
                    .build()
                    .unwrap(),
                CombiRuntime::Multi2 => Builder::new_multi_thread()
                    .worker_threads(2)
                    .build()
                    .unwrap(),
            };

            let ls = match ls {
                CombiLocalSet::Yes => Some(crate::task::LocalSet::new()),
                CombiLocalSet::No => None,
            };

            Self { rt, ls }
        }
        fn block_on<T>(&self, task: T) -> T::Output
        where
            T: Future,
        {
            match &self.ls {
                Some(ls) => ls.block_on(&self.rt, task),
                None => self.rt.block_on(task),
            }
        }
        fn spawn<T>(&self, task: T) -> JoinHandle<T::Output>
        where
            T: Future + Send + 'static,
            T::Output: Send + 'static,
        {
            match &self.ls {
                Some(ls) => ls.spawn_local(task),
                None => self.rt.spawn(task),
            }
        }
    }

    // The type used for the output of the future
    struct Output {
        panic_on_drop: bool,
        on_drop: Option<oneshot::Sender<()>>,
    }
    impl Output {
        fn disarm(&mut self) {
            self.panic_on_drop = false;
        }
    }
    impl Drop for Output {
        fn drop(&mut self) {
            let _ = self.on_drop.take().unwrap().send(());
            if self.panic_on_drop {
                panic!("Panicking in Output");
            }
        }
    }

    // A wrapper around the future that is spawned
    struct FutWrapper<F> {
        inner: F,
        on_drop: Option<oneshot::Sender<()>>,
        panic_on_drop: bool,
    }
    impl<F: Future> Future for FutWrapper<F> {
        type Output = F::Output;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
            unsafe {
                let me = Pin::into_inner_unchecked(self);
                let inner = Pin::new_unchecked(&mut me.inner);
                inner.poll(cx)
            }
        }
    }
    impl<F> Drop for FutWrapper<F> {
        fn drop(&mut self) {
            let _: Result<(), ()> = self.on_drop.take().unwrap().send(());
            if self.panic_on_drop {
                panic!("Panicking in FutWrapper");
            }
        }
    }

    // The channels passed to the task
    struct Signals {
        on_first_poll: Option<oneshot::Sender<()>>,
        wait_complete: Option<oneshot::Receiver<()>>,
        on_output_drop: Option<oneshot::Sender<()>>,
    }

    // The task we will spawn
    async fn my_task(mut signal: Signals, task: CombiTask, out: CombiOutput) -> Output {
        // Signal that we have been polled once
        let _ = signal.on_first_poll.take().unwrap().send(());

        // Wait for a signal, then complete the future
        let _ = signal.wait_complete.take().unwrap().await;

        // If the task gets past wait_complete without yielding, then aborts
        // may not be caught without this yield_now.
        crate::task::yield_now().await;

        if task == CombiTask::PanicOnRun || task == CombiTask::PanicOnRunAndDrop {
            panic!("Panicking in my_task on {:?}", std::thread::current().id());
        }

        Output {
            panic_on_drop: out == CombiOutput::PanicOnDrop,
            on_drop: signal.on_output_drop.take(),
        }
    }

    let rt = Rt::new(rt, ls);

    let (on_first_poll, wait_first_poll) = oneshot::channel();
    let (on_complete, wait_complete) = oneshot::channel();
    let (on_future_drop, wait_future_drop) = oneshot::channel();
    let (on_output_drop, wait_output_drop) = oneshot::channel();
    let signal = Signals {
        on_first_poll: Some(on_first_poll),
        wait_complete: Some(wait_complete),
        on_output_drop: Some(on_output_drop),
    };

    // === Spawn task ===
    let mut handle = Some(rt.spawn(FutWrapper {
        inner: my_task(signal, task, output),
        on_drop: Some(on_future_drop),
        panic_on_drop: task == CombiTask::PanicOnDrop || task == CombiTask::PanicOnRunAndDrop,
    }));

    // Keep track of whether the task has been killed with an abort
    let mut aborted = false;

    // If we want to poll the JoinHandle, do it now
    if ji == CombiJoinInterest::Polled {
        assert!(
            handle.as_mut().unwrap().now_or_never().is_none(),
            "Polling handle succeeded"
        );
    }

    // If we are either aborting the task via an abort handle, or dropping via
    // an abort handle, do that now.
    let mut abort_handle = if ah.is_some() || abort_src == CombiAbortSource::AbortHandle {
        handle.as_ref().map(JoinHandle::abort_handle)
    } else {
        None
    };

    let do_abort = |abort_handle: &mut Option<AbortHandle>,
                    join_handle: Option<&mut JoinHandle<_>>| {
        match abort_src {
            CombiAbortSource::AbortHandle => abort_handle.take().unwrap().abort(),
            CombiAbortSource::JoinHandle => join_handle.unwrap().abort(),
        }
    };

    if abort == CombiAbort::AbortedImmediately {
        do_abort(&mut abort_handle, handle.as_mut());
        aborted = true;
    }
    if jh == CombiJoinHandle::DropImmediately {
        drop(handle.take().unwrap());
    }

    // === Wait for first poll ===
    let got_polled = rt.block_on(wait_first_poll).is_ok();
    if !got_polled {
        // it's possible that we are aborted but still got polled
        assert!(
            aborted,
            "Task completed without ever being polled but was not aborted."
        );
    }

    if abort == CombiAbort::AbortedFirstPoll {
        do_abort(&mut abort_handle, handle.as_mut());
        aborted = true;
    }
    if jh == CombiJoinHandle::DropFirstPoll {
        drop(handle.take().unwrap());
    }
    if ah == Some(CombiJoinHandle::DropFirstPoll) {
        drop(abort_handle.take().unwrap());
    }

    // Signal the future that it can return now
    let _ = on_complete.send(());
    // === Wait for future to be dropped ===
    assert!(
        rt.block_on(wait_future_drop).is_ok(),
        "The future should always be dropped."
    );

    if abort == CombiAbort::AbortedAfterFinish {
        // Don't set aborted to true here as the task already finished
        do_abort(&mut abort_handle, handle.as_mut());
    }
    if jh == CombiJoinHandle::DropAfterNoConsume {
        if ah == Some(CombiJoinHandle::DropAfterNoConsume) {
            drop(handle.take().unwrap());
            // The runtime will usually have dropped every ref-count at this point,
            // in which case dropping the AbortHandle drops the output.
            //
            // (But it might race and still hold a ref-count)
            let panic = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                drop(abort_handle.take().unwrap());
            }));
            if panic.is_err() {
                assert!(
                    (output == CombiOutput::PanicOnDrop)
                        && (!matches!(task, CombiTask::PanicOnRun | CombiTask::PanicOnRunAndDrop))
                        && !aborted,
                    "Dropping AbortHandle shouldn't panic here"
                );
            }
        } else {
            // The runtime will usually have dropped every ref-count at this point,
            // in which case dropping the JoinHandle drops the output.
            //
            // (But it might race and still hold a ref-count)
            let panic = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                drop(handle.take().unwrap());
            }));
            if panic.is_err() {
                assert!(
                    (output == CombiOutput::PanicOnDrop)
                        && (!matches!(task, CombiTask::PanicOnRun | CombiTask::PanicOnRunAndDrop))
                        && !aborted,
                    "Dropping JoinHandle shouldn't panic here"
                );
            }
        }
    }

    // Check whether we drop after consuming the output
    if jh == CombiJoinHandle::DropAfterConsume {
        // Using as_mut() to not immediately drop the handle
        let result = rt.block_on(handle.as_mut().unwrap());

        match result {
            Ok(mut output) => {
                // Don't panic here.
                output.disarm();
                assert!(!aborted, "Task was aborted but returned output");
            }
            Err(err) if err.is_cancelled() => assert!(aborted, "Cancelled output but not aborted"),
            Err(err) if err.is_panic() => {
                assert!(
                    (task == CombiTask::PanicOnRun)
                        || (task == CombiTask::PanicOnDrop)
                        || (task == CombiTask::PanicOnRunAndDrop)
                        || (output == CombiOutput::PanicOnDrop),
                    "Panic but nothing should panic"
                );
            }
            _ => unreachable!(),
        }

        let mut handle = handle.take().unwrap();
        if abort == CombiAbort::AbortedAfterConsumeOutput {
            do_abort(&mut abort_handle, Some(&mut handle));
        }
        drop(handle);

        if ah == Some(CombiJoinHandle::DropAfterConsume) {
            drop(abort_handle.take());
        }
    }

    // The output should have been dropped now. Check whether the output
    // object was created at all.
    let output_created = rt.block_on(wait_output_drop).is_ok();
    assert_eq!(
        output_created,
        (!matches!(task, CombiTask::PanicOnRun | CombiTask::PanicOnRunAndDrop)) && !aborted,
        "Creation of output object"
    );
}
