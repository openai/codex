use std::fmt;
use std::ops::Deref;

use gix_url::Scheme;
use gix_url::Url;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A git remote URL with authentication credentials removed.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
#[ts(type = "string")]
pub struct SanitizedGitUrl(String);

impl SanitizedGitUrl {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<&str> for SanitizedGitUrl {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // Remote helpers wrap another remote, so peel off their prefixes
        // without recursion before passing the address to the Git URL parser.
        let mut address = value;
        while let Some((transport, nested_address)) = address.split_once("::")
            && !transport.is_empty()
            && transport.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
        {
            // Helpers can also receive executable command lines containing
            // arbitrary secrets; scan the whole payload once and reject those.
            if address.len() == value.len() && nested_address.chars().any(char::is_whitespace) {
                return Err("invalid git remote URL");
            }
            address = nested_address;
        }
        let helper_prefix = &value[..value.len() - address.len()];
        let original = value;
        let value = address;

        // Parse first to recognize both URL and SCP-style remotes and reject
        // malformed inputs before reconstructing the original text.
        let url = match Url::try_from(value) {
            Ok(url) => url,
            Err(_) => {
                // gix mistakes an IPv6 address's first colon for the SCP path
                // separator when a username comes before the bracketed host.
                let (user, host_and_path) =
                    value.split_once('@').ok_or("invalid git remote URL")?;
                let (host, path) = host_and_path
                    .strip_prefix('[')
                    .and_then(|host_and_path| host_and_path.split_once("]:"))
                    .ok_or("invalid git remote URL")?;
                if user.is_empty() || path.is_empty() {
                    return Err("invalid git remote URL");
                }
                // SCP paths are opaque and can contain characters that URL parsing rejects.
                let ssh_url = format!("ssh://{user}@[{host}]/");
                Url::try_from(ssh_url.as_str()).map_err(|_| "invalid git remote URL")?
            }
        };

        // SSH's conventional `git` user is a transport identity, not a secret;
        // every other username and all passwords must be removed.
        let preserve_ssh_git_user = url.scheme == Scheme::Ssh && url.user() == Some("git");

        // Inspect the authority directly because gix treats the complete
        // `file://` authority as its host and does not expose its userinfo.
        // Rebuilding only this part also preserves escaped repository paths.
        if let Some((scheme, authority_and_path)) = value.split_once("://") {
            // Git treats '?' and '#' as authority text on non-HTTP transports.
            let authority_end = if matches!(scheme, "http" | "https") {
                authority_and_path.find(['/', '?', '#'])
            } else {
                authority_and_path.find('/')
            }
            .unwrap_or(authority_and_path.len());
            let (authority, path) = authority_and_path.split_at(authority_end);
            let Some((_, host)) = authority.rsplit_once('@') else {
                return Ok(Self(original.to_owned()));
            };
            if preserve_ssh_git_user && url.password().is_none() {
                return Ok(Self(original.to_owned()));
            }
            let user = if preserve_ssh_git_user { "git@" } else { "" };
            return Ok(Self(format!(
                "{helper_prefix}{scheme}://{user}{host}{path}"
            )));
        }

        // Parsed serialization can alter Git paths, so preserve the exact
        // original representation whenever no credential needs removal.
        if url.password().is_none() && (url.user().is_none() || preserve_ssh_git_user) {
            return Ok(Self(original.to_owned()));
        }

        // SCP-style remotes have no URL authority; remove only `user@` and
        // leave the original `host:path` untouched.
        let authority_end = value.find(':').unwrap_or(value.len());
        let user_end = value[..authority_end]
            .rfind('@')
            .ok_or("invalid git remote URL")?;
        Ok(Self(format!("{helper_prefix}{}", &value[user_end + 1..])))
    }
}

impl TryFrom<String> for SanitizedGitUrl {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl From<SanitizedGitUrl> for String {
    fn from(value: SanitizedGitUrl) -> Self {
        value.0
    }
}

impl AsRef<str> for SanitizedGitUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for SanitizedGitUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for SanitizedGitUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(crate) fn deserialize_optional_sanitized_git_url<'de, D>(
    deserializer: D,
) -> Result<Option<SanitizedGitUrl>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Malformed legacy origins must not prevent the rest of a rollout from loading.
    Ok(Option::<String>::deserialize(deserializer)?
        .and_then(|value| SanitizedGitUrl::try_from(value).ok()))
}

#[cfg(test)]
#[path = "sanitized_git_url_tests.rs"]
mod tests;
