#! /usr/bin/env perl
# Copyright 2013-2016 The OpenSSL Project Authors. All Rights Reserved.
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
#
# AES-NI-CTR+GHASH stitch.
#
# February 2013
#
# OpenSSL GCM implementation is organized in such way that its
# performance is rather close to the sum of its streamed components,
# in the context parallelized AES-NI CTR and modulo-scheduled
# PCLMULQDQ-enabled GHASH. Unfortunately, as no stitch implementation
# was observed to perform significantly better than the sum of the
# components on contemporary CPUs, the effort was deemed impossible to
# justify. This module is based on combination of Intel submissions,
# [1] and [2], with MOVBE twist suggested by Ilya Albrekht and Max
# Locktyukhin of Intel Corp. who verified that it reduces shuffles
# pressure with notable relative improvement, achieving 1.0 cycle per
# byte processed with 128-bit key on Haswell processor, 0.74 - on
# Broadwell, 0.63 - on Skylake... [Mentioned results are raw profiled
# measurements for favourable packet size, one divisible by 96.
# Applications using the EVP interface will observe a few percent
# worse performance.]
#
# Knights Landing processes 1 byte in 1.25 cycles (measured with EVP).
#
# [1] http://rt.openssl.org/Ticket/Display.html?id=2900&user=guest&pass=guest
# [2] http://www.intel.com/content/dam/www/public/us/en/documents/software-support/enabling-high-performance-gcm.pdf

$flavour = shift;
$output  = shift;
if ($flavour =~ /\./) { $output = $flavour; undef $flavour; }

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

# |$avx| in ghash-x86_64.pl must be set to at least 1; otherwise tags will
# be computed incorrectly.
#
# In upstream, this is controlled by shelling out to the compiler to check
# versions, but BoringSSL is intended to be used with pre-generated perlasm
# output, so this isn't useful anyway.
#
# The upstream code uses the condition |$avx>1| even though no AVX2
# instructions are used, because it assumes MOVBE is supported by the assembler
# if and only if AVX2 is also supported by the assembler; see
# https://marc.info/?l=openssl-dev&m=146567589526984&w=2.
$avx = 2;

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

