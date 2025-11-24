#! /usr/bin/env perl
# Copyright 2014-2020 The OpenSSL Project Authors. All Rights Reserved.
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
# GHASH for ARMv8 Crypto Extension, 64-bit polynomial multiplication.
#
# June 2014
#
# Initial version was developed in tight cooperation with Ard
# Biesheuvel of Linaro from bits-n-pieces from other assembly modules.
# Just like aesv8-armx.pl this module supports both AArch32 and
# AArch64 execution modes.
#
# July 2014
#
# Implement 2x aggregated reduction [see ghash-x86.pl for background
# information].
#
# November 2017
#
# AArch64 register bank to "accommodate" 4x aggregated reduction and
# improve performance by 20-70% depending on processor.
#
# Current performance in cycles per processed byte:
#
#		64-bit PMULL	32-bit PMULL	32-bit NEON(*)
# Apple A7	0.58		0.92		5.62
# Cortex-A53	0.85		1.01		8.39
# Cortex-A57	0.73		1.17		7.61
# Denver	0.51		0.65		6.02
# Mongoose	0.65		1.10		8.06
# Kryo		0.76		1.16		8.00
#
# (*)	presented for reference/comparison purposes;

$flavour = shift;
$output  = shift;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

$Xi="x0";	# argument block
$Htbl="x1";
$inp="x2";
$len="x3";

$inc="x12";

