use std::sync::Arc;

use super::callbacks::audio_callback;
use super::callbacks::clear_timeout_callback;
use super::callbacks::content_block_callback;
use super::callbacks::exit_callback;
use super::callbacks::generated_image_callback;
use super::callbacks::image_callback;
use super::callbacks::load_callback;
use super::callbacks::notify_callback;
use super::callbacks::set_timeout_callback;
use super::callbacks::store_callback;
use super::callbacks::text_callback;
use super::callbacks::tool_callback;
use super::callbacks::yield_control_callback;
use codex_code_mode_protocol::EnabledToolMetadata;

pub(super) fn install_globals(scope: &mut v8::PinScope<'_, '_>) -> Result<(), String> {
    let global = scope.get_current_context().global(scope);
    delete_global(scope, global, "console")?;
    delete_global(scope, global, "Atomics")?;
    delete_global(scope, global, "SharedArrayBuffer")?;
    delete_global(scope, global, "WebAssembly")?;

    let clear_timeout = helper_function(scope, "clearTimeout", clear_timeout_callback)?;
    let set_timeout = helper_function(scope, "setTimeout", set_timeout_callback)?;
    let text = helper_function(scope, "text", text_callback)?;
    let content_block = helper_function(scope, "content", content_block_callback)?;
    let image = helper_function(scope, "image", image_callback)?;
    let audio = helper_function(scope, "audio", audio_callback)?;
    let generated_image = helper_function(scope, "generatedImage", generated_image_callback)?;
    let store = helper_function(scope, "store", store_callback)?;
    let load = helper_function(scope, "load", load_callback)?;
    let notify = helper_function(scope, "notify", notify_callback)?;
    let yield_control = helper_function(scope, "yield_control", yield_control_callback)?;
    let exit = helper_function(scope, "exit", exit_callback)?;

    for name in ["tools", "ALL_TOOLS"] {
        let key =
            v8::String::new(scope, name).ok_or_else(|| format!("failed to allocate {name}"))?;
        if global.set_lazy_data_property(scope, key.into(), catalog_global) != Some(true) {
            return Err(format!("failed to install {name}"));
        }
    }
    set_global(scope, global, "clearTimeout", clear_timeout.into())?;
    set_global(scope, global, "setTimeout", set_timeout.into())?;
    set_global(scope, global, "text", text.into())?;
    set_global(scope, global, "content", content_block.into())?;
    set_global(scope, global, "image", image.into())?;
    set_global(scope, global, "audio", audio.into())?;
    set_global(scope, global, "generatedImage", generated_image.into())?;
    set_global(scope, global, "store", store.into())?;
    set_global(scope, global, "load", load.into())?;
    set_global(scope, global, "notify", notify.into())?;
    set_global(scope, global, "yield_control", yield_control.into())?;
    set_global(scope, global, "exit", exit.into())?;

    let helpers = concat!(include_str!("settled.js"), "\n", include_str!("content.js"));
    let source = v8::String::new(scope, helpers)
        .ok_or_else(|| "failed to allocate JavaScript helpers".to_string())?;
    v8::Script::compile(scope, source, /*origin*/ None)
        .and_then(|script| script.run(scope))
        .ok_or_else(|| "failed to initialize JavaScript helpers".to_string())?;
    Ok(())
}

/// V8 replaces the lazy property with its writable value on the first read.
fn catalog_global(
    scope: &mut v8::PinScope<'_, '_>,
    key: v8::Local<v8::Name>,
    _args: v8::PropertyCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    let Some(state) = scope.get_slot::<super::RuntimeState>() else {
        super::value::throw_type_error(scope, "code-mode catalog is unavailable");
        return;
    };
    let tools = Arc::clone(&state.enabled_tools);
    let value = if key.to_rust_string_lossy(scope) == "tools" {
        build_tools_object(scope, &tools).map(Into::into)
    } else {
        build_all_tools_value(scope, &tools)
    };
    match value {
        Ok(value) => retval.set(value),
        Err(message) => super::value::throw_type_error(scope, &message),
    }
}

