/// This structure calls a function/closure when it is dropped.
/// The [`DropGuard::disarm`] method stops this from happening.
pub(crate) struct DropGuard<F: Fn()>(pub(crate) F);

impl<F: Fn()> DropGuard<F> {
    pub(crate) fn disarm(self) {
        std::mem::forget(self)
    }
}

impl<F> Drop for DropGuard<F>
where
    F: Fn(),
{
    fn drop(&mut self) {
        (self.0)()
    }
}

#[test]
fn test_dropguard_drop() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let count = AtomicUsize::new(0);
    assert_eq!(count.load(Ordering::Relaxed), 0);
    {
        let _ = count.fetch_add(1, Ordering::Relaxed);
        let _ = DropGuard(|| {
            let _ = count.fetch_sub(1, Ordering::Relaxed);
        });
    }
    assert_eq!(count.load(Ordering::Relaxed), 0);
}

#[test]
fn test_dropguard_disarm() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let count = AtomicUsize::new(0);
    assert_eq!(count.load(Ordering::Relaxed), 0);
    {
        let _ = count.fetch_add(1, Ordering::Relaxed);
        let guard = DropGuard(|| {
            let _ = count.fetch_sub(1, Ordering::Relaxed);
        });
        guard.disarm();
    }
    assert_eq!(count.load(Ordering::Relaxed), 1);
}
