//! Optional progress from the same rollout discovery and content verification used by final search.

use super::LocalThreadStore;
use super::SearchThreadsParams;
use super::StoredThreadSearchResult;
use super::ThreadSearchItem;
use super::eligible_search_item;
use super::materialize_search_results;
use super::rollout_search_path;
use super::verified_snippet;
use codex_protocol::ThreadId;
use codex_rollout::RolloutSearchCandidate;
use codex_rollout::RolloutSnippetMatcher;
use codex_rollout::state_db::list_threads_db;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;

const PROGRESS_RESULT_LIMIT: usize = 8;
const CANDIDATE_LIMIT: usize = 32;
const CANDIDATE_BATCH: usize = 16;

pub(super) struct SearchProgressContext<'a, 'term> {
    pub(super) store: &'a LocalThreadStore,
    pub(super) params: &'a SearchThreadsParams,
    pub(super) state_db: Option<&'a codex_state::StateRuntime>,
    pub(super) cursor: Option<&'a codex_rollout::Cursor>,
    pub(super) sort_key: codex_rollout::ThreadSortKey,
    pub(super) sort_direction: codex_rollout::SortDirection,
    pub(super) matcher: &'a mut RolloutSnippetMatcher<'term>,
}

impl SearchProgressContext<'_, '_> {
    pub(super) async fn report_search_candidates(
        self,
        mut candidates: mpsc::Receiver<RolloutSearchCandidate>,
        progress: mpsc::Sender<Vec<StoredThreadSearchResult>>,
    ) {
        let Some(db) = self.state_db else { return };
        let mut emitted = HashSet::<ThreadId>::new();
        let mut seen = HashSet::new();
        let mut scanned = 0;
        let mut batch = Vec::new();
        while scanned < CANDIDATE_LIMIT
            && emitted.len() < self.params.page_size.min(PROGRESS_RESULT_LIMIT)
        {
            if candidates
                .recv_many(&mut batch, CANDIDATE_BATCH.min(CANDIDATE_LIMIT - scanned))
                .await
                == 0
            {
                break;
            }
            scanned += batch.len();
            let mut ids = Vec::with_capacity(batch.len());
            let hits = batch
                .drain(..)
                .filter_map(|candidate| {
                    let path = rollout_search_path(&candidate.path);
                    let id = codex_rollout::thread_id_from_rollout_path(&path)?;
                    if !seen.insert(path.clone()) || emitted.contains(&id) {
                        return None;
                    }
                    ids.push(id);
                    Some((id, path))
                })
                .collect::<Vec<_>>();
            if hits.is_empty() {
                continue;
            }
            let relation = self
                .params
                .include_attached_threads
                .then_some(codex_state::ThreadRelationFilter::IncludeAttached);
            let Some(page) = list_threads_db(
                Some(db),
                db.sqlite(),
                ids.len(),
                self.cursor,
                self.sort_key,
                self.sort_direction,
                &self.params.allowed_sources,
                /*model_providers*/ None,
                /*cwd_filters*/ None,
                Some(&ids),
                relation,
                self.params.archived,
                /*section*/ None,
                /*project_id*/ None,
                /*search_term*/ None,
            )
            .await
            else {
                continue;
            };
            let page: codex_rollout::ThreadsPage = page.into();
            let mut items = page
                .items
                .into_iter()
                .filter_map(|item| item.thread_id.map(|id| (id, item)))
                .collect::<HashMap<_, _>>();
            let mut originals = None;
            for (id, path) in hits {
                let Some(item) = items.remove(&id) else {
                    continue;
                };
                if rollout_search_path(&item.path) != path
                    || !eligible_search_item(self.store, self.params, &item, &ids, &mut originals)
                        .await
                {
                    continue;
                }
                let Ok(Some(snippet)) = verified_snippet(self.matcher, &item.path).await else {
                    continue;
                };
                let data = materialize_search_results(
                    self.store,
                    Some(db),
                    self.params.archived,
                    vec![ThreadSearchItem { item, snippet }],
                )
                .await;
                if data.is_empty() {
                    continue;
                }
                if progress.try_send(data).is_ok() {
                    emitted.insert(id);
                }
                if emitted.len() >= self.params.page_size.min(PROGRESS_RESULT_LIMIT) {
                    break;
                }
            }
        }
    }
}
