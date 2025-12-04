use std::{fs, path::Path};

use tree_sitter_loader::Loader;

use crate::tests::helpers::fixtures::scratch_dir;

#[test]
fn detect_language_by_first_line_regex() {
    let strace_dir = tree_sitter_dir(
        r#"{
  "grammars": [
    {
      "name": "strace",
      "path": ".",
      "scope": "source.strace",
      "file-types": [
        "strace"
      ],
      "first-line-regex":  "[0-9:.]* *execve"
    }
  ],
  "metadata": {
    "version": "0.0.1"
  }
}
"#,
        "strace",
    );

    let mut loader = Loader::with_parser_lib_path(scratch_dir().to_path_buf());
    let config = loader
        .find_language_configurations_at_path(strace_dir.path(), false)
        .unwrap();

    // this is just to validate that we can read the tree-sitter.json correctly
    assert_eq!(config[0].scope.as_ref().unwrap(), "source.strace");

    let file_name = strace_dir.path().join("strace.log");
    fs::write(&file_name, "execve\nworld").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.strace".into())
    );

    let file_name = strace_dir.path().join("strace.log");
    fs::write(&file_name, "447845 execve\nworld").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.strace".into())
    );

    let file_name = strace_dir.path().join("strace.log");
    fs::write(&file_name, "hello\nexecve").unwrap();
    assert!(get_lang_scope(&loader, &file_name).is_none());

    let file_name = strace_dir.path().join("strace.log");
    fs::write(&file_name, "").unwrap();
    assert!(get_lang_scope(&loader, &file_name).is_none());

    let dummy_dir = tree_sitter_dir(
        r#"{
  "grammars": [
    {
      "name": "dummy",
      "scope": "source.dummy",
      "path": ".",
      "file-types": [
        "dummy"
      ]
    }
  ],
  "metadata": {
    "version": "0.0.1"
  }
}
"#,
        "dummy",
    );

    // file-type takes precedence over first-line-regex
    loader
        .find_language_configurations_at_path(dummy_dir.path(), false)
        .unwrap();
    let file_name = dummy_dir.path().join("strace.dummy");
    fs::write(&file_name, "execve").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.dummy".into())
    );
}

#[test]
fn detect_langauge_by_double_barrel_file_extension() {
    let blade_dir = tree_sitter_dir(
        r#"{
  "grammars": [
    {
      "name": "blade",
      "path": ".",
      "scope": "source.blade",
      "file-types": [
        "blade.php"
      ]
    }
  ],
  "metadata": {
    "version": "0.0.1"
  }
}
"#,
        "blade",
    );

    let mut loader = Loader::with_parser_lib_path(scratch_dir().to_path_buf());
    let config = loader
        .find_language_configurations_at_path(blade_dir.path(), false)
        .unwrap();

    // this is just to validate that we can read the tree-sitter.json correctly
    assert_eq!(config[0].scope.as_ref().unwrap(), "source.blade");

    let file_name = blade_dir.path().join("foo.blade.php");
    fs::write(&file_name, "").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.blade".into())
    );
}

#[test]
fn detect_language_without_filename() {
    let gitignore_dir = tree_sitter_dir(
        r#"{
  "grammars": [
    {
      "name": "gitignore",
      "path": ".",
      "scope": "source.gitignore",
      "file-types": [
        ".gitignore"
      ]
    }
  ],
  "metadata": {
    "version": "0.0.1"
  }
}
"#,
        "gitignore",
    );

    let mut loader = Loader::with_parser_lib_path(scratch_dir().to_path_buf());
    let config = loader
        .find_language_configurations_at_path(gitignore_dir.path(), false)
        .unwrap();

    // this is just to validate that we can read the tree-sitter.json correctly
    assert_eq!(config[0].scope.as_ref().unwrap(), "source.gitignore");

    let file_name = gitignore_dir.path().join(".gitignore");
    fs::write(&file_name, "").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.gitignore".into())
    );
}

#[test]
fn detect_language_without_file_extension() {
    let ssh_config_dir = tree_sitter_dir(
        r#"{
  "grammars": [
    {
      "name": "ssh_config",
      "path": ".",
      "scope": "source.ssh_config",
      "file-types": [
        "ssh_config"
      ]
    }
  ],
  "metadata": {
    "version": "0.0.1"
  }
}
"#,
        "ssh_config",
    );

    let mut loader = Loader::with_parser_lib_path(scratch_dir().to_path_buf());
    let config = loader
        .find_language_configurations_at_path(ssh_config_dir.path(), false)
        .unwrap();

    // this is just to validate that we can read the tree-sitter.json correctly
    assert_eq!(config[0].scope.as_ref().unwrap(), "source.ssh_config");

    let file_name = ssh_config_dir.path().join("ssh_config");
    fs::write(&file_name, "").unwrap();
    assert_eq!(
        get_lang_scope(&loader, &file_name),
        Some("source.ssh_config".into())
    );
}

fn tree_sitter_dir(tree_sitter_json: &str, name: &str) -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    fs::write(temp_dir.path().join("tree-sitter.json"), tree_sitter_json).unwrap();
    fs::create_dir_all(temp_dir.path().join("src/tree_sitter")).unwrap();
    fs::write(
        temp_dir.path().join("src/grammar.json"),
        format!(r#"{{"name":"{name}"}}"#),
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("src/parser.c"),
        format!(
            r#"
                #include "tree_sitter/parser.h"
                #ifdef _WIN32
                #define TS_PUBLIC __declspec(dllexport)
                #else
                #define TS_PUBLIC __attribute__((visibility("default")))
                #endif
                TS_PUBLIC const TSLanguage *tree_sitter_{name}() {{}}
            "#
        ),
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("src/tree_sitter/parser.h"),
        include_str!("../../../lib/src/parser.h"),
    )
    .unwrap();
    temp_dir
}

// If we manage to get the language scope, it means we correctly detected the file-type
fn get_lang_scope(loader: &Loader, file_name: &Path) -> Option<String> {
    loader
        .language_configuration_for_file_name(file_name)
        .ok()
        .and_then(|config| {
            if let Some((_, config)) = config {
                config.scope.clone()
            } else if let Ok(Some((_, config))) =
                loader.language_configuration_for_first_line_regex(file_name)
            {
                config.scope.clone()
            } else {
                None
            }
        })
}
