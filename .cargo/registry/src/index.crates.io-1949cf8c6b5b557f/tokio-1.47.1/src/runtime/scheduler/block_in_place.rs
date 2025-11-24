use crate::runtime::scheduler;

#[track_caller]
pub(crate) fn block_in_place<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    scheduler::multi_thread::block_in_place(f)
}
