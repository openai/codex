//! Tests copied from Go and manually rewritten in Rust.
//!
//! Source:
//!   - https://github.com/golang/go
//!
//! Copyright & License:
//!   - Copyright (c) 2009 The Go Authors
//!   - https://golang.org/AUTHORS
//!   - https://golang.org/LICENSE
//!   - https://golang.org/PATENTS

#![allow(clippy::redundant_clone)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, never, select, tick, unbounded, Receiver, Select, Sender};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

struct Chan<T> {
    inner: Arc<Mutex<ChanInner<T>>>,
}

struct ChanInner<T> {
    s: Option<Sender<T>>,
    r: Option<Receiver<T>>,
    // Receiver to use when r is None (Go blocks on receiving from nil)
    nil_r: Receiver<T>,
    // Sender to use when s is None (Go blocks on sending to nil)
    nil_s: Sender<T>,
    // Hold this receiver to prevent nil sender channel from disconnection
    _nil_sr: Receiver<T>,
}

impl<T> Clone for Chan<T> {
    fn clone(&self) -> Chan<T> {
        Chan {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Chan<T> {
    fn send(&self, msg: T) {
        let s = self
            .inner
            .lock()
            .unwrap()
            .s
            .as_ref()
            .expect("sending into closed channel")
            .clone();
        let _ = s.send(msg);
    }

    fn try_recv(&self) -> Option<T> {
        let r = self.inner.lock().unwrap().r.as_ref().unwrap().clone();
        r.try_recv().ok()
    }

    fn recv(&self) -> Option<T> {
        let r = self.inner.lock().unwrap().r.as_ref().unwrap().clone();
        r.recv().ok()
    }

    fn close_s(&self) {
        self.inner
            .lock()
            .unwrap()
            .s
            .take()
            .expect("channel sender already closed");
    }

    fn close_r(&self) {
        self.inner
            .lock()
            .unwrap()
            .r
            .take()
            .expect("channel receiver already closed");
    }

    fn has_rx(&self) -> bool {
        self.inner.lock().unwrap().r.is_some()
    }

    fn has_tx(&self) -> bool {
        self.inner.lock().unwrap().s.is_some()
    }

    fn rx(&self) -> Receiver<T> {
        let inner = self.inner.lock().unwrap();
        match inner.r.as_ref() {
            None => inner.nil_r.clone(),
            Some(r) => r.clone(),
        }
    }

    fn tx(&self) -> Sender<T> {
        let inner = self.inner.lock().unwrap();
        match inner.s.as_ref() {
            None => inner.nil_s.clone(),
            Some(s) => s.clone(),
        }
    }
}

impl<T> Iterator for Chan<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv()
    }
}

impl<'a, T> IntoIterator for &'a Chan<T> {
    type Item = T;
    type IntoIter = Chan<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.clone()
    }
}

fn make<T>(cap: usize) -> Chan<T> {
    let (s, r) = bounded(cap);
    let (nil_s, _nil_sr) = bounded(0);
    Chan {
        inner: Arc::new(Mutex::new(ChanInner {
            s: Some(s),
            r: Some(r),
            nil_r: never(),
            nil_s,
            _nil_sr,
        })),
    }
}

fn make_unbounded<T>() -> Chan<T> {
    let (s, r) = unbounded();
    let (nil_s, _nil_sr) = bounded(0);
    Chan {
        inner: Arc::new(Mutex::new(ChanInner {
            s: Some(s),
            r: Some(r),
            nil_r: never(),
            nil_s,
            _nil_sr,
        })),
    }
}

#[derive(Clone)]
struct WaitGroup(Arc<WaitGroupInner>);

struct WaitGroupInner {
    cond: Condvar,
    count: Mutex<i32>,
}

impl WaitGroup {
    fn new() -> WaitGroup {
        WaitGroup(Arc::new(WaitGroupInner {
            cond: Condvar::new(),
            count: Mutex::new(0),
        }))
    }

    fn add(&self, delta: i32) {
        let mut count = self.0.count.lock().unwrap();
        *count += delta;
        assert!(*count >= 0);
        self.0.cond.notify_all();
    }

    fn done(&self) {
        self.add(-1);
    }

    fn wait(&self) {
        let mut count = self.0.count.lock().unwrap();
        while *count > 0 {
            count = self.0.cond.wait(count).unwrap();
        }
    }
}

struct Defer<F: FnOnce()> {
    f: Option<Box<F>>,
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        let f = self.f.take().unwrap();
        let mut f = Some(f);
        let mut f = move || f.take().unwrap()();
        f();
    }
}

struct Counter;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            ALLOCATED.fetch_add(layout.size(), SeqCst);
        }
        ret
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        ALLOCATED.fetch_sub(layout.size(), SeqCst);
    }
}

#[global_allocator]
static A: Counter = Counter;

macro_rules! defer {
    ($body:expr) => {
        let _defer = Defer {
            f: Some(Box::new(|| $body)),
        };
    };
}

macro_rules! go {
    (@parse $v:ident, $($tail:tt)*) => {{
        let $v = $v.clone();
        go!(@parse $($tail)*)
    }};
    (@parse $body:expr) => {
        ::std::thread::spawn(move || {
            let res = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                $body
            }));
            if res.is_err() {
                eprintln!("goroutine panicked: {:?}", res);
                ::std::process::abort();
            }
        })
    };
    (@parse $($tail:tt)*) => {
        compile_error!("invalid `go!` syntax")
    };
    ($($tail:tt)*) => {{
        go!(@parse $($tail)*)
    }};
}

// https://github.com/golang/go/blob/master/test/chan/doubleselect.go
mod doubleselect {
    use super::*;

    #[cfg(miri)]
    const ITERATIONS: i32 = 100;
    #[cfg(not(miri))]
    const ITERATIONS: i32 = 10_000;

    fn sender(n: i32, c1: Chan<i32>, c2: Chan<i32>, c3: Chan<i32>, c4: Chan<i32>) {
        defer! { c1.close_s() }
        defer! { c2.close_s() }
        defer! { c3.close_s() }
        defer! { c4.close_s() }

        for i in 0..n {
            select! {
                send(c1.tx(), i) -> _ => {}
                send(c2.tx(), i) -> _ => {}
                send(c3.tx(), i) -> _ => {}
                send(c4.tx(), i) -> _ => {}
            }
        }
    }

