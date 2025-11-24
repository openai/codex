#! /usr/bin/env perl
# Copyright 2010-2016 The OpenSSL Project Authors. All Rights Reserved.
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
# March, May, June 2010
#
# The module implements "4-bit" GCM GHASH function and underlying
# single multiplication operation in GF(2^128). "4-bit" means that it
# uses 256 bytes per-key table [+64/128 bytes fixed table]. It has two
# code paths: vanilla x86 and vanilla SSE. Former will be executed on
# 486 and Pentium, latter on all others. SSE GHASH features so called
# "528B" variant of "4-bit" method utilizing additional 256+16 bytes
# of per-key storage [+512 bytes shared table]. Performance results
# are for streamed GHASH subroutine and are expressed in cycles per
# processed byte, less is better:
#
#		gcc 2.95.3(*)	SSE assembler	x86 assembler
#
# Pentium	105/111(**)	-		50
# PIII		68 /75		12.2		24
# P4		125/125		17.8		84(***)
# Opteron	66 /70		10.1		30
# Core2		54 /67		8.4		18
# Atom		105/105		16.8		53
# VIA Nano	69 /71		13.0		27
#
# (*)	gcc 3.4.x was observed to generate few percent slower code,
#	which is one of reasons why 2.95.3 results were chosen,
#	another reason is lack of 3.4.x results for older CPUs;
#	comparison with SSE results is not completely fair, because C
#	results are for vanilla "256B" implementation, while
#	assembler results are for "528B";-)
# (**)	second number is result for code compiled with -fPIC flag,
#	which is actually more relevant, because assembler code is
#	position-independent;
# (***)	see comment in non-MMX routine for further details;
#
# To summarize, it's >2-5 times faster than gcc-generated code. To
# anchor it to something else SHA1 assembler processes one byte in
# ~7 cycles on contemporary x86 cores. As for choice of MMX/SSE
# in particular, see comment at the end of the file...

# May 2010
#
# Add PCLMULQDQ version performing at 2.10 cycles per processed byte.
# The question is how close is it to theoretical limit? The pclmulqdq
# instruction latency appears to be 14 cycles and there can't be more
# than 2 of them executing at any given time. This means that single
# Karatsuba multiplication would take 28 cycles *plus* few cycles for
# pre- and post-processing. Then multiplication has to be followed by
# modulo-reduction. Given that aggregated reduction method [see
# "Carry-less Multiplication and Its Usage for Computing the GCM Mode"
# white paper by Intel] allows you to perform reduction only once in
# a while we can assume that asymptotic performance can be estimated
# as (28+Tmod/Naggr)/16, where Tmod is time to perform reduction
# and Naggr is the aggregation factor.
#
# Before we proceed to this implementation let's have closer look at
# the best-performing code suggested by Intel in their white paper.
# By tracing inter-register dependencies Tmod is estimated as ~19
# cycles and Naggr chosen by Intel is 4, resulting in 2.05 cycles per
# processed byte. As implied, this is quite optimistic estimate,
# because it does not account for Karatsuba pre- and post-processing,
# which for a single multiplication is ~5 cycles. Unfortunately Intel
# does not provide performance data for GHASH alone. But benchmarking
# AES_GCM_encrypt ripped out of Fig. 15 of the white paper with aadt
# alone resulted in 2.46 cycles per byte of out 16KB buffer. Note that
# the result accounts even for pre-computing of degrees of the hash
# key H, but its portion is negligible at 16KB buffer size.
#
# Moving on to the implementation in question. Tmod is estimated as
# ~13 cycles and Naggr is 2, giving asymptotic performance of ...
# 2.16. How is it possible that measured performance is better than
# optimistic theoretical estimate? There is one thing Intel failed
# to recognize. By serializing GHASH with CTR in same subroutine
# former's performance is really limited to above (Tmul + Tmod/Naggr)
# equation. But if GHASH procedure is detached, the modulo-reduction
# can be interleaved with Naggr-1 multiplications at instruction level
# and under ideal conditions even disappear from the equation. So that
# optimistic theoretical estimate for this implementation is ...
# 28/16=1.75, and not 2.16. Well, it's probably way too optimistic,
# at least for such small Naggr. I'd argue that (28+Tproc/Naggr),
# where Tproc is time required for Karatsuba pre- and post-processing,
# is more realistic estimate. In this case it gives ... 1.91 cycles.
# Or in other words, depending on how well we can interleave reduction
# and one of the two multiplications the performance should be between
# 1.91 and 2.16. As already mentioned, this implementation processes
# one byte out of 8KB buffer in 2.10 cycles, while x86_64 counterpart
# - in 2.02. x86_64 performance is better, because larger register
# bank allows to interleave reduction and multiplication better.
#
# Does it make sense to increase Naggr? To start with it's virtually
# impossible in 32-bit mode, because of limited register bank
# capacity. Otherwise improvement has to be weighed against slower
# setup, as well as code size and complexity increase. As even
# optimistic estimate doesn't promise 30% performance improvement,
# there are currently no plans to increase Naggr.
#
# Special thanks to David Woodhouse for providing access to a
# Westmere-based system on behalf of Intel Open Source Technology Centre.

