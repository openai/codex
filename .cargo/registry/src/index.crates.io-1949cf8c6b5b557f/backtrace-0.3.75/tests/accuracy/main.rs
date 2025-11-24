#![cfg(dbginfo = "collapsible")]
mod auxiliary;

macro_rules! pos {
    () => {
        (file!(), line!())
    };
}

#[collapse_debuginfo(yes)]
macro_rules! check {
    ($($pos:expr),*) => ({
        verify(&[$($pos,)* pos!()]);
    })
}

type Pos = (&'static str, u32);

#[test]
fn doit() {
    if
    // Skip musl which is by default statically linked and doesn't support
    // dynamic libraries.
    !cfg!(target_env = "musl")
    // Skip Miri, since it doesn't support dynamic libraries.
    && !cfg!(miri)
    {
        // TODO(#238) this shouldn't have to happen first in this function, but
        // currently it does.
        let mut dir = std::env::current_exe().unwrap();
        dir.pop();
        if cfg!(windows) {
            dir.push("dylib_dep.dll");
        } else if cfg!(target_vendor = "apple") {
            dir.push("libdylib_dep.dylib");
        } else if cfg!(target_os = "aix") {
            dir.push("libdylib_dep.a");
        } else {
            dir.push("libdylib_dep.so");
        }
        unsafe {
            let lib = libloading::Library::new(&dir).unwrap();
            let api = lib.get::<extern "C" fn(Pos, fn(Pos, Pos))>(b"foo").unwrap();
            api(pos!(), |a, b| {
                check!(a, b);
            });
        }
    }

    outer(pos!());
}

#[inline(never)]
fn outer(main_pos: Pos) {
    inner(main_pos, pos!());
    inner_inlined(main_pos, pos!());
}

#[inline(never)]
#[rustfmt::skip]
fn inner(main_pos: Pos, outer_pos: Pos) {
    check!(main_pos, outer_pos);
    check!(main_pos, outer_pos);
    let inner_pos = pos!(); auxiliary::callback(|aux_pos| {
        check!(main_pos, outer_pos, inner_pos, aux_pos);
    });
    let inner_pos = pos!(); auxiliary::callback_inlined(|aux_pos| {
        check!(main_pos, outer_pos, inner_pos, aux_pos);
    });
}

#[inline(always)]
#[rustfmt::skip]
fn inner_inlined(main_pos: Pos, outer_pos: Pos) {
    check!(main_pos, outer_pos);
    check!(main_pos, outer_pos);

    #[inline(always)]
    fn inner_further_inlined(main_pos: Pos, outer_pos: Pos, inner_pos: Pos) {
        check!(main_pos, outer_pos, inner_pos);
    }
    inner_further_inlined(main_pos, outer_pos, pos!());

    let inner_pos = pos!(); auxiliary::callback(|aux_pos| {
        check!(main_pos, outer_pos, inner_pos, aux_pos);
    });
    let inner_pos = pos!(); auxiliary::callback_inlined(|aux_pos| {
        check!(main_pos, outer_pos, inner_pos, aux_pos);
    });

    // this tests a distinction between two independent calls to the inlined function.
    // (un)fortunately, LLVM somehow merges two consecutive such calls into one node.
    inner_further_inlined(main_pos, outer_pos, pos!());
}

fn verify(filelines: &[Pos]) {
    let trace = backtrace::Backtrace::new();
    println!("-----------------------------------");
    println!("looking for:");
    for (file, line) in filelines.iter().rev() {
        println!("\t{file}:{line}");
    }
    println!("found:\n{trace:?}");
    let mut symbols = trace.frames().iter().flat_map(|frame| frame.symbols());
    let mut iter = filelines.iter().rev();
    while let Some((file, line)) = iter.next() {
        loop {
            let sym = match symbols.next() {
                Some(sym) => sym,
                None => panic!("failed to find {file}:{line}"),
            };
            if let Some(filename) = sym.filename() {
                if let Some(lineno) = sym.lineno() {
                    if filename.ends_with(file) && lineno == *line {
                        break;
                    }
                }
            }
        }
    }
}
