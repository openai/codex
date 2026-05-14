use crate::CopyOptions;
use crate::CreateDirectoryOptions;
use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemResult;
use crate::FileSystemSandboxContext;
use crate::ReadDirectoryEntry;
use crate::RemoveOptions;
use async_trait::async_trait;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::BTreeMap;
use std::io;
use std::sync::RwLock;

#[derive(Clone, Debug)]
enum EntryKind {
    Directory,
    File(Vec<u8>),
}

#[derive(Clone, Debug)]
struct Entry {
    kind: EntryKind,
    created_at_ms: i64,
    modified_at_ms: i64,
}

#[derive(Debug)]
struct State {
    entries: BTreeMap<AbsolutePathBuf, Entry>,
    next_timestamp_ms: i64,
}

impl State {
    fn next_timestamp_ms(&mut self) -> i64 {
        let timestamp = self.next_timestamp_ms;
        self.next_timestamp_ms += 1;
        timestamp
    }
}

/// Pure in-memory implementation of [`ExecutorFileSystem`] for tests and
/// injected-fs call sites that must avoid host filesystem access.
#[derive(Debug, Default)]
pub struct InMemoryFileSystem {
    state: RwLock<State>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_timestamp_ms: 1,
        }
    }
}

impl InMemoryFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_directory(&self, path: &AbsolutePathBuf) -> io::Result<()> {
        let mut state = self.state.write().expect("in-memory fs write lock");
        create_directory_all(&mut state, path)
    }

    pub fn seed_file(
        &self,
        path: &AbsolutePathBuf,
        contents: impl Into<Vec<u8>>,
    ) -> io::Result<()> {
        let mut state = self.state.write().expect("in-memory fs write lock");
        ensure_parent_directories(&mut state, path)?;
        if is_directory(&state, path) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("path already exists: {}", path.display()),
            ));
        }
        let timestamp = state.next_timestamp_ms();
        state.entries.insert(
            path.clone(),
            Entry {
                kind: EntryKind::File(contents.into()),
                created_at_ms: timestamp,
                modified_at_ms: timestamp,
            },
        );
        Ok(())
    }

    pub fn exists(&self, path: &AbsolutePathBuf) -> bool {
        let state = self.state.read().expect("in-memory fs read lock");
        entry_exists(&state, path)
    }

    pub fn file_contents(&self, path: &AbsolutePathBuf) -> Option<Vec<u8>> {
        let state = self.state.read().expect("in-memory fs read lock");
        match state.entries.get(path) {
            Some(Entry {
                kind: EntryKind::File(contents),
                ..
            }) => Some(contents.clone()),
            Some(Entry {
                kind: EntryKind::Directory,
                ..
            })
            | None => None,
        }
    }
}

