#! /usr/bin/env perl
# Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
#========================================================================
# Written by Xiaokang Qian <xiaokang.qian@arm.com> for the OpenSSL project,
# derived from https://github.com/ARM-software/AArch64cryptolib, original
# author Samuel Lee <Samuel.Lee@arm.com>. The module is, however, dual
# licensed under OpenSSL and SPDX BSD-3-Clause licenses depending on where you
# obtain it.
#========================================================================
#
# Approach - We want to reload constants as we have plenty of spare ASIMD slots around crypto units for loading
# Unroll x8 in main loop, main loop to act on 8 16B blocks per iteration, and then do modulo of the accumulated
# intermediate hashesfrom the 8 blocks.
#
#  ____________________________________________________
# |                                                    |
# | PRE                                                |
# |____________________________________________________|
# |                |                |                  |
# | CTR block 8k+13| AES block 8k+8 | GHASH block 8k+0 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+14| AES block 8k+9 | GHASH block 8k+1 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+15| AES block 8k+10| GHASH block 8k+2 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+16| AES block 8k+11| GHASH block 8k+3 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+17| AES block 8k+12| GHASH block 8k+4 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+18| AES block 8k+13| GHASH block 8k+5 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+19| AES block 8k+14| GHASH block 8k+6 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 8k+20| AES block 8k+15| GHASH block 8k+7 |
# |________________|____(mostly)____|__________________|
# |                                                    |
# | MODULO                                             |
# |____________________________________________________|
#
# PRE:
#     Ensure previous generated intermediate hash is aligned and merged with result for GHASH 4k+0
# EXT low_acc, low_acc, low_acc, #8
# EOR res_curr (8k+0), res_curr (4k+0), low_acc
#
# CTR block:
#     Increment and byte reverse counter in scalar registers and transfer to SIMD registers
# REV     ctr32, rev_ctr32
# ORR     ctr64, constctr96_top32, ctr32, LSL #32
# INS     ctr_next.d[0], constctr96_bottom64      // Keeping this in scalar registers to free up space in SIMD RF
# INS     ctr_next.d[1], ctr64X
# ADD     rev_ctr32, #1
#
# AES block:
#      Do AES encryption/decryption on CTR block X and EOR it with input block X. Take 256 bytes key below for example.
#      Doing small trick here of loading input in scalar registers, EORing with last key and then transferring
#      Given we are very constrained in our ASIMD registers this is quite important
#
#      Encrypt:
# LDR     input_low, [ input_ptr  ], #8
# LDR     input_high, [ input_ptr  ], #8
# EOR     input_low, k14_low
# EOR     input_high, k14_high
# INS     res_curr.d[0], input_low
# INS     res_curr.d[1], input_high
# AESE    ctr_curr, k0; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k1; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k2; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k3; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k4; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k5; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k6; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k7; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k8; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k9; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k10; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k11; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k12; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k13
# EOR     res_curr, res_curr, ctr_curr
# ST1     { res_curr.16b  }, [ output_ptr  ], #16
#
#     Decrypt:
# AESE    ctr_curr, k0; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k1; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k2; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k3; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k4; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k5; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k6; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k7; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k8; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k9; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k10; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k11; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k12; AESMC ctr_curr, ctr_curr
# AESE    ctr_curr, k13
# LDR     res_curr, [ input_ptr  ], #16
# EOR     res_curr, res_curr, ctr_curr
# MOV     output_low, res_curr.d[0]
# MOV     output_high, res_curr.d[1]
# EOR     output_low, k14_low
# EOR     output_high, k14_high
# STP     output_low, output_high, [ output_ptr  ], #16

# GHASH block X:
#     Do 128b karatsuba polynomial multiplication on block
#     We only have 64b->128b polynomial multipliers, naively that means we need to do 4 64b multiplies to generate a 128b
#
# multiplication:
#     Pmull(A,B) == (Pmull(Ah,Bh)<<128 | Pmull(Al,Bl)) ^ (Pmull(Ah,Bl) ^ Pmull(Al,Bh))<<64
#
#     The idea behind Karatsuba multiplication is that we can do just 3 64b multiplies:
#     Pmull(A,B) == (Pmull(Ah,Bh)<<128 | Pmull(Al,Bl)) ^ (Pmull(Ah^Al,Bh^Bl) ^ Pmull(Ah,Bh) ^ Pmull(Al,Bl))<<64
#
#     There is some complication here because the bit order of GHASH's PMULL is reversed compared to elsewhere, so we are
#     multiplying with "twisted" powers of H
#
# Note: We can PMULL directly into the acc_x in first GHASH of the loop
# Note: For scheduling big cores we want to split the processing to happen over two loop iterations - otherwise the critical
#       path latency dominates the performance
#
#       This has a knock on effect on register pressure, so we have to be a bit more clever with our temporary registers
#       than indicated here
# REV64   res_curr, res_curr
# INS     t_m.d[0], res_curr.d[1]
# EOR     t_m.8B, t_m.8B, res_curr.8B
# PMULL2  t_h, res_curr, HX
# PMULL   t_l, res_curr, HX
# PMULL   t_m, t_m, HX_k
# EOR     acc_h, acc_h, t_h
# EOR     acc_l, acc_l, t_l
# EOR     acc_m, acc_m, t_m
#
# MODULO: take the partial accumulators (~representing sum of 256b multiplication results), from GHASH and do modulo reduction on them
#         There is some complication here because the bit order of GHASH's PMULL is reversed compared to elsewhere, so we are doing modulo
#         with a reversed constant
# EOR3    acc_m, acc_m, acc_l, acc_h                     // Finish off karatsuba processing
# PMULL   t_mod, acc_h, mod_constant
# EXT     acc_h, acc_h, acc_h, #8
# EOR3     acc_m, acc_m, t_mod, acc_h
# PMULL   acc_h, acc_m, mod_constant
# EXT     acc_m, acc_m, acc_m, #8
# EOR3    acc_l, acc_l, acc_m, acc_h

$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate  ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate ) or
die "can't locate arm-xlate.pl";

die "only for 64 bit" if $flavour !~ /64/;

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

$code=<<___;
#include "arm_arch.h"

#if __ARM_MAX_ARCH__>=8
___
$code.=".arch   armv8-a+crypto\n.text\n";

$input_ptr="x0";  #argument block
$bit_length="x1";
$byte_length="x9";
$output_ptr="x2";
$current_tag="x3";
$counter="x16";
$constant_temp="x15";
$modulo_constant="x10";
$cc="x8";
{
my ($end_input_ptr,$main_end_input_ptr,$temp0_x,$temp1_x)=map("x$_",(4..7));
my ($temp2_x,$temp3_x)=map("x$_",(13..14));
my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$ctr4b,$ctr5b,$ctr6b,$ctr7b,$res0b,$res1b,$res2b,$res3b,$res4b,$res5b,$res6b,$res7b)=map("v$_.16b",(0..15));
my ($ctr0,$ctr1,$ctr2,$ctr3,$ctr4,$ctr5,$ctr6,$ctr7,$res0,$res1,$res2,$res3,$res4,$res5,$res6,$res7)=map("v$_",(0..15));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$ctr4d,$ctr5d,$ctr6d,$ctr7d)=map("d$_",(0..7));
my ($ctr0q,$ctr1q,$ctr2q,$ctr3q,$ctr4q,$ctr5q,$ctr6q,$ctr7q)=map("q$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q,$res4q,$res5q,$res6q,$res7q)=map("q$_",(8..15));

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3,$ctr_t4,$ctr_t5,$ctr_t6,$ctr_t7)=map("v$_",(8..15));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b,$ctr_t4b,$ctr_t5b,$ctr_t6b,$ctr_t7b)=map("v$_.16b",(8..15));
my ($ctr_t0q,$ctr_t1q,$ctr_t2q,$ctr_t3q,$ctr_t4q,$ctr_t5q,$ctr_t6q,$ctr_t7q)=map("q$_",(8..15));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(17..19));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(17..19));

my ($h1,$h12k,$h2,$h3,$h34k,$h4)=map("v$_",(20..25));
my ($h5,$h56k,$h6,$h7,$h78k,$h8)=map("v$_",(20..25));
my ($h1q,$h12kq,$h2q,$h3q,$h34kq,$h4q)=map("q$_",(20..25));
my ($h5q,$h56kq,$h6q,$h7q,$h78kq,$h8q)=map("q$_",(20..25));

my $t0="v16";
my $t0d="d16";

my $t1="v29";
my $t2=$res1;
my $t3=$t1;

my $t4=$res0;
my $t5=$res2;
my $t6=$t0;

my $t7=$res3;
my $t8=$res4;
my $t9=$res5;

my $t10=$res6;
my $t11="v21";
my $t12=$t1;

my $rtmp_ctr="v30";
my $rtmp_ctrq="q30";
my $rctr_inc="v31";
my $rctr_incd="d31";

my $mod_constantd=$t0d;
my $mod_constant=$t0;

my ($rk0,$rk1,$rk2)=map("v$_.16b",(26..28));
my ($rk3,$rk4,$rk5)=map("v$_.16b",(26..28));
my ($rk6,$rk7,$rk8)=map("v$_.16b",(26..28));
my ($rk9,$rk10,$rk11)=map("v$_.16b",(26..28));
my ($rk12,$rk13,$rk14)=map("v$_.16b",(26..28));
my ($rk0q,$rk1q,$rk2q)=map("q$_",(26..28));
my ($rk3q,$rk4q,$rk5q)=map("q$_",(26..28));
my ($rk6q,$rk7q,$rk8q)=map("q$_",(26..28));
my ($rk9q,$rk10q,$rk11q)=map("q$_",(26..28));
my ($rk12q,$rk13q,$rk14q)=map("q$_",(26..28));
my $rk2q1="v28.1q";
my $rk3q1="v26.1q";
my $rk4v="v27";


