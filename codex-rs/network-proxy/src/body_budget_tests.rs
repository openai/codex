//! Reservations survive request metadata disposal and are reused across transform stages.

use super::*;
use pretty_assertions::assert_eq;
use rama_core::futures::stream;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn fragmented_body_collection_releases_chunks_and_enforces_the_byte_limit() {
    struct Chunk {
        bytes: [u8; 1],
        live: Arc<AtomicUsize>,
    }

    impl AsRef<[u8]> for Chunk {
        fn as_ref(&self) -> &[u8] {
            &self.bytes
        }
    }

    impl Drop for Chunk {
        fn drop(&mut self) {
            self.live.fetch_sub(1, Ordering::SeqCst);
        }
    }

    for max_bytes in [1024, 1023] {
        let live = Arc::new(AtomicUsize::new(0));
        let chunks = (0..1024).map({
            let live = live.clone();
            move |_| {
                assert_eq!(
                    live.fetch_add(1, Ordering::SeqCst),
                    0,
                    "the collector retained a previous chunk"
                );
                Ok::<_, std::io::Error>(Bytes::from_owner(Chunk {
                    bytes: *b"x",
                    live: live.clone(),
                }))
            }
        });
        let result = collect_body(Body::from_stream(stream::iter(chunks)), max_bytes).await;
        assert_eq!(live.load(Ordering::SeqCst), 0);
        if max_bytes == 1024 {
            assert_eq!(result.unwrap(), (Bytes::from(vec![b'x'; 1024]), None));
        } else {
            assert!(result.is_err());
        }
    }
}

#[tokio::test]
async fn body_keeps_buffer_lease_and_nested_stages_reuse_it() {
    let mut request = Request::new(Body::from("payload"));
    let slots = Arc::new(Semaphore::new(BUFFER_SLOT_COUNT));
    let permit = reserve_in(&mut request, &slots).await.unwrap();
    let lease = Arc::downgrade(&permit);
    let nested = reserve_in(&mut request, &slots).await.unwrap();
    assert!(Arc::ptr_eq(&permit, &nested));
    let request = retain(request);
    drop(nested);
    drop(permit);
    let metadata = request.extensions().clone();
    let (parts, mut body) = request.into_parts();
    assert_eq!(body.size_hint().exact(), Some(7));
    drop(parts);
    assert!(lease.upgrade().is_some());
    let frame = body.frame().await.expect("body frame").expect("body data");
    drop(body);
    assert!(lease.upgrade().is_some());
    let bytes = frame.into_data().expect("data frame");
    assert_eq!(bytes.as_ref(), b"payload");
    let queued = bytes.clone();
    drop(bytes);
    assert!(lease.upgrade().is_some());
    let mut held = Vec::new();
    for _ in 1..8 {
        let permit = slots
            .clone()
            .try_acquire_owned()
            .expect("eight buffer slots");
        held.push(permit);
    }
    let mut ninth = Request::new(Body::empty());
    let reservation = reserve_in(&mut ninth, &slots);
    tokio::pin!(reservation);
    assert!(rama_core::futures::poll!(&mut reservation).is_pending());
    drop(queued);
    assert!(lease.upgrade().is_none());
    let _permit = reservation.await.expect("released buffer slot");
    drop(metadata);
}
