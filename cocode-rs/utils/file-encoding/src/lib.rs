//! File encoding and line ending detection and preservation utilities.
//!
//! This crate provides utilities to detect and preserve file encodings (UTF-8, UTF-16LE, UTF-16BE)
//! and line endings (LF, CRLF, CR) when reading and writing files.
//!
//! # Example
//!
//! ```no_run
//! use cocode_file_encoding::{detect_encoding, detect_line_ending, write_with_format, Encoding, LineEnding};
//! use std::path::Path;
//!
//! // Detect encoding from raw bytes
//! let bytes = std::fs::read("file.txt").unwrap();
//! let encoding = detect_encoding(&bytes);
//!
//! // Decode content
//! let content = encoding.decode(&bytes).unwrap();
//!
//! // Detect line ending from content
//! let line_ending = detect_line_ending(&content);
//!
//! // Write back preserving format
//! write_with_format(Path::new("file.txt"), &content, encoding, line_ending).unwrap();
//! ```

use std::io;
use std::path::Path;

/// File encoding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Encoding {
    /// UTF-8 encoding (default).
    #[default]
    Utf8,
    /// UTF-8 encoding with BOM (EF BB BF).
    Utf8WithBom,
    /// UTF-16 Little Endian encoding.
    Utf16Le,
    /// UTF-16 Big Endian encoding.
    Utf16Be,
}

impl Encoding {
    /// Decode bytes to string using this encoding.
    pub fn decode(&self, bytes: &[u8]) -> Result<String, EncodingError> {
        match self {
            Encoding::Utf8 | Encoding::Utf8WithBom => {
                // Skip BOM if present
                let content = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    &bytes[3..]
                } else {
                    bytes
                };
                String::from_utf8(content.to_vec())
                    .map_err(|e| EncodingError::InvalidUtf8(e.to_string()))
            }
            Encoding::Utf16Le => {
                // Skip BOM if present
                let content = if bytes.starts_with(&[0xFF, 0xFE]) {
                    &bytes[2..]
                } else {
                    bytes
                };
                decode_utf16le(content)
            }
            Encoding::Utf16Be => {
                // Skip BOM if present
                let content = if bytes.starts_with(&[0xFE, 0xFF]) {
                    &bytes[2..]
                } else {
                    bytes
                };
                decode_utf16be(content)
            }
        }
    }

    /// Encode string to bytes using this encoding.
    pub fn encode(&self, content: &str) -> Vec<u8> {
        match self {
            Encoding::Utf8 | Encoding::Utf8WithBom => content.as_bytes().to_vec(),
            Encoding::Utf16Le => encode_utf16le(content),
            Encoding::Utf16Be => encode_utf16be(content),
        }
    }

    /// Returns whether this encoding should include a BOM when writing.
    /// UTF-16 files typically include BOM for proper detection.
    /// UTF-8 with BOM preserves the original BOM.
    pub fn bom(&self) -> &'static [u8] {
        match self {
            Encoding::Utf8 => &[],
            Encoding::Utf8WithBom => &[0xEF, 0xBB, 0xBF],
            Encoding::Utf16Le => &[0xFF, 0xFE],
            Encoding::Utf16Be => &[0xFE, 0xFF],
        }
    }
}

/// Line ending type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// Unix-style line feed (LF, \n) - default.
    #[default]
    Lf,
    /// Windows-style carriage return + line feed (CRLF, \r\n).
    CrLf,
    /// Classic Mac-style carriage return (CR, \r).
    Cr,
}

impl LineEnding {
    /// Returns the string representation of this line ending.
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }
}

/// Encoding-related errors.
#[derive(Debug)]
pub enum EncodingError {
    /// Invalid UTF-8 sequence.
    InvalidUtf8(String),
    /// Invalid UTF-16 sequence.
    InvalidUtf16(String),
    /// I/O error.
    Io(io::Error),
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingError::InvalidUtf8(msg) => write!(f, "Invalid UTF-8: {msg}"),
            EncodingError::InvalidUtf16(msg) => write!(f, "Invalid UTF-16: {msg}"),
            EncodingError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for EncodingError {}

impl From<io::Error> for EncodingError {
    fn from(e: io::Error) -> Self {
        EncodingError::Io(e)
    }
}

/// Detect file encoding from raw bytes by checking for BOM.
///
/// Detection priority:
/// 1. UTF-16LE BOM (FF FE)
/// 2. UTF-16BE BOM (FE FF)
/// 3. UTF-8 BOM (EF BB BF) - returns Utf8WithBom to preserve BOM
/// 4. Default to UTF-8 (no BOM)
pub fn detect_encoding(bytes: &[u8]) -> Encoding {
    if bytes.len() >= 2 {
        // Check UTF-16 BOMs first
        if bytes.starts_with(&[0xFF, 0xFE]) {
            return Encoding::Utf16Le;
        }
        if bytes.starts_with(&[0xFE, 0xFF]) {
            return Encoding::Utf16Be;
        }
    }
    // Check UTF-8 BOM - preserve it by returning Utf8WithBom
    if bytes.len() >= 3 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Encoding::Utf8WithBom;
    }
    // Default to UTF-8 (no BOM)
    Encoding::Utf8
}

