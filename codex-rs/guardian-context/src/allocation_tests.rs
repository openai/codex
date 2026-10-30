#![cfg(test)]

use super::*;
use crate::ElasticSectionLimits;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

#[test]
fn groups_select_recent_whole_entries_and_redistribute_packing_gaps() {
    const ENTRY_TOKENS: usize = 256;
    const ITEM_COSTS: [usize; 3] = [50, 200, 180];
    const ALLOWANCE: usize = 660;
    const NEWEST: usize = 2;
    let groups = [
        ElasticSection::TranscriptMessages,
        ElasticSection::TranscriptTools,
    ];
    let limits = ElasticSectionLimits::new(ENTRY_TOKENS, ENTRY_TOKENS).unwrap();
    let config = ElasticBudgetConfig {
        sections: groups.map(|group| (group, limits)).into(),
    };
    let mut demand = ContextDemand {
        required_tokens: 0,
        content_counts: vec![None, None],
        groups: BTreeMap::new(),
    };
    for (section_index, group) in groups.into_iter().enumerate() {
        let mut entries = GroupDemand::default();
        for (item_index, cost) in ITEM_COSTS.into_iter().enumerate() {
            entries.pending_items.push(Candidate {
                section_index,
                item_index,
                cost,
            });
            entries.total_cost += cost;
        }
        demand.groups.insert(group, entries);
    }
    assert_eq!(
        allocate_items(demand, &config, ALLOWANCE),
        HashSet::from([(0, 0), (0, 1), (0, NEWEST), (1, 0), (1, NEWEST)])
    );
}

#[test]
fn whole_images_use_defaults_and_charge_shared_framing_once() {
    const ENTRY_TOKENS: usize = 128;
    const IMAGE_COUNT: usize = 3;
    const TOOL_COUNT: usize = 40;
    const TOOL_REPEATS: usize = 24;
    const LIMITED_TOKENS: usize = 1_000;
    const HISTORY_TOKENS: usize = 12_000;
    const NOTICE: &str = "Evidence omitted";
    let mut images = Vec::new();
    for index in 0..IMAGE_COUNT {
        let image = ContentItem::InputImage {
            image: ImageReference::Inline {
                image_url: format!("data:image/png;base64,original-{index}"),
            },
            detail: None,
        };
        images.push(Budgeted::optional(image.into(), BudgetPriority::Image));
    }
    let tool = Budgeted::optional(
        "tool evidence ".repeat(TOOL_REPEATS).into(),
        BudgetPriority::Tool,
    );
    let mut tools = vec![tool; TOOL_COUNT];
    tools.extend([
        Budgeted::required("exact user instruction".to_owned().into()),
        Budgeted::historical("manual approval".to_owned().into()),
    ]);
    let context = ComposedContext {
        sections: [
            ("transcript_images", images),
            ("conversation_transcript", tools),
        ]
        .map(|(id, items)| SectionOutput {
            id,
            delivery: SectionDelivery::UserContent(items),
        })
        .into(),
        truncations: Vec::new(),
    };
    let mut expected = context.clone();
    if let SectionDelivery::UserContent(images) = &mut expected.sections[0].delivery {
        images.remove(/*index*/ 0);
    }
    expected.sections.push(SectionOutput {
        id: "budget_omission",
        delivery: SectionDelivery::UserContent(vec![Budgeted::required(NOTICE.to_owned().into())]),
    });
    let limits = ElasticSectionLimits::new(ENTRY_TOKENS, ENTRY_TOKENS).unwrap();
    for sections in [
        BTreeMap::new(),
        BTreeMap::from([(ElasticSection::TranscriptTools, limits)]),
    ] {
        let config = ElasticBudgetConfig { sections };
        let input = PreparedElasticContext {
            context: context.clone(),
            config: &config,
        };
        let mut budget = RequestBudget {
            max_input_tokens: context.estimated_tokens() + HISTORY_TOKENS,
            existing_context_tokens: HISTORY_TOKENS,
        };
        let untouched = input.clone().enforce_budget(budget, NOTICE.into()).unwrap();
        assert_eq!(untouched.into_messages(), context.clone().into_messages());
        budget.max_input_tokens = expected.estimated_tokens() + HISTORY_TOKENS;
        let admitted = input.clone().enforce_budget(budget, NOTICE.into()).unwrap();
        assert_eq!(admitted.into_messages(), expected.clone().into_messages());
        budget.max_input_tokens = LIMITED_TOKENS + HISTORY_TOKENS;
        let admitted = input.enforce_budget(budget, NOTICE.into()).unwrap();
        assert!(admitted.estimated_tokens() <= LIMITED_TOKENS);
        let text = serde_json::to_string(&admitted.into_messages()).unwrap();
        assert!(text.contains("exact user instruction") && text.contains("manual approval"));
    }
}
