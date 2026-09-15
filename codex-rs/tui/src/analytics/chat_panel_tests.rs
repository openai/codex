//! Chat list coverage, responsive expansion, and controls for fixed-period data.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn chats_range_controls_and_visible_rows() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut chats = fixture::chats();
    let usage = chats.rows[0].usage.clone();
    chats.rows = (1..=30)
        .map(|index| chats::Chat {
            title: format!("Chat {index:02}"),
            usage: usage.clone(),
        })
        .collect();
    view.chats = Load::Ready(chats);
    press(&mut view, KeyCode::Char('6'));
    let before = (
        view.ranges,
        view.sections.0.each_ref().map(|state| state.cursor),
        view.sections.0.each_ref().map(|state| state.detail),
    );
    press(&mut view, KeyCode::Char('r'));
    assert_eq!(
        (
            view.ranges,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections.0.each_ref().map(|state| state.detail)
        ),
        before
    );
    let mut screens = Vec::new();
    for key in [KeyCode::Home, KeyCode::End, KeyCode::Enter] {
        press(&mut view, key);
        let output = screen(&mut view, /*width*/ 110, /*height*/ 24);
        assert!(!output.contains("7d") && !output.contains("r 7/30d"));
        let visible = (1..=30)
            .filter(|index| output.contains(&format!("Chat {index:02}")))
            .collect::<Vec<_>>();
        assert!(output.contains(&format!(
            "Showing {}–{} of 30 chats",
            visible.first().unwrap(),
            visible.last().unwrap()
        )));
        screens.push(output);
    }
    press(&mut view, KeyCode::Char('z'));
    press(&mut view, KeyCode::Char('r'));
    assert_eq!(view.ranges, [0; 3]);
    press(&mut view, KeyCode::Char('2'));
    press(&mut view, KeyCode::Char('r'));
    press(&mut view, KeyCode::Char('z'));
    assert!(screen(&mut view, /*width*/ 110, /*height*/ 24).contains("[30d]"));
    insta::assert_snapshot!(screens.join("\n"));
}

#[test]
fn chats_breakdown_ranks_nonzero_groups_and_reflows() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut chats = fixture::chats();
    let usage = chats.rows[0].usage.as_mut().unwrap();
    let seed = usage.groups[0].clone();
    usage.groups = [
        ("Zero model", 0),
        ("Small model", 215_610_920),
        ("Top model", 19_134_430_635),
        ("Refund model", -4),
    ]
    .map(
        |(model, credits)| codex_backend_client::ThreadUsageBreakdownGroup {
            model: Some(model.into()),
            speed: (credits != 19_134_430_635).then(|| "standard".into()),
            estimated_usage_credits_micros: credits,
            ..seed.clone()
        },
    )
    .to_vec();
    usage.estimated_usage_credits_micros = usage
        .groups
        .iter()
        .map(|group| group.estimated_usage_credits_micros)
        .sum();
    view.chats = Load::Ready(chats);
    press(&mut view, KeyCode::Char('6'));
    press(&mut view, KeyCode::Enter);
    let mut screens = Vec::new();
    for width in [110, 58] {
        view.follow_selection = true;
        let output = screen(&mut view, width, /*height*/ 32);
        assert!(output.find("Top model").unwrap() < output.find("Small model").unwrap());
        assert!(output.find("Small model").unwrap() < output.find("Refund model").unwrap());
        assert!(!output.contains("Zero model"));
        assert!(output.contains("-0.000004") && output.contains("Not reported"));
        assert!(output.contains("19,350.04") && output.contains("19,134.43"));
        assert!(output.contains("1 zero-credit group hidden · a show all"));
        assert!(output.contains("Refactor billing usage"));
        screens.push(output);
    }
    press(&mut view, KeyCode::Char('a'));
    let expanded = screen(&mut view, /*width*/ 110, /*height*/ 32);
    assert!(expanded.contains("Zero model") && expanded.contains("a hide zeros"));
    screens.push(expanded);
    press(&mut view, KeyCode::Char('a'));
    assert!(!view.show_zero_credit_groups);
    let wide = view.chat_lines(/*width*/ 220).0;
    assert!(wide.iter().all(|line| line.width() <= 96));
    insta::assert_snapshot!(screens.join("\n"));
}

