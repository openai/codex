//! Miscellaneous utilities.

use std::cell::Cell;
use std::num::Wrapping;
use std::thread;
use std::time::{Duration, Instant};

/// Randomly shuffles a slice.
pub(crate) fn shuffle<T>(v: &mut [T]) {
    let len = v.len();
    if len <= 1 {
        return;
    }

    std::thread_local! {
        static RNG: Cell<Wrapping<u32>> = const { Cell::new(Wrapping(1_406_868_647)) };
    }

    let _ = RNG.try_with(|rng| {
        for i in 1..len {
            // This is the 32-bit variant of Xorshift.
            //
            // Source: https://en.wikipedia.org/wiki/Xorshift
            let mut x = rng.get();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            rng.set(x);

            let x = x.0;
            let n = i + 1;

            // This is a fast alternative to `let j = x % n`.
            //
            // Author: Daniel Lemire
            // Source: https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
            let j = ((x as u64).wrapping_mul(n as u64) >> 32) as u32 as usize;

            v.swap(i, j);
        }
    });
}

/// Sleeps until the deadline, or forever if the deadline isn't specified.
pub(crate) fn sleep_until(deadline: Option<Instant>) {
    loop {
        match deadline {
            None => thread::sleep(Duration::from_secs(1000)),
            Some(d) => {
                let now = Instant::now();
                if now >= d {
                    break;
                }
                thread::sleep(d - now);
            }
        }
    }
}
