use serde_json::Value as JsonValue;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;

const EXTERNAL_PROJECTS_SUBDIR: &str = "projects";
const EXTERNAL_MEMORY_SUBDIR: &str = "memory";
const EXTERNAL_SETTINGS_FILE: &str = "settings.json";
const EXTERNAL_LOCAL_SETTINGS_FILE: &str = "settings.local.json";
const CUSTOM_MEMORY_SCOPE: &str = "_custom";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalMemoryFile {
    pub project_key: String,
    pub cwd: Option<PathBuf>,
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
}

/// Discovers every Markdown auto-memory file that can be resolved from persisted settings.
///
/// Default project directories are always scanned so older memories are not lost after a custom
/// directory is configured. File-backed settings use Claude Code's effective precedence:
/// managed, local project, shared project, then user. Ephemeral `--settings`, server-managed
/// settings, and OS policy stores cannot be reconstructed from the filesystem and are therefore
/// outside this discovery surface.
pub fn discover_external_memory_files(
    external_agent_home: &Path,
    repo_roots: &[PathBuf],
) -> io::Result<Vec<ExternalMemoryFile>> {
    discover_external_memory_files_with_managed_root(
        external_agent_home,
        repo_roots,
        managed_settings_root().as_deref(),
    )
}

fn discover_external_memory_files_with_managed_root(
    external_agent_home: &Path,
    repo_roots: &[PathBuf],
    managed_settings_root: Option<&Path>,
) -> io::Result<Vec<ExternalMemoryFile>> {
    let mut files = Vec::new();
    let mut visited_memory_roots = BTreeSet::new();
    discover_default_project_memory(external_agent_home, &mut visited_memory_roots, &mut files)?;

    let user_memory_directory = read_auto_memory_directory(
        &external_agent_home.join(EXTERNAL_SETTINGS_FILE),
        external_agent_home,
    )?;
    let managed_memory_directory =
        read_managed_auto_memory_directory(external_agent_home, managed_settings_root)?;
    if let Some(memory_root) = managed_memory_directory
        .as_ref()
        .or(user_memory_directory.as_ref())
    {
        collect_custom_memory_files(
            memory_root,
            CUSTOM_MEMORY_SCOPE.to_string(),
            /*cwd*/ None,
            &mut visited_memory_roots,
            &mut files,
        )?;
    }

    let mut sorted_repo_roots = repo_roots.to_vec();
    sorted_repo_roots.sort();
    sorted_repo_roots.dedup();
    for repo_root in sorted_repo_roots {
        let shared_memory_directory = read_auto_memory_directory(
            &repo_root.join(".claude").join(EXTERNAL_SETTINGS_FILE),
            external_agent_home,
        )?;
        let local_memory_directory = read_auto_memory_directory(
            &repo_root.join(".claude").join(EXTERNAL_LOCAL_SETTINGS_FILE),
            external_agent_home,
        )?;
        let effective_memory_directory = managed_memory_directory
            .as_ref()
            .or(local_memory_directory.as_ref())
            .or(shared_memory_directory.as_ref())
            .or(user_memory_directory.as_ref());
        if let Some(memory_root) = effective_memory_directory {
            collect_custom_memory_files(
                memory_root,
                format!("project:{}", repo_root.display()),
                Some(repo_root.clone()),
                &mut visited_memory_roots,
                &mut files,
            )?;
        }
    }

    files.sort_by(|left, right| {
        left.project_key
            .cmp(&right.project_key)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    Ok(files)
}

fn discover_default_project_memory(
    external_agent_home: &Path,
    visited_memory_roots: &mut BTreeSet<PathBuf>,
    files: &mut Vec<ExternalMemoryFile>,
) -> io::Result<()> {
    let projects_root = external_agent_home.join(EXTERNAL_PROJECTS_SUBDIR);
    if !projects_root.is_dir() {
        return Ok(());
    }

    let mut project_entries = fs::read_dir(projects_root)?.collect::<Result<Vec<_>, _>>()?;
    project_entries.sort_by_key(fs::DirEntry::file_name);
    for project_entry in project_entries {
        if !project_entry.file_type()?.is_dir() {
            continue;
        }
        let memory_root = project_entry.path().join(EXTERNAL_MEMORY_SUBDIR);
        if !memory_root.is_dir() || !visited_memory_roots.insert(normalized_path(&memory_root)) {
            continue;
        }
        let project_key = project_entry.file_name().to_string_lossy().into_owned();
        let cwd = resolve_project_cwd(&project_entry.path())?;
        collect_markdown_files(&memory_root, &memory_root, &project_key, cwd, files)?;
    }
    Ok(())
}

fn collect_custom_memory_files(
    memory_root: &Path,
    project_key: String,
    cwd: Option<PathBuf>,
    visited_memory_roots: &mut BTreeSet<PathBuf>,
    files: &mut Vec<ExternalMemoryFile>,
) -> io::Result<()> {
    if !memory_root.is_dir() || !visited_memory_roots.insert(normalized_path(memory_root)) {
        return Ok(());
    }
    collect_markdown_files(memory_root, memory_root, &project_key, cwd, files)
}

fn collect_markdown_files(
    source_root: &Path,
    current_dir: &Path,
    project_key: &str,
    cwd: Option<PathBuf>,
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
            collect_markdown_files(source_root, &entry.path(), project_key, cwd.clone(), files)?;
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
            cwd: cwd.clone(),
            source_path: entry.path(),
            relative_path,
        });
    }
    Ok(())
}

