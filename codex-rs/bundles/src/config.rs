use crate::runtime::RuntimePaths;
use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

const STARTUP_TIMEOUT_SEC: i64 = 120;

pub async fn write_codex_runtime_config(codex_home: &Path, paths: &RuntimePaths) -> Result<()> {
    let config_path = codex_home.join(codex_config::CONFIG_TOML_FILE);
    let mut doc = match fs::read_to_string(&config_path).await {
        Ok(raw) => raw
            .parse::<DocumentMut>()
            .with_context(|| format!("failed to parse {}", config_path.display()))?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", config_path.display()));
        }
    };

    set_path(
        &mut doc,
        &["js_repl_node_path"],
        value(paths.node_path.to_string_lossy().to_string()),
    );
    set_path(
        &mut doc,
        &["js_repl_node_module_dirs"],
        string_array([paths.node_modules_path.to_string_lossy().to_string()]),
    );

    let mut node_repl = TomlTable::new();
    node_repl["command"] = value(paths.node_repl_path.to_string_lossy().to_string());
    node_repl["args"] = TomlItem::Value(toml_edit::Array::new().into());
    node_repl["startup_timeout_sec"] = value(STARTUP_TIMEOUT_SEC);
    let mut env = TomlTable::new();
    env["NODE_REPL_NODE_MODULE_DIRS"] =
        value(paths.node_modules_path.to_string_lossy().to_string());
    env["NODE_REPL_NODE_PATH"] = value(paths.node_path.to_string_lossy().to_string());
    env["ARTIFACT_TOOL_PYTHON"] = value(paths.python_path.to_string_lossy().to_string());
    node_repl["env"] = TomlItem::Table(env);
    set_path(
        &mut doc,
        &["mcp_servers", "node_repl"],
        TomlItem::Table(node_repl),
    );

    fs::create_dir_all(codex_home)
        .await
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    fs::write(&config_path, doc.to_string())
        .await
        .with_context(|| format!("failed to write {}", config_path.display()))
}

pub fn codex_home() -> Result<PathBuf> {
    codex_utils_home_dir::find_codex_home().map_err(anyhow::Error::from)
}

fn set_path(doc: &mut DocumentMut, segments: &[&str], value: TomlItem) {
    let mut table = doc.as_table_mut();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        if !table.get(segment).is_some_and(TomlItem::is_table) {
            let mut new_table = TomlTable::new();
            new_table.set_implicit(true);
            table.insert(segment, TomlItem::Table(new_table));
        }
        table = match table.get_mut(segment).and_then(TomlItem::as_table_mut) {
            Some(table) => table,
            None => unreachable!("table was just inserted"),
        };
    }
    if let Some(last) = segments.last() {
        table.insert(last, value);
    }
}

fn string_array(values: impl IntoIterator<Item = String>) -> TomlItem {
    let mut array = toml_edit::Array::new();
    for item in values {
        array.push(item);
    }
    TomlItem::Value(array.into())
}