#[async_trait]
impl ExecutorFileSystem for InMemoryFileSystem {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        reject_sandbox_context(sandbox)?;
        let state = self.state.read().expect("in-memory fs read lock");
        match state.entries.get(path) {
            Some(Entry {
                kind: EntryKind::File(contents),
                ..
            }) => Ok(contents.clone()),
            Some(Entry {
                kind: EntryKind::Directory,
                ..
            }) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot read a directory as a file",
            )),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("path not found: {}", path.display()),
            )),
        }
    }

    async fn write_file(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        reject_sandbox_context(sandbox)?;
        let mut state = self.state.write().expect("in-memory fs write lock");
        ensure_file_parent_exists(&state, path)?;
        if is_directory(&state, path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot overwrite a directory with a file",
            ));
        }
        let timestamp = state.next_timestamp_ms();
        let created_at_ms = state
            .entries
            .get(path)
            .map(|entry| entry.created_at_ms)
            .unwrap_or(timestamp);
        state.entries.insert(
            path.clone(),
            Entry {
                kind: EntryKind::File(contents),
                created_at_ms,
                modified_at_ms: timestamp,
            },
        );
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        reject_sandbox_context(sandbox)?;
        let mut state = self.state.write().expect("in-memory fs write lock");
        if is_root(path) {
            return if options.recursive {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("directory already exists: {}", path.display()),
                ))
            };
        }

        if let Some(entry) = state.entries.get(path) {
            return if matches!(entry.kind, EntryKind::Directory) && options.recursive {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("path already exists: {}", path.display()),
                ))
            };
        }

        if options.recursive {
            create_directory_all(&mut state, path)
        } else {
            ensure_directory_parent_exists(&state, path)?;
            insert_directory(&mut state, path);
            Ok(())
        }
    }

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        reject_sandbox_context(sandbox)?;
        let state = self.state.read().expect("in-memory fs read lock");
        if is_root(path) {
            return Ok(directory_metadata(
                /*created_at_ms*/ 0, /*modified_at_ms*/ 0,
            ));
        }

        let entry = state.entries.get(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("path not found: {}", path.display()),
            )
        })?;
        Ok(match &entry.kind {
            EntryKind::Directory => directory_metadata(entry.created_at_ms, entry.modified_at_ms),
            EntryKind::File(_) => FileMetadata {
                is_directory: false,
                is_file: true,
                is_symlink: false,
                created_at_ms: entry.created_at_ms,
                modified_at_ms: entry.modified_at_ms,
            },
        })
    }

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        reject_sandbox_context(sandbox)?;
        let state = self.state.read().expect("in-memory fs read lock");
        if !is_root(path) && !is_directory(&state, path) {
            return if state.entries.contains_key(path) {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "cannot read directory entries from a file",
                ))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("path not found: {}", path.display()),
                ))
            };
        }

        let mut entries = Vec::new();
        for (entry_path, entry) in &state.entries {
            if entry_path.parent().as_ref() != Some(path) {
                continue;
            }
            entries.push(ReadDirectoryEntry {
                file_name: file_name(entry_path)?,
                is_directory: matches!(entry.kind, EntryKind::Directory),
                is_file: matches!(entry.kind, EntryKind::File(_)),
            });
        }
        Ok(entries)
    }

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        reject_sandbox_context(sandbox)?;
        if is_root(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot remove the virtual filesystem root",
            ));
        }

        let mut state = self.state.write().expect("in-memory fs write lock");
        let Some(entry) = state.entries.get(path).cloned() else {
            return if options.force {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("path not found: {}", path.display()),
                ))
            };
        };

        match entry.kind {
            EntryKind::File(_) => {
                state.entries.remove(path);
                Ok(())
            }
            EntryKind::Directory => {
                let has_children = state
                    .entries
                    .keys()
                    .any(|candidate| candidate.parent().as_ref() == Some(path));
                if has_children && !options.recursive {
                    return Err(io::Error::new(
                        io::ErrorKind::DirectoryNotEmpty,
                        format!("directory is not empty: {}", path.display()),
                    ));
                }
                state
                    .entries
                    .retain(|candidate, _| !candidate.starts_with(path));
                Ok(())
            }
        }
    }

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        options: CopyOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        reject_sandbox_context(sandbox)?;
        if destination_path == source_path || destination_path.starts_with(source_path) {
            let source_is_directory = {
                let state = self.state.read().expect("in-memory fs read lock");
                is_root(source_path) || is_directory(&state, source_path)
            };
            if source_is_directory {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "fs/copy cannot copy a directory to itself or one of its descendants",
                ));
            }
        }

        let snapshot = {
            let state = self.state.read().expect("in-memory fs read lock");
            snapshot_entry(&state, source_path)?
        };

        let mut state = self.state.write().expect("in-memory fs write lock");
        match snapshot {
            SnapshotEntry::File { contents } => {
                ensure_file_parent_exists(&state, destination_path)?;
                if is_directory(&state, destination_path) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "cannot copy a file over a directory",
                    ));
                }
                let timestamp = state.next_timestamp_ms();
                state.entries.insert(
                    destination_path.clone(),
                    Entry {
                        kind: EntryKind::File(contents),
                        created_at_ms: timestamp,
                        modified_at_ms: timestamp,
                    },
                );
                Ok(())
            }
            SnapshotEntry::Directory { entries } => {
                if !options.recursive {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "fs/copy requires recursive: true when sourcePath is a directory",
                    ));
                }
                create_directory_all(&mut state, destination_path)?;
                for (source_entry, snapshot_entry) in entries {
                    let relative =
                        source_entry
                            .strip_prefix(source_path.as_path())
                            .map_err(|err| {
                                io::Error::other(format!("failed to strip prefix: {err}"))
                            })?;
                    let destination_entry = destination_path.join(relative);
                    match snapshot_entry {
                        SnapshotDirectoryEntry::Directory => {
                            create_directory_all(&mut state, &destination_entry)?;
                        }
                        SnapshotDirectoryEntry::File(contents) => {
                            ensure_parent_directories(&mut state, &destination_entry)?;
                            let timestamp = state.next_timestamp_ms();
                            state.entries.insert(
                                destination_entry,
                                Entry {
                                    kind: EntryKind::File(contents),
                                    created_at_ms: timestamp,
                                    modified_at_ms: timestamp,
                                },
                            );
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

enum SnapshotEntry {
    File {
        contents: Vec<u8>,
    },
    Directory {
        entries: Vec<(AbsolutePathBuf, SnapshotDirectoryEntry)>,
    },
}

enum SnapshotDirectoryEntry {
    Directory,
    File(Vec<u8>),
}

fn snapshot_entry(state: &State, source_path: &AbsolutePathBuf) -> io::Result<SnapshotEntry> {
    if is_root(source_path) || is_directory(state, source_path) {
        let mut entries = Vec::new();
        for (path, entry) in &state.entries {
            if !path.starts_with(source_path) || path == source_path {
                continue;
            }
            entries.push((
                path.clone(),
                match &entry.kind {
                    EntryKind::Directory => SnapshotDirectoryEntry::Directory,
                    EntryKind::File(contents) => SnapshotDirectoryEntry::File(contents.clone()),
                },
            ));
        }
        return Ok(SnapshotEntry::Directory { entries });
    }

    match state.entries.get(source_path) {
        Some(Entry {
            kind: EntryKind::File(contents),
            ..
        }) => Ok(SnapshotEntry::File {
            contents: contents.clone(),
        }),
        Some(Entry {
            kind: EntryKind::Directory,
            ..
        }) => unreachable!("directory case handled above"),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("path not found: {}", source_path.display()),
        )),
    }
}

fn create_directory_all(state: &mut State, path: &AbsolutePathBuf) -> io::Result<()> {
    if is_root(path) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        create_directory_all(state, &parent)?;
    }
    if let Some(entry) = state.entries.get(path) {
        return if matches!(entry.kind, EntryKind::Directory) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("path already exists: {}", path.display()),
            ))
        };
    }
    insert_directory(state, path);
    Ok(())
}

