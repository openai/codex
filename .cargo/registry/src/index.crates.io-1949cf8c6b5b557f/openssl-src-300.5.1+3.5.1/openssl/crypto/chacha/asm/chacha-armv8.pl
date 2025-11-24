#! /usr/bin/env perl
# Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
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
# June 2015
#
# ChaCha20 for ARMv8.
#
# April 2019
#
# Replace 3xNEON+1xIALU code path with 4+1. 4+1 is actually fastest
# option on most(*), but not all, processors, yet 6+2 is retained.
# This is because penalties are considered tolerable in comparison to
# improvement on processors where 6+2 helps. Most notably +37% on
# ThunderX2. It's server-oriented processor which will have to serve
# as many requests as possible. While others are mostly clients, when
# performance doesn't have to be absolute top-notch, just fast enough,
# as majority of time is spent "entertaining" relatively slow human.
#
# Performance in cycles per byte out of large buffer.
#
#			IALU/gcc-4.9	4xNEON+1xIALU	6xNEON+2xIALU
#
# Apple A7		5.50/+49%	2.72		1.60
# Cortex-A53		8.40/+80%	4.06		4.45(*)
# Cortex-A57		8.06/+43%	4.15		4.40(*)
# Denver		4.50/+82%	2.30		2.70(*)
# X-Gene		9.50/+46%	8.20		8.90(*)
# Mongoose		8.00/+44%	2.74		3.12(*)
# Kryo			8.17/+50%	4.47		4.65(*)
# ThunderX2		7.22/+48%	5.64		4.10
#
# (*)	slower than 4+1:-(

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

sub AUTOLOAD()		# thunk [simplified] x86-style perlasm
{ my $opcode = $AUTOLOAD; $opcode =~ s/.*:://; $opcode =~ s/_/\./;
  my $arg = pop;
    $arg = "#$arg" if ($arg*1 eq $arg);
    $code .= "\t$opcode\t".join(',',@_,$arg)."\n";
}

my ($out,$inp,$len,$key,$ctr) = map("x$_",(0..4));

my @x=map("x$_",(5..17,19..21));
my @d=map("x$_",(22..28,30));

sub ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));

    (
	"&add_32	(@x[$a0],@x[$a0],@x[$b0])",
	 "&add_32	(@x[$a1],@x[$a1],@x[$b1])",
	  "&add_32	(@x[$a2],@x[$a2],@x[$b2])",
	   "&add_32	(@x[$a3],@x[$a3],@x[$b3])",
	"&eor_32	(@x[$d0],@x[$d0],@x[$a0])",
	 "&eor_32	(@x[$d1],@x[$d1],@x[$a1])",
	  "&eor_32	(@x[$d2],@x[$d2],@x[$a2])",
	   "&eor_32	(@x[$d3],@x[$d3],@x[$a3])",
	"&ror_32	(@x[$d0],@x[$d0],16)",
	 "&ror_32	(@x[$d1],@x[$d1],16)",
	  "&ror_32	(@x[$d2],@x[$d2],16)",
	   "&ror_32	(@x[$d3],@x[$d3],16)",

	"&add_32	(@x[$c0],@x[$c0],@x[$d0])",
	 "&add_32	(@x[$c1],@x[$c1],@x[$d1])",
	  "&add_32	(@x[$c2],@x[$c2],@x[$d2])",
	   "&add_32	(@x[$c3],@x[$c3],@x[$d3])",
	"&eor_32	(@x[$b0],@x[$b0],@x[$c0])",
	 "&eor_32	(@x[$b1],@x[$b1],@x[$c1])",
	  "&eor_32	(@x[$b2],@x[$b2],@x[$c2])",
	   "&eor_32	(@x[$b3],@x[$b3],@x[$c3])",
	"&ror_32	(@x[$b0],@x[$b0],20)",
	 "&ror_32	(@x[$b1],@x[$b1],20)",
	  "&ror_32	(@x[$b2],@x[$b2],20)",
	   "&ror_32	(@x[$b3],@x[$b3],20)",

	"&add_32	(@x[$a0],@x[$a0],@x[$b0])",
	 "&add_32	(@x[$a1],@x[$a1],@x[$b1])",
	  "&add_32	(@x[$a2],@x[$a2],@x[$b2])",
	   "&add_32	(@x[$a3],@x[$a3],@x[$b3])",
	"&eor_32	(@x[$d0],@x[$d0],@x[$a0])",
	 "&eor_32	(@x[$d1],@x[$d1],@x[$a1])",
	  "&eor_32	(@x[$d2],@x[$d2],@x[$a2])",
	   "&eor_32	(@x[$d3],@x[$d3],@x[$a3])",
	"&ror_32	(@x[$d0],@x[$d0],24)",
	 "&ror_32	(@x[$d1],@x[$d1],24)",
	  "&ror_32	(@x[$d2],@x[$d2],24)",
	   "&ror_32	(@x[$d3],@x[$d3],24)",

	"&add_32	(@x[$c0],@x[$c0],@x[$d0])",
	 "&add_32	(@x[$c1],@x[$c1],@x[$d1])",
	  "&add_32	(@x[$c2],@x[$c2],@x[$d2])",
	   "&add_32	(@x[$c3],@x[$c3],@x[$d3])",
	"&eor_32	(@x[$b0],@x[$b0],@x[$c0])",
	 "&eor_32	(@x[$b1],@x[$b1],@x[$c1])",
	  "&eor_32	(@x[$b2],@x[$b2],@x[$c2])",
	   "&eor_32	(@x[$b3],@x[$b3],@x[$c3])",
	"&ror_32	(@x[$b0],@x[$b0],25)",
	 "&ror_32	(@x[$b1],@x[$b1],25)",
	  "&ror_32	(@x[$b2],@x[$b2],25)",
	   "&ror_32	(@x[$b3],@x[$b3],25)"
    );
}

$code.=<<___;
#include "arm_arch.h"
#ifndef	__KERNEL__
.extern	OPENSSL_armcap_P
.hidden	OPENSSL_armcap_P

.extern ChaCha20_ctr32_sve
#endif

.rodata

.align	5
.Lsigma:
.quad	0x3320646e61707865,0x6b20657479622d32		// endian-neutral
.Lone:
.long	1,2,3,4
.Lrot24:
.long	0x02010003,0x06050407,0x0a09080b,0x0e0d0c0f
.asciz	"ChaCha20 for ARMv8, CRYPTOGAMS by \@dot-asm"

.text

.globl	ChaCha20_ctr32_dflt
.type	ChaCha20_ctr32_dflt,%function
.align	5
ChaCha20_ctr32_dflt:
	AARCH64_SIGN_LINK_REGISTER
	cmp	$len,#192
	b.lo	.Lshort
