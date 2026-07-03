use std::io;
use std::path::Path;
use std::path::PathBuf;

use super::bytes_to_path;
use super::read_bounded_file;
use crate::git_config::parse_git_boolean;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CommonConfigAuthority {
    Bare,
    Worktree(PathBuf),
    Unproven,
}

pub(crate) fn inspect_plain_common_config_authority(
    common_dir: &Path,
) -> io::Result<CommonConfigAuthority> {
    let config_path = common_dir.join("config");
    let metadata = match std::fs::symlink_metadata(&config_path) {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(CommonConfigAuthority::Unproven);
        }
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(CommonConfigAuthority::Unproven);
    }
    let bytes = read_bounded_file(
        &config_path,
        1024 * 1024,
        "Git common config is too large for authority proof",
    )?;
    let config = match gix::config::File::from_bytes_no_includes(
        &bytes,
        gix::config::file::Metadata::default(),
        gix::config::file::init::Options::default(),
    ) {
        Ok(config) => config,
        Err(_) => return Ok(CommonConfigAuthority::Unproven),
    };
    if config.sections_by_name("include").is_some()
        || config.sections_by_name("includeIf").is_some()
    {
        return Ok(CommonConfigAuthority::Unproven);
    }
    let bare = match unique_explicit_value(&config, "core", "bare") {
        Ok(Some(value)) => match parse_git_boolean(value.as_ref()) {
            Some(value) => Some(value),
            None => return Ok(CommonConfigAuthority::Unproven),
        },
        Ok(None) => None,
        Err(()) => return Ok(CommonConfigAuthority::Unproven),
    };
    match unique_explicit_value(&config, "extensions", "worktreeConfig") {
        Ok(Some(value)) if parse_git_boolean(value.as_ref()) == Some(false) => {}
        Ok(None) => {}
        Ok(Some(_)) | Err(()) => return Ok(CommonConfigAuthority::Unproven),
    }
    let worktree = match unique_explicit_value(&config, "core", "worktree") {
        Ok(Some(value)) => Some(value),
        Ok(None) => None,
        Err(()) => return Ok(CommonConfigAuthority::Unproven),
    };
    if let Some(worktree) = worktree {
        if bare == Some(true) {
            return Ok(CommonConfigAuthority::Unproven);
        }
        let worktree = bytes_to_path(&worktree)?;
        if worktree.as_os_str().is_empty() || !worktree.is_absolute() {
            return Ok(CommonConfigAuthority::Unproven);
        }
        #[cfg(windows)]
        if worktree
            .to_str()
            .is_none_or(crate::path_authority::windows_authority_path_is_ambiguous)
        {
            return Ok(CommonConfigAuthority::Unproven);
        }
        return Ok(CommonConfigAuthority::Worktree(worktree));
    }
    Ok(if bare == Some(true) {
        CommonConfigAuthority::Bare
    } else {
        CommonConfigAuthority::Unproven
    })
}

fn unique_explicit_value(
    config: &gix::config::File<'_>,
    section_name: &str,
    value_name: &str,
) -> Result<Option<Vec<u8>>, ()> {
    let Some(sections) = config.sections_by_name(section_name) else {
        return Ok(None);
    };
    let mut occurrence = None;
    for section in sections {
        if section.header().subsection_name().is_some() {
            return Err(());
        }
        for name in section.value_names() {
            let name: &str = name.as_ref();
            if !name.eq_ignore_ascii_case(value_name) {
                continue;
            }
            if occurrence.is_some() {
                return Err(());
            }
            let value = section
                .value_implicit(name)
                .ok_or(())?
                .map(|value| value.as_ref().to_vec());
            occurrence = Some(value);
        }
    }
    match occurrence {
        None => Ok(None),
        Some(Some(value)) => Ok(Some(value)),
        Some(None) => Err(()),
    }
}