fn insert_directory(state: &mut State, path: &AbsolutePathBuf) {
    let timestamp = state.next_timestamp_ms();
    state.entries.insert(
        path.clone(),
        Entry {
            kind: EntryKind::Directory,
            created_at_ms: timestamp,
            modified_at_ms: timestamp,
        },
    );
}

fn ensure_parent_directories(state: &mut State, path: &AbsolutePathBuf) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    create_directory_all(state, &parent)
}

fn ensure_file_parent_exists(state: &State, path: &AbsolutePathBuf) -> io::Result<()> {
    ensure_directory_parent_exists(state, path)
}

fn ensure_directory_parent_exists(state: &State, path: &AbsolutePathBuf) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if is_root(&parent) || is_directory(state, &parent) {
        return Ok(());
    }
    if state.entries.contains_key(&parent) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("parent is not a directory: {}", parent.display()),
        ));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("parent directory not found: {}", parent.display()),
    ))
}

fn directory_metadata(created_at_ms: i64, modified_at_ms: i64) -> FileMetadata {
    FileMetadata {
        is_directory: true,
        is_file: false,
        is_symlink: false,
        created_at_ms,
        modified_at_ms,
    }
}

fn entry_exists(state: &State, path: &AbsolutePathBuf) -> bool {
    is_root(path) || state.entries.contains_key(path)
}

fn is_directory(state: &State, path: &AbsolutePathBuf) -> bool {
    is_root(path)
        || matches!(
            state.entries.get(path),
            Some(Entry {
                kind: EntryKind::Directory,
                ..
            })
        )
}

fn is_root(path: &AbsolutePathBuf) -> bool {
    path.parent().is_none()
}

fn file_name(path: &AbsolutePathBuf) -> io::Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "path does not have a file name",
            )
        })
}

fn reject_sandbox_context(sandbox: Option<&FileSystemSandboxContext>) -> io::Result<()> {
    if sandbox.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "in-memory filesystem operations do not accept sandbox context",
        ));
    }
    Ok(())
}
