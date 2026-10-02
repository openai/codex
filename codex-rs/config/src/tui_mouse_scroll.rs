//! Keep transcript mouse-scroll speed validation and its configuration schema in sync.

use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::Schema;
use serde::Deserialize;
use serde::Deserializer;

pub(crate) fn schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = Option::<f64>::json_schema(generator).into_object();
    schema.number().exclusive_minimum = Some(0.0);
    schema.into()
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let speed = Option::<f64>::deserialize(deserializer)?;
    if speed.is_some_and(|speed| !speed.is_finite() || speed <= 0.0) {
        return Err(serde::de::Error::custom(
            "tui.mouse_scroll_speed must be a finite positive number",
        ));
    }
    Ok(speed)
}
