#! /usr/bin/env perl
# Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# This module implements SM4 with ASIMD on aarch64
#
# Feb 2022
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

$prefix="vpsm4";
my @vtmp=map("v$_",(0..3));
my @qtmp=map("q$_",(0..3));
my @data=map("v$_",(4..7));
my @datax=map("v$_",(8..11));
my ($rk0,$rk1)=("v12","v13");
my ($rka,$rkb)=("v14","v15");
my @vtmpx=map("v$_",(12..15));
my @sbox=map("v$_",(16..31));
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

# sbox operations for 4-lane of words
sub sbox() {
	my $dat = shift;

$code.=<<___;
	movi	@vtmp[0].16b,#64
	movi	@vtmp[1].16b,#128
	movi	@vtmp[2].16b,#192
	sub	@vtmp[0].16b,$dat.16b,@vtmp[0].16b
	sub	@vtmp[1].16b,$dat.16b,@vtmp[1].16b
	sub	@vtmp[2].16b,$dat.16b,@vtmp[2].16b
	tbl	$dat.16b,{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},$dat.16b
	tbl	@vtmp[0].16b,{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},@vtmp[0].16b
	tbl	@vtmp[1].16b,{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},@vtmp[1].16b
	tbl	@vtmp[2].16b,{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},@vtmp[2].16b
	add	@vtmp[0].2d,@vtmp[0].2d,@vtmp[1].2d
	add	@vtmp[2].2d,@vtmp[2].2d,$dat.2d
	add	$dat.2d,@vtmp[0].2d,@vtmp[2].2d

	ushr	@vtmp[0].4s,$dat.4s,32-2
	sli	@vtmp[0].4s,$dat.4s,2
	ushr	@vtmp[2].4s,$dat.4s,32-10
	eor	@vtmp[1].16b,@vtmp[0].16b,$dat.16b
	sli	@vtmp[2].4s,$dat.4s,10
	eor	@vtmp[1].16b,@vtmp[2].16b,$vtmp[1].16b
	ushr	@vtmp[0].4s,$dat.4s,32-18
	sli	@vtmp[0].4s,$dat.4s,18
	ushr	@vtmp[2].4s,$dat.4s,32-24
	eor	@vtmp[1].16b,@vtmp[0].16b,$vtmp[1].16b
	sli	@vtmp[2].4s,$dat.4s,24
	eor	$dat.16b,@vtmp[2].16b,@vtmp[1].16b
___
}