# January 2010
#
# Tweaked to optimize transitions between integer and FP operations
# on same XMM register, PCLMULQDQ subroutine was measured to process
# one byte in 2.07 cycles on Sandy Bridge, and in 2.12 - on Westmere.
# The minor regression on Westmere is outweighed by ~15% improvement
# on Sandy Bridge. Strangely enough attempt to modify 64-bit code in
# similar manner resulted in almost 20% degradation on Sandy Bridge,
# where original 64-bit code processes one byte in 1.95 cycles.

#####################################################################
# For reference, AMD Bulldozer processes one byte in 1.98 cycles in
# 32-bit mode and 1.89 in 64-bit.

# February 2013
#
# Overhaul: aggregate Karatsuba post-processing, improve ILP in
# reduction_alg9. Resulting performance is 1.96 cycles per byte on
# Westmere, 1.95 - on Sandy/Ivy Bridge, 1.76 - on Bulldozer.

# This file was patched in BoringSSL to remove the variable-time 4-bit
# implementation.

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC,"${dir}","${dir}../../../perlasm");
require "x86asm.pl";

$output=pop;
open STDOUT,">$output";

&asm_init($ARGV[0]);

$x86only=0;
$sse2=1;

if (!$x86only) {{{
if ($sse2) {{
######################################################################
# PCLMULQDQ version.

$Xip="eax";
$Htbl="edx";
$const="ecx";
$inp="esi";
$len="ebx";

($Xi,$Xhi)=("xmm0","xmm1");	$Hkey="xmm2";
($T1,$T2,$T3)=("xmm3","xmm4","xmm5");
($Xn,$Xhn)=("xmm6","xmm7");

&static_label("bswap");

sub clmul64x64_T2 {	# minimal "register" pressure
my ($Xhi,$Xi,$Hkey,$HK)=@_;

	&movdqa		($Xhi,$Xi);		#
	&pshufd		($T1,$Xi,0b01001110);
	&pshufd		($T2,$Hkey,0b01001110)	if (!defined($HK));
	&pxor		($T1,$Xi);		#
	&pxor		($T2,$Hkey)		if (!defined($HK));
			$HK=$T2			if (!defined($HK));

	&pclmulqdq	($Xi,$Hkey,0x00);	#######
	&pclmulqdq	($Xhi,$Hkey,0x11);	#######
	&pclmulqdq	($T1,$HK,0x00);		#######
	&xorps		($T1,$Xi);		#
	&xorps		($T1,$Xhi);		#

	&movdqa		($T2,$T1);		#
	&psrldq		($T1,8);
	&pslldq		($T2,8);		#
	&pxor		($Xhi,$T1);
	&pxor		($Xi,$T2);		#
}


if (1) {		# Algorithm 9 with <<1 twist.
			# Reduction is shorter and uses only two
			# temporary registers, which makes it better
			# candidate for interleaving with 64x64
			# multiplication. Pre-modulo-scheduled loop
			# was found to be ~20% faster than Algorithm 5
			# below. Algorithm 9 was therefore chosen for
			# further optimization...

sub reduction_alg9 {	# 17/11 times faster than Intel version
my ($Xhi,$Xi) = @_;

	# 1st phase
	&movdqa		($T2,$Xi);		#
	&movdqa		($T1,$Xi);
	&psllq		($Xi,5);
	&pxor		($T1,$Xi);		#
	&psllq		($Xi,1);
	&pxor		($Xi,$T1);		#
	&psllq		($Xi,57);		#
	&movdqa		($T1,$Xi);		#
	&pslldq		($Xi,8);
	&psrldq		($T1,8);		#
	&pxor		($Xi,$T2);
	&pxor		($Xhi,$T1);		#

	# 2nd phase
	&movdqa		($T2,$Xi);
	&psrlq		($Xi,1);
	&pxor		($Xhi,$T2);		#
	&pxor		($T2,$Xi);
	&psrlq		($Xi,5);
	&pxor		($Xi,$T2);		#
	&psrlq		($Xi,1);		#
	&pxor		($Xi,$Xhi)		#
}

&function_begin_B("gcm_init_clmul");
	&mov		($Htbl,&wparam(0));
	&mov		($Xip,&wparam(1));

	&call		(&label("pic"));
&set_label("pic");
	&blindpop	($const);
	&lea		($const,&DWP(&label("bswap")."-".&label("pic"),$const));

	&movdqu		($Hkey,&QWP(0,$Xip));
	&pshufd		($Hkey,$Hkey,0b01001110);# dword swap

	# <<1 twist
	&pshufd		($T2,$Hkey,0b11111111);	# broadcast uppermost dword
	&movdqa		($T1,$Hkey);
	&psllq		($Hkey,1);
	&pxor		($T3,$T3);		#
	&psrlq		($T1,63);
	&pcmpgtd	($T3,$T2);		# broadcast carry bit
	&pslldq		($T1,8);
	&por		($Hkey,$T1);		# H<<=1

	# magic reduction
	&pand		($T3,&QWP(16,$const));	# 0x1c2_polynomial
	&pxor		($Hkey,$T3);		# if(carry) H^=0x1c2_polynomial

	# calculate H^2
	&movdqa		($Xi,$Hkey);
	&clmul64x64_T2	($Xhi,$Xi,$Hkey);
	&reduction_alg9	($Xhi,$Xi);

	&pshufd		($T1,$Hkey,0b01001110);
	&pshufd		($T2,$Xi,0b01001110);
	&pxor		($T1,$Hkey);		# Karatsuba pre-processing
	&movdqu		(&QWP(0,$Htbl),$Hkey);	# save H
	&pxor		($T2,$Xi);		# Karatsuba pre-processing
	&movdqu		(&QWP(16,$Htbl),$Xi);	# save H^2
	&palignr	($T2,$T1,8);		# low part is H.lo^H.hi
	&movdqu		(&QWP(32,$Htbl),$T2);	# save Karatsuba "salt"

	&ret		();
&function_end_B("gcm_init_clmul");

&function_begin("gcm_ghash_clmul");
	&mov		($Xip,&wparam(0));
	&mov		($Htbl,&wparam(1));
	&mov		($inp,&wparam(2));
	&mov		($len,&wparam(3));

	&call		(&label("pic"));
&set_label("pic");
	&blindpop	($const);
	&lea		($const,&DWP(&label("bswap")."-".&label("pic"),$const));

	&movdqu		($Xi,&QWP(0,$Xip));
	&movdqa		($T3,&QWP(0,$const));
	&movdqu		($Hkey,&QWP(0,$Htbl));
	&pshufb		($Xi,$T3);

	&sub		($len,0x10);
	&jz		(&label("odd_tail"));

	#######
	# Xi+2 =[H*(Ii+1 + Xi+1)] mod P =
	#	[(H*Ii+1) + (H*Xi+1)] mod P =
	#	[(H*Ii+1) + H^2*(Ii+Xi)] mod P
	#
	&movdqu		($T1,&QWP(0,$inp));	# Ii
	&movdqu		($Xn,&QWP(16,$inp));	# Ii+1
	&pshufb		($T1,$T3);
	&pshufb		($Xn,$T3);
	&movdqu		($T3,&QWP(32,$Htbl));
	&pxor		($Xi,$T1);		# Ii+Xi

	&pshufd		($T1,$Xn,0b01001110);	# H*Ii+1
	&movdqa		($Xhn,$Xn);
	&pxor		($T1,$Xn);		#
	&lea		($inp,&DWP(32,$inp));	# i+=2

	&pclmulqdq	($Xn,$Hkey,0x00);	#######
	&pclmulqdq	($Xhn,$Hkey,0x11);	#######
	&pclmulqdq	($T1,$T3,0x00);		#######
	&movups		($Hkey,&QWP(16,$Htbl));	# load H^2
	&nop		();

	&sub		($len,0x20);
	&jbe		(&label("even_tail"));
	&jmp		(&label("mod_loop"));

&set_label("mod_loop",32);
	&pshufd		($T2,$Xi,0b01001110);	# H^2*(Ii+Xi)
	&movdqa		($Xhi,$Xi);
	&pxor		($T2,$Xi);		#
	&nop		();

	&pclmulqdq	($Xi,$Hkey,0x00);	#######
	&pclmulqdq	($Xhi,$Hkey,0x11);	#######
	&pclmulqdq	($T2,$T3,0x10);		#######
	&movups		($Hkey,&QWP(0,$Htbl));	# load H

	&xorps		($Xi,$Xn);		# (H*Ii+1) + H^2*(Ii+Xi)
	&movdqa		($T3,&QWP(0,$const));
	&xorps		($Xhi,$Xhn);
	 &movdqu	($Xhn,&QWP(0,$inp));	# Ii
	&pxor		($T1,$Xi);		# aggregated Karatsuba post-processing
	 &movdqu	($Xn,&QWP(16,$inp));	# Ii+1
	&pxor		($T1,$Xhi);		#

	 &pshufb	($Xhn,$T3);
	&pxor		($T2,$T1);		#

	&movdqa		($T1,$T2);		#
	&psrldq		($T2,8);
	&pslldq		($T1,8);		#
	&pxor		($Xhi,$T2);
	&pxor		($Xi,$T1);		#
	 &pshufb	($Xn,$T3);
	 &pxor		($Xhi,$Xhn);		# "Ii+Xi", consume early

	&movdqa		($Xhn,$Xn);		#&clmul64x64_TX	($Xhn,$Xn,$Hkey); H*Ii+1
	  &movdqa	($T2,$Xi);		#&reduction_alg9($Xhi,$Xi); 1st phase
	  &movdqa	($T1,$Xi);
	  &psllq	($Xi,5);
	  &pxor		($T1,$Xi);		#
	  &psllq	($Xi,1);
	  &pxor		($Xi,$T1);		#
	&pclmulqdq	($Xn,$Hkey,0x00);	#######
	&movups		($T3,&QWP(32,$Htbl));
	  &psllq	($Xi,57);		#
	  &movdqa	($T1,$Xi);		#
	  &pslldq	($Xi,8);
	  &psrldq	($T1,8);		#
	  &pxor		($Xi,$T2);
	  &pxor		($Xhi,$T1);		#
	&pshufd		($T1,$Xhn,0b01001110);
	  &movdqa	($T2,$Xi);		# 2nd phase
	  &psrlq	($Xi,1);
	&pxor		($T1,$Xhn);
	  &pxor		($Xhi,$T2);		#
	&pclmulqdq	($Xhn,$Hkey,0x11);	#######
	&movups		($Hkey,&QWP(16,$Htbl));	# load H^2
	  &pxor		($T2,$Xi);
	  &psrlq	($Xi,5);
	  &pxor		($Xi,$T2);		#
	  &psrlq	($Xi,1);		#
	  &pxor		($Xi,$Xhi)		#
	&pclmulqdq	($T1,$T3,0x00);		#######

	&lea		($inp,&DWP(32,$inp));
	&sub		($len,0x20);
	&ja		(&label("mod_loop"));

&set_label("even_tail");
	&pshufd		($T2,$Xi,0b01001110);	# H^2*(Ii+Xi)
	&movdqa		($Xhi,$Xi);
	&pxor		($T2,$Xi);		#

	&pclmulqdq	($Xi,$Hkey,0x00);	#######
	&pclmulqdq	($Xhi,$Hkey,0x11);	#######
	&pclmulqdq	($T2,$T3,0x10);		#######
	&movdqa		($T3,&QWP(0,$const));

	&xorps		($Xi,$Xn);		# (H*Ii+1) + H^2*(Ii+Xi)
	&xorps		($Xhi,$Xhn);
	&pxor		($T1,$Xi);		# aggregated Karatsuba post-processing
	&pxor		($T1,$Xhi);		#

	&pxor		($T2,$T1);		#

	&movdqa		($T1,$T2);		#
	&psrldq		($T2,8);
	&pslldq		($T1,8);		#
	&pxor		($Xhi,$T2);
	&pxor		($Xi,$T1);		#

	&reduction_alg9	($Xhi,$Xi);

	&test		($len,$len);
	&jnz		(&label("done"));

	&movups		($Hkey,&QWP(0,$Htbl));	# load H
&set_label("odd_tail");
	&movdqu		($T1,&QWP(0,$inp));	# Ii
	&pshufb		($T1,$T3);
	&pxor		($Xi,$T1);		# Ii+Xi

	&clmul64x64_T2	($Xhi,$Xi,$Hkey);	# H*(Ii+Xi)
	&reduction_alg9	($Xhi,$Xi);

&set_label("done");
	&pshufb		($Xi,$T3);
	&movdqu		(&QWP(0,$Xip),$Xi);
&function_end("gcm_ghash_clmul");

}

&set_label("bswap",64);
	&data_byte(15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0);
	&data_byte(1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2);	# 0x1c2_polynomial
}}	# $sse2
}}}	# !$x86only

