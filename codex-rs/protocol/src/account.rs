use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, JsonSchema, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum PlanType {
    #[default]
    Free,
    Go,
    Plus,
    Pro,
    ProLite,
    Team,
    #[serde(rename = "self_serve_business_usage_based")]
    #[ts(rename = "self_serve_business_usage_based")]
    SelfServeBusinessUsageBased,
    Business,
    #[serde(rename = "enterprise_cbp_usage_based")]
    #[ts(rename = "enterprise_cbp_usage_based")]
    EnterpriseCbpUsageBased,
    Enterprise,
    Edu,
    #[serde(other)]
    Unknown,
}

impl PlanType {
    pub fn is_team_like(self) -> bool {
        matches!(self, Self::Team | Self::SelfServeBusinessUsageBased)
    }

    pub fn is_business_like(self) -> bool {
        matches!(self, Self::Business | Self::EnterpriseCbpUsageBased)
    }
}

#[cfg(test)]
mod tests {
    use super::PlanType;
    use pretty_assertions::assert_eq;

    #[test]
    fn usage_based_plan_types_use_expected_wire_names() {
        assert_eq!(
            serde_json::to_string(&PlanType::SelfServeBusinessUsageBased)
                .expect("self-serve business usage based should serialize"),
            "\"self_serve_business_usage_based\""
        );
        assert_eq!(
            serde_json::to_string(&PlanType::EnterpriseCbpUsageBased)
                .expect("enterprise cbp usage based should serialize"),
            "\"enterprise_cbp_usage_based\""
        );
        assert_eq!(
            serde_json::to_string(&PlanType::ProLite).expect("prolite should serialize"),
            "\"prolite\""
        );
        assert_eq!(
            serde_json::from_str::<PlanType>("\"self_serve_business_usage_based\"")
                .expect("self-serve business usage based should deserialize"),
            PlanType::SelfServeBusinessUsageBased
        );
        assert_eq!(
            serde_json::from_str::<PlanType>("\"prolite\"").expect("prolite should deserialize"),
            PlanType::ProLite
        );
        assert_eq!(
            serde_json::from_str::<PlanType>("\"enterprise_cbp_usage_based\"")
                .expect("enterprise cbp usage based should deserialize"),
            PlanType::EnterpriseCbpUsageBased
        );
    }

    #[test]
    fn plan_family_helpers_group_usage_based_variants_with_existing_plans() {
        assert_eq!(PlanType::Team.is_team_like(), true);
        assert_eq!(PlanType::SelfServeBusinessUsageBased.is_team_like(), true);
        assert_eq!(PlanType::Business.is_team_like(), false);

        assert_eq!(PlanType::Business.is_business_like(), true);
        assert_eq!(PlanType::EnterpriseCbpUsageBased.is_business_like(), true);
        assert_eq!(PlanType::Team.is_business_like(), false);
    }
}
