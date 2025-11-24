#! /usr/bin/env perl
# Copyright 2009-2016 The OpenSSL Project Authors. All Rights Reserved.
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
# This module implements support for Intel AES-NI extension. In
# OpenSSL context it's used with Intel engine, but can also be used as
# drop-in replacement for crypto/aes/asm/aes-x86_64.pl [see below for
# details].
#
# Performance.
#
# Given aes(enc|dec) instructions' latency asymptotic performance for
# non-parallelizable modes such as CBC encrypt is 3.75 cycles per byte
# processed with 128-bit key. And given their throughput asymptotic
# performance for parallelizable modes is 1.25 cycles per byte. Being
# asymptotic limit it's not something you commonly achieve in reality,
# but how close does one get? Below are results collected for
# different modes and block sized. Pairs of numbers are for en-/
# decryption.
#
#	16-byte     64-byte     256-byte    1-KB        8-KB
# ECB	4.25/4.25   1.38/1.38   1.28/1.28   1.26/1.26	1.26/1.26
# CTR	5.42/5.42   1.92/1.92   1.44/1.44   1.28/1.28   1.26/1.26
# CBC	4.38/4.43   4.15/1.43   4.07/1.32   4.07/1.29   4.06/1.28
# CCM	5.66/9.42   4.42/5.41   4.16/4.40   4.09/4.15   4.06/4.07
# OFB	5.42/5.42   4.64/4.64   4.44/4.44   4.39/4.39   4.38/4.38
# CFB	5.73/5.85   5.56/5.62   5.48/5.56   5.47/5.55   5.47/5.55
#
# ECB, CTR, CBC and CCM results are free from EVP overhead. This means
# that otherwise used 'openssl speed -evp aes-128-??? -engine aesni
# [-decrypt]' will exhibit 10-15% worse results for smaller blocks.
# The results were collected with specially crafted speed.c benchmark
# in order to compare them with results reported in "Intel Advanced
# Encryption Standard (AES) New Instruction Set" White Paper Revision
# 3.0 dated May 2010. All above results are consistently better. This
# module also provides better performance for block sizes smaller than
# 128 bytes in points *not* represented in the above table.
#
# Looking at the results for 8-KB buffer.
#
# CFB and OFB results are far from the limit, because implementation
# uses "generic" CRYPTO_[c|o]fb128_encrypt interfaces relying on
# single-block aesni_encrypt, which is not the most optimal way to go.
# CBC encrypt result is unexpectedly high and there is no documented
# explanation for it. Seemingly there is a small penalty for feeding
# the result back to AES unit the way it's done in CBC mode. There is
# nothing one can do and the result appears optimal. CCM result is
# identical to CBC, because CBC-MAC is essentially CBC encrypt without
# saving output. CCM CTR "stays invisible," because it's neatly
# interleaved wih CBC-MAC. This provides ~30% improvement over
# "straightforward" CCM implementation with CTR and CBC-MAC performed
# disjointly. Parallelizable modes practically achieve the theoretical
# limit.
#
# Looking at how results vary with buffer size.
#
# Curves are practically saturated at 1-KB buffer size. In most cases
# "256-byte" performance is >95%, and "64-byte" is ~90% of "8-KB" one.
# CTR curve doesn't follow this pattern and is "slowest" changing one
# with "256-byte" result being 87% of "8-KB." This is because overhead
# in CTR mode is most computationally intensive. Small-block CCM
# decrypt is slower than encrypt, because first CTR and last CBC-MAC
# iterations can't be interleaved.
#
# Results for 192- and 256-bit keys.
#
# EVP-free results were observed to scale perfectly with number of
# rounds for larger block sizes, i.e. 192-bit result being 10/12 times
# lower and 256-bit one - 10/14. Well, in CBC encrypt case differences
# are a tad smaller, because the above mentioned penalty biases all
# results by same constant value. In similar way function call
# overhead affects small-block performance, as well as OFB and CFB
# results. Differences are not large, most common coefficients are
# 10/11.7 and 10/13.4 (as opposite to 10/12.0 and 10/14.0), but one
# observe even 10/11.2 and 10/12.4 (CTR, OFB, CFB)...

# January 2011
#
# While Westmere processor features 6 cycles latency for aes[enc|dec]
# instructions, which can be scheduled every second cycle, Sandy
# Bridge spends 8 cycles per instruction, but it can schedule them
# every cycle. This means that code targeting Westmere would perform
# suboptimally on Sandy Bridge. Therefore this update.
#
# In addition, non-parallelizable CBC encrypt (as well as CCM) is
# optimized. Relative improvement might appear modest, 8% on Westmere,
# but in absolute terms it's 3.77 cycles per byte encrypted with
# 128-bit key on Westmere, and 5.07 - on Sandy Bridge. These numbers
# should be compared to asymptotic limits of 3.75 for Westmere and
# 5.00 for Sandy Bridge. Actually, the fact that they get this close
# to asymptotic limits is quite amazing. Indeed, the limit is
# calculated as latency times number of rounds, 10 for 128-bit key,
# and divided by 16, the number of bytes in block, or in other words
# it accounts *solely* for aesenc instructions. But there are extra
# instructions, and numbers so close to the asymptotic limits mean
# that it's as if it takes as little as *one* additional cycle to
# execute all of them. How is it possible? It is possible thanks to
# out-of-order execution logic, which manages to overlap post-
# processing of previous block, things like saving the output, with
# actual encryption of current block, as well as pre-processing of
# current block, things like fetching input and xor-ing it with
# 0-round element of the key schedule, with actual encryption of
# previous block. Keep this in mind...
#
# For parallelizable modes, such as ECB, CBC decrypt, CTR, higher
# performance is achieved by interleaving instructions working on
# independent blocks. In which case asymptotic limit for such modes
# can be obtained by dividing above mentioned numbers by AES
# instructions' interleave factor. Westmere can execute at most 3
# instructions at a time, meaning that optimal interleave factor is 3,
# and that's where the "magic" number of 1.25 come from. "Optimal
# interleave factor" means that increase of interleave factor does
# not improve performance. The formula has proven to reflect reality
# pretty well on Westmere... Sandy Bridge on the other hand can
# execute up to 8 AES instructions at a time, so how does varying
# interleave factor affect the performance? Here is table for ECB
# (numbers are cycles per byte processed with 128-bit key):
#
# instruction interleave factor		3x	6x	8x
# theoretical asymptotic limit		1.67	0.83	0.625
# measured performance for 8KB block	1.05	0.86	0.84
#
# "as if" interleave factor		4.7x	5.8x	6.0x
#
# Further data for other parallelizable modes:
#
# CBC decrypt				1.16	0.93	0.74
# CTR					1.14	0.91	0.74
#
# Well, given 3x column it's probably inappropriate to call the limit
# asymptotic, if it can be surpassed, isn't it? What happens there?
# Rewind to CBC paragraph for the answer. Yes, out-of-order execution
# magic is responsible for this. Processor overlaps not only the
# additional instructions with AES ones, but even AES instructions
# processing adjacent triplets of independent blocks. In the 6x case
# additional instructions  still claim disproportionally small amount
# of additional cycles, but in 8x case number of instructions must be
# a tad too high for out-of-order logic to cope with, and AES unit
# remains underutilized... As you can see 8x interleave is hardly
# justifiable, so there no need to feel bad that 32-bit aesni-x86.pl
# utilizes 6x interleave because of limited register bank capacity.
#
# Higher interleave factors do have negative impact on Westmere
# performance. While for ECB mode it's negligible ~1.5%, other
# parallelizables perform ~5% worse, which is outweighed by ~25%
# improvement on Sandy Bridge. To balance regression on Westmere
# CTR mode was implemented with 6x aesenc interleave factor.

