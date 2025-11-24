#! /usr/bin/env perl
# Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

my ($s0,$s1,$s2,$s3,$s4,$s5,$s6,$s7)=map("x$_",(7..14));
my ($a8,$a10,$a12,$a14,$a9,$a11,$a13,$a15)=map("x$_",(7..14));
my ($t0,$t1,$t2,$t3)=map("x$_",(3..6));
my ($t4,$t5,$t6,$t7,$t8)=map("x$_",(15..17,19,20));

sub bn_mod_add() {
	my $mod = shift;
$code.=<<___;
	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Addition
	adds $s0,$s0,$s4
	adcs $s1,$s1,$s5
	adcs $s2,$s2,$s6
	adcs $s3,$s3,$s7
	adc $t4,xzr,xzr

	// Load polynomial
	adrp x2,$mod
	add x2,x2,:lo12:$mod
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Backup Addition
	mov $t0,$s0
	mov $t1,$s1
	mov $t2,$s2
	mov $t3,$s3

	// Sub polynomial
	subs $t0,$t0,$s4
	sbcs $t1,$t1,$s5
	sbcs $t2,$t2,$s6
	sbcs $t3,$t3,$s7
	sbcs $t4,$t4,xzr

	// Select based on carry
	csel $s0,$s0,$t0,cc
	csel $s1,$s1,$t1,cc
	csel $s2,$s2,$t2,cc
	csel $s3,$s3,$t3,cc

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]
___
}

sub bn_mod_sub() {
	my $mod = shift;
$code.=<<___;
	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Subtraction
	subs $s0,$s0,$s4
	sbcs $s1,$s1,$s5
	sbcs $s2,$s2,$s6
	sbcs $s3,$s3,$s7
	sbc $t4,xzr,xzr

	// Load polynomial
	adrp x2,$mod
	add x2,x2,:lo12:$mod
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Backup subtraction
	mov $t0,$s0
	mov $t1,$s1
	mov $t2,$s2
	mov $t3,$s3

	// Add polynomial
	adds $t0,$t0,$s4
	adcs $t1,$t1,$s5
	adcs $t2,$t2,$s6
	adcs $t3,$t3,$s7
	tst $t4,$t4

	// Select based on carry
	csel $s0,$s0,$t0,eq
	csel $s1,$s1,$t1,eq
	csel $s2,$s2,$t2,eq
	csel $s3,$s3,$t3,eq

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]
___
}

sub bn_mod_div_by_2() {
	my $mod = shift;
$code.=<<___;
	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]

	// Save the least significant bit
	mov $t0,$s0

	// Right shift 1
	extr $s0,$s1,$s0,#1
	extr $s1,$s2,$s1,#1
	extr $s2,$s3,$s2,#1
	lsr $s3,$s3,#1

	// Load mod
	adrp x2,$mod
	add x2,x2,:lo12:$mod
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Parity check
	tst $t0,#1
	csel $s4,xzr,$s4,eq
	csel $s5,xzr,$s5,eq
	csel $s6,xzr,$s6,eq
	csel $s7,xzr,$s7,eq

	// Add
	adds $s0,$s0,$s4
	adcs $s1,$s1,$s5
	adcs $s2,$s2,$s6
	adc $s3,$s3,$s7

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]
___
}

