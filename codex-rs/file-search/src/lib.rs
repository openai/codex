use ignore::WalkBuilder;
use ignore::overrides::Override;
use ignore::overrides::OverrideBuilder;
use nucleo_matcher::Matcher;
use nucleo_matcher::Utf32Str;
use nucleo_matcher::pattern::AtomKind;
use nucleo_matcher::pattern::CaseMatching;
use nucleo_matcher::pattern::Normalization;
use nucleo_matcher::pattern::Pattern;
use serde::Serialize;
use std::cell::UnsafeCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::process::Command as TokioCommand;

mod cli;

pub use cli::Cli;

const GIT_FILE_LIST_TTL: Duration = Duration::from_secs(1);
const GIT_REPO_INFO_NEGATIVE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct GitRepoInfo {
    repo_root: PathBuf,
    /// Search-directory path prefix from git's perspective. Always uses `/` separators and is either empty or ends with `/`.
    prefix: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GitFileListKey {
    repo_root: PathBuf,
    prefix: String,
}

#[derive(Clone)]
struct GitFileListEntry {
    created_at: Instant,
    files: Arc<Vec<String>>,
}

#[derive(Clone)]
struct GitRepoInfoEntry {
    created_at: Instant,
    repo_info: Option<GitRepoInfo>,
}

static GIT_REPO_INFO_CACHE: OnceLock<Mutex<HashMap<PathBuf, GitRepoInfoEntry>>> = OnceLock::new();
static GIT_FILE_LIST_CACHE: OnceLock<Mutex<HashMap<GitFileListKey, GitFileListEntry>>> =
    OnceLock::new();

fn canonicalize_for_cache(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn git_repo_info(search_directory: &Path) -> Option<GitRepoInfo> {
    let key = canonicalize_for_cache(search_directory);
    let cache = GIT_REPO_INFO_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    {
        let guard = cache.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(entry) = guard.get(&key) {
            if let Some(repo_info) = &entry.repo_info {
                return Some(repo_info.clone());
            }

            if entry.created_at.elapsed() < GIT_REPO_INFO_NEGATIVE_TTL {
                return None;
            }
        }
    }

    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel", "--show-prefix"])
        .current_dir(&key)
        .output()
        .ok()?;

    let repo_info = if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();
        let repo_root_raw = lines.next()?.trim();
        let prefix = lines.next().unwrap_or("").trim().to_string();

        Some(GitRepoInfo {
            repo_root: canonicalize_for_cache(Path::new(repo_root_raw)),
            prefix,
        })
    } else {
        None
    };

    let entry = GitRepoInfoEntry {
        created_at: Instant::now(),
        repo_info: repo_info.clone(),
    };
    let mut guard = cache.lock().unwrap_or_else(|err| err.into_inner());
    guard.retain(|_, entry| {
        entry.repo_info.is_some() || entry.created_at.elapsed() < GIT_REPO_INFO_NEGATIVE_TTL
    });
    guard.insert(key, entry);

    repo_info
}

fn normalize_git_path(path: &str) -> String {
    #[cfg(windows)]
    {
        path.replace('/', "\\")
    }

    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

fn excluded_by_matcher(exclude_matcher: Option<&Override>, path: &str) -> bool {
    let Some(exclude_matcher) = exclude_matcher else {
        return false;
    };

    if exclude_matcher.matched(path, false).is_ignore() {
        return true;
    }

    #[cfg(windows)]
    {
        if path.contains('\\') {
            let path = path.replace('\\', "/");
            return exclude_matcher.matched(&path, false).is_ignore();
        }
    }

    false
}

fn git_ls_files(repo_root: &Path, prefix: &str, extra_args: &[&str]) -> Option<Vec<String>> {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root);
    cmd.arg("ls-files").arg("-z");
    cmd.args(extra_args);

    let pathspec = prefix.trim_end_matches('/');
    if !pathspec.is_empty() {
        cmd.arg("--").arg(pathspec);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let mut paths = Vec::new();
    for entry in output.stdout.split(|b| *b == b'\0') {
        if entry.is_empty() {
            continue;
        }
        let Ok(path) = std::str::from_utf8(entry) else {
            continue;
        };

        let rel = if prefix.is_empty() {
            path
        } else if let Some(stripped) = path.strip_prefix(prefix) {
            stripped
        } else {
            continue;
        };
        paths.push(normalize_git_path(rel));
    }
    Some(paths)
}

fn git_files_for_search_directory(search_directory: &Path) -> Option<Arc<Vec<String>>> {
    let repo_info = git_repo_info(search_directory)?;
    let key = GitFileListKey {
        repo_root: repo_info.repo_root.clone(),
        prefix: repo_info.prefix.clone(),
    };

    let cache = GIT_FILE_LIST_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let guard = cache.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(entry) = guard.get(&key)
            && entry.created_at.elapsed() < GIT_FILE_LIST_TTL
        {
            return Some(entry.files.clone());
        }
    }

    let mut tracked = git_ls_files(&repo_info.repo_root, &repo_info.prefix, &[])?;
    let deleted = git_ls_files(&repo_info.repo_root, &repo_info.prefix, &["--deleted"])?;
    let untracked = git_ls_files(
        &repo_info.repo_root,
        &repo_info.prefix,
        &["--others", "--exclude-standard"],
    )?;

    let deleted_set: HashSet<String> = deleted.into_iter().collect();
    tracked.retain(|p| !deleted_set.contains(p));
    tracked.extend(untracked);

    let files = Arc::new(tracked);
    let entry = GitFileListEntry {
        created_at: Instant::now(),
        files: files.clone(),
    };

    let mut guard = cache.lock().unwrap_or_else(|err| err.into_inner());
    guard.retain(|_, entry| entry.created_at.elapsed() < GIT_FILE_LIST_TTL);
    guard.insert(key, entry);

    Some(files)
}

/// A single match result returned from the search.
///
/// * `score` – Relevance score returned by `nucleo_matcher`.
/// * `path`  – Path to the matched file (relative to the search directory).
/// * `indices` – Optional list of character indices that matched the query.
///   These are only filled when the caller of [`run`] sets
///   `compute_indices` to `true`.  The indices vector follows the
///   guidance from `nucleo_matcher::Pattern::indices`: they are
///   unique and sorted in ascending order so that callers can use
///   them directly for highlighting.
#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub score: u32,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>, // Sorted & deduplicated when present
}

