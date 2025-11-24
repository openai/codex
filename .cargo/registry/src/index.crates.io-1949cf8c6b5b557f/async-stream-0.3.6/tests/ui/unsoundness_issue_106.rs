use async_stream::stream;
use futures_util::StreamExt;

use std::pin::pin;

macro_rules! asynk {
    ($e:expr) => {
        async { $e }
    };
}

#[tokio::main]
async fn main() {
    pin!(stream! {
        let yield_42 = asynk!(yield 42_usize);
        let s = stream! {
            yield Box::new(12345);
            yield_42.await; // yield 42 -- wait that's not a Box!?
        };
        for await (n, i) in s.enumerate() {
            println!("Item at index {n}:\n    {i}");
            // Item at index 0:
            //     12345
            // Item at index 1:
            // Segmentation fault
        }
    })
    .next()
    .await;
}
