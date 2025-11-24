use core::fmt::Write as _;

use toml_writer::TomlWrite as _;

use super::array_of_tables::ArrayOfTablesSerializer;
use super::style::Style;
use super::value::KeySerializer;
use super::value::ValueSerializer;
use super::Buffer;
use super::Error;
use super::SerializationStrategy;
use super::Serializer;
use super::Table;
use crate::alloc_prelude::*;

#[doc(hidden)]
pub struct SerializeDocumentTable<'d> {
    buf: &'d mut Buffer,
    table: Table,
    key: Option<String>,
    style: Style,
}

impl<'d> SerializeDocumentTable<'d> {
    pub(crate) fn map(buf: &'d mut Buffer, table: Table, style: Style) -> Result<Self, Error> {
        Ok(Self {
            buf,
            table,
            key: None,
            style,
        })
    }

    fn end(self) -> Result<&'d mut Buffer, Error> {
        self.buf.push(self.table);
        Ok(self.buf)
    }
}

impl<'d> serde::ser::SerializeMap for SerializeDocumentTable<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut encoded_key = String::new();
        input.serialize(KeySerializer {
            dst: &mut encoded_key,
        })?;
        self.key = Some(encoded_key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let encoded_key = self
            .key
            .take()
            .expect("always called after `serialize_key`");
        match SerializationStrategy::from(value) {
            SerializationStrategy::Value => {
                let dst = self.table.body_mut();

                write!(dst, "{encoded_key}")?;
                dst.space()?;
                dst.keyval_sep()?;
                dst.space()?;
                let value_serializer = ValueSerializer::with_style(dst, self.style);
                let dst = value.serialize(value_serializer)?;
                dst.newline()?;
            }
            SerializationStrategy::ArrayOfTables => {
                self.table.has_children(true);
                let value_serializer = ArrayOfTablesSerializer::new(
                    self.buf,
                    self.table.clone(),
                    encoded_key,
                    self.style,
                );
                value.serialize(value_serializer)?;
            }
            SerializationStrategy::Table | SerializationStrategy::Unknown => {
                let child = self.buf.child_table(&mut self.table, encoded_key);
                let value_serializer = Serializer::with_table(self.buf, child, self.style);
                value.serialize(value_serializer)?;
            }
            SerializationStrategy::Skip => {
                // silently drop these key-value pairs
            }
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<'d> serde::ser::SerializeStruct for SerializeDocumentTable<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match SerializationStrategy::from(value) {
            SerializationStrategy::Value => {
                let dst = self.table.body_mut();

                dst.key(key)?;
                dst.space()?;
                dst.keyval_sep()?;
                dst.space()?;
                let value_serializer = ValueSerializer::with_style(dst, self.style);
                let dst = value.serialize(value_serializer)?;
                dst.newline()?;
            }
            SerializationStrategy::ArrayOfTables => {
                self.table.has_children(true);
                let value_serializer = ArrayOfTablesSerializer::new(
                    self.buf,
                    self.table.clone(),
                    key.to_owned(),
                    self.style,
                );
                value.serialize(value_serializer)?;
            }
            SerializationStrategy::Table | SerializationStrategy::Unknown => {
                let child = self.buf.child_table(&mut self.table, key.to_owned());
                let value_serializer = Serializer::with_table(self.buf, child, self.style);
                value.serialize(value_serializer)?;
            }
            SerializationStrategy::Skip => {
                // silently drop these key-value pairs
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<'d> serde::ser::SerializeStructVariant for SerializeDocumentTable<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeStruct::serialize_field(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}
