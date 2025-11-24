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
# March, June 2010
#
# The module implements "4-bit" GCM GHASH function and underlying
# single multiplication operation in GF(2^128). "4-bit" means that
# it uses 256 bytes per-key table [+128 bytes shared table]. GHASH
# function features so called "528B" variant utilizing additional
# 256+16 bytes of per-key storage [+512 bytes shared table].
# Performance results are for this streamed GHASH subroutine and are
# expressed in cycles per processed byte, less is better:
#
#		gcc 3.4.x(*)	assembler
#
# P4		28.6		14.0		+100%
# Opteron	19.3		7.7		+150%
# Core2		17.8		8.1(**)		+120%
# Atom		31.6		16.8		+88%
# VIA Nano	21.8		10.1		+115%
#
# (*)	comparison is not completely fair, because C results are
#	for vanilla "256B" implementation, while assembler results
#	are for "528B";-)
# (**)	it's mystery [to me] why Core2 result is not same as for
#	Opteron;

# May 2010
#
# Add PCLMULQDQ version performing at 2.02 cycles per processed byte.
# See ghash-x86.pl for background information and details about coding
# techniques.
#
# Special thanks to David Woodhouse for providing access to a
# Westmere-based system on behalf of Intel Open Source Technology Centre.

# December 2012
#
# Overhaul: aggregate Karatsuba post-processing, improve ILP in
# reduction_alg9, increase reduction aggregate factor to 4x. As for
# the latter. ghash-x86.pl discusses that it makes lesser sense to
# increase aggregate factor. Then why increase here? Critical path
# consists of 3 independent pclmulqdq instructions, Karatsuba post-
# processing and reduction. "On top" of this we lay down aggregated
# multiplication operations, triplets of independent pclmulqdq's. As
# issue rate for pclmulqdq is limited, it makes lesser sense to
# aggregate more multiplications than it takes to perform remaining
# non-multiplication operations. 2x is near-optimal coefficient for
# contemporary Intel CPUs (therefore modest improvement coefficient),
# but not for Bulldozer. Latter is because logical SIMD operations
# are twice as slow in comparison to Intel, so that critical path is
# longer. A CPU with higher pclmulqdq issue rate would also benefit
# from higher aggregate factor...
#
# Westmere	1.78(+13%)
# Sandy Bridge	1.80(+8%)
# Ivy Bridge	1.80(+7%)
# Haswell	0.55(+93%) (if system doesn't support AVX)
# Broadwell	0.45(+110%)(if system doesn't support AVX)
# Skylake	0.44(+110%)(if system doesn't support AVX)
# Bulldozer	1.49(+27%)
# Silvermont	2.88(+13%)
# Knights L	2.12(-)    (if system doesn't support AVX)
# Goldmont	1.08(+24%)

# March 2013
#
# ... 8x aggregate factor AVX code path is using reduction algorithm
# suggested by Shay Gueron[1]. Even though contemporary AVX-capable
# CPUs such as Sandy and Ivy Bridge can execute it, the code performs
# sub-optimally in comparison to above mentioned version. But thanks
# to Ilya Albrekht and Max Locktyukhin of Intel Corp. we knew that
# it performs in 0.41 cycles per byte on Haswell processor, in
# 0.29 on Broadwell, and in 0.36 on Skylake.
#
# Knights Landing achieves 1.09 cpb.
#
# [1] http://rt.openssl.org/Ticket/Display.html?id=2900&user=guest&pass=guest

# This file was patched in BoringSSL to remove the variable-time 4-bit
# implementation.

$flavour = shift;
$output  = shift;
if ($flavour =~ /\./) { $output = $flavour; undef $flavour; }

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

# See the notes about |$avx| in aesni-gcm-x86_64.pl; otherwise tags will be
# computed incorrectly.
#
# In upstream, this is controlled by shelling out to the compiler to check
# versions, but BoringSSL is intended to be used with pre-generated perlasm
# output, so this isn't useful anyway.
$avx = 1;

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

$do4xaggr=1;


$code=<<___;
.text
___


######################################################################
# PCLMULQDQ version.

@_4args=$win64?	("%rcx","%rdx","%r8", "%r9") :	# Win64 order
		("%rdi","%rsi","%rdx","%rcx");	# Unix order

($Xi,$Xhi)=("%xmm0","%xmm1");	$Hkey="%xmm2";
($T1,$T2,$T3)=("%xmm3","%xmm4","%xmm5");

sub clmul64x64_T2 {	# minimal register pressure
my ($Xhi,$Xi,$Hkey,$HK)=@_;

if (!defined($HK)) {	$HK = $T2;
$code.=<<___;
	movdqa		$Xi,$Xhi		#
	pshufd		\$0b01001110,$Xi,$T1
	pshufd		\$0b01001110,$Hkey,$T2
	pxor		$Xi,$T1			#
	pxor		$Hkey,$T2
___
} else {
$code.=<<___;
	movdqa		$Xi,$Xhi		#
	pshufd		\$0b01001110,$Xi,$T1
	pxor		$Xi,$T1			#
___
}
$code.=<<___;
	pclmulqdq	\$0x00,$Hkey,$Xi	#######
	pclmulqdq	\$0x11,$Hkey,$Xhi	#######
	pclmulqdq	\$0x00,$HK,$T1		#######
	pxor		$Xi,$T1			#
	pxor		$Xhi,$T1		#

	movdqa		$T1,$T2			#
	psrldq		\$8,$T1
	pslldq		\$8,$T2			#
	pxor		$T1,$Xhi
	pxor		$T2,$Xi			#
___
}

