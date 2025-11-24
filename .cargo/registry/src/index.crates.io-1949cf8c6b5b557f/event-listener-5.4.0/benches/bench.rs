use std::iter;

use criterion::{criterion_group, criterion_main, Criterion};
use event_listener::{Event, Listener};

const COUNT: usize = 8000;

fn bench_events(c: &mut Criterion) {
    c.bench_function("notify_and_wait", |b| {
        let ev = Event::new();
        let mut handles = Vec::with_capacity(COUNT);

        b.iter(|| {
            handles.extend(iter::repeat_with(|| ev.listen()).take(COUNT));

            ev.notify(COUNT);

            for handle in handles.drain(..) {
                handle.wait();
            }
        });
    });
}

criterion_group!(benches, bench_events);
criterion_main!(benches);
