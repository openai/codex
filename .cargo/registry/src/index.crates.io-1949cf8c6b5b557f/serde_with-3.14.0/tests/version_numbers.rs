//! Test Cases

use version_sync::{assert_contains_regex, assert_html_root_url_updated};

#[test]
fn test_changelog() {
    assert_contains_regex!("CHANGELOG.md", r"## \[{version}\]");
}

#[test]
fn test_html_root_url() {
    assert_html_root_url_updated!("src/lib.rs");
}

#[test]
fn test_serde_with_macros_dependency() {
    version_sync::assert_contains_regex!(
        "../serde_with/Cargo.toml",
        r#"^serde_with_macros = .*? version = "={version}""#
    );
    version_sync::assert_contains_regex!("../Cargo.toml", r#"^version = "{version}""#);
}

/// Check that all docs.rs links point to the current version
///
/// Parse all docs.rs links in `*.rs` and `*.md` files and check that they point to the current version.
/// If a link should point to latest version this can be done by using `latest` in the version.
/// The `*` version specifier is not allowed.
///
/// Arguably this should be part of version-sync. There is an open issue for this feature:
/// <https://github.com/mgeisler/version-sync/issues/72>
#[test]
fn test_docs_rs_url_point_to_current_version() -> Result<(), Box<dyn std::error::Error>> {
    let pkg_name = env!("CARGO_PKG_NAME");
    let pkg_version = env!("CARGO_PKG_VERSION");

    let re = regex::Regex::new(&format!(
        "https?://docs.rs/{pkg_name}/((\\d[^/]+|\\*|latest))/"
    ))?;
    let mut error = false;

    for entry in glob::glob("**/*.rs")?.chain(glob::glob("**/README.md")?) {
        let entry = entry?;
        let content = std::fs::read_to_string(&entry)?;
        for (line_number, line) in content.split('\n').enumerate() {
            for capture in re.captures_iter(line) {
                match capture
                    .get(1)
                    .expect("Will exist if regex matches")
                    .as_str()
                {
                    "latest" => {}
                    version if version != pkg_version => {
                        error = true;
                        println!(
                            "{}:{} pkg_version is {} but found URL {}",
                            entry.display(),
                            line_number + 1,
                            pkg_version,
                            capture.get(0).expect("Group 0 always exists").as_str()
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    if error {
        panic!("Found wrong URLs in file(s)");
    } else {
        Ok(())
    }
}
