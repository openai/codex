use async_stream::stream;

fn main() {
    stream! {
        Ok("value")
            .and_then(|v| {
                yield v;
                Ok(())
            });
    };
}
