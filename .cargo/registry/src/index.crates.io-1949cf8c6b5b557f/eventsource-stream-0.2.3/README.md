# eventsource-stream

A basic building block for building an Eventsource from a Stream of bytes array like objects. To
learn more about Server Sent Events (SSE) take a look at [the MDN
docs](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)

## Example

```rust
let mut stream = reqwest::Client::new()
    .get("http://localhost:7020/notifications")
    .send()
    .await?
    .bytes_stream()
    .eventsource();

while let Some(thing) = stream.next().await {
   println!("{:?}", thing);
}
```

License: MIT OR Apache-2.0
