#![cfg(test)]

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::sidebar::SidebarPane;
use super::test_support::enable_side_retention;
use super::test_support::side_frame;
use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::test_support::boxed_future;
use codex_app_server_client::AppServerPath;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimeOptions;
use codex_utils_path_uri::PathUri;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::Semaphore;

#[tokio::test]
async fn open_in_codex_reveals_a_file_when_the_calling_thread_returns() -> Result<()> {
    core_test_support::skip_if_sandbox!(Ok(()));
    boxed_future!(open_in_codex_scenario()).await
}

async fn open_in_codex_scenario() -> Result<()> {
    let model = responses::start_mock_server().await;
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let home = tempfile::tempdir()?;
    let workspace = tempfile::tempdir()?;
    let path = workspace.path().join("PLAN.md");
    std::fs::write(
        &path,
        format!(
            "# Navigation\n{}Use the sidebar.\n",
            "Before the target.\n".repeat(40)
        ),
    )?;
    let denied_plan = serde_json::to_string(&dunce::canonicalize(&path)?)?;
    app_test_support::MockResponsesConfig::new(&model.uri())
        .enable_feature(Feature::TuiWorkspace)
        .with_extra_config(&format!(
            "[permissions.no-plan.filesystem]\n\":root\" = \"read\"\n{denied_plan} = \"deny\"",
        ))
        .write(home.path())?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(dunce::canonicalize(workspace.path())?),
            ..Default::default()
        })
        .build()
        .await?;
    app.config = config.clone();
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;

    #[cfg(target_os = "linux")]
    let linux_sandbox_exe = Some(core_test_support::find_codex_linux_sandbox_exe()?);
    #[cfg(not(target_os = "linux"))]
    let linux_sandbox_exe = None;
    let environment = EnvironmentManager::create_for_tests_with_local(
        /*exec_server_url*/ None,
        ExecServerRuntimeOptions::new(
            core_test_support::exec_server_helper_exe()?,
            linux_sandbox_exe,
        )?,
    )
    .await;
    app.environment_manager = Arc::new(environment);
    let mut server = crate::start_app_server_for_picker(
        crate::backend_id::BackendId::new(),
        &config,
        &crate::AppServerTarget::Embedded,
        Vec::new(),
        codex_config::LoaderOverrides::without_managed_config_for_tests(),
        /*state_db*/ None,
        Arc::clone(&app.environment_manager),
    )
    .await?;
    server
        .start_dynamic_tool_mcp(
            config.clone(),
            app.local_settings.workspace_enabled(&config),
            app.app_event_tx.clone(),
            app.dynamic_tool_status_updates.clone(),
        )
        .await?;
    let caller = server.start_thread(&config).await?;
    let caller_id = caller.session.thread_id;

    let outside = dunce::canonicalize(home.path())?.join("outside.txt");
    std::fs::write(&outside, "outside")?;
    let escape = config.cwd.join("escape.txt");
    std::os::unix::fs::symlink(&outside, &escape)?;
    let system_escape = config.cwd.join("hosts.txt");
    std::os::unix::fs::symlink("/etc/hosts", &system_escape)?;
    let fs = server.file_system();
    let permissions = server.model_preview_permissions(caller_id).await?.unwrap();
    let environment = app.environment_manager.try_local_environment().unwrap();
    let read_preview = async |path: &std::path::Path| {
        let uri = PathUri::from_host_native_path(path)?;
        super::sidebar_document::model_read::read(
            &fs,
            &environment,
            caller_id,
            &AppServerPath::from_app_server(uri.inferred_native_path_string()),
            &uri,
            &permissions,
        )
        .await
    };
    for denied in [outside.as_path(), escape.as_path(), system_escape.as_path()] {
        assert!(
            read_preview(denied).await.is_err(),
            "preview must reject {denied:?}"
        );
    }
    let oversized = config.cwd.join("large.md");
    std::fs::File::create(&oversized)?.set_len(8 * 1024 * 1024 + 1)?;
    let error = read_preview(oversized.as_path()).await.unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error}");
    let background = server.start_thread(&config).await?;
    let background_id = background.session.thread_id;
    let switch_to =
        async |app: &mut App, thread: &crate::app_server_session::AppServerStartedThread| {
            app.clear_active_thread().await;
            app.enqueue_primary_thread_session(thread.session.clone(), thread.turns.clone())
                .await
        };
    switch_to(&mut app, &background).await?;

    let call = json!({"target":{"type":"file","path":"PLAN.md","line":42}});
    let model_requests = responses::mount_sse_sequence(
        &model,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "open-plan",
                    "mcp__codex_tui",
                    "open_in_codex",
                    &call.to_string(),
                ),
                responses::ev_completed("open"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("reply", "I opened the plan."),
                responses::ev_completed("done"),
            ]),
        ],
    )
    .await;
    let _: TurnStartResponse = server
        .request_handle()
        .request_typed(ClientRequest::TurnStart {
            request_id: RequestId::String("open-plan-turn".into()),
            params: TurnStartParams {
                thread_id: caller_id.to_string(),
                input: vec![codex_app_server_protocol::UserInput::Text {
                    text: "Show me line 42 of the plan.".into(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    boxed_future!(super::tests::wait_for_turn_completed(
        &mut app,
        &mut server,
        caller_id
    ))
    .await;
    let requests = model_requests.requests();
    let output = requests[1].function_call_output("open-plan")["output"].to_string();
    assert!(output.contains(&caller_id.to_string()), "{output}");
    insta::assert_snapshot!(
        "open_in_codex_model",
        context_snapshot::format_request_history_snapshot(
            "The model opens a workspace file and receives a queued result while the calling thread is in the background.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );

    let size = Size::new(/*width*/ 120, /*height*/ 12);
    let mut tui = crate::tui::test_support::make_test_tui_with_size(size)?;
    tui.set_owned_screen(/*owned*/ true)?;
    while let Ok(event) = events.try_recv() {
        boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert_eq!(app.current_displayed_thread_id(), Some(background_id));
    let visible_content = |app: &App| app.sidebar().and_then(SidebarPane::visible_content);
    assert!(visible_content(&app).is_none());

    let slots = Arc::new(Semaphore::new(crate::open_in_codex::MAX_PENDING_DOCUMENTS));
    let backend = server.backend_id();
    let request = |thread_id, name: &str| AppEvent::OpenModelSidebarDocument {
        backend,
        thread_id,
        document: crate::open_in_codex::DocumentRequest {
            original_path: AppServerPath::from_app_server(name),
            path: PathUri::from_abs_path(&config.cwd.join(name)),
            location: None,
            _pending: Arc::clone(&slots).try_acquire_owned().unwrap(),
        },
    };
    // Even when one thread fills its queue, a request for the visible chat can be accepted.
    for index in 0..9 {
        boxed_future!(app.handle_event(
            &mut tui,
            &mut server,
            request(background_id, &format!("{index}.md")),
        ))
        .await?;
    }
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS - 8
    );

    switch_to(&mut app, &caller).await?;
    let narrow = Size::new(/*width*/ 80, /*height*/ size.height);
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Resize(narrow))).await?;
    assert!(visible_content(&app).is_none());

    // A pending user click takes priority until it resolves, even across a thread switch.
    app.open_document(
        &mut tui,
        &mut server,
        caller_id,
        crate::markdown_render::FileLink {
            path: "old.md".into(),
            location: None,
        },
    )
    .await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Resize(size))).await?;
    assert!(visible_content(&app).is_none());
    switch_to(&mut app, &background).await?;
    let mut finish_document_request =
        async |app: &mut App, tui: &mut tui::Tui, server: &mut AppServerSession| -> Result<()> {
            loop {
                let event = events.recv().await.expect("document event");
                let outcome = match &event {
                    AppEvent::SidebarDocumentLoaded {
                        result: Err(error), ..
                    } => Err(color_eyre::eyre::eyre!("{error:#}")),
                    AppEvent::SidebarDocumentLoaded { .. }
                    | AppEvent::SidebarDocumentResolved { .. } => Ok(()),
                    _ => continue,
                };
                boxed_future!(app.handle_event(tui, server, event)).await?;
                assert!(app.model_preview_load.is_none());
                return outcome;
            }
        };
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    switch_to(&mut app, &caller).await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert_eq!(app.panel_focus(), Some(PaneFocus::Main));
    assert!(matches!(
        visible_content(&app),
        Some(SidebarContent::Document(_))
    ));
    let first_load = app.model_preview_load.expect("model preview loading");
    let preview = |name: &str| request(caller_id, name);
    boxed_future!(app.handle_event(&mut tui, &mut server, preview("PLAN.md"))).await?;
    // Drop the pending requests in the other chat before returning to it.
    app.sidebars
        .get_mut(&Some(background_id))
        .unwrap()
        .pending_documents
        .clear();
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert_eq!(app.model_preview_load, Some(first_load));
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS - 1
    );
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    boxed_future!(app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Paste("Keep typing".into())
    ))
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "Keep typing");
    assert_eq!(app.current_displayed_thread_id(), Some(caller_id));
    let screen = side_frame(&mut app, &mut tui, size)?;
    let sidebar = screen
        .lines()
        .filter_map(|line| line.split_once('│').map(|(_, content)| content.trim_end()))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("open_in_codex_sidebar", sidebar.trim_end());

    assert!(
        server
            .thread_settings_update(ThreadSettingsUpdateParams {
                thread_id: caller_id.to_string(),
                permissions: Some("no-plan".into()),
                ..Default::default()
            })
            .await?
    );

    // A concurrently running side chat stays selected even when the parent has focus.
    app.sidebar_mut()
        .show(SidebarContent::Chat(background_id), PaneFocus::Main);
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS
    );
    assert!(app.model_preview_load.is_some());
    let error = finish_document_request(&mut app, &mut tui, &mut server)
        .await
        .expect_err("cached preview must be reauthorized");
    assert!(
        error.to_string().contains("permissions do not allow"),
        "{error}"
    );
    assert!(visible_content(&app) == Some(SidebarContent::Chat(background_id)));

    // The same rule applies to a docked agent when the parent sends the request.
    let note = config.cwd.join("note.md");
    std::fs::write(&note, "Another workspace file")?;
    let pane = app.sidebar_mut();
    pane.agents.detail = Some(background_id);
    pane.show(SidebarContent::Subagents, PaneFocus::Main);
    assert_eq!(app.docked_agent(), Some((caller_id, background_id)));
    boxed_future!(app.handle_event(&mut tui, &mut server, preview("note.md"))).await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    let note_tab = app.model_preview_load.expect("model preview loading");
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS,
        "open tabs use a separate budget"
    );
    assert_eq!(app.docked_agent(), Some((caller_id, background_id)));
    assert_eq!(app.current_displayed_thread_id(), Some(caller_id));
    let enter = TuiEvent::Key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
    app.sidebar_mut()
        .show(SidebarContent::Document(note_tab), PaneFocus::Sidebar);
    assert!(
        app.handle_document_input(&mut tui, &mut server, &enter)
            .await
    );
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    let doc = app.sidebar_mut().documents.get_mut(&note_tab).unwrap();
    let (watch, refresh) = doc.watch.as_ref().unwrap();
    let watch = *watch;
    std::fs::write(&note, "Changed workspace file")?;
    refresh.send(())?;
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    let (_, cells) = app
        .sidebar_mut()
        .documents
        .get_mut(&note_tab)
        .unwrap()
        .reader()
        .unwrap();
    assert_eq!(cells[0].copy_source(), Some("Changed workspace file"));

    // A model request for the live file keeps its tab and subscription.
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Main);
    let mut revisit = preview("note.md");
    if let AppEvent::OpenModelSidebarDocument { document, .. } = &mut revisit {
        document.location = Some(crate::markdown_render::SourceLocation::single(
            /*line*/ 1,
            Some(1),
        ));
    }
    boxed_future!(app.handle_event(&mut tui, &mut server, revisit)).await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert!(app.model_preview_load.is_none());
    assert_eq!(
        app.sidebar().unwrap().documents[&note_tab]
            .watch
            .as_ref()
            .unwrap()
            .0,
        watch
    );

    // A backend without an in-process permissions reader requires an explicit open.
    let pane = app.sidebar_mut();
    pane.remove_document(note_tab);
    pane.agents.detail = None;
    pane.show(SidebarContent::Chooser, PaneFocus::Main);
    let mut daemon_view = server.request_view();
    boxed_future!(app.handle_event(&mut tui, &mut daemon_view, preview("note.md"))).await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut daemon_view, TuiEvent::Draw)).await?;
    let Some(SidebarContent::Document(tab)) = visible_content(&app) else {
        panic!("model request should show a confirmation tab");
    };
    let doc = &app.sidebar().unwrap().documents[&tab];
    assert!(doc.awaits_open());
    assert!(doc.watch.is_none());
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS
    );
    let wide = Size::new(/*width*/ 360, /*height*/ size.height);
    let screen = side_frame(&mut app, &mut tui, wide)?;
    let sidebar = screen
        .replace(config.cwd.as_path().to_str().unwrap(), "<workspace>")
        .lines()
        .filter_map(|line| {
            line.split_once('│')
                .map(|(_, content)| content.trim_end().to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("open_in_codex_confirmation", sidebar.trim_end());

    assert!(
        !app.handle_document_input(&mut tui, &mut daemon_view, &enter)
            .await
    );
    assert!(app.sidebar().unwrap().documents[&tab].watch.is_none());
    app.sidebar_mut()
        .show(SidebarContent::Document(tab), PaneFocus::Sidebar);
    assert!(
        app.handle_document_input(&mut tui, &mut daemon_view, &enter)
            .await
    );
    assert!(app.sidebar().unwrap().documents[&tab].watch.is_some());
    assert_eq!(
        slots.available_permits(),
        crate::open_in_codex::MAX_PENDING_DOCUMENTS
    );
    finish_document_request(&mut app, &mut tui, &mut daemon_view).await?;

    app.sidebar_mut().remove_document(tab);
    let other = config.cwd.join("other.md");
    std::fs::write(&other, "Next preview")?;
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Main);
    boxed_future!(app.handle_event(&mut tui, &mut server, preview("PLAN.md"))).await?;
    boxed_future!(app.handle_event(&mut tui, &mut server, preview("other.md"))).await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    let canceled = app.model_preview_load.expect("first preview loading");
    app.sidebar_mut().remove_document(canceled);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        finish_document_request(&mut app, &mut tui, &mut server),
    )
    .await?;
    assert!(result.unwrap_err().to_string().contains("Preview closed"));
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)).await?;
    assert!(app.model_preview_load.is_some());
    finish_document_request(&mut app, &mut tui, &mut server).await?;
    server.shutdown().await?;
    Ok(())
}
