#! /usr/bin/env perl
# Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# This module implements support for SM4 hw support on aarch64
# Oct 2021
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

$prefix="sm4_v8";
my @rks=map("v$_",(0..7));

sub rev32() {
my $dst = shift;
my $src = shift;
$code.=<<___;
#ifndef __AARCH64EB__
	rev32	$dst.16b,$src.16b
#endif
___
}

sub enc_blk () {
my $data = shift;
$code.=<<___;
	sm4e	$data.4s,@rks[0].4s
	sm4e	$data.4s,@rks[1].4s
	sm4e	$data.4s,@rks[2].4s
	sm4e	$data.4s,@rks[3].4s
	sm4e	$data.4s,@rks[4].4s
	sm4e	$data.4s,@rks[5].4s
	sm4e	$data.4s,@rks[6].4s
	sm4e	$data.4s,@rks[7].4s
	rev64	$data.4S,$data.4S
	ext	$data.16b,$data.16b,$data.16b,#8
___
}

sub enc_4blks () {
my $data0 = shift;
my $data1 = shift;
my $data2 = shift;
my $data3 = shift;
$code.=<<___;
	sm4e	$data0.4s,@rks[0].4s
	sm4e	$data1.4s,@rks[0].4s
	sm4e	$data2.4s,@rks[0].4s
	sm4e	$data3.4s,@rks[0].4s

	sm4e	$data0.4s,@rks[1].4s
	sm4e	$data1.4s,@rks[1].4s
	sm4e	$data2.4s,@rks[1].4s
	sm4e	$data3.4s,@rks[1].4s

	sm4e	$data0.4s,@rks[2].4s
	sm4e	$data1.4s,@rks[2].4s
	sm4e	$data2.4s,@rks[2].4s
	sm4e	$data3.4s,@rks[2].4s

	sm4e	$data0.4s,@rks[3].4s
	sm4e	$data1.4s,@rks[3].4s
	sm4e	$data2.4s,@rks[3].4s
	sm4e	$data3.4s,@rks[3].4s

	sm4e	$data0.4s,@rks[4].4s
	sm4e	$data1.4s,@rks[4].4s
	sm4e	$data2.4s,@rks[4].4s
	sm4e	$data3.4s,@rks[4].4s

	sm4e	$data0.4s,@rks[5].4s
	sm4e	$data1.4s,@rks[5].4s
	sm4e	$data2.4s,@rks[5].4s
	sm4e	$data3.4s,@rks[5].4s

	sm4e	$data0.4s,@rks[6].4s
	sm4e	$data1.4s,@rks[6].4s
	sm4e	$data2.4s,@rks[6].4s
	sm4e	$data3.4s,@rks[6].4s

	sm4e	$data0.4s,@rks[7].4s
	rev64	$data0.4S,$data0.4S
	sm4e	$data1.4s,@rks[7].4s
	ext	$data0.16b,$data0.16b,$data0.16b,#8
	rev64	$data1.4S,$data1.4S
	sm4e	$data2.4s,@rks[7].4s
	ext	$data1.16b,$data1.16b,$data1.16b,#8
	rev64	$data2.4S,$data2.4S
	sm4e	$data3.4s,@rks[7].4s
	ext	$data2.16b,$data2.16b,$data2.16b,#8
	rev64	$data3.4S,$data3.4S
	ext	$data3.16b,$data3.16b,$data3.16b,#8
___
}

$code=<<___;
#include "arm_arch.h"
.arch	armv8-a+crypto
.text
___

{{{
$code.=<<___;

.rodata
.type	_${prefix}_consts,%object
.align	6
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
	.long 0xa3b1bac6, 0x56aa3350, 0x677d9197, 0xb27022dc
.size _${prefix}_consts,.-_${prefix}_consts
.previous

___
}}}

