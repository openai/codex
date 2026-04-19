use codex_observability::DataClass;
use codex_observability::DetailLevel;
use codex_observability::FieldMeta;
use codex_observability::FieldPolicy;
use codex_observability::Observation;
use codex_observability::ObservationFieldVisitor;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::Serializer;
use serde_json::Value;

#[derive(Observation)]
#[observation(name = "turn.config_resolved")]
struct TurnConfigResolved<'a> {
    #[obs(level = "basic", class = "identifier")]
    thread_id: &'a str,

    #[obs(level = "basic", class = "identifier")]
    turn_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    model: &'a str,
}

#[derive(Observation)]
#[observation(name = "test.policy_filtered")]
struct PolicyFiltered<'a> {
    #[obs(level = "basic", class = "identifier")]
    thread_id: &'a str,

    #[obs(level = "basic", class = "operational")]
    status: &'a str,

    #[obs(level = "trace", class = "content")]
    raw_prompt: PanicsIfSerialized,

    #[obs(level = "basic", class = "secret_risk")]
    api_key: PanicsIfSerialized,
}

struct PanicsIfSerialized;

impl Serialize for PanicsIfSerialized {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        panic!("denied observation field should not be serialized")
    }
}

#[derive(Debug, PartialEq)]
struct CapturedField {
    name: &'static str,
    meta: FieldMeta,
    value: Value,
}

#[derive(Default)]
struct CapturingVisitor {
    fields: Vec<CapturedField>,
}

struct PolicyCapturingVisitor {
    policy: FieldPolicy,
    fields: Vec<CapturedField>,
}

impl ObservationFieldVisitor for CapturingVisitor {
    fn field<T: serde::Serialize + ?Sized>(
        &mut self,
        name: &'static str,
        meta: FieldMeta,
        value: &T,
    ) {
        let value = match serde_json::to_value(value) {
            Ok(value) => value,
            Err(err) => panic!("field should serialize: {err}"),
        };
        self.fields.push(CapturedField { name, meta, value });
    }
}

impl ObservationFieldVisitor for PolicyCapturingVisitor {
    fn field<T: serde::Serialize + ?Sized>(
        &mut self,
        name: &'static str,
        meta: FieldMeta,
        value: &T,
    ) {
        if !self.policy.allows(meta) {
            return;
        }

        let value = match serde_json::to_value(value) {
            Ok(value) => value,
            Err(err) => panic!("allowed field should serialize: {err}"),
        };
        self.fields.push(CapturedField { name, meta, value });
    }
}

#[test]
fn derive_visits_annotated_fields_with_metadata() {
    let event = TurnConfigResolved {
        thread_id: "thread-1",
        turn_id: "turn-1",
        model: "gpt-5.4",
    };

    let mut visitor = CapturingVisitor::default();
    event.visit_fields(&mut visitor);

    assert_eq!(TurnConfigResolved::NAME, "turn.config_resolved");
    assert_eq!(
        visitor.fields,
        vec![
            CapturedField {
                name: "thread_id",
                meta: FieldMeta::new(DetailLevel::Basic, DataClass::Identifier),
                value: Value::String("thread-1".to_string()),
            },
            CapturedField {
                name: "turn_id",
                meta: FieldMeta::new(DetailLevel::Basic, DataClass::Identifier),
                value: Value::String("turn-1".to_string()),
            },
            CapturedField {
                name: "model",
                meta: FieldMeta::new(DetailLevel::Basic, DataClass::Operational),
                value: Value::String("gpt-5.4".to_string()),
            },
        ]
    );
}

#[test]
fn policy_visitor_does_not_serialize_denied_fields() {
    let event = PolicyFiltered {
        thread_id: "thread-1",
        status: "completed",
        raw_prompt: PanicsIfSerialized,
        api_key: PanicsIfSerialized,
    };
    let mut visitor = PolicyCapturingVisitor {
        policy: FieldPolicy::new(
            DetailLevel::Basic,
            &[DataClass::Identifier, DataClass::Operational],
        ),
        fields: Vec::new(),
    };

    event.visit_fields(&mut visitor);

    assert_eq!(
        visitor.fields,
        vec![
            CapturedField {
                name: "thread_id",
                meta: FieldMeta::new(DetailLevel::Basic, DataClass::Identifier),
                value: Value::String("thread-1".to_string()),
            },
            CapturedField {
                name: "status",
                meta: FieldMeta::new(DetailLevel::Basic, DataClass::Operational),
                value: Value::String("completed".to_string()),
            },
        ]
    );
}
