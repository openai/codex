#! /usr/bin/env perl
# Copyright 2014-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project. The module is, however, dual licensed under OpenSSL and
# CRYPTOGAMS licenses depending on where you obtain it. For further
# details see http://www.openssl.org/~appro/cryptogams/.
# ====================================================================
#
# This module implements support for ARMv8 AES instructions. The
# module is endian-agnostic in sense that it supports both big- and
# little-endian cases. As does it support both 32- and 64-bit modes
# of operation. Latter is achieved by limiting amount of utilized
# registers to 16, which implies additional NEON load and integer
# instructions. This has no effect on mighty Apple A7, where results
# are literally equal to the theoretical estimates based on AES
# instruction latencies and issue rates. On Cortex-A53, an in-order
# execution core, this costs up to 10-15%, which is partially
# compensated by implementing dedicated code path for 128-bit
# CBC encrypt case. On Cortex-A57 parallelizable mode performance
# seems to be limited by sheer amount of NEON instructions...
#
# April 2019
#
# Key to performance of parallelize-able modes is round instruction
# interleaving. But which factor to use? There is optimal one for
# each combination of instruction latency and issue rate, beyond
# which increasing interleave factor doesn't pay off. While on cons
# side we have code size increase and resource waste on platforms for
# which interleave factor is too high. In other words you want it to
# be just right. So far interleave factor of 3x was serving well all
# platforms. But for ThunderX2 optimal interleave factor was measured
# to be 5x...
#
# Performance in cycles per byte processed with 128-bit key:
#
#		CBC enc		CBC dec		CTR
# Apple A7	2.39		1.20		1.20
# Cortex-A53	1.32		1.17/1.29(**)	1.36/1.46
# Cortex-A57(*)	1.95		0.82/0.85	0.89/0.93
# Cortex-A72	1.33		0.85/0.88	0.92/0.96
# Denver	1.96		0.65/0.86	0.76/0.80
# Mongoose	1.33		1.23/1.20	1.30/1.20
# Kryo		1.26		0.87/0.94	1.00/1.00
# ThunderX2	5.95		1.25		1.30
#
# (*)	original 3.64/1.34/1.32 results were for r0p0 revision
#	and are still same even for updated module;
# (**)	numbers after slash are for 32-bit code, which is 3x-
#	interleaved;

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

$prefix="aes_v8";

$_byte = ($flavour =~ /win/ ? "DCB" : ".byte");

$code=<<___;
#include "arm_arch.h"

#if __ARM_MAX_ARCH__>=7
___
$code.=".arch	armv8-a+crypto\n.text\n"		if ($flavour =~ /64/);
$code.=<<___						if ($flavour !~ /64/);
.arch	armv7-a	// don't confuse not-so-latest binutils with argv8 :-)
.fpu	neon
#ifdef	__thumb2__
.syntax	unified
.thumb
# define INST(a,b,c,d)	$_byte	c,d|0xc,a,b
#else
.code	32
# define INST(a,b,c,d)	$_byte	a,b,c,d
#endif

.text
___

