use eyre::Report;

#[test]
fn test_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Report>();
}

#[test]
fn test_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<Report>();
}