    fn mux(out: Chan<i32>, inp: Chan<i32>, done: Chan<bool>) {
        for v in inp {
            out.send(v);
        }
        done.send(true);
    }

    fn recver(inp: Chan<i32>) {
        let mut seen = HashMap::new();

        for v in &inp {
            if seen.contains_key(&v) {
                panic!("got duplicate value for {}", v);
            }
            seen.insert(v, true);
        }
    }

    #[test]
    fn main() {
        let c1 = make::<i32>(0);
        let c2 = make::<i32>(0);
        let c3 = make::<i32>(0);
        let c4 = make::<i32>(0);
        let done = make::<bool>(0);
        let cmux = make::<i32>(0);

        go!(c1, c2, c3, c4, sender(ITERATIONS, c1, c2, c3, c4));
        go!(cmux, c1, done, mux(cmux, c1, done));
        go!(cmux, c2, done, mux(cmux, c2, done));
        go!(cmux, c3, done, mux(cmux, c3, done));
        go!(cmux, c4, done, mux(cmux, c4, done));
        go!(done, cmux, {
            done.recv();
            done.recv();
            done.recv();
            done.recv();
            cmux.close_s();
        });
        recver(cmux);
    }
}

// https://github.com/golang/go/blob/master/test/chan/fifo.go
mod fifo {
    use super::*;

    const N: i32 = 10;

    #[test]
    fn asynch_fifo() {
        let ch = make::<i32>(N as usize);
        for i in 0..N {
            ch.send(i);
        }
        for i in 0..N {
            if ch.recv() != Some(i) {
                panic!("bad receive");
            }
        }
    }

    fn chain(ch: Chan<i32>, val: i32, inp: Chan<i32>, out: Chan<i32>) {
        inp.recv();
        if ch.recv() != Some(val) {
            panic!("{}", val);
        }
        out.send(1);
    }

    #[test]
    fn synch_fifo() {
        let ch = make::<i32>(0);
        let mut inp = make::<i32>(0);
        let start = inp.clone();

        for i in 0..N {
            let out = make::<i32>(0);
            go!(ch, i, inp, out, chain(ch, i, inp, out));
            inp = out;
        }

        start.send(0);
        for i in 0..N {
            ch.send(i);
        }
        inp.recv();
    }
}

// https://github.com/golang/go/blob/master/test/chan/goroutines.go
mod goroutines {
    use super::*;

    fn f(left: Chan<i32>, right: Chan<i32>) {
        left.send(right.recv().unwrap());
    }

    #[test]
    fn main() {
        let n = 100i32;

        let leftmost = make::<i32>(0);
        let mut right = leftmost.clone();
        let mut left = leftmost.clone();

        for _ in 0..n {
            right = make::<i32>(0);
            go!(left, right, f(left, right));
            left = right.clone();
        }

        go!(right, right.send(1));
        leftmost.recv().unwrap();
    }
}

// https://github.com/golang/go/blob/master/test/chan/nonblock.go
mod nonblock {
    use super::*;

    fn i32receiver(c: Chan<i32>, strobe: Chan<bool>) {
        if c.recv().unwrap() != 123 {
            panic!("i32 value");
        }
        strobe.send(true);
    }

    fn i32sender(c: Chan<i32>, strobe: Chan<bool>) {
        c.send(234);
        strobe.send(true);
    }

    fn i64receiver(c: Chan<i64>, strobe: Chan<bool>) {
        if c.recv().unwrap() != 123456 {
            panic!("i64 value");
        }
        strobe.send(true);
    }

    fn i64sender(c: Chan<i64>, strobe: Chan<bool>) {
        c.send(234567);
        strobe.send(true);
    }

    fn breceiver(c: Chan<bool>, strobe: Chan<bool>) {
        if !c.recv().unwrap() {
            panic!("b value");
        }
        strobe.send(true);
    }

    fn bsender(c: Chan<bool>, strobe: Chan<bool>) {
        c.send(true);
        strobe.send(true);
    }

    fn sreceiver(c: Chan<String>, strobe: Chan<bool>) {
        if c.recv().unwrap() != "hello" {
            panic!("x value");
        }
        strobe.send(true);
    }

    fn ssender(c: Chan<String>, strobe: Chan<bool>) {
        c.send("hello again".to_string());
        strobe.send(true);
    }

    const MAX_TRIES: usize = 10000; // Up to 100ms per test.

