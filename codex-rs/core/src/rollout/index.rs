use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ConversationId;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::SESSIONS_SUBDIR;

const INDEX_SUBDIR: &str = "index";
const BY_DIR_SUBDIR: &str = "by-dir";
const BY_SESSION_SUBDIR: &str = "by-session";

#[derive(Clone, Debug)]
pub struct SessionIndex {
    root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct IndexedSession {
    pub session_id: ConversationId,
    pub rollout_path: PathBuf,
    pub last_used_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct DirRecord {
    canonical_dir: String,
    entries: Vec<DirEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct DirEntry {
    session_id: ConversationId,
    rollout_path: PathBuf,
    last_used_at: String,
}

#[derive(Serialize, Deserialize)]
struct SessionRecord {
    session_id: ConversationId,
    directories: Vec<String>,
}

impl SessionIndex {
    pub fn new(codex_home: &Path) -> io::Result<Self> {
        let root = codex_home.join(SESSIONS_SUBDIR).join(INDEX_SUBDIR);
        fs::create_dir_all(root.join(BY_DIR_SUBDIR))?;
        fs::create_dir_all(root.join(BY_SESSION_SUBDIR))?;
        Ok(Self { root })
    }

    pub fn record_session_usage(
        &self,
        cwd: &Path,
        session_id: &ConversationId,
        rollout_path: &Path,
    ) -> io::Result<()> {
        let canonical_dir = normalize_dir(cwd);
        let dir_key = dir_hash(&canonical_dir);
        let dir_path = self.by_dir_path(&dir_key);
        let now = Utc::now().to_rfc3339();

        let mut record = self
            .read_dir_record(&dir_path)?
            .unwrap_or_else(|| DirRecord {
                canonical_dir: canonical_dir.clone(),
                entries: Vec::new(),
            });
        if record.canonical_dir != canonical_dir {
            record.canonical_dir = canonical_dir.clone();
        }

        let mut replaced = false;
        for entry in &mut record.entries {
            if entry.session_id == *session_id {
                entry.rollout_path = rollout_path.to_path_buf();
                entry.last_used_at = now.clone();
                replaced = true;
                break;
            }
        }
        if !replaced {
            record.entries.push(DirEntry {
                session_id: *session_id,
                rollout_path: rollout_path.to_path_buf(),
                last_used_at: now,
            });
        }
        record
            .entries
            .sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));
        self.write_dir_record(&dir_path, &record)?;

        self.update_session_record(session_id, |session_record| {
            if !session_record
                .directories
                .iter()
                .any(|dir| dir == &canonical_dir)
            {
                session_record.directories.push(canonical_dir.clone());
            }
            session_record.directories.sort_unstable();
        })
    }

    pub fn sessions_for_dir(&self, cwd: &Path) -> io::Result<Vec<IndexedSession>> {
        let canonical_dir = normalize_dir(cwd);
        let dir_key = dir_hash(&canonical_dir);
        let dir_path = self.by_dir_path(&dir_key);
        let Some(mut record) = self.read_dir_record(&dir_path)? else {
            return Ok(Vec::new());
        };

        let mut changed = false;
        let mut indexed: Vec<IndexedSession> = Vec::new();
        record.entries.retain(|entry| {
            if !entry.rollout_path.exists() {
                changed = true;
                if let Err(err) =
                    self.remove_directory_from_session(&entry.session_id, &canonical_dir)
                {
                    tracing::warn!(?err, "failed to prune session directory mapping");
                }
                return false;
            }
            match DateTime::parse_from_rfc3339(&entry.last_used_at) {
                Ok(parsed) => indexed.push(IndexedSession {
                    session_id: entry.session_id,
                    rollout_path: entry.rollout_path.clone(),
                    last_used_at: parsed.with_timezone(&Utc),
                }),
                Err(err) => {
                    tracing::warn!(?err, "failed to parse last_used_at for session index entry");
                    indexed.push(IndexedSession {
                        session_id: entry.session_id,
                        rollout_path: entry.rollout_path.clone(),
                        last_used_at: Utc::now(),
                    });
                }
            }
            true
        });

        if changed {
            self.write_dir_record(&dir_path, &record)?;
        }

        indexed.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));
        Ok(indexed)
    }

    fn remove_directory_from_session(
        &self,
        session_id: &ConversationId,
        canonical_dir: &str,
    ) -> io::Result<()> {
        self.update_session_record(session_id, |session_record| {
            session_record
                .directories
                .retain(|dir| dir != canonical_dir);
        })
    }

    fn update_session_record<F>(&self, session_id: &ConversationId, mut f: F) -> io::Result<()>
    where
        F: FnMut(&mut SessionRecord),
    {
        let path = self.by_session_path(session_id);
        let mut record = if path.exists() {
            let text = fs::read_to_string(&path)?;
            serde_json::from_str::<SessionRecord>(&text)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        } else {
            SessionRecord {
                session_id: *session_id,
                directories: Vec::new(),
            }
        };

        f(&mut record);

        if record.directories.is_empty() {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        } else {
            let json = serde_json::to_string_pretty(&record)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            fs::write(path, json)?;
        }
        Ok(())
    }

    fn by_dir_path(&self, key: &str) -> PathBuf {
        self.root.join(BY_DIR_SUBDIR).join(format!("{key}.json"))
    }

    fn by_session_path(&self, session_id: &ConversationId) -> PathBuf {
        self.root
            .join(BY_SESSION_SUBDIR)
            .join(format!("{session_id}.json"))
    }

    fn read_dir_record(&self, path: &Path) -> io::Result<Option<DirRecord>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path)?;
        let record = serde_json::from_str::<DirRecord>(&text)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Some(record))
    }

    fn write_dir_record(&self, path: &Path, record: &DirRecord) -> io::Result<()> {
        if record.entries.is_empty() {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
            return Ok(());
        }
        let json = serde_json::to_string_pretty(record)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        fs::write(path, json)
    }
}

fn dir_hash(canonical_dir: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_dir.as_bytes());
    let digest = hasher.finalize();
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalize_dir(path: &Path) -> String {
    let s = path.as_os_str().to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        return s.to_lowercase();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn records_and_reads_sessions_for_directory() {
        let temp = TempDir::new().expect("tempdir");
        let codex_home = temp.path();
        let index = SessionIndex::new(codex_home).expect("index");

        let cwd = codex_home.join("workspace");
        fs::create_dir_all(&cwd).expect("create cwd");
        let session_id = ConversationId::default();
        let rollout_path = codex_home.join(SESSIONS_SUBDIR).join("rollout.jsonl");
        fs::create_dir_all(rollout_path.parent().unwrap()).expect("create sessions");
        fs::write(&rollout_path, "{}\n").expect("write rollout");

        index
            .record_session_usage(&cwd, &session_id, &rollout_path)
            .expect("record");

        let sessions = index.sessions_for_dir(&cwd).expect("read");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].rollout_path, rollout_path);

        fs::remove_file(&rollout_path).expect("remove rollout");
        let sessions = index.sessions_for_dir(&cwd).expect("read after remove");
        assert!(sessions.is_empty());
    }
}
