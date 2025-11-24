#! /usr/bin/env perl
# Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# This module implements SM4 with ASIMD and AESE on AARCH64
#
# Dec 2022
#

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

$prefix="vpsm4_ex";
my @vtmp=map("v$_",(0..3));
my @qtmp=map("q$_",(0..3));
my @data=map("v$_",(4..7));
my @datax=map("v$_",(8..11));
my ($rk0,$rk1)=("v12","v13");
my ($rka,$rkb)=("v14","v15");
my @vtmpx=map("v$_",(12..15));
my ($vtmp4,$vtmp5)=("v24","v25");
my ($MaskV,$TAHMatV,$TALMatV,$ATAHMatV,$ATALMatV,$ANDMaskV)=("v26","v27","v28","v29","v30","v31");
my ($MaskQ,$TAHMatQ,$TALMatQ,$ATAHMatQ,$ATALMatQ,$ANDMaskQ)=("q26","q27","q28","q29","q30","q31");

my ($inp,$outp,$blocks,$rks)=("x0","x1","w2","x3");
my ($tmpw,$tmp,$wtmp0,$wtmp1,$wtmp2)=("w6","x6","w7","w8","w9");
my ($xtmp1,$xtmp2)=("x8","x9");
my ($ptr,$counter)=("x10","w11");
my ($word0,$word1,$word2,$word3)=("w12","w13","w14","w15");

sub rev32() {
	my $dst = shift;
	my $src = shift;

	if ($src and ("$src" ne "$dst")) {
$code.=<<___;
#ifndef __AARCH64EB__
	rev32	$dst.16b,$src.16b
#else
	mov	$dst.16b,$src.16b
#endif
___
	} else {
$code.=<<___;
#ifndef __AARCH64EB__
	rev32	$dst.16b,$dst.16b
#endif
___
	}
}

sub rev32_armeb() {
	my $dst = shift;
	my $src = shift;

	if ($src and ("$src" ne "$dst")) {
$code.=<<___;
#ifdef __AARCH64EB__
	rev32	$dst.16b,$src.16b
#else
	mov	$dst.16b,$src.16b
#endif
___
	} else {
$code.=<<___;
#ifdef __AARCH64EB__
	rev32	$dst.16b,$dst.16b
#endif
___
	}
}

sub rbit() {
	my $dst = shift;
	my $src = shift;
	my $std = shift;

	if ($src and ("$src" ne "$dst")) {
		if ($std eq "_gb") {
$code.=<<___;
			rbit $dst.16b,$src.16b
___
		} else {
$code.=<<___;
			mov $dst.16b,$src.16b
___
		}
	} else {
		if ($std eq "_gb") {
$code.=<<___;
			rbit $dst.16b,$src.16b
___
		}
	}
}

sub transpose() {
	my ($dat0,$dat1,$dat2,$dat3,$vt0,$vt1,$vt2,$vt3) = @_;

$code.=<<___;
	zip1	$vt0.4s,$dat0.4s,$dat1.4s
	zip2	$vt1.4s,$dat0.4s,$dat1.4s
	zip1	$vt2.4s,$dat2.4s,$dat3.4s
	zip2	$vt3.4s,$dat2.4s,$dat3.4s
	zip1	$dat0.2d,$vt0.2d,$vt2.2d
	zip2	$dat1.2d,$vt0.2d,$vt2.2d
	zip1	$dat2.2d,$vt1.2d,$vt3.2d
	zip2	$dat3.2d,$vt1.2d,$vt3.2d
___
}

# matrix multiplication Mat*x = (lowerMat*x) ^ (higherMat*x)
sub mul_matrix() {
	my $x = shift;
	my $higherMat = shift;
	my $lowerMat = shift;
	my $tmp = shift;
$code.=<<___;
	ushr	$tmp.16b, $x.16b, 4
	and		$x.16b, $x.16b, $ANDMaskV.16b
	tbl		$x.16b, {$lowerMat.16b}, $x.16b
	tbl		$tmp.16b, {$higherMat.16b}, $tmp.16b
	eor		$x.16b, $x.16b, $tmp.16b
___
}

# sbox operations for 4-lane of words
# sbox operation for 4-lane of words
sub sbox() {
	my $dat = shift;

$code.=<<___;
	// optimize sbox using AESE instruction
	tbl	@vtmp[0].16b, {$dat.16b}, $MaskV.16b
___
	&mul_matrix(@vtmp[0], $TAHMatV, $TALMatV, $vtmp4);
$code.=<<___;
	eor @vtmp[1].16b, @vtmp[1].16b, @vtmp[1].16b
	aese @vtmp[0].16b,@vtmp[1].16b
___
	&mul_matrix(@vtmp[0], $ATAHMatV, $ATALMatV, $vtmp4);
$code.=<<___;
	mov	$dat.16b,@vtmp[0].16b

	// linear transformation
	ushr	@vtmp[0].4s,$dat.4s,32-2
	ushr	@vtmp[1].4s,$dat.4s,32-10
	ushr	@vtmp[2].4s,$dat.4s,32-18
	ushr	@vtmp[3].4s,$dat.4s,32-24
	sli	@vtmp[0].4s,$dat.4s,2
	sli	@vtmp[1].4s,$dat.4s,10
	sli	@vtmp[2].4s,$dat.4s,18
	sli	@vtmp[3].4s,$dat.4s,24
	eor	$vtmp4.16b,@vtmp[0].16b,$dat.16b
	eor	$vtmp4.16b,$vtmp4.16b,$vtmp[1].16b
	eor	$dat.16b,@vtmp[2].16b,@vtmp[3].16b
	eor	$dat.16b,$dat.16b,$vtmp4.16b
___
}

# sbox operation for 8-lane of words
sub sbox_double() {
	my $dat = shift;
	my $datx = shift;

$code.=<<___;
	// optimize sbox using AESE instruction
	tbl	@vtmp[0].16b, {$dat.16b}, $MaskV.16b
	tbl	@vtmp[1].16b, {$datx.16b}, $MaskV.16b
___
	&mul_matrix(@vtmp[0], $TAHMatV, $TALMatV, $vtmp4);
	&mul_matrix(@vtmp[1], $TAHMatV, $TALMatV, $vtmp4);
$code.=<<___;
	eor $vtmp5.16b, $vtmp5.16b, $vtmp5.16b
	aese @vtmp[0].16b,$vtmp5.16b
	aese @vtmp[1].16b,$vtmp5.16b
___
	&mul_matrix(@vtmp[0], $ATAHMatV, $ATALMatV,$vtmp4);
	&mul_matrix(@vtmp[1], $ATAHMatV, $ATALMatV,$vtmp4);
$code.=<<___;
	mov	$dat.16b,@vtmp[0].16b
	mov	$datx.16b,@vtmp[1].16b

	// linear transformation
	ushr	@vtmp[0].4s,$dat.4s,32-2
	ushr	$vtmp5.4s,$datx.4s,32-2
	ushr	@vtmp[1].4s,$dat.4s,32-10
	ushr	@vtmp[2].4s,$dat.4s,32-18
	ushr	@vtmp[3].4s,$dat.4s,32-24
	sli	@vtmp[0].4s,$dat.4s,2
	sli	$vtmp5.4s,$datx.4s,2
	sli	@vtmp[1].4s,$dat.4s,10
	sli	@vtmp[2].4s,$dat.4s,18
	sli	@vtmp[3].4s,$dat.4s,24
	eor	$vtmp4.16b,@vtmp[0].16b,$dat.16b
	eor	$vtmp4.16b,$vtmp4.16b,@vtmp[1].16b
	eor	$dat.16b,@vtmp[2].16b,@vtmp[3].16b
	eor	$dat.16b,$dat.16b,$vtmp4.16b
	ushr	@vtmp[1].4s,$datx.4s,32-10
	ushr	@vtmp[2].4s,$datx.4s,32-18
	ushr	@vtmp[3].4s,$datx.4s,32-24
	sli	@vtmp[1].4s,$datx.4s,10
	sli	@vtmp[2].4s,$datx.4s,18
	sli	@vtmp[3].4s,$datx.4s,24
	eor	$vtmp4.16b,$vtmp5.16b,$datx.16b
	eor	$vtmp4.16b,$vtmp4.16b,@vtmp[1].16b
	eor	$datx.16b,@vtmp[2].16b,@vtmp[3].16b
	eor	$datx.16b,$datx.16b,$vtmp4.16b
___
}

