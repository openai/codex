#! /usr/bin/env perl
# Copyright 2015-2016 The OpenSSL Project Authors. All Rights Reserved.
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


######################################################################
## Constant-time SSSE3 AES core implementation.
## version 0.1
##
## By Mike Hamburg (Stanford University), 2009
## Public domain.
##
## For details see http://shiftleft.org/papers/vector_aes/ and
## http://crypto.stanford.edu/vpaes/.
##
######################################################################
# Adapted from the original x86_64 version and <appro@openssl.org>'s ARMv8
# version.
#
# armv7, aarch64, and x86_64 differ in several ways:
#
# * x86_64 SSSE3 instructions are two-address (destination operand is also a
#   source), while NEON is three-address (destination operand is separate from
#   two sources).
#
# * aarch64 has 32 SIMD registers available, while x86_64 and armv7 have 16.
#
# * x86_64 instructions can take memory references, while ARM is a load/store
#   architecture. This means we sometimes need a spare register.
#
# * aarch64 and x86_64 have 128-bit byte shuffle instructions (tbl and pshufb),
#   while armv7 only has a 64-bit byte shuffle (vtbl).
#
# This means this armv7 version must be a mix of both aarch64 and x86_64
# implementations. armv7 and aarch64 have analogous SIMD instructions, so we
# base the instructions on aarch64. However, we cannot use aarch64's register
# allocation. x86_64's register count matches, but x86_64 is two-address.
# vpaes-armv8.pl already accounts for this in the comments, which use
# three-address AVX instructions instead of the original SSSE3 ones. We base
# register usage on these comments, which are preserved in this file.
#
# This means we do not use separate input and output registers as in aarch64 and
# cannot pin as many constants in the preheat functions. However, the load/store
# architecture means we must still deviate from x86_64 in places.
#
# Next, we account for the byte shuffle instructions. vtbl takes 64-bit source
# and destination and 128-bit table. Fortunately, armv7 also allows addressing
# upper and lower halves of each 128-bit register. The lower half of q{N} is
# d{2*N}. The upper half is d{2*N+1}. Instead of the following non-existent
# instruction,
#
#     vtbl.8 q0, q1, q2   @ Index each of q2's 16 bytes into q1. Store in q0.
#
# we write:
#
#     vtbl.8 d0, q1, d4   @ Index each of d4's 8 bytes into q1. Store in d0.
#     vtbl.8 d1, q1, d5   @ Index each of d5's 8 bytes into q1. Store in d1.
#
# For readability, we write d0 and d1 as q0#lo and q0#hi, respectively and
# post-process before outputting. (This is adapted from ghash-armv4.pl.) Note,
# however, that destination (q0) and table (q1) registers may no longer match.
# We adjust the register usage from x86_64 to avoid this. (Unfortunately, the
# two-address pshufb always matched these operands, so this is common.)
#
# This file also runs against the limit of ARMv7's ADR pseudo-instruction. ADR
# expands to an ADD or SUB of the pc register to find an address. That immediate
# must fit in ARM's encoding scheme: 8 bits of constant and 4 bits of rotation.
# This means larger values must be more aligned.
#
# ARM additionally has two encodings, ARM and Thumb mode. Our assembly files may
# use either encoding (do we actually need to support this?). In ARM mode, the
# distances get large enough to require 16-byte alignment. Moving constants
# closer to their use resolves most of this, but common constants in
# _vpaes_consts are used by the whole file. Affected ADR instructions must be
# placed at 8 mod 16 (the pc register is 8 ahead). Instructions with this
# constraint have been commented.
#
# For details on ARM's immediate value encoding scheme, see
# https://alisdair.mcdiarmid.org/arm-immediate-value-encoding/
#
# Finally, a summary of armv7 and aarch64 SIMD syntax differences:
#
# * armv7 prefixes SIMD instructions with 'v', while aarch64 does not.
#
# * armv7 SIMD registers are named like q0 (and d0 for the half-width ones).
#   aarch64 names registers like v0, and denotes half-width operations in an
#   instruction suffix (see below).
#
# * aarch64 embeds size and lane information in register suffixes. v0.16b is
#   16 bytes, v0.8h is eight u16s, v0.4s is four u32s, and v0.2d is two u64s.
#   armv7 embeds the total size in the register name (see above) and the size of
#   each element in an instruction suffix, which may look like vmov.i8,
#   vshr.u8, or vtbl.8, depending on instruction.

use strict;

my $flavour = shift;
my $output;
while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {}

$0 =~ m/(.*[\/\\])[^\/\\]+$/;
my $dir=$1;
my $xlate;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

my $code = "";

$code.=<<___;
.syntax	unified

.arch	armv7-a
.fpu	neon

#if defined(__thumb2__)
.thumb
#else
.code	32
#endif

.text

.type	_vpaes_consts,%object
.align	7	@ totally strategic alignment
_vpaes_consts:
.Lk_mc_forward:	@ mc_forward
	.quad	0x0407060500030201, 0x0C0F0E0D080B0A09
	.quad	0x080B0A0904070605, 0x000302010C0F0E0D
	.quad	0x0C0F0E0D080B0A09, 0x0407060500030201
	.quad	0x000302010C0F0E0D, 0x080B0A0904070605
.Lk_mc_backward:@ mc_backward
	.quad	0x0605040702010003, 0x0E0D0C0F0A09080B
	.quad	0x020100030E0D0C0F, 0x0A09080B06050407
	.quad	0x0E0D0C0F0A09080B, 0x0605040702010003
	.quad	0x0A09080B06050407, 0x020100030E0D0C0F
