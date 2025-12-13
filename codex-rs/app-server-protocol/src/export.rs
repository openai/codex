use crate::ClientNotification;
use crate::ClientRequest;
use crate::ServerNotification;
use crate::ServerRequest;
use crate::export_client_notification_schemas;
use crate::export_client_param_schemas;
use crate::export_client_response_schemas;
use crate::export_client_responses;
use crate::export_server_notification_schemas;
use crate::export_server_param_schemas;
use crate::export_server_response_schemas;
use crate::export_server_responses;
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
}

impl Default for GenerateTsOptions {
    fn default() -> Self {
        Self {
            generate_indices: true,
            ensure_headers: true,
            run_prettier: true,
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
    ensure_dir(out_dir)?;
    let envelope_emitters: &[JsonSchemaEmitter] = &[
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
    for emit in envelope_emitters {
        schemas.push(emit(out_dir)?);
    }

    schemas.extend(export_client_param_schemas(out_dir)?);
    schemas.extend(export_client_response_schemas(out_dir)?);
    schemas.extend(export_server_param_schemas(out_dir)?);
    schemas.extend(export_server_response_schemas(out_dir)?);
    schemas.extend(export_client_notification_schemas(out_dir)?);
    schemas.extend(export_server_notification_schemas(out_dir)?);

    let bundle = build_schema_bundle(schemas)?;
    write_pretty_json(
        out_dir.join("codex_app_server_protocol.schemas.json"),
        &bundle,
    )?;

    Ok(())
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
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn generated_ts_has_no_optional_nullable_fields() -> Result<()> {
        // Assert that there are no types of the form "?: T | null" in the generated TS files.
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
        };
        generate_ts_with_options(&output_dir, None, options)?;

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
                        // optional field with a nullable type (i.e., "?: T | null"),
                        // which we explicitly disallow.
                        if field_prefix.chars().rev().find(|c| !c.is_whitespace()) == Some('?') {
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

        // If this assertion fails, it means a field was generated as
        // "?: T | null" — i.e., both optional (undefined) and nullable (null).
        // We only want either "?: T" or ": T | null".
        assert!(
            optional_nullable_offenders.is_empty(),
            "Generated TypeScript has optional fields with nullable types (disallowed '?: T | null'), add #[ts(optional)] to fix:\n{optional_nullable_offenders:?}"
        );

        Ok(())
    }
}