# sbox operation for one single word
sub sbox_1word () {
	my $word = shift;

$code.=<<___;
	mov	@vtmp[3].s[0],$word
	// optimize sbox using AESE instruction
	tbl	@vtmp[0].16b, {@vtmp[3].16b}, $MaskV.16b
___
	&mul_matrix(@vtmp[0], $TAHMatV, $TALMatV, @vtmp[2]);
$code.=<<___;
	eor @vtmp[1].16b, @vtmp[1].16b, @vtmp[1].16b
	aese @vtmp[0].16b,@vtmp[1].16b
___
	&mul_matrix(@vtmp[0], $ATAHMatV, $ATALMatV, @vtmp[2]);
$code.=<<___;

	mov	$wtmp0,@vtmp[0].s[0]
	eor	$word,$wtmp0,$wtmp0,ror #32-2
	eor	$word,$word,$wtmp0,ror #32-10
	eor	$word,$word,$wtmp0,ror #32-18
	eor	$word,$word,$wtmp0,ror #32-24
___
}

# sm4 for one block of data, in scalar registers word0/word1/word2/word3
sub sm4_1blk () {
	my $kptr = shift;

$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	// B0 ^= SBOX(B1 ^ B2 ^ B3 ^ RK0)
	eor	$tmpw,$word2,$word3
	eor	$wtmp2,$wtmp0,$word1
	eor	$tmpw,$tmpw,$wtmp2
___
	&sbox_1word($tmpw);
$code.=<<___;
	eor	$word0,$word0,$tmpw
	// B1 ^= SBOX(B0 ^ B2 ^ B3 ^ RK1)
	eor	$tmpw,$word2,$word3
	eor	$wtmp2,$word0,$wtmp1
	eor	$tmpw,$tmpw,$wtmp2
___
	&sbox_1word($tmpw);
$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	eor	$word1,$word1,$tmpw
	// B2 ^= SBOX(B0 ^ B1 ^ B3 ^ RK2)
	eor	$tmpw,$word0,$word1
	eor	$wtmp2,$wtmp0,$word3
	eor	$tmpw,$tmpw,$wtmp2
___
	&sbox_1word($tmpw);
$code.=<<___;
	eor	$word2,$word2,$tmpw
	// B3 ^= SBOX(B0 ^ B1 ^ B2 ^ RK3)
	eor	$tmpw,$word0,$word1
	eor	$wtmp2,$word2,$wtmp1
	eor	$tmpw,$tmpw,$wtmp2
___
	&sbox_1word($tmpw);
$code.=<<___;
	eor	$word3,$word3,$tmpw
___
}

# sm4 for 4-lanes of data, in neon registers data0/data1/data2/data3
sub sm4_4blks () {
	my $kptr = shift;

$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	dup	$rk0.4s,$wtmp0
	dup	$rk1.4s,$wtmp1

	// B0 ^= SBOX(B1 ^ B2 ^ B3 ^ RK0)
	eor	$rka.16b,@data[2].16b,@data[3].16b
	eor	$rk0.16b,@data[1].16b,$rk0.16b
	eor	$rk0.16b,$rka.16b,$rk0.16b
___
	&sbox($rk0);
$code.=<<___;
	eor	@data[0].16b,@data[0].16b,$rk0.16b

	// B1 ^= SBOX(B0 ^ B2 ^ B3 ^ RK1)
	eor	$rka.16b,$rka.16b,@data[0].16b
	eor	$rk1.16b,$rka.16b,$rk1.16b
___
	&sbox($rk1);
$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	eor	@data[1].16b,@data[1].16b,$rk1.16b

	dup	$rk0.4s,$wtmp0
	dup	$rk1.4s,$wtmp1

	// B2 ^= SBOX(B0 ^ B1 ^ B3 ^ RK2)
	eor	$rka.16b,@data[0].16b,@data[1].16b
	eor	$rk0.16b,@data[3].16b,$rk0.16b
	eor	$rk0.16b,$rka.16b,$rk0.16b
___
	&sbox($rk0);
$code.=<<___;
	eor	@data[2].16b,@data[2].16b,$rk0.16b

	// B3 ^= SBOX(B0 ^ B1 ^ B2 ^ RK3)
	eor	$rka.16b,$rka.16b,@data[2].16b
	eor	$rk1.16b,$rka.16b,$rk1.16b
___
	&sbox($rk1);
$code.=<<___;
	eor	@data[3].16b,@data[3].16b,$rk1.16b
___
}

# sm4 for 8 lanes of data, in neon registers
# data0/data1/data2/data3 datax0/datax1/datax2/datax3
sub sm4_8blks () {
	my $kptr = shift;

$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	// B0 ^= SBOX(B1 ^ B2 ^ B3 ^ RK0)
	dup	$rk0.4s,$wtmp0
	eor	$rka.16b,@data[2].16b,@data[3].16b
	eor	$rkb.16b,@datax[2].16b,@datax[3].16b
	eor	@vtmp[0].16b,@data[1].16b,$rk0.16b
	eor	@vtmp[1].16b,@datax[1].16b,$rk0.16b
	eor	$rk0.16b,$rka.16b,@vtmp[0].16b
	eor	$rk1.16b,$rkb.16b,@vtmp[1].16b
___
	&sbox_double($rk0,$rk1);
$code.=<<___;
	eor	@data[0].16b,@data[0].16b,$rk0.16b
	eor	@datax[0].16b,@datax[0].16b,$rk1.16b

	// B1 ^= SBOX(B0 ^ B2 ^ B3 ^ RK1)
	dup	$rk1.4s,$wtmp1
	eor	$rka.16b,$rka.16b,@data[0].16b
	eor	$rkb.16b,$rkb.16b,@datax[0].16b
	eor	$rk0.16b,$rka.16b,$rk1.16b
	eor	$rk1.16b,$rkb.16b,$rk1.16b
___
	&sbox_double($rk0,$rk1);
$code.=<<___;
	ldp	$wtmp0,$wtmp1,[$kptr],8
	eor	@data[1].16b,@data[1].16b,$rk0.16b
	eor	@datax[1].16b,@datax[1].16b,$rk1.16b

	// B2 ^= SBOX(B0 ^ B1 ^ B3 ^ RK2)
	dup	$rk0.4s,$wtmp0
	eor	$rka.16b,@data[0].16b,@data[1].16b
	eor	$rkb.16b,@datax[0].16b,@datax[1].16b
	eor	@vtmp[0].16b,@data[3].16b,$rk0.16b
	eor	@vtmp[1].16b,@datax[3].16b,$rk0.16b
	eor	$rk0.16b,$rka.16b,@vtmp[0].16b
	eor	$rk1.16b,$rkb.16b,@vtmp[1].16b
___
	&sbox_double($rk0,$rk1);
$code.=<<___;
	eor	@data[2].16b,@data[2].16b,$rk0.16b
	eor	@datax[2].16b,@datax[2].16b,$rk1.16b

	// B3 ^= SBOX(B0 ^ B1 ^ B2 ^ RK3)
	dup	$rk1.4s,$wtmp1
	eor	$rka.16b,$rka.16b,@data[2].16b
	eor	$rkb.16b,$rkb.16b,@datax[2].16b
	eor	$rk0.16b,$rka.16b,$rk1.16b
	eor	$rk1.16b,$rkb.16b,$rk1.16b
___
	&sbox_double($rk0,$rk1);
$code.=<<___;
	eor	@data[3].16b,@data[3].16b,$rk0.16b
	eor	@datax[3].16b,@datax[3].16b,$rk1.16b
___
}

