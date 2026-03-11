use std::collections::HashMap;
use std::pin::pin;
use std::sync::Once;

use crate::EnabledTool;
use crate::ExecutionResult;
use crate::ToolCallHandler;
use serde_json::Value as JsonValue;

const CODE_MODE_BOOTSTRAP_SOURCE: &str = include_str!("code_mode_bridge.js");
const CODE_MODE_BOOTSTRAP_FILENAME: &str = "code_mode_bootstrap.js";
const CODE_MODE_MAIN_FILENAME: &str = "code_mode_main.mjs";
const CODE_MODE_TOOLS_MODULE_NAME: &str = "tools.js";
const OPENAI_CODE_MODE_MODULE_NAME: &str = "@openai/code_mode";
const OPENAI_CODE_MODE_LEGACY_MODULE_NAME: &str = "openai/code_mode";
const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL: usize = 10_000;

static CODE_MODE_V8_INIT: Once = Once::new();

struct RuntimeState {
    enabled_tools: Vec<EnabledTool>,
    tools_module: Option<v8::Global<v8::Module>>,
    module_cache: HashMap<String, v8::Global<v8::Module>>,
    on_tool_call: Box<ToolCallHandler>,
    max_output_tokens_per_exec_call: usize,
}

pub fn execute(
    code: String,
    enabled_tools: Vec<EnabledTool>,
    stored_values: HashMap<String, JsonValue>,
    on_tool_call: Box<ToolCallHandler>,
) -> Result<ExecutionResult, String> {
    init_v8();

    let bootstrap_source = build_bootstrap_source(&enabled_tools, &stored_values)?;
    let mut isolate = v8::Isolate::new(v8::CreateParams::default());
    isolate.set_capture_stack_trace_for_uncaught_exceptions(true, 32);
    isolate.set_host_import_module_dynamically_callback(code_mode_dynamic_import_callback);
    isolate.set_slot(RuntimeState {
        enabled_tools: enabled_tools.clone(),
        tools_module: None,
        module_cache: HashMap::new(),
        on_tool_call,
        max_output_tokens_per_exec_call: DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL,
    });

    let scope = pin!(v8::HandleScope::new(&mut isolate));
    let scope = &mut scope.init();
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    install_binding(scope, "__codex_tool_call", code_mode_tool_call_callback)?;
    install_binding(
        scope,
        "__codex_set_max_output_tokens_per_exec_call_native",
        set_max_output_tokens_per_exec_call_callback,
    )?;
    run_script(scope, CODE_MODE_BOOTSTRAP_FILENAME, &bootstrap_source)?;

    let tools_module = create_tools_module(scope, &enabled_tools)?;
    let tools_module = v8::Global::new(scope, tools_module);
    let Some(runtime_state) = scope.get_slot_mut::<RuntimeState>() else {
        return Err("code_mode runtime state missing".to_string());
    };
    runtime_state.tools_module = Some(tools_module);

    let scope = pin!(v8::TryCatch::new(scope));
    let scope = &mut scope.init();
    let execution_outcome = execute_main_module(scope, &code);
    let content_items = read_content_items(scope)?;
    let stored_values = read_stored_values(scope)?;
    let Some(runtime_state) = scope.get_slot::<RuntimeState>() else {
        return Err("code_mode runtime state missing".to_string());
    };
    let (success, error_text) = match execution_outcome {
        Ok(()) => (true, None),
        Err(error_text) => (false, Some(error_text)),
    };
    Ok(ExecutionResult {
        content_items,
        stored_values,
        max_output_tokens_per_exec_call: runtime_state.max_output_tokens_per_exec_call,
        success,
        error_text,
    })
}

