#[test]
fn test_readme_deps() {
    version_sync::assert_markdown_deps_updated!("README.md");
}

#[test]
fn test_changelog() {
    version_sync::assert_contains_regex!(
        "CHANGELOG.md",
        r"^## Version {version} \(20\d\d-\d\d-\d\d\)"
    );
}

#[test]
fn test_html_root_url() {
    version_sync::assert_html_root_url_updated!("src/lib.rs");
}

#[test]
fn test_dependency_graph() {
    version_sync::assert_contains_regex!("src/lib.rs", "master/images/textwrap-{version}.svg");
}
