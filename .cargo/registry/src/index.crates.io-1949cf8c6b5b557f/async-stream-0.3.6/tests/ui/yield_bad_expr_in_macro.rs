use async_stream::stream;

fn main() {
    async fn work() {}

    stream! {
        tokio::select! {
            _ = work() => yield fn f() {},
        }
    };
}