&asciz("GHASH for x86, CRYPTOGAMS by <appro\@openssl.org>");
&asm_finish();

close STDOUT or die "error closing STDOUT: $!";

# A question was risen about choice of vanilla MMX. Or rather why wasn't
# SSE2 chosen instead? In addition to the fact that MMX runs on legacy
# CPUs such as PIII, "4-bit" MMX version was observed to provide better
# performance than *corresponding* SSE2 one even on contemporary CPUs.
# SSE2 results were provided by Peter-Michael Hager. He maintains SSE2
# implementation featuring full range of lookup-table sizes, but with
# per-invocation lookup table setup. Latter means that table size is
# chosen depending on how much data is to be hashed in every given call,
# more data - larger table. Best reported result for Core2 is ~4 cycles
# per processed byte out of 64KB block. This number accounts even for
# 64KB table setup overhead. As discussed in gcm128.c we choose to be
# more conservative in respect to lookup table sizes, but how do the
# results compare? Minimalistic "256B" MMX version delivers ~11 cycles
# on same platform. As also discussed in gcm128.c, next in line "8-bit
# Shoup's" or "4KB" method should deliver twice the performance of
# "256B" one, in other words not worse than ~6 cycles per byte. It
# should be also be noted that in SSE2 case improvement can be "super-
# linear," i.e. more than twice, mostly because >>8 maps to single
# instruction on SSE2 register. This is unlike "4-bit" case when >>4
# maps to same amount of instructions in both MMX and SSE2 cases.
# Bottom line is that switch to SSE2 is considered to be justifiable
# only in case we choose to implement "8-bit" method...