    #[test]
    fn main() {
        let ticker = tick(Duration::new(0, 10_000)); // 10 us
        let sleep = || {
            ticker.recv().unwrap();
            ticker.recv().unwrap();
            thread::yield_now();
            thread::yield_now();
            thread::yield_now();
        };

        let sync = make::<bool>(0);

        for buffer in 0..2 {
            let c32 = make::<i32>(buffer);
            let c64 = make::<i64>(buffer);
            let cb = make::<bool>(buffer);
            let cs = make::<String>(buffer);

            select! {
                recv(c32.rx()) -> _ => panic!("blocked i32sender"),
                default => {}
            }

            select! {
                recv(c64.rx()) -> _ => panic!("blocked i64sender"),
                default => {}
            }

            select! {
                recv(cb.rx()) -> _ => panic!("blocked bsender"),
                default => {}
            }

            select! {
                recv(cs.rx()) -> _ => panic!("blocked ssender"),
                default => {}
            }

            go!(c32, sync, i32receiver(c32, sync));
            let mut r#try = 0;
            loop {
                select! {
                    send(c32.tx(), 123) -> _ => break,
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("i32receiver buffer={}", buffer);
                            panic!("fail")
                        }
                        sleep();
                    }
                }
            }
            sync.recv();
            go!(c32, sync, i32sender(c32, sync));
            if buffer > 0 {
                sync.recv();
            }
            let mut r#try = 0;
            loop {
                select! {
                    recv(c32.rx()) -> v => {
                        if v != Ok(234) {
                            panic!("i32sender value");
                        }
                        break;
                    }
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("i32sender buffer={}", buffer);
                            panic!("fail");
                        }
                        sleep();
                    }
                }
            }
            if buffer == 0 {
                sync.recv();
            }

            go!(c64, sync, i64receiver(c64, sync));
            let mut r#try = 0;
            loop {
                select! {
                    send(c64.tx(), 123456) -> _ => break,
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("i64receiver buffer={}", buffer);
                            panic!("fail")
                        }
                        sleep();
                    }
                }
            }
            sync.recv();
            go!(c64, sync, i64sender(c64, sync));
            if buffer > 0 {
                sync.recv();
            }
            let mut r#try = 0;
            loop {
                select! {
                    recv(c64.rx()) -> v => {
                        if v != Ok(234567) {
                            panic!("i64sender value");
                        }
                        break;
                    }
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("i64sender buffer={}", buffer);
                            panic!("fail");
                        }
                        sleep();
                    }
                }
            }
            if buffer == 0 {
                sync.recv();
            }

            go!(cb, sync, breceiver(cb, sync));
            let mut r#try = 0;
            loop {
                select! {
                    send(cb.tx(), true) -> _ => break,
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("breceiver buffer={}", buffer);
                            panic!("fail")
                        }
                        sleep();
                    }
                }
            }
            sync.recv();
            go!(cb, sync, bsender(cb, sync));
            if buffer > 0 {
                sync.recv();
            }
            let mut r#try = 0;
            loop {
                select! {
                    recv(cb.rx()) -> v => {
                        if v != Ok(true) {
                            panic!("bsender value");
                        }
                        break;
                    }
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("bsender buffer={}", buffer);
                            panic!("fail");
                        }
                        sleep();
                    }
                }
            }
            if buffer == 0 {
                sync.recv();
            }

            go!(cs, sync, sreceiver(cs, sync));
            let mut r#try = 0;
            loop {
                select! {
                    send(cs.tx(), "hello".to_string()) -> _ => break,
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("sreceiver buffer={}", buffer);
                            panic!("fail")
                        }
                        sleep();
                    }
                }
            }
            sync.recv();
            go!(cs, sync, ssender(cs, sync));
            if buffer > 0 {
                sync.recv();
            }
            let mut r#try = 0;
            loop {
                select! {
                    recv(cs.rx()) -> v => {
                        if v != Ok("hello again".to_string()) {
                            panic!("ssender value");
                        }
                        break;
                    }
                    default => {
                        r#try += 1;
                        if r#try > MAX_TRIES {
                            println!("ssender buffer={}", buffer);
                            panic!("fail");
                        }
                        sleep();
                    }
                }
            }
            if buffer == 0 {
                sync.recv();
            }
        }
    }
}

// https://github.com/golang/go/blob/master/test/chan/select.go
mod select {
    use super::*;

    #[test]
    fn main() {
        let shift = Cell::new(0);
        let counter = Cell::new(0);

        let get_value = || {
            counter.set(counter.get() + 1);
            1 << shift.get()
        };

        let send = |mut a: Option<&Chan<u32>>, mut b: Option<&Chan<u32>>| {
            let mut i = 0;
            let never = make::<u32>(0);
            loop {
                let nil1 = never.tx();
                let nil2 = never.tx();
                let v1 = get_value();
                let v2 = get_value();
                select! {
                    send(a.map(|c| c.tx()).unwrap_or(nil1), v1) -> _ => {
                        i += 1;
                        a = None;
                    }
                    send(b.map(|c| c.tx()).unwrap_or(nil2), v2) -> _ => {
                        i += 1;
                        b = None;
                    }
                    default => break,
                }
                shift.set(shift.get() + 1);
            }
            i
        };

        let a = make::<u32>(1);
        let b = make::<u32>(1);

        assert_eq!(send(Some(&a), Some(&b)), 2);

        let av = a.recv().unwrap();
        let bv = b.recv().unwrap();
        assert_eq!(av | bv, 3);

        assert_eq!(send(Some(&a), None), 1);
        assert_eq!(counter.get(), 10);
    }
}

// https://github.com/golang/go/blob/master/test/chan/select2.go
mod select2 {
    use super::*;

    #[cfg(miri)]
    const N: i32 = 200;
    #[cfg(not(miri))]
    const N: i32 = 100000;

    #[test]
    fn main() {
        fn sender(c: &Chan<i32>, n: i32) {
            for _ in 0..n {
                c.send(1);
            }
        }

        fn receiver(c: &Chan<i32>, dummy: &Chan<i32>, n: i32) {
            for _ in 0..n {
                select! {
                    recv(c.rx()) -> _ => {}
                    recv(dummy.rx()) -> _ => {
                        panic!("dummy");
                    }
                }
            }
        }

        let c = make_unbounded::<i32>();
        let dummy = make_unbounded::<i32>();

        ALLOCATED.store(0, SeqCst);

        go!(c, sender(&c, N));
        receiver(&c, &dummy, N);

        let alloc = ALLOCATED.load(SeqCst);

        go!(c, sender(&c, N));
        receiver(&c, &dummy, N);

        assert!(
            !(ALLOCATED.load(SeqCst) > alloc
                && (ALLOCATED.load(SeqCst) - alloc) > (N as usize + 10000))
        )
    }
}

// https://github.com/golang/go/blob/master/test/chan/select3.go
mod select3 {
    // TODO
}

// https://github.com/golang/go/blob/master/test/chan/select4.go
mod select4 {
    use super::*;

    #[test]
    fn main() {
        let c = make::<i32>(1);
        let c1 = make::<i32>(0);
        c.send(42);
        select! {
            recv(c1.rx()) -> _ => panic!("BUG"),
            recv(c.rx()) -> v => assert_eq!(v, Ok(42)),
        }
    }
}

// https://github.com/golang/go/blob/master/test/chan/select6.go
mod select6 {
    use super::*;

    #[test]
    fn main() {
        let c1 = make::<bool>(0);
        let c2 = make::<bool>(0);
        let c3 = make::<bool>(0);

        go!(c1, c1.recv());
        go!(c1, c2, c3, {
            select! {
                recv(c1.rx()) -> _ => panic!("dummy"),
                recv(c2.rx()) -> _ => c3.send(true),
            }
            c1.recv();
        });
        go!(c2, c2.send(true));

        c3.recv();
        c1.send(true);
        c1.send(true);
    }
}

// https://github.com/golang/go/blob/master/test/chan/select7.go
mod select7 {
    use super::*;

    fn recv1(c: Chan<i32>) {
        c.recv().unwrap();
    }

    fn recv2(c: Chan<i32>) {
        select! {
            recv(c.rx()) -> _ => ()
        }
    }