{{{
my ($key,$keys)=("x0","x1");
my ($tmp)=("x2");
my ($key0,$key1,$key2,$key3,$key4,$key5,$key6,$key7)=map("v$_",(0..7));
my ($const0,$const1,$const2,$const3,$const4,$const5,$const6,$const7)=map("v$_",(16..23));
my ($fkconst) = ("v24");
$code.=<<___;
.globl	${prefix}_set_encrypt_key
.type	${prefix}_set_encrypt_key,%function
.align	5
${prefix}_set_encrypt_key:
	AARCH64_VALID_CALL_TARGET
	ld1	{$key0.4s},[$key]
	adrp	$tmp,.Lfk
	add	$tmp,$tmp,#:lo12:.Lfk
	ld1	{$fkconst.4s},[$tmp]
	adrp	$tmp,.Lck
	add	$tmp,$tmp,#:lo12:.Lck
	ld1	{$const0.4s,$const1.4s,$const2.4s,$const3.4s},[$tmp],64
___
	&rev32($key0, $key0);
$code.=<<___;
	ld1	{$const4.4s,$const5.4s,$const6.4s,$const7.4s},[$tmp]
	eor	$key0.16b,$key0.16b,$fkconst.16b;
	sm4ekey	$key0.4S,$key0.4S,$const0.4S
	sm4ekey	$key1.4S,$key0.4S,$const1.4S
	sm4ekey	$key2.4S,$key1.4S,$const2.4S
	sm4ekey	$key3.4S,$key2.4S,$const3.4S
	sm4ekey	$key4.4S,$key3.4S,$const4.4S
	st1	{$key0.4s,$key1.4s,$key2.4s,$key3.4s},[$keys],64
	sm4ekey	$key5.4S,$key4.4S,$const5.4S
	sm4ekey	$key6.4S,$key5.4S,$const6.4S
	sm4ekey	$key7.4S,$key6.4S,$const7.4S
	st1	{$key4.4s,$key5.4s,$key6.4s,$key7.4s},[$keys]
	ret
.size	${prefix}_set_encrypt_key,.-${prefix}_set_encrypt_key
___
}}}

{{{
my ($key,$keys)=("x0","x1");
my ($tmp)=("x2");
my ($key7,$key6,$key5,$key4,$key3,$key2,$key1,$key0)=map("v$_",(0..7));
my ($const0,$const1,$const2,$const3,$const4,$const5,$const6,$const7)=map("v$_",(16..23));
my ($fkconst) = ("v24");
$code.=<<___;
.globl	${prefix}_set_decrypt_key
.type	${prefix}_set_decrypt_key,%function
.align	5
${prefix}_set_decrypt_key:
	AARCH64_VALID_CALL_TARGET
	ld1	{$key0.4s},[$key]
	adrp	$tmp,.Lfk
	add	$tmp,$tmp,#:lo12:.Lfk
	ld1	{$fkconst.4s},[$tmp]
	adrp	$tmp,.Lck
	add	$tmp,$tmp,#:lo12:.Lck
	ld1	{$const0.4s,$const1.4s,$const2.4s,$const3.4s},[$tmp],64
___
	&rev32($key0, $key0);
$code.=<<___;
	ld1	{$const4.4s,$const5.4s,$const6.4s,$const7.4s},[$tmp]
	eor	$key0.16b, $key0.16b,$fkconst.16b;
	sm4ekey	$key0.4S,$key0.4S,$const0.4S
	sm4ekey	$key1.4S,$key0.4S,$const1.4S
	sm4ekey	$key2.4S,$key1.4S,$const2.4S
	rev64	$key0.4s,$key0.4s
	rev64	$key1.4s,$key1.4s
	ext	$key0.16b,$key0.16b,$key0.16b,#8
	ext	$key1.16b,$key1.16b,$key1.16b,#8
	sm4ekey	$key3.4S,$key2.4S,$const3.4S
	sm4ekey	$key4.4S,$key3.4S,$const4.4S
	rev64	$key2.4s,$key2.4s
	rev64	$key3.4s,$key3.4s
	ext	$key2.16b,$key2.16b,$key2.16b,#8
	ext	$key3.16b,$key3.16b,$key3.16b,#8
	sm4ekey	$key5.4S,$key4.4S,$const5.4S
	sm4ekey	$key6.4S,$key5.4S,$const6.4S
	rev64	$key4.4s,$key4.4s
	rev64	$key5.4s,$key5.4s
	ext	$key4.16b,$key4.16b,$key4.16b,#8
	ext	$key5.16b,$key5.16b,$key5.16b,#8
	sm4ekey	$key7.4S,$key6.4S,$const7.4S
	rev64	$key6.4s, $key6.4s
	rev64	$key7.4s, $key7.4s
	ext	$key6.16b,$key6.16b,$key6.16b,#8
	ext	$key7.16b,$key7.16b,$key7.16b,#8
	st1	{$key7.4s,$key6.4s,$key5.4s,$key4.4s},[$keys],64
	st1	{$key3.4s,$key2.4s,$key1.4s,$key0.4s},[$keys]
	ret
.size	${prefix}_set_decrypt_key,.-${prefix}_set_decrypt_key
___
}}}

