#![cfg_attr(not(feature = "protocol"), allow(unused))]

/// Helper macro to implement string based serialization.
///
/// If a type implements `Display` then this automatically
/// implements a serializer for that type that dispatches
/// appropriately.
macro_rules! impl_str_ser {
    ($type:ty) => {
        impl ::serde::ser::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }
    };
}

/// Helper macro to implement string based deserialization.
///
/// If a type implements `FromStr` then this automatically
/// implements a deserializer for that type that dispatches
/// appropriately.
#[allow(unused_macros)]
macro_rules! impl_str_de {
    ($type:ty) => {
        impl<'de> ::serde::de::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                <::std::borrow::Cow<str>>::deserialize(deserializer)?
                    .parse()
                    .map_err(::serde::de::Error::custom)
            }
        }
    };
}

/// Helper macro to implement string based serialization and deserialization.
///
/// If a type implements `FromStr` and `Display` then this automatically
/// implements a serializer/deserializer for that type that dispatches
/// appropriately.
#[allow(unused_macros)]
macro_rules! impl_str_serde {
    ($type:ty) => {
        impl_str_ser!($type);
        impl_str_de!($type);
    };
}

#[cfg(test)]
mod str_tests {
    use std::fmt;
    use std::io::Cursor;
    use std::str::FromStr;

    struct Test;

    impl FromStr for Test {
        type Err = &'static str;

        fn from_str(string: &str) -> Result<Self, Self::Err> {
            match string {
                "test" => Ok(Test),
                _ => Err("failed"),
            }
        }
    }

    impl fmt::Display for Test {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "test")
        }
    }

    impl_str_serde!(Test);

    #[test]
    fn test_serialize_string() {
        assert_eq!("\"test\"", serde_json::to_string(&Test).unwrap());
    }

    #[test]
    fn test_deserialize() {
        assert!(serde_json::from_str::<Test>("\"test\"").is_ok());
    }

    #[test]
    fn test_deserialize_owned() {
        assert!(serde_json::from_reader::<_, Test>(Cursor::new("\"test\"")).is_ok());
    }
}

/// Helper macro to implement serialization into base 16 (hex) string
/// representation. Implements `Display` and `Serialize`.
macro_rules! impl_hex_ser {
    ($type:ident) => {
        impl ::std::fmt::Display for $type {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{:#x}", self.0)
            }
        }

        impl ::serde::ser::Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }
    };
}

/// Helper macro to implement deserialization from both numeric values or their
/// base16 (hex) / base10 representations as string. Implements `FromStr` and
/// `Deserialize`.
macro_rules! impl_hex_de {
    ($type:ident, $num:ident) => {
        impl ::std::str::FromStr for $type {
            type Err = ::std::num::ParseIntError;

            fn from_str(s: &str) -> Result<$type, ::std::num::ParseIntError> {
                if s.starts_with("0x") || s.starts_with("0X") {
                    $num::from_str_radix(&s[2..], 16).map($type)
                } else {
                    s.parse::<$num>().map($type)
                }
            }
        }

        impl<'de> ::serde::de::Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<$type, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                struct HexVisitor;

                impl<'de> ::serde::de::Visitor<'de> for HexVisitor {
                    type Value = $type;

                    fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        write!(f, "a number or hex string")
                    }

                    fn visit_i64<E: ::serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                        Ok($type(v as $num))
                    }

                    fn visit_u64<E: ::serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                        Ok($type(v as $num))
                    }

                    fn visit_str<E: ::serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                        v.parse().map_err(::serde::de::Error::custom)
                    }
                }

                deserializer.deserialize_any(HexVisitor)
            }
        }
    };
}

/// Helper macro to implement serialization from both numeric values or their
/// hex representation as string.
///
/// This implements `Serialize`, `Deserialize`, `Display` and `FromStr`.
/// Serialization will always use a `"0xbeef"` representation of the value.
/// Deserialization supports raw numbers as well as string representations
/// in hex and base10.
macro_rules! impl_hex_serde {
    ($type:ident, $num:ident) => {
        impl_hex_ser!($type);
        impl_hex_de!($type, $num);
    };
}

#[cfg(test)]
mod hex_tests {
    use std::io::Cursor;

    #[derive(Debug, PartialEq)]
    struct Hex(u32);

    impl_hex_serde!(Hex, u32);

    #[test]
    fn test_hex_to_string() {
        assert_eq!("0x0", &Hex(0).to_string());
        assert_eq!("0x2a", &Hex(42).to_string());
    }

    #[test]
    fn test_hex_serialize() {
        assert_eq!("\"0x0\"", serde_json::to_string(&Hex(0)).unwrap());
        assert_eq!("\"0x2a\"", serde_json::to_string(&Hex(42)).unwrap());
    }

    #[test]
    fn test_hex_from_string() {
        assert_eq!(Hex(0), "0".parse().unwrap());
        assert_eq!(Hex(42), "42".parse().unwrap());
        assert_eq!(Hex(42), "0x2a".parse().unwrap());
        assert_eq!(Hex(42), "0X2A".parse().unwrap());
    }

    #[test]
    fn test_hex_deserialize() {
        assert_eq!(Hex(0), serde_json::from_str("\"0\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"42\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"0x2a\"").unwrap());
        assert_eq!(Hex(42), serde_json::from_str("\"0X2A\"").unwrap());
    }

    #[test]
    fn test_hex_deserialize_owned() {
        assert_eq!(
            Hex(0),
            serde_json::from_reader(Cursor::new("\"0\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"42\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"0x2a\"")).unwrap()
        );
        assert_eq!(
            Hex(42),
            serde_json::from_reader(Cursor::new("\"0X2A\"")).unwrap()
        );
    }

    #[test]
    fn test_invalid() {
        let result = serde_json::from_str::<Hex>("true").unwrap_err();
        assert_eq!(
            "invalid type: boolean `true`, expected a number or hex string at line 1 column 4",
            result.to_string()
        );
    }
}