# See the comment above regarding why the condition is ($avx>1) when there are
# no AVX2 instructions being used.
if ($avx>1) {{{

# On Windows, only four parameters are passed in registers. The last two
# parameters will be manually loaded into %rdi and %rsi.
my ($inp, $out, $len, $key, $ivp, $Htable) =
    $win64 ? ("%rcx", "%rdx", "%r8", "%r9", "%rdi", "%rsi") :
             ("%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9");

# The offset from %rbp to the Xip parameter. On Windows, all parameters have
# corresponding stack positions, not just ones passed on the stack.
# (0x40 = 6*8 + 0x10)
#
# Xip only needs to be accessed at the beginning and end of the function, and
# this function is short on registers, so we make it the last parameter for
# convenience.
my $Xip_offset = $win64 ? 0x40 : 0x10;

($Ii,$T1,$T2,$Hkey,
 $Z0,$Z1,$Z2,$Z3,$Xi) = map("%xmm$_",(0..8));

($inout0,$inout1,$inout2,$inout3,$inout4,$inout5,$rndkey) = map("%xmm$_",(9..15));

($counter,$rounds,$const,$in0,$end0)=("%ebx","%r10d","%r11","%r14","%r15");

$code=<<___;
.text

.type	_aesni_ctr32_ghash_6x,\@abi-omnipotent
.align	32
_aesni_ctr32_ghash_6x:
.cfi_startproc
	vmovdqu		0x20($const),$T2	# borrow $T2, .Lone_msb
	sub		\$6,$len
	vpxor		$Z0,$Z0,$Z0		# $Z0   = 0
	vmovdqu		0x00-0x80($key),$rndkey
	vpaddb		$T2,$T1,$inout1
	vpaddb		$T2,$inout1,$inout2
	vpaddb		$T2,$inout2,$inout3
	vpaddb		$T2,$inout3,$inout4
	vpaddb		$T2,$inout4,$inout5
	vpxor		$rndkey,$T1,$inout0
	vmovdqu		$Z0,16+8(%rsp)		# "$Z3" = 0
	jmp		.Loop6x

.align	32
.Loop6x:
	add		\$`6<<24`,$counter
	jc		.Lhandle_ctr32		# discard $inout[1-5]?
	vmovdqu		0x00-0x20($Htable),$Hkey	# $Hkey^1
	  vpaddb	$T2,$inout5,$T1		# next counter value
	  vpxor		$rndkey,$inout1,$inout1
	  vpxor		$rndkey,$inout2,$inout2

.Lresume_ctr32:
	vmovdqu		$T1,($ivp)		# save next counter value
	vpclmulqdq	\$0x10,$Hkey,$Z3,$Z1
	  vpxor		$rndkey,$inout3,$inout3
	  vmovups	0x10-0x80($key),$T2	# borrow $T2 for $rndkey
	vpclmulqdq	\$0x01,$Hkey,$Z3,$Z2

	# At this point, the current block of 96 (0x60) bytes has already been
	# loaded into registers. Concurrently with processing it, we want to
	# load the next 96 bytes of input for the next round. Obviously, we can
	# only do this if there are at least 96 more bytes of input beyond the
	# input we're currently processing, or else we'd read past the end of
	# the input buffer. Here, we set |%r12| to 96 if there are at least 96
	# bytes of input beyond the 96 bytes we're already processing, and we
	# set |%r12| to 0 otherwise. In the case where we set |%r12| to 96,
	# we'll read in the next block so that it is in registers for the next
	# loop iteration. In the case where we set |%r12| to 0, we'll re-read
	# the current block and then ignore what we re-read.
	#
	# At this point, |$in0| points to the current (already read into
	# registers) block, and |$end0| points to 2*96 bytes before the end of
	# the input. Thus, |$in0| > |$end0| means that we do not have the next
	# 96-byte block to read in, and |$in0| <= |$end0| means we do.
	xor		%r12,%r12
	cmp		$in0,$end0

	  vaesenc	$T2,$inout0,$inout0
	vmovdqu		0x30+8(%rsp),$Ii	# I[4]
	  vpxor		$rndkey,$inout4,$inout4
	vpclmulqdq	\$0x00,$Hkey,$Z3,$T1
	  vaesenc	$T2,$inout1,$inout1
	  vpxor		$rndkey,$inout5,$inout5
	setnc		%r12b
	vpclmulqdq	\$0x11,$Hkey,$Z3,$Z3
	  vaesenc	$T2,$inout2,$inout2
	vmovdqu		0x10-0x20($Htable),$Hkey	# $Hkey^2
	neg		%r12
	  vaesenc	$T2,$inout3,$inout3
	 vpxor		$Z1,$Z2,$Z2
	vpclmulqdq	\$0x00,$Hkey,$Ii,$Z1
	 vpxor		$Z0,$Xi,$Xi		# modulo-scheduled
	  vaesenc	$T2,$inout4,$inout4
	 vpxor		$Z1,$T1,$Z0
	and		\$0x60,%r12
	  vmovups	0x20-0x80($key),$rndkey
	vpclmulqdq	\$0x10,$Hkey,$Ii,$T1
	  vaesenc	$T2,$inout5,$inout5

	vpclmulqdq	\$0x01,$Hkey,$Ii,$T2
	lea		($in0,%r12),$in0
	  vaesenc	$rndkey,$inout0,$inout0
	 vpxor		16+8(%rsp),$Xi,$Xi	# modulo-scheduled [vpxor $Z3,$Xi,$Xi]
	vpclmulqdq	\$0x11,$Hkey,$Ii,$Hkey
	 vmovdqu	0x40+8(%rsp),$Ii	# I[3]
	  vaesenc	$rndkey,$inout1,$inout1
	movbe		0x58($in0),%r13
	  vaesenc	$rndkey,$inout2,$inout2
	movbe		0x50($in0),%r12
	  vaesenc	$rndkey,$inout3,$inout3
	mov		%r13,0x20+8(%rsp)
	  vaesenc	$rndkey,$inout4,$inout4
	mov		%r12,0x28+8(%rsp)
	vmovdqu		0x30-0x20($Htable),$Z1	# borrow $Z1 for $Hkey^3
	  vaesenc	$rndkey,$inout5,$inout5

	  vmovups	0x30-0x80($key),$rndkey
	 vpxor		$T1,$Z2,$Z2
	vpclmulqdq	\$0x00,$Z1,$Ii,$T1
	  vaesenc	$rndkey,$inout0,$inout0
	 vpxor		$T2,$Z2,$Z2
	vpclmulqdq	\$0x10,$Z1,$Ii,$T2
	  vaesenc	$rndkey,$inout1,$inout1
	 vpxor		$Hkey,$Z3,$Z3
	vpclmulqdq	\$0x01,$Z1,$Ii,$Hkey
	  vaesenc	$rndkey,$inout2,$inout2
	vpclmulqdq	\$0x11,$Z1,$Ii,$Z1
	 vmovdqu	0x50+8(%rsp),$Ii	# I[2]
	  vaesenc	$rndkey,$inout3,$inout3
	  vaesenc	$rndkey,$inout4,$inout4
	 vpxor		$T1,$Z0,$Z0
	vmovdqu		0x40-0x20($Htable),$T1	# borrow $T1 for $Hkey^4
	  vaesenc	$rndkey,$inout5,$inout5

	  vmovups	0x40-0x80($key),$rndkey
	 vpxor		$T2,$Z2,$Z2
	vpclmulqdq	\$0x00,$T1,$Ii,$T2
	  vaesenc	$rndkey,$inout0,$inout0
	 vpxor		$Hkey,$Z2,$Z2
	vpclmulqdq	\$0x10,$T1,$Ii,$Hkey
	  vaesenc	$rndkey,$inout1,$inout1
	movbe		0x48($in0),%r13
	 vpxor		$Z1,$Z3,$Z3
	vpclmulqdq	\$0x01,$T1,$Ii,$Z1
	  vaesenc	$rndkey,$inout2,$inout2
	movbe		0x40($in0),%r12
	vpclmulqdq	\$0x11,$T1,$Ii,$T1
	 vmovdqu	0x60+8(%rsp),$Ii	# I[1]
	  vaesenc	$rndkey,$inout3,$inout3
	mov		%r13,0x30+8(%rsp)
	  vaesenc	$rndkey,$inout4,$inout4
	mov		%r12,0x38+8(%rsp)
	 vpxor		$T2,$Z0,$Z0
	vmovdqu		0x60-0x20($Htable),$T2	# borrow $T2 for $Hkey^5
	  vaesenc	$rndkey,$inout5,$inout5

	  vmovups	0x50-0x80($key),$rndkey
	 vpxor		$Hkey,$Z2,$Z2
	vpclmulqdq	\$0x00,$T2,$Ii,$Hkey
	  vaesenc	$rndkey,$inout0,$inout0
	 vpxor		$Z1,$Z2,$Z2
	vpclmulqdq	\$0x10,$T2,$Ii,$Z1
	  vaesenc	$rndkey,$inout1,$inout1
	movbe		0x38($in0),%r13
	 vpxor		$T1,$Z3,$Z3
	vpclmulqdq	\$0x01,$T2,$Ii,$T1
	 vpxor		0x70+8(%rsp),$Xi,$Xi	# accumulate I[0]
	  vaesenc	$rndkey,$inout2,$inout2
	movbe		0x30($in0),%r12
	vpclmulqdq	\$0x11,$T2,$Ii,$T2
	  vaesenc	$rndkey,$inout3,$inout3
	mov		%r13,0x40+8(%rsp)
	  vaesenc	$rndkey,$inout4,$inout4
	mov		%r12,0x48+8(%rsp)
	 vpxor		$Hkey,$Z0,$Z0
	 vmovdqu	0x70-0x20($Htable),$Hkey	# $Hkey^6
	  vaesenc	$rndkey,$inout5,$inout5

	  vmovups	0x60-0x80($key),$rndkey
	 vpxor		$Z1,$Z2,$Z2
	vpclmulqdq	\$0x10,$Hkey,$Xi,$Z1
	  vaesenc	$rndkey,$inout0,$inout0
	 vpxor		$T1,$Z2,$Z2
	vpclmulqdq	\$0x01,$Hkey,$Xi,$T1
	  vaesenc	$rndkey,$inout1,$inout1
	movbe		0x28($in0),%r13
	 vpxor		$T2,$Z3,$Z3
	vpclmulqdq	\$0x00,$Hkey,$Xi,$T2
	  vaesenc	$rndkey,$inout2,$inout2
	movbe		0x20($in0),%r12
	vpclmulqdq	\$0x11,$Hkey,$Xi,$Xi
	  vaesenc	$rndkey,$inout3,$inout3
	mov		%r13,0x50+8(%rsp)
	  vaesenc	$rndkey,$inout4,$inout4
	mov		%r12,0x58+8(%rsp)
	vpxor		$Z1,$Z2,$Z2
	  vaesenc	$rndkey,$inout5,$inout5
	vpxor		$T1,$Z2,$Z2

	  vmovups	0x70-0x80($key),$rndkey
	vpslldq		\$8,$Z2,$Z1
	vpxor		$T2,$Z0,$Z0
	vmovdqu		0x10($const),$Hkey	# .Lpoly

	  vaesenc	$rndkey,$inout0,$inout0
	vpxor		$Xi,$Z3,$Z3
	  vaesenc	$rndkey,$inout1,$inout1
	vpxor		$Z1,$Z0,$Z0
	movbe		0x18($in0),%r13
	  vaesenc	$rndkey,$inout2,$inout2
	movbe		0x10($in0),%r12
	vpalignr	\$8,$Z0,$Z0,$Ii		# 1st phase
	vpclmulqdq	\$0x10,$Hkey,$Z0,$Z0
	mov		%r13,0x60+8(%rsp)
	  vaesenc	$rndkey,$inout3,$inout3
	mov		%r12,0x68+8(%rsp)
	  vaesenc	$rndkey,$inout4,$inout4
	  vmovups	0x80-0x80($key),$T1	# borrow $T1 for $rndkey
	  vaesenc	$rndkey,$inout5,$inout5

	  vaesenc	$T1,$inout0,$inout0
	  vmovups	0x90-0x80($key),$rndkey
	  vaesenc	$T1,$inout1,$inout1
	vpsrldq		\$8,$Z2,$Z2
	  vaesenc	$T1,$inout2,$inout2
	vpxor		$Z2,$Z3,$Z3
	  vaesenc	$T1,$inout3,$inout3
	vpxor		$Ii,$Z0,$Z0
	movbe		0x08($in0),%r13
	  vaesenc	$T1,$inout4,$inout4
	movbe		0x00($in0),%r12
	  vaesenc	$T1,$inout5,$inout5
	  vmovups	0xa0-0x80($key),$T1
	  cmp		\$11,$rounds
	  jb		.Lenc_tail		# 128-bit key

	  vaesenc	$rndkey,$inout0,$inout0
	  vaesenc	$rndkey,$inout1,$inout1
	  vaesenc	$rndkey,$inout2,$inout2
	  vaesenc	$rndkey,$inout3,$inout3
	  vaesenc	$rndkey,$inout4,$inout4
	  vaesenc	$rndkey,$inout5,$inout5

	  vaesenc	$T1,$inout0,$inout0
	  vaesenc	$T1,$inout1,$inout1
	  vaesenc	$T1,$inout2,$inout2
	  vaesenc	$T1,$inout3,$inout3
	  vaesenc	$T1,$inout4,$inout4
	  vmovups	0xb0-0x80($key),$rndkey
	  vaesenc	$T1,$inout5,$inout5
	  vmovups	0xc0-0x80($key),$T1
	  # 192-bit key support was removed.

	  vaesenc	$rndkey,$inout0,$inout0
	  vaesenc	$rndkey,$inout1,$inout1
	  vaesenc	$rndkey,$inout2,$inout2
	  vaesenc	$rndkey,$inout3,$inout3
	  vaesenc	$rndkey,$inout4,$inout4
	  vaesenc	$rndkey,$inout5,$inout5

	  vaesenc	$T1,$inout0,$inout0
	  vaesenc	$T1,$inout1,$inout1
	  vaesenc	$T1,$inout2,$inout2
	  vaesenc	$T1,$inout3,$inout3
	  vaesenc	$T1,$inout4,$inout4
	  vmovups	0xd0-0x80($key),$rndkey
	  vaesenc	$T1,$inout5,$inout5
	  vmovups	0xe0-0x80($key),$T1
	  jmp		.Lenc_tail		# 256-bit key

.align	32
.Lhandle_ctr32:
	vmovdqu		($const),$Ii		# borrow $Ii for .Lbswap_mask
	  vpshufb	$Ii,$T1,$Z2		# byte-swap counter
	  vmovdqu	0x30($const),$Z1	# borrow $Z1, .Ltwo_lsb
	  vpaddd	0x40($const),$Z2,$inout1	# .Lone_lsb
	  vpaddd	$Z1,$Z2,$inout2
	vmovdqu		0x00-0x20($Htable),$Hkey	# $Hkey^1
	  vpaddd	$Z1,$inout1,$inout3
	  vpshufb	$Ii,$inout1,$inout1
	  vpaddd	$Z1,$inout2,$inout4
	  vpshufb	$Ii,$inout2,$inout2
	  vpxor		$rndkey,$inout1,$inout1
	  vpaddd	$Z1,$inout3,$inout5
	  vpshufb	$Ii,$inout3,$inout3
	  vpxor		$rndkey,$inout2,$inout2
	  vpaddd	$Z1,$inout4,$T1		# byte-swapped next counter value
	  vpshufb	$Ii,$inout4,$inout4
	  vpshufb	$Ii,$inout5,$inout5
	  vpshufb	$Ii,$T1,$T1		# next counter value
	jmp		.Lresume_ctr32

.align	32
.Lenc_tail:
	  vaesenc	$rndkey,$inout0,$inout0
	vmovdqu		$Z3,16+8(%rsp)		# postpone vpxor $Z3,$Xi,$Xi
	vpalignr	\$8,$Z0,$Z0,$Xi		# 2nd phase
	  vaesenc	$rndkey,$inout1,$inout1
	vpclmulqdq	\$0x10,$Hkey,$Z0,$Z0
	  vpxor		0x00($inp),$T1,$T2
	  vaesenc	$rndkey,$inout2,$inout2
	  vpxor		0x10($inp),$T1,$Ii
	  vaesenc	$rndkey,$inout3,$inout3
	  vpxor		0x20($inp),$T1,$Z1
	  vaesenc	$rndkey,$inout4,$inout4
	  vpxor		0x30($inp),$T1,$Z2
	  vaesenc	$rndkey,$inout5,$inout5
	  vpxor		0x40($inp),$T1,$Z3
	  vpxor		0x50($inp),$T1,$Hkey
	  vmovdqu	($ivp),$T1		# load next counter value

	  vaesenclast	$T2,$inout0,$inout0
	  vmovdqu	0x20($const),$T2	# borrow $T2, .Lone_msb
	  vaesenclast	$Ii,$inout1,$inout1
	 vpaddb		$T2,$T1,$Ii
	mov		%r13,0x70+8(%rsp)
	lea		0x60($inp),$inp
	# These two prefetches were added in BoringSSL. See change that added them.
	 prefetcht0	512($inp)		# We use 96-byte block so prefetch 2 lines (128 bytes)
	 prefetcht0	576($inp)
	  vaesenclast	$Z1,$inout2,$inout2
	 vpaddb		$T2,$Ii,$Z1
	mov		%r12,0x78+8(%rsp)
	lea		0x60($out),$out
	  vmovdqu	0x00-0x80($key),$rndkey
	  vaesenclast	$Z2,$inout3,$inout3
	 vpaddb		$T2,$Z1,$Z2
	  vaesenclast	$Z3, $inout4,$inout4
	 vpaddb		$T2,$Z2,$Z3
	  vaesenclast	$Hkey,$inout5,$inout5
	 vpaddb		$T2,$Z3,$Hkey

	add		\$0x60,%rax
	sub		\$0x6,$len
	jc		.L6x_done

	  vmovups	$inout0,-0x60($out)	# save output
	 vpxor		$rndkey,$T1,$inout0
	  vmovups	$inout1,-0x50($out)
	 vmovdqa	$Ii,$inout1		# 0 latency
	  vmovups	$inout2,-0x40($out)
	 vmovdqa	$Z1,$inout2		# 0 latency
	  vmovups	$inout3,-0x30($out)
	 vmovdqa	$Z2,$inout3		# 0 latency
	  vmovups	$inout4,-0x20($out)
	 vmovdqa	$Z3,$inout4		# 0 latency
	  vmovups	$inout5,-0x10($out)
	 vmovdqa	$Hkey,$inout5		# 0 latency
	vmovdqu		0x20+8(%rsp),$Z3	# I[5]
	jmp		.Loop6x

.L6x_done:
	vpxor		16+8(%rsp),$Xi,$Xi	# modulo-scheduled
	vpxor		$Z0,$Xi,$Xi		# modulo-scheduled

	ret
.cfi_endproc
.size	_aesni_ctr32_ghash_6x,.-_aesni_ctr32_ghash_6x
___
######################################################################
#
# size_t aesni_gcm_[en|de]crypt(const void *inp, void *out, size_t len,
#		const AES_KEY *key, unsigned char iv[16], const u128 Htbl[9],
#		u128 *Xip);
$code.=<<___;
.globl	aesni_gcm_decrypt
.type	aesni_gcm_decrypt,\@abi-omnipotent
.align	32
aesni_gcm_decrypt:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
	xor	%rax,%rax

	# We call |_aesni_ctr32_ghash_6x|, which requires at least 96 (0x60)
	# bytes of input.
	cmp	\$0x60,$len			# minimal accepted length
	jb	.Lgcm_dec_abort

	push	%rbp
.cfi_push	%rbp
.seh_pushreg	%rbp
	mov	%rsp, %rbp			# save stack pointer
.cfi_def_cfa_register	%rbp
	push	%rbx
.cfi_push	%rbx
.seh_pushreg	%rbx
	push	%r12
.cfi_push	%r12
.seh_pushreg	%r12
	push	%r13
.cfi_push	%r13
.seh_pushreg	%r13
	push	%r14
.cfi_push	%r14
.seh_pushreg	%r14
	push	%r15
.cfi_push	%r15
.seh_pushreg	%r15
___
if ($win64) {
$code.=<<___
	lea	-0xa8(%rsp),%rsp		# 8 extra bytes to align the stack
.seh_stackalloc	0xa8
.seh_setframe	%rbp, 0xa8+5*8
	# Load the last two parameters. These go into %rdi and %rsi, which are
	# non-volatile on Windows, so stash them in the parameter stack area
	# first.
	mov	%rdi, 0x10(%rbp)
.seh_savereg	%rdi, 0xa8+5*8+0x10
	mov	%rsi, 0x18(%rbp)
.seh_savereg	%rsi, 0xa8+5*8+0x18
	mov	0x30(%rbp), $ivp
	mov	0x38(%rbp), $Htable
	# Save non-volatile XMM registers.
	movaps	%xmm6,-0xd0(%rbp)
.seh_savexmm	%xmm6, 0xa8+5*8-0xd0
	movaps	%xmm7,-0xc0(%rbp)
.seh_savexmm	%xmm7, 0xa8+5*8-0xc0
	movaps	%xmm8,-0xb0(%rbp)
.seh_savexmm	%xmm8, 0xa8+5*8-0xb0
	movaps	%xmm9,-0xa0(%rbp)
.seh_savexmm	%xmm9, 0xa8+5*8-0xa0
	movaps	%xmm10,-0x90(%rbp)
.seh_savexmm	%xmm10, 0xa8+5*8-0x90
	movaps	%xmm11,-0x80(%rbp)
.seh_savexmm	%xmm11, 0xa8+5*8-0x80
	movaps	%xmm12,-0x70(%rbp)
.seh_savexmm	%xmm12, 0xa8+5*8-0x70
	movaps	%xmm13,-0x60(%rbp)
.seh_savexmm	%xmm13, 0xa8+5*8-0x60
	movaps	%xmm14,-0x50(%rbp)
.seh_savexmm	%xmm14, 0xa8+5*8-0x50
	movaps	%xmm15,-0x40(%rbp)
.seh_savexmm	%xmm15, 0xa8+5*8-0x40
.seh_endprologue
___
}
$code.=<<___;
	vzeroupper

	mov		$Xip_offset(%rbp), %r12
	vmovdqu		($ivp),$T1		# input counter value
	add		\$-128,%rsp
	mov		12($ivp),$counter
	lea		.Lbswap_mask(%rip),$const
	lea		-0x80($key),$in0	# borrow $in0
	mov		\$0xf80,$end0		# borrow $end0
	vmovdqu		(%r12),$Xi		# load Xi
	and		\$-128,%rsp		# ensure stack alignment
	vmovdqu		($const),$Ii		# borrow $Ii for .Lbswap_mask
	lea		0x80($key),$key		# size optimization
	lea		0x20($Htable),$Htable	# size optimization
	mov		0xf0-0x80($key),$rounds
	vpshufb		$Ii,$Xi,$Xi

	and		$end0,$in0
	and		%rsp,$end0
	sub		$in0,$end0
	jc		.Ldec_no_key_aliasing
	cmp		\$768,$end0
	jnc		.Ldec_no_key_aliasing
	sub		$end0,%rsp		# avoid aliasing with key
.Ldec_no_key_aliasing:

	vmovdqu		0x50($inp),$Z3		# I[5]
	mov		$inp,$in0
	vmovdqu		0x40($inp),$Z0

	# |_aesni_ctr32_ghash_6x| requires |$end0| to point to 2*96 (0xc0)
	# bytes before the end of the input. Note, in particular, that this is
	# correct even if |$len| is not an even multiple of 96 or 16. XXX: This
	# seems to require that |$inp| + |$len| >= 2*96 (0xc0); i.e. |$inp| must
	# not be near the very beginning of the address space when |$len| < 2*96
	# (0xc0).
	lea		-0xc0($inp,$len),$end0

	vmovdqu		0x30($inp),$Z1
	shr		\$4,$len
	xor		%rax,%rax
	vmovdqu		0x20($inp),$Z2
	 vpshufb	$Ii,$Z3,$Z3		# passed to _aesni_ctr32_ghash_6x
	vmovdqu		0x10($inp),$T2
	 vpshufb	$Ii,$Z0,$Z0
	vmovdqu		($inp),$Hkey
	 vpshufb	$Ii,$Z1,$Z1
	vmovdqu		$Z0,0x30(%rsp)
	 vpshufb	$Ii,$Z2,$Z2
	vmovdqu		$Z1,0x40(%rsp)
	 vpshufb	$Ii,$T2,$T2
	vmovdqu		$Z2,0x50(%rsp)
	 vpshufb	$Ii,$Hkey,$Hkey
	vmovdqu		$T2,0x60(%rsp)
	vmovdqu		$Hkey,0x70(%rsp)

	call		_aesni_ctr32_ghash_6x

	mov		$Xip_offset(%rbp), %r12
	vmovups		$inout0,-0x60($out)	# save output
	vmovups		$inout1,-0x50($out)
	vmovups		$inout2,-0x40($out)
	vmovups		$inout3,-0x30($out)
	vmovups		$inout4,-0x20($out)
	vmovups		$inout5,-0x10($out)

	vpshufb		($const),$Xi,$Xi	# .Lbswap_mask
	vmovdqu		$Xi,(%r12)		# output Xi

	vzeroupper
___
$code.=<<___ if ($win64);
	movaps	-0xd0(%rbp),%xmm6
	movaps	-0xc0(%rbp),%xmm7
	movaps	-0xb0(%rbp),%xmm8
	movaps	-0xa0(%rbp),%xmm9
	movaps	-0x90(%rbp),%xmm10
	movaps	-0x80(%rbp),%xmm11
	movaps	-0x70(%rbp),%xmm12
	movaps	-0x60(%rbp),%xmm13
	movaps	-0x50(%rbp),%xmm14
	movaps	-0x40(%rbp),%xmm15
	mov	0x10(%rbp),%rdi
	mov	0x18(%rbp),%rsi
___
$code.=<<___;
	lea	-0x28(%rbp), %rsp	# restore %rsp to fixed allocation
.cfi_def_cfa	%rsp, 0x38
	pop	%r15
.cfi_pop	%r15
	pop	%r14
.cfi_pop	%r14
	pop	%r13
.cfi_pop	%r13
	pop	%r12
.cfi_pop	%r12
	pop	%rbx
.cfi_pop	%rbx
	pop	%rbp
.cfi_pop	%rbp
.Lgcm_dec_abort:
	ret
.seh_endproc
.cfi_endproc
.size	aesni_gcm_decrypt,.-aesni_gcm_decrypt
___

$code.=<<___;
.type	_aesni_ctr32_6x,\@abi-omnipotent
.align	32
_aesni_ctr32_6x:
.cfi_startproc
	vmovdqu		0x00-0x80($key),$Z0	# borrow $Z0 for $rndkey
	vmovdqu		0x20($const),$T2	# borrow $T2, .Lone_msb
	lea		-1($rounds),%r13
	vmovups		0x10-0x80($key),$rndkey
	lea		0x20-0x80($key),%r12
	vpxor		$Z0,$T1,$inout0
	add		\$`6<<24`,$counter
	jc		.Lhandle_ctr32_2
	vpaddb		$T2,$T1,$inout1
	vpaddb		$T2,$inout1,$inout2
	vpxor		$Z0,$inout1,$inout1
	vpaddb		$T2,$inout2,$inout3
	vpxor		$Z0,$inout2,$inout2
	vpaddb		$T2,$inout3,$inout4
	vpxor		$Z0,$inout3,$inout3
	vpaddb		$T2,$inout4,$inout5
	vpxor		$Z0,$inout4,$inout4
	vpaddb		$T2,$inout5,$T1
	vpxor		$Z0,$inout5,$inout5
	jmp		.Loop_ctr32

.align	16
.Loop_ctr32:
	vaesenc		$rndkey,$inout0,$inout0
	vaesenc		$rndkey,$inout1,$inout1
	vaesenc		$rndkey,$inout2,$inout2
	vaesenc		$rndkey,$inout3,$inout3
	vaesenc		$rndkey,$inout4,$inout4
	vaesenc		$rndkey,$inout5,$inout5
	vmovups		(%r12),$rndkey
	lea		0x10(%r12),%r12
	dec		%r13d
	jnz		.Loop_ctr32

	vmovdqu		(%r12),$Hkey		# last round key
	vaesenc		$rndkey,$inout0,$inout0
	vpxor		0x00($inp),$Hkey,$Z0
	vaesenc		$rndkey,$inout1,$inout1
	vpxor		0x10($inp),$Hkey,$Z1
	vaesenc		$rndkey,$inout2,$inout2
	vpxor		0x20($inp),$Hkey,$Z2
	vaesenc		$rndkey,$inout3,$inout3
	vpxor		0x30($inp),$Hkey,$Xi
	vaesenc		$rndkey,$inout4,$inout4
	vpxor		0x40($inp),$Hkey,$T2
	vaesenc		$rndkey,$inout5,$inout5
	vpxor		0x50($inp),$Hkey,$Hkey
	lea		0x60($inp),$inp

	vaesenclast	$Z0,$inout0,$inout0
	vaesenclast	$Z1,$inout1,$inout1
	vaesenclast	$Z2,$inout2,$inout2
	vaesenclast	$Xi,$inout3,$inout3
	vaesenclast	$T2,$inout4,$inout4
	vaesenclast	$Hkey,$inout5,$inout5
	vmovups		$inout0,0x00($out)
	vmovups		$inout1,0x10($out)
	vmovups		$inout2,0x20($out)
	vmovups		$inout3,0x30($out)
	vmovups		$inout4,0x40($out)
	vmovups		$inout5,0x50($out)
	lea		0x60($out),$out

	ret
.align	32
.Lhandle_ctr32_2:
	vpshufb		$Ii,$T1,$Z2		# byte-swap counter
	vmovdqu		0x30($const),$Z1	# borrow $Z1, .Ltwo_lsb
	vpaddd		0x40($const),$Z2,$inout1	# .Lone_lsb
	vpaddd		$Z1,$Z2,$inout2
	vpaddd		$Z1,$inout1,$inout3
	vpshufb		$Ii,$inout1,$inout1
	vpaddd		$Z1,$inout2,$inout4
	vpshufb		$Ii,$inout2,$inout2
	vpxor		$Z0,$inout1,$inout1
	vpaddd		$Z1,$inout3,$inout5
	vpshufb		$Ii,$inout3,$inout3
	vpxor		$Z0,$inout2,$inout2
	vpaddd		$Z1,$inout4,$T1		# byte-swapped next counter value
	vpshufb		$Ii,$inout4,$inout4
	vpxor		$Z0,$inout3,$inout3
	vpshufb		$Ii,$inout5,$inout5
	vpxor		$Z0,$inout4,$inout4
	vpshufb		$Ii,$T1,$T1		# next counter value
	vpxor		$Z0,$inout5,$inout5
	jmp	.Loop_ctr32
.cfi_endproc
.size	_aesni_ctr32_6x,.-_aesni_ctr32_6x

.globl	aesni_gcm_encrypt
.type	aesni_gcm_encrypt,\@abi-omnipotent
.align	32
aesni_gcm_encrypt:
.cfi_startproc
.seh_startproc
	_CET_ENDBR
#ifdef BORINGSSL_DISPATCH_TEST
.extern	BORINGSSL_function_hit
	movb \$1,BORINGSSL_function_hit+2(%rip)
#endif
	xor	%rax,%rax

	# We call |_aesni_ctr32_6x| twice, each call consuming 96 bytes of
	# input. Then we call |_aesni_ctr32_ghash_6x|, which requires at
	# least 96 more bytes of input.
	cmp	\$0x60*3,$len			# minimal accepted length
	jb	.Lgcm_enc_abort

	push	%rbp
.cfi_push	%rbp
.seh_pushreg	%rbp
	mov	%rsp, %rbp			# save stack pointer
.cfi_def_cfa_register	%rbp
	push	%rbx
.cfi_push	%rbx
.seh_pushreg	%rbx
	push	%r12
.cfi_push	%r12
.seh_pushreg	%r12
	push	%r13
.cfi_push	%r13
.seh_pushreg	%r13
	push	%r14
.cfi_push	%r14
.seh_pushreg	%r14
	push	%r15
.cfi_push	%r15
.seh_pushreg	%r15
___
if ($win64) {
$code.=<<___
	lea	-0xa8(%rsp),%rsp		# 8 extra bytes to align the stack
.seh_stackalloc	0xa8
.seh_setframe	%rbp, 0xa8+5*8
	# Load the last two parameters. These go into %rdi and %rsi, which are
	# non-volatile on Windows, so stash them in the parameter stack area
	# first.
	mov	%rdi, 0x10(%rbp)
.seh_savereg	%rdi, 0xa8+5*8+0x10
	mov	%rsi, 0x18(%rbp)
.seh_savereg	%rsi, 0xa8+5*8+0x18
	mov	0x30(%rbp), $ivp
	mov	0x38(%rbp), $Htable
	# Save non-volatile XMM registers.
	movaps	%xmm6,-0xd0(%rbp)
.seh_savexmm	%xmm6, 0xa8+5*8-0xd0
	movaps	%xmm7,-0xc0(%rbp)
.seh_savexmm	%xmm7, 0xa8+5*8-0xc0
	movaps	%xmm8,-0xb0(%rbp)
.seh_savexmm	%xmm8, 0xa8+5*8-0xb0
	movaps	%xmm9,-0xa0(%rbp)
.seh_savexmm	%xmm9, 0xa8+5*8-0xa0
	movaps	%xmm10,-0x90(%rbp)
.seh_savexmm	%xmm10, 0xa8+5*8-0x90
	movaps	%xmm11,-0x80(%rbp)
.seh_savexmm	%xmm11, 0xa8+5*8-0x80
	movaps	%xmm12,-0x70(%rbp)
.seh_savexmm	%xmm12, 0xa8+5*8-0x70
	movaps	%xmm13,-0x60(%rbp)
.seh_savexmm	%xmm13, 0xa8+5*8-0x60
	movaps	%xmm14,-0x50(%rbp)
.seh_savexmm	%xmm14, 0xa8+5*8-0x50
	movaps	%xmm15,-0x40(%rbp)
.seh_savexmm	%xmm15, 0xa8+5*8-0x40
.seh_endprologue
___
}
$code.=<<___;
	vzeroupper

	vmovdqu		($ivp),$T1		# input counter value
	add		\$-128,%rsp
	mov		12($ivp),$counter
	lea		.Lbswap_mask(%rip),$const
	lea		-0x80($key),$in0	# borrow $in0
	mov		\$0xf80,$end0		# borrow $end0
	lea		0x80($key),$key		# size optimization
	vmovdqu		($const),$Ii		# borrow $Ii for .Lbswap_mask
	and		\$-128,%rsp		# ensure stack alignment
	mov		0xf0-0x80($key),$rounds

	and		$end0,$in0
	and		%rsp,$end0
	sub		$in0,$end0
	jc		.Lenc_no_key_aliasing
	cmp		\$768,$end0
	jnc		.Lenc_no_key_aliasing
	sub		$end0,%rsp		# avoid aliasing with key
.Lenc_no_key_aliasing:

	mov		$out,$in0

	# |_aesni_ctr32_ghash_6x| requires |$end0| to point to 2*96 (0xc0)
	# bytes before the end of the input. Note, in particular, that this is
	# correct even if |$len| is not an even multiple of 96 or 16. Unlike in
	# the decryption case, there's no caveat that |$out| must not be near
	# the very beginning of the address space, because we know that
	# |$len| >= 3*96 from the check above, and so we know
	# |$out| + |$len| >= 2*96 (0xc0).
	lea		-0xc0($out,$len),$end0

	shr		\$4,$len

	call		_aesni_ctr32_6x
	vpshufb		$Ii,$inout0,$Xi		# save bswapped output on stack
	vpshufb		$Ii,$inout1,$T2
	vmovdqu		$Xi,0x70(%rsp)
	vpshufb		$Ii,$inout2,$Z0
	vmovdqu		$T2,0x60(%rsp)
	vpshufb		$Ii,$inout3,$Z1
	vmovdqu		$Z0,0x50(%rsp)
	vpshufb		$Ii,$inout4,$Z2
	vmovdqu		$Z1,0x40(%rsp)
	vpshufb		$Ii,$inout5,$Z3		# passed to _aesni_ctr32_ghash_6x
	vmovdqu		$Z2,0x30(%rsp)

	call		_aesni_ctr32_6x

	mov		$Xip_offset(%rbp), %r12
	lea		0x20($Htable),$Htable	# size optimization
	vmovdqu		(%r12),$Xi		# load Xi
	sub		\$12,$len
	mov		\$0x60*2,%rax
	vpshufb		$Ii,$Xi,$Xi

	call		_aesni_ctr32_ghash_6x
	vmovdqu		0x20(%rsp),$Z3		# I[5]
	 vmovdqu	($const),$Ii		# borrow $Ii for .Lbswap_mask
	vmovdqu		0x00-0x20($Htable),$Hkey	# $Hkey^1
	vpunpckhqdq	$Z3,$Z3,$T1
	vmovdqu		0x20-0x20($Htable),$rndkey	# borrow $rndkey for $HK
	 vmovups	$inout0,-0x60($out)	# save output
	 vpshufb	$Ii,$inout0,$inout0	# but keep bswapped copy
	vpxor		$Z3,$T1,$T1
	 vmovups	$inout1,-0x50($out)
	 vpshufb	$Ii,$inout1,$inout1
	 vmovups	$inout2,-0x40($out)
	 vpshufb	$Ii,$inout2,$inout2
	 vmovups	$inout3,-0x30($out)
	 vpshufb	$Ii,$inout3,$inout3
	 vmovups	$inout4,-0x20($out)
	 vpshufb	$Ii,$inout4,$inout4
	 vmovups	$inout5,-0x10($out)
	 vpshufb	$Ii,$inout5,$inout5
	 vmovdqu	$inout0,0x10(%rsp)	# free $inout0
___
{ my ($HK,$T3)=($rndkey,$inout0);

$code.=<<___;
	 vmovdqu	0x30(%rsp),$Z2		# I[4]
	 vmovdqu	0x10-0x20($Htable),$Ii	# borrow $Ii for $Hkey^2
	 vpunpckhqdq	$Z2,$Z2,$T2
	vpclmulqdq	\$0x00,$Hkey,$Z3,$Z1
	 vpxor		$Z2,$T2,$T2
	vpclmulqdq	\$0x11,$Hkey,$Z3,$Z3
	vpclmulqdq	\$0x00,$HK,$T1,$T1

	 vmovdqu	0x40(%rsp),$T3		# I[3]
	vpclmulqdq	\$0x00,$Ii,$Z2,$Z0
	 vmovdqu	0x30-0x20($Htable),$Hkey	# $Hkey^3
	vpxor		$Z1,$Z0,$Z0
	 vpunpckhqdq	$T3,$T3,$Z1
	vpclmulqdq	\$0x11,$Ii,$Z2,$Z2
	 vpxor		$T3,$Z1,$Z1
	vpxor		$Z3,$Z2,$Z2
	vpclmulqdq	\$0x10,$HK,$T2,$T2
	 vmovdqu	0x50-0x20($Htable),$HK
	vpxor		$T1,$T2,$T2

	 vmovdqu	0x50(%rsp),$T1		# I[2]
	vpclmulqdq	\$0x00,$Hkey,$T3,$Z3
	 vmovdqu	0x40-0x20($Htable),$Ii	# borrow $Ii for $Hkey^4
	vpxor		$Z0,$Z3,$Z3
	 vpunpckhqdq	$T1,$T1,$Z0
	vpclmulqdq	\$0x11,$Hkey,$T3,$T3
	 vpxor		$T1,$Z0,$Z0
	vpxor		$Z2,$T3,$T3
	vpclmulqdq	\$0x00,$HK,$Z1,$Z1
	vpxor		$T2,$Z1,$Z1

	 vmovdqu	0x60(%rsp),$T2		# I[1]
	vpclmulqdq	\$0x00,$Ii,$T1,$Z2
	 vmovdqu	0x60-0x20($Htable),$Hkey	# $Hkey^5
	vpxor		$Z3,$Z2,$Z2
	 vpunpckhqdq	$T2,$T2,$Z3
	vpclmulqdq	\$0x11,$Ii,$T1,$T1
	 vpxor		$T2,$Z3,$Z3
	vpxor		$T3,$T1,$T1
	vpclmulqdq	\$0x10,$HK,$Z0,$Z0
	 vmovdqu	0x80-0x20($Htable),$HK
	vpxor		$Z1,$Z0,$Z0

	 vpxor		0x70(%rsp),$Xi,$Xi	# accumulate I[0]
	vpclmulqdq	\$0x00,$Hkey,$T2,$Z1
	 vmovdqu	0x70-0x20($Htable),$Ii	# borrow $Ii for $Hkey^6
	 vpunpckhqdq	$Xi,$Xi,$T3
	vpxor		$Z2,$Z1,$Z1
	vpclmulqdq	\$0x11,$Hkey,$T2,$T2
	 vpxor		$Xi,$T3,$T3
	vpxor		$T1,$T2,$T2
	vpclmulqdq	\$0x00,$HK,$Z3,$Z3
	vpxor		$Z0,$Z3,$Z0

	vpclmulqdq	\$0x00,$Ii,$Xi,$Z2
	 vmovdqu	0x00-0x20($Htable),$Hkey	# $Hkey^1
	 vpunpckhqdq	$inout5,$inout5,$T1
	vpclmulqdq	\$0x11,$Ii,$Xi,$Xi
	 vpxor		$inout5,$T1,$T1
	vpxor		$Z1,$Z2,$Z1
	vpclmulqdq	\$0x10,$HK,$T3,$T3
	 vmovdqu	0x20-0x20($Htable),$HK
	vpxor		$T2,$Xi,$Z3
	vpxor		$Z0,$T3,$Z2

	 vmovdqu	0x10-0x20($Htable),$Ii	# borrow $Ii for $Hkey^2
	  vpxor		$Z1,$Z3,$T3		# aggregated Karatsuba post-processing
	vpclmulqdq	\$0x00,$Hkey,$inout5,$Z0
	  vpxor		$T3,$Z2,$Z2
	 vpunpckhqdq	$inout4,$inout4,$T2
	vpclmulqdq	\$0x11,$Hkey,$inout5,$inout5
	 vpxor		$inout4,$T2,$T2
	  vpslldq	\$8,$Z2,$T3
	vpclmulqdq	\$0x00,$HK,$T1,$T1
	  vpxor		$T3,$Z1,$Xi
	  vpsrldq	\$8,$Z2,$Z2
	  vpxor		$Z2,$Z3,$Z3

	vpclmulqdq	\$0x00,$Ii,$inout4,$Z1
	 vmovdqu	0x30-0x20($Htable),$Hkey	# $Hkey^3
	vpxor		$Z0,$Z1,$Z1
	 vpunpckhqdq	$inout3,$inout3,$T3
	vpclmulqdq	\$0x11,$Ii,$inout4,$inout4
	 vpxor		$inout3,$T3,$T3
	vpxor		$inout5,$inout4,$inout4
	  vpalignr	\$8,$Xi,$Xi,$inout5	# 1st phase
	vpclmulqdq	\$0x10,$HK,$T2,$T2
	 vmovdqu	0x50-0x20($Htable),$HK
	vpxor		$T1,$T2,$T2

	vpclmulqdq	\$0x00,$Hkey,$inout3,$Z0
	 vmovdqu	0x40-0x20($Htable),$Ii	# borrow $Ii for $Hkey^4
	vpxor		$Z1,$Z0,$Z0
	 vpunpckhqdq	$inout2,$inout2,$T1
	vpclmulqdq	\$0x11,$Hkey,$inout3,$inout3
	 vpxor		$inout2,$T1,$T1
	vpxor		$inout4,$inout3,$inout3
	  vxorps	0x10(%rsp),$Z3,$Z3	# accumulate $inout0
	vpclmulqdq	\$0x00,$HK,$T3,$T3
	vpxor		$T2,$T3,$T3

	  vpclmulqdq	\$0x10,0x10($const),$Xi,$Xi
	  vxorps	$inout5,$Xi,$Xi

	vpclmulqdq	\$0x00,$Ii,$inout2,$Z1
	 vmovdqu	0x60-0x20($Htable),$Hkey	# $Hkey^5
	vpxor		$Z0,$Z1,$Z1
	 vpunpckhqdq	$inout1,$inout1,$T2
	vpclmulqdq	\$0x11,$Ii,$inout2,$inout2
	 vpxor		$inout1,$T2,$T2
	  vpalignr	\$8,$Xi,$Xi,$inout5	# 2nd phase
	vpxor		$inout3,$inout2,$inout2
	vpclmulqdq	\$0x10,$HK,$T1,$T1
	 vmovdqu	0x80-0x20($Htable),$HK
	vpxor		$T3,$T1,$T1

	  vxorps	$Z3,$inout5,$inout5
	  vpclmulqdq	\$0x10,0x10($const),$Xi,$Xi
	  vxorps	$inout5,$Xi,$Xi

	vpclmulqdq	\$0x00,$Hkey,$inout1,$Z0
	 vmovdqu	0x70-0x20($Htable),$Ii	# borrow $Ii for $Hkey^6
	vpxor		$Z1,$Z0,$Z0
	 vpunpckhqdq	$Xi,$Xi,$T3
	vpclmulqdq	\$0x11,$Hkey,$inout1,$inout1
	 vpxor		$Xi,$T3,$T3
	vpxor		$inout2,$inout1,$inout1
	vpclmulqdq	\$0x00,$HK,$T2,$T2
	vpxor		$T1,$T2,$T2

	vpclmulqdq	\$0x00,$Ii,$Xi,$Z1
	vpclmulqdq	\$0x11,$Ii,$Xi,$Z3
	vpxor		$Z0,$Z1,$Z1
	vpclmulqdq	\$0x10,$HK,$T3,$Z2
	vpxor		$inout1,$Z3,$Z3
	vpxor		$T2,$Z2,$Z2

	vpxor		$Z1,$Z3,$Z0		# aggregated Karatsuba post-processing
	vpxor		$Z0,$Z2,$Z2
	vpslldq		\$8,$Z2,$T1
	vmovdqu		0x10($const),$Hkey	# .Lpoly
	vpsrldq		\$8,$Z2,$Z2
	vpxor		$T1,$Z1,$Xi
	vpxor		$Z2,$Z3,$Z3

	vpalignr	\$8,$Xi,$Xi,$T2		# 1st phase
	vpclmulqdq	\$0x10,$Hkey,$Xi,$Xi
	vpxor		$T2,$Xi,$Xi

	vpalignr	\$8,$Xi,$Xi,$T2		# 2nd phase
	vpclmulqdq	\$0x10,$Hkey,$Xi,$Xi
	vpxor		$Z3,$T2,$T2
	vpxor		$T2,$Xi,$Xi
___
}
$code.=<<___;
	mov		$Xip_offset(%rbp), %r12
	vpshufb		($const),$Xi,$Xi	# .Lbswap_mask
	vmovdqu		$Xi,(%r12)		# output Xi

	vzeroupper
___
$code.=<<___ if ($win64);
	movaps	-0xd0(%rbp),%xmm6
	movaps	-0xc0(%rbp),%xmm7
	movaps	-0xb0(%rbp),%xmm8
	movaps	-0xa0(%rbp),%xmm9
	movaps	-0x90(%rbp),%xmm10
	movaps	-0x80(%rbp),%xmm11
	movaps	-0x70(%rbp),%xmm12
	movaps	-0x60(%rbp),%xmm13
	movaps	-0x50(%rbp),%xmm14
	movaps	-0x40(%rbp),%xmm15
	mov	0x10(%rbp),%rdi
	mov	0x18(%rbp),%rsi
___
$code.=<<___;
	lea	-0x28(%rbp), %rsp	# restore %rsp to fixed allocation
.cfi_def_cfa	%rsp, 0x38
	pop	%r15
.cfi_pop	%r15
	pop	%r14
.cfi_pop	%r14
	pop	%r13
.cfi_pop	%r13
	pop	%r12
.cfi_pop	%r12
	pop	%rbx
.cfi_pop	%rbx
	pop	%rbp
.cfi_pop	%rbp
.Lgcm_enc_abort:
	ret
.seh_endproc
.cfi_endproc
.size	aesni_gcm_encrypt,.-aesni_gcm_encrypt
___

$code.=<<___;
.section .rodata
.align	64
.Lbswap_mask:
	.byte	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
.Lpoly:
	.byte	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2
.Lone_msb:
	.byte	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1
.Ltwo_lsb:
	.byte	2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
.Lone_lsb:
	.byte	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
.asciz	"AES-NI GCM module for x86_64, CRYPTOGAMS by <appro\@openssl.org>"
.align	64
.text
___
}}} else {{{
$code=<<___;	# assembler is too old
.text

.globl	aesni_gcm_encrypt
.type	aesni_gcm_encrypt,\@abi-omnipotent
aesni_gcm_encrypt:
	_CET_ENDBR
	xor	%eax,%eax
	ret
.size	aesni_gcm_encrypt,.-aesni_gcm_encrypt

.globl	aesni_gcm_decrypt
.type	aesni_gcm_decrypt,\@abi-omnipotent
aesni_gcm_decrypt:
	_CET_ENDBR
	xor	%eax,%eax
	ret
.size	aesni_gcm_decrypt,.-aesni_gcm_decrypt
___
}}}

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