sub reduction_alg9 {	# 17/11 times faster than Intel version
my ($Xhi,$Xi) = @_;

$code.=<<___;
	# 1st phase
	movdqa		$Xi,$T2			#
	movdqa		$Xi,$T1
	psllq		\$5,$Xi
	pxor		$Xi,$T1			#
	psllq		\$1,$Xi
	pxor		$T1,$Xi			#
	psllq		\$57,$Xi		#
	movdqa		$Xi,$T1			#
	pslldq		\$8,$Xi
	psrldq		\$8,$T1			#
	pxor		$T2,$Xi
	pxor		$T1,$Xhi		#

	# 2nd phase
	movdqa		$Xi,$T2
	psrlq		\$1,$Xi
	pxor		$T2,$Xhi		#
	pxor		$Xi,$T2
	psrlq		\$5,$Xi
	pxor		$T2,$Xi			#
	psrlq		\$1,$Xi			#
	pxor		$Xhi,$Xi		#
___
}

{ my ($Htbl,$Xip)=@_4args;
  my $HK="%xmm6";

$code.=<<___;
.globl	gcm_init_clmul
.type	gcm_init_clmul,\@abi-omnipotent
.align	16
gcm_init_clmul:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
.L_init_clmul:
___
$code.=<<___ if ($win64);
	sub	\$0x18,%rsp
.seh_stackalloc	0x18
	movaps	%xmm6,(%rsp)
.seh_savexmm	%xmm6, 0
.seh_endprologue
___
$code.=<<___;
	movdqu		($Xip),$Hkey
	pshufd		\$0b01001110,$Hkey,$Hkey	# dword swap

	# <<1 twist
	pshufd		\$0b11111111,$Hkey,$T2	# broadcast uppermost dword
	movdqa		$Hkey,$T1
	psllq		\$1,$Hkey
	pxor		$T3,$T3			#
	psrlq		\$63,$T1
	pcmpgtd		$T2,$T3			# broadcast carry bit
	pslldq		\$8,$T1
	por		$T1,$Hkey		# H<<=1

	# magic reduction
	pand		.L0x1c2_polynomial(%rip),$T3
	pxor		$T3,$Hkey		# if(carry) H^=0x1c2_polynomial

	# calculate H^2
	pshufd		\$0b01001110,$Hkey,$HK
	movdqa		$Hkey,$Xi
	pxor		$Hkey,$HK
___
	&clmul64x64_T2	($Xhi,$Xi,$Hkey,$HK);
	&reduction_alg9	($Xhi,$Xi);
$code.=<<___;
	pshufd		\$0b01001110,$Hkey,$T1
	pshufd		\$0b01001110,$Xi,$T2
	pxor		$Hkey,$T1		# Karatsuba pre-processing
	movdqu		$Hkey,0x00($Htbl)	# save H
	pxor		$Xi,$T2			# Karatsuba pre-processing
	movdqu		$Xi,0x10($Htbl)		# save H^2
	palignr		\$8,$T1,$T2		# low part is H.lo^H.hi...
	movdqu		$T2,0x20($Htbl)		# save Karatsuba "salt"
___
if ($do4xaggr) {
	&clmul64x64_T2	($Xhi,$Xi,$Hkey,$HK);	# H^3
	&reduction_alg9	($Xhi,$Xi);
$code.=<<___;
	movdqa		$Xi,$T3
___
	&clmul64x64_T2	($Xhi,$Xi,$Hkey,$HK);	# H^4
	&reduction_alg9	($Xhi,$Xi);
$code.=<<___;
	pshufd		\$0b01001110,$T3,$T1
	pshufd		\$0b01001110,$Xi,$T2
	pxor		$T3,$T1			# Karatsuba pre-processing
	movdqu		$T3,0x30($Htbl)		# save H^3
	pxor		$Xi,$T2			# Karatsuba pre-processing
	movdqu		$Xi,0x40($Htbl)		# save H^4
	palignr		\$8,$T1,$T2		# low part is H^3.lo^H^3.hi...
	movdqu		$T2,0x50($Htbl)		# save Karatsuba "salt"
___
}
$code.=<<___ if ($win64);
	movaps	(%rsp),%xmm6
	lea	0x18(%rsp),%rsp
___
$code.=<<___;
	ret
.cfi_endproc
.seh_endproc
.size	gcm_init_clmul,.-gcm_init_clmul
___
}


