use crate::ClientNotification;
use crate::ClientRequest;
use crate::ServerNotification;
use crate::ServerNotificationEnvelope;
use crate::ServerRequest;
use crate::TS;
use crate::export::GENERATED_TS_HEADER;
use crate::export::filter_experimental_ts_tree;
use crate::export::generate_index_ts_tree;
use crate::export::trim_trailing_line_whitespace;
use crate::precomputed_exports::PrecomputedExports;
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
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
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
    collect_typescript_fixture_file::<ServerNotificationEnvelope>(&mut files, &mut seen)?;

    filter_experimental_ts_tree(&mut files)?;
    generate_index_ts_tree(&mut files);
    for content in files.values_mut() {
        *content = trim_trailing_line_whitespace(content);
    }

    Ok(files
        .into_iter()
        .map(|(path, content)| (path, content.into_bytes()))
        .collect())
}

/// Regenerates `schema/typescript/`, `schema/json/`, and the stable embedded exports.
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
    if options.experimental_api {
        return write_experimental_precomputed_exports(schema_root, prettier);
    }

    let typescript_out_dir = schema_root.join("typescript");
    let json_out_dir = schema_root.join("json");

    ensure_empty_dir(&typescript_out_dir)?;
    ensure_empty_dir(&json_out_dir)?;

    crate::export::generate_ts_with_options(
        &typescript_out_dir,
        prettier,
        crate::export::GenerateTsOptions::default(),
    )?;
    crate::export::generate_json(&json_out_dir)?;

    let internal_dir = tempfile::tempdir().context("create internal schema temp dir")?;
    crate::export::generate_internal_json_schema(internal_dir.path())?;
    let exports = PrecomputedExports {
        typescript: collect_export_files_recursive(&typescript_out_dir)?,
        json_schema: collect_export_files_recursive(&json_out_dir)?,
        internal_json_schema: collect_export_files_recursive(internal_dir.path())?,
    };
    write_precomputed_exports(schema_root, "stable", &exports)?;

    Ok(())
}

fn write_experimental_precomputed_exports(
    schema_root: &Path,
    prettier: Option<&Path>,
) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create experimental schema temp dir")?;
    let typescript_out_dir = temp_dir.path().join("typescript");
    let json_out_dir = temp_dir.path().join("json");

    crate::export::generate_ts_with_options(
        &typescript_out_dir,
        prettier,
        crate::export::GenerateTsOptions {
            experimental_api: true,
            ..crate::export::GenerateTsOptions::default()
        },
    )?;
    crate::export::generate_json_with_experimental(&json_out_dir, /*experimental_api*/ true)?;

    let exports = PrecomputedExports {
        typescript: collect_export_files_recursive(&typescript_out_dir)?,
        json_schema: collect_export_files_recursive(&json_out_dir)?,
        internal_json_schema: BTreeMap::new(),
    };
    write_precomputed_exports(schema_root, "experimental", &exports)
}

fn write_precomputed_exports(
    schema_root: &Path,
    name: &str,
    exports: &PrecomputedExports,
) -> Result<()> {
    let output_dir = schema_root.join("precomputed");
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("create {}", output_dir.display()))?;
    let output_path = output_dir.join(format!("app-server-exports-{name}.json.zst"));
    let json = serde_json::to_vec(exports).context("serialize precomputed protocol exports")?;
    let compressed = zstd::stream::encode_all(Cursor::new(json), 19)
        .context("compress precomputed protocol exports")?;
    std::fs::write(&output_path, compressed)
        .with_context(|| format!("write {}", output_path.display()))
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
            entries.sort_by_key(|(key, _)| *key);
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
    files_recursive(root)?
        .into_iter()
        .map(|(relative_path, path)| Ok((relative_path, read_file_bytes(&path)?)))
        .collect()
}

pub(crate) fn collect_export_files_recursive(root: &Path) -> Result<BTreeMap<String, String>> {
    files_recursive(root)?
        .into_iter()
        .map(|(relative_path, path)| {
            let relative_path = relative_path
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("read UTF-8 export {}", path.display()))?
                .replace("\r\n", "\n");
            Ok((relative_path, contents))
        })
        .collect()
}

fn files_recursive(root: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut files = Vec::new();
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

            files.push((rel, path));
        }
    }

    files.sort_by(|(left, _), (right, _)| left.cmp(right));
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
