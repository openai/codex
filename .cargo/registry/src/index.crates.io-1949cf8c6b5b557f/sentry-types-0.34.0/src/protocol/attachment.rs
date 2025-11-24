use std::fmt;

use serde::Deserialize;

/// The different types an attachment can have.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize)]
pub enum AttachmentType {
    #[serde(rename = "event.attachment")]
    /// (default) A standard attachment without special meaning.
    Attachment,
    /// A minidump file that creates an error event and is symbolicated. The
    /// file should start with the `MDMP` magic bytes.
    #[serde(rename = "event.minidump")]
    Minidump,
    /// An Apple crash report file that creates an error event and is symbolicated.
    #[serde(rename = "event.applecrashreport")]
    AppleCrashReport,
    /// An XML file containing UE4 crash meta data. During event ingestion,
    /// event contexts and extra fields are extracted from this file.
    #[serde(rename = "unreal.context")]
    UnrealContext,
    /// A plain-text log file obtained from UE4 crashes. During event ingestion,
    /// the last logs are extracted into event breadcrumbs.
    #[serde(rename = "unreal.logs")]
    UnrealLogs,
}

impl Default for AttachmentType {
    fn default() -> Self {
        Self::Attachment
    }
}

impl AttachmentType {
    /// Gets the string value Sentry expects for the attachment type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Attachment => "event.attachment",
            Self::Minidump => "event.minidump",
            Self::AppleCrashReport => "event.applecrashreport",
            Self::UnrealContext => "unreal.context",
            Self::UnrealLogs => "unreal.logs",
        }
    }
}

#[derive(Clone, PartialEq, Default)]
/// Represents an attachment item.
pub struct Attachment {
    /// The actual attachment data.
    pub buffer: Vec<u8>,
    /// The filename of the attachment.
    pub filename: String,
    /// The Content Type of the attachment
    pub content_type: Option<String>,
    /// The special type of this attachment.
    pub ty: Option<AttachmentType>,
}

impl Attachment {
    /// Writes the attachment and its headers to the provided `Writer`.
    pub fn to_writer<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        writeln!(
            writer,
            r#"{{"type":"attachment","length":{length},"filename":"{filename}","attachment_type":"{at}","content_type":"{ct}"}}"#,
            filename = self.filename,
            length = self.buffer.len(),
            at = self.ty.unwrap_or_default().as_str(),
            ct = self
                .content_type
                .as_ref()
                .unwrap_or(&"application/octet-stream".to_string())
        )?;

        writer.write_all(&self.buffer)?;
        Ok(())
    }
}

// Implement Debug manually, otherwise users will be sad when they get a dump
// of decimal encoded bytes to their console
impl fmt::Debug for Attachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attachment")
            .field("buffer", &self.buffer.len())
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("type", &self.ty)
            .finish()
    }
}