{ my ($Xip,$Htbl,$inp,$len)=@_4args;
  my ($Xln,$Xmn,$Xhn,$Hkey2,$HK) = map("%xmm$_",(3..7));
  my ($T1,$T2,$T3)=map("%xmm$_",(8..10));

$code.=<<___;
.globl	gcm_ghash_clmul
.type	gcm_ghash_clmul,\@abi-omnipotent
.align	32
gcm_ghash_clmul:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
.L_ghash_clmul:
___
$code.=<<___ if ($win64);
	lea	-0x88(%rsp),%rax
	lea	-0x20(%rax),%rsp
.seh_stackalloc	0x20+0x88
	movaps	%xmm6,-0x20(%rax)
.seh_savexmm	%xmm6, 0x20-0x20
	movaps	%xmm7,-0x10(%rax)
.seh_savexmm	%xmm7, 0x20-0x10
	movaps	%xmm8,0(%rax)
.seh_savexmm	%xmm8, 0x20+0
	movaps	%xmm9,0x10(%rax)
.seh_savexmm	%xmm9, 0x20+0x10
	movaps	%xmm10,0x20(%rax)
.seh_savexmm	%xmm10, 0x20+0x20
	movaps	%xmm11,0x30(%rax)
.seh_savexmm	%xmm11, 0x20+0x30
	movaps	%xmm12,0x40(%rax)
.seh_savexmm	%xmm12, 0x20+0x40
	movaps	%xmm13,0x50(%rax)
.seh_savexmm	%xmm13, 0x20+0x50
	movaps	%xmm14,0x60(%rax)
.seh_savexmm	%xmm14, 0x20+0x60
	movaps	%xmm15,0x70(%rax)
.seh_savexmm	%xmm15, 0x20+0x70
.seh_endprologue
___
$code.=<<___;
	movdqa		.Lbswap_mask(%rip),$T3

	movdqu		($Xip),$Xi
	movdqu		($Htbl),$Hkey
	movdqu		0x20($Htbl),$HK
	pshufb		$T3,$Xi

	sub		\$0x10,$len
	jz		.Lodd_tail

	movdqu		0x10($Htbl),$Hkey2
___
if ($do4xaggr) {
my ($Xl,$Xm,$Xh,$Hkey3,$Hkey4)=map("%xmm$_",(11..15));

$code.=<<___;
	cmp		\$0x30,$len
	jb		.Lskip4x

	sub		\$0x30,$len
	mov		\$0xA040608020C0E000,%rax	# ((7..0)·0xE0)&0xff
	movdqu		0x30($Htbl),$Hkey3
	movdqu		0x40($Htbl),$Hkey4

	#######
	# Xi+4 =[(H*Ii+3) + (H^2*Ii+2) + (H^3*Ii+1) + H^4*(Ii+Xi)] mod P
	#
	movdqu		0x30($inp),$Xln
	 movdqu		0x20($inp),$Xl
	pshufb		$T3,$Xln
	 pshufb		$T3,$Xl
	movdqa		$Xln,$Xhn
	pshufd		\$0b01001110,$Xln,$Xmn
	pxor		$Xln,$Xmn
	pclmulqdq	\$0x00,$Hkey,$Xln
	pclmulqdq	\$0x11,$Hkey,$Xhn
	pclmulqdq	\$0x00,$HK,$Xmn

	movdqa		$Xl,$Xh
	pshufd		\$0b01001110,$Xl,$Xm
	pxor		$Xl,$Xm
	pclmulqdq	\$0x00,$Hkey2,$Xl
	pclmulqdq	\$0x11,$Hkey2,$Xh
	pclmulqdq	\$0x10,$HK,$Xm
	xorps		$Xl,$Xln
	xorps		$Xh,$Xhn
	movups		0x50($Htbl),$HK
	xorps		$Xm,$Xmn

	movdqu		0x10($inp),$Xl
	 movdqu		0($inp),$T1
	pshufb		$T3,$Xl
	 pshufb		$T3,$T1
	movdqa		$Xl,$Xh
	pshufd		\$0b01001110,$Xl,$Xm
	 pxor		$T1,$Xi
	pxor		$Xl,$Xm
	pclmulqdq	\$0x00,$Hkey3,$Xl
	 movdqa		$Xi,$Xhi
	 pshufd		\$0b01001110,$Xi,$T1
	 pxor		$Xi,$T1
	pclmulqdq	\$0x11,$Hkey3,$Xh
	pclmulqdq	\$0x00,$HK,$Xm
	xorps		$Xl,$Xln
	xorps		$Xh,$Xhn

	lea	0x40($inp),$inp
	sub	\$0x40,$len
	jc	.Ltail4x

	jmp	.Lmod4_loop
.align	32
.Lmod4_loop:
	pclmulqdq	\$0x00,$Hkey4,$Xi
	xorps		$Xm,$Xmn
	 movdqu		0x30($inp),$Xl
	 pshufb		$T3,$Xl
	pclmulqdq	\$0x11,$Hkey4,$Xhi
	xorps		$Xln,$Xi
	 movdqu		0x20($inp),$Xln
	 movdqa		$Xl,$Xh
	pclmulqdq	\$0x10,$HK,$T1
	 pshufd		\$0b01001110,$Xl,$Xm
	xorps		$Xhn,$Xhi
	 pxor		$Xl,$Xm
	 pshufb		$T3,$Xln
	movups		0x20($Htbl),$HK
	xorps		$Xmn,$T1
	 pclmulqdq	\$0x00,$Hkey,$Xl
	 pshufd		\$0b01001110,$Xln,$Xmn

	pxor		$Xi,$T1			# aggregated Karatsuba post-processing
	 movdqa		$Xln,$Xhn
	pxor		$Xhi,$T1		#
	 pxor		$Xln,$Xmn
	movdqa		$T1,$T2			#
	 pclmulqdq	\$0x11,$Hkey,$Xh
	pslldq		\$8,$T1
	psrldq		\$8,$T2			#
	pxor		$T1,$Xi
	movdqa		.L7_mask(%rip),$T1
	pxor		$T2,$Xhi		#
	movq		%rax,$T2

	pand		$Xi,$T1			# 1st phase
	pshufb		$T1,$T2			#
	pxor		$Xi,$T2			#
	 pclmulqdq	\$0x00,$HK,$Xm
	psllq		\$57,$T2		#
	movdqa		$T2,$T1			#
	pslldq		\$8,$T2
	 pclmulqdq	\$0x00,$Hkey2,$Xln
	psrldq		\$8,$T1			#
	pxor		$T2,$Xi
	pxor		$T1,$Xhi		#
	movdqu		0($inp),$T1

	movdqa		$Xi,$T2			# 2nd phase
	psrlq		\$1,$Xi
	 pclmulqdq	\$0x11,$Hkey2,$Xhn
	 xorps		$Xl,$Xln
	 movdqu		0x10($inp),$Xl
	 pshufb		$T3,$Xl
	 pclmulqdq	\$0x10,$HK,$Xmn
	 xorps		$Xh,$Xhn
	 movups		0x50($Htbl),$HK
	pshufb		$T3,$T1
	pxor		$T2,$Xhi		#
	pxor		$Xi,$T2
	psrlq		\$5,$Xi

	 movdqa		$Xl,$Xh
	 pxor		$Xm,$Xmn
	 pshufd		\$0b01001110,$Xl,$Xm
	pxor		$T2,$Xi			#
	pxor		$T1,$Xhi
	 pxor		$Xl,$Xm
	 pclmulqdq	\$0x00,$Hkey3,$Xl
	psrlq		\$1,$Xi			#
	pxor		$Xhi,$Xi		#
	movdqa		$Xi,$Xhi
	 pclmulqdq	\$0x11,$Hkey3,$Xh
	 xorps		$Xl,$Xln
	pshufd		\$0b01001110,$Xi,$T1
	pxor		$Xi,$T1

	 pclmulqdq	\$0x00,$HK,$Xm
	 xorps		$Xh,$Xhn

	lea	0x40($inp),$inp
	sub	\$0x40,$len
	jnc	.Lmod4_loop

.Ltail4x:
	pclmulqdq	\$0x00,$Hkey4,$Xi
	pclmulqdq	\$0x11,$Hkey4,$Xhi
	pclmulqdq	\$0x10,$HK,$T1
	xorps		$Xm,$Xmn
	xorps		$Xln,$Xi
	xorps		$Xhn,$Xhi
	pxor		$Xi,$Xhi		# aggregated Karatsuba post-processing
	pxor		$Xmn,$T1

	pxor		$Xhi,$T1		#
	pxor		$Xi,$Xhi

	movdqa		$T1,$T2			#
	psrldq		\$8,$T1
	pslldq		\$8,$T2			#
	pxor		$T1,$Xhi
	pxor		$T2,$Xi			#
___
	&reduction_alg9($Xhi,$Xi);
$code.=<<___;
	add	\$0x40,$len
	jz	.Ldone
	movdqu	0x20($Htbl),$HK
	sub	\$0x10,$len
	jz	.Lodd_tail
.Lskip4x:
___
}
$code.=<<___;
	#######
	# Xi+2 =[H*(Ii+1 + Xi+1)] mod P =
	#	[(H*Ii+1) + (H*Xi+1)] mod P =
	#	[(H*Ii+1) + H^2*(Ii+Xi)] mod P
	#
	movdqu		($inp),$T1		# Ii
	movdqu		16($inp),$Xln		# Ii+1
	pshufb		$T3,$T1
	pshufb		$T3,$Xln
	pxor		$T1,$Xi			# Ii+Xi

	movdqa		$Xln,$Xhn
	pshufd		\$0b01001110,$Xln,$Xmn
	pxor		$Xln,$Xmn
	pclmulqdq	\$0x00,$Hkey,$Xln
	pclmulqdq	\$0x11,$Hkey,$Xhn
	pclmulqdq	\$0x00,$HK,$Xmn

	lea		32($inp),$inp		# i+=2
	nop
	sub		\$0x20,$len
	jbe		.Leven_tail
	nop
	jmp		.Lmod_loop

.align	32
.Lmod_loop:
	movdqa		$Xi,$Xhi
	movdqa		$Xmn,$T1
	pshufd		\$0b01001110,$Xi,$Xmn	#
	pxor		$Xi,$Xmn		#

	pclmulqdq	\$0x00,$Hkey2,$Xi
	pclmulqdq	\$0x11,$Hkey2,$Xhi
	pclmulqdq	\$0x10,$HK,$Xmn

	pxor		$Xln,$Xi		# (H*Ii+1) + H^2*(Ii+Xi)
	pxor		$Xhn,$Xhi
	  movdqu	($inp),$T2		# Ii
	pxor		$Xi,$T1			# aggregated Karatsuba post-processing
	  pshufb	$T3,$T2
	  movdqu	16($inp),$Xln		# Ii+1

	pxor		$Xhi,$T1
	  pxor		$T2,$Xhi		# "Ii+Xi", consume early
	pxor		$T1,$Xmn
	 pshufb		$T3,$Xln
	movdqa		$Xmn,$T1		#
	psrldq		\$8,$T1
	pslldq		\$8,$Xmn		#
	pxor		$T1,$Xhi
	pxor		$Xmn,$Xi		#

	movdqa		$Xln,$Xhn		#

	  movdqa	$Xi,$T2			# 1st phase
	  movdqa	$Xi,$T1
	  psllq		\$5,$Xi
	  pxor		$Xi,$T1			#
	pclmulqdq	\$0x00,$Hkey,$Xln	#######
	  psllq		\$1,$Xi
	  pxor		$T1,$Xi			#
	  psllq		\$57,$Xi		#
	  movdqa	$Xi,$T1			#
	  pslldq	\$8,$Xi
	  psrldq	\$8,$T1			#
	  pxor		$T2,$Xi
	pshufd		\$0b01001110,$Xhn,$Xmn
	  pxor		$T1,$Xhi		#
	pxor		$Xhn,$Xmn		#

	  movdqa	$Xi,$T2			# 2nd phase
	  psrlq		\$1,$Xi
	pclmulqdq	\$0x11,$Hkey,$Xhn	#######
	  pxor		$T2,$Xhi		#
	  pxor		$Xi,$T2
	  psrlq		\$5,$Xi
	  pxor		$T2,$Xi			#
	lea		32($inp),$inp
	  psrlq		\$1,$Xi			#
	pclmulqdq	\$0x00,$HK,$Xmn		#######
	  pxor		$Xhi,$Xi		#

	sub		\$0x20,$len
	ja		.Lmod_loop

.Leven_tail:
	 movdqa		$Xi,$Xhi
	 movdqa		$Xmn,$T1
	 pshufd		\$0b01001110,$Xi,$Xmn	#
	 pxor		$Xi,$Xmn		#

	pclmulqdq	\$0x00,$Hkey2,$Xi
	pclmulqdq	\$0x11,$Hkey2,$Xhi
	pclmulqdq	\$0x10,$HK,$Xmn

	pxor		$Xln,$Xi		# (H*Ii+1) + H^2*(Ii+Xi)
	pxor		$Xhn,$Xhi
	pxor		$Xi,$T1
	pxor		$Xhi,$T1
	pxor		$T1,$Xmn
	movdqa		$Xmn,$T1		#
	psrldq		\$8,$T1
	pslldq		\$8,$Xmn		#
	pxor		$T1,$Xhi
	pxor		$Xmn,$Xi		#
___
	&reduction_alg9	($Xhi,$Xi);
$code.=<<___;
	test		$len,$len
	jnz		.Ldone

.Lodd_tail:
	movdqu		($inp),$T1		# Ii
	pshufb		$T3,$T1
	pxor		$T1,$Xi			# Ii+Xi
___
	&clmul64x64_T2	($Xhi,$Xi,$Hkey,$HK);	# H*(Ii+Xi)
	&reduction_alg9	($Xhi,$Xi);
$code.=<<___;
.Ldone:
	pshufb		$T3,$Xi
	movdqu		$Xi,($Xip)
___
$code.=<<___ if ($win64);
	movaps	(%rsp),%xmm6
	movaps	0x10(%rsp),%xmm7
	movaps	0x20(%rsp),%xmm8
	movaps	0x30(%rsp),%xmm9
	movaps	0x40(%rsp),%xmm10
	movaps	0x50(%rsp),%xmm11
	movaps	0x60(%rsp),%xmm12
	movaps	0x70(%rsp),%xmm13
	movaps	0x80(%rsp),%xmm14
	movaps	0x90(%rsp),%xmm15
	lea	0xa8(%rsp),%rsp
___
$code.=<<___;
	ret
.cfi_endproc
.seh_endproc
.size	gcm_ghash_clmul,.-gcm_ghash_clmul
___
}

