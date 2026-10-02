//! Writes selected rollouts to a bounded gzip tar at a caller-provided local path.
//! Invalid inputs are logged and skipped; only a complete archive is published.
//! Archive failures fall back to lazy individual attachments using the original sources.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;

use crate::FeedbackAttachment;
use crate::FeedbackAttachmentPath;

pub(super) fn archive_rollouts<'a>(
    output_path: &Path,
    rollout_paths: impl IntoIterator<Item = &'a FeedbackAttachmentPath>,
    rollouts: impl IntoIterator<Item = &'a FeedbackAttachment>,
    max_bytes: usize,
) -> Result<Option<FeedbackAttachmentPath>> {
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).context("failed to create rollout archive directory")?;
    let file = tempfile::NamedTempFile::new_in(parent)?;
    let writer = BoundedWriter {
        inner: BufWriter::new(file),
        written: 0,
        max_bytes,
    };
    let mut archive = tar::Builder::new(GzEncoder::new(writer, Compression::default()));
    let mut filenames = HashSet::new();
    for path in rollout_paths {
        match path.read_attachment(max_bytes) {
            Ok(Some(rollout)) => append_rollout(&mut archive, &mut filenames, &rollout, max_bytes)?,
            Ok(None) => {} // The attachment reader already logs oversized or nonregular files.
            Err(error) => tracing::error!(%error, "failed to read rollout; skipping attachment"),
        }
    }
    for rollout in rollouts {
        append_rollout(&mut archive, &mut filenames, rollout, max_bytes)?;
    }
    if filenames.is_empty() {
        return Ok(None);
    }
    let writer = archive
        .into_inner()?
        .finish()
        .context("failed to finish rollout archive")?;
    let file = writer.inner.into_inner()?;
    // Keep failed writes and incomplete gzip streams out of the destination path.
    file.persist_noclobber(output_path)?;
    Ok(Some(FeedbackAttachmentPath {
        path: output_path.to_path_buf(),
        attachment_filename_override: None,
    }))
}

pub(super) fn with_individual_fallback<'a>(
    archive: Result<Option<FeedbackAttachment>>,
    rollout_paths: Vec<&'a FeedbackAttachmentPath>,
    rollouts: Vec<FeedbackAttachment>,
    max_bytes: usize,
) -> impl Iterator<Item = Result<FeedbackAttachment>> + 'a {
    let (archive, fallback) = match archive {
        Ok(archive) => (archive, false),
        Err(error) => {
            tracing::error!(%error, "rollout archiving failed; uploading individual attachments");
            (None, true)
        }
    };
    archive
        .into_iter()
        .map(Ok)
        .chain(fallback.then_some(rollouts).into_iter().flatten().map(Ok))
        .chain(
            fallback
                .then_some(rollout_paths)
                .into_iter()
                .flatten()
                .map(move |path| {
                    path.read_attachment(max_bytes)?.context(
                        "feedback attachment is not a regular file or exceeds the size limit",
                    )
                }),
        )
}

fn append_rollout<W: Write>(
    archive: &mut tar::Builder<W>,
    filenames: &mut HashSet<String>,
    rollout: &FeedbackAttachment,
    max_bytes: usize,
) -> Result<()> {
    if rollout.buffer.len() > max_bytes {
        tracing::error!(
            bytes = rollout.buffer.len(),
            max_bytes,
            "rollout skipped: size limit exceeded"
        );
        return Ok(());
    }
    if rollout.filename.is_empty()
        || matches!(rollout.filename.as_str(), "." | "..")
        || rollout.filename.contains(['/', '\\', ':'])
        || filenames.contains(&rollout.filename)
    {
        tracing::error!("rollout skipped: invalid or duplicate archive filename");
        return Ok(());
    }
    let mut header = tar::Header::new_gnu();
    header.set_size(rollout.buffer.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(0);
    header.set_entry_type(tar::EntryType::Regular);
    archive
        .append_data(&mut header, &rollout.filename, rollout.buffer.as_slice())
        .context("failed to write rollout archive")?;
    filenames.insert(rollout.filename.clone());
    Ok(())
}

struct BoundedWriter<W> {
    inner: W,
    written: usize,
    max_bytes: usize,
}

impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.max_bytes.saturating_sub(self.written) {
            return Err(io::Error::other("rollout archive exceeds the size limit"));
        }
        let written = self.inner.write(buffer)?;
        self.written += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
