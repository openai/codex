#![cfg_attr(not(check_cfg), allow(unexpected_cfgs))]
#![allow(clippy::cast_possible_truncation)]

use std::str;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

#[test]
#[cfg_attr(not(exhaustive), ignore = "requires cfg(exhaustive)")]
fn test_exhaustive() {
    const BATCH_SIZE: u32 = 1_000_000;
    let counter = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));

    let mut workers = Vec::new();
    for _ in 0..num_cpus::get() {
        let counter = counter.clone();
        let finished = finished.clone();
        workers.push(thread::spawn(move || loop {
            let batch = counter.fetch_add(1, Ordering::Relaxed) as u32;
            if batch > u32::max_value() / BATCH_SIZE {
                return;
            }

            let min = batch * BATCH_SIZE;
            let max = if batch == u32::max_value() / BATCH_SIZE {
                u32::max_value()
            } else {
                min + BATCH_SIZE - 1
            };

            let mut bytes = [0u8; 24];
            let mut buffer = ryu::Buffer::new();
            for u in min..=max {
                let f = f32::from_bits(u);
                if !f.is_finite() {
                    continue;
                }
                let n = unsafe { ryu::raw::format32(f, &mut bytes[0]) };
                assert_eq!(Ok(Ok(f)), str::from_utf8(&bytes[..n]).map(str::parse));
                assert_eq!(Ok(f), buffer.format_finite(f).parse());
            }

            let increment = (max - min + 1) as usize;
            let update = finished.fetch_add(increment, Ordering::Relaxed);
            println!("{}", update + increment);
        }));
    }

    for w in workers {
        w.join().unwrap();
    }
}
