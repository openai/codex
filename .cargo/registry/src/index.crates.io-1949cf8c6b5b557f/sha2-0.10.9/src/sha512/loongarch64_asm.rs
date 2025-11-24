//! LoongArch64 assembly backend

macro_rules! c {
    ($($l:expr)*) => {
        concat!($($l ,)*)
    };
}

macro_rules! rounda {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            "ld.d    $a5, $a1, (" $i " * 8);"
            "revb.d  $a5, $a5;"
            roundtail!($i, $a, $b, $c, $d, $e, $f, $g, $h)
        )
    };
}

macro_rules! roundb {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            "ld.d    $a4, $sp, (((" $i " - 15) & 0xF) * 8);"
            "ld.d    $a5, $sp, (((" $i " - 16) & 0xF) * 8);"
            "ld.d    $a6, $sp, (((" $i " -  7) & 0xF) * 8);"
            "add.d   $a5, $a5, $a6;"
            "rotri.d $a6, $a4, 8;"
            "srli.d  $a7, $a4, 7;"
            "rotri.d $a4, $a4, 1;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.d   $a5, $a5, $a4;"
            "ld.d    $a4, $sp, (((" $i " -  2) & 0xF) * 8);"
            "rotri.d $a6, $a4, 61;"
            "srli.d  $a7, $a4,  6;"
            "rotri.d $a4, $a4, 19;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.d   $a5, $a5, $a4;"
            roundtail!($i, $a, $b, $c, $d, $e, $f, $g, $h)
        )
    };
}

macro_rules! roundtail {
    ($i:literal, $a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $f:literal, $g:literal, $h:literal) => {
        c!(
            // Part 0
            "rotri.d $a6, " $e ", 18;"
            "rotri.d $a7, " $e ", 41;"
            "rotri.d $a4, " $e ", 14;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "xor     $a6, " $g ", " $f ";"
            "ld.d    $a7, $a3, " $i " * 8;"
            "and     $a6, $a6, " $e ";"
            "xor     $a6, $a6, " $g ";"
            "add.d   $a4, $a4, $a6;"
            "add.d   $a4, $a4, $a7;"
            "add.d   " $h ", " $h ", $a5;"
            "add.d   " $h ", " $h ", $a4;"
            // Part 1
            "add.d   " $d ", " $d ", " $h ";"
            // Part 2
            "rotri.d $a6, " $a ", 39;"
            "rotri.d $a7, " $a ", 34;"
            "rotri.d $a4, " $a ", 28;"
            "xor     $a6, $a6, $a7;"
            "xor     $a4, $a4, $a6;"
            "add.d   " $h ", " $h ", $a4;"
            "or      $a4, " $c ", " $b ";"
            "and     $a6, " $c ", " $b ";"
            "and     $a4, $a4, " $a ";"
            "or      $a4, $a4, $a6;"
            "add.d   " $h ", " $h ", $a4;"
            "st.d    $a5, $sp, ((" $i " & 0xF) * 8);"
        )
    };
}

pub fn compress(state: &mut [u64; 8], blocks: &[[u8; 128]]) {
    if blocks.is_empty() {
        return;
    }

    unsafe {
        core::arch::asm!(
            // Allocate scratch stack space
            "addi.d  $sp, $sp, -128;",

            // Load state
            "ld.d    $t0, $a0, 0",
            "ld.d    $t1, $a0, 8",
            "ld.d    $t2, $a0, 16",
            "ld.d    $t3, $a0, 24",
            "ld.d    $t4, $a0, 32",
            "ld.d    $t5, $a0, 40",
            "ld.d    $t6, $a0, 48",
            "ld.d    $t7, $a0, 56",

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
            roundb!(64, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(65, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(66, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(67, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(68, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(69, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(70, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(71, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),
            roundb!(72, "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7"),
            roundb!(73, "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6"),
            roundb!(74, "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4", "$t5"),
            roundb!(75, "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3", "$t4"),
            roundb!(76, "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2", "$t3"),
            roundb!(77, "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1" , "$t2"),
            roundb!(78, "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0" , "$t1"),
            roundb!(79, "$t1" , "$t2", "$t3", "$t4", "$t5", "$t6", "$t7", "$t0"),

            // Update state registers
            "ld.d    $a4, $a0, 0",  // a
            "ld.d    $a5, $a0, 8",  // b
            "ld.d    $a6, $a0, 16",  // c
            "ld.d    $a7, $a0, 24", // d
            "add.d   $t0, $t0, $a4",
            "add.d   $t1, $t1, $a5",
            "add.d   $t2, $t2, $a6",
            "add.d   $t3, $t3, $a7",
            "ld.d    $a4, $a0, 32", // e
            "ld.d    $a5, $a0, 40", // f
            "ld.d    $a6, $a0, 48", // g
            "ld.d    $a7, $a0, 56", // h
            "add.d   $t4, $t4, $a4",
            "add.d   $t5, $t5, $a5",
            "add.d   $t6, $t6, $a6",
            "add.d   $t7, $t7, $a7",

            // Save updated state
            "st.d    $t0, $a0, 0",
            "st.d    $t1, $a0, 8",
            "st.d    $t2, $a0, 16",
            "st.d    $t3, $a0, 24",
            "st.d    $t4, $a0, 32",
            "st.d    $t5, $a0, 40",
            "st.d    $t6, $a0, 48",
            "st.d    $t7, $a0, 56",

            // Looping over blocks
            "addi.d  $a1, $a1, 128",
            "addi.d  $a2, $a2, -1",
            "bnez    $a2, 42b",

            // Restore stack register
            "addi.d  $sp, $sp, 128",

            in("$a0") state,
            inout("$a1") blocks.as_ptr() => _,
            inout("$a2") blocks.len() => _,
            in("$a3") crate::consts::K64.as_ptr(),

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