# April 2011
#
# Add aesni_xts_[en|de]crypt. Westmere spends 1.25 cycles processing
# one byte out of 8KB with 128-bit key, Sandy Bridge - 0.90. Just like
# in CTR mode AES instruction interleave factor was chosen to be 6x.

######################################################################
# Current large-block performance in cycles per byte processed with
# 128-bit key (less is better).
#
#		CBC en-/decrypt	CTR	XTS	ECB	OCB
# Westmere	3.77/1.25	1.25	1.25	1.26
# * Bridge	5.07/0.74	0.75	0.90	0.85	0.98
# Haswell	4.44/0.63	0.63	0.73	0.63	0.70
# Skylake	2.62/0.63	0.63	0.63	0.63
# Silvermont	5.75/3.54	3.56	4.12	3.87(*)	4.11
# Knights L	2.54/0.77	0.78	0.85	-	1.50
# Goldmont	3.82/1.26	1.26	1.29	1.29	1.50
# Bulldozer	5.77/0.70	0.72	0.90	0.70	0.95
# Ryzen		2.71/0.35	0.35	0.44	0.38	0.49
#
# (*)	Atom Silvermont ECB result is suboptimal because of penalties
#	incurred by operations on %xmm8-15. As ECB is not considered
#	critical, nothing was done to mitigate the problem.

$PREFIX="aes_hw";	# if $PREFIX is set to "AES", the script
			# generates drop-in replacement for
			# crypto/aes/asm/aes-x86_64.pl:-)

$flavour = shift;
$output  = shift;
if ($flavour =~ /\./) { $output = $flavour; undef $flavour; }

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

$movkey = $PREFIX eq "aes_hw" ? "movups" : "movups";
@_4args=$win64?	("%rcx","%rdx","%r8", "%r9") :	# Win64 order
		("%rdi","%rsi","%rdx","%rcx");	# Unix order

$code=".text\n";

$rounds="%eax";	# input to and changed by aesni_[en|de]cryptN !!!
# this is natural Unix argument order for public $PREFIX_[ecb|cbc]_encrypt ...
$inp="%rdi";
$out="%rsi";
$len="%rdx";
$key="%rcx";	# input to and changed by aesni_[en|de]cryptN !!!
$ivp="%r8";	# cbc, ctr, ...

$rnds_="%r10d";	# backup copy for $rounds
$key_="%r11";	# backup copy for $key

# %xmm register layout
$rndkey0="%xmm0";	$rndkey1="%xmm1";
$inout0="%xmm2";	$inout1="%xmm3";
$inout2="%xmm4";	$inout3="%xmm5";
$inout4="%xmm6";	$inout5="%xmm7";
$inout6="%xmm8";	$inout7="%xmm9";

$in2="%xmm6";		$in1="%xmm7";	# used in CBC decrypt, CTR, ...
$in0="%xmm8";		$iv="%xmm9";

# Inline version of internal aesni_[en|de]crypt1.
#
# Why folded loop? Because aes[enc|dec] is slow enough to accommodate
# cycles which take care of loop variables...
{ my $sn;
sub aesni_generate1 {
my ($p,$key,$rounds,$inout,$ivec)=@_;	$inout=$inout0 if (!defined($inout));
++$sn;
$code.=<<___;
	$movkey	($key),$rndkey0
	$movkey	16($key),$rndkey1
___
$code.=<<___ if (defined($ivec));
	xorps	$rndkey0,$ivec
	lea	32($key),$key
	xorps	$ivec,$inout
___
$code.=<<___ if (!defined($ivec));
	lea	32($key),$key
	xorps	$rndkey0,$inout
___
$code.=<<___;
.Loop_${p}1_$sn:
	aes${p}	$rndkey1,$inout
	dec	$rounds
	$movkey	($key),$rndkey1
	lea	16($key),$key
	jnz	.Loop_${p}1_$sn	# loop body is 16 bytes
	aes${p}last	$rndkey1,$inout
___
}}

# _aesni_[en|de]cryptN are private interfaces, N denotes interleave
# factor. Why 3x subroutine were originally used in loops? Even though
# aes[enc|dec] latency was originally 6, it could be scheduled only
# every *2nd* cycle. Thus 3x interleave was the one providing optimal
# utilization, i.e. when subroutine's throughput is virtually same as
# of non-interleaved subroutine [for number of input blocks up to 3].
# This is why it originally made no sense to implement 2x subroutine.
# But times change and it became appropriate to spend extra 192 bytes
# on 2x subroutine on Atom Silvermont account. For processors that
# can schedule aes[enc|dec] every cycle optimal interleave factor
# equals to corresponding instructions latency. 8x is optimal for
# * Bridge and "super-optimal" for other Intel CPUs...

sub aesni_generate2 {
my $dir=shift;
# As already mentioned it takes in $key and $rounds, which are *not*
# preserved. $inout[0-1] is cipher/clear text...
$code.=<<___;
.type	_aesni_${dir}rypt2,\@abi-omnipotent
.align	16
_aesni_${dir}rypt2:
.cfi_startproc
	$movkey	($key),$rndkey0
	shl	\$4,$rounds
	$movkey	16($key),$rndkey1
	xorps	$rndkey0,$inout0
	xorps	$rndkey0,$inout1
	$movkey	32($key),$rndkey0
	lea	32($key,$rounds),$key
	neg	%rax				# $rounds
	add	\$16,%rax

.L${dir}_loop2:
	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	$movkey		($key,%rax),$rndkey1
	add		\$32,%rax
	aes${dir}	$rndkey0,$inout0
	aes${dir}	$rndkey0,$inout1
	$movkey		-16($key,%rax),$rndkey0
	jnz		.L${dir}_loop2

	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}last	$rndkey0,$inout0
	aes${dir}last	$rndkey0,$inout1
	ret
