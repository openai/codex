//! Bounded per-isolate rejection tracking and script error capture.

use codex_plugin_host::protocol::ErrorCode;
use codex_plugin_host::protocol::MAX_SCRIPT_ERROR_BYTES;
use codex_plugin_host::protocol::PluginScriptError;

const MAX_REJECTIONS: usize = 32;

#[derive(Default)]
struct Failures {
    rejected: Vec<v8::Global<v8::Promise>>,
    overflowed: bool,
    script_error: Option<PluginScriptError>,
}

pub(super) fn install(isolate: &mut v8::OwnedIsolate) {
    isolate.set_slot(Failures::default());
    isolate.set_promise_reject_callback(rejected);
}

extern "C" fn rejected(message: v8::PromiseRejectMessage) {
    // SAFETY: V8 supplies the callback's active isolate and scope.
    v8::callback_scope!(unsafe scope, &message);
    match message.get_event() {
        v8::PromiseRejectEvent::PromiseRejectWithNoHandler => {
            let promise = v8::Global::new(scope, message.get_promise());
            let Some(state) = scope.get_slot_mut::<Failures>() else {
                return;
            };
            if state.rejected.len() == MAX_REJECTIONS {
                state.overflowed = true;
            } else {
                state.rejected.push(promise);
            }
        }
        v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject => {
            let promise = message.get_promise();
            if let Some(state) = scope.get_slot_mut::<Failures>() {
                state.rejected.retain(|candidate| candidate != &promise);
            }
        }
        v8::PromiseRejectEvent::PromiseRejectAfterResolved
        | v8::PromiseRejectEvent::PromiseResolveAfterResolved => {}
    }
}

pub(super) fn checkpoint(scope: &mut v8::PinScope<'_, '_>) -> Result<(), ErrorCode> {
    scope.perform_microtask_checkpoint();
    if scope.is_execution_terminating() {
        return Err(ErrorCode::ExecutionFailed);
    }
    let (rejected, overflowed) = {
        let state = scope
            .get_slot_mut::<Failures>()
            .ok_or(ErrorCode::ExecutionFailed)?;
        (
            std::mem::take(&mut state.rejected),
            std::mem::take(&mut state.overflowed),
        )
    };
    if let Some(promise) = rejected.first() {
        let promise = v8::Local::new(scope, promise);
        record_rejection(scope, promise);
        return Err(ErrorCode::ExecutionFailed);
    }
    if overflowed {
        return Err(ErrorCode::QuotaExceeded);
    }
    Ok(())
}

pub(super) fn record_rejection(scope: &mut v8::PinScope<'_, '_>, promise: v8::Local<v8::Promise>) {
    let reason = promise.result(scope);
    let message = v8::Exception::create_message(scope, reason);
    record(scope, Some(message));
}

pub(super) fn record(scope: &mut v8::PinScope<'_, '_>, message: Option<v8::Local<v8::Message>>) {
    let Some(message) = message else { return };
    let text = message.get(scope);
    let full_len = text.utf8_length(scope);
    let mut bytes = vec![0; full_len.min(MAX_SCRIPT_ERROR_BYTES)];
    let len = text.write_utf8_v2(scope, &mut bytes, v8::WriteFlags::kReplaceInvalidUtf8, None);
    let script_error = PluginScriptError {
        message: String::from_utf8_lossy(&bytes[..len]).into_owned(),
        line: message.get_line_number(scope),
        column: message.get_start_column().checked_add(1),
        truncated: len < full_len,
    };
    if let Some(state) = scope.get_slot_mut::<Failures>() {
        state.script_error = Some(script_error);
    }
}

pub(super) fn take(isolate: &mut v8::Isolate) -> Option<PluginScriptError> {
    isolate
        .get_slot_mut::<Failures>()
        .and_then(|state| state.script_error.take())
}
