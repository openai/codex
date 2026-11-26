//! Logical size of a fresh, single-writer shadow board. Count each stored copy of
//! serialized evidence, excluding SQLite metadata, indexes and search text.
use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use codex_extension_api::ExtensionMetrics;

const DEFAULT_BOARD_LIMIT_MIB: u32 = 32;

pub(super) struct ShadowMetrics(pub(super) Arc<dyn ExtensionMetrics>);

impl ShadowMetrics {
    pub(super) fn stopped(&self, stage: &'static str) {
        self.0.counter(
            "codex.guardian_v3.shadow.worker_failure",
            /*inc*/ 1,
            &[("stage", stage)],
        );
    }
}

#[derive(Debug)]
pub(super) struct PayloadLimit;

impl fmt::Display for PayloadLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Guardian shadow board payload limit exceeded")
    }
}

impl std::error::Error for PayloadLimit {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BoardSize {
    channels: usize,
    posts: usize,
    payload_bytes: u64,
    limit_bytes: u64,
}

impl BoardSize {
    pub(super) fn new(limit_mib: Option<NonZeroU32>) -> Self {
        Self {
            channels: 0,
            posts: 0,
            payload_bytes: 0,
            limit_bytes: u64::from(limit_mib.map_or(DEFAULT_BOARD_LIMIT_MIB, NonZeroU32::get))
                * 1024
                * 1024,
        }
    }

    pub(super) fn check_capacity(&self, bytes: u64) -> Result<(), PayloadLimit> {
        if bytes > self.limit_bytes - self.payload_bytes {
            return Err(PayloadLimit);
        }
        Ok(())
    }

    // Preflight the entire validated batch before creating channels or appending posts.
    // The sampler can select fewer sources than were serialized.
    pub(super) fn with_batch(
        mut self,
        channels: usize,
        routes: &[Vec<String>],
        texts: &[String],
    ) -> Result<Self, PayloadLimit> {
        self.channels = channels;
        for (destinations, text) in routes.iter().zip(texts) {
            let copies = destinations.iter().collect::<BTreeSet<_>>().len();
            let bytes = (text.len() as u64).saturating_mul(copies as u64);
            self.check_capacity(bytes)?;
            self.payload_bytes += bytes;
            self.posts += copies;
        }
        Ok(self)
    }

    pub(super) fn record(&self, metrics: &ShadowMetrics) {
        let counts = &[
            1., 2., 5., 10., 20., 50., 100., 200., 500., 1000., 5000., 10000.,
        ];
        metrics.0.histogram_with_boundaries(
            "codex.guardian_v3.shadow.board.channels",
            self.channels as i64,
            counts,
            &[],
        );
        metrics.0.histogram_with_boundaries(
            "codex.guardian_v3.shadow.board.posts",
            self.posts as i64,
            counts,
            &[],
        );
        metrics.0.histogram_with_boundaries(
            "codex.guardian_v3.shadow.board.payload_bytes",
            self.payload_bytes as i64,
            &[
                1024., 4096., 16384., 65536., 262144., 1048576., 4194304., 8388608., 16777216.,
                33554432.,
            ],
            &[],
        );
    }
}

#[cfg(test)]
#[path = "shadow_metrics_tests.rs"]
mod tests;