.cfi_endproc
.size	_aesni_${dir}rypt2,.-_aesni_${dir}rypt2
___
}
sub aesni_generate3 {
my $dir=shift;
# As already mentioned it takes in $key and $rounds, which are *not*
# preserved. $inout[0-2] is cipher/clear text...
$code.=<<___;
.type	_aesni_${dir}rypt3,\@abi-omnipotent
.align	16
_aesni_${dir}rypt3:
.cfi_startproc
	$movkey	($key),$rndkey0
	shl	\$4,$rounds
	$movkey	16($key),$rndkey1
	xorps	$rndkey0,$inout0
	xorps	$rndkey0,$inout1
	xorps	$rndkey0,$inout2
	$movkey	32($key),$rndkey0
	lea	32($key,$rounds),$key
	neg	%rax				# $rounds
	add	\$16,%rax

.L${dir}_loop3:
	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	$movkey		($key,%rax),$rndkey1
	add		\$32,%rax
	aes${dir}	$rndkey0,$inout0
	aes${dir}	$rndkey0,$inout1
	aes${dir}	$rndkey0,$inout2
	$movkey		-16($key,%rax),$rndkey0
	jnz		.L${dir}_loop3

	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	aes${dir}last	$rndkey0,$inout0
	aes${dir}last	$rndkey0,$inout1
	aes${dir}last	$rndkey0,$inout2
	ret
.cfi_endproc
.size	_aesni_${dir}rypt3,.-_aesni_${dir}rypt3
___
}
# 4x interleave is implemented to improve small block performance,
# most notably [and naturally] 4 block by ~30%. One can argue that one
# should have implemented 5x as well, but improvement would be <20%,
# so it's not worth it...
sub aesni_generate4 {
my $dir=shift;
# As already mentioned it takes in $key and $rounds, which are *not*
# preserved. $inout[0-3] is cipher/clear text...
$code.=<<___;
.type	_aesni_${dir}rypt4,\@abi-omnipotent
.align	16
_aesni_${dir}rypt4:
.cfi_startproc
	$movkey	($key),$rndkey0
	shl	\$4,$rounds
	$movkey	16($key),$rndkey1
	xorps	$rndkey0,$inout0
	xorps	$rndkey0,$inout1
	xorps	$rndkey0,$inout2
	xorps	$rndkey0,$inout3
	$movkey	32($key),$rndkey0
	lea	32($key,$rounds),$key
	neg	%rax				# $rounds
	.byte	0x0f,0x1f,0x00
	add	\$16,%rax

.L${dir}_loop4:
	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	aes${dir}	$rndkey1,$inout3
	$movkey		($key,%rax),$rndkey1
	add		\$32,%rax
	aes${dir}	$rndkey0,$inout0
	aes${dir}	$rndkey0,$inout1
	aes${dir}	$rndkey0,$inout2
	aes${dir}	$rndkey0,$inout3
	$movkey		-16($key,%rax),$rndkey0
	jnz		.L${dir}_loop4

	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	aes${dir}	$rndkey1,$inout3
	aes${dir}last	$rndkey0,$inout0
	aes${dir}last	$rndkey0,$inout1
	aes${dir}last	$rndkey0,$inout2
	aes${dir}last	$rndkey0,$inout3
	ret
.cfi_endproc
.size	_aesni_${dir}rypt4,.-_aesni_${dir}rypt4
___
}
sub aesni_generate6 {
my $dir=shift;
# As already mentioned it takes in $key and $rounds, which are *not*
# preserved. $inout[0-5] is cipher/clear text...
$code.=<<___;
.type	_aesni_${dir}rypt6,\@abi-omnipotent
.align	16
_aesni_${dir}rypt6:
.cfi_startproc
	$movkey		($key),$rndkey0
	shl		\$4,$rounds
	$movkey		16($key),$rndkey1
	xorps		$rndkey0,$inout0
	pxor		$rndkey0,$inout1
	pxor		$rndkey0,$inout2
	aes${dir}	$rndkey1,$inout0
	lea		32($key,$rounds),$key
	neg		%rax			# $rounds
	aes${dir}	$rndkey1,$inout1
	pxor		$rndkey0,$inout3
	pxor		$rndkey0,$inout4
	aes${dir}	$rndkey1,$inout2
	pxor		$rndkey0,$inout5
	$movkey		($key,%rax),$rndkey0
	add		\$16,%rax
	jmp		.L${dir}_loop6_enter
.align	16
.L${dir}_loop6:
	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
.L${dir}_loop6_enter:
	aes${dir}	$rndkey1,$inout3
	aes${dir}	$rndkey1,$inout4
	aes${dir}	$rndkey1,$inout5
	$movkey		($key,%rax),$rndkey1
	add		\$32,%rax
	aes${dir}	$rndkey0,$inout0
	aes${dir}	$rndkey0,$inout1
	aes${dir}	$rndkey0,$inout2
	aes${dir}	$rndkey0,$inout3
	aes${dir}	$rndkey0,$inout4
	aes${dir}	$rndkey0,$inout5
	$movkey		-16($key,%rax),$rndkey0
	jnz		.L${dir}_loop6

	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	aes${dir}	$rndkey1,$inout3
	aes${dir}	$rndkey1,$inout4
	aes${dir}	$rndkey1,$inout5
	aes${dir}last	$rndkey0,$inout0
	aes${dir}last	$rndkey0,$inout1
	aes${dir}last	$rndkey0,$inout2
	aes${dir}last	$rndkey0,$inout3
	aes${dir}last	$rndkey0,$inout4
	aes${dir}last	$rndkey0,$inout5
	ret
.cfi_endproc
.size	_aesni_${dir}rypt6,.-_aesni_${dir}rypt6
___
}
sub aesni_generate8 {
my $dir=shift;
# As already mentioned it takes in $key and $rounds, which are *not*
# preserved. $inout[0-7] is cipher/clear text...
$code.=<<___;
.type	_aesni_${dir}rypt8,\@abi-omnipotent
.align	16
_aesni_${dir}rypt8:
.cfi_startproc
	$movkey		($key),$rndkey0
	shl		\$4,$rounds
	$movkey		16($key),$rndkey1
	xorps		$rndkey0,$inout0
	xorps		$rndkey0,$inout1
	pxor		$rndkey0,$inout2
	pxor		$rndkey0,$inout3
	pxor		$rndkey0,$inout4
	lea		32($key,$rounds),$key
	neg		%rax			# $rounds
	aes${dir}	$rndkey1,$inout0
	pxor		$rndkey0,$inout5
	pxor		$rndkey0,$inout6
	aes${dir}	$rndkey1,$inout1
	pxor		$rndkey0,$inout7
	$movkey		($key,%rax),$rndkey0
	add		\$16,%rax
	jmp		.L${dir}_loop8_inner
.align	16
.L${dir}_loop8:
	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
.L${dir}_loop8_inner:
	aes${dir}	$rndkey1,$inout2
	aes${dir}	$rndkey1,$inout3
	aes${dir}	$rndkey1,$inout4
	aes${dir}	$rndkey1,$inout5
	aes${dir}	$rndkey1,$inout6
	aes${dir}	$rndkey1,$inout7
.L${dir}_loop8_enter:
	$movkey		($key,%rax),$rndkey1
	add		\$32,%rax
	aes${dir}	$rndkey0,$inout0
	aes${dir}	$rndkey0,$inout1
	aes${dir}	$rndkey0,$inout2
	aes${dir}	$rndkey0,$inout3
	aes${dir}	$rndkey0,$inout4
	aes${dir}	$rndkey0,$inout5
	aes${dir}	$rndkey0,$inout6
	aes${dir}	$rndkey0,$inout7
	$movkey		-16($key,%rax),$rndkey0
	jnz		.L${dir}_loop8

	aes${dir}	$rndkey1,$inout0
	aes${dir}	$rndkey1,$inout1
	aes${dir}	$rndkey1,$inout2
	aes${dir}	$rndkey1,$inout3
	aes${dir}	$rndkey1,$inout4
	aes${dir}	$rndkey1,$inout5
	aes${dir}	$rndkey1,$inout6
	aes${dir}	$rndkey1,$inout7
	aes${dir}last	$rndkey0,$inout0
	aes${dir}last	$rndkey0,$inout1
	aes${dir}last	$rndkey0,$inout2
	aes${dir}last	$rndkey0,$inout3
	aes${dir}last	$rndkey0,$inout4
	aes${dir}last	$rndkey0,$inout5
	aes${dir}last	$rndkey0,$inout6
	aes${dir}last	$rndkey0,$inout7
	ret
.cfi_endproc
.size	_aesni_${dir}rypt8,.-_aesni_${dir}rypt8
___
}
&aesni_generate2("enc") if ($PREFIX eq "aes_hw");
&aesni_generate3("enc") if ($PREFIX eq "aes_hw");
&aesni_generate4("enc") if ($PREFIX eq "aes_hw");
&aesni_generate6("enc") if ($PREFIX eq "aes_hw");
&aesni_generate8("enc") if ($PREFIX eq "aes_hw");

