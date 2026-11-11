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
    const MAXIMUM: usize = 300;
    const REQUIRED_COST: usize = 200;
    const GROWING_REQUIRED_COST: usize = 150;
    const FIXED_MINIMUM: usize = 100;
    const NO_HEADROOM_ALLOWANCE: usize = FIXED_MINIMUM * 2;
    const SECOND_MINIMUM_TOKENS: usize = ENTRY_TOKENS * 2;
    const GRANULAR_COUNT: usize = 9;
    const GRANULAR_MAXIMUM: usize = 3;
    const GRANULAR_FIRST_COUNT: usize = 2;
    const GRANULAR_SECOND_COUNT: usize = 3;
    const GRANULAR_ALLOWANCE: usize = GRANULAR_FIRST_COUNT + GRANULAR_SECOND_COUNT;
    for (required_cost, maximum, full_group, second_minimum) in [
        (0, None, Some(0), ENTRY_TOKENS),
        (0, Some(MAXIMUM), Some(1), ENTRY_TOKENS),
        (REQUIRED_COST, None, Some(1), ENTRY_TOKENS),
        (0, None, None, SECOND_MINIMUM_TOKENS),
        (GROWING_REQUIRED_COST, None, Some(0), FIXED_MINIMUM),
        (0, Some(GRANULAR_MAXIMUM), Some(0), 1),
    ] {
        let granular = second_minimum == 1;
        let granular_costs = [1; GRANULAR_COUNT];
        let item_costs = if granular {
            granular_costs.as_slice()
        } else {
            ITEM_COSTS.as_slice()
        };
        let minimum = second_minimum.min(ENTRY_TOKENS);
        let limits = ElasticSectionLimits::new(minimum, minimum).unwrap();
        let mut config = ElasticBudgetConfig {
            sections: groups.map(|group| (group, limits)).into(),
            ..Default::default()
        };
        config.sections.insert(
            groups[1],
            ElasticSectionLimits::new(second_minimum, minimum).unwrap(),
        );
        if let Some(maximum) = maximum {
            config.sections.insert(
                groups[0],
                limits.with_max_total_section_tokens(maximum).unwrap(),
            );
        }
        let config = crate::budget::tests::complete_minimum_budget(config);
        // Refill also handles proof pins that already use released packing gaps.
        let proof_pins = (required_cost == GROWING_REQUIRED_COST)
            .then(|| HashSet::from([(0, item_costs.len())]));
        for pinned in std::iter::once(HashSet::new()).chain(proof_pins) {
            let mut demand = ContextDemand {
                protected_tokens: 0,
                content_counts: vec![None, None],
                minimums_fit: false,
                sections: BTreeMap::new(),
            };
            for (section_index, group) in groups.into_iter().enumerate() {
                let mut entries = SectionDemand::default();
                if section_index == 0 && required_cost > 0 {
                    entries.required_items.push(Candidate {
                        section_index,
                        item_index: item_costs.len(),
                        cost: required_cost,
                    });
                    entries.total_cost = required_cost;
                }
                for (item_index, cost) in item_costs.iter().copied().enumerate() {
                    entries.optional_items.push(Candidate {
                        section_index,
                        item_index,
                        cost,
                    });
                    entries.total_cost += cost;
                }
                demand.sections.insert(group, entries);
            }
            let (mut allowance, mut expected) = if let Some(full_group) = full_group {
                (
                    ALLOWANCE,
                    HashSet::from([(0, 0), (0, NEWEST), (1, 0), (1, NEWEST), (full_group, 1)]),
                )
            } else {
                // Optional-only sections receive their minimum before spare capacity is shared.
                (
                    ENTRY_TOKENS + second_minimum,
                    HashSet::from([(0, 0), (0, NEWEST), (1, 0), (1, 1), (1, NEWEST)]),
                )
            };
            if granular {
                allowance = GRANULAR_ALLOWANCE;
                expected = (GRANULAR_COUNT - GRANULAR_FIRST_COUNT..GRANULAR_COUNT)
                    .map(|index| (0, index))
                    .chain(
                        (GRANULAR_COUNT - GRANULAR_SECOND_COUNT..GRANULAR_COUNT)
                            .map(|index| (1, index)),
                    )
                    .collect();
            } else if required_cost == GROWING_REQUIRED_COST {
                allowance = NO_HEADROOM_ALLOWANCE;
                expected = HashSet::from([(0, ITEM_COSTS.len()), (1, 0)]);
            } else if required_cost > 0 {
                expected = HashSet::from([(0, ITEM_COSTS.len()), (0, 0), (1, 1), (1, NEWEST)]);
            }
            assert_eq!(
                allocate_items(&mut demand, &config, allowance, &pinned).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn whole_images_use_defaults_and_charge_shared_framing_once() {
    const ENTRY_TOKENS: usize = 128;
    const IMAGE_COUNT: usize = 3;
    const TOOL_COUNT: usize = 41;
    const FRAMING_PARTS: usize = 3;
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
    // This count makes shared-framing rounding change when message picks are revisited.
    tools.extend((0..FRAMING_PARTS).map(|_| Budgeted::protected(String::new().into())));
    let mut context = ComposedContext {
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
    {
        let mut future = std::pin::pin!(crate::prepare_sync_images(
            &mut context,
            /*history*/ &[],
            Some(crate::DEFAULT_MAX_INPUT_TOKENS as i64),
            |image, _| Some(std::borrow::Cow::Borrowed(image)),
            |item| i64::try_from(crate::estimate_input_tokens(item)).unwrap(),
            || std::future::ready(0),
            crate::ContextBudgeting::Elastic,
        ));
        assert!(matches!(
            std::future::Future::poll(
                future.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop())
            ),
            std::task::Poll::Ready(())
        ));
    }
    let mut expected = context.clone();
    if let SectionDelivery::UserContent(images) = &mut expected.sections[0].delivery {
        images.remove(/*index*/ 0);
    }
    expected.sections.push(SectionOutput {
        id: "budget_omission",
        delivery: SectionDelivery::UserContent(vec![Budgeted::required(NOTICE.to_owned().into())]),
    });
    let notice_tokens = section_tokens(expected.sections.last().unwrap());
    let limits = ElasticSectionLimits::new(ENTRY_TOKENS, ENTRY_TOKENS).unwrap();
    for sections in [
        BTreeMap::new(),
        BTreeMap::from([
            (ElasticSection::TranscriptTools, limits),
            (ElasticSection::TranscriptImages, limits),
        ]),
    ] {
        let config = crate::budget::tests::complete_minimum_budget(ElasticBudgetConfig {
            sections,
            ..Default::default()
        });
        let mut demand = collect_demand(&context).unwrap();
        let allowance = context.estimated_tokens() - demand.protected_tokens;
        let expected_selection = demand
            .sections
            .values()
            .flat_map(|group| group.required_items.iter().chain(&group.optional_items))
            .map(|item| (item.section_index, item.item_index))
            .collect::<HashSet<_>>();
        let pinned = HashSet::from([(1, TOOL_COUNT), (1, TOOL_COUNT + 1)]);
        assert_eq!(
            allocate_items(&mut demand, &config, allowance, &pinned).unwrap(),
            expected_selection
        );
        let input = PreparedElasticContext {
            context: context.clone(),
            config: &config,
            delivered_omissions: None,
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
        let admitted = input.clone().enforce_budget(budget, NOTICE.into());
        if config
            .section_limits(ElasticSection::TranscriptTools)
            .min_total_section_tokens()
            >= LIMITED_TOKENS
        {
            assert!(matches!(
                admitted,
                Err(SectionError::EvidenceLimitExceeded {
                    section: "request_budget"
                })
            ));
        } else {
            let admitted = admitted.unwrap();
            assert!(admitted.estimated_tokens() <= LIMITED_TOKENS);
            let text = serde_json::to_string(&admitted.into_messages()).unwrap();
            assert!(text.contains("exact user instruction") && text.contains("manual approval"));
            // With no headroom, admission still funds the fixed reservations.
            let reserved: usize = demand
                .sections
                .iter()
                .map(|(group, section)| {
                    config
                        .section_limits(*group)
                        .min_total_section_tokens()
                        .min(section.total_cost)
                })
                .sum();
            budget.max_input_tokens =
                reserved + demand.protected_tokens + notice_tokens + HISTORY_TOKENS;
            let admitted = input.enforce_budget(budget, NOTICE.into()).unwrap();
            assert!(
                admitted.estimated_tokens() <= reserved + demand.protected_tokens + notice_tokens
            );
            let text = serde_json::to_string(&admitted.into_messages()).unwrap();
            assert!(text.contains("exact user instruction") && text.contains("manual approval"));
        }
    }
    // A section ceiling must apply even when the complete request fits globally.
    // Required records can grow past their minimum, bounded by their maximum.
    const IMAGE_MAXIMUM: usize = 20_500;
    const REQUIRED_MINIMUM: usize = 16;
    const REQUIRED_MAXIMUM: usize = REQUIRED_MINIMUM * 2;
    for maximum in [REQUIRED_MINIMUM, REQUIRED_MAXIMUM] {
        let config = crate::budget::tests::complete_minimum_budget(ElasticBudgetConfig {
            sections: BTreeMap::from([
                (
                    ElasticSection::TranscriptImages,
                    limits.with_max_total_section_tokens(IMAGE_MAXIMUM).unwrap(),
                ),
                (
                    ElasticSection::TranscriptMessages,
                    ElasticSectionLimits::new(REQUIRED_MINIMUM, REQUIRED_MINIMUM)
                        .unwrap()
                        .with_max_total_section_tokens(maximum)
                        .unwrap(),
                ),
            ]),
            ..Default::default()
        });
        let budget = RequestBudget {
            max_input_tokens: context.estimated_tokens() + HISTORY_TOKENS,
            existing_context_tokens: HISTORY_TOKENS,
        };
        let mut expected = expected.clone();
        if maximum == REQUIRED_MINIMUM
            && let SectionDelivery::UserContent(items) = &mut expected.sections[1].delivery
        {
            items.remove(TOOL_COUNT);
        }
        let admitted = PreparedElasticContext {
            context: context.clone(),
            config: &config,
            delivered_omissions: None,
        }
        .enforce_budget(budget, NOTICE.into())
        .unwrap();
        assert_eq!(admitted.into_messages(), expected.into_messages());
    }
}
