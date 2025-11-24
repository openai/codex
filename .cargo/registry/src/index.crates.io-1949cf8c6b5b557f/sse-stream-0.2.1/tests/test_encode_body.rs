use std::convert::Infallible;

use futures_util::StreamExt;
use sse_stream::{Sse, SseBody, SseStream};

#[tokio::test]
async fn test_encode_body() {
    let sse_sequence = [
        Sse::default().event("1").data("....."),
        Sse::default().event("2").data("....."),
        Sse::default().event("3").data("....."),
        Sse::default().event("4").data("....."),
    ];
    let stream =
        futures_util::stream::iter(sse_sequence.clone()).map(Result::<Sse, Infallible>::Ok);
    let body = SseBody::new(stream);
    let mut stream = SseStream::new(body);
    let mut receive_count = 0;
    while let Some(sse) = stream.next().await {
        let sse = sse.unwrap();
        assert_eq!(sse, sse_sequence[receive_count]);
        receive_count += 1;
    }
}
