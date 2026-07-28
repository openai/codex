use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use rmcp::model::PaginatedRequestParams;

pub(crate) async fn collect_paginated<T, F, Fut>(
    method: &str,
    overall_timeout: Option<Duration>,
    mut fetch: F,
) -> Result<Vec<T>>
where
    F: FnMut(Option<PaginatedRequestParams>) -> Fut,
    Fut: Future<Output = Result<(Vec<T>, Option<String>)>>,
{
    let collect = async {
        let mut collected = Vec::new();
        let mut cursor = None;
        let mut seen_cursors = HashSet::new();

        loop {
            let params = cursor.as_ref().map(|next: &String| {
                PaginatedRequestParams::default().with_cursor(Some(next.clone()))
            });
            let (items, next_cursor) = fetch(params).await?;
            collected.extend(items);

            let Some(next_cursor) = next_cursor else {
                return Ok(collected);
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(anyhow!("{method} returned a repeated pagination cursor"));
            }
            cursor = Some(next_cursor);
        }
    };

    match overall_timeout {
        Some(timeout) => tokio::time::timeout(timeout, collect)
            .await
            .map_err(|_| anyhow!("{method} pagination timed out after {timeout:?}"))?,
        None => collect.await,
    }
}

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod tests;
