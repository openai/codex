use crate::codex::PreviousTurnSettings;
use crate::codex::TurnContext;
use crate::environment_context::EnvironmentContext;
use crate::features::Feature;
use crate::model_visible_context::ContextualUserContextRole;
use crate::model_visible_context::DEVELOPER_FRAGMENT_SPEC;
use crate::model_visible_context::DeveloperContextRole;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::ModelVisibleContextFragmentSpec;
use crate::model_visible_context::ModelVisibleContextRole;
use crate::model_visible_context::TurnContextDiffFragment;
use crate::model_visible_context::TurnContextDiffParams;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnContextItem;
use std::marker::PhantomData;

struct PermissionsUpdateFragment {
    instructions: DeveloperInstructions,
}

impl ModelVisibleContextFragment for PermissionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn spec(&self) -> ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        self.instructions.clone().into_text()
    }
}

impl TurnContextDiffFragment for PermissionsUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.sandbox_policy == *turn_context.sandbox_policy.get()
            && previous.approval_policy == turn_context.approval_policy.value()
        {
            return None;
        }

        Some(Self {
            instructions: DeveloperInstructions::from_policy(
                turn_context.sandbox_policy.get(),
                turn_context.approval_policy.value(),
                context.exec_policy,
                &turn_context.cwd,
                turn_context.features.enabled(Feature::RequestPermissions),
            ),
        })
    }
}

struct CollaborationModeUpdateFragment {
    instructions: DeveloperInstructions,
}

impl ModelVisibleContextFragment for CollaborationModeUpdateFragment {
    type Role = DeveloperContextRole;

    fn spec(&self) -> ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        self.instructions.clone().into_text()
    }
}

impl TurnContextDiffFragment for CollaborationModeUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.collaboration_mode.as_ref() != Some(&turn_context.collaboration_mode) {
            // If the next mode has empty developer instructions, this returns None and we emit no
            // update, so prior collaboration instructions remain in the prompt history.
            Some(Self {
                instructions: DeveloperInstructions::from_collaboration_mode(
                    &turn_context.collaboration_mode,
                )?,
            })
        } else {
            None
        }
    }
}

pub(crate) struct RealtimeUpdateFragment {
    instructions: DeveloperInstructions,
}

impl ModelVisibleContextFragment for RealtimeUpdateFragment {
    type Role = DeveloperContextRole;

    fn spec(&self) -> ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        self.instructions.clone().into_text()
    }
}

impl TurnContextDiffFragment for RealtimeUpdateFragment {
    fn from_turn_context(
        turn_context: &TurnContext,
        context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if turn_context.realtime_active {
            return Some(Self {
                instructions: DeveloperInstructions::realtime_start_message(),
            });
        }

        context
            .previous_turn_settings
            .and_then(|settings| settings.realtime_active)
            .filter(|realtime_active| *realtime_active)
            .map(|_| Self {
                instructions: DeveloperInstructions::realtime_end_message("inactive"),
            })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        match (previous.realtime_active, turn_context.realtime_active) {
            (Some(true), false) => Some(Self {
                instructions: DeveloperInstructions::realtime_end_message("inactive"),
            }),
            (Some(false), true) => Some(Self {
                instructions: DeveloperInstructions::realtime_start_message(),
            }),
            (Some(true), true) | (Some(false), false) | (None, false) | (None, true) => None,
        }
    }
}

struct PersonalityUpdateFragment {
    instructions: DeveloperInstructions,
}

impl ModelVisibleContextFragment for PersonalityUpdateFragment {
    type Role = DeveloperContextRole;

    fn spec(&self) -> ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        self.instructions.clone().into_text()
    }
}

impl TurnContextDiffFragment for PersonalityUpdateFragment {
    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if !context.personality_feature_enabled {
            return None;
        }
        if turn_context.model_info.slug != previous.model {
            return None;
        }

        if let Some(personality) = turn_context.personality
            && turn_context.personality != previous.personality
        {
            let model_info = &turn_context.model_info;
            let personality_message = personality_message_for(model_info, personality)?;
            Some(Self {
                instructions: DeveloperInstructions::personality_spec_message(personality_message),
            })
        } else {
            None
        }
    }
}

