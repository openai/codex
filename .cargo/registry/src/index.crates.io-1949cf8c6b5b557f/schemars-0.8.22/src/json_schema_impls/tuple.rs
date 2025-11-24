use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;

macro_rules! tuple_impls {
    ($($len:expr => ($($name:ident)+))+) => {
        $(
            impl<$($name: JsonSchema),+> JsonSchema for ($($name,)+) {
                no_ref_schema!();

                fn schema_name() -> String {
                    let mut name = "Tuple_of_".to_owned();
                    name.push_str(&[$($name::schema_name()),+].join("_and_"));
                    name
                }

                fn schema_id() -> Cow<'static, str> {
                    let mut id = "(".to_owned();
                    id.push_str(&[$($name::schema_id()),+].join(","));
                    id.push(')');

                    Cow::Owned(id)
                }

                fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                    let items = vec![
                        $(generator.subschema_for::<$name>()),+
                    ];
                    SchemaObject {
                        instance_type: Some(InstanceType::Array.into()),
                        array: Some(Box::new(ArrayValidation {
                            items: Some(items.into()),
                            max_items: Some($len),
                            min_items: Some($len),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                    .into()
                }
            }
        )+
    }
}

tuple_impls! {
    1 => (T0)
    2 => (T0 T1)
    3 => (T0 T1 T2)
    4 => (T0 T1 T2 T3)
    5 => (T0 T1 T2 T3 T4)
    6 => (T0 T1 T2 T3 T4 T5)
    7 => (T0 T1 T2 T3 T4 T5 T6)
    8 => (T0 T1 T2 T3 T4 T5 T6 T7)
    9 => (T0 T1 T2 T3 T4 T5 T6 T7 T8)
    10 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9)
    11 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10)
    12 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11)
    13 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12)
    14 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13)
    15 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14)
    16 => (T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{schema_for, schema_object_for};
    use pretty_assertions::assert_eq;

    #[test]
    fn schema_for_map_any_value() {
        let schema = schema_object_for::<(i32, bool)>();
        assert_eq!(
            schema.instance_type,
            Some(SingleOrVec::from(InstanceType::Array))
        );
        let array_validation = schema.array.unwrap();
        assert_eq!(
            array_validation.items,
            Some(SingleOrVec::Vec(vec![
                schema_for::<i32>(),
                schema_for::<bool>()
            ]))
        );
        assert_eq!(array_validation.max_items, Some(2));
        assert_eq!(array_validation.min_items, Some(2));
    }
}
