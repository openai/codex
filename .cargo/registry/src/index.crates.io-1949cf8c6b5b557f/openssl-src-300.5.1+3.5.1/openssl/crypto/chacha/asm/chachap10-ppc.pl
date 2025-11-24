#! /usr/bin/env perl
# Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
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
# October 2015
#
# ChaCha20 for PowerPC/AltiVec.
#
# June 2018
#
# Add VSX 2.07 code path. Original 3xAltiVec+1xIALU is well-suited for
# processors that can't issue more than one vector instruction per
# cycle. But POWER8 (and POWER9) can issue a pair, and vector-only 4x
# interleave would perform better. Incidentally PowerISA 2.07 (first
# implemented by POWER8) defined new usable instructions, hence 4xVSX
# code path...
#
# Performance in cycles per byte out of large buffer.
#
#			IALU/gcc-4.x    3xAltiVec+1xIALU	4xVSX
#
# Freescale e300	13.6/+115%	-			-
# PPC74x0/G4e		6.81/+310%	3.81			-
# PPC970/G5		9.29/+160%	?			-
# POWER7		8.62/+61%	3.35			-
# POWER8		8.70/+51%	2.91			2.09
# POWER9		8.80/+29%	4.44(*)			2.45(**)
#
# (*)	this is trade-off result, it's possible to improve it, but
#	then it would negatively affect all others;
# (**)	POWER9 seems to be "allergic" to mixing vector and integer
#	instructions, which is why switch to vector-only code pays
#	off that much;

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

if ($flavour =~ /64/) {
	$SIZE_T	=8;
	$LRSAVE	=2*$SIZE_T;
	$STU	="stdu";
	$POP	="ld";
	$PUSH	="std";
	$UCMP	="cmpld";
} elsif ($flavour =~ /32/) {
	$SIZE_T	=4;
	$LRSAVE	=$SIZE_T;
	$STU	="stwu";
	$POP	="lwz";
	$PUSH	="stw";
	$UCMP	="cmplw";
} else { die "nonsense $flavour"; }

$LITTLE_ENDIAN = ($flavour=~/le$/) ? 1 : 0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}ppc-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/ppc-xlate.pl" and -f $xlate) or
die "can't locate ppc-xlate.pl";

open STDOUT,"| $^X $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";

$LOCALS=6*$SIZE_T;
$FRAME=$LOCALS+64+18*$SIZE_T;	# 64 is for local variables

sub AUTOLOAD()		# thunk [simplified] x86-style perlasm
{ my $opcode = $AUTOLOAD; $opcode =~ s/.*:://; $opcode =~ s/_/\./;
    $code .= "\t$opcode\t".join(',',@_)."\n";
}

my $sp = "r1";

my ($out,$inp,$len,$key,$ctr) = map("r$_",(3..7));


