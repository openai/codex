#! /usr/bin/env perl
# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# ====================================================================
# Written by Andy Polyakov, @dot-asm, initially for use in the OpenSSL
# project. The module is dual licensed under OpenSSL and CRYPTOGAMS
# licenses depending on where you obtain it. For further details see
# https://github.com/dot-asm/cryptogams/.
# ====================================================================
#
# This module implements Poly1305 hash for PowerPC.
#
# June 2015
#
# Numbers are cycles per processed byte with poly1305_blocks alone,
# and improvement coefficients relative to gcc-generated code.
#
#			-m32		-m64
#
# Freescale e300	14.8/+80%	-
# PPC74x0		7.60/+60%	-
# PPC970		7.00/+114%	3.51/+205%
# POWER7		3.75/+260%	1.93/+100%
# POWER8		-		2.03/+200%
# POWER9		-		2.00/+150%
#
# Do we need floating-point implementation for PPC? Results presented
# in poly1305_ieee754.c are tricky to compare to, because they are for
# compiler-generated code. On the other hand it's known that floating-
# point performance can be dominated by FPU latency, which means that
# there is limit even for ideally optimized (and even vectorized) code.
# And this limit is estimated to be higher than above -m64 results. Or
# in other words floating-point implementation can be meaningful to
# consider only in 32-bit application context. We probably have to
# recognize that 32-bit builds are getting less popular on high-end
# systems and therefore tend to target embedded ones, which might not
# even have FPU...
#
# On side note, Power ISA 2.07 enables vector base 2^26 implementation,
# and POWER8 might have capacity to break 1.0 cycle per byte barrier...
#
# January 2019
#
# ... Unfortunately not:-( Estimate was a projection of ARM result,
# but ARM has vector multiply-n-add instruction, while PowerISA does
# not, not one usable in the context. Improvement is ~40% over -m64
# result above and is ~1.43 on little-endian systems.

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

if ($flavour =~ /64/) {
	$SIZE_T	=8;
	$LRSAVE	=2*$SIZE_T;
	$UCMP	="cmpld";
	$STU	="stdu";
	$POP	="ld";
	$PUSH	="std";
} elsif ($flavour =~ /32/) {
	$SIZE_T	=4;
	$LRSAVE	=$SIZE_T;
	$UCMP	="cmplw";
	$STU	="stwu";
	$POP	="lwz";
	$PUSH	="stw";
} else { die "nonsense $flavour"; }

# Define endianness based on flavour
# i.e.: linux64le
$LITTLE_ENDIAN = ($flavour=~/le$/) ? $SIZE_T : 0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}ppc-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/ppc-xlate.pl" and -f $xlate) or
die "can't locate ppc-xlate.pl";

open STDOUT,"| $^X $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";

$FRAME=24*$SIZE_T;

$sp="r1";
my ($ctx,$inp,$len,$padbit) = map("r$_",(3..6));
my ($mac,$nonce)=($inp,$len);
my $mask = "r0";

