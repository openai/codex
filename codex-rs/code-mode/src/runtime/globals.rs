use super::RuntimeState;
use super::callbacks::exit_callback;
use super::callbacks::image_callback;
use super::callbacks::load_callback;
use super::callbacks::notify_callback;
use super::callbacks::store_callback;
use super::callbacks::text_callback;
use super::callbacks::tool_callback;
use super::callbacks::yield_control_callback;

pub(super) fn install_globals(scope: &mut v8::PinScope<'_, '_>) -> Result<(), String> {
    let global = scope.get_current_context().global(scope);
    let console = v8::String::new(scope, "console")
        .ok_or_else(|| "failed to allocate global `console`".to_string())?;
    if global.delete(scope, console.into()) != Some(true) {
        return Err("failed to remove global `console`".to_string());
    }

    let functions = build_functions_object(scope)?;
    let all_functions = build_all_functions_object(scope)?;
    let text = helper_function(scope, "text", text_callback)?;
    let image = helper_function(scope, "image", image_callback)?;
    let store = helper_function(scope, "store", store_callback)?;
    let load = helper_function(scope, "load", load_callback)?;
    let notify = helper_function(scope, "notify", notify_callback)?;
    let yield_control = helper_function(scope, "yield_control", yield_control_callback)?;
    let exit = helper_function(scope, "exit", exit_callback)?;

    // We use the word "tools" everywhere in the codebase to refer to functions the model can call.
    // The model sees these things as callables in the "functions" namespace. So we use "functions"
    // as the name of the JS global that holds its callable functions.
    set_global(scope, global, "functions", functions.into())?;
    set_global(scope, global, "ALL_FUNCTIONS", all_functions)?;
    set_global(scope, global, "text", text.into())?;
    set_global(scope, global, "image", image.into())?;
    set_global(scope, global, "store", store.into())?;
    set_global(scope, global, "load", load.into())?;
    set_global(scope, global, "notify", notify.into())?;
    set_global(scope, global, "yield_control", yield_control.into())?;
    set_global(scope, global, "exit", exit.into())?;
    Ok(())
}

fn build_functions_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let functions = v8::Object::new(scope);
    let enabled_tools = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.enabled_tools.clone())
        .unwrap_or_default();

    for tool in enabled_tools {
        let name = v8::String::new(scope, &tool.global_name)
            .ok_or_else(|| "failed to allocate tool name in code mode functions object".to_string())?;
        let function = tool_function(scope, &tool.tool_name)?;
        functions.set(scope, name.into(), function.into());
    }
    Ok(functions)
}

fn build_all_functions_object<'s>(
    scope: &mut v8::PinScope<'s, '_>,
) -> Result<v8::Local<'s, v8::Value>, String> {
    let enabled_tools = scope
        .get_slot::<RuntimeState>()
        .map(|state| state.enabled_tools.clone())
        .unwrap_or_default();
    let array = v8::Array::new(scope, enabled_tools.len() as i32);
    let name_key = v8::String::new(scope, "name")
        .ok_or_else(|| "failed to allocate ALL_FUNCTIONS name key".to_string())?;
    let description_key = v8::String::new(scope, "description")
        .ok_or_else(|| "failed to allocate ALL_FUNCTIONS description key".to_string())?;

    for (index, tool) in enabled_tools.iter().enumerate() {
        let item = v8::Object::new(scope);
        let name = v8::String::new(scope, &tool.global_name)
            .ok_or_else(|| "failed to allocate ALL_FUNCTIONS name".to_string())?;
        let description = v8::String::new(scope, &tool.description)
            .ok_or_else(|| "failed to allocate ALL_FUNCTIONS description".to_string())?;

        if item.set(scope, name_key.into(), name.into()) != Some(true) {
            return Err("failed to set ALL_FUNCTIONS name".to_string());
        }
        if item.set(scope, description_key.into(), description.into()) != Some(true) {
            return Err("failed to set ALL_FUNCTIONS description".to_string());
        }
        if array.set_index(scope, index as u32, item.into()) != Some(true) {
            return Err("failed to append ALL_FUNCTIONS metadata".to_string());
        }
    }

    Ok(array.into())
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
