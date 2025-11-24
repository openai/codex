//! LoongArch64 assembly backend

macro_rules! c {
    ($($l:expr)*) => {
        concat!($($l ,)*)
    };
}

macro_rules! rounda {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            "ld.w    $a5, $a1, (" $i " * 4);"
            "revb.2h $a5, $a5;"
            "rotri.w $a5, $a5, 16;"
            roundtail!($i, $a, $b, $c, $d, $e, $f, $g, $h)
        )
    };
}

macro_rules! roundb {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            "ld.w    $a4, $sp, (((" $i " - 15) & 0xF) * 4);"
            "ld.w    $a5, $sp, (((" $i " - 16) & 0xF) * 4);"
            "ld.w    $a6, $sp, (((" $i " -  7) & 0xF) * 4);"
            "add.w   $a5, $a5, $a6;"
            "rotri.w $a6, $a4, 18;"
            "srli.w  $a7, $a4, 3;"
            "rotri.w $a4, $a4, 7;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.w   $a5, $a5, $a4;"
            "ld.w    $a4, $sp, (((" $i " -  2) & 0xF) * 4);"
            "rotri.w $a6, $a4, 19;"
            "srli.w  $a7, $a4, 10;"
            "rotri.w $a4, $a4, 17;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.w   $a5, $a5, $a4;"
            roundtail!($i, $a, $b, $c, $d, $e, $f, $g, $h)
        )
    };
}

macro_rules! roundtail {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            // Part 0
            "rotri.w $a6, " $e ", 11;"
            "rotri.w $a7, " $e ", 25;"
            "rotri.w $a4, " $e ", 6;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "xor     $a6, " $g ", " $f ";"
            "ld.w    $a7, $a3, " $i " * 4;"
            "and     $a6, $a6, " $e ";"
            "xor     $a6, $a6, " $g ";"
            "add.w   $a4, $a4, $a6;"
            "add.w   $a4, $a4, $a7;"
            "add.w   " $h ", " $h ", $a5;"
            "add.w   " $h ", " $h ", $a4;"
            // Part 1
            "add.w   " $d ", " $d ", " $h ";"
            // Part 2
            "rotri.w $a6, " $a ", 13;"
            "rotri.w $a7, " $a ", 22;"
            "rotri.w $a4, " $a ", 2;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.w   " $h ", " $h ", $a4;"
            "or      $a4, " $c ", " $b ";"
            "and     $a6, " $c ", " $b ";"
            "and     $a4, $a4, " $a ";"
            "or      $a4, $a4, $a6;"
            "add.w   " $h ", " $h ", $a4;"
            "st.w    $a5, $sp, ((" $i " & 0xF) * 4);"
        )
    };
}

pub fn compress(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    if blocks.is_empty() {
        return;
    }

    unsafe {
        core::arch::asm!(
            // Allocate scratch stack space
            "addi.d  $sp, $sp, -64;",

            // Load state
            "ld.w    $t0, $a0, 0",
            "ld.w    $t1, $a0, 4",
            "ld.w    $t2, $a0, 8",
            "ld.w    $t3, $a0, 12",
            "ld.w    $t4, $a0, 16",
            "ld.w    $t5, $a0, 20",
            "ld.w    $t6, $a0, 24",
            "ld.w    $t7, $a0, 28",

            "42:",

            // Do 64 rounds of hashing
            rounda!( 0, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            rounda!( 1, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            rounda!( 2, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            rounda!( 3, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            rounda!( 4, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            rounda!( 5, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            rounda!( 6, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            rounda!( 7, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            rounda!( 8, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            rounda!( 9, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            rounda!(10, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            rounda!(11, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            rounda!(12, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            rounda!(13, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            rounda!(14, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            rounda!(15, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(16, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(17, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(18, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(19, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(20, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(21, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(22, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(23, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(24, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(25, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(26, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(27, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(28, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(29, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(30, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(31, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(32, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(33, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(34, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(35, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(36, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(37, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(38, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(39, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(40, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(41, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(42, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(43, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(44, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(45, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(46, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(47, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(48, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(49, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(50, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(51, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(52, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(53, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(54, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(55, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(56, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(57, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(58, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(59, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(60, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(61, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(62, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(63, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),

            // Update state registers
            "ld.w    $a4, $a0, 0",  // a
            "ld.w    $a5, $a0, 4",  // b
            "ld.w    $a6, $a0, 8",  // c
            "ld.w    $a7, $a0, 12", // d
            "add.w   $t0, $t0, $a4",
            "add.w   $t1, $t1, $a5",
            "add.w   $t2, $t2, $a6",
            "add.w   $t3, $t3, $a7",
            "ld.w    $a4, $a0, 16", // e
            "ld.w    $a5, $a0, 20", // f
            "ld.w    $a6, $a0, 24", // g
            "ld.w    $a7, $a0, 28", // h
            "add.w   $t4, $t4, $a4",
            "add.w   $t5, $t5, $a5",
            "add.w   $t6, $t6, $a6",
            "add.w   $t7, $t7, $a7",

            // Save updated state
            "st.w    $t0, $a0, 0",
            "st.w    $t1, $a0, 4",
            "st.w    $t2, $a0, 8",
            "st.w    $t3, $a0, 12",
            "st.w    $t4, $a0, 16",
            "st.w    $t5, $a0, 20",
            "st.w    $t6, $a0, 24",
            "st.w    $t7, $a0, 28",

            // Looping over blocks
            "addi.d  $a1, $a1, 64",
            "addi.d  $a2, $a2, -1",
            "bnez    $a2, 42b",

            // Restore stack register
            "addi.d  $sp, $sp, 64",

            in("$a0") state,
            inout("$a1") blocks.as_ptr() => _,
            inout("$a2") blocks.len() => _,
            in("$a3") crate::consts::K32.as_ptr(),

            // Clobbers
            out("$a4") _,
            out("$a5") _,
            out("$a6") _,
            out("$a7") _,
            out("$t0") _,
            out("$t1") _,
            out("$t2") _,
            out("$t3") _,
            out("$t4") _,
            out("$t5") _,
            out("$t6") _,
            out("$t7") _,

            options(preserves_flags),
        );
    }
}