$code=<<___;
.machine	"any"
.text
___
							if ($flavour =~ /64/) {
###############################################################################
# base 2^64 implementation

my ($h0,$h1,$h2,$d0,$d1,$d2, $r0,$r1,$s1, $t0,$t1) = map("r$_",(7..12,27..31));

$code.=<<___;
.globl	.poly1305_init_int
.align	4
.poly1305_init_int:
	xor	r0,r0,r0
	std	r0,0($ctx)		# zero hash value
	std	r0,8($ctx)
	std	r0,16($ctx)
	stw	r0,24($ctx)		# clear is_base2_26

	$UCMP	$inp,r0
	beq-	Lno_key
___
$code.=<<___	if ($LITTLE_ENDIAN);
	ld	$d0,0($inp)		# load key material
	ld	$d1,8($inp)
___
$code.=<<___	if (!$LITTLE_ENDIAN);
	li	$h0,4
	lwbrx	$d0,0,$inp		# load key material
	li	$d1,8
	lwbrx	$h0,$h0,$inp
	li	$h1,12
	lwbrx	$d1,$d1,$inp
	lwbrx	$h1,$h1,$inp
	insrdi	$d0,$h0,32,0
	insrdi	$d1,$h1,32,0
___
$code.=<<___;
	lis	$h1,0xfff		# 0x0fff0000
	ori	$h1,$h1,0xfffc		# 0x0ffffffc
	insrdi	$h1,$h1,32,0		# 0x0ffffffc0ffffffc
	ori	$h0,$h1,3		# 0x0ffffffc0fffffff

	and	$d0,$d0,$h0
	and	$d1,$d1,$h1

	std	$d0,32($ctx)		# store key
	std	$d1,40($ctx)

Lno_key:
	xor	r3,r3,r3
	blr
	.long	0
	.byte	0,12,0x14,0,0,0,2,0
.size	.poly1305_init_int,.-.poly1305_init_int

.globl	.poly1305_blocks
.align	4
.poly1305_blocks:
Lpoly1305_blocks:
	srdi.	$len,$len,4
	beq-	Labort

	$STU	$sp,-$FRAME($sp)
	mflr	r0
	$PUSH	r27,`$FRAME-$SIZE_T*5`($sp)
	$PUSH	r28,`$FRAME-$SIZE_T*4`($sp)
	$PUSH	r29,`$FRAME-$SIZE_T*3`($sp)
	$PUSH	r30,`$FRAME-$SIZE_T*2`($sp)
	$PUSH	r31,`$FRAME-$SIZE_T*1`($sp)
	$PUSH	r0,`$FRAME+$LRSAVE`($sp)

	ld	$r0,32($ctx)		# load key
	ld	$r1,40($ctx)

	ld	$h0,0($ctx)		# load hash value
	ld	$h1,8($ctx)
	ld	$h2,16($ctx)

	srdi	$s1,$r1,2
	mtctr	$len
	add	$s1,$s1,$r1		# s1 = r1 + r1>>2
	li	$mask,3
	b	Loop

.align	4
Loop:
___
$code.=<<___	if ($LITTLE_ENDIAN);
	ld	$t0,0($inp)		# load input
	ld	$t1,8($inp)
___
$code.=<<___	if (!$LITTLE_ENDIAN);
	li	$d0,4
	lwbrx	$t0,0,$inp		# load input
	li	$t1,8
	lwbrx	$d0,$d0,$inp
	li	$d1,12
	lwbrx	$t1,$t1,$inp
	lwbrx	$d1,$d1,$inp
	insrdi	$t0,$d0,32,0
	insrdi	$t1,$d1,32,0
___
$code.=<<___;
	addi	$inp,$inp,16

	addc	$h0,$h0,$t0		# accumulate input
	adde	$h1,$h1,$t1

	mulld	$d0,$h0,$r0		# h0*r0
	mulhdu	$d1,$h0,$r0
	adde	$h2,$h2,$padbit

	mulld	$t0,$h1,$s1		# h1*5*r1
	mulhdu	$t1,$h1,$s1
	addc	$d0,$d0,$t0
	adde	$d1,$d1,$t1

	mulld	$t0,$h0,$r1		# h0*r1
	mulhdu	$d2,$h0,$r1
	addc	$d1,$d1,$t0
	addze	$d2,$d2

	mulld	$t0,$h1,$r0		# h1*r0
	mulhdu	$t1,$h1,$r0
	addc	$d1,$d1,$t0
	adde	$d2,$d2,$t1

	mulld	$t0,$h2,$s1		# h2*5*r1
	mulld	$t1,$h2,$r0		# h2*r0
	addc	$d1,$d1,$t0
	adde	$d2,$d2,$t1

	andc	$t0,$d2,$mask		# final reduction step
	and	$h2,$d2,$mask
	srdi	$t1,$t0,2
	add	$t0,$t0,$t1
	addc	$h0,$d0,$t0
	addze	$h1,$d1
	addze	$h2,$h2

	bdnz	Loop

	std	$h0,0($ctx)		# store hash value
	std	$h1,8($ctx)
	std	$h2,16($ctx)

	$POP	r27,`$FRAME-$SIZE_T*5`($sp)
	$POP	r28,`$FRAME-$SIZE_T*4`($sp)
	$POP	r29,`$FRAME-$SIZE_T*3`($sp)
	$POP	r30,`$FRAME-$SIZE_T*2`($sp)
	$POP	r31,`$FRAME-$SIZE_T*1`($sp)
	addi	$sp,$sp,$FRAME
Labort:
	blr
	.long	0
	.byte	0,12,4,1,0x80,5,4,0
.size	.poly1305_blocks,.-.poly1305_blocks
___
{
my ($h0,$h1,$h2,$h3,$h4,$t0) = map("r$_",(7..12));

$code.=<<___;
.globl	.poly1305_emit
.align	5
.poly1305_emit:
	lwz	$h0,0($ctx)	# load hash value base 2^26
	lwz	$h1,4($ctx)
	lwz	$h2,8($ctx)
	lwz	$h3,12($ctx)
	lwz	$h4,16($ctx)
	lwz	r0,24($ctx)	# is_base2_26

	sldi	$h1,$h1,26	# base 2^26 -> base 2^64
	sldi	$t0,$h2,52
	srdi	$h2,$h2,12
	sldi	$h3,$h3,14
	add	$h0,$h0,$h1
	addc	$h0,$h0,$t0
	sldi	$t0,$h4,40
	srdi	$h4,$h4,24
	adde	$h1,$h2,$h3
	addc	$h1,$h1,$t0
	addze	$h2,$h4

	ld	$h3,0($ctx)	# load hash value base 2^64
	ld	$h4,8($ctx)
	ld	$t0,16($ctx)

	neg	r0,r0
	xor	$h0,$h0,$h3	# choose between radixes
	xor	$h1,$h1,$h4
	xor	$h2,$h2,$t0
	and	$h0,$h0,r0
	and	$h1,$h1,r0
	and	$h2,$h2,r0
	xor	$h0,$h0,$h3
	xor	$h1,$h1,$h4
	xor	$h2,$h2,$t0

	addic	$h3,$h0,5	# compare to modulus
	addze	$h4,$h1
	addze	$t0,$h2

	srdi	$t0,$t0,2	# see if it carried/borrowed
	neg	$t0,$t0

	andc	$h0,$h0,$t0
	and	$h3,$h3,$t0
	andc	$h1,$h1,$t0
	and	$h4,$h4,$t0
	or	$h0,$h0,$h3
	or	$h1,$h1,$h4

	lwz	$t0,4($nonce)
	lwz	$h2,12($nonce)
	lwz	$h3,0($nonce)
	lwz	$h4,8($nonce)

	insrdi	$h3,$t0,32,0
	insrdi	$h4,$h2,32,0

	addc	$h0,$h0,$h3	# accumulate nonce
	adde	$h1,$h1,$h4

	addi	$ctx,$mac,-1
	addi	$mac,$mac,7

	stbu	$h0,1($ctx)	# write [little-endian] result
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	srdi	$h0,$h0,8
	stbu	$h1,1($mac)
	srdi	$h1,$h1,8

	stbu	$h0,1($ctx)
	stbu	$h1,1($mac)

	blr
	.long	0
	.byte	0,12,0x14,0,0,0,3,0
.size	.poly1305_emit,.-.poly1305_emit
___
}							} else {
###############################################################################
# base 2^32 implementation

my ($h0,$h1,$h2,$h3,$h4, $r0,$r1,$r2,$r3, $s1,$s2,$s3,
    $t0,$t1,$t2,$t3, $D0,$D1,$D2,$D3, $d0,$d1,$d2,$d3
   ) = map("r$_",(7..12,14..31));

$code.=<<___;
.globl	.poly1305_init_int
.align	4
.poly1305_init_int:
	xor	r0,r0,r0
	stw	r0,0($ctx)		# zero hash value
	stw	r0,4($ctx)
	stw	r0,8($ctx)
	stw	r0,12($ctx)
	stw	r0,16($ctx)
	stw	r0,24($ctx)		# clear is_base2_26

	$UCMP	$inp,r0
	beq-	Lno_key
___
$code.=<<___	if ($LITTLE_ENDIAN);
	lw	$h0,0($inp)		# load key material
	lw	$h1,4($inp)
	lw	$h2,8($inp)
	lw	$h3,12($inp)
___
$code.=<<___	if (!$LITTLE_ENDIAN);
	li	$h1,4
	lwbrx	$h0,0,$inp		# load key material
	li	$h2,8
	lwbrx	$h1,$h1,$inp
	li	$h3,12
	lwbrx	$h2,$h2,$inp
	lwbrx	$h3,$h3,$inp
___
$code.=<<___;
	lis	$mask,0xf000		# 0xf0000000
	li	$r0,-4
	andc	$r0,$r0,$mask		# 0x0ffffffc

	andc	$h0,$h0,$mask
	and	$h1,$h1,$r0
	and	$h2,$h2,$r0
	and	$h3,$h3,$r0

	stw	$h0,32($ctx)		# store key
	stw	$h1,36($ctx)
	stw	$h2,40($ctx)
	stw	$h3,44($ctx)

Lno_key:
	xor	r3,r3,r3
	blr
	.long	0
	.byte	0,12,0x14,0,0,0,2,0
.size	.poly1305_init_int,.-.poly1305_init_int

.globl	.poly1305_blocks
.align	4
.poly1305_blocks:
Lpoly1305_blocks:
	srwi.	$len,$len,4
	beq-	Labort

	$STU	$sp,-$FRAME($sp)
	mflr	r0
	$PUSH	r14,`$FRAME-$SIZE_T*18`($sp)
	$PUSH	r15,`$FRAME-$SIZE_T*17`($sp)
	$PUSH	r16,`$FRAME-$SIZE_T*16`($sp)
	$PUSH	r17,`$FRAME-$SIZE_T*15`($sp)
	$PUSH	r18,`$FRAME-$SIZE_T*14`($sp)
	$PUSH	r19,`$FRAME-$SIZE_T*13`($sp)
	$PUSH	r20,`$FRAME-$SIZE_T*12`($sp)
	$PUSH	r21,`$FRAME-$SIZE_T*11`($sp)
	$PUSH	r22,`$FRAME-$SIZE_T*10`($sp)
	$PUSH	r23,`$FRAME-$SIZE_T*9`($sp)
	$PUSH	r24,`$FRAME-$SIZE_T*8`($sp)
	$PUSH	r25,`$FRAME-$SIZE_T*7`($sp)
	$PUSH	r26,`$FRAME-$SIZE_T*6`($sp)
	$PUSH	r27,`$FRAME-$SIZE_T*5`($sp)
	$PUSH	r28,`$FRAME-$SIZE_T*4`($sp)
	$PUSH	r29,`$FRAME-$SIZE_T*3`($sp)
	$PUSH	r30,`$FRAME-$SIZE_T*2`($sp)
	$PUSH	r31,`$FRAME-$SIZE_T*1`($sp)
	$PUSH	r0,`$FRAME+$LRSAVE`($sp)

	lwz	$r0,32($ctx)		# load key
	lwz	$r1,36($ctx)
	lwz	$r2,40($ctx)
	lwz	$r3,44($ctx)

	lwz	$h0,0($ctx)		# load hash value
	lwz	$h1,4($ctx)
	lwz	$h2,8($ctx)
	lwz	$h3,12($ctx)
	lwz	$h4,16($ctx)

	srwi	$s1,$r1,2
	srwi	$s2,$r2,2
	srwi	$s3,$r3,2
	add	$s1,$s1,$r1		# si = ri + ri>>2
	add	$s2,$s2,$r2
	add	$s3,$s3,$r3
	mtctr	$len
	li	$mask,3
	b	Loop

.align	4
Loop:
___
$code.=<<___	if ($LITTLE_ENDIAN);
	lwz	$d0,0($inp)		# load input
	lwz	$d1,4($inp)
	lwz	$d2,8($inp)
	lwz	$d3,12($inp)
___
$code.=<<___	if (!$LITTLE_ENDIAN);
	li	$d1,4
	lwbrx	$d0,0,$inp		# load input
	li	$d2,8
	lwbrx	$d1,$d1,$inp
	li	$d3,12
	lwbrx	$d2,$d2,$inp
	lwbrx	$d3,$d3,$inp
___
$code.=<<___;
	addi	$inp,$inp,16

	addc	$h0,$h0,$d0		# accumulate input
	adde	$h1,$h1,$d1
	adde	$h2,$h2,$d2

	mullw	$d0,$h0,$r0		# h0*r0
	mulhwu	$D0,$h0,$r0

	mullw	$d1,$h0,$r1		# h0*r1
	mulhwu	$D1,$h0,$r1

	mullw	$d2,$h0,$r2		# h0*r2
	mulhwu	$D2,$h0,$r2

	 adde	$h3,$h3,$d3
	 adde	$h4,$h4,$padbit

	mullw	$d3,$h0,$r3		# h0*r3
	mulhwu	$D3,$h0,$r3

	mullw	$t0,$h1,$s3		# h1*s3
	mulhwu	$t1,$h1,$s3

	mullw	$t2,$h1,$r0		# h1*r0
	mulhwu	$t3,$h1,$r0
	 addc	$d0,$d0,$t0
	 adde	$D0,$D0,$t1

	mullw	$t0,$h1,$r1		# h1*r1
	mulhwu	$t1,$h1,$r1
	 addc	$d1,$d1,$t2
	 adde	$D1,$D1,$t3

	mullw	$t2,$h1,$r2		# h1*r2
	mulhwu	$t3,$h1,$r2
	 addc	$d2,$d2,$t0
	 adde	$D2,$D2,$t1

	mullw	$t0,$h2,$s2		# h2*s2
	mulhwu	$t1,$h2,$s2
	 addc	$d3,$d3,$t2
	 adde	$D3,$D3,$t3

	mullw	$t2,$h2,$s3		# h2*s3
	mulhwu	$t3,$h2,$s3
	 addc	$d0,$d0,$t0
	 adde	$D0,$D0,$t1

	mullw	$t0,$h2,$r0		# h2*r0
	mulhwu	$t1,$h2,$r0
	 addc	$d1,$d1,$t2
	 adde	$D1,$D1,$t3

	mullw	$t2,$h2,$r1		# h2*r1
	mulhwu	$t3,$h2,$r1
	 addc	$d2,$d2,$t0
	 adde	$D2,$D2,$t1

	mullw	$t0,$h3,$s1		# h3*s1
	mulhwu	$t1,$h3,$s1
	 addc	$d3,$d3,$t2
	 adde	$D3,$D3,$t3

	mullw	$t2,$h3,$s2		# h3*s2
	mulhwu	$t3,$h3,$s2
	 addc	$d0,$d0,$t0
	 adde	$D0,$D0,$t1

	mullw	$t0,$h3,$s3		# h3*s3
	mulhwu	$t1,$h3,$s3
	 addc	$d1,$d1,$t2
	 adde	$D1,$D1,$t3

	mullw	$t2,$h3,$r0		# h3*r0
	mulhwu	$t3,$h3,$r0
	 addc	$d2,$d2,$t0
	 adde	$D2,$D2,$t1

	mullw	$t0,$h4,$s1		# h4*s1
	 addc	$d3,$d3,$t2
	 adde	$D3,$D3,$t3
	addc	$d1,$d1,$t0

	mullw	$t1,$h4,$s2		# h4*s2
	 addze	$D1,$D1
	addc	$d2,$d2,$t1
	addze	$D2,$D2

	mullw	$t2,$h4,$s3		# h4*s3
	addc	$d3,$d3,$t2
	addze	$D3,$D3

	mullw	$h4,$h4,$r0		# h4*r0

	addc	$h1,$d1,$D0
	adde	$h2,$d2,$D1
	adde	$h3,$d3,$D2
	adde	$h4,$h4,$D3

	andc	$D0,$h4,$mask		# final reduction step
	and	$h4,$h4,$mask
	srwi	$D1,$D0,2
	add	$D0,$D0,$D1
	addc	$h0,$d0,$D0
	addze	$h1,$h1
	addze	$h2,$h2
	addze	$h3,$h3
	addze	$h4,$h4

	bdnz	Loop

	stw	$h0,0($ctx)		# store hash value
	stw	$h1,4($ctx)
	stw	$h2,8($ctx)
	stw	$h3,12($ctx)
	stw	$h4,16($ctx)

	$POP	r14,`$FRAME-$SIZE_T*18`($sp)
	$POP	r15,`$FRAME-$SIZE_T*17`($sp)
	$POP	r16,`$FRAME-$SIZE_T*16`($sp)
	$POP	r17,`$FRAME-$SIZE_T*15`($sp)
	$POP	r18,`$FRAME-$SIZE_T*14`($sp)
	$POP	r19,`$FRAME-$SIZE_T*13`($sp)
	$POP	r20,`$FRAME-$SIZE_T*12`($sp)
	$POP	r21,`$FRAME-$SIZE_T*11`($sp)
	$POP	r22,`$FRAME-$SIZE_T*10`($sp)
	$POP	r23,`$FRAME-$SIZE_T*9`($sp)
	$POP	r24,`$FRAME-$SIZE_T*8`($sp)
	$POP	r25,`$FRAME-$SIZE_T*7`($sp)
	$POP	r26,`$FRAME-$SIZE_T*6`($sp)
	$POP	r27,`$FRAME-$SIZE_T*5`($sp)
	$POP	r28,`$FRAME-$SIZE_T*4`($sp)
	$POP	r29,`$FRAME-$SIZE_T*3`($sp)
	$POP	r30,`$FRAME-$SIZE_T*2`($sp)
	$POP	r31,`$FRAME-$SIZE_T*1`($sp)
	addi	$sp,$sp,$FRAME
Labort:
	blr
	.long	0
	.byte	0,12,4,1,0x80,18,4,0
.size	.poly1305_blocks,.-.poly1305_blocks
___
{
my ($h0,$h1,$h2,$h3,$h4,$t0,$t1) = map("r$_",(6..12));

$code.=<<___;
.globl	.poly1305_emit
.align	5
.poly1305_emit:
	lwz	r0,24($ctx)	# is_base2_26
	lwz	$h0,0($ctx)	# load hash value
	lwz	$h1,4($ctx)
	lwz	$h2,8($ctx)
	lwz	$h3,12($ctx)
	lwz	$h4,16($ctx)
	cmplwi	r0,0
	beq	Lemit_base2_32

	slwi	$t0,$h1,26	# base 2^26 -> base 2^32
	srwi	$h1,$h1,6
	slwi	$t1,$h2,20
	srwi	$h2,$h2,12
	addc	$h0,$h0,$t0
	slwi	$t0,$h3,14
	srwi	$h3,$h3,18
	adde	$h1,$h1,$t1
	slwi	$t1,$h4,8
	srwi	$h4,$h4,24
	adde	$h2,$h2,$t0
	adde	$h3,$h3,$t1
	addze	$h4,$h4

Lemit_base2_32:
	addic	r0,$h0,5	# compare to modulus
	addze	r0,$h1
	addze	r0,$h2
	addze	r0,$h3
	addze	r0,$h4

	srwi	r0,r0,2		# see if it carried/borrowed
	neg	r0,r0
	andi.	r0,r0,5

	addc	$h0,$h0,r0
	lwz	r0,0($nonce)
	addze	$h1,$h1
	lwz	$t0,4($nonce)
	addze	$h2,$h2
	lwz	$t1,8($nonce)
	addze	$h3,$h3
	lwz	$h4,12($nonce)

	addc	$h0,$h0,r0	# accumulate nonce
	adde	$h1,$h1,$t0
	adde	$h2,$h2,$t1
	adde	$h3,$h3,$h4

	addi	$ctx,$mac,-1
	addi	$mac,$mac,7

	stbu	$h0,1($ctx)	# write [little-endian] result
	srwi	$h0,$h0,8
	stbu	$h2,1($mac)
	srwi	$h2,$h2,8

	stbu	$h0,1($ctx)
	srwi	$h0,$h0,8
	stbu	$h2,1($mac)
	srwi	$h2,$h2,8

	stbu	$h0,1($ctx)
	srwi	$h0,$h0,8
	stbu	$h2,1($mac)
	srwi	$h2,$h2,8

	stbu	$h0,1($ctx)
	stbu	$h2,1($mac)

	stbu	$h1,1($ctx)
	srwi	$h1,$h1,8
	stbu	$h3,1($mac)
	srwi	$h3,$h3,8

	stbu	$h1,1($ctx)
	srwi	$h1,$h1,8
	stbu	$h3,1($mac)
	srwi	$h3,$h3,8

	stbu	$h1,1($ctx)
	srwi	$h1,$h1,8
	stbu	$h3,1($mac)
	srwi	$h3,$h3,8

	stbu	$h1,1($ctx)
	stbu	$h3,1($mac)

	blr
	.long	0
	.byte	0,12,0x14,0,0,0,3,0
.size	.poly1305_emit,.-.poly1305_emit
___
}							}
{{{
########################################################################
# PowerISA 2.07/VSX section                                            #
########################################################################

my $LOCALS= 6*$SIZE_T;
my $VSXFRAME = $LOCALS + 6*$SIZE_T;
   $VSXFRAME += 128;	# local variables
   $VSXFRAME += 12*16;	# v20-v31 offload

my $BIG_ENDIAN = ($flavour !~ /le/) ? 4 : 0;

########################################################################
# Layout of opaque area is following:
#
#	unsigned __int32 h[5];		# current hash value base 2^26
#	unsigned __int32 pad;
#	unsigned __int32 is_base2_26, pad;
#	unsigned __int64 r[2];		# key value base 2^64
#	struct { unsigned __int32 r^2, r^4, r^1, r^3; } r[9];
#
# where r^n are base 2^26 digits of powers of multiplier key. There are
# 5 digits, but last four are interleaved with multiples of 5, totalling
# in 9 elements: r0, r1, 5*r1, r2, 5*r2, r3, 5*r3, r4, 5*r4. Order of
# powers is as they appear in register, not memory.

my ($H0, $H1, $H2, $H3, $H4) = map("v$_",(0..4));
my ($I0, $I1, $I2, $I3, $I4) = map("v$_",(5..9));
my ($R0, $R1, $S1, $R2, $S2) = map("v$_",(10..14));
my      ($R3, $S3, $R4, $S4) = ($R1, $S1, $R2, $S2);
my ($ACC0, $ACC1, $ACC2, $ACC3, $ACC4) = map("v$_",(15..19));
my ($T0, $T1, $T2, $T3, $T4) = map("v$_",(20..24));
my ($_26,$_4,$_40,$_14,$mask26,$padbits,$I2perm) = map("v$_",(25..31));
my ($x00,$x60,$x70,$x10,$x20,$x30,$x40,$x50) = (0, map("r$_",(7,8,27..31)));
my ($ctx_,$_ctx,$const) = map("r$_",(10..12));

							if ($flavour =~ /64/) {
###############################################################################
# setup phase of poly1305_blocks_vsx is different on 32- and 64-bit platforms,
# but the base 2^26 computational part is same...

my ($h0,$h1,$h2,$d0,$d1,$d2, $r0,$r1,$s1, $t0,$t1) = map("r$_",(6..11,27..31));
my $mask = "r0";

$code.=<<___;
.globl	.poly1305_blocks_vsx
.align	5
.poly1305_blocks_vsx:
	lwz	r7,24($ctx)		# is_base2_26
	cmpldi	$len,128
	bge	__poly1305_blocks_vsx

	neg	r0,r7			# is_base2_26 as mask
	lwz	r7,0($ctx)		# load hash base 2^26
	lwz	r8,4($ctx)
	lwz	r9,8($ctx)
	lwz	r10,12($ctx)
	lwz	r11,16($ctx)

	sldi	r8,r8,26		# base 2^26 -> base 2^64
	sldi	r12,r9,52
	add	r7,r7,r8
	srdi	r9,r9,12
	sldi	r10,r10,14
	addc	r7,r7,r12
	sldi	r8,r11,40
	adde	r9,r9,r10
	srdi	r11,r11,24
	addc	r9,r9,r8
	addze	r11,r11

	ld	r8,0($ctx)		# load hash base 2^64
	ld	r10,8($ctx)
	ld	r12,16($ctx)

	xor	r7,r7,r8		# select between radixes
	xor	r9,r9,r10
	xor	r11,r11,r12
	and	r7,r7,r0
	and	r9,r9,r0
	and	r11,r11,r0
	xor	r7,r7,r8
	xor	r9,r9,r10
	xor	r11,r11,r12

	li	r0,0
	std	r7,0($ctx)		# store hash base 2^64
	std	r9,8($ctx)
	std	r11,16($ctx)
	stw	r0,24($ctx)		# clear is_base2_26

	b	Lpoly1305_blocks
	.long	0
	.byte	0,12,0x14,0,0,0,4,0
.size	.poly1305_blocks_vsx,.-.poly1305_blocks_vsx

.align	5
__poly1305_mul:
	mulld	$d0,$h0,$r0		# h0*r0
	mulhdu	$d1,$h0,$r0

	mulld	$t0,$h1,$s1		# h1*5*r1
	mulhdu	$t1,$h1,$s1
	addc	$d0,$d0,$t0
	adde	$d1,$d1,$t1

	mulld	$t0,$h0,$r1		# h0*r1
	mulhdu	$d2,$h0,$r1
	addc	$d1,$d1,$t0
	addze	$d2,$d2

	mulld	$t0,$h1,$r0		# h1*r0
	mulhdu	$t1,$h1,$r0
	addc	$d1,$d1,$t0
	adde	$d2,$d2,$t1

	mulld	$t0,$h2,$s1		# h2*5*r1
	mulld	$t1,$h2,$r0		# h2*r0
	addc	$d1,$d1,$t0
	adde	$d2,$d2,$t1

	andc	$t0,$d2,$mask		# final reduction step
	and	$h2,$d2,$mask
	srdi	$t1,$t0,2
	add	$t0,$t0,$t1
	addc	$h0,$d0,$t0
	addze	$h1,$d1
	addze	$h2,$h2

	blr
	.long	0
	.byte	0,12,0x14,0,0,0,0,0
.size	__poly1305_mul,.-__poly1305_mul

.align	5
__poly1305_splat:
	extrdi	$d0,$h0,26,38
	extrdi	$d1,$h0,26,12
	stw	$d0,0x00($t1)

	extrdi	$d2,$h0,12,0
	slwi	$d0,$d1,2
	stw	$d1,0x10($t1)
	add	$d0,$d0,$d1		# * 5
	stw	$d0,0x20($t1)

	insrdi	$d2,$h1,14,38
	slwi	$d0,$d2,2
	stw	$d2,0x30($t1)
	add	$d0,$d0,$d2		# * 5
	stw	$d0,0x40($t1)

	extrdi	$d1,$h1,26,24
	extrdi	$d2,$h1,24,0
	slwi	$d0,$d1,2
	stw	$d1,0x50($t1)
	add	$d0,$d0,$d1		# * 5
	stw	$d0,0x60($t1)

	insrdi	$d2,$h2,3,37
	slwi	$d0,$d2,2
	stw	$d2,0x70($t1)
	add	$d0,$d0,$d2		# * 5
	stw	$d0,0x80($t1)

	blr
	.long	0
	.byte	0,12,0x14,0,0,0,0,0
.size	__poly1305_splat,.-__poly1305_splat

.align	5
__poly1305_blocks_vsx:
	$STU	$sp,-$VSXFRAME($sp)
	mflr	r0
	li	r10,`15+$LOCALS+128`
	li	r11,`31+$LOCALS+128`
	mfspr	r12,256
	stvx	v20,r10,$sp
	addi	r10,r10,32
	stvx	v21,r11,$sp
	addi	r11,r11,32
	stvx	v22,r10,$sp
	addi	r10,r10,32
	stvx	v23,r11,$sp
	addi	r11,r11,32
	stvx	v24,r10,$sp
	addi	r10,r10,32
	stvx	v25,r11,$sp
	addi	r11,r11,32
	stvx	v26,r10,$sp
	addi	r10,r10,32
	stvx	v27,r11,$sp
	addi	r11,r11,32
	stvx	v28,r10,$sp
	addi	r10,r10,32
	stvx	v29,r11,$sp
	addi	r11,r11,32
	stvx	v30,r10,$sp
	stvx	v31,r11,$sp
	stw	r12,`$VSXFRAME-$SIZE_T*5-4`($sp)# save vrsave
	li	r12,-1
	mtspr	256,r12			# preserve all AltiVec registers
	$PUSH	r27,`$VSXFRAME-$SIZE_T*5`($sp)
	$PUSH	r28,`$VSXFRAME-$SIZE_T*4`($sp)
	$PUSH	r29,`$VSXFRAME-$SIZE_T*3`($sp)
	$PUSH	r30,`$VSXFRAME-$SIZE_T*2`($sp)
	$PUSH	r31,`$VSXFRAME-$SIZE_T*1`($sp)
	$PUSH	r0,`$VSXFRAME+$LRSAVE`($sp)

	bl	LPICmeup

	li	$x10,0x10
	li	$x20,0x20
	li	$x30,0x30
	li	$x40,0x40
	li	$x50,0x50
	lvx_u	$mask26,$x00,$const
	lvx_u	$_26,$x10,$const
	lvx_u	$_40,$x20,$const
	lvx_u	$I2perm,$x30,$const
	lvx_u	$padbits,$x40,$const

	cmplwi	r7,0			# is_base2_26?
	bne	Lskip_init_vsx

	ld	$r0,32($ctx)		# load key base 2^64
	ld	$r1,40($ctx)
	srdi	$s1,$r1,2
	li	$mask,3
	add	$s1,$s1,$r1		# s1 = r1 + r1>>2

	mr	$h0,$r0			# "calculate" r^1
	mr	$h1,$r1
	li	$h2,0
	addi	$t1,$ctx,`48+(12^$BIG_ENDIAN)`
	bl	__poly1305_splat

	bl	__poly1305_mul		# calculate r^2
	addi	$t1,$ctx,`48+(4^$BIG_ENDIAN)`
	bl	__poly1305_splat

	bl	__poly1305_mul		# calculate r^3
	addi	$t1,$ctx,`48+(8^$BIG_ENDIAN)`
	bl	__poly1305_splat

	bl	__poly1305_mul		# calculate r^4
	addi	$t1,$ctx,`48+(0^$BIG_ENDIAN)`
	bl	__poly1305_splat

	ld	$h0,0($ctx)		# load hash
	ld	$h1,8($ctx)
	ld	$h2,16($ctx)

	extrdi	$d0,$h0,26,38		# base 2^64 -> base 2^26
	extrdi	$d1,$h0,26,12
	extrdi	$d2,$h0,12,0
	mtvrwz	$H0,$d0
	insrdi	$d2,$h1,14,38
	mtvrwz	$H1,$d1
	extrdi	$d1,$h1,26,24
	mtvrwz	$H2,$d2
	extrdi	$d2,$h1,24,0
	mtvrwz	$H3,$d1
	insrdi	$d2,$h2,3,37
	mtvrwz	$H4,$d2
___
							} else {
###############################################################################
# 32-bit initialization

my ($h0,$h1,$h2,$h3,$h4,$t0,$t1) = map("r$_",(7..11,0,12));
my ($R3,$S3,$R4,$S4)=($I1,$I2,$I3,$I4);

$code.=<<___;
.globl	.poly1305_blocks_vsx
.align	5
.poly1305_blocks_vsx:
	lwz	r7,24($ctx)		# is_base2_26
	cmplwi	$len,128
	bge	__poly1305_blocks_vsx
	cmplwi	r7,0
	beq	Lpoly1305_blocks

	lwz	$h0,0($ctx)		# load hash
	lwz	$h1,4($ctx)
	lwz	$h2,8($ctx)
	lwz	$h3,12($ctx)
	lwz	$h4,16($ctx)

	slwi	$t0,$h1,26		# base 2^26 -> base 2^32
	srwi	$h1,$h1,6
	slwi	$t1,$h2,20
	srwi	$h2,$h2,12
	addc	$h0,$h0,$t0
	slwi	$t0,$h3,14
	srwi	$h3,$h3,18
	adde	$h1,$h1,$t1
	slwi	$t1,$h4,8
	srwi	$h4,$h4,24
	adde	$h2,$h2,$t0
	li	$t0,0
	adde	$h3,$h3,$t1
	addze	$h4,$h4

	stw	$h0,0($ctx)		# store hash base 2^32
	stw	$h1,4($ctx)
	stw	$h2,8($ctx)
	stw	$h3,12($ctx)
	stw	$h4,16($ctx)
	stw	$t0,24($ctx)		# clear is_base2_26

	b	Lpoly1305_blocks
	.long	0
	.byte	0,12,0x14,0,0,0,4,0
.size	.poly1305_blocks_vsx,.-.poly1305_blocks_vsx

.align	5
__poly1305_mul:
	vmulouw		$ACC0,$H0,$R0
	vmulouw		$ACC1,$H1,$R0
	vmulouw		$ACC2,$H2,$R0
	vmulouw		$ACC3,$H3,$R0
	vmulouw		$ACC4,$H4,$R0

	vmulouw		$T0,$H4,$S1
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H0,$R1
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H1,$R1
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H2,$R1
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H3,$R1
	vaddudm		$ACC4,$ACC4,$T0

	vmulouw		$T0,$H3,$S2
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H4,$S2
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H0,$R2
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H1,$R2
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H2,$R2
	vaddudm		$ACC4,$ACC4,$T0

	vmulouw		$T0,$H2,$S3
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H3,$S3
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H4,$S3
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H0,$R3
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H1,$R3
	vaddudm		$ACC4,$ACC4,$T0

	vmulouw		$T0,$H1,$S4
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H2,$S4
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H3,$S4
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H4,$S4
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H0,$R4
	vaddudm		$ACC4,$ACC4,$T0

	################################################################
	# lazy reduction

	vspltisb	$T0,2
	vsrd		$H4,$ACC3,$_26
	vsrd		$H1,$ACC0,$_26
	vand		$H3,$ACC3,$mask26
	vand		$H0,$ACC0,$mask26
	vaddudm		$H4,$H4,$ACC4		# h3 -> h4
	vaddudm		$H1,$H1,$ACC1		# h0 -> h1

	vsrd		$ACC4,$H4,$_26
	vsrd		$ACC1,$H1,$_26
	vand		$H4,$H4,$mask26
	vand		$H1,$H1,$mask26
	vaddudm		$H0,$H0,$ACC4
	vaddudm		$H2,$ACC2,$ACC1		# h1 -> h2

	vsld		$ACC4,$ACC4,$T0		# <<2
	vsrd		$ACC2,$H2,$_26
	vand		$H2,$H2,$mask26
	vaddudm		$H0,$H0,$ACC4		# h4 -> h0
	vaddudm		$H3,$H3,$ACC2		# h2 -> h3

	vsrd		$ACC0,$H0,$_26
	vsrd		$ACC3,$H3,$_26
	vand		$H0,$H0,$mask26
	vand		$H3,$H3,$mask26
	vaddudm		$H1,$H1,$ACC0		# h0 -> h1
	vaddudm		$H4,$H4,$ACC3		# h3 -> h4

	blr
	.long	0
	.byte	0,12,0x14,0,0,0,0,0
.size	__poly1305_mul,.-__poly1305_mul

.align	5
__poly1305_blocks_vsx:
	$STU	$sp,-$VSXFRAME($sp)
	mflr	r0
	li	r10,`15+$LOCALS+128`
	li	r11,`31+$LOCALS+128`
	mfspr	r12,256
	stvx	v20,r10,$sp
	addi	r10,r10,32
	stvx	v21,r11,$sp
	addi	r11,r11,32
	stvx	v22,r10,$sp
	addi	r10,r10,32
	stvx	v23,r11,$sp
	addi	r11,r11,32
	stvx	v24,r10,$sp
	addi	r10,r10,32
	stvx	v25,r11,$sp
	addi	r11,r11,32
	stvx	v26,r10,$sp
	addi	r10,r10,32
	stvx	v27,r11,$sp
	addi	r11,r11,32
	stvx	v28,r10,$sp
	addi	r10,r10,32
	stvx	v29,r11,$sp
	addi	r11,r11,32
	stvx	v30,r10,$sp
	stvx	v31,r11,$sp
	stw	r12,`$VSXFRAME-$SIZE_T*5-4`($sp)# save vrsave
	li	r12,-1
	mtspr	256,r12			# preserve all AltiVec registers
	$PUSH	r27,`$VSXFRAME-$SIZE_T*5`($sp)
	$PUSH	r28,`$VSXFRAME-$SIZE_T*4`($sp)
	$PUSH	r29,`$VSXFRAME-$SIZE_T*3`($sp)
	$PUSH	r30,`$VSXFRAME-$SIZE_T*2`($sp)
	$PUSH	r31,`$VSXFRAME-$SIZE_T*1`($sp)
	$PUSH	r0,`$VSXFRAME+$LRSAVE`($sp)

	bl	LPICmeup

	li	$x10,0x10
	li	$x20,0x20
	li	$x30,0x30
	li	$x40,0x40
	li	$x50,0x50
	lvx_u	$mask26,$x00,$const
	lvx_u	$_26,$x10,$const
	lvx_u	$_40,$x20,$const
	lvx_u	$I2perm,$x30,$const
	lvx_u	$padbits,$x40,$const

	cmplwi	r7,0			# is_base2_26?
	bne	Lskip_init_vsx

	lwz	$h1,32($ctx)		# load key base 2^32
	lwz	$h2,36($ctx)
	lwz	$h3,40($ctx)
	lwz	$h4,44($ctx)

	extrwi	$h0,$h1,26,6		# base 2^32 -> base 2^26
	extrwi	$h1,$h1,6,0
	insrwi	$h1,$h2,20,6
	extrwi	$h2,$h2,12,0
	insrwi	$h2,$h3,14,6
	extrwi	$h3,$h3,18,0
	insrwi	$h3,$h4,8,6
	extrwi	$h4,$h4,24,0

	mtvrwz	$R0,$h0
	slwi	$h0,$h1,2
	mtvrwz	$R1,$h1
	add	$h1,$h1,$h0
	mtvrwz	$S1,$h1
	slwi	$h1,$h2,2
	mtvrwz	$R2,$h2
	add	$h2,$h2,$h1
	mtvrwz	$S2,$h2
	slwi	$h2,$h3,2
	mtvrwz	$R3,$h3
	add	$h3,$h3,$h2
	mtvrwz	$S3,$h3
	slwi	$h3,$h4,2
	mtvrwz	$R4,$h4
	add	$h4,$h4,$h3
	mtvrwz	$S4,$h4

	vmr	$H0,$R0
	vmr	$H1,$R1
	vmr	$H2,$R2
	vmr	$H3,$R3
	vmr	$H4,$R4

	bl	__poly1305_mul		# r^1:- * r^1:-

	vpermdi	$R0,$H0,$R0,0b00
	vpermdi	$R1,$H1,$R1,0b00
	vpermdi	$R2,$H2,$R2,0b00
	vpermdi	$R3,$H3,$R3,0b00
	vpermdi	$R4,$H4,$R4,0b00
	vpermdi	$H0,$H0,$H0,0b00
	vpermdi	$H1,$H1,$H1,0b00
	vpermdi	$H2,$H2,$H2,0b00
	vpermdi	$H3,$H3,$H3,0b00
	vpermdi	$H4,$H4,$H4,0b00
	vsld	$S1,$R1,$T0		# <<2
	vsld	$S2,$R2,$T0
	vsld	$S3,$R3,$T0
	vsld	$S4,$R4,$T0
	vaddudm	$S1,$S1,$R1
	vaddudm	$S2,$S2,$R2
	vaddudm	$S3,$S3,$R3
	vaddudm	$S4,$S4,$R4

	bl	__poly1305_mul		# r^2:r^2 * r^2:r^1

	addi	$h0,$ctx,0x60
	lwz	$h1,0($ctx)		# load hash
	lwz	$h2,4($ctx)
	lwz	$h3,8($ctx)
	lwz	$h4,12($ctx)
	lwz	$t0,16($ctx)

	vmrgow	$R0,$R0,$H0		# r^2:r^4:r^1:r^3
	vmrgow	$R1,$R1,$H1
	vmrgow	$R2,$R2,$H2
	vmrgow	$R3,$R3,$H3
	vmrgow	$R4,$R4,$H4
	vslw	$S1,$R1,$T0		# <<2
	vslw	$S2,$R2,$T0
	vslw	$S3,$R3,$T0
	vslw	$S4,$R4,$T0
	vadduwm	$S1,$S1,$R1
	vadduwm	$S2,$S2,$R2
	vadduwm	$S3,$S3,$R3
	vadduwm	$S4,$S4,$R4

	stvx_u	$R0,$x30,$ctx
	stvx_u	$R1,$x40,$ctx
	stvx_u	$S1,$x50,$ctx
	stvx_u	$R2,$x00,$h0
	stvx_u	$S2,$x10,$h0
	stvx_u	$R3,$x20,$h0
	stvx_u	$S3,$x30,$h0
	stvx_u	$R4,$x40,$h0
	stvx_u	$S4,$x50,$h0

	extrwi	$h0,$h1,26,6		# base 2^32 -> base 2^26
	extrwi	$h1,$h1,6,0
	mtvrwz	$H0,$h0
	insrwi	$h1,$h2,20,6
	extrwi	$h2,$h2,12,0
	mtvrwz	$H1,$h1
	insrwi	$h2,$h3,14,6
	extrwi	$h3,$h3,18,0
	mtvrwz	$H2,$h2
	insrwi	$h3,$h4,8,6
	extrwi	$h4,$h4,24,0
	mtvrwz	$H3,$h3
	insrwi	$h4,$t0,3,5
	mtvrwz	$H4,$h4
___
							}
$code.=<<___;
	li	r0,1
	stw	r0,24($ctx)		# set is_base2_26
	b	Loaded_vsx

.align	4
Lskip_init_vsx:
	li		$x10,4
	li		$x20,8
	li		$x30,12
	li		$x40,16
	lvwzx_u		$H0,$x00,$ctx
	lvwzx_u		$H1,$x10,$ctx
	lvwzx_u		$H2,$x20,$ctx
	lvwzx_u		$H3,$x30,$ctx
	lvwzx_u		$H4,$x40,$ctx

Loaded_vsx:
	li		$x10,0x10
	li		$x20,0x20
	li		$x30,0x30
	li		$x40,0x40
	li		$x50,0x50
	li		$x60,0x60
	li		$x70,0x70
	addi		$ctx_,$ctx,64		# &ctx->r[1]
	addi		$_ctx,$sp,`$LOCALS+15`	# &ctx->r[1], r^2:r^4 shadow

	vxor		$T0,$T0,$T0		# ensure second half is zero
	vpermdi		$H0,$H0,$T0,0b00
	vpermdi		$H1,$H1,$T0,0b00
	vpermdi		$H2,$H2,$T0,0b00
	vpermdi		$H3,$H3,$T0,0b00
	vpermdi		$H4,$H4,$T0,0b00

	be?lvx_u	$_4,$x50,$const		# byte swap mask
	lvx_u		$T1,$x00,$inp		# load first input block
	lvx_u		$T2,$x10,$inp
	lvx_u		$T3,$x20,$inp
	lvx_u		$T4,$x30,$inp
	be?vperm	$T1,$T1,$T1,$_4
	be?vperm	$T2,$T2,$T2,$_4
	be?vperm	$T3,$T3,$T3,$_4
	be?vperm	$T4,$T4,$T4,$_4

	vpermdi		$I0,$T1,$T2,0b00	# smash input to base 2^26
	vspltisb	$_4,4
	vperm		$I2,$T1,$T2,$I2perm	# 0x...0e0f0001...1e1f1011
	vspltisb	$_14,14
	vpermdi		$I3,$T1,$T2,0b11

	vsrd		$I1,$I0,$_26
	vsrd		$I2,$I2,$_4
	vsrd		$I4,$I3,$_40
	vsrd		$I3,$I3,$_14
	vand		$I0,$I0,$mask26
	vand		$I1,$I1,$mask26
	vand		$I2,$I2,$mask26
	vand		$I3,$I3,$mask26

	vpermdi		$T1,$T3,$T4,0b00
	vperm		$T2,$T3,$T4,$I2perm	# 0x...0e0f0001...1e1f1011
	vpermdi		$T3,$T3,$T4,0b11

	vsrd		$T0,$T1,$_26
	vsrd		$T2,$T2,$_4
	vsrd		$T4,$T3,$_40
	vsrd		$T3,$T3,$_14
	vand		$T1,$T1,$mask26
	vand		$T0,$T0,$mask26
	vand		$T2,$T2,$mask26
	vand		$T3,$T3,$mask26

	# inp[2]:inp[0]:inp[3]:inp[1]
	vmrgow		$I4,$T4,$I4
	vmrgow		$I0,$T1,$I0
	vmrgow		$I1,$T0,$I1
	vmrgow		$I2,$T2,$I2
	vmrgow		$I3,$T3,$I3
	vor		$I4,$I4,$padbits

	lvx_splt	$R0,$x30,$ctx		# taking lvx_vsplt out of loop
	lvx_splt	$R1,$x00,$ctx_		# gives ~8% improvement
	lvx_splt	$S1,$x10,$ctx_
	lvx_splt	$R2,$x20,$ctx_
	lvx_splt	$S2,$x30,$ctx_
	lvx_splt	$T1,$x40,$ctx_
	lvx_splt	$T2,$x50,$ctx_
	lvx_splt	$T3,$x60,$ctx_
	lvx_splt	$T4,$x70,$ctx_
	stvx		$R1,$x00,$_ctx
	stvx		$S1,$x10,$_ctx
	stvx		$R2,$x20,$_ctx
	stvx		$S2,$x30,$_ctx
	stvx		$T1,$x40,$_ctx
	stvx		$T2,$x50,$_ctx
	stvx		$T3,$x60,$_ctx
	stvx		$T4,$x70,$_ctx

	addi		$inp,$inp,0x40
	addi		$const,$const,0x50
	addi		r0,$len,-64
	srdi		r0,r0,6
	mtctr		r0
	b		Loop_vsx

.align	4
Loop_vsx:
	################################################################
	## ((inp[0]*r^4+inp[2]*r^2+inp[4])*r^4+inp[6]*r^2
	## ((inp[1]*r^4+inp[3]*r^2+inp[5])*r^3+inp[7]*r
	##   \___________________/
	##
	## Note that we start with inp[2:3]*r^2. This is because it
	## doesn't depend on reduction in previous iteration.
	################################################################
	## d4 = h4*r0 + h3*r1   + h2*r2   + h1*r3   + h0*r4
	## d3 = h3*r0 + h2*r1   + h1*r2   + h0*r3   + h4*5*r4
	## d2 = h2*r0 + h1*r1   + h0*r2   + h4*5*r3 + h3*5*r4
	## d1 = h1*r0 + h0*r1   + h4*5*r2 + h3*5*r3 + h2*5*r4
	## d0 = h0*r0 + h4*5*r1 + h3*5*r2 + h2*5*r3 + h1*5*r4

	vmuleuw		$ACC0,$I0,$R0
	vmuleuw		$ACC1,$I0,$R1
	vmuleuw		$ACC2,$I0,$R2
	vmuleuw		$ACC3,$I1,$R2

	vmuleuw		$T0,$I1,$R0
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I1,$R1
	vaddudm		$ACC2,$ACC2,$T0
	 vmuleuw	$ACC4,$I2,$R2
	vmuleuw		$T0,$I4,$S1
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I2,$R1
	vaddudm		$ACC3,$ACC3,$T0
	lvx		$S3,$x50,$_ctx
	vmuleuw		$T0,$I3,$R1
	vaddudm		$ACC4,$ACC4,$T0
	lvx		$R3,$x40,$_ctx

	 vaddudm	$H2,$H2,$I2
	 vaddudm	$H0,$H0,$I0
	 vaddudm	$H3,$H3,$I3
	 vaddudm	$H1,$H1,$I1
	 vaddudm	$H4,$H4,$I4

	vmuleuw		$T0,$I3,$S2
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I4,$S2
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I2,$R0
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I3,$R0
	vaddudm		$ACC3,$ACC3,$T0
	lvx		$S4,$x70,$_ctx
	vmuleuw		$T0,$I4,$R0
	vaddudm		$ACC4,$ACC4,$T0
	lvx		$R4,$x60,$_ctx

	vmuleuw		$T0,$I2,$S3
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I3,$S3
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I4,$S3
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I0,$R3
	vaddudm		$ACC3,$ACC3,$T0
	vmuleuw		$T0,$I1,$R3
	vaddudm		$ACC4,$ACC4,$T0

	 be?lvx_u	$_4,$x00,$const		# byte swap mask
	 lvx_u		$T1,$x00,$inp		# load next input block
	 lvx_u		$T2,$x10,$inp
	 lvx_u		$T3,$x20,$inp
	 lvx_u		$T4,$x30,$inp
	 be?vperm	$T1,$T1,$T1,$_4
	 be?vperm	$T2,$T2,$T2,$_4
	 be?vperm	$T3,$T3,$T3,$_4
	 be?vperm	$T4,$T4,$T4,$_4

	vmuleuw		$T0,$I1,$S4
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I2,$S4
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I3,$S4
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I4,$S4
	vaddudm		$ACC3,$ACC3,$T0
	vmuleuw		$T0,$I0,$R4
	vaddudm		$ACC4,$ACC4,$T0

	 vpermdi	$I0,$T1,$T2,0b00	# smash input to base 2^26
	 vspltisb	$_4,4
	 vperm		$I2,$T1,$T2,$I2perm	# 0x...0e0f0001...1e1f1011
	 vpermdi	$I3,$T1,$T2,0b11

	# (hash + inp[0:1]) * r^4
	vmulouw		$T0,$H0,$R0
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H1,$R0
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H2,$R0
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H3,$R0
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H4,$R0
	vaddudm		$ACC4,$ACC4,$T0

	 vpermdi	$T1,$T3,$T4,0b00
	 vperm		$T2,$T3,$T4,$I2perm	# 0x...0e0f0001...1e1f1011
	 vpermdi	$T3,$T3,$T4,0b11

	vmulouw		$T0,$H2,$S3
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H3,$S3
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H4,$S3
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H0,$R3
	vaddudm		$ACC3,$ACC3,$T0
	lvx		$S1,$x10,$_ctx
	vmulouw		$T0,$H1,$R3
	vaddudm		$ACC4,$ACC4,$T0
	lvx		$R1,$x00,$_ctx

	 vsrd		$I1,$I0,$_26
	 vsrd		$I2,$I2,$_4
	 vsrd		$I4,$I3,$_40
	 vsrd		$I3,$I3,$_14

	vmulouw		$T0,$H1,$S4
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H2,$S4
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H3,$S4
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H4,$S4
	vaddudm		$ACC3,$ACC3,$T0
	lvx		$S2,$x30,$_ctx
	vmulouw		$T0,$H0,$R4
	vaddudm		$ACC4,$ACC4,$T0
	lvx		$R2,$x20,$_ctx

	 vand		$I0,$I0,$mask26
	 vand		$I1,$I1,$mask26
	 vand		$I2,$I2,$mask26
	 vand		$I3,$I3,$mask26

	vmulouw		$T0,$H4,$S1
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H0,$R1
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H1,$R1
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H2,$R1
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H3,$R1
	vaddudm		$ACC4,$ACC4,$T0

	 vsrd		$T2,$T2,$_4
	 vsrd		$_4,$T1,$_26
	 vsrd		$T4,$T3,$_40
	 vsrd		$T3,$T3,$_14

	vmulouw		$T0,$H3,$S2
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H4,$S2
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H0,$R2
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H1,$R2
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H2,$R2
	vaddudm		$ACC4,$ACC4,$T0

	 vand		$T1,$T1,$mask26
	 vand		$_4,$_4,$mask26
	 vand		$T2,$T2,$mask26
	 vand		$T3,$T3,$mask26

	################################################################
	# lazy reduction as discussed in "NEON crypto" by D.J. Bernstein
	# and P. Schwabe

	vspltisb	$T0,2
	vsrd		$H4,$ACC3,$_26
	vsrd		$H1,$ACC0,$_26
	vand		$H3,$ACC3,$mask26
	vand		$H0,$ACC0,$mask26
	vaddudm		$H4,$H4,$ACC4		# h3 -> h4
	vaddudm		$H1,$H1,$ACC1		# h0 -> h1

	 vmrgow		$I4,$T4,$I4
	 vmrgow		$I0,$T1,$I0
	 vmrgow		$I1,$_4,$I1
	 vmrgow		$I2,$T2,$I2
	 vmrgow		$I3,$T3,$I3
	 vor		$I4,$I4,$padbits

	vsrd		$ACC4,$H4,$_26
	vsrd		$ACC1,$H1,$_26
	vand		$H4,$H4,$mask26
	vand		$H1,$H1,$mask26
	vaddudm		$H0,$H0,$ACC4
	vaddudm		$H2,$ACC2,$ACC1		# h1 -> h2

	vsld		$ACC4,$ACC4,$T0		# <<2
	vsrd		$ACC2,$H2,$_26
	vand		$H2,$H2,$mask26
	vaddudm		$H0,$H0,$ACC4		# h4 -> h0
	vaddudm		$H3,$H3,$ACC2		# h2 -> h3

	vsrd		$ACC0,$H0,$_26
	vsrd		$ACC3,$H3,$_26
	vand		$H0,$H0,$mask26
	vand		$H3,$H3,$mask26
	vaddudm		$H1,$H1,$ACC0		# h0 -> h1
	vaddudm		$H4,$H4,$ACC3		# h3 -> h4

	addi		$inp,$inp,0x40
	bdnz		Loop_vsx

	neg		$len,$len
	andi.		$len,$len,0x30
	sub		$inp,$inp,$len

	lvx_u		$R0,$x30,$ctx		# load all powers
	lvx_u		$R1,$x00,$ctx_
	lvx_u		$S1,$x10,$ctx_
	lvx_u		$R2,$x20,$ctx_
	lvx_u		$S2,$x30,$ctx_

Last_vsx:
	vmuleuw		$ACC0,$I0,$R0
	vmuleuw		$ACC1,$I1,$R0
	vmuleuw		$ACC2,$I2,$R0
	vmuleuw		$ACC3,$I3,$R0
	vmuleuw		$ACC4,$I4,$R0

	vmuleuw		$T0,$I4,$S1
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I0,$R1
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I1,$R1
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I2,$R1
	vaddudm		$ACC3,$ACC3,$T0
	lvx_u		$S3,$x50,$ctx_
	vmuleuw		$T0,$I3,$R1
	vaddudm		$ACC4,$ACC4,$T0
	lvx_u		$R3,$x40,$ctx_

	 vaddudm	$H2,$H2,$I2
	 vaddudm	$H0,$H0,$I0
	 vaddudm	$H3,$H3,$I3
	 vaddudm	$H1,$H1,$I1
	 vaddudm	$H4,$H4,$I4

	vmuleuw		$T0,$I3,$S2
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I4,$S2
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I0,$R2
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I1,$R2
	vaddudm		$ACC3,$ACC3,$T0
	lvx_u		$S4,$x70,$ctx_
	vmuleuw		$T0,$I2,$R2
	vaddudm		$ACC4,$ACC4,$T0
	lvx_u		$R4,$x60,$ctx_

	vmuleuw		$T0,$I2,$S3
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I3,$S3
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I4,$S3
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I0,$R3
	vaddudm		$ACC3,$ACC3,$T0
	vmuleuw		$T0,$I1,$R3
	vaddudm		$ACC4,$ACC4,$T0

	vmuleuw		$T0,$I1,$S4
	vaddudm		$ACC0,$ACC0,$T0
	vmuleuw		$T0,$I2,$S4
	vaddudm		$ACC1,$ACC1,$T0
	vmuleuw		$T0,$I3,$S4
	vaddudm		$ACC2,$ACC2,$T0
	vmuleuw		$T0,$I4,$S4
	vaddudm		$ACC3,$ACC3,$T0
	vmuleuw		$T0,$I0,$R4
	vaddudm		$ACC4,$ACC4,$T0

	# (hash + inp[0:1]) * r^4
	vmulouw		$T0,$H0,$R0
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H1,$R0
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H2,$R0
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H3,$R0
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H4,$R0
	vaddudm		$ACC4,$ACC4,$T0

	vmulouw		$T0,$H2,$S3
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H3,$S3
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H4,$S3
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H0,$R3
	vaddudm		$ACC3,$ACC3,$T0
	lvx_u		$S1,$x10,$ctx_
	vmulouw		$T0,$H1,$R3
	vaddudm		$ACC4,$ACC4,$T0
	lvx_u		$R1,$x00,$ctx_

	vmulouw		$T0,$H1,$S4
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H2,$S4
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H3,$S4
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H4,$S4
	vaddudm		$ACC3,$ACC3,$T0
	lvx_u		$S2,$x30,$ctx_
	vmulouw		$T0,$H0,$R4
	vaddudm		$ACC4,$ACC4,$T0
	lvx_u		$R2,$x20,$ctx_

	vmulouw		$T0,$H4,$S1
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H0,$R1
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H1,$R1
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H2,$R1
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H3,$R1
	vaddudm		$ACC4,$ACC4,$T0

	vmulouw		$T0,$H3,$S2
	vaddudm		$ACC0,$ACC0,$T0
	vmulouw		$T0,$H4,$S2
	vaddudm		$ACC1,$ACC1,$T0
	vmulouw		$T0,$H0,$R2
	vaddudm		$ACC2,$ACC2,$T0
	vmulouw		$T0,$H1,$R2
	vaddudm		$ACC3,$ACC3,$T0
	vmulouw		$T0,$H2,$R2
	vaddudm		$ACC4,$ACC4,$T0

	################################################################
	# horizontal addition

	vpermdi		$H0,$ACC0,$ACC0,0b10
	vpermdi		$H1,$ACC1,$ACC1,0b10
	vpermdi		$H2,$ACC2,$ACC2,0b10
	vpermdi		$H3,$ACC3,$ACC3,0b10
	vpermdi		$H4,$ACC4,$ACC4,0b10
	vaddudm		$ACC0,$ACC0,$H0
	vaddudm		$ACC1,$ACC1,$H1
	vaddudm		$ACC2,$ACC2,$H2
	vaddudm		$ACC3,$ACC3,$H3
	vaddudm		$ACC4,$ACC4,$H4

	################################################################
	# lazy reduction

	vspltisb	$T0,2
	vsrd		$H4,$ACC3,$_26
	vsrd		$H1,$ACC0,$_26
	vand		$H3,$ACC3,$mask26
	vand		$H0,$ACC0,$mask26
	vaddudm		$H4,$H4,$ACC4		# h3 -> h4
	vaddudm		$H1,$H1,$ACC1		# h0 -> h1

	vsrd		$ACC4,$H4,$_26
	vsrd		$ACC1,$H1,$_26
	vand		$H4,$H4,$mask26
	vand		$H1,$H1,$mask26
	vaddudm		$H0,$H0,$ACC4
	vaddudm		$H2,$ACC2,$ACC1		# h1 -> h2

	vsld		$ACC4,$ACC4,$T0		# <<2
	vsrd		$ACC2,$H2,$_26
	vand		$H2,$H2,$mask26
	vaddudm		$H0,$H0,$ACC4		# h4 -> h0
	vaddudm		$H3,$H3,$ACC2		# h2 -> h3

	vsrd		$ACC0,$H0,$_26
	vsrd		$ACC3,$H3,$_26
	vand		$H0,$H0,$mask26
	vand		$H3,$H3,$mask26
	vaddudm		$H1,$H1,$ACC0		# h0 -> h1
	vaddudm		$H4,$H4,$ACC3		# h3 -> h4

	beq		Ldone_vsx

	add		r6,$const,$len

	be?lvx_u	$_4,$x00,$const		# byte swap mask
	lvx_u		$T1,$x00,$inp		# load last partial input block
	lvx_u		$T2,$x10,$inp
	lvx_u		$T3,$x20,$inp
	lvx_u		$T4,$x30,$inp
	be?vperm	$T1,$T1,$T1,$_4
	be?vperm	$T2,$T2,$T2,$_4
	be?vperm	$T3,$T3,$T3,$_4
	be?vperm	$T4,$T4,$T4,$_4

	vpermdi		$I0,$T1,$T2,0b00	# smash input to base 2^26
	vspltisb	$_4,4
	vperm		$I2,$T1,$T2,$I2perm	# 0x...0e0f0001...1e1f1011
	vpermdi		$I3,$T1,$T2,0b11

	vsrd		$I1,$I0,$_26
	vsrd		$I2,$I2,$_4
	vsrd		$I4,$I3,$_40
	vsrd		$I3,$I3,$_14
	vand		$I0,$I0,$mask26
	vand		$I1,$I1,$mask26
	vand		$I2,$I2,$mask26
	vand		$I3,$I3,$mask26

	vpermdi		$T0,$T3,$T4,0b00
	vperm		$T1,$T3,$T4,$I2perm	# 0x...0e0f0001...1e1f1011
	vpermdi		$T2,$T3,$T4,0b11

	lvx_u		$ACC0,$x00,r6
	lvx_u		$ACC1,$x30,r6

	vsrd		$T3,$T0,$_26
	vsrd		$T1,$T1,$_4
	vsrd		$T4,$T2,$_40
	vsrd		$T2,$T2,$_14
	vand		$T0,$T0,$mask26
	vand		$T3,$T3,$mask26
	vand		$T1,$T1,$mask26
	vand		$T2,$T2,$mask26

	# inp[2]:inp[0]:inp[3]:inp[1]
	vmrgow		$I4,$T4,$I4
	vmrgow		$I0,$T0,$I0
	vmrgow		$I1,$T3,$I1
	vmrgow		$I2,$T1,$I2
	vmrgow		$I3,$T2,$I3
	vor		$I4,$I4,$padbits

	vperm		$H0,$H0,$H0,$ACC0	# move hash to right lane
	vand		$I0,$I0,    $ACC1	# mask redundant input lane[s]
	vperm		$H1,$H1,$H1,$ACC0
	vand		$I1,$I1,    $ACC1
	vperm		$H2,$H2,$H2,$ACC0
	vand		$I2,$I2,    $ACC1
	vperm		$H3,$H3,$H3,$ACC0
	vand		$I3,$I3,    $ACC1
	vperm		$H4,$H4,$H4,$ACC0
	vand		$I4,$I4,    $ACC1

	vaddudm		$I0,$I0,$H0		# accumulate hash
	vxor		$H0,$H0,$H0		# wipe hash value
	vaddudm		$I1,$I1,$H1
	vxor		$H1,$H1,$H1
	vaddudm		$I2,$I2,$H2
	vxor		$H2,$H2,$H2
	vaddudm		$I3,$I3,$H3
	vxor		$H3,$H3,$H3
	vaddudm		$I4,$I4,$H4
	vxor		$H4,$H4,$H4

	xor.		$len,$len,$len
	b		Last_vsx

.align	4
Ldone_vsx:
	$POP	r0,`$VSXFRAME+$LRSAVE`($sp)
	li	$x10,4
	li	$x20,8
	li	$x30,12
	li	$x40,16
	stvwx_u	$H0,$x00,$ctx			# store hash
	stvwx_u	$H1,$x10,$ctx
	stvwx_u	$H2,$x20,$ctx
	stvwx_u	$H3,$x30,$ctx
	stvwx_u	$H4,$x40,$ctx

	lwz	r12,`$VSXFRAME-$SIZE_T*5-4`($sp)# pull vrsave
	mtlr	r0
	li	r10,`15+$LOCALS+128`
	li	r11,`31+$LOCALS+128`
	mtspr	256,r12				# restore vrsave
	lvx	v20,r10,$sp
	addi	r10,r10,32
	lvx	v21,r11,$sp
	addi	r11,r11,32
	lvx	v22,r10,$sp
	addi	r10,r10,32
	lvx	v23,r11,$sp
	addi	r11,r11,32
	lvx	v24,r10,$sp
	addi	r10,r10,32
	lvx	v25,r11,$sp
	addi	r11,r11,32
	lvx	v26,r10,$sp
	addi	r10,r10,32
	lvx	v27,r11,$sp
	addi	r11,r11,32
	lvx	v28,r10,$sp
	addi	r10,r10,32
	lvx	v29,r11,$sp
	addi	r11,r11,32
	lvx	v30,r10,$sp
	lvx	v31,r11,$sp
	$POP	r27,`$VSXFRAME-$SIZE_T*5`($sp)
	$POP	r28,`$VSXFRAME-$SIZE_T*4`($sp)
	$POP	r29,`$VSXFRAME-$SIZE_T*3`($sp)
	$POP	r30,`$VSXFRAME-$SIZE_T*2`($sp)
	$POP	r31,`$VSXFRAME-$SIZE_T*1`($sp)
	addi	$sp,$sp,$VSXFRAME
	blr
	.long	0
	.byte	0,12,0x04,1,0x80,5,4,0
	.long	0
.size	__poly1305_blocks_vsx,.-__poly1305_blocks_vsx

.align	6
LPICmeup:
	mflr	r0
	bcl	20,31,\$+4
	mflr	$const      # vvvvvv "distance" between . and 1st data entry
	addi	$const,$const,`64-8`
	mtlr	r0
	blr
	.long	0
	.byte	0,12,0x14,0,0,0,0,0
	.space	`64-9*4`

.quad	0x0000000003ffffff,0x0000000003ffffff	# mask26
.quad	0x000000000000001a,0x000000000000001a	# _26
.quad	0x0000000000000028,0x0000000000000028	# _40
.quad	0x000000000e0f0001,0x000000001e1f1011	# I2perm
.quad	0x0100000001000000,0x0100000001000000	# padbits
.quad	0x0706050403020100,0x0f0e0d0c0b0a0908	# byte swap for big-endian

.quad	0x0000000000000000,0x0000000004050607	# magic tail masks
.quad	0x0405060700000000,0x0000000000000000
.quad	0x0000000000000000,0x0405060700000000

.quad	0xffffffff00000000,0xffffffffffffffff
.quad	0xffffffff00000000,0xffffffff00000000
.quad	0x0000000000000000,0xffffffff00000000
___
}}}
$code.=<<___;
.asciz	"Poly1305 for PPC, CRYPTOGAMS by \@dot-asm"
___

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/ge;

	# instructions prefixed with '?' are endian-specific and need
	# to be adjusted accordingly...
	if ($flavour !~ /le$/) {	# big-endian
	    s/be\?//		or
	    s/le\?/#le#/
	} else {			# little-endian
	    s/le\?//		or
	    s/be\?/#be#/
	}

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT: $!";