{{{
sub gen_block () {
my $dir = shift;
my ($inp,$out,$rk)=map("x$_",(0..2));
my ($data)=("v16");
$code.=<<___;
.globl	${prefix}_${dir}crypt
.type	${prefix}_${dir}crypt,%function
.align	5
${prefix}_${dir}crypt:
	AARCH64_VALID_CALL_TARGET
	ld1	{$data.4s},[$inp]
	ld1	{@rks[0].4s,@rks[1].4s,@rks[2].4s,@rks[3].4s},[$rk],64
	ld1	{@rks[4].4s,@rks[5].4s,@rks[6].4s,@rks[7].4s},[$rk]
___
	&rev32($data,$data);
	&enc_blk($data);
	&rev32($data,$data);
$code.=<<___;
	st1	{$data.4s},[$out]
	ret
.size	${prefix}_${dir}crypt,.-${prefix}_${dir}crypt
___
}

&gen_block("en");
&gen_block("de");
}}}

{{{
my ($inp,$out,$len,$rk)=map("x$_",(0..3));
my ($enc) = ("w4");
my @dat=map("v$_",(16..23));
$code.=<<___;
.globl	${prefix}_ecb_encrypt
.type	${prefix}_ecb_encrypt,%function
.align	5
${prefix}_ecb_encrypt:
	AARCH64_VALID_CALL_TARGET
	ld1	{@rks[0].4s,@rks[1].4s,@rks[2].4s,@rks[3].4s},[$rk],#64
	ld1	{@rks[4].4s,@rks[5].4s,@rks[6].4s,@rks[7].4s},[$rk]
1:
	cmp	$len,#64
	b.lt	1f
	ld1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$inp],#64
	cmp	$len,#128
	b.lt	2f
	ld1	{@dat[4].4s,@dat[5].4s,@dat[6].4s,@dat[7].4s},[$inp],#64
	// 8 blocks
___
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&rev32(@dat[4],@dat[4]);
	&rev32(@dat[5],@dat[5]);
	&rev32(@dat[6],@dat[6]);
	&rev32(@dat[7],@dat[7]);
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&enc_4blks(@dat[4],@dat[5],@dat[6],@dat[7]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&rev32(@dat[4],@dat[4]);
	&rev32(@dat[5],@dat[5]);
$code.=<<___;
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
___
	&rev32(@dat[6],@dat[6]);
	&rev32(@dat[7],@dat[7]);
$code.=<<___;
	st1	{@dat[4].4s,@dat[5].4s,@dat[6].4s,@dat[7].4s},[$out],#64
	subs	$len,$len,#128
	b.gt	1b
	ret
	// 4 blocks
2:
___
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
$code.=<<___;
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	subs	$len,$len,#64
	b.gt	1b
1:
	subs	$len,$len,#16
	b.lt	1f
	ld1	{@dat[0].4s},[$inp],#16
___
	&rev32(@dat[0],@dat[0]);
	&enc_blk(@dat[0]);
	&rev32(@dat[0],@dat[0]);
$code.=<<___;
	st1	{@dat[0].4s},[$out],#16
	b.ne	1b
1:
	ret
.size	${prefix}_ecb_encrypt,.-${prefix}_ecb_encrypt
___
}}}

