use codex_file_search as file_search;
use pretty_assertions::assert_eq;
use std::num::NonZero;

#[test]
fn ignores_do_not_apply_from_parent_directories() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;

    let parent = temp.path();
    std::fs::write(parent.join(".gitignore"), "child/\n")?;

    let child = parent.join("child");
    std::fs::create_dir_all(&child)?;
    std::fs::write(child.join("needle-visible.txt"), "ok")?;

    let limit = NonZero::new(50).unwrap();
    let threads = NonZero::new(2).unwrap();

    let res = file_search::run(
        "needle",
        limit,
        &child,
        Vec::new(),
        threads,
        Default::default(),
        false,
        true,
    )?;

    let paths: Vec<String> = res.matches.into_iter().map(|m| m.path).collect();
    assert_eq!(paths, vec!["needle-visible.txt"]);

    Ok(())
}

#[test]
fn gitignore_inside_root_is_still_respected() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();

    std::fs::write(root.join(".gitignore"), "needle-ignored.txt\n")?;
    std::fs::write(root.join("needle-visible.txt"), "ok")?;
    std::fs::write(root.join("needle-ignored.txt"), "ok")?;

    let limit = NonZero::new(50).unwrap();
    let threads = NonZero::new(2).unwrap();

    let res = file_search::run(
        "needle",
        limit,
        root,
        Vec::new(),
        threads,
        Default::default(),
        false,
        true,
    )?;

    let paths: Vec<String> = res.matches.into_iter().map(|m| m.path).collect();
    assert_eq!(paths, vec!["needle-visible.txt"]);

    Ok(())
}