#[test]
fn chats_long_title_keeps_detail_indent_and_counts_missing_estimates() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    let mut chats = fixture::chats();
    chats.rows[0].title = "Create a comprehensive list of areas to investigate across the runtime and describe how each area affects performance in a long conversation".into();
    chats.rows[1].usage = None;
    chats.rows[2].usage = None;
    view.chats = Load::Ready(chats);
    press(&mut view, KeyCode::Char('6'));
    press(&mut view, KeyCode::Enter);
    let output = screen(&mut view, /*width*/ 110, /*height*/ 30);
    assert!(output.contains("2 chat estimates unavailable"));
    insta::assert_snapshot!(output);
}

#[tokio::test]
async fn unavailable_chats_do_not_open_invisible_details() {
    for key in [KeyCode::Enter, KeyCode::Right] {
        for state in 0..5 {
            let mut view = fixture::view(models::AccountKind::Enterprise);
            view.section = Section::Chats;
            view.chats = match state {
                0 => Load::Unavailable,
                1 => Load::Error("Temporary failure".into()),
                2 => Load::Ready(chats::Chats::default()),
                3 => Load::Ready(chats::Chats {
                    rows: vec![chats::Chat {
                        title: "Another workspace's private title".into(),
                        usage: None,
                    }],
                }),
                _ => Load::start(std::future::pending(), FrameRequester::test_dummy()),
            };
            press(&mut view, key);
            assert_eq!(view.sections[Section::Chats].detail, None);
            press(&mut view, KeyCode::Esc);
            assert!(view.is_done);
        }
    }
}

#[test]
fn unavailable_chat_titles_are_hidden_in_panel_and_overview() {
    let mut view = fixture::view(models::AccountKind::Enterprise);
    view.section = Section::Chats;
    view.chats = Load::Ready(chats::Chats {
        rows: vec![chats::Chat {
            title: "Another workspace's private title".into(),
            usage: None,
        }],
    });
    let mut screens = Vec::new();
    for zoomed in [true, false] {
        view.zoomed = zoomed;
        view.follow_selection = true;
        let output = screen(&mut view, /*width*/ 110, /*height*/ 28);
        assert!(!output.contains("Another workspace's private title"));
        assert!(output.contains("Chat usage unavailable"));
        screens.push(output);
    }
    insta::assert_snapshot!(screens.join("\n"));
}

#[test]
fn refreshing_chats_clears_details_before_replacing_the_ranked_rows() {
    for unavailable in [true, false] {
        let mut view = fixture::view(models::AccountKind::Enterprise);
        view.section = Section::Chats;
        press(&mut view, KeyCode::Enter);
        assert_eq!(view.sections[Section::Chats].detail, Some(0));
        press(&mut view, KeyCode::Char('R'));
        assert_eq!(view.sections[Section::Chats].detail, None);
        view.account = Load::Ready(codex_protocol::account::PlanType::EnterpriseCbpUsageBased);
        view.chats = if unavailable {
            Load::Error("Temporarily unavailable".into())
        } else {
            Load::Ready(chats::Chats::default())
        };
        press(&mut view, KeyCode::Esc);
        assert!(view.is_done);
    }
}

#[test]
fn moving_chat_selection_collapses_details_before_they_leave_the_viewport() {
    for (start, key) in [
        (0, KeyCode::Down),
        (1, KeyCode::Up),
        (0, KeyCode::End),
        (2, KeyCode::Home),
    ] {
        let mut view = fixture::view(models::AccountKind::Enterprise);
        view.section = Section::Chats;
        view.sections[Section::Chats].cursor = start;
        press(&mut view, KeyCode::Enter);
        assert_eq!(view.sections[Section::Chats].detail, Some(start));
        press(&mut view, key);
        assert_eq!(view.sections[Section::Chats].detail, None);
        if key == KeyCode::End {
            insta::assert_snapshot!(screen(&mut view, /*width*/ 110, /*height*/ 24));
        }
        press(&mut view, KeyCode::Esc);
        assert!(view.is_done);
    }
}