{
$code.=<<___;
#include "arm_arch.h"
.arch  armv8-a
.rodata

.align	5
// The polynomial p
.Lpoly:
.quad	0xffffffffffffffff,0xffffffff00000000,0xffffffffffffffff,0xfffffffeffffffff
// The order of polynomial n
.Lord:
.quad	0x53bbf40939d54123,0x7203df6b21c6052b,0xffffffffffffffff,0xfffffffeffffffff
// (p + 1) / 2
.Lpoly_div_2:
.quad	0x8000000000000000,0xffffffff80000000,0xffffffffffffffff,0x7fffffff7fffffff
// (n + 1) / 2
.Lord_div_2:
.quad	0xa9ddfa049ceaa092,0xb901efb590e30295,0xffffffffffffffff,0x7fffffff7fffffff

.text

// void bn_rshift1(BN_ULONG *a);
.globl	bn_rshift1
.type	bn_rshift1,%function
.align	5
bn_rshift1:
	AARCH64_VALID_CALL_TARGET
	// Load inputs
	ldp $s0,$s1,[x0]
	ldp $s2,$s3,[x0,#16]

	// Right shift
	extr $s0,$s1,$s0,#1
	extr $s1,$s2,$s1,#1
	extr $s2,$s3,$s2,#1
	lsr $s3,$s3,#1

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]

	ret
.size bn_rshift1,.-bn_rshift1

// void bn_sub(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
.globl	bn_sub
.type	bn_sub,%function
.align	5
bn_sub:
	AARCH64_VALID_CALL_TARGET
	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

	// Subtraction
	subs $s0,$s0,$s4
	sbcs $s1,$s1,$s5
	sbcs $s2,$s2,$s6
	sbc $s3,$s3,$s7

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]

	ret
.size bn_sub,.-bn_sub

// void ecp_sm2p256_div_by_2(BN_ULONG *r,const BN_ULONG *a);
.globl	ecp_sm2p256_div_by_2
.type	ecp_sm2p256_div_by_2,%function
.align	5
ecp_sm2p256_div_by_2:
	AARCH64_VALID_CALL_TARGET
___
	&bn_mod_div_by_2(".Lpoly_div_2");
$code.=<<___;
	ret
.size ecp_sm2p256_div_by_2,.-ecp_sm2p256_div_by_2

// void ecp_sm2p256_div_by_2_mod_ord(BN_ULONG *r,const BN_ULONG *a);
.globl	ecp_sm2p256_div_by_2_mod_ord
.type	ecp_sm2p256_div_by_2_mod_ord,%function
.align	5
ecp_sm2p256_div_by_2_mod_ord:
	AARCH64_VALID_CALL_TARGET
___
	&bn_mod_div_by_2(".Lord_div_2");
$code.=<<___;
	ret
.size ecp_sm2p256_div_by_2_mod_ord,.-ecp_sm2p256_div_by_2_mod_ord

// void ecp_sm2p256_mul_by_3(BN_ULONG *r,const BN_ULONG *a);
.globl	ecp_sm2p256_mul_by_3
.type	ecp_sm2p256_mul_by_3,%function
.align	5
ecp_sm2p256_mul_by_3:
	AARCH64_VALID_CALL_TARGET
	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]

	// 2*a
	adds $s0,$s0,$s0
	adcs $s1,$s1,$s1
	adcs $s2,$s2,$s2
	adcs $s3,$s3,$s3
	adcs $t4,xzr,xzr

	mov $t0,$s0
	mov $t1,$s1
	mov $t2,$s2
	mov $t3,$s3

	// Sub polynomial
	adrp x2,.Lpoly
	add x2,x2,:lo12:.Lpoly
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]
	subs $s0,$s0,$s4
	sbcs $s1,$s1,$s5
	sbcs $s2,$s2,$s6
	sbcs $s3,$s3,$s7
	sbcs $t4,$t4,xzr

	csel $s0,$s0,$t0,cs
	csel $s1,$s1,$t1,cs
	csel $s2,$s2,$t2,cs
	csel $s3,$s3,$t3,cs
	eor $t4,$t4,$t4

	// 3*a
	ldp $s4,$s5,[x1]
	ldp $s6,$s7,[x1,#16]
	adds $s0,$s0,$s4
	adcs $s1,$s1,$s5
	adcs $s2,$s2,$s6
	adcs $s3,$s3,$s7
	adcs $t4,xzr,xzr

	mov $t0,$s0
	mov $t1,$s1
	mov $t2,$s2
	mov $t3,$s3

	// Sub polynomial
	adrp x2,.Lpoly
	add x2,x2,:lo12:.Lpoly
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]
	subs $s0,$s0,$s4
	sbcs $s1,$s1,$s5
	sbcs $s2,$s2,$s6
	sbcs $s3,$s3,$s7
	sbcs $t4,$t4,xzr

	csel $s0,$s0,$t0,cs
	csel $s1,$s1,$t1,cs
	csel $s2,$s2,$t2,cs
	csel $s3,$s3,$t3,cs

	// Store results
	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]

	ret
