//! Vim navigation aliases share arrow behavior while preserving configured bindings.

use super::*;
use crate::analytics::models::AccountKind;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use pretty_assertions::assert_eq;
use serde_json::json;

fn navigate(views: &mut [AnalyticsView; 2], keys: &[(char, KeyCode)], size: (u16, u16)) -> String {
    let mut output = String::new();
    for &(alias, arrow) in keys {
        press(&mut views[0], KeyCode::Char(alias));
        press(&mut views[1], arrow);
        output = screen(&mut views[0], size.0, size.1);
        assert_eq!(output, screen(&mut views[1], size.0, size.1), "{alias}");
    }
    output
}

#[test]
fn vim_chart_navigation_matches_arrows_in_focused_and_dashboard_views() {
    for (kind, section) in [
        (AccountKind::Consumer, Section::Usage),
        (AccountKind::Consumer, Section::Activity),
        (AccountKind::Consumer, Section::Plugins),
        (AccountKind::Consumer, Section::Skills),
        (AccountKind::Business, Section::Usage),
        (AccountKind::Business, Section::Credits),
    ] {
        for zoomed in [true, false] {
            let mut views = [fixture::view(kind), fixture::view(kind)];
            for view in &mut views {
                view.section = section;
                view.zoomed = zoomed;
                view.sections[section].cursor = 3;
                view.sections[section].detail = Some(3);
            }
            navigate(
                &mut views,
                &[
                    ('h', KeyCode::Left),
                    ('h', KeyCode::Left),
                    ('l', KeyCode::Right),
                    ('j', KeyCode::Down),
                    ('k', KeyCode::Up),
                ],
                /*size*/ (96, 32),
            );
            assert_eq!(
                (
                    views[0].section,
                    views[0].sections[section].cursor,
                    views[0].sections[section].detail
                ),
                (section, 2, Some(2))
            );
        }
    }
}

#[test]
fn vim_plan_navigation_preserves_independent_windows_and_collapses_old_details() {
    let mut views = [
        fixture::view(AccountKind::Consumer),
        fixture::view(AccountKind::Consumer),
    ];
    for view in &mut views {
        let as_of = "2026-09-02T12:00:00Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap();
        view.plan.enabled = true;
        view.plan.report = Load::Ready(plan::Report {
            as_of,
            coverage_start: None,
            coverage_complete: true,
            approximate: false,
            periods: std::array::from_fn(|window| {
                let length = if window == 0 {
                    chrono::Duration::hours(/*hours*/ 5)
                } else {
                    chrono::Duration::days(/*days*/ 7)
                };
                (0..2)
                    .map(|index| plan::Period {
                        id: format!("{window}-{index}"),
                        start: as_of - length * index,
                        end: as_of - length * index + length,
                        complete: true,
                        used: Some(2500.0),
                        breakdowns: None,
                    })
                    .collect()
            }),
        });
        view.plan.poll();
        view.plan.expanded = [Some("0-0".into()), Some("1-0".into())];
        view.section = Section::Plan;
    }
    navigate(
        &mut views,
        &[
            ('l', KeyCode::Right),
            ('j', KeyCode::Down),
            ('k', KeyCode::Up),
            ('h', KeyCode::Left),
            ('j', KeyCode::Down),
        ],
        /*size*/ (120, 30),
    );
    assert_eq!(
        (
            views[0].plan.window,
            views[0].plan.cursor,
            &views[0].plan.expanded
        ),
        (0, [1, 0], &[None, None])
    );
}

#[test]
fn vim_summary_navigation_scrolls_like_arrows() {
    let mut views = [
        fixture::view(AccountKind::Consumer),
        fixture::view(AccountKind::Consumer),
    ];
    for view in &mut views {
        view.section = Section::Summary;
        view.profile = Load::Ready(
            serde_json::from_value(json!({
                "stats": {"lifetime_tokens": 123, "daily_usage_buckets": [
                    {"start_date": "2026-09-02", "tokens": 123}
                ]}
            }))
            .unwrap(),
        );
        assert!(screen(view, /*width*/ 64, /*height*/ 18).contains("scroll"));
    }
    navigate(
        &mut views,
        &[
            ('h', KeyCode::Left),
            ('l', KeyCode::Right),
            ('j', KeyCode::Down),
            ('j', KeyCode::Down),
            ('k', KeyCode::Up),
        ],
        /*size*/ (64, 18),
    );
    assert_eq!(views[0].scroll_offset, 1);
}

#[test]
fn vim_chat_navigation_expands_collapses_and_moves_like_arrows() {
    for kind in [AccountKind::Consumer, AccountKind::Business] {
        let mut views = [fixture::view(kind), fixture::view(kind)];
        for view in &mut views {
            view.section = Section::Chats;
            if kind == AccountKind::Consumer {
                view.tasks = Load::Ready(tasks::Chats {
                    rows: (0..2)
                        .map(|index| tasks::Chat {
                            title: format!("Example chat {index}"),
                            task: Some(
                                serde_json::from_value(json!({
                                    "thread_id": format!("chat-{index}"),
                                    "data_status": "available", "usage_source": "accounting",
                                    "weekly_limit_percent": 20 - index,
                                    "five_hour_limit_percent": null, "balance_usage_credits": null,
                                    "groups": []
                                }))
                                .unwrap(),
                            ),
                        })
                        .collect(),
                    ..tasks::Chats::default()
                });
            }
        }
        navigate(
            &mut views,
            &[
                ('l', KeyCode::Right),
                ('h', KeyCode::Left),
                ('l', KeyCode::Right),
                ('j', KeyCode::Down),
                ('l', KeyCode::Right),
                ('k', KeyCode::Up),
            ],
            /*size*/ (96, 32),
        );
        assert_eq!(
            (
                views[0].sections[Section::Chats].cursor,
                views[0].sections[Section::Chats].detail
            ),
            (0, None)
        );
    }
}

