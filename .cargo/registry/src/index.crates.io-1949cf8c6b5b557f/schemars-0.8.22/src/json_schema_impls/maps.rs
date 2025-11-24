use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;

macro_rules! map_impl {
    ($($desc:tt)+) => {
        impl $($desc)+
        where
            V: JsonSchema,
        {
            no_ref_schema!();

            fn schema_name() -> String {
                format!("Map_of_{}", V::schema_name())
            }

            fn schema_id() -> Cow<'static, str> {
                Cow::Owned(format!("Map<{}>", V::schema_id()))
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                let subschema = generator.subschema_for::<V>();
                SchemaObject {
                    instance_type: Some(InstanceType::Object.into()),
                    object: Some(Box::new(ObjectValidation {
                        additional_properties: Some(Box::new(subschema)),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
                .into()
            }
        }
    };
}

map_impl!(<K, V> JsonSchema for std::collections::BTreeMap<K, V>);
map_impl!(<K, V, H> JsonSchema for std::collections::HashMap<K, V, H>);
