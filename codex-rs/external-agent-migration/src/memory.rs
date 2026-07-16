use crate::sessions::summarize_session;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

const EXTERNAL_PROJECTS_SUBDIR: &str = "projects";
const EXTERNAL_MEMORY_SUBDIR: &str = "memory";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalMemoryFile {
    pub project_key: String,
    pub project_cwd: Option<PathBuf>,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
}

/// Discovers every Markdown file in each external-agent project's memory directory.
pub fn discover_external_memory_files(
    external_agent_home: &Path,
) -> io::Result<Vec<ExternalMemoryFile>> {
    let mut files = Vec::new();
    discover_project_memory(external_agent_home, &mut files)?;

    files.sort_by(|left, right| {
        left.project_key
            .cmp(&right.project_key)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    Ok(files)
}

fn discover_project_memory(
    external_agent_home: &Path,
    files: &mut Vec<ExternalMemoryFile>,
) -> io::Result<()> {
    let projects_root = external_agent_home.join(EXTERNAL_PROJECTS_SUBDIR);
    let projects_metadata = match fs::symlink_metadata(&projects_root) {
        Ok(projects_metadata) => projects_metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if !projects_metadata.file_type().is_dir() {
        return Ok(());
    }

    let mut project_entries = fs::read_dir(projects_root)?.collect::<Result<Vec<_>, _>>()?;
    project_entries.sort_by_key(fs::DirEntry::file_name);
    for project_entry in project_entries {
        if !project_entry.file_type()?.is_dir() {
            continue;
        }
        let memory_root = project_entry.path().join(EXTERNAL_MEMORY_SUBDIR);
        let memory_metadata = match fs::symlink_metadata(&memory_root) {
            Ok(memory_metadata) => memory_metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        if !memory_metadata.file_type().is_dir() {
            continue;
        }
        let project_key = project_entry.file_name().to_string_lossy().into_owned();
        let project_cwd = project_cwd_from_sessions(&project_entry.path())?;
        collect_markdown_files(
            &memory_root,
            &memory_root,
            &project_key,
            project_cwd.as_deref(),
            files,
        )?;
    }
    Ok(())
}

fn project_cwd_from_sessions(project_root: &Path) -> io::Result<Option<PathBuf>> {
    let mut sessions = fs::read_dir(project_root)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let path = entry.path();
            if !file_type.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("jsonl")
            {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| right.cmp(left));

    for (_, session_path) in sessions {
        if let Ok(Some(summary)) = summarize_session(&session_path) {
            let cwd = summary.migration.cwd;
            if !cwd.is_absolute() {
                continue;
            }
            let Ok(cwd) = fs::canonicalize(cwd) else {
                continue;
            };
            if cwd.is_dir() {
                return Ok(Some(cwd));
            }
        }
    }
    Ok(None)
}

fn collect_markdown_files(
    source_root: &Path,
    current_dir: &Path,
    project_key: &str,
    project_cwd: Option<&Path>,
    files: &mut Vec<ExternalMemoryFile>,
) -> io::Result<()> {
    let mut entries = fs::read_dir(current_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_markdown_files(source_root, &entry.path(), project_key, project_cwd, files)?;
            continue;
        }
        if !file_type.is_file() || !is_markdown_file(&entry.path()) {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(source_root)
            .map(Path::to_path_buf)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        files.push(ExternalMemoryFile {
            project_key: project_key.to_string(),
            project_cwd: project_cwd.map(Path::to_path_buf),
            source_path: entry.path(),
            relative_path,
        });
    }
    Ok(())
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
