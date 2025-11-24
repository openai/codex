#! /usr/bin/env perl
# Copyright 2014-2016 The OpenSSL Project Authors. All Rights Reserved.
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
# Performance in cycles per byte processed with 128-bit key:
#
#		CBC enc		CBC dec		CTR
# Apple A7	2.39		1.20		1.20
# Cortex-A53	1.32		1.29		1.46
# Cortex-A57(*)	1.95		0.85		0.93
# Denver	1.96		0.86		0.80
# Mongoose	1.33		1.20		1.20
#
# (*)	original 3.64/1.34/1.32 results were for r0p0 revision
#	and are still same even for updated module;

$flavour = shift;
$output  = shift;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

$prefix="aes_hw";

$code=<<___;
#if __ARM_MAX_ARCH__>=7
.text
___
$code.=".arch	armv8-a+crypto\n"			if ($flavour =~ /64/);
$code.=<<___						if ($flavour !~ /64/);
.arch	armv7-a	// don't confuse not-so-latest binutils with argv8 :-)
.fpu	neon
.code	32
#undef	__thumb2__
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


# On AArch64, put the data .rodata and use adrp + add for compatibility with
# execute-only memory. On AArch32, put it in .text and use adr.
$code.= ".section .rodata\n" if ($flavour =~ /64/);
$code.=<<___;
.align	5
.Lrcon:
.long	0x01,0x01,0x01,0x01
.long	0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d	// rotate-n-splat
.long	0x1b,0x1b,0x1b,0x1b

.text

.globl	${prefix}_set_encrypt_key
.type	${prefix}_set_encrypt_key,%function
.align	5
${prefix}_set_encrypt_key:
.Lenc_key:
___
$code.=<<___	if ($flavour =~ /64/);
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	AARCH64_VALID_CALL_TARGET
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
___
$code.=<<___;
	mov	$ptr,#-2
	cmp	$bits,#128
	b.lt	.Lenc_key_abort
	cmp	$bits,#256
	b.gt	.Lenc_key_abort
	tst	$bits,#0x3f
	b.ne	.Lenc_key_abort

___
$code.=<<___	if ($flavour =~ /64/);
	adrp	$ptr,:pg_hi21:.Lrcon
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
	// 192-bit key support was removed.
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

// 192-bit key support was removed.

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
___
}}}
{{{
my ($inp,$out,$len,$key,$ivp)=map("x$_",(0..4));
my ($rounds,$cnt,$key_)=("w5","w6","x7");
my ($ctr,$tctr0,$tctr1,$tctr2)=map("w$_",(8..10,12));
my $step="x12";		# aliases with $tctr2

my ($dat0,$dat1,$in0,$in1,$tmp0,$tmp1,$ivec,$rndlast)=map("q$_",(0..7));
my ($dat2,$in2,$tmp2)=map("q$_",(10,11,9));

my ($dat,$tmp)=($dat0,$tmp0);

### q8-q15	preloaded key schedule

$code.=<<___;
.globl	${prefix}_ctr32_encrypt_blocks
.type	${prefix}_ctr32_encrypt_blocks,%function
.align	5
${prefix}_ctr32_encrypt_blocks:
___
$code.=<<___	if ($flavour =~ /64/);
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	AARCH64_VALID_CALL_TARGET
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
	vld1.32		{$dat0},[$ivp]

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

	// ARM Cortex-A57 and Cortex-A72 cores running in 32-bit mode are
	// affected by silicon errata #1742098 [0] and #1655431 [1],
	// respectively, where the second instruction of an aese/aesmc
	// instruction pair may execute twice if an interrupt is taken right
	// after the first instruction consumes an input register of which a
	// single 32-bit lane has been updated the last time it was modified.
	//
	// This function uses a counter in one 32-bit lane. The vmov.32 lines
	// could write to $dat1 and $dat2 directly, but that trips this bugs.
	// We write to $ivec and copy to the final register as a workaround.
	//
	// [0] ARM-EPM-049219 v23 Cortex-A57 MPCore Software Developers Errata Notice
	// [1] ARM-EPM-012079 v11.0 Cortex-A72 MPCore Software Developers Errata Notice
#ifndef __ARMEB__
	rev		$ctr, $ctr
#endif
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
	 add		$tctr0,$ctr,#1
	aese		$dat2,q8
	aesmc		$dat2,$dat2
	 vld1.8		{$in1},[$inp],#16
	 rev		$tctr0,$tctr0
	aese		$tmp0,q9
	aesmc		$tmp0,$tmp0
	aese		$tmp1,q9
	aesmc		$tmp1,$tmp1
	 vld1.8		{$in2},[$inp],#16
	 mov		$key_,$key
	aese		$dat2,q9
	aesmc		$tmp2,$dat2
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
	 // Note the logic to update $dat0, $dat1, and $dat1 is written to work
	 // around a bug in ARM Cortex-A57 and Cortex-A72 cores running in
	 // 32-bit mode. See the comment above.
	 veor		$in2,$in2,$rndlast
	 vmov.32	${ivec}[3], $tctr0
	aese		$tmp2,q13
	aesmc		$tmp2,$tmp2
	 vorr		$dat0,$ivec,$ivec
	 rev		$tctr1,$tctr1
	aese		$tmp0,q14
	aesmc		$tmp0,$tmp0
	 vmov.32	${ivec}[3], $tctr1
	 rev		$tctr2,$ctr
	aese		$tmp1,q14
	aesmc		$tmp1,$tmp1
	 vorr		$dat1,$ivec,$ivec
	 vmov.32	${ivec}[3], $tctr2
	aese		$tmp2,q14
	aesmc		$tmp2,$tmp2
	 vorr		$dat2,$ivec,$ivec
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
$code.=<<___;
#endif
___
########################################
if ($flavour =~ /64/) {			######## 64-bit code
    my %opcode = (
	"aesd"	=>	0x4e285800,	"aese"	=>	0x4e284800,
	"aesimc"=>	0x4e287800,	"aesmc"	=>	0x4e286800	);

    local *unaes = sub {
	my ($mnemonic,$arg)=@_;

	$arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)/o	&&
	sprintf ".inst\t0x%08x\t//%s %s",
			$opcode{$mnemonic}|$1|($2<<5),
			$mnemonic,$arg;
    };

    foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/geo;

	s/\bq([0-9]+)\b/"v".($1<8?$1:$1+8).".16b"/geo;	# old->new registers
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
	    sprintf ".byte\t0x%02x,0x%02x,0x%02x,0x%02x\t@ %s %s",
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
	s/cclr\s+([^,]+),\s*([a-z]+)/mov$2	$1,#0/o	or
	s/vtbl\.8\s+(.*)/unvtbl($1)/geo			or
	s/vdup\.32\s+(.*)/unvdup32($1)/geo		or
	s/vmov\.32\s+(.*)/unvmov32($1)/geo		or
	s/^(\s+)b\./$1b/o				or
	s/^(\s+)mov\./$1mov/o				or
	s/^(\s+)ret/$1bx\tlr/o;

	print $_,"\n";
    }
}

close STDOUT or die "error closing STDOUT: $!";
