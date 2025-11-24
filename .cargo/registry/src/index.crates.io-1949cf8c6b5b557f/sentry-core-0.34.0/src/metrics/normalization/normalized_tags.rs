use regex::Regex;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::OnceLock;

use crate::metrics::TagMap;

pub fn normalize_tags(tags: &TagMap) -> NormalizedTags {
    NormalizedTags {
        tags: tags
            .iter()
            .map(|(k, v)| {
                (
                    NormalizedTags::normalize_key(super::truncate(k, 32)),
                    NormalizedTags::normalize_value(super::truncate(v, 200)),
                )
            })
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect(),
    }
}

pub struct NormalizedTags<'a> {
    tags: BTreeMap<Cow<'a, str>, String>,
}

impl<'a> NormalizedTags<'a> {
    pub fn with_default_tags(mut self, tags: &'a TagMap) -> Self {
        for (k, v) in tags {
            let k = Self::normalize_key(super::truncate(k, 32));
            let v = Self::normalize_value(super::truncate(v, 200));
            if !k.is_empty() && !v.is_empty() {
                self.tags.entry(k).or_insert(v);
            }
        }
        self
    }

    fn normalize_key(key: &str) -> Cow<str> {
        static METRIC_TAG_KEY_RE: OnceLock<Regex> = OnceLock::new();
        METRIC_TAG_KEY_RE
            .get_or_init(|| Regex::new(r"[^a-zA-Z0-9_\-./]").expect("Regex should compile"))
            .replace_all(key, "")
    }

    fn normalize_value(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        for c in value.chars() {
            match c {
                '\t' => escaped.push_str("\\t"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\\' => escaped.push_str("\\\\"),
                '|' => escaped.push_str("\\u{7c}"),
                ',' => escaped.push_str("\\u{2c}"),
                _ if c.is_control() => (),
                _ => escaped.push(c),
            }
        }
        escaped
    }
}

impl std::fmt::Display for NormalizedTags<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, (k, v)) in self.tags.iter().enumerate() {
            if i > 0 {
                f.write_char(',')?;
            }
            write!(f, "{k}:{v}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::TagMap;

    #[test]
    fn test_replacement_characters() {
        let tags = TagMap::from_iter(
            [
                ("a\na", "a\na"),
                ("b\rb", "b\rb"),
                ("c\tc", "c\tc"),
                ("d\\d", "d\\d"),
                ("e|e", "e|e"),
                ("f,f", "f,f"),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "aa:a\\na,bb:b\\rb,cc:c\\tc,dd:d\\\\d,ee:e\\u{7c}e,ff:f\\u{2c}f";

        let actual = super::normalize_tags(&tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_empty_tags() {
        let tags = TagMap::from_iter(
            [("+", "a"), ("a", ""), ("", "a"), ("", "")]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into())),
        );
        let expected = "";

        let actual = super::normalize_tags(&tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_special_characters() {
        let tags = TagMap::from([("aA1_-./+ö{ 😀".into(), "aA1_-./+ö{ 😀".into())]);
        let expected = "aA1_-./:aA1_-./+ö{ 😀";

        let actual = super::normalize_tags(&tags).to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_add_default_tags() {
        let default_tags = TagMap::from([
            ("release".into(), "default_release".into()),
            ("environment".into(), "production".into()),
        ]);
        let expected = "environment:production,release:default_release";

        let actual = super::normalize_tags(&TagMap::new())
            .with_default_tags(&default_tags)
            .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_override_default_tags() {
        let default_tags = TagMap::from([
            ("release".into(), "default_release".into()),
            ("environment".into(), "production".into()),
        ]);
        let expected = "environment:custom_env,release:custom_release";

        let actual = super::normalize_tags(&TagMap::from([
            ("release".into(), "custom_release".into()),
            ("environment".into(), "custom_env".into()),
        ]))
        .with_default_tags(&default_tags)
        .to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_length_restriction() {
        let expected = "dk".repeat(16)
            + ":"
            + "dv".repeat(100).as_str()
            + ","
            + "k".repeat(32).as_str()
            + ":"
            + "v".repeat(200).as_str();

        let actual = super::normalize_tags(&TagMap::from([(
            "k".repeat(35).into(),
            "v".repeat(210).into(),
        )]))
        .with_default_tags(&TagMap::from([(
            "dk".repeat(35).into(),
            "dv".repeat(210).into(),
        )]))
        .to_string();

        assert_eq!(expected, actual);
    }
}