.Lk_sr:		@ sr
	.quad	0x0706050403020100, 0x0F0E0D0C0B0A0908
	.quad	0x030E09040F0A0500, 0x0B06010C07020D08
	.quad	0x0F060D040B020900, 0x070E050C030A0108
	.quad	0x0B0E0104070A0D00, 0x0306090C0F020508

@
@ "Hot" constants
@
.Lk_inv:	@ inv, inva
	.quad	0x0E05060F0D080180, 0x040703090A0B0C02
	.quad	0x01040A060F0B0780, 0x030D0E0C02050809
.Lk_ipt:	@ input transform (lo, hi)
	.quad	0xC2B2E8985A2A7000, 0xCABAE09052227808
	.quad	0x4C01307D317C4D00, 0xCD80B1FCB0FDCC81
.Lk_sbo:	@ sbou, sbot
	.quad	0xD0D26D176FBDC700, 0x15AABF7AC502A878
	.quad	0xCFE474A55FBB6A00, 0x8E1E90D1412B35FA
.Lk_sb1:	@ sb1u, sb1t
	.quad	0x3618D415FAE22300, 0x3BF7CCC10D2ED9EF
	.quad	0xB19BE18FCB503E00, 0xA5DF7A6E142AF544
.Lk_sb2:	@ sb2u, sb2t
	.quad	0x69EB88400AE12900, 0xC2A163C8AB82234A
	.quad	0xE27A93C60B712400, 0x5EB7E955BC982FCD

.asciz  "Vector Permutation AES for ARMv7 NEON, Mike Hamburg (Stanford University)"
.size	_vpaes_consts,.-_vpaes_consts
.align	6
___

