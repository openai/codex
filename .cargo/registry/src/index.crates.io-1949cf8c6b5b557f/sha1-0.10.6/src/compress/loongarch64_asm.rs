//! LoongArch64 assembly backend

use core::arch::asm;

const K: [u32; 4] = [0x5A827999, 0x6ED9EBA1, 0x8F1BBCDC, 0xCA62C1D6];

macro_rules! c {
    ($($l:expr)*) => {
        concat!($($l ,)*)
    };
}

macro_rules! round0a {
    ($a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $i:literal) => {
        c!(
            "ld.w    $t5, $a1, (" $i " * 4);"
            "revb.2h $t5, $t5;"
            "rotri.w $t5, $t5, 16;"
            "add.w " $e ", " $e ", $t5;"
            "st.w    $t5, $sp, (" $i " * 4);"
            "xor     $t5, " $c "," $d ";"
            "and     $t5, $t5, " $b ";"
            "xor     $t5, $t5, " $d ";"
            roundtail!($a, $b, $e, $i, "$a4")
        )
    };
}

macro_rules! scheldule {
    ($i:literal, $e:literal) => {
        c!(
            "ld.w    $t5, $sp, (((" $i " - 3) & 0xF) * 4);"
            "ld.w    $t6, $sp, (((" $i " - 8) & 0xF) * 4);"
            "ld.w    $t7, $sp, (((" $i " - 14) & 0xF) * 4);"
            "ld.w    $t8, $sp, (((" $i " - 16) & 0xF) * 4);"
            "xor     $t5, $t5, $t6;"
            "xor     $t5, $t5, $t7;"
            "xor     $t5, $t5, $t8;"
            "rotri.w $t5, $t5, 31;"
            "add.w " $e "," $e ", $t5;"
            "st.w    $t5, $sp, ((" $i " & 0xF) * 4);"
        )
    };
}

macro_rules! round0b {
    ($a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $i:literal) => {
        c!(
            scheldule!($i, $e)
            "xor     $t5," $c "," $d ";"
            "and     $t5, $t5," $b ";"
            "xor     $t5, $t5," $d ";"
            roundtail!($a, $b, $e, $i, "$a4")
        )
    };
}

macro_rules! round1 {
    ($a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $i:literal) => {
        c!(
            scheldule!($i, $e)
            "xor     $t5," $b "," $c ";"
            "xor     $t5, $t5," $d ";"
            roundtail!($a, $b, $e, $i, "$a5")
        )
    };
}

macro_rules! round2 {
    ($a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $i:literal) => {
        c!(
            scheldule!($i, $e)
            "or      $t5," $c "," $d ";"
            "and     $t5, $t5, " $b ";"
            "and     $t7," $c "," $d ";"
            "or      $t5, $t5, $t7;"
            roundtail!($a, $b, $e, $i, "$a6")
        )
    };
}

macro_rules! round3 {
    ($a:literal, $b:literal, $c:literal, $d:literal, $e:literal, $i:literal) => {
        c!(
            scheldule!($i, $e)
            "xor     $t5," $b "," $c ";"
            "xor     $t5, $t5," $d ";"
            roundtail!($a, $b, $e, $i, "$a7")
        )
    };
}

macro_rules! roundtail {
    ($a:literal, $b:literal, $e:literal, $i:literal, $k:literal) => {
        c!(
            "rotri.w " $b "," $b ", 2;"
            "add.w " $e "," $e ", $t5;"
            "add.w " $e "," $e "," $k ";"
            "rotri.w $t5," $a ", 27;"
            "add.w " $e "," $e ", $t5;"
        )
    };
}

