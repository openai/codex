use core::fmt::Write as _;

use toml_writer::TomlWrite as _;

use super::Error;
use super::Style;
use crate::alloc_prelude::*;

#[doc(hidden)]
pub struct SerializeValueArray<'d> {
    dst: &'d mut String,
    seen_value: bool,
    style: Style,
    len: Option<usize>,
}

impl<'d> SerializeValueArray<'d> {
    pub(crate) fn seq(
        dst: &'d mut String,
        style: Style,
        len: Option<usize>,
    ) -> Result<Self, Error> {
        dst.open_array()?;
        Ok(Self {
            dst,
            seen_value: false,
            style,
            len,
        })
    }

    fn end(self) -> Result<&'d mut String, Error> {
        if self.multiline_array() && self.seen_value {
            self.dst.newline()?;
        }
        self.dst.close_array()?;
        Ok(self.dst)
    }

    fn multiline_array(&self) -> bool {
        self.style.multiline_array && 2 <= self.len.unwrap_or(usize::MAX)
    }
}

impl<'d> serde::ser::SerializeSeq for SerializeValueArray<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        if self.multiline_array() {
            self.dst.newline()?;
            write!(self.dst, "    ")?;
        } else {
            if self.seen_value {
                self.dst.val_sep()?;
                self.dst.space()?;
            }
        }
        self.seen_value = true;
        value.serialize(super::ValueSerializer::with_style(self.dst, self.style))?;
        if self.multiline_array() {
            self.dst.val_sep()?;
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<'d> serde::ser::SerializeTuple for SerializeValueArray<'d> {
    type Ok = &'d mut String;
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

impl<'d> serde::ser::SerializeTupleStruct for SerializeValueArray<'d> {
    type Ok = &'d mut String;
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

pub struct SerializeTupleVariant<'d> {
    inner: SerializeValueArray<'d>,
}

impl<'d> SerializeTupleVariant<'d> {
    pub(crate) fn tuple(
        dst: &'d mut String,
        variant: &'static str,
        len: usize,
        style: Style,
    ) -> Result<Self, Error> {
        dst.open_inline_table()?;
        dst.space()?;
        dst.key(variant)?;
        dst.space()?;
        dst.keyval_sep()?;
        dst.space()?;
        Ok(Self {
            inner: SerializeValueArray::seq(dst, style, Some(len))?,
        })
    }
}

impl<'d> serde::ser::SerializeTupleVariant for SerializeTupleVariant<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(&mut self.inner, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let dst = self.inner.end()?;
        dst.space()?;
        dst.close_inline_table()?;
        Ok(dst)
    }
}
