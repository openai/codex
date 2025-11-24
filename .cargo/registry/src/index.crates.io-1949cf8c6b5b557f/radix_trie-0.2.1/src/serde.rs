extern crate serde;

use self::serde::ser::SerializeMap;
use self::serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use super::{Trie, TrieCommon, TrieKey};
use std::fmt::{self, Formatter};
use std::marker::PhantomData;

impl<K, V> Serialize for Trie<K, V>
where
    K: Serialize + TrieKey,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

struct TrieVisitor<K, V> {
    marker: PhantomData<Trie<K, V>>,
}

impl<K, V> TrieVisitor<K, V> {
    fn new() -> Self {
        TrieVisitor {
            marker: PhantomData,
        }
    }
}

impl<'a, K, V> de::Visitor<'a> for TrieVisitor<K, V>
where
    K: Deserialize<'a> + Clone + Eq + PartialEq + TrieKey,
    V: Deserialize<'a>,
{
    type Value = Trie<K, V>;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "a serialized trie")
    }

    fn visit_map<M>(self, mut visitor: M) -> Result<Self::Value, M::Error>
    where
        M: de::MapAccess<'a>,
    {
        let mut values = Trie::new();

        while let Some((key, value)) = visitor.next_entry()? {
            values.insert(key, value);
        }

        Ok(values)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Trie::new())
    }
}

impl<'a, K, V> Deserialize<'a> for Trie<K, V>
where
    K: Deserialize<'a> + Clone + Eq + PartialEq + TrieKey,
    V: Deserialize<'a>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        // Instantiate our Visitor and ask the Deserializer to drive
        // it over the input data, resulting in an instance of MyMap.
        deserializer.deserialize_map(TrieVisitor::new())
    }
}

#[cfg(test)]
mod test {
    extern crate serde_test;
    use self::serde_test::Token;
    use super::super::Trie;

    macro_rules! tests_de {
        ($($name:ident => $value:expr => $tokens:expr,)+) => {
            $(#[test]
            fn $name() {
                // Test ser/de roundtripping
                serde_test::assert_de_tokens(&$value, $tokens);
            })+
        }
    }

    macro_rules! tests_ser {
        ($($name:ident => $value:expr => $tokens:expr,)+) => {
            $(#[test]
            fn $name() {
                serde_test::assert_ser_tokens(&$value, $tokens);
            })+
        }
    }

    macro_rules! trie {
        () => {
            Trie::new()
        };
        ($($key:expr => $value:expr),+) => {
            {
                let mut map = Trie::new();
                $(map.insert($key, $value);)+
                map
            }
        }
    }

    tests_ser! {
        test_ser_empty_trie => Trie::<&str, isize>::new() => &[
            Token::Map { len: Some(0) },
            Token::MapEnd,
        ],
        test_ser_single_element_trie => trie!["1" => 2] => &[
            Token::Map { len: Some(1) },

            Token::Str("1"),
            Token::I32(2),
            Token::MapEnd,
        ],
        test_ser_multiple_element_trie => trie!["1" => 2, "3" => 4] => &[
            Token::Map { len: Some(2) },
            Token::Str("1"),
            Token::I32(2),

            Token::Str("3"),
            Token::I32(4),
            Token::MapEnd,
        ],
        test_ser_deep_trie => trie!["1" => trie![], "2" => trie!["3" => 4, "5" => 6]] => &[
            Token::Map { len: Some(2) },
            Token::Str("1"),
            Token::Map { len: Some(0) },
            Token::MapEnd,

            Token::Str("2"),
            Token::Map { len: Some(2) },
            Token::Str("3"),
            Token::I32(4),

            Token::Str("5"),
            Token::I32(6),
            Token::MapEnd,
            Token::MapEnd,
        ],
    }

    tests_de! {
        test_de_empty_trie1 => Trie::<String, isize>::new() => &[
            Token::Unit,
        ],
        test_de_empty_trie2 => Trie::<String, isize>::new() => &[
            Token::Map { len: Some(0) },
            Token::MapEnd,
        ],
        test_de_single_element_trie => trie!["1".to_string() => 2] => &[
            Token::Map { len: Some(1) },
                Token::Str("1"),
                Token::I32(2),
            Token::MapEnd,
        ],
        test_de_multiple_element_trie => trie!["1".to_string()  => 2, "3".to_string()  => 4] => &[
            Token::Map { len: Some(2) },
                Token::Str("1"),
                Token::I32(2),

                Token::Str("3"),
                Token::I32(4),
            Token::MapEnd,
        ],
        test_de_deep_trie => trie!["1".to_string()  => trie![], "2".to_string()  => trie!["3".to_string()  => 4, "5".to_string()  => 6]] => &[
            Token::Map { len: Some(2) },
                Token::Str("1"),
                Token::Map { len: Some(0) },
                Token::MapEnd,

                Token::Str("2"),
                Token::Map { len: Some(2) },
                    Token::Str("3"),
                    Token::I32(4),

                    Token::Str("5"),
                    Token::I32(6),
                Token::MapEnd,
            Token::MapEnd,
        ],
        test_de_empty_trie3 => Trie::<String, isize>::new() => &[
            Token::UnitStruct { name: "Anything" },
        ],
        test_de_empty_trie4 => Trie::<String, isize>::new() => &[
            Token::Struct {
                name: "Anything",
                len: 0,
            },
            Token::StructEnd,
        ],
    }
}