# sbox operation for 8-lane of words
sub sbox_double() {
	my $dat = shift;
	my $datx = shift;

$code.=<<___;
	movi	@vtmp[3].16b,#64
	sub	@vtmp[0].16b,$dat.16b,@vtmp[3].16b
	sub	@vtmp[1].16b,@vtmp[0].16b,@vtmp[3].16b
	sub	@vtmp[2].16b,@vtmp[1].16b,@vtmp[3].16b
	tbl	$dat.16b,{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},$dat.16b
	tbl	@vtmp[0].16b,{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},@vtmp[0].16b
	tbl	@vtmp[1].16b,{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},@vtmp[1].16b
	tbl	@vtmp[2].16b,{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},@vtmp[2].16b
	add	@vtmp[1].2d,@vtmp[0].2d,@vtmp[1].2d
	add	$dat.2d,@vtmp[2].2d,$dat.2d
	add	$dat.2d,@vtmp[1].2d,$dat.2d

	sub	@vtmp[0].16b,$datx.16b,@vtmp[3].16b
	sub	@vtmp[1].16b,@vtmp[0].16b,@vtmp[3].16b
	sub	@vtmp[2].16b,@vtmp[1].16b,@vtmp[3].16b
	tbl	$datx.16b,{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},$datx.16b
	tbl	@vtmp[0].16b,{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},@vtmp[0].16b
	tbl	@vtmp[1].16b,{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},@vtmp[1].16b
	tbl	@vtmp[2].16b,{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},@vtmp[2].16b
	add	@vtmp[1].2d,@vtmp[0].2d,@vtmp[1].2d
	add	$datx.2d,@vtmp[2].2d,$datx.2d
	add	$datx.2d,@vtmp[1].2d,$datx.2d

	ushr	@vtmp[0].4s,$dat.4s,32-2
	sli	@vtmp[0].4s,$dat.4s,2
	ushr	@vtmp[2].4s,$datx.4s,32-2
	eor	@vtmp[1].16b,@vtmp[0].16b,$dat.16b
	sli	@vtmp[2].4s,$datx.4s,2

	ushr	@vtmp[0].4s,$dat.4s,32-10
	eor	@vtmp[3].16b,@vtmp[2].16b,$datx.16b
	sli	@vtmp[0].4s,$dat.4s,10
	ushr	@vtmp[2].4s,$datx.4s,32-10
	eor	@vtmp[1].16b,@vtmp[0].16b,$vtmp[1].16b
	sli	@vtmp[2].4s,$datx.4s,10

	ushr	@vtmp[0].4s,$dat.4s,32-18
	eor	@vtmp[3].16b,@vtmp[2].16b,$vtmp[3].16b
	sli	@vtmp[0].4s,$dat.4s,18
	ushr	@vtmp[2].4s,$datx.4s,32-18
	eor	@vtmp[1].16b,@vtmp[0].16b,$vtmp[1].16b
	sli	@vtmp[2].4s,$datx.4s,18

	ushr	@vtmp[0].4s,$dat.4s,32-24
	eor	@vtmp[3].16b,@vtmp[2].16b,$vtmp[3].16b
	sli	@vtmp[0].4s,$dat.4s,24
	ushr	@vtmp[2].4s,$datx.4s,32-24
	eor	$dat.16b,@vtmp[0].16b,@vtmp[1].16b
	sli	@vtmp[2].4s,$datx.4s,24
	eor	$datx.16b,@vtmp[2].16b,@vtmp[3].16b
___
}