{{{
my ($inp,$out,$len,$rk,$ivp)=map("x$_",(0..4));
my ($enc) = ("w5");
my @dat=map("v$_",(16..23));
my @in=map("v$_",(24..31));
my ($ivec) = ("v8");
$code.=<<___;
.globl	${prefix}_cbc_encrypt
.type	${prefix}_cbc_encrypt,%function
.align	5
${prefix}_cbc_encrypt:
	AARCH64_VALID_CALL_TARGET
	stp	d8,d9,[sp, #-16]!

	ld1	{@rks[0].4s,@rks[1].4s,@rks[2].4s,@rks[3].4s},[$rk],#64
	ld1	{@rks[4].4s,@rks[5].4s,@rks[6].4s,@rks[7].4s},[$rk]
	ld1	{$ivec.4s},[$ivp]
	cmp	$enc,#0
	b.eq	.Ldec
1:
	cmp	$len, #64
	b.lt	1f
	ld1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$inp],#64
	eor	@dat[0].16b,@dat[0].16b,$ivec.16b
___
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&enc_blk(@dat[0]);
$code.=<<___;
	eor	@dat[1].16b,@dat[1].16b,@dat[0].16b
___
	&enc_blk(@dat[1]);
	&rev32(@dat[0],@dat[0]);
$code.=<<___;
	eor	@dat[2].16b,@dat[2].16b,@dat[1].16b
___
	&enc_blk(@dat[2]);
	&rev32(@dat[1],@dat[1]);
$code.=<<___;
	eor	@dat[3].16b,@dat[3].16b,@dat[2].16b
___
	&enc_blk(@dat[3]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
$code.=<<___;
	mov	$ivec.16b,@dat[3].16b
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	subs	$len,$len,#64
	b.ne	1b
1:
	subs	$len,$len,#16
	b.lt	3f
	ld1	{@dat[0].4s},[$inp],#16
	eor	$ivec.16b,$ivec.16b,@dat[0].16b
___
	&rev32($ivec,$ivec);
	&enc_blk($ivec);
	&rev32($ivec,$ivec);
$code.=<<___;
	st1	{$ivec.4s},[$out],#16
	b.ne	1b
	b	3f
.Ldec:
1:
	cmp	$len, #64
	b.lt	1f
	ld1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$inp]
	ld1	{@in[0].4s,@in[1].4s,@in[2].4s,@in[3].4s},[$inp],#64
	cmp	$len,#128
	b.lt	2f
	// 8 blocks mode
	ld1	{@dat[4].4s,@dat[5].4s,@dat[6].4s,@dat[7].4s},[$inp]
	ld1	{@in[4].4s,@in[5].4s,@in[6].4s,@in[7].4s},[$inp],#64
___
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],$dat[3]);
	&rev32(@dat[4],@dat[4]);
	&rev32(@dat[5],@dat[5]);
	&rev32(@dat[6],@dat[6]);
	&rev32(@dat[7],$dat[7]);
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&enc_4blks(@dat[4],@dat[5],@dat[6],@dat[7]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&rev32(@dat[4],@dat[4]);
	&rev32(@dat[5],@dat[5]);
	&rev32(@dat[6],@dat[6]);
	&rev32(@dat[7],@dat[7]);
$code.=<<___;
	eor	@dat[0].16b,@dat[0].16b,$ivec.16b
	eor	@dat[1].16b,@dat[1].16b,@in[0].16b
	eor	@dat[2].16b,@dat[2].16b,@in[1].16b
	mov	$ivec.16b,@in[7].16b
	eor	@dat[3].16b,$dat[3].16b,@in[2].16b
	eor	@dat[4].16b,$dat[4].16b,@in[3].16b
	eor	@dat[5].16b,$dat[5].16b,@in[4].16b
	eor	@dat[6].16b,$dat[6].16b,@in[5].16b
	eor	@dat[7].16b,$dat[7].16b,@in[6].16b
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	st1	{@dat[4].4s,@dat[5].4s,@dat[6].4s,@dat[7].4s},[$out],#64
	subs	$len,$len,128
	b.gt	1b
	b	3f
	// 4 blocks mode
2:
___
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],$dat[3]);
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
$code.=<<___;
	eor	@dat[0].16b,@dat[0].16b,$ivec.16b
	eor	@dat[1].16b,@dat[1].16b,@in[0].16b
	mov	$ivec.16b,@in[3].16b
	eor	@dat[2].16b,@dat[2].16b,@in[1].16b
	eor	@dat[3].16b,$dat[3].16b,@in[2].16b
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	subs	$len,$len,#64
	b.gt	1b
