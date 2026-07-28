use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::anyhow;
use pretty_assertions::assert_eq;

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