# sbox operation for one single word
sub sbox_1word () {
	my $word = shift;

$code.=<<___;
	movi	@vtmp[1].16b,#64
	movi	@vtmp[2].16b,#128
	movi	@vtmp[3].16b,#192
	mov	@vtmp[0].s[0],$word

	sub	@vtmp[1].16b,@vtmp[0].16b,@vtmp[1].16b
	sub	@vtmp[2].16b,@vtmp[0].16b,@vtmp[2].16b
	sub	@vtmp[3].16b,@vtmp[0].16b,@vtmp[3].16b

	tbl	@vtmp[0].16b,{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},@vtmp[0].16b
	tbl	@vtmp[1].16b,{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},@vtmp[1].16b
	tbl	@vtmp[2].16b,{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},@vtmp[2].16b
	tbl	@vtmp[3].16b,{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},@vtmp[3].16b

	mov	$word,@vtmp[0].s[0]
	mov	$wtmp0,@vtmp[1].s[0]
	mov	$wtmp2,@vtmp[2].s[0]
	add	$wtmp0,$word,$wtmp0
	mov	$word,@vtmp[3].s[0]
	add	$wtmp0,$wtmp0,$wtmp2
	add	$wtmp0,$wtmp0,$word

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
	adrp	$ptr,.Lsbox
	add	$ptr,$ptr,#:lo12:.Lsbox
	ld1	{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},[$ptr],#64
	ld1	{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},[$ptr],#64
	ld1	{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},[$ptr],#64
	ld1	{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},[$ptr]
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
	adrp $ptr,.Lxts_magic
	ldr  @qtmp[0], [$ptr, #:lo12:.Lxts_magic]
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
.arch	armv8-a
.text

.rodata
.type	_${prefix}_consts,%object
.align	7
_${prefix}_consts:
.Lsbox:
	.byte 0xD6,0x90,0xE9,0xFE,0xCC,0xE1,0x3D,0xB7,0x16,0xB6,0x14,0xC2,0x28,0xFB,0x2C,0x05
	.byte 0x2B,0x67,0x9A,0x76,0x2A,0xBE,0x04,0xC3,0xAA,0x44,0x13,0x26,0x49,0x86,0x06,0x99
	.byte 0x9C,0x42,0x50,0xF4,0x91,0xEF,0x98,0x7A,0x33,0x54,0x0B,0x43,0xED,0xCF,0xAC,0x62
	.byte 0xE4,0xB3,0x1C,0xA9,0xC9,0x08,0xE8,0x95,0x80,0xDF,0x94,0xFA,0x75,0x8F,0x3F,0xA6
	.byte 0x47,0x07,0xA7,0xFC,0xF3,0x73,0x17,0xBA,0x83,0x59,0x3C,0x19,0xE6,0x85,0x4F,0xA8
	.byte 0x68,0x6B,0x81,0xB2,0x71,0x64,0xDA,0x8B,0xF8,0xEB,0x0F,0x4B,0x70,0x56,0x9D,0x35
	.byte 0x1E,0x24,0x0E,0x5E,0x63,0x58,0xD1,0xA2,0x25,0x22,0x7C,0x3B,0x01,0x21,0x78,0x87
	.byte 0xD4,0x00,0x46,0x57,0x9F,0xD3,0x27,0x52,0x4C,0x36,0x02,0xE7,0xA0,0xC4,0xC8,0x9E
	.byte 0xEA,0xBF,0x8A,0xD2,0x40,0xC7,0x38,0xB5,0xA3,0xF7,0xF2,0xCE,0xF9,0x61,0x15,0xA1
	.byte 0xE0,0xAE,0x5D,0xA4,0x9B,0x34,0x1A,0x55,0xAD,0x93,0x32,0x30,0xF5,0x8C,0xB1,0xE3
	.byte 0x1D,0xF6,0xE2,0x2E,0x82,0x66,0xCA,0x60,0xC0,0x29,0x23,0xAB,0x0D,0x53,0x4E,0x6F
	.byte 0xD5,0xDB,0x37,0x45,0xDE,0xFD,0x8E,0x2F,0x03,0xFF,0x6A,0x72,0x6D,0x6C,0x5B,0x51
	.byte 0x8D,0x1B,0xAF,0x92,0xBB,0xDD,0xBC,0x7F,0x11,0xD9,0x5C,0x41,0x1F,0x10,0x5A,0xD8
	.byte 0x0A,0xC1,0x31,0x88,0xA5,0xCD,0x7B,0xBD,0x2D,0x74,0xD0,0x12,0xB8,0xE5,0xB4,0xB0
	.byte 0x89,0x69,0x97,0x4A,0x0C,0x96,0x77,0x7E,0x65,0xB9,0xF1,0x09,0xC5,0x6E,0xC6,0x84
	.byte 0x18,0xF0,0x7D,0xEC,0x3A,0xDC,0x4D,0x20,0x79,0xEE,0x5F,0x3E,0xD7,0xCB,0x39,0x48
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

.size	_${prefix}_consts,.-_${prefix}_consts

.previous

___

{{{
my ($key,$keys,$enc)=("x0","x1","w2");
my ($pointer,$schedules,$wtmp,$roundkey)=("x5","x6","w7","w8");
my ($vkey,$vfk,$vmap)=("v5","v6","v7");
$code.=<<___;
.type	_vpsm4_set_key,%function
.align	4
_vpsm4_set_key:
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
	// sbox lookup
	mov	@data[0].s[0],$roundkey
	tbl	@vtmp[1].16b,{@sbox[0].16b,@sbox[1].16b,@sbox[2].16b,@sbox[3].16b},@data[0].16b
	sub	@data[0].16b,@data[0].16b,@vtmp[0].16b
	tbx	@vtmp[1].16b,{@sbox[4].16b,@sbox[5].16b,@sbox[6].16b,@sbox[7].16b},@data[0].16b
	sub	@data[0].16b,@data[0].16b,@vtmp[0].16b
	tbx	@vtmp[1].16b,{@sbox[8].16b,@sbox[9].16b,@sbox[10].16b,@sbox[11].16b},@data[0].16b
	sub	@data[0].16b,@data[0].16b,@vtmp[0].16b
	tbx	@vtmp[1].16b,{@sbox[12].16b,@sbox[13].16b,@sbox[14].16b,@sbox[15].16b},@data[0].16b
	mov	$wtmp,@vtmp[1].s[0]
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
.size	_vpsm4_set_key,.-_vpsm4_set_key
___
}}}


{{{
$code.=<<___;
.type	_vpsm4_enc_4blks,%function
.align	4
_vpsm4_enc_4blks:
	AARCH64_VALID_CALL_TARGET
___
	&encrypt_4blks();
$code.=<<___;
	ret
.size	_vpsm4_enc_4blks,.-_vpsm4_enc_4blks
___
}}}

{{{
$code.=<<___;
.type	_vpsm4_enc_8blks,%function
.align	4
_vpsm4_enc_8blks:
	AARCH64_VALID_CALL_TARGET
___
	&encrypt_8blks();
$code.=<<___;
	ret
.size	_vpsm4_enc_8blks,.-_vpsm4_enc_8blks
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
	bl	_vpsm4_set_key
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
	bl	_vpsm4_set_key
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
	mov	$rks,x2
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
my ($enc) = ("w4");
my @dat=map("v$_",(16..23));

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
	bl	_vpsm4_enc_8blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_8blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_8blks
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
	bl	_vpsm4_enc_4blks
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
	bl	_vpsm4_enc_4blks
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

my @tweak=@datax;

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
	b.lt	.Lxts_4_blocks_process${std}
___
	&mov_reg_to_vec(@twx[0],@twx[1],@vtmp[0]);
	&mov_reg_to_vec(@twx[2],@twx[3],@vtmp[1]);
	&mov_reg_to_vec(@twx[4],@twx[5],@vtmp[2]);
	&mov_reg_to_vec(@twx[6],@twx[7],@vtmp[3]);
	&mov_reg_to_vec(@twx[8],@twx[9],@vtmpx[0]);
	&mov_reg_to_vec(@twx[10],@twx[11],@vtmpx[1]);
	&mov_reg_to_vec(@twx[12],@twx[13],@vtmpx[2]);
	&mov_reg_to_vec(@twx[14],@twx[15],@vtmpx[3]);
$code.=<<___;
	ld1 {@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$inp],#64
___
	&rbit(@vtmp[0],@vtmp[0],$std);
	&rbit(@vtmp[1],@vtmp[1],$std);
	&rbit(@vtmp[2],@vtmp[2],$std);
	&rbit(@vtmp[3],@vtmp[3],$std);
$code.=<<___;
	eor @data[0].16b, @data[0].16b, @vtmp[0].16b
	eor @data[1].16b, @data[1].16b, @vtmp[1].16b
	eor @data[2].16b, @data[2].16b, @vtmp[2].16b
	eor @data[3].16b, @data[3].16b, @vtmp[3].16b
	ld1	{@datax[0].4s,$datax[1].4s,@datax[2].4s,@datax[3].4s},[$inp],#64
___
	&rbit(@vtmpx[0],@vtmpx[0],$std);
	&rbit(@vtmpx[1],@vtmpx[1],$std);
	&rbit(@vtmpx[2],@vtmpx[2],$std);
	&rbit(@vtmpx[3],@vtmpx[3],$std);
$code.=<<___;
	eor @datax[0].16b, @datax[0].16b, @vtmpx[0].16b
	eor @datax[1].16b, @datax[1].16b, @vtmpx[1].16b
	eor @datax[2].16b, @datax[2].16b, @vtmpx[2].16b
	eor @datax[3].16b, @datax[3].16b, @vtmpx[3].16b
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

	&mov_reg_to_vec(@twx[0],@twx[1],@vtmpx[0]);
	&compute_tweak(@twx[14],@twx[15],@twx[0],@twx[1]);
	&mov_reg_to_vec(@twx[2],@twx[3],@vtmpx[1]);
	&compute_tweak(@twx[0],@twx[1],@twx[2],@twx[3]);
	&mov_reg_to_vec(@twx[4],@twx[5],@vtmpx[2]);
	&compute_tweak(@twx[2],@twx[3],@twx[4],@twx[5]);
	&mov_reg_to_vec(@twx[6],@twx[7],@vtmpx[3]);
	&compute_tweak(@twx[4],@twx[5],@twx[6],@twx[7]);
	&mov_reg_to_vec(@twx[8],@twx[9],@tweak[0]);
	&compute_tweak(@twx[6],@twx[7],@twx[8],@twx[9]);
	&mov_reg_to_vec(@twx[10],@twx[11],@tweak[1]);
	&compute_tweak(@twx[8],@twx[9],@twx[10],@twx[11]);
	&mov_reg_to_vec(@twx[12],@twx[13],@tweak[2]);
	&compute_tweak(@twx[10],@twx[11],@twx[12],@twx[13]);
	&mov_reg_to_vec(@twx[14],@twx[15],@tweak[3]);
	&compute_tweak(@twx[12],@twx[13],@twx[14],@twx[15]);
$code.=<<___;
	eor @vtmp[0].16b, @vtmp[0].16b, @vtmpx[0].16b
	eor @vtmp[1].16b, @vtmp[1].16b, @vtmpx[1].16b
	eor @vtmp[2].16b, @vtmp[2].16b, @vtmpx[2].16b
	eor @vtmp[3].16b, @vtmp[3].16b, @vtmpx[3].16b
	eor @data[0].16b, @data[0].16b, @tweak[0].16b
	eor @data[1].16b, @data[1].16b, @tweak[1].16b
	eor @data[2].16b, @data[2].16b, @tweak[2].16b
	eor @data[3].16b, @data[3].16b, @tweak[3].16b

	// save the last tweak
	st1	{@tweak[3].4s},[$ivp]
	st1	{@vtmp[0].4s,@vtmp[1].4s,@vtmp[2].4s,@vtmp[3].4s},[$outp],#64
	st1	{@data[0].4s,@data[1].4s,@data[2].4s,@data[3].4s},[$outp],#64
	subs	$blocks,$blocks,#8
	b.gt	.Lxts_8_blocks_process${std}
	b	100f
.Lxts_4_blocks_process${std}:
___
	&mov_reg_to_vec(@twx[0],@twx[1],@tweak[0]);
	&mov_reg_to_vec(@twx[2],@twx[3],@tweak[1]);
	&mov_reg_to_vec(@twx[4],@twx[5],@tweak[2]);
	&mov_reg_to_vec(@twx[6],@twx[7],@tweak[3]);
$code.=<<___;
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
___
	&mov_reg_to_vec(@twx[8],@twx[9],@tweak[0]);
	&mov_reg_to_vec(@twx[10],@twx[11],@tweak[1]);
	&mov_reg_to_vec(@twx[12],@twx[13],@tweak[2]);
$code.=<<___;
	// save the last tweak
	st1	{@tweak[3].4s},[$ivp]
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
	st1	{@tweak[0].4s},[$ivp]
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
	st1	{@tweak[1].4s},[$ivp]
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
	st1	{@tweak[2].4s},[$ivp]
100:
	cmp $remain,0
	b.eq .return${std}

// This branch calculates the last two tweaks, 
// while the encryption/decryption length is larger than 32
.last_2blks_tweak${std}:
	ld1	{@tweak[0].4s},[$ivp]
___
	&rev32_armeb(@tweak[0],@tweak[0]);
	&compute_tweak_vec(@tweak[0],@tweak[1],$std);
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