if ($PREFIX eq "aes_hw") {
{
######################################################################
# void aesni_ctr32_encrypt_blocks (const void *in, void *out,
#                         size_t blocks, const AES_KEY *key,
#                         const char *ivec);
#
# Handles only complete blocks, operates on 32-bit counter and
# does not update *ivec! (see crypto/modes/ctr128.c for details)
#
# Overhaul based on suggestions from Shay Gueron and Vlad Krasnov,
# http://rt.openssl.org/Ticket/Display.html?id=3021&user=guest&pass=guest.
# Keywords are full unroll and modulo-schedule counter calculations
# with zero-round key xor.
{
my ($in0,$in1,$in2,$in3,$in4,$in5)=map("%xmm$_",(10..15));
my ($key0,$ctr)=("%ebp","${ivp}d");
my $frame_size = 0x80 + ($win64?160:0);

$code.=<<___;
.globl	${PREFIX}_ctr32_encrypt_blocks
.type	${PREFIX}_ctr32_encrypt_blocks,\@function,5
.align	16
${PREFIX}_ctr32_encrypt_blocks:
.cfi_startproc
	_CET_ENDBR
#ifdef BORINGSSL_DISPATCH_TEST
	movb \$1,BORINGSSL_function_hit(%rip)
#endif
	cmp	\$1,$len
	jne	.Lctr32_bulk

	# handle single block without allocating stack frame,
	# useful when handling edges
	movups	($ivp),$inout0
	movups	($inp),$inout1
	mov	240($key),%edx			# key->rounds
___
	&aesni_generate1("enc",$key,"%edx");
$code.=<<___;
	 pxor	$rndkey0,$rndkey0		# clear register bank
	 pxor	$rndkey1,$rndkey1
	xorps	$inout1,$inout0
	 pxor	$inout1,$inout1
	movups	$inout0,($out)
	 xorps	$inout0,$inout0
	jmp	.Lctr32_epilogue

.align	16
.Lctr32_bulk:
	lea	(%rsp),$key_			# use $key_ as frame pointer
.cfi_def_cfa_register	$key_
	push	%rbp
.cfi_push	%rbp
	sub	\$$frame_size,%rsp
	and	\$-16,%rsp	# Linux kernel stack can be incorrectly seeded
___
$code.=<<___ if ($win64);
	movaps	%xmm6,-0xa8($key_)		# offload everything
	movaps	%xmm7,-0x98($key_)
	movaps	%xmm8,-0x88($key_)
	movaps	%xmm9,-0x78($key_)
	movaps	%xmm10,-0x68($key_)
	movaps	%xmm11,-0x58($key_)
	movaps	%xmm12,-0x48($key_)
	movaps	%xmm13,-0x38($key_)
	movaps	%xmm14,-0x28($key_)
	movaps	%xmm15,-0x18($key_)
.Lctr32_body:
___
$code.=<<___;

	# 8 16-byte words on top of stack are counter values
	# xor-ed with zero-round key

	movdqu	($ivp),$inout0
	movdqu	($key),$rndkey0
	mov	12($ivp),$ctr			# counter LSB
	pxor	$rndkey0,$inout0
	mov	12($key),$key0			# 0-round key LSB
	movdqa	$inout0,0x00(%rsp)		# populate counter block
	bswap	$ctr
	movdqa	$inout0,$inout1
	movdqa	$inout0,$inout2
	movdqa	$inout0,$inout3
	movdqa	$inout0,0x40(%rsp)
	movdqa	$inout0,0x50(%rsp)
	movdqa	$inout0,0x60(%rsp)
	mov	%rdx,%r10			# about to borrow %rdx
	movdqa	$inout0,0x70(%rsp)

	lea	1($ctr),%rax
	 lea	2($ctr),%rdx
	bswap	%eax
	 bswap	%edx
	xor	$key0,%eax
	 xor	$key0,%edx
	pinsrd	\$3,%eax,$inout1
	lea	3($ctr),%rax
	movdqa	$inout1,0x10(%rsp)
	 pinsrd	\$3,%edx,$inout2
	bswap	%eax
	 mov	%r10,%rdx			# restore %rdx
	 lea	4($ctr),%r10
	 movdqa	$inout2,0x20(%rsp)
	xor	$key0,%eax
	 bswap	%r10d
	pinsrd	\$3,%eax,$inout3
	 xor	$key0,%r10d
	movdqa	$inout3,0x30(%rsp)
	lea	5($ctr),%r9
	 mov	%r10d,0x40+12(%rsp)
	bswap	%r9d
	 lea	6($ctr),%r10
	mov	240($key),$rounds		# key->rounds
	xor	$key0,%r9d
	 bswap	%r10d
	mov	%r9d,0x50+12(%rsp)
	 xor	$key0,%r10d
	lea	7($ctr),%r9
	 mov	%r10d,0x60+12(%rsp)
	bswap	%r9d
	xor	$key0,%r9d
	mov	%r9d,0x70+12(%rsp)

	$movkey	0x10($key),$rndkey1

	movdqa	0x40(%rsp),$inout4
	movdqa	0x50(%rsp),$inout5

	cmp	\$8,$len		# $len is in blocks
	jb	.Lctr32_tail		# short input if ($len<8)

	lea	0x80($key),$key		# size optimization
	sub	\$8,$len		# $len is biased by -8
	jmp	.Lctr32_loop8

.align	32
.Lctr32_loop8:
	 add		\$8,$ctr		# next counter value
	movdqa		0x60(%rsp),$inout6
	aesenc		$rndkey1,$inout0
	 mov		$ctr,%r9d
	movdqa		0x70(%rsp),$inout7
	aesenc		$rndkey1,$inout1
	 bswap		%r9d
	$movkey		0x20-0x80($key),$rndkey0
	aesenc		$rndkey1,$inout2
	 xor		$key0,%r9d
	 nop
	aesenc		$rndkey1,$inout3
	 mov		%r9d,0x00+12(%rsp)	# store next counter value
	 lea		1($ctr),%r9
	aesenc		$rndkey1,$inout4
	aesenc		$rndkey1,$inout5
	aesenc		$rndkey1,$inout6
	aesenc		$rndkey1,$inout7
	$movkey		0x30-0x80($key),$rndkey1
___
for($i=2;$i<8;$i++) {
my $rndkeyx = ($i&1)?$rndkey1:$rndkey0;
$code.=<<___;
	 bswap		%r9d
	aesenc		$rndkeyx,$inout0
	aesenc		$rndkeyx,$inout1
	 xor		$key0,%r9d
	 .byte		0x66,0x90
	aesenc		$rndkeyx,$inout2
	aesenc		$rndkeyx,$inout3
	 mov		%r9d,`0x10*($i-1)`+12(%rsp)
	 lea		$i($ctr),%r9
	aesenc		$rndkeyx,$inout4
	aesenc		$rndkeyx,$inout5
	aesenc		$rndkeyx,$inout6
	aesenc		$rndkeyx,$inout7
	$movkey		`0x20+0x10*$i`-0x80($key),$rndkeyx
___
}
$code.=<<___;
	 bswap		%r9d
	aesenc		$rndkey0,$inout0
	aesenc		$rndkey0,$inout1
	aesenc		$rndkey0,$inout2
	 xor		$key0,%r9d
	 movdqu		0x00($inp),$in0		# start loading input
	aesenc		$rndkey0,$inout3
	 mov		%r9d,0x70+12(%rsp)
	 cmp		\$11,$rounds
	aesenc		$rndkey0,$inout4
	aesenc		$rndkey0,$inout5
	aesenc		$rndkey0,$inout6
	aesenc		$rndkey0,$inout7
	$movkey		0xa0-0x80($key),$rndkey0

	jb		.Lctr32_enc_done

	aesenc		$rndkey1,$inout0
	aesenc		$rndkey1,$inout1
	aesenc		$rndkey1,$inout2
	aesenc		$rndkey1,$inout3
	aesenc		$rndkey1,$inout4
	aesenc		$rndkey1,$inout5
	aesenc		$rndkey1,$inout6
	aesenc		$rndkey1,$inout7
	$movkey		0xb0-0x80($key),$rndkey1

	aesenc		$rndkey0,$inout0
	aesenc		$rndkey0,$inout1
	aesenc		$rndkey0,$inout2
	aesenc		$rndkey0,$inout3
	aesenc		$rndkey0,$inout4
	aesenc		$rndkey0,$inout5
	aesenc		$rndkey0,$inout6
	aesenc		$rndkey0,$inout7
	$movkey		0xc0-0x80($key),$rndkey0
	# 192-bit key support was removed.

	aesenc		$rndkey1,$inout0
	aesenc		$rndkey1,$inout1
	aesenc		$rndkey1,$inout2
	aesenc		$rndkey1,$inout3
	aesenc		$rndkey1,$inout4
	aesenc		$rndkey1,$inout5
	aesenc		$rndkey1,$inout6
	aesenc		$rndkey1,$inout7
	$movkey		0xd0-0x80($key),$rndkey1

	aesenc		$rndkey0,$inout0
	aesenc		$rndkey0,$inout1
	aesenc		$rndkey0,$inout2
	aesenc		$rndkey0,$inout3
	aesenc		$rndkey0,$inout4
	aesenc		$rndkey0,$inout5
	aesenc		$rndkey0,$inout6
	aesenc		$rndkey0,$inout7
	$movkey		0xe0-0x80($key),$rndkey0
	jmp		.Lctr32_enc_done

.align	16
.Lctr32_enc_done:
	movdqu		0x10($inp),$in1
	pxor		$rndkey0,$in0		# input^=round[last]
	movdqu		0x20($inp),$in2
	pxor		$rndkey0,$in1
	movdqu		0x30($inp),$in3
	pxor		$rndkey0,$in2
	movdqu		0x40($inp),$in4
	pxor		$rndkey0,$in3
	movdqu		0x50($inp),$in5
	pxor		$rndkey0,$in4
	prefetcht0	0x1c0($inp)	# We process 128 bytes (8*16), so to prefetch 1 iteration
	prefetcht0	0x200($inp)	# We need to prefetch 2 64 byte lines
	pxor		$rndkey0,$in5
	aesenc		$rndkey1,$inout0
	aesenc		$rndkey1,$inout1
	aesenc		$rndkey1,$inout2
	aesenc		$rndkey1,$inout3
	aesenc		$rndkey1,$inout4
	aesenc		$rndkey1,$inout5
	aesenc		$rndkey1,$inout6
	aesenc		$rndkey1,$inout7
	movdqu		0x60($inp),$rndkey1	# borrow $rndkey1 for inp[6]
	lea		0x80($inp),$inp		# $inp+=8*16

	aesenclast	$in0,$inout0		# $inN is inp[N]^round[last]
	pxor		$rndkey0,$rndkey1	# borrowed $rndkey
	movdqu		0x70-0x80($inp),$in0
	aesenclast	$in1,$inout1
	pxor		$rndkey0,$in0
	movdqa		0x00(%rsp),$in1		# load next counter block
	aesenclast	$in2,$inout2
	aesenclast	$in3,$inout3
	movdqa		0x10(%rsp),$in2
	movdqa		0x20(%rsp),$in3
	aesenclast	$in4,$inout4
	aesenclast	$in5,$inout5
	movdqa		0x30(%rsp),$in4
	movdqa		0x40(%rsp),$in5
	aesenclast	$rndkey1,$inout6
	movdqa		0x50(%rsp),$rndkey0
	$movkey		0x10-0x80($key),$rndkey1#real 1st-round key
	aesenclast	$in0,$inout7

	movups		$inout0,($out)		# store 8 output blocks
	movdqa		$in1,$inout0
	movups		$inout1,0x10($out)
	movdqa		$in2,$inout1
	movups		$inout2,0x20($out)
	movdqa		$in3,$inout2
	movups		$inout3,0x30($out)
	movdqa		$in4,$inout3
	movups		$inout4,0x40($out)
	movdqa		$in5,$inout4
	movups		$inout5,0x50($out)
	movdqa		$rndkey0,$inout5
	movups		$inout6,0x60($out)
	movups		$inout7,0x70($out)
	lea		0x80($out),$out		# $out+=8*16

	sub	\$8,$len
	jnc	.Lctr32_loop8			# loop if $len-=8 didn't borrow

	add	\$8,$len			# restore real remaining $len
	jz	.Lctr32_done			# done if ($len==0)
	lea	-0x80($key),$key

.Lctr32_tail:
	# note that at this point $inout0..5 are populated with
	# counter values xor-ed with 0-round key
	lea	16($key),$key
	cmp	\$4,$len
	jb	.Lctr32_loop3
	je	.Lctr32_loop4

	# if ($len>4) compute 7 E(counter)
	shl		\$4,$rounds
	movdqa		0x60(%rsp),$inout6
	pxor		$inout7,$inout7

	$movkey		16($key),$rndkey0
	aesenc		$rndkey1,$inout0
	aesenc		$rndkey1,$inout1
	lea		32-16($key,$rounds),$key# prepare for .Lenc_loop8_enter
	neg		%rax
	aesenc		$rndkey1,$inout2
	add		\$16,%rax		# prepare for .Lenc_loop8_enter
	 movups		($inp),$in0
	aesenc		$rndkey1,$inout3
	aesenc		$rndkey1,$inout4
	 movups		0x10($inp),$in1		# pre-load input
	 movups		0x20($inp),$in2
	aesenc		$rndkey1,$inout5
	aesenc		$rndkey1,$inout6

	call            .Lenc_loop8_enter

	movdqu	0x30($inp),$in3
	pxor	$in0,$inout0
	movdqu	0x40($inp),$in0
	pxor	$in1,$inout1
	movdqu	$inout0,($out)			# store output
	pxor	$in2,$inout2
	movdqu	$inout1,0x10($out)
	pxor	$in3,$inout3
	movdqu	$inout2,0x20($out)
	pxor	$in0,$inout4
	movdqu	$inout3,0x30($out)
	movdqu	$inout4,0x40($out)
	cmp	\$6,$len
	jb	.Lctr32_done			# $len was 5, stop store

	movups	0x50($inp),$in1
	xorps	$in1,$inout5
	movups	$inout5,0x50($out)
	je	.Lctr32_done			# $len was 6, stop store

	movups	0x60($inp),$in2
	xorps	$in2,$inout6
	movups	$inout6,0x60($out)
	jmp	.Lctr32_done			# $len was 7, stop store

.align	32
.Lctr32_loop4:
	aesenc		$rndkey1,$inout0
	lea		16($key),$key
	dec		$rounds
	aesenc		$rndkey1,$inout1
	aesenc		$rndkey1,$inout2
	aesenc		$rndkey1,$inout3
	$movkey		($key),$rndkey1
	jnz		.Lctr32_loop4
	aesenclast	$rndkey1,$inout0
	aesenclast	$rndkey1,$inout1
	 movups		($inp),$in0		# load input
	 movups		0x10($inp),$in1
	aesenclast	$rndkey1,$inout2
	aesenclast	$rndkey1,$inout3
	 movups		0x20($inp),$in2
	 movups		0x30($inp),$in3

	xorps	$in0,$inout0
	movups	$inout0,($out)			# store output
	xorps	$in1,$inout1
	movups	$inout1,0x10($out)
	pxor	$in2,$inout2
	movdqu	$inout2,0x20($out)
	pxor	$in3,$inout3
	movdqu	$inout3,0x30($out)
	jmp	.Lctr32_done			# $len was 4, stop store

.align	32
.Lctr32_loop3:
	aesenc		$rndkey1,$inout0
	lea		16($key),$key
	dec		$rounds
	aesenc		$rndkey1,$inout1
	aesenc		$rndkey1,$inout2
	$movkey		($key),$rndkey1
	jnz		.Lctr32_loop3
	aesenclast	$rndkey1,$inout0
	aesenclast	$rndkey1,$inout1
	aesenclast	$rndkey1,$inout2

	movups	($inp),$in0			# load input
	xorps	$in0,$inout0
	movups	$inout0,($out)			# store output
	cmp	\$2,$len
	jb	.Lctr32_done			# $len was 1, stop store

	movups	0x10($inp),$in1
	xorps	$in1,$inout1
	movups	$inout1,0x10($out)
	je	.Lctr32_done			# $len was 2, stop store

	movups	0x20($inp),$in2
	xorps	$in2,$inout2
	movups	$inout2,0x20($out)		# $len was 3, stop store

.Lctr32_done:
	xorps	%xmm0,%xmm0			# clear register bank
	xor	$key0,$key0
	pxor	%xmm1,%xmm1
	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
___
$code.=<<___ if (!$win64);
	pxor	%xmm6,%xmm6
	pxor	%xmm7,%xmm7
	movaps	%xmm0,0x00(%rsp)		# clear stack
	pxor	%xmm8,%xmm8
	movaps	%xmm0,0x10(%rsp)
	pxor	%xmm9,%xmm9
	movaps	%xmm0,0x20(%rsp)
	pxor	%xmm10,%xmm10
	movaps	%xmm0,0x30(%rsp)
	pxor	%xmm11,%xmm11
	movaps	%xmm0,0x40(%rsp)
	pxor	%xmm12,%xmm12
	movaps	%xmm0,0x50(%rsp)
	pxor	%xmm13,%xmm13
	movaps	%xmm0,0x60(%rsp)
	pxor	%xmm14,%xmm14
	movaps	%xmm0,0x70(%rsp)
	pxor	%xmm15,%xmm15
___
$code.=<<___ if ($win64);
	movaps	-0xa8($key_),%xmm6
	movaps	%xmm0,-0xa8($key_)		# clear stack
	movaps	-0x98($key_),%xmm7
	movaps	%xmm0,-0x98($key_)
	movaps	-0x88($key_),%xmm8
	movaps	%xmm0,-0x88($key_)
	movaps	-0x78($key_),%xmm9
	movaps	%xmm0,-0x78($key_)
	movaps	-0x68($key_),%xmm10
	movaps	%xmm0,-0x68($key_)
	movaps	-0x58($key_),%xmm11
	movaps	%xmm0,-0x58($key_)
	movaps	-0x48($key_),%xmm12
	movaps	%xmm0,-0x48($key_)
	movaps	-0x38($key_),%xmm13
	movaps	%xmm0,-0x38($key_)
	movaps	-0x28($key_),%xmm14
	movaps	%xmm0,-0x28($key_)
	movaps	-0x18($key_),%xmm15
	movaps	%xmm0,-0x18($key_)
	movaps	%xmm0,0x00(%rsp)
	movaps	%xmm0,0x10(%rsp)
	movaps	%xmm0,0x20(%rsp)
	movaps	%xmm0,0x30(%rsp)
	movaps	%xmm0,0x40(%rsp)
	movaps	%xmm0,0x50(%rsp)
	movaps	%xmm0,0x60(%rsp)
	movaps	%xmm0,0x70(%rsp)
___
$code.=<<___;
	mov	-8($key_),%rbp
.cfi_restore	%rbp
	lea	($key_),%rsp
.cfi_def_cfa_register	%rsp
.Lctr32_epilogue:
	ret
.cfi_endproc
.size	${PREFIX}_ctr32_encrypt_blocks,.-${PREFIX}_ctr32_encrypt_blocks
___
} }}