    fn recv3(c: Chan<i32>) {
        let c2 = make::<i32>(1);
        select! {
            recv(c.rx()) -> _ => (),
            recv(c2.rx()) -> _ => ()
        }
    }

    fn send1(recv: fn(Chan<i32>)) {
        let c = make::<i32>(1);
        go!(c, recv(c));
        thread::yield_now();
        c.send(1);
    }

    fn send2(recv: fn(Chan<i32>)) {
        let c = make::<i32>(1);
        go!(c, recv(c));
        thread::yield_now();
        select! {
            send(c.tx(), 1) -> _ => ()
        }
    }

    fn send3(recv: fn(Chan<i32>)) {
        let c = make::<i32>(1);
        go!(c, recv(c));
        thread::yield_now();
        let c2 = make::<i32>(1);
        select! {
            send(c.tx(), 1) -> _ => (),
            send(c2.tx(), 1) -> _ => ()
        }
    }

    #[test]
    fn main() {
        send1(recv1);
        send2(recv1);
        send3(recv1);
        send1(recv2);
        send2(recv2);
        send3(recv2);
        send1(recv3);
        send2(recv3);
        send3(recv3);
    }
}

// https://github.com/golang/go/blob/master/test/chan/sieve1.go
mod sieve1 {
    use super::*;

    fn generate(ch: Chan<i32>) {
        let mut i = 2;
        loop {
            ch.send(i);
            i += 1;
        }
    }

    fn filter(in_ch: Chan<i32>, out_ch: Chan<i32>, prime: i32) {
        for i in in_ch {
            if i % prime != 0 {
                out_ch.send(i);
            }
        }
    }

    fn sieve(primes: Chan<i32>) {
        let mut ch = make::<i32>(1);
        go!(ch, generate(ch));
        loop {
            let prime = ch.recv().unwrap();
            primes.send(prime);

            let ch1 = make::<i32>(1);
            go!(ch, ch1, prime, filter(ch, ch1, prime));
            ch = ch1;
        }
    }

    #[test]
    fn main() {
        let primes = make::<i32>(1);
        go!(primes, sieve(primes));

        let a = [
            2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83,
            89, 97,
        ];
        #[cfg(miri)]
        let a = &a[..10];

        for item in a.iter() {
            let x = primes.recv().unwrap();
            if x != *item {
                println!("{} != {}", x, item);
                panic!("fail");
            }
        }
    }
}

// https://github.com/golang/go/blob/master/test/chan/zerosize.go
mod zerosize {
    use super::*;

    #[test]
    fn zero_size_struct() {
        struct ZeroSize;
        let _ = make::<ZeroSize>(0);
    }

    #[test]
    fn zero_size_array() {
        let _ = make::<[u8; 0]>(0);
    }
}

// https://github.com/golang/go/blob/master/src/runtime/chan_test.go
mod chan_test {
    use super::*;

    #[test]
    fn test_chan() {
        #[cfg(miri)]
        const N: i32 = 12;
        #[cfg(not(miri))]
        const N: i32 = 200;

        #[cfg(miri)]
        const MESSAGES_COUNT: i32 = 20;
        #[cfg(not(miri))]
        const MESSAGES_COUNT: i32 = 100;

        for cap in 0..N {
            {
                // Ensure that receive from empty chan blocks.
                let c = make::<i32>(cap as usize);

                let recv1 = Arc::new(Mutex::new(false));
                go!(c, recv1, {
                    c.recv();
                    *recv1.lock().unwrap() = true;
                });

                let recv2 = Arc::new(Mutex::new(false));
                go!(c, recv2, {
                    c.recv();
                    *recv2.lock().unwrap() = true;
                });

                thread::sleep(ms(1));

                if *recv1.lock().unwrap() || *recv2.lock().unwrap() {
                    panic!();
                }

                // Ensure that non-blocking receive does not block.
                select! {
                    recv(c.rx()) -> _ => panic!(),
                    default => {}
                }
                select! {
                    recv(c.rx()) -> _ => panic!(),
                    default => {}
                }

                c.send(0);
                c.send(0);
            }

            {
                // Ensure that send to full chan blocks.
                let c = make::<i32>(cap as usize);
                for i in 0..cap {
                    c.send(i);
                }

                let sent = Arc::new(Mutex::new(0));
                go!(sent, c, {
                    c.send(0);
                    *sent.lock().unwrap() = 1;
                });

                thread::sleep(ms(1));

                if *sent.lock().unwrap() != 0 {
                    panic!();
                }

                // Ensure that non-blocking send does not block.
                select! {
                    send(c.tx(), 0) -> _ => panic!(),
                    default => {}
                }
                c.recv();
            }

            {
                // Ensure that we receive 0 from closed chan.
                let c = make::<i32>(cap as usize);
                for i in 0..cap {
                    c.send(i);
                }
                c.close_s();

                for i in 0..cap {
                    let v = c.recv();
                    if v != Some(i) {
                        panic!();
                    }
                }

                if c.recv() != None {
                    panic!();
                }
                if c.try_recv() != None {
                    panic!();
                }
            }

            {
                // Ensure that close unblocks receive.
                let c = make::<i32>(cap as usize);
                let done = make::<bool>(0);

                go!(c, done, {
                    let v = c.try_recv();
                    done.send(v.is_none());
                });

                thread::sleep(ms(1));
                c.close_s();

                if !done.recv().unwrap() {
                    panic!();
                }
            }

            {
                // Send many integers,
                // ensure that we receive them non-corrupted in FIFO order.
                let c = make::<i32>(cap as usize);
                go!(c, {
                    for i in 0..MESSAGES_COUNT {
                        c.send(i);
                    }
                });
                for i in 0..MESSAGES_COUNT {
                    if c.recv() != Some(i) {
                        panic!();
                    }
                }

                // Same, but using recv2.
                go!(c, {
                    for i in 0..MESSAGES_COUNT {
                        c.send(i);
                    }
                });
                for i in 0..MESSAGES_COUNT {
                    if c.recv() != Some(i) {
                        panic!();
                    }
                }
            }
        }
    }

