use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

/// Records subagent interactions to a JSONL file.
///
/// Each entry is written as a single JSON line, suitable for post-hoc analysis
/// and debugging.
pub struct TranscriptRecorder {
    path: PathBuf,
}

impl TranscriptRecorder {
    /// Create a new recorder that writes to the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Append an entry to the transcript file.
    ///
    /// Each entry is serialized as a single JSON line followed by a newline.
    pub fn record(&self, entry: &serde_json::Value) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let line = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Returns the path to the transcript file.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Read all entries from a transcript file.
    ///
    /// Each line is parsed as a JSON value. Empty and invalid lines are skipped.
    pub fn read_transcript(path: &Path) -> std::io::Result<Vec<serde_json::Value>> {
        let content = std::fs::read_to_string(path)?;
        Ok(content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }

    /// Read transcript entries from this recorder's path.
    pub fn read_entries(&self) -> std::io::Result<Vec<serde_json::Value>> {
        Self::read_transcript(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcript_recorder_path() {
        let path = PathBuf::from("/tmp/test-transcript.jsonl");
        let recorder = TranscriptRecorder::new(path.clone());
        assert_eq!(recorder.path(), &path);
    }

    #[test]
    fn test_transcript_record_write() {
        let dir = std::env::temp_dir().join("cocode-transcript-test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("test.jsonl");

        // Clean up any previous run.
        let _ = std::fs::remove_file(&path);

        let recorder = TranscriptRecorder::new(path.clone());
        let entry1 = serde_json::json!({"role": "user", "content": "hello"});
        let entry2 = serde_json::json!({"role": "assistant", "content": "hi"});

        recorder.record(&entry1).expect("record entry 1");
        recorder.record(&entry2).expect("record entry 2");

        let contents = std::fs::read_to_string(&path).expect("read transcript");
        let lines: Vec<&str> = contents.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        let parsed1: serde_json::Value = serde_json::from_str(lines[0]).expect("parse line 1");
        assert_eq!(parsed1["role"], "user");
        let parsed2: serde_json::Value = serde_json::from_str(lines[1]).expect("parse line 2");
        assert_eq!(parsed2["role"], "assistant");

        // Clean up.
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_read_empty_file() {
        let dir = std::env::temp_dir().join("cocode-transcript-read-empty");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("empty.jsonl");

        // Write an empty file.
        std::fs::write(&path, "").expect("write empty file");

        let entries = TranscriptRecorder::read_transcript(&path).expect("read transcript");
        assert!(entries.is_empty());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_read_entries_roundtrip() {
        let dir = std::env::temp_dir().join("cocode-transcript-read-roundtrip");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("roundtrip.jsonl");
        let _ = std::fs::remove_file(&path);

        let recorder = TranscriptRecorder::new(path.clone());
        let entry1 = serde_json::json!({"role": "user", "content": "hello"});
        let entry2 = serde_json::json!({"role": "assistant", "content": "hi"});

        recorder.record(&entry1).expect("record entry 1");
        recorder.record(&entry2).expect("record entry 2");

        let entries = recorder.read_entries().expect("read entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["role"], "user");
        assert_eq!(entries[0]["content"], "hello");
        assert_eq!(entries[1]["role"], "assistant");
        assert_eq!(entries[1]["content"], "hi");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_read_skips_invalid_lines() {
        let dir = std::env::temp_dir().join("cocode-transcript-read-invalid");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("invalid.jsonl");

        // Write a mix of valid JSON, invalid JSON, and blank lines.
        let content = r#"{"role":"user","content":"hello"}
not valid json
{"role":"assistant","content":"hi"}

{broken
{"role":"system","content":"done"}
"#;
        std::fs::write(&path, content).expect("write test file");

        let entries = TranscriptRecorder::read_transcript(&path).expect("read transcript");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["role"], "user");
        assert_eq!(entries[1]["role"], "assistant");
        assert_eq!(entries[2]["role"], "system");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
