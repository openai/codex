#! /usr/bin/env perl
# Copyright 2010-2018 The OpenSSL Project Authors. All Rights Reserved.
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

#
# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project.
# ====================================================================
#
# April 2010
#
# The module implements "4-bit" GCM GHASH function and underlying
# single multiplication operation in GF(2^128). "4-bit" means that it
# uses 256 bytes per-key table [+32 bytes shared table]. There is no
# experimental performance data available yet. The only approximation
# that can be made at this point is based on code size. Inner loop is
# 32 instructions long and on single-issue core should execute in <40
# cycles. Having verified that gcc 3.4 didn't unroll corresponding
# loop, this assembler loop body was found to be ~3x smaller than
# compiler-generated one...
#
# July 2010
#
# Rescheduling for dual-issue pipeline resulted in 8.5% improvement on
# Cortex A8 core and ~25 cycles per processed byte (which was observed
# to be ~3 times faster than gcc-generated code:-)
#
# February 2011
#
# Profiler-assisted and platform-specific optimization resulted in 7%
# improvement on Cortex A8 core and ~23.5 cycles per byte.
#
# March 2011
#
# Add NEON implementation featuring polynomial multiplication, i.e. no
# lookup tables involved. On Cortex A8 it was measured to process one
# byte in 15 cycles or 55% faster than integer-only code.
#
# April 2014
#
# Switch to multiplication algorithm suggested in paper referred
# below and combine it with reduction algorithm from x86 module.
# Performance improvement over previous version varies from 65% on
# Snapdragon S4 to 110% on Cortex A9. In absolute terms Cortex A8
# processes one byte in 8.45 cycles, A9 - in 10.2, A15 - in 7.63,
# Snapdragon S4 - in 9.33.
#
# Câmara, D.; Gouvêa, C. P. L.; López, J. & Dahab, R.: Fast Software
# Polynomial Multiplication on ARM Processors using the NEON Engine.
#
# http://conradoplg.cryptoland.net/files/2010/12/mocrysen13.pdf

# ====================================================================
# Note about "528B" variant. In ARM case it makes lesser sense to
# implement it for following reasons:
#
# - performance improvement won't be anywhere near 50%, because 128-
#   bit shift operation is neatly fused with 128-bit xor here, and
#   "538B" variant would eliminate only 4-5 instructions out of 32
#   in the inner loop (meaning that estimated improvement is ~15%);
# - ARM-based systems are often embedded ones and extra memory
#   consumption might be unappreciated (for so little improvement);
#
# Byte order [in]dependence. =========================================
#
# Caller is expected to maintain specific *dword* order in Htable,
# namely with *least* significant dword of 128-bit value at *lower*
# address. This differs completely from C code and has everything to
# do with ldm instruction and order in which dwords are "consumed" by
# algorithm. *Byte* order within these dwords in turn is whatever
# *native* byte order on current platform. See gcm128.c for working
# example...

# This file was patched in BoringSSL to remove the variable-time 4-bit
# implementation.

$flavour = shift;
if ($flavour=~/\w[\w\-]*\.\w+$/) { $output=$flavour; undef $flavour; }
else { while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {} }

