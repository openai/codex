use crate::ClientNotification;
use crate::ClientRequest;
use crate::ServerNotification;
use crate::ServerRequest;
use crate::experimental_api::experimental_fields;
use crate::export_client_notification_schemas;
use crate::export_client_param_schemas;
use crate::export_client_response_schemas;
use crate::export_client_responses;
use crate::export_server_notification_schemas;
use crate::export_server_param_schemas;
use crate::export_server_response_schemas;
use crate::export_server_responses;
use crate::protocol::common::EXPERIMENTAL_CLIENT_METHOD_PARAM_TYPES;
use crate::protocol::common::EXPERIMENTAL_CLIENT_METHOD_RESPONSE_TYPES;
use crate::protocol::common::EXPERIMENTAL_CLIENT_METHODS;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_protocol::protocol::EventMsg;
use schemars::JsonSchema;
use schemars::schema_for;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use ts_rs::TS;

const HEADER: &str = "// GENERATED CODE! DO NOT MODIFY BY HAND!\n\n";
const IGNORED_DEFINITIONS: &[&str] = &["Option<()>"];

#[derive(Clone)]
pub struct GeneratedSchema {
    namespace: Option<String>,
    logical_name: String,
    value: Value,
    in_v1_dir: bool,
}

impl GeneratedSchema {
    fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    fn logical_name(&self) -> &str {
        &self.logical_name
    }

    fn value(&self) -> &Value {
        &self.value
    }
}

