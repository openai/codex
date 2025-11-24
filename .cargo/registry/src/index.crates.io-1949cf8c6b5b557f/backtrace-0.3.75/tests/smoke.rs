use backtrace::Frame;
use core::ffi::c_void;
use std::ptr;
use std::thread;

fn get_actual_fn_pointer(fp: *mut c_void) -> *mut c_void {
    // On AIX, the function name references a function descriptor.
    // A function descriptor consists of (See https://reviews.llvm.org/D62532)
    // * The address of the entry point of the function.
    // * The TOC base address for the function.
    // * The environment pointer.
    // Deref `fp` directly so that we can get the address of `fp`'s
    // entry point in text section.
    //
    // For TOC, one can find more information in
    // https://www.ibm.com/docs/en/aix/7.2?topic=program-understanding-programming-toc
    if cfg!(target_os = "aix") {
        unsafe {
            let actual_fn_entry = *(fp as *const *mut c_void);
            actual_fn_entry
        }
    } else {
        fp as *mut c_void
    }
}

#[test]
// FIXME: shouldn't ignore this test on i686-msvc, unsure why it's failing
#[cfg_attr(all(target_arch = "x86", target_env = "msvc"), ignore)]
#[inline(never)]
#[rustfmt::skip] // we care about line numbers here
fn smoke_test_frames() {
    frame_1(line!());
    #[inline(never)] fn frame_1(start_line: u32) { frame_2(start_line) }
    #[inline(never)] fn frame_2(start_line: u32) { frame_3(start_line) }
    #[inline(never)] fn frame_3(start_line: u32) { frame_4(start_line) }
    #[inline(never)] fn frame_4(start_line: u32) {
        let mut v = Vec::new();
        backtrace::trace(|cx| {
            v.push(cx.clone());
            true
        });

        // Various platforms have various bits of weirdness about their
        // backtraces. To find a good starting spot let's search through the
        // frames
        let target = get_actual_fn_pointer(frame_4 as *mut c_void);
        let offset = v
            .iter()
            .map(|frame| frame.symbol_address())
            .enumerate()
            .filter_map(|(i, sym)| {
                if sym >= target {
                    Some((sym, i))
                } else {
                    None
                }
            })
            .min()
            .unwrap()
            .1;
        let mut frames = v[offset..].iter();

        assert_frame(
            frames.next().unwrap(),
            get_actual_fn_pointer(frame_4 as *mut c_void) as usize,
            "frame_4",
            "tests/smoke.rs",
            start_line + 6,
            9,
        );
        assert_frame(
            frames.next().unwrap(),
            get_actual_fn_pointer(frame_3 as *mut c_void) as usize,
            "frame_3",
            "tests/smoke.rs",
            start_line + 3,
            52,
        );
        assert_frame(
            frames.next().unwrap(),
            get_actual_fn_pointer(frame_2 as *mut c_void) as usize,
            "frame_2",
            "tests/smoke.rs",
            start_line + 2,
            52,
        );
        assert_frame(
            frames.next().unwrap(),
            get_actual_fn_pointer(frame_1 as *mut c_void) as usize,
            "frame_1",
            "tests/smoke.rs",
            start_line + 1,
            52,
        );
        assert_frame(
            frames.next().unwrap(),
            get_actual_fn_pointer(smoke_test_frames as *mut c_void) as usize,
            "smoke_test_frames",
            "",
            0,
            0,
        );
    }

    fn assert_frame(
        frame: &Frame,
        actual_fn_pointer: usize,
        expected_name: &str,
        expected_file: &str,
        expected_line: u32,
        expected_col: u32,
    ) {
        backtrace::resolve_frame(frame, |sym| {
            print!("symbol  ip:{:?} address:{:?} ", frame.ip(), frame.symbol_address());
            if let Some(name) = sym.name() {
                print!("name:{name} ");
            }
            if let Some(file) = sym.filename() {
                print!("file:{} ", file.display());
            }
            if let Some(lineno) = sym.lineno() {
                print!("lineno:{lineno} ");
            }
            if let Some(colno) = sym.colno() {
                print!("colno:{colno} ");
            }
            println!();
        });

        let ip = frame.ip() as usize;
        let sym = frame.symbol_address() as usize;
        assert!(ip >= sym);
        assert!(
            sym >= actual_fn_pointer,
            "{:?} < {:?} ({} {}:{}:{})",
            sym as *const usize,
            actual_fn_pointer as *const usize,
            expected_name,
            expected_file,
            expected_line,
            expected_col,
        );

        // windows dbghelp is *quite* liberal (and wrong) in many of its reports
        // right now...
        //
        // This assertion can also fail for release builds, so skip it there
        if cfg!(debug_assertions) {
            assert!(sym - actual_fn_pointer < 1024);
        }

        let mut resolved = 0;

        let mut name = None;
        let mut addr = None;
        let mut col  = None;
        let mut line = None;
        let mut file = None;
        backtrace::resolve_frame(frame, |sym| {
            resolved += 1;
            name = sym.name().map(|v| v.to_string());
            addr = sym.addr();
            col  = sym.colno();
            line = sym.lineno();
            file = sym.filename().map(|v| v.to_path_buf());
        });
        assert!(resolved > 0);

        let name = name.expect("didn't find a name");

        // in release mode names get weird as functions can get merged
        // together with `mergefunc`, so only assert this in debug mode
        if cfg!(debug_assertions) {
            assert!(
                name.contains(expected_name),
                "didn't find `{expected_name}` in `{name}`"
            );
        }

        addr.expect("didn't find a symbol");

        if cfg!(debug_assertions) {
            let line = line.expect("didn't find a line number");
            let file = file.expect("didn't find a line number");
            if !expected_file.is_empty() {
                assert!(
                    file.ends_with(expected_file),
                    "{file:?} didn't end with {expected_file:?}"
                );
            }
            if expected_line != 0 {
                assert_eq!(
                    line,
                    expected_line,
                    "bad line number on frame for `{expected_name}`: {line} != {expected_line}");
            }

            // dbghelp on MSVC doesn't support column numbers
            if !cfg!(target_env = "msvc") {
                let col = col.expect("didn't find a column number");
                if expected_col != 0 {
                    assert_eq!(
                        col,
                        expected_col,
                        "bad column number on frame for `{expected_name}`: {col} != {expected_col}");
                }
            }
        }
    }
}

