use debugger_test::debugger_test;
use tinyvec::*;

#[inline(never)]
fn __break() {
  println!("breakpoint hit");
}

#[debugger_test(
  debugger = "cdb",
  commands = r#"
dx strings
dx inline_tv
dx inline_tv.__0
g
dx slice_vec
g
dx strings
"#,
  expected_statements = r#"
pattern:strings          : \{ len=0x3 \} \[Type: tinyvec::arrayvec::ArrayVec<array\$<.*str.*,7> >\]
pattern:\[<Raw View>\]     \[Type: tinyvec::arrayvec::ArrayVec<array\$<.*str.*,7> >\]
pattern:\[len\]            : 0x3 \[Type: unsigned short\]
pattern:\[capacity\]       : 7
pattern:\[0\]              : "a" \[Type: .*str.*\]
pattern:\[1\]              : "b" \[Type: .*str.*\]
pattern:\[2\]              : "c" \[Type: .*str.*\]

inline_tv        : Inline [Type: enum2$<tinyvec::tinyvec::TinyVec<array$<i32,4> > >]
    [<Raw View>]     [Type: enum2$<tinyvec::tinyvec::TinyVec<array$<i32,4> > >]
    [+0x004] __0              : { len=0x4 } [Type: tinyvec::arrayvec::ArrayVec<array$<i32,4> >]

inline_tv.__0    : { len=0x4 } [Type: tinyvec::arrayvec::ArrayVec<array$<i32,4> >]
    [<Raw View>]     [Type: tinyvec::arrayvec::ArrayVec<array$<i32,4> >]
    [len]            : 0x4 [Type: unsigned short]
    [capacity]       : 4
    [0]              : 1 [Type: i32]
    [1]              : 2 [Type: i32]
    [2]              : 3 [Type: i32]
    [3]              : 4 [Type: i32]

pattern:slice_vec        : \{ len=0x3 \} \[Type: tinyvec::slicevec::SliceVec<.*str.*>\]
pattern:\[<Raw View>\]     \[Type: tinyvec::slicevec::SliceVec<.*str.*>\]
pattern:\[len\]            : 0x3 \[Type: unsigned __int64\]
pattern:\[0\]              : "a" \[Type: .*str.*\]
pattern:\[1\]              : "b" \[Type: .*str.*\]
pattern:\[2\]              : "d" \[Type: .*str.*\]

pattern:strings          : \{ len=0x6 \} \[Type: tinyvec::arrayvec::ArrayVec<array\$<.*str.*,7> >\]
pattern:\[<Raw View>\]     \[Type: tinyvec::arrayvec::ArrayVec<array\$<.*str.*,7> >\]
pattern:\[len\]            : 0x6 \[Type: unsigned short\]
pattern:\[capacity\]       : 7
pattern:\[0\]              : "a" \[Type: .*str.*\]
pattern:\[1\]              : "b" \[Type: .*str.*\]
pattern:\[2\]              : "d" \[Type: .*str.*\]
pattern:\[3\]              : "e" \[Type: .*str.*\]
pattern:\[4\]              : "f" \[Type: .*str.*\]
pattern:\[5\]              : "g" \[Type: .*str.*\]
"#
)]
#[inline(never)]
fn test_debugger_visualizer() {
  let mut strings = ArrayVec::<[&str; 7]>::default();
  strings.push("a");
  strings.push("b");
  strings.push("c");
  assert_eq!(["a", "b", "c"], &strings[..]);

  let mut inline_tv = tiny_vec!([i32; 4] => 1, 2, 3);
  assert!(inline_tv.is_inline());

  inline_tv.push(4);
  __break();

  {
    let mut slice_vec = SliceVec::from(strings.as_mut_slice());
    assert_eq!(3, slice_vec.capacity());
    assert_eq!("c", slice_vec.remove(2));
    slice_vec.push("d");
    println!("{:?}", slice_vec);
    __break();

    assert_eq!(["a", "b", "d"], &slice_vec[..]);
  }

  strings.push("e");
  strings.push("f");
  strings.push("g");
  assert_eq!(["a", "b", "d", "e", "f", "g"], &strings[..]);
  __break();
}