1:
	subs	$len,$len,#16
	b.lt	3f
	ld1	{@dat[0].4s},[$inp],#16
	mov	@in[0].16b,@dat[0].16b
___
	&rev32(@dat[0],@dat[0]);
	&enc_blk(@dat[0]);
	&rev32(@dat[0],@dat[0]);
$code.=<<___;
	eor	@dat[0].16b,@dat[0].16b,$ivec.16b
	mov	$ivec.16b,@in[0].16b
	st1	{@dat[0].4s},[$out],#16
	b.ne	1b
3:
	// save back IV
	st1	{$ivec.4s},[$ivp]
	ldp	d8,d9,[sp],#16
	ret
.size	${prefix}_cbc_encrypt,.-${prefix}_cbc_encrypt
___
}}}

{{{
my ($inp,$out,$len,$rk,$ivp)=map("x$_",(0..4));
my ($ctr)=("w5");
my @dat=map("v$_",(16..23));
my @in=map("v$_",(24..31));
my ($ivec)=("v8");
$code.=<<___;
.globl	${prefix}_ctr32_encrypt_blocks
.type	${prefix}_ctr32_encrypt_blocks,%function
.align	5
${prefix}_ctr32_encrypt_blocks:
	AARCH64_VALID_CALL_TARGET
	stp	d8,d9,[sp, #-16]!

	ld1	{$ivec.4s},[$ivp]
	ld1	{@rks[0].4s,@rks[1].4s,@rks[2].4s,@rks[3].4s},[$rk],64
	ld1	{@rks[4].4s,@rks[5].4s,@rks[6].4s,@rks[7].4s},[$rk]
___
	&rev32($ivec,$ivec);
$code.=<<___;
	mov	$ctr,$ivec.s[3]
1:
	cmp	$len,#4
	b.lt	1f
	ld1	{@in[0].4s,@in[1].4s,@in[2].4s,@in[3].4s},[$inp],#64
	mov	@dat[0].16b,$ivec.16b
	mov	@dat[1].16b,$ivec.16b
	mov	@dat[2].16b,$ivec.16b
	mov	@dat[3].16b,$ivec.16b
	add	$ctr,$ctr,#1
	mov	$dat[1].s[3],$ctr
	add	$ctr,$ctr,#1
	mov	@dat[2].s[3],$ctr
	add	$ctr,$ctr,#1
	mov	@dat[3].s[3],$ctr
	cmp	$len,#8
	b.lt	2f
	ld1	{@in[4].4s,@in[5].4s,@in[6].4s,@in[7].4s},[$inp],#64
	mov	@dat[4].16b,$ivec.16b
	mov	@dat[5].16b,$ivec.16b
	mov	@dat[6].16b,$ivec.16b
	mov	@dat[7].16b,$ivec.16b
	add	$ctr,$ctr,#1
	mov	$dat[4].s[3],$ctr
	add	$ctr,$ctr,#1
	mov	@dat[5].s[3],$ctr
	add	$ctr,$ctr,#1
	mov	@dat[6].s[3],$ctr
	add	$ctr,$ctr,#1
	mov	@dat[7].s[3],$ctr
___
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&enc_4blks(@dat[4],@dat[5],@dat[6],@dat[7]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
	&rev32(@dat[4],@dat[4]);
	&rev32(@dat[5],@dat[5]);
	&rev32(@dat[6],@dat[6]);
	&rev32(@dat[7],@dat[7]);
$code.=<<___;
	eor	@dat[0].16b,@dat[0].16b,@in[0].16b
	eor	@dat[1].16b,@dat[1].16b,@in[1].16b
	eor	@dat[2].16b,@dat[2].16b,@in[2].16b
	eor	@dat[3].16b,@dat[3].16b,@in[3].16b
	eor	@dat[4].16b,@dat[4].16b,@in[4].16b
	eor	@dat[5].16b,@dat[5].16b,@in[5].16b
	eor	@dat[6].16b,@dat[6].16b,@in[6].16b
	eor	@dat[7].16b,@dat[7].16b,@in[7].16b
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	st1	{@dat[4].4s,@dat[5].4s,@dat[6].4s,@dat[7].4s},[$out],#64
	subs	$len,$len,#8
	b.eq	3f
	add	$ctr,$ctr,#1
	mov	$ivec.s[3],$ctr
	b	1b
2:
___
	&enc_4blks(@dat[0],@dat[1],@dat[2],@dat[3]);
	&rev32(@dat[0],@dat[0]);
	&rev32(@dat[1],@dat[1]);
	&rev32(@dat[2],@dat[2]);
	&rev32(@dat[3],@dat[3]);
$code.=<<___;
	eor	@dat[0].16b,@dat[0].16b,@in[0].16b
	eor	@dat[1].16b,@dat[1].16b,@in[1].16b
	eor	@dat[2].16b,@dat[2].16b,@in[2].16b
	eor	@dat[3].16b,@dat[3].16b,@in[3].16b
	st1	{@dat[0].4s,@dat[1].4s,@dat[2].4s,@dat[3].4s},[$out],#64
	subs	$len,$len,#4
	b.eq	3f
	add	$ctr,$ctr,#1
	mov	$ivec.s[3],$ctr
	b	1b
1:
	subs	$len,$len,#1
	b.lt	3f
	mov	$dat[0].16b,$ivec.16b
	ld1	{@in[0].4s},[$inp],#16
___
	&enc_blk(@dat[0]);
	&rev32(@dat[0],@dat[0]);
$code.=<<___;
	eor	$dat[0].16b,$dat[0].16b,@in[0].16b
	st1	{$dat[0].4s},[$out],#16
	b.eq	3f
	add	$ctr,$ctr,#1
	mov	$ivec.s[3],$ctr
	b	1b
3:
	ldp	d8,d9,[sp],#16
	ret
.size	${prefix}_ctr32_encrypt_blocks,.-${prefix}_ctr32_encrypt_blocks
___
}}}
########################################
{   my  %opcode = (
        "sm4e"          => 0xcec08400,
        "sm4ekey"       => 0xce60c800);

    sub unsm4 {
        my ($mnemonic,$arg)=@_;

        $arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)[^,]*(?:,\s*[qv]([0-9]+))?/o
        &&
        sprintf ".inst\t0x%08x\t//%s %s",
                        $opcode{$mnemonic}|$1|($2<<5)|($3<<16),
                        $mnemonic,$arg;
    }
}

open SELF,$0;
while(<SELF>) {
        next if (/^#!/);
        last if (!s/^#/\/\// and !/^$/);
        print;
}
close SELF;

foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/ge;

	s/\b(sm4\w+)\s+([qv].*)/unsm4($1,$2)/ge;
	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
