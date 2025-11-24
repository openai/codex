use super::Error;

#[doc(hidden)]
pub struct SerializeValueArray {
    values: Vec<crate::Item>,
}

impl SerializeValueArray {
    pub(crate) fn seq(len: Option<usize>) -> Self {
        let mut values = Vec::new();
        if let Some(len) = len {
            values.reserve(len);
        }
        Self { values }
    }
}

impl serde::ser::SerializeSeq for SerializeValueArray {
    type Ok = crate::Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let value = value.serialize(super::ValueSerializer {})?;
        self.values.push(crate::Item::Value(value));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(crate::Value::Array(crate::Array::with_vec(self.values)))
    }
}

impl serde::ser::SerializeTuple for SerializeValueArray {
    type Ok = crate::Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl serde::ser::SerializeTupleStruct for SerializeValueArray {
    type Ok = crate::Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

pub struct SerializeTupleVariant {
    variant: &'static str,
    inner: SerializeValueArray,
}

impl SerializeTupleVariant {
    pub(crate) fn tuple(variant: &'static str, len: usize) -> Self {
        Self {
            variant,
            inner: SerializeValueArray::seq(Some(len)),
        }
    }
}

impl serde::ser::SerializeTupleVariant for SerializeTupleVariant {
    type Ok = crate::Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(&mut self.inner, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let inner = serde::ser::SerializeSeq::end(self.inner)?;
        let mut items = crate::table::KeyValuePairs::new();
        let value = crate::Item::Value(inner);
        items.insert(crate::Key::new(self.variant), value);
        Ok(crate::Value::InlineTable(crate::InlineTable::with_pairs(
            items,
        )))
    }
}
