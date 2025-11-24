use async_stream::stream;

fn main() {
    stream! {
        let f = async {
            yield 123;
        };

        let v = f.await;
    };
}
