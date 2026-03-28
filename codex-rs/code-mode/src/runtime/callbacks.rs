use crate::response::FunctionCallOutputContentItem;

use super::EXIT_SENTINEL;
use super::RuntimeEvent;
use super::RuntimeState;
use super::value::json_to_v8;
use super::value::normalize_output_image;
use super::value::serialize_output_text;
use super::value::throw_type_error;
use super::value::v8_value_to_json;

type CallbackResult<'s, T = ()> = Result<T, String>;

struct CallbackThrow<'s>(v8::Local<'s, v8::Value>);

trait CallbackReturn<'s> {
    fn complete(self, scope: &mut v8::PinScope<'s, '_>, retval: &mut v8::ReturnValue<v8::Value>);
}

impl<'s> CallbackReturn<'s> for () {
    fn complete(self, _scope: &mut v8::PinScope<'s, '_>, _retval: &mut v8::ReturnValue<v8::Value>) {
    }
}

impl<'s> CallbackReturn<'s> for v8::Local<'s, v8::Value> {
    fn complete(self, _scope: &mut v8::PinScope<'s, '_>, retval: &mut v8::ReturnValue<v8::Value>) {
        retval.set(self);
    }
}

impl<'s> CallbackReturn<'s> for Option<v8::Local<'s, v8::Value>> {
    fn complete(self, _scope: &mut v8::PinScope<'s, '_>, retval: &mut v8::ReturnValue<v8::Value>) {
        if let Some(value) = self {
            retval.set(value);
        }
    }
}

impl<'s> CallbackReturn<'s> for CallbackThrow<'s> {
    fn complete(self, scope: &mut v8::PinScope<'s, '_>, _retval: &mut v8::ReturnValue<v8::Value>) {
        scope.throw_exception(self.0);
    }
}

// Keep each exported V8 callback as a thin adapter over an inner Result-returning
// implementation. This macro handles the final wiring from that Result into V8 by
// either leaving the return value alone, setting it, or throwing. A macro keeps
// the callsite readable without introducing lifetime friction from a generic helper.
macro_rules! run_callback {
    ($scope:expr, $retval:expr, $result:expr) => {
        match $result {
            Ok(value) => value.complete($scope, &mut $retval),
            Err(error_text) => throw_type_error($scope, &error_text),
        }
    };
}

pub(super) fn tool_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, tool_callback_inner(scope, args));
}

fn tool_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s, v8::Local<'s, v8::Value>> {
    let tool_name = args.data().to_rust_string_lossy(scope);
    let input = if args.length() == 0 {
        None
    } else {
        v8_value_to_json(scope, args.get(0))?
    };
    let resolver = v8::PromiseResolver::new(scope)
        .ok_or_else(|| "failed to create tool promise".to_string())?;
    let promise = resolver.get_promise(scope);

    let resolver = v8::Global::new(scope, resolver);
    let state = scope
        .get_slot_mut::<RuntimeState>()
        .ok_or_else(|| "runtime state unavailable".to_string())?;
    let next_tool_call_id = state.next_tool_call_id;
    let id = format!("tool-{next_tool_call_id}");
    state.next_tool_call_id = state.next_tool_call_id.saturating_add(1);
    let event_tx = state.event_tx.clone();
    state.pending_tool_calls.insert(id.clone(), resolver);
    let _ = event_tx.send(RuntimeEvent::ToolCall {
        id,
        name: tool_name,
        input,
    });
    Ok(promise.into())
}

pub(super) fn text_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, text_callback_inner(scope, args));
}

fn text_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s> {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let text = serialize_output_text(scope, value)?;
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText { text },
        ));
    }
    Ok(())
}

pub(super) fn image_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, image_callback_inner(scope, args));
}

fn image_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s> {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let image_item = normalize_output_image(scope, value)?;
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::ContentItem(image_item));
    }
    Ok(())
}

pub(super) fn store_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, store_callback_inner(scope, args));
}

fn store_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s> {
    let key = args
        .get(0)
        .to_string(scope)
        .map(|value| value.to_rust_string_lossy(scope))
        .ok_or_else(|| "store key must be a string".to_string())?;
    let serialized = match v8_value_to_json(scope, args.get(1))? {
        Some(value) => value,
        None => {
            return Err(format!(
                "Unable to store {key:?}. Only plain serializable objects can be stored."
            ));
        }
    };
    if let Some(state) = scope.get_slot_mut::<RuntimeState>() {
        state.stored_values.insert(key, serialized);
    }
    Ok(())
}

pub(super) fn load_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, load_callback_inner(scope, args));
}

fn load_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s, Option<v8::Local<'s, v8::Value>>> {
    let key = args
        .get(0)
        .to_string(scope)
        .map(|value| value.to_rust_string_lossy(scope))
        .ok_or_else(|| "load key must be a string".to_string())?;
    let value = scope
        .get_slot::<RuntimeState>()
        .and_then(|state| state.stored_values.get(&key))
        .cloned();
    let Some(value) = value else {
        return Ok(None);
    };
    let value =
        json_to_v8(scope, &value).ok_or_else(|| "failed to load stored value".to_string())?;
    Ok(Some(value))
}

pub(super) fn notify_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, notify_callback_inner(scope, args));
}

fn notify_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s> {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let text = serialize_output_text(scope, value)?;
    if text.trim().is_empty() {
        return Err("notify expects non-empty text".to_string());
    }
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::Notify {
            call_id: state.tool_call_id.clone(),
            text,
        });
    }
    Ok(())
}

pub(super) fn yield_control_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, yield_control_callback_inner(scope, args));
}

fn yield_control_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    _args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s> {
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::YieldRequested);
    }
    Ok(())
}

pub(super) fn exit_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    run_callback!(scope, retval, exit_callback_inner(scope, args));
}

fn exit_callback_inner<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    _args: v8::FunctionCallbackArguments,
) -> CallbackResult<'s, CallbackThrow<'s>> {
    if let Some(state) = scope.get_slot_mut::<RuntimeState>() {
        state.exit_requested = true;
    }
    let error = v8::String::new(scope, EXIT_SENTINEL)
        .ok_or_else(|| "failed to allocate exit sentinel".to_string())?;
    Ok(CallbackThrow(error.into()))
}
