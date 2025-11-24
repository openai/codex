use alloc::{borrow::ToOwned, string::String};
use core::{fmt, marker::PhantomData};

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};

use crate::generic::Cow;
use crate::traits::internal::{Beef, Capacity};

impl<T, U> Serialize for Cow<'_, T, U>
where
    T: Beef + Serialize + ?Sized,
    U: Capacity,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        T::serialize(self.as_ref(), serializer)
    }
}

struct CowVisitor<'de, 'a, T: Beef + ?Sized, U: Capacity>(
    PhantomData<fn() -> (&'de T, Cow<'a, T, U>)>,
);

impl<'de, 'a, U> Visitor<'de> for CowVisitor<'de, 'a, str, U>
where
    'de: 'a,
    U: Capacity,
{
    type Value = Cow<'a, str, U>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Cow::borrowed(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Cow::owned(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Cow::owned(value))
    }
}

impl<'de, 'a, U> Deserialize<'de> for Cow<'a, str, U>
where
    'de: 'a,
    U: Capacity,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CowVisitor::<'de, 'a, str, U>(PhantomData))
    }
}

impl<'de, 'a, T, U> Deserialize<'de> for Cow<'a, [T], U>
where
    [T]: Beef,
    U: Capacity,
    <[T] as ToOwned>::Owned: Deserialize<'de>,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <[T] as ToOwned>::Owned::deserialize(deserializer).map(Cow::owned)
    }
}

#[cfg(test)]
mod tests {
    use serde_derive::{Deserialize, Serialize};

    #[test]
    fn wide_cow_de() {
        use crate::Cow;

        #[derive(Serialize, Deserialize)]
        struct Test<'a> {
            #[serde(borrow)]
            foo: Cow<'a, str>,
            bar: Cow<'a, str>,
        }

        let json = r#"{"foo":"Hello","bar":"\tWorld!"}"#;
        let test: Test = serde_json::from_str(json).unwrap();

        assert_eq!(test.foo, "Hello");
        assert_eq!(test.bar, "\tWorld!");

        assert!(test.foo.is_borrowed());
        assert!(test.bar.is_owned());

        let out = serde_json::to_string(&test).unwrap();

        assert_eq!(json, out);
    }

    #[test]
    fn wide_cow_direct() {
        use crate::Cow;

        let json = r#""foo""#;
        let cow: Cow<str> = serde_json::from_str(json).unwrap();

        assert_eq!(cow, "foo");

        assert!(cow.is_borrowed());

        let json = r#""\tfoo""#;
        let cow: Cow<str> = serde_json::from_str(json).unwrap();

        assert_eq!(cow, "\tfoo");

        assert!(cow.is_owned());
    }

    #[test]
    fn wide_cow_direct_bytes() {
        use crate::Cow;

        let json = r#"[102, 111, 111]"#;
        let cow: Cow<[u8]> = serde_json::from_str(json).unwrap();

        assert_eq!(cow, &b"foo"[..]);

        // We need to stay generic over `[T]`, so no specialization for byte slices
        assert!(cow.is_owned());
    }
}
