//! Benchmarks for rustix.
//!
//! To enable these benchmarks, add `--cfg=criterion` to RUSTFLAGS and enable
//! the "fs", "time", and "process" cargo features.
//!
//! ```sh
//! RUSTFLAGS=--cfg=criterion cargo bench --features=fs,time,process,stdio
//! ```

#[cfg(any(
    not(criterion),
    not(feature = "fs"),
    not(feature = "process"),
    not(feature = "time"),
    not(feature = "stdio"),
    windows,
    target_os = "emscripten",
    target_os = "redox",
    target_os = "wasi",
))]
fn main() {
    unimplemented!(
        "Add --cfg=criterion to RUSTFLAGS and enable the \"fs\", \"time\", \"process\", and \"stdio\" cargo \
         features."
    )
}

#[cfg(not(any(
    not(criterion),
    not(feature = "fs"),
    not(feature = "process"),
    not(feature = "time"),
    not(feature = "stdio"),
    windows,
    target_os = "emscripten",
    target_os = "redox",
    target_os = "wasi",
)))]
use criterion::{criterion_group, criterion_main};

#[cfg(not(any(
    not(criterion),
    not(feature = "fs"),
    not(feature = "process"),
    not(feature = "time"),
    not(feature = "stdio"),
    windows,
    target_os = "emscripten",
    target_os = "redox",
    target_os = "wasi",
)))]
mod suite {
    use criterion::Criterion;

    pub(super) fn simple_statat(c: &mut Criterion) {
        use rustix::fs::{statat, AtFlags, CWD};

        c.bench_function("simple statat", |b| {
            b.iter(|| {
                statat(CWD, "/", AtFlags::empty()).unwrap();
            })
        });
    }

    pub(super) fn simple_statat_libc(c: &mut Criterion) {
        c.bench_function("simple statat libc", |b| {
            b.iter(|| {
                let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
                unsafe {
                    assert_eq!(
                        libc::fstatat(
                            libc::AT_FDCWD,
                            std::ffi::CString::new("/").unwrap().as_c_str().as_ptr() as _,
                            s.as_mut_ptr(),
                            0
                        ),
                        0
                    );
                }
            })
        });
    }

    pub(super) fn simple_statat_libc_cstr(c: &mut Criterion) {
        c.bench_function("simple statat libc cstr", |b| {
            b.iter(|| {
                let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
                unsafe {
                    assert_eq!(
                        libc::fstatat(
                            libc::AT_FDCWD,
                            rustix::cstr!("/").as_ptr() as _,
                            s.as_mut_ptr(),
                            0
                        ),
                        0
                    );
                }
            })
        });
    }

    pub(super) fn simple_statat_cstr(c: &mut Criterion) {
        use rustix::fs::{statat, AtFlags, CWD};

        c.bench_function("simple statat cstr", |b| {
            b.iter(|| {
                statat(CWD, rustix::cstr!("/"), AtFlags::empty()).unwrap();
            })
        });
    }

    pub(super) fn simple_fstat(c: &mut Criterion) {
        use rustix::fs::fstat;

        c.bench_function("simple fstat", |b| {
            b.iter(|| {
                fstat(rustix::stdio::stdin()).unwrap();
            })
        });
    }

    pub(super) fn simple_fstat_libc(c: &mut Criterion) {
        c.bench_function("simple fstat libc", |b| {
            b.iter(|| {
                let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
                unsafe {
                    assert_eq!(libc::fstat(libc::STDIN_FILENO, s.as_mut_ptr()), 0);
                }
            })
        });
    }

    #[cfg(not(target_os = "wasi"))]
    pub(super) fn simple_clock_gettime(c: &mut Criterion) {
        use rustix::time::{clock_gettime, ClockId};

        c.bench_function("simple clock_gettime", |b| {
            b.iter(|| {
                let _ = clock_gettime(ClockId::Monotonic);
            })
        });
    }

    #[cfg(not(target_os = "wasi"))]
    pub(super) fn simple_clock_gettime_libc(c: &mut Criterion) {
        c.bench_function("simple clock_gettime libc", |b| {
            b.iter(|| {
                let mut s = std::mem::MaybeUninit::<libc::timespec>::uninit();
                unsafe {
                    assert_eq!(
                        libc::clock_gettime(libc::CLOCK_MONOTONIC, s.as_mut_ptr()),
                        0
                    );
                    let _ = s.assume_init();
                }
            })
        });
    }

    #[cfg(not(target_os = "wasi"))]
    pub(super) fn simple_getpid(c: &mut Criterion) {
        use rustix::process::getpid;

        c.bench_function("simple getpid", |b| {
            b.iter(|| {
                let _ = getpid();
            })
        });
    }

    #[cfg(not(target_os = "wasi"))]
    pub(super) fn simple_getpid_libc(c: &mut Criterion) {
        c.bench_function("simple getpid libc", |b| {
            b.iter(|| unsafe {
                let _ = libc::getpid();
            })
        });
    }
}

#[cfg(not(any(
    not(criterion),
    not(feature = "fs"),
    not(feature = "process"),
    not(feature = "time"),
    not(feature = "stdio"),
    windows,
    target_os = "emscripten",
    target_os = "redox",
    target_os = "wasi",
)))]
criterion_group!(
    benches,
    suite::simple_statat,
    suite::simple_statat_libc,
    suite::simple_statat_libc_cstr,
    suite::simple_statat_cstr,
    suite::simple_fstat,
    suite::simple_fstat_libc,
    suite::simple_clock_gettime,
    suite::simple_clock_gettime_libc,
    suite::simple_getpid,
    suite::simple_getpid_libc
);
#[cfg(not(any(
    not(criterion),
    not(feature = "fs"),
    not(feature = "process"),
    not(feature = "time"),
    not(feature = "stdio"),
    windows,
    target_os = "emscripten",
    target_os = "redox",
    target_os = "wasi",
)))]
criterion_main!(benches);