# Assembler mnemonics are an eclectic mix of 32- and 64-bit syntax,
# NEON is mostly 32-bit mnemonics, integer - mostly 64. Goal is to
# maintain both 32- and 64-bit codes within single module and
# transliterate common code to either flavour with regex vodoo.
#
{{{
my ($inp,$bits,$out,$ptr,$rounds)=("x0","w1","x2","x3","w12");
my ($zero,$rcon,$mask,$in0,$in1,$tmp,$key)=
	$flavour=~/64/? map("q$_",(0..6)) : map("q$_",(0..3,8..10));


#
# This file generates .s file for 64-bit and 32-bit CPUs.
# We don't implement .rodata on 32-bit CPUs yet.
#
$code.=".rodata\n"	if ($flavour =~ /64/);
$code.=<<___;
.align	5
.Lrcon:
.long	0x01,0x01,0x01,0x01
.long	0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d	// rotate-n-splat
.long	0x1b,0x1b,0x1b,0x1b
___
$code.=".previous\n"	if ($flavour =~ /64/);

$code.=<<___;
.globl	${prefix}_set_encrypt_key
.type	${prefix}_set_encrypt_key,%function
.align	5
${prefix}_set_encrypt_key:
.Lenc_key:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
___
$code.=<<___;
	mov	$ptr,#-1
	cmp	$inp,#0
	b.eq	.Lenc_key_abort
	cmp	$out,#0
	b.eq	.Lenc_key_abort
	mov	$ptr,#-2
	cmp	$bits,#128
	b.lt	.Lenc_key_abort
	cmp	$bits,#256
	b.gt	.Lenc_key_abort
	tst	$bits,#0x3f
	b.ne	.Lenc_key_abort

___
$code.=<<___	if ($flavour =~ /64/);
	adrp	$ptr,.Lrcon
	add	$ptr,$ptr,:lo12:.Lrcon
___
$code.=<<___	if ($flavour !~ /64/);
	adr	$ptr,.Lrcon
___
$code.=<<___;
	cmp	$bits,#192

	veor	$zero,$zero,$zero
	vld1.8	{$in0},[$inp],#16
	mov	$bits,#8		// reuse $bits
	vld1.32	{$rcon,$mask},[$ptr],#32

	b.lt	.Loop128
	b.eq	.L192
	b	.L256

.align	4
.Loop128:
	vtbl.8	$key,{$in0},$mask
	vext.8	$tmp,$zero,$in0,#12
	vst1.32	{$in0},[$out],#16
	aese	$key,$zero
	subs	$bits,$bits,#1

	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	 veor	$key,$key,$rcon
	veor	$in0,$in0,$tmp
	vshl.u8	$rcon,$rcon,#1
	veor	$in0,$in0,$key
	b.ne	.Loop128

	vld1.32	{$rcon},[$ptr]

	vtbl.8	$key,{$in0},$mask
	vext.8	$tmp,$zero,$in0,#12
	vst1.32	{$in0},[$out],#16
	aese	$key,$zero

	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	 veor	$key,$key,$rcon
	veor	$in0,$in0,$tmp
	vshl.u8	$rcon,$rcon,#1
	veor	$in0,$in0,$key

	vtbl.8	$key,{$in0},$mask
	vext.8	$tmp,$zero,$in0,#12
	vst1.32	{$in0},[$out],#16
	aese	$key,$zero

	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	 veor	$key,$key,$rcon
	veor	$in0,$in0,$tmp
	veor	$in0,$in0,$key
	vst1.32	{$in0},[$out]
	add	$out,$out,#0x50

	mov	$rounds,#10
	b	.Ldone

.align	4
.L192:
	vld1.8	{$in1},[$inp],#8
	vmov.i8	$key,#8			// borrow $key
	vst1.32	{$in0},[$out],#16
	vsub.i8	$mask,$mask,$key	// adjust the mask

.Loop192:
	vtbl.8	$key,{$in1},$mask
	vext.8	$tmp,$zero,$in0,#12
#ifdef __ARMEB__
	vst1.32	{$in1},[$out],#16
	sub	$out,$out,#8
#else
	vst1.32	{$in1},[$out],#8
#endif
	aese	$key,$zero
	subs	$bits,$bits,#1

	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp

	vdup.32	$tmp,${in0}[3]
	veor	$tmp,$tmp,$in1
	 veor	$key,$key,$rcon
	vext.8	$in1,$zero,$in1,#12
	vshl.u8	$rcon,$rcon,#1
	veor	$in1,$in1,$tmp
	veor	$in0,$in0,$key
	veor	$in1,$in1,$key
	vst1.32	{$in0},[$out],#16
	b.ne	.Loop192

	mov	$rounds,#12
	add	$out,$out,#0x20
	b	.Ldone

.align	4
.L256:
	vld1.8	{$in1},[$inp]
	mov	$bits,#7
	mov	$rounds,#14
	vst1.32	{$in0},[$out],#16

.Loop256:
	vtbl.8	$key,{$in1},$mask
	vext.8	$tmp,$zero,$in0,#12
	vst1.32	{$in1},[$out],#16
	aese	$key,$zero
	subs	$bits,$bits,#1

	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in0,$in0,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	 veor	$key,$key,$rcon
	veor	$in0,$in0,$tmp
	vshl.u8	$rcon,$rcon,#1
	veor	$in0,$in0,$key
	vst1.32	{$in0},[$out],#16
	b.eq	.Ldone

	vdup.32	$key,${in0}[3]		// just splat
	vext.8	$tmp,$zero,$in1,#12
	aese	$key,$zero

	veor	$in1,$in1,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in1,$in1,$tmp
	vext.8	$tmp,$zero,$tmp,#12
	veor	$in1,$in1,$tmp

	veor	$in1,$in1,$key
	b	.Loop256

.Ldone:
	str	$rounds,[$out]
	mov	$ptr,#0

.Lenc_key_abort:
	mov	x0,$ptr			// return value
	`"ldr	x29,[sp],#16"		if ($flavour =~ /64/)`
	ret
.size	${prefix}_set_encrypt_key,.-${prefix}_set_encrypt_key

.globl	${prefix}_set_decrypt_key
.type	${prefix}_set_decrypt_key,%function
.align	5
${prefix}_set_decrypt_key:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
___
$code.=<<___	if ($flavour !~ /64/);
	stmdb	sp!,{r4,lr}
___
$code.=<<___;
	bl	.Lenc_key

	cmp	x0,#0
	b.ne	.Ldec_key_abort

	sub	$out,$out,#240		// restore original $out
	mov	x4,#-16
	add	$inp,$out,x12,lsl#4	// end of key schedule

	vld1.32	{v0.16b},[$out]
	vld1.32	{v1.16b},[$inp]
	vst1.32	{v0.16b},[$inp],x4
	vst1.32	{v1.16b},[$out],#16

.Loop_imc:
	vld1.32	{v0.16b},[$out]
	vld1.32	{v1.16b},[$inp]
	aesimc	v0.16b,v0.16b
	aesimc	v1.16b,v1.16b
	vst1.32	{v0.16b},[$inp],x4
	vst1.32	{v1.16b},[$out],#16
	cmp	$inp,$out
	b.hi	.Loop_imc

	vld1.32	{v0.16b},[$out]
	aesimc	v0.16b,v0.16b
	vst1.32	{v0.16b},[$inp]

	eor	x0,x0,x0		// return value
.Ldec_key_abort:
___
$code.=<<___	if ($flavour !~ /64/);
	ldmia	sp!,{r4,pc}
___
$code.=<<___	if ($flavour =~ /64/);
	ldp	x29,x30,[sp],#16
	AARCH64_VALIDATE_LINK_REGISTER
	ret
___
$code.=<<___;
.size	${prefix}_set_decrypt_key,.-${prefix}_set_decrypt_key
___
}}}
{{{
sub gen_block () {
my $dir = shift;
my ($e,$mc) = $dir eq "en" ? ("e","mc") : ("d","imc");
my ($inp,$out,$key)=map("x$_",(0..2));
my $rounds="w3";
my ($rndkey0,$rndkey1,$inout)=map("q$_",(0..3));

$code.=<<___;
.globl	${prefix}_${dir}crypt
.type	${prefix}_${dir}crypt,%function
.align	5
${prefix}_${dir}crypt:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
___
$code.=<<___;
	ldr	$rounds,[$key,#240]
	vld1.32	{$rndkey0},[$key],#16
	vld1.8	{$inout},[$inp]
	sub	$rounds,$rounds,#2
	vld1.32	{$rndkey1},[$key],#16

.Loop_${dir}c:
	aes$e	$inout,$rndkey0
	aes$mc	$inout,$inout
	vld1.32	{$rndkey0},[$key],#16
	subs	$rounds,$rounds,#2
	aes$e	$inout,$rndkey1
	aes$mc	$inout,$inout
	vld1.32	{$rndkey1},[$key],#16
	b.gt	.Loop_${dir}c

	aes$e	$inout,$rndkey0
	aes$mc	$inout,$inout
	vld1.32	{$rndkey0},[$key]
	aes$e	$inout,$rndkey1
	veor	$inout,$inout,$rndkey0

	vst1.8	{$inout},[$out]
	ret
.size	${prefix}_${dir}crypt,.-${prefix}_${dir}crypt
___
}
&gen_block("en");
&gen_block("de");
}}}

# Performance in cycles per byte.
# Processed with AES-ECB different key size.
# It shows the value before and after optimization as below:
# (before/after):
#
#		AES-128-ECB		AES-192-ECB		AES-256-ECB
# Cortex-A57	1.85/0.82		2.16/0.96		2.47/1.10
# Cortex-A72	1.64/0.85		1.82/0.99		2.13/1.14

# Optimization is implemented by loop unrolling and interleaving.
# Commonly, we choose the unrolling factor as 5, if the input
# data size smaller than 5 blocks, but not smaller than 3 blocks,
# choose 3 as the unrolling factor.
# If the input data size dsize >= 5*16 bytes, then take 5 blocks
# as one iteration, every loop the left size lsize -= 5*16.
# If 5*16 > lsize >= 3*16 bytes, take 3 blocks as one iteration,
# every loop lsize -=3*16.
# If lsize < 3*16 bytes, treat them as the tail, interleave the
# two blocks AES instructions.
# There is one special case, if the original input data size dsize
# = 16 bytes, we will treat it separately to improve the
# performance: one independent code block without LR, FP load and
# store, just looks like what the original ECB implementation does.

{{{
my ($inp,$out,$len,$key)=map("x$_",(0..3));
my ($enc,$rounds,$cnt,$key_,$step)=("w4","w5","w6","x7","x8");
my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$tmp2,$rndlast)=map("q$_",(0..7));

my ($dat,$tmp,$rndzero_n_last)=($dat0,$tmp0,$tmp1);

### q7	last round key
### q10-q15	q7 Last 7 round keys
### q8-q9	preloaded round keys except last 7 keys for big size
### q5, q6, q8-q9	preloaded round keys except last 7 keys for only 16 byte

{
my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

my ($dat3,$in3,$tmp3);	# used only in 64-bit mode
my ($dat4,$in4,$tmp4);
if ($flavour =~ /64/) {
    ($dat2,$dat3,$dat4,$in2,$in3,$in4,$tmp3,$tmp4)=map("q$_",(16..23));
}

$code.=<<___;
.globl	${prefix}_ecb_encrypt
.type	${prefix}_ecb_encrypt,%function
.align	5
${prefix}_ecb_encrypt:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
	subs	$len,$len,#16
	// Original input data size bigger than 16, jump to big size processing.
	b.ne    .Lecb_big_size
	vld1.8	{$dat0},[$inp]
	cmp	$enc,#0					// en- or decrypting?
	ldr	$rounds,[$key,#240]
	vld1.32	{q5-q6},[$key],#32			// load key schedule...

	b.eq .Lecb_small_dec
	aese	$dat0,q5
	aesmc	$dat0,$dat0
	vld1.32	{q8-q9},[$key],#32			// load key schedule...
	aese	$dat0,q6
	aesmc	$dat0,$dat0
	subs	$rounds,$rounds,#10			// if rounds==10, jump to aes-128-ecb processing
	b.eq    .Lecb_128_enc
.Lecb_round_loop:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	vld1.32	{q8},[$key],#16				// load key schedule...
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	vld1.32	{q9},[$key],#16				// load key schedule...
	subs	$rounds,$rounds,#2			// bias
	b.gt    .Lecb_round_loop
.Lecb_128_enc:
	vld1.32	{q10-q11},[$key],#32		// load key schedule...
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	vld1.32	{q12-q13},[$key],#32		// load key schedule...
	aese	$dat0,q10
	aesmc	$dat0,$dat0
	aese	$dat0,q11
	aesmc	$dat0,$dat0
	vld1.32	{q14-q15},[$key],#32		// load key schedule...
	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat0,q13
	aesmc	$dat0,$dat0
	vld1.32	{$rndlast},[$key]
	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat0,q15
	veor	$dat0,$dat0,$rndlast
	vst1.8	{$dat0},[$out]
	b	.Lecb_Final_abort
.Lecb_small_dec:
	aesd	$dat0,q5
	aesimc	$dat0,$dat0
	vld1.32	{q8-q9},[$key],#32			// load key schedule...
	aesd	$dat0,q6
	aesimc	$dat0,$dat0
	subs	$rounds,$rounds,#10			// bias
	b.eq    .Lecb_128_dec
.Lecb_dec_round_loop:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	vld1.32	{q8},[$key],#16				// load key schedule...
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	vld1.32	{q9},[$key],#16				// load key schedule...
	subs	$rounds,$rounds,#2			// bias
	b.gt    .Lecb_dec_round_loop
.Lecb_128_dec:
	vld1.32	{q10-q11},[$key],#32		// load key schedule...
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	vld1.32	{q12-q13},[$key],#32		// load key schedule...
	aesd	$dat0,q10
	aesimc	$dat0,$dat0
	aesd	$dat0,q11
	aesimc	$dat0,$dat0
	vld1.32	{q14-q15},[$key],#32		// load key schedule...
	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	vld1.32	{$rndlast},[$key]
	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat0,q15
	veor	$dat0,$dat0,$rndlast
	vst1.8	{$dat0},[$out]
	b	.Lecb_Final_abort
.Lecb_big_size:
___
$code.=<<___	if ($flavour =~ /64/);
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
___
$code.=<<___	if ($flavour !~ /64/);
	mov	ip,sp
	stmdb	sp!,{r4-r8,lr}
	vstmdb	sp!,{d8-d15}			@ ABI specification says so
	ldmia	ip,{r4-r5}			@ load remaining args
	subs	$len,$len,#16
___
$code.=<<___;
	mov	$step,#16
	b.lo	.Lecb_done
	cclr	$step,eq

	cmp	$enc,#0					// en- or decrypting?
	ldr	$rounds,[$key,#240]
	and	$len,$len,#-16
	vld1.8	{$dat},[$inp],$step

	vld1.32	{q8-q9},[$key]				// load key schedule...
	sub	$rounds,$rounds,#6
	add	$key_,$key,x5,lsl#4				// pointer to last 7 round keys
	sub	$rounds,$rounds,#2
	vld1.32	{q10-q11},[$key_],#32
	vld1.32	{q12-q13},[$key_],#32
	vld1.32	{q14-q15},[$key_],#32
	vld1.32	{$rndlast},[$key_]

	add	$key_,$key,#32
	mov	$cnt,$rounds
	b.eq	.Lecb_dec

	vld1.8	{$dat1},[$inp],#16
	subs	$len,$len,#32				// bias
	add	$cnt,$rounds,#2
	vorr	$in1,$dat1,$dat1
	vorr	$dat2,$dat1,$dat1
	vorr	$dat1,$dat,$dat
	b.lo	.Lecb_enc_tail

	vorr	$dat1,$in1,$in1
	vld1.8	{$dat2},[$inp],#16
___
$code.=<<___	if ($flavour =~ /64/);
	cmp	$len,#32
	b.lo	.Loop3x_ecb_enc

	vld1.8	{$dat3},[$inp],#16
	vld1.8	{$dat4},[$inp],#16
	sub	$len,$len,#32				// bias
	mov	$cnt,$rounds

.Loop5x_ecb_enc:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat3,q8
	aesmc	$dat3,$dat3
	aese	$dat4,q8
	aesmc	$dat4,$dat4
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat3,q9
	aesmc	$dat3,$dat3
	aese	$dat4,q9
	aesmc	$dat4,$dat4
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop5x_ecb_enc

	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat3,q8
	aesmc	$dat3,$dat3
	aese	$dat4,q8
	aesmc	$dat4,$dat4
	cmp	$len,#0x40					// because .Lecb_enc_tail4x
	sub	$len,$len,#0x50

	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat3,q9
	aesmc	$dat3,$dat3
	aese	$dat4,q9
	aesmc	$dat4,$dat4
	csel	x6,xzr,$len,gt			// borrow x6, $cnt, "gt" is not typo
	mov	$key_,$key

	aese	$dat0,q10
	aesmc	$dat0,$dat0
	aese	$dat1,q10
	aesmc	$dat1,$dat1
	aese	$dat2,q10
	aesmc	$dat2,$dat2
	aese	$dat3,q10
	aesmc	$dat3,$dat3
	aese	$dat4,q10
	aesmc	$dat4,$dat4
	add	$inp,$inp,x6				// $inp is adjusted in such way that
							// at exit from the loop $dat1-$dat4
							// are loaded with last "words"
	add	x6,$len,#0x60		    // because .Lecb_enc_tail4x

	aese	$dat0,q11
	aesmc	$dat0,$dat0
	aese	$dat1,q11
	aesmc	$dat1,$dat1
	aese	$dat2,q11
	aesmc	$dat2,$dat2
	aese	$dat3,q11
	aesmc	$dat3,$dat3
	aese	$dat4,q11
	aesmc	$dat4,$dat4

	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	aese	$dat3,q12
	aesmc	$dat3,$dat3
	aese	$dat4,q12
	aesmc	$dat4,$dat4

	aese	$dat0,q13
	aesmc	$dat0,$dat0
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	aese	$dat3,q13
	aesmc	$dat3,$dat3
	aese	$dat4,q13
	aesmc	$dat4,$dat4

	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	aese	$dat3,q14
	aesmc	$dat3,$dat3
	aese	$dat4,q14
	aesmc	$dat4,$dat4

	aese	$dat0,q15
	vld1.8	{$in0},[$inp],#16
	aese	$dat1,q15
	vld1.8	{$in1},[$inp],#16
	aese	$dat2,q15
	vld1.8	{$in2},[$inp],#16
	aese	$dat3,q15
	vld1.8	{$in3},[$inp],#16
	aese	$dat4,q15
	vld1.8	{$in4},[$inp],#16
	cbz	x6,.Lecb_enc_tail4x
	vld1.32 {q8},[$key_],#16			// re-pre-load rndkey[0]
	veor	$tmp0,$rndlast,$dat0
	vorr	$dat0,$in0,$in0
	veor	$tmp1,$rndlast,$dat1
	vorr	$dat1,$in1,$in1
	veor	$tmp2,$rndlast,$dat2
	vorr	$dat2,$in2,$in2
	veor	$tmp3,$rndlast,$dat3
	vorr	$dat3,$in3,$in3
	veor	$tmp4,$rndlast,$dat4
	vst1.8	{$tmp0},[$out],#16
	vorr	$dat4,$in4,$in4
	vst1.8	{$tmp1},[$out],#16
	mov	$cnt,$rounds
	vst1.8	{$tmp2},[$out],#16
	vld1.32 {q9},[$key_],#16			// re-pre-load rndkey[1]
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16
	b.hs	.Loop5x_ecb_enc

	add	$len,$len,#0x50
	cbz	$len,.Lecb_done

	add	$cnt,$rounds,#2
	subs	$len,$len,#0x30
	vorr	$dat0,$in2,$in2
	vorr	$dat1,$in3,$in3
	vorr	$dat2,$in4,$in4
	b.lo	.Lecb_enc_tail

	b	.Loop3x_ecb_enc

.align	4
.Lecb_enc_tail4x:
	veor	$tmp1,$rndlast,$dat1
	veor	$tmp2,$rndlast,$dat2
	veor	$tmp3,$rndlast,$dat3
	veor	$tmp4,$rndlast,$dat4
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16

	b	.Lecb_done
.align	4
___
$code.=<<___;
.Loop3x_ecb_enc:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop3x_ecb_enc

	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	subs	$len,$len,#0x30
	mov.lo	x6,$len				// x6, $cnt, is zero at this point
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	add	$inp,$inp,x6			// $inp is adjusted in such way that
						// at exit from the loop $dat1-$dat2
						// are loaded with last "words"
	mov	$key_,$key
	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	vld1.8	{$in0},[$inp],#16
	aese	$dat0,q13
	aesmc	$dat0,$dat0
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	vld1.8	{$in1},[$inp],#16
	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	vld1.8	{$in2},[$inp],#16
	aese	$dat0,q15
	aese	$dat1,q15
	aese	$dat2,q15
	vld1.32 {q8},[$key_],#16		// re-pre-load rndkey[0]
	add	$cnt,$rounds,#2
	veor	$tmp0,$rndlast,$dat0
	veor	$tmp1,$rndlast,$dat1
	veor	$dat2,$dat2,$rndlast
	vld1.32 {q9},[$key_],#16		// re-pre-load rndkey[1]
	vst1.8	{$tmp0},[$out],#16
	vorr	$dat0,$in0,$in0
	vst1.8	{$tmp1},[$out],#16
	vorr	$dat1,$in1,$in1
	vst1.8	{$dat2},[$out],#16
	vorr	$dat2,$in2,$in2
	b.hs	.Loop3x_ecb_enc

	cmn	$len,#0x30
	b.eq	.Lecb_done
	nop

.Lecb_enc_tail:
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lecb_enc_tail

	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	cmn	$len,#0x20
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	aese	$dat1,q15
	aese	$dat2,q15
	b.eq	.Lecb_enc_one
	veor	$tmp1,$rndlast,$dat1
	veor	$tmp2,$rndlast,$dat2
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	b	.Lecb_done

.Lecb_enc_one:
	veor	$tmp1,$rndlast,$dat2
	vst1.8	{$tmp1},[$out],#16
	b	.Lecb_done
___

$code.=<<___;
.align	5
.Lecb_dec:
	vld1.8	{$dat1},[$inp],#16
	subs	$len,$len,#32			// bias
	add	$cnt,$rounds,#2
	vorr	$in1,$dat1,$dat1
	vorr	$dat2,$dat1,$dat1
	vorr	$dat1,$dat,$dat
	b.lo	.Lecb_dec_tail

	vorr	$dat1,$in1,$in1
	vld1.8	{$dat2},[$inp],#16
___
$code.=<<___	if ($flavour =~ /64/);
	cmp	$len,#32
	b.lo	.Loop3x_ecb_dec

	vld1.8	{$dat3},[$inp],#16
	vld1.8	{$dat4},[$inp],#16
	sub	$len,$len,#32				// bias
	mov	$cnt,$rounds

.Loop5x_ecb_dec:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop5x_ecb_dec

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	cmp	$len,#0x40				// because .Lecb_tail4x
	sub	$len,$len,#0x50

	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	csel	x6,xzr,$len,gt		// borrow x6, $cnt, "gt" is not typo
	mov	$key_,$key

	aesd	$dat0,q10
	aesimc	$dat0,$dat0
	aesd	$dat1,q10
	aesimc	$dat1,$dat1
	aesd	$dat2,q10
	aesimc	$dat2,$dat2
	aesd	$dat3,q10
	aesimc	$dat3,$dat3
	aesd	$dat4,q10
	aesimc	$dat4,$dat4
	add	$inp,$inp,x6				// $inp is adjusted in such way that
							// at exit from the loop $dat1-$dat4
							// are loaded with last "words"
	add	x6,$len,#0x60			// because .Lecb_tail4x

	aesd	$dat0,q11
	aesimc	$dat0,$dat0
	aesd	$dat1,q11
	aesimc	$dat1,$dat1
	aesd	$dat2,q11
	aesimc	$dat2,$dat2
	aesd	$dat3,q11
	aesimc	$dat3,$dat3
	aesd	$dat4,q11
	aesimc	$dat4,$dat4

	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	aesd	$dat3,q12
	aesimc	$dat3,$dat3
	aesd	$dat4,q12
	aesimc	$dat4,$dat4

	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	aesd	$dat3,q13
	aesimc	$dat3,$dat3
	aesd	$dat4,q13
	aesimc	$dat4,$dat4

	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	aesd	$dat3,q14
	aesimc	$dat3,$dat3
	aesd	$dat4,q14
	aesimc	$dat4,$dat4

	aesd	$dat0,q15
	vld1.8	{$in0},[$inp],#16
	aesd	$dat1,q15
	vld1.8	{$in1},[$inp],#16
	aesd	$dat2,q15
	vld1.8	{$in2},[$inp],#16
	aesd	$dat3,q15
	vld1.8	{$in3},[$inp],#16
	aesd	$dat4,q15
	vld1.8	{$in4},[$inp],#16
	cbz	x6,.Lecb_tail4x
	vld1.32 {q8},[$key_],#16			// re-pre-load rndkey[0]
	veor	$tmp0,$rndlast,$dat0
	vorr	$dat0,$in0,$in0
	veor	$tmp1,$rndlast,$dat1
	vorr	$dat1,$in1,$in1
	veor	$tmp2,$rndlast,$dat2
	vorr	$dat2,$in2,$in2
	veor	$tmp3,$rndlast,$dat3
	vorr	$dat3,$in3,$in3
	veor	$tmp4,$rndlast,$dat4
	vst1.8	{$tmp0},[$out],#16
	vorr	$dat4,$in4,$in4
	vst1.8	{$tmp1},[$out],#16
	mov	$cnt,$rounds
	vst1.8	{$tmp2},[$out],#16
	vld1.32 {q9},[$key_],#16			// re-pre-load rndkey[1]
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16
	b.hs	.Loop5x_ecb_dec

	add	$len,$len,#0x50
	cbz	$len,.Lecb_done

	add	$cnt,$rounds,#2
	subs	$len,$len,#0x30
	vorr	$dat0,$in2,$in2
	vorr	$dat1,$in3,$in3
	vorr	$dat2,$in4,$in4
	b.lo	.Lecb_dec_tail

	b	.Loop3x_ecb_dec

.align	4
.Lecb_tail4x:
	veor	$tmp1,$rndlast,$dat1
	veor	$tmp2,$rndlast,$dat2
	veor	$tmp3,$rndlast,$dat3
	veor	$tmp4,$rndlast,$dat4
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16

	b	.Lecb_done
.align	4
___
$code.=<<___;
.Loop3x_ecb_dec:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop3x_ecb_dec

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	subs	$len,$len,#0x30
	mov.lo	x6,$len				// x6, $cnt, is zero at this point
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	add	$inp,$inp,x6 			// $inp is adjusted in such way that
						// at exit from the loop $dat1-$dat2
						// are loaded with last "words"
	mov	$key_,$key
	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	vld1.8	{$in0},[$inp],#16
	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	vld1.8	{$in1},[$inp],#16
	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	vld1.8	{$in2},[$inp],#16
	aesd	$dat0,q15
	aesd	$dat1,q15
	aesd	$dat2,q15
	vld1.32 {q8},[$key_],#16			// re-pre-load rndkey[0]
	add	$cnt,$rounds,#2
	veor	$tmp0,$rndlast,$dat0
	veor	$tmp1,$rndlast,$dat1
	veor	$dat2,$dat2,$rndlast
	vld1.32 {q9},[$key_],#16			// re-pre-load rndkey[1]
	vst1.8	{$tmp0},[$out],#16
	vorr	$dat0,$in0,$in0
	vst1.8	{$tmp1},[$out],#16
	vorr	$dat1,$in1,$in1
	vst1.8	{$dat2},[$out],#16
	vorr	$dat2,$in2,$in2
	b.hs	.Loop3x_ecb_dec

	cmn	$len,#0x30
	b.eq	.Lecb_done
	nop

.Lecb_dec_tail:
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lecb_dec_tail

	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	cmn	$len,#0x20
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	aesd	$dat1,q15
	aesd	$dat2,q15
	b.eq	.Lecb_dec_one
	veor	$tmp1,$rndlast,$dat1
	veor	$tmp2,$rndlast,$dat2
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	b	.Lecb_done

.Lecb_dec_one:
	veor	$tmp1,$rndlast,$dat2
	vst1.8	{$tmp1},[$out],#16

.Lecb_done:
___
}
$code.=<<___	if ($flavour !~ /64/);
	vldmia	sp!,{d8-d15}
	ldmia	sp!,{r4-r8,pc}
___
$code.=<<___	if ($flavour =~ /64/);
	ldr	x29,[sp],#16
___
$code.=<<___	if ($flavour =~ /64/);
.Lecb_Final_abort:
	ret
___
$code.=<<___;
.size	${prefix}_ecb_encrypt,.-${prefix}_ecb_encrypt
___
}}}
{{{
my ($inp,$out,$len,$key,$ivp)=map("x$_",(0..4)); my $enc="w5";
my ($rounds,$cnt,$key_,$step,$step1)=($enc,"w6","x7","x8","x12");
my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$ivec,$rndlast)=map("q$_",(0..7));

my ($dat,$tmp,$rndzero_n_last)=($dat0,$tmp0,$tmp1);
my ($key4,$key5,$key6,$key7)=("x6","x12","x14",$key);

### q8-q15	preloaded key schedule

$code.=<<___;
.globl	${prefix}_cbc_encrypt
.type	${prefix}_cbc_encrypt,%function
.align	5
${prefix}_cbc_encrypt:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
___
$code.=<<___	if ($flavour !~ /64/);
	mov	ip,sp
	stmdb	sp!,{r4-r8,lr}
	vstmdb	sp!,{d8-d15}            @ ABI specification says so
	ldmia	ip,{r4-r5}		@ load remaining args
___
$code.=<<___;
	subs	$len,$len,#16
	mov	$step,#16
	b.lo	.Lcbc_abort
	cclr	$step,eq

	cmp	$enc,#0			// en- or decrypting?
	ldr	$rounds,[$key,#240]
	and	$len,$len,#-16
	vld1.8	{$ivec},[$ivp]
	vld1.8	{$dat},[$inp],$step

	vld1.32	{q8-q9},[$key]		// load key schedule...
	sub	$rounds,$rounds,#6
	add	$key_,$key,x5,lsl#4	// pointer to last 7 round keys
	sub	$rounds,$rounds,#2
	vld1.32	{q10-q11},[$key_],#32
	vld1.32	{q12-q13},[$key_],#32
	vld1.32	{q14-q15},[$key_],#32
	vld1.32	{$rndlast},[$key_]

	add	$key_,$key,#32
	mov	$cnt,$rounds
	b.eq	.Lcbc_dec

	cmp	$rounds,#2
	veor	$dat,$dat,$ivec
	veor	$rndzero_n_last,q8,$rndlast
	b.eq	.Lcbc_enc128

	vld1.32	{$in0-$in1},[$key_]
	add	$key_,$key,#16
	add	$key4,$key,#16*4
	add	$key5,$key,#16*5
	aese	$dat,q8
	aesmc	$dat,$dat
	add	$key6,$key,#16*6
	add	$key7,$key,#16*7
	b	.Lenter_cbc_enc

.align	4
.Loop_cbc_enc:
	aese	$dat,q8
	aesmc	$dat,$dat
	 vst1.8	{$ivec},[$out],#16
.Lenter_cbc_enc:
	aese	$dat,q9
	aesmc	$dat,$dat
	aese	$dat,$in0
	aesmc	$dat,$dat
	vld1.32	{q8},[$key4]
	cmp	$rounds,#4
	aese	$dat,$in1
	aesmc	$dat,$dat
	vld1.32	{q9},[$key5]
	b.eq	.Lcbc_enc192

	aese	$dat,q8
	aesmc	$dat,$dat
	vld1.32	{q8},[$key6]
	aese	$dat,q9
	aesmc	$dat,$dat
	vld1.32	{q9},[$key7]
	nop

.Lcbc_enc192:
	aese	$dat,q8
	aesmc	$dat,$dat
	 subs	$len,$len,#16
	aese	$dat,q9
	aesmc	$dat,$dat
	 cclr	$step,eq
	aese	$dat,q10
	aesmc	$dat,$dat
	aese	$dat,q11
	aesmc	$dat,$dat
	 vld1.8	{q8},[$inp],$step
	aese	$dat,q12
	aesmc	$dat,$dat
	 veor	q8,q8,$rndzero_n_last
	aese	$dat,q13
	aesmc	$dat,$dat
	 vld1.32 {q9},[$key_]		// re-pre-load rndkey[1]
	aese	$dat,q14
	aesmc	$dat,$dat
	aese	$dat,q15
	veor	$ivec,$dat,$rndlast
	b.hs	.Loop_cbc_enc

	vst1.8	{$ivec},[$out],#16
	b	.Lcbc_done

.align	5
.Lcbc_enc128:
	vld1.32	{$in0-$in1},[$key_]
	aese	$dat,q8
	aesmc	$dat,$dat
	b	.Lenter_cbc_enc128
.Loop_cbc_enc128:
	aese	$dat,q8
	aesmc	$dat,$dat
	 vst1.8	{$ivec},[$out],#16
.Lenter_cbc_enc128:
	aese	$dat,q9
	aesmc	$dat,$dat
	 subs	$len,$len,#16
	aese	$dat,$in0
	aesmc	$dat,$dat
	 cclr	$step,eq
	aese	$dat,$in1
	aesmc	$dat,$dat
	aese	$dat,q10
	aesmc	$dat,$dat
	aese	$dat,q11
	aesmc	$dat,$dat
	 vld1.8	{q8},[$inp],$step
	aese	$dat,q12
	aesmc	$dat,$dat
	aese	$dat,q13
	aesmc	$dat,$dat
	aese	$dat,q14
	aesmc	$dat,$dat
	 veor	q8,q8,$rndzero_n_last
	aese	$dat,q15
	veor	$ivec,$dat,$rndlast
	b.hs	.Loop_cbc_enc128

	vst1.8	{$ivec},[$out],#16
	b	.Lcbc_done
___
{
my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

my ($dat3,$in3,$tmp3);	# used only in 64-bit mode
my ($dat4,$in4,$tmp4);
if ($flavour =~ /64/) {
    ($dat2,$dat3,$dat4,$in2,$in3,$in4,$tmp3,$tmp4)=map("q$_",(16..23));
}

$code.=<<___;
.align	5
.Lcbc_dec:
	vld1.8	{$dat2},[$inp],#16
	subs	$len,$len,#32		// bias
	add	$cnt,$rounds,#2
	vorr	$in1,$dat,$dat
	vorr	$dat1,$dat,$dat
	vorr	$in2,$dat2,$dat2
	b.lo	.Lcbc_dec_tail

	vorr	$dat1,$dat2,$dat2
	vld1.8	{$dat2},[$inp],#16
	vorr	$in0,$dat,$dat
	vorr	$in1,$dat1,$dat1
	vorr	$in2,$dat2,$dat2
___
$code.=<<___	if ($flavour =~ /64/);
	cmp	$len,#32
	b.lo	.Loop3x_cbc_dec

	vld1.8	{$dat3},[$inp],#16
	vld1.8	{$dat4},[$inp],#16
	sub	$len,$len,#32		// bias
	mov	$cnt,$rounds
	vorr	$in3,$dat3,$dat3
	vorr	$in4,$dat4,$dat4

.Loop5x_cbc_dec:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop5x_cbc_dec

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	 cmp	$len,#0x40		// because .Lcbc_tail4x
	 sub	$len,$len,#0x50

	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	 csel	x6,xzr,$len,gt		// borrow x6, $cnt, "gt" is not typo
	 mov	$key_,$key

	aesd	$dat0,q10
	aesimc	$dat0,$dat0
	aesd	$dat1,q10
	aesimc	$dat1,$dat1
	aesd	$dat2,q10
	aesimc	$dat2,$dat2
	aesd	$dat3,q10
	aesimc	$dat3,$dat3
	aesd	$dat4,q10
	aesimc	$dat4,$dat4
	 add	$inp,$inp,x6		// $inp is adjusted in such way that
					// at exit from the loop $dat1-$dat4
					// are loaded with last "words"
	 add	x6,$len,#0x60		// because .Lcbc_tail4x

	aesd	$dat0,q11
	aesimc	$dat0,$dat0
	aesd	$dat1,q11
	aesimc	$dat1,$dat1
	aesd	$dat2,q11
	aesimc	$dat2,$dat2
	aesd	$dat3,q11
	aesimc	$dat3,$dat3
	aesd	$dat4,q11
	aesimc	$dat4,$dat4

	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	aesd	$dat3,q12
	aesimc	$dat3,$dat3
	aesd	$dat4,q12
	aesimc	$dat4,$dat4

	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	aesd	$dat3,q13
	aesimc	$dat3,$dat3
	aesd	$dat4,q13
	aesimc	$dat4,$dat4

	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	aesd	$dat3,q14
	aesimc	$dat3,$dat3
	aesd	$dat4,q14
	aesimc	$dat4,$dat4

	 veor	$tmp0,$ivec,$rndlast
	aesd	$dat0,q15
	 veor	$tmp1,$in0,$rndlast
	 vld1.8	{$in0},[$inp],#16
	aesd	$dat1,q15
	 veor	$tmp2,$in1,$rndlast
	 vld1.8	{$in1},[$inp],#16
	aesd	$dat2,q15
	 veor	$tmp3,$in2,$rndlast
	 vld1.8	{$in2},[$inp],#16
	aesd	$dat3,q15
	 veor	$tmp4,$in3,$rndlast
	 vld1.8	{$in3},[$inp],#16
	aesd	$dat4,q15
	 vorr	$ivec,$in4,$in4
	 vld1.8	{$in4},[$inp],#16
	cbz	x6,.Lcbc_tail4x
	 vld1.32 {q8},[$key_],#16	// re-pre-load rndkey[0]
	veor	$tmp0,$tmp0,$dat0
	 vorr	$dat0,$in0,$in0
	veor	$tmp1,$tmp1,$dat1
	 vorr	$dat1,$in1,$in1
	veor	$tmp2,$tmp2,$dat2
	 vorr	$dat2,$in2,$in2
	veor	$tmp3,$tmp3,$dat3
	 vorr	$dat3,$in3,$in3
	veor	$tmp4,$tmp4,$dat4
	vst1.8	{$tmp0},[$out],#16
	 vorr	$dat4,$in4,$in4
	vst1.8	{$tmp1},[$out],#16
	 mov	$cnt,$rounds
	vst1.8	{$tmp2},[$out],#16
	 vld1.32 {q9},[$key_],#16	// re-pre-load rndkey[1]
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16
	b.hs	.Loop5x_cbc_dec

	add	$len,$len,#0x50
	cbz	$len,.Lcbc_done

	add	$cnt,$rounds,#2
	subs	$len,$len,#0x30
	vorr	$dat0,$in2,$in2
	vorr	$in0,$in2,$in2
	vorr	$dat1,$in3,$in3
	vorr	$in1,$in3,$in3
	vorr	$dat2,$in4,$in4
	vorr	$in2,$in4,$in4
	b.lo	.Lcbc_dec_tail

	b	.Loop3x_cbc_dec

.align	4
.Lcbc_tail4x:
	veor	$tmp1,$tmp0,$dat1
	veor	$tmp2,$tmp2,$dat2
	veor	$tmp3,$tmp3,$dat3
	veor	$tmp4,$tmp4,$dat4
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16

	b	.Lcbc_done
.align	4
___
$code.=<<___;
.Loop3x_cbc_dec:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop3x_cbc_dec

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	 veor	$tmp0,$ivec,$rndlast
	 subs	$len,$len,#0x30
	 veor	$tmp1,$in0,$rndlast
	 mov.lo	x6,$len			// x6, $cnt, is zero at this point
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	 veor	$tmp2,$in1,$rndlast
	 add	$inp,$inp,x6		// $inp is adjusted in such way that
					// at exit from the loop $dat1-$dat2
					// are loaded with last "words"
	 vorr	$ivec,$in2,$in2
	 mov	$key_,$key
	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	 vld1.8	{$in0},[$inp],#16
	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	 vld1.8	{$in1},[$inp],#16
	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	 vld1.8	{$in2},[$inp],#16
	aesd	$dat0,q15
	aesd	$dat1,q15
	aesd	$dat2,q15
	 vld1.32 {q8},[$key_],#16	// re-pre-load rndkey[0]
	 add	$cnt,$rounds,#2
	veor	$tmp0,$tmp0,$dat0
	veor	$tmp1,$tmp1,$dat1
	veor	$dat2,$dat2,$tmp2
	 vld1.32 {q9},[$key_],#16	// re-pre-load rndkey[1]
	vst1.8	{$tmp0},[$out],#16
	 vorr	$dat0,$in0,$in0
	vst1.8	{$tmp1},[$out],#16
	 vorr	$dat1,$in1,$in1
	vst1.8	{$dat2},[$out],#16
	 vorr	$dat2,$in2,$in2
	b.hs	.Loop3x_cbc_dec

	cmn	$len,#0x30
	b.eq	.Lcbc_done
	nop

.Lcbc_dec_tail:
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$cnt,$cnt,#2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lcbc_dec_tail

	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	 cmn	$len,#0x20
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	 veor	$tmp1,$ivec,$rndlast
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	 veor	$tmp2,$in1,$rndlast
	aesd	$dat1,q15
	aesd	$dat2,q15
	b.eq	.Lcbc_dec_one
	veor	$tmp1,$tmp1,$dat1
	veor	$tmp2,$tmp2,$dat2
	 vorr	$ivec,$in2,$in2
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	b	.Lcbc_done

.Lcbc_dec_one:
	veor	$tmp1,$tmp1,$dat2
	 vorr	$ivec,$in2,$in2
	vst1.8	{$tmp1},[$out],#16

.Lcbc_done:
	vst1.8	{$ivec},[$ivp]
.Lcbc_abort:
___
}
$code.=<<___	if ($flavour !~ /64/);
	vldmia	sp!,{d8-d15}
	ldmia	sp!,{r4-r8,pc}
___
$code.=<<___	if ($flavour =~ /64/);
	ldr	x29,[sp],#16
	ret
___
$code.=<<___;
.size	${prefix}_cbc_encrypt,.-${prefix}_cbc_encrypt
___
}}}

{{{
my ($inp,$out,$len,$key,$ivp)=map("x$_",(0..4));
my ($rounds,$roundsx,$cnt,$key_)=("w5","x5","w6","x7");
my ($ctr,$tctr0,$tctr1,$tctr2)=map("w$_",(8..10,12));
my ($tctr3,$tctr4,$tctr5,$tctr6)=map("w$_",(11,13..15));
my ($tctr7,$tctr8,$tctr9,$tctr10,$tctr11)=map("w$_",(19..23));

# q0-q7 => v0-v7; q8-q23 => v16-v31; q24-q31 => v8-v15
my ($ivec,$rndlast,$rndping,$rndpang)=map("q$_",(0..3));
my ($in0,$in1,$in2,$in3,$in4,$in5)=map("q$_",(4..9));
my ($in6,$in7,$in8,$in9,$in10,$in11)=map("q$_",(10..15));
my ($dat0,$dat1,$dat2,$dat3,$dat4,$dat5)=map("q$_",(16..21));
my ($dat6,$dat7,$dat8,$dat9,$dat10,$dat11)=map("q$_",(22..27));
my ($tmp0,$tmp1,$tmp2)=map("q$_",(25..27));

#q_X => qX, for ldp & stp
my ($in0q,$in1q,$in2q,$in3q)=map("q_$_",(4..7));
my ($in4q,$in5q,$in6q,$in7q,$in8q,$in9q,$in10q,$in11q)=map("q_$_",(16..23));

my ($dat8d,$dat9d,$dat10d,$dat11d)=map("d$_",(8..11));

$code.=<<___	if ($flavour =~ /64/);
.globl	${prefix}_ctr32_encrypt_blocks_unroll12_eor3
.type	${prefix}_ctr32_encrypt_blocks_unroll12_eor3,%function
.align	5
${prefix}_ctr32_encrypt_blocks_unroll12_eor3:
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp		x29,x30,[sp,#-80]!
	stp		d8,d9,[sp, #16]
	stp		d10,d11,[sp, #32]
	stp		d12,d13,[sp, #48]
	stp		d14,d15,[sp, #64]
	add		x29,sp,#0

	 ldr		$rounds,[$key,#240]

	 ldr		$ctr, [$ivp, #12]
#ifdef __AARCH64EB__
	vld1.8		{$dat0},[$ivp]
#else
	vld1.32		{$dat0},[$ivp]
#endif
	vld1.32		{$rndping-$rndpang},[$key]		// load key schedule...
	 sub		$rounds,$rounds,#4
	 cmp		$len,#2
	 add		$key_,$key,$roundsx,lsl#4	// pointer to last round key
	 sub		$rounds,$rounds,#2
	 add		$key_, $key_, #64
	vld1.32		{$rndlast},[$key_]
	 add		$key_,$key,#32
	 mov		$cnt,$rounds
#ifndef __AARCH64EB__
	rev		$ctr, $ctr
#endif

	vorr		$dat1,$dat0,$dat0
	 add		$tctr1, $ctr, #1
	vorr		$dat2,$dat0,$dat0
	 add		$ctr, $ctr, #2
	vorr		$ivec,$dat0,$dat0
	 rev		$tctr1, $tctr1
	vmov.32		${dat1}[3],$tctr1
	b.ls		.Lctr32_tail_unroll
	 cmp		$len,#6
	 rev		$tctr2, $ctr
	 sub		$len,$len,#3		// bias
	vmov.32		${dat2}[3],$tctr2
	b.lo		.Loop3x_ctr32_unroll
	 cmp		$len,#9
	vorr		$dat3,$dat0,$dat0
	 add		$tctr3, $ctr, #1
	vorr		$dat4,$dat0,$dat0
	 add		$tctr4, $ctr, #2
	 rev		$tctr3, $tctr3
	vorr		$dat5,$dat0,$dat0
	 add		$ctr, $ctr, #3
	 rev		$tctr4, $tctr4
	vmov.32		${dat3}[3],$tctr3
	 rev		$tctr5, $ctr
	vmov.32		${dat4}[3],$tctr4
	vmov.32		${dat5}[3],$tctr5
	 sub		$len,$len,#3
	b.lo		.Loop6x_ctr32_unroll

	// push regs to stack when 12 data chunks are interleaved
	 stp		x19,x20,[sp,#-16]!
	 stp		x21,x22,[sp,#-16]!
	 stp		x23,x24,[sp,#-16]!
	 stp		$dat8d,$dat9d,[sp,#-32]!
	 stp		$dat10d,$dat11d,[sp,#-32]!

	 add		$tctr6,$ctr,#1
	 add		$tctr7,$ctr,#2
	 add		$tctr8,$ctr,#3
	 add		$tctr9,$ctr,#4
	 add		$tctr10,$ctr,#5
	 add		$ctr,$ctr,#6
	vorr		$dat6,$dat0,$dat0
	 rev		$tctr6,$tctr6
	vorr		$dat7,$dat0,$dat0
	 rev		$tctr7,$tctr7
	vorr		$dat8,$dat0,$dat0
	 rev		$tctr8,$tctr8
	vorr		$dat9,$dat0,$dat0
	 rev		$tctr9,$tctr9
	vorr		$dat10,$dat0,$dat0
	 rev		$tctr10,$tctr10
	vorr		$dat11,$dat0,$dat0
	 rev		$tctr11,$ctr

	 sub		$len,$len,#6		// bias
	vmov.32		${dat6}[3],$tctr6
	vmov.32		${dat7}[3],$tctr7
	vmov.32		${dat8}[3],$tctr8
	vmov.32		${dat9}[3],$tctr9
	vmov.32		${dat10}[3],$tctr10
	vmov.32		${dat11}[3],$tctr11
	b		.Loop12x_ctr32_unroll

.align	4
.Loop12x_ctr32_unroll:
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	aese		$dat6,$rndping
	aesmc		$dat6,$dat6
	aese		$dat7,$rndping
	aesmc		$dat7,$dat7
	aese		$dat8,$rndping
	aesmc		$dat8,$dat8
	aese		$dat9,$rndping
	aesmc		$dat9,$dat9
	aese		$dat10,$rndping
	aesmc		$dat10,$dat10
	aese		$dat11,$rndping
	aesmc		$dat11,$dat11
	vld1.32		{$rndping},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	aese		$dat6,$rndpang
	aesmc		$dat6,$dat6
	aese		$dat7,$rndpang
	aesmc		$dat7,$dat7
	aese		$dat8,$rndpang
	aesmc		$dat8,$dat8
	aese		$dat9,$rndpang
	aesmc		$dat9,$dat9
	aese		$dat10,$rndpang
	aesmc		$dat10,$dat10
	aese		$dat11,$rndpang
	aesmc		$dat11,$dat11
	vld1.32		{$rndpang},[$key_],#16
	b.gt		.Loop12x_ctr32_unroll

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	aese		$dat6,$rndping
	aesmc		$dat6,$dat6
	aese		$dat7,$rndping
	aesmc		$dat7,$dat7
	aese		$dat8,$rndping
	aesmc		$dat8,$dat8
	aese		$dat9,$rndping
	aesmc		$dat9,$dat9
	aese		$dat10,$rndping
	aesmc		$dat10,$dat10
	aese		$dat11,$rndping
	aesmc		$dat11,$dat11
	vld1.32	 	{$rndping},[$key_],#16

	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	aese		$dat6,$rndpang
	aesmc		$dat6,$dat6
	aese		$dat7,$rndpang
	aesmc		$dat7,$dat7
	aese		$dat8,$rndpang
	aesmc		$dat8,$dat8
	aese		$dat9,$rndpang
	aesmc		$dat9,$dat9
	aese		$dat10,$rndpang
	aesmc		$dat10,$dat10
	aese		$dat11,$rndpang
	aesmc		$dat11,$dat11
	vld1.32	 	{$rndpang},[$key_],#16

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	 add		$tctr0,$ctr,#1
	 add		$tctr1,$ctr,#2
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	 add		$tctr2,$ctr,#3
	 add		$tctr3,$ctr,#4
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	 add		$tctr4,$ctr,#5
	 add		$tctr5,$ctr,#6
	 rev		$tctr0,$tctr0
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	 add		$tctr6,$ctr,#7
	 add		$tctr7,$ctr,#8
	 rev		$tctr1,$tctr1
	 rev		$tctr2,$tctr2
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	 add		$tctr8,$ctr,#9
	 add		$tctr9,$ctr,#10
	 rev		$tctr3,$tctr3
	 rev		$tctr4,$tctr4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	 add		$tctr10,$ctr,#11
	 add		$tctr11,$ctr,#12
	 rev		$tctr5,$tctr5
	 rev		$tctr6,$tctr6
	aese		$dat6,$rndping
	aesmc		$dat6,$dat6
	 rev		$tctr7,$tctr7
	 rev		$tctr8,$tctr8
	aese		$dat7,$rndping
	aesmc		$dat7,$dat7
	 rev		$tctr9,$tctr9
	 rev		$tctr10,$tctr10
	aese		$dat8,$rndping
	aesmc		$dat8,$dat8
	 rev		$tctr11,$tctr11
	aese		$dat9,$rndping
	aesmc		$dat9,$dat9
	aese		$dat10,$rndping
	aesmc		$dat10,$dat10
	aese		$dat11,$rndping
	aesmc		$dat11,$dat11
	vld1.32	 	{$rndping},[$key_],#16

	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	vld1.8		{$in0,$in1,$in2,$in3},[$inp],#64
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	aese		$dat6,$rndpang
	aesmc		$dat6,$dat6
	aese		$dat7,$rndpang
	aesmc		$dat7,$dat7
	vld1.8		{$in4,$in5,$in6,$in7},[$inp],#64
	aese		$dat8,$rndpang
	aesmc		$dat8,$dat8
	aese		$dat9,$rndpang
	aesmc		$dat9,$dat9
	aese		$dat10,$rndpang
	aesmc		$dat10,$dat10
	aese		$dat11,$rndpang
	aesmc		$dat11,$dat11
	vld1.8		{$in8,$in9,$in10,$in11},[$inp],#64
	vld1.32	 	{$rndpang},[$key_],#16

	 mov		$key_, $key
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	aese		$dat6,$rndping
	aesmc		$dat6,$dat6
	aese		$dat7,$rndping
	aesmc		$dat7,$dat7
	aese		$dat8,$rndping
	aesmc		$dat8,$dat8
	aese		$dat9,$rndping
	aesmc		$dat9,$dat9
	aese		$dat10,$rndping
	aesmc		$dat10,$dat10
	aese		$dat11,$rndping
	aesmc		$dat11,$dat11
	vld1.32	 	{$rndping},[$key_],#16	// re-pre-load rndkey[0]

	aese		$dat0,$rndpang
	 eor3		$in0,$in0,$rndlast,$dat0
	vorr		$dat0,$ivec,$ivec
	aese		$dat1,$rndpang
	 eor3		$in1,$in1,$rndlast,$dat1
	vorr		$dat1,$ivec,$ivec
	aese		$dat2,$rndpang
	 eor3		$in2,$in2,$rndlast,$dat2
	vorr		$dat2,$ivec,$ivec
	aese		$dat3,$rndpang
	 eor3		$in3,$in3,$rndlast,$dat3
	vorr		$dat3,$ivec,$ivec
	aese		$dat4,$rndpang
	 eor3		$in4,$in4,$rndlast,$dat4
	vorr		$dat4,$ivec,$ivec
	aese		$dat5,$rndpang
	 eor3		$in5,$in5,$rndlast,$dat5
	vorr		$dat5,$ivec,$ivec
	aese		$dat6,$rndpang
	 eor3		$in6,$in6,$rndlast,$dat6
	vorr		$dat6,$ivec,$ivec
	aese		$dat7,$rndpang
	 eor3		$in7,$in7,$rndlast,$dat7
	vorr		$dat7,$ivec,$ivec
	aese		$dat8,$rndpang
	 eor3		$in8,$in8,$rndlast,$dat8
	vorr		$dat8,$ivec,$ivec
	aese		$dat9,$rndpang
	 eor3		$in9,$in9,$rndlast,$dat9
	vorr		$dat9,$ivec,$ivec
	aese		$dat10,$rndpang
	 eor3		$in10,$in10,$rndlast,$dat10
	vorr		$dat10,$ivec,$ivec
	aese		$dat11,$rndpang
	 eor3		$in11,$in11,$rndlast,$dat11
	vorr		$dat11,$ivec,$ivec
	vld1.32	 	{$rndpang},[$key_],#16	// re-pre-load rndkey[1]

	vmov.32		${dat0}[3],$tctr0
	vmov.32		${dat1}[3],$tctr1
	vmov.32		${dat2}[3],$tctr2
	vmov.32		${dat3}[3],$tctr3
	vst1.8		{$in0,$in1,$in2,$in3},[$out],#64
	vmov.32		${dat4}[3],$tctr4
	vmov.32		${dat5}[3],$tctr5
	vmov.32		${dat6}[3],$tctr6
	vmov.32		${dat7}[3],$tctr7
	vst1.8		{$in4,$in5,$in6,$in7},[$out],#64
	vmov.32		${dat8}[3],$tctr8
	vmov.32		${dat9}[3],$tctr9
	vmov.32		${dat10}[3],$tctr10
	vmov.32		${dat11}[3],$tctr11
	vst1.8		{$in8,$in9,$in10,$in11},[$out],#64

	 mov		$cnt,$rounds

	 add		$ctr,$ctr,#12
	subs		$len,$len,#12
	b.hs		.Loop12x_ctr32_unroll

	// pop regs from stack when 12 data chunks are interleaved
	 ldp		$dat10d,$dat11d,[sp],#32
	 ldp		$dat8d,$dat9d,[sp],#32
	 ldp		x23,x24,[sp],#16
	 ldp		x21,x22,[sp],#16
	 ldp		x19,x20,[sp],#16

	 add		$len,$len,#12
	 cbz		$len,.Lctr32_done_unroll
	 sub		$ctr,$ctr,#12

	 cmp		$len,#2
	b.ls		.Lctr32_tail_unroll

	 cmp		$len,#6
	 sub		$len,$len,#3		// bias
	 add		$ctr,$ctr,#3
	b.lo		.Loop3x_ctr32_unroll

	 sub		$len,$len,#3
	 add		$ctr,$ctr,#3
	b.lo		.Loop6x_ctr32_unroll

.align	4
.Loop6x_ctr32_unroll:
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	vld1.32		{$rndping},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	vld1.32		{$rndpang},[$key_],#16
	b.gt		.Loop6x_ctr32_unroll

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	vld1.32	 	{$rndping},[$key_],#16

	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	vld1.32	 	{$rndpang},[$key_],#16

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	 add		$tctr0,$ctr,#1
	 add		$tctr1,$ctr,#2
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	 add		$tctr2,$ctr,#3
	 add		$tctr3,$ctr,#4
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	 add		$tctr4,$ctr,#5
	 add		$tctr5,$ctr,#6
	 rev		$tctr0,$tctr0
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	 rev		$tctr1,$tctr1
	 rev		$tctr2,$tctr2
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	 rev		$tctr3,$tctr3
	 rev		$tctr4,$tctr4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	 rev		$tctr5,$tctr5
	vld1.32	 	{$rndping},[$key_],#16

	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	vld1.8		{$in0,$in1,$in2,$in3},[$inp],#64
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	aese		$dat3,$rndpang
	aesmc		$dat3,$dat3
	vld1.8		{$in4,$in5},[$inp],#32
	aese		$dat4,$rndpang
	aesmc		$dat4,$dat4
	aese		$dat5,$rndpang
	aesmc		$dat5,$dat5
	vld1.32	 	{$rndpang},[$key_],#16

	 mov		$key_, $key
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	aese		$dat3,$rndping
	aesmc		$dat3,$dat3
	aese		$dat4,$rndping
	aesmc		$dat4,$dat4
	aese		$dat5,$rndping
	aesmc		$dat5,$dat5
	vld1.32	 	{$rndping},[$key_],#16	// re-pre-load rndkey[0]

	aese		$dat0,$rndpang
	 eor3		$in0,$in0,$rndlast,$dat0
	aese		$dat1,$rndpang
	 eor3		$in1,$in1,$rndlast,$dat1
	aese		$dat2,$rndpang
	 eor3		$in2,$in2,$rndlast,$dat2
	aese		$dat3,$rndpang
	 eor3		$in3,$in3,$rndlast,$dat3
	aese		$dat4,$rndpang
	 eor3		$in4,$in4,$rndlast,$dat4
	aese		$dat5,$rndpang
	 eor3		$in5,$in5,$rndlast,$dat5
	vld1.32	 	{$rndpang},[$key_],#16	// re-pre-load rndkey[1]

	vorr		$dat0,$ivec,$ivec
	vorr		$dat1,$ivec,$ivec
	vorr		$dat2,$ivec,$ivec
	vorr		$dat3,$ivec,$ivec
	vorr		$dat4,$ivec,$ivec
	vorr		$dat5,$ivec,$ivec

	vmov.32		${dat0}[3],$tctr0
	vmov.32		${dat1}[3],$tctr1
	vst1.8		{$in0,$in1,$in2,$in3},[$out],#64
	vmov.32		${dat2}[3],$tctr2
	vmov.32		${dat3}[3],$tctr3
	vst1.8		{$in4,$in5},[$out],#32
	vmov.32		${dat4}[3],$tctr4
	vmov.32		${dat5}[3],$tctr5

	 cbz		$len,.Lctr32_done_unroll
	 mov		$cnt,$rounds

	 cmp		$len,#2
	b.ls		.Lctr32_tail_unroll

	 sub		$len,$len,#3		// bias
	 add		$ctr,$ctr,#3
	 b		.Loop3x_ctr32_unroll

.align	4
.Loop3x_ctr32_unroll:
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	vld1.32		{$rndping},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	aese		$dat2,$rndpang
	aesmc		$dat2,$dat2
	vld1.32		{$rndpang},[$key_],#16
	b.gt		.Loop3x_ctr32_unroll

	aese		$dat0,$rndping
	aesmc		$tmp0,$dat0
	aese		$dat1,$rndping
	aesmc		$tmp1,$dat1
	vld1.8		{$in0,$in1,$in2},[$inp],#48
	vorr		$dat0,$ivec,$ivec
	aese		$dat2,$rndping
	aesmc		$dat2,$dat2
	vld1.32		{$rndping},[$key_],#16
	vorr		$dat1,$ivec,$ivec
	aese		$tmp0,$rndpang
	aesmc		$tmp0,$tmp0
	aese		$tmp1,$rndpang
	aesmc		$tmp1,$tmp1
	aese		$dat2,$rndpang
	aesmc		$tmp2,$dat2
	vld1.32		{$rndpang},[$key_],#16
	vorr		$dat2,$ivec,$ivec
	 add		$tctr0,$ctr,#1
	aese		$tmp0,$rndping
	aesmc		$tmp0,$tmp0
	aese		$tmp1,$rndping
	aesmc		$tmp1,$tmp1
	 add		$tctr1,$ctr,#2
	aese		$tmp2,$rndping
	aesmc		$tmp2,$tmp2
	vld1.32		{$rndping},[$key_],#16
	 add		$ctr,$ctr,#3
	aese		$tmp0,$rndpang
	aesmc		$tmp0,$tmp0
	aese		$tmp1,$rndpang
	aesmc		$tmp1,$tmp1

	 rev		$tctr0,$tctr0
	aese		$tmp2,$rndpang
	aesmc		$tmp2,$tmp2
	vld1.32		{$rndpang},[$key_],#16
	vmov.32		${dat0}[3], $tctr0
	 mov		$key_,$key
	 rev		$tctr1,$tctr1
	aese		$tmp0,$rndping
	aesmc		$tmp0,$tmp0

	aese		$tmp1,$rndping
	aesmc		$tmp1,$tmp1
	vmov.32		${dat1}[3], $tctr1
	 rev		$tctr2,$ctr
	aese		$tmp2,$rndping
	aesmc		$tmp2,$tmp2
	vmov.32		${dat2}[3], $tctr2

	aese		$tmp0,$rndpang
	aese		$tmp1,$rndpang
	aese		$tmp2,$rndpang

	 eor3		$in0,$in0,$rndlast,$tmp0
	vld1.32		{$rndping},[$key_],#16	// re-pre-load rndkey[0]
	 eor3		$in1,$in1,$rndlast,$tmp1
	 mov		$cnt,$rounds
	 eor3		$in2,$in2,$rndlast,$tmp2
	vld1.32		{$rndpang},[$key_],#16	// re-pre-load rndkey[1]
	vst1.8		{$in0,$in1,$in2},[$out],#48

	 cbz		$len,.Lctr32_done_unroll

.Lctr32_tail_unroll:
	 cmp		$len,#1
	b.eq		.Lctr32_tail_1_unroll

.Lctr32_tail_2_unroll:
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	vld1.32		{$rndping},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	vld1.32		{$rndpang},[$key_],#16
	b.gt		.Lctr32_tail_2_unroll

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	vld1.32		{$rndping},[$key_],#16
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	vld1.32		{$rndpang},[$key_],#16
	vld1.8		{$in0,$in1},[$inp],#32
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	vld1.32		{$rndping},[$key_],#16
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	aese		$dat1,$rndpang
	aesmc		$dat1,$dat1
	vld1.32		{$rndpang},[$key_],#16
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat1,$rndping
	aesmc		$dat1,$dat1
	aese		$dat0,$rndpang
	aese		$dat1,$rndpang

	 eor3		$in0,$in0,$rndlast,$dat0
	 eor3		$in1,$in1,$rndlast,$dat1
	vst1.8		{$in0,$in1},[$out],#32
	 b		.Lctr32_done_unroll

.Lctr32_tail_1_unroll:
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	vld1.32		{$rndping},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	vld1.32		{$rndpang},[$key_],#16
	b.gt		.Lctr32_tail_1_unroll

	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	vld1.32		{$rndping},[$key_],#16
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	vld1.32		{$rndpang},[$key_],#16
	vld1.8		{$in0},[$inp]
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	vld1.32		{$rndping},[$key_],#16
	aese		$dat0,$rndpang
	aesmc		$dat0,$dat0
	vld1.32		{$rndpang},[$key_],#16
	aese		$dat0,$rndping
	aesmc		$dat0,$dat0
	aese		$dat0,$rndpang

	 eor3		$in0,$in0,$rndlast,$dat0
	vst1.8		{$in0},[$out],#16

.Lctr32_done_unroll:
	ldp		d8,d9,[sp, #16]
	ldp		d10,d11,[sp, #32]
	ldp		d12,d13,[sp, #48]
	ldp		d14,d15,[sp, #64]
	ldr		x29,[sp],#80
	ret
.size	${prefix}_ctr32_encrypt_blocks_unroll12_eor3,.-${prefix}_ctr32_encrypt_blocks_unroll12_eor3
___
}}}

{{{
my ($inp,$out,$len,$key,$ivp)=map("x$_",(0..4));
my ($rounds,$cnt,$key_)=("w5","w6","x7");
my ($ctr,$tctr0,$tctr1,$tctr2)=map("w$_",(8..10,12));
my $step="x12";		# aliases with $tctr2

my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$ivec,$rndlast)=map("q$_",(0..7));
my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

# used only in 64-bit mode...
my ($dat3,$dat4,$in3,$in4)=map("q$_",(16..23));

my ($dat,$tmp)=($dat0,$tmp0);

### q8-q15	preloaded key schedule

$code.=<<___;
.globl	${prefix}_ctr32_encrypt_blocks
.type	${prefix}_ctr32_encrypt_blocks,%function
.align	5
${prefix}_ctr32_encrypt_blocks:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp		x29,x30,[sp,#-16]!
	add		x29,sp,#0
___
$code.=<<___	if ($flavour !~ /64/);
	mov		ip,sp
	stmdb		sp!,{r4-r10,lr}
	vstmdb		sp!,{d8-d15}            @ ABI specification says so
	ldr		r4, [ip]		@ load remaining arg
___
$code.=<<___;
	ldr		$rounds,[$key,#240]

	ldr		$ctr, [$ivp, #12]
#ifdef __ARMEB__
	vld1.8		{$dat0},[$ivp]
#else
	vld1.32		{$dat0},[$ivp]
#endif
	vld1.32		{q8-q9},[$key]		// load key schedule...
	sub		$rounds,$rounds,#4
	mov		$step,#16
	cmp		$len,#2
	add		$key_,$key,x5,lsl#4	// pointer to last 5 round keys
	sub		$rounds,$rounds,#2
	vld1.32		{q12-q13},[$key_],#32
	vld1.32		{q14-q15},[$key_],#32
	vld1.32		{$rndlast},[$key_]
	add		$key_,$key,#32
	mov		$cnt,$rounds
	cclr		$step,lo
#ifndef __ARMEB__
	rev		$ctr, $ctr
#endif
___
$code.=<<___	if ($flavour =~ /64/);
	vorr		$dat1,$dat0,$dat0
	add		$tctr1, $ctr, #1
	vorr		$dat2,$dat0,$dat0
	add		$ctr, $ctr, #2
	vorr		$ivec,$dat0,$dat0
	rev		$tctr1, $tctr1
	vmov.32		${dat1}[3],$tctr1
	b.ls		.Lctr32_tail
	rev		$tctr2, $ctr
	sub		$len,$len,#3		// bias
	vmov.32		${dat2}[3],$tctr2
___
$code.=<<___	if ($flavour !~ /64/);
	add		$tctr1, $ctr, #1
	vorr		$ivec,$dat0,$dat0
	rev		$tctr1, $tctr1
	vmov.32		${ivec}[3],$tctr1
	add		$ctr, $ctr, #2
	vorr		$dat1,$ivec,$ivec
	b.ls		.Lctr32_tail
	rev		$tctr2, $ctr
	vmov.32		${ivec}[3],$tctr2
	sub		$len,$len,#3		// bias
	vorr		$dat2,$ivec,$ivec
___
$code.=<<___	if ($flavour =~ /64/);
	cmp		$len,#32
	b.lo		.Loop3x_ctr32

	add		w13,$ctr,#1
	add		w14,$ctr,#2
	vorr		$dat3,$dat0,$dat0
	rev		w13,w13
	vorr		$dat4,$dat0,$dat0
	rev		w14,w14
	vmov.32		${dat3}[3],w13
	sub		$len,$len,#2		// bias
	vmov.32		${dat4}[3],w14
	add		$ctr,$ctr,#2
	b		.Loop5x_ctr32

.align	4
.Loop5x_ctr32:
	aese		$dat0,q8
	aesmc		$dat0,$dat0
	aese		$dat1,q8
	aesmc		$dat1,$dat1
	aese		$dat2,q8
	aesmc		$dat2,$dat2
	aese		$dat3,q8
	aesmc		$dat3,$dat3
	aese		$dat4,q8
	aesmc		$dat4,$dat4
	vld1.32		{q8},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,q9
	aesmc		$dat0,$dat0
	aese		$dat1,q9
	aesmc		$dat1,$dat1
	aese		$dat2,q9
	aesmc		$dat2,$dat2
	aese		$dat3,q9
	aesmc		$dat3,$dat3
	aese		$dat4,q9
	aesmc		$dat4,$dat4
	vld1.32		{q9},[$key_],#16
	b.gt		.Loop5x_ctr32

	mov		$key_,$key
	aese		$dat0,q8
	aesmc		$dat0,$dat0
	aese		$dat1,q8
	aesmc		$dat1,$dat1
	aese		$dat2,q8
	aesmc		$dat2,$dat2
	aese		$dat3,q8
	aesmc		$dat3,$dat3
	aese		$dat4,q8
	aesmc		$dat4,$dat4
	vld1.32	 	{q8},[$key_],#16	// re-pre-load rndkey[0]

	aese		$dat0,q9
	aesmc		$dat0,$dat0
	aese		$dat1,q9
	aesmc		$dat1,$dat1
	aese		$dat2,q9
	aesmc		$dat2,$dat2
	aese		$dat3,q9
	aesmc		$dat3,$dat3
	aese		$dat4,q9
	aesmc		$dat4,$dat4
	vld1.32	 	{q9},[$key_],#16	// re-pre-load rndkey[1]

	aese		$dat0,q12
	aesmc		$dat0,$dat0
	 add		$tctr0,$ctr,#1
	 add		$tctr1,$ctr,#2
	aese		$dat1,q12
	aesmc		$dat1,$dat1
	 add		$tctr2,$ctr,#3
	 add		w13,$ctr,#4
	aese		$dat2,q12
	aesmc		$dat2,$dat2
	 add		w14,$ctr,#5
	 rev		$tctr0,$tctr0
	aese		$dat3,q12
	aesmc		$dat3,$dat3
	 rev		$tctr1,$tctr1
	 rev		$tctr2,$tctr2
	aese		$dat4,q12
	aesmc		$dat4,$dat4
	 rev		w13,w13
	 rev		w14,w14

	aese		$dat0,q13
	aesmc		$dat0,$dat0
	aese		$dat1,q13
	aesmc		$dat1,$dat1
	aese		$dat2,q13
	aesmc		$dat2,$dat2
	aese		$dat3,q13
	aesmc		$dat3,$dat3
	aese		$dat4,q13
	aesmc		$dat4,$dat4

	aese		$dat0,q14
	aesmc		$dat0,$dat0
	 vld1.8		{$in0},[$inp],#16
	aese		$dat1,q14
	aesmc		$dat1,$dat1
	 vld1.8		{$in1},[$inp],#16
	aese		$dat2,q14
	aesmc		$dat2,$dat2
	 vld1.8		{$in2},[$inp],#16
	aese		$dat3,q14
	aesmc		$dat3,$dat3
	 vld1.8		{$in3},[$inp],#16
	aese		$dat4,q14
	aesmc		$dat4,$dat4
	 vld1.8		{$in4},[$inp],#16

	aese		$dat0,q15
	 veor		$in0,$in0,$rndlast
	aese		$dat1,q15
	 veor		$in1,$in1,$rndlast
	aese		$dat2,q15
	 veor		$in2,$in2,$rndlast
	aese		$dat3,q15
	 veor		$in3,$in3,$rndlast
	aese		$dat4,q15
	 veor		$in4,$in4,$rndlast

	veor		$in0,$in0,$dat0
	 vorr		$dat0,$ivec,$ivec
	veor		$in1,$in1,$dat1
	 vorr		$dat1,$ivec,$ivec
	veor		$in2,$in2,$dat2
	 vorr		$dat2,$ivec,$ivec
	veor		$in3,$in3,$dat3
	 vorr		$dat3,$ivec,$ivec
	veor		$in4,$in4,$dat4
	 vorr		$dat4,$ivec,$ivec

	vst1.8		{$in0},[$out],#16
	 vmov.32	${dat0}[3],$tctr0
	vst1.8		{$in1},[$out],#16
	 vmov.32	${dat1}[3],$tctr1
	vst1.8		{$in2},[$out],#16
	 vmov.32	${dat2}[3],$tctr2
	vst1.8		{$in3},[$out],#16
	 vmov.32	${dat3}[3],w13
	vst1.8		{$in4},[$out],#16
	 vmov.32	${dat4}[3],w14

	mov		$cnt,$rounds
	cbz		$len,.Lctr32_done

	add		$ctr,$ctr,#5
	subs		$len,$len,#5
	b.hs		.Loop5x_ctr32

	add		$len,$len,#5
	sub		$ctr,$ctr,#5

	cmp		$len,#2
	mov		$step,#16
	cclr		$step,lo
	b.ls		.Lctr32_tail

	sub		$len,$len,#3		// bias
	add		$ctr,$ctr,#3
___
$code.=<<___;
	b		.Loop3x_ctr32

.align	4
.Loop3x_ctr32:
	aese		$dat0,q8
	aesmc		$dat0,$dat0
	aese		$dat1,q8
	aesmc		$dat1,$dat1
	aese		$dat2,q8
	aesmc		$dat2,$dat2
	vld1.32		{q8},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,q9
	aesmc		$dat0,$dat0
	aese		$dat1,q9
	aesmc		$dat1,$dat1
	aese		$dat2,q9
	aesmc		$dat2,$dat2
	vld1.32		{q9},[$key_],#16
	b.gt		.Loop3x_ctr32

	aese		$dat0,q8
	aesmc		$tmp0,$dat0
	aese		$dat1,q8
	aesmc		$tmp1,$dat1
	 vld1.8		{$in0},[$inp],#16
___
$code.=<<___	if ($flavour =~ /64/);
	 vorr		$dat0,$ivec,$ivec
___
$code.=<<___	if ($flavour !~ /64/);
	 add		$tctr0,$ctr,#1
___
$code.=<<___;
	aese		$dat2,q8
	aesmc		$dat2,$dat2
	 vld1.8		{$in1},[$inp],#16
___
$code.=<<___	if ($flavour =~ /64/);
	 vorr		$dat1,$ivec,$ivec
___
$code.=<<___	if ($flavour !~ /64/);
	 rev		$tctr0,$tctr0
___
$code.=<<___;
	aese		$tmp0,q9
	aesmc		$tmp0,$tmp0
	aese		$tmp1,q9
	aesmc		$tmp1,$tmp1
	 vld1.8		{$in2},[$inp],#16
	 mov		$key_,$key
	aese		$dat2,q9
	aesmc		$tmp2,$dat2
___
$code.=<<___	if ($flavour =~ /64/);
	 vorr		$dat2,$ivec,$ivec
	 add		$tctr0,$ctr,#1
___
$code.=<<___;
	aese		$tmp0,q12
	aesmc		$tmp0,$tmp0
	aese		$tmp1,q12
	aesmc		$tmp1,$tmp1
	 veor		$in0,$in0,$rndlast
	 add		$tctr1,$ctr,#2
	aese		$tmp2,q12
	aesmc		$tmp2,$tmp2
	 veor		$in1,$in1,$rndlast
	 add		$ctr,$ctr,#3
	aese		$tmp0,q13
	aesmc		$tmp0,$tmp0
	aese		$tmp1,q13
	aesmc		$tmp1,$tmp1
	 veor		$in2,$in2,$rndlast
___
$code.=<<___	if ($flavour =~ /64/);
	 rev		$tctr0,$tctr0
	aese		$tmp2,q13
	aesmc		$tmp2,$tmp2
	 vmov.32	${dat0}[3], $tctr0
___
$code.=<<___	if ($flavour !~ /64/);
	 vmov.32	${ivec}[3], $tctr0
	aese		$tmp2,q13
	aesmc		$tmp2,$tmp2
	 vorr		$dat0,$ivec,$ivec
___
$code.=<<___;
	 rev		$tctr1,$tctr1
	aese		$tmp0,q14
	aesmc		$tmp0,$tmp0
___
$code.=<<___	if ($flavour !~ /64/);
	 vmov.32	${ivec}[3], $tctr1
	 rev		$tctr2,$ctr
___
$code.=<<___;
	aese		$tmp1,q14
	aesmc		$tmp1,$tmp1
___
$code.=<<___	if ($flavour =~ /64/);
	 vmov.32	${dat1}[3], $tctr1
	 rev		$tctr2,$ctr
	aese		$tmp2,q14
	aesmc		$tmp2,$tmp2
	 vmov.32	${dat2}[3], $tctr2
___
$code.=<<___	if ($flavour !~ /64/);
	 vorr		$dat1,$ivec,$ivec
	 vmov.32	${ivec}[3], $tctr2
	aese		$tmp2,q14
	aesmc		$tmp2,$tmp2
	 vorr		$dat2,$ivec,$ivec
___
$code.=<<___;
	 subs		$len,$len,#3
	aese		$tmp0,q15
	aese		$tmp1,q15
	aese		$tmp2,q15

	veor		$in0,$in0,$tmp0
	 vld1.32	 {q8},[$key_],#16	// re-pre-load rndkey[0]
	vst1.8		{$in0},[$out],#16
	veor		$in1,$in1,$tmp1
	 mov		$cnt,$rounds
	vst1.8		{$in1},[$out],#16
	veor		$in2,$in2,$tmp2
	 vld1.32	 {q9},[$key_],#16	// re-pre-load rndkey[1]
	vst1.8		{$in2},[$out],#16
	b.hs		.Loop3x_ctr32

	adds		$len,$len,#3
	b.eq		.Lctr32_done
	cmp		$len,#1
	mov		$step,#16
	cclr		$step,eq

.Lctr32_tail:
	aese		$dat0,q8
	aesmc		$dat0,$dat0
	aese		$dat1,q8
	aesmc		$dat1,$dat1
	vld1.32		{q8},[$key_],#16
	subs		$cnt,$cnt,#2
	aese		$dat0,q9
	aesmc		$dat0,$dat0
	aese		$dat1,q9
	aesmc		$dat1,$dat1
	vld1.32		{q9},[$key_],#16
	b.gt		.Lctr32_tail

	aese		$dat0,q8
	aesmc		$dat0,$dat0
	aese		$dat1,q8
	aesmc		$dat1,$dat1
	aese		$dat0,q9
	aesmc		$dat0,$dat0
	aese		$dat1,q9
	aesmc		$dat1,$dat1
	 vld1.8		{$in0},[$inp],$step
	aese		$dat0,q12
	aesmc		$dat0,$dat0
	aese		$dat1,q12
	aesmc		$dat1,$dat1
	 vld1.8		{$in1},[$inp]
	aese		$dat0,q13
	aesmc		$dat0,$dat0
	aese		$dat1,q13
	aesmc		$dat1,$dat1
	 veor		$in0,$in0,$rndlast
	aese		$dat0,q14
	aesmc		$dat0,$dat0
	aese		$dat1,q14
	aesmc		$dat1,$dat1
	 veor		$in1,$in1,$rndlast
	aese		$dat0,q15
	aese		$dat1,q15

	cmp		$len,#1
	veor		$in0,$in0,$dat0
	veor		$in1,$in1,$dat1
	vst1.8		{$in0},[$out],#16
	b.eq		.Lctr32_done
	vst1.8		{$in1},[$out]

.Lctr32_done:
___
$code.=<<___	if ($flavour !~ /64/);
	vldmia		sp!,{d8-d15}
	ldmia		sp!,{r4-r10,pc}
___
$code.=<<___	if ($flavour =~ /64/);
	ldr		x29,[sp],#16
	ret
___
$code.=<<___;
.size	${prefix}_ctr32_encrypt_blocks,.-${prefix}_ctr32_encrypt_blocks
___
}}}
# Performance in cycles per byte.
# Processed with AES-XTS different key size.
# It shows the value before and after optimization as below:
# (before/after):
#
#		AES-128-XTS		AES-256-XTS
# Cortex-A57	3.36/1.09		4.02/1.37
# Cortex-A72	3.03/1.02		3.28/1.33

# Optimization is implemented by loop unrolling and interleaving.
# Commonly, we choose the unrolling factor as 5, if the input
# data size smaller than 5 blocks, but not smaller than 3 blocks,
# choose 3 as the unrolling factor.
# If the input data size dsize >= 5*16 bytes, then take 5 blocks
# as one iteration, every loop the left size lsize -= 5*16.
# If lsize < 5*16 bytes, treat them as the tail. Note: left 4*16 bytes
# will be processed specially, which be integrated into the 5*16 bytes
# loop to improve the efficiency.
# There is one special case, if the original input data size dsize
# = 16 bytes, we will treat it separately to improve the
# performance: one independent code block without LR, FP load and
# store.
# Encryption will process the (length -tailcnt) bytes as mentioned
# previously, then encrypt the composite block as last second
# cipher block.
# Decryption will process the (length -tailcnt -1) bytes as mentioned
# previously, then decrypt the last second cipher block to get the
# last plain block(tail), decrypt the composite block as last second
# plain text block.

{{{
my ($inp,$out,$len,$key1,$key2,$ivp)=map("x$_",(0..5));
my ($rounds0,$rounds,$key_,$step,$ivl,$ivh)=("w5","w6","x7","x8","x9","x10");
my ($tmpoutp,$loutp,$l2outp,$tmpinp)=("x13","w14","w15","x20");
my ($tailcnt,$midnum,$midnumx,$constnum,$constnumx)=("x21","w22","x22","w19","x19");
my ($xoffset,$tmpmx,$tmpmw)=("x6","x11","w11");
my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$tmp2,$rndlast)=map("q$_",(0..7));
my ($iv0,$iv1,$iv2,$iv3,$iv4)=("v6.16b","v8.16b","v9.16b","v10.16b","v11.16b");
my ($ivd00,$ivd01,$ivd20,$ivd21)=("d6","v6.d[1]","d9","v9.d[1]");
my ($ivd10,$ivd11,$ivd30,$ivd31,$ivd40,$ivd41)=("d8","v8.d[1]","d10","v10.d[1]","d11","v11.d[1]");

my ($tmpin)=("v26.16b");
my ($dat,$tmp,$rndzero_n_last)=($dat0,$tmp0,$tmp1);

# q7	last round key
# q10-q15, q7	Last 7 round keys
# q8-q9	preloaded round keys except last 7 keys for big size
# q20, q21, q8-q9	preloaded round keys except last 7 keys for only 16 byte


my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

my ($dat3,$in3,$tmp3);	# used only in 64-bit mode
my ($dat4,$in4,$tmp4);
if ($flavour =~ /64/) {
    ($dat2,$dat3,$dat4,$in2,$in3,$in4,$tmp3,$tmp4)=map("q$_",(16..23));
}

$code.=<<___	if ($flavour =~ /64/);
.globl	${prefix}_xts_encrypt
.type	${prefix}_xts_encrypt,%function
.align	5
${prefix}_xts_encrypt:
___
$code.=<<___	if ($flavour =~ /64/);
	AARCH64_VALID_CALL_TARGET
	cmp	$len,#16
	// Original input data size bigger than 16, jump to big size processing.
	b.ne	.Lxts_enc_big_size
	// Encrypt the iv with key2, as the first XEX iv.
	ldr	$rounds,[$key2,#240]
	vld1.32	{$dat},[$key2],#16
	vld1.8	{$iv0},[$ivp]
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key2],#16

.Loop_enc_iv_enc:
	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2],#16
	subs	$rounds,$rounds,#2
	aese	$iv0,$dat1
	aesmc	$iv0,$iv0
	vld1.32	{$dat1},[$key2],#16
	b.gt	.Loop_enc_iv_enc

	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2]
	aese	$iv0,$dat1
	veor	$iv0,$iv0,$dat

	vld1.8	{$dat0},[$inp]
	veor	$dat0,$iv0,$dat0

	ldr	$rounds,[$key1,#240]
	vld1.32	{q20-q21},[$key1],#32		// load key schedule...

	aese	$dat0,q20
	aesmc	$dat0,$dat0
	vld1.32	{q8-q9},[$key1],#32		// load key schedule...
	aese	$dat0,q21
	aesmc	$dat0,$dat0
	subs	$rounds,$rounds,#10		// if rounds==10, jump to aes-128-xts processing
	b.eq	.Lxts_128_enc
.Lxts_enc_round_loop:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	vld1.32	{q8},[$key1],#16		// load key schedule...
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	vld1.32	{q9},[$key1],#16		// load key schedule...
	subs	$rounds,$rounds,#2		// bias
	b.gt	.Lxts_enc_round_loop
.Lxts_128_enc:
	vld1.32	{q10-q11},[$key1],#32		// load key schedule...
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	vld1.32	{q12-q13},[$key1],#32		// load key schedule...
	aese	$dat0,q10
	aesmc	$dat0,$dat0
	aese	$dat0,q11
	aesmc	$dat0,$dat0
	vld1.32	{q14-q15},[$key1],#32		// load key schedule...
	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat0,q13
	aesmc	$dat0,$dat0
	vld1.32	{$rndlast},[$key1]
	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat0,q15
	veor	$dat0,$dat0,$rndlast
	veor	$dat0,$dat0,$iv0
	vst1.8	{$dat0},[$out]
	b	.Lxts_enc_final_abort

.align	4
.Lxts_enc_big_size:
___
$code.=<<___	if ($flavour =~ /64/);
	stp	$constnumx,$tmpinp,[sp,#-64]!
	stp	$tailcnt,$midnumx,[sp,#48]
	stp	$ivd10,$ivd20,[sp,#32]
	stp	$ivd30,$ivd40,[sp,#16]

	// tailcnt store the tail value of length%16.
	and	$tailcnt,$len,#0xf
	and	$len,$len,#-16
	subs	$len,$len,#16
	mov	$step,#16
	b.lo	.Lxts_abort
	csel	$step,xzr,$step,eq

	// Firstly, encrypt the iv with key2, as the first iv of XEX.
	ldr	$rounds,[$key2,#240]
	vld1.32	{$dat},[$key2],#16
	vld1.8	{$iv0},[$ivp]
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key2],#16

.Loop_iv_enc:
	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2],#16
	subs	$rounds,$rounds,#2
	aese	$iv0,$dat1
	aesmc	$iv0,$iv0
	vld1.32	{$dat1},[$key2],#16
	b.gt	.Loop_iv_enc

	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2]
	aese	$iv0,$dat1
	veor	$iv0,$iv0,$dat

	// The iv for second block
	// $ivl- iv(low), $ivh - iv(high)
	// the five ivs stored into, $iv0,$iv1,$iv2,$iv3,$iv4
	fmov	$ivl,$ivd00
	fmov	$ivh,$ivd01
	mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd10,$ivl
	fmov	$ivd11,$ivh

	ldr	$rounds0,[$key1,#240]		// next starting point
	vld1.8	{$dat},[$inp],$step

	vld1.32	{q8-q9},[$key1]			// load key schedule...
	sub	$rounds0,$rounds0,#6
	add	$key_,$key1,$ivp,lsl#4		// pointer to last 7 round keys
	sub	$rounds0,$rounds0,#2
	vld1.32	{q10-q11},[$key_],#32
	vld1.32	{q12-q13},[$key_],#32
	vld1.32	{q14-q15},[$key_],#32
	vld1.32	{$rndlast},[$key_]

	add	$key_,$key1,#32
	mov	$rounds,$rounds0

	// Encryption
.Lxts_enc:
	vld1.8	{$dat2},[$inp],#16
	subs	$len,$len,#32			// bias
	add	$rounds,$rounds0,#2
	vorr	$in1,$dat,$dat
	vorr	$dat1,$dat,$dat
	vorr	$in3,$dat,$dat
	vorr	$in2,$dat2,$dat2
	vorr	$in4,$dat2,$dat2
	b.lo	.Lxts_inner_enc_tail
	veor	$dat,$dat,$iv0			// before encryption, xor with iv
	veor	$dat2,$dat2,$iv1

	// The iv for third block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd20,$ivl
	fmov	$ivd21,$ivh


	vorr	$dat1,$dat2,$dat2
	vld1.8	{$dat2},[$inp],#16
	vorr	$in0,$dat,$dat
	vorr	$in1,$dat1,$dat1
	veor	$in2,$dat2,$iv2 		// the third block
	veor	$dat2,$dat2,$iv2
	cmp	$len,#32
	b.lo	.Lxts_outer_enc_tail

	// The iv for fourth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd30,$ivl
	fmov	$ivd31,$ivh

	vld1.8	{$dat3},[$inp],#16
	// The iv for fifth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd40,$ivl
	fmov	$ivd41,$ivh

	vld1.8	{$dat4},[$inp],#16
	veor	$dat3,$dat3,$iv3		// the fourth block
	veor	$dat4,$dat4,$iv4
	sub	$len,$len,#32			// bias
	mov	$rounds,$rounds0
	b	.Loop5x_xts_enc

.align	4
.Loop5x_xts_enc:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat3,q8
	aesmc	$dat3,$dat3
	aese	$dat4,q8
	aesmc	$dat4,$dat4
	vld1.32	{q8},[$key_],#16
	subs	$rounds,$rounds,#2
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat3,q9
	aesmc	$dat3,$dat3
	aese	$dat4,q9
	aesmc	$dat4,$dat4
	vld1.32	{q9},[$key_],#16
	b.gt	.Loop5x_xts_enc

	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat3,q8
	aesmc	$dat3,$dat3
	aese	$dat4,q8
	aesmc	$dat4,$dat4
	subs	$len,$len,#0x50			// because .Lxts_enc_tail4x

	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat3,q9
	aesmc	$dat3,$dat3
	aese	$dat4,q9
	aesmc	$dat4,$dat4
	csel	$xoffset,xzr,$len,gt		// borrow x6, w6, "gt" is not typo
	mov	$key_,$key1

	aese	$dat0,q10
	aesmc	$dat0,$dat0
	aese	$dat1,q10
	aesmc	$dat1,$dat1
	aese	$dat2,q10
	aesmc	$dat2,$dat2
	aese	$dat3,q10
	aesmc	$dat3,$dat3
	aese	$dat4,q10
	aesmc	$dat4,$dat4
	add	$inp,$inp,$xoffset		// x0 is adjusted in such way that
						// at exit from the loop v1.16b-v26.16b
						// are loaded with last "words"
	add	$xoffset,$len,#0x60		// because .Lxts_enc_tail4x

	aese	$dat0,q11
	aesmc	$dat0,$dat0
	aese	$dat1,q11
	aesmc	$dat1,$dat1
	aese	$dat2,q11
	aesmc	$dat2,$dat2
	aese	$dat3,q11
	aesmc	$dat3,$dat3
	aese	$dat4,q11
	aesmc	$dat4,$dat4

	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	aese	$dat3,q12
	aesmc	$dat3,$dat3
	aese	$dat4,q12
	aesmc	$dat4,$dat4

	aese	$dat0,q13
	aesmc	$dat0,$dat0
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	aese	$dat3,q13
	aesmc	$dat3,$dat3
	aese	$dat4,q13
	aesmc	$dat4,$dat4

	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	aese	$dat3,q14
	aesmc	$dat3,$dat3
	aese	$dat4,q14
	aesmc	$dat4,$dat4

	veor	$tmp0,$rndlast,$iv0
	aese	$dat0,q15
	// The iv for first block of one iteration
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	veor	$tmp1,$rndlast,$iv1
	vld1.8	{$in0},[$inp],#16
	aese	$dat1,q15
	// The iv for second block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd10,$ivl
	fmov	$ivd11,$ivh
	veor	$tmp2,$rndlast,$iv2
	vld1.8	{$in1},[$inp],#16
	aese	$dat2,q15
	// The iv for third block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd20,$ivl
	fmov	$ivd21,$ivh
	veor	$tmp3,$rndlast,$iv3
	vld1.8	{$in2},[$inp],#16
	aese	$dat3,q15
	// The iv for fourth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd30,$ivl
	fmov	$ivd31,$ivh
	veor	$tmp4,$rndlast,$iv4
	vld1.8	{$in3},[$inp],#16
	aese	$dat4,q15

	// The iv for fifth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd40,$ivl
	fmov	$ivd41,$ivh

	vld1.8	{$in4},[$inp],#16
	cbz	$xoffset,.Lxts_enc_tail4x
	vld1.32 {q8},[$key_],#16		// re-pre-load rndkey[0]
	veor	$tmp0,$tmp0,$dat0
	veor	$dat0,$in0,$iv0
	veor	$tmp1,$tmp1,$dat1
	veor	$dat1,$in1,$iv1
	veor	$tmp2,$tmp2,$dat2
	veor	$dat2,$in2,$iv2
	veor	$tmp3,$tmp3,$dat3
	veor	$dat3,$in3,$iv3
	veor	$tmp4,$tmp4,$dat4
	vst1.8	{$tmp0},[$out],#16
	veor	$dat4,$in4,$iv4
	vst1.8	{$tmp1},[$out],#16
	mov	$rounds,$rounds0
	vst1.8	{$tmp2},[$out],#16
	vld1.32	{q9},[$key_],#16		// re-pre-load rndkey[1]
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16
	b.hs	.Loop5x_xts_enc


	// If left 4 blocks, borrow the five block's processing.
	cmn	$len,#0x10
	b.ne	.Loop5x_enc_after
	vorr	$iv4,$iv3,$iv3
	vorr	$iv3,$iv2,$iv2
	vorr	$iv2,$iv1,$iv1
	vorr	$iv1,$iv0,$iv0
	fmov	$ivl,$ivd40
	fmov	$ivh,$ivd41
	veor	$dat0,$iv0,$in0
	veor	$dat1,$iv1,$in1
	veor	$dat2,$in2,$iv2
	veor	$dat3,$in3,$iv3
	veor	$dat4,$in4,$iv4
	b.eq	.Loop5x_xts_enc

.Loop5x_enc_after:
	add	$len,$len,#0x50
	cbz	$len,.Lxts_enc_done

	add	$rounds,$rounds0,#2
	subs	$len,$len,#0x30
	b.lo	.Lxts_inner_enc_tail

	veor	$dat0,$iv0,$in2
	veor	$dat1,$iv1,$in3
	veor	$dat2,$in4,$iv2
	b	.Lxts_outer_enc_tail

.align	4
.Lxts_enc_tail4x:
	add	$inp,$inp,#16
	veor	$tmp1,$dat1,$tmp1
	vst1.8	{$tmp1},[$out],#16
	veor	$tmp2,$dat2,$tmp2
	vst1.8	{$tmp2},[$out],#16
	veor	$tmp3,$dat3,$tmp3
	veor	$tmp4,$dat4,$tmp4
	vst1.8	{$tmp3-$tmp4},[$out],#32

	b	.Lxts_enc_done
.align	4
.Lxts_outer_enc_tail:
	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$rounds,$rounds,#2
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lxts_outer_enc_tail

	aese	$dat0,q8
	aesmc	$dat0,$dat0
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	veor	$tmp0,$iv0,$rndlast
	subs	$len,$len,#0x30
	// The iv for first block
	fmov	$ivl,$ivd20
	fmov	$ivh,$ivd21
	//mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr#31
	eor	$ivl,$tmpmx,$ivl,lsl#1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	veor	$tmp1,$iv1,$rndlast
	csel	$xoffset,$len,$xoffset,lo       // x6, w6, is zero at this point
	aese	$dat0,q9
	aesmc	$dat0,$dat0
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	veor	$tmp2,$iv2,$rndlast

	add	$xoffset,$xoffset,#0x20
	add	$inp,$inp,$xoffset
	mov	$key_,$key1

	aese	$dat0,q12
	aesmc	$dat0,$dat0
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	aese	$dat0,q13
	aesmc	$dat0,$dat0
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	aese	$dat0,q14
	aesmc	$dat0,$dat0
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	aese	$dat0,q15
	aese	$dat1,q15
	aese	$dat2,q15
	vld1.8	{$in2},[$inp],#16
	add	$rounds,$rounds0,#2
	vld1.32	{q8},[$key_],#16                // re-pre-load rndkey[0]
	veor	$tmp0,$tmp0,$dat0
	veor	$tmp1,$tmp1,$dat1
	veor	$dat2,$dat2,$tmp2
	vld1.32	{q9},[$key_],#16                // re-pre-load rndkey[1]
	vst1.8	{$tmp0},[$out],#16
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$dat2},[$out],#16
	cmn	$len,#0x30
	b.eq	.Lxts_enc_done
.Lxts_encxor_one:
	vorr	$in3,$in1,$in1
	vorr	$in4,$in2,$in2
	nop

.Lxts_inner_enc_tail:
	cmn	$len,#0x10
	veor	$dat1,$in3,$iv0
	veor	$dat2,$in4,$iv1
	b.eq	.Lxts_enc_tail_loop
	veor	$dat2,$in4,$iv0
.Lxts_enc_tail_loop:
	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$rounds,$rounds,#2
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lxts_enc_tail_loop

	aese	$dat1,q8
	aesmc	$dat1,$dat1
	aese	$dat2,q8
	aesmc	$dat2,$dat2
	aese	$dat1,q9
	aesmc	$dat1,$dat1
	aese	$dat2,q9
	aesmc	$dat2,$dat2
	aese	$dat1,q12
	aesmc	$dat1,$dat1
	aese	$dat2,q12
	aesmc	$dat2,$dat2
	cmn	$len,#0x20
	aese	$dat1,q13
	aesmc	$dat1,$dat1
	aese	$dat2,q13
	aesmc	$dat2,$dat2
	veor	$tmp1,$iv0,$rndlast
	aese	$dat1,q14
	aesmc	$dat1,$dat1
	aese	$dat2,q14
	aesmc	$dat2,$dat2
	veor	$tmp2,$iv1,$rndlast
	aese	$dat1,q15
	aese	$dat2,q15
	b.eq	.Lxts_enc_one
	veor	$tmp1,$tmp1,$dat1
	vst1.8	{$tmp1},[$out],#16
	veor	$tmp2,$tmp2,$dat2
	vorr	$iv0,$iv1,$iv1
	vst1.8	{$tmp2},[$out],#16
	fmov	$ivl,$ivd10
	fmov	$ivh,$ivd11
	mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	b	.Lxts_enc_done

.Lxts_enc_one:
	veor	$tmp1,$tmp1,$dat2
	vorr	$iv0,$iv0,$iv0
	vst1.8	{$tmp1},[$out],#16
	fmov	$ivl,$ivd00
	fmov	$ivh,$ivd01
	mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	b	.Lxts_enc_done
.align	5
.Lxts_enc_done:
	// Process the tail block with cipher stealing.
	tst	$tailcnt,#0xf
	b.eq	.Lxts_abort

	mov	$tmpinp,$inp
	mov	$tmpoutp,$out
	sub	$out,$out,#16
.composite_enc_loop:
	subs	$tailcnt,$tailcnt,#1
	ldrb	$l2outp,[$out,$tailcnt]
	ldrb	$loutp,[$tmpinp,$tailcnt]
	strb	$l2outp,[$tmpoutp,$tailcnt]
	strb	$loutp,[$out,$tailcnt]
	b.gt	.composite_enc_loop
.Lxts_enc_load_done:
	vld1.8	{$tmpin},[$out]
	veor	$tmpin,$tmpin,$iv0

	// Encrypt the composite block to get the last second encrypted text block
	ldr	$rounds,[$key1,#240]		// load key schedule...
	vld1.32	{$dat},[$key1],#16
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key1],#16		// load key schedule...
.Loop_final_enc:
	aese	$tmpin,$dat0
	aesmc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key1],#16
	subs	$rounds,$rounds,#2
	aese	$tmpin,$dat1
	aesmc	$tmpin,$tmpin
	vld1.32	{$dat1},[$key1],#16
	b.gt	.Loop_final_enc

	aese	$tmpin,$dat0
	aesmc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key1]
	aese	$tmpin,$dat1
	veor	$tmpin,$tmpin,$dat0
	veor	$tmpin,$tmpin,$iv0
	vst1.8	{$tmpin},[$out]

.Lxts_abort:
	ldp	$tailcnt,$midnumx,[sp,#48]
	ldp	$ivd10,$ivd20,[sp,#32]
	ldp	$ivd30,$ivd40,[sp,#16]
	ldp	$constnumx,$tmpinp,[sp],#64
.Lxts_enc_final_abort:
	ret
.size	${prefix}_xts_encrypt,.-${prefix}_xts_encrypt
___

}}}
{{{
my ($inp,$out,$len,$key1,$key2,$ivp)=map("x$_",(0..5));
my ($rounds0,$rounds,$key_,$step,$ivl,$ivh)=("w5","w6","x7","x8","x9","x10");
my ($tmpoutp,$loutp,$l2outp,$tmpinp)=("x13","w14","w15","x20");
my ($tailcnt,$midnum,$midnumx,$constnum,$constnumx)=("x21","w22","x22","w19","x19");
my ($xoffset,$tmpmx,$tmpmw)=("x6","x11","w11");
my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$tmp2,$rndlast)=map("q$_",(0..7));
my ($iv0,$iv1,$iv2,$iv3,$iv4,$tmpin)=("v6.16b","v8.16b","v9.16b","v10.16b","v11.16b","v26.16b");
my ($ivd00,$ivd01,$ivd20,$ivd21)=("d6","v6.d[1]","d9","v9.d[1]");
my ($ivd10,$ivd11,$ivd30,$ivd31,$ivd40,$ivd41)=("d8","v8.d[1]","d10","v10.d[1]","d11","v11.d[1]");

my ($dat,$tmp,$rndzero_n_last)=($dat0,$tmp0,$tmp1);

# q7	last round key
# q10-q15, q7	Last 7 round keys
# q8-q9	preloaded round keys except last 7 keys for big size
# q20, q21, q8-q9	preloaded round keys except last 7 keys for only 16 byte

{
my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

my ($dat3,$in3,$tmp3);	# used only in 64-bit mode
my ($dat4,$in4,$tmp4);
if ($flavour =~ /64/) {
    ($dat2,$dat3,$dat4,$in2,$in3,$in4,$tmp3,$tmp4)=map("q$_",(16..23));
}

$code.=<<___	if ($flavour =~ /64/);
.globl	${prefix}_xts_decrypt
.type	${prefix}_xts_decrypt,%function
.align	5
${prefix}_xts_decrypt:
	AARCH64_VALID_CALL_TARGET
___
$code.=<<___	if ($flavour =~ /64/);
	cmp	$len,#16
	// Original input data size bigger than 16, jump to big size processing.
	b.ne	.Lxts_dec_big_size
	// Encrypt the iv with key2, as the first XEX iv.
	ldr	$rounds,[$key2,#240]
	vld1.32	{$dat},[$key2],#16
	vld1.8	{$iv0},[$ivp]
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key2],#16

.Loop_dec_small_iv_enc:
	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2],#16
	subs	$rounds,$rounds,#2
	aese	$iv0,$dat1
	aesmc	$iv0,$iv0
	vld1.32	{$dat1},[$key2],#16
	b.gt	.Loop_dec_small_iv_enc

	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2]
	aese	$iv0,$dat1
	veor	$iv0,$iv0,$dat

	vld1.8	{$dat0},[$inp]
	veor	$dat0,$iv0,$dat0

	ldr	$rounds,[$key1,#240]
	vld1.32	{q20-q21},[$key1],#32			// load key schedule...

	aesd	$dat0,q20
	aesimc	$dat0,$dat0
	vld1.32	{q8-q9},[$key1],#32			// load key schedule...
	aesd	$dat0,q21
	aesimc	$dat0,$dat0
	subs	$rounds,$rounds,#10			// bias
	b.eq	.Lxts_128_dec
.Lxts_dec_round_loop:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	vld1.32	{q8},[$key1],#16			// load key schedule...
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	vld1.32	{q9},[$key1],#16			// load key schedule...
	subs	$rounds,$rounds,#2			// bias
	b.gt	.Lxts_dec_round_loop
.Lxts_128_dec:
	vld1.32	{q10-q11},[$key1],#32			// load key schedule...
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	vld1.32	{q12-q13},[$key1],#32			// load key schedule...
	aesd	$dat0,q10
	aesimc	$dat0,$dat0
	aesd	$dat0,q11
	aesimc	$dat0,$dat0
	vld1.32	{q14-q15},[$key1],#32			// load key schedule...
	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	vld1.32	{$rndlast},[$key1]
	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat0,q15
	veor	$dat0,$dat0,$rndlast
	veor	$dat0,$iv0,$dat0
	vst1.8	{$dat0},[$out]
	b	.Lxts_dec_final_abort
.Lxts_dec_big_size:
___
$code.=<<___	if ($flavour =~ /64/);
	stp	$constnumx,$tmpinp,[sp,#-64]!
	stp	$tailcnt,$midnumx,[sp,#48]
	stp	$ivd10,$ivd20,[sp,#32]
	stp	$ivd30,$ivd40,[sp,#16]

	and	$tailcnt,$len,#0xf
	and	$len,$len,#-16
	subs	$len,$len,#16
	mov	$step,#16
	b.lo	.Lxts_dec_abort

	// Encrypt the iv with key2, as the first XEX iv
	ldr	$rounds,[$key2,#240]
	vld1.32	{$dat},[$key2],#16
	vld1.8	{$iv0},[$ivp]
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key2],#16

.Loop_dec_iv_enc:
	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2],#16
	subs	$rounds,$rounds,#2
	aese	$iv0,$dat1
	aesmc	$iv0,$iv0
	vld1.32	{$dat1},[$key2],#16
	b.gt	.Loop_dec_iv_enc

	aese	$iv0,$dat
	aesmc	$iv0,$iv0
	vld1.32	{$dat},[$key2]
	aese	$iv0,$dat1
	veor	$iv0,$iv0,$dat

	// The iv for second block
	// $ivl- iv(low), $ivh - iv(high)
	// the five ivs stored into, $iv0,$iv1,$iv2,$iv3,$iv4
	fmov	$ivl,$ivd00
	fmov	$ivh,$ivd01
	mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd10,$ivl
	fmov	$ivd11,$ivh

	ldr	$rounds0,[$key1,#240]		// load rounds number

	// The iv for third block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd20,$ivl
	fmov	$ivd21,$ivh

	vld1.32	{q8-q9},[$key1]			// load key schedule...
	sub	$rounds0,$rounds0,#6
	add	$key_,$key1,$ivp,lsl#4		// pointer to last 7 round keys
	sub	$rounds0,$rounds0,#2
	vld1.32	{q10-q11},[$key_],#32		// load key schedule...
	vld1.32	{q12-q13},[$key_],#32
	vld1.32	{q14-q15},[$key_],#32
	vld1.32	{$rndlast},[$key_]

	// The iv for fourth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd30,$ivl
	fmov	$ivd31,$ivh

	add	$key_,$key1,#32
	mov	$rounds,$rounds0
	b	.Lxts_dec

	// Decryption
.align	5
.Lxts_dec:
	tst	$tailcnt,#0xf
	b.eq	.Lxts_dec_begin
	subs	$len,$len,#16
	csel	$step,xzr,$step,eq
	vld1.8	{$dat},[$inp],#16
	b.lo	.Lxts_done
	sub	$inp,$inp,#16
.Lxts_dec_begin:
	vld1.8	{$dat},[$inp],$step
	subs	$len,$len,#32			// bias
	add	$rounds,$rounds0,#2
	vorr	$in1,$dat,$dat
	vorr	$dat1,$dat,$dat
	vorr	$in3,$dat,$dat
	vld1.8	{$dat2},[$inp],#16
	vorr	$in2,$dat2,$dat2
	vorr	$in4,$dat2,$dat2
	b.lo	.Lxts_inner_dec_tail
	veor	$dat,$dat,$iv0			// before decryt, xor with iv
	veor	$dat2,$dat2,$iv1

	vorr	$dat1,$dat2,$dat2
	vld1.8	{$dat2},[$inp],#16
	vorr	$in0,$dat,$dat
	vorr	$in1,$dat1,$dat1
	veor	$in2,$dat2,$iv2			// third block xox with third iv
	veor	$dat2,$dat2,$iv2
	cmp	$len,#32
	b.lo	.Lxts_outer_dec_tail

	vld1.8	{$dat3},[$inp],#16

	// The iv for fifth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd40,$ivl
	fmov	$ivd41,$ivh

	vld1.8	{$dat4},[$inp],#16
	veor	$dat3,$dat3,$iv3		// the fourth block
	veor	$dat4,$dat4,$iv4
	sub $len,$len,#32			// bias
	mov	$rounds,$rounds0
	b	.Loop5x_xts_dec

.align	4
.Loop5x_xts_dec:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	vld1.32	{q8},[$key_],#16		// load key schedule...
	subs	$rounds,$rounds,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	vld1.32	{q9},[$key_],#16		// load key schedule...
	b.gt	.Loop5x_xts_dec

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat3,q8
	aesimc	$dat3,$dat3
	aesd	$dat4,q8
	aesimc	$dat4,$dat4
	subs	$len,$len,#0x50			// because .Lxts_dec_tail4x

	aesd	$dat0,q9
	aesimc	$dat0,$dat
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat3,q9
	aesimc	$dat3,$dat3
	aesd	$dat4,q9
	aesimc	$dat4,$dat4
	csel	$xoffset,xzr,$len,gt		// borrow x6, w6, "gt" is not typo
	mov	$key_,$key1

	aesd	$dat0,q10
	aesimc	$dat0,$dat0
	aesd	$dat1,q10
	aesimc	$dat1,$dat1
	aesd	$dat2,q10
	aesimc	$dat2,$dat2
	aesd	$dat3,q10
	aesimc	$dat3,$dat3
	aesd	$dat4,q10
	aesimc	$dat4,$dat4
	add	$inp,$inp,$xoffset		// x0 is adjusted in such way that
						// at exit from the loop v1.16b-v26.16b
						// are loaded with last "words"
	add	$xoffset,$len,#0x60		// because .Lxts_dec_tail4x

	aesd	$dat0,q11
	aesimc	$dat0,$dat0
	aesd	$dat1,q11
	aesimc	$dat1,$dat1
	aesd	$dat2,q11
	aesimc	$dat2,$dat2
	aesd	$dat3,q11
	aesimc	$dat3,$dat3
	aesd	$dat4,q11
	aesimc	$dat4,$dat4

	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	aesd	$dat3,q12
	aesimc	$dat3,$dat3
	aesd	$dat4,q12
	aesimc	$dat4,$dat4

	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	aesd	$dat3,q13
	aesimc	$dat3,$dat3
	aesd	$dat4,q13
	aesimc	$dat4,$dat4

	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	aesd	$dat3,q14
	aesimc	$dat3,$dat3
	aesd	$dat4,q14
	aesimc	$dat4,$dat4

	veor	$tmp0,$rndlast,$iv0
	aesd	$dat0,q15
	// The iv for first block of next iteration.
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	veor	$tmp1,$rndlast,$iv1
	vld1.8	{$in0},[$inp],#16
	aesd	$dat1,q15
	// The iv for second block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd10,$ivl
	fmov	$ivd11,$ivh
	veor	$tmp2,$rndlast,$iv2
	vld1.8	{$in1},[$inp],#16
	aesd	$dat2,q15
	// The iv for third block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd20,$ivl
	fmov	$ivd21,$ivh
	veor	$tmp3,$rndlast,$iv3
	vld1.8	{$in2},[$inp],#16
	aesd	$dat3,q15
	// The iv for fourth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd30,$ivl
	fmov	$ivd31,$ivh
	veor	$tmp4,$rndlast,$iv4
	vld1.8	{$in3},[$inp],#16
	aesd	$dat4,q15

	// The iv for fifth block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd40,$ivl
	fmov	$ivd41,$ivh

	vld1.8	{$in4},[$inp],#16
	cbz	$xoffset,.Lxts_dec_tail4x
	vld1.32	{q8},[$key_],#16		// re-pre-load rndkey[0]
	veor	$tmp0,$tmp0,$dat0
	veor	$dat0,$in0,$iv0
	veor	$tmp1,$tmp1,$dat1
	veor	$dat1,$in1,$iv1
	veor	$tmp2,$tmp2,$dat2
	veor	$dat2,$in2,$iv2
	veor	$tmp3,$tmp3,$dat3
	veor	$dat3,$in3,$iv3
	veor	$tmp4,$tmp4,$dat4
	vst1.8	{$tmp0},[$out],#16
	veor	$dat4,$in4,$iv4
	vst1.8	{$tmp1},[$out],#16
	mov	$rounds,$rounds0
	vst1.8	{$tmp2},[$out],#16
	vld1.32	{q9},[$key_],#16		// re-pre-load rndkey[1]
	vst1.8	{$tmp3},[$out],#16
	vst1.8	{$tmp4},[$out],#16
	b.hs	.Loop5x_xts_dec

	cmn	$len,#0x10
	b.ne	.Loop5x_dec_after
	// If x2($len) equal to -0x10, the left blocks is 4.
	// After specially processing, utilize the five blocks processing again.
	// It will use the following IVs: $iv0,$iv0,$iv1,$iv2,$iv3.
	vorr	$iv4,$iv3,$iv3
	vorr	$iv3,$iv2,$iv2
	vorr	$iv2,$iv1,$iv1
	vorr	$iv1,$iv0,$iv0
	fmov	$ivl,$ivd40
	fmov	$ivh,$ivd41
	veor	$dat0,$iv0,$in0
	veor	$dat1,$iv1,$in1
	veor	$dat2,$in2,$iv2
	veor	$dat3,$in3,$iv3
	veor	$dat4,$in4,$iv4
	b.eq	.Loop5x_xts_dec

.Loop5x_dec_after:
	add	$len,$len,#0x50
	cbz	$len,.Lxts_done

	add	$rounds,$rounds0,#2
	subs	$len,$len,#0x30
	b.lo	.Lxts_inner_dec_tail

	veor	$dat0,$iv0,$in2
	veor	$dat1,$iv1,$in3
	veor	$dat2,$in4,$iv2
	b	.Lxts_outer_dec_tail

.align	4
.Lxts_dec_tail4x:
	add	$inp,$inp,#16
	tst	$tailcnt,#0xf
	veor	$tmp1,$dat1,$tmp0
	vst1.8	{$tmp1},[$out],#16
	veor	$tmp2,$dat2,$tmp2
	vst1.8	{$tmp2},[$out],#16
	veor	$tmp3,$dat3,$tmp3
	veor	$tmp4,$dat4,$tmp4
	vst1.8	{$tmp3-$tmp4},[$out],#32

	b.eq	.Lxts_dec_abort
	vld1.8	{$dat0},[$inp],#16
	b	.Lxts_done
.align	4
.Lxts_outer_dec_tail:
	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$rounds,$rounds,#2
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lxts_outer_dec_tail

	aesd	$dat0,q8
	aesimc	$dat0,$dat0
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	veor	$tmp0,$iv0,$rndlast
	subs	$len,$len,#0x30
	// The iv for first block
	fmov	$ivl,$ivd20
	fmov	$ivh,$ivd21
	mov	$constnum,#0x87
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd00,$ivl
	fmov	$ivd01,$ivh
	veor	$tmp1,$iv1,$rndlast
	csel	$xoffset,$len,$xoffset,lo	// x6, w6, is zero at this point
	aesd	$dat0,q9
	aesimc	$dat0,$dat0
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	veor	$tmp2,$iv2,$rndlast
	// The iv for second block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd10,$ivl
	fmov	$ivd11,$ivh

	add	$xoffset,$xoffset,#0x20
	add	$inp,$inp,$xoffset		// $inp is adjusted to the last data

	mov	$key_,$key1

	// The iv for third block
	extr	$midnumx,$ivh,$ivh,#32
	extr	$ivh,$ivh,$ivl,#63
	and	$tmpmw,$constnum,$midnum,asr #31
	eor	$ivl,$tmpmx,$ivl,lsl #1
	fmov	$ivd20,$ivl
	fmov	$ivd21,$ivh

	aesd	$dat0,q12
	aesimc	$dat0,$dat0
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	aesd	$dat0,q13
	aesimc	$dat0,$dat0
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	aesd	$dat0,q14
	aesimc	$dat0,$dat0
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	vld1.8	{$in2},[$inp],#16
	aesd	$dat0,q15
	aesd	$dat1,q15
	aesd	$dat2,q15
	vld1.32	{q8},[$key_],#16		// re-pre-load rndkey[0]
	add	$rounds,$rounds0,#2
	veor	$tmp0,$tmp0,$dat0
	veor	$tmp1,$tmp1,$dat1
	veor	$dat2,$dat2,$tmp2
	vld1.32	{q9},[$key_],#16		// re-pre-load rndkey[1]
	vst1.8	{$tmp0},[$out],#16
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$dat2},[$out],#16

	cmn	$len,#0x30
	add	$len,$len,#0x30
	b.eq	.Lxts_done
	sub	$len,$len,#0x30
	vorr	$in3,$in1,$in1
	vorr	$in4,$in2,$in2
	nop

.Lxts_inner_dec_tail:
	// $len == -0x10 means two blocks left.
	cmn	$len,#0x10
	veor	$dat1,$in3,$iv0
	veor	$dat2,$in4,$iv1
	b.eq	.Lxts_dec_tail_loop
	veor	$dat2,$in4,$iv0
.Lxts_dec_tail_loop:
	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	vld1.32	{q8},[$key_],#16
	subs	$rounds,$rounds,#2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	vld1.32	{q9},[$key_],#16
	b.gt	.Lxts_dec_tail_loop

	aesd	$dat1,q8
	aesimc	$dat1,$dat1
	aesd	$dat2,q8
	aesimc	$dat2,$dat2
	aesd	$dat1,q9
	aesimc	$dat1,$dat1
	aesd	$dat2,q9
	aesimc	$dat2,$dat2
	aesd	$dat1,q12
	aesimc	$dat1,$dat1
	aesd	$dat2,q12
	aesimc	$dat2,$dat2
	cmn	$len,#0x20
	aesd	$dat1,q13
	aesimc	$dat1,$dat1
	aesd	$dat2,q13
	aesimc	$dat2,$dat2
	veor	$tmp1,$iv0,$rndlast
	aesd	$dat1,q14
	aesimc	$dat1,$dat1
	aesd	$dat2,q14
	aesimc	$dat2,$dat2
	veor	$tmp2,$iv1,$rndlast
	aesd	$dat1,q15
	aesd	$dat2,q15
	b.eq	.Lxts_dec_one
	veor	$tmp1,$tmp1,$dat1
	veor	$tmp2,$tmp2,$dat2
	vorr	$iv0,$iv2,$iv2
	vorr	$iv1,$iv3,$iv3
	vst1.8	{$tmp1},[$out],#16
	vst1.8	{$tmp2},[$out],#16
	add	$len,$len,#16
	b	.Lxts_done

.Lxts_dec_one:
	veor	$tmp1,$tmp1,$dat2
	vorr	$iv0,$iv1,$iv1
	vorr	$iv1,$iv2,$iv2
	vst1.8	{$tmp1},[$out],#16
	add	$len,$len,#32

.Lxts_done:
	tst	$tailcnt,#0xf
	b.eq	.Lxts_dec_abort
	// Processing the last two blocks with cipher stealing.
	mov	x7,x3
	cbnz	x2,.Lxts_dec_1st_done
	vld1.8	{$dat0},[$inp],#16

	// Decrypt the last second block to get the last plain text block
.Lxts_dec_1st_done:
	eor	$tmpin,$dat0,$iv1
	ldr	$rounds,[$key1,#240]
	vld1.32	{$dat0},[$key1],#16
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key1],#16
.Loop_final_2nd_dec:
	aesd	$tmpin,$dat0
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key1],#16		// load key schedule...
	subs	$rounds,$rounds,#2
	aesd	$tmpin,$dat1
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat1},[$key1],#16		// load key schedule...
	b.gt	.Loop_final_2nd_dec

	aesd	$tmpin,$dat0
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key1]
	aesd	$tmpin,$dat1
	veor	$tmpin,$tmpin,$dat0
	veor	$tmpin,$tmpin,$iv1
	vst1.8	{$tmpin},[$out]

	mov	$tmpinp,$inp
	add	$tmpoutp,$out,#16

	// Composite the tailcnt "16 byte not aligned block" into the last second plain blocks
	// to get the last encrypted block.
.composite_dec_loop:
	subs	$tailcnt,$tailcnt,#1
	ldrb	$l2outp,[$out,$tailcnt]
	ldrb	$loutp,[$tmpinp,$tailcnt]
	strb	$l2outp,[$tmpoutp,$tailcnt]
	strb	$loutp,[$out,$tailcnt]
	b.gt	.composite_dec_loop
.Lxts_dec_load_done:
	vld1.8	{$tmpin},[$out]
	veor	$tmpin,$tmpin,$iv0

	// Decrypt the composite block to get the last second plain text block
	ldr	$rounds,[$key_,#240]
	vld1.32	{$dat},[$key_],#16
	sub	$rounds,$rounds,#2
	vld1.32	{$dat1},[$key_],#16
.Loop_final_dec:
	aesd	$tmpin,$dat0
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key_],#16		// load key schedule...
	subs	$rounds,$rounds,#2
	aesd	$tmpin,$dat1
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat1},[$key_],#16		// load key schedule...
	b.gt	.Loop_final_dec

	aesd	$tmpin,$dat0
	aesimc	$tmpin,$tmpin
	vld1.32	{$dat0},[$key_]
	aesd	$tmpin,$dat1
	veor	$tmpin,$tmpin,$dat0
	veor	$tmpin,$tmpin,$iv0
	vst1.8	{$tmpin},[$out]

.Lxts_dec_abort:
	ldp	$tailcnt,$midnumx,[sp,#48]
	ldp	$ivd10,$ivd20,[sp,#32]
	ldp	$ivd30,$ivd40,[sp,#16]
	ldp	$constnumx,$tmpinp,[sp],#64

.Lxts_dec_final_abort:
	ret
.size	${prefix}_xts_decrypt,.-${prefix}_xts_decrypt
___
}
}}}
$code.=<<___;
#endif
___
########################################
if ($flavour =~ /64/) {			######## 64-bit code
    my %opcode = (
	"aesd"	=>	0x4e285800,	"aese"	=>	0x4e284800,
	"aesimc"=>	0x4e287800,	"aesmc"	=>	0x4e286800,
	"eor3"	=>	0xce000000,	);

    local *unaes = sub {
	my ($mnemonic,$arg)=@_;

	$arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)/o	&&
	sprintf ".inst\t0x%08x\t//%s %s",
			$opcode{$mnemonic}|$1|($2<<5),
			$mnemonic,$arg;
    };

    sub unsha3 {
		 my ($mnemonic,$arg)=@_;

		 $arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)[^,]*(?:,\s*[qv]([0-9]+)[^,]*(?:,\s*[qv#]([0-9\-]+))?)?/
		 &&
		 sprintf ".inst\t0x%08x\t//%s %s",
			$opcode{$mnemonic}|$1|($2<<5)|($3<<16)|(eval($4)<<10),
			$mnemonic,$arg;
    }

    foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/geo;

	s/\bq([0-9]+)\b/"v".($1<8?$1:($1<24?$1+8:$1-16)).".16b"/geo;	# old->new registers
	s/\bq_([0-9]+)\b/"q".$1/geo;	# old->new registers
	s/@\s/\/\//o;			# old->new style commentary

	#s/[v]?(aes\w+)\s+([qv].*)/unaes($1,$2)/geo	or
	s/cclr\s+([wx])([^,]+),\s*([a-z]+)/csel	$1$2,$1zr,$1$2,$3/o	or
	s/mov\.([a-z]+)\s+([wx][0-9]+),\s*([wx][0-9]+)/csel	$2,$3,$2,$1/o	or
	s/vmov\.i8/movi/o	or	# fix up legacy mnemonics
	s/vext\.8/ext/o		or
	s/vrev32\.8/rev32/o	or
	s/vtst\.8/cmtst/o	or
	s/vshr/ushr/o		or
	s/^(\s+)v/$1/o		or	# strip off v prefix
	s/\bbx\s+lr\b/ret/o;
	s/\b(eor3)\s+(v.*)/unsha3($1,$2)/ge;

	# fix up remaining legacy suffixes
	s/\.[ui]?8//o;
	m/\],#8/o and s/\.16b/\.8b/go;
	s/\.[ui]?32//o and s/\.16b/\.4s/go;
	s/\.[ui]?64//o and s/\.16b/\.2d/go;
	s/\.[42]([sd])\[([0-3])\]/\.$1\[$2\]/o;

	# Switch preprocessor checks to aarch64 versions.
	s/__ARME([BL])__/__AARCH64E$1__/go;

	print $_,"\n";
    }
} else {				######## 32-bit code
    my %opcode = (
	"aesd"	=>	0xf3b00340,	"aese"	=>	0xf3b00300,
	"aesimc"=>	0xf3b003c0,	"aesmc"	=>	0xf3b00380	);

    local *unaes = sub {
	my ($mnemonic,$arg)=@_;

	if ($arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)/o) {
	    my $word = $opcode{$mnemonic}|(($1&7)<<13)|(($1&8)<<19)
					 |(($2&7)<<1) |(($2&8)<<2);
	    # since ARMv7 instructions are always encoded little-endian.
	    # correct solution is to use .inst directive, but older
	    # assemblers don't implement it:-(
	    sprintf "INST(0x%02x,0x%02x,0x%02x,0x%02x)\t@ %s %s",
			$word&0xff,($word>>8)&0xff,
			($word>>16)&0xff,($word>>24)&0xff,
			$mnemonic,$arg;
	}
    };

    sub unvtbl {
	my $arg=shift;

	$arg =~ m/q([0-9]+),\s*\{q([0-9]+)\},\s*q([0-9]+)/o &&
	sprintf	"vtbl.8	d%d,{q%d},d%d\n\t".
		"vtbl.8	d%d,{q%d},d%d", 2*$1,$2,2*$3, 2*$1+1,$2,2*$3+1;
    }

    sub unvdup32 {
	my $arg=shift;

	$arg =~ m/q([0-9]+),\s*q([0-9]+)\[([0-3])\]/o &&
	sprintf	"vdup.32	q%d,d%d[%d]",$1,2*$2+($3>>1),$3&1;
    }

    sub unvmov32 {
	my $arg=shift;

	$arg =~ m/q([0-9]+)\[([0-3])\],(.*)/o &&
	sprintf	"vmov.32	d%d[%d],%s",2*$1+($2>>1),$2&1,$3;
    }

    foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/geo;

	s/\b[wx]([0-9]+)\b/r$1/go;		# new->old registers
	s/\bv([0-9])\.[12468]+[bsd]\b/q$1/go;	# new->old registers
	s/\/\/\s?/@ /o;				# new->old style commentary

	# fix up remaining new-style suffixes
	s/\{q([0-9]+)\},\s*\[(.+)\],#8/sprintf "{d%d},[$2]!",2*$1/eo	or
	s/\],#[0-9]+/]!/o;

	s/[v]?(aes\w+)\s+([qv].*)/unaes($1,$2)/geo	or
	s/cclr\s+([^,]+),\s*([a-z]+)/mov.$2	$1,#0/o	or
	s/vtbl\.8\s+(.*)/unvtbl($1)/geo			or
	s/vdup\.32\s+(.*)/unvdup32($1)/geo		or
	s/vmov\.32\s+(.*)/unvmov32($1)/geo		or
	s/^(\s+)b\./$1b/o				or
	s/^(\s+)ret/$1bx\tlr/o;

	if (s/^(\s+)mov\.([a-z]+)/$1mov$2/) {
	    print "	it	$2\n";
	}

	print $_,"\n";
    }
}

close STDOUT or die "error closing STDOUT: $!";
