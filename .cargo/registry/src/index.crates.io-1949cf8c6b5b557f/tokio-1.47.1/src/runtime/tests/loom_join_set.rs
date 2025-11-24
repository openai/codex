use crate::runtime::Builder;
use crate::task::JoinSet;

#[test]
fn test_join_set() {
    loom::model(|| {
        let rt = Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();
        let mut set = JoinSet::new();

        rt.block_on(async {
            assert_eq!(set.len(), 0);
            set.spawn(async { () });
            assert_eq!(set.len(), 1);
            set.spawn(async { () });
            assert_eq!(set.len(), 2);
            let () = set.join_next().await.unwrap().unwrap();
            assert_eq!(set.len(), 1);
            set.spawn(async { () });
            assert_eq!(set.len(), 2);
            let () = set.join_next().await.unwrap().unwrap();
            assert_eq!(set.len(), 1);
            let () = set.join_next().await.unwrap().unwrap();
            assert_eq!(set.len(), 0);
            set.spawn(async { () });
            assert_eq!(set.len(), 1);
        });

        drop(set);
        drop(rt);
    });
}

#[test]
fn abort_all_during_completion() {
    use std::sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    };

    // These booleans assert that at least one execution had the task complete first, and that at
    // least one execution had the task be cancelled before it completed.
    let complete_happened = Arc::new(AtomicBool::new(false));
    let cancel_happened = Arc::new(AtomicBool::new(false));

    {
        let complete_happened = complete_happened.clone();
        let cancel_happened = cancel_happened.clone();
        loom::model(move || {
            let rt = Builder::new_multi_thread()
                .worker_threads(1)
                .build()
                .unwrap();

            let mut set = JoinSet::new();

            rt.block_on(async {
                set.spawn(async { () });
                set.abort_all();

                match set.join_next().await {
                    Some(Ok(())) => complete_happened.store(true, SeqCst),
                    Some(Err(err)) if err.is_cancelled() => cancel_happened.store(true, SeqCst),
                    Some(Err(err)) => panic!("fail: {}", err),
                    None => {
                        unreachable!("Aborting the task does not remove it from the JoinSet.")
                    }
                }

                assert!(matches!(set.join_next().await, None));
            });

            drop(set);
            drop(rt);
        });
    }

    assert!(complete_happened.load(SeqCst));
    assert!(cancel_happened.load(SeqCst));
}
