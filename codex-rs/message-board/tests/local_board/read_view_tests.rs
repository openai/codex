//! Fixed reads share SQLite storage while preserving the captured catalog and post boundary.

#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn read_views_freeze_catalog_and_pages_without_changing_live_handles()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?);
    let root = ThreadId::new();
    let host = Arc::new(Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(root, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let writer = LocalAgentMessageBoard::open(
        &sqlite,
        &LocalBoardOptions::default(),
        root.into(),
        host.clone(),
    )
    .await?;
    let reader = LocalBoard::new(sqlite.clone(), root.into())
        .read_only()
        .open(host.clone())
        .await?;
    for channel in ["alpha", "beta", "empty"] {
        writer
            .upsert_channel(
                root,
                UpsertChannelRequest {
                    channel_name: channel.into(),
                    description: "Original purpose".to_owned().try_into()?,
                },
            )
            .await?;
    }
    let original = AppendRequest {
        request_id: "original".into(),
        destination: PostDestination::Channel("alpha".into()),
        text: "proof original".into(),
    };
    let first = writer.append(root, original).await?;
    for (request_id, destination) in [
        ("second", PostDestination::Channel("alpha".into())),
        ("reply-one", PostDestination::Thread(first.thread_id)),
        ("reply-two", PostDestination::Thread(first.thread_id)),
        ("beta", PostDestination::Channel("beta".into())),
    ] {
        writer
            .append(
                root,
                AppendRequest {
                    request_id: request_id.into(),
                    destination,
                    text: format!("proof {request_id}"),
                },
            )
            .await?;
    }
    let catalog_query = ChannelQuery {
        query: None,
        direction: SortDirection::NewestFirst,
        page: PageRequest::default(),
    };
    let before_catalog = reader.list_channels(root, catalog_query.clone()).await?;
    let fixed = writer.read_view(root).await?;
    assert_eq!(fixed.channels(), before_catalog.channels.results);
    assert_eq!(
        fixed.list_channels(root, catalog_query.clone()).await?,
        before_catalog
    );
    let page = PageRequest {
        cursor: None,
        limit: NonZeroU32::MIN,
    };
    let search = PostQuery {
        channel_name: Some("alpha".into()),
        query: Some("PROOF".into()),
        after_message_id: None,
        author: Some(AgentPath::root()),
        page: page.clone(),
        max_chars_per_post: NonZeroU32::MAX,
    };
    let before_search = reader.search_posts(root, search.clone()).await?;
    let next_search = PostQuery {
        page: PageRequest {
            cursor: Some(before_search.next_cursor.clone().unwrap()),
            ..page.clone()
        },
        ..search.clone()
    };
    let before_next_search = reader.search_posts(root, next_search.clone()).await?;
    let threads = ThreadQuery {
        channel_name: "alpha".into(),
        sort: ThreadSort::Activity,
        direction: SortDirection::OldestFirst,
        page: PageRequest::default(),
        max_chars_per_post: NonZeroU32::MAX,
    };
    let before_threads = reader.list_threads(root, threads.clone()).await?;
    let read_thread = ReadThreadRequest {
        thread_id: first.thread_id,
        page: page.clone(),
        max_chars_per_post: NonZeroU32::MAX,
    };
    let before_replies = reader.read_thread(root, read_thread.clone()).await?;
    let read = ReadPostRequest {
        message_id: first.message_id,
        offset_chars: 1,
        limit_chars: NonZeroU32::MAX,
    };
    let content = reader.read_post(root, read.clone()).await?;

    // Both newer and backdated appends must stay outside the fixed sequence boundary.
    let mut later = Vec::new();
    for clock in [100, -100] {
        host.clock.store(clock, Ordering::SeqCst);
        for destination in [
            PostDestination::Channel("alpha".into()),
            PostDestination::Thread(first.thread_id),
        ] {
            later.push(
                writer
                    .append(
                        root,
                        AppendRequest {
                            request_id: format!("later-{clock}-{destination:?}"),
                            destination,
                            text: "proof later".into(),
                        },
                    )
                    .await?,
            );
        }
    }
    for channel in ["alpha", "new"] {
        writer
            .upsert_channel(
                root,
                UpsertChannelRequest {
                    channel_name: channel.into(),
                    description: "Updated purpose".to_owned().try_into()?,
                },
            )
            .await?;
    }
    assert_eq!(
        fixed.search_posts(root, search.clone()).await?,
        before_search
    );
    assert_eq!(
        fixed.search_posts(root, next_search).await?,
        before_next_search
    );
    assert_eq!(fixed.list_threads(root, threads).await?, before_threads);
    assert_eq!(
        fixed.read_thread(root, read_thread.clone()).await?,
        before_replies
    );
    assert_eq!(fixed.read_post(root, read.clone()).await?, content);
    assert_eq!(
        fixed.list_channels(root, catalog_query.clone()).await?,
        before_catalog
    );
    for post in later {
        assert!(
            fixed
                .read_post(
                    root,
                    ReadPostRequest {
                        message_id: post.message_id,
                        ..read.clone()
                    }
                )
                .await
                .is_err()
        );
        assert!(
            fixed
                .search_posts(
                    root,
                    PostQuery {
                        after_message_id: Some(post.message_id),
                        ..search.clone()
                    }
                )
                .await
                .is_err()
        );
        if post.message_id == post.thread_id {
            assert!(
                fixed
                    .read_thread(
                        root,
                        ReadThreadRequest {
                            thread_id: post.thread_id,
                            ..read_thread.clone()
                        }
                    )
                    .await
                    .is_err()
            );
        }
    }
    assert!(
        fixed
            .list_threads(
                root,
                ThreadQuery {
                    channel_name: "new".into(),
                    sort: ThreadSort::Created,
                    direction: SortDirection::NewestFirst,
                    page: page.clone(),
                    max_chars_per_post: NonZeroU32::MAX,
                }
            )
            .await
            .is_err()
    );
    assert_ne!(
        reader.search_posts(root, search.clone()).await?,
        fixed.search_posts(root, search).await?
    );
    assert_ne!(
        reader.list_channels(root, catalog_query.clone()).await?,
        before_catalog
    );
    assert_eq!(
        writer.read_view(root).await?.channels(),
        reader
            .list_channels(root, catalog_query.clone())
            .await?
            .channels
            .results
    );

    // A read-only live connection still permits its caller's subscription changes.
    let subscription = SubscriptionRequest {
        target: SubscriptionTarget::Channel("alpha".into()),
        target_agent: None,
        change: SubscriptionChange::Subscribe,
    };
    reader.set_subscription(root, subscription).await?;
    assert_eq!(
        reader.subscribed_channels(root, page).await?.results,
        vec!["alpha".to_owned()]
    );
    Ok(())
}
