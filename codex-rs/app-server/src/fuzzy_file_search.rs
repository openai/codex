use std::num::NonZero;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codex_app_server_protocol::FuzzyFileSearchResult;
use codex_file_search as file_search;
use tracing::warn;

const LIMIT_PER_ROOT: usize = 50;
const MAX_THREADS: usize = 12;
const COMPUTE_INDICES: bool = true;

pub(crate) async fn run_fuzzy_file_search(
    query: String,
    roots: Vec<String>,
    cancellation_flag: Arc<AtomicBool>,
) -> Vec<FuzzyFileSearchResult> {
    if roots.is_empty() {
        return Vec::new();
    }

    #[expect(clippy::expect_used)]
    let limit_per_root =
        NonZero::new(LIMIT_PER_ROOT).expect("LIMIT_PER_ROOT should be a valid non-zero usize");

    let cores = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1);
    let threads = cores.min(MAX_THREADS);
    #[expect(clippy::expect_used)]
    let threads = NonZero::new(threads.max(1)).expect("threads should be non-zero");
    let search_dirs: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();

    let mut files = match tokio::task::spawn_blocking(move || {
        file_search::run(
            query.as_str(),
            limit_per_root,
            search_dirs,
            Vec::new(),
            threads,
            cancellation_flag,
            COMPUTE_INDICES,
            true,
        )
    })
    .await
    {
        Ok(Ok(res)) => res
            .matches
            .into_iter()
            .map(|m| {
                let file_name = m.path.file_name().unwrap_or_default();
                FuzzyFileSearchResult {
                    root: m.root.to_string_lossy().to_string(),
                    path: m.path.to_string_lossy().to_string(),
                    file_name: file_name.to_string_lossy().to_string(),
                    score: m.score,
                    indices: m.indices,
                }
            })
            .collect::<Vec<_>>(),
        Ok(Err(err)) => {
            warn!("fuzzy-file-search failed: {err}");
            Vec::new()
        }
        Err(err) => {
            warn!("fuzzy-file-search join failed: {err}");
            Vec::new()
        }
    };

    files.sort_by(file_search::cmp_by_score_desc_then_path_asc::<
        FuzzyFileSearchResult,
        _,
        _,
    >(|f| f.score, |f| f.path.as_str()));

    files
}
