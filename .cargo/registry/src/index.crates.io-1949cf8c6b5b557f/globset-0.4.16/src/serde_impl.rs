use serde::{
    de::{Error, SeqAccess, Visitor},
    {Deserialize, Deserializer, Serialize, Serializer},
};

use crate::{Glob, GlobSet, GlobSetBuilder};

impl Serialize for Glob {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.glob())
    }
}

struct GlobVisitor;

impl<'de> Visitor<'de> for GlobVisitor {
    type Value = Glob;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("a glob pattern")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Glob::new(v).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Glob {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_str(GlobVisitor)
    }
}

struct GlobSetVisitor;

impl<'de> Visitor<'de> for GlobSetVisitor {
    type Value = GlobSet;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("an array of glob patterns")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut builder = GlobSetBuilder::new();
        while let Some(glob) = seq.next_element()? {
            builder.add(glob);
        }
        builder.build().map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for GlobSet {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(GlobSetVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{Glob, GlobSet};

    #[test]
    fn glob_deserialize_borrowed() {
        let string = r#"{"markdown": "*.md"}"#;

        let map: HashMap<String, Glob> =
            serde_json::from_str(&string).unwrap();
        assert_eq!(map["markdown"], Glob::new("*.md").unwrap());
    }

    #[test]
    fn glob_deserialize_owned() {
        let string = r#"{"markdown": "*.md"}"#;

        let v: serde_json::Value = serde_json::from_str(&string).unwrap();
        let map: HashMap<String, Glob> = serde_json::from_value(v).unwrap();
        assert_eq!(map["markdown"], Glob::new("*.md").unwrap());
    }

    #[test]
    fn glob_deserialize_error() {
        let string = r#"{"error": "["}"#;

        let map = serde_json::from_str::<HashMap<String, Glob>>(&string);

        assert!(map.is_err());
    }

    #[test]
    fn glob_json_works() {
        let test_glob = Glob::new("src/**/*.rs").unwrap();

        let ser = serde_json::to_string(&test_glob).unwrap();
        assert_eq!(ser, "\"src/**/*.rs\"");

        let de: Glob = serde_json::from_str(&ser).unwrap();
        assert_eq!(test_glob, de);
    }

    #[test]
    fn glob_set_deserialize() {
        let j = r#" ["src/**/*.rs", "README.md"] "#;
        let set: GlobSet = serde_json::from_str(j).unwrap();
        assert!(set.is_match("src/lib.rs"));
        assert!(!set.is_match("Cargo.lock"));
    }
}