fn init_v8() {
    CODE_MODE_V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

fn install_binding(
    scope: &mut v8::PinScope<'_, '_>,
    name: &str,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
) -> Result<(), String> {
    let function = v8::Function::new(scope, callback)
        .ok_or_else(|| format!("failed to install code_mode binding `{name}`"))?;
    let key = v8_string(scope, name)?;
    let global = scope.get_current_context().global(scope);
    if global.set(scope, key.into(), function.into()).is_some() {
        Ok(())
    } else {
        Err(format!("failed to bind `{name}`"))
    }
}

fn run_script(
    scope: &mut v8::PinScope<'_, '_>,
    filename: &str,
    source: &str,
) -> Result<(), String> {
    let scope = pin!(v8::TryCatch::new(scope));
    let scope = &mut scope.init();
    let source = v8_string(scope, source)?;
    let filename = v8_string(scope, filename)?;
    let origin = script_origin(scope, filename, false);
    let Some(script) = v8::Script::compile(scope, source, Some(&origin)) else {
        return Err(format_v8_exception(scope));
    };
    if script.run(scope).is_none() {
        return Err(format_v8_exception(scope));
    }
    Ok(())
}

fn script_origin<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    filename: v8::Local<'s, v8::String>,
    is_module: bool,
) -> v8::ScriptOrigin<'s> {
    v8::ScriptOrigin::new(
        scope,
        filename.into(),
        0,
        0,
        false,
        0,
        None,
        false,
        false,
        is_module,
        None,
    )
}

fn compile_module<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    identifier: &str,
    source_text: &str,
) -> Result<v8::Local<'s, v8::Module>, String> {
    let source = v8_string(scope, source_text)?;
    let identifier = v8_string(scope, identifier)?;
    let origin = script_origin(scope, identifier, true);
    let mut source = v8::script_compiler::Source::new(source, Some(&origin));
    v8::script_compiler::compile_module(scope, &mut source)
        .ok_or_else(|| "failed to compile code_mode module".to_string())
}

fn execute_main_module(
    try_catch: &mut v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>,
    code: &str,
) -> Result<(), String> {
    let source = v8_string(try_catch, code)?;
    let identifier = v8_string(try_catch, CODE_MODE_MAIN_FILENAME)?;
    let origin = script_origin(try_catch, identifier, true);
    let mut source = v8::script_compiler::Source::new(source, Some(&origin));
    let Some(module) = v8::script_compiler::compile_module(try_catch, &mut source) else {
        return Err(format_v8_exception(try_catch));
    };
    let Some(instantiated) = module.instantiate_module(try_catch, resolve_code_mode_module) else {
        return Err(format_v8_exception(try_catch));
    };
    if !instantiated {
        return Err("failed to instantiate code_mode module".to_string());
    }

    let Some(result) = module.evaluate(try_catch) else {
        return Err(format_v8_exception(try_catch));
    };
    if result.is_promise() {
        let promise = v8::Local::<v8::Promise>::try_from(result)
            .map_err(|_| "code_mode module evaluation did not return a promise".to_string())?;
        wait_for_module_promise(try_catch, module, promise)?;
    } else {
        try_catch.perform_microtask_checkpoint();
    }

    Ok(())
}

fn create_tools_module<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    enabled_tools: &[EnabledTool],
) -> Result<v8::Local<'s, v8::Module>, String> {
    let mut export_names = vec![v8_string(scope, "tools")?, v8_string(scope, "ALL_TOOLS")?];
    for tool in enabled_tools {
        if tool.tool_name != "tools" && is_valid_identifier(&tool.tool_name) {
            export_names.push(v8_string(scope, &tool.tool_name)?);
        }
    }
    let module_name = v8_string(scope, CODE_MODE_TOOLS_MODULE_NAME)?;
    Ok(v8::Module::create_synthetic_module(
        scope,
        module_name,
        &export_names,
        evaluate_tools_module,
    ))
}

