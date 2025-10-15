use codex_core::config::Config;
use std::path::Path;

const MAX_TITLE_CHARS: usize = 80;

#[derive(Debug)]
pub(crate) struct TerminalTitleGuard {
    #[cfg(target_os = "macos")]
    restore_title: String,
}

pub(crate) fn maybe_set_terminal_title(
    config: &Config,
    active_profile: Option<&str>,
) -> Option<TerminalTitleGuard> {
    let title = format_terminal_title(&config.cwd, active_profile);
    TerminalTitleGuard::new(title)
}

impl TerminalTitleGuard {
    #[cfg(target_os = "macos")]
    fn new(title: String) -> Option<Self> {
        use std::io::IsTerminal as _;

        if !std::io::stdout().is_terminal() {
            return None;
        }

        if set_terminal_title_raw(&title).is_err() {
            return None;
        }

        Some(Self {
            restore_title: default_restore_title(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    #[allow(clippy::unnecessary_wraps)]
    fn new(_title: String) -> Option<Self> {
        None
    }
}

impl Drop for TerminalTitleGuard {
    #[cfg(target_os = "macos")]
    fn drop(&mut self) {
        use std::io::IsTerminal as _;

        if std::io::stdout().is_terminal() {
            let _ = set_terminal_title_raw(&self.restore_title);
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn drop(&mut self) {}
}

pub(crate) fn format_terminal_title(project_root: &Path, active_profile: Option<&str>) -> String {
    let project_segment = project_title_segment(project_root);
    let profile_segment = active_profile.and_then(sanitize_component);

    let mut title = String::from("Codex");

    if let Some(ref project) = project_segment {
        title.push_str(" — ");
        title.push_str(project);
    }

    if let Some(ref profile) = profile_segment {
        title.push(' ');
        title.push('[');
        title.push_str(profile);
        title.push(']');
    }

    truncate_title(title)
}

fn project_title_segment(path: &Path) -> Option<String> {
    let candidate = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            let display = path.to_string_lossy();
            if display.is_empty() {
                None
            } else {
                Some(display.into_owned())
            }
        })?;

    sanitize_component(&candidate)
}

fn sanitize_component(component: &str) -> Option<String> {
    let trimmed = component.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut sanitized = String::with_capacity(trimmed.len());
    let mut replaced_any = false;

    for ch in trimmed.chars() {
        if ch.is_control() {
            sanitized.push('?');
            replaced_any = true;
        } else {
            sanitized.push(ch);
        }
    }

    let result = if replaced_any {
        sanitized
    } else {
        trimmed.to_owned()
    };

    if result.chars().all(|ch| ch == '?') {
        None
    } else {
        Some(result)
    }
}

fn truncate_title(title: String) -> String {
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title;
    }

    let truncated: String = title.chars().take(MAX_TITLE_CHARS - 1).collect();
    format!("{truncated}…")
}

#[cfg(target_os = "macos")]
fn set_terminal_title_raw(title: &str) -> std::io::Result<()> {
    use std::io::Write as _;

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(b"\x1b]0;")?;
    stdout.write_all(title.as_bytes())?;
    stdout.write_all(b"\x07")?;
    stdout.flush()
}

#[cfg(target_os = "macos")]
fn default_restore_title() -> String {
    use std::env;
    use std::path::PathBuf;

    let candidate = env::args_os().next().and_then(|arg0| {
        let path = PathBuf::from(arg0);
        let file_name = path.file_name()?.to_string_lossy().into_owned();
        sanitize_component(&file_name)
    });

    truncate_title(candidate.unwrap_or_else(|| "codex".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_project_only() {
        let title = format_terminal_title(Path::new("/Users/alice/project"), None);
        assert_eq!(title, "Codex — project");
    }

    #[test]
    fn formats_project_and_profile() {
        let title = format_terminal_title(Path::new("/Users/alice/project"), Some("work"));
        assert_eq!(title, "Codex — project [work]");
    }

    #[test]
    fn sanitizes_control_characters() {
        let title = format_terminal_title(Path::new("/tmp/foo\nbar"), Some(" profile\n"));
        assert_eq!(title, "Codex — foo?bar [profile]");
    }

    #[test]
    fn truncates_long_titles() {
        let long_name = "a".repeat(100);
        let path = format!("/tmp/{long_name}");
        let title = format_terminal_title(Path::new(&path), None);
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.ends_with('…'));
    }
}
