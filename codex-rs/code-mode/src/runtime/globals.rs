use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::description::EnabledToolMetadata;

use super::MODULE_TOOLS_SYMBOL_KEY;
use super::RuntimeState;
use super::callbacks::exit_callback;
use super::callbacks::image_callback;
use super::callbacks::load_callback;
use super::callbacks::noop_callback;
use super::callbacks::notify_callback;
use super::callbacks::store_callback;
use super::callbacks::text_callback;
use super::callbacks::tool_callback;
use super::callbacks::yield_control_callback;
use super::value::json_to_v8;

pub(super) fn install_globals(scope: &mut v8::PinScope<'_, '_>) -> Result<(), String> {
    let global = scope.get_current_context().global(scope);
    let tools = build_tools_object(scope)?;
    let module_tools = build_module_tools_object(scope)?;
    let all_tools = build_all_tools_value(scope)?;
    let console = build_console_object(scope)?;
    let text = helper_function(scope, "text", text_callback)?;
    let image = helper_function(scope, "image", image_callback)?;
    let store = helper_function(scope, "store", store_callback)?;
    let load = helper_function(scope, "load", load_callback)?;
    let notify = helper_function(scope, "notify", notify_callback)?;
    let yield_control = helper_function(scope, "yield_control", yield_control_callback)?;
    let exit = helper_function(scope, "exit", exit_callback)?;

    set_global(scope, global, "tools", tools.into())?;
    set_global(scope, global, "ALL_TOOLS", all_tools)?;
    let module_tools_symbol_description = v8::String::new(scope, MODULE_TOOLS_SYMBOL_KEY)
        .ok_or_else(|| "failed to allocate module tools symbol".to_string())?;
    let module_tools_symbol = v8::Symbol::for_key(scope, module_tools_symbol_description);
    if global.set(scope, module_tools_symbol.into(), module_tools.into()) != Some(true) {
        return Err("failed to set module tools symbol".to_string());
    }
    set_global(scope, global, "console", console.into())?;
    set_global(scope, global, "text", text.into())?;
    set_global(scope, global, "image", image.into())?;
    set_global(scope, global, "store", store.into())?;
    set_global(scope, global, "load", load.into())?;
    set_global(scope, global, "notify", notify.into())?;
    set_global(scope, global, "yield_control", yield_control.into())?;
    set_global(scope, global, "exit", exit.into())?;
    Ok(())
}

fn build_tools_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let tools = v8::Object::new(scope);
    let enabled_tools = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.enabled_tools.clone())
        .unwrap_or_default();

    for tool in enabled_tools {
        let name = v8::String::new(scope, &tool.global_name)
            .ok_or_else(|| "failed to allocate tool name".to_string())?;
        let function = tool_function(scope, &tool.tool_name)?;
        tools.set(scope, name.into(), function.into());
    }
    Ok(tools)
}

fn build_module_tools_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let module_tools = v8::Object::new(scope);
    let enabled_tools = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.enabled_tools.clone())
        .unwrap_or_default();
    let mut buckets = HashMap::<String, Vec<EnabledToolMetadata>>::new();

    for tool in enabled_tools {
        if tool.namespace.is_empty() {
            continue;
        }
        buckets
            .entry(tool.namespace.join("/"))
            .or_default()
            .push(tool);
    }

    for (key, tools) in buckets {
        let tool_object = v8::Object::new(scope);
        for tool in tools {
            let name = v8::String::new(scope, &tool.name)
                .ok_or_else(|| "failed to allocate module export name".to_string())?;
            let function = tool_function(scope, &tool.tool_name)?;
            tool_object.set(scope, name.into(), function.into());
        }
        let key = v8::String::new(scope, &key)
            .ok_or_else(|| "failed to allocate module namespace".to_string())?;
        module_tools.set(scope, key.into(), tool_object.into());
    }

    Ok(module_tools)
}

fn build_all_tools_value<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Value>, String> {
    let all_tools = scope
        .get_slot::<RuntimeState>()
        .map(|state| {
            state
                .enabled_tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.global_name,
                        "description": tool.description,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json_to_v8(scope, &JsonValue::Array(all_tools))
        .ok_or_else(|| "failed to build ALL_TOOLS metadata".to_string())
}

fn build_console_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let console = v8::Object::new(scope);
    for name in ["log", "info", "warn", "error", "debug"] {
        let key = v8::String::new(scope, name)
            .ok_or_else(|| "failed to allocate console key".to_string())?;
        let value = helper_function(scope, name, noop_callback)?;
        console.set(scope, key.into(), value.into());
    }
    Ok(console)
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
    tool_name: &str,
) -> Result<v8::Local<'s, v8::Function>, String> {
    let data = v8::String::new(scope, tool_name)
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