fn evaluate_tools_module<'s>(
    context: v8::Local<'s, v8::Context>,
    module: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Value>> {
    v8::callback_scope!(unsafe scope, context);
    let Some(global_name) = v8::String::new(scope, "tools") else {
        return throw_v8_exception(scope, "failed to allocate tools export name");
    };
    let global = context.global(scope);
    let Some(tools_value) = global.get(scope, global_name.into()) else {
        return throw_v8_exception(scope, "code_mode tools namespace missing");
    };
    let Ok(tools_object) = v8::Local::<v8::Object>::try_from(tools_value) else {
        return throw_v8_exception(scope, "code_mode tools namespace is not an object");
    };
    module.set_synthetic_module_export(scope, global_name, tools_object.into())?;
    let Some(all_tools_name) = v8::String::new(scope, "ALL_TOOLS") else {
        return throw_v8_exception(scope, "failed to allocate ALL_TOOLS export name");
    };
    let Some(all_tools_value) = global.get(scope, all_tools_name.into()) else {
        return throw_v8_exception(scope, "code_mode ALL_TOOLS export is unavailable");
    };
    module.set_synthetic_module_export(scope, all_tools_name, all_tools_value)?;

    let enabled_tools = match scope.get_slot::<RuntimeState>() {
        Some(runtime_state) => runtime_state.enabled_tools.clone(),
        None => return throw_v8_exception(scope, "code_mode runtime state missing"),
    };
    for tool in &enabled_tools {
        if !is_valid_identifier(&tool.tool_name) || tool.tool_name == "tools" {
            continue;
        }
        let Some(export_name) = v8::String::new(scope, &tool.tool_name) else {
            return throw_v8_exception(scope, "failed to allocate tool export name");
        };
        let Some(export_value) = tools_object.get(scope, export_name.into()) else {
            return throw_v8_exception(
                scope,
                &format!("code_mode tool export `{}` is unavailable", tool.tool_name),
            );
        };
        module.set_synthetic_module_export(scope, export_name, export_value)?;
    }

    Some(v8::undefined(scope).into())
}

fn resolve_code_mode_module<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    _import_attributes: v8::Local<'s, v8::FixedArray>,
    _referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
    v8::callback_scope!(unsafe scope, context);
    let specifier = specifier.to_rust_string_lossy(scope);
    match resolve_module(scope, &specifier) {
        Ok(module) => Some(module),
        Err(error) => throw_v8_exception(scope, &error),
    }
}

fn code_mode_dynamic_import_callback<'s, 'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    _host_defined_options: v8::Local<'s, v8::Data>,
    _resource_name: v8::Local<'s, v8::Value>,
    specifier: v8::Local<'s, v8::String>,
    _import_attributes: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
    let resolver = v8::PromiseResolver::new(scope)?;
    let promise = resolver.get_promise(scope);
    let specifier = specifier.to_rust_string_lossy(scope);

    match resolve_module(scope, &specifier).and_then(|module| {
        instantiate_and_evaluate_module(scope, module)?;
        Ok(module.get_module_namespace())
    }) {
        Ok(namespace) => {
            let _ = resolver.resolve(scope, namespace);
        }
        Err(error) => {
            let error = v8_string(scope, &error).ok()?;
            let _ = resolver.reject(scope, error.into());
        }
    }

    Some(promise)
}

fn resolve_module<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    specifier: &str,
) -> Result<v8::Local<'s, v8::Module>, String> {
    if specifier == CODE_MODE_TOOLS_MODULE_NAME {
        let Some(runtime_state) = scope.get_slot::<RuntimeState>() else {
            return Err("code_mode runtime state missing".to_string());
        };
        let Some(tools_module) = runtime_state.tools_module.as_ref() else {
            return Err("code_mode tools module missing".to_string());
        };
        return Ok(v8::Local::new(scope, tools_module));
    }

    if let Some(runtime_state) = scope.get_slot::<RuntimeState>()
        && let Some(module) = runtime_state.module_cache.get(specifier)
    {
        return Ok(v8::Local::new(scope, module));
    }

    let enabled_tools = scope
        .get_slot::<RuntimeState>()
        .ok_or_else(|| "code_mode runtime state missing".to_string())?
        .enabled_tools
        .clone();
    let source = build_module_source(specifier, &enabled_tools)?;
    let module = compile_module(scope, specifier, &source)?;
    let module_handle = v8::Global::new(scope, module);
    let Some(runtime_state) = scope.get_slot_mut::<RuntimeState>() else {
        return Err("code_mode runtime state missing".to_string());
    };
    runtime_state
        .module_cache
        .insert(specifier.to_string(), module_handle);
    Ok(module)
}

fn build_module_source(specifier: &str, enabled_tools: &[EnabledTool]) -> Result<String, String> {
    match specifier {
        OPENAI_CODE_MODE_MODULE_NAME | OPENAI_CODE_MODE_LEGACY_MODULE_NAME => {
            Ok(build_code_mode_module_source())
        }
        _ => {
            let Some(namespace) = parse_namespaced_tools_specifier(specifier) else {
                return Err(format!("Unsupported import in code_mode: {specifier}"));
            };
            build_namespaced_tools_module_source(enabled_tools, &namespace)
        }
    }
}

