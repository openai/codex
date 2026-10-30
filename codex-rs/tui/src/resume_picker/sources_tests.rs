#![cfg(test)]
//! Exercises independent worker replies, stale pages, and action ownership.

use super::*;
use crate::resume_picker::tests::make_row;
use crate::resume_picker::tests::page;
use pretty_assertions::assert_eq;
fn source() -> (
    Source,
    mpsc::UnboundedReceiver<PickerLoadRequest>,
    mpsc::UnboundedSender<BackgroundEvent>,
) {
    let (requests, receiver) = mpsc::unbounded_channel();
    let (events, responses) = mpsc::unbounded_channel();
    (
        Source {
            backend_id: BackendId::new(),
            loader: Arc::new(move |request| {
                let _ = requests.send(request);
            }),
            events: responses,
            task: tokio::spawn(async {}),
        },
        receiver,
        events,
    )
}
fn request(token: usize, cursor: Option<PageCursor>) -> PickerLoadRequest {
    PickerLoadRequest::Page(PageLoadRequest {
        request_token: token,
        listing_token: token,
        cursor,
        search_token: None,
        mode: PageLoadMode::StoreDefault,
        cwd_filter: None,
        filter_mode: SessionFilterMode::All,
        status: SessionStatus::Active,
        provider_filter: ProviderFilter::Any,
        sort_key: ThreadSortKey::UpdatedAt,
    })
}
async fn next(receiver: &mut mpsc::UnboundedReceiver<BackgroundEvent>) -> BackgroundEvent {
    tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}
