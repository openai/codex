use crate::codex::TurnContext;
use crate::model_visible_context::ContextualUserContextRole;
use crate::model_visible_context::DeveloperContextRole;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::ModelVisibleContextRole;
use crate::model_visible_context::TurnContextDiffParams;
use crate::model_visible_fragments::BuiltTurnStateFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnContextItem;
use std::marker::PhantomData;

// Adjacent ContentItems in a single message are effectively concatenated in
// the model-visible token stream, so we inject an explicit separator between
// text fragments to preserve boundaries.
const MODEL_VISIBLE_FRAGMENT_SEPARATOR: &str = "\n\n";

struct ModelVisibleContextEnvelopeBuilder<R: ModelVisibleContextRole> {
    content: Vec<ContentItem>,
    role: PhantomData<R>,
}

impl<R: ModelVisibleContextRole> ModelVisibleContextEnvelopeBuilder<R> {
    fn new() -> Self {
        Self {
            content: Vec::new(),
            role: PhantomData,
        }
    }

    fn push_fragment(&mut self, fragment: impl ModelVisibleContextFragment<Role = R>) {
        if let Some(ContentItem::InputText { text }) = self.content.last_mut()
            && !text.ends_with(MODEL_VISIBLE_FRAGMENT_SEPARATOR)
        {
            text.push_str(MODEL_VISIBLE_FRAGMENT_SEPARATOR);
        }
        self.content.push(fragment.into_content_item());
    }

    fn build(self) -> Option<ResponseItem> {
        build_message::<R>(self.content)
    }
}

pub(crate) struct DeveloperEnvelopeBuilder(
    ModelVisibleContextEnvelopeBuilder<DeveloperContextRole>,
);

impl Default for DeveloperEnvelopeBuilder {
    fn default() -> Self {
        Self(ModelVisibleContextEnvelopeBuilder::new())
    }
}

impl DeveloperEnvelopeBuilder {
    pub(crate) fn push(
        &mut self,
        fragment: impl ModelVisibleContextFragment<Role = DeveloperContextRole>,
    ) {
        self.0.push_fragment(fragment);
    }

    pub(crate) fn build(self) -> Option<ResponseItem> {
        self.0.build()
    }
}

pub(crate) struct ContextualUserEnvelopeBuilder(
    ModelVisibleContextEnvelopeBuilder<ContextualUserContextRole>,
);

impl Default for ContextualUserEnvelopeBuilder {
    fn default() -> Self {
        Self(ModelVisibleContextEnvelopeBuilder::new())
    }
}

impl ContextualUserEnvelopeBuilder {
    pub(crate) fn push_fragment(
        &mut self,
        fragment: impl ModelVisibleContextFragment<Role = ContextualUserContextRole>,
    ) {
        self.0.push_fragment(fragment);
    }

    pub(crate) fn build(self) -> Option<ResponseItem> {
        self.0.build()
    }
}

fn build_message<R: ModelVisibleContextRole>(content: Vec<ContentItem>) -> Option<ResponseItem> {
    if content.is_empty() {
        return None;
    }

    Some(ResponseItem::Message {
        id: None,
        role: R::MESSAGE_ROLE.to_string(),
        content,
        end_turn: None,
        phase: None,
    })
}

pub(crate) fn build_settings_update_items(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
    params: &TurnContextDiffParams<'_>,
) -> Vec<ResponseItem> {
    let mut developer_envelope = DeveloperEnvelopeBuilder::default();
    let mut contextual_user_envelope = ContextualUserEnvelopeBuilder::default();

    for fragment in
        crate::model_visible_fragments::build_turn_state_fragments(previous, next, params)
    {
        match fragment {
            BuiltTurnStateFragment::Developer(fragment) => developer_envelope.push(fragment),
            BuiltTurnStateFragment::ContextualUser(fragment) => {
                contextual_user_envelope.push_fragment(fragment);
            }
        }
    }

    let mut items = Vec::with_capacity(2);
    if let Some(developer_message) = developer_envelope.build() {
        items.push(developer_message);
    }
    if let Some(model_visible_context) = contextual_user_envelope.build() {
        items.push(model_visible_context);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_visible_context::ContextualUserContextRole;
    use crate::model_visible_context::DeveloperContextRole;
    use crate::model_visible_context::DeveloperTextFragment;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn developer_envelope_builder_emits_one_message_in_order() {
        let mut builder = DeveloperEnvelopeBuilder::default();
        builder.push(DeveloperTextFragment::new("first"));
        builder.push(DeveloperTextFragment::new("second"));

        let item = builder.build().expect("developer message expected");
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected message");
        };

        assert_eq!(role, "developer");
        assert_eq!(
            content,
            vec![
                ContentItem::InputText {
                    text: "first\n\n".to_string()
                },
                ContentItem::InputText {
                    text: "second".to_string()
                },
            ]
        );
    }

    #[derive(Clone, Copy)]
    struct FakeFragment {
        text: &'static str,
    }

    impl ModelVisibleContextFragment for FakeFragment {
        type Role = ContextualUserContextRole;

        fn render_text(&self) -> String {
            self.text.to_string()
        }
    }

    #[test]
    fn contextual_user_envelope_builder_emits_one_message_in_order() {
        let mut builder = ContextualUserEnvelopeBuilder::default();
        builder.push_fragment(FakeFragment { text: "alpha" });
        builder.push_fragment(FakeFragment { text: "beta" });

        let item = builder.build().expect("user message expected");
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected message");
        };

        assert_eq!(role, "user");
        assert_eq!(
            content,
            vec![
                ContentItem::InputText {
                    text: "alpha\n\n".to_string()
                },
                ContentItem::InputText {
                    text: "beta".to_string()
                },
            ]
        );
    }

    #[test]
    fn empty_envelope_builders_return_none() {
        assert!(DeveloperEnvelopeBuilder::default().build().is_none());
        assert!(ContextualUserEnvelopeBuilder::default().build().is_none());
    }

    #[test]
    fn build_message_sets_role() {
        let item = build_message::<DeveloperContextRole>(vec![ContentItem::InputText {
            text: "body".to_string(),
        }])
        .expect("message expected");
        let ResponseItem::Message { role, .. } = item else {
            panic!("expected message");
        };
        assert_eq!(role, "developer");
    }
}