fn parse_namespaced_tools_specifier(specifier: &str) -> Option<Vec<&str>> {
    let namespace = specifier
        .strip_prefix("tools/")?
        .strip_suffix(".js")?
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    (!namespace.is_empty()).then_some(namespace)
}

fn build_code_mode_module_source() -> String {
    [
        "export const load = globalThis.__codex_load;",
        "export const output_text = globalThis.__codex_output_text;",
        "export const output_image = globalThis.__codex_output_image;",
        "export const set_max_output_tokens_per_exec_call = globalThis.__codex_set_max_output_tokens_per_exec_call;",
        "export const store = globalThis.__codex_store;",
    ]
    .join("\n")
}

fn build_namespaced_tools_module_source(
    enabled_tools: &[EnabledTool],
    namespace: &[&str],
) -> Result<String, String> {
    let mut source = String::from("const tools = Object.create(null);\n");
    for tool in enabled_tools {
        if !namespaces_match(&tool.namespace, namespace) {
            continue;
        }
        let export_name = js_string_literal(&tool.name)?;
        let tool_name = js_string_literal(&tool.tool_name)?;
        source.push_str(&format!(
            "Object.defineProperty(tools, {export_name}, {{ value: async (args) => globalThis.__codex_tool_call({tool_name}, args), configurable: false, enumerable: true, writable: false }});\n"
        ));
    }
    source.push_str("Object.freeze(tools);\nexport { tools };\n");
    for tool in enabled_tools {
        if namespaces_match(&tool.namespace, namespace) && is_valid_identifier(&tool.name) {
            let export_name = js_string_literal(&tool.name)?;
            source.push_str(&format!(
                "export const {} = tools[{export_name}];\n",
                tool.name
            ));
        }
    }
    Ok(source)
}

fn namespaces_match(left: &[String], right: &[&str]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .map(String::as_str)
            .zip(right.iter().copied())
            .all(|(left, right)| left == right)
}

fn js_string_literal(value: &str) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|err| format!("failed to serialize code_mode string: {err}"))
}

fn instantiate_and_evaluate_module(
    scope: &mut v8::PinScope<'_, '_>,
    module: v8::Local<'_, v8::Module>,
) -> Result<(), String> {
    match module.get_status() {
        v8::ModuleStatus::Uninstantiated => {
            let Some(instantiated) = module.instantiate_module(scope, resolve_code_mode_module)
            else {
                return Err("failed to instantiate code_mode module".to_string());
            };
            if !instantiated {
                return Err("failed to instantiate code_mode module".to_string());
            }
        }
        v8::ModuleStatus::Instantiating => {
            return Err("code_mode module is already instantiating".to_string());
        }
        v8::ModuleStatus::Instantiated
        | v8::ModuleStatus::Evaluating
        | v8::ModuleStatus::Evaluated => {}
        v8::ModuleStatus::Errored => {
            return Err(format_v8_value(scope, module.get_exception()));
        }
    }

    match module.get_status() {
        v8::ModuleStatus::Instantiated => {
            let Some(result) = module.evaluate(scope) else {
                return Err("failed to evaluate code_mode module".to_string());
            };
            if result.is_promise() {
                let promise = v8::Local::<v8::Promise>::try_from(result).map_err(|_| {
                    "code_mode module evaluation did not return a promise".to_string()
                })?;
                scope.perform_microtask_checkpoint();
                match promise.state() {
                    v8::PromiseState::Fulfilled => {}
                    v8::PromiseState::Rejected => {
                        return Err(format_v8_value(scope, promise.result(scope)));
                    }
                    v8::PromiseState::Pending => {
                        return Err("code_mode module evaluation did not settle".to_string());
                    }
                }
            }
        }
        v8::ModuleStatus::Evaluated => {}
        v8::ModuleStatus::Evaluating => {
            return Err("code_mode module is already evaluating".to_string());
        }
        v8::ModuleStatus::Errored => {
            return Err(format_v8_value(scope, module.get_exception()));
        }
        v8::ModuleStatus::Uninstantiated | v8::ModuleStatus::Instantiating => {}
    }

    Ok(())
}

