//! Compiled hook matchers retain their original spelling for configuration identity.
//! Regex parsing happens during discovery; dispatch only evaluates admitted matchers.

#[derive(Debug, Clone)]
pub(crate) enum HookMatcher {
    All(String),
    Exact(String),
    Regex(regex::Regex),
}

impl HookMatcher {
    pub(crate) fn new(pattern: &str) -> Result<Self, regex::Error> {
        if pattern.is_empty() || pattern == "*" {
            Ok(Self::All(pattern.to_string()))
        } else if pattern
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '|')
        {
            Ok(Self::Exact(pattern.to_string()))
        } else {
            regex::Regex::new(pattern).map(Self::Regex)
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::All(pattern) | Self::Exact(pattern) => pattern,
            Self::Regex(regex) => regex.as_str(),
        }
    }

    pub(crate) fn matches(&self, input: Option<&str>) -> bool {
        match self {
            Self::All(_) => true,
            Self::Exact(pattern) => {
                input.is_some_and(|input| pattern.split('|').any(|candidate| candidate == input))
            }
            Self::Regex(regex) => input.is_some_and(|input| regex.is_match(input)),
        }
    }
}

impl PartialEq for HookMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for HookMatcher {}