pub(crate) struct ModelInstructionsUpdateFragment {
    instructions: DeveloperInstructions,
}

impl ModelVisibleContextFragment for ModelInstructionsUpdateFragment {
    type Role = DeveloperContextRole;

    fn spec(&self) -> ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        self.instructions.clone().into_text()
    }
}

impl TurnContextDiffFragment for ModelInstructionsUpdateFragment {
    fn from_turn_context(
        turn_context: &TurnContext,
        context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        let previous_turn_settings = context.previous_turn_settings?;
        if previous_turn_settings.model == turn_context.model_info.slug {
            return None;
        }

        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        if model_instructions.is_empty() {
            return None;
        }

        Some(Self {
            instructions: DeveloperInstructions::model_switch_message(model_instructions),
        })
    }

    fn diff_from_turn_context_item(
        previous: &TurnContextItem,
        turn_context: &TurnContext,
        _context: &TurnContextDiffParams<'_>,
    ) -> Option<Self> {
        if previous.model == turn_context.model_info.slug {
            return None;
        }

        let model_instructions = turn_context
            .model_info
            .get_model_instructions(turn_context.personality);
        if model_instructions.is_empty() {
            return None;
        }

        Some(Self {
            instructions: DeveloperInstructions::model_switch_message(model_instructions),
        })
    }
}

pub(crate) fn personality_message_for(
    model_info: &ModelInfo,
    personality: Personality,
) -> Option<String> {
    model_info
        .model_messages
        .as_ref()
        .and_then(|spec| spec.get_personality_message(Some(personality)))
        .filter(|message| !message.is_empty())
}

pub(crate) fn build_model_instructions_update_item(
    previous_turn_settings: Option<&PreviousTurnSettings>,
    turn_context: &TurnContext,
) -> Option<DeveloperInstructions> {
    let previous_turn_settings = previous_turn_settings?;
    if previous_turn_settings.model == turn_context.model_info.slug {
        return None;
    }

    let model_instructions = turn_context
        .model_info
        .get_model_instructions(turn_context.personality);
    if model_instructions.is_empty() {
        return None;
    }

    Some(DeveloperInstructions::model_switch_message(
        model_instructions,
    ))
}

fn build_permissions_update_item(
    previous: Option<&TurnContextItem>,
    turn_context: &TurnContext,
    context: &TurnContextDiffParams<'_>,
) -> Option<DeveloperInstructions> {
    previous
        .and_then(|previous| {
            PermissionsUpdateFragment::diff_from_turn_context_item(previous, turn_context, context)
        })
        .map(|fragment| fragment.instructions)
}

fn build_collaboration_mode_update_item(
    previous: Option<&TurnContextItem>,
    turn_context: &TurnContext,
    context: &TurnContextDiffParams<'_>,
) -> Option<DeveloperInstructions> {
    previous
        .and_then(|previous| {
            CollaborationModeUpdateFragment::diff_from_turn_context_item(
                previous,
                turn_context,
                context,
            )
        })
        .map(|fragment| fragment.instructions)
}

pub(crate) fn build_realtime_update_item(
    previous: Option<&TurnContextItem>,
    previous_turn_settings: Option<&PreviousTurnSettings>,
    turn_context: &TurnContext,
) -> Option<DeveloperInstructions> {
    match (
        previous.and_then(|item| item.realtime_active),
        turn_context.realtime_active,
    ) {
        (Some(true), false) => Some(DeveloperInstructions::realtime_end_message("inactive")),
        (Some(false), true) | (None, true) => Some(DeveloperInstructions::realtime_start_message()),
        (Some(true), true) | (Some(false), false) => None,
        (None, false) => previous_turn_settings
            .and_then(|settings| settings.realtime_active)
            .filter(|realtime_active| *realtime_active)
            .map(|_| DeveloperInstructions::realtime_end_message("inactive")),
    }
}

fn build_personality_update_item(
    previous: Option<&TurnContextItem>,
    turn_context: &TurnContext,
    context: &TurnContextDiffParams<'_>,
) -> Option<DeveloperInstructions> {
    previous
        .and_then(|previous| {
            PersonalityUpdateFragment::diff_from_turn_context_item(previous, turn_context, context)
        })
        .map(|fragment| fragment.instructions)
}

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
        role: R::RESPONSE_ROLE.to_string(),
        content,
        end_turn: None,
        phase: None,
    })
}