{ my ($inp,$bits,$key) = @_4args;
  $bits =~ s/%r/%e/;
# This is based on submission from Intel by
#	Huang Ying
#	Vinodh Gopal
#	Kahraman Akdemir
#
# Aggressively optimized in respect to aeskeygenassist's critical path
# and is contained in %xmm0-5 to meet Win64 ABI requirement.
#
# int ${PREFIX}_set_encrypt_key(const unsigned char *inp,
#				int bits, AES_KEY * const key);
#
# input:	$inp	user-supplied key
#		$bits	$inp length in bits
#		$key	pointer to key schedule
# output:	%eax	0 denoting success, -1 or -2 - failure (see C)
#		$bits	rounds-1 (used in aesni_set_decrypt_key)
#		*$key	key schedule
#		$key	pointer to key schedule (used in
#			aesni_set_decrypt_key)
#
# Subroutine is frame-less, which means that only volatile registers
# are used. Note that it's declared "abi-omnipotent", which means that
# amount of volatile registers is smaller on Windows.
#
# There are two variants of this function, one which uses aeskeygenassist
# ("base") and one which uses aesenclast + pshufb ("alt"). See aes/internal.h
# for details.
$code.=<<___;
.globl	${PREFIX}_set_encrypt_key_base
.type	${PREFIX}_set_encrypt_key_base,\@abi-omnipotent
.align	16
${PREFIX}_set_encrypt_key_base:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
#ifdef BORINGSSL_DISPATCH_TEST
	movb \$1,BORINGSSL_function_hit+3(%rip)
#endif
	sub	\$8,%rsp
.cfi_adjust_cfa_offset	8
.seh_stackalloc	8
.seh_endprologue
	movups	($inp),%xmm0		# pull first 128 bits of *userKey
	xorps	%xmm4,%xmm4		# low dword of xmm4 is assumed 0
	lea	16($key),%rax		# %rax is used as modifiable copy of $key
	cmp	\$256,$bits
	je	.L14rounds
	# 192-bit key support was removed.

	cmp	\$128,$bits
	jne	.Lbad_keybits

.L10rounds:
	mov	\$9,$bits			# 10 rounds for 128-bit key

	$movkey	%xmm0,($key)			# round 0
	aeskeygenassist	\$0x1,%xmm0,%xmm1	# round 1
	call		.Lkey_expansion_128_cold
	aeskeygenassist	\$0x2,%xmm0,%xmm1	# round 2
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x4,%xmm0,%xmm1	# round 3
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x8,%xmm0,%xmm1	# round 4
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x10,%xmm0,%xmm1	# round 5
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x20,%xmm0,%xmm1	# round 6
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x40,%xmm0,%xmm1	# round 7
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x80,%xmm0,%xmm1	# round 8
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x1b,%xmm0,%xmm1	# round 9
	call		.Lkey_expansion_128
	aeskeygenassist	\$0x36,%xmm0,%xmm1	# round 10
	call		.Lkey_expansion_128
	$movkey	%xmm0,(%rax)
	mov	$bits,80(%rax)	# 240(%rdx)
	xor	%eax,%eax
	jmp	.Lenc_key_ret

	# 192-bit key support was removed.

.align	16
.L14rounds:
	movups	16($inp),%xmm2			# remaining half of *userKey
	mov	\$13,$bits			# 14 rounds for 256
	lea	16(%rax),%rax

	$movkey	%xmm0,($key)			# round 0
	$movkey	%xmm2,16($key)			# round 1
	aeskeygenassist	\$0x1,%xmm2,%xmm1	# round 2
	call		.Lkey_expansion_256a_cold
	aeskeygenassist	\$0x1,%xmm0,%xmm1	# round 3
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x2,%xmm2,%xmm1	# round 4
	call		.Lkey_expansion_256a
	aeskeygenassist	\$0x2,%xmm0,%xmm1	# round 5
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x4,%xmm2,%xmm1	# round 6
	call		.Lkey_expansion_256a
	aeskeygenassist	\$0x4,%xmm0,%xmm1	# round 7
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x8,%xmm2,%xmm1	# round 8
	call		.Lkey_expansion_256a
	aeskeygenassist	\$0x8,%xmm0,%xmm1	# round 9
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x10,%xmm2,%xmm1	# round 10
	call		.Lkey_expansion_256a
	aeskeygenassist	\$0x10,%xmm0,%xmm1	# round 11
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x20,%xmm2,%xmm1	# round 12
	call		.Lkey_expansion_256a
	aeskeygenassist	\$0x20,%xmm0,%xmm1	# round 13
	call		.Lkey_expansion_256b
	aeskeygenassist	\$0x40,%xmm2,%xmm1	# round 14
	call		.Lkey_expansion_256a
	$movkey	%xmm0,(%rax)
	mov	$bits,16(%rax)	# 240(%rdx)
	xor	%rax,%rax
	jmp	.Lenc_key_ret

.align	16
.Lbad_keybits:
	mov	\$-2,%rax
.Lenc_key_ret:
	pxor	%xmm0,%xmm0
	pxor	%xmm1,%xmm1
	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
	add	\$8,%rsp
.cfi_adjust_cfa_offset	-8
	ret
.cfi_endproc
.seh_endproc

.align	16
.Lkey_expansion_128:
.cfi_startproc
	$movkey	%xmm0,(%rax)
	lea	16(%rax),%rax
.Lkey_expansion_128_cold:
	shufps	\$0b00010000,%xmm0,%xmm4
	xorps	%xmm4, %xmm0
	shufps	\$0b10001100,%xmm0,%xmm4
	xorps	%xmm4, %xmm0
	shufps	\$0b11111111,%xmm1,%xmm1	# critical path
	xorps	%xmm1,%xmm0
	ret
.cfi_endproc

.align	16
.Lkey_expansion_256a:
.cfi_startproc
	$movkey	%xmm2,(%rax)
	lea	16(%rax),%rax
.Lkey_expansion_256a_cold:
	shufps	\$0b00010000,%xmm0,%xmm4
	xorps	%xmm4,%xmm0
	shufps	\$0b10001100,%xmm0,%xmm4
	xorps	%xmm4,%xmm0
	shufps	\$0b11111111,%xmm1,%xmm1	# critical path
	xorps	%xmm1,%xmm0
	ret
.cfi_endproc

.align 16
.Lkey_expansion_256b:
.cfi_startproc
	$movkey	%xmm0,(%rax)
	lea	16(%rax),%rax

	shufps	\$0b00010000,%xmm2,%xmm4
	xorps	%xmm4,%xmm2
	shufps	\$0b10001100,%xmm2,%xmm4
	xorps	%xmm4,%xmm2
	shufps	\$0b10101010,%xmm1,%xmm1	# critical path
	xorps	%xmm1,%xmm2
	ret
.cfi_endproc
.size	${PREFIX}_set_encrypt_key_base,.-${PREFIX}_set_encrypt_key_base

.globl	${PREFIX}_set_encrypt_key_alt
.type	${PREFIX}_set_encrypt_key_alt,\@abi-omnipotent
.align	16
${PREFIX}_set_encrypt_key_alt:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
#ifdef BORINGSSL_DISPATCH_TEST
	movb \$1,BORINGSSL_function_hit+3(%rip)
#endif
	sub	\$8,%rsp
.cfi_adjust_cfa_offset	8
.seh_stackalloc	8
.seh_endprologue
	movups	($inp),%xmm0		# pull first 128 bits of *userKey
	xorps	%xmm4,%xmm4		# low dword of xmm4 is assumed 0
	lea	16($key),%rax		# %rax is used as modifiable copy of $key
	cmp	\$256,$bits
	je	.L14rounds_alt
	# 192-bit key support was removed.
	cmp	\$128,$bits
	jne	.Lbad_keybits_alt

	mov	\$9,$bits			# 10 rounds for 128-bit key
	movdqa	.Lkey_rotate(%rip),%xmm5
	mov	\$8,%r10d
	movdqa	.Lkey_rcon1(%rip),%xmm4
	movdqa	%xmm0,%xmm2
	movdqu	%xmm0,($key)
	jmp	.Loop_key128

.align	16
.Loop_key128:
	pshufb		%xmm5,%xmm0
	aesenclast	%xmm4,%xmm0
	pslld		\$1,%xmm4
	lea		16(%rax),%rax

	movdqa		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm3,%xmm2

	pxor		%xmm2,%xmm0
	movdqu		%xmm0,-16(%rax)
	movdqa		%xmm0,%xmm2

	dec	%r10d
	jnz	.Loop_key128

	movdqa		.Lkey_rcon1b(%rip),%xmm4

	pshufb		%xmm5,%xmm0
	aesenclast	%xmm4,%xmm0
	pslld		\$1,%xmm4

	movdqa		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm3,%xmm2

	pxor		%xmm2,%xmm0
	movdqu		%xmm0,(%rax)

	movdqa		%xmm0,%xmm2
	pshufb		%xmm5,%xmm0
	aesenclast	%xmm4,%xmm0

	movdqa		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm2,%xmm3
	pslldq		\$4,%xmm2
	pxor		%xmm3,%xmm2

	pxor		%xmm2,%xmm0
	movdqu		%xmm0,16(%rax)

	mov	$bits,96(%rax)	# 240($key)
	xor	%eax,%eax
	jmp	.Lenc_key_ret_alt

	# 192-bit key support was removed.

.align	16
.L14rounds_alt:
	movups	16($inp),%xmm2			# remaining half of *userKey
	mov	\$13,$bits			# 14 rounds for 256
	lea	16(%rax),%rax
	movdqa	.Lkey_rotate(%rip),%xmm5
	movdqa	.Lkey_rcon1(%rip),%xmm4
	mov	\$7,%r10d
	movdqu	%xmm0,0($key)
	movdqa	%xmm2,%xmm1
	movdqu	%xmm2,16($key)
	jmp	.Loop_key256

.align	16
.Loop_key256:
	pshufb		%xmm5,%xmm2
	aesenclast	%xmm4,%xmm2

	movdqa		%xmm0,%xmm3
	pslldq		\$4,%xmm0
	pxor		%xmm0,%xmm3
	pslldq		\$4,%xmm0
	pxor		%xmm0,%xmm3
	pslldq		\$4,%xmm0
	pxor		%xmm3,%xmm0
	pslld		\$1,%xmm4

	pxor		%xmm2,%xmm0
	movdqu		%xmm0,(%rax)

	dec	%r10d
	jz	.Ldone_key256

	pshufd		\$0xff,%xmm0,%xmm2
	pxor		%xmm3,%xmm3
	aesenclast	%xmm3,%xmm2

	movdqa		%xmm1,%xmm3
	pslldq		\$4,%xmm1
	pxor		%xmm1,%xmm3
	pslldq		\$4,%xmm1
	pxor		%xmm1,%xmm3
	pslldq		\$4,%xmm1
	pxor		%xmm3,%xmm1

	pxor		%xmm1,%xmm2
	movdqu		%xmm2,16(%rax)
	lea		32(%rax),%rax
	movdqa		%xmm2,%xmm1

	jmp	.Loop_key256

.Ldone_key256:
	mov	$bits,16(%rax)	# 240($key)
	xor	%eax,%eax
	jmp	.Lenc_key_ret_alt

.align	16
.Lbad_keybits_alt:
	mov	\$-2,%rax
.Lenc_key_ret_alt:
	pxor	%xmm0,%xmm0
	pxor	%xmm1,%xmm1
	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
	add	\$8,%rsp
.cfi_adjust_cfa_offset	-8
	ret
.cfi_endproc
.seh_endproc
.size	${PREFIX}_set_encrypt_key_alt,.-${PREFIX}_set_encrypt_key_alt
___
}

