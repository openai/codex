use backtrace::Backtrace;

// 50-character module name
mod _234567890_234567890_234567890_234567890_234567890 {
    // 50-character struct name
    #[allow(non_camel_case_types)]
    pub struct _234567890_234567890_234567890_234567890_234567890<T>(T);
    impl<T> _234567890_234567890_234567890_234567890_234567890<T> {
        #[allow(dead_code)]
        pub fn new() -> crate::Backtrace {
            crate::Backtrace::new()
        }
    }
}

// Long function names must be truncated to (MAX_SYM_NAME - 1) characters.
// Only run this test for msvc, since gnu prints "<no info>" for all frames.
#[test]
#[cfg(all(windows, target_env = "msvc"))]
fn test_long_fn_name() {
    use _234567890_234567890_234567890_234567890_234567890::_234567890_234567890_234567890_234567890_234567890 as S;

    // 10 repetitions of struct name, so fully qualified function name is
    // atleast 10 * (50 + 50) * 2 = 2000 characters long.
    // It's actually longer since it also includes `::`, `<>` and the
    // name of the current module
    let bt = S::<S<S<S<S<S<S<S<S<S<i32>>>>>>>>>>::new();
    println!("{bt:?}");

    let mut found_long_name_frame = false;

    for frame in bt.frames() {
        let symbols = frame.symbols();
        if symbols.is_empty() {
            continue;
        }

        if let Some(function_name) = symbols[0].name() {
            let function_name = function_name.as_str().unwrap();
            if function_name.contains("::_234567890_234567890_234567890_234567890_234567890") {
                found_long_name_frame = true;
                assert!(function_name.len() > 200);
            }
        }
    }

    assert!(found_long_name_frame);
}