{
my ($inp,$out,$key) = map("r$_", (0..2));

my ($invlo,$invhi) = map("q$_", (10..11));
my ($sb1u,$sb1t,$sb2u,$sb2t) = map("q$_", (12..15));

$code.=<<___;
@@
@@  _aes_preheat
@@
@@  Fills q9-q15 as specified below.
@@
.type	_vpaes_preheat,%function
.align	4
_vpaes_preheat:
	adr	r10, .Lk_inv
	vmov.i8	q9, #0x0f		@ .Lk_s0F
	vld1.64	{q10,q11}, [r10]!	@ .Lk_inv
	add	r10, r10, #64		@ Skip .Lk_ipt, .Lk_sbo
	vld1.64	{q12,q13}, [r10]!	@ .Lk_sb1
	vld1.64	{q14,q15}, [r10]	@ .Lk_sb2
	bx	lr

@@
@@  _aes_encrypt_core
@@
@@  AES-encrypt q0.
@@
@@  Inputs:
@@     q0 = input
@@     q9-q15 as in _vpaes_preheat
@@    [$key] = scheduled keys
@@
@@  Output in q0
@@  Clobbers  q1-q5, r8-r11
@@  Preserves q6-q8 so you get some local vectors
@@
@@
.type	_vpaes_encrypt_core,%function
.align 4
_vpaes_encrypt_core:
	mov	r9, $key
	ldr	r8, [$key,#240]		@ pull rounds
	adr	r11, .Lk_ipt
	@ vmovdqa	.Lk_ipt(%rip),	%xmm2	# iptlo
	@ vmovdqa	.Lk_ipt+16(%rip), %xmm3	# ipthi
	vld1.64	{q2, q3}, [r11]
	adr	r11, .Lk_mc_forward+16
	vld1.64	{q5}, [r9]!		@ vmovdqu	(%r9),	%xmm5		# round0 key
	vand	q1, q0, q9		@ vpand	%xmm9,	%xmm0,	%xmm1
	vshr.u8	q0, q0, #4		@ vpsrlb	\$4,	%xmm0,	%xmm0
	vtbl.8	q1#lo, {q2}, q1#lo	@ vpshufb	%xmm1,	%xmm2,	%xmm1
	vtbl.8	q1#hi, {q2}, q1#hi
	vtbl.8	q2#lo, {q3}, q0#lo	@ vpshufb	%xmm0,	%xmm3,	%xmm2
	vtbl.8	q2#hi, {q3}, q0#hi
	veor	q0, q1, q5		@ vpxor	%xmm5,	%xmm1,	%xmm0
	veor	q0, q0, q2		@ vpxor	%xmm2,	%xmm0,	%xmm0

	@ .Lenc_entry ends with a bnz instruction which is normally paired with
	@ subs in .Lenc_loop.
	tst	r8, r8
	b	.Lenc_entry

.align 4
.Lenc_loop:
	@ middle of middle round
	add	r10, r11, #0x40
	vtbl.8	q4#lo, {$sb1t}, q2#lo	@ vpshufb	%xmm2,	%xmm13,	%xmm4	# 4 = sb1u
	vtbl.8	q4#hi, {$sb1t}, q2#hi
	vld1.64	{q1}, [r11]!		@ vmovdqa	-0x40(%r11,%r10), %xmm1	# .Lk_mc_forward[]
	vtbl.8	q0#lo, {$sb1u}, q3#lo	@ vpshufb	%xmm3,	%xmm12,	%xmm0	# 0 = sb1t
	vtbl.8	q0#hi, {$sb1u}, q3#hi
	veor	q4, q4, q5		@ vpxor		%xmm5,	%xmm4,	%xmm4	# 4 = sb1u + k
	vtbl.8	q5#lo, {$sb2t}, q2#lo	@ vpshufb	%xmm2,	%xmm15,	%xmm5	# 4 = sb2u
	vtbl.8	q5#hi, {$sb2t}, q2#hi
	veor	q0, q0, q4		@ vpxor		%xmm4,	%xmm0,	%xmm0	# 0 = A
	vtbl.8	q2#lo, {$sb2u}, q3#lo	@ vpshufb	%xmm3,	%xmm14,	%xmm2	# 2 = sb2t
	vtbl.8	q2#hi, {$sb2u}, q3#hi
	vld1.64	{q4}, [r10]		@ vmovdqa	(%r11,%r10), %xmm4	# .Lk_mc_backward[]
	vtbl.8	q3#lo, {q0}, q1#lo	@ vpshufb	%xmm1,	%xmm0,	%xmm3	# 0 = B
	vtbl.8	q3#hi, {q0}, q1#hi
	veor	q2, q2, q5		@ vpxor		%xmm5,	%xmm2,	%xmm2	# 2 = 2A
	@ Write to q5 instead of q0, so the table and destination registers do
	@ not overlap.
	vtbl.8	q5#lo, {q0}, q4#lo	@ vpshufb	%xmm4,	%xmm0,	%xmm0	# 3 = D
	vtbl.8	q5#hi, {q0}, q4#hi
	veor	q3, q3, q2		@ vpxor		%xmm2,	%xmm3,	%xmm3	# 0 = 2A+B
	vtbl.8	q4#lo, {q3}, q1#lo	@ vpshufb	%xmm1,	%xmm3,	%xmm4	# 0 = 2B+C
	vtbl.8	q4#hi, {q3}, q1#hi
	@ Here we restore the original q0/q5 usage.
	veor	q0, q5, q3		@ vpxor		%xmm3,	%xmm0,	%xmm0	# 3 = 2A+B+D
	and	r11, r11, #~(1<<6)	@ and		\$0x30,	%r11		# ... mod 4
	veor	q0, q0, q4		@ vpxor		%xmm4,	%xmm0, %xmm0	# 0 = 2A+3B+C+D
	subs	r8, r8, #1		@ nr--

.Lenc_entry:
	@ top of round
	vand	q1, q0, q9		@ vpand		%xmm0,	%xmm9,	%xmm1   # 0 = k
	vshr.u8	q0, q0, #4		@ vpsrlb	\$4,	%xmm0,	%xmm0	# 1 = i
	vtbl.8	q5#lo, {$invhi}, q1#lo	@ vpshufb	%xmm1,	%xmm11,	%xmm5	# 2 = a/k
	vtbl.8	q5#hi, {$invhi}, q1#hi
	veor	q1, q1, q0		@ vpxor		%xmm0,	%xmm1,	%xmm1	# 0 = j
	vtbl.8	q3#lo, {$invlo}, q0#lo	@ vpshufb	%xmm0, 	%xmm10,	%xmm3  	# 3 = 1/i
	vtbl.8	q3#hi, {$invlo}, q0#hi
	vtbl.8	q4#lo, {$invlo}, q1#lo	@ vpshufb	%xmm1, 	%xmm10,	%xmm4  	# 4 = 1/j
	vtbl.8	q4#hi, {$invlo}, q1#hi
	veor	q3, q3, q5		@ vpxor		%xmm5,	%xmm3,	%xmm3	# 3 = iak = 1/i + a/k
	veor	q4, q4, q5		@ vpxor		%xmm5,	%xmm4,	%xmm4  	# 4 = jak = 1/j + a/k
	vtbl.8	q2#lo, {$invlo}, q3#lo	@ vpshufb	%xmm3,	%xmm10,	%xmm2  	# 2 = 1/iak
	vtbl.8	q2#hi, {$invlo}, q3#hi
	vtbl.8	q3#lo, {$invlo}, q4#lo	@ vpshufb	%xmm4,	%xmm10,	%xmm3	# 3 = 1/jak
	vtbl.8	q3#hi, {$invlo}, q4#hi
	veor	q2, q2, q1		@ vpxor		%xmm1,	%xmm2,	%xmm2  	# 2 = io
	veor	q3, q3, q0		@ vpxor		%xmm0,	%xmm3,	%xmm3	# 3 = jo
	vld1.64	{q5}, [r9]!		@ vmovdqu	(%r9),	%xmm5
	bne	.Lenc_loop

	@ middle of last round
	add	r10, r11, #0x80

	adr	r11, .Lk_sbo
	@ Read to q1 instead of q4, so the vtbl.8 instruction below does not
	@ overlap table and destination registers.
	vld1.64 {q1}, [r11]!		@ vmovdqa	-0x60(%r10), %xmm4	# 3 : sbou
	vld1.64 {q0}, [r11]		@ vmovdqa	-0x50(%r10), %xmm0	# 0 : sbot	.Lk_sbo+16
	vtbl.8	q4#lo, {q1}, q2#lo	@ vpshufb	%xmm2,	%xmm4,	%xmm4	# 4 = sbou
	vtbl.8	q4#hi, {q1}, q2#hi
	vld1.64	{q1}, [r10]		@ vmovdqa	0x40(%r11,%r10), %xmm1	# .Lk_sr[]
	@ Write to q2 instead of q0 below, to avoid overlapping table and
	@ destination registers.
	vtbl.8	q2#lo, {q0}, q3#lo	@ vpshufb	%xmm3,	%xmm0,	%xmm0	# 0 = sb1t
	vtbl.8	q2#hi, {q0}, q3#hi
	veor	q4, q4, q5		@ vpxor	%xmm5,	%xmm4,	%xmm4	# 4 = sb1u + k
	veor	q2, q2, q4		@ vpxor	%xmm4,	%xmm0,	%xmm0	# 0 = A
	@ Here we restore the original q0/q2 usage.
	vtbl.8	q0#lo, {q2}, q1#lo	@ vpshufb	%xmm1,	%xmm0,	%xmm0
	vtbl.8	q0#hi, {q2}, q1#hi
	bx	lr
.size	_vpaes_encrypt_core,.-_vpaes_encrypt_core
___
}
{
my ($inp,$bits,$out,$dir)=("r0","r1","r2","r3");
my ($rcon,$s0F,$invlo,$invhi,$s63) = map("q$_",(8..12));

$code.=<<___;
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
@@                                                    @@
@@                  AES key schedule                  @@
@@                                                    @@
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@

@ This function diverges from both x86_64 and armv7 in which constants are
@ pinned. x86_64 has a common preheat function for all operations. aarch64
@ separates them because it has enough registers to pin nearly all constants.
@ armv7 does not have enough registers, but needing explicit loads and stores
@ also complicates using x86_64's register allocation directly.
@
@ We pin some constants for convenience and leave q14 and q15 free to load
@ others on demand.

@
@  Key schedule constants
@
.type	_vpaes_key_consts,%object
.align	4
_vpaes_key_consts:
.Lk_rcon:	@ rcon
	.quad	0x1F8391B9AF9DEEB6, 0x702A98084D7C7D81

.Lk_opt:	@ output transform
	.quad	0xFF9F4929D6B66000, 0xF7974121DEBE6808
	.quad	0x01EDBD5150BCEC00, 0xE10D5DB1B05C0CE0
.Lk_deskew:	@ deskew tables: inverts the sbox's "skew"
	.quad	0x07E4A34047A4E300, 0x1DFEB95A5DBEF91A
	.quad	0x5F36B5DC83EA6900, 0x2841C2ABF49D1E77
.size	_vpaes_key_consts,.-_vpaes_key_consts

.type	_vpaes_key_preheat,%function
.align	4
_vpaes_key_preheat:
	adr	r11, .Lk_rcon
	vmov.i8	$s63, #0x5b			@ .Lk_s63
	adr	r10, .Lk_inv			@ Must be aligned to 8 mod 16.
	vmov.i8	$s0F, #0x0f			@ .Lk_s0F
	vld1.64	{$invlo,$invhi}, [r10]		@ .Lk_inv
	vld1.64	{$rcon}, [r11]			@ .Lk_rcon
	bx	lr
.size	_vpaes_key_preheat,.-_vpaes_key_preheat

.type	_vpaes_schedule_core,%function
.align	4
_vpaes_schedule_core:
	@ We only need to save lr, but ARM requires an 8-byte stack alignment,
	@ so save an extra register.
	stmdb	sp!, {r3,lr}

	bl	_vpaes_key_preheat	@ load the tables

	adr	r11, .Lk_ipt		@ Must be aligned to 8 mod 16.
	vld1.64	{q0}, [$inp]!		@ vmovdqu	(%rdi),	%xmm0		# load key (unaligned)

	@ input transform
	@ Use q4 here rather than q3 so .Lschedule_am_decrypting does not
	@ overlap table and destination.
	vmov	q4, q0			@ vmovdqa	%xmm0,	%xmm3
	bl	_vpaes_schedule_transform
	adr	r10, .Lk_sr		@ Must be aligned to 8 mod 16.
	vmov	q7, q0			@ vmovdqa	%xmm0,	%xmm7

	add	r8, r8, r10

	@ encrypting, output zeroth round key after transform
	vst1.64	{q0}, [$out]		@ vmovdqu	%xmm0,	(%rdx)

	@ *ring*: Decryption removed.

.Lschedule_go:
	cmp	$bits, #192		@ cmp	\$192,	%esi
	bhi	.Lschedule_256
	@ 128: fall though

@@
@@  .schedule_128
@@
@@  128-bit specific part of key schedule.
@@
@@  This schedule is really simple, because all its parts
@@  are accomplished by the subroutines.
@@
.Lschedule_128:
	mov	$inp, #10		@ mov	\$10, %esi

.Loop_schedule_128:
	bl 	_vpaes_schedule_round
	subs	$inp, $inp, #1		@ dec	%esi
	beq 	.Lschedule_mangle_last
	bl	_vpaes_schedule_mangle	@ write output
	b 	.Loop_schedule_128

@@
@@  .aes_schedule_256
@@
@@  256-bit specific part of key schedule.
@@
@@  The structure here is very similar to the 128-bit
@@  schedule, but with an additional "low side" in
@@  q6.  The low side's rounds are the same as the
@@  high side's, except no rcon and no rotation.
@@
.align	4
.Lschedule_256:
	vld1.64	{q0}, [$inp]			@ vmovdqu	16(%rdi),%xmm0		# load key part 2 (unaligned)
	bl	_vpaes_schedule_transform	@ input transform
	mov	$inp, #7			@ mov	\$7, %esi

.Loop_schedule_256:
	bl	_vpaes_schedule_mangle		@ output low result
	vmov	q6, q0				@ vmovdqa	%xmm0,	%xmm6		# save cur_lo in xmm6

	@ high round
	bl	_vpaes_schedule_round
	subs	$inp, $inp, #1			@ dec	%esi
	beq 	.Lschedule_mangle_last
	bl	_vpaes_schedule_mangle

	@ low round. swap xmm7 and xmm6
	vdup.32	q0, q0#hi[1]		@ vpshufd	\$0xFF,	%xmm0,	%xmm0
	vmov.i8	q4, #0
	vmov	q5, q7			@ vmovdqa	%xmm7,	%xmm5
	vmov	q7, q6			@ vmovdqa	%xmm6,	%xmm7
	bl	_vpaes_schedule_low_round
	vmov	q7, q5			@ vmovdqa	%xmm5,	%xmm7

	b	.Loop_schedule_256

@@
@@  .aes_schedule_mangle_last
@@
@@  Mangler for last round of key schedule
@@  Mangles q0
@@    when encrypting, outputs out(q0) ^ 63
@@    when decrypting, outputs unskew(q0)
@@
@@  Always called right before return... jumps to cleanup and exits
@@
.align	4
.Lschedule_mangle_last:
	@ schedule last round key from xmm0
	adr	r11, .Lk_deskew			@ lea	.Lk_deskew(%rip),%r11	# prepare to deskew

	@ encrypting
	vld1.64	{q1}, [r8]		@ vmovdqa	(%r8,%r10),%xmm1
	adr	r11, .Lk_opt		@ lea		.Lk_opt(%rip),	%r11		# prepare to output transform
	add	$out, $out, #32		@ add		\$32,	%rdx
	vmov	q2, q0
	vtbl.8	q0#lo, {q2}, q1#lo	@ vpshufb	%xmm1,	%xmm0,	%xmm0		# output permute
	vtbl.8	q0#hi, {q2}, q1#hi

.Lschedule_mangle_last_dec:
	sub	$out, $out, #16			@ add	\$-16,	%rdx
	veor	q0, q0, $s63			@ vpxor	.Lk_s63(%rip),	%xmm0,	%xmm0
	bl	_vpaes_schedule_transform	@ output transform
	vst1.64	{q0}, [$out]			@ vmovdqu	%xmm0,	(%rdx)		# save last key

	@ cleanup
	veor	q0, q0, q0		@ vpxor	%xmm0,	%xmm0,	%xmm0
	veor	q1, q1, q1		@ vpxor	%xmm1,	%xmm1,	%xmm1
	veor	q2, q2, q2		@ vpxor	%xmm2,	%xmm2,	%xmm2
	veor	q3, q3, q3		@ vpxor	%xmm3,	%xmm3,	%xmm3
	veor	q4, q4, q4		@ vpxor	%xmm4,	%xmm4,	%xmm4
	veor	q5, q5, q5		@ vpxor	%xmm5,	%xmm5,	%xmm5
	veor	q6, q6, q6		@ vpxor	%xmm6,	%xmm6,	%xmm6
	veor	q7, q7, q7		@ vpxor	%xmm7,	%xmm7,	%xmm7
	ldmia	sp!, {r3,pc}		@ return
.size	_vpaes_schedule_core,.-_vpaes_schedule_core

@@
@@  .aes_schedule_round
@@
@@  Runs one main round of the key schedule on q0, q7
@@
@@  Specifically, runs subbytes on the high dword of q0
@@  then rotates it by one byte and xors into the low dword of
@@  q7.
@@
@@  Adds rcon from low byte of q8, then rotates q8 for
@@  next rcon.
@@
@@  Smears the dwords of q7 by xoring the low into the
@@  second low, result into third, result into highest.
@@
@@  Returns results in q7 = q0.
@@  Clobbers q1-q4, r11.
@@
.type	_vpaes_schedule_round,%function
.align	4
_vpaes_schedule_round:
	@ extract rcon from xmm8
	vmov.i8	q4, #0				@ vpxor		%xmm4,	%xmm4,	%xmm4
	vext.8	q1, $rcon, q4, #15		@ vpalignr	\$15,	%xmm8,	%xmm4,	%xmm1
	vext.8	$rcon, $rcon, $rcon, #15	@ vpalignr	\$15,	%xmm8,	%xmm8,	%xmm8
	veor	q7, q7, q1			@ vpxor		%xmm1,	%xmm7,	%xmm7

	@ rotate
	vdup.32	q0, q0#hi[1]			@ vpshufd	\$0xFF,	%xmm0,	%xmm0
	vext.8	q0, q0, q0, #1			@ vpalignr	\$1,	%xmm0,	%xmm0,	%xmm0

	@ fall through...

	@ low round: same as high round, but no rotation and no rcon.
_vpaes_schedule_low_round:
	@ The x86_64 version pins .Lk_sb1 in %xmm13 and .Lk_sb1+16 in %xmm12.
	@ We pin other values in _vpaes_key_preheat, so load them now.
	adr	r11, .Lk_sb1
	vld1.64	{q14,q15}, [r11]

	@ smear xmm7
	vext.8	q1, q4, q7, #12			@ vpslldq	\$4,	%xmm7,	%xmm1
	veor	q7, q7, q1			@ vpxor	%xmm1,	%xmm7,	%xmm7
	vext.8	q4, q4, q7, #8			@ vpslldq	\$8,	%xmm7,	%xmm4

	@ subbytes
	vand	q1, q0, $s0F			@ vpand		%xmm9,	%xmm0,	%xmm1		# 0 = k
	vshr.u8	q0, q0, #4			@ vpsrlb	\$4,	%xmm0,	%xmm0		# 1 = i
	 veor	q7, q7, q4			@ vpxor		%xmm4,	%xmm7,	%xmm7
	vtbl.8	q2#lo, {$invhi}, q1#lo		@ vpshufb	%xmm1,	%xmm11,	%xmm2		# 2 = a/k
	vtbl.8	q2#hi, {$invhi}, q1#hi
	veor	q1, q1, q0			@ vpxor		%xmm0,	%xmm1,	%xmm1		# 0 = j
	vtbl.8	q3#lo, {$invlo}, q0#lo		@ vpshufb	%xmm0, 	%xmm10,	%xmm3		# 3 = 1/i
	vtbl.8	q3#hi, {$invlo}, q0#hi
	veor	q3, q3, q2			@ vpxor		%xmm2,	%xmm3,	%xmm3		# 3 = iak = 1/i + a/k
	vtbl.8	q4#lo, {$invlo}, q1#lo		@ vpshufb	%xmm1,	%xmm10,	%xmm4		# 4 = 1/j
	vtbl.8	q4#hi, {$invlo}, q1#hi
	 veor	q7, q7, $s63			@ vpxor		.Lk_s63(%rip),	%xmm7,	%xmm7
	vtbl.8	q3#lo, {$invlo}, q3#lo		@ vpshufb	%xmm3,	%xmm10,	%xmm3		# 2 = 1/iak
	vtbl.8	q3#hi, {$invlo}, q3#hi
	veor	q4, q4, q2			@ vpxor		%xmm2,	%xmm4,	%xmm4		# 4 = jak = 1/j + a/k
	vtbl.8	q2#lo, {$invlo}, q4#lo		@ vpshufb	%xmm4,	%xmm10,	%xmm2		# 3 = 1/jak
	vtbl.8	q2#hi, {$invlo}, q4#hi
	veor	q3, q3, q1			@ vpxor		%xmm1,	%xmm3,	%xmm3		# 2 = io
	veor	q2, q2, q0			@ vpxor		%xmm0,	%xmm2,	%xmm2		# 3 = jo
	vtbl.8	q4#lo, {q15}, q3#lo		@ vpshufb	%xmm3,	%xmm13,	%xmm4		# 4 = sbou
	vtbl.8	q4#hi, {q15}, q3#hi
	vtbl.8	q1#lo, {q14}, q2#lo		@ vpshufb	%xmm2,	%xmm12,	%xmm1		# 0 = sb1t
	vtbl.8	q1#hi, {q14}, q2#hi
	veor	q1, q1, q4			@ vpxor		%xmm4,	%xmm1,	%xmm1		# 0 = sbox output

	@ add in smeared stuff
	veor	q0, q1, q7			@ vpxor	%xmm7,	%xmm1,	%xmm0
	veor	q7, q1, q7			@ vmovdqa	%xmm0,	%xmm7
	bx	lr
.size	_vpaes_schedule_round,.-_vpaes_schedule_round

@@
@@  .aes_schedule_transform
@@
@@  Linear-transform q0 according to tables at [r11]
@@
@@  Requires that q9 = 0x0F0F... as in preheat
@@  Output in q0
@@  Clobbers q1, q2, q14, q15
@@
.type	_vpaes_schedule_transform,%function
.align	4
_vpaes_schedule_transform:
	vld1.64	{q14,q15}, [r11]	@ vmovdqa	(%r11),	%xmm2 	# lo
					@ vmovdqa	16(%r11),	%xmm1 # hi
	vand	q1, q0, $s0F		@ vpand	%xmm9,	%xmm0,	%xmm1
	vshr.u8	q0, q0, #4		@ vpsrlb	\$4,	%xmm0,	%xmm0
	vtbl.8	q2#lo, {q14}, q1#lo	@ vpshufb	%xmm1,	%xmm2,	%xmm2
	vtbl.8	q2#hi, {q14}, q1#hi
	vtbl.8	q0#lo, {q15}, q0#lo	@ vpshufb	%xmm0,	%xmm1,	%xmm0
	vtbl.8	q0#hi, {q15}, q0#hi
	veor	q0, q0, q2		@ vpxor	%xmm2,	%xmm0,	%xmm0
	bx	lr
.size	_vpaes_schedule_transform,.-_vpaes_schedule_transform

@@
@@  .aes_schedule_mangle
@@
@@  Mangles q0 from (basis-transformed) standard version
@@  to our version.
@@
@@  On encrypt,
@@    xor with 0x63
@@    multiply by circulant 0,1,1,1
@@    apply shiftrows transform
@@
@@  On decrypt,
@@    xor with 0x63
@@    multiply by "inverse mixcolumns" circulant E,B,D,9
@@    deskew
@@    apply shiftrows transform
@@
@@
@@  Writes out to [r2], and increments or decrements it
@@  Keeps track of round number mod 4 in r8
@@  Preserves q0
@@  Clobbers q1-q5
@@
.type	_vpaes_schedule_mangle,%function
.align	4
_vpaes_schedule_mangle:
	tst	$dir, $dir
	vmov	q4, q0			@ vmovdqa	%xmm0,	%xmm4	# save xmm0 for later
	adr	r11, .Lk_mc_forward	@ Must be aligned to 8 mod 16.
	vld1.64	{q5}, [r11]		@ vmovdqa	.Lk_mc_forward(%rip),%xmm5

	@ encrypting
	@ Write to q2 so we do not overlap table and destination below.
	veor	q2, q0, $s63		@ vpxor		.Lk_s63(%rip),	%xmm0,	%xmm4
	add	$out, $out, #16		@ add		\$16,	%rdx
	vtbl.8	q4#lo, {q2}, q5#lo	@ vpshufb	%xmm5,	%xmm4,	%xmm4
	vtbl.8	q4#hi, {q2}, q5#hi
	vtbl.8	q1#lo, {q4}, q5#lo	@ vpshufb	%xmm5,	%xmm4,	%xmm1
	vtbl.8	q1#hi, {q4}, q5#hi
	vtbl.8	q3#lo, {q1}, q5#lo	@ vpshufb	%xmm5,	%xmm1,	%xmm3
	vtbl.8	q3#hi, {q1}, q5#hi
	veor	q4, q4, q1		@ vpxor		%xmm1,	%xmm4,	%xmm4
	vld1.64	{q1}, [r8]		@ vmovdqa	(%r8,%r10),	%xmm1
	veor	q3, q3, q4		@ vpxor		%xmm4,	%xmm3,	%xmm3

.Lschedule_mangle_both:
	@ Write to q2 so table and destination do not overlap.
	vtbl.8	q2#lo, {q3}, q1#lo	@ vpshufb	%xmm1,	%xmm3,	%xmm3
	vtbl.8	q2#hi, {q3}, q1#hi
	add	r8, r8, #64-16		@ add	\$-16,	%r8
	and	r8, r8, #~(1<<6)	@ and	\$0x30,	%r8
	vst1.64	{q2}, [$out]		@ vmovdqu	%xmm3,	(%rdx)
	bx	lr
.size	_vpaes_schedule_mangle,.-_vpaes_schedule_mangle

.globl	vpaes_set_encrypt_key
.type	vpaes_set_encrypt_key,%function
.align	4
vpaes_set_encrypt_key:
	stmdb	sp!, {r7-r11, lr}
	vstmdb	sp!, {d8-d15}

	lsr	r9, $bits, #5		@ shr	\$5,%eax
	add	r9, r9, #5		@ \$5,%eax
	str	r9, [$out,#240]		@ mov	%eax,240(%rdx)	# AES_KEY->rounds = nbits/32+5;

	mov	$dir, #0		@ mov	\$0,%ecx
	mov	r8, #0x30		@ mov	\$0x30,%r8d
	bl	_vpaes_schedule_core
	eor	r0, r0, r0

	vldmia	sp!, {d8-d15}
	ldmia	sp!, {r7-r11, pc}	@ return
.size	vpaes_set_encrypt_key,.-vpaes_set_encrypt_key
___
}

{
my ($out, $inp) = map("r$_", (0..1));
my ($s0F, $s63, $s63_raw, $mc_forward) = map("q$_", (9..12));

$code .= <<___;

@ Additional constants for converting to bsaes.
.type	_vpaes_convert_consts,%object
.align	4
_vpaes_convert_consts:
@ .Lk_opt_then_skew applies skew(opt(x)) XOR 0x63, where skew is the linear
@ transform in the AES S-box. 0x63 is incorporated into the low half of the
@ table. This was computed with the following script:
@
@   def u64s_to_u128(x, y):
@       return x | (y << 64)
@   def u128_to_u64s(w):
@       return w & ((1<<64)-1), w >> 64
@   def get_byte(w, i):
@       return (w >> (i*8)) & 0xff
@   def apply_table(table, b):
@       lo = b & 0xf
@       hi = b >> 4
@       return get_byte(table[0], lo) ^ get_byte(table[1], hi)
@   def opt(b):
@       table = [
@           u64s_to_u128(0xFF9F4929D6B66000, 0xF7974121DEBE6808),
@           u64s_to_u128(0x01EDBD5150BCEC00, 0xE10D5DB1B05C0CE0),
@       ]
@       return apply_table(table, b)
@   def rot_byte(b, n):
@       return 0xff & ((b << n) | (b >> (8-n)))
@   def skew(x):
@       return (x ^ rot_byte(x, 1) ^ rot_byte(x, 2) ^ rot_byte(x, 3) ^
@               rot_byte(x, 4))
@   table = [0, 0]
@   for i in range(16):
@       table[0] |= (skew(opt(i)) ^ 0x63) << (i*8)
@       table[1] |= skew(opt(i<<4)) << (i*8)
@   print("\t.quad\t0x%016x, 0x%016x" % u128_to_u64s(table[0]))
@   print("\t.quad\t0x%016x, 0x%016x" % u128_to_u64s(table[1]))
.Lk_opt_then_skew:
	.quad	0x9cb8436798bc4763, 0x6440bb9f6044bf9b
	.quad	0x1f30062936192f00, 0xb49bad829db284ab

@ void vpaes_encrypt_key_to_bsaes(AES_KEY *bsaes, const AES_KEY *vpaes);
.globl	vpaes_encrypt_key_to_bsaes
.type	vpaes_encrypt_key_to_bsaes,%function
.align	4
vpaes_encrypt_key_to_bsaes:
	stmdb	sp!, {r11, lr}

	@ See _vpaes_schedule_core for the key schedule logic. In particular,
	@ _vpaes_schedule_transform(.Lk_ipt) (section 2.2 of the paper),
	@ _vpaes_schedule_mangle (section 4.3), and .Lschedule_mangle_last
	@ contain the transformations not in the bsaes representation. This
	@ function inverts those transforms.
	@
	@ Note also that bsaes-armv7.pl expects aes-armv4.pl's key
	@ representation, which does not match the other aes_nohw_*
	@ implementations. The ARM aes_nohw_* stores each 32-bit word
	@ byteswapped, as a convenience for (unsupported) big-endian ARM, at the
	@ cost of extra REV and VREV32 operations in little-endian ARM.

	vmov.i8	$s0F, #0x0f		@ Required by _vpaes_schedule_transform
	adr	r2, .Lk_mc_forward	@ Must be aligned to 8 mod 16.
	add	r3, r2, 0x90		@ .Lk_sr+0x10-.Lk_mc_forward = 0x90 (Apple's toolchain doesn't support the expression)

	vld1.64	{$mc_forward}, [r2]
	vmov.i8	$s63, #0x5b		@ .Lk_s63 from vpaes-x86_64
	adr	r11, .Lk_opt		@ Must be aligned to 8 mod 16.
	vmov.i8	$s63_raw, #0x63		@ .LK_s63 without .Lk_ipt applied

	@ vpaes stores one fewer round count than bsaes, but the number of keys
	@ is the same.
	ldr	r2, [$inp,#240]
	add	r2, r2, #1
	str	r2, [$out,#240]

	@ The first key is transformed with _vpaes_schedule_transform(.Lk_ipt).
	@ Invert this with .Lk_opt.
	vld1.64	{q0}, [$inp]!
	bl	_vpaes_schedule_transform
	vrev32.8	q0, q0
	vst1.64	{q0}, [$out]!

	@ The middle keys have _vpaes_schedule_transform(.Lk_ipt) applied,
	@ followed by _vpaes_schedule_mangle. _vpaes_schedule_mangle XORs 0x63,
	@ multiplies by the circulant 0,1,1,1, then applies ShiftRows.
.Loop_enc_key_to_bsaes:
	vld1.64	{q0}, [$inp]!

	@ Invert the ShiftRows step (see .Lschedule_mangle_both). Note we cycle
	@ r3 in the opposite direction and start at .Lk_sr+0x10 instead of 0x30.
	@ We use r3 rather than r8 to avoid a callee-saved register.
	vld1.64	{q1}, [r3]
	vtbl.8  q2#lo, {q0}, q1#lo
	vtbl.8  q2#hi, {q0}, q1#hi
	add	r3, r3, #16
	and	r3, r3, #~(1<<6)
	vmov	q0, q2

	@ Handle the last key differently.
	subs	r2, r2, #1
	beq	.Loop_enc_key_to_bsaes_last

	@ Multiply by the circulant. This is its own inverse.
	vtbl.8	q1#lo, {q0}, $mc_forward#lo
	vtbl.8	q1#hi, {q0}, $mc_forward#hi
	vmov	q0, q1
	vtbl.8	q2#lo, {q1}, $mc_forward#lo
	vtbl.8	q2#hi, {q1}, $mc_forward#hi
	veor	q0, q0, q2
	vtbl.8	q1#lo, {q2}, $mc_forward#lo
	vtbl.8	q1#hi, {q2}, $mc_forward#hi
	veor	q0, q0, q1

	@ XOR and finish.
	veor	q0, q0, $s63
	bl	_vpaes_schedule_transform
	vrev32.8	q0, q0
	vst1.64	{q0}, [$out]!
	b	.Loop_enc_key_to_bsaes

.Loop_enc_key_to_bsaes_last:
	@ The final key does not have a basis transform (note
	@ .Lschedule_mangle_last inverts the original transform). It only XORs
	@ 0x63 and applies ShiftRows. The latter was already inverted in the
	@ loop. Note that, because we act on the original representation, we use
	@ $s63_raw, not $s63.
	veor	q0, q0, $s63_raw
	vrev32.8	q0, q0
	vst1.64	{q0}, [$out]

	@ Wipe registers which contained key material.
	veor	q0, q0, q0
	veor	q1, q1, q1
	veor	q2, q2, q2

	ldmia	sp!, {r11, pc}	@ return
.size	vpaes_encrypt_key_to_bsaes,.-vpaes_encrypt_key_to_bsaes
___
}

{
# Register-passed parameters.
my ($inp, $out, $len, $key) = map("r$_", 0..3);
# Temporaries. _vpaes_encrypt_core already uses r8..r11, so overlap $ivec and
# $tmp. $ctr is r7 because it must be preserved across calls.
my ($ctr, $ivec, $tmp) = map("r$_", 7..9);

# void vpaes_ctr32_encrypt_blocks(const uint8_t *in, uint8_t *out, size_t len,
#                                 const AES_KEY *key, const uint8_t ivec[16]);
$code .= <<___;
.globl	vpaes_ctr32_encrypt_blocks
.type	vpaes_ctr32_encrypt_blocks,%function
.align	4
vpaes_ctr32_encrypt_blocks:
	mov	ip, sp
	stmdb	sp!, {r7-r11, lr}
	@ This function uses q4-q7 (d8-d15), which are callee-saved.
	vstmdb	sp!, {d8-d15}

	cmp	$len, #0
	@ $ivec is passed on the stack.
	ldr	$ivec, [ip]
	beq	.Lctr32_done

	@ _vpaes_encrypt_core expects the key in r2, so swap $len and $key.
	mov	$tmp, $key
	mov	$key, $len
	mov	$len, $tmp
___
my ($len, $key) = ($key, $len);
$code .= <<___;

	@ Load the IV and counter portion.
	ldr	$ctr, [$ivec, #12]
	vld1.8	{q7}, [$ivec]

	bl	_vpaes_preheat
	rev	$ctr, $ctr		@ The counter is big-endian.

.Lctr32_loop:
	vmov	q0, q7
	vld1.8	{q6}, [$inp]!		@ Load input ahead of time
	bl	_vpaes_encrypt_core
	veor	q0, q0, q6		@ XOR input and result
	vst1.8	{q0}, [$out]!
	subs	$len, $len, #1
	@ Update the counter.
	add	$ctr, $ctr, #1
	rev	$tmp, $ctr
	vmov.32	q7#hi[1], $tmp
	bne	.Lctr32_loop

.Lctr32_done:
	vldmia	sp!, {d8-d15}
	ldmia	sp!, {r7-r11, pc}	@ return
.size	vpaes_ctr32_encrypt_blocks,.-vpaes_ctr32_encrypt_blocks
___
}

foreach (split("\n",$code)) {
	s/\bq([0-9]+)#(lo|hi)/sprintf "d%d",2*$1+($2 eq "hi")/geo;
	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
