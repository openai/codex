use crate::de::{Deserialize, DeserializeSeed, Deserializer};

/// A DeserializeSeed helper for implementing deserialize_in_place Visitors.
///
/// Wraps a mutable reference and calls deserialize_in_place on it.
pub struct InPlaceSeed<'a, T: 'a>(pub &'a mut T);

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, 'de, T> DeserializeSeed<'de> for InPlaceSeed<'a, T>
where
    T: Deserialize<'de>,
{
    type Value = ();
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_in_place(deserializer, self.0)
    }
}