$code.=<<___;
.section .rodata
.align	64
.Lbswap_mask:
	.byte	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
.Lincrement32:
	.long	6,6,6,0
.Lincrement64:
	.long	1,0,0,0
.Lincrement1:
	.byte	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1
.Lkey_rotate:
	.long	0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d
.Lkey_rotate192:
	.long	0x04070605,0x04070605,0x04070605,0x04070605
.Lkey_rcon1:
	.long	1,1,1,1
.Lkey_rcon1b:
	.long	0x1b,0x1b,0x1b,0x1b

.asciz  "AES for Intel AES-NI, CRYPTOGAMS by <appro\@openssl.org>"
.align	64
.text
___

# EXCEPTION_DISPOSITION handler (EXCEPTION_RECORD *rec,ULONG64 frame,
#		CONTEXT *context,DISPATCHER_CONTEXT *disp)
if ($win64) {
$rec="%rcx";
$frame="%rdx";
$context="%r8";
$disp="%r9";

$code.=<<___;
.extern	__imp_RtlVirtualUnwind
___
$code.=<<___ if ($PREFIX eq "aes_hw");
.type	ctr_xts_se_handler,\@abi-omnipotent
.align	16
ctr_xts_se_handler:
	push	%rsi
	push	%rdi
	push	%rbx
	push	%rbp
	push	%r12
	push	%r13
	push	%r14
	push	%r15
	pushfq
	sub	\$64,%rsp

	mov	120($context),%rax	# pull context->Rax
	mov	248($context),%rbx	# pull context->Rip

	mov	8($disp),%rsi		# disp->ImageBase
	mov	56($disp),%r11		# disp->HandlerData

	mov	0(%r11),%r10d		# HandlerData[0]
	lea	(%rsi,%r10),%r10	# prologue lable
	cmp	%r10,%rbx		# context->Rip<prologue label
	jb	.Lcommon_seh_tail

	mov	152($context),%rax	# pull context->Rsp

	mov	4(%r11),%r10d		# HandlerData[1]
	lea	(%rsi,%r10),%r10	# epilogue label
	cmp	%r10,%rbx		# context->Rip>=epilogue label
	jae	.Lcommon_seh_tail

	mov	208($context),%rax	# pull context->R11

	lea	-0xa8(%rax),%rsi	# %xmm save area
	lea	512($context),%rdi	# & context.Xmm6
	mov	\$20,%ecx		# 10*sizeof(%xmm0)/sizeof(%rax)
	.long	0xa548f3fc		# cld; rep movsq

	mov	-8(%rax),%rbp		# restore saved %rbp
	mov	%rbp,160($context)	# restore context->Rbp


.Lcommon_seh_tail:
	mov	8(%rax),%rdi
	mov	16(%rax),%rsi
	mov	%rax,152($context)	# restore context->Rsp
	mov	%rsi,168($context)	# restore context->Rsi
	mov	%rdi,176($context)	# restore context->Rdi

	mov	40($disp),%rdi		# disp->ContextRecord
	mov	$context,%rsi		# context
	mov	\$154,%ecx		# sizeof(CONTEXT)
	.long	0xa548f3fc		# cld; rep movsq

	mov	$disp,%rsi
	xor	%rcx,%rcx		# arg1, UNW_FLAG_NHANDLER
	mov	8(%rsi),%rdx		# arg2, disp->ImageBase
	mov	0(%rsi),%r8		# arg3, disp->ControlPc
	mov	16(%rsi),%r9		# arg4, disp->FunctionEntry
	mov	40(%rsi),%r10		# disp->ContextRecord
	lea	56(%rsi),%r11		# &disp->HandlerData
	lea	24(%rsi),%r12		# &disp->EstablisherFrame
	mov	%r10,32(%rsp)		# arg5
	mov	%r11,40(%rsp)		# arg6
	mov	%r12,48(%rsp)		# arg7
	mov	%rcx,56(%rsp)		# arg8, (NULL)
	call	*__imp_RtlVirtualUnwind(%rip)

	mov	\$1,%eax		# ExceptionContinueSearch
	add	\$64,%rsp
	popfq
	pop	%r15
	pop	%r14
	pop	%r13
	pop	%r12
	pop	%rbp
	pop	%rbx
	pop	%rdi
	pop	%rsi
	ret
.size	ctr_xts_se_handler,.-ctr_xts_se_handler

.section	.pdata
.align	4
___
$code.=<<___ if ($PREFIX eq "aes_hw");
	.rva	.LSEH_begin_${PREFIX}_ctr32_encrypt_blocks
	.rva	.LSEH_end_${PREFIX}_ctr32_encrypt_blocks
	.rva	.LSEH_info_ctr32
___
$code.=<<___;
.section	.xdata
.align	8
___
$code.=<<___ if ($PREFIX eq "aes_hw");
.LSEH_info_ctr32:
	.byte	9,0,0,0
	.rva	ctr_xts_se_handler
	.rva	.Lctr32_body,.Lctr32_epilogue		# HandlerData[]
___
}

