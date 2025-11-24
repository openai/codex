use crate::_alloc_prelude::*;
use crate::_private::allow_null;
use crate::{json_schema, Schema, SchemaGenerator};
use core::fmt::Display;
use serde_json::{Error, Map, Value};

pub(crate) struct Serializer<'a> {
    pub(crate) generator: &'a mut SchemaGenerator,
    pub(crate) include_title: bool,
}

pub(crate) struct SerializeSeq<'a> {
    generator: &'a mut SchemaGenerator,
    items: Option<Schema>,
}

pub(crate) struct SerializeTuple<'a> {
    generator: &'a mut SchemaGenerator,
    items: Vec<Schema>,
    title: &'static str,
}

pub(crate) struct SerializeMap<'a> {
    generator: &'a mut SchemaGenerator,
    properties: Map<String, Value>,
    current_key: Option<String>,
    title: &'static str,
}

macro_rules! forward_to_subschema_for {
    ($fn:ident, $ty:ty) => {
        fn $fn(self, _value: $ty) -> Result<Self::Ok, Self::Error> {
            Ok(self.generator.subschema_for::<$ty>())
        }
    };
}

macro_rules! return_instance_type {
    ($fn:ident, $ty:ty, $instance_type:expr) => {
        fn $fn(self, _value: $ty) -> Result<Self::Ok, Self::Error> {
            Ok(json_schema!({
                "type": $instance_type
            }))
        }
    };
}

impl<'a> serde::Serializer for Serializer<'a> {
    type Ok = Schema;
    type Error = Error;

    type SerializeSeq = SerializeSeq<'a>;
    type SerializeTuple = SerializeTuple<'a>;
    type SerializeTupleStruct = SerializeTuple<'a>;
    type SerializeTupleVariant = Self;
    type SerializeMap = SerializeMap<'a>;
    type SerializeStruct = SerializeMap<'a>;
    type SerializeStructVariant = Self;

    return_instance_type!(serialize_i8, i8, "integer");
    return_instance_type!(serialize_i16, i16, "integer");
    return_instance_type!(serialize_i32, i32, "integer");
    return_instance_type!(serialize_i64, i64, "integer");
    return_instance_type!(serialize_i128, i128, "integer");
    return_instance_type!(serialize_u8, u8, "integer");
    return_instance_type!(serialize_u16, u16, "integer");
    return_instance_type!(serialize_u32, u32, "integer");
    return_instance_type!(serialize_u64, u64, "integer");
    return_instance_type!(serialize_u128, u128, "integer");
    return_instance_type!(serialize_f32, f32, "number");
    return_instance_type!(serialize_f64, f64, "number");

    forward_to_subschema_for!(serialize_bool, bool);
    forward_to_subschema_for!(serialize_char, char);
    forward_to_subschema_for!(serialize_str, &str);
    forward_to_subschema_for!(serialize_bytes, &[u8]);

    fn collect_str<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        Ok(self.generator.subschema_for::<&str>())
    }

    fn collect_map<K, V, I>(self, iter: I) -> Result<Self::Ok, Self::Error>
    where
        K: serde::Serialize,
        V: serde::Serialize,
        I: IntoIterator<Item = (K, V)>,
    {
        let value_schema = iter
            .into_iter()
            .try_fold(None, |acc, (_, v)| {
                if acc == Some(true.into()) {
                    return Ok(acc);
                }

                let schema = v.serialize(Serializer {
                    generator: self.generator,
                    include_title: false,
                })?;
                Ok(match &acc {
                    None => Some(schema),
                    Some(items) if items != &schema => Some(true.into()),
                    _ => acc,
                })
            })?
            .unwrap_or(true.into());

        Ok(json_schema!({
            "type": "object",
            "additionalProperties": value_schema,
        }))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.generator.subschema_for::<Value>())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        let mut schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;

        allow_null(self.generator, &mut schema);

        Ok(schema)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(self.generator.subschema_for::<()>())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(true.into())
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        let include_title = self.include_title;
        let mut schema = value.serialize(self)?;

        if include_title && !name.is_empty() {
            schema.ensure_object().insert("title".into(), name.into());
        }

        Ok(schema)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        Ok(true.into())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeSeq {
            generator: self.generator,
            items: None,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(SerializeTuple {
            generator: self.generator,
            items: Vec::with_capacity(len),
            title: "",
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let title = if self.include_title { name } else { "" };
        Ok(SerializeTuple {
            generator: self.generator,
            items: Vec::with_capacity(len),
            title,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMap {
            generator: self.generator,
            properties: Map::new(),
            current_key: None,
            title: "",
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let title = if self.include_title { name } else { "" };
        Ok(SerializeMap {
            generator: self.generator,
            properties: Map::new(),
            current_key: None,
            title,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(self)
    }
}

impl serde::ser::SerializeTupleVariant for Serializer<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(true.into())
    }
}

impl serde::ser::SerializeStructVariant for Serializer<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(true.into())
    }
}

impl serde::ser::SerializeSeq for SerializeSeq<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        if self.items != Some(true.into()) {
            let schema = value.serialize(Serializer {
                generator: self.generator,
                include_title: false,
            })?;
            match &self.items {
                None => self.items = Some(schema),
                Some(items) => {
                    if items != &schema {
                        self.items = Some(true.into());
                    }
                }
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let items = self.items.unwrap_or(true.into());

        Ok(json_schema!({
            "type": "array",
            "items": items
        }))
    }
}

impl serde::ser::SerializeTuple for SerializeTuple<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        let schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.items.push(schema);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let len = self.items.len();
        let mut schema = json_schema!({
            "type": "array",
            "prefixItems": self.items,
            "maxItems": len,
            "minItems": len,
        });

        if !self.title.is_empty() {
            schema
                .ensure_object()
                .insert("title".into(), self.title.into());
        }

        Ok(schema)
    }
}

impl serde::ser::SerializeTupleStruct for SerializeTuple<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        serde::ser::SerializeTuple::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeTuple::end(self)
    }
}

impl serde::ser::SerializeMap for SerializeMap<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        // FIXME this is too lenient - we should return an error if serde_json
        // doesn't allow T to be a key of a map.
        let json = serde_json::to_string(key)?;
        self.current_key = Some(
            json.trim_start_matches('"')
                .trim_end_matches('"')
                .to_string(),
        );

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        let key = self.current_key.take().unwrap_or_default();
        let schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.properties.insert(key, schema.into());

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut schema = json_schema!({
            "type": "object",
            "properties": self.properties,
        });

        if !self.title.is_empty() {
            schema
                .ensure_object()
                .insert("title".into(), self.title.into());
        }

        Ok(schema)
    }
}

impl serde::ser::SerializeStruct for SerializeMap<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + ?Sized,
    {
        let prop_schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.properties.insert(key.to_string(), prop_schema.into());

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeMap::end(self)
    }
}