if ($flavour && $flavour ne "void") {
    $0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
    ( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
    ( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
    die "can't locate arm-xlate.pl";

    open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
    *STDOUT=*OUT;
} else {
    open OUT,">$output";
    *STDOUT=*OUT;
}

$Xi="r0";	# argument block
$Htbl="r1";
$inp="r2";
$len="r3";

$code=<<___;
@ Silence ARMv8 deprecated IT instruction warnings. This file is used by both
@ ARMv7 and ARMv8 processors and does not use ARMv8 instructions. (ARMv8 PMULL
@ instructions are in aesv8-armx.pl.)
.arch  armv7-a

.text
#if defined(__thumb2__) || defined(__clang__)
.syntax	unified
#define ldrplb  ldrbpl
#define ldrneb  ldrbne
#endif
#if defined(__thumb2__)
.thumb
#else
.code	32
#endif
___
{
my ($Xl,$Xm,$Xh,$IN)=map("q$_",(0..3));
my ($t0,$t1,$t2,$t3)=map("q$_",(8..12));
my ($Hlo,$Hhi,$Hhl,$k48,$k32,$k16)=map("d$_",(26..31));

sub clmul64x64 {
my ($r,$a,$b)=@_;
$code.=<<___;
	vext.8		$t0#lo, $a, $a, #1	@ A1
	vmull.p8	$t0, $t0#lo, $b		@ F = A1*B
	vext.8		$r#lo, $b, $b, #1	@ B1
	vmull.p8	$r, $a, $r#lo		@ E = A*B1
	vext.8		$t1#lo, $a, $a, #2	@ A2
	vmull.p8	$t1, $t1#lo, $b		@ H = A2*B
	vext.8		$t3#lo, $b, $b, #2	@ B2
	vmull.p8	$t3, $a, $t3#lo		@ G = A*B2
	vext.8		$t2#lo, $a, $a, #3	@ A3
	veor		$t0, $t0, $r		@ L = E + F
	vmull.p8	$t2, $t2#lo, $b		@ J = A3*B
	vext.8		$r#lo, $b, $b, #3	@ B3
	veor		$t1, $t1, $t3		@ M = G + H
	vmull.p8	$r, $a, $r#lo		@ I = A*B3
	veor		$t0#lo, $t0#lo, $t0#hi	@ t0 = (L) (P0 + P1) << 8
	vand		$t0#hi, $t0#hi, $k48
	vext.8		$t3#lo, $b, $b, #4	@ B4
	veor		$t1#lo, $t1#lo, $t1#hi	@ t1 = (M) (P2 + P3) << 16
	vand		$t1#hi, $t1#hi, $k32
	vmull.p8	$t3, $a, $t3#lo		@ K = A*B4
	veor		$t2, $t2, $r		@ N = I + J
	veor		$t0#lo, $t0#lo, $t0#hi
	veor		$t1#lo, $t1#lo, $t1#hi
	veor		$t2#lo, $t2#lo, $t2#hi	@ t2 = (N) (P4 + P5) << 24
	vand		$t2#hi, $t2#hi, $k16
	vext.8		$t0, $t0, $t0, #15
	veor		$t3#lo, $t3#lo, $t3#hi	@ t3 = (K) (P6 + P7) << 32
	vmov.i64	$t3#hi, #0
	vext.8		$t1, $t1, $t1, #14
	veor		$t2#lo, $t2#lo, $t2#hi
	vmull.p8	$r, $a, $b		@ D = A*B
	vext.8		$t3, $t3, $t3, #12
	vext.8		$t2, $t2, $t2, #13
	veor		$t0, $t0, $t1
	veor		$t2, $t2, $t3
	veor		$r, $r, $t0
	veor		$r, $r, $t2
___
}

$code.=<<___;
#if __ARM_MAX_ARCH__>=7
.arch	armv7-a
.fpu	neon

.global	gcm_init_neon
.type	gcm_init_neon,%function
.align	4
gcm_init_neon:
	vld1.64		$IN#hi,[r1]!		@ load H
	vmov.i8		$t0,#0xe1
	vld1.64		$IN#lo,[r1]
	vshl.i64	$t0#hi,#57
	vshr.u64	$t0#lo,#63		@ t0=0xc2....01
	vdup.8		$t1,$IN#hi[7]
	vshr.u64	$Hlo,$IN#lo,#63
	vshr.s8		$t1,#7			@ broadcast carry bit
	vshl.i64	$IN,$IN,#1
	vand		$t0,$t0,$t1
	vorr		$IN#hi,$Hlo		@ H<<<=1
	veor		$IN,$IN,$t0		@ twisted H
	vstmia		r0,{$IN}

	ret					@ bx lr
.size	gcm_init_neon,.-gcm_init_neon

.global	gcm_gmult_neon
.type	gcm_gmult_neon,%function
.align	4
gcm_gmult_neon:
	vld1.64		$IN#hi,[$Xi]!		@ load Xi
	vld1.64		$IN#lo,[$Xi]!
	vmov.i64	$k48,#0x0000ffffffffffff
	vldmia		$Htbl,{$Hlo-$Hhi}	@ load twisted H
	vmov.i64	$k32,#0x00000000ffffffff
#ifdef __ARMEL__
	vrev64.8	$IN,$IN
#endif
	vmov.i64	$k16,#0x000000000000ffff
	veor		$Hhl,$Hlo,$Hhi		@ Karatsuba pre-processing
	mov		$len,#16
	b		.Lgmult_neon
.size	gcm_gmult_neon,.-gcm_gmult_neon

.global	gcm_ghash_neon
.type	gcm_ghash_neon,%function
.align	4
gcm_ghash_neon:
	vld1.64		$Xl#hi,[$Xi]!		@ load Xi
	vld1.64		$Xl#lo,[$Xi]!
	vmov.i64	$k48,#0x0000ffffffffffff
	vldmia		$Htbl,{$Hlo-$Hhi}	@ load twisted H
	vmov.i64	$k32,#0x00000000ffffffff
#ifdef __ARMEL__
	vrev64.8	$Xl,$Xl
#endif
	vmov.i64	$k16,#0x000000000000ffff
	veor		$Hhl,$Hlo,$Hhi		@ Karatsuba pre-processing

.Loop_neon:
	vld1.64		$IN#hi,[$inp]!		@ load inp
	vld1.64		$IN#lo,[$inp]!
#ifdef __ARMEL__
	vrev64.8	$IN,$IN
#endif
	veor		$IN,$Xl			@ inp^=Xi
.Lgmult_neon:
___
	&clmul64x64	($Xl,$Hlo,"$IN#lo");	# H.lo·Xi.lo
$code.=<<___;
	veor		$IN#lo,$IN#lo,$IN#hi	@ Karatsuba pre-processing
___
	&clmul64x64	($Xm,$Hhl,"$IN#lo");	# (H.lo+H.hi)·(Xi.lo+Xi.hi)
	&clmul64x64	($Xh,$Hhi,"$IN#hi");	# H.hi·Xi.hi
$code.=<<___;
	veor		$Xm,$Xm,$Xl		@ Karatsuba post-processing
	veor		$Xm,$Xm,$Xh
	veor		$Xl#hi,$Xl#hi,$Xm#lo
	veor		$Xh#lo,$Xh#lo,$Xm#hi	@ Xh|Xl - 256-bit result

	@ equivalent of reduction_avx from ghash-x86_64.pl
	vshl.i64	$t1,$Xl,#57		@ 1st phase
	vshl.i64	$t2,$Xl,#62
	veor		$t2,$t2,$t1		@
	vshl.i64	$t1,$Xl,#63
	veor		$t2, $t2, $t1		@
 	veor		$Xl#hi,$Xl#hi,$t2#lo	@
	veor		$Xh#lo,$Xh#lo,$t2#hi

	vshr.u64	$t2,$Xl,#1		@ 2nd phase
	veor		$Xh,$Xh,$Xl
	veor		$Xl,$Xl,$t2		@
	vshr.u64	$t2,$t2,#6
	vshr.u64	$Xl,$Xl,#1		@
	veor		$Xl,$Xl,$Xh		@
	veor		$Xl,$Xl,$t2		@

	subs		$len,#16
	bne		.Loop_neon

#ifdef __ARMEL__
	vrev64.8	$Xl,$Xl
#endif
	sub		$Xi,#16
	vst1.64		$Xl#hi,[$Xi]!		@ write out Xi
	vst1.64		$Xl#lo,[$Xi]

	ret					@ bx lr
.size	gcm_ghash_neon,.-gcm_ghash_neon
#endif
___
}
$code.=<<___;
.asciz  "GHASH for ARMv4/NEON, CRYPTOGAMS by <appro\@openssl.org>"
.align  2
___

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/geo;

	s/\bq([0-9]+)#(lo|hi)/sprintf "d%d",2*$1+($2 eq "hi")/geo	or
	s/\bret\b/bx	lr/go		or
	s/\bbx\s+lr\b/.word\t0xe12fff1e/go;    # make it possible to compile with -march=armv4

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT: $!"; # enforce flush
