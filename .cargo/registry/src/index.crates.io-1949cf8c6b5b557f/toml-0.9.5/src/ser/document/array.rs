use core::fmt::Write as _;

use toml_writer::TomlWrite as _;

use super::style::Style;
use super::value::ValueSerializer;
use super::Buffer;
use super::Error;
use super::Table;

#[doc(hidden)]
pub struct SerializeDocumentTupleVariant<'d> {
    buf: &'d mut Buffer,
    table: Table,
    seen_value: bool,
    style: Style,
}

impl<'d> SerializeDocumentTupleVariant<'d> {
    pub(crate) fn tuple(
        buf: &'d mut Buffer,
        mut table: Table,
        variant: &'static str,
        _len: usize,
        style: Style,
    ) -> Result<Self, Error> {
        let dst = table.body_mut();
        dst.key(variant)?;
        dst.space()?;
        dst.keyval_sep()?;
        dst.space()?;
        dst.open_array()?;
        Ok(Self {
            buf,
            table,
            seen_value: false,
            style,
        })
    }
}

impl<'d> serde::ser::SerializeTupleVariant for SerializeDocumentTupleVariant<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let dst = self.table.body_mut();

        if self.style.multiline_array {
            dst.newline()?;
            write!(dst, "    ")?;
        } else {
            if self.seen_value {
                dst.val_sep()?;
                dst.space()?;
            }
        }
        self.seen_value = true;
        value.serialize(ValueSerializer::with_style(dst, self.style))?;
        if self.style.multiline_array {
            dst.val_sep()?;
        }
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        let dst = self.table.body_mut();
        if self.style.multiline_array && self.seen_value {
            dst.newline()?;
        }
        dst.close_array()?;
        dst.newline()?;
        self.buf.push(self.table);
        Ok(self.buf)
    }
}
