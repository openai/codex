//! Compatibility coverage for bundled theme files and existing user overrides.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn model_theme_files_preserve_custom_precedence_and_invalid_file_warnings() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join("themes")).unwrap();
    let path = custom_theme_path("ada", home.path());
    std::fs::write(&path, model_themes::THEMES[1].1).unwrap();
    assert_eq!(
        resolve_theme_by_name("ada", Some(home.path())),
        resolve_theme_by_name("babbage", /*codex_home*/ None)
    );
    let entries = list_available_themes(Some(home.path()));
    let entries = entries
        .iter()
        .filter(|entry| entry.name == "ada")
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_custom);

    let uppercase_path = custom_theme_path("Ada", home.path());
    std::fs::rename(&path, &uppercase_path).unwrap();
    let entries = list_available_themes(Some(home.path()));
    let lowercase = entries.iter().find(|entry| entry.name == "ada").unwrap();
    assert_eq!(lowercase.is_custom, path.exists());
    assert!(
        entries
            .iter()
            .any(|entry| entry.name == "Ada" && entry.is_custom)
    );
    std::fs::rename(&uppercase_path, &path).unwrap();

    std::fs::write(&path, "invalid theme").unwrap();
    assert!(
        validate_theme_name(Some("ada"), Some(home.path()))
            .unwrap()
            .contains("could not be loaded")
    );
    assert!(resolve_theme_by_name("ada", Some(home.path())).is_none());
    assert_eq!(
        resolve_theme_with_override(Some("ada"), Some(home.path())),
        resolve_theme_with_override(/*name*/ None, Some(home.path()))
    );
    assert!(
        !list_available_themes(Some(home.path()))
            .iter()
            .any(|entry| entry.name == "ada")
    );
}
