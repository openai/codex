#! /usr/bin/env perl
# Copyright 2012-2016 The OpenSSL Project Authors. All Rights Reserved.
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


# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project.
#
# Specific modes and adaptation for Linux kernel by Ard Biesheuvel
# of Linaro.
# ====================================================================

# Bit-sliced AES for ARM NEON
#
# February 2012.
#
# This implementation is direct adaptation of bsaes-x86_64 module for
# ARM NEON. Except that this module is endian-neutral [in sense that
# it can be compiled for either endianness] by courtesy of vld1.8's
# neutrality. Initial version doesn't implement interface to OpenSSL,
# only low-level primitives and unsupported entry points, just enough
# to collect performance results, which for Cortex-A8 core are:
#
# encrypt	19.5 cycles per byte processed with 128-bit key
# decrypt	22.1 cycles per byte processed with 128-bit key
# key conv.	440  cycles per 128-bit key/0.18 of 8x block
#
# Snapdragon S4 encrypts byte in 17.6 cycles and decrypts in 19.7,
# which is [much] worse than anticipated (for further details see
# http://www.openssl.org/~appro/Snapdragon-S4.html).
#
# Cortex-A15 manages in 14.2/16.1 cycles [when integer-only code
# manages in 20.0 cycles].
#
# When comparing to x86_64 results keep in mind that NEON unit is
# [mostly] single-issue and thus can't [fully] benefit from
# instruction-level parallelism. And when comparing to aes-armv4
# results keep in mind key schedule conversion overhead (see
# bsaes-x86_64.pl for further details)...
#
#						<appro@openssl.org>

# April-August 2013
# Add CBC, CTR and XTS subroutines and adapt for kernel use; courtesy of Ard.

$flavour = shift;
if ($flavour=~/\w[\w\-]*\.\w+$/) { $output=$flavour; undef $flavour; }
else { while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {} }

