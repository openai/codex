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
use ts_rs::ExportError;
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

fn export_ts_with_context<F>(label: &str, export: F) -> Result<()>
where
    F: FnOnce() -> std::result::Result<(), ExportError>,
{
    match export() {
        Ok(()) => Ok(()),
        Err(ExportError::CannotBeExported(ty)) => Err(anyhow!(
            "failed to export {label}: dependency {ty} cannot be exported"
        )),
        Err(err) => Err(err.into()),
    }
}

pub fn generate_types(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts(out_dir, prettier)?;
    generate_json(out_dir)?;
    Ok(())
}

pub fn generate_ts(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    ensure_dir(out_dir)?;

    export_ts_with_context("ClientRequest", || ClientRequest::export_all_to(out_dir))?;
    export_ts_with_context("client responses", || export_client_responses(out_dir))?;
    export_ts_with_context("ClientNotification", || {
        ClientNotification::export_all_to(out_dir)
    })?;

    export_ts_with_context("ServerRequest", || ServerRequest::export_all_to(out_dir))?;
    export_ts_with_context("server responses", || export_server_responses(out_dir))?;
    export_ts_with_context("ServerNotification", || {
        ServerNotification::export_all_to(out_dir)
    })?;

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
