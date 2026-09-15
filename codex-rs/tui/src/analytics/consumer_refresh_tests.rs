//! Consumer chat columns follow reported metrics without substituting billing estimates.
use super::*;
use crate::analytics::tasks::Chat;
use crate::analytics::tasks::Chats;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn consumer_chats_keep_missing_rows_and_only_offer_known_metrics() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.section = Section::Chats;
    view.tasks = Load::Ready(Chats {
        rows: vec![
            Chat { title: "Private title from another account".into(), task: None },
            Chat { title: "Unverified backend row".into(), task: Some(serde_json::from_value(json!({
                "thread_id":"unavailable", "data_status":"unavailable", "usage_source":"unknown",
                "weekly_limit_percent":null,"five_hour_limit_percent":null,"balance_usage_credits":null,"groups":[]
            })).unwrap()) },
            Chat { title: "Refunded task".into(), task: Some(serde_json::from_value(json!({
                "thread_id":"refund", "data_status":"partial", "usage_source":"credits",
                "weekly_limit_percent":null,"five_hour_limit_percent":null,"balance_usage_credits":"-0.000004","groups":[{
                    "product_experience":"codex", "model":"gpt-5.5", "reasoning_effort":"high", "speed":"fast",
                    "weekly_limit_percent":null,"five_hour_limit_percent":null,"balance_usage_credits":"-0.000004"
                }]
            })).unwrap()) },
        ], ..Chats::default()
    });
    assert_eq!(view.task_metrics(), vec![2]);
    assert_eq!(
        view.task_rows()
            .iter()
            .map(|row| row.title.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Refunded task",
            "Private title from another account",
            "Unverified backend row"
        ]
    );
    press(&mut view, KeyCode::Down);
    press(&mut view, KeyCode::Enter);
    assert_eq!(view.sections[Section::Chats].detail, None);
    let hidden = screen(&mut view, /*width*/ 90, /*height*/ 30);
    assert!(!hidden.contains("Private title") && !hidden.contains("Unverified backend row"));
    press(&mut view, KeyCode::Up);
    press(&mut view, KeyCode::Enter);
    insta::assert_snapshot!(screen(&mut view, /*width*/ 90, /*height*/ 30));
    let chats = match &mut view.tasks {
        Load::Ready(chats) => chats,
        _ => unreachable!(),
    };
    chats.rows[2]
        .task
        .as_mut()
        .unwrap()
        .amounts
        .five_hour_limit_percent = Some(0.0);
    chats.rows[2]
        .task
        .as_mut()
        .unwrap()
        .amounts
        .weekly_limit_percent = Some(125.0);
    assert_eq!(view.task_metrics(), vec![0, 1, 2]);
    insta::assert_snapshot!(
        "consumer_chat_all_metrics",
        screen(&mut view, /*width*/ 110, /*height*/ 30)
    );
    press(&mut view, KeyCode::Char('s'));
    insta::assert_snapshot!(
        "consumer_chat_available_limits",
        screen(&mut view, /*width*/ 58, /*height*/ 30)
    );
}

#[test]
fn consumer_overview_shows_top_five_and_closes_with_retained_details() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.section = Section::Chats;
    view.tasks = Load::Ready(Chats {
        rows: (0..6).map(|index| Chat {
            title: format!("Task {index}"),
            task: Some(serde_json::from_value(json!({
                "thread_id":format!("task-{index}"), "data_status":"available", "usage_source":"credits",
                "balance_usage_credits":index.to_string(), "groups":[]
            })).unwrap()),
        }).collect(),
        ..Chats::default()
    });
    press(&mut view, KeyCode::End);
    press(&mut view, KeyCode::Enter);
    assert_eq!(view.sections[Section::Chats].detail, Some(5));
    press(&mut view, KeyCode::Char('z'));
    let content = view
        .task_lines(/*width*/ 70)
        .0
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.contains("Task 5") && !content.contains("Task 0"));
    insta::assert_snapshot!(content);
    press(&mut view, KeyCode::Esc);
    assert!(view.is_done);
}
