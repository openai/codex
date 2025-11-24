#![cfg(feature = "serde")]
#![cfg_attr(docsrs, doc(cfg(feature = "serde")))]

use super::{BigInt, Sign};

use serde::de::{Error, Unexpected};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for Sign {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Note: do not change the serialization format, or it may break
        // forward and backward compatibility of serialized data!
        match *self {
            Sign::Minus => (-1i8).serialize(serializer),
            Sign::NoSign => 0i8.serialize(serializer),
            Sign::Plus => 1i8.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Sign {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let sign = i8::deserialize(deserializer)?;
        match sign {
            -1 => Ok(Sign::Minus),
            0 => Ok(Sign::NoSign),
            1 => Ok(Sign::Plus),
            _ => Err(D::Error::invalid_value(
                Unexpected::Signed(sign.into()),
                &"a sign of -1, 0, or 1",
            )),
        }
    }
}

impl Serialize for BigInt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Note: do not change the serialization format, or it may break
        // forward and backward compatibility of serialized data!
        (self.sign, &self.data).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BigInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (sign, data) = Deserialize::deserialize(deserializer)?;
        Ok(BigInt::from_biguint(sign, data))
    }
}