/// Returns the final path component for a matched path, falling back to the full path.
pub fn file_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[derive(Debug)]
pub struct FileSearchResults {
    pub matches: Vec<FileMatch>,
    pub total_match_count: usize,
}

pub trait Reporter {
    fn report_match(&self, file_match: &FileMatch);
    fn warn_matches_truncated(&self, total_match_count: usize, shown_match_count: usize);
    fn warn_no_search_pattern(&self, search_directory: &Path);
}

pub async fn run_main<T: Reporter>(
    Cli {
        pattern,
        limit,
        cwd,
        compute_indices,
        json: _,
        exclude,
        threads,
    }: Cli,
    reporter: T,
) -> anyhow::Result<()> {
    let search_directory = match cwd {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };
    let pattern_text = match pattern {
        Some(pattern) => pattern,
        None => {
            reporter.warn_no_search_pattern(&search_directory);
            #[cfg(unix)]
            TokioCommand::new("ls")
                .arg("-al")
                .current_dir(search_directory)
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .await?;
            #[cfg(windows)]
            {
                TokioCommand::new("cmd")
                    .arg("/c")
                    .arg(search_directory)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()
                    .await?;
            }
            return Ok(());
        }
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let FileSearchResults {
        total_match_count,
        matches,
    } = run(
        &pattern_text,
        limit,
        &search_directory,
        exclude,
        threads,
        cancel_flag,
        compute_indices,
        true,
    )?;
    let match_count = matches.len();
    let matches_truncated = total_match_count > match_count;

    for file_match in matches {
        reporter.report_match(&file_match);
    }
    if matches_truncated {
        reporter.warn_matches_truncated(total_match_count, match_count);
    }

    Ok(())
}

/// The worker threads will periodically check `cancel_flag` to see if they
/// should stop processing files.
#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern_text: &str,
    limit: NonZero<usize>,
    search_directory: &Path,
    exclude: Vec<String>,
    threads: NonZero<usize>,
    cancel_flag: Arc<AtomicBool>,
    compute_indices: bool,
    respect_gitignore: bool,
) -> anyhow::Result<FileSearchResults> {
    let pattern = create_pattern(pattern_text);
    // Create one BestMatchesList per worker thread so that each worker can
    // operate independently. The results across threads will be merged when
    // the traversal is complete.
    let WorkerCount {
        num_walk_builder_threads,
        num_best_matches_lists,
    } = create_worker_count(threads);
    let best_matchers_per_worker: Vec<UnsafeCell<BestMatchesList>> = (0..num_best_matches_lists)
        .map(|_| {
            UnsafeCell::new(BestMatchesList::new(
                limit.get(),
                pattern.clone(),
                Matcher::new(nucleo_matcher::Config::DEFAULT),
            ))
        })
        .collect();

    let exclude_matcher = if exclude.is_empty() {
        None
    } else {
        let mut override_builder = OverrideBuilder::new(search_directory);
        for exclude in &exclude {
            // The `!` prefix is used to indicate an exclude pattern.
            let exclude_pattern = format!("!{exclude}");
            override_builder.add(&exclude_pattern)?;
        }
        Some(override_builder.build()?)
    };

    let git_best_lists = if respect_gitignore {
        git_files_for_search_directory(search_directory).map(|files| {
            build_best_lists_for_paths(
                files,
                limit.get(),
                pattern.clone(),
                cancel_flag.clone(),
                exclude_matcher.clone(),
                num_best_matches_lists,
            )
        })
    } else {
        None
    };

    if git_best_lists.is_none() {
        // Fall back to a raw filesystem walk when git isn't available or we aren't in a repository.
        //
        // If `respect_gitignore` is set, we keep ignore filtering enabled so `.gitignore`/`.ignore`
        // files are still honored in non-git directories. If it isn't set, we disable ignore
        // filtering and walk everything.
        //
        // We use the same tree-walker library that ripgrep uses so that we can leverage the
        // parallelism it provides.
        let mut walk_builder = WalkBuilder::new(search_directory);
        walk_builder
            .threads(num_walk_builder_threads)
            // Allow hidden entries.
            .hidden(false)
            // Follow symlinks to search their contents.
            .follow_links(true);

        if respect_gitignore {
            // Don't require git to be present to apply git-related ignore rules.
            walk_builder.require_git(false);
        } else {
            // Do not apply ignore rules when git isn't being used.
            walk_builder
                .git_ignore(false)
                .git_global(false)
                .git_exclude(false)
                .ignore(false)
                .parents(false);
        }

        if let Some(matcher) = &exclude_matcher {
            walk_builder.overrides(matcher.clone());
        }

        let walker = walk_builder.build_parallel();

        // Each worker created by `WalkParallel::run()` will have its own `BestMatchesList` to update.
        let index_counter = AtomicUsize::new(0);
        walker.run(|| {
            let index = index_counter.fetch_add(1, Ordering::Relaxed);
            let best_list_ptr = best_matchers_per_worker[index].get();
            let best_list = unsafe { &mut *best_list_ptr };

            // Each worker keeps a local counter so we only read the atomic flag
            // every N entries which is cheaper than checking on every file.
            const CHECK_INTERVAL: usize = 1024;
            let mut processed = 0;

            let cancel = cancel_flag.clone();

            Box::new(move |entry| {
                if let Some(path) = get_file_path(&entry, search_directory) {
                    best_list.insert(path);
                }

                processed += 1;
                if processed % CHECK_INTERVAL == 0 && cancel.load(Ordering::Relaxed) {
                    ignore::WalkState::Quit
                } else {
                    ignore::WalkState::Continue
                }
            })
        });
    }

    fn get_file_path<'a>(
        entry_result: &'a Result<ignore::DirEntry, ignore::Error>,
        search_directory: &std::path::Path,
    ) -> Option<&'a str> {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => return None,
        };
        if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            return None;
        }
        let path = entry.path();
        match path.strip_prefix(search_directory) {
            Ok(rel_path) => rel_path.to_str(),
            Err(_) => None,
        }
    }

    // If the cancel flag is set, we return early with an empty result.
    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(FileSearchResults {
            matches: Vec::new(),
            total_match_count: 0,
        });
    }

    // Merge results across best_matchers_per_worker.
    let mut global_heap: BinaryHeap<Reverse<(u32, String)>> = BinaryHeap::new();
    let mut total_match_count = 0;
    if let Some(best_lists) = &git_best_lists {
        for best_list in best_lists {
            total_match_count += best_list.num_matches;
            for &Reverse((score, ref line)) in best_list.binary_heap.iter() {
                if global_heap.len() < limit.get() {
                    global_heap.push(Reverse((score, line.clone())));
                } else if let Some(min_element) = global_heap.peek()
                    && score > min_element.0.0
                {
                    global_heap.pop();
                    global_heap.push(Reverse((score, line.clone())));
                }
            }
        }
    } else {
        for best_list_cell in best_matchers_per_worker.iter() {
            let best_list = unsafe { &*best_list_cell.get() };
            total_match_count += best_list.num_matches;
            for &Reverse((score, ref line)) in best_list.binary_heap.iter() {
                if global_heap.len() < limit.get() {
                    global_heap.push(Reverse((score, line.clone())));
                } else if let Some(min_element) = global_heap.peek()
                    && score > min_element.0.0
                {
                    global_heap.pop();
                    global_heap.push(Reverse((score, line.clone())));
                }
            }
        }
    }

    let mut raw_matches: Vec<(u32, String)> = global_heap.into_iter().map(|r| r.0).collect();
    sort_matches(&mut raw_matches);

    // Transform into `FileMatch`, optionally computing indices.
    let mut matcher = if compute_indices {
        Some(Matcher::new(nucleo_matcher::Config::DEFAULT))
    } else {
        None
    };

    let matches: Vec<FileMatch> = raw_matches
        .into_iter()
        .map(|(score, path)| {
            let indices = if compute_indices {
                let mut buf = Vec::<char>::new();
                let haystack: Utf32Str<'_> = Utf32Str::new(&path, &mut buf);
                let mut idx_vec: Vec<u32> = Vec::new();
                if let Some(ref mut m) = matcher {
                    // Ignore the score returned from indices – we already have `score`.
                    pattern.indices(haystack, m, &mut idx_vec);
                }
                idx_vec.sort_unstable();
                idx_vec.dedup();
                Some(idx_vec)
            } else {
                None
            };

            FileMatch {
                score,
                path,
                indices,
            }
        })
        .collect();

    Ok(FileSearchResults {
        matches,
        total_match_count,
    })
}