/// Detect line ending style from string content.
///
/// Uses simple heuristic aligned with Claude Code: if content contains CRLF,
/// treat as CRLF; otherwise LF. This works for 99% of cases.
pub fn detect_line_ending(content: &str) -> LineEnding {
    if content.contains("\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

/// Check if content has a trailing newline.
pub fn has_trailing_newline(content: &str) -> bool {
    content.ends_with('\n')
}

/// Preserve trailing newline state from original content.
///
/// If original had a trailing newline and modified doesn't, add one.
/// If original didn't have a trailing newline and modified does, remove it.
/// This prevents spurious diffs from trailing newline changes.
pub fn preserve_trailing_newline(original: &str, modified: &str) -> String {
    let had_trailing = original.ends_with('\n');
    let has_trailing = modified.ends_with('\n');

    match (had_trailing, has_trailing) {
        (true, false) => format!("{modified}\n"),
        (false, true) => modified.trim_end_matches('\n').to_string(),
        _ => modified.to_string(),
    }
}

/// Normalize line endings in content to the specified format.
///
/// Converts all line endings (CRLF, CR, LF) to the target format.
pub fn normalize_line_endings(content: &str, target: LineEnding) -> String {
    // First normalize everything to LF
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");

    // Then convert to target
    match target {
        LineEnding::Lf => normalized,
        LineEnding::CrLf => normalized.replace('\n', "\r\n"),
        LineEnding::Cr => normalized.replace('\n', "\r"),
    }
}

/// Read a file and detect its encoding and line ending.
///
/// Returns the decoded content, detected encoding, and detected line ending.
pub fn read_with_format(path: &Path) -> Result<(String, Encoding, LineEnding), EncodingError> {
    let bytes = std::fs::read(path)?;
    let encoding = detect_encoding(&bytes);
    let content = encoding.decode(&bytes)?;
    let line_ending = detect_line_ending(&content);
    Ok((content, encoding, line_ending))
}

/// Write content to a file with the specified encoding and line ending.
///
/// Normalizes line endings to the target format before writing.
pub fn write_with_format(
    path: &Path,
    content: &str,
    encoding: Encoding,
    line_ending: LineEnding,
) -> Result<(), EncodingError> {
    // Normalize line endings
    let normalized = normalize_line_endings(content, line_ending);

    // Encode content
    let mut bytes = encoding.bom().to_vec();
    bytes.extend(encoding.encode(&normalized));

    std::fs::write(path, bytes)?;
    Ok(())
}

/// Async version: Read a file and detect its encoding and line ending.
pub async fn read_with_format_async(
    path: &Path,
) -> Result<(String, Encoding, LineEnding), EncodingError> {
    let bytes = tokio::fs::read(path).await?;
    let encoding = detect_encoding(&bytes);
    let content = encoding.decode(&bytes)?;
    let line_ending = detect_line_ending(&content);
    Ok((content, encoding, line_ending))
}

/// Async version: Write content to a file with the specified encoding and line ending.
pub async fn write_with_format_async(
    path: &Path,
    content: &str,
    encoding: Encoding,
    line_ending: LineEnding,
) -> Result<(), EncodingError> {
    // Normalize line endings
    let normalized = normalize_line_endings(content, line_ending);

    // Encode content
    let mut bytes = encoding.bom().to_vec();
    bytes.extend(encoding.encode(&normalized));

    tokio::fs::write(path, bytes).await?;
    Ok(())
}

// Internal: Decode UTF-16LE bytes to String
fn decode_utf16le(bytes: &[u8]) -> Result<String, EncodingError> {
    if bytes.len() % 2 != 0 {
        return Err(EncodingError::InvalidUtf16(
            "UTF-16LE requires even number of bytes".to_string(),
        ));
    }
    let u16_iter = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]));
    char::decode_utf16(u16_iter)
        .collect::<Result<String, _>>()
        .map_err(|e| EncodingError::InvalidUtf16(e.to_string()))
}

