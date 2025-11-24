// Reproduces https://github.com/hawkw/sharded-slab/issues/83
use memory_stats::memory_stats;
use sharded_slab::Config;
use sharded_slab::Slab;

struct CustomConfig;
impl Config for CustomConfig {
    const RESERVED_BITS: usize = 1; // This is the cause.
}

#[test]
fn reserved_bits_doesnt_leak() {
    let slab = Slab::new_with_config::<CustomConfig>();
    for n in 0..1000 {
        let mem_before = memory_stats().unwrap();
        let key = slab.insert(0).unwrap();
        slab.remove(key);
        let usage = memory_stats().unwrap();
        eprintln!(
            "n: {n:<4}\tkey: {key:#08x} rss: {:>16} vs:{:>16}",
            usage.physical_mem, usage.virtual_mem
        );

        assert_eq!(mem_before.virtual_mem, usage.virtual_mem);
    }
}
