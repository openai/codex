use super::*;
use pretty_assertions::assert_eq;

#[test]
fn parse_guardian_assessment_extracts_embedded_json() {
    let parsed = parse_guardian_assessment(Some(
        "preface {\"risk_level\":\"medium\",\"user_authorization\":\"low\",\"outcome\":\"allow\",\"rationale\":\"ok\"}",
    ))
    .expect("guardian assessment");

    assert_eq!(
        parsed,
        GuardianAssessment {
            risk_level: GuardianRiskLevel::Medium,
            user_authorization: GuardianUserAuthorization::Low,
            outcome: GuardianAssessmentOutcome::Allow,
            rationale: "ok".to_string(),
        }
    );
}

#[test]
fn parse_guardian_assessment_treats_bare_allow_as_low_risk() {
    let parsed =
        parse_guardian_assessment(Some(r#"{"outcome":"allow"}"#)).expect("guardian assessment");

    assert_eq!(
        parsed,
        GuardianAssessment {
            risk_level: GuardianRiskLevel::Low,
            user_authorization: GuardianUserAuthorization::Unknown,
            outcome: GuardianAssessmentOutcome::Allow,
            rationale: "Auto-review returned a low-risk allow decision.".to_string(),
        }
    );
}

#[test]
fn parse_guardian_assessment_treats_bare_deny_as_high_risk() {
    let parsed =
        parse_guardian_assessment(Some(r#"{"outcome":"deny"}"#)).expect("guardian assessment");

    assert_eq!(
        parsed,
        GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            user_authorization: GuardianUserAuthorization::Unknown,
            outcome: GuardianAssessmentOutcome::Deny,
            rationale: "Auto-review returned a deny decision without a rationale.".to_string(),
        }
    );
}
