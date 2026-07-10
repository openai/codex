use std::io;

use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FS_READ_TEXT_PREFIXES_BATCH_MAX_PATHS;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use super::discovery::MAX_CONCURRENT_SKILL_LOADS;
use super::extract_frontmatter;

const SKILL_FRONTMATTER_PREFIX_BYTES: usize = 2 * 1024;

enum PrefixRead {
    Ready(io::Result<String>),
    Full(PathUri),
}

pub(super) async fn read_skill_frontmatter_texts(
    fs: &dyn ExecutorFileSystem,
    paths: &[PathUri],
) -> Vec<io::Result<String>> {
    let mut pending = Vec::with_capacity(paths.len());

    for chunk in paths.chunks(FS_READ_TEXT_PREFIXES_BATCH_MAX_PATHS) {
        let results = fs
            .read_text_prefixes_batch(chunk, SKILL_FRONTMATTER_PREFIX_BYTES, /*sandbox*/ None)
            .await;
        let results = match results {
            Ok(results) if results.len() == chunk.len() => results,
            Ok(_) | Err(_) => {
                pending.extend(chunk.iter().cloned().map(PrefixRead::Full));
                continue;
            }
        };
        for (path, result) in chunk.iter().cloned().zip(results) {
            match result {
                Ok(prefix) if extract_frontmatter(&prefix.text).is_some() => {
                    pending.push(PrefixRead::Ready(Ok(prefix.text)));
                }
                Ok(prefix) if prefix.complete => {
                    pending.push(PrefixRead::Ready(Ok(prefix.text)));
                }
                Ok(_) => pending.push(PrefixRead::Full(path)),
                Err(error) => pending.push(PrefixRead::Ready(Err(error))),
            }
        }
    }

    futures::stream::iter(pending)
        .map(|read| async move {
            match read {
                PrefixRead::Ready(result) => result,
                PrefixRead::Full(path) => fs.read_file_text(&path, /*sandbox*/ None).await,
            }
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .collect()
        .await
}

#[cfg(test)]
#[path = "text_prefix_tests.rs"]
mod tests;
