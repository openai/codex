use tokio_stream::{self as stream, StreamExt};
use tokio_test::{assert_pending, assert_ready, assert_ready_err, assert_ready_ok, task};

mod support {
    pub(crate) mod mpsc;
}

use support::mpsc;

#[allow(clippy::let_unit_value)]
#[tokio::test]
async fn empty_unit() {
    // Drains the stream.
    let mut iter = vec![(), (), ()].into_iter();
    let _: () = stream::iter(&mut iter).collect().await;
    assert!(iter.next().is_none());
}

#[tokio::test]
async fn empty_vec() {
    let coll: Vec<u32> = stream::empty().collect().await;
    assert!(coll.is_empty());
}

#[tokio::test]
async fn empty_box_slice() {
    let coll: Box<[u32]> = stream::empty().collect().await;
    assert!(coll.is_empty());
}

#[tokio::test]
async fn empty_string() {
    let coll: String = stream::empty::<&str>().collect().await;
    assert!(coll.is_empty());
}

#[tokio::test]
async fn empty_result() {
    let coll: Result<Vec<u32>, &str> = stream::empty().collect().await;
    assert_eq!(Ok(vec![]), coll);
}

#[tokio::test]
async fn collect_vec_items() {
    let (tx, rx) = mpsc::unbounded_channel_stream();
    let mut fut = task::spawn(rx.collect::<Vec<i32>>());

    assert_pending!(fut.poll());

    tx.send(1).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    tx.send(2).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    drop(tx);
    assert!(fut.is_woken());
    let coll = assert_ready!(fut.poll());
    assert_eq!(vec![1, 2], coll);
}

#[tokio::test]
async fn collect_string_items() {
    let (tx, rx) = mpsc::unbounded_channel_stream();

    let mut fut = task::spawn(rx.collect::<String>());

    assert_pending!(fut.poll());

    tx.send("hello ".to_string()).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    tx.send("world".to_string()).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    drop(tx);
    assert!(fut.is_woken());
    let coll = assert_ready!(fut.poll());
    assert_eq!("hello world", coll);
}

#[tokio::test]
async fn collect_str_items() {
    let (tx, rx) = mpsc::unbounded_channel_stream();

    let mut fut = task::spawn(rx.collect::<String>());

    assert_pending!(fut.poll());

    tx.send("hello ").unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    tx.send("world").unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    drop(tx);
    assert!(fut.is_woken());
    let coll = assert_ready!(fut.poll());
    assert_eq!("hello world", coll);
}

#[tokio::test]
async fn collect_results_ok() {
    let (tx, rx) = mpsc::unbounded_channel_stream();

    let mut fut = task::spawn(rx.collect::<Result<String, &str>>());

    assert_pending!(fut.poll());

    tx.send(Ok("hello ")).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    tx.send(Ok("world")).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    drop(tx);
    assert!(fut.is_woken());
    let coll = assert_ready_ok!(fut.poll());
    assert_eq!("hello world", coll);
}

#[tokio::test]
async fn collect_results_err() {
    let (tx, rx) = mpsc::unbounded_channel_stream();

    let mut fut = task::spawn(rx.collect::<Result<String, &str>>());

    assert_pending!(fut.poll());

    tx.send(Ok("hello ")).unwrap();
    assert!(fut.is_woken());
    assert_pending!(fut.poll());

    tx.send(Err("oh no")).unwrap();
    assert!(fut.is_woken());
    let err = assert_ready_err!(fut.poll());
    assert_eq!("oh no", err);
}
