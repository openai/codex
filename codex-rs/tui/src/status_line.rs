use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;
use tokio::process::Command;
use tokio::time::timeout;

use crate::tui::FrameRequester;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathStyle {
    Auto,
    Absolute,
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GitShow {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, Default)]
struct GitState {
    in_repo: bool,
    branch: Option<String>,
    repo_root: Option<PathBuf>,
    cwd: PathBuf,
}

/// Lightweight status line that shows current path and git branch.
/// Rendered under the composer and above the key hints.
pub(crate) struct StatusLineWidget {
    enabled: bool,
    path_style: PathStyle,
    git_show: GitShow,
    no_color: bool,
    state: Arc<Mutex<GitState>>,
}

impl StatusLineWidget {
    pub(crate) fn new(frame_requester: FrameRequester) -> Self {
        let default_enabled = cfg!(not(test));
        let enabled_env = std::env::var("CODEX_STATUS_LINE").ok();
        let env_enabled = match enabled_env.as_deref() {
            Some("off") | Some("0") => false,
            Some("on") | Some("1") => true,
            _ => default_enabled,
        };
        let enabled = env_enabled && !STATUS_LINE_FORCE_DISABLE.load(Ordering::Relaxed);
        let path_style = match std::env::var("CODEX_STATUS_PATH_STYLE") {
            Ok(v) if v.eq_ignore_ascii_case("absolute") => PathStyle::Absolute,
            Ok(v) if v.eq_ignore_ascii_case("relative") => PathStyle::Relative,
            _ => PathStyle::Auto,
        };
        let git_show = match std::env::var("CODEX_STATUS_GIT_SHOW") {
            Ok(v) if v.eq_ignore_ascii_case("always") => GitShow::Always,
            Ok(v) if v.eq_ignore_ascii_case("never") => GitShow::Never,
            _ => GitShow::Auto,
        };
        let refresh_ms = std::env::var("CODEX_STATUS_REFRESH_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2000);
        let no_color = std::env::var_os("NO_COLOR").is_some();

        let state = Arc::new(Mutex::new(GitState {
            in_repo: false,
            branch: None,
            repo_root: None,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }));

        let widget = Self {
            enabled,
            path_style,
            git_show,
            no_color,
            state: state.clone(),
        };

        if enabled {
            // Spawn background refresher.
            tokio::spawn(async move {
                loop {
                    refresh_git_state(&state).await;
                    // Request a frame so the UI repaints if anything changed.
                    frame_requester.schedule_frame();
                    tokio::time::sleep(Duration::from_millis(refresh_ms)).await;
                }
            });
        }

        widget
    }

    pub fn desired_height(&self) -> u16 {
        if self.enabled { 1 } else { 0 }
    }
}

impl WidgetRef for StatusLineWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if !self.enabled || area.is_empty() {
            return;
        }

        let Ok(guard) = self.state.lock() else {
            return;
        };
        let state = guard.clone();

        // Determine display path: relative to repo root when applicable.
        let mut display_path = match self.path_style {
            PathStyle::Absolute => state.cwd.display().to_string(),
            PathStyle::Relative => match &state.repo_root {
                Some(root) if state.cwd.starts_with(root) => {
                    let rel = pathdiff::diff_paths(&state.cwd, root)
                        .unwrap_or_else(|| PathBuf::from("."));
                    let s = rel.display().to_string();
                    if s.is_empty() {
                        ".".to_string()
                    } else {
                        format!("/{s}")
                    }
                }
                _ => state.cwd.display().to_string(),
            },
            PathStyle::Auto => match &state.repo_root {
                Some(root) if state.cwd.starts_with(root) => {
                    let rel = pathdiff::diff_paths(&state.cwd, root)
                        .unwrap_or_else(|| PathBuf::from("."));
                    let s = rel.display().to_string();
                    if s.is_empty() {
                        ".".to_string()
                    } else {
                        format!("/{s}")
                    }
                }
                _ => state.cwd.display().to_string(),
            },
        };

        // Conservative width budget for the status row.
        let max_path = area.width.saturating_sub(10) as usize;
        display_path = shorten_mid(&display_path, max_path.clamp(20, 60));

        let mut spans: Vec<Span<'static>> = Vec::new();
        if self.no_color {
            spans.push(Span::from(display_path));
        } else {
            spans.push(Span::from(display_path).cyan());
        }

        let show_branch = match self.git_show {
            GitShow::Never => false,
            GitShow::Always => true,
            GitShow::Auto => state.in_repo,
        } && state.branch.is_some();

        if show_branch {
            let branch = state.branch.unwrap_or_default();
            spans.push("  ".into());
            spans.push("|".dim());
            spans.push("  ".into());
            if self.no_color {
                spans.push(Span::from(branch));
            } else {
                spans.push(Span::from(branch).blue());
            }
        }

        Line::from(spans).render_ref(area, buf);
    }
}

async fn refresh_git_state(state: &Arc<Mutex<GitState>>) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Try to get repo root.
    let repo_root = match run_git_capture_stdout(&["rev-parse", "--show-toplevel"]).await {
        Ok(out) => Some(PathBuf::from(out.trim())),
        Err(_) => None,
    };
    // Try to get branch.
    let branch = match run_git_capture_stdout(&["rev-parse", "--abbrev-ref", "HEAD"]).await {
        Ok(out) => Some(out.trim().to_string()),
        Err(_) => None,
    };
    let in_repo = repo_root.is_some();

    let Ok(mut guard) = state.lock() else {
        return;
    };
    guard.cwd = cwd;
    guard.repo_root = repo_root;
    guard.branch = branch;
    guard.in_repo = in_repo;
}

async fn run_git_capture_stdout(args: &[&str]) -> std::io::Result<String> {
    let fut = Command::new("git").args(args).output();
    let out = match timeout(Duration::from_millis(800), fut).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(std::io::Error::other(format!("git {args:?} timed out"))),
    };
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(std::io::Error::other(format!(
            "git {args:?} failed with status {}",
            out.status
        )))
    }
}

fn shorten_mid(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return "...".to_string();
    }
    let keep = (max - 3) / 2;
    format!("{}...{}", &s[..keep], &s[s.len().saturating_sub(keep)..])
}

static STATUS_LINE_FORCE_DISABLE: AtomicBool = AtomicBool::new(false);

/// Disable the status line regardless of env configuration.
pub(crate) fn force_disable_status_line() {
    STATUS_LINE_FORCE_DISABLE.store(true, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn shorten_mid_truncates() {
        assert_eq!(shorten_mid("/a/b/c", 50), "/a/b/c");
        let s = shorten_mid("/this/is/a/very/long/path/segment/here", 20);
        assert!(s.len() <= 20, "got {} chars: {s}", s.len());
        assert!(s.contains("..."));
    }
}