    #[test]
    fn test_nonblock_recv_race() {
        #[cfg(miri)]
        const N: usize = 100;
        #[cfg(not(miri))]
        const N: usize = 1000;

        for _ in 0..N {
            let c = make::<i32>(1);
            c.send(1);

            let t = go!(c, {
                select! {
                    recv(c.rx()) -> _ => {}
                    default => panic!("chan is not ready"),
                }
            });

            c.close_s();
            c.recv();
            t.join().unwrap();
        }
    }

    #[test]
    fn test_nonblock_select_race() {
        #[cfg(miri)]
        const N: usize = 100;
        #[cfg(not(miri))]
        const N: usize = 1000;

        let done = make::<bool>(1);
        for _ in 0..N {
            let c1 = make::<i32>(1);
            let c2 = make::<i32>(1);
            c1.send(1);

            go!(c1, c2, done, {
                select! {
                    recv(c1.rx()) -> _ => {}
                    recv(c2.rx()) -> _ => {}
                    default => {
                        done.send(false);
                        return;
                    }
                }
                done.send(true);
            });

            c2.send(1);
            select! {
                recv(c1.rx()) -> _ => {}
                default => {}
            }
            if !done.recv().unwrap() {
                panic!("no chan is ready");
            }
        }
    }

    #[test]
    fn test_nonblock_select_race2() {
        #[cfg(miri)]
        const N: usize = 100;
        #[cfg(not(miri))]
        const N: usize = 1000;

        let done = make::<bool>(1);
        for _ in 0..N {
            let c1 = make::<i32>(1);
            let c2 = make::<i32>(0);
            c1.send(1);

            go!(c1, c2, done, {
                select! {
                    recv(c1.rx()) -> _ => {}
                    recv(c2.rx()) -> _ => {}
                    default => {
                        done.send(false);
                        return;
                    }
                }
                done.send(true);
            });

            c2.close_s();
            select! {
                recv(c1.rx()) -> _ => {}
                default => {}
            }
            if !done.recv().unwrap() {
                panic!("no chan is ready");
            }
        }
    }

    #[test]
    fn test_self_select() {
        // Ensure that send/recv on the same chan in select
        // does not crash nor deadlock.

        #[cfg(miri)]
        const N: usize = 100;
        #[cfg(not(miri))]
        const N: usize = 1000;

        for &cap in &[0, 10] {
            let wg = WaitGroup::new();
            wg.add(2);
            let c = make::<i32>(cap);

            for p in 0..2 {
                let p = p;
                go!(wg, p, c, {
                    defer! { wg.done() }
                    for i in 0..N {
                        if p == 0 || i % 2 == 0 {
                            select! {
                                send(c.tx(), p) -> _ => {}
                                recv(c.rx()) -> v => {
                                    if cap == 0 && v.ok() == Some(p) {
                                        panic!("self receive");
                                    }
                                }
                            }
                        } else {
                            select! {
                                recv(c.rx()) -> v => {
                                    if cap == 0 && v.ok() == Some(p) {
                                        panic!("self receive");
                                    }
                                }
                                send(c.tx(), p) -> _ => {}
                            }
                        }
                    }
                });
            }
            wg.wait();
        }
    }

    #[test]
    fn test_select_stress() {
        #[cfg(miri)]
        const N: usize = 100;
        #[cfg(not(miri))]
        const N: usize = 10000;

        let c = vec![
            make::<i32>(0),
            make::<i32>(0),
            make::<i32>(2),
            make::<i32>(3),
        ];

        // There are 4 goroutines that send N values on each of the chans,
        // + 4 goroutines that receive N values on each of the chans,
        // + 1 goroutine that sends N values on each of the chans in a single select,
        // + 1 goroutine that receives N values on each of the chans in a single select.
        // All these sends, receives and selects interact chaotically at runtime,
        // but we are careful that this whole construct does not deadlock.
        let wg = WaitGroup::new();
        wg.add(10);

        for k in 0..4 {
            go!(k, c, wg, {
                for _ in 0..N {
                    c[k].send(0);
                }
                wg.done();
            });
            go!(k, c, wg, {
                for _ in 0..N {
                    c[k].recv();
                }
                wg.done();
            });
        }

        go!(c, wg, {
            let mut n = [0; 4];
            let mut c1 = c.iter().map(|c| Some(c.rx().clone())).collect::<Vec<_>>();

            for _ in 0..4 * N {
                let index = {
                    let mut sel = Select::new();
                    let mut opers = [!0; 4];
                    for &i in &[3, 2, 0, 1] {
                        if let Some(c) = &c1[i] {
                            opers[i] = sel.recv(c);
                        }
                    }

                    let oper = sel.select();
                    let mut index = !0;
                    for i in 0..4 {
                        if opers[i] == oper.index() {
                            index = i;
                            let _ = oper.recv(c1[i].as_ref().unwrap());
                            break;
                        }
                    }
                    index
                };

                n[index] += 1;
                if n[index] == N {
                    c1[index] = None;
                }
            }
            wg.done();
        });

        go!(c, wg, {
            let mut n = [0; 4];
            let mut c1 = c.iter().map(|c| Some(c.tx().clone())).collect::<Vec<_>>();

            for _ in 0..4 * N {
                let index = {
                    let mut sel = Select::new();
                    let mut opers = [!0; 4];
                    for &i in &[0, 1, 2, 3] {
                        if let Some(c) = &c1[i] {
                            opers[i] = sel.send(c);
                        }
                    }

                    let oper = sel.select();
                    let mut index = !0;
                    for i in 0..4 {
                        if opers[i] == oper.index() {
                            index = i;
                            let _ = oper.send(c1[i].as_ref().unwrap(), 0);
                            break;
                        }
                    }
                    index
                };

                n[index] += 1;
                if n[index] == N {
                    c1[index] = None;
                }
            }
            wg.done();
        });

        wg.wait();
    }

