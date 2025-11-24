#![cfg(feature = "serde")]
#![cfg_attr(docsrs, doc(cfg(feature = "serde")))]

use super::{biguint_from_vec, BigUint};

use alloc::vec::Vec;
use core::{cmp, fmt, mem};
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// `cautious` is based on the function of the same name in `serde`, but specialized to `u32`:
// https://github.com/dtolnay/serde/blob/399ef081ecc36d2f165ff1f6debdcbf6a1dc7efb/serde/src/private/size_hint.rs#L11-L22
fn cautious(hint: Option<usize>) -> usize {
    const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

    cmp::min(
        hint.unwrap_or(0),
        MAX_PREALLOC_BYTES / mem::size_of::<u32>(),
    )
}

impl Serialize for BigUint {
    cfg_digit!(
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            // Note: do not change the serialization format, or it may break forward
            // and backward compatibility of serialized data!  If we ever change the
            // internal representation, we should still serialize in base-`u32`.
            let data: &[u32] = &self.data;
            data.serialize(serializer)
        }

        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            use serde::ser::SerializeSeq;

            if let Some((&last, data)) = self.data.split_last() {
                let last_lo = last as u32;
                let last_hi = (last >> 32) as u32;
                let u32_len = data.len() * 2 + 1 + (last_hi != 0) as usize;
                let mut seq = serializer.serialize_seq(Some(u32_len))?;
                for &x in data {
                    seq.serialize_element(&(x as u32))?;
                    seq.serialize_element(&((x >> 32) as u32))?;
                }
                seq.serialize_element(&last_lo)?;
                if last_hi != 0 {
                    seq.serialize_element(&last_hi)?;
                }
                seq.end()
            } else {
                let data: &[u32] = &[];
                data.serialize(serializer)
            }
        }
    );
}

impl<'de> Deserialize<'de> for BigUint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(U32Visitor)
    }
}

struct U32Visitor;

impl<'de> Visitor<'de> for U32Visitor {
    type Value = BigUint;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence of unsigned 32-bit numbers")
    }

    cfg_digit!(
        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let len = cautious(seq.size_hint());
            let mut data = Vec::with_capacity(len);

            while let Some(value) = seq.next_element::<u32>()? {
                data.push(value);
            }

            Ok(biguint_from_vec(data))
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            use crate::big_digit::BigDigit;
            use num_integer::Integer;

            let u32_len = cautious(seq.size_hint());
            let len = Integer::div_ceil(&u32_len, &2);
            let mut data = Vec::with_capacity(len);

            while let Some(lo) = seq.next_element::<u32>()? {
                let mut value = BigDigit::from(lo);
                if let Some(hi) = seq.next_element::<u32>()? {
                    value |= BigDigit::from(hi) << 32;
                    data.push(value);
                } else {
                    data.push(value);
                    break;
                }
            }

            Ok(biguint_from_vec(data))
        }
    );
}