pub(crate) fn build_settings_update_items(
    previous: Option<&TurnContextItem>,
    next: &TurnContext,
    context: &TurnContextDiffParams<'_>,
) -> Vec<ResponseItem> {
    let mut developer_envelope = DeveloperEnvelopeBuilder::default();
    for fragment in [
        // Keep model-switch instructions first so model-specific guidance is read before
        // any other context diffs on this turn.
        ModelInstructionsUpdateFragment::from_turn_context(next, context)
            .map(|fragment| fragment.instructions),
        build_permissions_update_item(previous, next, context),
        build_collaboration_mode_update_item(previous, next, context),
        previous
            .and_then(|previous| {
                RealtimeUpdateFragment::diff_from_turn_context_item(previous, next, context)
            })
            .or_else(|| RealtimeUpdateFragment::from_turn_context(next, context))
            .map(|fragment| fragment.instructions),
        build_personality_update_item(previous, next, context),
    ]
    .into_iter()
    .flatten()
    {
        developer_envelope.push(fragment);
    }
    let mut contextual_user_envelope = ContextualUserEnvelopeBuilder::default();
    for fragment in [
        // Add new contextual-user diff fragments here.
        previous.and_then(|previous| {
            EnvironmentContext::diff_from_turn_context_item(previous, next, context)
        }),
    ]
    .into_iter()
    .flatten()
    {
        contextual_user_envelope.push_fragment(fragment);
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
    use crate::model_visible_context::ModelVisibleContextFragmentSpec;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn developer_envelope_builder_emits_one_message_in_order() {
        let mut builder = DeveloperEnvelopeBuilder::default();
        builder.push(DeveloperInstructions::new("first"));
        builder.push(DeveloperInstructions::new("second"));

        let item = builder.build().expect("developer message expected");
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected message");
        };

        assert_eq!(role, "developer");
        assert_eq!(
            content,
            vec![
                ContentItem::InputText {
                    text: "first".to_string()
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

        fn spec(&self) -> ModelVisibleContextFragmentSpec {
            ModelVisibleContextFragmentSpec::contextual_user("<fake>", "</fake>")
        }

        fn render_text(&self) -> String {
            self.text.to_string()
        }
    }

    #[derive(Clone, Copy)]
    struct FakeDeveloperFragment {
        text: &'static str,
    }

    impl ModelVisibleContextFragment for FakeDeveloperFragment {
        type Role = DeveloperContextRole;

        fn spec(&self) -> ModelVisibleContextFragmentSpec {
            ModelVisibleContextFragmentSpec::markerless()
        }

        fn render_text(&self) -> String {
            self.text.to_string()
        }
    }

    #[test]
    fn contextual_user_envelope_builder_emits_one_message_in_order() {
        let mut builder = ContextualUserEnvelopeBuilder::default();
        builder.push_fragment(FakeFragment { text: "first" });
        builder.push_fragment(FakeFragment { text: "second" });

        let item = builder.build().expect("user message expected");
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected message");
        };

        assert_eq!(role, "user");
        assert_eq!(
            content,
            vec![
                ContentItem::InputText {
                    text: "first".to_string()
                },
                ContentItem::InputText {
                    text: "second".to_string()
                },
            ]
        );
    }

    #[test]
    fn developer_envelope_builder_emits_one_message_with_custom_fragments() {
        let mut builder = DeveloperEnvelopeBuilder::default();
        builder.push(FakeDeveloperFragment { text: "first" });
        builder.push(FakeDeveloperFragment { text: "second" });

        let item = builder.build().expect("developer message expected");
        let ResponseItem::Message { role, content, .. } = item else {
            panic!("expected message");
        };

        assert_eq!(role, "developer");
        assert_eq!(
            content,
            vec![
                ContentItem::InputText {
                    text: "first".to_string()
                },
                ContentItem::InputText {
                    text: "second".to_string()
                },
            ]
        );
    }
}
