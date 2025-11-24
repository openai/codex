use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use chrono::prelude::*;
use serde_json::json;
use std::borrow::Cow;

impl JsonSchema for Weekday {
    no_ref_schema!();

    fn schema_name() -> String {
        "Weekday".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("chrono::Weekday")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            enum_values: Some(vec![
                json!("Mon"),
                json!("Tue"),
                json!("Wed"),
                json!("Thu"),
                json!("Fri"),
                json!("Sat"),
                json!("Sun"),
            ]),
            ..Default::default()
        }
        .into()
    }
}

macro_rules! formatted_string_impl {
    ($ty:ident, $format:literal) => {
        formatted_string_impl!($ty, $format, JsonSchema for $ty);
    };
    ($ty:ident, $format:literal, $($desc:tt)+) => {
        impl $($desc)+ {
            no_ref_schema!();

            fn schema_name() -> String {
                stringify!($ty).to_owned()
            }

            fn schema_id() -> Cow<'static, str>  {
                Cow::Borrowed(stringify!(chrono::$ty))
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                SchemaObject {
                    instance_type: Some(InstanceType::String.into()),
                    format: Some($format.to_owned()),
                    ..Default::default()
                }
                .into()
            }
        }
    };
}

formatted_string_impl!(NaiveDate, "date");
formatted_string_impl!(NaiveDateTime, "partial-date-time");
formatted_string_impl!(NaiveTime, "partial-date-time");
formatted_string_impl!(DateTime, "date-time", <Tz: TimeZone> JsonSchema for DateTime<Tz>);
