use async_stream::stream;
use futures_util::StreamExt;

use std::pin::pin;

#[tokio::main]
async fn main() {
    let mut outer = vec![];
    {
        let v = vec![0; 10];
        let v_ref = &v;
        let mut s = pin!(stream! {
            for x in v_ref {
                yield x
            }
        });
        while let Some(x) = s.next().await {
            outer.push(x);
        }
    };
    // use-after-free
    println!("{outer:?}"); // […garbage allocator internals…, 0, 0, 0]
}
