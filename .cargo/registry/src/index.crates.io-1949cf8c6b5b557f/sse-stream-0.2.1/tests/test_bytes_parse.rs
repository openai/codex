use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::Full;

#[tokio::test]
async fn test_bytes_parse() {
    let bytes = include_bytes!("data/test_stream.sse");
    let body = Full::<Bytes>::from(bytes.to_vec());

    let mut sse_body = sse_stream::SseStream::new(body);
    while let Some(sse) = sse_body.next().await {
        println!("{:?}", sse.unwrap());
    }
}