fn build_best_lists_for_paths(
    files: Arc<Vec<String>>,
    max_count: usize,
    pattern: Pattern,
    cancel_flag: Arc<AtomicBool>,
    exclude_matcher: Option<Override>,
    worker_count: usize,
) -> Vec<BestMatchesList> {
    if files.is_empty() {
        return Vec::new();
    }

    let worker_count = worker_count.clamp(1, files.len());
    if worker_count == 1 {
        let mut best_list = BestMatchesList::new(
            max_count,
            pattern,
            Matcher::new(nucleo_matcher::Config::DEFAULT),
        );

        const CHECK_INTERVAL: usize = 1024;
        let mut processed = 0usize;

        for path in &*files {
            if excluded_by_matcher(exclude_matcher.as_ref(), path.as_str()) {
                continue;
            }
            best_list.insert(path.as_str());

            processed += 1;
            if processed.is_multiple_of(CHECK_INTERVAL) && cancel_flag.load(Ordering::Relaxed) {
                break;
            }
        }

        return vec![best_list];
    }

    let chunk_size = files.len().div_ceil(worker_count).max(1);
    let mut handles = Vec::with_capacity(worker_count);

    for idx in 0..worker_count {
        let start = idx * chunk_size;
        if start >= files.len() {
            break;
        }
        let end = ((idx + 1) * chunk_size).min(files.len());

        let files = files.clone();
        let pattern = pattern.clone();
        let cancel_flag = cancel_flag.clone();
        let exclude_matcher = exclude_matcher.clone();

        handles.push(std::thread::spawn(move || {
            let mut best_list = BestMatchesList::new(
                max_count,
                pattern,
                Matcher::new(nucleo_matcher::Config::DEFAULT),
            );

            const CHECK_INTERVAL: usize = 1024;
            let mut processed = 0usize;

            for path in &files[start..end] {
                if excluded_by_matcher(exclude_matcher.as_ref(), path.as_str()) {
                    continue;
                }
                best_list.insert(path.as_str());

                processed += 1;
                if processed.is_multiple_of(CHECK_INTERVAL) && cancel_flag.load(Ordering::Relaxed) {
                    break;
                }
            }

            best_list
        }));
    }

    let mut best_lists = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(best_list) = handle.join() {
            best_lists.push(best_list);
        }
    }

    best_lists
}

