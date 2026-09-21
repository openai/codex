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
    assert!(screen(&mut view, /*width*/ 90, /*height*/ 30).contains("-0.000004"));
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
    chats.rows.push(Chat {
        title: "Plan-covered task".into(),
        task: Some(serde_json::from_value(json!({
            "thread_id":"plan", "data_status":"available", "usage_source":"included_plan",
            "weekly_limit_percent":12.1,"five_hour_limit_percent":null,"balance_usage_credits":"0E-10","groups":[]
        })).unwrap()),
    });
    for (raw, expected) in [
        ("0E-10", "0.00"),
        ("0.0000000000", "0.00"),
        ("-0e+10", "0.00"),
        ("1e-1000", "1e-1000"),
        ("-1e-1000", "-1e-1000"),
        ("1e1000", "1e1000"),
        ("12.3", "12.30"),
        ("12.3400000000", "12.34"),
        ("12.346", "12.35"),
        ("2.675", "2.68"),
        ("-2.675", "-2.68"),
        ("1.005", "1.01"),
        ("1.0049999999", "1.00"),
        ("9.999", "10.00"),
        ("0012.3", "12.30"),
        ("1.23e2", "123.00"),
        ("1234e-2", "12.34"),
        ("9007199254740993", "9007199254740993"),
        ("999999999999999999", "999999999999999999"),
        ("141436", "141,436.00"),
        ("0.01", "0.01"),
        ("-0.000004", "-0.000004"),
        ("1e-7", "1e-7"),
    ] {
        let amounts = serde_json::from_value(json!({"balance_usage_credits":raw})).unwrap();
        assert_eq!(task_panel::amount(Some(&amounts), /*metric*/ 2), expected);
    }
    assert_eq!(view.task_metrics(), vec![0, 1, 2]);
    insta::assert_snapshot!(
        "consumer_chat_all_metrics",
        screen(&mut view, /*width*/ 110, /*height*/ 30)
    );
    press(&mut view, KeyCode::Char('s'));
    assert_eq!(view.task_metric(), 1);
}

#[test]
fn consumer_chats_show_all_rows_and_collapse_details_before_closing() {
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
    screen(&mut view, /*width*/ 120, /*height*/ 42);
    press(&mut view, KeyCode::End);
    press(&mut view, KeyCode::Enter);
    assert_eq!(view.sections[Section::Chats].detail, Some(5));
    let content = view
        .task_lines(/*width*/ 70)
        .0
        .iter()
        .map(|line| line.to_string().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.contains("Task 5") && content.contains("Task 0"));
    press(&mut view, KeyCode::Esc);
    assert!(!view.is_done);
    press(&mut view, KeyCode::Esc);
    assert!(view.is_done);
}
