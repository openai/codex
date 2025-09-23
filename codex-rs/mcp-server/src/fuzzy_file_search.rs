use std::num::NonZero;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codex_file_search as file_search;
use codex_protocol::mcp_protocol::FuzzyFileSearchResult;
use tokio::task::JoinSet;
use tracing::warn;

const LIMIT_PER_ROOT: usize = 50;
const DEFAULT_THREAD_COUNT: usize = 2;
const COMPUTE_INDICES: bool = true;

pub(crate) async fn run_fuzzy_file_search(
    query: String,
    roots: Vec<String>,
    cancellation_flag: Arc<AtomicBool>,
) -> Vec<FuzzyFileSearchResult> {
    let mut files: Vec<FuzzyFileSearchResult> = Vec::new();
    #[expect(clippy::expect_used)]
    let limit_per_root =
        NonZero::new(LIMIT_PER_ROOT).expect("LIMIT_PER_ROOT should be a valid non-zero usize");
    #[expect(clippy::expect_used)]
    let threads = NonZero::new(DEFAULT_THREAD_COUNT)
        .expect("DEFAULT_THREAD_COUNT should be a valid non-zero usize");

    let mut join_set = JoinSet::new();

    for root in roots {
        let search_dir = PathBuf::from(&root);
        let query = query.clone();
        let cancel_flag = cancellation_flag.clone();
        join_set.spawn(async move {
            match file_search::run(
                query.as_str(),
                limit_per_root,
                &search_dir,
                Vec::new(),
                threads,
                cancel_flag,
                COMPUTE_INDICES,
            ) {
                Ok(res) => Ok((root, res)),
                Err(err) => Err((root, err)),
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok((root, res))) => {
                for m in res.matches {
                    let result = FuzzyFileSearchResult {
                        root: root.clone(),
                        path: m.path,
                        score: m.score,
                        indices: m.indices,
                    };
                    files.push(result);
                }
            }
            Ok(Err((root, err))) => {
                warn!("fuzzy-file-search in dir '{root}' failed: {err}");
            }
            Err(err) => {
                warn!("fuzzy-file-search join_next failed: {err}");
            }
        }
    }

    files.sort_by(file_search::cmp_by_score_desc_then_path_asc::<
        FuzzyFileSearchResult,
        _,
        _,
    >(|f| f.score, |f| f.path.as_str()));

    files
}
