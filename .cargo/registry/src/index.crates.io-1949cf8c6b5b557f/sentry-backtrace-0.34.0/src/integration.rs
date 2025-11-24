use std::thread;

use sentry_core::protocol::{Event, Thread};
use sentry_core::{ClientOptions, Integration};

use crate::current_stacktrace;
use crate::process::process_event_stacktrace;

/// Integration to process Event stacktraces.
///
/// This integration will trim backtraces, depending on the `trim_backtraces`
/// and `extra_border_frames` options.
/// It will then classify each frame according to the `in_app_include` and
/// `in_app_exclude` options.
#[derive(Debug, Default)]
pub struct ProcessStacktraceIntegration;

impl ProcessStacktraceIntegration {
    /// Creates a new Integration to process stacktraces.
    pub fn new() -> Self {
        Self
    }
}

impl Integration for ProcessStacktraceIntegration {
    fn name(&self) -> &'static str {
        "process-stacktrace"
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        options: &ClientOptions,
    ) -> Option<Event<'static>> {
        for exc in &mut event.exception {
            if let Some(ref mut stacktrace) = exc.stacktrace {
                process_event_stacktrace(stacktrace, options);
            }
        }
        for th in &mut event.threads {
            if let Some(ref mut stacktrace) = th.stacktrace {
                process_event_stacktrace(stacktrace, options);
            }
        }
        if let Some(ref mut stacktrace) = event.stacktrace {
            process_event_stacktrace(stacktrace, options);
        }
        Some(event)
    }
}

/// Integration to attach stacktraces to Events.
///
/// This integration will add an additional thread backtrace to captured
/// messages, respecting the `attach_stacktrace` option.
#[derive(Debug, Default)]
pub struct AttachStacktraceIntegration;

impl AttachStacktraceIntegration {
    /// Creates a new Integration to attach stacktraces to Events.
    pub fn new() -> Self {
        Self
    }
}

impl Integration for AttachStacktraceIntegration {
    fn name(&self) -> &'static str {
        "attach-stacktrace"
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        options: &ClientOptions,
    ) -> Option<Event<'static>> {
        if options.attach_stacktrace && !has_stacktrace(&event) {
            let thread = current_thread(true);
            if thread.stacktrace.is_some() {
                event.threads.values.push(thread);
            }
        }
        Some(event)
    }
}

fn has_stacktrace(event: &Event) -> bool {
    event.stacktrace.is_some()
        || event.exception.iter().any(|exc| exc.stacktrace.is_some())
        || event.threads.iter().any(|thrd| thrd.stacktrace.is_some())
}

/// Captures information about the current thread.
///
/// If `with_stack` is set to `true` the current stacktrace is
/// attached.
pub fn current_thread(with_stack: bool) -> Thread {
    // NOTE: `as_u64` is nightly only
    // See https://github.com/rust-lang/rust/issues/67939
    let thread_id: u64 = unsafe { std::mem::transmute(thread::current().id()) };
    Thread {
        id: Some(thread_id.to_string().into()),
        name: thread::current().name().map(str::to_owned),
        current: true,
        stacktrace: if with_stack {
            current_stacktrace()
        } else {
            None
        },
        ..Default::default()
    }
}
