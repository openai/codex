use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
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
fn tui_app_server_runtime_source_does_not_depend_on_manager_escape_hatches() {
    let src_dir = codex_utils_cargo_bin::find_resource!("src")
        .unwrap_or_else(|err| panic!("failed to resolve src runfile: {err}"));
    let sources = rust_sources_under(&src_dir);
    let forbidden = [
        "AuthManager",
        "ThreadManager",
        "auth_manager(",
        "thread_manager(",
    ];

    let violations: Vec<String> = sources
        .iter()
        .flat_map(|path| {
            let contents = fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
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
