#[futures_test::test]
async fn it_works() {
    let fut = async { true };
    assert!(fut.await);

    let fut = async { false };
    assert!(!fut.await);
}

#[should_panic]
#[futures_test::test]
async fn it_is_being_run() {
    let fut = async { false };
    assert!(fut.await);
}

#[futures_test::test]
async fn return_ty() -> Result<(), ()> {
    Ok(())
}
