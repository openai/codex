# Exomind Norm Feedback Template (M4)

Use this template to record false positives, false negatives, and rule iteration proposals.

## 1. Context

- `report_date`:
- `repo`:
- `branch_or_pr`:
- `reporter`:
- `rule_id`:
- `rule_version`:

## 2. Feedback Type

- `feedback_type`: `false_positive | false_negative | conflict | improvement`
- `impact_level`: `critical | high | medium | low`

## 3. Observation

- `expected_behavior`:
- `actual_behavior`:
- `evidence`: file path(s), line(s), or report artifact links.

## 4. Root Cause (Initial)

- `suspected_cause`: matcher gap, missing context, precedence issue, or bad action mapping.
- `related_rules`: list of rule ids if conflict exists.

## 5. Proposed Change

- `proposal_summary`:
- `schema_or_matcher_change`:
- `action_change`:
- `rollout_plan`: 10% -> 50% -> 100%.

## 6. Decision & Release

- `owner_review_result`: accepted/rejected/deferred.
- `decision_reason`:
- `target_release_version`:
- `post_release_validation_window`:

## 7. Closure Checklist

- [ ] Added/updated tests or fixtures.
- [ ] Updated catalog version.
- [ ] Linked to tracking issue or PR.
- [ ] Added regression sample to knowledge base.