#[test]
fn many_threads() {
    let threads = (0..16)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..16 {
                    backtrace::trace(|frame| {
                        backtrace::resolve(frame.ip(), |symbol| {
                            let _s = symbol.name().map(|s| s.to_string());
                        });
                        true
                    });
                }
            })
        })
        .collect::<Vec<_>>();

    for t in threads {
        t.join().unwrap()
    }
}

#[test]
#[cfg(feature = "serde")]
fn is_serde() {
    extern crate serde;

    fn is_serialize<T: serde::ser::Serialize>() {}
    fn is_deserialize<T: serde::de::DeserializeOwned>() {}

    is_serialize::<backtrace::Backtrace>();
    is_deserialize::<backtrace::Backtrace>();
}

#[test]
fn sp_smoke_test() {
    let mut refs = vec![];
    recursive_stack_references(&mut refs);
    return;

    #[inline(never)]
    fn recursive_stack_references(refs: &mut Vec<*mut c_void>) {
        assert!(refs.len() < 5);

        let mut x = refs.len();
        refs.push(ptr::addr_of_mut!(x).cast());

        if refs.len() < 5 {
            recursive_stack_references(refs);
            eprintln!("exiting: {x}");
            return;
        }

        backtrace::trace(make_trace_closure(refs));
        eprintln!("exiting: {x}");
    }

    // NB: the following `make_*` functions are pulled out of line, rather than
    // defining their results as inline closures at their call sites, so that
    // the resulting closures don't have "recursive_stack_references" in their
    // mangled names.

    fn make_trace_closure<'a>(
        refs: &'a mut Vec<*mut c_void>,
    ) -> impl FnMut(&backtrace::Frame) -> bool + 'a {
        let mut child_sp = None;
        let mut child_ref = None;
        move |frame| {
            eprintln!("\n=== frame ===================================");

            let mut is_recursive_stack_references = false;
            backtrace::resolve(frame.ip(), |sym| {
                is_recursive_stack_references |=
                    sym.name()
                        .and_then(|name| name.as_str())
                        .map_or(false, |name| {
                            eprintln!("name = {name}");
                            name.contains("recursive_stack_references")
                        })
            });

            let sp = frame.sp();
            eprintln!("sp  = {sp:p}");
            if sp as usize == 0 {
                // If the SP is null, then we don't have an implementation for
                // getting the SP on this target. Just keep walking the stack,
                // but don't make our assertions about the on-stack pointers and
                // SP values.
                return true;
            }

            // The stack grows down.
            if let Some(child_sp) = child_sp {
                assert!(child_sp <= sp);
            }

            if is_recursive_stack_references {
                let r = refs.pop().unwrap();
                eprintln!("ref = {:p}", r);
                if sp as usize != 0 {
                    assert!(r > sp);
                    if let Some(child_ref) = child_ref {
                        assert!(sp >= child_ref);
                    }
                }
                child_ref = Some(r);
            }

            child_sp = Some(sp);
            true
        }
    }
}
