use std::fmt::{self, Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Operating system version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Version {
    /// Unknown version.
    Unknown,
    /// Semantic version (major.minor.patch).
    Semantic(u64, u64, u64),
    /// Rolling version. Optionally contains the release date in the string format.
    Rolling(Option<String>),
    /// Custom version format.
    Custom(String),
}

impl Version {
    /// Constructs `VersionType` from the given string.
    ///
    /// Returns `VersionType::Unknown` if the string is empty. If it can be parsed as a semantic
    /// version, then `VersionType::Semantic`, otherwise `VersionType::Custom`.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::Version;
    ///
    /// let v = Version::from_string("custom");
    /// assert_eq!(Version::Custom("custom".to_owned()), v);
    ///
    /// let v = Version::from_string("1.2.3");
    /// assert_eq!(Version::Semantic(1, 2, 3), v);
    /// ```
    pub fn from_string<S: Into<String> + AsRef<str>>(s: S) -> Self {
        if s.as_ref().is_empty() {
            Self::Unknown
        } else if let Some((major, minor, patch)) = parse_version(s.as_ref()) {
            Self::Semantic(major, minor, patch)
        } else {
            Self::Custom(s.into())
        }
    }
}

impl Default for Version {
    fn default() -> Self {
        Version::Unknown
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Self::Unknown => f.write_str("Unknown"),
            Self::Semantic(major, minor, patch) => write!(f, "{major}.{minor}.{patch}"),
            Self::Rolling(ref date) => {
                let date = match date {
                    Some(date) => format!(" ({date})"),
                    None => "".to_owned(),
                };
                write!(f, "Rolling Release{date}")
            }
            Self::Custom(ref version) => write!(f, "{version}"),
        }
    }
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut iter = s.trim().split_terminator('.').fuse();

    let major = iter.next().and_then(|s| s.parse().ok())?;
    let minor = iter.next().unwrap_or("0").parse().ok()?;
    let patch = iter.next().unwrap_or("0").parse().ok()?;

    if iter.next().is_some() {
        return None;
    }

    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_semantic_version() {
        let data = [
            ("", None),
            ("version", None),
            ("1", Some((1, 0, 0))),
            ("1.", Some((1, 0, 0))),
            ("1.2", Some((1, 2, 0))),
            ("1.2.", Some((1, 2, 0))),
            ("1.2.3", Some((1, 2, 3))),
            ("1.2.3.", Some((1, 2, 3))),
            ("1.2.3.  ", Some((1, 2, 3))),
            ("   1.2.3.", Some((1, 2, 3))),
            ("   1.2.3.  ", Some((1, 2, 3))),
            ("1.2.3.4", None),
            ("1.2.3.4.5.6.7.8.9", None),
        ];

        for (s, expected) in &data {
            let result = parse_version(s);
            assert_eq!(expected, &result);
        }
    }

    #[test]
    fn from_string() {
        let custom_version = "some version";
        let data = [
            ("", Version::Unknown),
            ("1.2.3", Version::Semantic(1, 2, 3)),
            (custom_version, Version::Custom(custom_version.to_owned())),
        ];

        for (s, expected) in &data {
            let version = Version::from_string(*s);
            assert_eq!(expected, &version);
        }
    }

    #[test]
    fn default() {
        assert_eq!(Version::Unknown, Version::default());
    }

    #[test]
    fn display() {
        let data = [
            (Version::Unknown, "Unknown"),
            (Version::Semantic(1, 5, 0), "1.5.0"),
            (Version::Rolling(None), "Rolling Release"),
            (
                Version::Rolling(Some("date".to_owned())),
                "Rolling Release (date)",
            ),
        ];

        for (version, expected) in &data {
            assert_eq!(expected, &version.to_string());
        }
    }
}