sub encrypt_1blk_norev() {
	my $dat = shift;

$code.=<<___;
	mov	$ptr,$rks
	mov	$counter,#8
	mov	$word0,$dat.s[0]
	mov	$word1,$dat.s[1]
	mov	$word2,$dat.s[2]
	mov	$word3,$dat.s[3]
10:
___
	&sm4_1blk($ptr);
$code.=<<___;
	subs	$counter,$counter,#1
	b.ne	10b
	mov	$dat.s[0],$word3
	mov	$dat.s[1],$word2
	mov	$dat.s[2],$word1
	mov	$dat.s[3],$word0
___
}

sub encrypt_1blk() {
	my $dat = shift;

	&encrypt_1blk_norev($dat);
	&rev32($dat,$dat);
}

sub encrypt_4blks() {
$code.=<<___;
	mov	$ptr,$rks
	mov	$counter,#8
10:
___
	&sm4_4blks($ptr);
$code.=<<___;
	subs	$counter,$counter,#1
	b.ne	10b
___
	&rev32(@vtmp[3],@data[0]);
	&rev32(@vtmp[2],@data[1]);
	&rev32(@vtmp[1],@data[2]);
	&rev32(@vtmp[0],@data[3]);
}

sub encrypt_8blks() {
$code.=<<___;
	mov	$ptr,$rks
	mov	$counter,#8
10:
___
	&sm4_8blks($ptr);
$code.=<<___;
	subs	$counter,$counter,#1
	b.ne	10b
___
	&rev32(@vtmp[3],@data[0]);
	&rev32(@vtmp[2],@data[1]);
	&rev32(@vtmp[1],@data[2]);
	&rev32(@vtmp[0],@data[3]);
	&rev32(@data[3],@datax[0]);
	&rev32(@data[2],@datax[1]);
	&rev32(@data[1],@datax[2]);
	&rev32(@data[0],@datax[3]);
}

