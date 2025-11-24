use crate::{
    signature_parser::SignatureParser, utils::*, DynamicDeserialize, DynamicType, Signature,
};
use serde::{
    de::{Deserialize, DeserializeSeed, Deserializer, Error, Visitor},
    Serialize, Serializer,
};
use std::marker::PhantomData;

/// A helper type to serialize or deserialize a tuple whose elements implement [DynamicType] but
/// not [Type].
///
/// This is required because tuples already have an implementation of [DynamicType] via the blanket
/// implementation of [DynamicType] where `T: Type`, but that results in a bound of [Type] on each
/// element, which is stronger than needed for serializing.
///
/// [Type]: trait.Type.html
#[derive(Debug, Copy, Clone)]
pub struct DynamicTuple<T>(pub T);

impl DynamicType for DynamicTuple<()> {
    fn dynamic_signature(&self) -> Signature<'_> {
        Signature::from_static_str_unchecked("")
    }
}

impl<T: Serialize> Serialize for DynamicTuple<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DynamicTuple<()> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <()>::deserialize(deserializer).map(DynamicTuple)
    }
}

/// A helper type for [DynamicTuple]'s [DynamicDeserialize] implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleSeed<'a, T, S> {
    sig: Signature<'a>,
    seeds: S,
    marker: PhantomData<T>,
}

impl<'a, T, S> DynamicType for TupleSeed<'a, T, S> {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.sig.clone()
    }
}

struct TupleVisitor<T, S> {
    seeds: S,
    marker: PhantomData<T>,
}

macro_rules! tuple_impls {
    ($($len:expr => ($($n:tt $name:ident)+))+) => {
        $(
            impl<$($name),+> DynamicType for DynamicTuple<($($name,)+)>
            where
                $($name: DynamicType,)+
            {
                fn dynamic_signature(&self) -> Signature<'_> {
                    let mut sig = String::with_capacity(255);
                    sig.push(STRUCT_SIG_START_CHAR);
                    $(
                        sig.push_str(DynamicType::dynamic_signature(&self.0.$n).as_str());
                    )+
                    sig.push(STRUCT_SIG_END_CHAR);

                    Signature::from_string_unchecked(sig)
                }
            }

            impl<'de, $($name),+> DeserializeSeed<'de> for TupleSeed<'de, ($($name,)+), ($(<$name as DynamicDeserialize<'de>>::Deserializer,)+)>
            where
                $($name: DynamicDeserialize<'de>,)+
            {
                type Value = DynamicTuple<($($name,)+)>;

                fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
                    deserializer.deserialize_tuple($len, TupleVisitor { seeds: self.seeds, marker: self.marker })
                }
            }

            impl<'de, $($name),+> Visitor<'de> for TupleVisitor<($($name,)+), ($(<$name as DynamicDeserialize<'de>>::Deserializer,)+)>
            where
                $($name: DynamicDeserialize<'de>,)+
            {
                type Value = DynamicTuple<($($name,)+)>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a tuple")
                }

                fn visit_seq<V>(self, mut visitor: V) -> Result<DynamicTuple<($($name,)+)>, V::Error>
                where
                    V: serde::de::SeqAccess<'de>,
                {
                    Ok(DynamicTuple(($({
                        match visitor.next_element_seed(self.seeds.$n) {
                            Ok(Some(elt)) => elt,
                            Ok(None) => return Err(V::Error::invalid_length($len, &"")),
                            Err(e) => return Err(e),
                        }
                    },)+)))
                }
            }

            impl<'de, $($name),+> DynamicDeserialize<'de> for DynamicTuple<($($name,)+)>
            where
                $($name: DynamicDeserialize<'de>,)+
            {
                type Deserializer = TupleSeed<'de, ($($name,)+), ($(<$name as DynamicDeserialize<'de>>::Deserializer,)+)>;

                fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
                    where S: TryInto<Signature<'de>>, S::Error: Into<zvariant::Error>
                {
                    let sig = signature.try_into().map_err(Into::into)?;
                    if !sig.starts_with(zvariant::STRUCT_SIG_START_CHAR) {
                        return Err(zvariant::Error::IncorrectType);
                    }
                    if !sig.ends_with(zvariant::STRUCT_SIG_END_CHAR) {
                        return Err(zvariant::Error::IncorrectType);
                    }

                    let end = sig.len() - 1;
                    let mut sig_parser = SignatureParser::new(sig.slice(1..end));

                    let seeds = ($({
                        let elt_sig = sig_parser.parse_next_signature()?;
                        $name::deserializer_for_signature(elt_sig)?
                    },)+);

                    Ok(TupleSeed { sig, seeds, marker: PhantomData })
                }
            }
        )+
    }
}

tuple_impls! {
    1 => (0 T0)
    2 => (0 T0 1 T1)
    3 => (0 T0 1 T1 2 T2)
    4 => (0 T0 1 T1 2 T2 3 T3)
    5 => (0 T0 1 T1 2 T2 3 T3 4 T4)
    6 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5)
    7 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6)
    8 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7)
    9 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8)
    10 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9)
    11 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10)
    12 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11)
    13 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12)
    14 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13)
    15 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14)
    16 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15)
}
