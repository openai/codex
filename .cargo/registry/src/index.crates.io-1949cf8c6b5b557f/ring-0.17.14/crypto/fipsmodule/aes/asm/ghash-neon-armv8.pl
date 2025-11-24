#! /usr/bin/env perl
# Copyright 2010-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project.
# ====================================================================

# This file was adapted to AArch64 from the 32-bit version in ghash-armv4.pl. It
# implements the multiplication algorithm described in:
#
# Câmara, D.; Gouvêa, C. P. L.; López, J. & Dahab, R.: Fast Software
# Polynomial Multiplication on ARM Processors using the NEON Engine.
#
# http://conradoplg.cryptoland.net/files/2010/12/mocrysen13.pdf
#
# The main distinction to keep in mind between 32-bit NEON and AArch64 SIMD is
# AArch64 cannot compute over the upper halves of SIMD registers. In 32-bit
# NEON, the low and high halves of the 128-bit register q0 are accessible as
# 64-bit registers d0 and d1, respectively. In AArch64, dN is the lower half of
# vN. Where the 32-bit version would use the upper half, this file must keep
# halves in separate registers.
#
# The other distinction is in syntax. 32-bit NEON embeds lane information in the
# instruction name, while AArch64 uses suffixes on the registers. For instance,
# left-shifting 64-bit lanes of a SIMD register in 32-bit would be written:
#
#     vshl.i64 q0, q0, #1
#
# in 64-bit, it would be written:
#
#     shl v0.2d, v0.2d, #1
#
# See Programmer's Guide for ARMv8-A, section 7 for details.
# http://infocenter.arm.com/help/topic/com.arm.doc.den0024a/DEN0024A_v8_architecture_PG.pdf
#
# Finally, note the 8-bit and 64-bit polynomial multipliers in AArch64 differ
# only by suffix. pmull vR.8h, vA.8b, vB.8b multiplies eight 8-bit polynomials
# and is always available. pmull vR.1q, vA.1d, vB.1d multiplies a 64-bit
# polynomial and is conditioned on the PMULL extension. This file emulates the
# latter with the former.

use strict;

my $flavour = shift;
my $output;
if ($flavour=~/\w[\w\-]*\.\w+$/) { $output=$flavour; undef $flavour; }
else { while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {} }