    #[test]
    fn test_select_fairness() {
        #[cfg(miri)]
        const TRIALS: usize = 100;
        #[cfg(not(miri))]
        const TRIALS: usize = 10000;

        let c1 = make::<u8>(TRIALS + 1);
        let c2 = make::<u8>(TRIALS + 1);

        for _ in 0..TRIALS + 1 {
            c1.send(1);
            c2.send(2);
        }

        let c3 = make::<u8>(0);
        let c4 = make::<u8>(0);
        let out = make::<u8>(0);
        let done = make::<u8>(0);
        let wg = WaitGroup::new();

        wg.add(1);
        go!(wg, c1, c2, c3, c4, out, done, {
            defer! { wg.done() };
            loop {
                let b;
                select! {
                    recv(c3.rx()) -> m => b = m.unwrap(),
                    recv(c4.rx()) -> m => b = m.unwrap(),
                    recv(c1.rx()) -> m => b = m.unwrap(),
                    recv(c2.rx()) -> m => b = m.unwrap(),
                }
                select! {
                    send(out.tx(), b) -> _ => {}
                    recv(done.rx()) -> _ => return,
                }
            }
        });

        let (mut cnt1, mut cnt2) = (0, 0);
        for _ in 0..TRIALS {
            match out.recv() {
                Some(1) => cnt1 += 1,
                Some(2) => cnt2 += 1,
                b => panic!("unexpected value {:?} on channel", b),
            }
        }

        // If the select in the goroutine is fair,
        // cnt1 and cnt2 should be about the same value.
        // With 10,000 trials, the expected margin of error at
        // a confidence level of five nines is 4.4172 / (2 * Sqrt(10000)).

        let r = cnt1 as f64 / TRIALS as f64;
        let e = (r - 0.5).abs();

        if e > 4.4172 / (2.0 * (TRIALS as f64).sqrt()) {
            panic!(
                "unfair select: in {} trials, results were {}, {}",
                TRIALS, cnt1, cnt2,
            );
        }

        done.close_s();
        wg.wait();
    }

    #[test]
    fn test_chan_send_interface() {
        struct Mt;

        let c = make::<Box<dyn Any>>(1);
        c.send(Box::new(Mt));

        select! {
            send(c.tx(), Box::new(Mt)) -> _ => {}
            default => {}
        }

        select! {
            send(c.tx(), Box::new(Mt)) -> _ => {}
            send(c.tx(), Box::new(Mt)) -> _ => {}
            default => {}
        }
    }

    #[test]
    fn test_pseudo_random_send() {
        #[cfg(miri)]
        const N: usize = 20;
        #[cfg(not(miri))]
        const N: usize = 100;

        for cap in 0..N {
            let c = make::<i32>(cap);
            let l = Arc::new(Mutex::new(vec![0i32; N]));
            let done = make::<bool>(0);

            go!(c, done, l, {
                let mut l = l.lock().unwrap();
                for i in 0..N {
                    thread::yield_now();
                    l[i] = c.recv().unwrap();
                }
                done.send(true);
            });

            for _ in 0..N {
                select! {
                    send(c.tx(), 1) -> _ => {}
                    send(c.tx(), 0) -> _ => {}
                }
            }
            done.recv();

            let mut n0 = 0;
            let mut n1 = 0;
            for &i in l.lock().unwrap().iter() {
                n0 += (i + 1) % 2;
                n1 += i;
            }

            if n0 <= N as i32 / 10 || n1 <= N as i32 / 10 {
                panic!(
                    "Want pseudorandom, got {} zeros and {} ones (chan cap {})",
                    n0, n1, cap,
                );
            }
        }
    }

    #[test]
    fn test_multi_consumer() {
        const NWORK: usize = 23;
        #[cfg(miri)]
        const NITER: usize = 50;
        #[cfg(not(miri))]
        const NITER: usize = 271828;

        let pn = [2, 3, 7, 11, 13, 17, 19, 23, 27, 31];

        let q = make::<i32>(NWORK * 3);
        let r = make::<i32>(NWORK * 3);

        let wg = WaitGroup::new();
        for i in 0..NWORK {
            wg.add(1);
            let w = i;
            go!(q, r, wg, pn, {
                for v in &q {
                    if pn[w % pn.len()] == v {
                        thread::yield_now();
                    }
                    r.send(v);
                }
                wg.done();
            });
        }

        let expect = Arc::new(Mutex::new(0));
        go!(q, r, expect, wg, pn, {
            for i in 0..NITER {
                let v = pn[i % pn.len()];
                *expect.lock().unwrap() += v;
                q.send(v);
            }
            q.close_s();
            wg.wait();
            r.close_s();
        });

        let mut n = 0;
        let mut s = 0;
        for v in &r {
            n += 1;
            s += v;
        }

        if n != NITER || s != *expect.lock().unwrap() {
            panic!();
        }
    }

    #[test]
    fn test_select_duplicate_channel() {
        // This test makes sure we can queue a G on
        // the same channel multiple times.
        let c = make::<i32>(0);
        let d = make::<i32>(0);
        let e = make::<i32>(0);

        go!(c, d, e, {
            select! {
                recv(c.rx()) -> _ => {}
                recv(d.rx()) -> _ => {}
                recv(e.rx()) -> _ => {}
            }
            e.send(9);
        });
        thread::sleep(ms(1));

        go!(c, c.recv());
        thread::sleep(ms(1));

        d.send(7);
        e.recv();
        c.send(8);
    }
}

// https://github.com/golang/go/blob/master/test/closedchan.go
mod closedchan {
    // TODO
}

// https://github.com/golang/go/blob/master/src/runtime/chanbarrier_test.go
mod chanbarrier_test {
    // TODO
}

// https://github.com/golang/go/blob/master/src/runtime/race/testdata/chan_test.go
mod race_chan_test {
    // TODO
}

// https://github.com/golang/go/blob/master/test/ken/chan.go
mod chan {
    use super::*;

    const MESSAGES_PER_CHANEL: u32 = 76;
    const MESSAGES_RANGE_LEN: u32 = 100;
    const END: i32 = 10000;

    struct ChanWithVals {
        chan: Chan<i32>,
        /// Next value to send
        sv: Arc<AtomicI32>,
        /// Next value to receive
        rv: Arc<AtomicI32>,
    }

    struct Totals {
        /// Total sent messages
        tots: u32,
        /// Total received messages
        totr: u32,
    }

    struct Context {
        nproc: Arc<Mutex<i32>>,
        cval: Arc<Mutex<i32>>,
        tot: Arc<Mutex<Totals>>,
        nc: ChanWithVals,
        randx: Arc<Mutex<i32>>,
    }

    impl ChanWithVals {
        fn with_capacity(capacity: usize) -> Self {
            ChanWithVals {
                chan: make(capacity),
                sv: Arc::new(AtomicI32::new(0)),
                rv: Arc::new(AtomicI32::new(0)),
            }
        }