$code.=<<___;
.globl	gcm_init_avx
.type	gcm_init_avx,\@abi-omnipotent
.align	32
gcm_init_avx:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
___
if ($avx) {
my ($Htbl,$Xip)=@_4args;
my $HK="%xmm6";

$code.=<<___ if ($win64);
	sub	\$0x18,%rsp
.seh_stackalloc	0x18
	movaps	%xmm6,(%rsp)
.seh_savexmm	%xmm6, 0
.seh_endprologue
___
$code.=<<___;
	vzeroupper

	vmovdqu		($Xip),$Hkey
	vpshufd		\$0b01001110,$Hkey,$Hkey	# dword swap

	# <<1 twist
	vpshufd		\$0b11111111,$Hkey,$T2	# broadcast uppermost dword
	vpsrlq		\$63,$Hkey,$T1
	vpsllq		\$1,$Hkey,$Hkey
	vpxor		$T3,$T3,$T3		#
	vpcmpgtd	$T2,$T3,$T3		# broadcast carry bit
	vpslldq		\$8,$T1,$T1
	vpor		$T1,$Hkey,$Hkey		# H<<=1

	# magic reduction
	vpand		.L0x1c2_polynomial(%rip),$T3,$T3
	vpxor		$T3,$Hkey,$Hkey		# if(carry) H^=0x1c2_polynomial

	vpunpckhqdq	$Hkey,$Hkey,$HK
	vmovdqa		$Hkey,$Xi
	vpxor		$Hkey,$HK,$HK
	mov		\$4,%r10		# up to H^8
	jmp		.Linit_start_avx
___

sub clmul64x64_avx {
my ($Xhi,$Xi,$Hkey,$HK)=@_;

if (!defined($HK)) {	$HK = $T2;
$code.=<<___;
	vpunpckhqdq	$Xi,$Xi,$T1
	vpunpckhqdq	$Hkey,$Hkey,$T2
	vpxor		$Xi,$T1,$T1		#
	vpxor		$Hkey,$T2,$T2
___
} else {
$code.=<<___;
	vpunpckhqdq	$Xi,$Xi,$T1
	vpxor		$Xi,$T1,$T1		#
___
}
$code.=<<___;
	vpclmulqdq	\$0x11,$Hkey,$Xi,$Xhi	#######
	vpclmulqdq	\$0x00,$Hkey,$Xi,$Xi	#######
	vpclmulqdq	\$0x00,$HK,$T1,$T1	#######
	vpxor		$Xi,$Xhi,$T2		#
	vpxor		$T2,$T1,$T1		#

	vpslldq		\$8,$T1,$T2		#
	vpsrldq		\$8,$T1,$T1
	vpxor		$T2,$Xi,$Xi		#
	vpxor		$T1,$Xhi,$Xhi
___
}

sub reduction_avx {
my ($Xhi,$Xi) = @_;

$code.=<<___;
	vpsllq		\$57,$Xi,$T1		# 1st phase
	vpsllq		\$62,$Xi,$T2
	vpxor		$T1,$T2,$T2		#
	vpsllq		\$63,$Xi,$T1
	vpxor		$T1,$T2,$T2		#
	vpslldq		\$8,$T2,$T1		#
	vpsrldq		\$8,$T2,$T2
	vpxor		$T1,$Xi,$Xi		#
	vpxor		$T2,$Xhi,$Xhi

	vpsrlq		\$1,$Xi,$T2		# 2nd phase
	vpxor		$Xi,$Xhi,$Xhi
	vpxor		$T2,$Xi,$Xi		#
	vpsrlq		\$5,$T2,$T2
	vpxor		$T2,$Xi,$Xi		#
	vpsrlq		\$1,$Xi,$Xi		#
	vpxor		$Xhi,$Xi,$Xi		#
___
}

$code.=<<___;
.align	32
.Linit_loop_avx:
	vpalignr	\$8,$T1,$T2,$T3		# low part is H.lo^H.hi...
	vmovdqu		$T3,-0x10($Htbl)	# save Karatsuba "salt"
___
	&clmul64x64_avx	($Xhi,$Xi,$Hkey,$HK);	# calculate H^3,5,7
	&reduction_avx	($Xhi,$Xi);
$code.=<<___;
.Linit_start_avx:
	vmovdqa		$Xi,$T3
___
	&clmul64x64_avx	($Xhi,$Xi,$Hkey,$HK);	# calculate H^2,4,6,8
	&reduction_avx	($Xhi,$Xi);
$code.=<<___;
	vpshufd		\$0b01001110,$T3,$T1
	vpshufd		\$0b01001110,$Xi,$T2
	vpxor		$T3,$T1,$T1		# Karatsuba pre-processing
	vmovdqu		$T3,0x00($Htbl)		# save H^1,3,5,7
	vpxor		$Xi,$T2,$T2		# Karatsuba pre-processing
	vmovdqu		$Xi,0x10($Htbl)		# save H^2,4,6,8
	lea		0x30($Htbl),$Htbl
	sub		\$1,%r10
	jnz		.Linit_loop_avx

	vpalignr	\$8,$T2,$T1,$T3		# last "salt" is flipped
	vmovdqu		$T3,-0x10($Htbl)

	vzeroupper
___
$code.=<<___ if ($win64);
	movaps	(%rsp),%xmm6
	lea	0x18(%rsp),%rsp
___
$code.=<<___;
	ret
.seh_endproc
.cfi_endproc
.size	gcm_init_avx,.-gcm_init_avx
___
} else {
$code.=<<___;
	jmp	.L_init_clmul
.size	gcm_init_avx,.-gcm_init_avx
___
}

