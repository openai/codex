use super::MarketplaceAddError;
use crate::plugins::validate_marketplace_root;
use crate::plugins::validate_plugin_segment;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MarketplaceSource {
    Git {
        url: String,
        ref_name: Option<String>,
    },
}

pub(super) fn parse_marketplace_source(
    source: &str,
    explicit_ref: Option<String>,
) -> Result<MarketplaceSource, MarketplaceAddError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MarketplaceAddError::InvalidRequest(
            "marketplace source must not be empty".to_string(),
        ));
    }

    let (base_source, parsed_ref) = split_source_ref(source);
    let ref_name = explicit_ref.or(parsed_ref);

    if looks_like_local_path(&base_source) {
        return Err(MarketplaceAddError::InvalidRequest(
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo".to_string(),
        ));
    }

    if is_ssh_git_url(&base_source) || is_git_url(&base_source) {
        return Ok(MarketplaceSource::Git {
            url: normalize_git_url(&base_source),
            ref_name,
        });
    }

    if looks_like_github_shorthand(&base_source) {
        return Ok(MarketplaceSource::Git {
            url: format!("https://github.com/{base_source}.git"),
            ref_name,
        });
    }

    Err(MarketplaceAddError::InvalidRequest(format!(
        "invalid marketplace source format: {source}"
    )))
}

pub(super) fn validate_marketplace_source_root(root: &Path) -> Result<String, MarketplaceAddError> {
    let marketplace_name = validate_marketplace_root(root)
        .map_err(|err| MarketplaceAddError::InvalidRequest(err.to_string()))?;
    validate_plugin_segment(&marketplace_name, "marketplace name")
        .map_err(MarketplaceAddError::InvalidRequest)?;
    Ok(marketplace_name)
}

fn split_source_ref(source: &str) -> (String, Option<String>) {
    if let Some((base, ref_name)) = source.rsplit_once('#') {
        return (base.to_string(), non_empty_ref(ref_name));
    }
    if !source.contains("://")
        && !is_ssh_git_url(source)
        && let Some((base, ref_name)) = source.rsplit_once('@')
    {
        return (base.to_string(), non_empty_ref(ref_name));
    }
    (source.to_string(), None)
}

fn non_empty_ref(ref_name: &str) -> Option<String> {
    let ref_name = ref_name.trim();
    (!ref_name.is_empty()).then(|| ref_name.to_string())
}

fn normalize_git_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.starts_with("https://github.com/") && !url.ends_with(".git") {
        format!("{url}.git")
    } else {
        url.to_string()
    }
}

fn looks_like_local_path(source: &str) -> bool {
    source.starts_with("./")
        || source.starts_with("../")
        || source.starts_with('/')
        || source.starts_with("~/")
        || source == "."
        || source == ".."
}

fn is_ssh_git_url(source: &str) -> bool {
    source.starts_with("ssh://") || source.starts_with("git@") && source.contains(':')
}

fn is_git_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn looks_like_github_shorthand(source: &str) -> bool {
    let mut segments = source.split('/');
    let owner = segments.next();
    let repo = segments.next();
    let extra = segments.next();
    owner.is_some_and(is_github_shorthand_segment)
        && repo.is_some_and(is_github_shorthand_segment)
        && extra.is_none()
}

fn is_github_shorthand_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

impl MarketplaceSource {
    pub(super) fn display(&self) -> String {
        match self {
            Self::Git { url, ref_name } => match ref_name {
                Some(ref_name) => format!("{url}#{ref_name}"),
                None => url.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn github_shorthand_parses_ref_suffix() {
        assert_eq!(
            parse_marketplace_source("owner/repo@main", /*explicit_ref*/ None).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }

    #[test]
    fn git_url_parses_fragment_ref() {
        assert_eq!(
            parse_marketplace_source(
                "https://example.com/team/repo.git#v1",
                /*explicit_ref*/ None
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "https://example.com/team/repo.git".to_string(),
                ref_name: Some("v1".to_string()),
            }
        );
    }

    #[test]
    fn explicit_ref_overrides_source_ref() {
        assert_eq!(
            parse_marketplace_source("owner/repo@main", Some("release".to_string())).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("release".to_string()),
            }
        );
    }

    #[test]
    fn github_shorthand_and_git_url_normalize_to_same_source() {
        let shorthand = parse_marketplace_source("owner/repo", /*explicit_ref*/ None).unwrap();
        let git_url = parse_marketplace_source(
            "https://github.com/owner/repo.git",
            /*explicit_ref*/ None,
        )
        .unwrap();

        assert_eq!(shorthand, git_url);
        assert_eq!(
            shorthand,
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn github_url_with_trailing_slash_normalizes_without_extra_path_segment() {
        assert_eq!(
            parse_marketplace_source("https://github.com/owner/repo/", /*explicit_ref*/ None)
                .unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn non_github_https_source_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source("https://gitlab.com/owner/repo", /*explicit_ref*/ None)
                .unwrap(),
            MarketplaceSource::Git {
                url: "https://gitlab.com/owner/repo".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn file_url_source_is_rejected() {
        let err =
            parse_marketplace_source("file:///tmp/marketplace.git", /*explicit_ref*/ None)
                .unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid marketplace source format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_marketplace_source_rejects_local_directory_source() {
        let err = parse_marketplace_source("./marketplace", /*explicit_ref*/ None).unwrap_err();

        assert_eq!(
            err.to_string(),
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo"
        );
    }

    #[test]
    fn ssh_url_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source(
                "ssh://git@github.com/owner/repo.git#main",
                /*explicit_ref*/ None,
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "ssh://git@github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }
}
