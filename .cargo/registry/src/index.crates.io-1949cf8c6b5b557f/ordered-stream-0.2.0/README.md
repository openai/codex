[![](https://docs.rs/ordered-stream/badge.svg)](https://docs.rs/ordered-stream/) [![](https://img.shields.io/crates/v/ordered-stream)](https://crates.io/crates/ordered-stream)

# Streams that produce elements with an associated ordering

 Say you have a bunch of events that all have a timestamp, sequence number, or other ordering
 attribute.  If you get these events from multiple [`Stream`]s, then you should be able to produce
 a "composite" stream by joining each of the individual streams, so long as each originating stream
 is ordered.

 However, if you actually implement this, you discover that you need to buffer at least one
 element from each stream in order to avoid ordering inversions if the sources are independent
 (including just running in different tasks).  This presents a problem if one of the sources
 rarely produces events: that slow source can stall all other streams in order to handle the
 case where the slowness is due to an earlier element instead of just having no elements.

 The [`OrderedStream`] trait provides a way to solve this problem: if you can ask a stream if it
 will ever have any events that should be delivered before a given event, then you can often
 avoid blocking the composite stream when data is ready.

 ```rust
 use futures_core::Stream;
 use ordered_stream::FromStream;
 use ordered_stream::JoinMultiple;
 use ordered_stream::OrderedStream;
 use ordered_stream::OrderedStreamExt;
 use std::pin::Pin;
 use std::time::SystemTime;

 pub struct Message {
     time: SystemTime,
     level: u8,
     data: String,
     source: String,
 }

 pub struct RemoteLogSource {
     stream: Pin<Box<dyn Stream<Item = Message>>>,
     min_level: u8,
 }

 pub async fn display_logs(logs: &mut [RemoteLogSource]) {
     let mut streams: Vec<_> = logs
         .iter_mut()
         .map(|s| {
             let min = s.min_level;
             FromStream::with_ordering(&mut s.stream, |m| m.time)
                 .filter(move |m| m.level >= min)
                 .peekable()
         })
         .collect();
     let mut joined = JoinMultiple(streams);
     while let Some(msg) = joined.next().await {
         println!("{:?}: {}", msg.time, msg.data);
     }
 }
 ```

[`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
