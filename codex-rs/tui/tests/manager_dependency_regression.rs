#![allow(clippy::expect_used)]
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).expect("source directory should be readable");
    for entry in entries {
        let entry = entry.expect("source directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_sources_under(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[test]
fn tui_runtime_source_does_not_depend_on_manager_escape_hatches() {
    let src_file = codex_utils_cargo_bin::find_resource!("src/chatwidget.rs")
        .expect("chatwidget source runfile should resolve");
    let src_dir = src_file
        .parent()
        .expect("chatwidget source file should have a parent");
    let sources = rust_sources_under(src_dir);
    let forbidden = [
        "AuthManager",
        "ThreadManager",
        "auth_manager(",
        "thread_manager(",
    ];

    let violations: Vec<String> = sources
        .iter()
        .flat_map(|path| {
            let contents = fs::read_to_string(path).expect("Rust source file should be readable");
            let path_display = path.display().to_string();
            forbidden
                .iter()
                .filter(move |needle| contents.contains(**needle))
                .map(move |needle| format!("{path_display} contains `{needle}`"))
        })
        .collect();

    assert!(
        violations.is_empty(),
        "unexpected manager dependency regression(s):\n{}",
        violations.join("\n")
    );
}