// Internal: Decode UTF-16BE bytes to String
fn decode_utf16be(bytes: &[u8]) -> Result<String, EncodingError> {
    if bytes.len() % 2 != 0 {
        return Err(EncodingError::InvalidUtf16(
            "UTF-16BE requires even number of bytes".to_string(),
        ));
    }
    let u16_iter = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]));
    char::decode_utf16(u16_iter)
        .collect::<Result<String, _>>()
        .map_err(|e| EncodingError::InvalidUtf16(e.to_string()))
}

// Internal: Encode String to UTF-16LE bytes
fn encode_utf16le(content: &str) -> Vec<u8> {
    content
        .encode_utf16()
        .flat_map(|code_unit| code_unit.to_le_bytes())
        .collect()
}

// Internal: Encode String to UTF-16BE bytes
fn encode_utf16be(content: &str) -> Vec<u8> {
    content
        .encode_utf16()
        .flat_map(|code_unit| code_unit.to_be_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_encoding_utf8_no_bom() {
        let bytes = b"Hello World";
        assert_eq!(detect_encoding(bytes), Encoding::Utf8);
    }

    #[test]
    fn test_detect_encoding_utf8_with_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'H', b'e', b'l', b'l', b'o'];
        assert_eq!(detect_encoding(&bytes), Encoding::Utf8WithBom);
    }

    #[test]
    fn test_detect_encoding_utf16le() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("Hello".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(detect_encoding(&bytes), Encoding::Utf16Le);
    }

    #[test]
    fn test_detect_encoding_utf16be() {
        let mut bytes = vec![0xFE, 0xFF];
        bytes.extend("Hello".encode_utf16().flat_map(|u| u.to_be_bytes()));
        assert_eq!(detect_encoding(&bytes), Encoding::Utf16Be);
    }

    #[test]
    fn test_decode_utf8() {
        let bytes = b"Hello World";
        let content = Encoding::Utf8.decode(bytes).unwrap();
        assert_eq!(content, "Hello World");
    }

    #[test]
    fn test_decode_utf8_with_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'H', b'i'];
        let content = Encoding::Utf8.decode(&bytes).unwrap();
        assert_eq!(content, "Hi");
    }

    #[test]
    fn test_decode_utf16le() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("Hi".encode_utf16().flat_map(|u| u.to_le_bytes()));
        let content = Encoding::Utf16Le.decode(&bytes).unwrap();
        assert_eq!(content, "Hi");
    }

    #[test]
    fn test_decode_utf16be() {
        let mut bytes = vec![0xFE, 0xFF];
        bytes.extend("Hi".encode_utf16().flat_map(|u| u.to_be_bytes()));
        let content = Encoding::Utf16Be.decode(&bytes).unwrap();
        assert_eq!(content, "Hi");
    }

    #[test]
    fn test_detect_line_ending_lf() {
        let content = "line1\nline2\nline3";
        assert_eq!(detect_line_ending(content), LineEnding::Lf);
    }

    #[test]
    fn test_detect_line_ending_crlf() {
        let content = "line1\r\nline2\r\nline3";
        assert_eq!(detect_line_ending(content), LineEnding::CrLf);
    }

    #[test]
    fn test_detect_line_ending_cr_only_returns_lf() {
        // CR-only line endings (old Mac OS 9) are rare and not worth special casing
        // Simplified detection returns LF for CR-only content
        let content = "line1\rline2\rline3";
        assert_eq!(detect_line_ending(content), LineEnding::Lf);
    }

    #[test]
    fn test_detect_line_ending_mixed_prefers_crlf() {
        let content = "line1\r\nline2\nline3\r\n";
        assert_eq!(detect_line_ending(content), LineEnding::CrLf);
    }

    #[test]
    fn test_detect_line_ending_no_newlines() {
        let content = "no newlines here";
        assert_eq!(detect_line_ending(content), LineEnding::Lf);
    }

    #[test]
    fn test_normalize_line_endings_to_crlf() {
        let content = "line1\nline2\nline3";
        let normalized = normalize_line_endings(content, LineEnding::CrLf);
        assert_eq!(normalized, "line1\r\nline2\r\nline3");
    }

    #[test]
    fn test_normalize_line_endings_to_lf() {
        let content = "line1\r\nline2\r\nline3";
        let normalized = normalize_line_endings(content, LineEnding::Lf);
        assert_eq!(normalized, "line1\nline2\nline3");
    }

    #[test]
    fn test_normalize_mixed_to_lf() {
        let content = "line1\r\nline2\rline3\nline4";
        let normalized = normalize_line_endings(content, LineEnding::Lf);
        assert_eq!(normalized, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_encode_utf16le() {
        let encoded = Encoding::Utf16Le.encode("Hi");
        // 'H' = 0x0048, 'i' = 0x0069 in UTF-16LE
        assert_eq!(encoded, vec![0x48, 0x00, 0x69, 0x00]);
    }

    #[test]
    fn test_encode_utf16be() {
        let encoded = Encoding::Utf16Be.encode("Hi");
        // 'H' = 0x0048, 'i' = 0x0069 in UTF-16BE
        assert_eq!(encoded, vec![0x00, 0x48, 0x00, 0x69]);
    }

    #[test]
    fn test_roundtrip_utf8_lf() {
        let original = "Hello\nWorld\n";
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        write_with_format(&path, original, Encoding::Utf8, LineEnding::Lf).unwrap();
        let (content, enc, le) = read_with_format(&path).unwrap();

        assert_eq!(content, original);
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(le, LineEnding::Lf);
    }

    #[test]
    fn test_roundtrip_utf8_crlf() {
        let original = "Hello\nWorld\n";
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        write_with_format(&path, original, Encoding::Utf8, LineEnding::CrLf).unwrap();
        let (content, enc, le) = read_with_format(&path).unwrap();

        assert_eq!(content, "Hello\r\nWorld\r\n");
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(le, LineEnding::CrLf);
    }

    #[test]
    fn test_roundtrip_utf16le() {
        let original = "Hello\nWorld\n";
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        write_with_format(&path, original, Encoding::Utf16Le, LineEnding::Lf).unwrap();
        let (content, enc, le) = read_with_format(&path).unwrap();

        assert_eq!(content, original);
        assert_eq!(enc, Encoding::Utf16Le);
        assert_eq!(le, LineEnding::Lf);
    }

    #[test]
    fn test_roundtrip_utf16be_crlf() {
        let original = "Hello\nWorld\n";
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        write_with_format(&path, original, Encoding::Utf16Be, LineEnding::CrLf).unwrap();
        let (content, enc, le) = read_with_format(&path).unwrap();

        assert_eq!(content, "Hello\r\nWorld\r\n");
        assert_eq!(enc, Encoding::Utf16Be);
        assert_eq!(le, LineEnding::CrLf);
    }

    #[test]
    fn test_bom_bytes() {
        assert!(Encoding::Utf8.bom().is_empty());
        assert_eq!(Encoding::Utf8WithBom.bom(), &[0xEF, 0xBB, 0xBF]);
        assert_eq!(Encoding::Utf16Le.bom(), &[0xFF, 0xFE]);
        assert_eq!(Encoding::Utf16Be.bom(), &[0xFE, 0xFF]);
    }

    #[test]
    fn test_line_ending_as_str() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
        assert_eq!(LineEnding::Cr.as_str(), "\r");
    }

    #[test]
    fn test_has_trailing_newline() {
        assert!(has_trailing_newline("hello\n"));
        assert!(has_trailing_newline("hello\r\n"));
        assert!(!has_trailing_newline("hello"));
        assert!(!has_trailing_newline(""));
    }

    #[test]
    fn test_preserve_trailing_newline_add() {
        // Original had trailing newline, modified doesn't - add it
        let preserved = preserve_trailing_newline("hello\n", "world");
        assert_eq!(preserved, "world\n");
    }

    #[test]
    fn test_preserve_trailing_newline_remove() {
        // Original didn't have trailing newline, modified does - remove it
        let preserved = preserve_trailing_newline("hello", "world\n");
        assert_eq!(preserved, "world");
    }

    #[test]
    fn test_preserve_trailing_newline_keep_both() {
        // Both have trailing newline - keep as is
        let preserved = preserve_trailing_newline("hello\n", "world\n");
        assert_eq!(preserved, "world\n");
    }

    #[test]
    fn test_preserve_trailing_newline_keep_neither() {
        // Neither has trailing newline - keep as is
        let preserved = preserve_trailing_newline("hello", "world");
        assert_eq!(preserved, "world");
    }

    #[test]
    fn test_roundtrip_utf8_with_bom() {
        let original = "Hello\nWorld\n";
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        // Write with BOM
        write_with_format(&path, original, Encoding::Utf8WithBom, LineEnding::Lf).unwrap();

        // Verify BOM is written
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..3], &[0xEF, 0xBB, 0xBF]);

        // Read back and verify encoding is detected as Utf8WithBom
        let (content, enc, le) = read_with_format(&path).unwrap();
        assert_eq!(content, original);
        assert_eq!(enc, Encoding::Utf8WithBom);
        assert_eq!(le, LineEnding::Lf);
    }
}