{
my ($Xl,$Xm,$Xh,$IN)=map("q$_",(0..3));
my ($t0,$t1,$t2,$xC2,$H,$Hhl,$H2)=map("q$_",(8..14));

$code=<<___;
#if __ARM_MAX_ARCH__>=7
.text
___
$code.=".arch	armv8-a+crypto\n"	if ($flavour =~ /64/);
$code.=<<___				if ($flavour !~ /64/);
.fpu	neon
.code	32
#undef	__thumb2__
___

################################################################################
# void gcm_init_clmul(u128 Htable[16],const u64 H[2]);
#
# input:	128-bit H - secret parameter E(K,0^128)
# output:	precomputed table filled with degrees of twisted H;
#		H is twisted to handle reverse bitness of GHASH;
#		only few of 16 slots of Htable[16] are used;
#		data is opaque to outside world (which allows to
#		optimize the code independently);
#
$code.=<<___;
.global	gcm_init_clmul
.type	gcm_init_clmul,%function
.align	4
gcm_init_clmul:
	AARCH64_VALID_CALL_TARGET
	vld1.64		{$t1},[x1]		@ load input H
	vmov.i8		$xC2,#0xe1
	vshl.i64	$xC2,$xC2,#57		@ 0xc2.0
	vext.8		$IN,$t1,$t1,#8
	vshr.u64	$t2,$xC2,#63
	vdup.32		$t1,${t1}[1]
	vext.8		$t0,$t2,$xC2,#8		@ t0=0xc2....01
	vshr.u64	$t2,$IN,#63
	vshr.s32	$t1,$t1,#31		@ broadcast carry bit
	vand		$t2,$t2,$t0
	vshl.i64	$IN,$IN,#1
	vext.8		$t2,$t2,$t2,#8
	vand		$t0,$t0,$t1
	vorr		$IN,$IN,$t2		@ H<<<=1
	veor		$H,$IN,$t0		@ twisted H
	vst1.64		{$H},[x0],#16		@ store Htable[0]

	@ calculate H^2
	vext.8		$t0,$H,$H,#8		@ Karatsuba pre-processing
	vpmull.p64	$Xl,$H,$H
	veor		$t0,$t0,$H
	vpmull2.p64	$Xh,$H,$H
	vpmull.p64	$Xm,$t0,$t0

	vext.8		$t1,$Xl,$Xh,#8		@ Karatsuba post-processing
	veor		$t2,$Xl,$Xh
	veor		$Xm,$Xm,$t1
	veor		$Xm,$Xm,$t2
	vpmull.p64	$t2,$Xl,$xC2		@ 1st phase

	vmov		$Xh#lo,$Xm#hi		@ Xh|Xm - 256-bit result
	vmov		$Xm#hi,$Xl#lo		@ Xm is rotated Xl
	veor		$Xl,$Xm,$t2

	vext.8		$t2,$Xl,$Xl,#8		@ 2nd phase
	vpmull.p64	$Xl,$Xl,$xC2
	veor		$t2,$t2,$Xh
	veor		$H2,$Xl,$t2

	vext.8		$t1,$H2,$H2,#8		@ Karatsuba pre-processing
	veor		$t1,$t1,$H2
	vext.8		$Hhl,$t0,$t1,#8		@ pack Karatsuba pre-processed
	vst1.64		{$Hhl-$H2},[x0],#32	@ store Htable[1..2]
___
if ($flavour =~ /64/) {
my ($t3,$Yl,$Ym,$Yh) = map("q$_",(4..7));

$code.=<<___;
	@ calculate H^3 and H^4
	vpmull.p64	$Xl,$H, $H2
	 vpmull.p64	$Yl,$H2,$H2
	vpmull2.p64	$Xh,$H, $H2
	 vpmull2.p64	$Yh,$H2,$H2
	vpmull.p64	$Xm,$t0,$t1
	 vpmull.p64	$Ym,$t1,$t1

	vext.8		$t0,$Xl,$Xh,#8		@ Karatsuba post-processing
	 vext.8		$t1,$Yl,$Yh,#8
	veor		$t2,$Xl,$Xh
	veor		$Xm,$Xm,$t0
	 veor		$t3,$Yl,$Yh
	 veor		$Ym,$Ym,$t1
	veor		$Xm,$Xm,$t2
	vpmull.p64	$t2,$Xl,$xC2		@ 1st phase
	 veor		$Ym,$Ym,$t3
	 vpmull.p64	$t3,$Yl,$xC2

	vmov		$Xh#lo,$Xm#hi		@ Xh|Xm - 256-bit result
	 vmov		$Yh#lo,$Ym#hi
	vmov		$Xm#hi,$Xl#lo		@ Xm is rotated Xl
	 vmov		$Ym#hi,$Yl#lo
	veor		$Xl,$Xm,$t2
	 veor		$Yl,$Ym,$t3

	vext.8		$t2,$Xl,$Xl,#8		@ 2nd phase
	 vext.8		$t3,$Yl,$Yl,#8
	vpmull.p64	$Xl,$Xl,$xC2
	 vpmull.p64	$Yl,$Yl,$xC2
	veor		$t2,$t2,$Xh
	 veor		$t3,$t3,$Yh
	veor		$H, $Xl,$t2		@ H^3
	 veor		$H2,$Yl,$t3		@ H^4

	vext.8		$t0,$H, $H,#8		@ Karatsuba pre-processing
	 vext.8		$t1,$H2,$H2,#8
	veor		$t0,$t0,$H
	 veor		$t1,$t1,$H2
	vext.8		$Hhl,$t0,$t1,#8		@ pack Karatsuba pre-processed
	vst1.64		{$H-$H2},[x0]		@ store Htable[3..5]
___
}
$code.=<<___;
	ret
.size	gcm_init_clmul,.-gcm_init_clmul
___
################################################################################
# void gcm_gmult_clmul(u64 Xi[2],const u128 Htable[16]);
#
# input:	Xi - current hash value;
#		Htable - table precomputed in gcm_init_clmul;
# output:	Xi - next hash value Xi;
#
$code.=<<___;
.global	gcm_gmult_clmul
.type	gcm_gmult_clmul,%function
.align	4
gcm_gmult_clmul:
	AARCH64_VALID_CALL_TARGET
	vld1.64		{$t1},[$Xi]		@ load Xi
	vmov.i8		$xC2,#0xe1
	vld1.64		{$H-$Hhl},[$Htbl]	@ load twisted H, ...
	vshl.u64	$xC2,$xC2,#57
#ifndef __ARMEB__
	vrev64.8	$t1,$t1
#endif
	vext.8		$IN,$t1,$t1,#8

	vpmull.p64	$Xl,$H,$IN		@ H.lo·Xi.lo
	veor		$t1,$t1,$IN		@ Karatsuba pre-processing
	vpmull2.p64	$Xh,$H,$IN		@ H.hi·Xi.hi
	vpmull.p64	$Xm,$Hhl,$t1		@ (H.lo+H.hi)·(Xi.lo+Xi.hi)

	vext.8		$t1,$Xl,$Xh,#8		@ Karatsuba post-processing
	veor		$t2,$Xl,$Xh
	veor		$Xm,$Xm,$t1
	veor		$Xm,$Xm,$t2
	vpmull.p64	$t2,$Xl,$xC2		@ 1st phase of reduction

	vmov		$Xh#lo,$Xm#hi		@ Xh|Xm - 256-bit result
	vmov		$Xm#hi,$Xl#lo		@ Xm is rotated Xl
	veor		$Xl,$Xm,$t2

	vext.8		$t2,$Xl,$Xl,#8		@ 2nd phase of reduction
	vpmull.p64	$Xl,$Xl,$xC2
	veor		$t2,$t2,$Xh
	veor		$Xl,$Xl,$t2

#ifndef __ARMEB__
	vrev64.8	$Xl,$Xl
#endif
	vext.8		$Xl,$Xl,$Xl,#8
	vst1.64		{$Xl},[$Xi]		@ write out Xi

	ret
.size	gcm_gmult_clmul,.-gcm_gmult_clmul
___
}

