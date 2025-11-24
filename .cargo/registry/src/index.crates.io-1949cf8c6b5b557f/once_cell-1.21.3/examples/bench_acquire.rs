//! Benchmark the overhead that the synchronization of `OnceCell::get` causes.
//! We do some other operations that write to memory to get an imprecise but somewhat realistic
//! measurement.

use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicUsize, Ordering};

const N_THREADS: usize = 16;
const N_ROUNDS: usize = 1_000_000;

static CELL: OnceCell<usize> = OnceCell::new();
static OTHER: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let start = std::time::Instant::now();
    let threads =
        (0..N_THREADS).map(|i| std::thread::spawn(move || thread_main(i))).collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    println!("{:?}", start.elapsed());
    println!("{:?}", OTHER.load(Ordering::Relaxed));
}

#[inline(never)]
fn thread_main(i: usize) {
    // The operations we do here don't really matter, as long as we do multiple writes, and
    // everything is messy enough to prevent the compiler from optimizing the loop away.
    let mut data = [i; 128];
    let mut accum = 0usize;
    for _ in 0..N_ROUNDS {
        let _value = CELL.get_or_init(|| i + 1);
        let k = OTHER.fetch_add(data[accum & 0x7F] as usize, Ordering::Relaxed);
        for j in data.iter_mut() {
            *j = (*j).wrapping_add(accum);
            accum = accum.wrapping_add(k);
        }
    }
}