/// Sort matches in-place by descending score, then ascending path.
fn sort_matches(matches: &mut [(u32, String)]) {
    matches.sort_by(cmp_by_score_desc_then_path_asc::<(u32, String), _, _>(
        |t| t.0,
        |t| t.1.as_str(),
    ));
}

/// Returns a comparator closure suitable for `slice.sort_by(...)` that orders
/// items by descending score and then ascending path using the provided accessors.
pub fn cmp_by_score_desc_then_path_asc<T, FScore, FPath>(
    score_of: FScore,
    path_of: FPath,
) -> impl FnMut(&T, &T) -> std::cmp::Ordering
where
    FScore: Fn(&T) -> u32,
    FPath: Fn(&T) -> &str,
{
    use std::cmp::Ordering;
    move |a, b| match score_of(b).cmp(&score_of(a)) {
        Ordering::Equal => path_of(a).cmp(path_of(b)),
        other => other,
    }
}

/// Maintains the `max_count` best matches for a given pattern.
struct BestMatchesList {
    max_count: usize,
    num_matches: usize,
    pattern: Pattern,
    matcher: Matcher,
    binary_heap: BinaryHeap<Reverse<(u32, String)>>,

    /// Internal buffer for converting strings to UTF-32.
    utf32buf: Vec<char>,
}

