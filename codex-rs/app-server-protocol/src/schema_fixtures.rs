use crate::ClientNotification;
use crate::ClientRequest;
use crate::ServerNotification;
use crate::ServerRequest;
use crate::export::GENERATED_TS_HEADER;
use crate::export::filter_experimental_ts_tree;
use crate::export::generate_index_ts_tree;
use crate::protocol::common::visit_client_response_types;
use crate::protocol::common::visit_server_response_types;
use anyhow::Context;
use anyhow::Result;
use serde_json::Map;
use serde_json::Value;
use std::any::TypeId;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use ts_rs::TS;
use ts_rs::TypeVisitor;

#[derive(Clone, Copy, Debug, Default)]
pub struct SchemaFixtureOptions {
    pub experimental_api: bool,
}

pub fn read_schema_fixture_tree(schema_root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let typescript_root = schema_root.join("typescript");
    let json_root = schema_root.join("json");

    let mut all = BTreeMap::new();
    for (rel, bytes) in collect_files_recursive(&typescript_root)? {
        all.insert(PathBuf::from("typescript").join(rel), bytes);
    }
    for (rel, bytes) in collect_files_recursive(&json_root)? {
        all.insert(PathBuf::from("json").join(rel), bytes);
    }

    Ok(all)
}

pub fn read_schema_fixture_subtree(
    schema_root: &Path,
    label: &str,
) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let subtree_root = schema_root.join(label);
    collect_files_recursive(&subtree_root)
        .with_context(|| format!("read schema fixture subtree {}", subtree_root.display()))
}

#[doc(hidden)]
pub fn generate_typescript_schema_fixture_subtree_for_tests() -> Result<BTreeMap<PathBuf, Vec<u8>>>
{
    let files = generate_typescript_schema_fixture_subtree(SchemaFixtureOptions::default())?;

    Ok(files
        .into_iter()
        .map(|(path, content)| (path, content.into_bytes()))
        .collect())
}

fn generate_typescript_schema_fixture_subtree(
    options: SchemaFixtureOptions,
) -> Result<BTreeMap<PathBuf, String>> {
    let mut files = BTreeMap::new();
    let mut seen = HashSet::new();

    collect_typescript_fixture_file::<ClientRequest>(&mut files, &mut seen)?;
    visit_typescript_fixture_dependencies(&mut files, &mut seen, |visitor| {
        visit_client_response_types(visitor);
    })?;
    collect_typescript_fixture_file::<ClientNotification>(&mut files, &mut seen)?;
    collect_typescript_fixture_file::<ServerRequest>(&mut files, &mut seen)?;
    visit_typescript_fixture_dependencies(&mut files, &mut seen, |visitor| {
        visit_server_response_types(visitor);
    })?;
    collect_typescript_fixture_file::<ServerNotification>(&mut files, &mut seen)?;

    if !options.experimental_api {
        filter_experimental_ts_tree(&mut files)?;
    }
    generate_index_ts_tree(&mut files);

    Ok(files)
}

/// Regenerates `schema/typescript/` and `schema/json/`.
///
/// This is intended to be used by tooling (e.g., `just write-app-server-schema`).
/// It deletes any previously generated files so stale artifacts are removed.
pub fn write_schema_fixtures(schema_root: &Path, prettier: Option<&Path>) -> Result<()> {
    write_schema_fixtures_with_options(schema_root, prettier, SchemaFixtureOptions::default())
}

/// Regenerates schema fixtures with configurable options.
pub fn write_schema_fixtures_with_options(
    schema_root: &Path,
    prettier: Option<&Path>,
    options: SchemaFixtureOptions,
) -> Result<()> {
    let typescript_out_dir = schema_root.join("typescript");
    let json_out_dir = schema_root.join("json");

    ensure_empty_dir(&typescript_out_dir)?;
    ensure_empty_dir(&json_out_dir)?;

    write_typescript_schema_fixture_subtree(
        &typescript_out_dir,
        &generate_typescript_schema_fixture_subtree(options)?,
        prettier,
    )?;
    crate::generate_json_with_experimental(&json_out_dir, options.experimental_api)?;

    Ok(())
}