pub fn compress(state: &mut [u32; 5], blocks: &[[u8; 64]]) {
    if blocks.is_empty() {
        return;
    }

    unsafe {
        asm!(
            // Allocate scratch stack space
            "addi.d  $sp, $sp, -64;",

            // Load state
            "ld.w    $t0, $a0, 0",
            "ld.w    $t1, $a0, 4",
            "ld.w    $t2, $a0, 8",
            "ld.w    $t3, $a0, 12",
            "ld.w    $t4, $a0, 16",

            "42:",

            round0a!("$t0", "$t1", "$t2", "$t3", "$t4",  0),
            round0a!("$t4", "$t0", "$t1", "$t2", "$t3",  1),
            round0a!("$t3", "$t4", "$t0", "$t1", "$t2",  2),
            round0a!("$t2", "$t3", "$t4", "$t0", "$t1",  3),
            round0a!("$t1", "$t2", "$t3", "$t4", "$t0",  4),
            round0a!("$t0", "$t1", "$t2", "$t3", "$t4",  5),
            round0a!("$t4", "$t0", "$t1", "$t2", "$t3",  6),
            round0a!("$t3", "$t4", "$t0", "$t1", "$t2",  7),
            round0a!("$t2", "$t3", "$t4", "$t0", "$t1",  8),
            round0a!("$t1", "$t2", "$t3", "$t4", "$t0",  9),
            round0a!("$t0", "$t1", "$t2", "$t3", "$t4", 10),
            round0a!("$t4", "$t0", "$t1", "$t2", "$t3", 11),
            round0a!("$t3", "$t4", "$t0", "$t1", "$t2", 12),
            round0a!("$t2", "$t3", "$t4", "$t0", "$t1", 13),
            round0a!("$t1", "$t2", "$t3", "$t4", "$t0", 14),
            round0a!("$t0", "$t1", "$t2", "$t3", "$t4", 15),
            round0b!("$t4", "$t0", "$t1", "$t2", "$t3", 16),
            round0b!("$t3", "$t4", "$t0", "$t1", "$t2", 17),
            round0b!("$t2", "$t3", "$t4", "$t0", "$t1", 18),
            round0b!("$t1", "$t2", "$t3", "$t4", "$t0", 19),
            round1!("$t0", "$t1", "$t2", "$t3", "$t4", 20),
            round1!("$t4", "$t0", "$t1", "$t2", "$t3", 21),
            round1!("$t3", "$t4", "$t0", "$t1", "$t2", 22),
            round1!("$t2", "$t3", "$t4", "$t0", "$t1", 23),
            round1!("$t1", "$t2", "$t3", "$t4", "$t0", 24),
            round1!("$t0", "$t1", "$t2", "$t3", "$t4", 25),
            round1!("$t4", "$t0", "$t1", "$t2", "$t3", 26),
            round1!("$t3", "$t4", "$t0", "$t1", "$t2", 27),
            round1!("$t2", "$t3", "$t4", "$t0", "$t1", 28),
            round1!("$t1", "$t2", "$t3", "$t4", "$t0", 29),
            round1!("$t0", "$t1", "$t2", "$t3", "$t4", 30),
            round1!("$t4", "$t0", "$t1", "$t2", "$t3", 31),
            round1!("$t3", "$t4", "$t0", "$t1", "$t2", 32),
            round1!("$t2", "$t3", "$t4", "$t0", "$t1", 33),
            round1!("$t1", "$t2", "$t3", "$t4", "$t0", 34),
            round1!("$t0", "$t1", "$t2", "$t3", "$t4", 35),
            round1!("$t4", "$t0", "$t1", "$t2", "$t3", 36),
            round1!("$t3", "$t4", "$t0", "$t1", "$t2", 37),
            round1!("$t2", "$t3", "$t4", "$t0", "$t1", 38),
            round1!("$t1", "$t2", "$t3", "$t4", "$t0", 39),
            round2!("$t0", "$t1", "$t2", "$t3", "$t4", 40),
            round2!("$t4", "$t0", "$t1", "$t2", "$t3", 41),
            round2!("$t3", "$t4", "$t0", "$t1", "$t2", 42),
            round2!("$t2", "$t3", "$t4", "$t0", "$t1", 43),
            round2!("$t1", "$t2", "$t3", "$t4", "$t0", 44),
            round2!("$t0", "$t1", "$t2", "$t3", "$t4", 45),
            round2!("$t4", "$t0", "$t1", "$t2", "$t3", 46),
            round2!("$t3", "$t4", "$t0", "$t1", "$t2", 47),
            round2!("$t2", "$t3", "$t4", "$t0", "$t1", 48),
            round2!("$t1", "$t2", "$t3", "$t4", "$t0", 49),
            round2!("$t0", "$t1", "$t2", "$t3", "$t4", 50),
            round2!("$t4", "$t0", "$t1", "$t2", "$t3", 51),
            round2!("$t3", "$t4", "$t0", "$t1", "$t2", 52),
            round2!("$t2", "$t3", "$t4", "$t0", "$t1", 53),
            round2!("$t1", "$t2", "$t3", "$t4", "$t0", 54),
            round2!("$t0", "$t1", "$t2", "$t3", "$t4", 55),
            round2!("$t4", "$t0", "$t1", "$t2", "$t3", 56),
            round2!("$t3", "$t4", "$t0", "$t1", "$t2", 57),
            round2!("$t2", "$t3", "$t4", "$t0", "$t1", 58),
            round2!("$t1", "$t2", "$t3", "$t4", "$t0", 59),
            round3!("$t0", "$t1", "$t2", "$t3", "$t4", 60),
            round3!("$t4", "$t0", "$t1", "$t2", "$t3", 61),
            round3!("$t3", "$t4", "$t0", "$t1", "$t2", 62),
            round3!("$t2", "$t3", "$t4", "$t0", "$t1", 63),
            round3!("$t1", "$t2", "$t3", "$t4", "$t0", 64),
            round3!("$t0", "$t1", "$t2", "$t3", "$t4", 65),
            round3!("$t4", "$t0", "$t1", "$t2", "$t3", 66),
            round3!("$t3", "$t4", "$t0", "$t1", "$t2", 67),
            round3!("$t2", "$t3", "$t4", "$t0", "$t1", 68),
            round3!("$t1", "$t2", "$t3", "$t4", "$t0", 69),
            round3!("$t0", "$t1", "$t2", "$t3", "$t4", 70),
            round3!("$t4", "$t0", "$t1", "$t2", "$t3", 71),
            round3!("$t3", "$t4", "$t0", "$t1", "$t2", 72),
            round3!("$t2", "$t3", "$t4", "$t0", "$t1", 73),
            round3!("$t1", "$t2", "$t3", "$t4", "$t0", 74),
            round3!("$t0", "$t1", "$t2", "$t3", "$t4", 75),
            round3!("$t4", "$t0", "$t1", "$t2", "$t3", 76),
            round3!("$t3", "$t4", "$t0", "$t1", "$t2", 77),
            round3!("$t2", "$t3", "$t4", "$t0", "$t1", 78),
            round3!("$t1", "$t2", "$t3", "$t4", "$t0", 79),

            // Update state registers
            "ld.w    $t5, $a0, 0",  // a
            "ld.w    $t6, $a0, 4",  // b
            "ld.w    $t7, $a0, 8",  // c
            "ld.w    $t8, $a0, 12", // d
            "add.w   $t0, $t0, $t5",
            "ld.w    $t5, $a0, 16", // e
            "add.w   $t1, $t1, $t6",
            "add.w   $t2, $t2, $t7",
            "add.w   $t3, $t3, $t8",
            "add.w   $t4, $t4, $t5",

            // Save updated state
            "st.w    $t0, $a0, 0",
            "st.w    $t1, $a0, 4",
            "st.w    $t2, $a0, 8",
            "st.w    $t3, $a0, 12",
            "st.w    $t4, $a0, 16",

            // Looping over blocks
            "addi.d  $a1, $a1, 64",
            "addi.d  $a2, $a2, -1",
            "bnez    $a2, 42b",

            // Restore stack register
            "addi.d  $sp, $sp, 64",

            in("$a0") state,
            inout("$a1") blocks.as_ptr() => _,
            inout("$a2") blocks.len() => _,

            in("$a4") K[0],
            in("$a5") K[1],
            in("$a6") K[2],
            in("$a7") K[3],

            // Clobbers
            out("$t0") _,
            out("$t1") _,
            out("$t2") _,
            out("$t3") _,
            out("$t4") _,
            out("$t5") _,
            out("$t6") _,
            out("$t7") _,
            out("$t8") _,

            options(preserves_flags),
        );
    }
}
