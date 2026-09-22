use super::*;
use pretty_assertions::assert_eq;

#[test]
fn experimental_features_keep_following_choice_at_wrap_boundary() {
    let (app_tx, _app_rx) = tokio::sync::mpsc::unbounded_channel();
    let view = ExperimentalFeaturesView::new(
        vec![
            ExperimentalFeatureItem {
                key: "first".into(),
                name: "First feature".into(),
                description: "A description that fits exactly at this boundary.".into(),
                enabled: true,
                writable: true,
            },
            ExperimentalFeatureItem {
                key: "last".into(),
                name: "Last feature".into(),
                description: "Keep this choice visible.".into(),
                enabled: false,
                writable: true,
            },
        ],
        ThreadId::new(),
        /*catalog_rx*/ None,
        AppEventSender::new(app_tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );

    let mut snapshots = Vec::new();
    for width in [68, 69, 70, 71, 72] {
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            view.desired_height(width),
        );
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer);
        let text = buffer_text(&buffer);
        assert!(
            text.contains("Last feature"),
            "missing last choice at width {width}"
        );
        snapshots.push(format!("width={width}\n{text}"));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn experimental_features_analytics_plan_history() {
    let feature = Feature::AnalyticsPlanHistory;
    let stage = feature.stage();
    let (app_tx, _app_rx) = tokio::sync::mpsc::unbounded_channel();
    let view = ExperimentalFeaturesView::new(
        vec![ExperimentalFeatureItem {
            key: feature.key().to_string(),
            name: stage.experimental_menu_name().unwrap().to_string(),
            description: stage.experimental_menu_description().unwrap().to_string(),
            enabled: feature.default_enabled(),
            writable: true,
        }],
        ThreadId::new(),
        /*catalog_rx*/ None,
        AppEventSender::new(app_tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    snapshot_view("experimental_features_analytics_plan_history", &view);
}

fn server_feature(name: &str) -> ExperimentalFeature {
    ExperimentalFeature {
        name: name.to_string(),
        stage: ExperimentalFeatureStage::Beta,
        display_name: Some(format!("Server {name}")),
        description: Some("Description supplied by the server.".to_string()),
        announcement: None,
        enabled: true,
        default_enabled: false,
    }
}

#[test]
fn experimental_features_discovery_preserves_server_order_and_accepts_new_keys() {
    let (app_tx, mut app_rx) = tokio::sync::mpsc::unbounded_channel();
    let (catalog_tx, catalog_rx) = oneshot::channel();
    let mut view = ExperimentalFeaturesView::new(
        Vec::new(),
        ThreadId::new(),
        Some(catalog_rx),
        AppEventSender::new(app_tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    view.toggle_selected();
    let mut stable = server_feature("stable_feature");
    stable.stage = ExperimentalFeatureStage::Stable;
    catalog_tx
        .send(Ok(vec![
            server_feature("network_proxy"),
            server_feature("future_feature"),
            stable,
            server_feature("prevent_idle_sleep"),
        ]))
        .unwrap();
    assert!(view.pre_draw_tick(Instant::now()));
    assert_eq!(
        view.features
            .iter()
            .map(|item| (item.key.as_str(), item.enabled))
            .collect::<Vec<_>>(),
        vec![
            ("network_proxy", true),
            ("future_feature", true),
            ("prevent_idle_sleep", true)
        ]
    );
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 100,
        view.desired_height(/*width*/ 100),
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer);
    insta::assert_snapshot!(
        "experimental_features_server_discovery",
        buffer_text(&buffer)
    );
    // New server features are writable without a local Feature enum variant.
    view.state.selected_idx = Some(1);
    view.toggle_selected();
    assert!(!view.features[1].enabled);
    view.state.selected_idx = Some(0);
    view.toggle_selected();
    view.features[2].enabled = false;
    view.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(
        matches!(app_rx.try_recv().unwrap(), AppEvent::UpdateFeatureFlags { updates }
        if updates == vec![(Feature::PreventIdleSleep, false)])
    );
    let AppEvent::SaveExperimentalFeatures {
        updates,
        response_tx,
        ..
    } = app_rx.try_recv().unwrap()
    else {
        panic!("expected feature edits");
    };
    assert_eq!(
        updates,
        vec![
            ("network_proxy".to_string(), false),
            ("future_feature".to_string(), false)
        ]
    );
    snapshot_view("experimental_features_saving", &view);
    response_tx
        .send(Err("Saved but readback failed".to_string()))
        .unwrap();
    view.pre_draw_tick(Instant::now());
    snapshot_view("experimental_features_save_failed", &view);

    // Reverting uncertain saves to their original values must still write them.
    view.features[0].enabled = true;
    view.features[1].enabled = true;
    view.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::SaveExperimentalFeatures {
        updates,
        response_tx,
        ..
    } = app_rx.try_recv().unwrap()
    else {
        panic!("expected corrective save without repeating special controls");
    };
    assert_eq!(
        updates,
        vec![
            ("network_proxy".to_string(), true),
            ("future_feature".to_string(), true)
        ]
    );
    response_tx
        .send(Err("Persistent write rejection".to_string()))
        .unwrap();
    view.pre_draw_tick(Instant::now());
    // Cancel saves new special edits without retrying the failed generic write.
    view.features[2].enabled = true;
    view.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(view.is_complete());
    assert!(
        matches!(app_rx.try_recv().unwrap(), AppEvent::UpdateFeatureFlags { updates }
        if updates == vec![(Feature::PreventIdleSleep, true)])
    );
    assert!(app_rx.try_recv().is_err());
}

#[test]
fn experimental_features_empty_error_and_cancel_remain_usable() {
    let mut changed_default = server_feature("network_proxy");
    changed_default.default_enabled = true;
    for (snapshot, result) in [
        ("experimental_features_empty", Ok(Vec::new())),
        (
            "experimental_features_unavailable",
            Err("method not found".to_string()),
        ),
        (
            "experimental_features_changed_default",
            Ok(vec![changed_default]),
        ),
    ] {
        let (app_tx, mut app_rx) = tokio::sync::mpsc::unbounded_channel();
        let (catalog_tx, catalog_rx) = oneshot::channel();
        let mut view = ExperimentalFeaturesView::new(
            Vec::new(),
            ThreadId::new(),
            Some(catalog_rx),
            AppEventSender::new(app_tx),
            crate::keymap::RuntimeKeymap::defaults().list,
        );
        assert!(view.next_frame_delay().is_some());
        catalog_tx.send(result).unwrap();
        assert!(view.pre_draw_tick(Instant::now()));
        assert_eq!(view.next_frame_delay(), None);
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            /*width*/ 80,
            view.desired_height(/*width*/ 80),
        );
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer);
        insta::assert_snapshot!(snapshot, buffer_text(&buffer));
        view.on_ctrl_c();
        assert!(view.is_complete());
        assert!(app_rx.try_recv().is_err());
    }
    let (app_tx, _app_rx) = tokio::sync::mpsc::unbounded_channel();
    let (catalog_tx, catalog_rx) = oneshot::channel();
    let view = ExperimentalFeaturesView::new(
        Vec::new(),
        ThreadId::new(),
        Some(catalog_rx),
        AppEventSender::new(app_tx),
        crate::keymap::RuntimeKeymap::defaults().list,
    );
    drop(view);
    assert!(catalog_tx.send(Ok(Vec::new())).is_err());
}

fn buffer_text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn snapshot_view(name: &str, view: &ExperimentalFeaturesView) {
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 80,
        view.desired_height(/*width*/ 80),
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer);
    insta::assert_snapshot!(name, buffer_text(&buffer));
}

#[test]
fn voice_discovery_requires_the_client_runtime() {
    for supported in [false, true] {
        let (app_tx, _app_rx) = tokio::sync::mpsc::unbounded_channel();
        let (catalog_tx, catalog_rx) = oneshot::channel();
        let mut view = ExperimentalFeaturesView::new(
            Vec::new(),
            ThreadId::new(),
            Some(catalog_rx),
            AppEventSender::new(app_tx),
            crate::keymap::RuntimeKeymap::defaults().list,
        );
        view.voice_supported = supported;
        catalog_tx
            .send(Ok(vec![server_feature("realtime_conversation")]))
            .unwrap();
        assert!(view.pre_draw_tick(Instant::now()));
        assert_eq!(view.features.len(), usize::from(supported));
        snapshot_view(
            if supported {
                "voice_runtime_available"
            } else {
                "voice_runtime_unavailable"
            },
            &view,
        );
    }
}