#[test]
fn configured_dashboard_accept_precedes_report_shortcuts() {
    for binding in ["r", "g", "m", "R", "tab", "1"] {
        let mut config = TuiKeymap::default();
        config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(binding.into())));
        let mut view = fixture::view(AccountKind::Business);
        view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
        view.section = Section::Usage;
        view.zoomed = false;
        let previous = (
            view.section,
            view.ranges,
            view.sections[Section::Usage].group,
        );
        let key = if binding == "tab" {
            KeyCode::Tab
        } else {
            KeyCode::Char(binding.chars().next().unwrap())
        };
        press(&mut view, key);
        assert_eq!(
            (
                view.zoomed,
                view.section,
                view.ranges,
                view.sections[Section::Usage].group
            ),
            (true, previous.0, previous.1, previous.2),
            "{binding}"
        );
    }
}

#[test]
fn configured_question_mark_keeps_its_action_and_hides_help_hint() {
    for (binding, modifiers) in [("?", KeyModifiers::NONE), ("shift-?", KeyModifiers::SHIFT)] {
        let mut config = TuiKeymap::default();
        config.list.cancel = Some(KeybindingsSpec::One(KeybindingSpec(binding.into())));
        let mut view = fixture::view(AccountKind::Consumer);
        view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
        let footer = view.footer_lines(/*width*/ 120, /*scrollable*/ false);
        assert!(
            !footer
                .iter()
                .any(|line| line.to_string().contains("? help"))
        );
        assert!(
            !view
                .help_lines(/*width*/ 120)
                .iter()
                .any(|line| line.to_string().contains("toggle this help"))
        );
        view.handle_key(KeyEvent::new(KeyCode::Char('?'), modifiers));
        assert_eq!((view.is_done, view.show_help), (true, false));
    }
}

#[test]
fn configured_vim_keys_win_over_horizontal_aliases() {
    let mut config = TuiKeymap::default();
    config.list.jump_top = Some(KeybindingsSpec::Many(vec![
        KeybindingSpec("h".into()),
        KeybindingSpec("l".into()),
    ]));
    let mut view = fixture::view(AccountKind::Consumer);
    view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
    for alias in ['h', 'l'] {
        view.sections[Section::Usage].cursor = 3;
        press(&mut view, KeyCode::Char(alias));
        assert_eq!(view.sections[Section::Usage].cursor, 0);
    }
}

#[test]
fn vim_aliases_respect_removed_and_remapped_arrows() {
    let mut config = TuiKeymap::default();
    config.list.move_left = Some(KeybindingsSpec::Many(Vec::new()));
    config.list.move_right = Some(KeybindingsSpec::Many(Vec::new()));
    config.list.move_up = Some(KeybindingsSpec::One(KeybindingSpec("up".into())));
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("down".into())));
    let mut view = fixture::view(AccountKind::Consumer);
    view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
    view.sections[Section::Usage].cursor = 3;
    for alias in ['h', 'j', 'k', 'l'] {
        press(&mut view, KeyCode::Char(alias));
        assert_eq!(view.sections[Section::Usage].cursor, 3);
    }

    config.list.move_up = Some(KeybindingsSpec::One(KeybindingSpec("right".into())));
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("left".into())));
    view.keymap = RuntimeKeymap::from_config(&config).unwrap().list;
    press(&mut view, KeyCode::Char('h'));
    assert_eq!(view.sections[Section::Usage].cursor, 4);
    press(&mut view, KeyCode::Char('l'));
    assert_eq!(view.sections[Section::Usage].cursor, 3);
}

#[test]
fn horizontal_aliases_ignore_modifiers_and_release_but_accept_repeat() {
    let mut view = fixture::view(AccountKind::Consumer);
    view.sections[Section::Usage].cursor = 3;
    for alias in ['h', 'l'] {
        for modifiers in [
            KeyModifiers::ALT,
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ] {
            view.handle_key(KeyEvent::new(KeyCode::Char(alias), modifiers));
            assert_eq!(view.sections[Section::Usage].cursor, 3);
        }
        view.handle_key(KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Char(alias), KeyModifiers::NONE)
        });
        assert_eq!(view.sections[Section::Usage].cursor, 3);
    }
    for (alias, cursor) in [('h', 2), ('l', 3)] {
        view.handle_key(KeyEvent {
            kind: KeyEventKind::Repeat,
            ..KeyEvent::new(KeyCode::Char(alias), KeyModifiers::NONE)
        });
        assert_eq!(view.sections[Section::Usage].cursor, cursor);
    }
}