impl BestMatchesList {
    fn new(max_count: usize, pattern: Pattern, matcher: Matcher) -> Self {
        Self {
            max_count,
            num_matches: 0,
            pattern,
            matcher,
            binary_heap: BinaryHeap::new(),
            utf32buf: Vec::<char>::new(),
        }
    }

    fn insert(&mut self, line: &str) {
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut self.utf32buf);
        if let Some(score) = self.pattern.score(haystack, &mut self.matcher) {
            // In the tests below, we verify that score() returns None for a
            // non-match, so we can categorically increment the count here.
            self.num_matches += 1;

            if self.binary_heap.len() < self.max_count {
                self.binary_heap.push(Reverse((score, line.to_string())));
            } else if let Some(min_element) = self.binary_heap.peek()
                && score > min_element.0.0
            {
                self.binary_heap.pop();
                self.binary_heap.push(Reverse((score, line.to_string())));
            }
        }
    }
}

struct WorkerCount {
    num_walk_builder_threads: usize,
    num_best_matches_lists: usize,
}

fn create_worker_count(num_workers: NonZero<usize>) -> WorkerCount {
    // It appears that the number of times the function passed to
    // `WalkParallel::run()` is called is: the number of threads specified to
    // the builder PLUS ONE.
    //
    // In `WalkParallel::visit()`, the builder function gets called once here:
    // https://github.com/BurntSushi/ripgrep/blob/79cbe89deb1151e703f4d91b19af9cdcc128b765/crates/ignore/src/walk.rs#L1233
    //
    // And then once for every worker here:
    // https://github.com/BurntSushi/ripgrep/blob/79cbe89deb1151e703f4d91b19af9cdcc128b765/crates/ignore/src/walk.rs#L1288
    let num_walk_builder_threads = num_workers.get();
    let num_best_matches_lists = num_walk_builder_threads + 1;

    WorkerCount {
        num_walk_builder_threads,
        num_best_matches_lists,
    }
}