.size ecp_sm2p256_mul_by_3,.-ecp_sm2p256_mul_by_3

// void ecp_sm2p256_add(BN_ULONG *r,const BN_ULONG *a,const BN_ULONG *b);
.globl	ecp_sm2p256_add
.type	ecp_sm2p256_add,%function
.align	5
ecp_sm2p256_add:
	AARCH64_VALID_CALL_TARGET
___
	&bn_mod_add(".Lpoly");
$code.=<<___;
	ret
.size ecp_sm2p256_add,.-ecp_sm2p256_add

// void ecp_sm2p256_sub(BN_ULONG *r,const BN_ULONG *a,const BN_ULONG *b);
.globl	ecp_sm2p256_sub
.type	ecp_sm2p256_sub,%function
.align	5
ecp_sm2p256_sub:
	AARCH64_VALID_CALL_TARGET
___
	&bn_mod_sub(".Lpoly");
$code.=<<___;
	ret
.size ecp_sm2p256_sub,.-ecp_sm2p256_sub

// void ecp_sm2p256_sub_mod_ord(BN_ULONG *r,const BN_ULONG *a,const BN_ULONG *b);
.globl	ecp_sm2p256_sub_mod_ord
.type	ecp_sm2p256_sub_mod_ord,%function
.align	5
ecp_sm2p256_sub_mod_ord:
	AARCH64_VALID_CALL_TARGET
___
	&bn_mod_sub(".Lord");
$code.=<<___;
	ret
.size ecp_sm2p256_sub_mod_ord,.-ecp_sm2p256_sub_mod_ord