sub load_sbox () {
	my $data = shift;

$code.=<<___;
	adrp $xtmp2, .Lsbox_magic
	ldr $MaskQ, [$xtmp2, #:lo12:.Lsbox_magic]
	ldr $TAHMatQ, [$xtmp2, #:lo12:.Lsbox_magic+16]
	ldr $TALMatQ, [$xtmp2, #:lo12:.Lsbox_magic+32]
	ldr $ATAHMatQ, [$xtmp2, #:lo12:.Lsbox_magic+48]
	ldr $ATALMatQ, [$xtmp2, #:lo12:.Lsbox_magic+64]
	ldr $ANDMaskQ, [$xtmp2, #:lo12:.Lsbox_magic+80]
___
}

sub mov_reg_to_vec() {
	my $src0 = shift;
	my $src1 = shift;
	my $desv = shift;
$code.=<<___;
	mov $desv.d[0],$src0
	mov $desv.d[1],$src1
___
	&rev32_armeb($desv,$desv);
}

sub mov_vec_to_reg() {
	my $srcv = shift;
	my $des0 = shift;
	my $des1 = shift;
$code.=<<___;
	mov $des0,$srcv.d[0]
	mov $des1,$srcv.d[1]
___
}

sub compute_tweak() {
	my $src0 = shift;
	my $src1 = shift;
	my $des0 = shift;
	my $des1 = shift;
$code.=<<___;
	mov $wtmp0,0x87
	extr	$xtmp2,$src1,$src1,#32
	extr	$des1,$src1,$src0,#63
	and	$wtmp1,$wtmp0,$wtmp2,asr#31
	eor	$des0,$xtmp1,$src0,lsl#1
___
}

sub compute_tweak_vec() {
	my $src = shift;
	my $des = shift;
	my $std = shift;
	&rbit(@vtmp[2],$src,$std);
$code.=<<___;
	adrp  $xtmp2, .Lxts_magic
	ldr  @qtmp[0], [$xtmp2, #:lo12:.Lxts_magic]
	shl  $des.16b, @vtmp[2].16b, #1
	ext  @vtmp[1].16b, @vtmp[2].16b, @vtmp[2].16b,#15
	ushr @vtmp[1].16b, @vtmp[1].16b, #7
	mul  @vtmp[1].16b, @vtmp[1].16b, @vtmp[0].16b
	eor  $des.16b, $des.16b, @vtmp[1].16b
___
	&rbit($des,$des,$std);
}

$code=<<___;
#include "arm_arch.h"
.arch	armv8-a+crypto
.text

.type	_${prefix}_consts,%object
.align	7
_${prefix}_consts:
.Lck:
	.long 0x00070E15, 0x1C232A31, 0x383F464D, 0x545B6269
	.long 0x70777E85, 0x8C939AA1, 0xA8AFB6BD, 0xC4CBD2D9
	.long 0xE0E7EEF5, 0xFC030A11, 0x181F262D, 0x343B4249
	.long 0x50575E65, 0x6C737A81, 0x888F969D, 0xA4ABB2B9
	.long 0xC0C7CED5, 0xDCE3EAF1, 0xF8FF060D, 0x141B2229
	.long 0x30373E45, 0x4C535A61, 0x686F767D, 0x848B9299
	.long 0xA0A7AEB5, 0xBCC3CAD1, 0xD8DFE6ED, 0xF4FB0209
	.long 0x10171E25, 0x2C333A41, 0x484F565D, 0x646B7279
.Lfk:
	.quad 0x56aa3350a3b1bac6,0xb27022dc677d9197
.Lshuffles:
	.quad 0x0B0A090807060504,0x030201000F0E0D0C
.Lxts_magic:
	.quad 0x0101010101010187,0x0101010101010101
.Lsbox_magic:
	.quad 0x0b0e0104070a0d00,0x0306090c0f020508
	.quad 0x62185a2042387a00,0x22581a6002783a40
	.quad 0x15df62a89e54e923,0xc10bb67c4a803df7
	.quad 0xb9aa6b78c1d21300,0x1407c6d56c7fbead
	.quad 0x6404462679195b3b,0xe383c1a1fe9edcbc
	.quad 0x0f0f0f0f0f0f0f0f,0x0f0f0f0f0f0f0f0f

.size	_${prefix}_consts,.-_${prefix}_consts
___

{{{
my ($key,$keys,$enc)=("x0","x1","w2");
my ($pointer,$schedules,$wtmp,$roundkey)=("x5","x6","w7","w8");
my ($vkey,$vfk,$vmap)=("v5","v6","v7");
$code.=<<___;
.type	_${prefix}_set_key,%function
.align	4
_${prefix}_set_key:
	AARCH64_VALID_CALL_TARGET
	ld1	{$vkey.4s},[$key]
___
	&load_sbox();
	&rev32($vkey,$vkey);
$code.=<<___;
	adrp	$pointer,.Lshuffles
	add	$pointer,$pointer,#:lo12:.Lshuffles
	ld1	{$vmap.2d},[$pointer]
	adrp	$pointer,.Lfk
	add	$pointer,$pointer,#:lo12:.Lfk
	ld1	{$vfk.2d},[$pointer]
	eor	$vkey.16b,$vkey.16b,$vfk.16b
	mov	$schedules,#32
	adrp	$pointer,.Lck
	add	$pointer,$pointer,#:lo12:.Lck
	movi	@vtmp[0].16b,#64
	cbnz	$enc,1f
	add	$keys,$keys,124
1:
	mov	$wtmp,$vkey.s[1]
	ldr	$roundkey,[$pointer],#4
	eor	$roundkey,$roundkey,$wtmp
	mov	$wtmp,$vkey.s[2]
	eor	$roundkey,$roundkey,$wtmp
	mov	$wtmp,$vkey.s[3]
	eor	$roundkey,$roundkey,$wtmp
	// optimize sbox using AESE instruction
	mov	@data[0].s[0],$roundkey
	tbl	@vtmp[0].16b, {@data[0].16b}, $MaskV.16b
___
	&mul_matrix(@vtmp[0], $TAHMatV, $TALMatV, @vtmp[2]);
$code.=<<___;
	eor @vtmp[1].16b, @vtmp[1].16b, @vtmp[1].16b
	aese @vtmp[0].16b,@vtmp[1].16b
___
	&mul_matrix(@vtmp[0], $ATAHMatV, $ATALMatV, @vtmp[2]);
$code.=<<___;
	mov	$wtmp,@vtmp[0].s[0]
	eor	$roundkey,$wtmp,$wtmp,ror #19
	eor	$roundkey,$roundkey,$wtmp,ror #9
	mov	$wtmp,$vkey.s[0]
	eor	$roundkey,$roundkey,$wtmp
	mov	$vkey.s[0],$roundkey
	cbz	$enc,2f
	str	$roundkey,[$keys],#4
	b	3f
2:
	str	$roundkey,[$keys],#-4
3:
	tbl	$vkey.16b,{$vkey.16b},$vmap.16b
	subs	$schedules,$schedules,#1
	b.ne	1b
	ret
.size	_${prefix}_set_key,.-_${prefix}_set_key
___
}}}


{{{
$code.=<<___;
.type	_${prefix}_enc_4blks,%function
.align	4
_${prefix}_enc_4blks:
	AARCH64_VALID_CALL_TARGET
___
	&encrypt_4blks();
$code.=<<___;
	ret
.size	_${prefix}_enc_4blks,.-_${prefix}_enc_4blks
___
}}}

{{{
$code.=<<___;
.type	_${prefix}_enc_8blks,%function
.align	4
_${prefix}_enc_8blks:
	AARCH64_VALID_CALL_TARGET
___
	&encrypt_8blks();
$code.=<<___;
	ret
.size	_${prefix}_enc_8blks,.-_${prefix}_enc_8blks
___
}}}


{{{
my ($key,$keys)=("x0","x1");
$code.=<<___;
.globl	${prefix}_set_encrypt_key
.type	${prefix}_set_encrypt_key,%function
.align	5
${prefix}_set_encrypt_key:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-16]!
	mov	w2,1
	bl	_${prefix}_set_key
	ldp	x29,x30,[sp],#16
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_set_encrypt_key,.-${prefix}_set_encrypt_key
___
}}}

{{{
my ($key,$keys)=("x0","x1");
$code.=<<___;
.globl	${prefix}_set_decrypt_key
.type	${prefix}_set_decrypt_key,%function
.align	5
${prefix}_set_decrypt_key:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-16]!
	mov	w2,0
	bl	_${prefix}_set_key
	ldp	x29,x30,[sp],#16
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_set_decrypt_key,.-${prefix}_set_decrypt_key
___
}}}

{{{
sub gen_block () {
	my $dir = shift;
	my ($inp,$outp,$rk)=map("x$_",(0..2));

$code.=<<___;
.globl	${prefix}_${dir}crypt
.type	${prefix}_${dir}crypt,%function
.align	5
${prefix}_${dir}crypt:
	AARCH64_VALID_CALL_TARGET
	ld1	{@data[0].4s},[$inp]
___
	&load_sbox();
	&rev32(@data[0],@data[0]);
$code.=<<___;
	mov	$rks,$rk
___
	&encrypt_1blk(@data[0]);
$code.=<<___;
	st1	{@data[0].4s},[$outp]
	ret
.size	${prefix}_${dir}crypt,.-${prefix}_${dir}crypt
___
}
&gen_block("en");
&gen_block("de");
}}}

{{{
$code.=<<___;
.globl	${prefix}_ecb_encrypt
.type	${prefix}_ecb_encrypt,%function
.align	5
${prefix}_ecb_encrypt:
	AARCH64_SIGN_LINK_REGISTER
	// convert length into blocks
	lsr	x2,x2,4
	stp	d8,d9,[sp,#-80]!
	stp	d10,d11,[sp,#16]
	stp	d12,d13,[sp,#32]
	stp	d14,d15,[sp,#48]
	stp	x29,x30,[sp,#64]
___
	&load_sbox();
$code.=<<___;
.Lecb_8_blocks_process:
	cmp	$blocks,#8
	b.lt	.Lecb_4_blocks_process
	ld4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
	ld4	{@datax[0].4s,$datax[1].4s,@datax[2].4s,@datax[3].4s},[$inp],#64
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
	&rev32(@datax[0],@datax[0]);
	&rev32(@datax[1],@datax[1]);
	&rev32(@datax[2],@datax[2]);
	&rev32(@datax[3],@datax[3]);
$code.=<<___;
	bl	_${prefix}_enc_8blks
	st4	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	st4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#8
	b.gt	.Lecb_8_blocks_process
	b	100f
.Lecb_4_blocks_process:
	cmp	$blocks,#4
	b.lt	1f
	ld4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	st4	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	sub	$blocks,$blocks,#4
1:
	// process last block
	cmp	$blocks,#1
	b.lt	100f
	b.gt	1f
	ld1	{@data[0].4s},[$inp]
___
	&rev32(@data[0],@data[0]);
	&encrypt_1blk(@data[0]);
$code.=<<___;
	st1	{@data[0].4s},[$outp]
	b	100f
1:	// process last 2 blocks
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[0],[$inp],#16
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[1],[$inp],#16
	cmp	$blocks,#2
	b.gt	1f
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	st4	{@vtmp[0].s-@vtmp[3].s}[0],[$outp],#16
	st4	{@vtmp[0].s-@vtmp[3].s}[1],[$outp]
	b	100f
1:	// process last 3 blocks
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[2],[$inp],#16
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	st4	{@vtmp[0].s-@vtmp[3].s}[0],[$outp],#16
	st4	{@vtmp[0].s-@vtmp[3].s}[1],[$outp],#16
	st4	{@vtmp[0].s-@vtmp[3].s}[2],[$outp]
100:
	ldp	d10,d11,[sp,#16]
	ldp	d12,d13,[sp,#32]
	ldp	d14,d15,[sp,#48]
	ldp	x29,x30,[sp,#64]
	ldp	d8,d9,[sp],#80
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_ecb_encrypt,.-${prefix}_ecb_encrypt
___
}}}

{{{
my ($len,$ivp,$enc)=("x2","x4","w5");
my $ivec0=("v3");
my $ivec1=("v15");

$code.=<<___;
.globl	${prefix}_cbc_encrypt
.type	${prefix}_cbc_encrypt,%function
.align	5
${prefix}_cbc_encrypt:
	AARCH64_VALID_CALL_TARGET
	lsr	$len,$len,4
___
	&load_sbox();
$code.=<<___;
	cbz	$enc,.Ldec
	ld1	{$ivec0.4s},[$ivp]
.Lcbc_4_blocks_enc:
	cmp	$blocks,#4
	b.lt	1f
	ld1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
	eor	@data[0].16b,@data[0].16b,$ivec0.16b
___
	&rev32(@data[1],@data[1]);
	&rev32(@data[0],@data[0]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
	&encrypt_1blk_norev(@data[0]);
$code.=<<___;
	eor	@data[1].16b,@data[1].16b,@data[0].16b
___
	&encrypt_1blk_norev(@data[1]);
	&rev32(@data[0],@data[0]);

$code.=<<___;
	eor	@data[2].16b,@data[2].16b,@data[1].16b
___
	&encrypt_1blk_norev(@data[2]);
	&rev32(@data[1],@data[1]);
$code.=<<___;
	eor	@data[3].16b,@data[3].16b,@data[2].16b
___
	&encrypt_1blk_norev(@data[3]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	orr	$ivec0.16b,@data[3].16b,@data[3].16b
	st1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#4
	b.ne	.Lcbc_4_blocks_enc
	b	2f
1:
	subs	$blocks,$blocks,#1
	b.lt	2f
	ld1	{@data[0].4s},[$inp],#16
	eor	$ivec0.16b,$ivec0.16b,@data[0].16b
___
	&rev32($ivec0,$ivec0);
	&encrypt_1blk($ivec0);
$code.=<<___;
	st1	{$ivec0.4s},[$outp],#16
	b	1b
2:
	// save back IV
	st1	{$ivec0.4s},[$ivp]
	ret

.Ldec:
	// decryption mode starts
	AARCH64_SIGN_LINK_REGISTER
	stp	d8,d9,[sp,#-80]!
	stp	d10,d11,[sp,#16]
	stp	d12,d13,[sp,#32]
	stp	d14,d15,[sp,#48]
	stp	x29,x30,[sp,#64]
.Lcbc_8_blocks_dec:
	cmp	$blocks,#8
	b.lt	1f
	ld4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp]
	add	$ptr,$inp,#64
	ld4	{@datax[0].4s,@datax[1].4s,@datax[2].4s,@datax[3].4s},[$ptr]
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],$data[3]);
	&rev32(@datax[0],@datax[0]);
	&rev32(@datax[1],@datax[1]);
	&rev32(@datax[2],@datax[2]);
	&rev32(@datax[3],$datax[3]);
$code.=<<___;
	bl	_${prefix}_enc_8blks
___
	&transpose(@vtmp,@datax);
	&transpose(@data,@datax);
$code.=<<___;
	ld1	{$ivec1.4s},[$ivp]
	ld1	{@datax[0].4s,@datax[1].4s,@datax[2].4s,@datax[3].4s},[$inp],#64
	// note ivec1 and vtmpx[3] are reusing the same register
	// care needs to be taken to avoid conflict
	eor	@vtmp[0].16b,@vtmp[0].16b,$ivec1.16b
	ld1	{@vtmpx[0].4s,@vtmpx[1].4s,@vtmpx[2].4s,@vtmpx[3].4s},[$inp],#64
	eor	@vtmp[1].16b,@vtmp[1].16b,@datax[0].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@datax[1].16b
	eor	@vtmp[3].16b,$vtmp[3].16b,@datax[2].16b
	// save back IV
	st1	{$vtmpx[3].4s}, [$ivp]
	eor	@data[0].16b,@data[0].16b,$datax[3].16b
	eor	@data[1].16b,@data[1].16b,@vtmpx[0].16b
	eor	@data[2].16b,@data[2].16b,@vtmpx[1].16b
	eor	@data[3].16b,$data[3].16b,@vtmpx[2].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	st1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#8
	b.gt	.Lcbc_8_blocks_dec
	b.eq	100f
1:
	ld1	{$ivec1.4s},[$ivp]
.Lcbc_4_blocks_dec:
	cmp	$blocks,#4
	b.lt	1f
	ld4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp]
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],$data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	ld1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
___
	&transpose(@vtmp,@datax);
$code.=<<___;
	eor	@vtmp[0].16b,@vtmp[0].16b,$ivec1.16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@data[0].16b
	orr	$ivec1.16b,@data[3].16b,@data[3].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@data[1].16b
	eor	@vtmp[3].16b,$vtmp[3].16b,@data[2].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	subs	$blocks,$blocks,#4
	b.gt	.Lcbc_4_blocks_dec
	// save back IV
	st1	{@data[3].4s}, [$ivp]
	b	100f
1:	// last block
	subs	$blocks,$blocks,#1
	b.lt	100f
	b.gt	1f
	ld1	{@data[0].4s},[$inp],#16
	// save back IV
	st1	{$data[0].4s}, [$ivp]
___
	&rev32(@datax[0],@data[0]);
	&encrypt_1blk(@datax[0]);
$code.=<<___;
	eor	@datax[0].16b,@datax[0].16b,$ivec1.16b
	st1	{@datax[0].4s},[$outp],#16
	b	100f
1:	// last two blocks
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[0],[$inp]
	add	$ptr,$inp,#16
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[1],[$ptr],#16
	subs	$blocks,$blocks,1
	b.gt	1f
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	ld1	{@data[0].4s,@data[1].4s},[$inp],#32
___
	&transpose(@vtmp,@datax);
$code.=<<___;
	eor	@vtmp[0].16b,@vtmp[0].16b,$ivec1.16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@data[0].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s},[$outp],#32
	// save back IV
	st1	{@data[1].4s}, [$ivp]
	b	100f
1:	// last 3 blocks
	ld4	{@data[0].s,@data[1].s,@data[2].s,@data[3].s}[2],[$ptr]
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
$code.=<<___;
	bl	_${prefix}_enc_4blks
	ld1	{@data[0].4s,@data[1].4s,@data[2].4s},[$inp],#48
___
	&transpose(@vtmp,@datax);
$code.=<<___;
	eor	@vtmp[0].16b,@vtmp[0].16b,$ivec1.16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@data[0].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@data[1].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s},[$outp],#48
	// save back IV
	st1	{@data[2].4s}, [$ivp]
100:
	ldp	d10,d11,[sp,#16]
	ldp	d12,d13,[sp,#32]
	ldp	d14,d15,[sp,#48]
	ldp	x29,x30,[sp,#64]
	ldp	d8,d9,[sp],#80
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_cbc_encrypt,.-${prefix}_cbc_encrypt
___
}}}

{{{
my ($ivp)=("x4");
my ($ctr)=("w5");
my $ivec=("v3");

$code.=<<___;
.globl	${prefix}_ctr32_encrypt_blocks
.type	${prefix}_ctr32_encrypt_blocks,%function
.align	5
${prefix}_ctr32_encrypt_blocks:
	AARCH64_VALID_CALL_TARGET
	ld1	{$ivec.4s},[$ivp]
___
	&rev32($ivec,$ivec);
	&load_sbox();
$code.=<<___;
	cmp	$blocks,#1
	b.ne	1f
	// fast processing for one single block without
	// context saving overhead
___
	&encrypt_1blk($ivec);
$code.=<<___;
	ld1	{@data[0].4s},[$inp]
	eor	@data[0].16b,@data[0].16b,$ivec.16b
	st1	{@data[0].4s},[$outp]
	ret
1:
	AARCH64_SIGN_LINK_REGISTER
	stp	d8,d9,[sp,#-80]!
	stp	d10,d11,[sp,#16]
	stp	d12,d13,[sp,#32]
	stp	d14,d15,[sp,#48]
	stp	x29,x30,[sp,#64]
	mov	$word0,$ivec.s[0]
	mov	$word1,$ivec.s[1]
	mov	$word2,$ivec.s[2]
	mov	$ctr,$ivec.s[3]
.Lctr32_4_blocks_process:
	cmp	$blocks,#4
	b.lt	1f
	dup	@data[0].4s,$word0
	dup	@data[1].4s,$word1
	dup	@data[2].4s,$word2
	mov	@data[3].s[0],$ctr
	add	$ctr,$ctr,#1
	mov	$data[3].s[1],$ctr
	add	$ctr,$ctr,#1
	mov	@data[3].s[2],$ctr
	add	$ctr,$ctr,#1
	mov	@data[3].s[3],$ctr
	add	$ctr,$ctr,#1
	cmp	$blocks,#8
	b.ge	.Lctr32_8_blocks_process
	bl	_${prefix}_enc_4blks
	ld4	{@vtmpx[0].4s,@vtmpx[1].4s,@vtmpx[2].4s,@vtmpx[3].4s},[$inp],#64
	eor	@vtmp[0].16b,@vtmp[0].16b,@vtmpx[0].16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@vtmpx[1].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@vtmpx[2].16b
	eor	@vtmp[3].16b,@vtmp[3].16b,@vtmpx[3].16b
	st4	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	subs	$blocks,$blocks,#4
	b.ne	.Lctr32_4_blocks_process
	b	100f
.Lctr32_8_blocks_process:
	dup	@datax[0].4s,$word0
	dup	@datax[1].4s,$word1
	dup	@datax[2].4s,$word2
	mov	@datax[3].s[0],$ctr
	add	$ctr,$ctr,#1
	mov	$datax[3].s[1],$ctr
	add	$ctr,$ctr,#1
	mov	@datax[3].s[2],$ctr
	add	$ctr,$ctr,#1
	mov	@datax[3].s[3],$ctr
	add	$ctr,$ctr,#1
	bl	_${prefix}_enc_8blks
	ld4	{@vtmpx[0].4s,@vtmpx[1].4s,@vtmpx[2].4s,@vtmpx[3].4s},[$inp],#64
	ld4	{@datax[0].4s,@datax[1].4s,@datax[2].4s,@datax[3].4s},[$inp],#64
	eor	@vtmp[0].16b,@vtmp[0].16b,@vtmpx[0].16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@vtmpx[1].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@vtmpx[2].16b
	eor	@vtmp[3].16b,@vtmp[3].16b,@vtmpx[3].16b
	eor	@data[0].16b,@data[0].16b,@datax[0].16b
	eor	@data[1].16b,@data[1].16b,@datax[1].16b
	eor	@data[2].16b,@data[2].16b,@datax[2].16b
	eor	@data[3].16b,@data[3].16b,@datax[3].16b
	st4	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	st4	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#8
	b.ne	.Lctr32_4_blocks_process
	b	100f
1:	// last block processing
	subs	$blocks,$blocks,#1
	b.lt	100f
	b.gt	1f
	mov	$ivec.s[0],$word0
	mov	$ivec.s[1],$word1
	mov	$ivec.s[2],$word2
	mov	$ivec.s[3],$ctr
___
	&encrypt_1blk($ivec);
$code.=<<___;
	ld1	{@data[0].4s},[$inp]
	eor	@data[0].16b,@data[0].16b,$ivec.16b
	st1	{@data[0].4s},[$outp]
	b	100f
1:	// last 2 blocks processing
	dup	@data[0].4s,$word0
	dup	@data[1].4s,$word1
	dup	@data[2].4s,$word2
	mov	@data[3].s[0],$ctr
	add	$ctr,$ctr,#1
	mov	@data[3].s[1],$ctr
	subs	$blocks,$blocks,#1
	b.ne	1f
	bl	_${prefix}_enc_4blks
	ld4	{@vtmpx[0].s,@vtmpx[1].s,@vtmpx[2].s,@vtmpx[3].s}[0],[$inp],#16
	ld4	{@vtmpx[0].s,@vtmpx[1].s,@vtmpx[2].s,@vtmpx[3].s}[1],[$inp],#16
	eor	@vtmp[0].16b,@vtmp[0].16b,@vtmpx[0].16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@vtmpx[1].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@vtmpx[2].16b
	eor	@vtmp[3].16b,@vtmp[3].16b,@vtmpx[3].16b
	st4	{@vtmp[0].s,@vtmp[1].s,@vtmp[2].s,@vtmp[3].s}[0],[$outp],#16
	st4	{@vtmp[0].s,@vtmp[1].s,@vtmp[2].s,@vtmp[3].s}[1],[$outp],#16
	b	100f
1:	// last 3 blocks processing
	add	$ctr,$ctr,#1
	mov	@data[3].s[2],$ctr
	bl	_${prefix}_enc_4blks
	ld4	{@vtmpx[0].s,@vtmpx[1].s,@vtmpx[2].s,@vtmpx[3].s}[0],[$inp],#16
	ld4	{@vtmpx[0].s,@vtmpx[1].s,@vtmpx[2].s,@vtmpx[3].s}[1],[$inp],#16
	ld4	{@vtmpx[0].s,@vtmpx[1].s,@vtmpx[2].s,@vtmpx[3].s}[2],[$inp],#16
	eor	@vtmp[0].16b,@vtmp[0].16b,@vtmpx[0].16b
	eor	@vtmp[1].16b,@vtmp[1].16b,@vtmpx[1].16b
	eor	@vtmp[2].16b,@vtmp[2].16b,@vtmpx[2].16b
	eor	@vtmp[3].16b,@vtmp[3].16b,@vtmpx[3].16b
	st4	{@vtmp[0].s,@vtmp[1].s,@vtmp[2].s,@vtmp[3].s}[0],[$outp],#16
	st4	{@vtmp[0].s,@vtmp[1].s,@vtmp[2].s,@vtmp[3].s}[1],[$outp],#16
	st4	{@vtmp[0].s,@vtmp[1].s,@vtmp[2].s,@vtmp[3].s}[2],[$outp],#16
100:
	ldp	d10,d11,[sp,#16]
	ldp	d12,d13,[sp,#32]
	ldp	d14,d15,[sp,#48]
	ldp	x29,x30,[sp,#64]
	ldp	d8,d9,[sp],#80
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_ctr32_encrypt_blocks,.-${prefix}_ctr32_encrypt_blocks
___
}}}


{{{
my ($blocks,$len)=("x2","x2");
my $ivp=("x5");
my @twx=map("x$_",(12..27));
my ($rks1,$rks2)=("x26","x27");
my $lastBlk=("x26");
my $enc=("w28");
my $remain=("x29");

my @tweak=map("v$_",(16..23));
my $lastTweak=("v25");

sub gen_xts_cipher() {
	my $std = shift;
$code.=<<___;
.globl	${prefix}_xts_encrypt${std}
.type	${prefix}_xts_encrypt${std},%function
.align	5
${prefix}_xts_encrypt${std}:
	AARCH64_SIGN_LINK_REGISTER
	stp	x15, x16, [sp, #-0x10]!
	stp	x17, x18, [sp, #-0x10]!
	stp	x19, x20, [sp, #-0x10]!
	stp	x21, x22, [sp, #-0x10]!
	stp	x23, x24, [sp, #-0x10]!
	stp	x25, x26, [sp, #-0x10]!
	stp	x27, x28, [sp, #-0x10]!
	stp	x29, x30, [sp, #-0x10]!
	stp	d8, d9, [sp, #-0x10]!
	stp	d10, d11, [sp, #-0x10]!
	stp	d12, d13, [sp, #-0x10]!
	stp	d14, d15, [sp, #-0x10]!
	mov	$rks1,x3
	mov	$rks2,x4
	mov	$enc,w6
	ld1	{@tweak[0].4s}, [$ivp]
	mov	$rks,$rks2
___
	&load_sbox();
	&rev32(@tweak[0],@tweak[0]);
	&encrypt_1blk(@tweak[0]);
$code.=<<___;
	mov	$rks,$rks1
	and	$remain,$len,#0x0F
	// convert length into blocks
	lsr	$blocks,$len,4
	cmp	$blocks,#1
	b.lt .return${std}

	cmp $remain,0
	// If the encryption/decryption Length is N times of 16,
	// the all blocks are encrypted/decrypted in .xts_encrypt_blocks${std}
	b.eq .xts_encrypt_blocks${std}

	// If the encryption/decryption length is not N times of 16,
	// the last two blocks are encrypted/decrypted in .last_2blks_tweak${std} or .only_2blks_tweak${std}
	// the other blocks are encrypted/decrypted in .xts_encrypt_blocks${std}
	subs $blocks,$blocks,#1
	b.eq .only_2blks_tweak${std}
.xts_encrypt_blocks${std}:
___
	&rbit(@tweak[0],@tweak[0],$std);
	&rev32_armeb(@tweak[0],@tweak[0]);
	&mov_vec_to_reg(@tweak[0],@twx[0],@twx[1]);
	&compute_tweak(@twx[0],@twx[1],@twx[2],@twx[3]);
	&compute_tweak(@twx[2],@twx[3],@twx[4],@twx[5]);
	&compute_tweak(@twx[4],@twx[5],@twx[6],@twx[7]);
	&compute_tweak(@twx[6],@twx[7],@twx[8],@twx[9]);
	&compute_tweak(@twx[8],@twx[9],@twx[10],@twx[11]);
	&compute_tweak(@twx[10],@twx[11],@twx[12],@twx[13]);
	&compute_tweak(@twx[12],@twx[13],@twx[14],@twx[15]);
$code.=<<___;
.Lxts_8_blocks_process${std}:
	cmp	$blocks,#8
___
	&mov_reg_to_vec(@twx[0],@twx[1],@tweak[0]);
	&compute_tweak(@twx[14],@twx[15],@twx[0],@twx[1]);
	&mov_reg_to_vec(@twx[2],@twx[3],@tweak[1]);
	&compute_tweak(@twx[0],@twx[1],@twx[2],@twx[3]);
	&mov_reg_to_vec(@twx[4],@twx[5],@tweak[2]);
	&compute_tweak(@twx[2],@twx[3],@twx[4],@twx[5]);
	&mov_reg_to_vec(@twx[6],@twx[7],@tweak[3]);
	&compute_tweak(@twx[4],@twx[5],@twx[6],@twx[7]);
	&mov_reg_to_vec(@twx[8],@twx[9],@tweak[4]);
	&compute_tweak(@twx[6],@twx[7],@twx[8],@twx[9]);
	&mov_reg_to_vec(@twx[10],@twx[11],@tweak[5]);
	&compute_tweak(@twx[8],@twx[9],@twx[10],@twx[11]);
	&mov_reg_to_vec(@twx[12],@twx[13],@tweak[6]);
	&compute_tweak(@twx[10],@twx[11],@twx[12],@twx[13]);
	&mov_reg_to_vec(@twx[14],@twx[15],@tweak[7]);
	&compute_tweak(@twx[12],@twx[13],@twx[14],@twx[15]);
$code.=<<___;
	b.lt	.Lxts_4_blocks_process${std}
	ld1 {@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
___
	&rbit(@tweak[0],@tweak[0],$std);
	&rbit(@tweak[1],@tweak[1],$std);
	&rbit(@tweak[2],@tweak[2],$std);
	&rbit(@tweak[3],@tweak[3],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	eor @data[1].16b, @data[1].16b, @tweak[1].16b
	eor @data[2].16b, @data[2].16b, @tweak[2].16b
	eor @data[3].16b, @data[3].16b, @tweak[3].16b
	ld1	{@datax[0].4s,$datax[1].4s,@datax[2].4s,@datax[3].4s},[$inp],#64
___
	&rbit(@tweak[4],@tweak[4],$std);
	&rbit(@tweak[5],@tweak[5],$std);
	&rbit(@tweak[6],@tweak[6],$std);
	&rbit(@tweak[7],@tweak[7],$std);
$code.=<<___;
	eor @datax[0].16b, @datax[0].16b, @tweak[4].16b
	eor @datax[1].16b, @datax[1].16b, @tweak[5].16b
	eor @datax[2].16b, @datax[2].16b, @tweak[6].16b
	eor @datax[3].16b, @datax[3].16b, @tweak[7].16b
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
	&rev32(@datax[0],@datax[0]);
	&rev32(@datax[1],@datax[1]);
	&rev32(@datax[2],@datax[2]);
	&rev32(@datax[3],@datax[3]);
	&transpose(@data,@vtmp);
	&transpose(@datax,@vtmp);
$code.=<<___;
	bl	_${prefix}_enc_8blks
___
	&transpose(@vtmp,@datax);
	&transpose(@data,@datax);
$code.=<<___;
	eor @vtmp[0].16b, @vtmp[0].16b, @tweak[0].16b
	eor @vtmp[1].16b, @vtmp[1].16b, @tweak[1].16b
	eor @vtmp[2].16b, @vtmp[2].16b, @tweak[2].16b
	eor @vtmp[3].16b, @vtmp[3].16b, @tweak[3].16b
	eor @data[0].16b, @data[0].16b, @tweak[4].16b
	eor @data[1].16b, @data[1].16b, @tweak[5].16b
	eor @data[2].16b, @data[2].16b, @tweak[6].16b
	eor @data[3].16b, @data[3].16b, @tweak[7].16b

	// save the last tweak
	mov $lastTweak.16b,@tweak[7].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	st1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#8
	b.gt	.Lxts_8_blocks_process${std}
	b	100f
.Lxts_4_blocks_process${std}:
	cmp	$blocks,#4
	b.lt	1f
	ld1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
___
	&rbit(@tweak[0],@tweak[0],$std);
	&rbit(@tweak[1],@tweak[1],$std);
	&rbit(@tweak[2],@tweak[2],$std);
	&rbit(@tweak[3],@tweak[3],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	eor @data[1].16b, @data[1].16b, @tweak[1].16b
	eor @data[2].16b, @data[2].16b, @tweak[2].16b
	eor @data[3].16b, @data[3].16b, @tweak[3].16b
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&rev32(@data[3],@data[3]);
	&transpose(@data,@vtmp);
$code.=<<___;
	bl	_${prefix}_enc_4blks
___
	&transpose(@vtmp,@data);
$code.=<<___;
	eor @vtmp[0].16b, @vtmp[0].16b, @tweak[0].16b
	eor @vtmp[1].16b, @vtmp[1].16b, @tweak[1].16b
	eor @vtmp[2].16b, @vtmp[2].16b, @tweak[2].16b
	eor @vtmp[3].16b, @vtmp[3].16b, @tweak[3].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	sub	$blocks,$blocks,#4
	mov @tweak[0].16b,@tweak[4].16b
	mov @tweak[1].16b,@tweak[5].16b
	mov @tweak[2].16b,@tweak[6].16b
	// save the last tweak
	mov $lastTweak.16b,@tweak[3].16b
1:
	// process last block
	cmp	$blocks,#1
	b.lt	100f
	b.gt	1f
	ld1	{@data[0].4s},[$inp],#16
___
	&rbit(@tweak[0],@tweak[0],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
___
	&rev32(@data[0],@data[0]);
	&encrypt_1blk(@data[0]);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	st1	{@data[0].4s},[$outp],#16
	// save the last tweak
	mov $lastTweak.16b,@tweak[0].16b
	b	100f
1:  // process last 2 blocks
	cmp	$blocks,#2
	b.gt	1f
	ld1	{@data[0].4s,@data[1].4s},[$inp],#32
___
	&rbit(@tweak[0],@tweak[0],$std);
	&rbit(@tweak[1],@tweak[1],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	eor @data[1].16b, @data[1].16b, @tweak[1].16b
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&transpose(@data,@vtmp);
$code.=<<___;
	bl	_${prefix}_enc_4blks
___
	&transpose(@vtmp,@data);
$code.=<<___;
	eor @vtmp[0].16b, @vtmp[0].16b, @tweak[0].16b
	eor @vtmp[1].16b, @vtmp[1].16b, @tweak[1].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s},[$outp],#32
	// save the last tweak
	mov $lastTweak.16b,@tweak[1].16b
	b	100f
1:  // process last 3 blocks
	ld1	{@data[0].4s,@data[1].4s,@data[2].4s},[$inp],#48
___
	&rbit(@tweak[0],@tweak[0],$std);
	&rbit(@tweak[1],@tweak[1],$std);
	&rbit(@tweak[2],@tweak[2],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	eor @data[1].16b, @data[1].16b, @tweak[1].16b
	eor @data[2].16b, @data[2].16b, @tweak[2].16b
___
	&rev32(@data[0],@data[0]);
	&rev32(@data[1],@data[1]);
	&rev32(@data[2],@data[2]);
	&transpose(@data,@vtmp);
$code.=<<___;
	bl	_${prefix}_enc_4blks
___
	&transpose(@vtmp,@data);
$code.=<<___;
	eor @vtmp[0].16b, @vtmp[0].16b, @tweak[0].16b
	eor @vtmp[1].16b, @vtmp[1].16b, @tweak[1].16b
	eor @vtmp[2].16b, @vtmp[2].16b, @tweak[2].16b
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s},[$outp],#48
	// save the last tweak
	mov $lastTweak.16b,@tweak[2].16b
100:
	cmp $remain,0
	b.eq .return${std}

// This branch calculates the last two tweaks, 
// while the encryption/decryption length is larger than 32
.last_2blks_tweak${std}:
___
	&rev32_armeb($lastTweak,$lastTweak);
	&compute_tweak_vec($lastTweak,@tweak[1],$std);
	&compute_tweak_vec(@tweak[1],@tweak[2],$std);
$code.=<<___;
	b .check_dec${std}


// This branch calculates the last two tweaks, 
// while the encryption/decryption length is equal to 32, who only need two tweaks
.only_2blks_tweak${std}:
	mov @tweak[1].16b,@tweak[0].16b
___
	&rev32_armeb(@tweak[1],@tweak[1]);
	&compute_tweak_vec(@tweak[1],@tweak[2],$std);
$code.=<<___;
	b .check_dec${std}


// Determine whether encryption or decryption is required.
// The last two tweaks need to be swapped for decryption.
.check_dec${std}:
	// encryption:1 decryption:0
	cmp $enc,1
	b.eq .process_last_2blks${std}
	mov @vtmp[0].16B,@tweak[1].16b
	mov @tweak[1].16B,@tweak[2].16b
	mov @tweak[2].16B,@vtmp[0].16b

.process_last_2blks${std}:
___
	&rev32_armeb(@tweak[1],@tweak[1]);
	&rev32_armeb(@tweak[2],@tweak[2]);
$code.=<<___;
	ld1	{@data[0].4s},[$inp],#16
	eor @data[0].16b, @data[0].16b, @tweak[1].16b
___
	&rev32(@data[0],@data[0]);
	&encrypt_1blk(@data[0]);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[1].16b
	st1	{@data[0].4s},[$outp],#16

	sub $lastBlk,$outp,16
	.loop${std}:
		subs $remain,$remain,1
		ldrb	$wtmp0,[$lastBlk,$remain]
		ldrb	$wtmp1,[$inp,$remain]
		strb	$wtmp1,[$lastBlk,$remain]
		strb	$wtmp0,[$outp,$remain]
	b.gt .loop${std}
	ld1		{@data[0].4s}, [$lastBlk]	
	eor @data[0].16b, @data[0].16b, @tweak[2].16b
___
	&rev32(@data[0],@data[0]);
	&encrypt_1blk(@data[0]);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @tweak[2].16b
	st1		{@data[0].4s}, [$lastBlk]
.return${std}:
	ldp		d14, d15, [sp], #0x10
	ldp		d12, d13, [sp], #0x10
	ldp		d10, d11, [sp], #0x10
	ldp		d8, d9, [sp], #0x10
	ldp		x29, x30, [sp], #0x10
	ldp		x27, x28, [sp], #0x10
	ldp		x25, x26, [sp], #0x10
	ldp		x23, x24, [sp], #0x10
	ldp		x21, x22, [sp], #0x10
	ldp		x19, x20, [sp], #0x10
	ldp		x17, x18, [sp], #0x10
	ldp		x15, x16, [sp], #0x10
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	${prefix}_xts_encrypt${std},.-${prefix}_xts_encrypt${std}
___
} # end of gen_xts_cipher
&gen_xts_cipher("_gb");
&gen_xts_cipher("");
}}}

########################################
open SELF,$0;
while(<SELF>) {
		next if (/^#!/);
		last if (!s/^#/\/\// and !/^$/);
		print;
}
close SELF;

foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/ge;
	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