        fn closed() -> Self {
            let ch = ChanWithVals::with_capacity(0);
            ch.chan.close_r();
            ch.chan.close_s();
            ch
        }

        fn rv(&self) -> i32 {
            self.rv.load(SeqCst)
        }

        fn sv(&self) -> i32 {
            self.sv.load(SeqCst)
        }

        fn send(&mut self, tot: &Mutex<Totals>) -> bool {
            {
                let mut tot = tot.lock().unwrap();
                tot.tots += 1
            }
            let esv = expect(self.sv(), self.sv());
            self.sv.store(esv, SeqCst);
            if self.sv() == END {
                self.chan.close_s();
                return true;
            }
            false
        }

        fn recv(&mut self, v: i32, tot: &Mutex<Totals>) -> bool {
            {
                let mut tot = tot.lock().unwrap();
                tot.totr += 1
            }
            let erv = expect(self.rv(), v);
            self.rv.store(erv, SeqCst);
            if self.rv() == END {
                self.chan.close_r();
                return true;
            }
            false
        }
    }

    impl Clone for ChanWithVals {
        fn clone(&self) -> Self {
            ChanWithVals {
                chan: self.chan.clone(),
                sv: self.sv.clone(),
                rv: self.rv.clone(),
            }
        }
    }

    impl Context {
        fn nproc(&self) -> &Mutex<i32> {
            self.nproc.as_ref()
        }

        fn cval(&self) -> &Mutex<i32> {
            self.cval.as_ref()
        }

        fn tot(&self) -> &Mutex<Totals> {
            self.tot.as_ref()
        }

        fn randx(&self) -> &Mutex<i32> {
            self.randx.as_ref()
        }
    }

    impl Clone for Context {
        fn clone(&self) -> Self {
            Context {
                nproc: self.nproc.clone(),
                cval: self.cval.clone(),
                tot: self.tot.clone(),
                nc: self.nc.clone(),
                randx: self.randx.clone(),
            }
        }
    }

    fn nrand(n: i32, randx: &Mutex<i32>) -> i32 {
        let mut randx = randx.lock().unwrap();
        *randx += 10007;
        if *randx >= 1000000 {
            *randx -= 1000000
        }
        *randx % n
    }

    fn change_nproc(adjust: i32, nproc: &Mutex<i32>) -> i32 {
        let mut nproc = nproc.lock().unwrap();
        *nproc += adjust;
        *nproc
    }

    fn mkchan(c: usize, n: usize, cval: &Mutex<i32>) -> Vec<ChanWithVals> {
        let mut ca = Vec::<ChanWithVals>::with_capacity(n);
        let mut cval = cval.lock().unwrap();
        for _ in 0..n {
            *cval += MESSAGES_RANGE_LEN as i32;
            let chl = ChanWithVals::with_capacity(c);
            chl.sv.store(*cval, SeqCst);
            chl.rv.store(*cval, SeqCst);
            ca.push(chl);
        }
        ca
    }

    fn expect(v: i32, v0: i32) -> i32 {
        if v == v0 {
            return if v % MESSAGES_RANGE_LEN as i32 == MESSAGES_PER_CHANEL as i32 - 1 {
                END
            } else {
                v + 1
            };
        }
        panic!("got {}, expected {}", v, v0 + 1);
    }

    fn send(mut c: ChanWithVals, ctx: Context) {
        loop {
            for _ in 0..=nrand(10, ctx.randx()) {
                thread::yield_now();
            }
            c.chan.tx().send(c.sv()).unwrap();
            if c.send(ctx.tot()) {
                break;
            }
        }
        change_nproc(-1, ctx.nproc());
    }

    fn recv(mut c: ChanWithVals, ctx: Context) {
        loop {
            for _ in (0..nrand(10, ctx.randx())).rev() {
                thread::yield_now();
            }
            let v = c.chan.rx().recv().unwrap();
            if c.recv(v, ctx.tot()) {
                break;
            }
        }
        change_nproc(-1, ctx.nproc());
    }

    #[allow(clippy::too_many_arguments)]
    fn sel(
        mut r0: ChanWithVals,
        mut r1: ChanWithVals,
        mut r2: ChanWithVals,
        mut r3: ChanWithVals,
        mut s0: ChanWithVals,
        mut s1: ChanWithVals,
        mut s2: ChanWithVals,
        mut s3: ChanWithVals,
        ctx: Context,
    ) {
        let mut a = 0; // local chans running

        if r0.chan.has_rx() {
            a += 1;
        }
        if r1.chan.has_rx() {
            a += 1;
        }
        if r2.chan.has_rx() {
            a += 1;
        }
        if r3.chan.has_rx() {
            a += 1;
        }
        if s0.chan.has_tx() {
            a += 1;
        }
        if s1.chan.has_tx() {
            a += 1;
        }
        if s2.chan.has_tx() {
            a += 1;
        }
        if s3.chan.has_tx() {
            a += 1;
        }

        loop {
            for _ in 0..=nrand(5, ctx.randx()) {
                thread::yield_now();
            }
            select! {
                recv(r0.chan.rx()) -> v => if r0.recv(v.unwrap(), ctx.tot()) { a -= 1 },
                recv(r1.chan.rx()) -> v => if r1.recv(v.unwrap(), ctx.tot()) { a -= 1 },
                recv(r2.chan.rx()) -> v => if r2.recv(v.unwrap(), ctx.tot()) { a -= 1 },
                recv(r3.chan.rx()) -> v => if r3.recv(v.unwrap(), ctx.tot()) { a -= 1 },
                send(s0.chan.tx(), s0.sv()) -> _ => if s0.send(ctx.tot()) { a -= 1 },
                send(s1.chan.tx(), s1.sv()) -> _ => if s1.send(ctx.tot()) { a -= 1 },
                send(s2.chan.tx(), s2.sv()) -> _ => if s2.send(ctx.tot()) { a -= 1 },
                send(s3.chan.tx(), s3.sv()) -> _ => if s3.send(ctx.tot()) { a -= 1 },
            }
            if a == 0 {
                break;
            }
        }
        change_nproc(-1, ctx.nproc());
    }

    fn get(vec: &[ChanWithVals], idx: usize) -> ChanWithVals {
        vec.get(idx).unwrap().clone()
    }

