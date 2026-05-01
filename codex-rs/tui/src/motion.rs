//! Centralized motion primitives for the TUI.
//!
//! Callers choose an explicit reduced-motion fallback here instead of reaching
//! directly for time-varying spinner or shimmer helpers.

use std::time::Instant;

use ratatui::style::Stylize;
use ratatui::text::Span;

use crate::shimmer::shimmer_spans;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionMode {
    Animated,
    Reduced,
}

impl MotionMode {
    pub(crate) fn from_animations_enabled(animations_enabled: bool) -> Self {
        if animations_enabled {
            Self::Animated
        } else {
            Self::Reduced
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReducedMotionIndicator {
    Hidden,
    StaticBullet,
}

pub(crate) fn activity_indicator(
    start_time: Option<Instant>,
    motion_mode: MotionMode,
    reduced_motion_indicator: ReducedMotionIndicator,
) -> Option<Span<'static>> {
    match motion_mode {
        MotionMode::Animated => Some(animated_activity_indicator(start_time)),
        MotionMode::Reduced => match reduced_motion_indicator {
            ReducedMotionIndicator::Hidden => None,
            ReducedMotionIndicator::StaticBullet => Some("•".dim()),
        },
    }
}

pub(crate) fn shimmer_text(text: &str, motion_mode: MotionMode) -> Vec<Span<'static>> {
    match motion_mode {
        MotionMode::Animated => shimmer_spans(text),
        MotionMode::Reduced => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string().into()]
            }
        }
    }
}

fn animated_activity_indicator(start_time: Option<Instant>) -> Span<'static> {
    let elapsed = start_time.map(|st| st.elapsed()).unwrap_or_default();
    if supports_color::on_cached(supports_color::Stream::Stdout)
        .map(|level| level.has_16m)
        .unwrap_or(false)
    {
        shimmer_spans("•")
            .into_iter()
            .next()
            .unwrap_or_else(|| "•".into())
    } else {
        let blink_on = (elapsed.as_millis() / 600).is_multiple_of(2);
        if blink_on { "•".into() } else { "◦".dim() }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn reduced_motion_activity_indicator_uses_explicit_fallback() {
        assert_eq!(
            activity_indicator(
                /*start_time*/ None,
                MotionMode::Reduced,
                ReducedMotionIndicator::Hidden,
            ),
            None
        );
        assert_eq!(
            activity_indicator(
                /*start_time*/ None,
                MotionMode::Reduced,
                ReducedMotionIndicator::StaticBullet,
            ),
            Some("•".dim())
        );
    }

    #[test]
    fn reduced_motion_shimmer_text_is_plain_text() {
        assert_eq!(
            shimmer_text("Loading", MotionMode::Reduced),
            vec!["Loading".into()]
        );
        assert_eq!(
            shimmer_text("", MotionMode::Reduced),
            Vec::<Span<'static>>::new()
        );
    }

    #[test]
    fn animation_primitives_are_only_used_by_motion_module() {
        let direct_spinner = regex_lite::Regex::new(r"(^|[^A-Za-z0-9_])spinner\s*\(").unwrap();
        let direct_shimmer =
            regex_lite::Regex::new(r"(^|[^A-Za-z0-9_])shimmer_spans\s*\(").unwrap();
        let lib_rs = codex_utils_cargo_bin::find_resource!("src/lib.rs")
            .expect("failed to locate TUI source");
        let src_dir = lib_rs.parent().expect("lib.rs should have a parent");

        let mut source_files = Vec::new();
        collect_rust_files(src_dir, &mut source_files).expect("failed to collect TUI source files");

        let mut violations = Vec::new();
        for path in source_files {
            let relative_path = path
                .strip_prefix(src_dir)
                .expect("source file should be under src")
                .to_string_lossy()
                .replace('\\', "/");
            if animation_primitive_allowlisted_path(&relative_path) {
                continue;
            }

            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {relative_path}: {err}"));
            for (line_number, line) in contents.lines().enumerate() {
                let code = line.split_once("//").map_or(line, |(code, _)| code);
                if direct_spinner.is_match(code) {
                    violations.push(format!(
                        "{relative_path}:{} contains a direct `spinner(...)` call; use crate::motion instead",
                        line_number + 1
                    ));
                }
                if direct_shimmer.is_match(code) {
                    violations.push(format!(
                        "{relative_path}:{} contains a direct `shimmer_spans(...)` call; use crate::motion instead",
                        line_number + 1
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "direct animation primitive usage found:\n{}",
            violations.join("\n")
        );
    }

    fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_rust_files(&path, files)?;
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        Ok(())
    }

    fn animation_primitive_allowlisted_path(relative_path: &str) -> bool {
        matches!(relative_path, "motion.rs" | "shimmer.rs")
    }
}