$code.=<<___;
.globl	gcm_ghash_avx
.type	gcm_ghash_avx,\@abi-omnipotent
.align	32
gcm_ghash_avx:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
___
if ($avx) {
my ($Xip,$Htbl,$inp,$len)=@_4args;
my ($Xlo,$Xhi,$Xmi,
    $Zlo,$Zhi,$Zmi,
    $Hkey,$HK,$T1,$T2,
    $Xi,$Xo,$Tred,$bswap,$Ii,$Ij) = map("%xmm$_",(0..15));

$code.=<<___ if ($win64);
	lea	-0x88(%rsp),%rax
	lea	-0x20(%rax),%rsp
.seh_stackalloc	0x20+0x88
	movaps	%xmm6,-0x20(%rax)
.seh_savexmm	%xmm6, 0x20-0x20
	movaps	%xmm7,-0x10(%rax)
.seh_savexmm	%xmm7, 0x20-0x10
	movaps	%xmm8,0(%rax)
.seh_savexmm	%xmm8, 0x20+0
	movaps	%xmm9,0x10(%rax)
.seh_savexmm	%xmm9, 0x20+0x10
	movaps	%xmm10,0x20(%rax)
.seh_savexmm	%xmm10, 0x20+0x20
	movaps	%xmm11,0x30(%rax)
.seh_savexmm	%xmm11, 0x20+0x30
	movaps	%xmm12,0x40(%rax)
.seh_savexmm	%xmm12, 0x20+0x40
	movaps	%xmm13,0x50(%rax)
.seh_savexmm	%xmm13, 0x20+0x50
	movaps	%xmm14,0x60(%rax)
.seh_savexmm	%xmm14, 0x20+0x60
	movaps	%xmm15,0x70(%rax)
.seh_savexmm	%xmm15, 0x20+0x70
.seh_endprologue
___
$code.=<<___;
	vzeroupper

	vmovdqu		($Xip),$Xi		# load $Xi
	lea		.L0x1c2_polynomial(%rip),%r10
	lea		0x40($Htbl),$Htbl	# size optimization
	vmovdqu		.Lbswap_mask(%rip),$bswap
	vpshufb		$bswap,$Xi,$Xi
	cmp		\$0x80,$len
	jb		.Lshort_avx
	sub		\$0x80,$len

	vmovdqu		0x70($inp),$Ii		# I[7]
	vmovdqu		0x00-0x40($Htbl),$Hkey	# $Hkey^1
	vpshufb		$bswap,$Ii,$Ii
	vmovdqu		0x20-0x40($Htbl),$HK

	vpunpckhqdq	$Ii,$Ii,$T2
	 vmovdqu	0x60($inp),$Ij		# I[6]
	vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	vpxor		$Ii,$T2,$T2
	 vpshufb	$bswap,$Ij,$Ij
	vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	 vmovdqu	0x10-0x40($Htbl),$Hkey	# $Hkey^2
	 vpunpckhqdq	$Ij,$Ij,$T1
	 vmovdqu	0x50($inp),$Ii		# I[5]
	vpclmulqdq	\$0x00,$HK,$T2,$Xmi
	 vpxor		$Ij,$T1,$T1

	 vpshufb	$bswap,$Ii,$Ii
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	 vpunpckhqdq	$Ii,$Ii,$T2
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	 vmovdqu	0x30-0x40($Htbl),$Hkey	# $Hkey^3
	 vpxor		$Ii,$T2,$T2
	 vmovdqu	0x40($inp),$Ij		# I[4]
	vpclmulqdq	\$0x10,$HK,$T1,$Zmi
	 vmovdqu	0x50-0x40($Htbl),$HK

	 vpshufb	$bswap,$Ij,$Ij
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	vpxor		$Xhi,$Zhi,$Zhi
	 vpunpckhqdq	$Ij,$Ij,$T1
	vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	 vmovdqu	0x40-0x40($Htbl),$Hkey	# $Hkey^4
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T2,$Xmi
	 vpxor		$Ij,$T1,$T1

	 vmovdqu	0x30($inp),$Ii		# I[3]
	vpxor		$Zlo,$Xlo,$Xlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	vpxor		$Zhi,$Xhi,$Xhi
	 vpshufb	$bswap,$Ii,$Ii
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	 vmovdqu	0x60-0x40($Htbl),$Hkey	# $Hkey^5
	vpxor		$Zmi,$Xmi,$Xmi
	 vpunpckhqdq	$Ii,$Ii,$T2
	vpclmulqdq	\$0x10,$HK,$T1,$Zmi
	 vmovdqu	0x80-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2

	 vmovdqu	0x20($inp),$Ij		# I[2]
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	vpxor		$Xhi,$Zhi,$Zhi
	 vpshufb	$bswap,$Ij,$Ij
	vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	 vmovdqu	0x70-0x40($Htbl),$Hkey	# $Hkey^6
	vpxor		$Xmi,$Zmi,$Zmi
	 vpunpckhqdq	$Ij,$Ij,$T1
	vpclmulqdq	\$0x00,$HK,$T2,$Xmi
	 vpxor		$Ij,$T1,$T1

	 vmovdqu	0x10($inp),$Ii		# I[1]
	vpxor		$Zlo,$Xlo,$Xlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	vpxor		$Zhi,$Xhi,$Xhi
	 vpshufb	$bswap,$Ii,$Ii
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	 vmovdqu	0x90-0x40($Htbl),$Hkey	# $Hkey^7
	vpxor		$Zmi,$Xmi,$Xmi
	 vpunpckhqdq	$Ii,$Ii,$T2
	vpclmulqdq	\$0x10,$HK,$T1,$Zmi
	 vmovdqu	0xb0-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2

	 vmovdqu	($inp),$Ij		# I[0]
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	vpxor		$Xhi,$Zhi,$Zhi
	 vpshufb	$bswap,$Ij,$Ij
	vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	 vmovdqu	0xa0-0x40($Htbl),$Hkey	# $Hkey^8
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x10,$HK,$T2,$Xmi

	lea		0x80($inp),$inp
	cmp		\$0x80,$len
	jb		.Ltail_avx

	vpxor		$Xi,$Ij,$Ij		# accumulate $Xi
	sub		\$0x80,$len
	jmp		.Loop8x_avx

.align	32
.Loop8x_avx:
	vpunpckhqdq	$Ij,$Ij,$T1
	 vmovdqu	0x70($inp),$Ii		# I[7]
	vpxor		$Xlo,$Zlo,$Zlo
	vpxor		$Ij,$T1,$T1
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xi
	 vpshufb	$bswap,$Ii,$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xo
	 vmovdqu	0x00-0x40($Htbl),$Hkey	# $Hkey^1
	 vpunpckhqdq	$Ii,$Ii,$T2
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Tred
	 vmovdqu	0x20-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2

	  vmovdqu	0x60($inp),$Ij		# I[6]
	 vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	vpxor		$Zlo,$Xi,$Xi		# collect result
	  vpshufb	$bswap,$Ij,$Ij
	 vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	vxorps		$Zhi,$Xo,$Xo
	  vmovdqu	0x10-0x40($Htbl),$Hkey	# $Hkey^2
	 vpunpckhqdq	$Ij,$Ij,$T1
	 vpclmulqdq	\$0x00,$HK,  $T2,$Xmi
	vpxor		$Zmi,$Tred,$Tred
	 vxorps		$Ij,$T1,$T1

	  vmovdqu	0x50($inp),$Ii		# I[5]
	vpxor		$Xi,$Tred,$Tred		# aggregated Karatsuba post-processing
	 vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	vpxor		$Xo,$Tred,$Tred
	vpslldq		\$8,$Tred,$T2
	 vpxor		$Xlo,$Zlo,$Zlo
	 vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	vpsrldq		\$8,$Tred,$Tred
	vpxor		$T2, $Xi, $Xi
	  vmovdqu	0x30-0x40($Htbl),$Hkey	# $Hkey^3
	  vpshufb	$bswap,$Ii,$Ii
	vxorps		$Tred,$Xo, $Xo
	 vpxor		$Xhi,$Zhi,$Zhi
	 vpunpckhqdq	$Ii,$Ii,$T2
	 vpclmulqdq	\$0x10,$HK,  $T1,$Zmi
	  vmovdqu	0x50-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2
	 vpxor		$Xmi,$Zmi,$Zmi

	  vmovdqu	0x40($inp),$Ij		# I[4]
	vpalignr	\$8,$Xi,$Xi,$Tred	# 1st phase
	 vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	  vpshufb	$bswap,$Ij,$Ij
	 vpxor		$Zlo,$Xlo,$Xlo
	 vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	  vmovdqu	0x40-0x40($Htbl),$Hkey	# $Hkey^4
	 vpunpckhqdq	$Ij,$Ij,$T1
	 vpxor		$Zhi,$Xhi,$Xhi
	 vpclmulqdq	\$0x00,$HK,  $T2,$Xmi
	 vxorps		$Ij,$T1,$T1
	 vpxor		$Zmi,$Xmi,$Xmi

	  vmovdqu	0x30($inp),$Ii		# I[3]
	vpclmulqdq	\$0x10,(%r10),$Xi,$Xi
	 vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	  vpshufb	$bswap,$Ii,$Ii
	 vpxor		$Xlo,$Zlo,$Zlo
	 vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	  vmovdqu	0x60-0x40($Htbl),$Hkey	# $Hkey^5
	 vpunpckhqdq	$Ii,$Ii,$T2
	 vpxor		$Xhi,$Zhi,$Zhi
	 vpclmulqdq	\$0x10,$HK,  $T1,$Zmi
	  vmovdqu	0x80-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2
	 vpxor		$Xmi,$Zmi,$Zmi

	  vmovdqu	0x20($inp),$Ij		# I[2]
	 vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	  vpshufb	$bswap,$Ij,$Ij
	 vpxor		$Zlo,$Xlo,$Xlo
	 vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	  vmovdqu	0x70-0x40($Htbl),$Hkey	# $Hkey^6
	 vpunpckhqdq	$Ij,$Ij,$T1
	 vpxor		$Zhi,$Xhi,$Xhi
	 vpclmulqdq	\$0x00,$HK,  $T2,$Xmi
	 vpxor		$Ij,$T1,$T1
	 vpxor		$Zmi,$Xmi,$Xmi
	vxorps		$Tred,$Xi,$Xi

	  vmovdqu	0x10($inp),$Ii		# I[1]
	vpalignr	\$8,$Xi,$Xi,$Tred	# 2nd phase
	 vpclmulqdq	\$0x00,$Hkey,$Ij,$Zlo
	  vpshufb	$bswap,$Ii,$Ii
	 vpxor		$Xlo,$Zlo,$Zlo
	 vpclmulqdq	\$0x11,$Hkey,$Ij,$Zhi
	  vmovdqu	0x90-0x40($Htbl),$Hkey	# $Hkey^7
	vpclmulqdq	\$0x10,(%r10),$Xi,$Xi
	vxorps		$Xo,$Tred,$Tred
	 vpunpckhqdq	$Ii,$Ii,$T2
	 vpxor		$Xhi,$Zhi,$Zhi
	 vpclmulqdq	\$0x10,$HK,  $T1,$Zmi
	  vmovdqu	0xb0-0x40($Htbl),$HK
	 vpxor		$Ii,$T2,$T2
	 vpxor		$Xmi,$Zmi,$Zmi

	  vmovdqu	($inp),$Ij		# I[0]
	 vpclmulqdq	\$0x00,$Hkey,$Ii,$Xlo
	  vpshufb	$bswap,$Ij,$Ij
	 vpclmulqdq	\$0x11,$Hkey,$Ii,$Xhi
	  vmovdqu	0xa0-0x40($Htbl),$Hkey	# $Hkey^8
	vpxor		$Tred,$Ij,$Ij
	 vpclmulqdq	\$0x10,$HK,  $T2,$Xmi
	vpxor		$Xi,$Ij,$Ij		# accumulate $Xi

	lea		0x80($inp),$inp
	sub		\$0x80,$len
	jnc		.Loop8x_avx

	add		\$0x80,$len
	jmp		.Ltail_no_xor_avx

.align	32
.Lshort_avx:
	vmovdqu		-0x10($inp,$len),$Ii	# very last word
	lea		($inp,$len),$inp
	vmovdqu		0x00-0x40($Htbl),$Hkey	# $Hkey^1
	vmovdqu		0x20-0x40($Htbl),$HK
	vpshufb		$bswap,$Ii,$Ij

	vmovdqa		$Xlo,$Zlo		# subtle way to zero $Zlo,
	vmovdqa		$Xhi,$Zhi		# $Zhi and
	vmovdqa		$Xmi,$Zmi		# $Zmi
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x20($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x10-0x40($Htbl),$Hkey	# $Hkey^2
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vpsrldq		\$8,$HK,$HK
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x30($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x30-0x40($Htbl),$Hkey	# $Hkey^3
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vmovdqu		0x50-0x40($Htbl),$HK
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x40($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x40-0x40($Htbl),$Hkey	# $Hkey^4
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vpsrldq		\$8,$HK,$HK
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x50($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x60-0x40($Htbl),$Hkey	# $Hkey^5
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vmovdqu		0x80-0x40($Htbl),$HK
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x60($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x70-0x40($Htbl),$Hkey	# $Hkey^6
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vpsrldq		\$8,$HK,$HK
	sub		\$0x10,$len
	jz		.Ltail_avx

	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	 vmovdqu	-0x70($inp),$Ii
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vmovdqu		0x90-0x40($Htbl),$Hkey	# $Hkey^7
	 vpshufb	$bswap,$Ii,$Ij
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi
	vmovq		0xb8-0x40($Htbl),$HK
	sub		\$0x10,$len
	jmp		.Ltail_avx

.align	32
.Ltail_avx:
	vpxor		$Xi,$Ij,$Ij		# accumulate $Xi
.Ltail_no_xor_avx:
	vpunpckhqdq	$Ij,$Ij,$T1
	vpxor		$Xlo,$Zlo,$Zlo
	vpclmulqdq	\$0x00,$Hkey,$Ij,$Xlo
	vpxor		$Ij,$T1,$T1
	vpxor		$Xhi,$Zhi,$Zhi
	vpclmulqdq	\$0x11,$Hkey,$Ij,$Xhi
	vpxor		$Xmi,$Zmi,$Zmi
	vpclmulqdq	\$0x00,$HK,$T1,$Xmi

	vmovdqu		(%r10),$Tred

	vpxor		$Xlo,$Zlo,$Xi
	vpxor		$Xhi,$Zhi,$Xo
	vpxor		$Xmi,$Zmi,$Zmi

	vpxor		$Xi, $Zmi,$Zmi		# aggregated Karatsuba post-processing
	vpxor		$Xo, $Zmi,$Zmi
	vpslldq		\$8, $Zmi,$T2
	vpsrldq		\$8, $Zmi,$Zmi
	vpxor		$T2, $Xi, $Xi
	vpxor		$Zmi,$Xo, $Xo

	vpclmulqdq	\$0x10,$Tred,$Xi,$T2	# 1st phase
	vpalignr	\$8,$Xi,$Xi,$Xi
	vpxor		$T2,$Xi,$Xi

	vpclmulqdq	\$0x10,$Tred,$Xi,$T2	# 2nd phase
	vpalignr	\$8,$Xi,$Xi,$Xi
	vpxor		$Xo,$Xi,$Xi
	vpxor		$T2,$Xi,$Xi

	cmp		\$0,$len
	jne		.Lshort_avx

	vpshufb		$bswap,$Xi,$Xi
	vmovdqu		$Xi,($Xip)
	vzeroupper
___
$code.=<<___ if ($win64);
	movaps	(%rsp),%xmm6
	movaps	0x10(%rsp),%xmm7
	movaps	0x20(%rsp),%xmm8
	movaps	0x30(%rsp),%xmm9
	movaps	0x40(%rsp),%xmm10
	movaps	0x50(%rsp),%xmm11
	movaps	0x60(%rsp),%xmm12
	movaps	0x70(%rsp),%xmm13
	movaps	0x80(%rsp),%xmm14
	movaps	0x90(%rsp),%xmm15
	lea	0xa8(%rsp),%rsp
___
$code.=<<___;
	ret
.cfi_endproc
.seh_endproc
.size	gcm_ghash_avx,.-gcm_ghash_avx
___
} else {
$code.=<<___;
	jmp	.L_ghash_clmul
.size	gcm_ghash_avx,.-gcm_ghash_avx
___
}

$code.=<<___;
.section .rodata
.align	64
.Lbswap_mask:
	.byte	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
.L0x1c2_polynomial:
	.byte	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2
.L7_mask:
	.long	7,0,7,0
.align	64

.asciz	"GHASH for x86_64, CRYPTOGAMS by <appro\@openssl.org>"
.align	64
.text
___

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
