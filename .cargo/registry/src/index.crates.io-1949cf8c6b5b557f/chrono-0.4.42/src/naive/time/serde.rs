use super::NaiveTime;
use core::fmt;
use serde::{de, ser};

// TODO not very optimized for space (binary formats would want something better)
// TODO round-trip for general leap seconds (not just those with second = 60)

impl ser::Serialize for NaiveTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(&self)
    }
}

struct NaiveTimeVisitor;

impl de::Visitor<'_> for NaiveTimeVisitor {
    type Value = NaiveTime;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a formatted time string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

impl<'de> de::Deserialize<'de> for NaiveTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(NaiveTimeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::NaiveTime;

    #[test]
    fn test_serde_serialize() {
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_opt(0, 0, 0).unwrap()).ok(),
            Some(r#""00:00:00""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_milli_opt(0, 0, 0, 950).unwrap()).ok(),
            Some(r#""00:00:00.950""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_milli_opt(0, 0, 59, 1_000).unwrap()).ok(),
            Some(r#""00:00:60""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_opt(0, 1, 2).unwrap()).ok(),
            Some(r#""00:01:02""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_nano_opt(3, 5, 7, 98765432).unwrap()).ok(),
            Some(r#""03:05:07.098765432""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_opt(7, 8, 9).unwrap()).ok(),
            Some(r#""07:08:09""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveTime::from_hms_micro_opt(12, 34, 56, 789).unwrap()).ok(),
            Some(r#""12:34:56.000789""#.into())
        );
        let leap = NaiveTime::from_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap();
        assert_eq!(serde_json::to_string(&leap).ok(), Some(r#""23:59:60.999999999""#.into()));
    }

    #[test]
    fn test_serde_deserialize() {
        let from_str = serde_json::from_str::<NaiveTime>;

        assert_eq!(from_str(r#""00:00:00""#).ok(), Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
        assert_eq!(from_str(r#""0:0:0""#).ok(), Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()));
        assert_eq!(
            from_str(r#""00:00:00.950""#).ok(),
            Some(NaiveTime::from_hms_milli_opt(0, 0, 0, 950).unwrap())
        );
        assert_eq!(
            from_str(r#""0:0:0.95""#).ok(),
            Some(NaiveTime::from_hms_milli_opt(0, 0, 0, 950).unwrap())
        );
        assert_eq!(
            from_str(r#""00:00:60""#).ok(),
            Some(NaiveTime::from_hms_milli_opt(0, 0, 59, 1_000).unwrap())
        );
        assert_eq!(from_str(r#""00:01:02""#).ok(), Some(NaiveTime::from_hms_opt(0, 1, 2).unwrap()));
        assert_eq!(
            from_str(r#""03:05:07.098765432""#).ok(),
            Some(NaiveTime::from_hms_nano_opt(3, 5, 7, 98765432).unwrap())
        );
        assert_eq!(from_str(r#""07:08:09""#).ok(), Some(NaiveTime::from_hms_opt(7, 8, 9).unwrap()));
        assert_eq!(
            from_str(r#""12:34:56.000789""#).ok(),
            Some(NaiveTime::from_hms_micro_opt(12, 34, 56, 789).unwrap())
        );
        assert_eq!(
            from_str(r#""23:59:60.999999999""#).ok(),
            Some(NaiveTime::from_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap())
        );
        assert_eq!(
            from_str(r#""23:59:60.9999999999997""#).ok(), // excess digits are ignored
            Some(NaiveTime::from_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap())
        );

        // bad formats
        assert!(from_str(r#""""#).is_err());
        assert!(from_str(r#""000000""#).is_err());
        assert!(from_str(r#""00:00:61""#).is_err());
        assert!(from_str(r#""00:60:00""#).is_err());
        assert!(from_str(r#""24:00:00""#).is_err());
        assert!(from_str(r#""23:59:59,1""#).is_err());
        assert!(from_str(r#""012:34:56""#).is_err());
        assert!(from_str(r#""hh:mm:ss""#).is_err());
        assert!(from_str(r#"0"#).is_err());
        assert!(from_str(r#"86399"#).is_err());
        assert!(from_str(r#"{}"#).is_err());
    }

    #[test]
    fn test_serde_bincode() {
        // Bincode is relevant to test separately from JSON because
        // it is not self-describing.
        use bincode::{deserialize, serialize};

        let t = NaiveTime::from_hms_nano_opt(3, 5, 7, 98765432).unwrap();
        let encoded = serialize(&t).unwrap();
        let decoded: NaiveTime = deserialize(&encoded).unwrap();
        assert_eq!(t, decoded);
    }
}
