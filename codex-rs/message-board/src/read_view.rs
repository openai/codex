//! Read-only board snapshots with a stable channel catalog and post cutoff.

use crate::ChannelPage;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::Page;
use crate::PostContent;
use crate::PostPreview;
use crate::PostQuery;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::ThreadPage;
use crate::ThreadQuery;
use crate::ThreadSummary;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use std::sync::Arc;

/// A read-only capture of one board. All handles share its namespace, catalog and post cutoff.
/// Later appends and description edits are invisible. Deletion can make captured posts unavailable.
pub trait BoardReadView: Send + Sync {
    fn identity(&self) -> SessionId;
    fn storage_namespace(&self) -> &str;
    fn channels(&self) -> &[ChannelSummary];

    /// Keep the capture and search literal values in JSON and in the string-encoded JSON at this pointer.
    fn with_json_search(&self, pointer: &'static str) -> Arc<dyn BoardReadView>;

    fn list_channels(
        &self,
        caller: ThreadId,
        query: ChannelQuery,
    ) -> BoxFuture<'_, Result<ChannelPage>>;

    fn list_threads(
        &self,
        caller: ThreadId,
        query: ThreadQuery,
    ) -> BoxFuture<'_, Result<Page<ThreadSummary>>>;

    fn search_posts(
        &self,
        caller: ThreadId,
        query: PostQuery,
    ) -> BoxFuture<'_, Result<Page<PostPreview>>>;

    fn read_thread(
        &self,
        caller: ThreadId,
        request: ReadThreadRequest,
    ) -> BoxFuture<'_, Result<ThreadPage>>;

    fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> BoxFuture<'_, Result<PostContent>>;

    /// Return one page after the given insertion position, in ascending order.
    /// Zero precedes the first post. Positions stay valid across captures of the same board.
    /// A page shorter than the default board page size exhausts the captured channel.
    fn appended_request_ids<'a>(
        &'a self,
        caller: ThreadId,
        channel: &'a str,
        after: i64,
    ) -> BoxFuture<'a, Result<Vec<(i64, String)>>>;

    /// Return whole post bodies for one page in the requested order, within this capture.
    fn read_post_texts<'a>(
        &'a self,
        caller: ThreadId,
        positions: &'a [i64],
    ) -> BoxFuture<'a, Result<Vec<String>>>;
}