fn build_tools_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    enabled_tools: &[EnabledToolMetadata],
) -> Result<v8::Local<'s, v8::Object>, String> {
    let prototype = v8::Object::new(scope)
        .get_prototype(scope)
        .ok_or_else(|| "failed to get tool object prototype".to_string())?;
    let mut names = Vec::with_capacity(enabled_tools.len());
    let mut functions = Vec::with_capacity(enabled_tools.len());

    for (tool_index, tool) in enabled_tools.iter().enumerate() {
        let name = v8::String::new(scope, &tool.global_name)
            .ok_or_else(|| "failed to allocate tool name".to_string())?;
        let function = tool_function(scope, tool_index)?;
        names.push(name.into());
        functions.push(function.into());
    }
    Ok(v8::Object::with_prototype_and_properties(
        scope, prototype, &names, &functions,
    ))
}

fn build_all_tools_value<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    enabled_tools: &[EnabledToolMetadata],
) -> Result<v8::Local<'s, v8::Value>, String> {
    let mut items = Vec::with_capacity(enabled_tools.len());
    let name_key = v8::String::new(scope, "name")
        .ok_or_else(|| "failed to allocate ALL_TOOLS name key".to_string())?;
    let description_key = v8::String::new(scope, "description")
        .ok_or_else(|| "failed to allocate ALL_TOOLS description key".to_string())?;

    for tool in enabled_tools {
        let item = v8::Object::new(scope);
        let name = v8::String::new(scope, &tool.global_name)
            .ok_or_else(|| "failed to allocate ALL_TOOLS name".to_string())?;
        let description = v8::String::new(scope, &tool.description)
            .ok_or_else(|| "failed to allocate ALL_TOOLS description".to_string())?;

        if item.create_data_property(scope, name_key.into(), name.into()) != Some(true) {
            return Err("failed to set ALL_TOOLS name".to_string());
        }
        if item.create_data_property(scope, description_key.into(), description.into())
            != Some(true)
        {
            return Err("failed to set ALL_TOOLS description".to_string());
        }
        items.push(item.into());
    }

    Ok(v8::Array::new_with_elements(scope, &items).into())
}

fn helper_function<'s, F>(
    scope: &mut v8::PinScope<'s, '_>,
    name: &str,
    callback: F,
) -> Result<v8::Local<'s, v8::Function>, String>
where
    F: v8::MapFnTo<v8::FunctionCallback>,
{
    let name =
        v8::String::new(scope, name).ok_or_else(|| "failed to allocate helper name".to_string())?;
    let template = v8::FunctionTemplate::builder(callback)
        .data(name.into())
        .build(scope);
    template
        .get_function(scope)
        .ok_or_else(|| "failed to create helper function".to_string())
}

fn tool_function<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    tool_index: usize,
) -> Result<v8::Local<'s, v8::Function>, String> {
    let data = v8::String::new(scope, &tool_index.to_string())
        .ok_or_else(|| "failed to allocate tool callback data".to_string())?;
    let template = v8::FunctionTemplate::builder(tool_callback)
        .data(data.into())
        .build(scope);
    template
        .get_function(scope)
        .ok_or_else(|| "failed to create tool function".to_string())
}

fn set_global<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    global: v8::Local<'s, v8::Object>,
    name: &str,
    value: v8::Local<'s, v8::Value>,
) -> Result<(), String> {
    let key = v8::String::new(scope, name)
        .ok_or_else(|| format!("failed to allocate global `{name}`"))?;
    if global.set(scope, key.into(), value) == Some(true) {
        Ok(())
    } else {
        Err(format!("failed to set global `{name}`"))
    }
}

fn delete_global<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    global: v8::Local<'s, v8::Object>,
    name: &str,
) -> Result<(), String> {
    let key = v8::String::new(scope, name)
        .ok_or_else(|| format!("failed to allocate global `{name}`"))?;
    if global.delete(scope, key.into()) == Some(true) {
        Ok(())
    } else {
        Err(format!("failed to remove global `{name}`"))
    }
}
