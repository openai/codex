use serde::de::{Deserialize, Error, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;
use std::marker::PhantomData;

/// Map to serializeable representation
pub trait IntoSerializable {
    type Output;
    fn into_serializable(self) -> Self::Output;
}

/// Map from deserialized representation
pub trait FromDeserialized: Sized {
    type Input;
    fn from_deserialized<E>(input: Self::Input) -> Result<Self, E>
    where
        E: Error;
}

/// Serde combinator. A sequence visitor that maps deserialized elements
/// lazily; the visitor can also emit new errors if the elements have errors.
pub struct MappedSequenceVisitor<T, R, F>
where
    F: Fn(T) -> Result<R, &'static str>,
{
    f: F,
    marker: PhantomData<fn() -> T>,
}

impl<'de, F, T, R> MappedSequenceVisitor<T, R, F>
where
    T: Deserialize<'de>,
    F: Fn(T) -> Result<R, &'static str>,
{
    pub fn new(f: F) -> Self {
        MappedSequenceVisitor {
            f: f,
            marker: PhantomData,
        }
    }
}

impl<'de, F, T, R> Visitor<'de> for MappedSequenceVisitor<T, R, F>
where
    T: Deserialize<'de>,
    F: Fn(T) -> Result<R, &'static str>,
{
    type Value = Vec<R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a sequence")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut v = Vec::new();
        while let Some(elem) = seq.next_element()? {
            match (self.f)(elem) {
                Err(s) => Err(<A::Error>::custom(s))?,
                Ok(x) => v.push(x),
            }
        }
        Ok(v)
    }
}

pub trait CollectSeqWithLength: Serializer {
    fn collect_seq_with_length<I>(self, length: usize, iterable: I) -> Result<Self::Ok, Self::Error>
    where
        I: IntoIterator,
        I::Item: Serialize,
    {
        let mut count = 0;
        let mut seq = self.serialize_seq(Some(length))?;
        for element in iterable {
            seq.serialize_element(&element)?;
            count += 1;
        }
        debug_assert_eq!(length, count, "collect_seq_with_length: length mismatch!");
        seq.end()
    }

    fn collect_seq_exact<I>(self, iterable: I) -> Result<Self::Ok, Self::Error>
    where
        I: IntoIterator,
        I::Item: Serialize,
        I::IntoIter: ExactSizeIterator,
    {
        let iter = iterable.into_iter();
        self.collect_seq_with_length(iter.len(), iter)
    }
}

impl<S> CollectSeqWithLength for S where S: Serializer {}
