#![cfg(test)]

use super::ToolSearchHandler;
use super::ToolSearchSourceListing;
use super::tests::tool_info;
use crate::tools::handlers::McpHandler;
use codex_tools::ToolExecutor;
use pretty_assertions::assert_eq;
use std::pin::pin;

#[test]
fn cancelling_the_first_search_preserves_the_pending_build() {
    tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(/*val*/ 1)
        .build()
        .unwrap()
        .block_on(async {
            let info = McpHandler::new(tool_info("calendar", "create_event", "Create events"))
                .unwrap()
                .search_info()
                .unwrap();
            let expected = vec![info.entry.to_loadable_spec()];
            let handler = ToolSearchHandler::new(vec![info], ToolSearchSourceListing::Include);

            // Occupy the sole blocking worker until both searches have tried to get the index.
            let (release, wait) = std::sync::mpsc::channel::<()>();
            let (started, ready) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = wait.recv();
            });
            ready.await.unwrap();

            let first_build = {
                let mut search = pin!(handler.search("Create events", /*limit*/ 1));
                assert!(futures::poll!(&mut search).is_pending());
                handler.index.get().unwrap().clone()
            };
            let mut search = pin!(handler.search("Create events", /*limit*/ 1));
            assert!(futures::poll!(&mut search).is_pending());
            assert!(first_build.same_channel(handler.index.get().unwrap()));

            drop(release);
            blocker.await.unwrap();
            assert_eq!(search.await.unwrap(), expected);
        });
}
