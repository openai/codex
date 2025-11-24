use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use either::Either;
use std::borrow::Cow;

impl<L: JsonSchema, R: JsonSchema> JsonSchema for Either<L, R> {
    no_ref_schema!();

    fn schema_name() -> String {
        format!("Either_{}_or_{}", L::schema_name(), R::schema_name())
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!(
            "either::Either<{}, {}>",
            L::schema_id(),
            R::schema_id()
        ))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = SchemaObject::default();
        schema.subschemas().any_of = Some(vec![
            generator.subschema_for::<L>(),
            generator.subschema_for::<R>(),
        ]);
        schema.into()
    }
}