fn reply(token: usize, thread: ThreadId, cursor: Option<&str>) -> BackgroundEvent {
    let mut row = make_row("/tmp/worker.jsonl", "2025-01-01T00:00:00Z", "conversation");
    row.thread_id = Some(thread);
    BackgroundEvent::Page {
        request_token: token,
        search_token: None,
        mode: PageLoadMode::StoreDefault,
        page: Ok(page(
            vec![row],
            cursor,
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
        )),
    }
}
#[tokio::test]
async fn sources_stream_rows_and_route_actions_to_each_source() {
    let (first, mut first_requests, first_events) = source();
    let first_id = first.backend_id;
    let (second, mut second_requests, second_events) = source();
    let second_id = second.backend_id;
    let (third, mut third_requests, third_events) = source();
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![first, second, third], output);
    let PickerLoadRequest::Page(mut initial_request) =
        request(/*token*/ 1, /*cursor*/ None)
    else {
        unreachable!()
    };
    initial_request.mode = PageLoadMode::StateDbOnly;
    loader(PickerLoadRequest::Page(initial_request));
    for requests in [
        &mut first_requests,
        &mut second_requests,
        &mut third_requests,
    ] {
        assert!(matches!(
            requests.recv().await,
            Some(PickerLoadRequest::Page(_))
        ));
    }
    let thread = ThreadId::new();
    second_events
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StateDbOnly,
            page: Err(std::io::Error::other("indexed listing failed")),
        })
        .unwrap();
    assert!(events.try_recv().is_err());
    let Some(PickerLoadRequest::Page(fallback_request)) = second_requests.recv().await else {
        panic!("expected source-specific fallback")
    };
    assert_eq!(fallback_request.mode, PageLoadMode::StoreDefault);
    second_events
        .send(reply(/*token*/ 1, thread, Some("second-next")))
        .unwrap();
    let BackgroundEvent::PartialPage { page, .. } = next(&mut events).await else {
        panic!("expected responsive source page")
    };
    assert_eq!(page.rows[0].backend_id, Some(second_id));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            mode: PageLoadMode::StoreDefault,
            page: Ok(_),
            ..
        }
    ));
    loader(request(/*token*/ 2, Some(PageCursor::Catalog)));
    let Some(PickerLoadRequest::Page(second_page)) = second_requests.recv().await else {
        panic!("expected the responsive source to continue")
    };
    assert!(
        matches!(second_page.cursor, Some(PageCursor::AppServer(cursor)) if cursor == "second-next")
    );
    assert!(first_requests.try_recv().is_err());
    first_events
        .send(reply(/*token*/ 1, thread, Some("first-next")))
        .unwrap();
    let BackgroundEvent::PartialPage { page, .. } = next(&mut events).await else {
        panic!("expected other source page")
    };
    assert_eq!(page.rows[0].backend_id, Some(first_id));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            request_token: 2,
            page: Ok(_),
            ..
        }
    ));
    loader(request(/*token*/ 3, Some(PageCursor::Catalog)));
    let Some(PickerLoadRequest::Page(next_page)) = first_requests.recv().await else {
        panic!("expected the newly ready source to continue")
    };
    assert_eq!(next_page.mode, PageLoadMode::StateDbOnly);
    assert!(
        matches!(next_page.cursor, Some(PageCursor::AppServer(cursor)) if cursor == "first-next")
    );
    assert!(second_requests.try_recv().is_err());
    loader(PickerLoadRequest::Archive {
        backend_id: first_id,
        thread_id: thread,
    });
    assert!(
        matches!(first_requests.recv().await, Some(PickerLoadRequest::Archive { backend_id, thread_id }) if (backend_id, thread_id) == (first_id, thread))
    );
    let (_cancel, cancellation) = oneshot::channel();
    loader(PickerLoadRequest::Transcript {
        backend_id: second_id,
        thread_id: thread,
        cancellation,
    });
    assert!(
        matches!(second_requests.recv().await, Some(PickerLoadRequest::Transcript { backend_id, thread_id, .. }) if (backend_id, thread_id) == (second_id, thread))
    );
    drop(third_events);
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError { .. }
    ));
    second_events
        .send(reply(
            /*token*/ 2,
            ThreadId::new(),
            /*cursor*/ None,
        ))
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::PartialPage { .. }
    ));
    first_events
        .send(reply(
            /*token*/ 3,
            ThreadId::new(),
            /*cursor*/ Some("last-page"),
        ))
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::PartialPage { .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page { page: Ok(_), .. }
    ));
    loader(request(/*token*/ 4, Some(PageCursor::Catalog)));
    let Some(PickerLoadRequest::Page(last_page)) = first_requests.recv().await else {
        panic!("expected last page")
    };
    assert!(
        matches!(last_page.cursor, Some(PageCursor::AppServer(cursor)) if cursor == "last-page")
    );
    first_events
        .send(BackgroundEvent::Page {
            request_token: 4,
            search_token: None,
            mode: PageLoadMode::StateDbOnly,
            page: Err(std::io::Error::other("last page failed")),
        })
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError { .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            request_token: 4,
            page: Ok(_),
            ..
        }
    ));
    drop(loader);
    task.await.unwrap();
    assert!(first_requests.recv().await.is_none());
    assert!(second_requests.recv().await.is_none());
}
#[tokio::test]
async fn late_source_failure_clears_pending_cursor() {
    let (first, mut first_requests, first_events) = source();
    let (second, mut second_requests, second_events) = source();
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![first, second], output);
    loader(request(/*token*/ 1, /*cursor*/ None));
    first_requests.recv().await.unwrap();
    second_requests.recv().await.unwrap();

    first_events
        .send(reply(
            /*token*/ 1,
            ThreadId::new(),
            /*cursor*/ None,
        ))
        .unwrap();
    let BackgroundEvent::PartialPage { page, .. } = next(&mut events).await else {
        panic!("expected first source page")
    };
    assert!(matches!(page.next_cursor, Some(PageCursor::SourcesPending)));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page { .. }
    ));

    second_events
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StoreDefault,
            page: Err(std::io::Error::other("source failed")),
        })
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError {
            next_cursor: None,
            ..
        }
    ));

    drop(loader);
    task.await.unwrap();
}

#[tokio::test]
async fn late_source_failure_finishes_catalog_progress() {
    let (first, mut first_requests, first_events) = source();
    let (second, mut second_requests, second_events) = source();
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![first, second], output);
    loader(request(/*token*/ 1, /*cursor*/ None));
    first_requests.recv().await.unwrap();
    second_requests.recv().await.unwrap();
    first_events
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StoreDefault,
            page: Ok(page(
                Vec::new(),
                /*next_cursor*/ None,
                /*num_scanned_files*/ 0,
                /*reached_scan_cap*/ false,
            )),
        })
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::PartialPage { .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            page: Ok(PickerPage {
                next_cursor: Some(PageCursor::SourcesPending),
                ..
            }),
            ..
        }
    ));
    second_events
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StoreDefault,
            page: Err(std::io::Error::other("listing failed")),
        })
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError {
            next_cursor: None,
            ..
        }
    ));
    drop(loader);
    task.await.unwrap();
}

