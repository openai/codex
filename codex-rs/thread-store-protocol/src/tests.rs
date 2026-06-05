use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn thread_history_mutation_round_trips_typed_variant() {
    let mutation = ThreadHistoryMutation::ThreadItem(ThreadItemMutation {
        metadata: ThreadHistoryMutationMetadata { schema_version: 1 },
        payload: json!({ "itemKey": "item-1" }),
    });
    let value = json!({
        "type": "thread_item",
        "schema_version": 1,
        "payload": {
            "itemKey": "item-1",
        },
    });

    assert_eq!(
        serde_json::to_value(&mutation).expect("serialize mutation"),
        value
    );
    assert_eq!(
        serde_json::from_value::<ThreadHistoryMutation>(value).expect("deserialize mutation"),
        mutation
    );
}
