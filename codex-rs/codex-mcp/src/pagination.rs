use std::collections::HashSet;
use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use rmcp::model::PaginatedRequestParams;

const MAX_MCP_CATALOG_PAGES: usize = 100;
const MAX_MCP_CATALOG_ITEMS: usize = 2_048;
const MAX_MCP_PAGINATION_CURSOR_BYTES: usize = 64 * 1024;
const DEFAULT_MCP_PAGINATION_TIMEOUT: Duration = Duration::from_secs(30);

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
        let mut page_count = 0;

        loop {
            if page_count == MAX_MCP_CATALOG_PAGES {
                return Err(anyhow!(
                    "{method} exceeded the pagination limit of {MAX_MCP_CATALOG_PAGES} pages"
                ));
            }
            page_count += 1;
            let params = cursor.as_ref().map(|next: &String| {
                PaginatedRequestParams::default().with_cursor(Some(next.clone()))
            });
            let (items, next_cursor) = fetch(params).await?;
            if items.len() > MAX_MCP_CATALOG_ITEMS.saturating_sub(collected.len()) {
                return Err(anyhow!(
                    "{method} exceeded the catalog limit of {MAX_MCP_CATALOG_ITEMS} items"
                ));
            }
            collected.extend(items);

            let Some(next_cursor) = next_cursor else {
                return Ok(collected);
            };
            if next_cursor.len() > MAX_MCP_PAGINATION_CURSOR_BYTES {
                return Err(anyhow!(
                    "{method} returned a pagination cursor exceeding {MAX_MCP_PAGINATION_CURSOR_BYTES} bytes"
                ));
            }
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(anyhow!("{method} returned a repeated pagination cursor"));
            }
            cursor = Some(next_cursor);
        }
    };

    let timeout = overall_timeout.unwrap_or(DEFAULT_MCP_PAGINATION_TIMEOUT);
    tokio::time::timeout(timeout, collect)
        .await
        .map_err(|_| anyhow!("{method} pagination timed out after {timeout:?}"))?
}

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod tests;
