use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) const EXTERNAL_OUTPUT_THRESHOLD_BYTES: usize = 4 * 1024 * 1024;
const BLOB_SUBDIR: &str = "mongodb_blobs";
const OUTPUT_JSON_PATH: &str = "/payload/output";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ExternalItemField {
    pub(super) json_path: String,
    pub(super) file_name: String,
    pub(super) byte_len: usize,
    pub(super) sha256: String,
}

pub(super) fn externalize_rollout_item(
    codex_home: &Path,
    item: &mut RolloutItem,
) -> ThreadStoreResult<Vec<ExternalItemField>> {
    let Some(output) = text_output_mut(item) else {
        return Ok(Vec::new());
    };
    if output.len() <= EXTERNAL_OUTPUT_THRESHOLD_BYTES {
        return Ok(Vec::new());
    }

    let raw_output = std::mem::take(output);
    let sha256 = sha256_hex(raw_output.as_bytes());
    let file_name = format!("sha256-{sha256}.raw");
    write_blob(codex_home, &file_name, raw_output.as_bytes())?;
    Ok(vec![ExternalItemField {
        json_path: OUTPUT_JSON_PATH.to_string(),
        file_name,
        byte_len: raw_output.len(),
        sha256,
    }])
}

pub(super) fn hydrate_rollout_item(
    codex_home: &Path,
    item: &mut RolloutItem,
    external_fields: &[ExternalItemField],
) -> ThreadStoreResult<()> {
    for field in external_fields {
        if field.json_path != OUTPUT_JSON_PATH {
            return Err(invalid_blob(format!(
                "unsupported external rollout item path {}",
                field.json_path
            )));
        }
        let output = text_output_mut(item).ok_or_else(|| {
            invalid_blob(format!(
                "external field {} does not refer to a text tool output",
                field.json_path
            ))
        })?;
        let bytes = read_blob(codex_home, field)?;
        *output = String::from_utf8(bytes)
            .map_err(|err| invalid_blob(format!("blob {} is not UTF-8: {err}", field.file_name)))?;
    }
    Ok(())
}

pub(super) fn remove_blob(codex_home: &Path, file_name: &str) -> ThreadStoreResult<()> {
    validate_file_name(file_name)?;
    let path = blob_dir(codex_home).join(file_name);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(blob_io_error(&path, err)),
    }
}

pub(super) fn clear_blob_dir(codex_home: &Path) -> ThreadStoreResult<()> {
    let path = blob_dir(codex_home);
    match std::fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(blob_io_error(&path, err)),
    }
}

fn text_output_mut(item: &mut RolloutItem) -> Option<&mut String> {
    let RolloutItem::ResponseItem(
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. },
    ) = item
    else {
        return None;
    };
    let FunctionCallOutputBody::Text(output) = &mut output.body else {
        return None;
    };
    Some(output)
}

fn write_blob(codex_home: &Path, file_name: &str, bytes: &[u8]) -> ThreadStoreResult<()> {
    validate_file_name(file_name)?;
    let dir = blob_dir(codex_home);
    std::fs::create_dir_all(&dir).map_err(|err| blob_io_error(&dir, err))?;
    let final_path = dir.join(file_name);
    if final_path
        .metadata()
        .is_ok_and(|metadata| metadata.len() == bytes.len() as u64)
    {
        return Ok(());
    }

    for attempt in 0..100 {
        let temp_path = dir.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            attempt
        ));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(blob_io_error(&temp_path, err)),
        };
        file.write_all(bytes)
            .map_err(|err| blob_io_error(&temp_path, err))?;
        file.sync_all()
            .map_err(|err| blob_io_error(&temp_path, err))?;
        drop(file);
        match std::fs::rename(&temp_path, &final_path) {
            Ok(()) => return Ok(()),
            Err(err) if final_path.exists() => {
                std::fs::remove_file(&temp_path)
                    .map_err(|remove_err| blob_io_error(&temp_path, remove_err))?;
                if final_path
                    .metadata()
                    .is_ok_and(|metadata| metadata.len() == bytes.len() as u64)
                {
                    return Ok(());
                }
                return Err(blob_io_error(&final_path, err));
            }
            Err(err) => return Err(blob_io_error(&final_path, err)),
        }
    }
    Err(invalid_blob(format!(
        "could not allocate a temporary file for {file_name}"
    )))
}

fn read_blob(codex_home: &Path, field: &ExternalItemField) -> ThreadStoreResult<Vec<u8>> {
    validate_file_name(&field.file_name)?;
    let path = blob_dir(codex_home).join(&field.file_name);
    let bytes = std::fs::read(&path).map_err(|err| blob_io_error(&path, err))?;
    if bytes.len() != field.byte_len {
        return Err(invalid_blob(format!(
            "blob {} has {} bytes, expected {}",
            field.file_name,
            bytes.len(),
            field.byte_len
        )));
    }
    let actual_sha256 = sha256_hex(&bytes);
    if actual_sha256 != field.sha256 {
        return Err(invalid_blob(format!(
            "blob {} has SHA-256 {actual_sha256}, expected {}",
            field.file_name, field.sha256
        )));
    }
    Ok(bytes)
}

fn validate_file_name(file_name: &str) -> ThreadStoreResult<()> {
    let path = Path::new(file_name);
    if file_name.is_empty()
        || path.components().count() != 1
        || path.file_name().and_then(|name| name.to_str()) != Some(file_name)
    {
        return Err(invalid_blob(format!(
            "invalid blob file name {file_name:?}"
        )));
    }
    Ok(())
}

fn blob_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(BLOB_SUBDIR)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn blob_io_error(path: &Path, err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!(
            "Mongo thread store blob failure at {}: {err}",
            path.display()
        ),
    }
}

fn invalid_blob(message: String) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("Mongo thread store blob failure: {message}"),
    }
}