#[tokio::test]
async fn single_source_initial_failure_preserves_fallback_error() {
    let (source, mut requests, replies) = source();
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![source], output);
    let PickerLoadRequest::Page(mut initial) = request(/*token*/ 1, /*cursor*/ None) else {
        unreachable!()
    };
    initial.mode = PageLoadMode::StateDbOnly;
    loader(PickerLoadRequest::Page(initial));
    let Some(PickerLoadRequest::Page(initial)) = requests.recv().await else {
        panic!("expected initial listing")
    };
    assert_eq!(initial.mode, PageLoadMode::StateDbOnly);
    replies
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StateDbOnly,
            page: Err(std::io::Error::other("indexed listing failed")),
        })
        .unwrap();
    let Some(PickerLoadRequest::Page(fallback)) = requests.recv().await else {
        panic!("expected store fallback")
    };
    assert_eq!(fallback.mode, PageLoadMode::StoreDefault);
    replies
        .send(BackgroundEvent::Page {
            request_token: 1,
            search_token: None,
            mode: PageLoadMode::StoreDefault,
            page: Err(std::io::Error::other("store listing failed")),
        })
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError { .. }
    ));
    let BackgroundEvent::Page {
        mode,
        page: Err(err),
        ..
    } = next(&mut events).await
    else {
        panic!("expected the fallback error")
    };
    assert_eq!(mode, PageLoadMode::StoreDefault);
    assert_eq!(err.to_string(), "store listing failed");
    drop(loader);
    task.await.unwrap();
}