fn create_pattern(pattern: &str) -> Pattern {
    Pattern::new(
        pattern,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn run_git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git {:?} failed.\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn mk_repo() -> TempDir {
        let dir = TempDir::new().expect("tempdir should create");
        run_git(dir.path(), &["init"]);
        run_git(dir.path(), &["config", "user.email", "codex@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Codex Tests"]);
        dir
    }

    #[test]
    fn verify_score_is_none_for_non_match() {
        let mut utf32buf = Vec::<char>::new();
        let line = "hello";
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut utf32buf);
        let pattern = create_pattern("zzz");
        let score = pattern.score(haystack, &mut matcher);
        assert_eq!(score, None);
    }

    #[test]
    fn tie_breakers_sort_by_path_when_scores_equal() {
        let mut matches = vec![
            (100, "b_path".to_string()),
            (100, "a_path".to_string()),
            (90, "zzz".to_string()),
        ];

        sort_matches(&mut matches);

        // Highest score first; ties broken alphabetically.
        let expected = vec![
            (100, "a_path".to_string()),
            (100, "b_path".to_string()),
            (90, "zzz".to_string()),
        ];

        assert_eq!(matches, expected);
    }

    #[test]
    fn file_name_from_path_uses_basename() {
        assert_eq!(file_name_from_path("foo/bar.txt"), "bar.txt");
    }

    #[test]
    fn file_name_from_path_falls_back_to_full_path() {
        assert_eq!(file_name_from_path(""), "");
    }

    #[test]
    fn gitignore_filters_untracked_ignored_files_and_dirs() -> anyhow::Result<()> {
        let repo = mk_repo();
        std::fs::write(
            repo.path().join(".gitignore"),
            "ignored_dir/\nignored_file.txt\n",
        )?;
        std::fs::write(repo.path().join("ignored_file.txt"), "ignored")?;
        std::fs::create_dir_all(repo.path().join("ignored_dir"))?;
        std::fs::write(repo.path().join("ignored_dir").join("a.txt"), "ignored")?;
        std::fs::write(repo.path().join("kept.txt"), "kept")?;

        let results = run(
            "txt",
            NonZero::new(50).unwrap(),
            repo.path(),
            Vec::new(),
            NonZero::new(2).unwrap(),
            Arc::new(AtomicBool::new(false)),
            false,
            true,
        )?;

        let paths: Vec<&str> = results.matches.iter().map(|m| m.path.as_str()).collect();
        let ignored_path = Path::new("ignored_dir")
            .join("a.txt")
            .to_string_lossy()
            .to_string();
        assert!(paths.contains(&"kept.txt"));
        assert!(!paths.contains(&"ignored_file.txt"));
        assert!(!paths.contains(&ignored_path.as_str()));
        Ok(())
    }

    #[test]
    fn gitignore_filters_untracked_ignored_files_and_dirs_outside_git_repo() -> anyhow::Result<()> {
        let dir = TempDir::new().expect("tempdir should create");
        std::fs::write(
            dir.path().join(".gitignore"),
            "ignored_dir/\nignored_file.txt\n",
        )?;
        std::fs::write(dir.path().join("ignored_file.txt"), "ignored")?;
        std::fs::create_dir_all(dir.path().join("ignored_dir"))?;
        std::fs::write(dir.path().join("ignored_dir").join("a.txt"), "ignored")?;
        std::fs::write(dir.path().join("kept.txt"), "kept")?;

        let results = run(
            "txt",
            NonZero::new(50).unwrap(),
            dir.path(),
            Vec::new(),
            NonZero::new(2).unwrap(),
            Arc::new(AtomicBool::new(false)),
            false,
            true,
        )?;

        let paths: Vec<&str> = results.matches.iter().map(|m| m.path.as_str()).collect();
        let ignored_path = Path::new("ignored_dir")
            .join("a.txt")
            .to_string_lossy()
            .to_string();
        assert!(paths.contains(&"kept.txt"));
        assert!(!paths.contains(&"ignored_file.txt"));
        assert!(!paths.contains(&ignored_path.as_str()));
        Ok(())
    }

    #[test]
    fn gitignore_does_not_filter_tracked_files() -> anyhow::Result<()> {
        let repo = mk_repo();
        std::fs::write(repo.path().join("tracked.log"), "tracked")?;
        run_git(repo.path(), &["add", "tracked.log"]);
        run_git(repo.path(), &["commit", "-m", "track"]);

        std::fs::write(repo.path().join(".gitignore"), "*.log\n")?;
        std::fs::write(repo.path().join("ignored.log"), "ignored")?;

        let results = run(
            "log",
            NonZero::new(50).unwrap(),
            repo.path(),
            Vec::new(),
            NonZero::new(2).unwrap(),
            Arc::new(AtomicBool::new(false)),
            false,
            true,
        )?;

        let paths: Vec<&str> = results.matches.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.contains(&"tracked.log"));
        assert!(!paths.contains(&"ignored.log"));
        Ok(())
    }

    #[test]
    fn git_repo_info_revalidates_negative_cache_entries() -> anyhow::Result<()> {
        let dir = TempDir::new().expect("tempdir should create");

        assert!(git_repo_info(dir.path()).is_none());
        run_git(dir.path(), &["init"]);

        let key = canonicalize_for_cache(dir.path());
        let cache = GIT_REPO_INFO_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().unwrap_or_else(|err| err.into_inner());
        let entry = guard
            .get_mut(&key)
            .expect("git_repo_info should cache negative results");
        entry.created_at = Instant::now() - GIT_REPO_INFO_NEGATIVE_TTL - Duration::from_secs(1);
        drop(guard);

        assert!(git_repo_info(dir.path()).is_some());
        Ok(())
    }
}
