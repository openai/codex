use crate::ClientNotification;
use crate::ClientRequest;
use crate::ServerNotification;
use crate::ServerRequest;
use crate::export_client_response_schemas;
use crate::export_client_responses;
use crate::export_server_response_schemas;
use crate::export_server_responses;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use schemars::JsonSchema;
use schemars::schema::RootSchema;
use schemars::schema_for;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use ts_rs::TS;

const HEADER: &str = "// GENERATED CODE! DO NOT MODIFY BY HAND!\n\n";

macro_rules! for_each_schema_type {
    ($macro:ident) => {
        $macro!(crate::RequestId);
        $macro!(crate::JSONRPCMessage);
        $macro!(crate::JSONRPCRequest);
        $macro!(crate::JSONRPCNotification);
        $macro!(crate::JSONRPCResponse);
        $macro!(crate::JSONRPCError);
        $macro!(crate::JSONRPCErrorError);
        $macro!(crate::AddConversationListenerParams);
        $macro!(crate::AddConversationSubscriptionResponse);
        $macro!(crate::ApplyPatchApprovalParams);
        $macro!(crate::ApplyPatchApprovalResponse);
        $macro!(crate::ArchiveConversationParams);
        $macro!(crate::ArchiveConversationResponse);
        $macro!(crate::AuthMode);
        $macro!(crate::AuthStatusChangeNotification);
        $macro!(crate::CancelLoginChatGptParams);
        $macro!(crate::CancelLoginChatGptResponse);
        $macro!(crate::ClientInfo);
        $macro!(crate::ClientNotification);
        $macro!(crate::ClientRequest);
        $macro!(crate::ConversationSummary);
        $macro!(crate::ExecCommandApprovalParams);
        $macro!(crate::ExecCommandApprovalResponse);
        $macro!(crate::ExecOneOffCommandParams);
        $macro!(crate::ExecOneOffCommandResponse);
        $macro!(crate::FuzzyFileSearchParams);
        $macro!(crate::FuzzyFileSearchResponse);
        $macro!(crate::FuzzyFileSearchResult);
        $macro!(crate::GetAuthStatusParams);
        $macro!(crate::GetAuthStatusResponse);
        $macro!(crate::GetUserAgentResponse);
        $macro!(crate::GetUserSavedConfigResponse);
        $macro!(crate::GitDiffToRemoteParams);
        $macro!(crate::GitDiffToRemoteResponse);
        $macro!(crate::GitSha);
        $macro!(crate::InitializeParams);
        $macro!(crate::InitializeResponse);
        $macro!(crate::InputItem);
        $macro!(crate::InterruptConversationParams);
        $macro!(crate::InterruptConversationResponse);
        $macro!(crate::ListConversationsParams);
        $macro!(crate::ListConversationsResponse);
        $macro!(crate::LoginApiKeyParams);
        $macro!(crate::LoginApiKeyResponse);
        $macro!(crate::LoginChatGptCompleteNotification);
        $macro!(crate::LoginChatGptResponse);
        $macro!(crate::LogoutChatGptParams);
        $macro!(crate::LogoutChatGptResponse);
        $macro!(crate::NewConversationParams);
        $macro!(crate::NewConversationResponse);
        $macro!(crate::Profile);
        $macro!(crate::RemoveConversationListenerParams);
        $macro!(crate::RemoveConversationSubscriptionResponse);
        $macro!(crate::ResumeConversationParams);
        $macro!(crate::ResumeConversationResponse);
        $macro!(crate::SandboxSettings);
        $macro!(crate::SendUserMessageParams);
        $macro!(crate::SendUserMessageResponse);
        $macro!(crate::SendUserTurnParams);
        $macro!(crate::SendUserTurnResponse);
        $macro!(crate::ServerNotification);
        $macro!(crate::ServerRequest);
        $macro!(crate::SessionConfiguredNotification);
        $macro!(crate::SetDefaultModelParams);
        $macro!(crate::SetDefaultModelResponse);
        $macro!(crate::Tools);
        $macro!(crate::UserInfoResponse);
        $macro!(crate::UserSavedConfig);
        $macro!(codex_protocol::protocol::EventMsg);
        $macro!(codex_protocol::protocol::FileChange);
        $macro!(codex_protocol::parse_command::ParsedCommand);
        $macro!(codex_protocol::protocol::SandboxPolicy);
    };
}

pub fn generate_types(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts(out_dir, prettier)?;
    generate_json(out_dir)?;
    Ok(())
}