fn write_typescript_schema_fixture_subtree(
    output_dir: &Path,
    files: &BTreeMap<PathBuf, String>,
    prettier: Option<&Path>,
) -> Result<()> {
    let mut ts_files = Vec::with_capacity(files.len());
    for (relative_path, content) in files {
        let output_path = output_dir.join(relative_path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let output = format!("{GENERATED_TS_HEADER}{content}");
        std::fs::write(&output_path, output)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        ts_files.push(output_path);
    }

    if let Some(prettier_bin) = prettier
        && !ts_files.is_empty()
    {
        let status = Command::new(prettier_bin)
            .arg("--write")
            .arg("--log-level")
            .arg("warn")
            .args(ts_files.iter().map(|path| path.as_os_str()))
            .status()
            .with_context(|| format!("failed to invoke Prettier at {}", prettier_bin.display()))?;
        if !status.success() {
            anyhow::bail!("Prettier failed with status {status}");
        }
    }

    Ok(())
}

fn ensure_empty_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("failed to remove {}", dir.display()))?;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    Ok(())
}

fn read_file_bytes(path: &Path) -> Result<Vec<u8>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if path.extension().is_some_and(|ext| ext == "json") {
        let value: Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse JSON in {}", path.display()))?;
        let value = canonicalize_json(&value);
        let normalized = serde_json::to_vec_pretty(&value)
            .with_context(|| format!("failed to reserialize JSON in {}", path.display()))?;
        return Ok(normalized);
    }
    if path.extension().is_some_and(|ext| ext == "ts") {
        // Windows checkouts (and some generators) may produce CRLF; normalize so the
        // fixture test is platform-independent.
        let text = String::from_utf8(bytes)
            .with_context(|| format!("expected UTF-8 TypeScript in {}", path.display()))?;
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        // Fixture comparisons care about schema content, not whether the generator
        // re-prepended the standard banner to every TypeScript file.
        let text = text
            .strip_prefix(GENERATED_TS_HEADER)
            .unwrap_or(&text)
            .to_string();
        return Ok(text.into_bytes());
    }
    Ok(bytes)
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            // NOTE: We sort some JSON arrays to make schema fixture comparisons stable across
            // platforms.
            //
            // In general, JSON array ordering is significant. However, this code path is used
            // only by `schema_fixtures_match_generated` to compare our *vendored* JSON schema
            // files against freshly generated output. Some parts of schema generation end up
            // with non-deterministic ordering across platforms (often due to map iteration order
            // upstream), which can cause Windows CI failures even when the generated schema is
            // semantically equivalent.
            //
            // JSON Schema itself also contains a number of array-valued keywords whose ordering
            // does not affect validation semantics (e.g. `required`, `type`, `enum`, `anyOf`,
            // `oneOf`, `allOf`). That makes it reasonable to treat many schema-emitted arrays as
            // order-insensitive for the purpose of fixture diffs.
            //
            // To avoid accidentally changing the meaning of arrays where order *could* matter
            // (e.g. tuple validation / `prefixItems`-style arrays), we only sort arrays when we
            // can derive a stable sort key for *every* element. If we cannot, we preserve the
            // original ordering.
            let items = items.iter().map(canonicalize_json).collect::<Vec<_>>();
            let mut sortable = Vec::with_capacity(items.len());
            for item in &items {
                let Some(key) = schema_array_item_sort_key(item) else {
                    return Value::Array(items);
                };
                let stable = serde_json::to_string(item).unwrap_or_default();
                sortable.push((key, stable));
            }

            let mut items = items.into_iter().zip(sortable).collect::<Vec<_>>();

            items.sort_by(
                |(_, (key_left, stable_left)), (_, (key_right, stable_right))| match key_left
                    .cmp(key_right)
                {
                    Ordering::Equal => stable_left.cmp(stable_right),
                    other => other,
                },
            );

            Value::Array(items.into_iter().map(|(item, _)| item).collect())
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = Map::with_capacity(map.len());
            for (key, child) in entries {
                sorted.insert(key.clone(), canonicalize_json(child));
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

fn schema_array_item_sort_key(item: &Value) -> Option<String> {
    match item {
        Value::Null => Some("null".to_string()),
        Value::Bool(b) => Some(format!("b:{b}")),
        Value::Number(n) => Some(format!("n:{n}")),
        Value::String(s) => Some(format!("s:{s}")),
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get("$ref") {
                Some(format!("ref:{reference}"))
            } else if let Some(Value::String(title)) = map.get("title") {
                Some(format!("title:{title}"))
            } else {
                None
            }
        }
        Value::Array(_) => None,
    }
}

