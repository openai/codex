mod compare;
mod model;
mod parse;
mod violation;

#[cfg(test)]
mod test_support;

use compare::compare_api_schemas;
pub use model::ApiSchema;
pub(crate) use model::Arguments;
pub(crate) use model::ArraySchema;
pub(crate) use model::ConstraintSet;
pub(crate) use model::JsonType;
pub(crate) use model::Method;
pub(crate) use model::ObjectSchema;
pub(crate) use model::SchemaId;
pub(crate) use model::SchemaNode;
pub(crate) use model::SchemaRules;
pub(crate) use model::TypeSet;
pub(crate) use model::UnionKind;
pub(crate) use model::UnionSchema;
pub(crate) use model::ValueSet;
pub(crate) use violation::Location;
pub use violation::SchemaBreakage;
pub(crate) use violation::SchemaPath;
pub(crate) use violation::SchemaSnapshot;
pub(crate) use violation::Violation;
pub use violation::ViolationKind;

use anyhow::Result;

/// Finds values that the current request schema no longer accepts.
pub fn find_request_narrowing(
    base: &ApiSchema,
    current: &ApiSchema,
) -> Result<Vec<SchemaBreakage>> {
    Ok(compare_api_schemas(base, current)?
        .iter()
        .map(Violation::breakage)
        .collect())
}