fn code_mode_tool_call_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue<v8::Value>,
) {
    let Some(resolver) = v8::PromiseResolver::new(scope) else {
        return;
    };
    let promise = resolver.get_promise(scope);
    rv.set(promise.into());

    let result = run_tool_call(scope, &args).and_then(|value| json_to_v8(scope, &value));
    match result {
        Ok(value) => {
            let _ = resolver.resolve(scope, value);
        }
        Err(error) => {
            if let Some(error) = v8::String::new(scope, &error) {
                let _ = resolver.reject(scope, error.into());
            }
        }
    }
}

fn set_max_output_tokens_per_exec_call_callback(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue<v8::Value>,
) {
    let Some(value) = args.get(0).integer_value(scope) else {
        let _ = throw_v8_exception::<v8::Value>(
            scope,
            "max_output_tokens_per_exec_call must be a non-negative safe integer",
        );
        return;
    };
    let Ok(value) = usize::try_from(value) else {
        let _ = throw_v8_exception::<v8::Value>(
            scope,
            "max_output_tokens_per_exec_call must be a non-negative safe integer",
        );
        return;
    };

    let Some(runtime_state) = scope.get_slot_mut::<RuntimeState>() else {
        let _ = throw_v8_exception::<v8::Value>(scope, "code_mode runtime state missing");
        return;
    };
    runtime_state.max_output_tokens_per_exec_call = value;
    rv.set(v8::Number::new(scope, value as f64).into());
}

fn run_tool_call(
    scope: &mut v8::PinScope<'_, '_>,
    args: &v8::FunctionCallbackArguments,
) -> Result<JsonValue, String> {
    let tool_name = args
        .get(0)
        .to_string(scope)
        .ok_or_else(|| "code_mode tool call requires a tool name".to_string())?
        .to_rust_string_lossy(scope);
    let input = json_from_v8(scope, args.get(1))?;

    let Some(runtime_state) = scope.get_slot_mut::<RuntimeState>() else {
        return Err("code_mode runtime state missing".to_string());
    };
    (runtime_state.on_tool_call)(tool_name, input)
}

fn wait_for_module_promise(
    scope: &mut v8::PinScope<'_, '_>,
    module: v8::Local<'_, v8::Module>,
    promise: v8::Local<'_, v8::Promise>,
) -> Result<(), String> {
    for _ in 0..32 {
        match promise.state() {
            v8::PromiseState::Fulfilled => return Ok(()),
            v8::PromiseState::Rejected => {
                return Err(format_v8_value(scope, promise.result(scope)));
            }
            v8::PromiseState::Pending => {
                scope.perform_microtask_checkpoint();
            }
        }
    }

    let stalled = module.get_stalled_top_level_await_message(scope);
    if let Some((_module, message)) = stalled.into_iter().next() {
        let pending = message.get(scope).to_rust_string_lossy(scope);
        let filename = message
            .get_script_resource_name(scope)
            .map(|name| name.to_rust_string_lossy(scope))
            .unwrap_or_else(|| CODE_MODE_MAIN_FILENAME.to_string());
        let line = message.get_line_number(scope).unwrap_or_default();
        return Err(format!("{filename}:{line}: {pending}"));
    }

    Err("code_mode top-level await did not settle".to_string())
}

fn read_content_items(scope: &mut v8::PinScope<'_, '_>) -> Result<Vec<JsonValue>, String> {
    read_json_global(scope, "globalThis.__codexContentItems ?? []")
}

fn read_stored_values(
    scope: &mut v8::PinScope<'_, '_>,
) -> Result<HashMap<String, JsonValue>, String> {
    read_json_global(scope, "globalThis.__codexStoredValues ?? {}")
}

fn read_json_global<T>(scope: &mut v8::PinScope<'_, '_>, expression: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let source = v8_string(scope, &format!("JSON.stringify({expression})"))?;
    let script = v8::Script::compile(scope, source, None)
        .ok_or_else(|| format!("failed to read {expression}"))?;
    let value = script
        .run(scope)
        .ok_or_else(|| format!("failed to evaluate {expression}"))?;
    let serialized = value
        .to_string(scope)
        .ok_or_else(|| format!("failed to serialize {expression}"))?
        .to_rust_string_lossy(scope);
    serde_json::from_str(&serialized).map_err(|err| format!("invalid {expression}: {err}"))
}

