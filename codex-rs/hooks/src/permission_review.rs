use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use schemars::schema::SubschemaValidation;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PermissionRequestGuardianReview {
    pub status: PermissionRequestGuardianReviewStatus,
    #[schemars(schema_with = "nullable_permission_request_guardian_review_decision_schema")]
    pub decision: Option<PermissionRequestGuardianReviewDecision>,
    #[schemars(schema_with = "nullable_guardian_risk_level_schema")]
    pub risk_level: Option<GuardianRiskLevel>,
    #[schemars(schema_with = "nullable_guardian_user_authorization_schema")]
    pub user_authorization: Option<GuardianUserAuthorization>,
    #[schemars(schema_with = "nullable_string_schema")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequestGuardianReviewStatus {
    Approved,
    Denied,
    Aborted,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRequestGuardianReviewDecision {
    Allow,
    Deny,
}

pub(crate) fn nullable_permission_request_guardian_review_schema(
    generator: &mut SchemaGenerator,
) -> Schema {
    nullable_schema(generator.subschema_for::<PermissionRequestGuardianReview>())
}

fn nullable_permission_request_guardian_review_decision_schema(
    generator: &mut SchemaGenerator,
) -> Schema {
    nullable_schema(generator.subschema_for::<PermissionRequestGuardianReviewDecision>())
}

fn nullable_guardian_risk_level_schema(generator: &mut SchemaGenerator) -> Schema {
    nullable_schema(generator.subschema_for::<GuardianRiskLevel>())
}

fn nullable_guardian_user_authorization_schema(generator: &mut SchemaGenerator) -> Schema {
    nullable_schema(generator.subschema_for::<GuardianUserAuthorization>())
}

fn nullable_string_schema(_generator: &mut SchemaGenerator) -> Schema {
    Schema::Object(SchemaObject {
        instance_type: Some(vec![InstanceType::String, InstanceType::Null].into()),
        ..Default::default()
    })
}

fn nullable_schema(schema: Schema) -> Schema {
    Schema::Object(SchemaObject {
        subschemas: Some(Box::new(SubschemaValidation {
            any_of: Some(vec![
                schema,
                Schema::Object(SchemaObject {
                    instance_type: Some(InstanceType::Null.into()),
                    ..Default::default()
                }),
            ]),
            ..Default::default()
        })),
        ..Default::default()
    })
}