type JsonSchemaEmitter = fn(&Path) -> Result<GeneratedSchema>;
pub fn generate_types(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts(out_dir, prettier)?;
    generate_json(out_dir)?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct GenerateTsOptions {
    pub generate_indices: bool,
    pub ensure_headers: bool,
    pub run_prettier: bool,
    pub experimental_api: bool,
}

impl Default for GenerateTsOptions {
    fn default() -> Self {
        Self {
            generate_indices: true,
            ensure_headers: true,
            run_prettier: true,
            experimental_api: false,
        }
    }
}

pub fn generate_ts(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts_with_options(out_dir, prettier, GenerateTsOptions::default())
}

pub fn generate_ts_with_options(
    out_dir: &Path,
    prettier: Option<&Path>,
    options: GenerateTsOptions,
) -> Result<()> {
    let v2_out_dir = out_dir.join("v2");
    ensure_dir(out_dir)?;
    ensure_dir(&v2_out_dir)?;

    ClientRequest::export_all_to(out_dir)?;
    export_client_responses(out_dir)?;
    ClientNotification::export_all_to(out_dir)?;

    ServerRequest::export_all_to(out_dir)?;
    export_server_responses(out_dir)?;
    ServerNotification::export_all_to(out_dir)?;

    if !options.experimental_api {
        filter_experimental_ts(out_dir)?;
    }

    if options.generate_indices {
        generate_index_ts(out_dir)?;
        generate_index_ts(&v2_out_dir)?;
    }

    // Ensure our header is present on all TS files (root + subdirs like v2/).
    let mut ts_files = Vec::new();
    let should_collect_ts_files =
        options.ensure_headers || (options.run_prettier && prettier.is_some());
    if should_collect_ts_files {
        ts_files = ts_files_in_recursive(out_dir)?;
    }

    if options.ensure_headers {
        for file in &ts_files {
            prepend_header_if_missing(file)?;
        }
    }

    // Optionally run Prettier on all generated TS files.
    if options.run_prettier
        && let Some(prettier_bin) = prettier
        && !ts_files.is_empty()
    {
        let status = Command::new(prettier_bin)
            .arg("--write")
            .arg("--log-level")
            .arg("warn")
            .args(ts_files.iter().map(|p| p.as_os_str()))
            .status()
            .with_context(|| format!("Failed to invoke Prettier at {}", prettier_bin.display()))?;
        if !status.success() {
            return Err(anyhow!("Prettier failed with status {status}"));
        }
    }

    Ok(())
}

pub fn generate_json(out_dir: &Path) -> Result<()> {
    generate_json_with_experimental(out_dir, false)
}

pub fn generate_json_with_experimental(out_dir: &Path, experimental_api: bool) -> Result<()> {
    ensure_dir(out_dir)?;
    let envelope_emitters: Vec<JsonSchemaEmitter> = vec![
        |d| write_json_schema_with_return::<crate::RequestId>(d, "RequestId"),
        |d| write_json_schema_with_return::<crate::JSONRPCMessage>(d, "JSONRPCMessage"),
        |d| write_json_schema_with_return::<crate::JSONRPCRequest>(d, "JSONRPCRequest"),
        |d| write_json_schema_with_return::<crate::JSONRPCNotification>(d, "JSONRPCNotification"),
        |d| write_json_schema_with_return::<crate::JSONRPCResponse>(d, "JSONRPCResponse"),
        |d| write_json_schema_with_return::<crate::JSONRPCError>(d, "JSONRPCError"),
        |d| write_json_schema_with_return::<crate::JSONRPCErrorError>(d, "JSONRPCErrorError"),
        |d| write_json_schema_with_return::<crate::ClientRequest>(d, "ClientRequest"),
        |d| write_json_schema_with_return::<crate::ServerRequest>(d, "ServerRequest"),
        |d| write_json_schema_with_return::<crate::ClientNotification>(d, "ClientNotification"),
        |d| write_json_schema_with_return::<crate::ServerNotification>(d, "ServerNotification"),
        |d| write_json_schema_with_return::<EventMsg>(d, "EventMsg"),
    ];

    let mut schemas: Vec<GeneratedSchema> = Vec::new();
    for emit in &envelope_emitters {
        schemas.push(emit(out_dir)?);
    }

    schemas.extend(export_client_param_schemas(out_dir)?);
    schemas.extend(export_client_response_schemas(out_dir)?);
    schemas.extend(export_server_param_schemas(out_dir)?);
    schemas.extend(export_server_response_schemas(out_dir)?);
    schemas.extend(export_client_notification_schemas(out_dir)?);
    schemas.extend(export_server_notification_schemas(out_dir)?);

    let mut bundle = build_schema_bundle(schemas)?;
    if !experimental_api {
        filter_experimental_schema(&mut bundle)?;
    }
    write_pretty_json(
        out_dir.join("codex_app_server_protocol.schemas.json"),
        &bundle,
    )?;

    if !experimental_api {
        filter_experimental_json_files(out_dir)?;
    }

    Ok(())
}

fn filter_experimental_ts(out_dir: &Path) -> Result<()> {
    let registered_fields = experimental_fields();
    let experimental_method_types = experimental_method_types();
    // Most generated TS files are filtered by schema processing, but
    // `ClientRequest.ts` and any type with `#[experimental(...)]` fields need
    // direct post-processing because they encode method/field information in
    // file-local unions/interfaces.
    filter_client_request_ts(out_dir, EXPERIMENTAL_CLIENT_METHODS)?;
    filter_experimental_type_fields_ts(out_dir, &registered_fields)?;
    remove_generated_type_files(out_dir, &experimental_method_types, "ts")?;
    Ok(())
}

/// Removes union arms from `ClientRequest.ts` for methods marked experimental.
fn filter_client_request_ts(out_dir: &Path, experimental_methods: &[&str]) -> Result<()> {
    let path = out_dir.join("ClientRequest.ts");
    if !path.exists() {
        return Ok(());
    }
    let mut content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let Some((prefix, body, suffix)) = split_type_alias(&content) else {
        return Ok(());
    };
    let experimental_methods: HashSet<&str> = experimental_methods
        .iter()
        .copied()
        .filter(|method| !method.is_empty())
        .collect();
    let arms = split_top_level(&body, '|');
    let filtered_arms: Vec<String> = arms
        .into_iter()
        .filter(|arm| {
            extract_method_from_arm(arm)
                .is_none_or(|method| !experimental_methods.contains(method.as_str()))
        })
        .collect();
    let new_body = filtered_arms.join(" | ");
    content = format!("{prefix}{new_body}{suffix}");
    let import_usage_scope = split_type_alias(&content)
        .map(|(_, body, _)| body)
        .unwrap_or_else(|| new_body.clone());
    content = prune_unused_type_imports(content, &import_usage_scope);

    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Removes experimental properties from generated TypeScript type files.
fn filter_experimental_type_fields_ts(
    out_dir: &Path,
    experimental_fields: &[&'static crate::experimental_api::ExperimentalField],
) -> Result<()> {
    let mut fields_by_type_name: HashMap<String, HashSet<String>> = HashMap::new();
    for field in experimental_fields {
        fields_by_type_name
            .entry(field.type_name.to_string())
            .or_default()
            .insert(field.field_name.to_string());
    }
    if fields_by_type_name.is_empty() {
        return Ok(());
    }

    for path in ts_files_in_recursive(out_dir)? {
        let Some(type_name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(experimental_field_names) = fields_by_type_name.get(type_name) else {
            continue;
        };
        filter_experimental_fields_in_ts_file(&path, experimental_field_names)?;
    }

    Ok(())
}

fn filter_experimental_fields_in_ts_file(
    path: &Path,
    experimental_field_names: &HashSet<String>,
) -> Result<()> {
    let mut content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let Some((open_brace, close_brace)) = type_body_brace_span(&content) else {
        return Ok(());
    };
    let inner = &content[open_brace + 1..close_brace];
    let fields = split_top_level_multi(inner, &[',', ';']);
    let filtered_fields: Vec<String> = fields
        .into_iter()
        .filter(|field| {
            let field = strip_leading_block_comments(field);
            parse_property_name(field)
                .is_none_or(|name| !experimental_field_names.contains(name.as_str()))
        })
        .collect();
    let new_inner = filtered_fields.join(", ");
    let prefix = &content[..open_brace + 1];
    let suffix = &content[close_brace..];
    content = format!("{prefix}{new_inner}{suffix}");
    let import_usage_scope = split_type_alias(&content)
        .map(|(_, body, _)| body)
        .unwrap_or_else(|| new_inner.clone());
    content = prune_unused_type_imports(content, &import_usage_scope);
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn filter_experimental_schema(bundle: &mut Value) -> Result<()> {
    let registered_fields = experimental_fields();
    filter_experimental_fields_in_root(bundle, &registered_fields);
    filter_experimental_fields_in_definitions(bundle, &registered_fields);
    prune_experimental_methods(bundle, EXPERIMENTAL_CLIENT_METHODS);
    remove_experimental_method_type_definitions(bundle);
    Ok(())
}

fn filter_experimental_fields_in_root(
    schema: &mut Value,
    experimental_fields: &[&'static crate::experimental_api::ExperimentalField],
) {
    let Some(title) = schema.get("title").and_then(Value::as_str) else {
        return;
    };
    let title = title.to_string();

    for field in experimental_fields {
        if title != field.type_name {
            continue;
        }
        remove_property_from_schema(schema, field.field_name);
    }
}

fn filter_experimental_fields_in_definitions(
    bundle: &mut Value,
    experimental_fields: &[&'static crate::experimental_api::ExperimentalField],
) {
    let Some(definitions) = bundle.get_mut("definitions").and_then(Value::as_object_mut) else {
        return;
    };

    filter_experimental_fields_in_definitions_map(definitions, experimental_fields);
}

fn filter_experimental_fields_in_definitions_map(
    definitions: &mut Map<String, Value>,
    experimental_fields: &[&'static crate::experimental_api::ExperimentalField],
) {
    for (def_name, def_schema) in definitions.iter_mut() {
        if is_namespace_map(def_schema) {
            if let Some(namespace_defs) = def_schema.as_object_mut() {
                filter_experimental_fields_in_definitions_map(namespace_defs, experimental_fields);
            }
            continue;
        }

        for field in experimental_fields {
            if !definition_matches_type(def_name, field.type_name) {
                continue;
            }
            remove_property_from_schema(def_schema, field.field_name);
        }
    }
}

fn is_namespace_map(value: &Value) -> bool {
    let Value::Object(map) = value else {
        return false;
    };

    if map.keys().any(|key| key.starts_with('$')) {
        return false;
    }

    let looks_like_schema = map.contains_key("type")
        || map.contains_key("properties")
        || map.contains_key("anyOf")
        || map.contains_key("oneOf")
        || map.contains_key("allOf");

    !looks_like_schema && map.values().all(Value::is_object)
}

fn definition_matches_type(def_name: &str, type_name: &str) -> bool {
    def_name == type_name || def_name.ends_with(&format!("::{type_name}"))
}

fn remove_property_from_schema(schema: &mut Value, field_name: &str) {
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove(field_name);
    }

    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|entry| entry.as_str() != Some(field_name));
    }

    if let Some(inner_schema) = schema.get_mut("schema") {
        remove_property_from_schema(inner_schema, field_name);
    }
}

fn prune_experimental_methods(bundle: &mut Value, experimental_methods: &[&str]) {
    let experimental_methods: HashSet<&str> = experimental_methods
        .iter()
        .copied()
        .filter(|method| !method.is_empty())
        .collect();
    prune_experimental_methods_inner(bundle, &experimental_methods);
}

fn prune_experimental_methods_inner(value: &mut Value, experimental_methods: &HashSet<&str>) {
    match value {
        Value::Array(items) => {
            items.retain(|item| !is_experimental_method_variant(item, experimental_methods));
            for item in items {
                prune_experimental_methods_inner(item, experimental_methods);
            }
        }
        Value::Object(map) => {
            for entry in map.values_mut() {
                prune_experimental_methods_inner(entry, experimental_methods);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_experimental_method_variant(value: &Value, experimental_methods: &HashSet<&str>) -> bool {
    let Value::Object(map) = value else {
        return false;
    };
    let Some(properties) = map.get("properties").and_then(Value::as_object) else {
        return false;
    };
    let Some(method_schema) = properties.get("method").and_then(Value::as_object) else {
        return false;
    };

    if let Some(method) = method_schema.get("const").and_then(Value::as_str) {
        return experimental_methods.contains(method);
    }

    if let Some(values) = method_schema.get("enum").and_then(Value::as_array)
        && values.len() == 1
        && let Some(method) = values[0].as_str()
    {
        return experimental_methods.contains(method);
    }

    false
}

fn filter_experimental_json_files(out_dir: &Path) -> Result<()> {
    for path in json_files_in_recursive(out_dir)? {
        let mut value = read_json_value(&path)?;
        filter_experimental_schema(&mut value)?;
        write_pretty_json(path, &value)?;
    }
    let experimental_method_types = experimental_method_types();
    remove_generated_type_files(out_dir, &experimental_method_types, "json")?;
    Ok(())
}

fn experimental_method_types() -> HashSet<String> {
    let mut type_names = HashSet::new();
    collect_experimental_type_names(EXPERIMENTAL_CLIENT_METHOD_PARAM_TYPES, &mut type_names);
    collect_experimental_type_names(EXPERIMENTAL_CLIENT_METHOD_RESPONSE_TYPES, &mut type_names);
    type_names
}

fn collect_experimental_type_names(entries: &[&str], out: &mut HashSet<String>) {
    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let name = trimmed.rsplit("::").next().unwrap_or(trimmed);
        if !name.is_empty() {
            out.insert(name.to_string());
        }
    }
}

fn remove_generated_type_files(
    out_dir: &Path,
    type_names: &HashSet<String>,
    extension: &str,
) -> Result<()> {
    for type_name in type_names {
        for subdir in ["", "v1", "v2"] {
            let path = if subdir.is_empty() {
                out_dir.join(format!("{type_name}.{extension}"))
            } else {
                out_dir
                    .join(subdir)
                    .join(format!("{type_name}.{extension}"))
            };
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn remove_experimental_method_type_definitions(bundle: &mut Value) {
    let type_names = experimental_method_types();
    let Some(definitions) = bundle.get_mut("definitions").and_then(Value::as_object_mut) else {
        return;
    };
    remove_experimental_method_type_definitions_map(definitions, &type_names);
}

fn remove_experimental_method_type_definitions_map(
    definitions: &mut Map<String, Value>,
    experimental_type_names: &HashSet<String>,
) {
    let keys_to_remove: Vec<String> = definitions
        .keys()
        .filter(|def_name| {
            experimental_type_names
                .iter()
                .any(|type_name| definition_matches_type(def_name, type_name))
        })
        .cloned()
        .collect();
    for key in keys_to_remove {
        definitions.remove(&key);
    }

    for value in definitions.values_mut() {
        if !is_namespace_map(value) {
            continue;
        }
        if let Some(namespace_defs) = value.as_object_mut() {
            remove_experimental_method_type_definitions_map(
                namespace_defs,
                experimental_type_names,
            );
        }
    }
}

fn prune_unused_type_imports(content: String, type_alias_body: &str) -> String {
    let trailing_newline = content.ends_with('\n');
    let mut lines = Vec::new();
    for line in content.lines() {
        if let Some(type_name) = parse_imported_type_name(line)
            && !type_alias_body.contains(type_name)
        {
            continue;
        }
        lines.push(line);
    }

    let mut rewritten = lines.join("\n");
    if trailing_newline {
        rewritten.push('\n');
    }
    rewritten
}

fn parse_imported_type_name(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = line.strip_prefix("import type {")?;
    let (type_name, _) = rest.split_once("} from ")?;
    let type_name = type_name.trim();
    if type_name.is_empty() || type_name.contains(',') || type_name.contains(" as ") {
        return None;
    }
    Some(type_name)
}

fn json_files_in_recursive(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if matches!(path.extension().and_then(|ext| ext.to_str()), Some("json")) {
                out.push(path);
            }
        }
    }
    Ok(out)
}

fn read_json_value(path: &Path) -> Result<Value> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn split_type_alias(content: &str) -> Option<(String, String, String)> {
    let eq_index = content.find('=')?;
    let semi_index = content.rfind(';')?;
    if semi_index <= eq_index {
        return None;
    }
    let prefix = content[..eq_index + 1].to_string();
    let body = content[eq_index + 1..semi_index].to_string();
    let suffix = content[semi_index..].to_string();
    Some((prefix, body, suffix))
}

fn type_body_brace_span(content: &str) -> Option<(usize, usize)> {
    if let Some(eq_index) = content.find('=') {
        let after_eq = &content[eq_index + 1..];
        let (open_rel, close_rel) = find_top_level_brace_span(after_eq)?;
        return Some((eq_index + 1 + open_rel, eq_index + 1 + close_rel));
    }

    const INTERFACE_MARKER: &str = "export interface";
    let interface_index = content.find(INTERFACE_MARKER)?;
    let after_interface = &content[interface_index + INTERFACE_MARKER.len()..];
    let (open_rel, close_rel) = find_top_level_brace_span(after_interface)?;
    Some((
        interface_index + INTERFACE_MARKER.len() + open_rel,
        interface_index + INTERFACE_MARKER.len() + close_rel,
    ))
}

fn find_top_level_brace_span(input: &str) -> Option<(usize, usize)> {
    let mut state = ScanState::default();
    let mut open_index = None;
    for (index, ch) in input.char_indices() {
        if !state.in_string() && ch == '{' && state.depth.is_top_level() {
            open_index = Some(index);
        }
        state.observe(ch);
        if !state.in_string()
            && ch == '}'
            && state.depth.is_top_level()
            && let Some(open) = open_index
        {
            return Some((open, index));
        }
    }
    None
}

fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    split_top_level_multi(input, &[delimiter])
}

fn split_top_level_multi(input: &str, delimiters: &[char]) -> Vec<String> {
    let mut state = ScanState::default();
    let mut start = 0usize;
    let mut parts = Vec::new();
    for (index, ch) in input.char_indices() {
        if !state.in_string() && state.depth.is_top_level() && delimiters.contains(&ch) {
            let part = input[start..index].trim();
            if !part.is_empty() {
                parts.push(part.to_string());
            }
            start = index + ch.len_utf8();
        }
        state.observe(ch);
    }
    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    parts
}

fn extract_method_from_arm(arm: &str) -> Option<String> {
    let (open, close) = find_top_level_brace_span(arm)?;
    let inner = &arm[open + 1..close];
    for field in split_top_level(inner, ',') {
        let Some((name, value)) = parse_property(field.as_str()) else {
            continue;
        };
        if name != "method" {
            continue;
        }
        let value = value.trim_start();
        let (literal, _) = parse_string_literal(value)?;
        return Some(literal);
    }
    None
}

fn parse_property(input: &str) -> Option<(String, &str)> {
    let name = parse_property_name(input)?;
    let colon_index = input.find(':')?;
    Some((name, input[colon_index + 1..].trim_start()))
}

fn strip_leading_block_comments(input: &str) -> &str {
    let mut rest = input.trim_start();
    loop {
        let Some(after_prefix) = rest.strip_prefix("/*") else {
            return rest;
        };
        let Some(end_rel) = after_prefix.find("*/") else {
            return rest;
        };
        rest = after_prefix[end_rel + 2..].trim_start();
    }
}

fn parse_property_name(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((literal, consumed)) = parse_string_literal(trimmed) {
        let rest = trimmed[consumed..].trim_start();
        if rest.starts_with(':') {
            return Some(literal);
        }
        return None;
    }

    let mut end = 0usize;
    for (index, ch) in trimmed.char_indices() {
        if !is_ident_char(ch) {
            break;
        }
        end = index + ch.len_utf8();
    }
    if end == 0 {
        return None;
    }
    let name = &trimmed[..end];
    let rest = trimmed[end..].trim_start();
    let rest = if let Some(stripped) = rest.strip_prefix('?') {
        stripped.trim_start()
    } else {
        rest
    };
    if rest.starts_with(':') {
        return Some(name.to_string());
    }
    None
}

fn parse_string_literal(input: &str) -> Option<(String, usize)> {
    let mut chars = input.char_indices();
    let (start_index, quote) = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut escape = false;
    for (index, ch) in chars {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == quote {
            let literal = input[start_index + 1..index].to_string();
            let consumed = index + ch.len_utf8();
            return Some((literal, consumed));
        }
    }
    None
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[derive(Default)]
struct ScanState {
    depth: Depth,
    string_delim: Option<char>,
    escape: bool,
}

impl ScanState {
    fn observe(&mut self, ch: char) {
        if let Some(delim) = self.string_delim {
            if self.escape {
                self.escape = false;
                return;
            }
            if ch == '\\' {
                self.escape = true;
                return;
            }
            if ch == delim {
                self.string_delim = None;
            }
            return;
        }

        match ch {
            '"' | '\'' => {
                self.string_delim = Some(ch);
            }
            '{' => self.depth.brace += 1,
            '}' => self.depth.brace = (self.depth.brace - 1).max(0),
            '[' => self.depth.bracket += 1,
            ']' => self.depth.bracket = (self.depth.bracket - 1).max(0),
            '(' => self.depth.paren += 1,
            ')' => self.depth.paren = (self.depth.paren - 1).max(0),
            '<' => self.depth.angle += 1,
            '>' => {
                if self.depth.angle > 0 {
                    self.depth.angle -= 1;
                }
            }
            _ => {}
        }
    }

    fn in_string(&self) -> bool {
        self.string_delim.is_some()
    }
}

#[derive(Default)]
struct Depth {
    brace: i32,
    bracket: i32,
    paren: i32,
    angle: i32,
}

impl Depth {
    fn is_top_level(&self) -> bool {
        self.brace == 0 && self.bracket == 0 && self.paren == 0 && self.angle == 0
    }
}

fn build_schema_bundle(schemas: Vec<GeneratedSchema>) -> Result<Value> {
    const SPECIAL_DEFINITIONS: &[&str] = &[
        "ClientNotification",
        "ClientRequest",
        "EventMsg",
        "ServerNotification",
        "ServerRequest",
    ];

    let namespaced_types = collect_namespaced_types(&schemas);
    let mut definitions = Map::new();

    for schema in schemas {
        let GeneratedSchema {
            namespace,
            logical_name,
            mut value,
            in_v1_dir,
        } = schema;

        if IGNORED_DEFINITIONS.contains(&logical_name.as_str()) {
            continue;
        }

        if let Some(ref ns) = namespace {
            rewrite_refs_to_namespace(&mut value, ns);
        }

        let mut forced_namespace_refs: Vec<(String, String)> = Vec::new();
        if let Value::Object(ref mut obj) = value
            && let Some(defs) = obj.remove("definitions")
            && let Value::Object(defs_obj) = defs
        {
            for (def_name, mut def_schema) in defs_obj {
                if IGNORED_DEFINITIONS.contains(&def_name.as_str()) {
                    continue;
                }
                if SPECIAL_DEFINITIONS.contains(&def_name.as_str()) {
                    continue;
                }
                annotate_schema(&mut def_schema, Some(def_name.as_str()));
                let target_namespace = match namespace {
                    Some(ref ns) => Some(ns.clone()),
                    None => namespace_for_definition(&def_name, &namespaced_types)
                        .cloned()
                        .filter(|_| !in_v1_dir),
                };
                if let Some(ref ns) = target_namespace {
                    if namespace.as_deref() == Some(ns.as_str()) {
                        rewrite_refs_to_namespace(&mut def_schema, ns);
                        insert_into_namespace(&mut definitions, ns, def_name.clone(), def_schema)?;
                    } else if !forced_namespace_refs
                        .iter()
                        .any(|(name, existing_ns)| name == &def_name && existing_ns == ns)
                    {
                        forced_namespace_refs.push((def_name.clone(), ns.clone()));
                    }
                } else {
                    definitions.insert(def_name, def_schema);
                }
            }
        }

        for (name, ns) in forced_namespace_refs {
            rewrite_named_ref_to_namespace(&mut value, &ns, &name);
        }

        if let Some(ref ns) = namespace {
            insert_into_namespace(&mut definitions, ns, logical_name.clone(), value)?;
        } else {
            definitions.insert(logical_name, value);
        }
    }

    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        Value::String("http://json-schema.org/draft-07/schema#".into()),
    );
    root.insert(
        "title".to_string(),
        Value::String("CodexAppServerProtocol".into()),
    );
    root.insert("type".to_string(), Value::String("object".into()));
    root.insert("definitions".to_string(), Value::Object(definitions));

    Ok(Value::Object(root))
}

fn insert_into_namespace(
    definitions: &mut Map<String, Value>,
    namespace: &str,
    name: String,
    schema: Value,
) -> Result<()> {
    let entry = definitions
        .entry(namespace.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    match entry {
        Value::Object(map) => {
            map.insert(name, schema);
            Ok(())
        }
        _ => Err(anyhow!("expected namespace {namespace} to be an object")),
    }
}

fn write_json_schema_with_return<T>(out_dir: &Path, name: &str) -> Result<GeneratedSchema>
where
    T: JsonSchema,
{
    let file_stem = name.trim();
    let schema = schema_for!(T);
    let mut schema_value = serde_json::to_value(schema)?;
    annotate_schema(&mut schema_value, Some(file_stem));
    // If the name looks like a namespaced path (e.g., "v2::Type"), mirror
    // the TypeScript layout and write to out_dir/v2/Type.json. Otherwise
    // write alongside the legacy files.
    let (raw_namespace, logical_name) = split_namespace(file_stem);
    let out_path = if let Some(ns) = raw_namespace {
        let dir = out_dir.join(ns);
        ensure_dir(&dir)?;
        dir.join(format!("{logical_name}.json"))
    } else {
        out_dir.join(format!("{file_stem}.json"))
    };

    if !IGNORED_DEFINITIONS.contains(&logical_name) {
        write_pretty_json(out_path, &schema_value)
            .with_context(|| format!("Failed to write JSON schema for {file_stem}"))?;
    }

    let namespace = match raw_namespace {
        Some("v1") | None => None,
        Some(ns) => Some(ns.to_string()),
    };
    Ok(GeneratedSchema {
        in_v1_dir: raw_namespace == Some("v1"),
        namespace,
        logical_name: logical_name.to_string(),
        value: schema_value,
    })
}

pub(crate) fn write_json_schema<T>(out_dir: &Path, name: &str) -> Result<GeneratedSchema>
where
    T: JsonSchema,
{
    write_json_schema_with_return::<T>(out_dir, name)
}

fn write_pretty_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_vec_pretty(value)
        .with_context(|| format!("Failed to serialize JSON schema to {}", path.display()))?;
    fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Split a fully-qualified type name like "v2::Type" into its namespace and logical name.
fn split_namespace(name: &str) -> (Option<&str>, &str) {
    name.split_once("::")
        .map_or((None, name), |(ns, rest)| (Some(ns), rest))
}

/// Recursively rewrite $ref values that point at "#/definitions/..." so that
/// they point to a namespaced location under the bundle.
fn rewrite_refs_to_namespace(value: &mut Value, ns: &str) {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(r)) = obj.get_mut("$ref")
                && let Some(suffix) = r.strip_prefix("#/definitions/")
            {
                let prefix = format!("{ns}/");
                if !suffix.starts_with(&prefix) {
                    *r = format!("#/definitions/{ns}/{suffix}");
                }
            }
            for v in obj.values_mut() {
                rewrite_refs_to_namespace(v, ns);
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                rewrite_refs_to_namespace(v, ns);
            }
        }
        _ => {}
    }
}

fn collect_namespaced_types(schemas: &[GeneratedSchema]) -> HashMap<String, String> {
    let mut types = HashMap::new();
    for schema in schemas {
        if let Some(ns) = schema.namespace() {
            types
                .entry(schema.logical_name().to_string())
                .or_insert_with(|| ns.to_string());
            if let Some(Value::Object(defs)) = schema.value().get("definitions") {
                for key in defs.keys() {
                    types.entry(key.clone()).or_insert_with(|| ns.to_string());
                }
            }
            if let Some(Value::Object(defs)) = schema.value().get("$defs") {
                for key in defs.keys() {
                    types.entry(key.clone()).or_insert_with(|| ns.to_string());
                }
            }
        }
    }
    types
}

fn namespace_for_definition<'a>(
    name: &str,
    types: &'a HashMap<String, String>,
) -> Option<&'a String> {
    if let Some(ns) = types.get(name) {
        return Some(ns);
    }
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != name {
        return types.get(trimmed);
    }
    None
}

fn variant_definition_name(base: &str, variant: &Value) -> Option<String> {
    if let Some(props) = variant.get("properties").and_then(Value::as_object) {
        if let Some(method_literal) = literal_from_property(props, "method") {
            let pascal = to_pascal_case(method_literal);
            return Some(match base {
                "ClientRequest" | "ServerRequest" => format!("{pascal}Request"),
                "ClientNotification" | "ServerNotification" => format!("{pascal}Notification"),
                _ => format!("{pascal}{base}"),
            });
        }

        if let Some(type_literal) = literal_from_property(props, "type") {
            let pascal = to_pascal_case(type_literal);
            return Some(match base {
                "EventMsg" => format!("{pascal}EventMsg"),
                _ => format!("{pascal}{base}"),
            });
        }

        if props.len() == 1
            && let Some(key) = props.keys().next()
        {
            let pascal = to_pascal_case(key);
            return Some(format!("{pascal}{base}"));
        }
    }

    if let Some(required) = variant.get("required").and_then(Value::as_array)
        && required.len() == 1
        && let Some(key) = required[0].as_str()
    {
        let pascal = to_pascal_case(key);
        return Some(format!("{pascal}{base}"));
    }

    None
}

fn literal_from_property<'a>(props: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    props.get(key).and_then(string_literal)
}

fn string_literal(value: &Value) -> Option<&str> {
    value.get("const").and_then(Value::as_str).or_else(|| {
        value
            .get("enum")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(Value::as_str)
    })
}

fn annotate_schema(value: &mut Value, base: Option<&str>) {
    match value {
        Value::Object(map) => annotate_object(map, base),
        Value::Array(items) => {
            for item in items {
                annotate_schema(item, base);
            }
        }
        _ => {}
    }
}

fn annotate_object(map: &mut Map<String, Value>, base: Option<&str>) {
    let owner = map.get("title").and_then(Value::as_str).map(str::to_owned);
    if let Some(owner) = owner.as_deref()
        && let Some(Value::Object(props)) = map.get_mut("properties")
    {
        set_discriminator_titles(props, owner);
    }

    if let Some(Value::Array(variants)) = map.get_mut("oneOf") {
        annotate_variant_list(variants, base);
    }
    if let Some(Value::Array(variants)) = map.get_mut("anyOf") {
        annotate_variant_list(variants, base);
    }

    if let Some(Value::Object(defs)) = map.get_mut("definitions") {
        for (name, schema) in defs.iter_mut() {
            annotate_schema(schema, Some(name.as_str()));
        }
    }

    if let Some(Value::Object(defs)) = map.get_mut("$defs") {
        for (name, schema) in defs.iter_mut() {
            annotate_schema(schema, Some(name.as_str()));
        }
    }

    if let Some(Value::Object(props)) = map.get_mut("properties") {
        for value in props.values_mut() {
            annotate_schema(value, base);
        }
    }

    if let Some(items) = map.get_mut("items") {
        annotate_schema(items, base);
    }

    if let Some(additional) = map.get_mut("additionalProperties") {
        annotate_schema(additional, base);
    }

    for (key, child) in map.iter_mut() {
        match key.as_str() {
            "oneOf"
            | "anyOf"
            | "definitions"
            | "$defs"
            | "properties"
            | "items"
            | "additionalProperties" => {}
            _ => annotate_schema(child, base),
        }
    }
}

fn annotate_variant_list(variants: &mut [Value], base: Option<&str>) {
    let mut seen = HashSet::new();

    for variant in variants.iter() {
        if let Some(name) = variant_title(variant) {
            seen.insert(name.to_owned());
        }
    }

    for variant in variants.iter_mut() {
        let mut variant_name = variant_title(variant).map(str::to_owned);

        if variant_name.is_none()
            && let Some(base_name) = base
            && let Some(name) = variant_definition_name(base_name, variant)
        {
            let mut candidate = name.clone();
            let mut index = 2;
            while seen.contains(&candidate) {
                candidate = format!("{name}{index}");
                index += 1;
            }
            if let Some(obj) = variant.as_object_mut() {
                obj.insert("title".into(), Value::String(candidate.clone()));
            }
            seen.insert(candidate.clone());
            variant_name = Some(candidate);
        }

        if let Some(name) = variant_name.as_deref()
            && let Some(obj) = variant.as_object_mut()
            && let Some(Value::Object(props)) = obj.get_mut("properties")
        {
            set_discriminator_titles(props, name);
        }

        annotate_schema(variant, base);
    }
}

const DISCRIMINATOR_KEYS: &[&str] = &["type", "method", "mode", "status", "role", "reason"];

fn set_discriminator_titles(props: &mut Map<String, Value>, owner: &str) {
    for key in DISCRIMINATOR_KEYS {
        if let Some(prop_schema) = props.get_mut(*key)
            && string_literal(prop_schema).is_some()
            && let Value::Object(prop_obj) = prop_schema
        {
            if prop_obj.contains_key("title") {
                continue;
            }
            let suffix = to_pascal_case(key);
            prop_obj.insert("title".into(), Value::String(format!("{owner}{suffix}")));
        }
    }
}

fn variant_title(value: &Value) -> Option<&str> {
    value
        .as_object()
        .and_then(|obj| obj.get("title"))
        .and_then(Value::as_str)
}

fn to_pascal_case(input: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in input.chars() {
        if c == '_' || c == '-' {
            capitalize_next = true;
            continue;
        }

        if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

fn ensure_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create output directory {}", dir.display()))
}

fn rewrite_named_ref_to_namespace(value: &mut Value, ns: &str, name: &str) {
    let direct = format!("#/definitions/{name}");
    let prefixed = format!("{direct}/");
    let replacement = format!("#/definitions/{ns}/{name}");
    let replacement_prefixed = format!("{replacement}/");
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(reference)) = obj.get_mut("$ref") {
                if reference == &direct {
                    *reference = replacement;
                } else if let Some(rest) = reference.strip_prefix(&prefixed) {
                    *reference = format!("{replacement_prefixed}{rest}");
                }
            }
            for child in obj.values_mut() {
                rewrite_named_ref_to_namespace(child, ns, name);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_named_ref_to_namespace(child, ns, name);
            }
        }
        _ => {}
    }
}

fn prepend_header_if_missing(path: &Path) -> Result<()> {
    let mut content = String::new();
    {
        let mut f = fs::File::open(path)
            .with_context(|| format!("Failed to open {} for reading", path.display()))?;
        f.read_to_string(&mut content)
            .with_context(|| format!("Failed to read {}", path.display()))?;
    }

    if content.starts_with(HEADER) {
        return Ok(());
    }

    let mut f = fs::File::create(path)
        .with_context(|| format!("Failed to open {} for writing", path.display()))?;
    f.write_all(HEADER.as_bytes())
        .with_context(|| format!("Failed to write header to {}", path.display()))?;
    f.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write content to {}", path.display()))?;
    Ok(())
}

fn ts_files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("Failed to read dir {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension() == Some(OsStr::new("ts")) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn ts_files_in_recursive(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in
            fs::read_dir(&d).with_context(|| format!("Failed to read dir {}", d.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() && path.extension() == Some(OsStr::new("ts")) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Generate an index.ts file that re-exports all generated types.
/// This allows consumers to import all types from a single file.
fn generate_index_ts(out_dir: &Path) -> Result<PathBuf> {
    let mut entries: Vec<String> = Vec::new();
    let mut stems: Vec<String> = ts_files_in(out_dir)?
        .into_iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            if stem == "index" { None } else { Some(stem) }
        })
        .collect();
    stems.sort();
    stems.dedup();

    for name in stems {
        entries.push(format!("export type {{ {name} }} from \"./{name}\";\n"));
    }

    // If this is the root out_dir and a ./v2 folder exists with TS files,
    // expose it as a namespace to avoid symbol collisions at the root.
    let v2_dir = out_dir.join("v2");
    let has_v2_ts = ts_files_in(&v2_dir).map(|v| !v.is_empty()).unwrap_or(false);
    if has_v2_ts {
        entries.push("export * as v2 from \"./v2\";\n".to_string());
    }

    let mut content =
        String::with_capacity(HEADER.len() + entries.iter().map(String::len).sum::<usize>());
    content.push_str(HEADER);
    for line in &entries {
        content.push_str(line);
    }

    let index_path = out_dir.join("index.ts");
    let mut f = fs::File::create(&index_path)
        .with_context(|| format!("Failed to create {}", index_path.display()))?;
    f.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write {}", index_path.display()))?;
    Ok(index_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::v2;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn generated_ts_optional_nullable_fields_only_in_params() -> Result<()> {
        // Assert that "?: T | null" only appears in generated *Params types.
        let output_dir = std::env::temp_dir().join(format!("codex_ts_types_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;

        struct TempDirGuard(PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let _guard = TempDirGuard(output_dir.clone());

        // Avoid doing more work than necessary to keep the test from timing out.
        let options = GenerateTsOptions {
            generate_indices: false,
            ensure_headers: false,
            run_prettier: false,
            experimental_api: false,
        };
        generate_ts_with_options(&output_dir, None, options)?;

        let client_request_ts = fs::read_to_string(output_dir.join("ClientRequest.ts"))?;
        assert_eq!(client_request_ts.contains("mock/experimentalMethod"), false);
        assert_eq!(
            client_request_ts.contains("MockExperimentalMethodParams"),
            false
        );
        let thread_start_ts =
            fs::read_to_string(output_dir.join("v2").join("ThreadStartParams.ts"))?;
        assert_eq!(thread_start_ts.contains("mockExperimentalField"), false);
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodParams.ts")
                .exists(),
            false
        );
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodResponse.ts")
                .exists(),
            false
        );

        let mut undefined_offenders = Vec::new();
        let mut optional_nullable_offenders = BTreeSet::new();
        let mut stack = vec![output_dir];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }

                if matches!(path.extension().and_then(|ext| ext.to_str()), Some("ts")) {
                    // Only allow "?: T | null" in objects representing JSON-RPC requests,
                    // which we assume are called "*Params".
                    let allow_optional_nullable = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| {
                            stem.ends_with("Params")
                                || stem == "InitializeCapabilities"
                                || matches!(
                                    stem,
                                    "CollabAgentRef"
                                        | "CollabAgentStatusEntry"
                                        | "CollabAgentSpawnEndEvent"
                                        | "CollabAgentInteractionEndEvent"
                                        | "CollabCloseEndEvent"
                                        | "CollabResumeBeginEvent"
                                        | "CollabResumeEndEvent"
                                )
                        });

                    let contents = fs::read_to_string(&path)?;
                    if contents.contains("| undefined") {
                        undefined_offenders.push(path.clone());
                    }

                    const SKIP_PREFIXES: &[&str] = &[
                        "const ",
                        "let ",
                        "var ",
                        "export const ",
                        "export let ",
                        "export var ",
                    ];

                    let mut search_start = 0;
                    while let Some(idx) = contents[search_start..].find("| null") {
                        let abs_idx = search_start + idx;
                        // Find the property-colon for this field by scanning forward
                        // from the start of the segment and ignoring nested braces,
                        // brackets, and parens. This avoids colons inside nested
                        // type literals like `{ [k in string]?: string }`.

                        let line_start_idx =
                            contents[..abs_idx].rfind('\n').map(|i| i + 1).unwrap_or(0);

                        let mut segment_start_idx = line_start_idx;
                        if let Some(rel_idx) = contents[line_start_idx..abs_idx].rfind(',') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }
                        if let Some(rel_idx) = contents[line_start_idx..abs_idx].rfind('{') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }
                        if let Some(rel_idx) = contents[line_start_idx..abs_idx].rfind('}') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }

                        // Scan forward for the colon that separates the field name from its type.
                        let mut level_brace = 0_i32;
                        let mut level_brack = 0_i32;
                        let mut level_paren = 0_i32;
                        let mut in_single = false;
                        let mut in_double = false;
                        let mut escape = false;
                        let mut prop_colon_idx = None;
                        for (i, ch) in contents[segment_start_idx..abs_idx].char_indices() {
                            let idx_abs = segment_start_idx + i;
                            if escape {
                                escape = false;
                                continue;
                            }
                            match ch {
                                '\\' => {
                                    // Only treat as escape when inside a string.
                                    if in_single || in_double {
                                        escape = true;
                                    }
                                }
                                '\'' => {
                                    if !in_double {
                                        in_single = !in_single;
                                    }
                                }
                                '"' => {
                                    if !in_single {
                                        in_double = !in_double;
                                    }
                                }
                                '{' if !in_single && !in_double => level_brace += 1,
                                '}' if !in_single && !in_double => level_brace -= 1,
                                '[' if !in_single && !in_double => level_brack += 1,
                                ']' if !in_single && !in_double => level_brack -= 1,
                                '(' if !in_single && !in_double => level_paren += 1,
                                ')' if !in_single && !in_double => level_paren -= 1,
                                ':' if !in_single
                                    && !in_double
                                    && level_brace == 0
                                    && level_brack == 0
                                    && level_paren == 0 =>
                                {
                                    prop_colon_idx = Some(idx_abs);
                                    break;
                                }
                                _ => {}
                            }
                        }

                        let Some(colon_idx) = prop_colon_idx else {
                            search_start = abs_idx + 5;
                            continue;
                        };

                        let mut field_prefix = contents[segment_start_idx..colon_idx].trim();
                        if field_prefix.is_empty() {
                            search_start = abs_idx + 5;
                            continue;
                        }

                        if let Some(comment_idx) = field_prefix.rfind("*/") {
                            field_prefix = field_prefix[comment_idx + 2..].trim_start();
                        }

                        if field_prefix.is_empty() {
                            search_start = abs_idx + 5;
                            continue;
                        }

                        if SKIP_PREFIXES
                            .iter()
                            .any(|prefix| field_prefix.starts_with(prefix))
                        {
                            search_start = abs_idx + 5;
                            continue;
                        }

                        if field_prefix.contains('(') {
                            search_start = abs_idx + 5;
                            continue;
                        }

                        // If the last non-whitespace before ':' is '?', then this is an
                        // optional field with a nullable type (i.e., "?: T | null").
                        // These are only allowed in *Params types.
                        if field_prefix.chars().rev().find(|c| !c.is_whitespace()) == Some('?')
                            && !allow_optional_nullable
                        {
                            let line_number =
                                contents[..abs_idx].chars().filter(|c| *c == '\n').count() + 1;
                            let offending_line_end = contents[line_start_idx..]
                                .find('\n')
                                .map(|i| line_start_idx + i)
                                .unwrap_or(contents.len());
                            let offending_snippet =
                                contents[line_start_idx..offending_line_end].trim();

                            optional_nullable_offenders.insert(format!(
                                "{}:{}: {offending_snippet}",
                                path.display(),
                                line_number
                            ));
                        }

                        search_start = abs_idx + 5;
                    }
                }
            }
        }

        assert!(
            undefined_offenders.is_empty(),
            "Generated TypeScript still includes unions with `undefined` in {undefined_offenders:?}"
        );

        // If this assertion fails, it means a field was generated as "?: T | null",
        // which is both optional (undefined) and nullable (null), for a type not ending
        // in "Params" (which represent JSON-RPC requests).
        assert!(
            optional_nullable_offenders.is_empty(),
            "Generated TypeScript has optional nullable fields outside *Params types (disallowed '?: T | null'):\n{optional_nullable_offenders:?}"
        );

        Ok(())
    }

    #[test]
    fn generate_ts_with_experimental_api_retains_experimental_entries() -> Result<()> {
        let output_dir =
            std::env::temp_dir().join(format!("codex_ts_types_experimental_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;

        struct TempDirGuard(PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let _guard = TempDirGuard(output_dir.clone());

        let options = GenerateTsOptions {
            generate_indices: false,
            ensure_headers: false,
            run_prettier: false,
            experimental_api: true,
        };
        generate_ts_with_options(&output_dir, None, options)?;

        let client_request_ts = fs::read_to_string(output_dir.join("ClientRequest.ts"))?;
        assert_eq!(client_request_ts.contains("mock/experimentalMethod"), true);
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodParams.ts")
                .exists(),
            true
        );
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodResponse.ts")
                .exists(),
            true
        );

        let thread_start_ts =
            fs::read_to_string(output_dir.join("v2").join("ThreadStartParams.ts"))?;
        assert_eq!(thread_start_ts.contains("mockExperimentalField"), true);

        Ok(())
    }

    #[test]
    fn stable_schema_filter_removes_mock_thread_start_field() -> Result<()> {
        let output_dir = std::env::temp_dir().join(format!("codex_schema_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;
        let schema = write_json_schema_with_return::<v2::ThreadStartParams>(
            &output_dir,
            "ThreadStartParams",
        )?;
        let mut bundle = build_schema_bundle(vec![schema])?;
        filter_experimental_schema(&mut bundle)?;

        let definitions = bundle["definitions"]
            .as_object()
            .expect("schema bundle should include definitions");
        let (_, def_schema) = definitions
            .iter()
            .find(|(name, _)| definition_matches_type(name, "ThreadStartParams"))
            .expect("ThreadStartParams definition should exist");
        let properties = def_schema["properties"]
            .as_object()
            .expect("ThreadStartParams should have properties");
        assert_eq!(properties.contains_key("mockExperimentalField"), false);
        let _cleanup = fs::remove_dir_all(&output_dir);
        Ok(())
    }

    #[test]
    fn experimental_type_fields_ts_filter_handles_interface_shape() -> Result<()> {
        let output_dir = std::env::temp_dir().join(format!("codex_ts_filter_{}", Uuid::now_v7()));
        fs::create_dir_all(&output_dir)?;

        struct TempDirGuard(PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let _guard = TempDirGuard(output_dir.clone());
        let path = output_dir.join("CustomParams.ts");
        let content = r#"export interface CustomParams {
  stableField: string | null;
  unstableField: string | null;
  otherStableField: boolean;
}
"#;
        fs::write(&path, content)?;

        static CUSTOM_FIELD: crate::experimental_api::ExperimentalField =
            crate::experimental_api::ExperimentalField {
                type_name: "CustomParams",
                field_name: "unstableField",
                reason: "custom/unstableField",
            };
        filter_experimental_type_fields_ts(&output_dir, &[&CUSTOM_FIELD])?;

        let filtered = fs::read_to_string(&path)?;
        assert_eq!(filtered.contains("unstableField"), false);
        assert_eq!(filtered.contains("stableField"), true);
        assert_eq!(filtered.contains("otherStableField"), true);
        Ok(())
    }

    #[test]
    fn experimental_type_fields_ts_filter_keeps_imports_used_in_intersection_suffix() -> Result<()>
    {
        let output_dir = std::env::temp_dir().join(format!("codex_ts_filter_{}", Uuid::now_v7()));
        fs::create_dir_all(&output_dir)?;

        struct TempDirGuard(PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let _guard = TempDirGuard(output_dir.clone());
        let path = output_dir.join("Config.ts");
        let content = r#"import type { JsonValue } from "../serde_json/JsonValue";
import type { Keep } from "./Keep";

export type Config = { stableField: Keep, unstableField: string | null } & ({ [key in string]?: number | string | boolean | Array<JsonValue> | { [key in string]?: JsonValue } | null });
"#;
        fs::write(&path, content)?;

        static CUSTOM_FIELD: crate::experimental_api::ExperimentalField =
            crate::experimental_api::ExperimentalField {
                type_name: "Config",
                field_name: "unstableField",
                reason: "custom/unstableField",
            };
        filter_experimental_type_fields_ts(&output_dir, &[&CUSTOM_FIELD])?;

        let filtered = fs::read_to_string(&path)?;
        assert_eq!(filtered.contains("unstableField"), false);
        assert_eq!(
            filtered.contains(r#"import type { JsonValue } from "../serde_json/JsonValue";"#),
            true
        );
        assert_eq!(
            filtered.contains(r#"import type { Keep } from "./Keep";"#),
            true
        );
        Ok(())
    }

    #[test]
    fn stable_schema_filter_removes_mock_experimental_method() -> Result<()> {
        let output_dir = std::env::temp_dir().join(format!("codex_schema_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;
        let schema =
            write_json_schema_with_return::<crate::ClientRequest>(&output_dir, "ClientRequest")?;
        let mut bundle = build_schema_bundle(vec![schema])?;
        filter_experimental_schema(&mut bundle)?;

        let bundle_str = serde_json::to_string(&bundle)?;
        assert_eq!(bundle_str.contains("mock/experimentalMethod"), false);
        let _cleanup = fs::remove_dir_all(&output_dir);
        Ok(())
    }

    #[test]
    fn generate_json_filters_experimental_fields_and_methods() -> Result<()> {
        let output_dir = std::env::temp_dir().join(format!("codex_schema_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;
        generate_json_with_experimental(&output_dir, false)?;

        let thread_start_json =
            fs::read_to_string(output_dir.join("v2").join("ThreadStartParams.json"))?;
        assert_eq!(thread_start_json.contains("mockExperimentalField"), false);

        let client_request_json = fs::read_to_string(output_dir.join("ClientRequest.json"))?;
        assert_eq!(
            client_request_json.contains("mock/experimentalMethod"),
            false
        );

        let bundle_json =
            fs::read_to_string(output_dir.join("codex_app_server_protocol.schemas.json"))?;
        assert_eq!(bundle_json.contains("mockExperimentalField"), false);
        assert_eq!(bundle_json.contains("MockExperimentalMethodParams"), false);
        assert_eq!(
            bundle_json.contains("MockExperimentalMethodResponse"),
            false
        );
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodParams.json")
                .exists(),
            false
        );
        assert_eq!(
            output_dir
                .join("v2")
                .join("MockExperimentalMethodResponse.json")
                .exists(),
            false
        );

        let _cleanup = fs::remove_dir_all(&output_dir);
        Ok(())
    }
}
