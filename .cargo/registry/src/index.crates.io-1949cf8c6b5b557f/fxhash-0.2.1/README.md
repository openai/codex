# Fx Hash

This hashing algorithm was extracted from the Rustc compiler.  This is the same hashing algoirthm used for some internal operations in FireFox.  The strength of this algorithm is in hashing 8 bytes at a time on 64-bit platforms, where the FNV algorithm works on one byte at a time.

## Disclaimer

It is **not a cryptographically secure** hash, so it is strongly recommended that you do not use this hash for cryptographic purproses.  Furthermore, this hashing algorithm was not designed to prevent any attacks for determining collisions which could be used to potentially cause quadratic behavior in `HashMap`s.  So it is not recommended to expose this hash in places where collissions or DDOS attacks may be a concern.

## Examples

Building an Fx backed hashmap.

```rust
extern crate fxhash;
use fxhash::FxHashMap;

let mut hashmap = FxHashMap::new();

hashmap.insert("black", 0);
hashmap.insert("white", 255);
```

Building an Fx backed hashset.

```rust
extern crate fxhash;
use fxhash::FxHashSet;

let mut hashmap = FxHashSet::new();

hashmap.insert("black");
hashmap.insert("white");
```

## Benchmarks

Generally `fxhash` is than `fnv` on `u32`, `u64`, or any byte sequence with length >= 5.  However, keep in mind that hashing speed is not the only characteristic worth considering.  That being said, Rustc had an observable increase in speed when switching from `fnv` backed hashmaps to `fx` based hashmaps.

    bench_fnv_003     ... bench:      3 ns/iter (+/- 0)
    bench_fnv_004     ... bench:      2 ns/iter (+/- 0)
    bench_fnv_011     ... bench:      6 ns/iter (+/- 1)
    bench_fnv_012     ... bench:      5 ns/iter (+/- 1)
    bench_fnv_023     ... bench:     14 ns/iter (+/- 3)
    bench_fnv_024     ... bench:     14 ns/iter (+/- 4)
    bench_fnv_068     ... bench:     57 ns/iter (+/- 11)
    bench_fnv_132     ... bench:    145 ns/iter (+/- 30)
    bench_fx_003      ... bench:      4 ns/iter (+/- 0)
    bench_fx_004      ... bench:      3 ns/iter (+/- 1)
    bench_fx_011      ... bench:      5 ns/iter (+/- 2)
    bench_fx_012      ... bench:      4 ns/iter (+/- 1)
    bench_fx_023      ... bench:      7 ns/iter (+/- 3)
    bench_fx_024      ... bench:      4 ns/iter (+/- 1)
    bench_fx_068      ... bench:     10 ns/iter (+/- 3)
    bench_fx_132      ... bench:     19 ns/iter (+/- 5)
    bench_seahash_003 ... bench:     30 ns/iter (+/- 12)
    bench_seahash_004 ... bench:     32 ns/iter (+/- 22)
    bench_seahash_011 ... bench:     30 ns/iter (+/- 4)
    bench_seahash_012 ... bench:     31 ns/iter (+/- 1)
    bench_seahash_023 ... bench:     32 ns/iter (+/- 6)
    bench_seahash_024 ... bench:     31 ns/iter (+/- 5)
    bench_seahash_068 ... bench:     40 ns/iter (+/- 9)
    bench_seahash_132 ... bench:     50 ns/iter (+/- 12)