use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::anyhow;
use pretty_assertions::assert_eq;

use super::MAX_MCP_CATALOG_ITEMS;
use super::MAX_MCP_CATALOG_PAGES;
use super::MAX_MCP_PAGINATION_CURSOR_BYTES;
use super::collect_paginated;

#[tokio::test]
async fn collects_all_pages_including_an_empty_cursor() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&requests);

    let pages = collect_paginated("tools/list", /*overall_timeout*/ None, move |params| {
        let observed = Arc::clone(&observed);
        async move {
            let cursor = params.and_then(|params| params.cursor);
            observed.lock().expect("request lock").push(cursor.clone());
            match cursor.as_deref() {
                None => Ok((vec!["first"], Some(String::new()))),
                Some("") => Ok((vec!["second"], Some("last".to_string()))),
                Some("last") => Ok((vec!["third"], None)),
                Some(cursor) => Err(anyhow!("unexpected cursor: {cursor}")),
            }
        }
    })
    .await
    .expect("paginated request succeeds");

    assert_eq!(pages, vec!["first", "second", "third"]);
    assert_eq!(
        *requests.lock().expect("request lock"),
        vec![None, Some(String::new()), Some("last".to_string())]
    );
}

#[tokio::test]
async fn rejects_nonconsecutive_repeated_cursors() {
    let error = collect_paginated(
        "resources/list",
        /*overall_timeout*/ None,
        |params| async move {
            let cursor = params.and_then(|params| params.cursor);
            let next = match cursor.as_deref() {
                None => "first",
                Some("first") => "second",
                Some("second") => "first",
                Some(cursor) => return Err(anyhow!("unexpected cursor: {cursor}")),
            };
            Ok((Vec::<()>::new(), Some(next.to_string())))
        },
    )
    .await
    .expect_err("a repeated cursor must fail");

    assert_eq!(
        error.to_string(),
        "resources/list returned a repeated pagination cursor"
    );
}

#[tokio::test]
async fn rejects_excessive_pagination_before_fetching_another_page() {
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&requests);

    let error = collect_paginated(
        "tools/list",
        /*overall_timeout*/ None,
        move |_params| {
            let observed = Arc::clone(&observed);
            async move {
                let page = observed.fetch_add(1, Ordering::Relaxed);
                Ok((Vec::<()>::new(), Some(page.to_string())))
            }
        },
    )
    .await
    .expect_err("unbounded pagination must fail");

    assert_eq!(
        error.to_string(),
        format!("tools/list exceeded the pagination limit of {MAX_MCP_CATALOG_PAGES} pages")
    );
    assert_eq!(requests.load(Ordering::Relaxed), MAX_MCP_CATALOG_PAGES);
}

#[tokio::test]
async fn rejects_a_page_exceeding_the_catalog_item_limit() {
    let error = collect_paginated(
        "tools/list",
        /*overall_timeout*/ None,
        |_params| async { Ok((vec![(); MAX_MCP_CATALOG_ITEMS + 1], None)) },
    )
    .await
    .expect_err("an oversized catalog page must fail");

    assert_eq!(
        error.to_string(),
        format!("tools/list exceeded the catalog limit of {MAX_MCP_CATALOG_ITEMS} items")
    );
}

#[tokio::test]
async fn rejects_a_catalog_exceeding_the_item_limit_across_pages() {
    let error = collect_paginated(
        "tools/list",
        /*overall_timeout*/ None,
        |params| async move {
            match params.and_then(|params| params.cursor) {
                None => Ok((vec![(); MAX_MCP_CATALOG_ITEMS], Some("last".to_string()))),
                Some(cursor) if cursor == "last" => Ok((vec![()], None)),
                Some(cursor) => Err(anyhow!("unexpected cursor: {cursor}")),
            }
        },
    )
    .await
    .expect_err("catalog items must be bounded across pages");

    assert_eq!(
        error.to_string(),
        format!("tools/list exceeded the catalog limit of {MAX_MCP_CATALOG_ITEMS} items")
    );
}

#[tokio::test]
async fn rejects_oversized_pagination_cursors_before_following_them() {
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&requests);

    let error = collect_paginated(
        "resources/list",
        /*overall_timeout*/ None,
        move |_params| {
            let observed = Arc::clone(&observed);
            async move {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok((
                    Vec::<()>::new(),
                    Some("x".repeat(MAX_MCP_PAGINATION_CURSOR_BYTES + 1)),
                ))
            }
        },
    )
    .await
    .expect_err("an oversized pagination cursor must fail");

    assert_eq!(
        error.to_string(),
        format!(
            "resources/list returned a pagination cursor exceeding {MAX_MCP_PAGINATION_CURSOR_BYTES} bytes"
        )
    );
    assert_eq!(requests.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn forwards_page_failures() {
    let error = collect_paginated(
        "resources/templates/list",
        /*overall_timeout*/ None,
        |_params| async { Err::<(Vec<()>, Option<String>), _>(anyhow!("page failed")) },
    )
    .await
    .expect_err("a page error must fail");

    assert_eq!(error.to_string(), "page failed");
}

#[tokio::test(start_paused = true)]
async fn applies_a_default_timeout_when_no_timeout_is_configured() {
    let error = collect_paginated(
        "resources/list",
        /*overall_timeout*/ None,
        |_params| async {
            tokio::time::sleep(Duration::from_secs(31)).await;
            Ok((Vec::<()>::new(), None))
        },
    )
    .await
    .expect_err("pagination without a configured timeout must still be bounded");

    assert_eq!(
        error.to_string(),
        "resources/list pagination timed out after 30s"
    );
}

#[tokio::test(start_paused = true)]
async fn applies_one_timeout_across_individually_timely_pages() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&requests);

    let error = collect_paginated("tools/list", Some(Duration::from_secs(5)), move |params| {
        let observed = Arc::clone(&observed);
        async move {
            let cursor = params.and_then(|params| params.cursor);
            observed.lock().expect("request lock").push(cursor.clone());
            tokio::time::sleep(Duration::from_secs(2)).await;

            match cursor.as_deref() {
                None => Ok((vec!["first"], Some("second".to_string()))),
                Some("second") => Ok((vec!["second"], Some("third".to_string()))),
                Some("third") => Ok((vec!["third"], None)),
                Some(cursor) => Err(anyhow!("unexpected cursor: {cursor}")),
            }
        }
    })
    .await
    .expect_err("the combined page duration must exceed the shared timeout");

    assert_eq!(
        error.to_string(),
        "tools/list pagination timed out after 5s"
    );
    assert_eq!(
        *requests.lock().expect("request lock"),
        vec![None, Some("second".to_string()), Some("third".to_string())]
    );
}
