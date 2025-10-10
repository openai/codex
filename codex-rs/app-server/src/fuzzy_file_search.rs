use std::num::NonZero;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codex_app_server_protocol::FuzzyFileSearchResult;
use codex_file_search as file_search;
use tokio::task::JoinSet;
use tracing::warn;

const LIMIT_PER_ROOT: usize = 50;
const MAX_THREADS: usize = 12;
const COMPUTE_INDICES: bool = true;

pub(crate) async fn run_fuzzy_file_search(
    query: String,
    roots: Vec<String>,
    cancellation_flag: Arc<AtomicBool>,
) -> Vec<FuzzyFileSearchResult> {
    #[expect(clippy::expect_used)]
    let limit_per_root =
        NonZero::new(LIMIT_PER_ROOT).expect("LIMIT_PER_ROOT should be a valid non-zero usize");

    let cores = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1);
    let threads = cores.min(MAX_THREADS);
    let threads_per_root = (threads / roots.len()).max(1);
    let threads = NonZero::new(threads_per_root).unwrap_or(NonZeroUsize::MIN);

    let mut files: Vec<FuzzyFileSearchResult> = Vec::new();
    let mut join_set = JoinSet::new();

    for root in roots {
        let search_dir = PathBuf::from(&root);
        let query = query.clone();
        let cancel_flag = cancellation_flag.clone();
        join_set.spawn_blocking(move || {
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
                    let path = m.path;
                    //TODO(shijie): Move file name generation to file_search lib.
                    let file_name = Path::new(&path)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    let result = FuzzyFileSearchResult {
                        root: root.clone(),
                        path,
                        file_name,
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