fn collect_files_recursive(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("failed to read dir {}", dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("failed to read dir entry in {}", dir.display()))?;
            let path = entry.path();
            // On some platforms, Bazel runfiles are symlinks. `DirEntry::file_type()` does not
            // follow symlinks, so use `metadata()` here to treat symlinks as the files/dirs they
            // point to.
            let metadata = std::fs::metadata(&path)
                .with_context(|| format!("failed to stat {}", path.display()))?;
            if metadata.is_dir() {
                stack.push(path);
                continue;
            } else if !metadata.is_file() {
                continue;
            }

            let rel = path
                .strip_prefix(root)
                .with_context(|| {
                    format!(
                        "failed to strip prefix {} from {}",
                        root.display(),
                        path.display()
                    )
                })?
                .to_path_buf();

            files.insert(rel, read_file_bytes(&path)?);
        }
    }

    Ok(files)
}

fn collect_typescript_fixture_file<T: TS + 'static + ?Sized>(
    files: &mut BTreeMap<PathBuf, String>,
    seen: &mut HashSet<TypeId>,
) -> Result<()> {
    let Some(output_path) = T::output_path() else {
        return Ok(());
    };
    if !seen.insert(TypeId::of::<T>()) {
        return Ok(());
    }

    let contents = T::export_to_string().context("export TypeScript fixture content")?;
    let output_path = normalize_relative_fixture_path(&output_path);
    files.insert(
        output_path,
        contents.replace("\r\n", "\n").replace('\r', "\n"),
    );

    let mut visitor = TypeScriptFixtureCollector {
        files,
        seen,
        error: None,
    };
    T::visit_dependencies(&mut visitor);
    if let Some(error) = visitor.error {
        return Err(error);
    }

    Ok(())
}

fn normalize_relative_fixture_path(path: &Path) -> PathBuf {
    path.components().collect()
}

fn visit_typescript_fixture_dependencies(
    files: &mut BTreeMap<PathBuf, String>,
    seen: &mut HashSet<TypeId>,
    visit: impl FnOnce(&mut TypeScriptFixtureCollector<'_>),
) -> Result<()> {
    let mut visitor = TypeScriptFixtureCollector {
        files,
        seen,
        error: None,
    };
    visit(&mut visitor);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    Ok(())
}

struct TypeScriptFixtureCollector<'a> {
    files: &'a mut BTreeMap<PathBuf, String>,
    seen: &'a mut HashSet<TypeId>,
    error: Option<anyhow::Error>,
}

impl TypeVisitor for TypeScriptFixtureCollector<'_> {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if self.error.is_some() {
            return;
        }
        self.error = collect_typescript_fixture_file::<T>(self.files, self.seen).err();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn canonicalize_json_sorts_string_arrays() {
        let value = serde_json::json!(["b", "a"]);
        let expected = serde_json::json!(["a", "b"]);
        assert_eq!(canonicalize_json(&value), expected);
    }

    #[test]
    fn canonicalize_json_sorts_schema_ref_arrays() {
        let value = serde_json::json!([
            {"$ref": "#/definitions/B"},
            {"$ref": "#/definitions/A"}
        ]);
        let expected = serde_json::json!([
            {"$ref": "#/definitions/A"},
            {"$ref": "#/definitions/B"}
        ]);
        assert_eq!(canonicalize_json(&value), expected);
    }
}
