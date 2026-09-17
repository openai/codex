//! Covers sparkle dismissal across ordinary and prepared-image user submissions.

use super::*;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use codex_app_server_protocol::ImageReference;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;

#[tokio::test]
async fn ordinary_and_prepared_image_submissions_consume_the_sparkle_before_rendering() {
    let image = UserInput::Image {
        image: ImageReference::Inline {
            url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/lFkAAAAASUVORK5CYII=".into(),
        },
        detail: None,
    };
    let text = "describe";
    let visible = |chat: &ChatWidget| {
        with_test_default_colors(
            DefaultColors {
                fg: (230, 216, 255),
                bg: (36, 27, 53),
            },
            || {
                render_bottom_popup(chat, /*width*/ 80)
                    .chars()
                    .any(|ch| "⠁⠂⠄⠈⠐⠠⡀⢀".contains(ch))
            },
        )
    };

    for prepared_images in [None, Some(vec![image])] {
        let (mut chat, _events, mut ops) = make_chatwidget_manual(Some("gpt-6-astra")).await;
        chat.thread_id = Some(ThreadId::new());
        chat.local_settings.tui.animations = true;
        chat.local_settings.tui.whimsy = true;
        chat.bottom_pane
            .mark_fresh_task_for_sparkle("gpt-6-astra", &chat.local_settings.tui);
        let untouched = chat.bottom_pane.composer_draft_snapshot().sparkle_draft;
        assert_eq!(untouched, Default::default());
        assert!(visible(&chat));

        let mut expected_items = prepared_images.clone().unwrap_or_default();
        expected_items.push(UserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        });
        let message = UserMessage::from(text);
        let (accepted, _command) = match prepared_images {
            Some(images) => chat.submit_user_message_with_prepared_images(
                message,
                UserMessageHistoryRecord::UserMessageText,
                ShellEscapePolicy::Disallow,
                UserMessageSource::Prompt,
                Some(images),
            ),
            None => chat.submit_user_message_with_history_and_shell_escape_policy(
                message,
                UserMessageHistoryRecord::UserMessageText,
                ShellEscapePolicy::Disallow,
                UserMessageSource::Prompt,
            ),
        };

        assert!(accepted);
        assert_ne!(
            chat.bottom_pane.composer_draft_snapshot().sparkle_draft,
            untouched
        );
        assert!(!visible(&chat));
        let Op::UserTurn { items, .. } = next_submit_op(&mut ops) else {
            unreachable!("next_submit_op only returns UserTurn")
        };
        assert_eq!(items, expected_items);
    }
}