pub fn generate_ts(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    ensure_dir(out_dir)?;

    ClientRequest::export_all_to(out_dir)?;
    export_client_responses(out_dir)?;
    ClientNotification::export_all_to(out_dir)?;

    ServerRequest::export_all_to(out_dir)?;
    export_server_responses(out_dir)?;
    ServerNotification::export_all_to(out_dir)?;

    generate_index_ts(out_dir)?;

    let ts_files = ts_files_in(out_dir)?;
    for file in &ts_files {
        prepend_header_if_missing(file)?;
    }

    if let Some(prettier_bin) = prettier
        && !ts_files.is_empty()
    {
        let status = Command::new(prettier_bin)
            .arg("--write")
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
    let mut bundle: BTreeMap<String, RootSchema> = BTreeMap::new();

    macro_rules! add_schema {
        ($ty:path) => {{
            let name = type_basename(stringify!($ty));
            let schema = write_json_schema_with_return::<$ty>(out_dir, &name)?;
            bundle.insert(name, schema);
        }};
    }

    for_each_schema_type!(add_schema);

    export_client_response_schemas(out_dir)?;
    export_server_response_schemas(out_dir)?;

    let mut definitions = Map::new();

    const SPECIAL_DEFINITIONS: &[&str] = &[
        "ClientNotification",
        "ClientRequest",
        "EventMsg",
        "FileChange",
        "InputItem",
        "ParsedCommand",
        "SandboxPolicy",
        "ServerNotification",
        "ServerRequest",
    ];

    for (name, schema) in bundle {
        let mut schema_value = serde_json::to_value(schema)?;
        if let Value::Object(ref mut obj) = schema_value {
            if let Some(defs) = obj.remove("definitions")
                && let Value::Object(defs_obj) = defs
            {
                for (def_name, def_schema) in defs_obj {
                    if !SPECIAL_DEFINITIONS.contains(&def_name.as_str()) {
                        definitions.insert(def_name, def_schema);
                    }
                }
            }

            if let Some(Value::Array(one_of)) = obj.get_mut("oneOf") {
                for variant in one_of.iter_mut() {
                    if let Some(variant_name) = variant_definition_name(&name, variant)
                        && let Value::Object(variant_obj) = variant
                    {
                        variant_obj.insert("title".into(), Value::String(variant_name));
                    }
                }
            }
        }
        definitions.insert(name, schema_value);
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

    write_pretty_json(
        out_dir.join("codex_app_server_protocol.schemas.json"),
        &Value::Object(root),
    )?;

    Ok(())
}

fn write_json_schema_with_return<T>(out_dir: &Path, name: &str) -> Result<RootSchema>
where
    T: JsonSchema,
{
    let file_stem = name.trim();
    let schema = schema_for!(T);
    write_pretty_json(out_dir.join(format!("{file_stem}.json")), &schema)
        .with_context(|| format!("Failed to write JSON schema for {file_stem}"))?;
    Ok(schema)
}

pub(crate) fn write_json_schema<T>(out_dir: &Path, name: &str) -> Result<()>
where
    T: JsonSchema,
{
    write_json_schema_with_return::<T>(out_dir, name).map(|_| ())
}

fn write_pretty_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_vec_pretty(value)
        .with_context(|| format!("Failed to serialize JSON schema to {}", path.display()))?;
    fs::write(&path, json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}
fn type_basename(type_path: &str) -> String {
    type_path
        .rsplit_once("::")
        .map(|(_, name)| name)
        .unwrap_or(type_path)
        .trim()
        .to_string()
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

        if let Some(mode_literal) = literal_from_property(props, "mode") {
            let pascal = to_pascal_case(mode_literal);
            return Some(match base {
                "SandboxPolicy" => format!("{pascal}SandboxPolicy"),
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
    props
        .get(key)
        .and_then(|value| value.get("enum"))
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
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
    fn generated_ts_omits_undefined_unions_for_optionals() -> Result<()> {
        let output_dir = std::env::temp_dir().join(format!("codex_ts_types_{}", Uuid::now_v7()));
        fs::create_dir(&output_dir)?;

        struct TempDirGuard(PathBuf);

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let _guard = TempDirGuard(output_dir.clone());

        generate_ts(&output_dir, None)?;

        let mut undefined_offenders = Vec::new();
        let mut missing_optional_marker = BTreeSet::new();
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
                        let Some(colon_idx) = contents[..abs_idx].rfind(':') else {
                            search_start = abs_idx + 5;
                            continue;
                        };

                        let line_start_idx = contents[..colon_idx]
                            .rfind('\n')
                            .map(|i| i + 1)
                            .unwrap_or(0);

                        let mut segment_start_idx = line_start_idx;
                        if let Some(rel_idx) = contents[line_start_idx..colon_idx].rfind(',') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }
                        if let Some(rel_idx) = contents[line_start_idx..colon_idx].rfind('{') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }
                        if let Some(rel_idx) = contents[line_start_idx..colon_idx].rfind('}') {
                            segment_start_idx = segment_start_idx.max(line_start_idx + rel_idx + 1);
                        }

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

                        if field_prefix.chars().rev().find(|c| !c.is_whitespace()) == Some('?') {
                            search_start = abs_idx + 5;
                            continue;
                        }

                        let line_number =
                            contents[..abs_idx].chars().filter(|c| *c == '\n').count() + 1;
                        let offending_line_end = contents[line_start_idx..]
                            .find('\n')
                            .map(|i| line_start_idx + i)
                            .unwrap_or(contents.len());
                        let offending_snippet = contents[line_start_idx..offending_line_end].trim();

                        missing_optional_marker.insert(format!(
                            "{}:{}: {offending_snippet}",
                            path.display(),
                            line_number
                        ));

                        search_start = abs_idx + 5;
                    }
                }
            }
        }

        assert!(
            undefined_offenders.is_empty(),
            "Generated TypeScript still includes unions with `undefined` in {undefined_offenders:?}"
        );

        // If this test fails, it means that a struct field that is `Option<T>` in Rust
        // is being generated as `T | null` in TypeScript, without the optional marker
        // (`?`). To fix this, add #[ts(optional_fields = nullable)] to the struct definition.
        assert!(
            missing_optional_marker.is_empty(),
            "Generated TypeScript has nullable fields without an optional marker: {missing_optional_marker:?}"
        );

        Ok(())
    }
}
