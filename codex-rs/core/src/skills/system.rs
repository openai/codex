use include_dir::Dir;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use thiserror::Error;

const SYSTEM_SKILLS_DIR: Dir =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/skills/assets/samples");

const SYSTEM_SKILLS_DIR_NAME: &str = ".system";
const SKILLS_DIR_NAME: &str = "skills";

pub(crate) fn system_cache_root_dir(codex_home: &Path) -> PathBuf {
    codex_home
        .join(SKILLS_DIR_NAME)
        .join(SYSTEM_SKILLS_DIR_NAME)
}

pub(crate) fn install_system_skills(codex_home: &Path) -> Result<(), SystemSkillsError> {
    let skills_root_dir = codex_home.join(SKILLS_DIR_NAME);
    fs::create_dir_all(&skills_root_dir)
        .map_err(|source| SystemSkillsError::io("create skills root dir", source))?;

    let dest_system = system_cache_root_dir(codex_home);
    let staged_system =
        skills_root_dir.join(format!("{SYSTEM_SKILLS_DIR_NAME}-tmp-{}", rand_suffix()));
    if staged_system.exists() {
        fs::remove_dir_all(&staged_system).map_err(|source| {
            SystemSkillsError::io("remove existing system skills tmp dir", source)
        })?;
    }

    write_embedded_dir(&SYSTEM_SKILLS_DIR, &staged_system)?;
    atomic_swap_dir(&staged_system, &dest_system, &skills_root_dir)?;
    Ok(())
}

fn write_embedded_dir(dir: &Dir<'_>, dest: &Path) -> Result<(), SystemSkillsError> {
    fs::create_dir_all(dest)
        .map_err(|source| SystemSkillsError::io("create system skills tmp dir", source))?;

    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                fs::create_dir_all(dest.join(subdir.path())).map_err(|source| {
                    SystemSkillsError::io("create system skills tmp subdir", source)
                })?;
                write_embedded_dir(subdir, dest)?;
            }
            include_dir::DirEntry::File(file) => {
                let path = dest.join(file.path());
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        SystemSkillsError::io("create system skills tmp file parent", source)
                    })?;
                }
                fs::write(&path, file.contents())
                    .map_err(|source| SystemSkillsError::io("write system skill file", source))?;
            }
        }
    }

    Ok(())
}

fn atomic_swap_dir(staged: &Path, dest: &Path, parent: &Path) -> Result<(), SystemSkillsError> {
    if let Some(dest_parent) = dest.parent() {
        fs::create_dir_all(dest_parent)
            .map_err(|source| SystemSkillsError::io("create system skills dest parent", source))?;
    }

    let backup_base = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skills");
    let backup = parent.join(format!("{backup_base}.old-{}", rand_suffix()));
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|source| SystemSkillsError::io("remove old system skills backup", source))?;
    }

    if dest.exists() {
        fs::rename(dest, &backup)
            .map_err(|source| SystemSkillsError::io("rename system skills to backup", source))?;
    }

    if let Err(err) = fs::rename(staged, dest) {
        if backup.exists() {
            let _ = fs::rename(&backup, dest);
        }
        return Err(SystemSkillsError::io(
            "rename staged system skills into place",
            err,
        ));
    }

    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|source| SystemSkillsError::io("remove system skills backup", source))?;
    }

    Ok(())
}

fn rand_suffix() -> String {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{pid:x}-{nanos:x}")
}

#[derive(Debug, Error)]
pub(crate) enum SystemSkillsError {
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl SystemSkillsError {
    fn io(action: &'static str, source: std::io::Error) -> Self {
        Self::Io { action, source }
    }
}
