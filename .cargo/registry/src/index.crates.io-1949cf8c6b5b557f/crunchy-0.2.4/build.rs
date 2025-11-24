use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

const LOWER_LIMIT: usize = 16;

fn main() {
    let limit = if cfg!(feature="limit_2048") {
        2048
    } else if cfg!(feature="limit_1024") {
        1024
    } else if cfg!(feature="limit_512") {
        512
    } else if cfg!(feature="limit_256") {
        256
    } else if cfg!(feature="limit_128") {
        128
    } else {
        64
    };

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("lib.rs");
    let mut f = File::create(&dest_path).unwrap();

    let mut output = String::new();

    output.push_str(r#"
/// Unroll the given for loop
///
/// Example:
///
/// ```ignore
/// unroll! {
///   for i in 0..5 {
///     println!("Iteration {}", i);
///   }
/// }
/// ```
///
/// will expand into:
///
/// ```ignore
/// { println!("Iteration {}", 0); }
/// { println!("Iteration {}", 1); }
/// { println!("Iteration {}", 2); }
/// { println!("Iteration {}", 3); }
/// { println!("Iteration {}", 4); }
/// ```
#[macro_export]
macro_rules! unroll {
    (for $v:ident in 0..0 $c:block) => {};

    (for $v:ident < $max:tt in ($start:tt..$end:tt).step_by($val:expr) {$($c:tt)*}) => {
        {
            let step = $val;
            let start = $start;
            let end = start + ($end - start) / step;
            unroll! {
                for val < $max in start..end {
                    let $v: usize = ((val - start) * step) + start;

                    $($c)*
                }
            }
        }
    };

    (for $v:ident in ($start:tt..$end:tt).step_by($val:expr) {$($c:tt)*}) => {
        unroll! {
            for $v < $end in ($start..$end).step_by($val) {$($c)*}
        }
    };

    (for $v:ident in ($start:tt..$end:tt) {$($c:tt)*}) => {
        unroll!{
            for $v in $start..$end {$($c)*}
        }
    };

    (for $v:ident in $start:tt..$end:tt {$($c:tt)*}) => {
        #[allow(non_upper_case_globals)]
        #[allow(unused_comparisons)]
        {
            unroll!(@$v, 0, $end, {
                    if $v >= $start {$($c)*}
                }
            );
        }
    };

    (for $v:ident < $max:tt in $start:tt..$end:tt $c:block) => {
        #[allow(non_upper_case_globals)]
        {
            let range = $start..$end;
            assert!(
                $max >= range.end,
                "`{}` out of range `{:?}`",
                stringify!($max),
                range,
            );
            unroll!(
                @$v,
                0,
                $max,
                {
                    if $v >= range.start && $v < range.end {
                        $c
                    }
                }
            );
        }
    };

    (for $v:ident in 0..$end:tt {$($statement:tt)*}) => {
        #[allow(non_upper_case_globals)]
        { unroll!(@$v, 0, $end, {$($statement)*}); }
    };

"#);

    for i in 0..limit + 1 {
        output.push_str(format!("    (@$v:ident, $a:expr, {}, $c:block) => {{\n", i).as_str());

        if i <= LOWER_LIMIT {
            output.push_str(format!("        {{ const $v: usize = $a; $c }}\n").as_str());

            for a in 1..i {
                output.push_str(format!("        {{ const $v: usize = $a + {}; $c }}\n", a).as_str());
            }
        } else {
            let half = i / 2;

            if i % 2 == 0 {
                output.push_str(format!("        unroll!(@$v, $a, {0}, $c);\n", half).as_str());
                output.push_str(format!("        unroll!(@$v, $a + {0}, {0}, $c);\n", half).as_str());
            } else {
                if half > 1 {
                    output.push_str(format!("        unroll!(@$v, $a, {}, $c);\n", i - 1).as_str())
                }

                output.push_str(format!("        {{ const $v: usize = $a + {}; $c }}\n", i - 1).as_str());
            }
        }

        output.push_str("    };\n\n");
    }

    output.push_str("}\n\n");

    output.push_str(format!(r#"
#[cfg(all(test, feature = "std"))]
mod tests {{
    #[test]
    fn invalid_range() {{
        let mut a: Vec<usize> = vec![];
        unroll! {{
                for i in (5..4) {{
                    a.push(i);
                }}
            }}
        assert_eq!(a, vec![]);
    }}

    #[test]
    fn start_at_one_with_step() {{
        let mut a: Vec<usize> = vec![];
        unroll! {{
                for i in (2..4).step_by(1) {{
                    a.push(i);
                }}
            }}
        assert_eq!(a, vec![2, 3]);
    }}

    #[test]
    fn start_at_one() {{
        let mut a: Vec<usize> = vec![];
        unroll! {{
                for i in 1..4 {{
                    a.push(i);
                }}
            }}
        assert_eq!(a, vec![1, 2, 3]);
    }}

    #[test]
    fn test_all() {{
        {{
            let a: Vec<usize> = vec![];
            unroll! {{
                for i in 0..0 {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (0..0).collect::<Vec<usize>>());
        }}
        {{
            let mut a: Vec<usize> = vec![];
            unroll! {{
                for i in 0..1 {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (0..1).collect::<Vec<usize>>());
        }}
        {{
            let mut a: Vec<usize> = vec![];
            unroll! {{
                for i in 0..{0} {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (0..{0}).collect::<Vec<usize>>());
        }}
        {{
            let mut a: Vec<usize> = vec![];
            let start = {0} / 4;
            let end = start * 3;
            unroll! {{
                for i < {0} in start..end {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (start..end).collect::<Vec<usize>>());
        }}
        {{
            let mut a: Vec<usize> = vec![];
            unroll! {{
                for i in (0..{0}).step_by(2) {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (0..{0} / 2).map(|x| x * 2).collect::<Vec<usize>>());
        }}
        {{
            let mut a: Vec<usize> = vec![];
            let start = {0} / 4;
            let end = start * 3;
            unroll! {{
                for i < {0} in (start..end).step_by(2) {{
                    a.push(i);
                }}
            }}
            assert_eq!(a, (start..end).filter(|x| x % 2 == 0).collect::<Vec<usize>>());
        }}
    }}
}}
"#, limit).as_str());

    f.write_all(output.as_bytes()).unwrap();

    println!("cargo:rustc-env=CRUNCHY_LIB_SUFFIX={}lib.rs", std::path::MAIN_SEPARATOR);
}
