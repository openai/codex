use syn::Ident;

use crate::util::IdentString;

impl<'de> serde::Deserialize<'de> for IdentString {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(IdentStringVisitor)
    }
}

struct IdentStringVisitor;

impl<'de> serde::de::Visitor<'de> for IdentStringVisitor {
    type Value = IdentString;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "a valid ident")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let ident: Ident = syn::parse_str(v).map_err(serde::de::Error::custom)?;
        Ok(IdentString::new(ident))
    }
}

impl serde::Serialize for IdentString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let raw_ident = r#""ident""#;
        let deserialized: IdentString =
            serde_json::from_str(&raw_ident).expect("ident must be valid");
        let serialized = serde_json::to_string(&deserialized).expect("ident must be serializable");

        assert_eq!(raw_ident, serialized);
    }
}
