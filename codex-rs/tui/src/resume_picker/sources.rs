//! Fans listing requests out to independent workers and routes actions to row sources.
//! Closing the picker cancels reads while accepted mutations and source shutdown continue
//! independently; stale listing replies cannot replace current pages.

use super::*;
use crate::backend_id::BackendId;
use futures::StreamExt;
use futures::stream::SelectAll;
use std::collections::HashSet;

struct PendingPage {
    request: PageLoadRequest,
    page: catalog::PageRequest,
}

fn completion_page(catalog: &catalog::ThreadCatalog, failed: &HashSet<BackendId>) -> PickerPage {
    PickerPage {
        rows: Vec::new(),
        read_states: HashMap::new(),
        history_modes: HashMap::new(),
        next_cursor: catalog.next_cursor(failed),
        num_scanned_files: 0,
        reached_scan_cap: false,
    }
}

pub(super) struct Source {
    pub(super) backend_id: BackendId,
    pub(super) loader: PickerLoader,
    pub(super) events: mpsc::UnboundedReceiver<BackgroundEvent>,
    pub(super) task: tokio::task::JoinHandle<()>,
}

pub(super) fn spawn(
    sources: Vec<Source>,
    output: mpsc::UnboundedSender<BackgroundEvent>,
) -> PickerBackground {
    assert!(!sources.is_empty());
    let (input, mut requests) = mpsc::unbounded_channel();
    let mut workers = HashMap::new();
    let mut events = SelectAll::new();
    let mut tasks = Vec::new();
    for Source {
        backend_id,
        loader,
        events: receiver,
        task,
    } in sources
    {
        assert!(workers.insert(backend_id, loader).is_none());
        tasks.push(task);
        events.push(
            UnboundedReceiverStream::new(receiver)
                .map(move |event| (backend_id, Some(event)))
                .chain(futures::stream::once(async move { (backend_id, None) }))
                .boxed(),
        );
    }
    let task = tokio::spawn(async move {
        let mut catalog = catalog::ThreadCatalog::default();
        let mut pending: HashMap<BackendId, PendingPage> = HashMap::new();
        let mut awaiting_completion = None;
        let mut failed = HashSet::new();
        loop {
            tokio::select! {
                biased;
                _ = output.closed(), if !requests.is_closed() => requests.close(),
                request = requests.recv() => {
                    let Some(request) = request else { break };
                    match request {
                        PickerLoadRequest::Page(request) => {
                            if request.cursor.is_none() {
                                catalog.reset();
                                pending.clear();
                                awaiting_completion = None;
                                failed.retain(|id| !workers.contains_key(id));
                            }
                            let request_token = request.request_token;
                            let mut started = false;
                            for (id, worker) in &workers {
                                if failed.contains(id) { continue; }
                                if let Some(page) = catalog.begin_page(*id, request.mode) {
                                    let mut source_request = request.clone();
                                    source_request.cursor = page.cursor.clone();
                                    source_request.mode = page.mode;
                                    pending.insert(*id, PendingPage {
                                        request: source_request.clone(),
                                        page,
                                    });
                                    worker(PickerLoadRequest::Page(source_request));
                                    started = true;
                                }
                            }
                            if started {
                                awaiting_completion = Some(request);
                            } else {
                                let page = if workers.is_empty() {
                                    Err(std::io::Error::other("No session backends are available."))
                                } else {
                                    Ok(completion_page(&catalog, &failed))
                                };
                                let _ = output.send(BackgroundEvent::Page {
                                    request_token,
                                    search_token: request.search_token,
                                    mode: request.mode,
                                    page,
                                });
                            }
                        }
                        request => {
                            let backend = match &request {
                                PickerLoadRequest::Preview { backend_id, .. }
                                | PickerLoadRequest::Acknowledge { backend_id, .. }
                                | PickerLoadRequest::Transcript { backend_id, .. }
                                | PickerLoadRequest::Archive { backend_id, .. }
                                | PickerLoadRequest::Unarchive { backend_id, .. } => *backend_id,
                                PickerLoadRequest::Page(_) => unreachable!(),
                            };
                            if let Some(worker) = workers.get(&backend) {
                                worker(request);
                            } else {
                                let error = std::io::Error::other("The session's backend is unavailable.");
                                let event = match request {
                                    PickerLoadRequest::Preview { thread_id, .. } => BackgroundEvent::Preview { thread_id, preview: Err(error) },
                                    PickerLoadRequest::Acknowledge { .. } => continue,
                                    PickerLoadRequest::Transcript { thread_id, .. } => BackgroundEvent::Transcript { backend_id: backend, thread_id, transcript: Err(error) },
                                    PickerLoadRequest::Archive { thread_id, .. } => BackgroundEvent::Archive { thread_id, result: Err(error) },
                                    PickerLoadRequest::Unarchive { thread_id, .. } => BackgroundEvent::Unarchive { thread_id, result: Err(error) },
                                    PickerLoadRequest::Page(_) => unreachable!(),
                                };
                                let _ = output.send(event);
                            }
                        }
                    }
                }
                Some((id, event)) = events.next() => {
                    let event = match event {
                        Some(event) => event,
                        None => {
                            workers.remove(&id);
                            failed.insert(id);
                            let Some(pending_page) = pending.get(&id) else { continue };
                            BackgroundEvent::Page {
                                request_token: pending_page.request.request_token,
                                search_token: pending_page.request.search_token,
                                mode: pending_page.request.mode,
                                page: Err(std::io::Error::other("The session backend disconnected.")),
                            }
                        }
                    };
                    match event {
                        BackgroundEvent::Page { request_token, page, .. } => {
                            let Some(pending_page) = pending.get(&id) else { continue };
                            if pending_page.request.request_token != request_token { continue; }
                            let Some(mut pending_page) = pending.remove(&id) else { continue };
                            let is_initial_page = pending_page.page.cursor.is_none();
                            let should_fallback = pending_page.page.mode == PageLoadMode::StateDbOnly
                                && is_initial_page
                                && workers.contains_key(&id)
                                && (page.is_err()
                                    || page.as_ref().is_ok_and(|page| page.rows.is_empty()));
                            if should_fallback {
                                if let Some(request) = awaiting_completion.as_mut()
                                    && request.request_token == request_token
                                {
                                    request.mode = PageLoadMode::StoreDefault;
                                }
                                let mut fallback = pending_page.request;
                                fallback.mode = PageLoadMode::StoreDefault;
                                fallback.cursor = pending_page.page.cursor.clone();
                                pending_page.page.mode = PageLoadMode::StoreDefault;
                                pending_page.request = fallback.clone();
                                pending.insert(id, pending_page);
                                workers[&id](PickerLoadRequest::Page(fallback));
                                continue;
                            }
                            let mut wait_for_other_source = false;
                            let mut completion_error = None;
                            match page {
                                Ok(page) => {
                                    let Some(Ok(mut page)) = catalog.finish_page(pending_page.page, Ok(page)) else { continue };
                                    page.next_cursor = catalog.next_cursor(&failed);
                                    let _ = output.send(BackgroundEvent::PartialPage {
                                        listing_token: pending_page.request.listing_token,
                                        page,
                                    });
                                }
                                Err(err) => {
                                    let Some(Err(err)) = catalog.finish_page(pending_page.page, Err(err)) else { continue };
                                    failed.insert(id);
                                    if let Some(request) = awaiting_completion.as_mut()
                                        && request.request_token == request_token
                                    {
                                        request.mode = PageLoadMode::StoreDefault;
                                    }
                                    let _ = output.send(BackgroundEvent::SourceError {
                                        listing_token: pending_page.request.listing_token,
                                        next_cursor: catalog.next_cursor(&failed),
                                    });
                                    if is_initial_page {
                                        wait_for_other_source = pending.values().any(|pending| {
                                            pending.request.request_token == request_token
                                        });
                                        completion_error = Some(err);
                                    }
                                }
                            }
                            let can_continue = matches!(
                                catalog.next_cursor(&failed),
                                Some(PageCursor::Catalog)
                            );
                            let completes_source_request = awaiting_completion
                                .as_ref()
                                .is_some_and(|request| {
                                    request.request_token == request_token
                                        && !wait_for_other_source
                                });
                            if (completes_source_request || can_continue)
                                && let Some(request) = awaiting_completion.take()
                            {
                                let page = if completes_source_request {
                                    completion_error.map_or_else(
                                        || Ok(completion_page(&catalog, &failed)),
                                        Err,
                                    )
                                } else {
                                    Ok(completion_page(&catalog, &failed))
                                };
                                let _ = output.send(BackgroundEvent::Page {
                                    request_token: request.request_token,
                                    search_token: request.search_token,
                                    mode: request.mode,
                                    page,
                                });
                            }
                        }
                        event => { let _ = output.send(event); }
                    }
                }
            }
        }
        drop(workers);
        drop(events);
        for task in tasks {
            if let Err(err) = task.await {
                tracing::warn!(%err, "Picker source task stopped unexpectedly");
            }
        }
    });
    PickerBackground {
        loader: Arc::new(move |request| {
            let _ = input.send(request);
        }),
        task,
    }
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
