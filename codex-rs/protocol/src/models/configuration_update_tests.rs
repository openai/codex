use super::ConfigurationReasoning;
use crate::models::ResponseItem;
use crate::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn configuration_update_round_trips_without_message_fields() {
    let value = json!({
        "type": "configuration_update",
        "reasoning": { "effort": "high" }
    });
    let mut item: ResponseItem = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        item,
        ResponseItem::ConfigurationUpdate {
            reasoning: ConfigurationReasoning {
                effort: ReasoningEffort::High,
            },
        }
    );

    item.set_id(Some(crate::uuid_v7_item_id_generator_for_test()(
        "configuration-id",
    )));
    item.set_turn_id_if_missing("turn-id");
    item.set_create_time_if_missing(serde_json::Number::from(123));
    assert_eq!(
        (item.id(), item.id_prefix(), item.turn_id()),
        (None, None, None)
    );
    assert_eq!(serde_json::to_value(item).unwrap(), value);
}

#[test]
fn configuration_update_preserves_model_defined_effort() {
    let value = json!({
        "type": "configuration_update",
        "reasoning": { "effort": "model-specific" }
    });
    let item: ResponseItem = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        item,
        ResponseItem::ConfigurationUpdate {
            reasoning: ConfigurationReasoning {
                effort: ReasoningEffort::Custom("model-specific".to_string()),
            },
        }
    );
    assert_eq!(serde_json::to_value(item).unwrap(), value);
}
