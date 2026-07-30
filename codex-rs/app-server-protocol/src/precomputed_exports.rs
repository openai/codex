use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Cursor;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub(crate) const GENERATED_TS_HEADER: &str = "// GENERATED CODE! DO NOT MODIFY BY HAND!\n\n";
const STABLE_EXPORTS: &[u8] =
    include_bytes!("../schema/precomputed/app-server-exports-stable.json.zst");
const EXPERIMENTAL_EXPORTS: &[u8] =
    include_bytes!("../schema/precomputed/app-server-exports-experimental.json.zst");

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

#[derive(Deserialize)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq, serde::Serialize))]
pub(crate) struct PrecomputedExports {
    pub(crate) typescript: BTreeMap<String, String>,
    pub(crate) json_schema: BTreeMap<String, String>,
    pub(crate) internal_json_schema: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
enum ExportSet {
    Stable,
    Experimental,
}

pub fn generate_types(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts(out_dir, prettier)?;
    generate_json(out_dir)
}

pub fn generate_ts(out_dir: &Path, prettier: Option<&Path>) -> Result<()> {
    generate_ts_with_options(out_dir, prettier, GenerateTsOptions::default())
}

pub fn generate_ts_with_options(
    out_dir: &Path,
    prettier: Option<&Path>,
    options: GenerateTsOptions,
) -> Result<()> {
    let export_set = if options.experimental_api {
        ExportSet::Experimental
    } else {
        ExportSet::Stable
    };
    let exports = load_exports(export_set)?;
    let ts_files = write_typescript_exports(out_dir, &exports.typescript, options)?;

    if options.run_prettier
        && let Some(prettier_bin) = prettier
        && !ts_files.is_empty()
    {
        let status = Command::new(prettier_bin)
            .arg("--write")
            .arg("--log-level")
            .arg("warn")
            .args(ts_files.iter().map(PathBuf::as_path))
            .status()
            .with_context(|| format!("Failed to invoke Prettier at {}", prettier_bin.display()))?;
        if !status.success() {
            bail!("Prettier failed with status {status}");
        }
    }

    trim_trailing_whitespace_in_ts_files(&ts_files)
}

pub fn generate_json(out_dir: &Path) -> Result<()> {
    generate_json_with_experimental(out_dir, /*experimental_api*/ false)
}

pub fn generate_json_with_experimental(out_dir: &Path, experimental_api: bool) -> Result<()> {
    let export_set = if experimental_api {
        ExportSet::Experimental
    } else {
        ExportSet::Stable
    };
    let exports = load_exports(export_set)?;
    write_exports(out_dir, &exports.json_schema)?;
    Ok(())
}

pub fn generate_internal_json_schema(out_dir: &Path) -> Result<()> {
    let exports = load_exports(ExportSet::Stable)?;
    write_exports(out_dir, &exports.internal_json_schema)?;
    Ok(())
}

fn load_exports(export_set: ExportSet) -> Result<PrecomputedExports> {
    let compressed = match export_set {
        ExportSet::Stable => STABLE_EXPORTS,
        ExportSet::Experimental => EXPERIMENTAL_EXPORTS,
    };
    let bytes = zstd::stream::decode_all(Cursor::new(compressed))
        .context("decompress precomputed app-server protocol exports")?;
    serde_json::from_slice(&bytes).context("decode precomputed app-server protocol exports")
}

fn write_typescript_exports(
    out_dir: &Path,
    exports: &BTreeMap<String, String>,
    options: GenerateTsOptions,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (relative_path, archived_contents) in exports {
        let is_index = Path::new(relative_path).file_name() == Some(OsStr::new("index.ts"));
        if is_index && !options.generate_indices {
            continue;
        }

        let contents = if options.ensure_headers || is_index {
            archived_contents.as_str()
        } else {
            archived_contents
                .strip_prefix(GENERATED_TS_HEADER)
                .unwrap_or(archived_contents)
        };
        written.push(write_export(out_dir, relative_path, contents)?);
    }
    Ok(written)
}

fn write_exports(out_dir: &Path, exports: &BTreeMap<String, String>) -> Result<Vec<PathBuf>> {
    exports
        .iter()
        .map(|(relative_path, contents)| write_export(out_dir, relative_path, contents))
        .collect()
}

fn write_export(out_dir: &Path, relative_path: &str, contents: &str) -> Result<PathBuf> {
    let relative_path = validated_relative_path(relative_path)?;
    let output_path = out_dir.join(relative_path);
    let parent = output_path
        .parent()
        .context("precomputed export path should have a parent")?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    fs::write(&output_path, contents)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;
    Ok(output_path)
}

fn validated_relative_path(path: &str) -> Result<&Path> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("invalid precomputed export path: {}", path.display());
    }
    Ok(path)
}

fn trim_trailing_whitespace_in_ts_files(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let trimmed = trim_trailing_line_whitespace(&content);
        if trimmed != content {
            fs::write(path, trimmed)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

fn trim_trailing_line_whitespace(content: &str) -> String {
    let mut trimmed = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        if let Some(line_without_newline) = line.strip_suffix('\n') {
            trimmed.push_str(line_without_newline.trim_end_matches([' ', '\t']));
            trimmed.push('\n');
        } else {
            trimmed.push_str(line.trim_end_matches([' ', '\t']));
        }
    }
    trimmed
}

#[cfg(test)]
pub(crate) fn decode_precomputed_exports(experimental_api: bool) -> Result<PrecomputedExports> {
    let export_set = if experimental_api {
        ExportSet::Experimental
    } else {
        ExportSet::Stable
    };
    load_exports(export_set)
}
