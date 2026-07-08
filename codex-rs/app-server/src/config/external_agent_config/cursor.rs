use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use super::CURSOR_AGENT_DIR;
use super::CURSOR_LEGACY_RULES_FILE;

pub(super) fn rule_sources(repo_root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    let legacy_rules = repo_root.join(CURSOR_LEGACY_RULES_FILE);
    if is_non_empty_text_file(&legacy_rules)? {
        sources.push(legacy_rules);
    }

    let rules_dir = repo_root.join(CURSOR_AGENT_DIR).join("rules");
    if rules_dir.is_dir() {
        for entry in fs::read_dir(rules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file()
                || !matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("md" | "mdc")
                )
            {
                continue;
            }
            let contents = fs::read_to_string(&path)?;
            if rule_is_always_applied(&contents)
                && !strip_markdown_frontmatter(&contents).trim().is_empty()
            {
                sources.push(path);
            }
        }
    }
    sources.sort();
    Ok(sources)
}

fn rule_is_always_applied(contents: &str) -> bool {
    let Some(frontmatter) = markdown_frontmatter(contents) else {
        return false;
    };
    frontmatter.lines().any(|line| {
        line.split_once(':').is_some_and(|(key, value)| {
            key.trim() == "alwaysApply" && value.trim().eq_ignore_ascii_case("true")
        })
    })
}

fn markdown_frontmatter(contents: &str) -> Option<&str> {
    let contents = contents.strip_prefix("---")?;
    let contents = contents
        .strip_prefix("\r\n")
        .or_else(|| contents.strip_prefix('\n'))?;
    contents
        .find("\n---")
        .map(|frontmatter_end| &contents[..frontmatter_end])
}

pub(super) fn strip_markdown_frontmatter(contents: &str) -> &str {
    let Some(contents) = contents.strip_prefix("---") else {
        return contents;
    };
    let Some(contents) = contents
        .strip_prefix("\r\n")
        .or_else(|| contents.strip_prefix('\n'))
    else {
        return contents;
    };
    let Some(frontmatter_end) = contents.find("\n---") else {
        return contents;
    };
    contents[frontmatter_end + "\n---".len()..]
        .strip_prefix("\r\n")
        .or_else(|| contents[frontmatter_end + "\n---".len()..].strip_prefix('\n'))
        .unwrap_or(&contents[frontmatter_end + "\n---".len()..])
}

fn is_non_empty_text_file(path: &Path) -> io::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    Ok(!fs::read_to_string(path)?.trim().is_empty())
}