fn build_bootstrap_source(
    enabled_tools: &[EnabledTool],
    stored_values: &HashMap<String, JsonValue>,
) -> Result<String, String> {
    let enabled_tools_json = serde_json::to_string(enabled_tools)
        .map_err(|err| format!("failed to serialize enabled tools: {err}"))?;
    let stored_values_json = serde_json::to_string(stored_values)
        .map_err(|err| format!("failed to serialize code_mode stored values: {err}"))?;
    Ok(CODE_MODE_BOOTSTRAP_SOURCE
        .replace(
            "__CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__",
            &enabled_tools_json,
        )
        .replace(
            "__CODE_MODE_STORED_VALUES_PLACEHOLDER__",
            &stored_values_json,
        ))
}

fn v8_string<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    text: &str,
) -> Result<v8::Local<'s, v8::String>, String> {
    v8::String::new(scope, text).ok_or_else(|| "failed to allocate V8 string".to_string())
}

fn json_from_v8(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
) -> Result<Option<JsonValue>, String> {
    if value.is_undefined() {
        return Ok(None);
    }

    let Some(serialized) = v8::json::stringify(scope, value) else {
        return Err("code_mode tool arguments must be JSON-serializable".to_string());
    };
    let serialized = serialized.to_rust_string_lossy(scope);
    serde_json::from_str(&serialized)
        .map(Some)
        .map_err(|err| format!("invalid code_mode tool arguments: {err}"))
}

fn json_to_v8<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: &JsonValue,
) -> Result<v8::Local<'s, v8::Value>, String> {
    let serialized = serde_json::to_string(value)
        .map_err(|err| format!("failed to serialize code_mode tool response: {err}"))?;
    let serialized = v8_string(scope, &serialized)?;
    v8::json::parse(scope, serialized)
        .ok_or_else(|| "failed to deserialize code_mode tool response into V8".to_string())
}

fn throw_v8_exception<'s, T>(
    scope: &mut v8::PinScope<'s, '_>,
    message: &str,
) -> Option<v8::Local<'s, T>> {
    if let Some(message) = v8::String::new(scope, message) {
        scope.throw_exception(message.into());
    }
    None
}

fn format_v8_exception(try_catch: &mut v8::PinnedRef<'_, v8::TryCatch<v8::HandleScope>>) -> String {
    let Some(exception) = try_catch.exception() else {
        return "JavaScript execution failed".to_string();
    };

    if let Some(stack_trace) = try_catch.stack_trace()
        && let Some(stack_trace) = stack_trace.to_string(try_catch)
    {
        let stack_trace = stack_trace.to_rust_string_lossy(try_catch);
        if !stack_trace.trim().is_empty() {
            return stack_trace;
        }
    }

    let exception_string = exception
        .to_string(try_catch)
        .map(|value| value.to_rust_string_lossy(try_catch))
        .unwrap_or_else(|| "JavaScript execution failed".to_string());
    let Some(message) = try_catch.message() else {
        return exception_string;
    };

    let filename = message
        .get_script_resource_name(try_catch)
        .and_then(|value| value.to_string(try_catch))
        .map(|value| value.to_rust_string_lossy(try_catch))
        .unwrap_or_else(|| "(unknown)".to_string());
    let line = message.get_line_number(try_catch).unwrap_or_default();
    format!("{filename}:{line}: {exception_string}")
}

fn format_v8_value(scope: &mut v8::PinScope<'_, '_>, value: v8::Local<'_, v8::Value>) -> String {
    if value.is_object()
        && let Ok(object) = v8::Local::<v8::Object>::try_from(value)
        && let Some(stack_key) = v8::String::new(scope, "stack")
        && let Some(stack_value) = object.get(scope, stack_key.into())
        && let Some(stack_value) = stack_value.to_string(scope)
    {
        let stack_value = stack_value.to_rust_string_lossy(scope);
        if !stack_value.trim().is_empty() {
            return stack_value;
        }
    }

    value
        .to_string(scope)
        .map(|value| value.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "JavaScript execution failed".to_string())
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}
