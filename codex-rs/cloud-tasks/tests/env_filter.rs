use codex_cloud_tasks_client::CloudBackend;
use codex_cloud_tasks_client::MockClient;

#[tokio::test]
async fn mock_backend_varies_by_env() {
    let client = MockClient;

    let root = CloudBackend::list_tasks(&client, None).await.unwrap();
    assert!(root.iter().any(|t| t.title.contains("Update README")));

    let a = CloudBackend::list_tasks(&client, Some("env-A"))
        .await
        .unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].title, "A: First");

    let b = CloudBackend::list_tasks(&client, Some("env-B"))
        .await
        .unwrap();
    assert_eq!(b.len(), 2);
    assert!(b[0].title.starts_with("B: "));
}