{{{
my ($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
    $xc0,$xc1,$xc2,$xc3, $xd0,$xd1,$xd2,$xd3) = map("v$_",(0..15));
my @K = map("v$_",(16..19));
my $CTR = "v26";
my ($xt0,$xt1,$xt2,$xt3) = map("v$_",(27..30));
my ($sixteen,$twelve,$eight,$seven) = ($xt0,$xt1,$xt2,$xt3);
my $beperm = "v31";

my ($x00,$x10,$x20,$x30) = (0, map("r$_",(8..10)));

my $FRAME=$LOCALS+64+7*16;	# 7*16 is for v26-v31 offload


sub VSX_lane_ROUND_4x {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my @x=map("\"v$_\"",(0..15));

	(
	"&vadduwm	(@x[$a0],@x[$a0],@x[$b0])",	# Q1
	 "&vadduwm	(@x[$a1],@x[$a1],@x[$b1])",	# Q2
	  "&vadduwm	(@x[$a2],@x[$a2],@x[$b2])",	# Q3
	   "&vadduwm	(@x[$a3],@x[$a3],@x[$b3])",	# Q4
	"&vxor		(@x[$d0],@x[$d0],@x[$a0])",
	 "&vxor		(@x[$d1],@x[$d1],@x[$a1])",
	  "&vxor	(@x[$d2],@x[$d2],@x[$a2])",
	   "&vxor	(@x[$d3],@x[$d3],@x[$a3])",
	"&vrlw		(@x[$d0],@x[$d0],'$sixteen')",
	 "&vrlw		(@x[$d1],@x[$d1],'$sixteen')",
	  "&vrlw	(@x[$d2],@x[$d2],'$sixteen')",
	   "&vrlw	(@x[$d3],@x[$d3],'$sixteen')",

	"&vadduwm	(@x[$c0],@x[$c0],@x[$d0])",
	 "&vadduwm	(@x[$c1],@x[$c1],@x[$d1])",
	  "&vadduwm	(@x[$c2],@x[$c2],@x[$d2])",
	   "&vadduwm	(@x[$c3],@x[$c3],@x[$d3])",
	"&vxor		(@x[$b0],@x[$b0],@x[$c0])",
	 "&vxor		(@x[$b1],@x[$b1],@x[$c1])",
	  "&vxor	(@x[$b2],@x[$b2],@x[$c2])",
	   "&vxor	(@x[$b3],@x[$b3],@x[$c3])",
	"&vrlw		(@x[$b0],@x[$b0],'$twelve')",
	 "&vrlw		(@x[$b1],@x[$b1],'$twelve')",
	  "&vrlw	(@x[$b2],@x[$b2],'$twelve')",
	   "&vrlw	(@x[$b3],@x[$b3],'$twelve')",

	"&vadduwm	(@x[$a0],@x[$a0],@x[$b0])",
	 "&vadduwm	(@x[$a1],@x[$a1],@x[$b1])",
	  "&vadduwm	(@x[$a2],@x[$a2],@x[$b2])",
	   "&vadduwm	(@x[$a3],@x[$a3],@x[$b3])",
	"&vxor		(@x[$d0],@x[$d0],@x[$a0])",
	 "&vxor		(@x[$d1],@x[$d1],@x[$a1])",
	  "&vxor	(@x[$d2],@x[$d2],@x[$a2])",
	   "&vxor	(@x[$d3],@x[$d3],@x[$a3])",
	"&vrlw		(@x[$d0],@x[$d0],'$eight')",
	 "&vrlw		(@x[$d1],@x[$d1],'$eight')",
	  "&vrlw	(@x[$d2],@x[$d2],'$eight')",
	   "&vrlw	(@x[$d3],@x[$d3],'$eight')",

	"&vadduwm	(@x[$c0],@x[$c0],@x[$d0])",
	 "&vadduwm	(@x[$c1],@x[$c1],@x[$d1])",
	  "&vadduwm	(@x[$c2],@x[$c2],@x[$d2])",
	   "&vadduwm	(@x[$c3],@x[$c3],@x[$d3])",
	"&vxor		(@x[$b0],@x[$b0],@x[$c0])",
	 "&vxor		(@x[$b1],@x[$b1],@x[$c1])",
	  "&vxor	(@x[$b2],@x[$b2],@x[$c2])",
	   "&vxor	(@x[$b3],@x[$b3],@x[$c3])",
	"&vrlw		(@x[$b0],@x[$b0],'$seven')",
	 "&vrlw		(@x[$b1],@x[$b1],'$seven')",
	  "&vrlw	(@x[$b2],@x[$b2],'$seven')",
	   "&vrlw	(@x[$b3],@x[$b3],'$seven')"
	);
}

$code.=<<___;

.globl	.ChaCha20_ctr32_vsx_p10
.align	5
.ChaCha20_ctr32_vsx_p10:
	${UCMP}i $len,255
	ble	.Not_greater_than_8x
	b	ChaCha20_ctr32_vsx_8x
.Not_greater_than_8x:
	$STU	$sp,-$FRAME($sp)
	mflr	r0
	li	r10,`15+$LOCALS+64`
	li	r11,`31+$LOCALS+64`
	mfspr	r12,256
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
	stw	r12,`$FRAME-4`($sp)		# save vrsave
	li	r12,-4096+63
	$PUSH	r0, `$FRAME+$LRSAVE`($sp)
	mtspr	256,r12				# preserve 29 AltiVec registers

	bl	Lconsts				# returns pointer Lsigma in r12
	lvx_4w	@K[0],0,r12			# load sigma
	addi	r12,r12,0x70
	li	$x10,16
	li	$x20,32
	li	$x30,48
	li	r11,64

	lvx_4w	@K[1],0,$key			# load key
	lvx_4w	@K[2],$x10,$key
	lvx_4w	@K[3],0,$ctr			# load counter

	vxor	$xt0,$xt0,$xt0
	lvx_4w	$xt1,r11,r12
	vspltw	$CTR,@K[3],0
	vsldoi	@K[3],@K[3],$xt0,4
	vsldoi	@K[3],$xt0,@K[3],12		# clear @K[3].word[0]
	vadduwm	$CTR,$CTR,$xt1

	be?lvsl	$beperm,0,$x10			# 0x00..0f
	be?vspltisb $xt0,3			# 0x03..03
	be?vxor	$beperm,$beperm,$xt0		# swap bytes within words

	li	r0,10				# inner loop counter
	mtctr	r0
	b	Loop_outer_vsx

.align	5
Loop_outer_vsx:
	lvx	$xa0,$x00,r12			# load [smashed] sigma
	lvx	$xa1,$x10,r12
	lvx	$xa2,$x20,r12
	lvx	$xa3,$x30,r12

	vspltw	$xb0,@K[1],0			# smash the key
	vspltw	$xb1,@K[1],1
	vspltw	$xb2,@K[1],2
	vspltw	$xb3,@K[1],3

	vspltw	$xc0,@K[2],0
	vspltw	$xc1,@K[2],1
	vspltw	$xc2,@K[2],2
	vspltw	$xc3,@K[2],3

	vmr	$xd0,$CTR			# smash the counter
	vspltw	$xd1,@K[3],1
	vspltw	$xd2,@K[3],2
	vspltw	$xd3,@K[3],3

	vspltisw $sixteen,-16			# synthesize constants
	vspltisw $twelve,12
	vspltisw $eight,8
	vspltisw $seven,7

Loop_vsx_4x:
___
	foreach (&VSX_lane_ROUND_4x(0, 4, 8,12)) { eval; }
	foreach (&VSX_lane_ROUND_4x(0, 5,10,15)) { eval; }
$code.=<<___;

	bdnz	Loop_vsx_4x

	vadduwm	$xd0,$xd0,$CTR

	vmrgew	$xt0,$xa0,$xa1			# transpose data
	vmrgew	$xt1,$xa2,$xa3
	vmrgow	$xa0,$xa0,$xa1
	vmrgow	$xa2,$xa2,$xa3
	vmrgew	$xt2,$xb0,$xb1
	vmrgew	$xt3,$xb2,$xb3
	vpermdi	$xa1,$xa0,$xa2,0b00
	vpermdi	$xa3,$xa0,$xa2,0b11
	vpermdi	$xa0,$xt0,$xt1,0b00
	vpermdi	$xa2,$xt0,$xt1,0b11

	vmrgow	$xb0,$xb0,$xb1
	vmrgow	$xb2,$xb2,$xb3
	vmrgew	$xt0,$xc0,$xc1
	vmrgew	$xt1,$xc2,$xc3
	vpermdi	$xb1,$xb0,$xb2,0b00
	vpermdi	$xb3,$xb0,$xb2,0b11
	vpermdi	$xb0,$xt2,$xt3,0b00
	vpermdi	$xb2,$xt2,$xt3,0b11

	vmrgow	$xc0,$xc0,$xc1
	vmrgow	$xc2,$xc2,$xc3
	vmrgew	$xt2,$xd0,$xd1
	vmrgew	$xt3,$xd2,$xd3
	vpermdi	$xc1,$xc0,$xc2,0b00
	vpermdi	$xc3,$xc0,$xc2,0b11
	vpermdi	$xc0,$xt0,$xt1,0b00
	vpermdi	$xc2,$xt0,$xt1,0b11

	vmrgow	$xd0,$xd0,$xd1
	vmrgow	$xd2,$xd2,$xd3
	vspltisw $xt0,4
	vadduwm  $CTR,$CTR,$xt0		# next counter value
	vpermdi	$xd1,$xd0,$xd2,0b00
	vpermdi	$xd3,$xd0,$xd2,0b11
	vpermdi	$xd0,$xt2,$xt3,0b00
	vpermdi	$xd2,$xt2,$xt3,0b11

	vadduwm	$xa0,$xa0,@K[0]
	vadduwm	$xb0,$xb0,@K[1]
	vadduwm	$xc0,$xc0,@K[2]
	vadduwm	$xd0,$xd0,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx

	vadduwm	$xa0,$xa1,@K[0]
	vadduwm	$xb0,$xb1,@K[1]
	vadduwm	$xc0,$xc1,@K[2]
	vadduwm	$xd0,$xd1,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx

	vadduwm	$xa0,$xa2,@K[0]
	vadduwm	$xb0,$xb2,@K[1]
	vadduwm	$xc0,$xc2,@K[2]
	vadduwm	$xd0,$xd2,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx

	vadduwm	$xa0,$xa3,@K[0]
	vadduwm	$xb0,$xb3,@K[1]
	vadduwm	$xc0,$xc3,@K[2]
	vadduwm	$xd0,$xd3,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	mtctr	r0
	bne	Loop_outer_vsx

Ldone_vsx:
	lwz	r12,`$FRAME-4`($sp)		# pull vrsave
	li	r10,`15+$LOCALS+64`
	li	r11,`31+$LOCALS+64`
	$POP	r0, `$FRAME+$LRSAVE`($sp)
	mtspr	256,r12				# restore vrsave
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
	mtlr	r0
	addi	$sp,$sp,$FRAME
	blr

.align	4
Ltail_vsx:
	addi	r11,$sp,$LOCALS
	mtctr	$len
	stvx_4w	$xa0,$x00,r11			# offload block to stack
	stvx_4w	$xb0,$x10,r11
	stvx_4w	$xc0,$x20,r11
	stvx_4w	$xd0,$x30,r11
	subi	r12,r11,1			# prepare for *++ptr
	subi	$inp,$inp,1
	subi	$out,$out,1

Loop_tail_vsx:
	lbzu	r6,1(r12)
	lbzu	r7,1($inp)
	xor	r6,r6,r7
	stbu	r6,1($out)
	bdnz	Loop_tail_vsx

	stvx_4w	$K[0],$x00,r11			# wipe copy of the block
	stvx_4w	$K[0],$x10,r11
	stvx_4w	$K[0],$x20,r11
	stvx_4w	$K[0],$x30,r11

	b	Ldone_vsx
	.long	0
	.byte	0,12,0x04,1,0x80,0,5,0
	.long	0
.size	.ChaCha20_ctr32_vsx_p10,.-.ChaCha20_ctr32_vsx_p10
___
}}}