#ifndef	__KERNEL__
	adrp	x17,OPENSSL_armcap_P
	ldr	w17,[x17,#:lo12:OPENSSL_armcap_P]
.Lcheck_neon:
	tst	w17,#ARMV7_NEON
	b.ne	.LChaCha20_neon
#endif

.Lshort:
	stp	x29,x30,[sp,#-96]!
	add	x29,sp,#0

	adrp	@x[0],.Lsigma
	add	@x[0],@x[0],:lo12:.Lsigma
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]
	stp	x25,x26,[sp,#64]
	stp	x27,x28,[sp,#80]
	sub	sp,sp,#64

	ldp	@d[0],@d[1],[@x[0]]		// load sigma
	ldp	@d[2],@d[3],[$key]		// load key
	ldp	@d[4],@d[5],[$key,#16]
	ldp	@d[6],@d[7],[$ctr]		// load counter
#ifdef	__AARCH64EB__
	ror	@d[2],@d[2],#32
	ror	@d[3],@d[3],#32
	ror	@d[4],@d[4],#32
	ror	@d[5],@d[5],#32
	ror	@d[6],@d[6],#32
	ror	@d[7],@d[7],#32
#endif

.Loop_outer:
	mov.32	@x[0],@d[0]			// unpack key block
	lsr	@x[1],@d[0],#32
	mov.32	@x[2],@d[1]
	lsr	@x[3],@d[1],#32
	mov.32	@x[4],@d[2]
	lsr	@x[5],@d[2],#32
	mov.32	@x[6],@d[3]
	lsr	@x[7],@d[3],#32
	mov.32	@x[8],@d[4]
	lsr	@x[9],@d[4],#32
	mov.32	@x[10],@d[5]
	lsr	@x[11],@d[5],#32
	mov.32	@x[12],@d[6]
	lsr	@x[13],@d[6],#32
	mov.32	@x[14],@d[7]
	lsr	@x[15],@d[7],#32

	mov	$ctr,#10
	subs	$len,$len,#64
.Loop:
	sub	$ctr,$ctr,#1
___
	foreach (&ROUND(0, 4, 8,12)) { eval; }
	foreach (&ROUND(0, 5,10,15)) { eval; }
$code.=<<___;
	cbnz	$ctr,.Loop

	add.32	@x[0],@x[0],@d[0]		// accumulate key block
	add	@x[1],@x[1],@d[0],lsr#32
	add.32	@x[2],@x[2],@d[1]
	add	@x[3],@x[3],@d[1],lsr#32
	add.32	@x[4],@x[4],@d[2]
	add	@x[5],@x[5],@d[2],lsr#32
	add.32	@x[6],@x[6],@d[3]
	add	@x[7],@x[7],@d[3],lsr#32
	add.32	@x[8],@x[8],@d[4]
	add	@x[9],@x[9],@d[4],lsr#32
	add.32	@x[10],@x[10],@d[5]
	add	@x[11],@x[11],@d[5],lsr#32
	add.32	@x[12],@x[12],@d[6]
	add	@x[13],@x[13],@d[6],lsr#32
	add.32	@x[14],@x[14],@d[7]
	add	@x[15],@x[15],@d[7],lsr#32

	b.lo	.Ltail

	add	@x[0],@x[0],@x[1],lsl#32	// pack
	add	@x[2],@x[2],@x[3],lsl#32
	ldp	@x[1],@x[3],[$inp,#0]		// load input
	add	@x[4],@x[4],@x[5],lsl#32
	add	@x[6],@x[6],@x[7],lsl#32
	ldp	@x[5],@x[7],[$inp,#16]
	add	@x[8],@x[8],@x[9],lsl#32
	add	@x[10],@x[10],@x[11],lsl#32
	ldp	@x[9],@x[11],[$inp,#32]
	add	@x[12],@x[12],@x[13],lsl#32
	add	@x[14],@x[14],@x[15],lsl#32
	ldp	@x[13],@x[15],[$inp,#48]
	add	$inp,$inp,#64
#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	eor	@x[0],@x[0],@x[1]
	eor	@x[2],@x[2],@x[3]
	eor	@x[4],@x[4],@x[5]
	eor	@x[6],@x[6],@x[7]
	eor	@x[8],@x[8],@x[9]
	eor	@x[10],@x[10],@x[11]
	eor	@x[12],@x[12],@x[13]
	eor	@x[14],@x[14],@x[15]

	stp	@x[0],@x[2],[$out,#0]		// store output
	 add	@d[6],@d[6],#1			// increment counter
	stp	@x[4],@x[6],[$out,#16]
	stp	@x[8],@x[10],[$out,#32]
	stp	@x[12],@x[14],[$out,#48]
	add	$out,$out,#64

	b.hi	.Loop_outer

	ldp	x19,x20,[x29,#16]
	add	sp,sp,#64
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
.Labort:
	AARCH64_VALIDATE_LINK_REGISTER
	ret

.align	4
.Ltail:
	add	$len,$len,#64
.Less_than_64:
	sub	$out,$out,#1
	add	$inp,$inp,$len
	add	$out,$out,$len
	add	$ctr,sp,$len
	neg	$len,$len

	add	@x[0],@x[0],@x[1],lsl#32	// pack
	add	@x[2],@x[2],@x[3],lsl#32
	add	@x[4],@x[4],@x[5],lsl#32
	add	@x[6],@x[6],@x[7],lsl#32
	add	@x[8],@x[8],@x[9],lsl#32
	add	@x[10],@x[10],@x[11],lsl#32
	add	@x[12],@x[12],@x[13],lsl#32
	add	@x[14],@x[14],@x[15],lsl#32
#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	stp	@x[0],@x[2],[sp,#0]
	stp	@x[4],@x[6],[sp,#16]
	stp	@x[8],@x[10],[sp,#32]
	stp	@x[12],@x[14],[sp,#48]

.Loop_tail:
	ldrb	w10,[$inp,$len]
	ldrb	w11,[$ctr,$len]
	add	$len,$len,#1
	eor	w10,w10,w11
	strb	w10,[$out,$len]
	cbnz	$len,.Loop_tail

	stp	xzr,xzr,[sp,#0]
	stp	xzr,xzr,[sp,#16]
	stp	xzr,xzr,[sp,#32]
	stp	xzr,xzr,[sp,#48]

	ldp	x19,x20,[x29,#16]
	add	sp,sp,#64
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	ChaCha20_ctr32_dflt,.-ChaCha20_ctr32_dflt

.globl	ChaCha20_ctr32
.type	ChaCha20_ctr32,%function
.align	5
ChaCha20_ctr32:
	AARCH64_SIGN_LINK_REGISTER
	cbz	$len,.Labort
	cmp	$len,#192
	b.lo	.Lshort
#ifndef	__KERNEL__
	adrp	x17,OPENSSL_armcap_P
	ldr	w17,[x17,#:lo12:OPENSSL_armcap_P]
	tst	w17,#ARMV8_SVE
	b.eq	.Lcheck_neon
	stp	x29,x30,[sp,#-16]!
	sub	sp,sp,#16
	// SVE handling will inevitably increment the counter
	// Neon/Scalar code that follows to process tail data needs to
	// use new counter, unfortunately the input counter buffer
	// pointed to by ctr is meant to be read-only per API contract
	// we have to copy the buffer to stack to be writable by SVE
	ldp	x5,x6,[$ctr]
	stp	x5,x6,[sp]
	mov	$ctr,sp
	bl	ChaCha20_ctr32_sve
	cbz	$len,1f
	bl	ChaCha20_ctr32_dflt
1:
	add	sp,sp,#16
	ldp	x29,x30,[sp],#16
	AARCH64_VALIDATE_LINK_REGISTER
	ret
#endif
	b	.Lshort
.size	ChaCha20_ctr32,.-ChaCha20_ctr32
___

{{{
my @K = map("v$_.4s",(0..3));
my ($xt0,$xt1,$xt2,$xt3, $CTR,$ROT24) = map("v$_.4s",(4..9));
my @X = map("v$_.4s",(16,20,24,28, 17,21,25,29, 18,22,26,30, 19,23,27,31));
my ($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
    $xc0,$xc1,$xc2,$xc3, $xd0,$xd1,$xd2,$xd3) = @X;

sub NEON_lane_ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my @x=map("'$_'",@X);

	(
	"&add		(@x[$a0],@x[$a0],@x[$b0])",	# Q1
	 "&add		(@x[$a1],@x[$a1],@x[$b1])",	# Q2
	  "&add		(@x[$a2],@x[$a2],@x[$b2])",	# Q3
	   "&add	(@x[$a3],@x[$a3],@x[$b3])",	# Q4
	"&eor		(@x[$d0],@x[$d0],@x[$a0])",
	 "&eor		(@x[$d1],@x[$d1],@x[$a1])",
	  "&eor		(@x[$d2],@x[$d2],@x[$a2])",
	   "&eor	(@x[$d3],@x[$d3],@x[$a3])",
	"&rev32_16	(@x[$d0],@x[$d0])",
	 "&rev32_16	(@x[$d1],@x[$d1])",
	  "&rev32_16	(@x[$d2],@x[$d2])",
	   "&rev32_16	(@x[$d3],@x[$d3])",

	"&add		(@x[$c0],@x[$c0],@x[$d0])",
	 "&add		(@x[$c1],@x[$c1],@x[$d1])",
	  "&add		(@x[$c2],@x[$c2],@x[$d2])",
	   "&add	(@x[$c3],@x[$c3],@x[$d3])",
	"&eor		('$xt0',@x[$b0],@x[$c0])",
	 "&eor		('$xt1',@x[$b1],@x[$c1])",
	  "&eor		('$xt2',@x[$b2],@x[$c2])",
	   "&eor	('$xt3',@x[$b3],@x[$c3])",
	"&ushr		(@x[$b0],'$xt0',20)",
	 "&ushr		(@x[$b1],'$xt1',20)",
	  "&ushr	(@x[$b2],'$xt2',20)",
	   "&ushr	(@x[$b3],'$xt3',20)",
	"&sli		(@x[$b0],'$xt0',12)",
	 "&sli		(@x[$b1],'$xt1',12)",
	  "&sli		(@x[$b2],'$xt2',12)",
	   "&sli	(@x[$b3],'$xt3',12)",

	"&add		(@x[$a0],@x[$a0],@x[$b0])",
	 "&add		(@x[$a1],@x[$a1],@x[$b1])",
	  "&add		(@x[$a2],@x[$a2],@x[$b2])",
	   "&add	(@x[$a3],@x[$a3],@x[$b3])",
	"&eor		('$xt0',@x[$d0],@x[$a0])",
	 "&eor		('$xt1',@x[$d1],@x[$a1])",
	  "&eor		('$xt2',@x[$d2],@x[$a2])",
	   "&eor	('$xt3',@x[$d3],@x[$a3])",
	"&tbl		(@x[$d0],'{$xt0}','$ROT24')",
	 "&tbl		(@x[$d1],'{$xt1}','$ROT24')",
	  "&tbl		(@x[$d2],'{$xt2}','$ROT24')",
	   "&tbl	(@x[$d3],'{$xt3}','$ROT24')",

	"&add		(@x[$c0],@x[$c0],@x[$d0])",
	 "&add		(@x[$c1],@x[$c1],@x[$d1])",
	  "&add		(@x[$c2],@x[$c2],@x[$d2])",
	   "&add	(@x[$c3],@x[$c3],@x[$d3])",
	"&eor		('$xt0',@x[$b0],@x[$c0])",
	 "&eor		('$xt1',@x[$b1],@x[$c1])",
	  "&eor		('$xt2',@x[$b2],@x[$c2])",
	   "&eor	('$xt3',@x[$b3],@x[$c3])",
	"&ushr		(@x[$b0],'$xt0',25)",
	 "&ushr		(@x[$b1],'$xt1',25)",
	  "&ushr	(@x[$b2],'$xt2',25)",
	   "&ushr	(@x[$b3],'$xt3',25)",
	"&sli		(@x[$b0],'$xt0',7)",
	 "&sli		(@x[$b1],'$xt1',7)",
	  "&sli		(@x[$b2],'$xt2',7)",
	   "&sli	(@x[$b3],'$xt3',7)"
	);
}

$code.=<<___;

#ifdef	__KERNEL__
.globl	ChaCha20_neon
#endif
.type	ChaCha20_neon,%function
.align	5
ChaCha20_neon:
	AARCH64_SIGN_LINK_REGISTER
.LChaCha20_neon:
	stp	x29,x30,[sp,#-96]!
	add	x29,sp,#0

	adrp	@x[0],.Lsigma
	add	@x[0],@x[0],:lo12:.Lsigma
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]
	stp	x25,x26,[sp,#64]
	stp	x27,x28,[sp,#80]
	cmp	$len,#512
	b.hs	.L512_or_more_neon

	sub	sp,sp,#64

	ldp	@d[0],@d[1],[@x[0]]		// load sigma
	ld1	{@K[0]},[@x[0]],#16
	ldp	@d[2],@d[3],[$key]		// load key
	ldp	@d[4],@d[5],[$key,#16]
	ld1	{@K[1],@K[2]},[$key]
	ldp	@d[6],@d[7],[$ctr]		// load counter
	ld1	{@K[3]},[$ctr]
	stp	d8,d9,[sp]			// meet ABI requirements
	ld1	{$CTR,$ROT24},[@x[0]]
#ifdef	__AARCH64EB__
	rev64	@K[0],@K[0]
	ror	@d[2],@d[2],#32
	ror	@d[3],@d[3],#32
	ror	@d[4],@d[4],#32
	ror	@d[5],@d[5],#32
	ror	@d[6],@d[6],#32
	ror	@d[7],@d[7],#32
#endif

.Loop_outer_neon:
	dup	$xa0,@{K[0]}[0]			// unpack key block
	 mov.32	@x[0],@d[0]
	dup	$xa1,@{K[0]}[1]
	 lsr	@x[1],@d[0],#32
	dup	$xa2,@{K[0]}[2]
	 mov.32	@x[2],@d[1]
	dup	$xa3,@{K[0]}[3]
	 lsr	@x[3],@d[1],#32
	dup	$xb0,@{K[1]}[0]
	 mov.32	@x[4],@d[2]
	dup	$xb1,@{K[1]}[1]
	 lsr	@x[5],@d[2],#32
	dup	$xb2,@{K[1]}[2]
	 mov.32	@x[6],@d[3]
	dup	$xb3,@{K[1]}[3]
	 lsr	@x[7],@d[3],#32
	dup	$xd0,@{K[3]}[0]
	 mov.32	@x[8],@d[4]
	dup	$xd1,@{K[3]}[1]
	 lsr	@x[9],@d[4],#32
	dup	$xd2,@{K[3]}[2]
	 mov.32	@x[10],@d[5]
	dup	$xd3,@{K[3]}[3]
	 lsr	@x[11],@d[5],#32
	add	$xd0,$xd0,$CTR
	 mov.32	@x[12],@d[6]
	dup	$xc0,@{K[2]}[0]
	 lsr	@x[13],@d[6],#32
	dup	$xc1,@{K[2]}[1]
	 mov.32	@x[14],@d[7]
	dup	$xc2,@{K[2]}[2]
	 lsr	@x[15],@d[7],#32
	dup	$xc3,@{K[2]}[3]

	mov	$ctr,#10
	subs	$len,$len,#320
.Loop_neon:
	sub	$ctr,$ctr,#1
___
	my @plus_one=&ROUND(0,4,8,12);
	foreach (&NEON_lane_ROUND(0,4,8,12))  { eval; eval(shift(@plus_one)); }

	@plus_one=&ROUND(0,5,10,15);
	foreach (&NEON_lane_ROUND(0,5,10,15)) { eval; eval(shift(@plus_one)); }
$code.=<<___;
	cbnz	$ctr,.Loop_neon

	add	$xd0,$xd0,$CTR

	zip1	$xt0,$xa0,$xa1			// transpose data
	zip1	$xt1,$xa2,$xa3
	zip2	$xt2,$xa0,$xa1
	zip2	$xt3,$xa2,$xa3
	zip1.64	$xa0,$xt0,$xt1
	zip2.64	$xa1,$xt0,$xt1
	zip1.64	$xa2,$xt2,$xt3
	zip2.64	$xa3,$xt2,$xt3

	zip1	$xt0,$xb0,$xb1
	zip1	$xt1,$xb2,$xb3
	zip2	$xt2,$xb0,$xb1
	zip2	$xt3,$xb2,$xb3
	zip1.64	$xb0,$xt0,$xt1
	zip2.64	$xb1,$xt0,$xt1
	zip1.64	$xb2,$xt2,$xt3
	zip2.64	$xb3,$xt2,$xt3

	zip1	$xt0,$xc0,$xc1
	 add.32	@x[0],@x[0],@d[0]		// accumulate key block
	zip1	$xt1,$xc2,$xc3
	 add	@x[1],@x[1],@d[0],lsr#32
	zip2	$xt2,$xc0,$xc1
	 add.32	@x[2],@x[2],@d[1]
	zip2	$xt3,$xc2,$xc3
	 add	@x[3],@x[3],@d[1],lsr#32
	zip1.64	$xc0,$xt0,$xt1
	 add.32	@x[4],@x[4],@d[2]
	zip2.64	$xc1,$xt0,$xt1
	 add	@x[5],@x[5],@d[2],lsr#32
	zip1.64	$xc2,$xt2,$xt3
	 add.32	@x[6],@x[6],@d[3]
	zip2.64	$xc3,$xt2,$xt3
	 add	@x[7],@x[7],@d[3],lsr#32

	zip1	$xt0,$xd0,$xd1
	 add.32	@x[8],@x[8],@d[4]
	zip1	$xt1,$xd2,$xd3
	 add	@x[9],@x[9],@d[4],lsr#32
	zip2	$xt2,$xd0,$xd1
	 add.32	@x[10],@x[10],@d[5]
	zip2	$xt3,$xd2,$xd3
	 add	@x[11],@x[11],@d[5],lsr#32
	zip1.64	$xd0,$xt0,$xt1
	 add.32	@x[12],@x[12],@d[6]
	zip2.64	$xd1,$xt0,$xt1
	 add	@x[13],@x[13],@d[6],lsr#32
	zip1.64	$xd2,$xt2,$xt3
	 add.32	@x[14],@x[14],@d[7]
	zip2.64	$xd3,$xt2,$xt3
	 add	@x[15],@x[15],@d[7],lsr#32

	b.lo	.Ltail_neon

	add	@x[0],@x[0],@x[1],lsl#32	// pack
	add	@x[2],@x[2],@x[3],lsl#32
	ldp	@x[1],@x[3],[$inp,#0]		// load input
	 add	$xa0,$xa0,@K[0]			// accumulate key block
	add	@x[4],@x[4],@x[5],lsl#32
	add	@x[6],@x[6],@x[7],lsl#32
	ldp	@x[5],@x[7],[$inp,#16]
	 add	$xb0,$xb0,@K[1]
	add	@x[8],@x[8],@x[9],lsl#32
	add	@x[10],@x[10],@x[11],lsl#32
	ldp	@x[9],@x[11],[$inp,#32]
	 add	$xc0,$xc0,@K[2]
	add	@x[12],@x[12],@x[13],lsl#32
	add	@x[14],@x[14],@x[15],lsl#32
	ldp	@x[13],@x[15],[$inp,#48]
	 add	$xd0,$xd0,@K[3]
	add	$inp,$inp,#64
#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	ld1.8	{$xt0-$xt3},[$inp],#64
	eor	@x[0],@x[0],@x[1]
	 add	$xa1,$xa1,@K[0]
	eor	@x[2],@x[2],@x[3]
	 add	$xb1,$xb1,@K[1]
	eor	@x[4],@x[4],@x[5]
	 add	$xc1,$xc1,@K[2]
	eor	@x[6],@x[6],@x[7]
	 add	$xd1,$xd1,@K[3]
	eor	@x[8],@x[8],@x[9]
	 eor	$xa0,$xa0,$xt0
	 movi	$xt0,#5
	eor	@x[10],@x[10],@x[11]
	 eor	$xb0,$xb0,$xt1
	eor	@x[12],@x[12],@x[13]
	 eor	$xc0,$xc0,$xt2
	eor	@x[14],@x[14],@x[15]
	 eor	$xd0,$xd0,$xt3
	 add	$CTR,$CTR,$xt0			// += 5
	 ld1.8	{$xt0-$xt3},[$inp],#64

	stp	@x[0],@x[2],[$out,#0]		// store output
	 add	@d[6],@d[6],#5			// increment counter
	stp	@x[4],@x[6],[$out,#16]
	stp	@x[8],@x[10],[$out,#32]
	stp	@x[12],@x[14],[$out,#48]
	add	$out,$out,#64

	st1.8	{$xa0-$xd0},[$out],#64
	 add	$xa2,$xa2,@K[0]
	 add	$xb2,$xb2,@K[1]
	 add	$xc2,$xc2,@K[2]
	 add	$xd2,$xd2,@K[3]
	ld1.8	{$xa0-$xd0},[$inp],#64

	eor	$xa1,$xa1,$xt0
	eor	$xb1,$xb1,$xt1
	eor	$xc1,$xc1,$xt2
	eor	$xd1,$xd1,$xt3
	st1.8	{$xa1-$xd1},[$out],#64
	 add	$xa3,$xa3,@K[0]
	 add	$xb3,$xb3,@K[1]
	 add	$xc3,$xc3,@K[2]
	 add	$xd3,$xd3,@K[3]
	ld1.8	{$xa1-$xd1},[$inp],#64

	eor	$xa2,$xa2,$xa0
	eor	$xb2,$xb2,$xb0
	eor	$xc2,$xc2,$xc0
	eor	$xd2,$xd2,$xd0
	st1.8	{$xa2-$xd2},[$out],#64

	eor	$xa3,$xa3,$xa1
	eor	$xb3,$xb3,$xb1
	eor	$xc3,$xc3,$xc1
	eor	$xd3,$xd3,$xd1
	st1.8	{$xa3-$xd3},[$out],#64

	b.hi	.Loop_outer_neon

	ldp	d8,d9,[sp]			// meet ABI requirements

	ldp	x19,x20,[x29,#16]
	add	sp,sp,#64
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret

.align	4
.Ltail_neon:
	add	$len,$len,#320
	ldp	d8,d9,[sp]			// meet ABI requirements
	cmp	$len,#64
	b.lo	.Less_than_64

	add	@x[0],@x[0],@x[1],lsl#32	// pack
	add	@x[2],@x[2],@x[3],lsl#32
	ldp	@x[1],@x[3],[$inp,#0]		// load input
	add	@x[4],@x[4],@x[5],lsl#32
	add	@x[6],@x[6],@x[7],lsl#32
	ldp	@x[5],@x[7],[$inp,#16]
	add	@x[8],@x[8],@x[9],lsl#32
	add	@x[10],@x[10],@x[11],lsl#32
	ldp	@x[9],@x[11],[$inp,#32]
	add	@x[12],@x[12],@x[13],lsl#32
	add	@x[14],@x[14],@x[15],lsl#32
	ldp	@x[13],@x[15],[$inp,#48]
	add	$inp,$inp,#64
#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	eor	@x[0],@x[0],@x[1]
	eor	@x[2],@x[2],@x[3]
	eor	@x[4],@x[4],@x[5]
	eor	@x[6],@x[6],@x[7]
	eor	@x[8],@x[8],@x[9]
	eor	@x[10],@x[10],@x[11]
	eor	@x[12],@x[12],@x[13]
	eor	@x[14],@x[14],@x[15]

	stp	@x[0],@x[2],[$out,#0]		// store output
	 add	$xa0,$xa0,@K[0]			// accumulate key block
	stp	@x[4],@x[6],[$out,#16]
	 add	$xb0,$xb0,@K[1]
	stp	@x[8],@x[10],[$out,#32]
	 add	$xc0,$xc0,@K[2]
	stp	@x[12],@x[14],[$out,#48]
	 add	$xd0,$xd0,@K[3]
	add	$out,$out,#64
	b.eq	.Ldone_neon
	sub	$len,$len,#64
	cmp	$len,#64
	b.lo	.Last_neon

	ld1.8	{$xt0-$xt3},[$inp],#64
	eor	$xa0,$xa0,$xt0
	eor	$xb0,$xb0,$xt1
	eor	$xc0,$xc0,$xt2
	eor	$xd0,$xd0,$xt3
	st1.8	{$xa0-$xd0},[$out],#64
	b.eq	.Ldone_neon

	add	$xa0,$xa1,@K[0]
	add	$xb0,$xb1,@K[1]
	sub	$len,$len,#64
	add	$xc0,$xc1,@K[2]
	cmp	$len,#64
	add	$xd0,$xd1,@K[3]
	b.lo	.Last_neon

	ld1.8	{$xt0-$xt3},[$inp],#64
	eor	$xa1,$xa0,$xt0
	eor	$xb1,$xb0,$xt1
	eor	$xc1,$xc0,$xt2
	eor	$xd1,$xd0,$xt3
	st1.8	{$xa1-$xd1},[$out],#64
	b.eq	.Ldone_neon

	add	$xa0,$xa2,@K[0]
	add	$xb0,$xb2,@K[1]
	sub	$len,$len,#64
	add	$xc0,$xc2,@K[2]
	cmp	$len,#64
	add	$xd0,$xd2,@K[3]
	b.lo	.Last_neon

	ld1.8	{$xt0-$xt3},[$inp],#64
	eor	$xa2,$xa0,$xt0
	eor	$xb2,$xb0,$xt1
	eor	$xc2,$xc0,$xt2
	eor	$xd2,$xd0,$xt3
	st1.8	{$xa2-$xd2},[$out],#64
	b.eq	.Ldone_neon

	add	$xa0,$xa3,@K[0]
	add	$xb0,$xb3,@K[1]
	add	$xc0,$xc3,@K[2]
	add	$xd0,$xd3,@K[3]
	sub	$len,$len,#64

.Last_neon:
	st1.8	{$xa0-$xd0},[sp]

	sub	$out,$out,#1
	add	$inp,$inp,$len
	add	$out,$out,$len
	add	$ctr,sp,$len
	neg	$len,$len

.Loop_tail_neon:
	ldrb	w10,[$inp,$len]
	ldrb	w11,[$ctr,$len]
	add	$len,$len,#1
	eor	w10,w10,w11
	strb	w10,[$out,$len]
	cbnz	$len,.Loop_tail_neon

	stp	xzr,xzr,[sp,#0]
	stp	xzr,xzr,[sp,#16]
	stp	xzr,xzr,[sp,#32]
	stp	xzr,xzr,[sp,#48]

.Ldone_neon:
	ldp	x19,x20,[x29,#16]
	add	sp,sp,#64
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	ChaCha20_neon,.-ChaCha20_neon
___
{
my @K = map("v$_.4s",(0..6));
my ($T0,$T1,$T2,$T3,$T4,$T5)=@K;
my ($A0,$B0,$C0,$D0,$A1,$B1,$C1,$D1,$A2,$B2,$C2,$D2,
    $A3,$B3,$C3,$D3,$A4,$B4,$C4,$D4,$A5,$B5,$C5,$D5) = map("v$_.4s",(8..31));
my $rot24 = @K[6];
my $ONE = "v7.4s";

sub NEONROUND {
my $odd = pop;
my ($a,$b,$c,$d,$t)=@_;

	(
	"&add		('$a','$a','$b')",
	"&eor		('$d','$d','$a')",
	"&rev32_16	('$d','$d')",		# vrot ($d,16)

	"&add		('$c','$c','$d')",
	"&eor		('$t','$b','$c')",
	"&ushr		('$b','$t',20)",
	"&sli		('$b','$t',12)",

	"&add		('$a','$a','$b')",
	"&eor		('$d','$d','$a')",
	"&tbl		('$d','{$d}','$rot24')",

	"&add		('$c','$c','$d')",
	"&eor		('$t','$b','$c')",
	"&ushr		('$b','$t',25)",
	"&sli		('$b','$t',7)",

	"&ext		('$c','$c','$c',8)",
	"&ext		('$d','$d','$d',$odd?4:12)",
	"&ext		('$b','$b','$b',$odd?12:4)"
	);
}

$code.=<<___;
.type	ChaCha20_512_neon,%function
.align	5
ChaCha20_512_neon:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-96]!
	add	x29,sp,#0

	adrp	@x[0],.Lsigma
	add	@x[0],@x[0],:lo12:.Lsigma
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]
	stp	x25,x26,[sp,#64]
	stp	x27,x28,[sp,#80]

.L512_or_more_neon:
	sub	sp,sp,#128+64

	eor	$ONE,$ONE,$ONE
	ldp	@d[0],@d[1],[@x[0]]		// load sigma
	ld1	{@K[0]},[@x[0]],#16
	ldp	@d[2],@d[3],[$key]		// load key
	ldp	@d[4],@d[5],[$key,#16]
	ld1	{@K[1],@K[2]},[$key]
	ldp	@d[6],@d[7],[$ctr]		// load counter
	ld1	{@K[3]},[$ctr]
	ld1	{$ONE}[0],[@x[0]]
	add	$key,@x[0],#16			// .Lrot24
#ifdef	__AARCH64EB__
	rev64	@K[0],@K[0]
	ror	@d[2],@d[2],#32
	ror	@d[3],@d[3],#32
	ror	@d[4],@d[4],#32
	ror	@d[5],@d[5],#32
	ror	@d[6],@d[6],#32
	ror	@d[7],@d[7],#32
#endif
	add	@K[3],@K[3],$ONE		// += 1
	stp	@K[0],@K[1],[sp,#0]		// off-load key block, invariant part
	add	@K[3],@K[3],$ONE		// not typo
	str	@K[2],[sp,#32]
	add	@K[4],@K[3],$ONE
	add	@K[5],@K[4],$ONE
	add	@K[6],@K[5],$ONE
	shl	$ONE,$ONE,#2			// 1 -> 4

	stp	d8,d9,[sp,#128+0]		// meet ABI requirements
	stp	d10,d11,[sp,#128+16]
	stp	d12,d13,[sp,#128+32]
	stp	d14,d15,[sp,#128+48]

	sub	$len,$len,#512			// not typo

.Loop_outer_512_neon:
	 mov	$A0,@K[0]
	 mov	$A1,@K[0]
	 mov	$A2,@K[0]
	 mov	$A3,@K[0]
	 mov	$A4,@K[0]
	 mov	$A5,@K[0]
	 mov	$B0,@K[1]
	mov.32	@x[0],@d[0]			// unpack key block
	 mov	$B1,@K[1]
	lsr	@x[1],@d[0],#32
	 mov	$B2,@K[1]
	mov.32	@x[2],@d[1]
	 mov	$B3,@K[1]
	lsr	@x[3],@d[1],#32
	 mov	$B4,@K[1]
	mov.32	@x[4],@d[2]
	 mov	$B5,@K[1]
	lsr	@x[5],@d[2],#32
	 mov	$D0,@K[3]
	mov.32	@x[6],@d[3]
	 mov	$D1,@K[4]
	lsr	@x[7],@d[3],#32
	 mov	$D2,@K[5]
	mov.32	@x[8],@d[4]
	 mov	$D3,@K[6]
	lsr	@x[9],@d[4],#32
	 mov	$C0,@K[2]
	mov.32	@x[10],@d[5]
	 mov	$C1,@K[2]
	lsr	@x[11],@d[5],#32
	 add	$D4,$D0,$ONE			// +4
	mov.32	@x[12],@d[6]
	 add	$D5,$D1,$ONE			// +4
	lsr	@x[13],@d[6],#32
	 mov	$C2,@K[2]
	mov.32	@x[14],@d[7]
	 mov	$C3,@K[2]
	lsr	@x[15],@d[7],#32
	 mov	$C4,@K[2]
	 stp	@K[3],@K[4],[sp,#48]		// off-load key block, variable part
	 mov	$C5,@K[2]
	 stp	@K[5],@K[6],[sp,#80]

	mov	$ctr,#5
	ld1	{$rot24},[$key]
	subs	$len,$len,#512
.Loop_upper_neon:
	sub	$ctr,$ctr,#1
___
	my @thread0=&NEONROUND($A0,$B0,$C0,$D0,$T0,0);
	my @thread1=&NEONROUND($A1,$B1,$C1,$D1,$T1,0);
	my @thread2=&NEONROUND($A2,$B2,$C2,$D2,$T2,0);
	my @thread3=&NEONROUND($A3,$B3,$C3,$D3,$T3,0);
	my @thread4=&NEONROUND($A4,$B4,$C4,$D4,$T4,0);
	my @thread5=&NEONROUND($A5,$B5,$C5,$D5,$T5,0);
	my @thread67=(&ROUND(0,4,8,12),&ROUND(0,5,10,15));
	my $diff = ($#thread0+1)*6 - $#thread67 - 1;
	my $i = 0;

	foreach (@thread0) {
		eval;			eval(shift(@thread67));
		eval(shift(@thread1));	eval(shift(@thread67));
		eval(shift(@thread2));	eval(shift(@thread67));
		eval(shift(@thread3));	eval(shift(@thread67));
		eval(shift(@thread4));	eval(shift(@thread67));
		eval(shift(@thread5));	eval(shift(@thread67));
	}

	@thread0=&NEONROUND($A0,$B0,$C0,$D0,$T0,1);
	@thread1=&NEONROUND($A1,$B1,$C1,$D1,$T1,1);
	@thread2=&NEONROUND($A2,$B2,$C2,$D2,$T2,1);
	@thread3=&NEONROUND($A3,$B3,$C3,$D3,$T3,1);
	@thread4=&NEONROUND($A4,$B4,$C4,$D4,$T4,1);
	@thread5=&NEONROUND($A5,$B5,$C5,$D5,$T5,1);
	@thread67=(&ROUND(0,4,8,12),&ROUND(0,5,10,15));

	foreach (@thread0) {
		eval;			eval(shift(@thread67));
		eval(shift(@thread1));	eval(shift(@thread67));
		eval(shift(@thread2));	eval(shift(@thread67));
		eval(shift(@thread3));	eval(shift(@thread67));
		eval(shift(@thread4));	eval(shift(@thread67));
		eval(shift(@thread5));	eval(shift(@thread67));
	}
$code.=<<___;
	cbnz	$ctr,.Loop_upper_neon

	add.32	@x[0],@x[0],@d[0]		// accumulate key block
	add	@x[1],@x[1],@d[0],lsr#32
	add.32	@x[2],@x[2],@d[1]
	add	@x[3],@x[3],@d[1],lsr#32
	add.32	@x[4],@x[4],@d[2]
	add	@x[5],@x[5],@d[2],lsr#32
	add.32	@x[6],@x[6],@d[3]
	add	@x[7],@x[7],@d[3],lsr#32
	add.32	@x[8],@x[8],@d[4]
	add	@x[9],@x[9],@d[4],lsr#32
	add.32	@x[10],@x[10],@d[5]
	add	@x[11],@x[11],@d[5],lsr#32
	add.32	@x[12],@x[12],@d[6]
	add	@x[13],@x[13],@d[6],lsr#32
	add.32	@x[14],@x[14],@d[7]
	add	@x[15],@x[15],@d[7],lsr#32

	add	@x[0],@x[0],@x[1],lsl#32	// pack
	add	@x[2],@x[2],@x[3],lsl#32
	ldp	@x[1],@x[3],[$inp,#0]		// load input
	add	@x[4],@x[4],@x[5],lsl#32
	add	@x[6],@x[6],@x[7],lsl#32
	ldp	@x[5],@x[7],[$inp,#16]
	add	@x[8],@x[8],@x[9],lsl#32
	add	@x[10],@x[10],@x[11],lsl#32
	ldp	@x[9],@x[11],[$inp,#32]
	add	@x[12],@x[12],@x[13],lsl#32
	add	@x[14],@x[14],@x[15],lsl#32
	ldp	@x[13],@x[15],[$inp,#48]
	add	$inp,$inp,#64
#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	eor	@x[0],@x[0],@x[1]
	eor	@x[2],@x[2],@x[3]
	eor	@x[4],@x[4],@x[5]
	eor	@x[6],@x[6],@x[7]
	eor	@x[8],@x[8],@x[9]
	eor	@x[10],@x[10],@x[11]
	eor	@x[12],@x[12],@x[13]
	eor	@x[14],@x[14],@x[15]

	 stp	@x[0],@x[2],[$out,#0]		// store output
	 add	@d[6],@d[6],#1			// increment counter
	mov.32	@x[0],@d[0]			// unpack key block
	lsr	@x[1],@d[0],#32
	 stp	@x[4],@x[6],[$out,#16]
	mov.32	@x[2],@d[1]
	lsr	@x[3],@d[1],#32
	 stp	@x[8],@x[10],[$out,#32]
	mov.32	@x[4],@d[2]
	lsr	@x[5],@d[2],#32
	 stp	@x[12],@x[14],[$out,#48]
	 add	$out,$out,#64
	mov.32	@x[6],@d[3]
	lsr	@x[7],@d[3],#32
	mov.32	@x[8],@d[4]
	lsr	@x[9],@d[4],#32
	mov.32	@x[10],@d[5]
	lsr	@x[11],@d[5],#32
	mov.32	@x[12],@d[6]
	lsr	@x[13],@d[6],#32
	mov.32	@x[14],@d[7]
	lsr	@x[15],@d[7],#32

	mov	$ctr,#5
.Loop_lower_neon:
	sub	$ctr,$ctr,#1
___
	@thread0=&NEONROUND($A0,$B0,$C0,$D0,$T0,0);
	@thread1=&NEONROUND($A1,$B1,$C1,$D1,$T1,0);
	@thread2=&NEONROUND($A2,$B2,$C2,$D2,$T2,0);
	@thread3=&NEONROUND($A3,$B3,$C3,$D3,$T3,0);
	@thread4=&NEONROUND($A4,$B4,$C4,$D4,$T4,0);
	@thread5=&NEONROUND($A5,$B5,$C5,$D5,$T5,0);
	@thread67=(&ROUND(0,4,8,12),&ROUND(0,5,10,15));

	foreach (@thread0) {
		eval;			eval(shift(@thread67));
		eval(shift(@thread1));	eval(shift(@thread67));
		eval(shift(@thread2));	eval(shift(@thread67));
		eval(shift(@thread3));	eval(shift(@thread67));
		eval(shift(@thread4));	eval(shift(@thread67));
		eval(shift(@thread5));	eval(shift(@thread67));
	}

	@thread0=&NEONROUND($A0,$B0,$C0,$D0,$T0,1);
	@thread1=&NEONROUND($A1,$B1,$C1,$D1,$T1,1);
	@thread2=&NEONROUND($A2,$B2,$C2,$D2,$T2,1);
	@thread3=&NEONROUND($A3,$B3,$C3,$D3,$T3,1);
	@thread4=&NEONROUND($A4,$B4,$C4,$D4,$T4,1);
	@thread5=&NEONROUND($A5,$B5,$C5,$D5,$T5,1);
	@thread67=(&ROUND(0,4,8,12),&ROUND(0,5,10,15));

	foreach (@thread0) {
		eval;			eval(shift(@thread67));
		eval(shift(@thread1));	eval(shift(@thread67));
		eval(shift(@thread2));	eval(shift(@thread67));
		eval(shift(@thread3));	eval(shift(@thread67));
		eval(shift(@thread4));	eval(shift(@thread67));
		eval(shift(@thread5));	eval(shift(@thread67));
	}
$code.=<<___;
	cbnz	$ctr,.Loop_lower_neon

	add.32	@x[0],@x[0],@d[0]		// accumulate key block
	 ldp	@K[0],@K[1],[sp,#0]
	add	@x[1],@x[1],@d[0],lsr#32
	 ldp	@K[2],@K[3],[sp,#32]
	add.32	@x[2],@x[2],@d[1]
	 ldp	@K[4],@K[5],[sp,#64]
	add	@x[3],@x[3],@d[1],lsr#32
	 ldr	@K[6],[sp,#96]
	 add	$A0,$A0,@K[0]
	add.32	@x[4],@x[4],@d[2]
	 add	$A1,$A1,@K[0]
	add	@x[5],@x[5],@d[2],lsr#32
	 add	$A2,$A2,@K[0]
	add.32	@x[6],@x[6],@d[3]
	 add	$A3,$A3,@K[0]
	add	@x[7],@x[7],@d[3],lsr#32
	 add	$A4,$A4,@K[0]
	add.32	@x[8],@x[8],@d[4]
	 add	$A5,$A5,@K[0]
	add	@x[9],@x[9],@d[4],lsr#32
	 add	$C0,$C0,@K[2]
	add.32	@x[10],@x[10],@d[5]
	 add	$C1,$C1,@K[2]
	add	@x[11],@x[11],@d[5],lsr#32
	 add	$C2,$C2,@K[2]
	add.32	@x[12],@x[12],@d[6]
	 add	$C3,$C3,@K[2]
	add	@x[13],@x[13],@d[6],lsr#32
	 add	$C4,$C4,@K[2]
	add.32	@x[14],@x[14],@d[7]
	 add	$C5,$C5,@K[2]
	add	@x[15],@x[15],@d[7],lsr#32
	 add	$D4,$D4,$ONE			// +4
	add	@x[0],@x[0],@x[1],lsl#32	// pack
	 add	$D5,$D5,$ONE			// +4
	add	@x[2],@x[2],@x[3],lsl#32
	 add	$D0,$D0,@K[3]
	ldp	@x[1],@x[3],[$inp,#0]		// load input
	 add	$D1,$D1,@K[4]
	add	@x[4],@x[4],@x[5],lsl#32
	 add	$D2,$D2,@K[5]
	add	@x[6],@x[6],@x[7],lsl#32
	 add	$D3,$D3,@K[6]
	ldp	@x[5],@x[7],[$inp,#16]
	 add	$D4,$D4,@K[3]
	add	@x[8],@x[8],@x[9],lsl#32
	 add	$D5,$D5,@K[4]
	add	@x[10],@x[10],@x[11],lsl#32
	 add	$B0,$B0,@K[1]
	ldp	@x[9],@x[11],[$inp,#32]
	 add	$B1,$B1,@K[1]
	add	@x[12],@x[12],@x[13],lsl#32
	 add	$B2,$B2,@K[1]
	add	@x[14],@x[14],@x[15],lsl#32
	 add	$B3,$B3,@K[1]
	ldp	@x[13],@x[15],[$inp,#48]
	 add	$B4,$B4,@K[1]
	add	$inp,$inp,#64
	 add	$B5,$B5,@K[1]

#ifdef	__AARCH64EB__
	rev	@x[0],@x[0]
	rev	@x[2],@x[2]
	rev	@x[4],@x[4]
	rev	@x[6],@x[6]
	rev	@x[8],@x[8]
	rev	@x[10],@x[10]
	rev	@x[12],@x[12]
	rev	@x[14],@x[14]
#endif
	ld1.8	{$T0-$T3},[$inp],#64
	eor	@x[0],@x[0],@x[1]
	eor	@x[2],@x[2],@x[3]
	eor	@x[4],@x[4],@x[5]
	eor	@x[6],@x[6],@x[7]
	eor	@x[8],@x[8],@x[9]
	 eor	$A0,$A0,$T0
	eor	@x[10],@x[10],@x[11]
	 eor	$B0,$B0,$T1
	eor	@x[12],@x[12],@x[13]
	 eor	$C0,$C0,$T2
	eor	@x[14],@x[14],@x[15]
	 eor	$D0,$D0,$T3
	 ld1.8	{$T0-$T3},[$inp],#64

	stp	@x[0],@x[2],[$out,#0]		// store output
	 add	@d[6],@d[6],#7			// increment counter
	stp	@x[4],@x[6],[$out,#16]
	stp	@x[8],@x[10],[$out,#32]
	stp	@x[12],@x[14],[$out,#48]
	add	$out,$out,#64
	st1.8	{$A0-$D0},[$out],#64

	ld1.8	{$A0-$D0},[$inp],#64
	eor	$A1,$A1,$T0
	eor	$B1,$B1,$T1
	eor	$C1,$C1,$T2
	eor	$D1,$D1,$T3
	st1.8	{$A1-$D1},[$out],#64

	ld1.8	{$A1-$D1},[$inp],#64
	eor	$A2,$A2,$A0
	 ldp	@K[0],@K[1],[sp,#0]
	eor	$B2,$B2,$B0
	 ldp	@K[2],@K[3],[sp,#32]
	eor	$C2,$C2,$C0
	eor	$D2,$D2,$D0
	st1.8	{$A2-$D2},[$out],#64

	ld1.8	{$A2-$D2},[$inp],#64
	eor	$A3,$A3,$A1
	eor	$B3,$B3,$B1
	eor	$C3,$C3,$C1
	eor	$D3,$D3,$D1
	st1.8	{$A3-$D3},[$out],#64

	ld1.8	{$A3-$D3},[$inp],#64
	eor	$A4,$A4,$A2
	eor	$B4,$B4,$B2
	eor	$C4,$C4,$C2
	eor	$D4,$D4,$D2
	st1.8	{$A4-$D4},[$out],#64

	shl	$A0,$ONE,#1			// 4 -> 8
	eor	$A5,$A5,$A3
	eor	$B5,$B5,$B3
	eor	$C5,$C5,$C3
	eor	$D5,$D5,$D3
	st1.8	{$A5-$D5},[$out],#64

	add	@K[3],@K[3],$A0			// += 8
	add	@K[4],@K[4],$A0
	add	@K[5],@K[5],$A0
	add	@K[6],@K[6],$A0

	b.hs	.Loop_outer_512_neon

	adds	$len,$len,#512
	ushr	$ONE,$ONE,#1			// 4 -> 2

	ldp	d10,d11,[sp,#128+16]		// meet ABI requirements
	ldp	d12,d13,[sp,#128+32]
	ldp	d14,d15,[sp,#128+48]

	stp	@K[0],@K[0],[sp,#0]		// wipe off-load area
	stp	@K[0],@K[0],[sp,#32]
	stp	@K[0],@K[0],[sp,#64]

	b.eq	.Ldone_512_neon

	sub	$key,$key,#16			// .Lone
	cmp	$len,#192
	add	sp,sp,#128
	sub	@K[3],@K[3],$ONE		// -= 2
	ld1	{$CTR,$ROT24},[$key]
	b.hs	.Loop_outer_neon

	ldp	d8,d9,[sp,#0]			// meet ABI requirements
	eor	@K[1],@K[1],@K[1]
	eor	@K[2],@K[2],@K[2]
	eor	@K[3],@K[3],@K[3]
	eor	@K[4],@K[4],@K[4]
	eor	@K[5],@K[5],@K[5]
	eor	@K[6],@K[6],@K[6]
	b	.Loop_outer

.Ldone_512_neon:
	ldp	d8,d9,[sp,#128+0]		// meet ABI requirements
	ldp	x19,x20,[x29,#16]
	add	sp,sp,#128+64
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	ChaCha20_512_neon,.-ChaCha20_512_neon
___
}
}}}

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/geo;

	(s/\b([a-z]+)\.32\b/$1/ and (s/x([0-9]+)/w$1/g or 1))	or
	(m/\b(eor|ext|mov|tbl)\b/ and (s/\.4s/\.16b/g or 1))	or
	(s/\b((?:ld|st)1)\.8\b/$1/ and (s/\.4s/\.16b/g or 1))	or
	(m/\b(ld|st)[rp]\b/ and (s/v([0-9]+)\.4s/q$1/g or 1))	or
	(m/\b(dup|ld1)\b/ and (s/\.4(s}?\[[0-3]\])/.$1/g or 1))	or
	(s/\b(zip[12])\.64\b/$1/ and (s/\.4s/\.2d/g or 1))	or
	(s/\brev32\.16\b/rev32/ and (s/\.4s/\.8h/g or 1));

	#s/\bq([0-9]+)#(lo|hi)/sprintf "d%d",2*$1+($2 eq "hi")/geo;

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT: $!";	# flush
