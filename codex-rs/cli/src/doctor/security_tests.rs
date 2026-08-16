use pretty_assertions::assert_eq;

use super::super::redacted_json_check;
use super::CheckStatus;
use super::EndpointInspection;
use super::endpoint_check;

#[test]
fn no_endpoint_products_require_no_remediation() {
    let check = endpoint_check(EndpointInspection::Complete(Vec::new()));

    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.remediation, None);
    assert!(check.issues.is_empty());
}

#[test]
fn unavailable_endpoint_inspection_does_not_require_remediation() {
    let check = endpoint_check(EndpointInspection::Unavailable);

    assert_eq!(check.status, CheckStatus::Warning);
    assert!(check.summary.to_ascii_lowercase().contains("unavailable"));
    assert_eq!(check.details, vec!["endpoint products: unavailable"]);
    assert_eq!(check.remediation, None);
    assert!(check.issues.is_empty());
}

#[test]
fn each_endpoint_product_requires_vendor_specific_app_exclusions() {
    let cases: [(&str, &[&str]); 5] = [
        ("CrowdStrike Falcon", &["certificate", "ioa"]),
        ("BeyondTrust Privilege Management", &["application"]),
        ("Microsoft Defender", &["executable", "path", "certificate"]),
        ("SentinelOne", &["signer", "hash", "path"]),
        ("Jamf Protect", &["threat prevention"]),
    ];

    for (product, strategies) in cases {
        let check = endpoint_check(EndpointInspection::Complete(vec![product]));

        assert_eq!(check.status, CheckStatus::Warning, "{product}");
        assert!(
            check
                .details
                .iter()
                .any(|detail| detail.starts_with("exclusion targets: ")),
            "{product}: {:?}",
            check.details
        );
        let remediation = check
            .remediation
            .as_deref()
            .expect("detected endpoint protection should require remediation")
            .to_ascii_lowercase();
        assert_eq!(
            remediation,
            "ask your security administrator to verify codex exclusions and required helper allowances",
            "{product}"
        );
        assert!(
            !serde_json::to_string(&redacted_json_check(&check))
                .expect("serialize endpoint check")
                .to_ascii_lowercase()
                .contains("chatgpt"),
            "{product}: endpoint diagnostics should only refer to Codex"
        );

        assert_eq!(check.issues.len(), 1, "{product}");
        let issue = &check.issues[0];
        assert_eq!(issue.severity, CheckStatus::Warning, "{product}");
        assert!(
            issue
                .cause
                .to_ascii_lowercase()
                .contains(&product.to_ascii_lowercase()),
            "{product}: {}",
            issue.cause
        );
        assert!(
            issue.cause.contains("Verify Codex exclusions."),
            "{product}: {}",
            issue.cause
        );

        let remedy = issue
            .remedy
            .as_deref()
            .expect("endpoint product should have vendor-specific guidance")
            .to_ascii_lowercase();
        assert!(
            strategies.iter().any(|strategy| remedy.contains(strategy)),
            "{product}: {remedy}"
        );
        assert!(!remedy.contains("administrator-approved"), "{product}");
        assert!(!remedy.contains(';'), "{product}");
        for sentence in remedy.split('.') {
            assert!(
                sentence.split_whitespace().count() <= 20,
                "{product}: {sentence}"
            );
        }

        if product == "BeyondTrust Privilege Management" {
            assert!(remedy.contains("rule"), "{product}: {remedy}");
            assert!(remedy.contains("blocking"), "{product}: {remedy}");
            assert!(
                remedy.contains("do not grant administrator privileges"),
                "{product}: {remedy}"
            );
        }

        if product == "Microsoft Defender" {
            assert!(
                remedy.contains("attack surface reduction"),
                "{product}: {remedy}"
            );
            assert!(
                remedy.contains("controlled folder access"),
                "{product}: {remedy}"
            );
        }

        if product == "CrowdStrike Falcon" {
            assert!(remedy.contains("sensor visibility"), "{product}: {remedy}");
            assert!(remedy.contains("descendant"), "{product}: {remedy}");
        }

        if product == "Jamf Protect" {
            assert!(remedy.contains("exception"), "{product}: {remedy}");
            assert!(remedy.contains("signing"), "{product}: {remedy}");
        }
    }
}

#[test]
fn multiple_endpoint_products_each_get_distinct_guidance() {
    let products = ["CrowdStrike Falcon", "Microsoft Defender"];
    let check = endpoint_check(EndpointInspection::Complete(products.to_vec()));

    assert_eq!(check.status, CheckStatus::Warning);
    assert_eq!(check.issues.len(), products.len());

    for product in products {
        let matching_issues = check
            .issues
            .iter()
            .filter(|issue| {
                issue
                    .cause
                    .to_ascii_lowercase()
                    .contains(&product.to_ascii_lowercase())
            })
            .count();

        assert_eq!(matching_issues, 1, "{product}");
    }
}

#[test]
fn endpoint_product_guidance_preserves_structured_json_contract() {
    let check = endpoint_check(EndpointInspection::Complete(vec!["Microsoft Defender"]));
    let json = serde_json::to_value(redacted_json_check(&check)).expect("serialize endpoint check");

    assert_eq!(json["id"], "security.endpoint");
    assert_eq!(json["category"], "security");
    assert_eq!(json["status"], "warning");
    assert_eq!(json["details"]["endpoint products"], "Microsoft Defender");
    assert!(json["details"]["exclusion targets"].is_string());
    assert_eq!(json["issues"].as_array().map(Vec::len), Some(1));
    assert!(json["issues"][0]["remedy"].is_string());
}

#[test]
fn detected_products_preserve_partial_inspection_visibility() {
    let check = endpoint_check(EndpointInspection::Partial(vec!["Microsoft Defender"]));

    assert_eq!(check.status, CheckStatus::Warning);
    assert!(
        check
            .details
            .contains(&"additional endpoint products: unavailable".to_string())
    );
    assert_eq!(check.issues.len(), 1);
}