#[tokio::test]
async fn reset_rejects_old_replies_and_missing_sources_never_fall_back() {
    let (source, mut requests, replies) = source();
    let owner = source.backend_id;
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![source], output);
    loader(request(/*token*/ 1, /*cursor*/ None));
    requests.recv().await.unwrap();
    loader(request(/*token*/ 2, /*cursor*/ None));
    let stale = ThreadId::new();
    let current = ThreadId::new();
    replies
        .send(reply(/*token*/ 1, stale, /*cursor*/ None))
        .unwrap();
    replies
        .send(reply(/*token*/ 2, current, /*cursor*/ None))
        .unwrap();
    let BackgroundEvent::PartialPage { page, .. } = next(&mut events).await else {
        panic!("expected current page")
    };
    assert_eq!(page.rows[0].backend_id, Some(owner));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            request_token: 2,
            ..
        }
    ));
    loader(PickerLoadRequest::Archive {
        backend_id: BackendId::new(),
        thread_id: stale,
    });
    assert!(
        matches!(next(&mut events).await, BackgroundEvent::Archive { thread_id, result: Err(_), .. } if thread_id == stale)
    );
    requests.recv().await.unwrap();
    loader(PickerLoadRequest::Archive {
        backend_id: owner,
        thread_id: current,
    });
    drop(events);
    assert!(
        matches!(requests.recv().await, Some(PickerLoadRequest::Archive { thread_id, .. }) if thread_id == current)
    );
    task.await.unwrap();
    assert!(requests.recv().await.is_none());
}
#[tokio::test]
async fn reset_does_not_restore_a_disconnected_source() {
    let (preferred, mut preferred_requests, preferred_events) = source();
    let (fallback, mut fallback_requests, fallback_events) = source();
    let owner = fallback.backend_id;
    let (output, mut events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![preferred, fallback], output);
    let PickerLoadRequest::Page(mut initial) = request(/*token*/ 1, /*cursor*/ None) else {
        unreachable!()
    };
    initial.mode = PageLoadMode::StateDbOnly;
    loader(PickerLoadRequest::Page(initial));
    preferred_requests.recv().await.unwrap();
    fallback_requests.recv().await.unwrap();
    drop(preferred_events);
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::SourceError { .. }
    ));
    fallback_events
        .send(reply(
            /*token*/ 1,
            ThreadId::new(),
            /*cursor*/ None,
        ))
        .unwrap();
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::PartialPage { .. }
    ));
    assert!(matches!(
        next(&mut events).await,
        BackgroundEvent::Page {
            request_token: 1,
            mode: PageLoadMode::StoreDefault,
            page: Ok(_),
            ..
        }
    ));
    loader(request(/*token*/ 2, /*cursor*/ None));
    let thread = ThreadId::new();
    fallback_requests.recv().await.unwrap();
    fallback_events
        .send(reply(/*token*/ 2, thread, /*cursor*/ None))
        .unwrap();
    let BackgroundEvent::PartialPage { page, .. } = next(&mut events).await else {
        panic!("expected fallback page after reset")
    };
    assert_eq!(page.rows[0].backend_id, Some(owner));
    drop(loader);
    task.await.unwrap();
}
#[tokio::test]
async fn detached_picker_cleanup_drains_accepted_mutations() {
    let backend_id = BackendId::new();
    let thread_id = ThreadId::new();
    let (requests, mut request_rx) = mpsc::unbounded_channel();
    let (source_events, source_responses) = mpsc::unbounded_channel();
    let (mutation_finished, mutation_completion) = oneshot::channel();
    let source_task = tokio::spawn(async move {
        assert!(matches!(
            request_rx.recv().await,
            Some(PickerLoadRequest::Page(_))
        ));
        source_events.closed().await;
        assert!(
            matches!(request_rx.recv().await, Some(PickerLoadRequest::Archive { backend_id: source, thread_id: thread }) if (source, thread) == (backend_id, thread_id))
        );
        mutation_finished.send(()).unwrap();
        assert!(request_rx.recv().await.is_none());
    });
    let source = Source {
        backend_id,
        loader: Arc::new(move |request| {
            let _ = requests.send(request);
        }),
        events: source_responses,
        task: source_task,
    };
    let (output, events) = mpsc::unbounded_channel();
    let PickerBackground { loader, task } = spawn(vec![source], output);
    loader(request(/*token*/ 1, /*cursor*/ None));
    loader(PickerLoadRequest::Archive {
        backend_id,
        thread_id,
    });
    drop(events);
    drop(task);
    mutation_completion.await.unwrap();
}
#[tokio::test]
async fn partial_failure_keeps_healthy_rows_visible() {
    let (mut picker, _) = crate::resume_picker::tests::local_db_first_state();
    picker.pagination.start_load(
        /*request_token*/ 42,
        /*search_token*/ None,
        PageLoadMode::StoreDefault,
    );
    picker
        .handle_background_event(BackgroundEvent::Page {
            request_token: 42,
            search_token: None,
            mode: PageLoadMode::StoreDefault,
            page: Ok(page(
                Vec::new(),
                /*next_cursor*/ None,
                /*num_scanned_files*/ 0,
                /*reached_scan_cap*/ false,
            )),
        })
        .await
        .unwrap();
    assert!(!picker.pagination.is_loading());
    let thread = ThreadId::new();
    let BackgroundEvent::Page { page: Ok(page), .. } =
        reply(/*token*/ 42, thread, /*cursor*/ None)
    else {
        unreachable!()
    };
    picker
        .handle_background_event(BackgroundEvent::PartialPage {
            listing_token: picker.listing_token,
            page,
        })
        .await
        .unwrap();
    picker.query = "missing".into();
    picker.apply_filter();
    picker.search_state = SearchState::Active { token: 1 };
    picker.pagination.next_cursor = Some(PageCursor::SourcesPending);
    picker
        .handle_background_event(BackgroundEvent::SourceError {
            listing_token: picker.listing_token,
            next_cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(picker.all_rows.len(), 1);
    assert!(!picker.pagination.is_loading());
    assert!(matches!(picker.search_state, SearchState::Idle));
    let backend = crate::test_backend::VT100Backend::new(/*width*/ 80, /*height*/ 1);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend).unwrap();
    terminal.set_viewport_area(Rect::new(0, 0, 80, 1));
    {
        let mut frame = terminal.get_frame();
        let line = search_line(&picker, frame.area().width);
        frame.render_widget_ref(&line, frame.area());
    }
    terminal.flush().unwrap();
    insta::assert_snapshot!("partial_backend_failure", terminal.backend().to_string());
}

#[tokio::test]
async fn queued_partial_page_from_previous_listing_is_ignored() {
    let (mut picker, requests) = crate::resume_picker::tests::local_db_first_state();
    picker.start_initial_load();
    let stale_listing_token = requests.lock().unwrap()[0].listing_token;
    picker.start_initial_load();
    assert_ne!(stale_listing_token, picker.listing_token);
    picker
        .handle_background_event(BackgroundEvent::PartialPage {
            listing_token: stale_listing_token,
            page: page(
                vec![make_row("/repo/stale", "2025-01-01T00:00:00Z", "stale row")],
                /*next_cursor*/ None,
                /*num_scanned_files*/ 1,
                /*reached_scan_cap*/ false,
            ),
        })
        .await
        .unwrap();
    picker
        .handle_background_event(BackgroundEvent::SourceError {
            listing_token: stale_listing_token,
            next_cursor: None,
        })
        .await
        .unwrap();
    assert!(picker.all_rows.is_empty());
    assert!(picker.inline_error.is_none());
}
