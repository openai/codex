//! This file contains mocks of the metrics types used in the I/O driver.
//!
//! The reason these mocks don't live in `src/runtime/mock.rs` is because
//! these need to be available in the case when `net` is enabled but
//! `rt` is not.

cfg_not_rt_and_metrics_and_net! {
    #[derive(Default)]
    pub(crate) struct IoDriverMetrics {}

    impl IoDriverMetrics {
        pub(crate) fn incr_fd_count(&self) {}
        pub(crate) fn dec_fd_count(&self) {}
        pub(crate) fn incr_ready_count_by(&self, _amt: u64) {}
    }
}

cfg_net! {
    cfg_rt! {
        cfg_unstable_metrics! {
            pub(crate) use crate::runtime::IoDriverMetrics;
        }
    }
}