##This is 8 block in parallel implementation. The heart of chacha round uses vector instruction that has access to 
# vsr[32+X]. To perform the 8 parallel block we tend to use all 32 register to hold the 8 block info.
# WE need to store few register value on side, so we can use VSR{32+X} for few vector instructions used in round op and hold intermediate value.
# WE use the VSR[0]-VSR[31] for holding intermediate value and perform 8 block in parallel.
#
{{{
#### ($out,$inp,$len,$key,$ctr) = map("r$_",(3..7));
my ($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
    $xc0,$xc1,$xc2,$xc3, $xd0,$xd1,$xd2,$xd3,
    $xa4,$xa5,$xa6,$xa7, $xb4,$xb5,$xb6,$xb7,
    $xc4,$xc5,$xc6,$xc7, $xd4,$xd5,$xd6,$xd7) = map("v$_",(0..31));
my ($xcn4,$xcn5,$xcn6,$xcn7, $xdn4,$xdn5,$xdn6,$xdn7) = map("v$_",(8..15));
my ($xan0,$xbn0,$xcn0,$xdn0) = map("v$_",(0..3));
my @K = map("v$_",27,(24..26));
my ($xt0,$xt1,$xt2,$xt3,$xt4) = map("v$_",23,(28..31));
my $xr0 = "v4";
my $CTR0 = "v22";
my $CTR1 = "v5";
my $beperm = "v31";
my ($x00,$x10,$x20,$x30) = (0, map("r$_",(8..10)));
my ($xv0,$xv1,$xv2,$xv3,$xv4,$xv5,$xv6,$xv7) = map("v$_",(0..7));
my ($xv8,$xv9,$xv10,$xv11,$xv12,$xv13,$xv14,$xv15,$xv16,$xv17) = map("v$_",(8..17));
my ($xv18,$xv19,$xv20,$xv21) = map("v$_",(18..21));
my ($xv22,$xv23,$xv24,$xv25,$xv26) = map("v$_",(22..26));

my $FRAME=$LOCALS+64+9*16;	# 8*16 is for v24-v31 offload

sub VSX_lane_ROUND_8x {
my ($a0,$b0,$c0,$d0,$a4,$b4,$c4,$d4)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my ($a5,$b5,$c5,$d5)=map(($_&~3)+(($_+1)&3),($a4,$b4,$c4,$d4));
my ($a6,$b6,$c6,$d6)=map(($_&~3)+(($_+1)&3),($a5,$b5,$c5,$d5));
my ($a7,$b7,$c7,$d7)=map(($_&~3)+(($_+1)&3),($a6,$b6,$c6,$d6));
my ($xv8,$xv9,$xv10,$xv11,$xv12,$xv13,$xv14,$xv15,$xv16,$xv17) = map("\"v$_\"",(8..17));
my @x=map("\"v$_\"",(0..31));

	(
	"&vxxlor        ($xv15 ,@x[$c7],@x[$c7])",      #copy v30 to v13
	"&vxxlorc       (@x[$c7], $xv9,$xv9)",

	"&vadduwm	(@x[$a0],@x[$a0],@x[$b0])",	# Q1
	 "&vadduwm	(@x[$a1],@x[$a1],@x[$b1])",	# Q2
	  "&vadduwm	(@x[$a2],@x[$a2],@x[$b2])",	# Q3
	   "&vadduwm	(@x[$a3],@x[$a3],@x[$b3])",	# Q4
	"&vadduwm	(@x[$a4],@x[$a4],@x[$b4])",	# Q1
	 "&vadduwm	(@x[$a5],@x[$a5],@x[$b5])",	# Q2
	  "&vadduwm	(@x[$a6],@x[$a6],@x[$b6])",	# Q3
	   "&vadduwm	(@x[$a7],@x[$a7],@x[$b7])",	# Q4

	"&vxor		(@x[$d0],@x[$d0],@x[$a0])",
	 "&vxor		(@x[$d1],@x[$d1],@x[$a1])",
	  "&vxor	(@x[$d2],@x[$d2],@x[$a2])",
	   "&vxor	(@x[$d3],@x[$d3],@x[$a3])",
	"&vxor		(@x[$d4],@x[$d4],@x[$a4])",
	 "&vxor		(@x[$d5],@x[$d5],@x[$a5])",
	  "&vxor	(@x[$d6],@x[$d6],@x[$a6])",
	   "&vxor	(@x[$d7],@x[$d7],@x[$a7])",

	"&vrlw		(@x[$d0],@x[$d0],@x[$c7])",
	 "&vrlw		(@x[$d1],@x[$d1],@x[$c7])",
	  "&vrlw	(@x[$d2],@x[$d2],@x[$c7])",
	   "&vrlw	(@x[$d3],@x[$d3],@x[$c7])",
	"&vrlw		(@x[$d4],@x[$d4],@x[$c7])",
	 "&vrlw		(@x[$d5],@x[$d5],@x[$c7])",
	  "&vrlw	(@x[$d6],@x[$d6],@x[$c7])",
	   "&vrlw	(@x[$d7],@x[$d7],@x[$c7])",

	"&vxxlor        ($xv13 ,@x[$a7],@x[$a7])",
	"&vxxlorc       (@x[$c7], $xv15,$xv15)",
	"&vxxlorc       (@x[$a7], $xv10,$xv10)",

	"&vadduwm	(@x[$c0],@x[$c0],@x[$d0])",
	 "&vadduwm	(@x[$c1],@x[$c1],@x[$d1])",
	  "&vadduwm	(@x[$c2],@x[$c2],@x[$d2])",
	   "&vadduwm	(@x[$c3],@x[$c3],@x[$d3])",
	"&vadduwm	(@x[$c4],@x[$c4],@x[$d4])",
	 "&vadduwm	(@x[$c5],@x[$c5],@x[$d5])",
	  "&vadduwm	(@x[$c6],@x[$c6],@x[$d6])",
	   "&vadduwm	(@x[$c7],@x[$c7],@x[$d7])",

	"&vxor		(@x[$b0],@x[$b0],@x[$c0])",
	 "&vxor		(@x[$b1],@x[$b1],@x[$c1])",
	  "&vxor	(@x[$b2],@x[$b2],@x[$c2])",
	   "&vxor	(@x[$b3],@x[$b3],@x[$c3])",
	"&vxor		(@x[$b4],@x[$b4],@x[$c4])",
	 "&vxor		(@x[$b5],@x[$b5],@x[$c5])",
	  "&vxor	(@x[$b6],@x[$b6],@x[$c6])",
	   "&vxor	(@x[$b7],@x[$b7],@x[$c7])",

	"&vrlw		(@x[$b0],@x[$b0],@x[$a7])",
	 "&vrlw		(@x[$b1],@x[$b1],@x[$a7])",
	  "&vrlw	(@x[$b2],@x[$b2],@x[$a7])",
	   "&vrlw	(@x[$b3],@x[$b3],@x[$a7])",
	"&vrlw		(@x[$b4],@x[$b4],@x[$a7])",
	 "&vrlw		(@x[$b5],@x[$b5],@x[$a7])",
	  "&vrlw	(@x[$b6],@x[$b6],@x[$a7])",
	   "&vrlw	(@x[$b7],@x[$b7],@x[$a7])",

	"&vxxlorc       (@x[$a7], $xv13,$xv13)",
	"&vxxlor	($xv15 ,@x[$c7],@x[$c7])",                 
	"&vxxlorc       (@x[$c7], $xv11,$xv11)",


	"&vadduwm	(@x[$a0],@x[$a0],@x[$b0])",
	 "&vadduwm	(@x[$a1],@x[$a1],@x[$b1])",
	  "&vadduwm	(@x[$a2],@x[$a2],@x[$b2])",
	   "&vadduwm	(@x[$a3],@x[$a3],@x[$b3])",
	"&vadduwm	(@x[$a4],@x[$a4],@x[$b4])",
	 "&vadduwm	(@x[$a5],@x[$a5],@x[$b5])",
	  "&vadduwm	(@x[$a6],@x[$a6],@x[$b6])",
	   "&vadduwm	(@x[$a7],@x[$a7],@x[$b7])",

	"&vxor		(@x[$d0],@x[$d0],@x[$a0])",
	 "&vxor		(@x[$d1],@x[$d1],@x[$a1])",
	  "&vxor	(@x[$d2],@x[$d2],@x[$a2])",
	   "&vxor	(@x[$d3],@x[$d3],@x[$a3])",
	"&vxor		(@x[$d4],@x[$d4],@x[$a4])",
	 "&vxor		(@x[$d5],@x[$d5],@x[$a5])",
	  "&vxor	(@x[$d6],@x[$d6],@x[$a6])",
	   "&vxor	(@x[$d7],@x[$d7],@x[$a7])",

	"&vrlw		(@x[$d0],@x[$d0],@x[$c7])",
	 "&vrlw		(@x[$d1],@x[$d1],@x[$c7])",
	  "&vrlw	(@x[$d2],@x[$d2],@x[$c7])",
	   "&vrlw	(@x[$d3],@x[$d3],@x[$c7])",
	"&vrlw		(@x[$d4],@x[$d4],@x[$c7])",
	 "&vrlw		(@x[$d5],@x[$d5],@x[$c7])",
	  "&vrlw	(@x[$d6],@x[$d6],@x[$c7])",
	   "&vrlw	(@x[$d7],@x[$d7],@x[$c7])",

	"&vxxlorc       (@x[$c7], $xv15,$xv15)",
	"&vxxlor        ($xv13 ,@x[$a7],@x[$a7])",               
	"&vxxlorc       (@x[$a7], $xv12,$xv12)",

	"&vadduwm	(@x[$c0],@x[$c0],@x[$d0])",
	 "&vadduwm	(@x[$c1],@x[$c1],@x[$d1])",
	  "&vadduwm	(@x[$c2],@x[$c2],@x[$d2])",
	   "&vadduwm	(@x[$c3],@x[$c3],@x[$d3])",
	"&vadduwm	(@x[$c4],@x[$c4],@x[$d4])",
	 "&vadduwm	(@x[$c5],@x[$c5],@x[$d5])",
	  "&vadduwm	(@x[$c6],@x[$c6],@x[$d6])",
	   "&vadduwm	(@x[$c7],@x[$c7],@x[$d7])",
	"&vxor		(@x[$b0],@x[$b0],@x[$c0])",
	 "&vxor		(@x[$b1],@x[$b1],@x[$c1])",
	  "&vxor	(@x[$b2],@x[$b2],@x[$c2])",
	   "&vxor	(@x[$b3],@x[$b3],@x[$c3])",
	"&vxor		(@x[$b4],@x[$b4],@x[$c4])",
	 "&vxor		(@x[$b5],@x[$b5],@x[$c5])",
	  "&vxor	(@x[$b6],@x[$b6],@x[$c6])",
	   "&vxor	(@x[$b7],@x[$b7],@x[$c7])",
	"&vrlw		(@x[$b0],@x[$b0],@x[$a7])",
	 "&vrlw		(@x[$b1],@x[$b1],@x[$a7])",
	  "&vrlw	(@x[$b2],@x[$b2],@x[$a7])",
	   "&vrlw	(@x[$b3],@x[$b3],@x[$a7])",
	"&vrlw		(@x[$b4],@x[$b4],@x[$a7])",
	 "&vrlw		(@x[$b5],@x[$b5],@x[$a7])",
	  "&vrlw	(@x[$b6],@x[$b6],@x[$a7])",
	   "&vrlw	(@x[$b7],@x[$b7],@x[$a7])",

	"&vxxlorc       (@x[$a7], $xv13,$xv13)",
	);
}

$code.=<<___;

.globl	.ChaCha20_ctr32_vsx_8x
.align	5
.ChaCha20_ctr32_vsx_8x:
	$STU	$sp,-$FRAME($sp)
	mflr	r0
	li	r10,`15+$LOCALS+64`
	li	r11,`31+$LOCALS+64`
	mfspr	r12,256
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
	stw	r12,`$FRAME-4`($sp)		# save vrsave
	li	r12,-4096+63
	$PUSH	r0, `$FRAME+$LRSAVE`($sp)
	mtspr	256,r12				# preserve 29 AltiVec registers

	bl	Lconsts				# returns pointer Lsigma in r12

	lvx_4w	@K[0],0,r12			# load sigma
	addi	r12,r12,0x70
	li	$x10,16
	li	$x20,32
	li	$x30,48
	li	r11,64

	vspltisw $xa4,-16			# synthesize constants
	vspltisw $xb4,12			# synthesize constants
	vspltisw $xc4,8			# synthesize constants
	vspltisw $xd4,7			# synthesize constants

	lvx	$xa0,$x00,r12			# load [smashed] sigma
	lvx	$xa1,$x10,r12
	lvx	$xa2,$x20,r12
	lvx	$xa3,$x30,r12

	vxxlor	$xv9   ,$xa4,$xa4               #save shift val in vr9-12
	vxxlor	$xv10  ,$xb4,$xb4
	vxxlor	$xv11  ,$xc4,$xc4
	vxxlor	$xv12  ,$xd4,$xd4
	vxxlor	$xv22  ,$xa0,$xa0               #save sigma in vr22-25
	vxxlor	$xv23  ,$xa1,$xa1
	vxxlor	$xv24  ,$xa2,$xa2
	vxxlor	$xv25  ,$xa3,$xa3

	lvx_4w	@K[1],0,$key			# load key
	lvx_4w	@K[2],$x10,$key
	lvx_4w	@K[3],0,$ctr			# load counter
	vspltisw $xt3,4


	vxor	$xt2,$xt2,$xt2
	lvx_4w	$xt1,r11,r12
	vspltw	$xa2,@K[3],0			#save the original count after spltw
	vsldoi	@K[3],@K[3],$xt2,4
	vsldoi	@K[3],$xt2,@K[3],12		# clear @K[3].word[0]
	vadduwm	$xt1,$xa2,$xt1
	vadduwm $xt3,$xt1,$xt3     		# next counter value
	vspltw	$xa0,@K[2],2                    # save the K[2] spltw 2 and save v8.

	be?lvsl	  $beperm,0,$x10			# 0x00..0f
	be?vspltisb $xt0,3			# 0x03..03
	be?vxor   $beperm,$beperm,$xt0		# swap bytes within words
	be?vxxlor $xv26 ,$beperm,$beperm

	vxxlor	$xv0 ,@K[0],@K[0]               # K0,k1,k2 to vr0,1,2
	vxxlor	$xv1 ,@K[1],@K[1]
	vxxlor	$xv2 ,@K[2],@K[2]
	vxxlor	$xv3 ,@K[3],@K[3]
	vxxlor	$xv4 ,$xt1,$xt1                #CTR ->4, CTR+4-> 5
	vxxlor	$xv5 ,$xt3,$xt3
	vxxlor	$xv8 ,$xa0,$xa0

	li	r0,10				# inner loop counter
	mtctr	r0
	b	Loop_outer_vsx_8x

.align	5
Loop_outer_vsx_8x:
	vxxlorc	$xa0,$xv22,$xv22	        # load [smashed] sigma
	vxxlorc	$xa1,$xv23,$xv23
	vxxlorc	$xa2,$xv24,$xv24
	vxxlorc	$xa3,$xv25,$xv25
	vxxlorc	$xa4,$xv22,$xv22
	vxxlorc	$xa5,$xv23,$xv23
	vxxlorc	$xa6,$xv24,$xv24
	vxxlorc	$xa7,$xv25,$xv25

	vspltw	$xb0,@K[1],0			# smash the key
	vspltw	$xb1,@K[1],1
	vspltw	$xb2,@K[1],2
	vspltw	$xb3,@K[1],3
	vspltw	$xb4,@K[1],0			# smash the key
	vspltw	$xb5,@K[1],1
	vspltw	$xb6,@K[1],2
	vspltw	$xb7,@K[1],3

	vspltw	$xc0,@K[2],0
	vspltw	$xc1,@K[2],1
	vspltw	$xc2,@K[2],2
	vspltw	$xc3,@K[2],3
	vspltw	$xc4,@K[2],0
	vspltw	$xc7,@K[2],3
	vspltw	$xc5,@K[2],1

	vxxlorc	$xd0,$xv4,$xv4			# smash the counter
	vspltw	$xd1,@K[3],1
	vspltw	$xd2,@K[3],2
	vspltw	$xd3,@K[3],3
	vxxlorc	$xd4,$xv5,$xv5			# smash the counter
	vspltw	$xd5,@K[3],1
	vspltw	$xd6,@K[3],2
	vspltw	$xd7,@K[3],3
	vxxlorc	$xc6,$xv8,$xv8                  #copy of vlspt k[2],2 is in v8.v26 ->k[3] so need to wait until k3 is done

Loop_vsx_8x:
___
	foreach (&VSX_lane_ROUND_8x(0,4, 8,12,16,20,24,28)) { eval; }
	foreach (&VSX_lane_ROUND_8x(0,5,10,15,16,21,26,31)) { eval; }
$code.=<<___;

	bdnz	        Loop_vsx_8x
	vxxlor	        $xv13 ,$xd4,$xd4                # save the register vr24-31
	vxxlor	        $xv14 ,$xd5,$xd5                #
	vxxlor	        $xv15 ,$xd6,$xd6                #
	vxxlor	        $xv16 ,$xd7,$xd7                #

	vxxlor	        $xv18 ,$xc4,$xc4                #
	vxxlor	        $xv19 ,$xc5,$xc5                #
	vxxlor	        $xv20 ,$xc6,$xc6                #
	vxxlor	        $xv21 ,$xc7,$xc7                #

	vxxlor	        $xv6  ,$xb6,$xb6                # save vr23, so we get 8 regs
	vxxlor	        $xv7  ,$xb7,$xb7                # save vr23, so we get 8 regs
	be?vxxlorc      $beperm,$xv26,$xv26             # copy back the beperm.

	vxxlorc	   @K[0],$xv0,$xv0                #27
	vxxlorc	   @K[1],$xv1,$xv1 		  #24
	vxxlorc	   @K[2],$xv2,$xv2		  #25
	vxxlorc	   @K[3],$xv3,$xv3		  #26
	vxxlorc	   $CTR0,$xv4,$xv4
###changing to vertical

	vmrgew	$xt0,$xa0,$xa1			# transpose data
	vmrgew	$xt1,$xa2,$xa3
	vmrgow	$xa0,$xa0,$xa1
	vmrgow	$xa2,$xa2,$xa3

	vmrgew	$xt2,$xb0,$xb1
	vmrgew	$xt3,$xb2,$xb3
	vmrgow	$xb0,$xb0,$xb1
	vmrgow	$xb2,$xb2,$xb3

	vadduwm	$xd0,$xd0,$CTR0

	vpermdi	$xa1,$xa0,$xa2,0b00
	vpermdi	$xa3,$xa0,$xa2,0b11
	vpermdi	$xa0,$xt0,$xt1,0b00
	vpermdi	$xa2,$xt0,$xt1,0b11
	vpermdi	$xb1,$xb0,$xb2,0b00
	vpermdi	$xb3,$xb0,$xb2,0b11
	vpermdi	$xb0,$xt2,$xt3,0b00
	vpermdi	$xb2,$xt2,$xt3,0b11

	vmrgew	$xt0,$xc0,$xc1
	vmrgew	$xt1,$xc2,$xc3
	vmrgow	$xc0,$xc0,$xc1
	vmrgow	$xc2,$xc2,$xc3
	vmrgew	$xt2,$xd0,$xd1
	vmrgew	$xt3,$xd2,$xd3
	vmrgow	$xd0,$xd0,$xd1
	vmrgow	$xd2,$xd2,$xd3

	vpermdi	$xc1,$xc0,$xc2,0b00
	vpermdi	$xc3,$xc0,$xc2,0b11
	vpermdi	$xc0,$xt0,$xt1,0b00
	vpermdi	$xc2,$xt0,$xt1,0b11
	vpermdi	$xd1,$xd0,$xd2,0b00
	vpermdi	$xd3,$xd0,$xd2,0b11
	vpermdi	$xd0,$xt2,$xt3,0b00
	vpermdi	$xd2,$xt2,$xt3,0b11

	vspltisw $xt0,8
	vadduwm  $CTR0,$CTR0,$xt0		# next counter value
	vxxlor	 $xv4 ,$CTR0,$CTR0	        #CTR+4-> 5

	vadduwm	$xa0,$xa0,@K[0]
	vadduwm	$xb0,$xb0,@K[1]
	vadduwm	$xc0,$xc0,@K[2]
	vadduwm	$xd0,$xd0,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xa0,$xa1,@K[0]
	vadduwm	$xb0,$xb1,@K[1]
	vadduwm	$xc0,$xc1,@K[2]
	vadduwm	$xd0,$xd1,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xa0,$xa2,@K[0]
	vadduwm	$xb0,$xb2,@K[1]
	vadduwm	$xc0,$xc2,@K[2]
	vadduwm	$xd0,$xd2,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xa0,$xa3,@K[0]
	vadduwm	$xb0,$xb3,@K[1]
	vadduwm	$xc0,$xc3,@K[2]
	vadduwm	$xd0,$xd3,@K[3]

	be?vperm $xa0,$xa0,$xa0,$beperm
	be?vperm $xb0,$xb0,$xb0,$beperm
	be?vperm $xc0,$xc0,$xc0,$beperm
	be?vperm $xd0,$xd0,$xd0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x

	lvx_4w	$xt0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xt0,$xt0,$xa0
	vxor	$xt1,$xt1,$xb0
	vxor	$xt2,$xt2,$xc0
	vxor	$xt3,$xt3,$xd0

	stvx_4w	$xt0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

#blk4-7: 24:31 remain the same as we can use the same logic above . Reg a4-b7 remain same.Load c4,d7--> position 8-15.we can reuse vr24-31.
#VR0-3 : are used to load temp value, vr4 --> as xr0 instead of xt0.

	vxxlorc	   $CTR1 ,$xv5,$xv5

	vxxlorc	   $xcn4 ,$xv18,$xv18
	vxxlorc	   $xcn5 ,$xv19,$xv19
	vxxlorc	   $xcn6 ,$xv20,$xv20
	vxxlorc	   $xcn7 ,$xv21,$xv21

	vxxlorc	   $xdn4 ,$xv13,$xv13
	vxxlorc	   $xdn5 ,$xv14,$xv14
	vxxlorc	   $xdn6 ,$xv15,$xv15
	vxxlorc	   $xdn7 ,$xv16,$xv16
	vadduwm	   $xdn4,$xdn4,$CTR1

	vxxlorc	   $xb6 ,$xv6,$xv6
	vxxlorc	   $xb7 ,$xv7,$xv7
#use xa1->xr0, as xt0...in the block 4-7

	vmrgew	$xr0,$xa4,$xa5			# transpose data
	vmrgew	$xt1,$xa6,$xa7
	vmrgow	$xa4,$xa4,$xa5
	vmrgow	$xa6,$xa6,$xa7
	vmrgew	$xt2,$xb4,$xb5
	vmrgew	$xt3,$xb6,$xb7
	vmrgow	$xb4,$xb4,$xb5
	vmrgow	$xb6,$xb6,$xb7

	vpermdi	$xa5,$xa4,$xa6,0b00
	vpermdi	$xa7,$xa4,$xa6,0b11
	vpermdi	$xa4,$xr0,$xt1,0b00
	vpermdi	$xa6,$xr0,$xt1,0b11
	vpermdi	$xb5,$xb4,$xb6,0b00
	vpermdi	$xb7,$xb4,$xb6,0b11
	vpermdi	$xb4,$xt2,$xt3,0b00
	vpermdi	$xb6,$xt2,$xt3,0b11

	vmrgew	$xr0,$xcn4,$xcn5
	vmrgew	$xt1,$xcn6,$xcn7
	vmrgow	$xcn4,$xcn4,$xcn5
	vmrgow	$xcn6,$xcn6,$xcn7
	vmrgew	$xt2,$xdn4,$xdn5
	vmrgew	$xt3,$xdn6,$xdn7
	vmrgow	$xdn4,$xdn4,$xdn5
	vmrgow	$xdn6,$xdn6,$xdn7

	vpermdi	$xcn5,$xcn4,$xcn6,0b00
	vpermdi	$xcn7,$xcn4,$xcn6,0b11
	vpermdi	$xcn4,$xr0,$xt1,0b00
	vpermdi	$xcn6,$xr0,$xt1,0b11
	vpermdi	$xdn5,$xdn4,$xdn6,0b00
	vpermdi	$xdn7,$xdn4,$xdn6,0b11
	vpermdi	$xdn4,$xt2,$xt3,0b00
	vpermdi	$xdn6,$xt2,$xt3,0b11

	vspltisw $xr0,8
	vadduwm  $CTR1,$CTR1,$xr0		# next counter value
	vxxlor	 $xv5 ,$CTR1,$CTR1	        #CTR+4-> 5

	vadduwm	$xan0,$xa4,@K[0]
	vadduwm	$xbn0,$xb4,@K[1]
	vadduwm	$xcn0,$xcn4,@K[2]
	vadduwm	$xdn0,$xdn4,@K[3]

	be?vperm $xan0,$xan0,$xan0,$beperm
	be?vperm $xbn0,$xbn0,$xbn0,$beperm
	be?vperm $xcn0,$xcn0,$xcn0,$beperm
	be?vperm $xdn0,$xdn0,$xdn0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x_1

	lvx_4w	$xr0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xr0,$xr0,$xan0
	vxor	$xt1,$xt1,$xbn0
	vxor	$xt2,$xt2,$xcn0
	vxor	$xt3,$xt3,$xdn0

	stvx_4w	$xr0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xan0,$xa5,@K[0]
	vadduwm	$xbn0,$xb5,@K[1]
	vadduwm	$xcn0,$xcn5,@K[2]
	vadduwm	$xdn0,$xdn5,@K[3]

	be?vperm $xan0,$xan0,$xan0,$beperm
	be?vperm $xbn0,$xbn0,$xbn0,$beperm
	be?vperm $xcn0,$xcn0,$xcn0,$beperm
	be?vperm $xdn0,$xdn0,$xdn0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x_1

	lvx_4w	$xr0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xr0,$xr0,$xan0
	vxor	$xt1,$xt1,$xbn0
	vxor	$xt2,$xt2,$xcn0
	vxor	$xt3,$xt3,$xdn0

	stvx_4w	$xr0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xan0,$xa6,@K[0]
	vadduwm	$xbn0,$xb6,@K[1]
	vadduwm	$xcn0,$xcn6,@K[2]
	vadduwm	$xdn0,$xdn6,@K[3]

	be?vperm $xan0,$xan0,$xan0,$beperm
	be?vperm $xbn0,$xbn0,$xbn0,$beperm
	be?vperm $xcn0,$xcn0,$xcn0,$beperm
	be?vperm $xdn0,$xdn0,$xdn0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x_1

	lvx_4w	$xr0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xr0,$xr0,$xan0
	vxor	$xt1,$xt1,$xbn0
	vxor	$xt2,$xt2,$xcn0
	vxor	$xt3,$xt3,$xdn0

	stvx_4w	$xr0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	vadduwm	$xan0,$xa7,@K[0]
	vadduwm	$xbn0,$xb7,@K[1]
	vadduwm	$xcn0,$xcn7,@K[2]
	vadduwm	$xdn0,$xdn7,@K[3]

	be?vperm $xan0,$xan0,$xan0,$beperm
	be?vperm $xbn0,$xbn0,$xbn0,$beperm
	be?vperm $xcn0,$xcn0,$xcn0,$beperm
	be?vperm $xdn0,$xdn0,$xdn0,$beperm

	${UCMP}i $len,0x40
	blt	Ltail_vsx_8x_1

	lvx_4w	$xr0,$x00,$inp
	lvx_4w	$xt1,$x10,$inp
	lvx_4w	$xt2,$x20,$inp
	lvx_4w	$xt3,$x30,$inp

	vxor	$xr0,$xr0,$xan0
	vxor	$xt1,$xt1,$xbn0
	vxor	$xt2,$xt2,$xcn0
	vxor	$xt3,$xt3,$xdn0

	stvx_4w	$xr0,$x00,$out
	stvx_4w	$xt1,$x10,$out
	addi	$inp,$inp,0x40
	stvx_4w	$xt2,$x20,$out
	subi	$len,$len,0x40
	stvx_4w	$xt3,$x30,$out
	addi	$out,$out,0x40
	beq	Ldone_vsx_8x

	mtctr	r0
	bne	Loop_outer_vsx_8x

Ldone_vsx_8x:
	lwz	r12,`$FRAME-4`($sp)		# pull vrsave
	li	r10,`15+$LOCALS+64`
	li	r11,`31+$LOCALS+64`
	$POP	r0, `$FRAME+$LRSAVE`($sp)
	mtspr	256,r12				# restore vrsave
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
	mtlr	r0
	addi	$sp,$sp,$FRAME
	blr

.align	4
Ltail_vsx_8x:
	addi	r11,$sp,$LOCALS
	mtctr	$len
	stvx_4w	$xa0,$x00,r11			# offload block to stack
	stvx_4w	$xb0,$x10,r11
	stvx_4w	$xc0,$x20,r11
	stvx_4w	$xd0,$x30,r11
	subi	r12,r11,1			# prepare for *++ptr
	subi	$inp,$inp,1
	subi	$out,$out,1
	bl      Loop_tail_vsx_8x
Ltail_vsx_8x_1:
	addi	r11,$sp,$LOCALS
	mtctr	$len
	stvx_4w	$xan0,$x00,r11			# offload block to stack
	stvx_4w	$xbn0,$x10,r11
	stvx_4w	$xcn0,$x20,r11
	stvx_4w	$xdn0,$x30,r11
	subi	r12,r11,1			# prepare for *++ptr
	subi	$inp,$inp,1
	subi	$out,$out,1
        bl      Loop_tail_vsx_8x

Loop_tail_vsx_8x:
	lbzu	r6,1(r12)
	lbzu	r7,1($inp)
	xor	r6,r6,r7
	stbu	r6,1($out)
	bdnz	Loop_tail_vsx_8x

	stvx_4w	$K[0],$x00,r11			# wipe copy of the block
	stvx_4w	$K[0],$x10,r11
	stvx_4w	$K[0],$x20,r11
	stvx_4w	$K[0],$x30,r11

	b	Ldone_vsx_8x
	.long	0
	.byte	0,12,0x04,1,0x80,0,5,0
	.long	0
.size	.ChaCha20_ctr32_vsx_8x,.-.ChaCha20_ctr32_vsx_8x
___
}}}


