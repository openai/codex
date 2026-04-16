use crate::response::FunctionCallOutputContentItem;

use super::EXIT_SENTINEL;
use super::RuntimeEvent;
use super::RuntimeState;
use super::timers;
use super::value::json_to_v8;
use super::value::normalize_output_image;
use super::value::serialize_output_text;
use super::value::throw_type_error;
use super::value::v8_value_to_json;

pub(super) fn tool_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let tool_index = match args.data().to_rust_string_lossy(scope).parse::<usize>() {
        Ok(tool_index) => tool_index,
        Err(_) => {
            throw_type_error(scope, "invalid tool callback data");
            return;
        }
    };
    let input = if args.length() == 0 {
        Ok(None)
    } else {
        v8_value_to_json(scope, args.get(0))
    };
    let input = match input {
        Ok(input) => input,
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            return;
        }
    };

    let Some(resolver) = v8::PromiseResolver::new(scope) else {
        throw_type_error(scope, "failed to create tool promise");
        return;
    };
    let promise = resolver.get_promise(scope);

    let resolver = v8::Global::new(scope, resolver);
    let tool_name = {
        let Some(state) = scope.get_slot::<RuntimeState>() else {
            throw_type_error(scope, "runtime state unavailable");
            return;
        };
        let Some(tool_name) = state
            .enabled_tools
            .get(tool_index)
            .map(|tool| tool.tool_name.clone())
        else {
            throw_type_error(scope, "tool callback data is out of range");
            return;
        };
        tool_name
    };

    let Some(state) = scope.get_slot_mut::<RuntimeState>() else {
        throw_type_error(scope, "runtime state unavailable");
        return;
    };
    let id = format!("tool-{}", state.next_tool_call_id);
    state.next_tool_call_id = state.next_tool_call_id.saturating_add(1);
    let event_tx = state.event_tx.clone();
    state.pending_tool_calls.insert(id.clone(), resolver);
    let _ = event_tx.send(RuntimeEvent::ToolCall {
        id,
        name: tool_name,
        input,
    });
    retval.set(promise.into());
}

pub(super) fn text_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let text = match serialize_output_text(scope, value) {
        Ok(text) => text,
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            return;
        }
    };
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText { text },
        ));
    }
    retval.set(v8::undefined(scope).into());
}

pub(super) fn image_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let detail_override = if args.length() < 2 {
        None
    } else {
        let detail = args.get(1);
        if detail.is_string() {
            Some(detail.to_rust_string_lossy(scope))
        } else if detail.is_null() || detail.is_undefined() {
            None
        } else {
            throw_type_error(scope, "image detail must be a string when provided");
            return;
        }
    };
    let image_item = match normalize_output_image(scope, value, detail_override) {
        Ok(image_item) => image_item,
        Err(()) => return,
    };
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::ContentItem(image_item));
    }
    retval.set(v8::undefined(scope).into());
}

pub(super) fn store_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    _retval: v8::ReturnValue<v8::Value>,
) {
    let key = match args.get(0).to_string(scope) {
        Some(key) => key.to_rust_string_lossy(scope),
        None => {
            throw_type_error(scope, "store key must be a string");
            return;
        }
    };
    let value = args.get(1);
    let serialized = match v8_value_to_json(scope, value) {
        Ok(Some(value)) => value,
        Ok(None) => {
            throw_type_error(
                scope,
                &format!("Unable to store {key:?}. Only plain serializable objects can be stored."),
            );
            return;
        }
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            return;
        }
    };
    if let Some(state) = scope.get_slot_mut::<RuntimeState>() {
        state.stored_values.insert(key, serialized);
    }
}

pub(super) fn load_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let key = match args.get(0).to_string(scope) {
        Some(key) => key.to_rust_string_lossy(scope),
        None => {
            throw_type_error(scope, "load key must be a string");
            return;
        }
    };
    let value = scope
        .get_slot::<RuntimeState>()
        .and_then(|state| state.stored_values.get(&key))
        .cloned();
    let Some(value) = value else {
        retval.set(v8::undefined(scope).into());
        return;
    };
    let Some(value) = json_to_v8(scope, &value) else {
        throw_type_error(scope, "failed to load stored value");
        return;
    };
    retval.set(value);
}

pub(super) fn notify_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let value = if args.length() == 0 {
        v8::undefined(scope).into()
    } else {
        args.get(0)
    };
    let text = match serialize_output_text(scope, value) {
        Ok(text) => text,
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            return;
        }
    };
    if text.trim().is_empty() {
        throw_type_error(scope, "notify expects non-empty text");
        return;
    }
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::Notify {
            call_id: state.tool_call_id.clone(),
            text,
        });
    }
    retval.set(v8::undefined(scope).into());
}

pub(super) fn set_timeout_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let timeout_id = match timers::schedule_timeout(scope, args) {
        Ok(timeout_id) => timeout_id,
        Err(error_text) => {
            throw_type_error(scope, &error_text);
            return;
        }
    };

    retval.set(v8::Number::new(scope, timeout_id as f64).into());
}

pub(super) fn clear_timeout_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    if let Err(error_text) = timers::clear_timeout(scope, args) {
        throw_type_error(scope, &error_text);
        return;
    }

    retval.set(v8::undefined(scope).into());
}

pub(super) fn yield_control_callback(
    scope: &mut v8::PinScope<'_, '_>,
    _args: v8::FunctionCallbackArguments,
    _retval: v8::ReturnValue<v8::Value>,
) {
    if let Some(state) = scope.get_slot::<RuntimeState>() {
        let _ = state.event_tx.send(RuntimeEvent::YieldRequested);
    }
}

pub(super) fn exit_callback(
    scope: &mut v8::PinScope<'_, '_>,
    _args: v8::FunctionCallbackArguments,
    _retval: v8::ReturnValue<v8::Value>,
) {
    if let Some(state) = scope.get_slot_mut::<RuntimeState>() {
        state.exit_requested = true;
    }
    if let Some(error) = v8::String::new(scope, EXIT_SENTINEL) {
        scope.throw_exception(error.into());
    }
}
