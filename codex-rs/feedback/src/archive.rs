//! Atomic local feedback export with bounded contents and no file replacement.

use std::io;
use std::io::Write;
use std::path::Path;

use crate::FeedbackAttachment;
use crate::FeedbackAttachmentPath;
use anyhow::Context;
use anyhow::Result;

pub(crate) enum Input<'a> {
    Owned(FeedbackAttachment),
    Path(&'a FeedbackAttachmentPath),
}
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub(crate) struct Limits {
    pub max_bytes: usize,
    pub max_attachments: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 200 * 1024 * 1024,
            max_attachments: 128,
        }
    }
}

// Count uncompressed entry contents, including report and manifest serialization.
struct BudgetWriter<'a, W> {
    inner: &'a mut W,
    remaining: &'a mut usize,
}

impl<W: Write> Write for BudgetWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.remaining.checked_sub(buffer.len()).ok_or_else(|| {
            io::Error::other("feedback archive exceeds the uncompressed size limit")
        })?;
        self.inner.write_all(buffer)?;
        *self.remaining = remaining;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn save<'a>(
    path: &Path,
    report: &[u8],
    attachments: impl Iterator<Item = Input<'a>>,
    limits: Limits,
) -> Result<()> {
    anyhow::ensure!(path.is_absolute(), "feedback save path must be absolute");
    anyhow::ensure!(!path.exists(), "feedback save path already exists");
    let parent = path.parent().context("feedback save path has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer = ZipWriter::new(temporary.as_file_mut());
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    let mut remaining = limits.max_bytes;
    writer.start_file("report.json", options)?;
    BudgetWriter {
        inner: &mut writer,
        remaining: &mut remaining,
    }
    .write_all(report)?;
    let mut manifest = Vec::new();
    for (index, attachment) in attachments.enumerate() {
        // Inspect metadata before reading the next file or compressing its contents.
        anyhow::ensure!(
            index < limits.max_attachments,
            "feedback archive exceeds the attachment count limit"
        );
        let attachment = match attachment {
            Input::Owned(attachment) => attachment,
            Input::Path(path) => path.read_attachment(remaining)
                .with_context(|| format!("failed to read selected attachment {}", path.path.display()))?
                .with_context(|| format!("selected attachment {} is missing, nonregular, or exceeds the remaining decoded size limit", path.path.display()))?,
        };
        let basename = attachment
            .filename
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty() && *name != "." && *name != "..")
            .unwrap_or("attachment");
        // Numbered directories retain duplicate names without unsafe ZIP paths.
        let entry = format!("attachments/{index:04}/{basename}");
        writer.start_file(&entry, options)?;
        BudgetWriter {
            inner: &mut writer,
            remaining: &mut remaining,
        }
        .write_all(&attachment.buffer)?;
        manifest.push(serde_json::json!({
            "entry": entry,
            "filename": attachment.filename,
            "contentType": attachment.content_type,
        }));
    }
    writer.start_file("manifest.json", options)?;
    serde_json::to_writer(
        BudgetWriter {
            inner: &mut writer,
            remaining: &mut remaining,
        },
        &manifest,
    )?;
    writer.finish()?;
    temporary.as_file().sync_all()?;
    // Same-directory publication is atomic and must not replace an existing file.
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
