use async_stream::stream;
use futures_util::pin_mut;
use futures_util::stream::StreamExt;

#[tokio::test]
async fn spans_preserved() {
    let s = stream! {
     assert_eq!(line!(), 8);
    };
    pin_mut!(s);

    while s.next().await.is_some() {
        unreachable!();
    }
}