if ($flavour && $flavour ne "void") {
    $0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
    ( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
    ( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
    die "can't locate arm-xlate.pl";

    open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
    *STDOUT=*OUT;
} else {
    open OUT,">$output";
    *STDOUT=*OUT;
}

my ($inp,$out,$len,$key)=("r0","r1","r2","r3");
my @XMM=map("q$_",(0..15));

{
my ($key,$rounds,$const)=("r4","r5","r6");

sub Dlo()   { shift=~m|q([1]?[0-9])|?"d".($1*2):"";     }
sub Dhi()   { shift=~m|q([1]?[0-9])|?"d".($1*2+1):"";   }

sub Sbox {
# input in  lsb > [b0, b1, b2, b3, b4, b5, b6, b7] < msb
# output in lsb > [b0, b1, b4, b6, b3, b7, b2, b5] < msb
my @b=@_[0..7];
my @t=@_[8..11];
my @s=@_[12..15];
	&InBasisChange	(@b);
	&Inv_GF256	(@b[6,5,0,3,7,1,4,2],@t,@s);
	&OutBasisChange	(@b[7,1,4,2,6,5,0,3]);
}

sub InBasisChange {
# input in  lsb > [b0, b1, b2, b3, b4, b5, b6, b7] < msb
# output in lsb > [b6, b5, b0, b3, b7, b1, b4, b2] < msb
my @b=@_[0..7];
$code.=<<___;
	veor	@b[2], @b[2], @b[1]
	veor	@b[5], @b[5], @b[6]
	veor	@b[3], @b[3], @b[0]
	veor	@b[6], @b[6], @b[2]
	veor	@b[5], @b[5], @b[0]

	veor	@b[6], @b[6], @b[3]
	veor	@b[3], @b[3], @b[7]
	veor	@b[7], @b[7], @b[5]
	veor	@b[3], @b[3], @b[4]
	veor	@b[4], @b[4], @b[5]

	veor	@b[2], @b[2], @b[7]
	veor	@b[3], @b[3], @b[1]
	veor	@b[1], @b[1], @b[5]
___
}

sub OutBasisChange {
# input in  lsb > [b0, b1, b2, b3, b4, b5, b6, b7] < msb
# output in lsb > [b6, b1, b2, b4, b7, b0, b3, b5] < msb
my @b=@_[0..7];
$code.=<<___;
	veor	@b[0], @b[0], @b[6]
	veor	@b[1], @b[1], @b[4]
	veor	@b[4], @b[4], @b[6]
	veor	@b[2], @b[2], @b[0]
	veor	@b[6], @b[6], @b[1]

	veor	@b[1], @b[1], @b[5]
	veor	@b[5], @b[5], @b[3]
	veor	@b[3], @b[3], @b[7]
	veor	@b[7], @b[7], @b[5]
	veor	@b[2], @b[2], @b[5]

	veor	@b[4], @b[4], @b[7]
___
}

sub InvSbox {
# input in lsb 	> [b0, b1, b2, b3, b4, b5, b6, b7] < msb
# output in lsb	> [b0, b1, b6, b4, b2, b7, b3, b5] < msb
my @b=@_[0..7];
my @t=@_[8..11];
my @s=@_[12..15];
	&InvInBasisChange	(@b);
	&Inv_GF256		(@b[5,1,2,6,3,7,0,4],@t,@s);
	&InvOutBasisChange	(@b[3,7,0,4,5,1,2,6]);
}

sub InvInBasisChange {		# OutBasisChange in reverse (with twist)
my @b=@_[5,1,2,6,3,7,0,4];
$code.=<<___
	 veor	@b[1], @b[1], @b[7]
	veor	@b[4], @b[4], @b[7]

	veor	@b[7], @b[7], @b[5]
	 veor	@b[1], @b[1], @b[3]
	veor	@b[2], @b[2], @b[5]
	veor	@b[3], @b[3], @b[7]

	veor	@b[6], @b[6], @b[1]
	veor	@b[2], @b[2], @b[0]
	 veor	@b[5], @b[5], @b[3]
	veor	@b[4], @b[4], @b[6]
	veor	@b[0], @b[0], @b[6]
	veor	@b[1], @b[1], @b[4]
___
}

sub InvOutBasisChange {		# InBasisChange in reverse
my @b=@_[2,5,7,3,6,1,0,4];
$code.=<<___;
	veor	@b[1], @b[1], @b[5]
	veor	@b[2], @b[2], @b[7]

	veor	@b[3], @b[3], @b[1]
	veor	@b[4], @b[4], @b[5]
	veor	@b[7], @b[7], @b[5]
	veor	@b[3], @b[3], @b[4]
	 veor 	@b[5], @b[5], @b[0]
	veor	@b[3], @b[3], @b[7]
	 veor	@b[6], @b[6], @b[2]
	 veor	@b[2], @b[2], @b[1]
	veor	@b[6], @b[6], @b[3]

	veor	@b[3], @b[3], @b[0]
	veor	@b[5], @b[5], @b[6]
___
}

sub Mul_GF4 {
#;*************************************************************
#;* Mul_GF4: Input x0-x1,y0-y1 Output x0-x1 Temp t0 (8) *
#;*************************************************************
my ($x0,$x1,$y0,$y1,$t0,$t1)=@_;
$code.=<<___;
	veor 	$t0, $y0, $y1
	vand	$t0, $t0, $x0
	veor	$x0, $x0, $x1
	vand	$t1, $x1, $y0
	vand	$x0, $x0, $y1
	veor	$x1, $t1, $t0
	veor	$x0, $x0, $t1
___
}

sub Mul_GF4_N {				# not used, see next subroutine
# multiply and scale by N
my ($x0,$x1,$y0,$y1,$t0)=@_;
$code.=<<___;
	veor	$t0, $y0, $y1
	vand	$t0, $t0, $x0
	veor	$x0, $x0, $x1
	vand	$x1, $x1, $y0
	vand	$x0, $x0, $y1
	veor	$x1, $x1, $x0
	veor	$x0, $x0, $t0
___
}

sub Mul_GF4_N_GF4 {
# interleaved Mul_GF4_N and Mul_GF4
my ($x0,$x1,$y0,$y1,$t0,
    $x2,$x3,$y2,$y3,$t1)=@_;
$code.=<<___;
	veor	$t0, $y0, $y1
	 veor 	$t1, $y2, $y3
	vand	$t0, $t0, $x0
	 vand	$t1, $t1, $x2
	veor	$x0, $x0, $x1
	 veor	$x2, $x2, $x3
	vand	$x1, $x1, $y0
	 vand	$x3, $x3, $y2
	vand	$x0, $x0, $y1
	 vand	$x2, $x2, $y3
	veor	$x1, $x1, $x0
	 veor	$x2, $x2, $x3
	veor	$x0, $x0, $t0
	 veor	$x3, $x3, $t1
___
}
sub Mul_GF16_2 {
my @x=@_[0..7];
my @y=@_[8..11];
my @t=@_[12..15];
$code.=<<___;
	veor	@t[0], @x[0], @x[2]
	veor	@t[1], @x[1], @x[3]
___
	&Mul_GF4  	(@x[0], @x[1], @y[0], @y[1], @t[2..3]);
$code.=<<___;
	veor	@y[0], @y[0], @y[2]
	veor	@y[1], @y[1], @y[3]
___
	Mul_GF4_N_GF4	(@t[0], @t[1], @y[0], @y[1], @t[3],
			 @x[2], @x[3], @y[2], @y[3], @t[2]);
$code.=<<___;
	veor	@x[0], @x[0], @t[0]
	veor	@x[2], @x[2], @t[0]
	veor	@x[1], @x[1], @t[1]
	veor	@x[3], @x[3], @t[1]

	veor	@t[0], @x[4], @x[6]
	veor	@t[1], @x[5], @x[7]
___
	&Mul_GF4_N_GF4	(@t[0], @t[1], @y[0], @y[1], @t[3],
			 @x[6], @x[7], @y[2], @y[3], @t[2]);
$code.=<<___;
	veor	@y[0], @y[0], @y[2]
	veor	@y[1], @y[1], @y[3]
___
	&Mul_GF4  	(@x[4], @x[5], @y[0], @y[1], @t[2..3]);
$code.=<<___;
	veor	@x[4], @x[4], @t[0]
	veor	@x[6], @x[6], @t[0]
	veor	@x[5], @x[5], @t[1]
	veor	@x[7], @x[7], @t[1]
___
}
sub Inv_GF256 {
#;********************************************************************
#;* Inv_GF256: Input x0-x7 Output x0-x7 Temp t0-t3,s0-s3 (144)       *
#;********************************************************************
my @x=@_[0..7];
my @t=@_[8..11];
my @s=@_[12..15];
# direct optimizations from hardware
$code.=<<___;
	veor	@t[3], @x[4], @x[6]
	veor	@t[2], @x[5], @x[7]
	veor	@t[1], @x[1], @x[3]
	veor	@s[1], @x[7], @x[6]
	 vmov	@t[0], @t[2]
	veor	@s[0], @x[0], @x[2]

	vorr	@t[2], @t[2], @t[1]
	veor	@s[3], @t[3], @t[0]
	vand	@s[2], @t[3], @s[0]
	vorr	@t[3], @t[3], @s[0]
	veor	@s[0], @s[0], @t[1]
	vand	@t[0], @t[0], @t[1]
	veor	@t[1], @x[3], @x[2]
	vand	@s[3], @s[3], @s[0]
	vand	@s[1], @s[1], @t[1]
	veor	@t[1], @x[4], @x[5]
	veor	@s[0], @x[1], @x[0]
	veor	@t[3], @t[3], @s[1]
	veor	@t[2], @t[2], @s[1]
	vand	@s[1], @t[1], @s[0]
	vorr	@t[1], @t[1], @s[0]
	veor	@t[3], @t[3], @s[3]
	veor	@t[0], @t[0], @s[1]
	veor	@t[2], @t[2], @s[2]
	veor	@t[1], @t[1], @s[3]
	veor	@t[0], @t[0], @s[2]
	vand	@s[0], @x[7], @x[3]
	veor	@t[1], @t[1], @s[2]
	vand	@s[1], @x[6], @x[2]
	vand	@s[2], @x[5], @x[1]
	vorr	@s[3], @x[4], @x[0]
	veor	@t[3], @t[3], @s[0]
	veor	@t[1], @t[1], @s[2]
	veor	@t[0], @t[0], @s[3]
	veor	@t[2], @t[2], @s[1]

	@ Inv_GF16 \t0, \t1, \t2, \t3, \s0, \s1, \s2, \s3

	@ new smaller inversion

	vand	@s[2], @t[3], @t[1]
	vmov	@s[0], @t[0]

	veor	@s[1], @t[2], @s[2]
	veor	@s[3], @t[0], @s[2]
	veor	@s[2], @t[0], @s[2]	@ @s[2]=@s[3]

	vbsl	@s[1], @t[1], @t[0]
	vbsl	@s[3], @t[3], @t[2]
	veor	@t[3], @t[3], @t[2]

	vbsl	@s[0], @s[1], @s[2]
	vbsl	@t[0], @s[2], @s[1]

	vand	@s[2], @s[0], @s[3]
	veor	@t[1], @t[1], @t[0]

	veor	@s[2], @s[2], @t[3]
___
# output in s3, s2, s1, t1

# Mul_GF16_2 \x0, \x1, \x2, \x3, \x4, \x5, \x6, \x7, \t2, \t3, \t0, \t1, \s0, \s1, \s2, \s3

# Mul_GF16_2 \x0, \x1, \x2, \x3, \x4, \x5, \x6, \x7, \s3, \s2, \s1, \t1, \s0, \t0, \t2, \t3
	&Mul_GF16_2(@x,@s[3,2,1],@t[1],@s[0],@t[0,2,3]);

### output msb > [x3,x2,x1,x0,x7,x6,x5,x4] < lsb
}

# AES linear components

sub ShiftRows {
my @x=@_[0..7];
my @t=@_[8..11];
my $mask=pop;
$code.=<<___;
	vldmia	$key!, {@t[0]-@t[3]}
	veor	@t[0], @t[0], @x[0]
	veor	@t[1], @t[1], @x[1]
	vtbl.8	`&Dlo(@x[0])`, {@t[0]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[0])`, {@t[0]}, `&Dhi($mask)`
	vldmia	$key!, {@t[0]}
	veor	@t[2], @t[2], @x[2]
	vtbl.8	`&Dlo(@x[1])`, {@t[1]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[1])`, {@t[1]}, `&Dhi($mask)`
	vldmia	$key!, {@t[1]}
	veor	@t[3], @t[3], @x[3]
	vtbl.8	`&Dlo(@x[2])`, {@t[2]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[2])`, {@t[2]}, `&Dhi($mask)`
	vldmia	$key!, {@t[2]}
	vtbl.8	`&Dlo(@x[3])`, {@t[3]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[3])`, {@t[3]}, `&Dhi($mask)`
	vldmia	$key!, {@t[3]}
	veor	@t[0], @t[0], @x[4]
	veor	@t[1], @t[1], @x[5]
	vtbl.8	`&Dlo(@x[4])`, {@t[0]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[4])`, {@t[0]}, `&Dhi($mask)`
	veor	@t[2], @t[2], @x[6]
	vtbl.8	`&Dlo(@x[5])`, {@t[1]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[5])`, {@t[1]}, `&Dhi($mask)`
	veor	@t[3], @t[3], @x[7]
	vtbl.8	`&Dlo(@x[6])`, {@t[2]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[6])`, {@t[2]}, `&Dhi($mask)`
	vtbl.8	`&Dlo(@x[7])`, {@t[3]}, `&Dlo($mask)`
	vtbl.8	`&Dhi(@x[7])`, {@t[3]}, `&Dhi($mask)`
___
}

sub MixColumns {
# modified to emit output in order suitable for feeding back to aesenc[last]
my @x=@_[0..7];
my @t=@_[8..15];
my $inv=@_[16];	# optional
$code.=<<___;
	vext.8	@t[0], @x[0], @x[0], #12	@ x0 <<< 32
	vext.8	@t[1], @x[1], @x[1], #12
	 veor	@x[0], @x[0], @t[0]		@ x0 ^ (x0 <<< 32)
	vext.8	@t[2], @x[2], @x[2], #12
	 veor	@x[1], @x[1], @t[1]
	vext.8	@t[3], @x[3], @x[3], #12
	 veor	@x[2], @x[2], @t[2]
	vext.8	@t[4], @x[4], @x[4], #12
	 veor	@x[3], @x[3], @t[3]
	vext.8	@t[5], @x[5], @x[5], #12
	 veor	@x[4], @x[4], @t[4]
	vext.8	@t[6], @x[6], @x[6], #12
	 veor	@x[5], @x[5], @t[5]
	vext.8	@t[7], @x[7], @x[7], #12
	 veor	@x[6], @x[6], @t[6]

	veor	@t[1], @t[1], @x[0]
	 veor	@x[7], @x[7], @t[7]
	 vext.8	@x[0], @x[0], @x[0], #8		@ (x0 ^ (x0 <<< 32)) <<< 64)
	veor	@t[2], @t[2], @x[1]
	veor	@t[0], @t[0], @x[7]
	veor	@t[1], @t[1], @x[7]
	 vext.8	@x[1], @x[1], @x[1], #8
	veor	@t[5], @t[5], @x[4]
	 veor	@x[0], @x[0], @t[0]
	veor	@t[6], @t[6], @x[5]
	 veor	@x[1], @x[1], @t[1]
	 vext.8	@t[0], @x[4], @x[4], #8
	veor	@t[4], @t[4], @x[3]
	 vext.8	@t[1], @x[5], @x[5], #8
	veor	@t[7], @t[7], @x[6]
	 vext.8	@x[4], @x[3], @x[3], #8
	veor	@t[3], @t[3], @x[2]
	 vext.8	@x[5], @x[7], @x[7], #8
	veor	@t[4], @t[4], @x[7]
	 vext.8	@x[3], @x[6], @x[6], #8
	veor	@t[3], @t[3], @x[7]
	 vext.8	@x[6], @x[2], @x[2], #8
	veor	@x[7], @t[1], @t[5]
___
$code.=<<___ if (!$inv);
	veor	@x[2], @t[0], @t[4]
	veor	@x[4], @x[4], @t[3]
	veor	@x[5], @x[5], @t[7]
	veor	@x[3], @x[3], @t[6]
	 @ vmov	@x[2], @t[0]
	veor	@x[6], @x[6], @t[2]
	 @ vmov	@x[7], @t[1]
___
$code.=<<___ if ($inv);
	veor	@t[3], @t[3], @x[4]
	veor	@x[5], @x[5], @t[7]
	veor	@x[2], @x[3], @t[6]
	veor	@x[3], @t[0], @t[4]
	veor	@x[4], @x[6], @t[2]
	vmov	@x[6], @t[3]
	 @ vmov	@x[7], @t[1]
___
}

sub InvMixColumns_orig {
my @x=@_[0..7];
my @t=@_[8..15];

$code.=<<___;
	@ multiplication by 0x0e
	vext.8	@t[7], @x[7], @x[7], #12
	vmov	@t[2], @x[2]
	veor	@x[2], @x[2], @x[5]		@ 2 5
	veor	@x[7], @x[7], @x[5]		@ 7 5
	vext.8	@t[0], @x[0], @x[0], #12
	vmov	@t[5], @x[5]
	veor	@x[5], @x[5], @x[0]		@ 5 0		[1]
	veor	@x[0], @x[0], @x[1]		@ 0 1
	vext.8	@t[1], @x[1], @x[1], #12
	veor	@x[1], @x[1], @x[2]		@ 1 25
	veor	@x[0], @x[0], @x[6]		@ 01 6		[2]
	vext.8	@t[3], @x[3], @x[3], #12
	veor	@x[1], @x[1], @x[3]		@ 125 3		[4]
	veor	@x[2], @x[2], @x[0]		@ 25 016	[3]
	veor	@x[3], @x[3], @x[7]		@ 3 75
	veor	@x[7], @x[7], @x[6]		@ 75 6		[0]
	vext.8	@t[6], @x[6], @x[6], #12
	vmov	@t[4], @x[4]
	veor	@x[6], @x[6], @x[4]		@ 6 4
	veor	@x[4], @x[4], @x[3]		@ 4 375		[6]
	veor	@x[3], @x[3], @x[7]		@ 375 756=36
	veor	@x[6], @x[6], @t[5]		@ 64 5		[7]
	veor	@x[3], @x[3], @t[2]		@ 36 2
	vext.8	@t[5], @t[5], @t[5], #12
	veor	@x[3], @x[3], @t[4]		@ 362 4		[5]
___
					my @y = @x[7,5,0,2,1,3,4,6];
$code.=<<___;
	@ multiplication by 0x0b
	veor	@y[1], @y[1], @y[0]
	veor	@y[0], @y[0], @t[0]
	vext.8	@t[2], @t[2], @t[2], #12
	veor	@y[1], @y[1], @t[1]
	veor	@y[0], @y[0], @t[5]
	vext.8	@t[4], @t[4], @t[4], #12
	veor	@y[1], @y[1], @t[6]
	veor	@y[0], @y[0], @t[7]
	veor	@t[7], @t[7], @t[6]		@ clobber t[7]

	veor	@y[3], @y[3], @t[0]
	 veor	@y[1], @y[1], @y[0]
	vext.8	@t[0], @t[0], @t[0], #12
	veor	@y[2], @y[2], @t[1]
	veor	@y[4], @y[4], @t[1]
	vext.8	@t[1], @t[1], @t[1], #12
	veor	@y[2], @y[2], @t[2]
	veor	@y[3], @y[3], @t[2]
	veor	@y[5], @y[5], @t[2]
	veor	@y[2], @y[2], @t[7]
	vext.8	@t[2], @t[2], @t[2], #12
	veor	@y[3], @y[3], @t[3]
	veor	@y[6], @y[6], @t[3]
	veor	@y[4], @y[4], @t[3]
	veor	@y[7], @y[7], @t[4]
	vext.8	@t[3], @t[3], @t[3], #12
	veor	@y[5], @y[5], @t[4]
	veor	@y[7], @y[7], @t[7]
	veor	@t[7], @t[7], @t[5]		@ clobber t[7] even more
	veor	@y[3], @y[3], @t[5]
	veor	@y[4], @y[4], @t[4]

	veor	@y[5], @y[5], @t[7]
	vext.8	@t[4], @t[4], @t[4], #12
	veor	@y[6], @y[6], @t[7]
	veor	@y[4], @y[4], @t[7]

	veor	@t[7], @t[7], @t[5]
	vext.8	@t[5], @t[5], @t[5], #12

	@ multiplication by 0x0d
	veor	@y[4], @y[4], @y[7]
	 veor	@t[7], @t[7], @t[6]		@ restore t[7]
	veor	@y[7], @y[7], @t[4]
	vext.8	@t[6], @t[6], @t[6], #12
	veor	@y[2], @y[2], @t[0]
	veor	@y[7], @y[7], @t[5]
	vext.8	@t[7], @t[7], @t[7], #12
	veor	@y[2], @y[2], @t[2]

	veor	@y[3], @y[3], @y[1]
	veor	@y[1], @y[1], @t[1]
	veor	@y[0], @y[0], @t[0]
	veor	@y[3], @y[3], @t[0]
	veor	@y[1], @y[1], @t[5]
	veor	@y[0], @y[0], @t[5]
	vext.8	@t[0], @t[0], @t[0], #12
	veor	@y[1], @y[1], @t[7]
	veor	@y[0], @y[0], @t[6]
	veor	@y[3], @y[3], @y[1]
	veor	@y[4], @y[4], @t[1]
	vext.8	@t[1], @t[1], @t[1], #12

	veor	@y[7], @y[7], @t[7]
	veor	@y[4], @y[4], @t[2]
	veor	@y[5], @y[5], @t[2]
	veor	@y[2], @y[2], @t[6]
	veor	@t[6], @t[6], @t[3]		@ clobber t[6]
	vext.8	@t[2], @t[2], @t[2], #12
	veor	@y[4], @y[4], @y[7]
	veor	@y[3], @y[3], @t[6]

	veor	@y[6], @y[6], @t[6]
	veor	@y[5], @y[5], @t[5]
	vext.8	@t[5], @t[5], @t[5], #12
	veor	@y[6], @y[6], @t[4]
	vext.8	@t[4], @t[4], @t[4], #12
	veor	@y[5], @y[5], @t[6]
	veor	@y[6], @y[6], @t[7]
	vext.8	@t[7], @t[7], @t[7], #12
	veor	@t[6], @t[6], @t[3]		@ restore t[6]
	vext.8	@t[3], @t[3], @t[3], #12

	@ multiplication by 0x09
	veor	@y[4], @y[4], @y[1]
	veor	@t[1], @t[1], @y[1]		@ t[1]=y[1]
	veor	@t[0], @t[0], @t[5]		@ clobber t[0]
	vext.8	@t[6], @t[6], @t[6], #12
	veor	@t[1], @t[1], @t[5]
	veor	@y[3], @y[3], @t[0]
	veor	@t[0], @t[0], @y[0]		@ t[0]=y[0]
	veor	@t[1], @t[1], @t[6]
	veor	@t[6], @t[6], @t[7]		@ clobber t[6]
	veor	@y[4], @y[4], @t[1]
	veor	@y[7], @y[7], @t[4]
	veor	@y[6], @y[6], @t[3]
	veor	@y[5], @y[5], @t[2]
	veor	@t[4], @t[4], @y[4]		@ t[4]=y[4]
	veor	@t[3], @t[3], @y[3]		@ t[3]=y[3]
	veor	@t[5], @t[5], @y[5]		@ t[5]=y[5]
	veor	@t[2], @t[2], @y[2]		@ t[2]=y[2]
	veor	@t[3], @t[3], @t[7]
	veor	@XMM[5], @t[5], @t[6]
	veor	@XMM[6], @t[6], @y[6]		@ t[6]=y[6]
	veor	@XMM[2], @t[2], @t[6]
	veor	@XMM[7], @t[7], @y[7]		@ t[7]=y[7]

	vmov	@XMM[0], @t[0]
	vmov	@XMM[1], @t[1]
	@ vmov	@XMM[2], @t[2]
	vmov	@XMM[3], @t[3]
	vmov	@XMM[4], @t[4]
	@ vmov	@XMM[5], @t[5]
	@ vmov	@XMM[6], @t[6]
	@ vmov	@XMM[7], @t[7]
___
}

sub InvMixColumns {
my @x=@_[0..7];
my @t=@_[8..15];

# Thanks to Jussi Kivilinna for providing pointer to
#
# | 0e 0b 0d 09 |   | 02 03 01 01 |   | 05 00 04 00 |
# | 09 0e 0b 0d | = | 01 02 03 01 | x | 00 05 00 04 |
# | 0d 09 0e 0b |   | 01 01 02 03 |   | 04 00 05 00 |
# | 0b 0d 09 0e |   | 03 01 01 02 |   | 00 04 00 05 |

$code.=<<___;
	@ multiplication by 0x05-0x00-0x04-0x00
	vext.8	@t[0], @x[0], @x[0], #8
	vext.8	@t[6], @x[6], @x[6], #8
	vext.8	@t[7], @x[7], @x[7], #8
	veor	@t[0], @t[0], @x[0]
	vext.8	@t[1], @x[1], @x[1], #8
	veor	@t[6], @t[6], @x[6]
	vext.8	@t[2], @x[2], @x[2], #8
	veor	@t[7], @t[7], @x[7]
	vext.8	@t[3], @x[3], @x[3], #8
	veor	@t[1], @t[1], @x[1]
	vext.8	@t[4], @x[4], @x[4], #8
	veor	@t[2], @t[2], @x[2]
	vext.8	@t[5], @x[5], @x[5], #8
	veor	@t[3], @t[3], @x[3]
	veor	@t[4], @t[4], @x[4]
	veor	@t[5], @t[5], @x[5]

	 veor	@x[0], @x[0], @t[6]
	 veor	@x[1], @x[1], @t[6]
	 veor	@x[2], @x[2], @t[0]
	 veor	@x[4], @x[4], @t[2]
	 veor	@x[3], @x[3], @t[1]
	 veor	@x[1], @x[1], @t[7]
	 veor	@x[2], @x[2], @t[7]
	 veor	@x[4], @x[4], @t[6]
	 veor	@x[5], @x[5], @t[3]
	 veor	@x[3], @x[3], @t[6]
	 veor	@x[6], @x[6], @t[4]
	 veor	@x[4], @x[4], @t[7]
	 veor	@x[5], @x[5], @t[7]
	 veor	@x[7], @x[7], @t[5]
___
	&MixColumns	(@x,@t,1);	# flipped 2<->3 and 4<->6
}

sub swapmove {
my ($a,$b,$n,$mask,$t)=@_;
$code.=<<___;
	vshr.u64	$t, $b, #$n
	veor		$t, $t, $a
	vand		$t, $t, $mask
	veor		$a, $a, $t
	vshl.u64	$t, $t, #$n
	veor		$b, $b, $t
___
}
sub swapmove2x {
my ($a0,$b0,$a1,$b1,$n,$mask,$t0,$t1)=@_;
$code.=<<___;
	vshr.u64	$t0, $b0, #$n
	 vshr.u64	$t1, $b1, #$n
	veor		$t0, $t0, $a0
	 veor		$t1, $t1, $a1
	vand		$t0, $t0, $mask
	 vand		$t1, $t1, $mask
	veor		$a0, $a0, $t0
	vshl.u64	$t0, $t0, #$n
	 veor		$a1, $a1, $t1
	 vshl.u64	$t1, $t1, #$n
	veor		$b0, $b0, $t0
	 veor		$b1, $b1, $t1
___
}

sub bitslice {
my @x=reverse(@_[0..7]);
my ($t0,$t1,$t2,$t3)=@_[8..11];
$code.=<<___;
	vmov.i8	$t0,#0x55			@ compose .LBS0
	vmov.i8	$t1,#0x33			@ compose .LBS1
___
	&swapmove2x(@x[0,1,2,3],1,$t0,$t2,$t3);
	&swapmove2x(@x[4,5,6,7],1,$t0,$t2,$t3);
$code.=<<___;
	vmov.i8	$t0,#0x0f			@ compose .LBS2
___
	&swapmove2x(@x[0,2,1,3],2,$t1,$t2,$t3);
	&swapmove2x(@x[4,6,5,7],2,$t1,$t2,$t3);

	&swapmove2x(@x[0,4,1,5],4,$t0,$t2,$t3);
	&swapmove2x(@x[2,6,3,7],4,$t0,$t2,$t3);
}

$code.=<<___;
#ifndef __KERNEL__
# define VFP_ABI_PUSH	vstmdb	sp!,{d8-d15}
# define VFP_ABI_POP	vldmia	sp!,{d8-d15}
# define VFP_ABI_FRAME	0x40
#else
# define VFP_ABI_PUSH
# define VFP_ABI_POP
# define VFP_ABI_FRAME	0
# define BSAES_ASM_EXTENDED_KEY
# define __ARM_MAX_ARCH__ 7
#endif

#ifdef __thumb__
# define adrl adr
#endif

#if __ARM_MAX_ARCH__>=7
.arch	armv7-a
.fpu	neon

.text
.syntax	unified 	@ ARMv7-capable assembler is expected to handle this
#if defined(__thumb2__) && !defined(__APPLE__)
.thumb
#else
.code   32
# undef __thumb2__
#endif

.type	_bsaes_const,%object
.align	6
_bsaes_const:
.LM0ISR:	@ InvShiftRows constants
	.quad	0x0a0e0206070b0f03, 0x0004080c0d010509
.LISR:
	.quad	0x0504070602010003, 0x0f0e0d0c080b0a09
.LISRM0:
	.quad	0x01040b0e0205080f, 0x0306090c00070a0d
.LM0SR:		@ ShiftRows constants
	.quad	0x0a0e02060f03070b, 0x0004080c05090d01
.LSR:
	.quad	0x0504070600030201, 0x0f0e0d0c0a09080b
.LSRM0:
	.quad	0x0304090e00050a0f, 0x01060b0c0207080d
.LM0:
	.quad	0x02060a0e03070b0f, 0x0004080c0105090d
.LREVM0SR:
	.quad	0x090d01050c000408, 0x03070b0f060a0e02
.asciz	"Bit-sliced AES for NEON, CRYPTOGAMS by <appro\@openssl.org>"
.align	6
.size	_bsaes_const,.-_bsaes_const

.type	_bsaes_encrypt8,%function
.align	4
_bsaes_encrypt8:
	adr	$const,.
	vldmia	$key!, {@XMM[9]}		@ round 0 key
#if defined(__thumb2__) || defined(__APPLE__)
	adr	$const,.LM0SR
#else
	sub	$const,$const,#_bsaes_encrypt8-.LM0SR
#endif

	vldmia	$const!, {@XMM[8]}		@ .LM0SR
_bsaes_encrypt8_alt:
	veor	@XMM[10], @XMM[0], @XMM[9]	@ xor with round0 key
	veor	@XMM[11], @XMM[1], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[0])`, {@XMM[10]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[0])`, {@XMM[10]}, `&Dhi(@XMM[8])`
	veor	@XMM[12], @XMM[2], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[1])`, {@XMM[11]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[1])`, {@XMM[11]}, `&Dhi(@XMM[8])`
	veor	@XMM[13], @XMM[3], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[2])`, {@XMM[12]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[2])`, {@XMM[12]}, `&Dhi(@XMM[8])`
	veor	@XMM[14], @XMM[4], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[3])`, {@XMM[13]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[3])`, {@XMM[13]}, `&Dhi(@XMM[8])`
	veor	@XMM[15], @XMM[5], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[4])`, {@XMM[14]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[4])`, {@XMM[14]}, `&Dhi(@XMM[8])`
	veor	@XMM[10], @XMM[6], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[5])`, {@XMM[15]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[5])`, {@XMM[15]}, `&Dhi(@XMM[8])`
	veor	@XMM[11], @XMM[7], @XMM[9]
	 vtbl.8	`&Dlo(@XMM[6])`, {@XMM[10]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[6])`, {@XMM[10]}, `&Dhi(@XMM[8])`
	 vtbl.8	`&Dlo(@XMM[7])`, {@XMM[11]}, `&Dlo(@XMM[8])`
	 vtbl.8	`&Dhi(@XMM[7])`, {@XMM[11]}, `&Dhi(@XMM[8])`
_bsaes_encrypt8_bitslice:
___
	&bitslice	(@XMM[0..7, 8..11]);
$code.=<<___;
	sub	$rounds,$rounds,#1
	b	.Lenc_sbox
.align	4
.Lenc_loop:
___
	&ShiftRows	(@XMM[0..7, 8..12]);
$code.=".Lenc_sbox:\n";
	&Sbox		(@XMM[0..7, 8..15]);
$code.=<<___;
	subs	$rounds,$rounds,#1
	bcc	.Lenc_done
___
	&MixColumns	(@XMM[0,1,4,6,3,7,2,5, 8..15]);
$code.=<<___;
	vldmia	$const, {@XMM[12]}		@ .LSR
	ite	eq				@ Thumb2 thing, samity check in ARM
	addeq	$const,$const,#0x10
	bne	.Lenc_loop
	vldmia	$const, {@XMM[12]}		@ .LSRM0
	b	.Lenc_loop
.align	4
.Lenc_done:
___
	# output in lsb > [t0, t1, t4, t6, t3, t7, t2, t5] < msb
	&bitslice	(@XMM[0,1,4,6,3,7,2,5, 8..11]);
$code.=<<___;
	vldmia	$key, {@XMM[8]}			@ last round key
	veor	@XMM[4], @XMM[4], @XMM[8]
	veor	@XMM[6], @XMM[6], @XMM[8]
	veor	@XMM[3], @XMM[3], @XMM[8]
	veor	@XMM[7], @XMM[7], @XMM[8]
	veor	@XMM[2], @XMM[2], @XMM[8]
	veor	@XMM[5], @XMM[5], @XMM[8]
	veor	@XMM[0], @XMM[0], @XMM[8]
	veor	@XMM[1], @XMM[1], @XMM[8]
	bx	lr
.size	_bsaes_encrypt8,.-_bsaes_encrypt8
___
}
{
my ($out,$inp,$rounds,$const)=("r12","r4","r5","r6");

sub bitslice_key {
my @x=reverse(@_[0..7]);
my ($bs0,$bs1,$bs2,$t2,$t3)=@_[8..12];

	&swapmove	(@x[0,1],1,$bs0,$t2,$t3);
$code.=<<___;
	@ &swapmove(@x[2,3],1,$t0,$t2,$t3);
	vmov	@x[2], @x[0]
	vmov	@x[3], @x[1]
___
	#&swapmove2x(@x[4,5,6,7],1,$t0,$t2,$t3);

	&swapmove2x	(@x[0,2,1,3],2,$bs1,$t2,$t3);
$code.=<<___;
	@ &swapmove2x(@x[4,6,5,7],2,$t1,$t2,$t3);
	vmov	@x[4], @x[0]
	vmov	@x[6], @x[2]
	vmov	@x[5], @x[1]
	vmov	@x[7], @x[3]
___
	&swapmove2x	(@x[0,4,1,5],4,$bs2,$t2,$t3);
	&swapmove2x	(@x[2,6,3,7],4,$bs2,$t2,$t3);
}

$code.=<<___;
.type	_bsaes_key_convert,%function
.align	4
_bsaes_key_convert:
	adr	$const,.
	vld1.8	{@XMM[7]},  [$inp]!		@ load round 0 key
#if defined(__thumb2__) || defined(__APPLE__)
	adr	$const,.LM0
#else
	sub	$const,$const,#_bsaes_key_convert-.LM0
#endif
	vld1.8	{@XMM[15]}, [$inp]!		@ load round 1 key

	vmov.i8	@XMM[8],  #0x01			@ bit masks
	vmov.i8	@XMM[9],  #0x02
	vmov.i8	@XMM[10], #0x04
	vmov.i8	@XMM[11], #0x08
	vmov.i8	@XMM[12], #0x10
	vmov.i8	@XMM[13], #0x20
	vldmia	$const, {@XMM[14]}		@ .LM0

#ifdef __ARMEL__
	vrev32.8	@XMM[7],  @XMM[7]
	vrev32.8	@XMM[15], @XMM[15]
#endif
	sub	$rounds,$rounds,#1
	vstmia	$out!, {@XMM[7]}		@ save round 0 key
	b	.Lkey_loop

.align	4
.Lkey_loop:
	vtbl.8	`&Dlo(@XMM[7])`,{@XMM[15]},`&Dlo(@XMM[14])`
	vtbl.8	`&Dhi(@XMM[7])`,{@XMM[15]},`&Dhi(@XMM[14])`
	vmov.i8	@XMM[6],  #0x40
	vmov.i8	@XMM[15], #0x80

	vtst.8	@XMM[0], @XMM[7], @XMM[8]
	vtst.8	@XMM[1], @XMM[7], @XMM[9]
	vtst.8	@XMM[2], @XMM[7], @XMM[10]
	vtst.8	@XMM[3], @XMM[7], @XMM[11]
	vtst.8	@XMM[4], @XMM[7], @XMM[12]
	vtst.8	@XMM[5], @XMM[7], @XMM[13]
	vtst.8	@XMM[6], @XMM[7], @XMM[6]
	vtst.8	@XMM[7], @XMM[7], @XMM[15]
	vld1.8	{@XMM[15]}, [$inp]!		@ load next round key
	vmvn	@XMM[0], @XMM[0]		@ "pnot"
	vmvn	@XMM[1], @XMM[1]
	vmvn	@XMM[5], @XMM[5]
	vmvn	@XMM[6], @XMM[6]
#ifdef __ARMEL__
	vrev32.8	@XMM[15], @XMM[15]
#endif
	subs	$rounds,$rounds,#1
	vstmia	$out!,{@XMM[0]-@XMM[7]}		@ write bit-sliced round key
	bne	.Lkey_loop

	vmov.i8	@XMM[7],#0x63			@ compose .L63
	@ don't save last round key
	bx	lr
.size	_bsaes_key_convert,.-_bsaes_key_convert
___
}

{
my ($inp,$out,$len,$key, $ctr,$fp,$rounds)=(map("r$_",(0..3,8..10)));
my $const = "r6";	# shared with _bsaes_encrypt8_alt
my $keysched = "sp";

$code.=<<___;
.global	bsaes_ctr32_encrypt_blocks
.type	bsaes_ctr32_encrypt_blocks,%function
.align	5
bsaes_ctr32_encrypt_blocks:
	@ In OpenSSL, short inputs fall back to aes_nohw_* here. We patch this
	@ out to retain a constant-time implementation.
	mov	ip, sp
	stmdb	sp!, {r4-r10, lr}
	VFP_ABI_PUSH
	ldr	$ctr, [ip]			@ ctr is 1st arg on the stack
	sub	sp, sp, #0x10			@ scratch space to carry over the ctr
	mov	$fp, sp				@ save sp

	ldr	$rounds, [$key, #240]		@ get # of rounds
#ifndef	BSAES_ASM_EXTENDED_KEY
	@ allocate the key schedule on the stack
	sub	r12, sp, $rounds, lsl#7		@ 128 bytes per inner round key
	add	r12, #`128-32`			@ size of bit-sliced key schedule

	@ populate the key schedule
	mov	r4, $key			@ pass key
	mov	r5, $rounds			@ pass # of rounds
	mov	sp, r12				@ sp is $keysched
	bl	_bsaes_key_convert
	veor	@XMM[7],@XMM[7],@XMM[15]	@ fix up last round key
	vstmia	r12, {@XMM[7]}			@ save last round key

	vld1.8	{@XMM[0]}, [$ctr]		@ load counter
#ifdef	__APPLE__
	mov	$ctr, #:lower16:(.LREVM0SR-.LM0)
	add	$ctr, $const, $ctr
#else
	add	$ctr, $const, #.LREVM0SR-.LM0	@ borrow $ctr
#endif
	vldmia	$keysched, {@XMM[4]}		@ load round0 key
#else
	ldr	r12, [$key, #244]
	eors	r12, #1
	beq	0f

	@ populate the key schedule
	str	r12, [$key, #244]
	mov	r4, $key			@ pass key
	mov	r5, $rounds			@ pass # of rounds
	add	r12, $key, #248			@ pass key schedule
	bl	_bsaes_key_convert
	veor	@XMM[7],@XMM[7],@XMM[15]	@ fix up last round key
	vstmia	r12, {@XMM[7]}			@ save last round key

.align	2
0:	add	r12, $key, #248
	vld1.8	{@XMM[0]}, [$ctr]		@ load counter
	adrl	$ctr, .LREVM0SR			@ borrow $ctr
	vldmia	r12, {@XMM[4]}			@ load round0 key
	sub	sp, #0x10			@ place for adjusted round0 key
#endif

	vmov.i32	@XMM[8],#1		@ compose 1<<96
	veor		@XMM[9],@XMM[9],@XMM[9]
	vrev32.8	@XMM[0],@XMM[0]
	vext.8		@XMM[8],@XMM[9],@XMM[8],#4
	vrev32.8	@XMM[4],@XMM[4]
	vadd.u32	@XMM[9],@XMM[8],@XMM[8]	@ compose 2<<96
	vstmia	$keysched, {@XMM[4]}		@ save adjusted round0 key
	b	.Lctr_enc_loop

.align	4
.Lctr_enc_loop:
	vadd.u32	@XMM[10], @XMM[8], @XMM[9]	@ compose 3<<96
	vadd.u32	@XMM[1], @XMM[0], @XMM[8]	@ +1
	vadd.u32	@XMM[2], @XMM[0], @XMM[9]	@ +2
	vadd.u32	@XMM[3], @XMM[0], @XMM[10]	@ +3
	vadd.u32	@XMM[4], @XMM[1], @XMM[10]
	vadd.u32	@XMM[5], @XMM[2], @XMM[10]
	vadd.u32	@XMM[6], @XMM[3], @XMM[10]
	vadd.u32	@XMM[7], @XMM[4], @XMM[10]
	vadd.u32	@XMM[10], @XMM[5], @XMM[10]	@ next counter

	@ Borrow prologue from _bsaes_encrypt8 to use the opportunity
	@ to flip byte order in 32-bit counter

	vldmia		$keysched, {@XMM[9]}		@ load round0 key
#ifndef	BSAES_ASM_EXTENDED_KEY
	add		r4, $keysched, #0x10		@ pass next round key
#else
	add		r4, $key, #`248+16`
#endif
	vldmia		$ctr, {@XMM[8]}			@ .LREVM0SR
	mov		r5, $rounds			@ pass rounds
	vstmia		$fp, {@XMM[10]}			@ save next counter
#ifdef	__APPLE__
	mov		$const, #:lower16:(.LREVM0SR-.LSR)
	sub		$const, $ctr, $const
#else
	sub		$const, $ctr, #.LREVM0SR-.LSR	@ pass constants
#endif

	bl		_bsaes_encrypt8_alt

	subs		$len, $len, #8
	blo		.Lctr_enc_loop_done

	vld1.8		{@XMM[8]-@XMM[9]}, [$inp]!	@ load input
	vld1.8		{@XMM[10]-@XMM[11]}, [$inp]!
	veor		@XMM[0], @XMM[8]
	veor		@XMM[1], @XMM[9]
	vld1.8		{@XMM[12]-@XMM[13]}, [$inp]!
	veor		@XMM[4], @XMM[10]
	veor		@XMM[6], @XMM[11]
	vld1.8		{@XMM[14]-@XMM[15]}, [$inp]!
	veor		@XMM[3], @XMM[12]
	vst1.8		{@XMM[0]-@XMM[1]}, [$out]!	@ write output
	veor		@XMM[7], @XMM[13]
	veor		@XMM[2], @XMM[14]
	vst1.8		{@XMM[4]}, [$out]!
	veor		@XMM[5], @XMM[15]
	vst1.8		{@XMM[6]}, [$out]!
	vmov.i32	@XMM[8], #1			@ compose 1<<96
	vst1.8		{@XMM[3]}, [$out]!
	veor		@XMM[9], @XMM[9], @XMM[9]
	vst1.8		{@XMM[7]}, [$out]!
	vext.8		@XMM[8], @XMM[9], @XMM[8], #4
	vst1.8		{@XMM[2]}, [$out]!
	vadd.u32	@XMM[9],@XMM[8],@XMM[8]		@ compose 2<<96
	vst1.8		{@XMM[5]}, [$out]!
	vldmia		$fp, {@XMM[0]}			@ load counter

	bne		.Lctr_enc_loop
	b		.Lctr_enc_done

.align	4
.Lctr_enc_loop_done:
	add		$len, $len, #8
	vld1.8		{@XMM[8]}, [$inp]!	@ load input
	veor		@XMM[0], @XMM[8]
	vst1.8		{@XMM[0]}, [$out]!	@ write output
	cmp		$len, #2
	blo		.Lctr_enc_done
	vld1.8		{@XMM[9]}, [$inp]!
	veor		@XMM[1], @XMM[9]
	vst1.8		{@XMM[1]}, [$out]!
	beq		.Lctr_enc_done
	vld1.8		{@XMM[10]}, [$inp]!
	veor		@XMM[4], @XMM[10]
	vst1.8		{@XMM[4]}, [$out]!
	cmp		$len, #4
	blo		.Lctr_enc_done
	vld1.8		{@XMM[11]}, [$inp]!
	veor		@XMM[6], @XMM[11]
	vst1.8		{@XMM[6]}, [$out]!
	beq		.Lctr_enc_done
	vld1.8		{@XMM[12]}, [$inp]!
	veor		@XMM[3], @XMM[12]
	vst1.8		{@XMM[3]}, [$out]!
	cmp		$len, #6
	blo		.Lctr_enc_done
	vld1.8		{@XMM[13]}, [$inp]!
	veor		@XMM[7], @XMM[13]
	vst1.8		{@XMM[7]}, [$out]!
	beq		.Lctr_enc_done
	vld1.8		{@XMM[14]}, [$inp]
	veor		@XMM[2], @XMM[14]
	vst1.8		{@XMM[2]}, [$out]!

.Lctr_enc_done:
	vmov.i32	q0, #0
	vmov.i32	q1, #0
#ifndef	BSAES_ASM_EXTENDED_KEY
.Lctr_enc_bzero:			@ wipe key schedule [if any]
	vstmia		$keysched!, {q0-q1}
	cmp		$keysched, $fp
	bne		.Lctr_enc_bzero
#else
	vstmia		$keysched, {q0-q1}
#endif

	mov	sp, $fp
	add	sp, #0x10		@ add sp,$fp,#0x10 is no good for thumb
	VFP_ABI_POP
	ldmia	sp!, {r4-r10, pc}	@ return

	@ OpenSSL contains aes_nohw_* fallback code here. We patch this
	@ out to retain a constant-time implementation.
.size	bsaes_ctr32_encrypt_blocks,.-bsaes_ctr32_encrypt_blocks
___
}
$code.=<<___;
#endif
___

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

open SELF,$0;
while(<SELF>) {
	next if (/^#!/);
        last if (!s/^#/@/ and !/^$/);
        print;
}
close SELF;

print $code;

close STDOUT or die "error closing STDOUT: $!";