if ($flavour && $flavour ne "void") {
    $0 =~ m/(.*[\/\\])[^\/\\]+$/;
    my $dir = $1;
    my $xlate;
    ( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
    ( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
    die "can't locate arm-xlate.pl";

    open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
    *STDOUT=*OUT;
} else {
    open OUT,">$output";
    *STDOUT=*OUT;
}

my ($Xi, $Htbl, $inp, $len) = map("x$_", (0..3));	# argument block
my ($Xl, $Xm, $Xh, $INlo, $INhi) = map("v$_", (0..4));
my ($Hlo, $Hhi, $Hhl) = map("v$_", (5..7));
# d8-d15 are callee-saved, so avoid v8-v15. AArch64 SIMD has plenty of registers
# to spare.
my ($t0, $t1, $t2, $t3) = map("v$_", (16..19));
my ($t0l_t1l, $t0h_t1h, $t2l_t3l, $t2h_t3h) = map("v$_", (20..23));
my ($k48_k32, $k16_k0) = map("v$_", (24..25));

my $code = "";

# clmul64x64 emits code which emulates pmull $r.1q, $a.1d, $b.1d. $r, $a, and $b
# must be distinct from $t* and $k*. $t* are clobbered by the emitted code.
sub clmul64x64 {
my ($r, $a, $b) = @_;
$code .= <<___;
	ext	$t0.8b, $a.8b, $a.8b, #1	// A1
	pmull	$t0.8h, $t0.8b, $b.8b		// F = A1*B
	ext	$r.8b, $b.8b, $b.8b, #1		// B1
	pmull	$r.8h, $a.8b, $r.8b		// E = A*B1
	ext	$t1.8b, $a.8b, $a.8b, #2	// A2
	pmull	$t1.8h, $t1.8b, $b.8b		// H = A2*B
	ext	$t3.8b, $b.8b, $b.8b, #2	// B2
	pmull	$t3.8h, $a.8b, $t3.8b		// G = A*B2
	ext	$t2.8b, $a.8b, $a.8b, #3	// A3
	eor	$t0.16b, $t0.16b, $r.16b	// L = E + F
	pmull	$t2.8h, $t2.8b, $b.8b		// J = A3*B
	ext	$r.8b, $b.8b, $b.8b, #3		// B3
	eor	$t1.16b, $t1.16b, $t3.16b	// M = G + H
	pmull	$r.8h, $a.8b, $r.8b		// I = A*B3

	// Here we diverge from the 32-bit version. It computes the following
	// (instructions reordered for clarity):
	//
	//     veor	\$t0#lo, \$t0#lo, \$t0#hi	@ t0 = P0 + P1 (L)
	//     vand	\$t0#hi, \$t0#hi, \$k48
	//     veor	\$t0#lo, \$t0#lo, \$t0#hi
	//
	//     veor	\$t1#lo, \$t1#lo, \$t1#hi	@ t1 = P2 + P3 (M)
	//     vand	\$t1#hi, \$t1#hi, \$k32
	//     veor	\$t1#lo, \$t1#lo, \$t1#hi
	//
	//     veor	\$t2#lo, \$t2#lo, \$t2#hi	@ t2 = P4 + P5 (N)
	//     vand	\$t2#hi, \$t2#hi, \$k16
	//     veor	\$t2#lo, \$t2#lo, \$t2#hi
	//
	//     veor	\$t3#lo, \$t3#lo, \$t3#hi	@ t3 = P6 + P7 (K)
	//     vmov.i64	\$t3#hi, #0
	//
	// \$kN is a mask with the bottom N bits set. AArch64 cannot compute on
	// upper halves of SIMD registers, so we must split each half into
	// separate registers. To compensate, we pair computations up and
	// parallelize.

	ext	$t3.8b, $b.8b, $b.8b, #4	// B4
	eor	$t2.16b, $t2.16b, $r.16b	// N = I + J
	pmull	$t3.8h, $a.8b, $t3.8b		// K = A*B4

	// This can probably be scheduled more efficiently. For now, we just
	// pair up independent instructions.
	zip1	$t0l_t1l.2d, $t0.2d, $t1.2d
	zip1	$t2l_t3l.2d, $t2.2d, $t3.2d
	zip2	$t0h_t1h.2d, $t0.2d, $t1.2d
	zip2	$t2h_t3h.2d, $t2.2d, $t3.2d
	eor	$t0l_t1l.16b, $t0l_t1l.16b, $t0h_t1h.16b
	eor	$t2l_t3l.16b, $t2l_t3l.16b, $t2h_t3h.16b
	and	$t0h_t1h.16b, $t0h_t1h.16b, $k48_k32.16b
	and	$t2h_t3h.16b, $t2h_t3h.16b, $k16_k0.16b
	eor	$t0l_t1l.16b, $t0l_t1l.16b, $t0h_t1h.16b
	eor	$t2l_t3l.16b, $t2l_t3l.16b, $t2h_t3h.16b
	zip1	$t0.2d, $t0l_t1l.2d, $t0h_t1h.2d
	zip1	$t2.2d, $t2l_t3l.2d, $t2h_t3h.2d
	zip2	$t1.2d, $t0l_t1l.2d, $t0h_t1h.2d
	zip2	$t3.2d, $t2l_t3l.2d, $t2h_t3h.2d

	ext	$t0.16b, $t0.16b, $t0.16b, #15	// t0 = t0 << 8
	ext	$t1.16b, $t1.16b, $t1.16b, #14	// t1 = t1 << 16
	pmull	$r.8h, $a.8b, $b.8b		// D = A*B
	ext	$t3.16b, $t3.16b, $t3.16b, #12	// t3 = t3 << 32
	ext	$t2.16b, $t2.16b, $t2.16b, #13	// t2 = t2 << 24
	eor	$t0.16b, $t0.16b, $t1.16b
	eor	$t2.16b, $t2.16b, $t3.16b
	eor	$r.16b, $r.16b, $t0.16b
	eor	$r.16b, $r.16b, $t2.16b
___
}

$code .= <<___;
.text

.global	gcm_init_neon
.type	gcm_init_neon,%function
.align	4
gcm_init_neon:
	AARCH64_VALID_CALL_TARGET
	// This function is adapted from gcm_init_v8. xC2 is t3.
	ld1	{$t1.2d}, [x1]			// load H
	movi	$t3.16b, #0xe1
	shl	$t3.2d, $t3.2d, #57		// 0xc2.0
	ext	$INlo.16b, $t1.16b, $t1.16b, #8
	ushr	$t2.2d, $t3.2d, #63
	dup	$t1.4s, $t1.s[1]
	ext	$t0.16b, $t2.16b, $t3.16b, #8	// t0=0xc2....01
	ushr	$t2.2d, $INlo.2d, #63
	sshr	$t1.4s, $t1.4s, #31		// broadcast carry bit
	and	$t2.16b, $t2.16b, $t0.16b
	shl	$INlo.2d, $INlo.2d, #1
	ext	$t2.16b, $t2.16b, $t2.16b, #8
	and	$t0.16b, $t0.16b, $t1.16b
	orr	$INlo.16b, $INlo.16b, $t2.16b	// H<<<=1
	eor	$Hlo.16b, $INlo.16b, $t0.16b	// twisted H
	st1	{$Hlo.2d}, [x0]			// store Htable[0]
	ret
.size	gcm_init_neon,.-gcm_init_neon

.global	gcm_gmult_neon
.type	gcm_gmult_neon,%function
.align	4
gcm_gmult_neon:
	AARCH64_VALID_CALL_TARGET
	ld1	{$INlo.16b}, [$Xi]		// load Xi
	ld1	{$Hlo.1d}, [$Htbl], #8		// load twisted H
	ld1	{$Hhi.1d}, [$Htbl]
	adrp	x9, :pg_hi21:.Lmasks		// load constants
	add	x9, x9, :lo12:.Lmasks
	ld1	{$k48_k32.2d, $k16_k0.2d}, [x9]
	rev64	$INlo.16b, $INlo.16b		// byteswap Xi
	ext	$INlo.16b, $INlo.16b, $INlo.16b, #8
	eor	$Hhl.8b, $Hlo.8b, $Hhi.8b	// Karatsuba pre-processing

	mov	$len, #16
	b	.Lgmult_neon
.size	gcm_gmult_neon,.-gcm_gmult_neon

.global	gcm_ghash_neon
.type	gcm_ghash_neon,%function
.align	4
gcm_ghash_neon:
	AARCH64_VALID_CALL_TARGET
	ld1	{$Xl.16b}, [$Xi]		// load Xi
	ld1	{$Hlo.1d}, [$Htbl], #8		// load twisted H
	ld1	{$Hhi.1d}, [$Htbl]
	adrp	x9, :pg_hi21:.Lmasks		// load constants
	add	x9, x9, :lo12:.Lmasks
	ld1	{$k48_k32.2d, $k16_k0.2d}, [x9]
	rev64	$Xl.16b, $Xl.16b		// byteswap Xi
	ext	$Xl.16b, $Xl.16b, $Xl.16b, #8
	eor	$Hhl.8b, $Hlo.8b, $Hhi.8b	// Karatsuba pre-processing

.Loop_neon:
	ld1	{$INlo.16b}, [$inp], #16	// load inp
	rev64	$INlo.16b, $INlo.16b		// byteswap inp
	ext	$INlo.16b, $INlo.16b, $INlo.16b, #8
	eor	$INlo.16b, $INlo.16b, $Xl.16b	// inp ^= Xi

.Lgmult_neon:
	// Split the input into $INlo and $INhi. (The upper halves are unused,
	// so it is okay to leave them alone.)
	ins	$INhi.d[0], $INlo.d[1]
___
&clmul64x64	($Xl, $Hlo, $INlo);		# H.lo·Xi.lo
$code .= <<___;
	eor	$INlo.8b, $INlo.8b, $INhi.8b	// Karatsuba pre-processing
___
&clmul64x64	($Xm, $Hhl, $INlo);		# (H.lo+H.hi)·(Xi.lo+Xi.hi)
&clmul64x64	($Xh, $Hhi, $INhi);		# H.hi·Xi.hi
$code .= <<___;
	ext	$t0.16b, $Xl.16b, $Xh.16b, #8
	eor	$Xm.16b, $Xm.16b, $Xl.16b	// Karatsuba post-processing
	eor	$Xm.16b, $Xm.16b, $Xh.16b
	eor	$Xm.16b, $Xm.16b, $t0.16b	// Xm overlaps Xh.lo and Xl.hi
	ins	$Xl.d[1], $Xm.d[0]		// Xh|Xl - 256-bit result
	// This is a no-op due to the ins instruction below.
	// ins	$Xh.d[0], $Xm.d[1]

	// equivalent of reduction_avx from ghash-x86_64.pl
	shl	$t1.2d, $Xl.2d, #57		// 1st phase
	shl	$t2.2d, $Xl.2d, #62
	eor	$t2.16b, $t2.16b, $t1.16b	//
	shl	$t1.2d, $Xl.2d, #63
	eor	$t2.16b, $t2.16b, $t1.16b	//
	// Note Xm contains {Xl.d[1], Xh.d[0]}.
	eor	$t2.16b, $t2.16b, $Xm.16b
	ins	$Xl.d[1], $t2.d[0]		// Xl.d[1] ^= t2.d[0]
	ins	$Xh.d[0], $t2.d[1]		// Xh.d[0] ^= t2.d[1]

	ushr	$t2.2d, $Xl.2d, #1		// 2nd phase
	eor	$Xh.16b, $Xh.16b,$Xl.16b
	eor	$Xl.16b, $Xl.16b,$t2.16b	//
	ushr	$t2.2d, $t2.2d, #6
	ushr	$Xl.2d, $Xl.2d, #1		//
	eor	$Xl.16b, $Xl.16b, $Xh.16b	//
	eor	$Xl.16b, $Xl.16b, $t2.16b	//

	subs	$len, $len, #16
	bne	.Loop_neon

	rev64	$Xl.16b, $Xl.16b		// byteswap Xi and write
	ext	$Xl.16b, $Xl.16b, $Xl.16b, #8
	st1	{$Xl.16b}, [$Xi]

	ret
.size	gcm_ghash_neon,.-gcm_ghash_neon

.section	.rodata
.align	4
.Lmasks:
.quad	0x0000ffffffffffff	// k48
.quad	0x00000000ffffffff	// k32
.quad	0x000000000000ffff	// k16
.quad	0x0000000000000000	// k0
.asciz  "GHASH for ARMv8, derived from ARMv4 version by <appro\@openssl.org>"
.align  2
___

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/geo;

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT: $!"; # enforce flush