.macro RDC
	// a = |  s7   | ... | s0  |, where si are 64-bit quantities
	//   = |a15|a14| ... |a1|a0|, where ai are 32-bit quantities
	// |    s7     |    s6     |    s5     |    s4     |
	// | a15 | a14 | a13 | a12 | a11 | a10 | a9  | a8  |
	// |    s3     |    s2     |    s1     |    s0     |
	// | a7  | a6  | a5  | a4  | a3  | a2  | a1  | a0  |
	// =================================================
	// | a8  | a11 | a10 | a9  | a8  |   0 |    s4     | (+)
	// | a9  | a15 |    s6     | a11 |   0 | a10 | a9  | (+)
	// | a10 |   0 | a14 | a13 | a12 |   0 |    s5     | (+)
	// | a11 |   0 |    s7     | a13 |   0 | a12 | a11 | (+)
	// | a12 |   0 |    s7     | a13 |   0 |    s6     | (+)
	// | a12 |   0 |   0 | a15 | a14 |   0 | a14 | a13 | (+)
	// | a13 |   0 |   0 |   0 | a15 |   0 | a14 | a13 | (+)
	// | a13 |   0 |   0 |   0 |   0 |   0 |    s7     | (+)
	// | a14 |   0 |   0 |   0 |   0 |   0 |    s7     | (+)
	// | a14 |   0 |   0 |   0 |   0 |   0 |   0 | a15 | (+)
	// | a15 |   0 |   0 |   0 |   0 |   0 |   0 | a15 | (+)
	// | a15 |   0 |   0 |   0 |   0 |   0 |   0 |   0 | (+)
	// |    s7     |   0 |   0 |   0 |   0 |   0 |   0 | (+)
	// |   0 |   0 |   0 |   0 |   0 | a8  |   0 |   0 | (-)
	// |   0 |   0 |   0 |   0 |   0 | a9  |   0 |   0 | (-)
	// |   0 |   0 |   0 |   0 |   0 | a13 |   0 |   0 | (-)
	// |   0 |   0 |   0 |   0 |   0 | a14 |   0 |   0 | (-)
	// | U[7]| U[6]| U[5]| U[4]| U[3]| U[2]| U[1]| U[0]|
	// |    V[3]   |    V[2]   |    V[1]   |    V[0]   |

	// 1. 64-bit addition
	// t2=s6+s7+s7
	adds $t2,$s6,$s7
	adcs $t1,xzr,xzr
	adds $t2,$t2,$s7
	adcs $t1,$t1,xzr
	// t3=s4+s5+t2
	adds $t3,$s4,$t2
	adcs $t4,$t1,xzr
	adds $t3,$t3,$s5
	adcs $t4,$t4,xzr
	// sum
	adds $s0,$s0,$t3
	adcs $s1,$s1,$t4
	adcs $s2,$s2,$t2
	adcs $s3,$s3,$s7
	adcs $t0,xzr,xzr
	adds $s3,$s3,$t1
	adcs $t0,$t0,xzr

	stp $s0,$s1,[sp,#32]
	stp $s2,$s3,[sp,#48]

	// 2. 64-bit to 32-bit spread
	mov $t1,#0xffffffff
	mov $s0,$s4
	mov $s1,$s5
	mov $s2,$s6
	mov $s3,$s7
	and $s0,$s0,$t1 // a8
	and $s1,$s1,$t1 // a10
	and $s2,$s2,$t1 // a12
	and $s3,$s3,$t1 // a14
	lsr $s4,$s4,#32 // a9
	lsr $s5,$s5,#32 // a11
	lsr $s6,$s6,#32 // a13
	lsr $s7,$s7,#32 // a15

	// 3. 32-bit addition
	add $t1,$a14,$a12  // t1 <- a12 + a14
	add $t2,$a15,$a13  // t2 <- a13 + a15
	add $t3,$a8,$a9    // t3 <- a8 + a9
	add $t4,$a14,$a10  // t4 <- a10 + a14
	add $a15,$a15,$a11 // a15 <- a11 + a15
	add $a12,$t2,$t1   // a12 <- a12 + a13 + a14 + a15
	add $a10,$a10,$a12 // a10 <- a10 + a12 + a13 + a14 + a15
	add $a10,$a10,$a12 // a10 <- a10 + 2*(a12 + a13 + a14 + a15)
	add $a10,$a10,$t3  // a10 <- a8 + a9 + a10 + 2*(a12 + a13 + a14 + a15)
	add $a10,$a10,$a11 // a10 <- a8 + a9 + a10 + a11 + 2*(a12 + a13 + a14 + a15)
	add $a12,$a12,$a13 // a12 <- a12 + 2*a13 + a14 + a15
	add $a12,$a12,$a11 // a12 <- a11 + a12 + 2*a13 + a14 + a15
	add $a12,$a12,$a8  // a12 <- a8 + a11 + a12 + 2*a13 + a14 + a15
	add $t3,$t3,$a14   // t3 <- a8 + a9 + a14
	add $t3,$t3,$a13   // t3 <- a8 + a9 + a13 + a14
	add $a9,$a9,$t2    // a9 <- a9 + a13 + a15
	add $a11,$a11,$a9  // a11 <- a9 + a11 + a13 + a15
	add $a11,$a11,$t2  // a11 <- a9 + a11 + 2*(a13 + a15)
	add $t1,$t1,$t4    // t1 <- a10 + a12 + 2*a14

	// U[0]  s5	a9 + a11 + 2*(a13 + a15)
	// U[1]  t1	a10 + a12 + 2*a14
	// U[2] -t3	a8 + a9 + a13 + a14
	// U[3]  s2	a8 + a11 + a12 + 2*a13 + a14 + a15
	// U[4]  s4	a9 + a13 + a15
	// U[5]  t4	a10 + a14
	// U[6]  s7	a11 + a15
	// U[7]  s1	a8 + a9 + a10 + a11 + 2*(a12 + a13 + a14 + a15)

	// 4. 32-bit to 64-bit
	lsl $s0,$t1,#32
	extr $t1,$s2,$t1,#32
	extr $s2,$t4,$s2,#32
	extr $t4,$s1,$t4,#32
	lsr $s1,$s1,#32

	// 5. 64-bit addition
	adds $s5,$s5,$s0
	adcs $t1,$t1,xzr
	adcs $s4,$s4,$s2
	adcs $s7,$s7,$t4
	adcs $t0,$t0,$s1

	// V[0]	s5
	// V[1]	t1
	// V[2]	s4
	// V[3]	s7
	// carry	t0
	// sub	t3

	// 5. Process s0-s3
	ldp $s0,$s1,[sp,#32]
	ldp $s2,$s3,[sp,#48]
	// add with V0-V3
	adds $s0,$s0,$s5
	adcs $s1,$s1,$t1
	adcs $s2,$s2,$s4
	adcs $s3,$s3,$s7
	adcs $t0,$t0,xzr
	// sub with t3
	subs $s1,$s1,$t3
	sbcs $s2,$s2,xzr
	sbcs $s3,$s3,xzr
	sbcs $t0,$t0,xzr

	// 6. MOD
	// First Mod
	lsl $t1,$t0,#32
	subs $t2,$t1,$t0

	adds $s0,$s0,$t0
	adcs $s1,$s1,$t2
	adcs $s2,$s2,xzr
	adcs $s3,$s3,$t1

	// Last Mod
	// return y - p if y > p else y
	mov $s4,$s0
	mov $s5,$s1
	mov $s6,$s2
	mov $s7,$s3

	adrp $t0,.Lpoly
	add $t0,$t0,:lo12:.Lpoly
	ldp $t1,$t2,[$t0]
	ldp $t3,$t4,[$t0,#16]

	adcs $t5,xzr,xzr

	subs $s0,$s0,$t1
	sbcs $s1,$s1,$t2
	sbcs $s2,$s2,$t3
	sbcs $s3,$s3,$t4
	sbcs $t5,$t5,xzr

	csel $s0,$s0,$s4,cs
	csel $s1,$s1,$s5,cs
	csel $s2,$s2,$s6,cs
	csel $s3,$s3,$s7,cs

.endm

// void ecp_sm2p256_mul(BN_ULONG *r, const BN_ULONG *a, const BN_ULONG *b);
.globl	ecp_sm2p256_mul
.type	ecp_sm2p256_mul,%function
.align	5
ecp_sm2p256_mul:
	AARCH64_SIGN_LINK_REGISTER
	// Store scalar registers
	stp x29,x30,[sp,#-80]!
	add x29,sp,#0
	stp x16,x17,[sp,#16]
	stp x19,x20,[sp,#64]

	// Load inputs
	ldp $s0,$s1,[x1]
	ldp $s2,$s3,[x1,#16]
	ldp $s4,$s5,[x2]
	ldp $s6,$s7,[x2,#16]

// ### multiplication ###
	// ========================
	//             s3 s2 s1 s0
	// *           s7 s6 s5 s4
	// ------------------------
	// +           s0 s0 s0 s0
	//              *  *  *  *
	//             s7 s6 s5 s4
	//          s1 s1 s1 s1
	//           *  *  *  *
	//          s7 s6 s5 s4
	//       s2 s2 s2 s2
	//        *  *  *  *
	//       s7 s6 s5 s4
	//    s3 s3 s3 s3
	//     *  *  *  *
	//    s7 s6 s5 s4
	// ------------------------
	// s7 s6 s5 s4 s3 s2 s1 s0
	// ========================

// ### s0*s4 ###
	mul $t5,$s0,$s4
	umulh $t2,$s0,$s4

// ### s1*s4 + s0*s5 ###
	mul $t0,$s1,$s4
	umulh $t1,$s1,$s4
	adds $t2,$t2,$t0
	adcs $t3,$t1,xzr

	mul $t0,$s0,$s5
	umulh $t1,$s0,$s5
	adds $t2,$t2,$t0
	adcs $t3,$t3,$t1
	adcs $t4,xzr,xzr

// ### s2*s4 + s1*s5 + s0*s6 ###
	mul $t0,$s2,$s4
	umulh $t1,$s2,$s4
	adds $t3,$t3,$t0
	adcs $t4,$t4,$t1

	mul $t0,$s1,$s5
	umulh $t1,$s1,$s5
	adds $t3,$t3,$t0
	adcs $t4,$t4,$t1
	adcs $t6,xzr,xzr

	mul $t0,$s0,$s6
	umulh $t1,$s0,$s6
	adds $t3,$t3,$t0
	adcs $t4,$t4,$t1
	adcs $t6,$t6,xzr

// ### s3*s4 + s2*s5 + s1*s6 + s0*s7 ###
	mul $t0,$s3,$s4
	umulh $t1,$s3,$s4
	adds $t4,$t4,$t0
	adcs $t6,$t6,$t1
	adcs $t7,xzr,xzr

	mul $t0,$s2,$s5
	umulh $t1,$s2,$s5
	adds $t4,$t4,$t0
	adcs $t6,$t6,$t1
	adcs $t7,$t7,xzr

	mul $t0,$s1,$s6
	umulh $t1,$s1,$s6
	adds $t4,$t4,$t0
	adcs $t6,$t6,$t1
	adcs $t7,$t7,xzr

	mul $t0,$s0,$s7
	umulh $t1,$s0,$s7
	adds $t4,$t4,$t0
	adcs $t6,$t6,$t1
	adcs $t7,$t7,xzr

// ### s3*s5 + s2*s6 + s1*s7 ###
	mul $t0,$s3,$s5
	umulh $t1,$s3,$s5
	adds $t6,$t6,$t0
	adcs $t7,$t7,$t1
	adcs $t8,xzr,xzr

	mul $t0,$s2,$s6
	umulh $t1,$s2,$s6
	adds $t6,$t6,$t0
	adcs $t7,$t7,$t1
	adcs $t8,$t8,xzr

	mul $t0,$s1,$s7
	umulh $t1,$s1,$s7
	adds $s4,$t6,$t0
	adcs $t7,$t7,$t1
	adcs $t8,$t8,xzr

// ### s3*s6 + s2*s7 ###
	mul $t0,$s3,$s6
	umulh $t1,$s3,$s6
	adds $t7,$t7,$t0
	adcs $t8,$t8,$t1
	adcs $t6,xzr,xzr

	mul $t0,$s2,$s7
	umulh $t1,$s2,$s7
	adds $s5,$t7,$t0
	adcs $t8,$t8,$t1
	adcs $t6,$t6,xzr

// ### s3*s7 ###
	mul $t0,$s3,$s7
	umulh $t1,$s3,$s7
	adds $s6,$t8,$t0
	adcs $s7,$t6,$t1

	mov $s0,$t5
	mov $s1,$t2
	mov $s2,$t3
	mov $s3,$t4

	// result of mul: s7 s6 s5 s4 s3 s2 s1 s0

// ### Reduction ###
	RDC

	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]

	// Restore scalar registers
	ldp x16,x17,[sp,#16]
	ldp x19,x20,[sp,#64]
	ldp x29,x30,[sp],#80

	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size ecp_sm2p256_mul,.-ecp_sm2p256_mul

// void ecp_sm2p256_sqr(BN_ULONG *r, const BN_ULONG *a);
.globl	ecp_sm2p256_sqr
.type	ecp_sm2p256_sqr,%function
.align	5

ecp_sm2p256_sqr:
	AARCH64_SIGN_LINK_REGISTER
	// Store scalar registers
	stp x29,x30,[sp,#-80]!
	add x29,sp,#0
	stp x16,x17,[sp,#16]
	stp x19,x20,[sp,#64]

	// Load inputs
	ldp $s4,$s5,[x1]
	ldp $s6,$s7,[x1,#16]

// ### square ###
	// ========================
	//             s7 s6 s5 s4
	// *           s7 s6 s5 s4
	// ------------------------
	// +           s4 s4 s4 s4
	//              *  *  *  *
	//             s7 s6 s5 s4
	//          s5 s5 s5 s5
	//           *  *  *  *
	//          s7 s6 s5 s4
	//       s6 s6 s6 s6
	//        *  *  *  *
	//       s7 s6 s5 s4
	//    s7 s7 s7 s7
	//     *  *  *  *
	//    s7 s6 s5 s4
	// ------------------------
	// s7 s6 s5 s4 s3 s2 s1 s0
	// ========================

// ### s4*s5 ###
	mul $s1,$s4,$s5
	umulh $s2,$s4,$s5

// ### s4*s6 ###
	mul $t0,$s6,$s4
	umulh $s3,$s6,$s4
	adds $s2,$s2,$t0
	adcs $s3,$s3,xzr

// ### s4*s7 + s5*s6 ###
	mul $t0,$s7,$s4
	umulh $t1,$s7,$s4
	adds $s3,$s3,$t0
	adcs $s0,$t1,xzr

	mul $t0,$s6,$s5
	umulh $t1,$s6,$s5
	adds $s3,$s3,$t0
	adcs $s0,$s0,$t1
	adcs $t2,xzr,xzr

// ### s5*s7 ###
	mul $t0,$s7,$s5
	umulh $t1,$s7,$s5
	adds $s0,$s0,$t0
	adcs $t2,$t2,$t1

// ### s6*s7 ###
	mul $t0,$s7,$s6
	umulh $t1,$s7,$s6
	adds $t2,$t2,$t0
	adcs $t3,$t1,xzr

// ### 2*(t3,t2,s0,s3,s2,s1) ###
	adds $s1,$s1,$s1
	adcs $s2,$s2,$s2
	adcs $s3,$s3,$s3
	adcs $s0,$s0,$s0
	adcs $t2,$t2,$t2
	adcs $t3,$t3,$t3
	adcs $t4,xzr,xzr

// ### s4*s4 ###
	mul $t5,$s4,$s4
	umulh $t6,$s4,$s4

// ### s5*s5 ###
	mul $s4,$s5,$s5
	umulh $s5,$s5,$s5

// ### s6*s6 ###
	mul $t0,$s6,$s6
	umulh $t1,$s6,$s6

// ### s7*s7 ###
	mul $t7,$s7,$s7
	umulh $t8,$s7,$s7

	adds $s1,$s1,$t6
	adcs $s2,$s2,$s4
	adcs $s3,$s3,$s5
	adcs $s0,$s0,$t0
	adcs $t2,$t2,$t1
	adcs $t3,$t3,$t7
	adcs $t4,$t4,$t8

	mov $s4,$s0
	mov $s0,$t5
	mov $s5,$t2
	mov $s6,$t3
	mov $s7,$t4

	// result of mul: s7 s6 s5 s4 s3 s2 s1 s0

// ### Reduction ###
	RDC

	stp $s0,$s1,[x0]
	stp $s2,$s3,[x0,#16]

	// Restore scalar registers
	ldp x16,x17,[sp,#16]
	ldp x19,x20,[sp,#64]
	ldp x29,x30,[sp],#80

	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size ecp_sm2p256_sqr,.-ecp_sm2p256_sqr
___
}

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/ge;

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT: $!";		# enforce flush
