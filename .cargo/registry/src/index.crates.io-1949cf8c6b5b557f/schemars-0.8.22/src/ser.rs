use crate::schema::*;
use crate::JsonSchema;
use crate::{r#gen::SchemaGenerator, Map};
use serde_json::{Error, Value};
use std::{convert::TryInto, fmt::Display};

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
    properties: Map<String, Schema>,
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
    ($fn:ident, $ty:ty, $instance_type:ident) => {
        fn $fn(self, _value: $ty) -> Result<Self::Ok, Self::Error> {
            Ok(SchemaObject {
                instance_type: Some(InstanceType::$instance_type.into()),
                ..Default::default()
            }
            .into())
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

    return_instance_type!(serialize_i8, i8, Integer);
    return_instance_type!(serialize_i16, i16, Integer);
    return_instance_type!(serialize_i32, i32, Integer);
    return_instance_type!(serialize_i64, i64, Integer);
    return_instance_type!(serialize_i128, i128, Integer);
    return_instance_type!(serialize_u8, u8, Integer);
    return_instance_type!(serialize_u16, u16, Integer);
    return_instance_type!(serialize_u32, u32, Integer);
    return_instance_type!(serialize_u64, u64, Integer);
    return_instance_type!(serialize_u128, u128, Integer);
    return_instance_type!(serialize_f32, f32, Number);
    return_instance_type!(serialize_f64, f64, Number);

    forward_to_subschema_for!(serialize_bool, bool);
    forward_to_subschema_for!(serialize_char, char);
    forward_to_subschema_for!(serialize_str, &str);
    forward_to_subschema_for!(serialize_bytes, &[u8]);

    fn collect_str<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display,
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
                if acc == Some(Schema::Bool(true)) {
                    return Ok(acc);
                }

                let schema = v.serialize(Serializer {
                    generator: self.generator,
                    include_title: false,
                })?;
                Ok(match &acc {
                    None => Some(schema),
                    Some(items) if items != &schema => Some(Schema::Bool(true)),
                    _ => acc,
                })
            })?
            .unwrap_or(Schema::Bool(true));

        Ok(SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                additional_properties: Some(Box::new(value_schema)),
                ..ObjectValidation::default()
            })),
            ..SchemaObject::default()
        }
        .into())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.generator.subschema_for::<Option<Value>>())
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        // FIXME nasty duplication of `impl JsonSchema for Option<T>`
        fn add_null_type(instance_type: &mut SingleOrVec<InstanceType>) {
            match instance_type {
                SingleOrVec::Single(ty) if **ty != InstanceType::Null => {
                    *instance_type = vec![**ty, InstanceType::Null].into()
                }
                SingleOrVec::Vec(ty) if !ty.contains(&InstanceType::Null) => {
                    ty.push(InstanceType::Null)
                }
                _ => {}
            };
        }

        let mut schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;

        if self.generator.settings().option_add_null_type {
            schema = match schema {
                Schema::Bool(true) => Schema::Bool(true),
                Schema::Bool(false) => <()>::json_schema(self.generator),
                Schema::Object(SchemaObject {
                    instance_type: Some(ref mut instance_type),
                    ..
                }) => {
                    add_null_type(instance_type);
                    schema
                }
                schema => SchemaObject {
                    subschemas: Some(Box::new(SubschemaValidation {
                        any_of: Some(vec![schema, <()>::json_schema(self.generator)]),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into(),
            }
        }

        if self.generator.settings().option_nullable {
            let mut schema_obj = schema.into_object();
            schema_obj
                .extensions
                .insert("nullable".to_owned(), serde_json::json!(true));
            schema = Schema::Object(schema_obj);
        };

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
        Ok(Schema::Bool(true))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        let include_title = self.include_title;
        let mut result = value.serialize(self);

        if include_title {
            if let Ok(Schema::Object(ref mut object)) = result {
                object.metadata().title = Some(name.to_string());
            }
        }

        result
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Ok(Schema::Bool(true))
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

    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Schema::Bool(true))
    }
}

impl serde::ser::SerializeStructVariant for Serializer<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Schema::Bool(true))
    }
}

impl serde::ser::SerializeSeq for SerializeSeq<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        if self.items != Some(Schema::Bool(true)) {
            let schema = value.serialize(Serializer {
                generator: self.generator,
                include_title: false,
            })?;
            match &self.items {
                None => self.items = Some(schema),
                Some(items) => {
                    if items != &schema {
                        self.items = Some(Schema::Bool(true))
                    }
                }
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let items = self.items.unwrap_or(Schema::Bool(true));
        Ok(SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(items.into()),
                ..ArrayValidation::default()
            })),
            ..SchemaObject::default()
        }
        .into())
    }
}

impl serde::ser::SerializeTuple for SerializeTuple<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        let schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.items.push(schema);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let len = self.items.len().try_into().ok();
        let mut schema = SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(SingleOrVec::Vec(self.items)),
                max_items: len,
                min_items: len,
                ..ArrayValidation::default()
            })),
            ..SchemaObject::default()
        };

        if !self.title.is_empty() {
            schema.metadata().title = Some(self.title.to_owned());
        }

        Ok(schema.into())
    }
}

impl serde::ser::SerializeTupleStruct for SerializeTuple<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
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

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
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

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        let key = self.current_key.take().unwrap_or_default();
        let schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.properties.insert(key, schema);

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            object: Some(Box::new(ObjectValidation {
                properties: self.properties,
                ..ObjectValidation::default()
            })),
            ..SchemaObject::default()
        };

        if !self.title.is_empty() {
            schema.metadata().title = Some(self.title.to_owned());
        }

        Ok(schema.into())
    }
}

impl serde::ser::SerializeStruct for SerializeMap<'_> {
    type Ok = Schema;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        let prop_schema = value.serialize(Serializer {
            generator: self.generator,
            include_title: false,
        })?;
        self.properties.insert(key.to_string(), prop_schema);

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeMap::end(self)
    }
}