$code.=<<___;
.asciz  "GHASH for ARMv8, CRYPTOGAMS by <appro\@openssl.org>"
.align  2
#endif
___

if ($flavour =~ /64/) {			######## 64-bit code
    sub unvmov {
	my $arg=shift;

	$arg =~ m/q([0-9]+)#(lo|hi),\s*q([0-9]+)#(lo|hi)/o &&
	sprintf	"ins	v%d.d[%d],v%d.d[%d]",$1<8?$1:$1+8,($2 eq "lo")?0:1,
					     $3<8?$3:$3+8,($4 eq "lo")?0:1;
    }
    foreach(split("\n",$code)) {
	s/cclr\s+([wx])([^,]+),\s*([a-z]+)/csel	$1$2,$1zr,$1$2,$3/o	or
	s/vmov\.i8/movi/o		or	# fix up legacy mnemonics
	s/vmov\s+(.*)/unvmov($1)/geo	or
	s/vext\.8/ext/o			or
	s/vshr\.s/sshr\.s/o		or
	s/vshr/ushr/o			or
	s/^(\s+)v/$1/o			or	# strip off v prefix
	s/\bbx\s+lr\b/ret/o;

	s/\bq([0-9]+)\b/"v".($1<8?$1:$1+8).".16b"/geo;	# old->new registers
	s/@\s/\/\//o;				# old->new style commentary

	# fix up remaining legacy suffixes
	s/\.[ui]?8(\s)/$1/o;
	s/\.[uis]?32//o and s/\.16b/\.4s/go;
	m/\.p64/o and s/\.16b/\.1q/o;		# 1st pmull argument
	m/l\.p64/o and s/\.16b/\.1d/go;		# 2nd and 3rd pmull arguments
	s/\.[uisp]?64//o and s/\.16b/\.2d/go;
	s/\.[42]([sd])\[([0-3])\]/\.$1\[$2\]/o;

	# Switch preprocessor checks to aarch64 versions.
	s/__ARME([BL])__/__AARCH64E$1__/go;

	print $_,"\n";
    }
} else {				######## 32-bit code
    sub unvdup32 {
	my $arg=shift;

	$arg =~ m/q([0-9]+),\s*q([0-9]+)\[([0-3])\]/o &&
	sprintf	"vdup.32	q%d,d%d[%d]",$1,2*$2+($3>>1),$3&1;
    }
    sub unvpmullp64 {
	my ($mnemonic,$arg)=@_;

	if ($arg =~ m/q([0-9]+),\s*q([0-9]+),\s*q([0-9]+)/o) {
	    my $word = 0xf2a00e00|(($1&7)<<13)|(($1&8)<<19)
				 |(($2&7)<<17)|(($2&8)<<4)
				 |(($3&7)<<1) |(($3&8)<<2);
	    $word |= 0x00010001	 if ($mnemonic =~ "2");
	    # since ARMv7 instructions are always encoded little-endian.
	    # correct solution is to use .inst directive, but older
	    # assemblers don't implement it:-(
	    sprintf ".byte\t0x%02x,0x%02x,0x%02x,0x%02x\t@ %s %s",
			$word&0xff,($word>>8)&0xff,
			($word>>16)&0xff,($word>>24)&0xff,
			$mnemonic,$arg;
	}
    }

    foreach(split("\n",$code)) {
	s/\b[wx]([0-9]+)\b/r$1/go;		# new->old registers
	s/\bv([0-9])\.[12468]+[bsd]\b/q$1/go;	# new->old registers
	s/\/\/\s?/@ /o;				# new->old style commentary

	# fix up remaining new-style suffixes
	s/\],#[0-9]+/]!/o;

	s/cclr\s+([^,]+),\s*([a-z]+)/mov$2	$1,#0/o			or
	s/vdup\.32\s+(.*)/unvdup32($1)/geo				or
	s/v?(pmull2?)\.p64\s+(.*)/unvpmullp64($1,$2)/geo		or
	s/\bq([0-9]+)#(lo|hi)/sprintf "d%d",2*$1+($2 eq "hi")/geo	or
	s/^(\s+)b\./$1b/o						or
	s/^(\s+)ret/$1bx\tlr/o;

	print $_,"\n";
    }
}

close STDOUT or die "error closing STDOUT: $!"; # enforce flush