$code.=<<___;
.align	5
Lconsts:
	mflr	r0
	bcl	20,31,\$+4
	mflr	r12	#vvvvv "distance between . and Lsigma
	addi	r12,r12,`64-8`
	mtlr	r0
	blr
	.long	0
	.byte	0,12,0x14,0,0,0,0,0
	.space	`64-9*4`
Lsigma:
	.long   0x61707865,0x3320646e,0x79622d32,0x6b206574
	.long	1,0,0,0
	.long	2,0,0,0
	.long	3,0,0,0
	.long	4,0,0,0
___
$code.=<<___ 	if ($LITTLE_ENDIAN);
	.long	0x0e0f0c0d,0x0a0b0809,0x06070405,0x02030001
	.long	0x0d0e0f0c,0x090a0b08,0x05060704,0x01020300
___
$code.=<<___ 	if (!$LITTLE_ENDIAN);	# flipped words
	.long	0x02030001,0x06070405,0x0a0b0809,0x0e0f0c0d
	.long	0x01020300,0x05060704,0x090a0b08,0x0d0e0f0c
___
$code.=<<___;
	.long	0x61707865,0x61707865,0x61707865,0x61707865
	.long	0x3320646e,0x3320646e,0x3320646e,0x3320646e
	.long	0x79622d32,0x79622d32,0x79622d32,0x79622d32
	.long	0x6b206574,0x6b206574,0x6b206574,0x6b206574
	.long	0,1,2,3
        .long   0x03020100,0x07060504,0x0b0a0908,0x0f0e0d0c
.asciz  "ChaCha20 for PowerPC/AltiVec, CRYPTOGAMS by <appro\@openssl.org>"
.align	2
___

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/ge;

	# instructions prefixed with '?' are endian-specific and need
	# to be adjusted accordingly...
	if ($flavour !~ /le$/) {	# big-endian
	    s/be\?//		or
	    s/le\?/#le#/	or
	    s/\?lvsr/lvsl/	or
	    s/\?lvsl/lvsr/	or
	    s/\?(vperm\s+v[0-9]+,\s*)(v[0-9]+,\s*)(v[0-9]+,\s*)(v[0-9]+)/$1$3$2$4/ or
	    s/vrldoi(\s+v[0-9]+,\s*)(v[0-9]+,)\s*([0-9]+)/vsldoi$1$2$2 16-$3/;
	} else {			# little-endian
	    s/le\?//		or
	    s/be\?/#be#/	or
	    s/\?([a-z]+)/$1/	or
	    s/vrldoi(\s+v[0-9]+,\s*)(v[0-9]+,)\s*([0-9]+)/vsldoi$1$2$2 $3/;
	}

	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
