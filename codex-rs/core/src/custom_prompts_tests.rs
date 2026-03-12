use super::*;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn empty_when_dir_missing() {
    let tmp = tempdir().expect("create TempDir");
    let missing = tmp.path().join("nope");
    let found = discover_prompts_in(&missing).await;
    assert!(found.is_empty());
}

#[tokio::test]
async fn discovers_and_sorts_files() {
    let tmp = tempdir().expect("create TempDir");
    let dir = tmp.path();
    fs::write(dir.join("b.md"), b"b").unwrap();
    fs::write(dir.join("a.md"), b"a").unwrap();
    fs::create_dir(dir.join("subdir")).unwrap();
    let found = discover_prompts_in(dir).await;
    let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[tokio::test]
async fn excludes_builtins() {
    let tmp = tempdir().expect("create TempDir");
    let dir = tmp.path();
    fs::write(dir.join("init.md"), b"ignored").unwrap();
    fs::write(dir.join("foo.md"), b"ok").unwrap();
    let mut exclude = HashSet::new();
    exclude.insert("init".to_string());
    let found = discover_prompts_in_excluding(dir, &exclude).await;
    let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["foo"]);
}

#[tokio::test]
async fn skips_non_utf8_files() {
    let tmp = tempdir().expect("create TempDir");
    let dir = tmp.path();
    // Valid UTF-8 file
    fs::write(dir.join("good.md"), b"hello").unwrap();
    // Invalid UTF-8 content in .md file (e.g., lone 0xFF byte)
    fs::write(dir.join("bad.md"), vec![0xFF, 0xFE, b'\n']).unwrap();
    let found = discover_prompts_in(dir).await;
    let names: Vec<String> = found.into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["good"]);
}

#[tokio::test]
#[cfg(unix)]
async fn discovers_symlinked_md_files() {
    let tmp = tempdir().expect("create TempDir");
    let dir = tmp.path();

    // Create a real file
    fs::write(dir.join("real.md"), b"real content").unwrap();

    // Create a symlink to the real file
    std::os::unix::fs::symlink(dir.join("real.md"), dir.join("link.md")).unwrap();

    let found = discover_prompts_in(dir).await;
    let names: Vec<String> = found.into_iter().map(|e| e.name).collect();

    // Both real and link should be discovered, sorted alphabetically
    assert_eq!(names, vec!["link", "real"]);
}

#[tokio::test]
async fn parses_frontmatter_and_strips_from_body() {
    let tmp = tempdir().expect("create TempDir");
    let dir = tmp.path();
    let file = dir.join("withmeta.md");
    let text = "---\nname: ignored\ndescription: \"Quick review command\"\nargument-hint: \"[file] [priority]\"\n---\nActual body with $1 and $ARGUMENTS";
    fs::write(&file, text).unwrap();

    let found = discover_prompts_in(dir).await;
    assert_eq!(found.len(), 1);
    let p = &found[0];
    assert_eq!(p.name, "withmeta");
    assert_eq!(p.description.as_deref(), Some("Quick review command"));
    assert_eq!(p.argument_hint.as_deref(), Some("[file] [priority]"));
    // Body should not include the frontmatter delimiters.
    assert_eq!(p.content, "Actual body with $1 and $ARGUMENTS");
}

#[test]
fn parse_frontmatter_preserves_body_newlines() {
    let content = "---\r\ndescription: \"Line endings\"\r\nargument_hint: \"[arg]\"\r\n---\r\nFirst line\r\nSecond line\r\n";
    let (desc, hint, body) = parse_frontmatter(content);
    assert_eq!(desc.as_deref(), Some("Line endings"));
    assert_eq!(hint.as_deref(), Some("[arg]"));
    assert_eq!(body, "First line\r\nSecond line\r\n");
}
