lazy_static::lazy_static! {
    /// Cache the physical CPU count to avoid calling `num_cpus::get()`
    /// multiple times, which is expensive when creating pools in quick
    /// succession.
    static ref CPU_COUNT: usize = num_cpus::get();
}

/// Get the default maximum size of a pool, which is `cpu_core_count * 2`
/// including logical cores (Hyper-Threading).
pub(crate) fn get_default_pool_max_size() -> usize {
    *CPU_COUNT * 2
}