#########################################################################################
# size_t unroll8_eor3_aes_gcm_enc_128_kernel(const uint8_t * plaintext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * ciphertext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_enc_128_kernel
.type   unroll8_eor3_aes_gcm_enc_128_kernel,%function
.align  4
unroll8_eor3_aes_gcm_enc_128_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L128_enc_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
	mov	x5, #0xc200000000000000
	stp	x5, xzr, [sp, #64]
	add	$modulo_constant, sp, #64

	mov	$constant_temp, #0x100000000				@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp
	mov	$main_end_input_ptr, $byte_length
	ld1	{ $ctr0b}, [$counter]					@ CTR block 0

	sub	$main_end_input_ptr, $main_end_input_ptr, #1	 	@ byte_len - 1

	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80		@ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5
	ldp	$rk0q, $rk1q, [$cc, #0]				  	@ load rk0, rk1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 7

	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1

	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 1

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3

	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4

	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7

	ld1	{ $acc_lb}, [$current_tag]
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7

	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	ldr	$rk10q, [$cc, #160]					@ load rk10

	aese	$ctr3b, $rk9						@ AES block 8k+11 - round 9
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	aese	$ctr2b, $rk9						@ AES block 8k+10 - round 9

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr6b, $rk9						@ AES block 8k+14 - round 9

	aese	$ctr4b, $rk9						@ AES block 8k+12 - round 9
	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr
	aese	$ctr0b, $rk9						@ AES block 8k+8 - round 9

	aese	$ctr7b, $rk9						@ AES block 8k+15 - round 9
	aese	$ctr5b, $rk9						@ AES block 8k+13 - round 9
	aese	$ctr1b, $rk9						@ AES block 8k+9 - round 9

	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3		@ end_input_ptr
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	b.ge	.L128_enc_tail						@ handle tail

	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 0, 1 - load plaintext

	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 2, 3 - load plaintext

	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 4, 5 - load plaintext

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 6, 7 - load plaintext
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks

	eor3	$res0b, $ctr_t0b, $ctr0b, $rk10				@ AES block 0 - result
	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8

	eor3	$res1b, $ctr_t1b, $ctr1b, $rk10				@ AES block 1 - result
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	eor3	$res5b, $ctr_t5b, $ctr5b, $rk10				@ AES block 5 - result
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9

	eor3	$res2b, $ctr_t2b, $ctr2b, $rk10				@ AES block 2 - result
	eor3	$res6b, $ctr_t6b, $ctr6b, $rk10				@ AES block 6 - result
	eor3	$res4b, $ctr_t4b, $ctr4b, $rk10				@ AES block 4 - result

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10

	eor3	$res3b, $ctr_t3b, $ctr3b, $rk10				@ AES block 3 - result
	eor3	$res7b, $ctr_t7b, $ctr7b,$rk10				@ AES block 7 - result
	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 2, 3 - store result

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11
	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 4, 5 - store result

	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 6, 7 - store result

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12
	b.ge	.L128_enc_prepretail					@ do prepretail

.L128_enc_main_loop:							@ main loop start
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	rev64	$res1b, $res1b						@ GHASH block 8k+1
	rev64	$res0b, $res0b						@ GHASH block 8k
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k
	rev64	$res5b, $res5b						@ GHASH block 8k+5 (t0, t1, t2 and t3 free)
	rev64	$res3b, $res3b						@ GHASH block 8k+3

	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15

	rev64	$res7b, $res7b						@ GHASH block 8k+7 (t0, t1, t2 and t3 free)

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low

	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high

	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h3l | h3h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0

	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	eor3	$acc_hb, $acc_hb, $t1.16b,$t2.16b			@ GHASH block 8k+2, 8k+3 - high
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1

	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid

	rev64	$res6b, $res6b						@ GHASH block 8k+6 (t0, t1, and t2 free)
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low

	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2

	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	rev64	$res4b, $res4b						@ GHASH block 8k+4 (t0, t1, and t2 free)

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h1l | h1h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4

	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5

	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5

	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load plaintext

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load plaintext

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7

	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 8k+12, 8k+13 - load plaintext
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17

	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ldr	$rk10q, [$cc, #160]					@ load rk10

	ext	$t12.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment
	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8

	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8

	aese	$ctr2b, $rk9						@ AES block 8k+10 - round 9
	aese	$ctr4b, $rk9						@ AES block 8k+12 - round 9
	aese	$ctr1b, $rk9						@ AES block 8k+9 - round 9

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 8k+14, 8k+15 - load plaintext
	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19

	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL
	eor3	$res4b, $ctr_t4b, $ctr4b, $rk10				@ AES block 4 - result
	aese	$ctr7b, $rk9						@ AES block 8k+15 - round 9

	aese	$ctr6b, $rk9						@ AES block 8k+14 - round 9
	aese	$ctr3b, $rk9						@ AES block 8k+11 - round 9

	eor3	$res2b, $ctr_t2b, $ctr2b, $rk10				@ AES block 8k+10 - result

	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18
	aese	$ctr0b, $rk9						@ AES block 8k+8 - round 9

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20

	eor3	$res7b, $ctr_t7b, $ctr7b, $rk10				@ AES block 7 - result
	aese	$ctr5b, $rk9						@ AES block 8k+13 - round 9
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low

	eor3	$res1b, $ctr_t1b, $ctr1b, $rk10				@ AES block 8k+9 - result
	eor3	$res3b, $ctr_t3b, $ctr3b, $rk10				@ AES block 8k+11 - result
	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19

	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment
	eor3	$res5b, $ctr_t5b, $ctr5b, $rk10				@ AES block 5 - result
	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17

	eor3	$res0b, $ctr_t0b, $ctr0b, $rk10				@ AES block 8k+8 - result
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result

	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result
	eor3	$res6b, $ctr_t6b, $ctr6b, $rk10				@ AES block 6 - result

	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 8k+12, 8k+13 - store result
	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low

	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 8k+14, 8k+15 - store result
	b.lt	.L128_enc_main_loop

.L128_enc_prepretail:							@ PREPRETAIL
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	rev64	$res0b, $res0b						@ GHASH block 8k
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h6k | h5k
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13
	rev64	$res3b, $res3b						@ GHASH block 8k+3

	rev64	$res2b, $res2b						@ GHASH block 8k+2
	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	rev64	$res5b, $res5b						@ GHASH block 8k+5 (t0, t1, t2 and t3 free)
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid

	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid

	rev64	$res4b, $res4b						@ GHASH block 8k+4 (t0, t1, and t2 free)
	rev64	$res7b, $res7b						@ GHASH block 8k+7 (t0, t1, t2 and t3 free)

	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15

	rev64	$res6b, $res6b						@ GHASH block 8k+6 (t0, t1, and t2 free)

	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0

	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0

	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid

	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2

	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h1l | h1h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3

	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3

	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high

	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high

	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4

	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6

	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	ext	$t12.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	ext	$acc_mb, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	eor3	$acc_lb, $acc_lb, $acc_hb, $acc_mb		 	@ MODULO - fold into low
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8

	ldr	$rk10q, [$cc, #160]					@ load rk10
	aese	$ctr6b, $rk9						@ AES block 8k+14 - round 9
	aese	$ctr2b, $rk9						@ AES block 8k+10 - round 9

	aese	$ctr0b, $rk9						@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk9						@ AES block 8k+9 - round 9

	aese	$ctr3b, $rk9						@ AES block 8k+11 - round 9
	aese	$ctr5b, $rk9						@ AES block 8k+13 - round 9

	aese	$ctr4b, $rk9						@ AES block 8k+12 - round 9
	aese	$ctr7b, $rk9						@ AES block 8k+15 - round 9
.L128_enc_tail:								@ TAIL

	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr 	@ main_end_input_ptr is number of bytes left to process
	ldr	$ctr_t0q, [$input_ptr], #16				@ AES block 8k+8 - load plaintext

	mov	$t1.16b, $rk10
	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8

	eor3	$res1b, $ctr_t0b, $ctr0b, $t1.16b			@ AES block 8k+8 - result
	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag
	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	ext     $h7.16b, $h7.16b, $h7.16b, #8

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8k | h7k
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	cmp	$main_end_input_ptr, #112
	b.gt	.L128_enc_blocks_more_than_7

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	movi	$acc_h.8b, #0

	cmp	$main_end_input_ptr, #96
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr2b
	mov	$ctr2b, $ctr1b

	movi	$acc_l.8b, #0
	movi	$acc_m.8b, #0
	b.gt	.L128_enc_blocks_more_than_6

	mov	$ctr7b, $ctr6b
	cmp	$main_end_input_ptr, #80

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr1b
	b.gt	.L128_enc_blocks_more_than_5

	cmp	$main_end_input_ptr, #64
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr1b
	b.gt	.L128_enc_blocks_more_than_4

	mov	$ctr7b, $ctr6b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr1b
	cmp	$main_end_input_ptr, #48
	b.gt	.L128_enc_blocks_more_than_3

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr1b

	cmp	$main_end_input_ptr, #32
	ldr	$h34kq, [$current_tag, #96]					@ load h4k | h3k
	b.gt	.L128_enc_blocks_more_than_2

	cmp	$main_end_input_ptr, #16

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr1b
	b.gt	.L128_enc_blocks_more_than_1

	ldr	$h12kq, [$current_tag, #48]					@ load h2k | h1k
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b	 .L128_enc_blocks_less_than_1
.L128_enc_blocks_more_than_7:						@ blocks left >  7
	st1	{ $res1b}, [$output_ptr], #16				@ AES final-7 block  - store result

	rev64	$res0b, $res1b						@ GHASH final-7 block
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-6 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high

	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor3	$res1b, $ctr_t1b, $ctr1b, $t1.16b			@ AES final-6 block - result

	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d				@ GHASH final-7 block - mid
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low
.L128_enc_blocks_more_than_6:						@ blocks left >  6

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-6 block - store result

	rev64	$res0b, $res1b						@ GHASH final-6 block
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-5 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid

	eor3	$res1b, $ctr_t1b, $ctr2b, $t1.16b			@ AES final-5 block - result
	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high
.L128_enc_blocks_more_than_5:						@ blocks left >  5

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-5 block - store result

	rev64	$res0b, $res1b						@ GHASH final-5 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-4 block - load plaintext
	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid

	eor3	$res1b, $ctr_t1b, $ctr3b, $t1.16b			@ AES final-4 block - result
	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
.L128_enc_blocks_more_than_4:						@ blocks left >  4

	st1	{ $res1b}, [$output_ptr], #16			  	@ AES final-4 block - store result

	rev64	$res0b, $res1b						@ GHASH final-4 block

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-3 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid

	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high
	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low

	eor3	$res1b, $ctr_t1b, $ctr4b, $t1.16b			@ AES final-3 block - result
	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
.L128_enc_blocks_more_than_3:						@ blocks left >  3

	st1	{ $res1b}, [$output_ptr], #16			  	@ AES final-3 block - store result

	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8

	rev64	$res0b, $res1b						@ GHASH final-3 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-2 block - load plaintext

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low

	eor3	$res1b, $ctr_t1b, $ctr5b, $t1.16b			@ AES final-2 block - result

	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid
	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high
.L128_enc_blocks_more_than_2:						@ blocks left >  2

	st1	{ $res1b}, [$output_ptr], #16			  	@ AES final-2 block - store result

	rev64	$res0b, $res1b						@ GHASH final-2 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-1 block - load plaintext

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid
	eor3	$res1b, $ctr_t1b, $ctr6b, $t1.16b			@ AES final-1 block - result

	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high

	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low
	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low
.L128_enc_blocks_more_than_1:						@ blocks left >  1

	st1	{ $res1b}, [$output_ptr], #16			  	@ AES final-1 block - store result

	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	rev64	$res0b, $res1b						@ GHASH final-1 block
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid
	eor3	$res1b, $ctr_t1b, $ctr7b, $t1.16b			@ AES final block - result

	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid

	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low
	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low
.L128_enc_blocks_less_than_1:						@ blocks left <= 1

	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b
	str	$rtmp_ctrq, [$counter]					@ store the updated counter
	and	$bit_length, $bit_length, #127			 	@ bit_length %= 128

	sub	$bit_length, $bit_length, #128			 	@ bit_length -= 128

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])

	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff
	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored
	and	$bit_length, $bit_length, #127			 	@ bit_length %= 128

	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff
	cmp	$bit_length, #64

	csel	$temp2_x, $temp1_x, $temp0_x, lt
	csel	$temp3_x, $temp0_x, xzr, lt

	mov	$ctr0.d[1], $temp3_x
	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits

	rev64	$res0b, $res1b						@ GHASH final block

	bif	$res1b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing
	st1	{ $res1b}, [$output_ptr]				@ store all 16B

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid

	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext	$h1.16b, $h1.16b, $h1.16b, #8

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid

	pmull2  $rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high
	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low

	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		  	@ MODULO - top 64b align with mid

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		  	@ MODULO - karatsuba tidy up

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b		 	@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ext	$t11.16b, $acc_mb, $acc_mb, #8			  	@ MODULO - other mid alignment

	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		  	@ MODULO - fold into low
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]
	mov	x0, $byte_length

	ldp	d10, d11, [sp, #16]
	ldp	d12, d13, [sp, #32]
	ldp	d14, d15, [sp, #48]
	ldp	d8, d9, [sp], #80
	ret

.L128_enc_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_enc_128_kernel,.-unroll8_eor3_aes_gcm_enc_128_kernel
___

#########################################################################################
# size_t unroll8_eor3_aes_gcm_dec_128_kernel(const uint8_t * ciphertext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * plaintext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_dec_128_kernel
.type   unroll8_eor3_aes_gcm_dec_128_kernel,%function
.align  4
unroll8_eor3_aes_gcm_dec_128_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L128_dec_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
	mov	x5, #0xc200000000000000
	stp	x5, xzr, [sp, #64]
	add	$modulo_constant, sp, #64

	mov	$main_end_input_ptr, $byte_length
	ld1	{ $ctr0b}, [$counter]					@ CTR block 0

	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	sub	$main_end_input_ptr, $main_end_input_ptr, #1		@ byte_len - 1

	mov	$constant_temp, #0x100000000				@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp
	ld1	{ $acc_lb}, [$current_tag]
	  ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80	@ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5

	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 0
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 0

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1

	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1

	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2

	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3

	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3

	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3

	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5

	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5

	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6

	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7

	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 7

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 8

	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 8

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 8

	aese	$ctr0b, $rk9						@ AES block 0 - round 9
	aese	$ctr1b, $rk9						@ AES block 1 - round 9
	aese	$ctr6b, $rk9						@ AES block 6 - round 9

	ldr	$rk10q, [$cc, #160]					@ load rk10
	aese	$ctr4b, $rk9						@ AES block 4 - round 9
	aese	$ctr3b, $rk9						@ AES block 3 - round 9

	aese	$ctr2b, $rk9						@ AES block 2 - round 9
	aese	$ctr5b, $rk9						@ AES block 5 - round 9
	aese	$ctr7b, $rk9						@ AES block 7 - round 9

	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3		@ end_input_ptr
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	b.ge	.L128_dec_tail						@ handle tail

	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 0, 1 - load ciphertext

	eor3	$ctr0b, $res0b, $ctr0b, $rk10				@ AES block 0 - result
	eor3	$ctr1b, $res1b, $ctr1b, $rk10				@ AES block 1 - result
	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8
	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 2, 3 - load ciphertext

	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 4, 5 - load ciphertext

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9
	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 6, 7 - load ciphertext

	eor3	$ctr3b, $res3b, $ctr3b, $rk10				@ AES block 3 - result
	eor3	$ctr2b, $res2b, $ctr2b, $rk10				@ AES block 2 - result
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 2, 3 - store result

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10

	eor3	$ctr6b, $res6b, $ctr6b, $rk10				@ AES block 6 - result

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11

	eor3	$ctr4b, $res4b, $ctr4b, $rk10				@ AES block 4 - result
	eor3	$ctr5b, $res5b, $ctr5b, $rk10				@ AES block 5 - result
	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 4, 5 - store result

	eor3	$ctr7b, $res7b, $ctr7b, $rk10				@ AES block 7 - result
	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 6, 7 - store result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12

	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12
	b.ge	.L128_dec_prepretail					@ do prepretail

.L128_dec_main_loop:							@ main loop start
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8

	rev64	$res1b, $res1b						@ GHASH block 8k+1
	rev64	$res0b, $res0b						@ GHASH block 8k
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	rev64	$res6b, $res6b						@ GHASH block 8k+6
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8

	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	rev64	$res2b, $res2b						@ GHASH block 8k+2
	rev64	$res4b, $res4b						@ GHASH block 8k+4
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high
	rev64	$res3b, $res3b						@ GHASH block 8k+3

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	rev64	$res5b, $res5b						@ GHASH block 8k+5

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high

	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1

	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2

	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	rev64	$res7b, $res7b						@ GHASH block 8k+7
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3

	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high

	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4

	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid

	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5

	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b 			@ GHASH block 8k+4, 8k+5 - mid
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load ciphertext

	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load ciphertext
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18

	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 8k+12, 8k+13 - load ciphertext
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 8k+14, 8k+15 - load ciphertext
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8

	aese	$ctr0b, $rk9						@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk9						@ AES block 8k+9 - round 9
	ldr	$rk10q, [$cc, #160]					@ load rk10

	aese	$ctr6b, $rk9						@ AES block 8k+14 - round 9
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr2b, $rk9						@ AES block 8k+10 - round 9

	aese	$ctr7b, $rk9						@ AES block 8k+15 - round 9
	aese	$ctr4b, $rk9						@ AES block 8k+12 - round 9
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment

	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19

	aese	$ctr3b, $rk9						@ AES block 8k+11 - round 9
	aese	$ctr5b, $rk9						@ AES block 8k+13 - round 9
	eor3	$ctr1b, $res1b, $ctr1b, $rk10				@ AES block 8k+9 - result

	eor3	$ctr0b, $res0b, $ctr0b, $rk10				@ AES block 8k+8 - result
	eor3	$ctr7b, $res7b, $ctr7b, $rk10				@ AES block 8k+15 - result
	eor3	$ctr6b, $res6b, $ctr6b, $rk10				@ AES block 8k+14 - result

	eor3	$ctr2b, $res2b, $ctr2b, $rk10				@ AES block 8k+10 - result
	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result
	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17

	eor3	$ctr4b, $res4b, $ctr4b, $rk10				@ AES block 8k+12 - result
	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16

	eor3	$ctr3b, $res3b, $ctr3b, $rk10				@ AES block 8k+11 - result
	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result

	eor3	$ctr5b, $res5b, $ctr5b, $rk10				@ AES block 8k+13 - result
	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18

	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 8k+12, 8k+13 - store result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20

	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 8k+14, 8k+15 - store result
	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19
	b.lt	.L128_dec_main_loop

.L128_dec_prepretail:							@ PREPRETAIL
	rev64	$res3b, $res3b						@ GHASH block 8k+3
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	rev64	$res0b, $res0b						@ GHASH block 8k

	rev64	$res2b, $res2b						@ GHASH block 8k+2
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1

	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	rev64	$res5b, $res5b						@ GHASH block 8k+5

	rev64	$res4b, $res4b						@ GHASH block 8k+4

	rev64	$res6b, $res6b						@ GHASH block 8k+6

	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k
	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0

	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k - mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	rev64	$res7b, $res7b						@ GHASH block 8k+7
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4

	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5

	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5

	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	ldr	$rk10q, [$cc, #160]					@ load rk10

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment

	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8

	aese	$ctr6b, $rk9						@ AES block 8k+14 - round 9
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8

	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15
	aese	$ctr2b, $rk9						@ AES block 8k+10 - round 9

	aese	$ctr3b, $rk9						@ AES block 8k+11 - round 9
	aese	$ctr5b, $rk9						@ AES block 8k+13 - round 9
	aese	$ctr0b, $rk9						@ AES block 8k+8 - round 9

	aese	$ctr4b, $rk9						@ AES block 8k+12 - round 9
	aese	$ctr1b, $rk9						@ AES block 8k+9 - round 9
	aese	$ctr7b, $rk9						@ AES block 8k+15 - round 9

.L128_dec_tail:								@ TAIL

	mov	$t1.16b, $rk10
	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr 	@ main_end_input_ptr is number of bytes left to process

	cmp	$main_end_input_ptr, #112

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8k | h7k
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	ldr	$res1q, [$input_ptr], #16				@ AES block 8k+8 - load ciphertext

	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag

	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	ext     $h7.16b, $h7.16b, $h7.16b, #8

	eor3	$res4b, $res1b, $ctr0b, $t1.16b				@ AES block 8k+8 - result
	b.gt	.L128_dec_blocks_more_than_7

	cmp	$main_end_input_ptr, #96
	mov	$ctr7b, $ctr6b
	movi	$acc_l.8b, #0

	movi	$acc_h.8b, #0
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr2b
	mov	$ctr2b, $ctr1b

	movi	$acc_m.8b, #0
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L128_dec_blocks_more_than_6

	cmp	$main_end_input_ptr, #80
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr1b
	b.gt	.L128_dec_blocks_more_than_5

	cmp	$main_end_input_ptr, #64

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L128_dec_blocks_more_than_4

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr1b
	cmp	$main_end_input_ptr, #48
	b.gt	.L128_dec_blocks_more_than_3

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	cmp	$main_end_input_ptr, #32

	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	mov	$ctr6b, $ctr1b
	b.gt	.L128_dec_blocks_more_than_2

	cmp	$main_end_input_ptr, #16

	mov	$ctr7b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	L128_dec_blocks_more_than_1

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	b	 .L128_dec_blocks_less_than_1
.L128_dec_blocks_more_than_7:						@ blocks left >  7
	rev64	$res0b, $res1b						@ GHASH final-7 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid

	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	ldr	$res1q, [$input_ptr], #16				@ AES final-6 block - load ciphertext

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-7 block  - store result
	eor3	$res4b, $res1b, $ctr1b, $t1.16b				@ AES final-6 block - result

	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d			 	@ GHASH final-7 block - mid
.L128_dec_blocks_more_than_6:						@ blocks left >  6

	rev64	$res0b, $res1b						@ GHASH final-6 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid

	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low
	ldr	$res1q, [$input_ptr], #16				@ AES final-5 block - load ciphertext
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-6 block - store result
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
	eor3	$res4b, $res1b, $ctr2b, $t1.16b				@ AES final-5 block - result
.L128_dec_blocks_more_than_5:						@ blocks left >  5

	rev64	$res0b, $res1b						@ GHASH final-5 block

	ldr	$res1q, [$input_ptr], #16				@ AES final-4 block - load ciphertext
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-5 block - store result

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid

	eor3	$res4b, $res1b, $ctr3b, $t1.16b				@ AES final-4 block - result

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid
	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid
	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high
.L128_dec_blocks_more_than_4:						@ blocks left >  4

	rev64	$res0b, $res1b						@ GHASH final-4 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	ldr	$res1q, [$input_ptr], #16				@ AES final-3 block - load ciphertext

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high

	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-4 block - store result
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid

	eor3	$res4b, $res1b, $ctr4b, $t1.16b				@ AES final-3 block - result
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low

	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
.L128_dec_blocks_more_than_3:						@ blocks left >  3

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-3 block - store result
	rev64	$res0b, $res1b						@ GHASH final-3 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid

	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid

	ldr	$res1q, [$input_ptr], #16				@ AES final-2 block - load ciphertext

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low
	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	eor3	$res4b, $res1b, $ctr5b, $t1.16b				@ AES final-2 block - result
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low

	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high
	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
.L128_dec_blocks_more_than_2:						@ blocks left >  2

	rev64	$res0b, $res1b						@ GHASH final-2 block

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-2 block - store result

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid

	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low

	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high
	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid
	ldr	$res1q, [$input_ptr], #16				@ AES final-1 block - load ciphertext

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low

	eor3	$res4b, $res1b, $ctr6b, $t1.16b				@ AES final-1 block - result
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high
.L128_dec_blocks_more_than_1:						@ blocks left >  1

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-1 block - store result
	rev64	$res0b, $res1b						@ GHASH final-1 block

	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid

	ldr	$res1q, [$input_ptr], #16				@ AES final block - load ciphertext
	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid
	eor3	$res4b, $res1b, $ctr7b, $t1.16b				@ AES final block - result

	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
.L128_dec_blocks_less_than_1:						@ blocks left <= 1

	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	sub	$bit_length, $bit_length, #128				@ bit_length -= 128

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])

	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff
	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	cmp	$bit_length, #64
	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff

	csel	$temp2_x, $temp1_x, $temp0_x, lt
	csel	$temp3_x, $temp0_x, xzr, lt

	mov	$ctr0.d[1], $temp3_x
	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits

	rev64	$res0b, $res1b						@ GHASH final block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	pmull2  $rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high
	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high
	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid

	bif	$res4b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid
	st1	{ $res4b}, [$output_ptr]				@ store all 16B

	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low

	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low

	eor	$t10.16b, $acc_hb, $acc_lb				@ MODULO - karatsuba tidy up

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	ext	$acc_hb, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	eor	$acc_mb, $acc_mb, $t10.16b				@ MODULO - karatsuba tidy up

	eor3	$acc_mb, $acc_mb, $acc_hb, $t11.16b			@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ext	$acc_mb, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment

	eor3	$acc_lb, $acc_lb, $acc_mb, $acc_hb			@ MODULO - fold into low
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]
	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b

	str	$rtmp_ctrq, [$counter]					@ store the updated counter

	mov	x0, $byte_length

	ldp	d10, d11, [sp, #16]
	ldp	d12, d13, [sp, #32]
	ldp	d14, d15, [sp, #48]
	ldp	d8, d9, [sp], #80
	ret
.L128_dec_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_dec_128_kernel,.-unroll8_eor3_aes_gcm_dec_128_kernel
___
}

{
my ($end_input_ptr,$main_end_input_ptr,$temp0_x,$temp1_x)=map("x$_",(4..7));
my ($temp2_x,$temp3_x)=map("x$_",(13..14));
my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$ctr4b,$ctr5b,$ctr6b,$ctr7b,$res0b,$res1b,$res2b,$res3b,$res4b,$res5b,$res6b,$res7b)=map("v$_.16b",(0..15));
my ($ctr0,$ctr1,$ctr2,$ctr3,$ctr4,$ctr5,$ctr6,$ctr7,$res0,$res1,$res2,$res3,$res4,$res5,$res6,$res7)=map("v$_",(0..15));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$ctr4d,$ctr5d,$ctr6d,$ctr7d)=map("d$_",(0..7));
my ($ctr0q,$ctr1q,$ctr2q,$ctr3q,$ctr4q,$ctr5q,$ctr6q,$ctr7q)=map("q$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q,$res4q,$res5q,$res6q,$res7q)=map("q$_",(8..15));

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3,$ctr_t4,$ctr_t5,$ctr_t6,$ctr_t7)=map("v$_",(8..15));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b,$ctr_t4b,$ctr_t5b,$ctr_t6b,$ctr_t7b)=map("v$_.16b",(8..15));
my ($ctr_t0q,$ctr_t1q,$ctr_t2q,$ctr_t3q,$ctr_t4q,$ctr_t5q,$ctr_t6q,$ctr_t7q)=map("q$_",(8..15));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(17..19));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(17..19));

my ($h1,$h12k,$h2,$h3,$h34k,$h4)=map("v$_",(20..25));
my ($h5,$h56k,$h6,$h7,$h78k,$h8)=map("v$_",(20..25));
my ($h1q,$h12kq,$h2q,$h3q,$h34kq,$h4q)=map("q$_",(20..25));
my ($h5q,$h56kq,$h6q,$h7q,$h78kq,$h8q)=map("q$_",(20..25));

my $t0="v16";
my $t0d="d16";

my $t1="v29";
my $t2=$res1;
my $t3=$t1;

my $t4=$res0;
my $t5=$res2;
my $t6=$t0;

my $t7=$res3;
my $t8=$res4;
my $t9=$res5;

my $t10=$res6;
my $t11="v21";
my $t12=$t1;

my $rtmp_ctr="v30";
my $rtmp_ctrq="q30";
my $rctr_inc="v31";
my $rctr_incd="d31";

my $mod_constantd=$t0d;
my $mod_constant=$t0;

my ($rk0,$rk1,$rk2)=map("v$_.16b",(26..28));
my ($rk3,$rk4,$rk5)=map("v$_.16b",(26..28));
my ($rk6,$rk7,$rk8)=map("v$_.16b",(26..28));
my ($rk9,$rk10,$rk11)=map("v$_.16b",(26..28));
my ($rk12,$rk13,$rk14)=map("v$_.16b",(26..28));
my ($rk0q,$rk1q,$rk2q)=map("q$_",(26..28));
my ($rk3q,$rk4q,$rk5q)=map("q$_",(26..28));
my ($rk6q,$rk7q,$rk8q)=map("q$_",(26..28));
my ($rk9q,$rk10q,$rk11q)=map("q$_",(26..28));
my ($rk12q,$rk13q,$rk14q)=map("q$_",(26..28));
my $rk2q1="v28.1q";
my $rk3q1="v26.1q";
my $rk4v="v27";

#########################################################################################
# size_t unroll8_eor3_aes_gcm_enc_192_kernel(const uint8_t * plaintext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * ciphertext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_enc_192_kernel
.type   unroll8_eor3_aes_gcm_enc_192_kernel,%function
.align  4
unroll8_eor3_aes_gcm_enc_192_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L192_enc_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
	mov	x5, #0xc200000000000000
	stp	x5, xzr, [sp, #64]
	add	$modulo_constant, sp, #64

	mov	$main_end_input_ptr, $byte_length
	ld1	{ $ctr0b}, [$counter]					@ CTR block 0

	mov	$constant_temp, #0x100000000				@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4
	sub	$main_end_input_ptr, $main_end_input_ptr, #1		@ byte_len - 1

	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80	@ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1

	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 0

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 1

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 1

	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3

	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3

	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3

	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4

	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 7

	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 8

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 8

	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 8

	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3		@ end_input_ptr
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 9

        ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 9

	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 9
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 9

	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 9
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 9

	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 14 - round 10
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 9
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 11 - round 10

	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 9 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 13 - round 10
	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 12 - round 10

	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 10 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 15 - round 10

	aese	$ctr6b, $rk11						@ AES block 14 - round 11
	aese	$ctr3b, $rk11						@ AES block 11 - round 11

	aese	$ctr4b, $rk11						@ AES block 12 - round 11
	aese	$ctr7b, $rk11						@ AES block 15 - round 11
	ldr	$rk12q, [$cc, #192]					@ load rk12

	aese	$ctr1b, $rk11						@ AES block 9 - round 11
	aese	$ctr5b, $rk11						@ AES block 13 - round 11

	aese	$ctr2b, $rk11						@ AES block 10 - round 11
	aese	$ctr0b, $rk11						@ AES block 8 - round 11
	b.ge	.L192_enc_tail						@ handle tail

	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 0, 1 - load plaintext

	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 2, 3 - load plaintext

	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 4, 5 - load plaintext

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 6, 7 - load plaintext

	eor3	$res0b, $ctr_t0b, $ctr0b, $rk12				@ AES block 0 - result
	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8

	eor3	$res3b, $ctr_t3b, $ctr3b, $rk12				@ AES block 3 - result
	eor3	$res1b, $ctr_t1b, $ctr1b, $rk12				@ AES block 1 - result

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9
	eor3	$res4b, $ctr_t4b, $ctr4b, $rk12				@ AES block 4 - result

	eor3	$res5b, $ctr_t5b, $ctr5b, $rk12				@ AES block 5 - result
	eor3	$res7b, $ctr_t7b, $ctr7b, $rk12				@ AES block 7 - result
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	eor3	$res2b, $ctr_t2b, $ctr2b, $rk12				@ AES block 2 - result
	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10

	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 2, 3 - store result
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11
	eor3	$res6b, $ctr_t6b, $ctr6b, $rk12				@ AES block 6 - result

	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 4, 5 - store result

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12
	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 6, 7 - store result
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12

	b.ge	.L192_enc_prepretail					@ do prepretail

.L192_enc_main_loop:							@ main loop start
	rev64	$res4b, $res4b						@ GHASH block 8k+4 (t0, t1, and t2 free)
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	rev64	$res2b, $res2b						@ GHASH block 8k+2

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8

	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	rev64	$res0b, $res0b						@ GHASH block 8k
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8

	rev64	$res1b, $res1b						@ GHASH block 8k+1
	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev64	$res3b, $res3b						@ GHASH block 8k+3
	rev64	$res5b, $res5b						@ GHASH block 8k+5 (t0, t1, t2 and t3 free)

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2

	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2

	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k - mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3

	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3

	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4

	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	rev64	$res7b, $res7b						@ GHASH block 8k+7 (t0, t1, t2 and t3 free)

	rev64	$res6b, $res6b						@ GHASH block 8k+6 (t0, t1, and t2 free)
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5

	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high

	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid

	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9
	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load plaintext

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9

	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17

	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	ldr	$rk12q, [$cc, #192]					@ load rk12
	ext	$t12.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load plaintext

	aese	$ctr4b, $rk11						@ AES block 8k+12 - round 11
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 8k+12, 8k+13 - load plaintext

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 8k+14, 8k+15 - load plaintext
	aese	$ctr2b, $rk11						@ AES block 8k+10 - round 11
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10

	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10

	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low

	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10
	aese	$ctr5b, $rk11						@ AES block 8k+13 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18

	aese	$ctr7b, $rk11						@ AES block 8k+15 - round 11
	aese	$ctr0b, $rk11						@ AES block 8k+8 - round 11
	eor3	$res4b, $ctr_t4b, $ctr4b, $rk12				@ AES block 4 - result

	aese	$ctr6b, $rk11						@ AES block 8k+14 - round 11
	aese	$ctr3b, $rk11						@ AES block 8k+11 - round 11
	aese	$ctr1b, $rk11						@ AES block 8k+9 - round 11

	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19
	eor3	$res7b, $ctr_t7b, $ctr7b, $rk12				@ AES block 7 - result

	eor3	$res2b, $ctr_t2b, $ctr2b, $rk12				@ AES block 8k+10 - result
	eor3	$res0b, $ctr_t0b, $ctr0b, $rk12				@ AES block 8k+8 - result
	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18

	eor3	$res1b, $ctr_t1b, $ctr1b, $rk12				@ AES block 8k+9 - result
	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result
	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment

	eor3	$res6b, $ctr_t6b, $ctr6b, $rk12				@ AES block 6 - result
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20
	eor3	$res5b, $ctr_t5b, $ctr5b, $rk12				@ AES block 5 - result
	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low

	eor3	$res3b, $ctr_t3b, $ctr3b, $rk12				@ AES block 8k+11 - result
	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19

	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result

	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 8k+12, 8k+13 - store result

	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL
	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 8k+14, 8k+15 - store result
	b.lt	.L192_enc_main_loop

.L192_enc_prepretail:							@ PREPRETAIL
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	rev64	$res0b, $res0b						@ GHASH block 8k
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	rev64	$res3b, $res3b						@ GHASH block 8k+3
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8

	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1

	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low

	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2
	rev64	$res5b, $res5b						@ GHASH block 8k+5 (t0, t1, t2 and t3 free)
	rev64	$res6b, $res6b						@ GHASH block 8k+6 (t0, t1, and t2 free)

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3

	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	rev64	$res4b, $res4b						@ GHASH block 8k+4 (t0, t1, and t2 free)

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4

	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	rev64	$res7b, $res7b						@ GHASH block 8k+7 (t0, t1, t2 and t3 free)
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low

	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid

	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ext	$t12.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8

	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8

	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9

	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ldr	$rk12q, [$cc, #192]					@ load rk12

	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10

	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10

	aese	$ctr1b, $rk11						@ AES block 8k+9 - round 11
	aese	$ctr7b, $rk11						@ AES block 8k+15 - round 11

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr3b, $rk11						@ AES block 8k+11 - round 11

	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15
	aese	$ctr2b, $rk11						@ AES block 8k+10 - round 11
	aese	$ctr0b, $rk11						@ AES block 8k+8 - round 11

	aese	$ctr6b, $rk11						@ AES block 8k+14 - round 11
	aese	$ctr4b, $rk11						@ AES block 8k+12 - round 11
	aese	$ctr5b, $rk11						@ AES block 8k+13 - round 11

.L192_enc_tail:								@ TAIL

	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
        ext     $h5.16b, $h5.16b, $h5.16b, #8
	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr 	@ main_end_input_ptr is number of bytes left to process

	ldr	$ctr_t0q, [$input_ptr], #16				@ AES block 8k+8 - l3ad plaintext

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8k | h7k
        ext     $h8.16b, $h8.16b, $h8.16b, #8

	mov	$t1.16b, $rk12

	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
        ext     $h6.16b, $h6.16b, $h6.16b, #8
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	cmp	$main_end_input_ptr, #112

	eor3	$res1b, $ctr_t0b, $ctr0b, $t1.16b			@ AES block 8k+8 - result
	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag
	b.gt	.L192_enc_blocks_more_than_7

	cmp	$main_end_input_ptr, #96
	mov	$ctr7b, $ctr6b
	movi	$acc_h.8b, #0

	mov	$ctr6b, $ctr5b
	movi	$acc_l.8b, #0
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr2b

	mov	$ctr2b, $ctr1b
	movi	$acc_m.8b, #0
	b.gt	.L192_enc_blocks_more_than_6

	mov	$ctr7b, $ctr6b
	cmp	$main_end_input_ptr, #80

	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b

	mov	$ctr3b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L192_enc_blocks_more_than_5

	cmp	$main_end_input_ptr, #64
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr1b
	b.gt	.L192_enc_blocks_more_than_4

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr1b

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	cmp	$main_end_input_ptr, #48
	b.gt	.L192_enc_blocks_more_than_3

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	cmp	$main_end_input_ptr, #32
	b.gt	.L192_enc_blocks_more_than_2

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	cmp	$main_end_input_ptr, #16
	mov	$ctr7b, $ctr1b
	b.gt	.L192_enc_blocks_more_than_1

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	b	 .L192_enc_blocks_less_than_1
.L192_enc_blocks_more_than_7:						@ blocks left >  7
	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-7 block  - store result

	rev64	$res0b, $res1b						@ GHASH final-7 block
	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-6 block - load plaintext

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high

	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d			 	@ GHASH final-7 block - mid
	eor3	$res1b, $ctr_t1b, $ctr1b, $t1.16b			@ AES final-6 block - result
.L192_enc_blocks_more_than_6:						@ blocks left >  6

	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-6 block - store result

	rev64	$res0b, $res1b						@ GHASH final-6 block

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-5 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid

	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low
	eor3	$res1b, $ctr_t1b, $ctr2b, $t1.16b			@ AES final-5 block - result

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid

	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
.L192_enc_blocks_more_than_5:						@ blocks left >  5

	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-5 block - store result

	rev64	$res0b, $res1b						@ GHASH final-5 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-4 block - load plaintext
	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid
	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low
	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid

	eor3	$res1b, $ctr_t1b, $ctr3b, $t1.16b			@ AES final-4 block - result
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
.L192_enc_blocks_more_than_4:						@ blocks left >  4

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-4 block - store result

	rev64	$res0b, $res1b						@ GHASH final-4 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-3 block - load plaintext
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid

	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low

	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
	eor3	$res1b, $ctr_t1b, $ctr4b, $t1.16b			@ AES final-3 block - result
.L192_enc_blocks_more_than_3:						@ blocks left >  3

	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-3 block - store result

	rev64	$res0b, $res1b						@ GHASH final-3 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-2 block - load plaintext
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid

	eor3	$res1b, $ctr_t1b, $ctr5b, $t1.16b			@ AES final-2 block - result
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low

	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high
	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high
.L192_enc_blocks_more_than_2:						@ blocks left >  2

	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-2 block - store result

	rev64	$res0b, $res1b						@ GHASH final-2 block
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-1 block - load plaintext
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid

	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low
	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid
	eor3	$res1b, $ctr_t1b, $ctr6b, $t1.16b			@ AES final-1 block - result
.L192_enc_blocks_more_than_1:						@ blocks left >  1

	ldr	$h2q, [$current_tag, #64]				@ load h1l | h1h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-1 block - store result

	rev64	$res0b, $res1b						@ GHASH final-1 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid
	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low
	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final block - load plaintext
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid

	eor3	$res1b, $ctr_t1b, $ctr7b, $t1.16b			@ AES final block - result
	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high
.L192_enc_blocks_less_than_1:						@ blocks left <= 1

	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff
	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	sub	$bit_length, $bit_length, #128				@ bit_length -= 128

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])

	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	cmp	$bit_length, #64
	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff

	csel	$temp2_x, $temp1_x, $temp0_x, lt
	csel	$temp3_x, $temp0_x, xzr, lt

	mov	$ctr0.d[1], $temp3_x
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8

	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored
	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits

	rev64	$res0b, $res1b						@ GHASH final block
	bif	$res1b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing

	st1	{ $res1b}, [$output_ptr]				@ store all 16B

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid
	pmull2  $rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high
	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low

	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid

	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment

	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b

	str	$rtmp_ctrq, [$counter]					@ store the updated counter
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up

	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment

	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
		ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]

	mov	x0, $byte_length					@ return sizes

	ldp	d10, d11, [sp, #16]
	ldp	d12, d13, [sp, #32]
	ldp	d14, d15, [sp, #48]
	ldp	d8, d9, [sp], #80
	ret

.L192_enc_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_enc_192_kernel,.-unroll8_eor3_aes_gcm_enc_192_kernel
___

#########################################################################################
# size_t unroll8_eor3_aes_gcm_dec_192_kernel(const uint8_t * ciphertext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * plaintext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_dec_192_kernel
.type   unroll8_eor3_aes_gcm_dec_192_kernel,%function
.align  4
unroll8_eor3_aes_gcm_dec_192_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L192_dec_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
        mov     x5, #0xc200000000000000
	stp     x5, xzr, [sp, #64]
	add     $modulo_constant, sp, #64

	mov	$main_end_input_ptr, $byte_length
	ld1	{ $ctr0b}, [$counter]					@ CTR block 0
	ld1	{ $acc_lb}, [$current_tag]

		mov	$constant_temp, #0x100000000			@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 0

	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1

	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 1

	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 1

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2

	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3

	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4

	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5

	sub	$main_end_input_ptr, $main_end_input_ptr, #1		@ byte_len - 1

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 7

	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7

	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 8
	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80	@ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 8

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 8

	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3		@ end_input_ptr
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 9

	ld1	{ $acc_lb}, [$current_tag]
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb

	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 9
	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 9
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 9

	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 9

	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 9
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 9

	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 10

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 10

	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 10
	ldr	$rk12q, [$cc, #192]					@ load rk12

	aese	$ctr0b, $rk11						@ AES block 0 - round 11
	aese	$ctr1b, $rk11						@ AES block 1 - round 11
	aese	$ctr4b, $rk11						@ AES block 4 - round 11

	aese	$ctr6b, $rk11						@ AES block 6 - round 11
	aese	$ctr5b, $rk11						@ AES block 5 - round 11
	aese	$ctr7b, $rk11						@ AES block 7 - round 11

	aese	$ctr2b, $rk11						@ AES block 2 - round 11
	aese	$ctr3b, $rk11						@ AES block 3 - round 11
	b.ge	.L192_dec_tail						@ handle tail

	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 0, 1 - load ciphertext

	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 2, 3 - load ciphertext

	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 4, 5 - load ciphertext

	eor3	$ctr1b, $res1b, $ctr1b, $rk12				@ AES block 1 - result
	eor3	$ctr0b, $res0b, $ctr0b, $rk12				@ AES block 0 - result
	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9
	eor3	$ctr3b, $res3b, $ctr3b, $rk12				@ AES block 3 - result

	eor3	$ctr2b, $res2b, $ctr2b, $rk12				@ AES block 2 - result
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 2, 3 - store result
	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 6, 7 - load ciphertext

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10

	eor3	$ctr4b, $res4b, $ctr4b, $rk12				@ AES block 4 - result

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11

	eor3	$ctr5b, $res5b, $ctr5b, $rk12				@ AES block 5 - result
	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 4, 5 - store result
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks

	eor3	$ctr6b, $res6b, $ctr6b, $rk12				@ AES block 6 - result
	eor3	$ctr7b, $res7b, $ctr7b, $rk12				@ AES block 7 - result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12
	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 6, 7 - store result
	b.ge	.L192_dec_prepretail					@ do prepretail

.L192_dec_main_loop:							@ main loop start
	rev64	$res1b, $res1b						@ GHASH block 8k+1
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	rev64	$res0b, $res0b						@ GHASH block 8k
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	rev64	$res4b, $res4b						@ GHASH block 8k+4
	rev64	$res3b, $res3b						@ GHASH block 8k+3

	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	rev64	$res5b, $res5b						@ GHASH block 8k+5

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0

	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8

	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high

	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high

	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	rev64	$res7b, $res7b						@ GHASH block 8k+7

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5

	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	rev64	$res6b, $res6b						@ GHASH block 8k+6

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high

	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8

	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16

	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9

	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load ciphertext

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load ciphertext

	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment

	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 8k+12, 8k+13 - load ciphertext

	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18
	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	ldr	$rk12q, [$cc, #192]					@ load rk12

	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 8k+14, 8k+15 - load ciphertext
	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10

	aese	$ctr0b, $rk11						@ AES block 8k+8 - round 11
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment
	aese	$ctr1b, $rk11						@ AES block 8k+9 - round 11

	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	aese	$ctr6b, $rk11						@ AES block 8k+14 - round 11
	aese	$ctr3b, $rk11						@ AES block 8k+11 - round 11

	eor3	$ctr0b, $res0b, $ctr0b, $rk12				@ AES block 8k+8 - result
	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10

	aese	$ctr4b, $rk11						@ AES block 8k+12 - round 11
	aese	$ctr2b, $rk11						@ AES block 8k+10 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19

	aese	$ctr7b, $rk11						@ AES block 8k+15 - round 11
	aese	$ctr5b, $rk11						@ AES block 8k+13 - round 11
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low

	eor3	$ctr1b, $res1b, $ctr1b, $rk12				@ AES block 8k+9 - result
	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result
	eor3	$ctr3b, $res3b, $ctr3b, $rk12				@ AES block 8k+11 - result

	eor3	$ctr2b, $res2b, $ctr2b, $rk12				@ AES block 8k+10 - result
	eor3	$ctr7b, $res7b, $ctr7b, $rk12				@ AES block 8k+15 - result
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result

	eor3	$ctr5b, $res5b, $ctr5b, $rk12				@ AES block 8k+13 - result
	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19

	eor3	$ctr4b, $res4b, $ctr4b, $rk12				@ AES block 8k+12 - result
	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 8k+12, 8k+13 - store result
	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL

	eor3	$ctr6b, $res6b, $ctr6b, $rk12				@ AES block 8k+14 - result
	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 8k+14, 8k+15 - store result
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16

	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17
	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20
	b.lt	.L192_dec_main_loop

.L192_dec_prepretail:							@ PREPRETAIL
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	rev64	$res0b, $res0b						@ GHASH block 8k
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0

	rev64	$res3b, $res3b						@ GHASH block 8k+3
	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high

	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1

	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid

	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	rev64	$res5b, $res5b						@ GHASH block 8k+5
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low

	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2

	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3

	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3

	rev64	$res7b, $res7b						@ GHASH block 8k+7

	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	rev64	$res4b, $res4b						@ GHASH block 8k+4

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4

	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4

	rev64	$res6b, $res6b						@ GHASH block 8k+6
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5

	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high

	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5

	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7

	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9

	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9

	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ldr	$rk12q, [$cc, #192]					@ load rk12
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment

	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10

	aese	$ctr0b, $rk11						@ AES block 8k+8 - round 11
	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
	aese	$ctr5b, $rk11						@ AES block 8k+13 - round 11

	aese	$ctr2b, $rk11						@ AES block 8k+10 - round 11
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10

	aese	$ctr6b, $rk11						@ AES block 8k+14 - round 11
	aese	$ctr4b, $rk11						@ AES block 8k+12 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	aese	$ctr3b, $rk11						@ AES block 8k+11 - round 11
	aese	$ctr1b, $rk11						@ AES block 8k+9 - round 11
	aese	$ctr7b, $rk11						@ AES block 8k+15 - round 11

.L192_dec_tail:								@ TAIL

	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr 	@ main_end_input_ptr is number of bytes left to process

	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
        ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$res1q, [$input_ptr], #16				@ AES block 8k+8 - load ciphertext

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8k | h7k
        ext     $h8.16b, $h8.16b, $h8.16b, #8

	mov	$t1.16b, $rk12

	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
        ext     $h6.16b, $h6.16b, $h6.16b, #8
        ext     $h7.16b, $h7.16b, $h7.16b, #8
	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag

	eor3	$res4b, $res1b, $ctr0b, $t1.16b				@ AES block 8k+8 - result
	cmp	$main_end_input_ptr, #112
	b.gt	.L192_dec_blocks_more_than_7

	mov	$ctr7b, $ctr6b
	movi	$acc_h.8b, #0
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b

	cmp	$main_end_input_ptr, #96
	movi	$acc_l.8b, #0
	mov	$ctr3b, $ctr2b

	mov	$ctr2b, $ctr1b
	movi	$acc_m.8b, #0
	b.gt	.L192_dec_blocks_more_than_6

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr1b

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	cmp	$main_end_input_ptr, #80
	b.gt	.L192_dec_blocks_more_than_5

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr1b
	cmp	$main_end_input_ptr, #64

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L192_dec_blocks_more_than_4

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr1b
	cmp	$main_end_input_ptr, #48
	b.gt	.L192_dec_blocks_more_than_3

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	cmp	$main_end_input_ptr, #32

	mov	$ctr6b, $ctr1b
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	b.gt	.L192_dec_blocks_more_than_2

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr7b, $ctr1b
	cmp	$main_end_input_ptr, #16
	b.gt	.L192_dec_blocks_more_than_1

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	b	 .L192_dec_blocks_less_than_1
.L192_dec_blocks_more_than_7:						@ blocks left >  7
	rev64	$res0b, $res1b						@ GHASH final-7 block

	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid
	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid
	ldr	$res1q, [$input_ptr], #16				@ AES final-6 block - load ciphertext

	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-7 block  - store result

	eor3	$res4b, $res1b, $ctr1b, $t1.16b				@ AES final-6 block - result

	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d			 	@ GHASH final-7 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
.L192_dec_blocks_more_than_6:						@ blocks left >  6

	rev64	$res0b, $res1b						@ GHASH final-6 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ldr	$res1q, [$input_ptr], #16				@ AES final-5 block - load ciphertext
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-6 block - store result
	eor3	$res4b, $res1b, $ctr2b, $t1.16b				@ AES final-5 block - result

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high
	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid
	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low
.L192_dec_blocks_more_than_5:						@ blocks left >  5

	rev64	$res0b, $res1b						@ GHASH final-5 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid
	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high

	ldr	$res1q, [$input_ptr], #16				@ AES final-4 block - load ciphertext

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high
	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low

	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-5 block - store result

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
	eor3	$res4b, $res1b, $ctr3b, $t1.16b				@ AES final-4 block - result
.L192_dec_blocks_more_than_4:						@ blocks left >  4

	rev64	$res0b, $res1b						@ GHASH final-4 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ldr	$res1q, [$input_ptr], #16				@ AES final-3 block - load ciphertext
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid
	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low

	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-4 block - store result
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high

	eor3	$res4b, $res1b, $ctr4b, $t1.16b				@ AES final-3 block - result

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high
.L192_dec_blocks_more_than_3:						@ blocks left >  3

	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	rev64	$res0b, $res1b						@ GHASH final-3 block
	ldr	$res1q, [$input_ptr], #16				@ AES final-2 block - load ciphertext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid
	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low

	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-3 block - store result
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid
	eor3	$res4b, $res1b, $ctr5b, $t1.16b				@ AES final-2 block - result

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid

	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
.L192_dec_blocks_more_than_2:						@ blocks left >  2

	rev64	$res0b, $res1b						@ GHASH final-2 block
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid
	ldr	$res1q, [$input_ptr], #16				@ AES final-1 block - load ciphertext

	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high
	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low

	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-2 block - store result

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid
	eor3	$res4b, $res1b, $ctr6b, $t1.16b				@ AES final-1 block - result
.L192_dec_blocks_more_than_1:						@ blocks left >  1

	rev64	$res0b, $res1b						@ GHASH final-1 block
	ldr	$res1q, [$input_ptr], #16				@ AES final block - load ciphertext
	ldr	$h2q, [$current_tag, #64]				@ load h1l | h1h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k

	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-1 block - store result

	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high

	eor3	$res4b, $res1b, $ctr7b, $t1.16b				@ AES final block - result

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high
.L192_dec_blocks_less_than_1:						@ blocks left <= 1

	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b
	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	sub	$bit_length, $bit_length, #128				@ bit_length -= 128
	str	$rtmp_ctrq, [$counter]					@ store the updated counter

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])
	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff

	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff
	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	cmp	$bit_length, #64

	csel	$temp2_x, $temp1_x, $temp0_x, lt
	csel	$temp3_x, $temp0_x, xzr, lt
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8

	mov	$ctr0.d[1], $temp3_x
	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored

	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits
	bif	$res4b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing

	rev64	$res0b, $res1b						@ GHASH final block

	st1	{ $res4b}, [$output_ptr]				@ store all 16B

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid
	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low

	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid
	pmull2  $rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high

	eor	$t10.16b, $acc_hb, $acc_lb				@ MODULO - karatsuba tidy up
	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d			@ MODULO - top 64b align with mid
	ext	$acc_hb, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	eor	$acc_mb, $acc_mb, $t10.16b				@ MODULO - karatsuba tidy up

	eor3	$acc_mb, $acc_mb, $acc_hb, $t11.16b			@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ext	$acc_mb, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment

	eor3	$acc_lb, $acc_lb, $acc_mb, $acc_hb			@ MODULO - fold into low
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]

	mov	x0, $byte_length

	ldp	d10, d11, [sp, #16]
	ldp	d12, d13, [sp, #32]
	ldp	d14, d15, [sp, #48]
	ldp	d8, d9, [sp], #80
	ret

.L192_dec_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_dec_192_kernel,.-unroll8_eor3_aes_gcm_dec_192_kernel
___
}

{

my ($end_input_ptr,$main_end_input_ptr,$temp0_x,$temp1_x)=map("x$_",(4..7));
my ($temp2_x,$temp3_x)=map("x$_",(13..14));
my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$ctr4b,$ctr5b,$ctr6b,$ctr7b,$res0b,$res1b,$res2b,$res3b,$res4b,$res5b,$res6b,$res7b)=map("v$_.16b",(0..15));
my ($ctr0,$ctr1,$ctr2,$ctr3,$ctr4,$ctr5,$ctr6,$ctr7,$res0,$res1,$res2,$res3,$res4,$res5,$res6,$res7)=map("v$_",(0..15));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$ctr4d,$ctr5d,$ctr6d,$ctr7d)=map("d$_",(0..7));
my ($ctr0q,$ctr1q,$ctr2q,$ctr3q,$ctr4q,$ctr5q,$ctr6q,$ctr7q)=map("q$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q,$res4q,$res5q,$res6q,$res7q)=map("q$_",(8..15));

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3,$ctr_t4,$ctr_t5,$ctr_t6,$ctr_t7)=map("v$_",(8..15));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b,$ctr_t4b,$ctr_t5b,$ctr_t6b,$ctr_t7b)=map("v$_.16b",(8..15));
my ($ctr_t0q,$ctr_t1q,$ctr_t2q,$ctr_t3q,$ctr_t4q,$ctr_t5q,$ctr_t6q,$ctr_t7q)=map("q$_",(8..15));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(17..19));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(17..19));

my ($h1,$h12k,$h2,$h3,$h34k,$h4)=map("v$_",(20..25));
my ($h5,$h56k,$h6,$h7,$h78k,$h8)=map("v$_",(20..25));
my ($h1q,$h12kq,$h2q,$h3q,$h34kq,$h4q)=map("q$_",(20..25));
my ($h5q,$h56kq,$h6q,$h7q,$h78kq,$h8q)=map("q$_",(20..25));

my $t0="v16";
my $t0d="d16";

my $t1="v29";
my $t2=$res1;
my $t3=$t1;

my $t4=$res0;
my $t5=$res2;
my $t6=$t0;

my $t7=$res3;
my $t8=$res4;
my $t9=$res5;

my $t10=$res6;
my $t11="v21";
my $t12=$t1;

my $rtmp_ctr="v30";
my $rtmp_ctrq="q30";
my $rctr_inc="v31";
my $rctr_incd="d31";

my $mod_constantd=$t0d;
my $mod_constant=$t0;

my ($rk0,$rk1,$rk2)=map("v$_.16b",(26..28));
my ($rk3,$rk4,$rk5)=map("v$_.16b",(26..28));
my ($rk6,$rk7,$rk8)=map("v$_.16b",(26..28));
my ($rk9,$rk10,$rk11)=map("v$_.16b",(26..28));
my ($rk12,$rk13,$rk14)=map("v$_.16b",(26..28));
my ($rk0q,$rk1q,$rk2q)=map("q$_",(26..28));
my ($rk3q,$rk4q,$rk5q)=map("q$_",(26..28));
my ($rk6q,$rk7q,$rk8q)=map("q$_",(26..28));
my ($rk9q,$rk10q,$rk11q)=map("q$_",(26..28));
my ($rk12q,$rk13q,$rk14q)=map("q$_",(26..28));
my $rk2q1="v28.1q";
my $rk3q1="v26.1q";
my $rk4v="v27";
#########################################################################################
# size_t unroll8_eor3_aes_gcm_enc_256_kernel(const uint8_t * plaintext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * ciphertext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_enc_256_kernel
.type   unroll8_eor3_aes_gcm_enc_256_kernel,%function
.align  4
unroll8_eor3_aes_gcm_enc_256_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L256_enc_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
	mov	x5, #0xc200000000000000
	stp	x5, xzr, [sp, #64]
	add	$modulo_constant, sp, #64

	ld1	{ $ctr0b}, [$counter]					@ CTR block 0

	mov	$main_end_input_ptr, $byte_length

	mov	$constant_temp, #0x100000000			@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp
	sub	$main_end_input_ptr, $main_end_input_ptr, #1		@ byte_len - 1

	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80	@ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5
	ldp	$rk0q, $rk1q, [$cc, #0]				 	@ load rk0, rk1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 0

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1

	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 1

	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3

	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4

	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5

	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 8

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 8

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 8

	ld1	{ $acc_lb}, [$current_tag]
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 9
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 9

	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 9
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 9
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 9

	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 9

	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 10
	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 10
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 9

	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 10

	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 10

	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 11
	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13
	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 11

	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 11
	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 11
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 11

	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 11
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 11
	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 11

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 7
	ldr	$rk14q, [$cc, #224]					@ load rk14

	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 12
	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 12
	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 12

	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 12
	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 12
	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 12

	aese	$ctr2b, $rk13						@ AES block 2 - round 13
	aese	$ctr1b, $rk13						@ AES block 1 - round 13
	aese	$ctr4b, $rk13						@ AES block 4 - round 13

	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 12
	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 12

	aese	$ctr0b, $rk13						@ AES block 0 - round 13
	aese	$ctr5b, $rk13						@ AES block 5 - round 13

	aese	$ctr6b, $rk13						@ AES block 6 - round 13
	aese	$ctr7b, $rk13						@ AES block 7 - round 13
	aese	$ctr3b, $rk13						@ AES block 3 - round 13

	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3		@ end_input_ptr
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	b.ge	.L256_enc_tail						@ handle tail

	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 0, 1 - load plaintext

	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 2, 3 - load plaintext

	eor3	$res0b, $ctr_t0b, $ctr0b, $rk14				@ AES block 0 - result
	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8

	eor3	$res1b, $ctr_t1b, $ctr1b, $rk14				@ AES block 1 - result
	eor3	$res3b, $ctr_t3b, $ctr3b, $rk14				@ AES block 3 - result

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9
	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 4, 5 - load plaintext

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 6, 7 - load plaintext
	eor3	$res2b, $ctr_t2b, $ctr2b, $rk14				@ AES block 2 - result
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 2, 3 - store result

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11

	eor3	$res4b, $ctr_t4b, $ctr4b, $rk14				@ AES block 4 - result

	eor3	$res7b, $ctr_t7b, $ctr7b, $rk14				@ AES block 7 - result
	eor3	$res6b, $ctr_t6b, $ctr6b, $rk14				@ AES block 6 - result
	eor3	$res5b, $ctr_t5b, $ctr5b, $rk14				@ AES block 5 - result

	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 4, 5 - store result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12

	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 6, 7 - store result
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12
	b.ge	.L256_enc_prepretail					@ do prepretail

.L256_enc_main_loop:							@ main loop start
	ldp	$rk0q, $rk1q, [$cc, #0]					@ load rk0, rk1

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	rev64	$res3b, $res3b						@ GHASH block 8k+3
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14
	rev64	$res0b, $res0b						@ GHASH block 8k

	rev64	$res4b, $res4b						@ GHASH block 8k+4
	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	eor	$res0b, $res0b, $acc_lb				 	@ PRE 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	rev64	$res6b, $res6b						@ GHASH block 8k+6
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high

	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	rev64	$res2b, $res2b						@ GHASH block 8k+2

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3

	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	rev64	$res5b, $res5b						@ GHASH block 8k+5

	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low

	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4

	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4

	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	rev64	$res7b, $res7b						@ GHASH block 8k+7

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5

	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5

	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6

	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6

	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low

	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7

	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8

	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8

	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9

	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9

	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid

	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9

	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10

	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high

	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13
	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16

	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment
	ldp	$ctr_t0q, $ctr_t1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load plaintext
	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 11

	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 11

	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 11
	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 11

	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 11

	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 12
	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 11

	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 12
	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 12
	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17
	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 11
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up

	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 12
	ldr	$rk14q, [$cc, #224]					@ load rk14
	aese	$ctr7b, $rk13						@ AES block 8k+15 - round 13

	ldp	$ctr_t2q, $ctr_t3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load plaintext
	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 12
	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 12

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 12
	ldp	$ctr_t4q, $ctr_t5q, [$input_ptr], #32			@ AES block 4, 5 - load plaintext

	ldp	$ctr_t6q, $ctr_t7q, [$input_ptr], #32			@ AES block 6, 7 - load plaintext
	aese	$ctr2b, $rk13						@ AES block 8k+10 - round 13
	aese	$ctr4b, $rk13						@ AES block 8k+12 - round 13

	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18
	aese	$ctr5b, $rk13						@ AES block 8k+13 - round 13

	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 12
	aese	$ctr3b, $rk13						@ AES block 8k+11 - round 13
	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL

	eor3	$res2b, $ctr_t2b, $ctr2b, $rk14				@ AES block 8k+10 - result
	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19

	aese	$ctr0b, $rk13						@ AES block 8k+8 - round 13
	aese	$ctr6b, $rk13						@ AES block 8k+14 - round 13
	eor3	$res5b, $ctr_t5b, $ctr5b, $rk14				@ AES block 5 - result

	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr1b, $rk13						@ AES block 8k+9 - round 13

	eor3	$res4b, $ctr_t4b, $ctr4b, $rk14				@ AES block 4 - result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20
	eor3	$res3b, $ctr_t3b, $ctr3b, $rk14				@ AES block 8k+11 - result

	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19
	eor3	$res1b, $ctr_t1b, $ctr1b, $rk14				@ AES block 8k+9 - result
	eor3	$res0b, $ctr_t0b, $ctr0b, $rk14				@ AES block 8k+8 - result

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20
	stp	$res0q, $res1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result
	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18

	eor3	$res7b, $ctr_t7b, $ctr7b, $rk14				@ AES block 7 - result
	eor3	$acc_lb, $acc_lb, $t11.16b, $acc_hb		 	@ MODULO - fold into low
	stp	$res2q, $res3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result

	eor3	$res6b, $ctr_t6b, $ctr6b, $rk14				@ AES block 6 - result
	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17
	stp	$res4q, $res5q, [$output_ptr], #32			@ AES block 4, 5 - store result

	stp	$res6q, $res7q, [$output_ptr], #32			@ AES block 6, 7 - store result
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16
	b.lt	.L256_enc_main_loop

.L256_enc_prepretail:							@ PREPRETAIL
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldp	$rk0q, $rk1q, [$cc, #0]					@ load rk0, rk1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	rev64	$res2b, $res2b						@ GHASH block 8k+2

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	rev64	$res5b, $res5b						@ GHASH block 8k+5
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0

	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0

	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0

	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	rev64	$res0b, $res0b						@ GHASH block 8k
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1

	rev64	$res1b, $res1b						@ GHASH block 8k+1
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1

	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1

	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	eor	$res0b, $res0b, $acc_lb					@ PRE 1

	rev64	$res3b, $res3b						@ GHASH block 8k+3
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	rev64	$res6b, $res6b						@ GHASH block 8k+6
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high

	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high

	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3

	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4

	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5
	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4

	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid

	rev64	$res4b, $res4b						@ GHASH block 8k+4
	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5

	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	rev64	$res7b, $res7b						@ GHASH block 8k+7
	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6

	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7

	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7

	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7

	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8

	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high
	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid

	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high

	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low

	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9

	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9

	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10

	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 11

	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13
	ext	$t11.16b, $acc_hb, $acc_hb, #8			 	@ MODULO - other top alignment
	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 11

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 11
	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 11

	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 11
	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 11
	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 11

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 11
	ldr	$rk14q, [$cc, #224]					@ load rk14

	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 12
	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 12
	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 12

	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 12
	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 12
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment

	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 12
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 12
	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 12
	aese	$ctr0b, $rk13						@ AES block 8k+8 - round 13

	eor3	$acc_lb, $acc_lb, $t11.16b, $acc_hb		 	@ MODULO - fold into low
	aese	$ctr5b, $rk13						@ AES block 8k+13 - round 13
	aese	$ctr1b, $rk13						@ AES block 8k+9 - round 13

	aese	$ctr3b, $rk13						@ AES block 8k+11 - round 13
	aese	$ctr4b, $rk13						@ AES block 8k+12 - round 13
	aese	$ctr7b, $rk13						@ AES block 8k+15 - round 13

	aese	$ctr2b, $rk13						@ AES block 8k+10 - round 13
	aese	$ctr6b, $rk13						@ AES block 8k+14 - round 13
.L256_enc_tail:								@ TAIL

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8l | h8h
        ext     $h8.16b, $h8.16b, $h8.16b, #8
	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr		@ main_end_input_ptr is number of bytes left to process

	ldr	$ctr_t0q, [$input_ptr], #16				@ AES block 8k+8 - load plaintext

	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
        ext     $h5.16b, $h5.16b, $h5.16b, #8

	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag
	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
        ext     $h6.16b, $h6.16b, $h6.16b, #8
        ext     $h7.16b, $h7.16b, $h7.16b, #8
	mov	$t1.16b, $rk14

	cmp	$main_end_input_ptr, #112
	eor3	$res1b, $ctr_t0b, $ctr0b, $t1.16b				@ AES block 8k+8 - result
	b.gt	.L256_enc_blocks_more_than_7

	movi	$acc_l.8b, #0
	mov	$ctr7b, $ctr6b
	movi	$acc_h.8b, #0

	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b

	mov	$ctr3b, $ctr2b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr2b, $ctr1b

	movi	$acc_m.8b, #0
	cmp	$main_end_input_ptr, #96
	b.gt	.L256_enc_blocks_more_than_6

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b
	cmp	$main_end_input_ptr, #80

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr1b

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L256_enc_blocks_more_than_5

	mov	$ctr7b, $ctr6b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr4b

	cmp	$main_end_input_ptr, #64
	mov	$ctr4b, $ctr1b
	b.gt	.L256_enc_blocks_more_than_4

	cmp	$main_end_input_ptr, #48
	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L256_enc_blocks_more_than_3

	cmp	$main_end_input_ptr, #32
	mov	$ctr7b, $ctr6b
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	mov	$ctr6b, $ctr1b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	b.gt	.L256_enc_blocks_more_than_2

	mov	$ctr7b, $ctr1b

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	cmp	$main_end_input_ptr, #16
	b.gt	.L256_enc_blocks_more_than_1

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	b	 .L256_enc_blocks_less_than_1
.L256_enc_blocks_more_than_7:						@ blocks left >  7
	st1	{ $res1b}, [$output_ptr], #16				@ AES final-7 block  - store result

	rev64	$res0b, $res1b						@ GHASH final-7 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-6 block - load plaintext

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid
	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid
	eor3	$res1b, $ctr_t1b, $ctr1b, $t1.16b			@ AES final-6 block - result

	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d				@ GHASH final-7 block - mid
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low
.L256_enc_blocks_more_than_6:						@ blocks left >  6

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-6 block - store result

	rev64	$res0b, $res1b						@ GHASH final-6 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-5 block - load plaintext

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid

	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid
	eor3	$res1b, $ctr_t1b, $ctr2b, $t1.16b			@ AES final-5 block - result

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high
.L256_enc_blocks_more_than_5:						@ blocks left >  5

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-5 block - store result

	rev64	$res0b, $res1b						@ GHASH final-5 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid

	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-4 block - load plaintext
	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low

	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
	eor3	$res1b, $ctr_t1b, $ctr3b, $t1.16b			@ AES final-4 block - result
.L256_enc_blocks_more_than_4:						@ blocks left >  4

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-4 block - store result

	rev64	$res0b, $res1b						@ GHASH final-4 block

	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-3 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high

	eor3	$res1b, $ctr_t1b, $ctr4b, $t1.16b			@ AES final-3 block - result
	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low

	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high
.L256_enc_blocks_more_than_3:						@ blocks left >  3

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-3 block - store result

	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	rev64	$res0b, $res1b						@ GHASH final-3 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid
	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-2 block - load plaintext

	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low

	eor3	$res1b, $ctr_t1b, $ctr5b, $t1.16b			@ AES final-2 block - result
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low
.L256_enc_blocks_more_than_2:						@ blocks left >  2

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8

	st1	{ $res1b}, [$output_ptr], #16			 	@ AES final-2 block - store result

	rev64	$res0b, $res1b						@ GHASH final-2 block
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final-1 block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high
	eor3	$res1b, $ctr_t1b, $ctr6b, $t1.16b			@ AES final-1 block - result

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high

	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid
	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low
.L256_enc_blocks_more_than_1:						@ blocks left >  1

	st1	{ $res1b}, [$output_ptr], #16				@ AES final-1 block - store result

	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	rev64	$res0b, $res1b						@ GHASH final-1 block
	ldr	$ctr_t1q, [$input_ptr], #16				@ AES final block - load plaintext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid
	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high

	eor3	$res1b, $ctr_t1b, $ctr7b, $t1.16b			@ AES final block - result
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high

	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low
	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
.L256_enc_blocks_less_than_1:						@ blocks left <= 1

	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	sub	$bit_length, $bit_length, #128				@ bit_length -= 128

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])

	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff
	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	cmp	$bit_length, #64
	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff

	csel	$temp3_x, $temp0_x, xzr, lt
	csel	$temp2_x, $temp1_x, $temp0_x, lt

	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8

	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored
	mov	$ctr0.d[1], $temp3_x

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits

	rev64	$res0b, $res1b						@ GHASH final block

	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b
	bif	$res1b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing
	str	$rtmp_ctrq, [$counter]					@ store the updated counter

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	st1	{ $res1b}, [$output_ptr]				@ store all 16B

	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid
	pmull2	$rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high
	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low

	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid

	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	ext	$t11.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d			@ MODULO - top 64b align with mid

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment

	eor3	$acc_lb, $acc_lb, $acc_hb, $t11.16b		 	@ MODULO - fold into low
		ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]
	mov	x0, $byte_length					@ return sizes

        ldp     d10, d11, [sp, #16]
	ldp     d12, d13, [sp, #32]
	ldp     d14, d15, [sp, #48]
	ldp     d8, d9, [sp], #80
	ret

.L256_enc_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_enc_256_kernel,.-unroll8_eor3_aes_gcm_enc_256_kernel
___

{
#########################################################################################
# size_t unroll8_eor3_aes_gcm_dec_256_kernel(const uint8_t * ciphertext,
#                                            uint64_t plaintext_length,
#                                            uint8_t * plaintext,
#                                            uint64_t *Xi,
#                                            unsigned char ivec[16],
#                                            const void *key);
#
$code.=<<___;
.global unroll8_eor3_aes_gcm_dec_256_kernel
.type   unroll8_eor3_aes_gcm_dec_256_kernel,%function
.align  4
unroll8_eor3_aes_gcm_dec_256_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz	x1, .L256_dec_ret
	stp	d8, d9, [sp, #-80]!
	lsr	$byte_length, $bit_length, #3
	mov	$counter, x4
	mov	$cc, x5
	stp	d10, d11, [sp, #16]
	stp	d12, d13, [sp, #32]
	stp	d14, d15, [sp, #48]
	mov	x5, #0xc200000000000000
	stp	x5, xzr, [sp, #64]
	add	$modulo_constant, sp, #64

	ld1	{ $ctr0b}, [$counter]					@ CTR block 0

	mov	$constant_temp, #0x100000000			@ set up counter increment
	movi	$rctr_inc.16b, #0x0
	mov	$rctr_inc.d[1], $constant_temp
	mov	$main_end_input_ptr, $byte_length

	sub	$main_end_input_ptr, $main_end_input_ptr, #1		@ byte_len - 1

	rev32	$rtmp_ctr.16b, $ctr0.16b				@ set up reversed counter

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 0

	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 1

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 2
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 2
	ldp	$rk0q, $rk1q, [$cc, #0]				  	@ load rk0, rk1

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 3
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 3

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 4
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 4

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 0

	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 5
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 5

	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 0

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 6
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 6

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 7
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 0

	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b		        @ AES block 6 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 0
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b		        @ AES block 7 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b		        @ AES block 6 - round 1
	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b		        @ AES block 4 - round 1
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b		        @ AES block 0 - round 1

	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 1
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 1

	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 1
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 1

	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 2

	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 2
	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 2

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 2
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 2
	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5

	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 3
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 3

	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 3
	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 3

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 3
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 3
	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 3

	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 3

	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 4
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 4

	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 4
	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 4

	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 4
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 4

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 5

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 5

	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 5

	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 5

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 5

	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 6

	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 6
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 6
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 7
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 7

	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 7
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 7

	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 7
	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 7

	and	$main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffff80 @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 8

	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 8
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 8
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 8

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 8
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 8

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 9

	ld1	{ $acc_lb}, [$current_tag]
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	add	$end_input_ptr, $input_ptr, $bit_length, lsr #3 @ end_input_ptr
	add	$main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 9
	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 9

	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 9
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 9

	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 9

	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 9

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 10

	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 10

	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 10
	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13

	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s @ CTR block 7

	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 11
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 11
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 11

	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 11
	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 11
	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 11

	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 11
	ldr	$rk14q, [$cc, #224]					@ load rk14

	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b			@ AES block 1 - round 12
	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 4 - round 12
	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 5 - round 12

	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks
	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 3 - round 12
	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 2 - round 12

	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 6 - round 12
	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 0 - round 12
	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 7 - round 12

	aese	$ctr5b, $rk13						@ AES block 5 - round 13
	aese	$ctr1b, $rk13						@ AES block 1 - round 13
	aese	$ctr2b, $rk13						@ AES block 2 - round 13

	aese	$ctr0b, $rk13						@ AES block 0 - round 13
	aese	$ctr4b, $rk13						@ AES block 4 - round 13
	aese	$ctr6b, $rk13						@ AES block 6 - round 13

	aese	$ctr3b, $rk13						@ AES block 3 - round 13
	aese	$ctr7b, $rk13						@ AES block 7 - round 13
	b.ge	.L256_dec_tail						@ handle tail

	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 0, 1 - load ciphertext

	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 2, 3 - load ciphertext

	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 4, 5 - load ciphertext

	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 6, 7 - load ciphertext
	cmp	$input_ptr, $main_end_input_ptr				@ check if we have <= 8 blocks

	eor3	$ctr1b, $res1b, $ctr1b, $rk14				@ AES block 1 - result
	eor3	$ctr0b, $res0b, $ctr0b, $rk14				@ AES block 0 - result
	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 0, 1 - store result

	rev32	$ctr0.16b, $rtmp_ctr.16b				@ CTR block 8
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8
	eor3	$ctr3b, $res3b, $ctr3b, $rk14				@ AES block 3 - result

	eor3	$ctr5b, $res5b, $ctr5b, $rk14				@ AES block 5 - result

	eor3	$ctr4b, $res4b, $ctr4b, $rk14				@ AES block 4 - result
	rev32	$ctr1.16b, $rtmp_ctr.16b				@ CTR block 9
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 9

	eor3	$ctr2b, $res2b, $ctr2b, $rk14				@ AES block 2 - result
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 2, 3 - store result

	rev32	$ctr2.16b, $rtmp_ctr.16b				@ CTR block 10
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 10

	eor3	$ctr6b, $res6b, $ctr6b, $rk14				@ AES block 6 - result

	rev32	$ctr3.16b, $rtmp_ctr.16b				@ CTR block 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 11
	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 4, 5 - store result

	eor3	$ctr7b, $res7b, $ctr7b, $rk14				@ AES block 7 - result
	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 6, 7 - store result

	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 12
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 12
	b.ge	.L256_dec_prepretail					@ do prepretail

.L256_dec_main_loop:							@ main loop start
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	ldp	$rk0q, $rk1q, [$cc, #0]					@ load rk0, rk1
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	rev64	$res1b, $res1b						@ GHASH block 8k+1
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14
	rev64	$res0b, $res0b						@ GHASH block 8k

	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	rev64	$res4b, $res4b						@ GHASH block 8k+4
	rev64	$res3b, $res3b						@ GHASH block 8k+3

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	rev64	$res7b, $res7b						@ GHASH block 8k+7

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0

	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0

	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3

	eor	$res0b, $res0b, $acc_lb					@ PRE 1
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1

	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1

	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1

	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2
	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2

	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid

	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high
	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high

	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4

	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid
	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low

	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5

	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	rev64	$res5b, $res5b						@ GHASH block 8k+5

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid
	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	rev64	$res6b, $res6b						@ GHASH block 8k+6
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7
	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9

	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7

	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7
	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7

	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low
	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8

	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8

	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8
	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high

	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8

	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9

	ldp	$res0q, $res1q, [$input_ptr], #32			@ AES block 8k+8, 8k+9 - load ciphertext
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9

	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high

	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10

	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low

	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid
	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high

	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10
	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10

	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10

	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	rev32	$h1.16b, $rtmp_ctr.16b					@ CTR block 8k+16
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+16
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 11
	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13

	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 11
	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 11

	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid
	rev32	$h2.16b, $rtmp_ctr.16b					@ CTR block 8k+17
	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 11

	ldp	$res2q, $res3q, [$input_ptr], #32			@ AES block 8k+10, 8k+11 - load ciphertext
	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 11
	ext	$t11.16b, $acc_hb, $acc_hb, #8				 @ MODULO - other top alignment

	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 11
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+17
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 11

	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 12
	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 12
	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 12

	rev32	$h3.16b, $rtmp_ctr.16b					@ CTR block 8k+18
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+18
	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d			@ MODULO - top 64b align with mid

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up
	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 12
	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 11

	ldr	$rk14q, [$cc, #224]					@ load rk14
	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 12
	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 12

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid
	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 12
	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 12

	ldp	$res4q, $res5q, [$input_ptr], #32			@ AES block 8k+12, 8k+13 - load ciphertext
	aese	$ctr1b, $rk13						@ AES block 8k+9 - round 13
	aese	$ctr2b, $rk13						@ AES block 8k+10 - round 13

	ldp	$res6q, $res7q, [$input_ptr], #32			@ AES block 8k+14, 8k+15 - load ciphertext
	aese	$ctr0b, $rk13						@ AES block 8k+8 - round 13
	aese	$ctr5b, $rk13						@ AES block 8k+13 - round 13

	rev32	$h4.16b, $rtmp_ctr.16b					@ CTR block 8k+19
	eor3	$ctr2b, $res2b, $ctr2b, $rk14				@ AES block 8k+10 - result
	eor3	$ctr1b, $res1b, $ctr1b, $rk14				@ AES block 8k+9 - result

	ext	$t11.16b, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment
	aese	$ctr7b, $rk13						@ AES block 8k+15 - round 13

	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+19
	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr4b, $rk13						@ AES block 8k+12 - round 13

	eor3	$ctr5b, $res5b, $ctr5b, $rk14				@ AES block 8k+13 - result
	eor3	$ctr0b, $res0b, $ctr0b, $rk14				@ AES block 8k+8 - result
	aese	$ctr3b, $rk13						@ AES block 8k+11 - round 13

	stp	$ctr0q, $ctr1q, [$output_ptr], #32			@ AES block 8k+8, 8k+9 - store result
	mov	$ctr0.16b, $h1.16b					@ CTR block 8k+16
	eor3	$ctr4b, $res4b, $ctr4b, $rk14				@ AES block 8k+12 - result

	eor3	$acc_lb, $acc_lb, $t11.16b, $acc_hb		 	@ MODULO - fold into low
	eor3	$ctr3b, $res3b, $ctr3b, $rk14				@ AES block 8k+11 - result
	stp	$ctr2q, $ctr3q, [$output_ptr], #32			@ AES block 8k+10, 8k+11 - store result

	mov	$ctr3.16b, $h4.16b					@ CTR block 8k+19
	mov	$ctr2.16b, $h3.16b					@ CTR block 8k+18
	aese	$ctr6b, $rk13						@ AES block 8k+14 - round 13

	mov	$ctr1.16b, $h2.16b					@ CTR block 8k+17
	stp	$ctr4q, $ctr5q, [$output_ptr], #32			@ AES block 8k+12, 8k+13 - store result
	eor3	$ctr7b, $res7b, $ctr7b, $rk14				@ AES block 8k+15 - result

	eor3	$ctr6b, $res6b, $ctr6b, $rk14				@ AES block 8k+14 - result
	rev32	$ctr4.16b, $rtmp_ctr.16b				@ CTR block 8k+20
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+20

	cmp	$input_ptr, $main_end_input_ptr				@ LOOP CONTROL
	stp	$ctr6q, $ctr7q, [$output_ptr], #32			@ AES block 8k+14, 8k+15 - store result
	b.lt	.L256_dec_main_loop

.L256_dec_prepretail:							@ PREPRETAIL
	ldp	$rk0q, $rk1q, [$cc, #0]					@ load rk0, rk1
	rev32	$ctr5.16b, $rtmp_ctr.16b				@ CTR block 8k+13
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+13

	rev64	$res4b, $res4b						@ GHASH block 8k+4
	ldr	$h56kq, [$current_tag, #144]				@ load h6k | h5k
	ldr	$h78kq, [$current_tag, #192]				@ load h8k | h7k

	rev32	$ctr6.16b, $rtmp_ctr.16b				@ CTR block 8k+14
	rev64	$res0b, $res0b						@ GHASH block 8k
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+14

	ext	$acc_lb, $acc_lb, $acc_lb, #8				@ PRE 0
	ldr	$h7q, [$current_tag, #176]				@ load h7l | h7h
	ext     $h7.16b, $h7.16b, $h7.16b, #8
	ldr	$h8q, [$current_tag, #208]				@ load h8l | h8h
	ext     $h8.16b, $h8.16b, $h8.16b, #8
	rev64	$res1b, $res1b						@ GHASH block 8k+1

	rev32	$ctr7.16b, $rtmp_ctr.16b				@ CTR block 8k+15
	rev64	$res2b, $res2b						@ GHASH block 8k+2
	ldr	$h5q, [$current_tag, #128]				@ load h5l | h5h
	ext     $h5.16b, $h5.16b, $h5.16b, #8
	ldr	$h6q, [$current_tag, #160]				@ load h6l | h6h
	ext     $h6.16b, $h6.16b, $h6.16b, #8

	aese	$ctr0b, $rk0  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 0
	aese	$ctr1b, $rk0  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 0
	aese	$ctr4b, $rk0  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 0

	aese	$ctr3b, $rk0  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 0
	aese	$ctr5b, $rk0  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 0
	aese	$ctr6b, $rk0  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 0

	aese	$ctr4b, $rk1  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 1
	aese	$ctr7b, $rk0  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 0
	aese	$ctr2b, $rk0  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 0

	ldp	$rk2q, $rk3q, [$cc, #32]				@ load rk2, rk3
	aese	$ctr0b, $rk1  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 1
	eor	$res0b, $res0b, $acc_lb					@ PRE 1

	aese	$ctr7b, $rk1  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 1
	aese	$ctr6b, $rk1  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 1
	aese	$ctr2b, $rk1  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 1

	aese	$ctr3b, $rk1  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 1
	aese	$ctr1b, $rk1  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 1
	aese	$ctr5b, $rk1  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 1

	pmull2  $t0.1q, $res1.2d, $h7.2d				@ GHASH block 8k+1 - high
	trn1	$acc_m.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH block 8k - low

	rev64	$res3b, $res3b						@ GHASH block 8k+3
	pmull	$h7.1q, $res1.1d, $h7.1d				@ GHASH block 8k+1 - low

	aese	$ctr5b, $rk2  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 2
	aese	$ctr7b, $rk2  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 2
	aese	$ctr1b, $rk2  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 2

	aese	$ctr3b, $rk2  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 2
	aese	$ctr6b, $rk2  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 2
	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH block 8k - high

	aese	$ctr0b, $rk2  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 2
	aese	$ctr7b, $rk3  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 3

	aese	$ctr5b, $rk3  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 3
	rev64	$res6b, $res6b						@ GHASH block 8k+6

	aese	$ctr0b, $rk3  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 3
	aese	$ctr2b, $rk2  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 2
	aese	$ctr6b, $rk3  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 3

	pmull2  $t1.1q, $res2.2d, $h6.2d				@ GHASH block 8k+2 - high
	trn2	$res0.2d, $res1.2d, $res0.2d				@ GHASH block 8k, 8k+1 - mid
	aese	$ctr4b, $rk2  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 2

	ldp	$rk4q, $rk5q, [$cc, #64]				@ load rk4, rk5
	aese	$ctr1b, $rk3  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 3
	pmull2  $t2.1q, $res3.2d, $h5.2d				@ GHASH block 8k+3 - high

	aese	$ctr2b, $rk3  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 3
	eor	$acc_hb, $acc_hb, $t0.16b				@ GHASH block 8k+1 - high
	eor	$res0.16b, $res0.16b, $acc_m.16b			@ GHASH block 8k, 8k+1 - mid

	aese	$ctr4b, $rk3  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 3
	pmull	$h6.1q, $res2.1d, $h6.1d				@ GHASH block 8k+2 - low
	aese	$ctr3b, $rk3  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 3

	eor3	$acc_hb, $acc_hb, $t1.16b, $t2.16b			@ GHASH block 8k+2, 8k+3 - high
	trn1	$t3.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid
	trn2	$res2.2d, $res3.2d, $res2.2d				@ GHASH block 8k+2, 8k+3 - mid

	pmull2  $acc_m.1q, $res0.2d, $h78k.2d				@ GHASH block 8k	- mid
	pmull	$h5.1q, $res3.1d, $h5.1d				@ GHASH block 8k+3 - low
	eor	$acc_lb, $acc_lb, $h7.16b				@ GHASH block 8k+1 - low

	pmull	$h78k.1q, $res0.1d, $h78k.1d				@ GHASH block 8k+1 - mid
	aese	$ctr5b, $rk4  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 4
	aese	$ctr0b, $rk4  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 4

	eor3	$acc_lb, $acc_lb, $h6.16b, $h5.16b			@ GHASH block 8k+2, 8k+3 - low
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8
	aese	$ctr7b, $rk4  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 4

	aese	$ctr2b, $rk4  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 4
	aese	$ctr6b, $rk4  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 4
	eor	$acc_mb, $acc_mb, $h78k.16b				@ GHASH block 8k+1 - mid

	eor	$res2.16b, $res2.16b, $t3.16b				@ GHASH block 8k+2, 8k+3 - mid
	aese	$ctr7b, $rk5  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 5
	aese	$ctr1b, $rk4  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 4

	aese	$ctr2b, $rk5  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 5
	aese	$ctr3b, $rk4  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 4
	aese	$ctr4b, $rk4  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 4

	aese	$ctr1b, $rk5  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 5
	pmull2  $t3.1q, $res2.2d, $h56k.2d				@ GHASH block 8k+2 - mid
	aese	$ctr6b, $rk5  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 5

	aese	$ctr4b, $rk5  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 5
	aese	$ctr3b, $rk5  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 5
	pmull	$h56k.1q, $res2.1d, $h56k.1d				@ GHASH block 8k+3 - mid

	aese	$ctr0b, $rk5  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 5
	aese	$ctr5b, $rk5  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 5
	ldp	$rk6q, $rk7q, [$cc, #96]				@ load rk6, rk7

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	rev64	$res7b, $res7b						@ GHASH block 8k+7
	rev64	$res5b, $res5b						@ GHASH block 8k+5

	eor3	$acc_mb, $acc_mb, $h56k.16b, $t3.16b			@ GHASH block 8k+2, 8k+3 - mid

	trn1	$t6.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr0b, $rk6  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 6
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	aese	$ctr6b, $rk6  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 6

	aese	$ctr5b, $rk6  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 6
	aese	$ctr7b, $rk6  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 6

	pmull2  $t4.1q, $res4.2d, $h4.2d				@ GHASH block 8k+4 - high
	pmull2  $t5.1q, $res5.2d, $h3.2d				@ GHASH block 8k+5 - high
	pmull	$h4.1q, $res4.1d, $h4.1d				@ GHASH block 8k+4 - low

	trn2	$res4.2d, $res5.2d, $res4.2d				@ GHASH block 8k+4, 8k+5 - mid
	pmull	$h3.1q, $res5.1d, $h3.1d				@ GHASH block 8k+5 - low
	trn1	$t9.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr7b, $rk7  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 7
	pmull2  $t7.1q, $res6.2d, $h2.2d				@ GHASH block 8k+6 - high
	aese	$ctr1b, $rk6  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 6

	aese	$ctr2b, $rk6  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 6
	aese	$ctr3b, $rk6  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 6
	aese	$ctr4b, $rk6  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 6

	ldp	$rk8q, $rk9q, [$cc, #128]				@ load rk8, rk9
	pmull	$h2.1q, $res6.1d, $h2.1d				@ GHASH block 8k+6 - low
	aese	$ctr5b, $rk7  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 7

	aese	$ctr1b, $rk7  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 7
	aese	$ctr4b, $rk7  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 7

	aese	$ctr6b, $rk7  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 7
	aese	$ctr2b, $rk7  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 7
	eor3	$acc_hb, $acc_hb, $t4.16b, $t5.16b			@ GHASH block 8k+4, 8k+5 - high

	aese	$ctr0b, $rk7  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 7
	trn2	$res6.2d, $res7.2d, $res6.2d				@ GHASH block 8k+6, 8k+7 - mid
	aese	$ctr3b, $rk7  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 7

	aese	$ctr0b, $rk8  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 8
	aese	$ctr7b, $rk8  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 8
	aese	$ctr4b, $rk8  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 8

	aese	$ctr1b, $rk8  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 8
	aese	$ctr5b, $rk8  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 8
	aese	$ctr6b, $rk8  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 8

	aese	$ctr3b, $rk8  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 8
	aese	$ctr4b, $rk9  \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 9
	eor	$res4.16b, $res4.16b, $t6.16b				@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr0b, $rk9  \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 9
	aese	$ctr1b, $rk9  \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 9
	eor	$res6.16b, $res6.16b, $t9.16b				@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr6b, $rk9  \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 9
	aese	$ctr7b, $rk9  \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 9
	pmull2  $t6.1q, $res4.2d, $h34k.2d				@ GHASH block 8k+4 - mid

	aese	$ctr2b, $rk8  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 8
	pmull	$h34k.1q, $res4.1d, $h34k.1d				@ GHASH block 8k+5 - mid
	pmull2  $t8.1q, $res7.2d, $h1.2d				@ GHASH block 8k+7 - high

	pmull2  $t9.1q, $res6.2d, $h12k.2d				@ GHASH block 8k+6 - mid
	pmull	$h12k.1q, $res6.1d, $h12k.1d				@ GHASH block 8k+7 - mid
	pmull	$h1.1q, $res7.1d, $h1.1d				@ GHASH block 8k+7 - low

	ldp	$rk10q, $rk11q, [$cc, #160]				@ load rk10, rk11
	eor3	$acc_lb, $acc_lb, $h4.16b, $h3.16b			@ GHASH block 8k+4, 8k+5 - low
	eor3	$acc_mb, $acc_mb, $h34k.16b, $t6.16b			@ GHASH block 8k+4, 8k+5 - mid

	aese	$ctr2b, $rk9  \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 9
	aese	$ctr3b, $rk9  \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 9
	aese	$ctr5b, $rk9  \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 9

	eor3	$acc_hb, $acc_hb, $t7.16b, $t8.16b			@ GHASH block 8k+6, 8k+7 - high
	eor3	$acc_lb, $acc_lb, $h2.16b, $h1.16b			@ GHASH block 8k+6, 8k+7 - low
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant

	eor3	$acc_mb, $acc_mb, $h12k.16b, $t9.16b			@ GHASH block 8k+6, 8k+7 - mid

	aese	$ctr4b, $rk10 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 10
	aese	$ctr6b, $rk10 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 10
	aese	$ctr5b, $rk10 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 10

	aese	$ctr0b, $rk10 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 10
	aese	$ctr2b, $rk10 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 10
	aese	$ctr3b, $rk10 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 10

	eor3	$acc_mb, $acc_mb, $acc_hb, $acc_lb		 	@ MODULO - karatsuba tidy up

	aese	$ctr7b, $rk10 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 10
	aese	$ctr1b, $rk10 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 10
	ldp	$rk12q, $rk13q, [$cc, #192]				@ load rk12, rk13

	ext	$t11.16b, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment

	aese	$ctr2b, $rk11 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 11
	aese	$ctr1b, $rk11 \n  aesmc	$ctr1b, $ctr1b			@ AES block 8k+9 - round 11
	aese	$ctr0b, $rk11 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 11

	pmull	$t12.1q, $acc_h.1d, $mod_constant.1d			@ MODULO - top 64b align with mid
	aese	$ctr3b, $rk11 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 11

	aese	$ctr7b, $rk11 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 11
	aese	$ctr6b, $rk11 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 11
	aese	$ctr4b, $rk11 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 11

	aese	$ctr5b, $rk11 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 11
	aese	$ctr3b, $rk12 \n  aesmc	$ctr3b, $ctr3b			@ AES block 8k+11 - round 12

	eor3	$acc_mb, $acc_mb, $t12.16b, $t11.16b			@ MODULO - fold into mid

	aese	$ctr3b, $rk13						@ AES block 8k+11 - round 13
	aese	$ctr2b, $rk12 \n  aesmc	$ctr2b, $ctr2b			@ AES block 8k+10 - round 12
	aese	$ctr6b, $rk12 \n  aesmc	$ctr6b, $ctr6b			@ AES block 8k+14 - round 12

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low
	aese	$ctr4b, $rk12 \n  aesmc	$ctr4b, $ctr4b			@ AES block 8k+12 - round 12
	aese	$ctr7b, $rk12 \n  aesmc	$ctr7b, $ctr7b			@ AES block 8k+15 - round 12

	aese	$ctr0b, $rk12 \n  aesmc	$ctr0b, $ctr0b			@ AES block 8k+8 - round 12
	ldr	$rk14q, [$cc, #224]					@ load rk14
	aese	$ctr1b, $rk12 \n  aesmc	$ctr1b, $ctr1b	        	@ AES block 8k+9 - round 12

	aese	$ctr4b, $rk13						@ AES block 8k+12 - round 13
	ext	$t11.16b, $acc_mb, $acc_mb, #8			 	@ MODULO - other mid alignment
	aese	$ctr5b, $rk12 \n  aesmc	$ctr5b, $ctr5b			@ AES block 8k+13 - round 12

	aese	$ctr6b, $rk13						@ AES block 8k+14 - round 13
	aese	$ctr2b, $rk13						@ AES block 8k+10 - round 13
	aese	$ctr1b, $rk13						@ AES block 8k+9 - round 13

	aese	$ctr5b, $rk13						@ AES block 8k+13 - round 13
	eor3	$acc_lb, $acc_lb, $t11.16b, $acc_hb		 	@ MODULO - fold into low
	add	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s		@ CTR block 8k+15

	aese	$ctr7b, $rk13						@ AES block 8k+15 - round 13
	aese	$ctr0b, $rk13						@ AES block 8k+8 - round 13
.L256_dec_tail:								@ TAIL

	ext	$t0.16b, $acc_lb, $acc_lb, #8				@ prepare final partial tag
	sub	$main_end_input_ptr, $end_input_ptr, $input_ptr		@ main_end_input_ptr is number of bytes left to process
	cmp	$main_end_input_ptr, #112

	ldr	$res1q, [$input_ptr], #16				@ AES block 8k+8 - load ciphertext

	ldp	$h78kq, $h8q, [$current_tag, #192]			@ load h8k | h7k
        ext     $h8.16b, $h8.16b, $h8.16b, #8
	mov	$t1.16b, $rk14

	ldp	$h5q, $h56kq, [$current_tag, #128]			@ load h5l | h5h
        ext     $h5.16b, $h5.16b, $h5.16b, #8

	eor3	$res4b, $res1b, $ctr0b, $t1.16b				@ AES block 8k+8 - result
	ldp	$h6q, $h7q, [$current_tag, #160]			@ load h6l | h6h
        ext     $h6.16b, $h6.16b, $h6.16b, #8
        ext     $h7.16b, $h7.16b, $h7.16b, #8
	b.gt	.L256_dec_blocks_more_than_7

	mov	$ctr7b, $ctr6b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr3b
	movi	$acc_l.8b, #0

	movi	$acc_h.8b, #0
	movi	$acc_m.8b, #0
	mov	$ctr3b, $ctr2b

	cmp	$main_end_input_ptr, #96
	mov	$ctr2b, $ctr1b
	b.gt	.L256_dec_blocks_more_than_6

	mov	$ctr7b, $ctr6b
	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr4b
	cmp	$main_end_input_ptr, #80
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr4b, $ctr3b
	mov	$ctr3b, $ctr1b
	b.gt	.L256_dec_blocks_more_than_5

	cmp	$main_end_input_ptr, #64
	mov	$ctr7b, $ctr6b
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr6b, $ctr5b

	mov	$ctr5b, $ctr4b
	mov	$ctr4b, $ctr1b
	b.gt	.L256_dec_blocks_more_than_4

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b
	cmp	$main_end_input_ptr, #48

	mov	$ctr6b, $ctr5b
	mov	$ctr5b, $ctr1b
	b.gt	.L256_dec_blocks_more_than_3

	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k
	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	mov	$ctr7b, $ctr6b

	cmp	$main_end_input_ptr, #32
	mov	$ctr6b, $ctr1b
	b.gt	.L256_dec_blocks_more_than_2

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s

	mov	$ctr7b, $ctr1b
	cmp	$main_end_input_ptr, #16
	b.gt	.L256_dec_blocks_more_than_1

	sub	$rtmp_ctr.4s, $rtmp_ctr.4s, $rctr_inc.4s
	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	b	 .L256_dec_blocks_less_than_1
.L256_dec_blocks_more_than_7:						@ blocks left >  7
	rev64	$res0b, $res1b						@ GHASH final-7 block
	ldr	$res1q, [$input_ptr], #16				@ AES final-6 block - load ciphertext
	st1	{ $res4b}, [$output_ptr], #16				@ AES final-7 block  - store result

	ins	$acc_m.d[0], $h78k.d[1]					@ GHASH final-7 block - mid

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-7 block - mid
	eor3	$res4b, $res1b, $ctr1b, $t1.16b				@ AES final-6 block - result

	pmull2  $acc_h.1q, $res0.2d, $h8.2d				@ GHASH final-7 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-7 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$acc_l.1q, $res0.1d, $h8.1d				@ GHASH final-7 block - low
	pmull	$acc_m.1q, $rk4v.1d, $acc_m.1d			 	@ GHASH final-7 block - mid
.L256_dec_blocks_more_than_6:						@ blocks left >  6

	rev64	$res0b, $res1b						@ GHASH final-6 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	ldr	$res1q, [$input_ptr], #16				@ AES final-5 block - load ciphertext
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-6 block - mid
	st1	{ $res4b}, [$output_ptr], #16				@ AES final-6 block - store result
	pmull2  $rk2q1, $res0.2d, $h7.2d				@ GHASH final-6 block - high

	pmull	$rk3q1, $res0.1d, $h7.1d				@ GHASH final-6 block - low

	eor3	$res4b, $res1b, $ctr2b, $t1.16b				@ AES final-5 block - result
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-6 block - low
	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-6 block - mid

	pmull	$rk4v.1q, $rk4v.1d, $h78k.1d				@ GHASH final-6 block - mid

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-6 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-6 block - high
.L256_dec_blocks_more_than_5:						@ blocks left >  5

	rev64	$res0b, $res1b						@ GHASH final-5 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	pmull2  $rk2q1, $res0.2d, $h6.2d				@ GHASH final-5 block - high
	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-5 block - mid

	ldr	$res1q, [$input_ptr], #16				@ AES final-4 block - load ciphertext

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-5 block - mid
	st1	{ $res4b}, [$output_ptr], #16			  	@ AES final-5 block - store result

	pmull	$rk3q1, $res0.1d, $h6.1d				@ GHASH final-5 block - low
	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-5 block - mid

	pmull2  $rk4v.1q, $rk4v.2d, $h56k.2d				@ GHASH final-5 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-5 block - high
	eor3	$res4b, $res1b, $ctr3b, $t1.16b				@ AES final-4 block - result
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-5 block - low

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-5 block - mid
	movi	$t0.8b, #0						@ suppress further partial tag feed in
.L256_dec_blocks_more_than_4:						@ blocks left >  4

	rev64	$res0b, $res1b						@ GHASH final-4 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-4 block - mid
	ldr	$res1q, [$input_ptr], #16				@ AES final-3 block - load ciphertext

	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$rk3q1, $res0.1d, $h5.1d				@ GHASH final-4 block - low
	pmull2  $rk2q1, $res0.2d, $h5.2d				@ GHASH final-4 block - high

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-4 block - mid

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-4 block - high

	pmull	$rk4v.1q, $rk4v.1d, $h56k.1d				@ GHASH final-4 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-4 block - low
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-4 block - store result

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-4 block - mid
	eor3	$res4b, $res1b, $ctr4b, $t1.16b				@ AES final-3 block - result
.L256_dec_blocks_more_than_3:						@ blocks left >  3

	ldr	$h4q, [$current_tag, #112]				@ load h4l | h4h
	ext     $h4.16b, $h4.16b, $h4.16b, #8
	rev64	$res0b, $res1b						@ GHASH final-3 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag
	ldr	$res1q, [$input_ptr], #16				@ AES final-2 block - load ciphertext
	ldr	$h34kq, [$current_tag, #96]				@ load h4k | h3k

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-3 block - mid
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-3 block - store result

	eor3	$res4b, $res1b, $ctr5b, $t1.16b				@ AES final-2 block - result

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-3 block - mid

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-3 block - mid
	pmull	$rk3q1, $res0.1d, $h4.1d				@ GHASH final-3 block - low
	pmull2  $rk2q1, $res0.2d, $h4.2d				@ GHASH final-3 block - high

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	pmull2  $rk4v.1q, $rk4v.2d, $h34k.2d				@ GHASH final-3 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-3 block - low

	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-3 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-3 block - mid
.L256_dec_blocks_more_than_2:						@ blocks left >  2

	rev64	$res0b, $res1b						@ GHASH final-2 block

	ldr	$h3q, [$current_tag, #80]				@ load h3l | h3h
	ext     $h3.16b, $h3.16b, $h3.16b, #8
	ldr	$res1q, [$input_ptr], #16				@ AES final-1 block - load ciphertext

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-2 block - mid

	pmull	$rk3q1, $res0.1d, $h3.1d				@ GHASH final-2 block - low
	st1	{ $res4b}, [$output_ptr], #16			  	@ AES final-2 block - store result
	eor3	$res4b, $res1b, $ctr6b, $t1.16b				@ AES final-1 block - result

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-2 block - mid
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-2 block - low
	movi	$t0.8b, #0						@ suppress further partial tag feed in

	pmull	$rk4v.1q, $rk4v.1d, $h34k.1d				@ GHASH final-2 block - mid
	pmull2  $rk2q1, $res0.2d, $h3.2d				@ GHASH final-2 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-2 block - mid
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-2 block - high
.L256_dec_blocks_more_than_1:						@ blocks left >  1

	rev64	$res0b, $res1b						@ GHASH final-1 block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$rk4v.d[0], $res0.d[1]					@ GHASH final-1 block - mid
	ldr	$h2q, [$current_tag, #64]				@ load h2l | h2h
	ext     $h2.16b, $h2.16b, $h2.16b, #8

	eor	$rk4v.8b, $rk4v.8b, $res0.8b				@ GHASH final-1 block - mid
	ldr	$res1q, [$input_ptr], #16				@ AES final block - load ciphertext
	st1	{ $res4b}, [$output_ptr], #16			 	@ AES final-1 block - store result

	ldr	$h12kq, [$current_tag, #48]				@ load h2k | h1k
	pmull	$rk3q1, $res0.1d, $h2.1d				@ GHASH final-1 block - low

	ins	$rk4v.d[1], $rk4v.d[0]					@ GHASH final-1 block - mid

	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final-1 block - low

	eor3	$res4b, $res1b, $ctr7b, $t1.16b				@ AES final block - result
	pmull2  $rk2q1, $res0.2d, $h2.2d				@ GHASH final-1 block - high

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d				@ GHASH final-1 block - mid

	movi	$t0.8b, #0						@ suppress further partial tag feed in
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final-1 block - high

	eor	$acc_mb, $acc_mb, $rk4v.16b				@ GHASH final-1 block - mid
.L256_dec_blocks_less_than_1:						@ blocks left <= 1

	ld1	{ $rk0}, [$output_ptr]					@ load existing bytes where the possibly partial last block is to be stored
	mvn	$temp0_x, xzr						@ temp0_x = 0xffffffffffffffff
	and	$bit_length, $bit_length, #127				@ bit_length %= 128

	sub	$bit_length, $bit_length, #128				@ bit_length -= 128
	rev32	$rtmp_ctr.16b, $rtmp_ctr.16b
	str	$rtmp_ctrq, [$counter]					@ store the updated counter

	neg	$bit_length, $bit_length				@ bit_length = 128 - #bits in input (in range [1,128])

	and	$bit_length, $bit_length, #127			 	@ bit_length %= 128

	lsr	$temp0_x, $temp0_x, $bit_length				@ temp0_x is mask for top 64b of last block
	cmp	$bit_length, #64
	mvn	$temp1_x, xzr						@ temp1_x = 0xffffffffffffffff

	csel	$temp3_x, $temp0_x, xzr, lt
	csel	$temp2_x, $temp1_x, $temp0_x, lt

	mov	$ctr0.d[0], $temp2_x					@ ctr0b is mask for last block
	mov	$ctr0.d[1], $temp3_x

	and	$res1b, $res1b, $ctr0b					@ possibly partial last block has zeroes in highest bits
	ldr	$h1q, [$current_tag, #32]				@ load h1l | h1h
	ext     $h1.16b, $h1.16b, $h1.16b, #8
	bif	$res4b, $rk0, $ctr0b					@ insert existing bytes in top end of result before storing

	rev64	$res0b, $res1b						@ GHASH final block

	eor	$res0b, $res0b, $t0.16b					@ feed in partial tag

	ins	$t0.d[0], $res0.d[1]					@ GHASH final block - mid
	pmull2  $rk2q1, $res0.2d, $h1.2d				@ GHASH final block - high

	eor	$t0.8b, $t0.8b, $res0.8b				@ GHASH final block - mid

	pmull	$rk3q1, $res0.1d, $h1.1d				@ GHASH final block - low
	eor	$acc_hb, $acc_hb, $rk2					@ GHASH final block - high

	pmull	$t0.1q, $t0.1d, $h12k.1d				@ GHASH final block - mid

	eor	$acc_mb, $acc_mb, $t0.16b				@ GHASH final block - mid
	ldr	$mod_constantd, [$modulo_constant]			@ MODULO - load modulo constant
	eor	$acc_lb, $acc_lb, $rk3					@ GHASH final block - low

	pmull	$t11.1q, $acc_h.1d, $mod_constant.1d		 	@ MODULO - top 64b align with mid
	eor	$t10.16b, $acc_hb, $acc_lb				@ MODULO - karatsuba tidy up

	ext	$acc_hb, $acc_hb, $acc_hb, #8				@ MODULO - other top alignment
	st1	{ $res4b}, [$output_ptr]				@ store all 16B

	eor	$acc_mb, $acc_mb, $t10.16b				@ MODULO - karatsuba tidy up

	eor	$t11.16b, $acc_hb, $t11.16b				@ MODULO - fold into mid
	eor	$acc_mb, $acc_mb, $t11.16b				@ MODULO - fold into mid

	pmull	$acc_h.1q, $acc_m.1d, $mod_constant.1d			@ MODULO - mid 64b align with low

	ext	$acc_mb, $acc_mb, $acc_mb, #8				@ MODULO - other mid alignment
	eor	$acc_lb, $acc_lb, $acc_hb				@ MODULO - fold into low

	eor	$acc_lb, $acc_lb, $acc_mb				@ MODULO - fold into low
	ext	$acc_lb, $acc_lb, $acc_lb, #8
	rev64	$acc_lb, $acc_lb
	st1	{ $acc_l.16b }, [$current_tag]
	mov	x0, $byte_length

        ldp     d10, d11, [sp, #16]
	ldp     d12, d13, [sp, #32]
	ldp     d14, d15, [sp, #48]
	ldp     d8, d9, [sp], #80
	ret

.L256_dec_ret:
	mov w0, #0x0
	ret
.size unroll8_eor3_aes_gcm_dec_256_kernel,.-unroll8_eor3_aes_gcm_dec_256_kernel
___
}
}

$code.=<<___;
.asciz  "AES GCM module for ARMv8, SPDX BSD-3-Clause by <xiaokang.qian\@arm.com>"
.align  2
#endif
___

{
    my  %opcode = (
    "rax1"    => 0xce608c00,    "eor3"    => 0xce000000,
    "bcax"    => 0xce200000,    "xar"    => 0xce800000    );

    sub unsha3 {
         my ($mnemonic,$arg)=@_;

         $arg =~ m/[qv]([0-9]+)[^,]*,\s*[qv]([0-9]+)[^,]*(?:,\s*[qv]([0-9]+)[^,]*(?:,\s*[qv#]([0-9\-]+))?)?/
         &&
         sprintf ".inst\t0x%08x\t//%s %s",
            $opcode{$mnemonic}|$1|($2<<5)|($3<<16)|(eval($4)<<10),
            $mnemonic,$arg;
    }
    sub unvmov {
        my $arg=shift;

        $arg =~ m/q([0-9]+)#(lo|hi),\s*q([0-9]+)#(lo|hi)/o &&
        sprintf "ins    v%d.d[%d],v%d.d[%d]",$1<8?$1:$1+8,($2 eq "lo")?0:1,
                             $3<8?$3:$3+8,($4 eq "lo")?0:1;
    }

     foreach(split("\n",$code)) {
        s/@\s/\/\//o;               # old->new style commentary
        s/\`([^\`]*)\`/eval($1)/ge;

        m/\bld1r\b/ and s/\.16b/.2d/g    or
        s/\b(eor3|rax1|xar|bcax)\s+(v.*)/unsha3($1,$2)/ge;
        print $_,"\n";
     }
}

close STDOUT or die "error closing STDOUT: $!"; # enforce flush