sub rex {
  local *opcode=shift;
  my ($dst,$src)=@_;
  my $rex=0;

    $rex|=0x04			if($dst>=8);
    $rex|=0x01			if($src>=8);
    push @opcode,$rex|0x40	if($rex);
}

sub aesni {
  my $line=shift;
  my @opcode=(0x66);

    if ($line=~/(aeskeygenassist)\s+\$([x0-9a-f]+),\s*%xmm([0-9]+),\s*%xmm([0-9]+)/) {
	rex(\@opcode,$4,$3);
	push @opcode,0x0f,0x3a,0xdf;
	push @opcode,0xc0|($3&7)|(($4&7)<<3);	# ModR/M
	my $c=$2;
	push @opcode,$c=~/^0/?oct($c):$c;
	return ".byte\t".join(',',@opcode);
    }
    elsif ($line=~/(aes[a-z]+)\s+%xmm([0-9]+),\s*%xmm([0-9]+)/) {
	my %opcodelet = (
		"aesimc" => 0xdb,
		"aesenc" => 0xdc,	"aesenclast" => 0xdd,
		"aesdec" => 0xde,	"aesdeclast" => 0xdf
	);
	return undef if (!defined($opcodelet{$1}));
	rex(\@opcode,$3,$2);
	push @opcode,0x0f,0x38,$opcodelet{$1};
	push @opcode,0xc0|($2&7)|(($3&7)<<3);	# ModR/M
	return ".byte\t".join(',',@opcode);
    }
    elsif ($line=~/(aes[a-z]+)\s+([0x1-9a-fA-F]*)\(%rsp\),\s*%xmm([0-9]+)/) {
	my %opcodelet = (
		"aesenc" => 0xdc,	"aesenclast" => 0xdd,
		"aesdec" => 0xde,	"aesdeclast" => 0xdf
	);
	return undef if (!defined($opcodelet{$1}));
	my $off = $2;
	push @opcode,0x44 if ($3>=8);
	push @opcode,0x0f,0x38,$opcodelet{$1};
	push @opcode,0x44|(($3&7)<<3),0x24;	# ModR/M
	push @opcode,($off=~/^0/?oct($off):$off)&0xff;
	return ".byte\t".join(',',@opcode);
    }
    return $line;
}

sub movbe {
	".byte	0x0f,0x38,0xf1,0x44,0x24,".shift;
}

$code =~ s/\`([^\`]*)\`/eval($1)/gem;
$code =~ s/\b(aes.*%xmm[0-9]+).*$/aesni($1)/gem;
#$code =~ s/\bmovbe\s+%eax/bswap %eax; mov %eax/gm;	# debugging artefact
$code =~ s/\bmovbe\s+%eax,\s*([0-9]+)\(%rsp\)/movbe($1)/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
