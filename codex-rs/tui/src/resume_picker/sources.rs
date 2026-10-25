//! Fans listing requests out to independent workers and routes actions to row sources.
//! Closing the picker cancels reads while accepted mutations and source shutdown continue
//! independently; stale listing replies cannot replace current pages.

use super::*;
use crate::backend_id::BackendId;
use futures::StreamExt;
use futures::stream::SelectAll;
use std::collections::HashSet;

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
        let mut pending = HashMap::new();
        let mut failed = HashSet::new();
        let mut current: Option<PageLoadRequest> = None;
        let mut succeeded = false;
        let mut error = None;
        loop {
            tokio::select! {
                biased;
                _ = output.closed(), if !requests.is_closed() => requests.close(),
                request = requests.recv() => {
                    let Some(request) = request else { break };
                    match request {
                        PickerLoadRequest::Page(request) => {
                            succeeded = false;
                            if request.cursor.is_none() {
                                catalog.reset();
                                pending.clear();
                                failed.retain(|id| !workers.contains_key(id));
                            }
                            error = workers.is_empty().then(|| std::io::Error::other("No session backends are available."));
                            for (id, worker) in &workers {
                                if failed.contains(id) { continue; }
                                if let Some(page) = catalog.begin_page(*id, request.mode) {
                                    let mut source_request = request.clone();
                                    source_request.cursor = page.cursor.clone();
                                    source_request.mode = page.mode;
                                    pending.insert(*id, page);
                                    worker(PickerLoadRequest::Page(source_request));
                                }
                            }
                            current = Some(request);
                        }
                        request => {
                            let backend = match &request {
                                PickerLoadRequest::Preview { backend_id, .. }
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
                                    PickerLoadRequest::Transcript { thread_id, .. } => BackgroundEvent::Transcript { thread_id, transcript: Err(error) },
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
                            let Some(request) = current.as_ref().filter(|_| pending.contains_key(&id)) else { continue };
                            BackgroundEvent::Page { request_token: request.request_token, search_token: request.search_token, mode: request.mode,
                                page: Err(std::io::Error::other("The session backend disconnected.")) }
                        }
                    };
                    match event {
                        BackgroundEvent::Page { request_token, page, .. } => {
                            if current.as_ref().is_none_or(|request| request.request_token != request_token) { continue; }
                            let Some(mut page_request) = pending.remove(&id) else { continue };
                            let should_fallback = page_request.mode == PageLoadMode::StateDbOnly
                                && page_request.cursor.is_none()
                                && workers.contains_key(&id)
                                && (page.is_err()
                                    || page.as_ref().is_ok_and(|page| page.rows.is_empty()));
                            if should_fallback {
                                let Some(request) = current.as_mut() else {
                                    continue;
                                };
                                request.mode = PageLoadMode::StoreDefault;
                                let mut fallback = request.clone();
                                fallback.cursor = page_request.cursor.clone();
                                page_request.mode = PageLoadMode::StoreDefault;
                                pending.insert(id, page_request);
                                workers[&id](PickerLoadRequest::Page(fallback));
                                continue;
                            }
                            match page {
                                Ok(page) => {
                                    let Some(Ok(mut page)) = catalog.finish_page(page_request, Ok(page)) else { continue };
                                    succeeded = true;
                                    // Only the catalog owns source cursors. The UI receives a
                                    // continuation marker once this round's workers have replied.
                                    page.next_cursor = None;
                                    let _ = output.send(BackgroundEvent::PartialPage { request_token, page });
                                }
                                Err(err) => {
                                    let Some(Err(err)) = catalog.finish_page(page_request, Err(err)) else { continue };
                                    failed.insert(id);
                                    error = Some(err);
                                    let _ = output.send(BackgroundEvent::SourceError { request_token });
                                }
                            }
                        }
                        event => { let _ = output.send(event); }
                    }
                }
            }
            if pending.is_empty()
                && let Some(request) = current.take()
            {
                let page = if !succeeded && let Some(error) = error.take() {
                    Err(error)
                } else {
                    Ok(PickerPage {
                        rows: Vec::new(),
                        history_modes: HashMap::new(),
                        next_cursor: catalog.has_more(&failed).then_some(PageCursor::Catalog),
                        num_scanned_files: 0,
                        reached_scan_cap: false,
                    })
                };
                let _ = output.send(BackgroundEvent::Page {
                    request_token: request.request_token,
                    search_token: request.search_token,
                    mode: request.mode,
                    page,
                });
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
