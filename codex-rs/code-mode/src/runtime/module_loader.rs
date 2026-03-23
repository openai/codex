use serde_json::Value as JsonValue;

use super::CompletionState;
use super::EXIT_SENTINEL;
use super::RuntimeState;
use super::value::json_to_v8;
use super::value::value_to_error_text;

pub(super) fn evaluate_main_module(
    scope: &mut v8::PinScope<'_, '_>,
    source_text: &str,
) -> Result<Option<v8::Global<v8::Promise>>, String> {
    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    let mut tc = tc.init();
    let source = v8::String::new(&tc, source_text)
        .ok_or_else(|| "failed to allocate exec source".to_string())?;
    let origin = script_origin(&mut tc, "exec_main.mjs")?;
    let mut source = v8::script_compiler::Source::new(source, Some(&origin));
    let module = v8::script_compiler::compile_module(&tc, &mut source).ok_or_else(|| {
        tc.exception()
            .map(|exception| value_to_error_text(&mut tc, exception))
            .unwrap_or_else(|| "unknown code mode exception".to_string())
    })?;
    module
        .instantiate_module(&tc, resolve_module_callback)
        .ok_or_else(|| {
            tc.exception()
                .map(|exception| value_to_error_text(&mut tc, exception))
                .unwrap_or_else(|| "unknown code mode exception".to_string())
        })?;
    let result = match module.evaluate(&tc) {
        Some(result) => result,
        None => {
            if let Some(exception) = tc.exception() {
                if is_exit_exception(&mut tc, exception) {
                    return Ok(None);
                }
                return Err(value_to_error_text(&mut tc, exception));
            }
            return Err("unknown code mode exception".to_string());
        }
    };
    tc.perform_microtask_checkpoint();

    if result.is_promise() {
        let promise = v8::Local::<v8::Promise>::try_from(result)
            .map_err(|_| "failed to read exec promise".to_string())?;
        return Ok(Some(v8::Global::new(&tc, promise)));
    }

    Ok(None)
}

fn is_exit_exception(
    scope: &mut v8::PinScope<'_, '_>,
    exception: v8::Local<'_, v8::Value>,
) -> bool {
    scope
        .get_slot::<RuntimeState>()
        .map(|state| state.exit_requested)
        .unwrap_or(false)
        && exception.is_string()
        && exception.to_rust_string_lossy(scope) == EXIT_SENTINEL
}

pub(super) fn resolve_tool_response(
    scope: &mut v8::PinScope<'_, '_>,
    id: &str,
    response: Result<JsonValue, String>,
) -> Result<(), String> {
    let resolver = {
        let state = scope
            .get_slot_mut::<RuntimeState>()
            .ok_or_else(|| "runtime state unavailable".to_string())?;
        state.pending_tool_calls.remove(id)
    }
    .ok_or_else(|| format!("unknown tool call `{id}`"))?;

    let tc = std::pin::pin!(v8::TryCatch::new(scope));
    let mut tc = tc.init();
    let resolver = v8::Local::new(&tc, &resolver);
    match response {
        Ok(result) => {
            let value = json_to_v8(&mut tc, &result)
                .ok_or_else(|| "failed to serialize tool response".to_string())?;
            resolver.resolve(&tc, value);
        }
        Err(error_text) => {
            let value = v8::String::new(&tc, &error_text)
                .ok_or_else(|| "failed to allocate tool error".to_string())?;
            resolver.reject(&tc, value.into());
        }
    }
    if tc.has_caught() {
        return Err(tc
            .exception()
            .map(|exception| value_to_error_text(&mut tc, exception))
            .unwrap_or_else(|| "unknown code mode exception".to_string()));
    }
    Ok(())
}

pub(super) fn completion_state(
    scope: &mut v8::PinScope<'_, '_>,
    pending_promise: Option<&v8::Global<v8::Promise>>,
) -> CompletionState {
    let stored_values = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.stored_values.clone())
        .unwrap_or_default();

    let Some(pending_promise) = pending_promise else {
        return CompletionState::Completed {
            stored_values,
            error_text: None,
        };
    };

    let promise = v8::Local::new(scope, pending_promise);
    match promise.state() {
        v8::PromiseState::Pending => CompletionState::Pending,
        v8::PromiseState::Fulfilled => CompletionState::Completed {
            stored_values,
            error_text: None,
        },
        v8::PromiseState::Rejected => {
            let result = promise.result(scope);
            let error_text = if is_exit_exception(scope, result) {
                None
            } else {
                Some(value_to_error_text(scope, result))
            };
            CompletionState::Completed {
                stored_values,
                error_text,
            }
        }
    }
}

fn script_origin<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    resource_name_: &str,
) -> Result<v8::ScriptOrigin<'s>, String> {
    let resource_name = v8::String::new(scope, resource_name_)
        .ok_or_else(|| "failed to allocate script origin".to_string())?;
    let source_map_url = v8::String::new(scope, resource_name_)
        .ok_or_else(|| "failed to allocate source map url".to_string())?;
    Ok(v8::ScriptOrigin::new(
        scope,
        resource_name.into(),
        0,
        0,
        true,
        0,
        Some(source_map_url.into()),
        true,
        false,
        true,
        None,
    ))
}

fn resolve_module_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    _import_attributes: v8::Local<'s, v8::FixedArray>,
    _referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
    v8::callback_scope!(unsafe scope, context);
    let specifier = specifier.to_rust_string_lossy(scope);
    resolve_module(scope, &specifier)
}

pub(super) fn dynamic_import_callback<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    _host_defined_options: v8::Local<'s, v8::Data>,
    _resource_name: v8::Local<'s, v8::Value>,
    specifier: v8::Local<'s, v8::String>,
    _import_attributes: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
    let specifier = specifier.to_rust_string_lossy(scope);
    let resolver = v8::PromiseResolver::new(scope)?;

    match resolve_module(scope, &specifier) {
        Some(module) => {
            if module.get_status() == v8::ModuleStatus::Uninstantiated
                && module
                    .instantiate_module(scope, resolve_module_callback)
                    .is_none()
            {
                let error = v8::String::new(scope, "failed to instantiate module")
                    .map(Into::into)
                    .unwrap_or_else(|| v8::undefined(scope).into());
                resolver.reject(scope, error);
                return Some(resolver.get_promise(scope));
            }
            if matches!(
                module.get_status(),
                v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated
            ) && module.evaluate(scope).is_none()
            {
                let error = v8::String::new(scope, "failed to evaluate module")
                    .map(Into::into)
                    .unwrap_or_else(|| v8::undefined(scope).into());
                resolver.reject(scope, error);
                return Some(resolver.get_promise(scope));
            }
            let namespace = module.get_module_namespace();
            resolver.resolve(scope, namespace);
            Some(resolver.get_promise(scope))
        }
        None => {
            let error = v8::String::new(scope, "unsupported import in exec")
                .map(Into::into)
                .unwrap_or_else(|| v8::undefined(scope).into());
            resolver.reject(scope, error);
            Some(resolver.get_promise(scope))
        }
    }
}

fn resolve_module<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    specifier: &str,
) -> Option<v8::Local<'s, v8::Module>> {
    if let Some(message) =
        v8::String::new(scope, &format!("Unsupported import in exec: {specifier}"))
    {
        scope.throw_exception(message.into());
    } else {
        scope.throw_exception(v8::undefined(scope).into());
    }
    None
}
