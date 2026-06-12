use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use pretty_assertions::assert_eq;

use super::has_ignored_component;

#[test]
fn detects_configured_ignored_path_components() {
    let ignored_components =
        BTreeSet::from([OsString::from(".git"), OsString::from(".watch-metadata")]);
    let actual = [
        has_ignored_component(
            Path::new(".agents/skills/.git/FETCH_HEAD"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/demo/.git/config"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/.watch-metadata/state"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/demo/.watch-metadata/cache"),
            &ignored_components,
        ),
    ];

    assert_eq!(actual, [true, true, true, true]);
}

#[test]
fn preserves_similarly_named_skill_paths() {
    let ignored_components =
        BTreeSet::from([OsString::from(".git"), OsString::from(".watch-metadata")]);
    let actual = [
        has_ignored_component(
            Path::new(".agents/skills/demo/SKILL.md"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/demo/.gitignore"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/demo/skill.git.md"),
            &ignored_components,
        ),
        has_ignored_component(
            Path::new(".agents/skills/demo/.watch-metadata-cache/script.py"),
            &ignored_components,
        ),
    ];

    assert_eq!(actual, [false, false, false, false]);
}