    /// Direct send to direct recv
    fn test1(c: ChanWithVals, ctx: &mut Context) {
        change_nproc(2, ctx.nproc());
        go!(c, ctx, send(c, ctx));
        go!(c, ctx, recv(c, ctx));
    }

    /// Direct send to select recv
    fn test2(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 4, ctx.cval());

        change_nproc(4, ctx.nproc());
        go!(ca, ctx, send(get(&ca, 0), ctx));
        go!(ca, ctx, send(get(&ca, 1), ctx));
        go!(ca, ctx, send(get(&ca, 2), ctx));
        go!(ca, ctx, send(get(&ca, 3), ctx));

        change_nproc(1, ctx.nproc());
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx,
            )
        );
    }

    /// Select send to direct recv
    fn test3(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 4, ctx.cval());

        change_nproc(4, ctx.nproc());
        go!(ca, ctx, recv(get(&ca, 0), ctx));
        go!(ca, ctx, recv(get(&ca, 1), ctx));
        go!(ca, ctx, recv(get(&ca, 2), ctx));
        go!(ca, ctx, recv(get(&ca, 3), ctx));

        change_nproc(1, ctx.nproc());
        go!(
            ca,
            ctx,
            sel(
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx,
            )
        );
    }

    /// Select send to select recv, 4 channels
    fn test4(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 4, ctx.cval());

        change_nproc(2, ctx.nproc());
        go!(
            ca,
            ctx,
            sel(
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx,
            )
        );
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx.nc.clone(),
                ctx,
            )
        );
    }

    /// Select send to select recv, 8 channels
    fn test5(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 8, ctx.cval());

        change_nproc(2, ctx.nproc());
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 4),
                get(&ca, 5),
                get(&ca, 6),
                get(&ca, 7),
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx,
            )
        );
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                get(&ca, 4),
                get(&ca, 5),
                get(&ca, 6),
                get(&ca, 7),
                ctx,
            )
        );
    }

    // Direct and select send to direct and select recv
    fn test6(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 12, ctx.cval());

        change_nproc(4, ctx.nproc());
        go!(ca, ctx, send(get(&ca, 4), ctx));
        go!(ca, ctx, send(get(&ca, 5), ctx));
        go!(ca, ctx, send(get(&ca, 6), ctx));
        go!(ca, ctx, send(get(&ca, 7), ctx));

        change_nproc(4, ctx.nproc());
        go!(ca, ctx, recv(get(&ca, 8), ctx));
        go!(ca, ctx, recv(get(&ca, 9), ctx));
        go!(ca, ctx, recv(get(&ca, 10), ctx));
        go!(ca, ctx, recv(get(&ca, 11), ctx));

        change_nproc(2, ctx.nproc());
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 4),
                get(&ca, 5),
                get(&ca, 6),
                get(&ca, 7),
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                ctx,
            )
        );
        go!(
            ca,
            ctx,
            sel(
                get(&ca, 0),
                get(&ca, 1),
                get(&ca, 2),
                get(&ca, 3),
                get(&ca, 8),
                get(&ca, 9),
                get(&ca, 10),
                get(&ca, 11),
                ctx,
            )
        );
    }

    fn wait(ctx: &mut Context) {
        thread::yield_now();
        while change_nproc(0, ctx.nproc()) != 0 {
            thread::yield_now();
        }
    }

    fn tests(c: usize, ctx: &mut Context) {
        let ca = mkchan(c, 4, ctx.cval());
        test1(get(&ca, 0), ctx);
        test1(get(&ca, 1), ctx);
        test1(get(&ca, 2), ctx);
        test1(get(&ca, 3), ctx);
        wait(ctx);

        test2(c, ctx);
        wait(ctx);

        test3(c, ctx);
        wait(ctx);

        test4(c, ctx);
        wait(ctx);

        test5(c, ctx);
        wait(ctx);

        test6(c, ctx);
        wait(ctx);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn main() {
        let mut ctx = Context {
            nproc: Arc::new(Mutex::new(0)),
            cval: Arc::new(Mutex::new(0)),
            tot: Arc::new(Mutex::new(Totals { tots: 0, totr: 0 })),
            nc: ChanWithVals::closed(),
            randx: Arc::new(Mutex::new(0)),
        };

        tests(0, &mut ctx);
        tests(1, &mut ctx);
        tests(10, &mut ctx);
        tests(100, &mut ctx);

        #[rustfmt::skip]
        let t = 4 * // buffer sizes
                    (4*4 + // tests 1,2,3,4 channels
                            8 + // test 5 channels
                            12) * // test 6 channels
                        MESSAGES_PER_CHANEL; // sends/recvs on a channel

        let tot = ctx.tot.lock().unwrap();
        if tot.tots != t || tot.totr != t {
            panic!("tots={} totr={} sb={}", tot.tots, tot.totr, t);
        }
    }
}

// https://github.com/golang/go/blob/master/test/ken/chan1.go
mod chan1 {
    use super::*;

    // sent messages
    #[cfg(miri)]
    const N: usize = 20;
    #[cfg(not(miri))]
    const N: usize = 1000;
    // receiving "goroutines"
    const M: usize = 10;
    // channel buffering
    const W: usize = 2;

    fn r(c: Chan<usize>, m: usize, h: Arc<Mutex<[usize; N]>>) {
        loop {
            select! {
                recv(c.rx()) -> rr => {
                    let r = rr.unwrap();
                    let mut data = h.lock().unwrap();
                    if data[r] != 1 {
                        println!("r\nm={}\nr={}\nh={}\n", m, r, data[r]);
                        panic!("fail")
                    }
                    data[r] = 2;
                }
            }
        }
    }

    fn s(c: Chan<usize>, h: Arc<Mutex<[usize; N]>>) {
        for n in 0..N {
            let r = n;
            let mut data = h.lock().unwrap();
            if data[r] != 0 {
                println!("s");
                panic!("fail");
            }
            data[r] = 1;
            // https://github.com/crossbeam-rs/crossbeam/pull/615#discussion_r550281094
            drop(data);
            c.send(r);
        }
    }

    #[test]
    fn main() {
        let h = Arc::new(Mutex::new([0usize; N]));
        let c = make::<usize>(W);
        for m in 0..M {
            go!(c, h, {
                r(c, m, h);
            });
            thread::yield_now();
        }
        thread::yield_now();
        thread::yield_now();
        s(c, h);
    }
}