fn resolve_project_cwd(project_root: &Path) -> io::Result<Option<PathBuf>> {
    let mut sessions = fs::read_dir(project_root)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
                && entry.file_type().is_ok_and(|file_type| file_type.is_file())
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        modified_time(&right.path())
            .cmp(&modified_time(&left.path()))
            .then_with(|| right.file_name().cmp(&left.file_name()))
    });

    for session in sessions {
        let reader = io::BufReader::new(fs::File::open(session.path())?);
        for line in reader.lines() {
            let line = line?;
            let Ok(value) = serde_json::from_str::<JsonValue>(&line) else {
                continue;
            };
            let Some(cwd) = value.get("cwd").and_then(JsonValue::as_str) else {
                continue;
            };
            let cwd = PathBuf::from(cwd);
            if !cwd.is_absolute() {
                continue;
            }
            return Ok(Some(normalized_path(&cwd)));
        }
    }
    Ok(None)
}

fn modified_time(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn read_managed_auto_memory_directory(
    external_agent_home: &Path,
    managed_settings_root: Option<&Path>,
) -> io::Result<Option<PathBuf>> {
    let Some(managed_root) = managed_settings_root else {
        return Ok(None);
    };
    let mut effective = read_auto_memory_directory(
        &managed_root.join("managed-settings.json"),
        external_agent_home,
    )?;
    let drop_ins_root = managed_root.join("managed-settings.d");
    if drop_ins_root.is_dir() {
        let mut drop_ins = fs::read_dir(drop_ins_root)?.collect::<Result<Vec<_>, _>>()?;
        drop_ins.sort_by_key(fs::DirEntry::file_name);
        for drop_in in drop_ins {
            if drop_in.file_type()?.is_file()
                && drop_in.path().extension().and_then(|value| value.to_str()) == Some("json")
                && let Some(memory_directory) =
                    read_auto_memory_directory(&drop_in.path(), external_agent_home)?
            {
                effective = Some(memory_directory);
            }
        }
    }
    Ok(effective)
}

#[cfg(target_os = "macos")]
fn managed_settings_root() -> Option<PathBuf> {
    Some(PathBuf::from("/Library/Application Support/ClaudeCode"))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn managed_settings_root() -> Option<PathBuf> {
    Some(PathBuf::from("/etc/claude-code"))
}

#[cfg(windows)]
fn managed_settings_root() -> Option<PathBuf> {
    std::env::var_os("ProgramFiles").map(|root| PathBuf::from(root).join("ClaudeCode"))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    windows
)))]
fn managed_settings_root() -> Option<PathBuf> {
    None
}

fn read_auto_memory_directory(
    settings_path: &Path,
    external_agent_home: &Path,
) -> io::Result<Option<PathBuf>> {
    if !settings_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(settings_path)?;
    let settings: JsonValue = serde_json::from_str(&raw).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid external-agent settings at {}: {err}",
                settings_path.display()
            ),
        )
    })?;
    let Some(configured_path) = settings
        .get("autoMemoryDirectory")
        .and_then(JsonValue::as_str)
    else {
        return Ok(None);
    };
    resolve_configured_path(configured_path, external_agent_home).map(Some)
}

fn resolve_configured_path(
    configured_path: &str,
    external_agent_home: &Path,
) -> io::Result<PathBuf> {
    let home = external_agent_home.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "external-agent home has no parent directory",
        )
    })?;
    if configured_path == "~" {
        return Ok(home.to_path_buf());
    }
    if let Some(relative_to_home) = configured_path.strip_prefix("~/") {
        if relative_to_home.is_empty() {
            return Err(invalid_data(
                "autoMemoryDirectory must name a directory below ~/",
            ));
        }
        return Ok(home.join(relative_to_home));
    }
    let configured_path = PathBuf::from(configured_path);
    if !configured_path.is_absolute() {
        return Err(invalid_data(
            "autoMemoryDirectory must be absolute or start with ~/",
        ));
    }
    Ok(configured_path)
}

fn normalized_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
