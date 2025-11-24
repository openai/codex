cfg_not_loom! {
    mod atomic_waker;
    mod notify;
    mod semaphore_batch;
}

cfg_loom! {
    mod loom_atomic_waker;
    mod loom_broadcast;
    mod loom_list;
    mod loom_mpsc;
    mod loom_notify;
    mod loom_oneshot;
    mod loom_semaphore_batch;
    mod loom_watch;
    mod loom_rwlock;
    mod loom_set_once;
}
