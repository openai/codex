use debugger_test::debugger_test;
use smallvec::{smallvec, SmallVec};

#[inline(never)]
fn __break() {}

#[debugger_test(
    debugger = "cdb",
    commands = r#"
.nvlist
dx sv

g

dx sv

g

dx sv
"#,
    expected_statements = r#"
sv               : { len=0x2 is_inline=true } [Type: smallvec::SmallVec<array$<i32,4> >]
    [<Raw View>]     [Type: smallvec::SmallVec<array$<i32,4> >]
    [capacity]       : 4
    [len]            : 0x2 [Type: unsigned __int64]
    [0]              : 1 [Type: int]
    [1]              : 2 [Type: int]

sv               : { len=0x5 is_inline=false } [Type: smallvec::SmallVec<array$<i32,4> >]
    [<Raw View>]     [Type: smallvec::SmallVec<array$<i32,4> >]
    [capacity]       : 0x8 [Type: unsigned __int64]
    [len]            : 0x5 [Type: unsigned __int64]
    [0]              : 5 [Type: int]
    [1]              : 2 [Type: int]
    [2]              : 3 [Type: int]
    [3]              : 4 [Type: int]
    [4]              : 5 [Type: int]

sv               : { len=0x5 is_inline=false } [Type: smallvec::SmallVec<array$<i32,4> >]
    [<Raw View>]     [Type: smallvec::SmallVec<array$<i32,4> >]
    [capacity]       : 0x8 [Type: unsigned __int64]
    [len]            : 0x5 [Type: unsigned __int64]
    [0]              : 2 [Type: int]
    [1]              : 3 [Type: int]
    [2]              : 4 [Type: int]
    [3]              : 5 [Type: int]
    [4]              : 5 [Type: int]
"#
)]
#[inline(never)]
fn test_debugger_visualizer() {
    // This SmallVec can hold up to 4 items on the stack:
    let mut sv: SmallVec<[i32; 4]> = smallvec![1, 2];
    __break();

    // Overfill the SmallVec to move its contents to the heap
    for i in 3..6 {
        sv.push(i);
    }

    // Update the contents of the first value of the SmallVec.
    sv[0] = sv[1] + sv[2];
    __break();

    // Sort the SmallVec in place.
    sv.sort();
    __break();
}
