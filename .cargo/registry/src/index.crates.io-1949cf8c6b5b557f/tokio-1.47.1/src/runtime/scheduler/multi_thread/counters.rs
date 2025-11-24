#[cfg(tokio_internal_mt_counters)]
mod imp {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering::Relaxed;

    static NUM_MAINTENANCE: AtomicUsize = AtomicUsize::new(0);
    static NUM_NOTIFY_LOCAL: AtomicUsize = AtomicUsize::new(0);
    static NUM_UNPARKS_LOCAL: AtomicUsize = AtomicUsize::new(0);
    static NUM_LIFO_SCHEDULES: AtomicUsize = AtomicUsize::new(0);
    static NUM_LIFO_CAPPED: AtomicUsize = AtomicUsize::new(0);

    impl Drop for super::Counters {
        fn drop(&mut self) {
            let notifies_local = NUM_NOTIFY_LOCAL.load(Relaxed);
            let unparks_local = NUM_UNPARKS_LOCAL.load(Relaxed);
            let maintenance = NUM_MAINTENANCE.load(Relaxed);
            let lifo_scheds = NUM_LIFO_SCHEDULES.load(Relaxed);
            let lifo_capped = NUM_LIFO_CAPPED.load(Relaxed);

            println!("---");
            println!("notifies (local): {}", notifies_local);
            println!(" unparks (local): {}", unparks_local);
            println!("     maintenance: {}", maintenance);
            println!("  LIFO schedules: {}", lifo_scheds);
            println!("     LIFO capped: {}", lifo_capped);
        }
    }

    pub(crate) fn inc_num_inc_notify_local() {
        NUM_NOTIFY_LOCAL.fetch_add(1, Relaxed);
    }

    pub(crate) fn inc_num_unparks_local() {
        NUM_UNPARKS_LOCAL.fetch_add(1, Relaxed);
    }

    pub(crate) fn inc_num_maintenance() {
        NUM_MAINTENANCE.fetch_add(1, Relaxed);
    }

    pub(crate) fn inc_lifo_schedules() {
        NUM_LIFO_SCHEDULES.fetch_add(1, Relaxed);
    }

    pub(crate) fn inc_lifo_capped() {
        NUM_LIFO_CAPPED.fetch_add(1, Relaxed);
    }
}

#[cfg(not(tokio_internal_mt_counters))]
mod imp {
    pub(crate) fn inc_num_inc_notify_local() {}
    pub(crate) fn inc_num_unparks_local() {}
    pub(crate) fn inc_num_maintenance() {}
    pub(crate) fn inc_lifo_schedules() {}
    pub(crate) fn inc_lifo_capped() {}
}

#[derive(Debug)]
pub(crate) struct Counters;

pub(super) use imp::*;
