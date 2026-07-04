use super::*;
use crate::app_event::ExitMode;
use crate::app_event::PaneGeneration;
use crate::app_event::PaneSlot;
use pretty_assertions::assert_eq;

fn origin(pane: PaneSlot) -> ConversationOrigin {
    ConversationOrigin {
        pane,
        generation: PaneGeneration::fresh(),
    }
}

#[test]
fn current_generation_dispatches_to_its_conversation() {
    let current = origin(PaneSlot::Parent);
    let routed = normalize_conversation_event(
        AppEvent::FromConversation {
            target: current,
            event: Box::new(AppEvent::RawOutputModeChanged { enabled: true }),
        },
        Some(current),
    )
    .expect("current event should be delivered");

    assert_eq!(routed.0, Some(current));
    assert!(matches!(
        routed.1,
        AppEvent::RawOutputModeChanged { enabled: true }
    ));
}

#[test]
fn stale_presentation_event_is_dropped() {
    let stale = origin(PaneSlot::Parent);
    let current = origin(PaneSlot::Parent);

    assert!(
        normalize_conversation_event(
            AppEvent::FromConversation {
                target: stale,
                event: Box::new(AppEvent::RawOutputModeChanged { enabled: true }),
            },
            Some(current),
        )
        .is_none()
    );
}

#[test]
fn stale_widget_completion_is_dropped() {
    let stale = origin(PaneSlot::Parent);
    let current = origin(PaneSlot::Parent);

    assert!(
        normalize_conversation_event(
            AppEvent::FromConversation {
                target: stale,
                event: Box::new(AppEvent::PluginMentionsLoaded { plugins: None }),
            },
            Some(current),
        )
        .is_none()
    );
}

#[test]
fn stale_durable_event_is_delivered_without_pane_dispatch() {
    let stale = origin(PaneSlot::Side);
    let current = origin(PaneSlot::Parent);
    let routed = normalize_conversation_event(
        AppEvent::FromConversation {
            target: stale,
            event: Box::new(AppEvent::Exit(ExitMode::ShutdownFirst)),
        },
        Some(current),
    )
    .expect("durable event should be delivered");

    assert_eq!(routed.0, None);
    assert!(matches!(routed.1, AppEvent::Exit(ExitMode::ShutdownFirst)));
}
