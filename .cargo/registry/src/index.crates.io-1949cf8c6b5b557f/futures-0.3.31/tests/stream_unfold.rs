use futures::future;
use futures::stream;
use futures_test::future::FutureTestExt;
use futures_test::{assert_stream_done, assert_stream_next, assert_stream_pending};

#[test]
fn unfold1() {
    let mut stream = stream::unfold(0, |state| {
        if state <= 2 {
            future::ready(Some((state * 2, state + 1))).pending_once()
        } else {
            future::ready(None).pending_once()
        }
    });

    // Creates the future with the closure
    // Not ready (delayed future)
    assert_stream_pending!(stream);
    // Future is ready, yields the item
    assert_stream_next!(stream, 0);

    // Repeat
    assert_stream_pending!(stream);
    assert_stream_next!(stream, 2);

    assert_stream_pending!(stream);
    assert_stream_next!(stream, 4);

    // No more items
    assert_stream_pending!(stream);
    assert_stream_done!(stream);
}
