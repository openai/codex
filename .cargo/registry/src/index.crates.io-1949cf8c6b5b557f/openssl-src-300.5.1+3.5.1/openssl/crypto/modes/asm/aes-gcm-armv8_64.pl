#! /usr/bin/env perl
# Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
#========================================================================
# Written by Fangming Fang <fangming.fang@arm.com> for the OpenSSL project,
# derived from https://github.com/ARM-software/AArch64cryptolib, original
# author Samuel Lee <Samuel.Lee@arm.com>. The module is, however, dual
# licensed under OpenSSL and CRYPTOGAMS licenses depending on where you
# obtain it. For further details see http://www.openssl.org/~appro/cryptogams/.
#========================================================================
#
# Approach - assume we don't want to reload constants, so reserve ~half of vector register file for constants
#
# main loop to act on 4 16B blocks per iteration, and then do modulo of the accumulated intermediate hashes from the 4 blocks
#
#  ____________________________________________________
# |                                                    |
# | PRE                                                |
# |____________________________________________________|
# |                |                |                  |
# | CTR block 4k+8 | AES block 4k+4 | GHASH block 4k+0 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 4k+9 | AES block 4k+5 | GHASH block 4k+1 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 4k+10| AES block 4k+6 | GHASH block 4k+2 |
# |________________|________________|__________________|
# |                |                |                  |
# | CTR block 4k+11| AES block 4k+7 | GHASH block 4k+3 |
# |________________|____(mostly)____|__________________|
# |                                                    |
# | MODULO                                             |
# |____________________________________________________|
#
# PRE:
#     Ensure previous generated intermediate hash is aligned and merged with result for GHASH 4k+0
# EXT low_acc, low_acc, low_acc, #8
# EOR res_curr (4k+0), res_curr (4k+0), low_acc
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
#     Do AES encryption/decryption on CTR block X and EOR it with input block X. Take 256 bytes key below for example.
#     Doing small trick here of loading input in scalar registers, EORing with last key and then transferring
#     Given we are very constrained in our ASIMD registers this is quite important
#
#     Encrypt:
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
#
# GHASH block X:
#     do 128b karatsuba polynomial multiplication on block
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
# EOR     acc_m, acc_m, acc_h
# EOR     acc_m, acc_m, acc_l                     // Finish off karatsuba processing
# PMULL   t_mod, acc_h, mod_constant
# EXT     acc_h, acc_h, acc_h, #8
# EOR     acc_m, acc_m, acc_h
# EOR     acc_m, acc_m, t_mod
# PMULL   acc_h, acc_m, mod_constant
# EXT     acc_m, acc_m, acc_m, #8
# EOR     acc_l, acc_l, acc_h
# EOR     acc_l, acc_l, acc_m

$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate  ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate ) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

$input_ptr="x0";  #argument block
$bit_length="x1";
$output_ptr="x2";
$current_tag="x3";
$counter="x16";
$cc="x8";

{
my ($end_input_ptr,$main_end_input_ptr,$input_l0,$input_h0)=map("x$_",(4..7));
my ($input_l1,$input_h1,$input_l2,$input_h2,$input_l3,$input_h3)=map("x$_",(19..24));
my ($output_l1,$output_h1,$output_l2,$output_h2,$output_l3,$output_h3)=map("x$_",(19..24));
my ($output_l0,$output_h0)=map("x$_",(6..7));

my $ctr32w="w9";
my ($ctr32x,$ctr96_b64x,$ctr96_t32x,$rctr32x,$rk10_l,$rk10_h,$len)=map("x$_",(9..15));
my ($ctr96_t32w,$rctr32w)=map("w$_",(11..12));

my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$res0b,$res1b,$res2b,$res3b)=map("v$_.16b",(0..7));
my ($ctr0,$ctr1,$ctr2,$ctr3,$res0,$res1,$res2,$res3)=map("v$_",(0..7));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$res0d,$res1d,$res2d,$res3d)=map("d$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q)=map("q$_",(4..7));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(9..11));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(9..11));
my ($acc_hd,$acc_md,$acc_ld)=map("d$_",(9..11));

my ($h1,$h2,$h3,$h4,$h12k,$h34k)=map("v$_",(12..17));
my ($h1q,$h2q,$h3q,$h4q)=map("q$_",(12..15));
my ($h1b,$h2b,$h3b,$h4b)=map("v$_.16b",(12..15));

my $t0="v8";
my $t0d="d8";

my ($t1,$t2,$t3)=map("v$_",(28..30));
my ($t1d,$t2d,$t3d)=map("d$_",(28..30));

my $t4="v8";
my $t4d="d8";
my $t5="v28";
my $t5d="d28";
my $t6="v31";
my $t6d="d31";

my $t7="v4";
my $t7d="d4";
my $t8="v29";
my $t8d="d29";
my $t9="v30";
my $t9d="d30";

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3)=map("v$_",(4..7));
my ($ctr_t0d,$ctr_t1d,$ctr_t2d,$ctr_t3d)=map("d$_",(4..7));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b)=map("v$_.16b",(4..7));

my $mod_constantd="d8";
my $mod_constant="v8";
my $mod_t="v31";

my ($rk0,$rk1,$rk2,$rk3,$rk4,$rk5,$rk6,$rk7,$rk8,$rk9)=map("v$_.16b",(18..27));
my ($rk0s,$rk1s,$rk2s,$rk3s,$rk4s,$rk5s,$rk6s,$rk7s,$rk8s,$rk9s)=map("v$_.4s",(18..27));
my ($rk0q,$rk1q,$rk2q,$rk3q,$rk4q,$rk5q,$rk6q,$rk7q,$rk8q,$rk9q)=map("q$_",(18..27));
my $rk2q1="v20.1q";
my $rk3q1="v21.1q";
my $rk4v="v22";
my $rk4d="d22";

$code=<<___;
#include "arm_arch.h"

#if __ARM_MAX_ARCH__>=8
___
$code.=".arch   armv8-a+crypto\n.text\n"    if ($flavour =~ /64/);
$code.=<<___                    if ($flavour !~ /64/);
.fpu    neon
#ifdef __thumb2__
.syntax        unified
.thumb
# define INST(a,b,c,d) $_byte  c,0xef,a,b
#else
.code  32
# define INST(a,b,c,d) $_byte  a,b,c,0xf2
#endif

.text
___

#########################################################################################
# size_t aes_gcm_enc_128_kernel(const uint8_t * plaintext,
#                               uint64_t plaintext_length,
#                               uint8_t * ciphertext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_enc_128_kernel
.type   aes_gcm_enc_128_kernel,%function
.align  4
aes_gcm_enc_128_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L128_enc_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk10_l, $rk10_h, [$cc, #160]                     @ load rk10
#ifdef __AARCH64EB__
	ror     $rk10_l, $rk10_l, #32
	ror     $rk10_h, $rk10_h, #32
#endif
	ld1     {$acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	lsr     $main_end_input_ptr, $bit_length, #3              @ byte_len
	mov     $len, $main_end_input_ptr

	ld1     {$rk0s}, [$cc], #16								  @ load rk0
	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   @ end_input_ptr
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      @ byte_len - 1

	lsr     $rctr32x, $ctr96_t32x, #32
	ldr     $h4q, [$current_tag, #112]                        @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 1
	rev     $rctr32w, $rctr32w                                @ rev_ctr32

	add     $rctr32w, $rctr32w, #1                            @ increment rev_ctr32
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w
	ld1     {$rk1s}, [$cc], #16								  @ load rk1

	rev     $ctr32w, $rctr32w                                 @ CTR block 1
	add     $rctr32w, $rctr32w, #1                            @ CTR block 1
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 3

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 1
	ld1     { $ctr0b}, [$counter]                             @ special case vector load initial counter so we can start first AES block as quickly as possible

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 2

	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 2

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 2
	rev     $ctr32w, $rctr32w                                 @ CTR block 3

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 3
	ld1     {$rk2s}, [$cc], #16								  @ load rk2

	add     $rctr32w, $rctr32w, #1                            @ CTR block 3
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 3

	ldr     $h3q, [$current_tag, #80]                         @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 0
	ld1     {$rk3s}, [$cc], #16								  @ load rk3

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 0
	ldr     $h1q, [$current_tag, #32]                         @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 0
	ld1     {$rk4s}, [$cc], #16								  @ load rk4

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 0
	ld1     {$rk5s}, [$cc], #16								  @ load rk5

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 1
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      @ h4l | h3l

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 1
	ld1     {$rk6s}, [$cc], #16								  @ load rk6

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 1
	ld1     {$rk7s}, [$cc], #16								  @ load rk7

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 1
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      @ h4h | h3h

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 2
	ld1     {$rk8s}, [$cc], #16								  @ load rk8

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 2
	ldr     $h2q, [$current_tag, #64]                         @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 2
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  @ h4k | h3k

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 3

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 3

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 3
	ld1     {$rk9s}, [$cc], #16								  @ load rk9

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 3

	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0    @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      @ h2l | h1l

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 4
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 4
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 4 blocks

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 4

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 5

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 5

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 6

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 4

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 6
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      @ h2h | h1h

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 6

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 5

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 7

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 7

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 6

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 7

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 8

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 7

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 8

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 8

	aese    $ctr2b, $rk9                                      @ AES block 2 - round 9

	aese    $ctr0b, $rk9                                      @ AES block 0 - round 9

	eor     $h12k.16b, $h12k.16b, $t0.16b                     @ h2k | h1k

	aese    $ctr1b, $rk9                                      @ AES block 1 - round 9

	aese    $ctr3b, $rk9                                      @ AES block 3 - round 9
	b.ge    .L128_enc_tail                                    @ handle tail

	ldp     $input_l0, $input_h0, [$input_ptr, #0]            @ AES block 0 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           @ AES block 2 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	ldp     $input_l1, $input_h1, [$input_ptr, #16]           @ AES block 1 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           @ AES block 3 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	eor     $input_l0, $input_l0, $rk10_l                     @ AES block 0 - round 10 low
	eor     $input_h0, $input_h0, $rk10_h                     @ AES block 0 - round 10 high

	eor     $input_l2, $input_l2, $rk10_l                     @ AES block 2 - round 10 low
	fmov    $ctr_t0d, $input_l0                               @ AES block 0 - mov low

	eor     $input_l1, $input_l1, $rk10_l                     @ AES block 1 - round 10 low
	eor     $input_h2, $input_h2, $rk10_h                     @ AES block 2 - round 10 high
	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 0 - mov high

	fmov    $ctr_t1d, $input_l1                               @ AES block 1 - mov low
	eor     $input_h1, $input_h1, $rk10_h                     @ AES block 1 - round 10 high

	eor     $input_l3, $input_l3, $rk10_l                     @ AES block 3 - round 10 low
	fmov    $ctr_t1.d[1], $input_h1                           @ AES block 1 - mov high

	fmov    $ctr_t2d, $input_l2                               @ AES block 2 - mov low
	eor     $input_h3, $input_h3, $rk10_h                     @ AES block 3 - round 10 high
	rev     $ctr32w, $rctr32w                                 @ CTR block 4

	fmov    $ctr_t2.d[1], $input_h2                           @ AES block 2 - mov high
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4

	eor     $res0b, $ctr_t0b, $ctr0b                          @ AES block 0 - result
	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4
	rev     $ctr32w, $rctr32w                                 @ CTR block 5

	eor     $res1b, $ctr_t1b, $ctr1b                          @ AES block 1 - result
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 5
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 5

	add     $rctr32w, $rctr32w, #1                            @ CTR block 5
	add     $input_ptr, $input_ptr, #64                       @ AES input_ptr update
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 5

	fmov    $ctr_t3d, $input_l3                               @ AES block 3 - mov low
	rev     $ctr32w, $rctr32w                                 @ CTR block 6
	st1     { $res0b}, [$output_ptr], #16                     @ AES block 0 - store result

	fmov    $ctr_t3.d[1], $input_h3                           @ AES block 3 - mov high
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 6

	add     $rctr32w, $rctr32w, #1                            @ CTR block 6
	eor     $res2b, $ctr_t2b, $ctr2b                          @ AES block 2 - result
	st1     { $res1b}, [$output_ptr], #16                     @ AES block 1 - store result

	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 6
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 8 blocks

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 6
	rev     $ctr32w, $rctr32w                                 @ CTR block 7
	st1     { $res2b}, [$output_ptr], #16                     @ AES block 2 - store result

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 7

	eor     $res3b, $ctr_t3b, $ctr3b                          @ AES block 3 - result
	st1     { $res3b}, [$output_ptr], #16                     @ AES block 3 - store result
	b.ge    .L128_enc_prepretail                              @ do prepretail

	.L128_enc_main_loop:                                      @ main loop start
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           @ AES block 4k+3 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	rev64   $res0b, $res0b                                    @ GHASH block 4k (only t0 is free)
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2 (t0, t1, and t2 free)

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+3

	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	rev64   $res1b, $res1b                                    @ GHASH block 4k+1 (t0 and t1 free)

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+3
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+3

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	eor     $input_h3, $input_h3, $rk10_h                     @ AES block 4k+3 - round 10 high

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid
	ldp     $input_l0, $input_h0, [$input_ptr, #0]            @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+8

	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+8

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+8
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3 (t0, t1, t2 and t3 free)

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	eor     $input_h0, $input_h0, $rk10_h                     @ AES block 4k+4 - round 10 high

	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	eor     $input_l0, $input_l0, $rk10_l                     @ AES block 4k+4 - round 10 low

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low
	movi    $mod_constant.8b, #0xc2

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5
	ldp     $input_l1, $input_h1, [$input_ptr, #16]           @ AES block 4k+5 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           @ AES block 4k+6 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	eor     $input_l1, $input_l1, $rk10_l                     @ AES block 4k+5 - round 10 low

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6
	eor     $input_l3, $input_l3, $rk10_l                     @ AES block 4k+3 - round 10 low

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	fmov    $ctr_t0d, $input_l0                               @ AES block 4k+4 - mov low
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 4k+4 - mov high

	add     $input_ptr, $input_ptr, #64                       @ AES input_ptr update
	fmov    $ctr_t3d, $input_l3                               @ AES block 4k+3 - mov low
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	fmov    $ctr_t1d, $input_l1                               @ AES block 4k+5 - mov low

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	eor     $input_h1, $input_h1, $rk10_h                     @ AES block 4k+5 - round 10 high

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7
	fmov    $ctr_t1.d[1], $input_h1                           @ AES block 4k+5 - mov high

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	fmov    $ctr_t3.d[1], $input_h3                           @ AES block 4k+3 - mov high

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	cmp     $input_ptr, $main_end_input_ptr                   @ LOOP CONTROL

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr0b, $rk9                                      @ AES block 4k+4 - round 9
	eor     $input_l2, $input_l2, $rk10_l                     @ AES block 4k+6 - round 10 low
	eor     $input_h2, $input_h2, $rk10_h                     @ AES block 4k+6 - round 10 high

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	fmov    $ctr_t2d, $input_l2                               @ AES block 4k+6 - mov low

	aese    $ctr1b, $rk9                                      @ AES block 4k+5 - round 9
	fmov    $ctr_t2.d[1], $input_h2                           @ AES block 4k+6 - mov high

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	eor     $res0b, $ctr_t0b, $ctr0b                          @ AES block 4k+4 - result

	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4k+8
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4k+8
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+9
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	eor     $res1b, $ctr_t1b, $ctr1b                          @ AES block 4k+5 - result

	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+9
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+9
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 4k+9

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d            @ MODULO - mid 64b align with low
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 4k+9
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+10

	aese    $ctr2b, $rk9                                      @ AES block 4k+6 - round 9
	st1     { $res0b}, [$output_ptr], #16                     @ AES block 4k+4 - store result
	eor     $res2b, $ctr_t2b, $ctr2b                          @ AES block 4k+6 - result
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+10

	aese    $ctr3b, $rk9                                      @ AES block 4k+7 - round 9
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+10
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+10

	eor     $acc_lb, $acc_lb, $acc_hb                         @ MODULO - fold into low
	st1     { $res1b}, [$output_ptr], #16                     @ AES block 4k+5 - store result

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+10
	st1     { $res2b}, [$output_ptr], #16                     @ AES block 4k+6 - store result
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+11

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+11
	eor     $res3b, $ctr_t3b, $ctr3b                          @ AES block 4k+3 - result

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	st1     { $res3b}, [$output_ptr], #16                     @ AES block 4k+3 - store result
	b.lt    .L128_enc_main_loop

	.L128_enc_prepretail:                                     @ PREPRETAIL
	rev64   $res0b, $res0b                                    @ GHASH block 4k (only t0 is free)
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+3
	rev64   $res1b, $res1b                                    @ GHASH block 4k+1 (t0 and t1 free)

	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+3
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+3

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2 (t0, t1, and t2 free)

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low

	rev64   $res3b, $res3b                                    @ GHASH block 4k+3 (t0, t1, t2 and t3 free)
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid

	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0

	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low
	movi    $mod_constant.8b, #0xc2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3

	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4

	pmull   $t1.1q, $acc_h.1d, $mod_constant.1d
	eor     $acc_mb, $acc_mb, $acc_hb                         @ karatsuba tidy up

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	ext     $acc_hb, $acc_hb, $acc_hb, #8

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	eor     $acc_mb, $acc_mb, $acc_lb

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5
	eor     $acc_mb, $acc_mb, $t1.16b

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $acc_hb

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7

	pmull   $t1.1q, $acc_m.1d, $mod_constant.1d

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7
	ext     $acc_mb, $acc_mb, $acc_mb, #8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	eor     $acc_lb, $acc_lb, $t1.16b

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8

	aese    $ctr3b, $rk9                                      @ AES block 4k+7 - round 9

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8

	aese    $ctr0b, $rk9                                      @ AES block 4k+4 - round 9

	aese    $ctr1b, $rk9                                      @ AES block 4k+5 - round 9
	eor     $acc_lb, $acc_lb, $acc_mb

	aese    $ctr2b, $rk9                                      @ AES block 4k+6 - round 9
	.L128_enc_tail:                                           @ TAIL

	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   @ main_end_input_ptr is number of bytes left to process
	ldp     $input_l0, $input_h0, [$input_ptr], #16           @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	cmp     $main_end_input_ptr, #48

	ext     $t0.16b, $acc_lb, $acc_lb, #8                     @ prepare final partial tag
	eor     $input_l0, $input_l0, $rk10_l                     @ AES block 4k+4 - round 10 low
	eor     $input_h0, $input_h0, $rk10_h                     @ AES block 4k+4 - round 10 high

	fmov    $ctr_t0d, $input_l0                               @ AES block 4k+4 - mov low

	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 4k+4 - mov high

	eor     $res1b, $ctr_t0b, $ctr0b                          @ AES block 4k+4 - result

	b.gt    .L128_enc_blocks_more_than_3

	sub     $rctr32w, $rctr32w, #1
	movi    $acc_l.8b, #0
	mov     $ctr3b, $ctr2b

	cmp     $main_end_input_ptr, #32
	mov     $ctr2b, $ctr1b
	movi    $acc_h.8b, #0

	movi    $acc_m.8b, #0
	b.gt    .L128_enc_blocks_more_than_2

	mov     $ctr3b, $ctr1b
	cmp     $main_end_input_ptr, #16

	sub     $rctr32w, $rctr32w, #1
	b.gt    .L128_enc_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L128_enc_blocks_less_than_1
	.L128_enc_blocks_more_than_3:                             @ blocks left >  3
	st1     { $res1b}, [$output_ptr], #16                     @ AES final-3 block  - store result

	ldp     $input_l0, $input_h0, [$input_ptr], #16           @ AES final-2 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	rev64   $res0b, $res1b                                    @ GHASH final-3 block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag
	eor     $input_h0, $input_h0, $rk10_h                     @ AES final-2 block - round 10 high
	eor     $input_l0, $input_l0, $rk10_l                     @ AES final-2 block - round 10 low

	fmov    $res1d, $input_l0                                 @ AES final-2 block - mov low

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in
	fmov    $res1.d[1], $input_h0                             @ AES final-2 block - mov high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH final-3 block - low
	mov     $rk4d, $res0.d[1]                                 @ GHASH final-3 block - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH final-3 block - high

	mov     $acc_md, $h34k.d[1]                               @ GHASH final-3 block - mid

	eor     $res1b, $res1b, $ctr1b                            @ AES final-2 block - result
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-3 block - mid

	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                    @ GHASH final-3 block - mid
	.L128_enc_blocks_more_than_2:                             @ blocks left >  2

	st1     { $res1b}, [$output_ptr], #16                     @ AES final-2 block - store result

	rev64   $res0b, $res1b                                    @ GHASH final-2 block
	ldp     $input_l0, $input_h0, [$input_ptr], #16           @ AES final-1 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	eor     $input_l0, $input_l0, $rk10_l                     @ AES final-1 block - round 10 low

	fmov    $res1d, $input_l0                                 @ AES final-1 block - mov low
	eor     $input_h0, $input_h0, $rk10_h                     @ AES final-1 block - round 10 high

	pmull2  $rk2q1, $res0.2d, $h3.2d                          @ GHASH final-2 block - high
	fmov    $res1.d[1], $input_h0                             @ AES final-1 block - mov high

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                          @ GHASH final-2 block - low

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-2 block - high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-2 block - mid

	eor     $res1b, $res1b, $ctr2b                            @ AES final-1 block - result

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-2 block - low

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                      @ GHASH final-2 block - mid

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in

	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-2 block - mid
	.L128_enc_blocks_more_than_1:                             @ blocks left >  1

	st1     { $res1b}, [$output_ptr], #16                     @ AES final-1 block - store result

	rev64   $res0b, $res1b                                    @ GHASH final-1 block
	ldp     $input_l0, $input_h0, [$input_ptr], #16           @ AES final block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	eor     $input_h0, $input_h0, $rk10_h                     @ AES final block - round 10 high
	eor     $input_l0, $input_l0, $rk10_l                     @ AES final block - round 10 low

	fmov    $res1d, $input_l0                                 @ AES final block - mov low

	pmull2  $rk2q1, $res0.2d, $h2.2d                          @ GHASH final-1 block - high
	fmov    $res1.d[1], $input_h0                             @ AES final block - mov high

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-1 block - mid

	pmull   $rk3q1, $res0.1d, $h2.1d                          @ GHASH final-1 block - low

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-1 block - mid

	eor     $res1b, $res1b, $ctr3b                            @ AES final block - result

	ins     $rk4v.d[1], $rk4v.d[0]                            @ GHASH final-1 block - mid

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                      @ GHASH final-1 block - mid

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-1 block - low

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-1 block - high

	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-1 block - mid
	movi    $t0.8b, #0                                        @ suppress further partial tag feed in
	.L128_enc_blocks_less_than_1:                             @ blocks left <= 1

	and     $bit_length, $bit_length, #127                    @ bit_length %= 128
	mvn     $rk10_l, xzr                                      @ rk10_l = 0xffffffffffffffff

	mvn     $rk10_h, xzr                                      @ rk10_h = 0xffffffffffffffff
	sub     $bit_length, $bit_length, #128                    @ bit_length -= 128

	neg     $bit_length, $bit_length                          @ bit_length = 128 - #bits in input (in range [1,128])

	and     $bit_length, $bit_length, #127                    @ bit_length %= 128

	lsr     $rk10_h, $rk10_h, $bit_length                     @ rk10_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $input_l0, $rk10_l, $rk10_h, lt
	csel    $input_h0, $rk10_h, xzr, lt

	fmov    $ctr0d, $input_l0                                 @ ctr0b is mask for last block

	fmov    $ctr0.d[1], $input_h0

	and     $res1b, $res1b, $ctr0b                            @ possibly partial last block has zeroes in highest bits

	rev64   $res0b, $res1b                                    @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	mov     $t0d, $res0.d[1]                                  @ GHASH final block - mid

	pmull   $rk3q1, $res0.1d, $h1.1d                          @ GHASH final block - low
	ld1     { $rk0}, [$output_ptr]                            @ load existing bytes where the possibly partial last block is to be stored

	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH final block - mid
#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif
	pmull2  $rk2q1, $res0.2d, $h1.2d                          @ GHASH final block - high

	pmull   $t0.1q, $t0.1d, $h12k.1d                          @ GHASH final block - mid

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final block - low

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final block - high

	eor     $acc_mb, $acc_mb, $t0.16b                         @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid

	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d            @ MODULO - mid 64b align with low

	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	bif     $res1b, $rk0, $ctr0b                              @ insert existing bytes in top end of result before storing

	eor     $acc_lb, $acc_lb, $acc_hb                         @ MODULO - fold into low
	st1     { $res1b}, [$output_ptr]                          @ store all 16B

	str     $ctr32w, [$counter, #12]                          @ store the updated counter

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]
	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

.L128_enc_ret:
	mov w0, #0x0
	ret
.size aes_gcm_enc_128_kernel,.-aes_gcm_enc_128_kernel
___

#########################################################################################
# size_t aes_gcm_dec_128_kernel(const uint8_t * ciphertext,
#                               uint64_t plaintext_length,
#                               uint8_t * plaintext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_dec_128_kernel
.type   aes_gcm_dec_128_kernel,%function
.align  4
aes_gcm_dec_128_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L128_dec_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	lsr     $main_end_input_ptr, $bit_length, #3              @ byte_len
	mov     $len, $main_end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk10_l, $rk10_h, [$cc, #160]                     @ load rk10
#ifdef __AARCH64EB__
	ror     $rk10_h, $rk10_h, 32
	ror     $rk10_l, $rk10_l, 32
#endif
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      @ byte_len - 1
	ld1     {$rk0s}, [$cc], #16                                @ load rk0

	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0 @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	ld1     { $ctr0b}, [$counter]                             @ special case vector load initial counter so we can start first AES block as quickly as possible

	ldr     $h2q, [$current_tag, #64]                         @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif
	lsr     $rctr32x, $ctr96_t32x, #32
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 2

	ld1     {$rk1s}, [$cc], #16                                @ load rk1
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w
	rev     $rctr32w, $rctr32w                                @ rev_ctr32

	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 1
	add     $rctr32w, $rctr32w, #1                            @ increment rev_ctr32

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 0
	rev     $ctr32w, $rctr32w                                 @ CTR block 1

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 1
	ld1     {$rk2s}, [$cc], #16                                @ load rk2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 1

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 2

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 2

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 2
	rev     $ctr32w, $rctr32w                                 @ CTR block 3

	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 3
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 3
	add     $rctr32w, $rctr32w, #1                            @ CTR block 3

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 3
	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   @ end_input_ptr

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 0
	ld1     {$rk3s}, [$cc], #16                                @ load rk3

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 2
	ld1     {$rk4s}, [$cc], #16                                @ load rk4

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 0
	ld1     {$rk5s}, [$cc], #16                                @ load rk5

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 1
	ld1     {$rk6s}, [$cc], #16                                @ load rk6

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 0

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 1

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 2

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 1
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 3
	ld1     {$rk7s}, [$cc], #16                                @ load rk7

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 3

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 2
	ld1     {$rk8s}, [$cc], #16                                @ load rk8

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 4

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 3

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 3
	ldr     $h3q, [$current_tag, #80]                         @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 4
	ld1     {$rk9s}, [$cc], #16                                @ load rk9

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 5

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 4

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 4

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 5
	ldr     $h1q, [$current_tag, #32]                         @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 5

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 6

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 6

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 6

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 6
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      @ h2h | h1h

	ldr     $h4q, [$current_tag, #112]                        @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      @ h2l | h1l
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 7

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 7

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 7
	eor     $h12k.16b, $h12k.16b, $t0.16b                     @ h2k | h1k

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 7

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 8
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      @ h4l | h3l

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 8

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 8
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      @ h4h | h3h

	aese    $ctr2b, $rk9                                      @ AES block 2 - round 9

	aese    $ctr3b, $rk9                                      @ AES block 3 - round 9

	aese    $ctr0b, $rk9                                      @ AES block 0 - round 9
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 4 blocks

	aese    $ctr1b, $rk9                                      @ AES block 1 - round 9
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  @ h4k | h3k
	b.ge    .L128_dec_tail                                    @ handle tail

	ld1     {$res0b, $res1b}, [$input_ptr], #32               @ AES block 0 - load ciphertext; AES block 1 - load ciphertext

	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 1 - result
	ld1     {$res2b}, [$input_ptr], #16                       @ AES block 2 - load ciphertext

	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 0 - result
	rev64   $res0b, $res0b                                    @ GHASH block 0
	rev     $ctr32w, $rctr32w                                 @ CTR block 4

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4
	ld1     {$res3b}, [$input_ptr], #16                       @ AES block 3 - load ciphertext

	rev64   $res1b, $res1b                                    @ GHASH block 1
	mov     $output_l1, $ctr1.d[0]                            @ AES block 1 - mov low

	mov     $output_h1, $ctr1.d[1]                            @ AES block 1 - mov high

	mov     $output_l0, $ctr0.d[0]                            @ AES block 0 - mov low
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 8 blocks

	mov     $output_h0, $ctr0.d[1]                            @ AES block 0 - mov high

	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4
	rev     $ctr32w, $rctr32w                                 @ CTR block 5
	eor     $output_l1, $output_l1, $rk10_l                   @ AES block 1 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 5
	add     $rctr32w, $rctr32w, #1                            @ CTR block 5
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 5

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 5
	rev     $ctr32w, $rctr32w                                 @ CTR block 6
	add     $rctr32w, $rctr32w, #1                            @ CTR block 6

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 6

	eor     $output_h1, $output_h1, $rk10_h                   @ AES block 1 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	eor     $output_l0, $output_l0, $rk10_l                   @ AES block 0 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 2 - result

	eor     $output_h0, $output_h0, $rk10_h                   @ AES block 0 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 0 - store result

	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 1 - store result
	b.ge    .L128_dec_prepretail                              @ do prepretail

	.L128_dec_main_loop:                                      @ main loop start
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high
	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6

	rev64   $res2b, $res2b                                    @ GHASH block 4k+2
	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7

	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	eor     $output_l3, $output_l3, $rk10_l                   @ AES block 4k+3 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	eor     $output_h2, $output_h2, $rk10_h                   @ AES block 4k+2 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4
	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid
	eor     $output_h3, $output_h3, $rk10_h                   @ AES block 4k+3 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5
	eor     $output_l2, $output_l2, $rk10_l                   @ AES block 4k+2 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	movi    $mod_constant.8b, #0xc2

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high
	ld1     {$res0b}, [$input_ptr], #16                       @ AES block 4k+3 - load ciphertext

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+8

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	ld1     {$res1b}, [$input_ptr], #16                       @ AES block 4k+4 - load ciphertext
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr0b, $rk9                                      @ AES block 4k+4 - round 9
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+8

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	aese    $ctr1b, $rk9                                      @ AES block 4k+5 - round 9

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 4k+4 - result

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	ld1     {$res2b}, [$input_ptr], #16                       @ AES block 4k+5 - load ciphertext

	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+8
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid
	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 4k+5 - result

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	ld1     {$res3b}, [$input_ptr], #16                       @ AES block 4k+6 - load ciphertext

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6

	rev64   $res1b, $res1b                                    @ GHASH block 4k+5
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid
	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4k+8

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4k+8
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+9

	aese    $ctr2b, $rk9                                      @ AES block 4k+6 - round 9
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+9
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8
	eor     $output_h0, $output_h0, $rk10_h                   @ AES block 4k+4 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low
	mov     $output_h1, $ctr1.d[1]                            @ AES block 4k+5 - mov high
	eor     $output_l0, $output_l0, $rk10_l                   @ AES block 4k+4 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 4k+6 - result
	mov     $output_l1, $ctr1.d[0]                            @ AES block 4k+5 - mov low
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+9

	aese    $ctr3b, $rk9                                      @ AES block 4k+7 - round 9
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 4k+9
	cmp     $input_ptr, $main_end_input_ptr                   @ LOOP CONTROL

	rev64   $res0b, $res0b                                    @ GHASH block 4k+4
	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 4k+9

	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+10
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+10

	eor     $output_h1, $output_h1, $rk10_h                   @ AES block 4k+5 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 4k+4 - store result

	eor     $output_l1, $output_l1, $rk10_l                   @ AES block 4k+5 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 4k+5 - store result

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+10
	b.lt    L128_dec_main_loop

	.L128_dec_prepretail:                                     @ PREPRETAIL
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high

	eor     $res0b, $res0b, $acc_lb                           @ PRE 1
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6

	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7
	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	rev64   $res3b, $res3b                                    @ GHASH block 4k+3

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high
	movi    $mod_constant.8b, #0xc2

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	eor     $output_l3, $output_l3, $rk10_l                   @ AES block 4k+3 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $output_l2, $output_l2, $rk10_l                   @ AES block 4k+2 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	aese    $ctr1b, $rk9                                      @ AES block 4k+5 - round 9

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	eor     $output_h3, $output_h3, $rk10_h                   @ AES block 4k+3 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8
	eor     $output_h2, $output_h2, $rk10_h                   @ AES block 4k+2 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	aese    $ctr0b, $rk9                                      @ AES block 4k+4 - round 9
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	aese    $ctr2b, $rk9                                      @ AES block 4k+6 - round 9
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result

	aese    $ctr3b, $rk9                                      @ AES block 4k+7 - round 9
	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	.L128_dec_tail:                                           @ TAIL

	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   @ main_end_input_ptr is number of bytes left to process
	ld1     { $res1b}, [$input_ptr], #16                      @ AES block 4k+4 - load ciphertext

	eor     $ctr0b, $res1b, $ctr0b                            @ AES block 4k+4 - result

	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high

	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low

	cmp     $main_end_input_ptr, #48

	eor     $output_h0, $output_h0, $rk10_h                   @ AES block 4k+4 - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	ext     $t0.16b, $acc_lb, $acc_lb, #8                     @ prepare final partial tag
	eor     $output_l0, $output_l0, $rk10_l                   @ AES block 4k+4 - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	b.gt    .L128_dec_blocks_more_than_3

	mov     $ctr3b, $ctr2b
	sub     $rctr32w, $rctr32w, #1
	movi    $acc_l.8b, #0

	movi    $acc_h.8b, #0
	mov     $ctr2b, $ctr1b

	movi    $acc_m.8b, #0
	cmp     $main_end_input_ptr, #32
	b.gt     .L128_dec_blocks_more_than_2

	cmp     $main_end_input_ptr, #16

	mov     $ctr3b, $ctr1b
	sub     $rctr32w, $rctr32w, #1
	b.gt    .L128_dec_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L128_dec_blocks_less_than_1
	.L128_dec_blocks_more_than_3:                             @ blocks left >  3
	rev64   $res0b, $res1b                                    @ GHASH final-3 block
	ld1     { $res1b}, [$input_ptr], #16                      @ AES final-2 block - load ciphertext

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	mov     $acc_md, $h34k.d[1]                               @ GHASH final-3 block - mid
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-3 block  - store result
	eor     $ctr0b, $res1b, $ctr1b                            @ AES final-2 block - result

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-3 block - mid
	mov     $output_h0, $ctr0.d[1]                            @ AES final-2 block - mov high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH final-3 block - low
	mov     $output_l0, $ctr0.d[0]                            @ AES final-2 block - mov low

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH final-3 block - high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-3 block - mid

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in
	eor     $output_h0, $output_h0, $rk10_h                   @ AES final-2 block - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                    @ GHASH final-3 block - mid
	eor     $output_l0, $output_l0, $rk10_l                   @ AES final-2 block - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	.L128_dec_blocks_more_than_2:                             @ blocks left >  2

	rev64   $res0b, $res1b                                    @ GHASH final-2 block
	ld1     { $res1b}, [$input_ptr], #16                      @ AES final-1 block - load ciphertext

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	eor     $ctr0b, $res1b, $ctr2b                            @ AES final-1 block - result
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-2 block  - store result

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                          @ GHASH final-2 block - low

	pmull2  $rk2q1, $res0.2d, $h3.2d                          @ GHASH final-2 block - high
	mov     $output_l0, $ctr0.d[0]                            @ AES final-1 block - mov low

	mov     $output_h0, $ctr0.d[1]                            @ AES final-1 block - mov high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-2 block - mid

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                      @ GHASH final-2 block - mid

	eor     $output_l0, $output_l0, $rk10_l                   @ AES final-1 block - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-2 block - low

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-2 block - high

	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-2 block - mid
	eor     $output_h0, $output_h0, $rk10_h                   @ AES final-1 block - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	.L128_dec_blocks_more_than_1:                             @ blocks left >  1

	rev64   $res0b, $res1b                                    @ GHASH final-1 block

	ld1     { $res1b}, [$input_ptr], #16                      @ AES final block - load ciphertext
	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-1 block - mid

	eor     $ctr0b, $res1b, $ctr3b                            @ AES final block - result

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-1 block - mid

	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-1 block  - store result
	mov     $output_l0, $ctr0.d[0]                            @ AES final block - mov low

	mov     $output_h0, $ctr0.d[1]                            @ AES final block - mov high
	ins     $rk4v.d[1], $rk4v.d[0]                            @ GHASH final-1 block - mid

	pmull   $rk3q1, $res0.1d, $h2.1d                          @ GHASH final-1 block - low

	pmull2  $rk2q1, $res0.2d, $h2.2d                          @ GHASH final-1 block - high

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                      @ GHASH final-1 block - mid
	movi    $t0.8b, #0                                        @ suppress further partial tag feed in

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-1 block - low

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-1 block - high
	eor     $output_h0, $output_h0, $rk10_h                   @ AES final block - round 10 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $output_l0, $output_l0, $rk10_l                   @ AES final block - round 10 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-1 block - mid
	.L128_dec_blocks_less_than_1:                                            @ blocks left <= 1

	mvn     $rk10_h, xzr                                      @ rk10_h = 0xffffffffffffffff
	and     $bit_length, $bit_length, #127                    @ bit_length %= 128

	mvn     $rk10_l, xzr                                      @ rk10_l = 0xffffffffffffffff
	sub     $bit_length, $bit_length, #128                    @ bit_length -= 128

	neg     $bit_length, $bit_length                          @ bit_length = 128 - #bits in input (in range [1,128])

	and     $bit_length, $bit_length, #127                    @ bit_length %= 128

	lsr     $rk10_h, $rk10_h, $bit_length                     @ rk10_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $ctr96_b64x, $rk10_h, xzr, lt
	csel    $ctr32x, $rk10_l, $rk10_h, lt

	fmov    $ctr0d, $ctr32x                                   @ ctr0b is mask for last block

	mov     $ctr0.d[1], $ctr96_b64x

	and     $res1b, $res1b, $ctr0b                            @ possibly partial last block has zeroes in highest bits

	rev64   $res0b, $res1b                                    @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	ldp     $end_input_ptr, $main_end_input_ptr, [$output_ptr] @ load existing bytes we need to not overwrite

	and     $output_h0, $output_h0, $ctr96_b64x

	pmull2  $rk2q1, $res0.2d, $h1.2d                          @ GHASH final block - high
	mov     $t0d, $res0.d[1]                                  @ GHASH final block - mid

	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH final block - mid
	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final block - high

	pmull   $t0.1q, $t0.1d, $h12k.1d                          @ GHASH final block - mid

	pmull   $rk3q1, $res0.1d, $h1.1d                          @ GHASH final block - low
	bic     $end_input_ptr, $end_input_ptr, $ctr32x           @ mask out low existing bytes
	and     $output_l0, $output_l0, $ctr32x

#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif

	eor     $acc_mb, $acc_mb, $t0.16b                         @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final block - low

	bic     $main_end_input_ptr, $main_end_input_ptr, $ctr96_b64x   @ mask out high existing bytes
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid

	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	orr     $output_l0, $output_l0, $end_input_ptr
	str     $ctr32w, [$counter, #12]                          @ store the updated counter

	orr     $output_h0, $output_h0, $main_end_input_ptr
	stp     $output_l0, $output_h0, [$output_ptr]
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]

	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

	.L128_dec_ret:
	mov w0, #0x0
	ret
.size aes_gcm_dec_128_kernel,.-aes_gcm_dec_128_kernel
___
}

{
my ($end_input_ptr,$main_end_input_ptr,$input_l0,$input_h0)=map("x$_",(4..7));
my ($input_l1,$input_h1,$input_l2,$input_h2,$input_l3,$input_h3)=map("x$_",(19..24));
my ($output_l1,$output_h1,$output_l2,$output_h2,$output_l3,$output_h3)=map("x$_",(19..24));
my ($output_l0,$output_h0)=map("x$_",(6..7));

my $ctr32w="w9";
my ($ctr32x,$ctr96_b64x,$ctr96_t32x,$rctr32x,$rk12_l,$rk12_h,$len)=map("x$_",(9..15));
my ($ctr96_t32w,$rctr32w)=map("w$_",(11..12));

my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$res0b,$res1b,$res2b,$res3b)=map("v$_.16b",(0..7));
my ($ctr0,$ctr1,$ctr2,$ctr3,$res0,$res1,$res2,$res3)=map("v$_",(0..7));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$res0d,$res1d,$res2d,$res3d)=map("d$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q)=map("q$_",(4..7));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(9..11));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(9..11));
my ($acc_hd,$acc_md,$acc_ld)=map("d$_",(9..11));

my ($h1,$h2,$h3,$h4,$h12k,$h34k)=map("v$_",(12..17));
my ($h1q,$h2q,$h3q,$h4q)=map("q$_",(12..15));
my ($h1b,$h2b,$h3b,$h4b)=map("v$_.16b",(12..15));

my $t0="v8";
my $t0d="d8";
my $t3="v4";
my $t3d="d4";

my ($t1,$t2)=map("v$_",(30..31));
my ($t1d,$t2d)=map("d$_",(30..31));

my $t4="v30";
my $t4d="d30";
my $t5="v8";
my $t5d="d8";
my $t6="v31";
my $t6d="d31";

my $t7="v5";
my $t7d="d5";
my $t8="v6";
my $t8d="d6";
my $t9="v30";
my $t9d="d30";

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3)=map("v$_",(4..7));
my ($ctr_t0d,$ctr_t1d,$ctr_t2d,$ctr_t3d)=map("d$_",(4..7));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b)=map("v$_.16b",(4..7));

my $mod_constantd="d8";
my $mod_constant="v8";
my $mod_t="v31";

my ($rk0,$rk1,$rk2,$rk3,$rk4,$rk5,$rk6,$rk7,$rk8,$rk9,$rk10,$rk11)=map("v$_.16b",(18..29));
my ($rk0q,$rk1q,$rk2q,$rk3q,$rk4q,$rk5q,$rk6q,$rk7q,$rk8q,$rk9q,$rk10q,$rk11q)=map("q$_",(18..29));
my ($rk0s,$rk1s,$rk2s,$rk3s,$rk4s,$rk5s,$rk6s,$rk7s,$rk8s,$rk9s,$rk10s,$rk11s)=map("v$_.4s",(18..29));
my $rk2q1="v20.1q";
my $rk3q1="v21.1q";
my $rk4v="v22";
my $rk4d="d22";

#########################################################################################
# size_t aes_gcm_enc_192_kernel(const uint8_t * plaintext,
#                               uint64_t plaintext_length,
#                               uint8_t * ciphertext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_enc_192_kernel
.type   aes_gcm_enc_192_kernel,%function
.align  4
aes_gcm_enc_192_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L192_enc_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]             @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk12_l, $rk12_h, [$cc, #192]                     @ load rk12
#ifdef __AARCH64EB__
	ror     $rk12_l, $rk12_l, #32
	ror     $rk12_h, $rk12_h, #32
#endif
	ld1     {$rk0s}, [$cc], #16	                             @ load rk0

	ld1     {$rk1s}, [$cc], #16	                             @ load rk1

	ld1     {$rk2s}, [$cc], #16	                             @ load rk2

	lsr     $rctr32x, $ctr96_t32x, #32
	ld1     {$rk3s}, [$cc], #16	                             @ load rk3
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w

	ld1     {$rk4s}, [$cc], #16	                             @ load rk4
	rev     $rctr32w, $rctr32w                               @ rev_ctr32

	add     $rctr32w, $rctr32w, #1                           @ increment rev_ctr32
	fmov    $ctr3d, $ctr96_b64x                              @ CTR block 3

	rev     $ctr32w, $rctr32w                                @ CTR block 1
	add     $rctr32w, $rctr32w, #1                           @ CTR block 1
	fmov    $ctr1d, $ctr96_b64x                              @ CTR block 1

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 1
	ld1     { $ctr0b}, [$counter]                            @ special case vector load initial counter so we can start first AES block as quickly as possible

	fmov    $ctr1.d[1], $ctr32x                              @ CTR block 1
	rev     $ctr32w, $rctr32w                                @ CTR block 2
	add     $rctr32w, $rctr32w, #1                           @ CTR block 2

	fmov    $ctr2d, $ctr96_b64x                              @ CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 2

	fmov    $ctr2.d[1], $ctr32x                              @ CTR block 2
	rev     $ctr32w, $rctr32w                                @ CTR block 3

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 3
	ld1     {$rk5s}, [$cc], #16	                             @ load rk5

	fmov    $ctr3.d[1], $ctr32x                              @ CTR block 3

	ld1     {$rk6s}, [$cc], #16	                             @ load rk6

	ld1     {$rk7s}, [$cc], #16	                             @ load rk7

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 0
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 0
	ld1     {$rk8s}, [$cc], #16	                             @ load rk8

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 0
	ldr     $h4q, [$current_tag, #112]                       @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 0
	ld1     {$rk9s}, [$cc], #16	                             @ load rk9

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 1
	ld1     {$rk10s}, [$cc], #16	                         @ load rk10

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 1
	ldr     $h1q, [$current_tag, #32]                        @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 1
	ld1     {$rk11s}, [$cc], #16	                         @ load rk11

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 1
	ldr     $h3q, [$current_tag, #80]                        @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 2

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 2

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 3
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                     @ h4h | h3h

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 3

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 2
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                     @ h4l | h3l

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 4

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 3

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 3

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 5

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 4

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 4

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 6

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 4

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 5

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 5

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 5

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 6
	ldr     $h2q, [$current_tag, #64]                        @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 6

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 6

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 7

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 7
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                     @ h2l | h1l

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 7

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 8

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 7
	trn1    $t0.2d,    $h1.2d,    $h2.2d                     @ h2h | h1h

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 8

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 8

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 9

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 9

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 9

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 9

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b         @ AES block 0 - round 10

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b         @ AES block 2 - round 10

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b         @ AES block 1 - round 10
	lsr     $main_end_input_ptr, $bit_length, #3             @ byte_len
	mov     $len, $main_end_input_ptr

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b         @ AES block 3 - round 10
	sub     $main_end_input_ptr, $main_end_input_ptr, #1     @ byte_len - 1

	eor     $h12k.16b, $h12k.16b, $t0.16b                    @ h2k | h1k
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0   @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	eor     $h34k.16b, $h34k.16b, $acc_h.16b                 @ h4k | h3k

	aese    $ctr2b, $rk11                                    @ AES block 2 - round 11
	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3  @ end_input_ptr
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese    $ctr1b, $rk11                                    @ AES block 1 - round 11
	cmp     $input_ptr, $main_end_input_ptr                  @ check if we have <= 4 blocks

	aese    $ctr0b, $rk11                                    @ AES block 0 - round 11
	add     $rctr32w, $rctr32w, #1                           @ CTR block 3

	aese    $ctr3b, $rk11                                    @ AES block 3 - round 11
	b.ge    .L192_enc_tail                                   @ handle tail

	rev     $ctr32w, $rctr32w                                @ CTR block 4
	ldp     $input_l0, $input_h0, [$input_ptr, #0]           @ AES block 0 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 4
	ldp     $input_l2, $input_h2, [$input_ptr, #32]          @ AES block 2 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	ldp     $input_l3, $input_h3, [$input_ptr, #48]          @ AES block 3 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	ldp     $input_l1, $input_h1, [$input_ptr, #16]          @ AES block 1 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	add     $input_ptr, $input_ptr, #64                      @ AES input_ptr update
	cmp     $input_ptr, $main_end_input_ptr                  @ check if we have <= 8 blocks

	eor     $input_l0, $input_l0, $rk12_l                    @ AES block 0 - round 12 low

	eor     $input_h0, $input_h0, $rk12_h                    @ AES block 0 - round 12 high
	eor     $input_h2, $input_h2, $rk12_h                    @ AES block 2 - round 12 high
	fmov    $ctr_t0d, $input_l0                              @ AES block 0 - mov low

	eor     $input_h3, $input_h3, $rk12_h                    @ AES block 3 - round 12 high
	fmov    $ctr_t0.d[1], $input_h0                          @ AES block 0 - mov high

	eor     $input_l2, $input_l2, $rk12_l                    @ AES block 2 - round 12 low
	eor     $input_l1, $input_l1, $rk12_l                    @ AES block 1 - round 12 low

	fmov    $ctr_t1d, $input_l1                              @ AES block 1 - mov low
	eor     $input_h1, $input_h1, $rk12_h                    @ AES block 1 - round 12 high

	fmov    $ctr_t1.d[1], $input_h1                          @ AES block 1 - mov high

	eor     $input_l3, $input_l3, $rk12_l                    @ AES block 3 - round 12 low
	fmov    $ctr_t2d, $input_l2                              @ AES block 2 - mov low

	add     $rctr32w, $rctr32w, #1                           @ CTR block 4
	eor     $res0b, $ctr_t0b, $ctr0b                         @ AES block 0 - result
	fmov    $ctr0d, $ctr96_b64x                              @ CTR block 4

	fmov    $ctr0.d[1], $ctr32x                              @ CTR block 4
	rev     $ctr32w, $rctr32w                                @ CTR block 5

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 5
	add     $rctr32w, $rctr32w, #1                           @ CTR block 5

	fmov    $ctr_t3d, $input_l3                              @ AES block 3 - mov low
	st1     { $res0b}, [$output_ptr], #16                    @ AES block 0 - store result

	fmov    $ctr_t2.d[1], $input_h2                          @ AES block 2 - mov high

	eor     $res1b, $ctr_t1b, $ctr1b                         @ AES block 1 - result
	fmov    $ctr1d, $ctr96_b64x                              @ CTR block 5
	st1     { $res1b}, [$output_ptr], #16                    @ AES block 1 - store result

	fmov    $ctr_t3.d[1], $input_h3                          @ AES block 3 - mov high

	fmov    $ctr1.d[1], $ctr32x                              @ CTR block 5
	rev     $ctr32w, $rctr32w                                @ CTR block 6

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 6

	add     $rctr32w, $rctr32w, #1                           @ CTR block 6
	eor     $res2b, $ctr_t2b, $ctr2b                         @ AES block 2 - result
	fmov    $ctr2d, $ctr96_b64x                              @ CTR block 6

	fmov    $ctr2.d[1], $ctr32x                              @ CTR block 6
	rev     $ctr32w, $rctr32w                                @ CTR block 7

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 7
	st1     { $res2b}, [$output_ptr], #16                    @ AES block 2 - store result

	eor     $res3b, $ctr_t3b, $ctr3b                         @ AES block 3 - result
	st1     { $res3b}, [$output_ptr], #16                    @ AES block 3 - store result
	b.ge    .L192_enc_prepretail                             @ do prepretail

	.L192_enc_main_loop:                                     @ main loop start
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 0
	rev64   $res1b, $res1b                                   @ GHASH block 4k+1 (t0 and t1 free)

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 0
	ldp     $input_l1, $input_h1, [$input_ptr, #16]          @ AES block 4k+5 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	ext     $acc_lb, $acc_lb, $acc_lb, #8                    @ PRE 0
	fmov    $ctr3d, $ctr96_b64x                              @ CTR block 4k+3
	rev64   $res0b, $res0b                                   @ GHASH block 4k (only t0 is free)

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 1
	fmov    $ctr3.d[1], $ctr32x                              @ CTR block 4k+3

	pmull2  $t1.1q, $res1.2d, $h3.2d                         @ GHASH block 4k+1 - high
	rev64   $res3b, $res3b                                   @ GHASH block 4k+3 (t0, t1, t2 and t3 free)
	ldp     $input_l2, $input_h2, [$input_ptr, #32]          @ AES block 4k+6 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 0
	ldp     $input_l3, $input_h3, [$input_ptr, #48]          @ AES block 4k+3 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	pmull   $t2.1q, $res1.1d, $h3.1d                         @ GHASH block 4k+1 - low
	eor     $res0b, $res0b, $acc_lb                          @ PRE 1

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 1

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 1
	rev64   $res2b, $res2b                                   @ GHASH block 4k+2 (t0, t1, and t2 free)

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 0
	eor     $input_h3, $input_h3, $rk12_h                    @ AES block 4k+3 - round 12 high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                      @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                 @ GHASH block 4k - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 2

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 1
	eor     $input_l2, $input_l2, $rk12_l                    @ AES block 4k+6 - round 12 low

	eor     $t0.8b, $t0.8b, $res0.8b                         @ GHASH block 4k - mid
	eor     $acc_lb, $acc_lb, $t2.16b                        @ GHASH block 4k+1 - low

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 3
	eor     $input_l1, $input_l1, $rk12_l                    @ AES block 4k+5 - round 12 low

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 2
	mov     $t6d, $res2.d[1]                                 @ GHASH block 4k+2 - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      @ GHASH block 4k - high
	mov     $t3d, $res1.d[1]                                 @ GHASH block 4k+1 - mid

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 2

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 3

	mov     $acc_md, $h34k.d[1]                              @ GHASH block 4k - mid
	eor     $acc_hb, $acc_hb, $t1.16b                        @ GHASH block 4k+1 - high

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 2
	eor     $t6.8b, $t6.8b, $res2.8b                         @ GHASH block 4k+2 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                         @ GHASH block 4k+2 - high

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 4
	eor     $t3.8b, $t3.8b, $res1.8b                         @ GHASH block 4k+1 - mid

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 3

	pmull2  $t7.1q, $res3.2d, $h1.2d                         @ GHASH block 4k+3 - high
	eor     $input_h1, $input_h1, $rk12_h                    @ AES block 4k+5 - round 12 high
	ins     $t6.d[1], $t6.d[0]                               @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 5
	add     $rctr32w, $rctr32w, #1                           @ CTR block 4k+3

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 4
	eor     $acc_hb, $acc_hb, $t4.16b                        @ GHASH block 4k+2 - high

	pmull   $t3.1q, $t3.1d, $h34k.1d                         @ GHASH block 4k+1 - mid
	eor     $input_h2, $input_h2, $rk12_h                    @ AES block 4k+6 - round 12 high

	pmull2  $t6.1q, $t6.2d, $h12k.2d                         @ GHASH block 4k+2 - mid
	eor     $input_l3, $input_l3, $rk12_l                    @ AES block 4k+3 - round 12 low
	mov     $t9d, $res3.d[1]                                 @ GHASH block 4k+3 - mid

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                     @ GHASH block 4k - mid
	rev     $ctr32w, $rctr32w                                @ CTR block 4k+8

	pmull   $t5.1q, $res2.1d, $h2.1d                         @ GHASH block 4k+2 - low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 4k+8

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 3
	eor     $t9.8b, $t9.8b, $res3.8b                         @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 4
	ldp     $input_l0, $input_h0, [$input_ptr, #0]           @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 6
	eor     $acc_lb, $acc_lb, $t5.16b                        @ GHASH block 4k+2 - low

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 4
	add     $input_ptr, $input_ptr, #64                      @ AES input_ptr update

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 5
	movi    $mod_constant.8b, #0xc2

	pmull   $t8.1q, $res3.1d, $h1.1d                         @ GHASH block 4k+3 - low
	eor     $input_h0, $input_h0, $rk12_h                    @ AES block 4k+4 - round 12 high
	eor     $acc_mb, $acc_mb, $t3.16b                        @ GHASH block 4k+1 - mid

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 5
	eor     $input_l0, $input_l0, $rk12_l                    @ AES block 4k+4 - round 12 low

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 6
	shl     $mod_constantd, $mod_constantd, #56              @ mod_constant

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 5
	eor     $acc_hb, $acc_hb, $t7.16b                        @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 7
	fmov    $ctr_t1d, $input_l1                              @ AES block 4k+5 - mov low

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 7
	eor     $acc_mb, $acc_mb, $t6.16b                        @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 6
	fmov    $ctr_t1.d[1], $input_h1                          @ AES block 4k+5 - mov high

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 8
	eor     $acc_lb, $acc_lb, $t8.16b                        @ GHASH block 4k+3 - low

	pmull   $t9.1q, $t9.1d, $h12k.1d                         @ GHASH block 4k+3 - mid
	cmp     $input_ptr, $main_end_input_ptr                  @ LOOP CONTROL
	fmov    $ctr_t0d, $input_l0                              @ AES block 4k+4 - mov low

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 6
	fmov    $ctr_t0.d[1], $input_h0                          @ AES block 4k+4 - mov high

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 8
	fmov    $ctr_t3d, $input_l3                              @ AES block 4k+3 - mov low

	eor     $acc_mb, $acc_mb, $t9.16b                        @ GHASH block 4k+3 - mid
	eor     $t9.16b, $acc_lb, $acc_hb                        @ MODULO - karatsuba tidy up
	add     $rctr32w, $rctr32w, #1                           @ CTR block 4k+8

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 7
	fmov    $ctr_t3.d[1], $input_h3                          @ AES block 4k+3 - mov high

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d           @ MODULO - top 64b align with mid
	ext     $acc_hb, $acc_hb, $acc_hb, #8                    @ MODULO - other top alignment
	fmov    $ctr_t2d, $input_l2                              @ AES block 4k+6 - mov low

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 7

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 9
	eor     $acc_mb, $acc_mb, $t9.16b                        @ MODULO - karatsuba tidy up

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 8

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 9

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 10
	eor     $acc_mb, $acc_mb, $mod_t.16b                     @ MODULO - fold into mid

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 9

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 9

	aese    $ctr0b, $rk11                                    @ AES block 4k+4 - round 11

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 10
	eor     $acc_mb, $acc_mb, $acc_hb                        @ MODULO - fold into mid

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 10

	eor     $res0b, $ctr_t0b, $ctr0b                         @ AES block 4k+4 - result
	fmov    $ctr0d, $ctr96_b64x                              @ CTR block 4k+8

	aese    $ctr1b, $rk11                                    @ AES block 4k+5 - round 11
	fmov    $ctr0.d[1], $ctr32x                              @ CTR block 4k+8
	rev     $ctr32w, $rctr32w                                @ CTR block 4k+9

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d           @ MODULO - mid 64b align with low
	fmov    $ctr_t2.d[1], $input_h2                          @ AES block 4k+6 - mov high
	st1     { $res0b}, [$output_ptr], #16                    @ AES block 4k+4 - store result

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 10
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 4k+9

	eor     $res1b, $ctr_t1b, $ctr1b                         @ AES block 4k+5 - result
	add     $rctr32w, $rctr32w, #1                           @ CTR block 4k+9
	fmov    $ctr1d, $ctr96_b64x                              @ CTR block 4k+9

	aese    $ctr2b, $rk11                                    @ AES block 4k+6 - round 11
	fmov    $ctr1.d[1], $ctr32x                              @ CTR block 4k+9
	rev     $ctr32w, $rctr32w                                @ CTR block 4k+10

	add     $rctr32w, $rctr32w, #1                           @ CTR block 4k+10
	ext     $acc_mb, $acc_mb, $acc_mb, #8                    @ MODULO - other mid alignment
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 4k+10

	st1     { $res1b}, [$output_ptr], #16                    @ AES block 4k+5 - store result
	eor     $acc_lb, $acc_lb, $acc_hb                        @ MODULO - fold into low

	aese    $ctr3b, $rk11                                    @ AES block 4k+7 - round 11
	eor     $res2b, $ctr_t2b, $ctr2b                         @ AES block 4k+6 - result
	fmov    $ctr2d, $ctr96_b64x                              @ CTR block 4k+10

	st1     { $res2b}, [$output_ptr], #16                    @ AES block 4k+6 - store result
	fmov    $ctr2.d[1], $ctr32x                              @ CTR block 4k+10
	rev     $ctr32w, $rctr32w                                @ CTR block 4k+11

	eor     $acc_lb, $acc_lb, $acc_mb                        @ MODULO - fold into low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32           @ CTR block 4k+11

	eor     $res3b, $ctr_t3b, $ctr3b                         @ AES block 4k+3 - result
	st1     { $res3b}, [$output_ptr], #16                    @ AES block 4k+3 - store result
	b.lt    .L192_enc_main_loop

	.L192_enc_prepretail:                                    @ PREPRETAIL
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 0
	rev64   $res0b, $res0b                                   @ GHASH block 4k (only t0 is free)

	fmov    $ctr3d, $ctr96_b64x                              @ CTR block 4k+3
	ext     $acc_lb, $acc_lb, $acc_lb, #8                    @ PRE 0
	add     $rctr32w, $rctr32w, #1                           @ CTR block 4k+3

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 0
	rev64   $res1b, $res1b                                   @ GHASH block 4k+1 (t0 and t1 free)

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 0

	fmov    $ctr3.d[1], $ctr32x                              @ CTR block 4k+3
	eor     $res0b, $res0b, $acc_lb                          @ PRE 1
	mov     $acc_md, $h34k.d[1]                              @ GHASH block 4k - mid

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 1
	rev64   $res2b, $res2b                                   @ GHASH block 4k+2 (t0, t1, and t2 free)

	pmull2  $t1.1q, $res1.2d, $h3.2d                         @ GHASH block 4k+1 - high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                      @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                 @ GHASH block 4k - mid

	pmull   $t2.1q, $res1.1d, $h3.1d                         @ GHASH block 4k+1 - low
	rev64   $res3b, $res3b                                   @ GHASH block 4k+3 (t0, t1, t2 and t3 free)

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      @ GHASH block 4k - high

	eor     $t0.8b, $t0.8b, $res0.8b                         @ GHASH block 4k - mid
	mov     $t3d, $res1.d[1]                                 @ GHASH block 4k+1 - mid

	eor     $acc_lb, $acc_lb, $t2.16b                        @ GHASH block 4k+1 - low
	mov     $t6d, $res2.d[1]                                 @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 0
	eor     $acc_hb, $acc_hb, $t1.16b                        @ GHASH block 4k+1 - high

	pmull2  $t4.1q, $res2.2d, $h2.2d                         @ GHASH block 4k+2 - high

	eor     $t3.8b, $t3.8b, $res1.8b                         @ GHASH block 4k+1 - mid
	eor     $t6.8b, $t6.8b, $res2.8b                         @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 1

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 1
	eor     $acc_hb, $acc_hb, $t4.16b                        @ GHASH block 4k+2 - high

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 1

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 2
	mov     $t9d, $res3.d[1]                                 @ GHASH block 4k+3 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                         @ GHASH block 4k+3 - high
	ins     $t6.d[1], $t6.d[0]                               @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 2

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                     @ GHASH block 4k - mid
	eor     $t9.8b, $t9.8b, $res3.8b                         @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 3

	pmull2  $t6.1q, $t6.2d, $h12k.2d                         @ GHASH block 4k+2 - mid

	pmull   $t3.1q, $t3.1d, $h34k.1d                         @ GHASH block 4k+1 - mid

	pmull   $t9.1q, $t9.1d, $h12k.1d                         @ GHASH block 4k+3 - mid
	eor     $acc_hb, $acc_hb, $t7.16b                        @ GHASH block 4k+3 - high

	pmull   $t5.1q, $res2.1d, $h2.1d                         @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 3
	eor     $acc_mb, $acc_mb, $t3.16b                        @ GHASH block 4k+1 - mid

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 2
	eor     $acc_lb, $acc_lb, $t5.16b                        @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 4

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 3
	eor     $acc_mb, $acc_mb, $t6.16b                        @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 3

	pmull   $t8.1q, $res3.1d, $h1.1d                         @ GHASH block 4k+3 - low
	movi    $mod_constant.8b, #0xc2

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 4

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 4

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                        @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 5

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 5
	eor     $acc_lb, $acc_lb, $t8.16b                        @ GHASH block 4k+3 - low

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 5

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $acc_hb                        @ karatsuba tidy up

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 6
	shl     $mod_constantd, $mod_constantd, #56              @ mod_constant

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 7

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 7
	eor     $acc_mb, $acc_mb, $acc_lb

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 7

	pmull   $t1.1q, $acc_h.1d, $mod_constant.1d

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 6
	ext     $acc_hb, $acc_hb, $acc_hb, #8

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 8

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $t1.16b

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 7

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 8

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 9

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 8
	eor     $acc_mb, $acc_mb, $acc_hb

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 9

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 9

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 9

	pmull   $t1.1q, $acc_m.1d, $mod_constant.1d

	ext     $acc_mb, $acc_mb, $acc_mb, #8

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b         @ AES block 4k+7 - round 10

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b         @ AES block 4k+4 - round 10

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b         @ AES block 4k+6 - round 10

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b         @ AES block 4k+5 - round 10
	eor     $acc_lb, $acc_lb, $t1.16b

	aese    $ctr0b, $rk11                                    @ AES block 4k+4 - round 11

	aese    $ctr3b, $rk11                                    @ AES block 4k+7 - round 11

	aese    $ctr2b, $rk11                                    @ AES block 4k+6 - round 11

	aese    $ctr1b, $rk11                                    @ AES block 4k+5 - round 11
	eor     $acc_lb, $acc_lb, $acc_mb
	.L192_enc_tail:                                          @ TAIL

	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr  @ main_end_input_ptr is number of bytes left to process
	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $input_l0, $input_l0, $rk12_l                    @ AES block 4k+4 - round 12 low
	eor     $input_h0, $input_h0, $rk12_h                    @ AES block 4k+4 - round 12 high

	fmov    $ctr_t0d, $input_l0                              @ AES block 4k+4 - mov low

	fmov    $ctr_t0.d[1], $input_h0                          @ AES block 4k+4 - mov high
	cmp     $main_end_input_ptr, #48

	eor     $res1b, $ctr_t0b, $ctr0b                         @ AES block 4k+4 - result

	ext     $t0.16b, $acc_lb, $acc_lb, #8                    @ prepare final partial tag
	b.gt    .L192_enc_blocks_more_than_3

	sub     $rctr32w, $rctr32w, #1
	movi    $acc_m.8b, #0

	mov     $ctr3b, $ctr2b
	movi    $acc_h.8b, #0
	cmp     $main_end_input_ptr, #32

	mov     $ctr2b, $ctr1b
	movi    $acc_l.8b, #0
	b.gt    .L192_enc_blocks_more_than_2

	sub     $rctr32w, $rctr32w, #1

	mov     $ctr3b, $ctr1b
	cmp     $main_end_input_ptr, #16
	b.gt    .L192_enc_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L192_enc_blocks_less_than_1
	.L192_enc_blocks_more_than_3:                            @ blocks left >  3
	st1     { $res1b}, [$output_ptr], #16                    @ AES final-3 block  - store result

	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final-2 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	rev64   $res0b, $res1b                                   @ GHASH final-3 block

	eor     $input_l0, $input_l0, $rk12_l                    @ AES final-2 block - round 12 low
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	eor     $input_h0, $input_h0, $rk12_h                    @ AES final-2 block - round 12 high
	fmov    $res1d, $input_l0                                @ AES final-2 block - mov low

	fmov    $res1.d[1], $input_h0                            @ AES final-2 block - mov high

	mov     $rk4d, $res0.d[1]                                @ GHASH final-3 block - mid

	pmull   $acc_l.1q, $res0.1d, $h4.1d                      @ GHASH final-3 block - low

	mov     $acc_md, $h34k.d[1]                              @ GHASH final-3 block - mid

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-3 block - mid

	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      @ GHASH final-3 block - high

	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                   @ GHASH final-3 block - mid
	eor     $res1b, $res1b, $ctr1b                           @ AES final-2 block - result
	.L192_enc_blocks_more_than_2:                            @ blocks left >  2

	st1     { $res1b}, [$output_ptr], #16                    @ AES final-2 block - store result

	rev64   $res0b, $res1b                                   @ GHASH final-2 block
	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final-1 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	eor     $input_h0, $input_h0, $rk12_h                    @ AES final-1 block - round 12 high

	pmull2  $rk2q1, $res0.2d, $h3.2d                         @ GHASH final-2 block - high
	mov     $rk4d, $res0.d[1]                                @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                         @ GHASH final-2 block - low
	eor     $input_l0, $input_l0, $rk12_l                    @ AES final-1 block - round 12 low

	fmov    $res1d, $input_l0                                @ AES final-1 block - mov low

	fmov    $res1.d[1], $input_h0                            @ AES final-1 block - mov high
	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-2 block - high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-2 block - mid

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-2 block - low

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                     @ GHASH final-2 block - mid

	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	eor     $res1b, $res1b, $ctr2b                           @ AES final-1 block - result

	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-2 block - mid
	.L192_enc_blocks_more_than_1:                            @ blocks left >  1

	st1     { $res1b}, [$output_ptr], #16                    @ AES final-1 block - store result

	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	rev64   $res0b, $res1b                                   @ GHASH final-1 block

	eor     $input_l0, $input_l0, $rk12_l                    @ AES final block - round 12 low
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag
	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	mov     $rk4d, $res0.d[1]                                @ GHASH final-1 block - mid

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-1 block - mid
	eor     $input_h0, $input_h0, $rk12_h                    @ AES final block - round 12 high
	fmov    $res1d, $input_l0                                @ AES final block - mov low

	pmull2  $rk2q1, $res0.2d, $h2.2d                         @ GHASH final-1 block - high
	fmov    $res1.d[1], $input_h0                            @ AES final block - mov high

	ins     $rk4v.d[1], $rk4v.d[0]                           @ GHASH final-1 block - mid

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-1 block - high

	pmull   $rk3q1, $res0.1d, $h2.1d                         @ GHASH final-1 block - low

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                     @ GHASH final-1 block - mid

	eor     $res1b, $res1b, $ctr3b                           @ AES final block - result

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-1 block - low

	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-1 block - mid
	.L192_enc_blocks_less_than_1:                            @ blocks left <= 1

	ld1     { $rk0}, [$output_ptr]                           @ load existing bytes where the possibly partial last block is to be stored
#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif
	and     $bit_length, $bit_length, #127                   @ bit_length %= 128

	sub     $bit_length, $bit_length, #128                   @ bit_length -= 128
	mvn     $rk12_h, xzr                                     @ rk12_h = 0xffffffffffffffff

	neg     $bit_length, $bit_length                         @ bit_length = 128 - #bits in input (in range [1,128])
	mvn     $rk12_l, xzr                                     @ rk12_l = 0xffffffffffffffff

	and     $bit_length, $bit_length, #127                   @ bit_length %= 128

	lsr     $rk12_h, $rk12_h, $bit_length                    @ rk12_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $input_l0, $rk12_l, $rk12_h, lt
	csel    $input_h0, $rk12_h, xzr, lt

	fmov    $ctr0d, $input_l0                                @ ctr0b is mask for last block

	fmov    $ctr0.d[1], $input_h0

	and     $res1b, $res1b, $ctr0b                           @ possibly partial last block has zeroes in highest bits

	rev64   $res0b, $res1b                                   @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	mov     $t0d, $res0.d[1]                                 @ GHASH final block - mid

	pmull   $rk3q1, $res0.1d, $h1.1d                         @ GHASH final block - low

	pmull2  $rk2q1, $res0.2d, $h1.2d                         @ GHASH final block - high

	eor     $t0.8b, $t0.8b, $res0.8b                         @ GHASH final block - mid

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final block - low

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final block - high

	pmull   $t0.1q, $t0.1d, $h12k.1d                         @ GHASH final block - mid

	eor     $acc_mb, $acc_mb, $t0.16b                        @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $t9.16b, $acc_lb, $acc_hb                        @ MODULO - karatsuba tidy up

	shl     $mod_constantd, $mod_constantd, #56              @ mod_constant

	bif     $res1b, $rk0, $ctr0b                             @ insert existing bytes in top end of result before storing

	eor     $acc_mb, $acc_mb, $t9.16b                        @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d           @ MODULO - top 64b align with mid

	ext     $acc_hb, $acc_hb, $acc_hb, #8                    @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                     @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                        @ MODULO - fold into mid

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d           @ MODULO - mid 64b align with low

	ext     $acc_mb, $acc_mb, $acc_mb, #8                    @ MODULO - other mid alignment

	eor     $acc_lb, $acc_lb, $acc_hb                        @ MODULO - fold into low
	str     $ctr32w, [$counter, #12]                         @ store the updated counter

	st1     { $res1b}, [$output_ptr]                         @ store all 16B

	eor     $acc_lb, $acc_lb, $acc_mb                        @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]

	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

.L192_enc_ret:
	mov w0, #0x0
	ret
.size aes_gcm_enc_192_kernel,.-aes_gcm_enc_192_kernel
___

#########################################################################################
# size_t aes_gcm_dec_192_kernel(const uint8_t * ciphertext,
#                               uint64_t plaintext_length,
#                               uint8_t * plaintext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_dec_192_kernel
.type   aes_gcm_dec_192_kernel,%function
.align  4
aes_gcm_dec_192_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L192_dec_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   @ end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk12_l, $rk12_h, [$cc, #192]                     @ load rk12
#ifdef __AARCH64EB__
	ror     $rk12_l, $rk12_l, #32
	ror     $rk12_h, $rk12_h, #32
#endif
	ld1     { $ctr0b}, [$counter]                             @ special case vector load initial counter so we can start first AES block as quickly as possible

	ld1     {$rk0s}, [$cc], #16                                  @ load rk0

	lsr     $main_end_input_ptr, $bit_length, #3              @ byte_len
	mov     $len, $main_end_input_ptr
	ld1     {$rk1s}, [$cc], #16                               @ load rk1

	lsr     $rctr32x, $ctr96_t32x, #32
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 3

	rev     $rctr32w, $rctr32w                                @ rev_ctr32
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 1

	add     $rctr32w, $rctr32w, #1                            @ increment rev_ctr32
	ld1     {$rk2s}, [$cc], #16                               @ load rk2

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 0
	rev     $ctr32w, $rctr32w                                 @ CTR block 1

	add     $rctr32w, $rctr32w, #1                            @ CTR block 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 1
	ld1     {$rk3s}, [$cc], #16                               @ load rk3

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 2

	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 2

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 2
	rev     $ctr32w, $rctr32w                                 @ CTR block 3

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 3

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 3

	ld1     {$rk4s}, [$cc], #16                               @ load rk4

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 2

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 0
	ld1     {$rk5s}, [$cc], #16                               @ load rk5

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 0
	ldr     $h4q, [$current_tag, #112]                        @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 0
	ldr     $h2q, [$current_tag, #64]                         @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 1
	ldr     $h3q, [$current_tag, #80]                         @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 1

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 1
	ldr     $h1q, [$current_tag, #32]                         @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 2
	ld1     {$rk6s}, [$cc], #16                               @ load rk6

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 3
	ld1     {$rk7s}, [$cc], #16                               @ load rk7

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 2
	ld1     {$rk8s}, [$cc], #16                               @ load rk8

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 2
	ld1     {$rk9s}, [$cc], #16                               @ load rk9

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 3
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 3
	add     $rctr32w, $rctr32w, #1                            @ CTR block 3

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 3
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      @ h4h | h3h

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 4
	ld1     {$rk10s}, [$cc], #16                              @ load rk10

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 4
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      @ h4l | h3l

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 4

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 4
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      @ h2l | h1l

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 5
	ld1     {$rk11s}, [$cc], #16                              @ load rk11

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 5

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 5

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 6

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 6

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 6

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 7

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 7

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 6

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 8

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 7

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 9

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 9

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 8
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      @ byte_len - 1

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 8
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0    @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 10
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 9
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 4 blocks

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 9
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      @ h2h | h1h

	aese    $ctr3b, $rk11                                     @ AES block 3 - round 11

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 10

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 10

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 10
	eor     $h12k.16b, $h12k.16b, $t0.16b                     @ h2k | h1k

	aese    $ctr2b, $rk11                                     @ AES block 2 - round 11

	aese    $ctr1b, $rk11                                     @ AES block 1 - round 11
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  @ h4k | h3k

	aese    $ctr0b, $rk11                                     @ AES block 0 - round 11
	b.ge    .L192_dec_tail                                    @ handle tail

	ld1     {$res0b, $res1b}, [$input_ptr], #32               @ AES block 0,1 - load ciphertext

	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 1 - result

	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 0 - result
	rev     $ctr32w, $rctr32w                                 @ CTR block 4
	ld1     {$res2b, $res3b}, [$input_ptr], #32               @ AES block 2,3 - load ciphertext

	mov     $output_l1, $ctr1.d[0]                            @ AES block 1 - mov low

	mov     $output_h1, $ctr1.d[1]                            @ AES block 1 - mov high

	mov     $output_l0, $ctr0.d[0]                            @ AES block 0 - mov low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4

	mov     $output_h0, $ctr0.d[1]                            @ AES block 0 - mov high
	rev64   $res0b, $res0b                                    @ GHASH block 0

	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4
	rev64   $res1b, $res1b                                    @ GHASH block 1
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 8 blocks

	eor     $output_l1, $output_l1, $rk12_l                   @ AES block 1 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4
	rev     $ctr32w, $rctr32w                                 @ CTR block 5

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 5
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 5
	eor     $output_h1, $output_h1, $rk12_h                   @ AES block 1 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	add     $rctr32w, $rctr32w, #1                            @ CTR block 5
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 5
	eor     $output_l0, $output_l0, $rk12_l                   @ AES block 0 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	rev     $ctr32w, $rctr32w                                 @ CTR block 6
	eor     $output_h0, $output_h0, $rk12_h                   @ AES block 0 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 0 - store result
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 6

	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 1 - store result

	add     $rctr32w, $rctr32w, #1                            @ CTR block 6
	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 2 - result
	b.ge    .L192_dec_prepretail                              @ do prepretail

	.L192_dec_main_loop:                                      @ main loop start
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low

	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high
	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2
	eor     $output_h2, $output_h2, $rk12_h                   @ AES block 4k+2 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low
	eor     $output_l2, $output_l2, $rk12_l                   @ AES block 4k+2 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3

	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5

	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	movi    $mod_constant.8b, #0xc2

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	ld1     {$res0b}, [$input_ptr], #16                       @ AES block 4k+4 - load ciphertext

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	ld1     {$res1b}, [$input_ptr], #16                       @ AES block 4k+5 - load ciphertext
	eor     $output_l3, $output_l3, $rk12_l                   @ AES block 4k+3 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr0b, $rk11                                     @ AES block 4k+4 - round 11
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	ld1     {$res2b}, [$input_ptr], #16                       @ AES block 4k+6 - load ciphertext

	aese    $ctr1b, $rk11                                     @ AES block 4k+5 - round 11
	ld1     {$res3b}, [$input_ptr], #16                       @ AES block 4k+7 - load ciphertext
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	cmp     $input_ptr, $main_end_input_ptr                   @ LOOP CONTROL

	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 4k+4 - result
	eor     $output_h3, $output_h3, $rk12_h                   @ AES block 4k+3 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif
	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 4k+5 - result

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+8

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	mov     $output_l1, $ctr1.d[0]                            @ AES block 4k+5 - mov low

	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result
	rev64   $res1b, $res1b                                    @ GHASH block 4k+5

	aese    $ctr2b, $rk11                                     @ AES block 4k+6 - round 11
	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10
	mov     $output_h1, $ctr1.d[1]                            @ AES block 4k+5 - mov high

	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4k+8
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+8
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 4k+6 - result
	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4k+8
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+9

	eor     $output_l0, $output_l0, $rk12_l                   @ AES block 4k+4 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+9
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 4k+9
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+9
	eor     $output_l1, $output_l1, $rk12_l                   @ AES block 4k+5 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 4k+9
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+10
	eor     $output_h1, $output_h1, $rk12_h                   @ AES block 4k+5 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	eor     $output_h0, $output_h0, $rk12_h                   @ AES block 4k+4 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 4k+4 - store result
	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low

	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+10
	rev64   $res0b, $res0b                                    @ GHASH block 4k+4
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+10

	aese    $ctr3b, $rk11                                     @ AES block 4k+7 - round 11
	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 4k+5 - store result
	b.lt    .L192_dec_main_loop

	.L192_dec_prepretail:                                     @ PREPRETAIL
	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	eor     $res0b, $res0b, $acc_lb                           @ PRE 1
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	eor     $output_h3, $output_h3, $rk12_h                   @ AES block 4k+3 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2
	eor     $output_l2, $output_l2, $rk12_l                   @ AES block 4k+2 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif
	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high
	eor     $output_h2, $output_h2, $rk12_h                   @ AES block 4k+2 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	eor     $output_l3, $output_l3, $rk12_l                   @ AES block 4k+3 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	rev64   $res3b, $res3b                                    @ GHASH block 4k+3
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0

	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high
	movi    $mod_constant.8b, #0xc2

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10

	aese    $ctr0b, $rk11
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	aese    $ctr2b, $rk11

	aese    $ctr1b, $rk11

	aese    $ctr3b, $rk11

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	.L192_dec_tail:                                           @ TAIL

	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   @ main_end_input_ptr is number of bytes left to process
	ld1     { $res1b}, [$input_ptr], #16                      @ AES block 4k+4 - load ciphertext

	eor     $ctr0b, $res1b, $ctr0b                            @ AES block 4k+4 - result

	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high

	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low

	ext     $t0.16b, $acc_lb, $acc_lb, #8                     @ prepare final partial tag

	cmp     $main_end_input_ptr, #48

	eor     $output_h0, $output_h0, $rk12_h                   @ AES block 4k+4 - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $output_l0, $output_l0, $rk12_l                   @ AES block 4k+4 - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	b.gt    .L192_dec_blocks_more_than_3

	movi    $acc_l.8b, #0
	movi    $acc_h.8b, #0

	mov     $ctr3b, $ctr2b
	mov     $ctr2b, $ctr1b
	sub     $rctr32w, $rctr32w, #1

	movi    $acc_m.8b, #0
	cmp     $main_end_input_ptr, #32
	b.gt    .L192_dec_blocks_more_than_2

	mov     $ctr3b, $ctr1b
	cmp     $main_end_input_ptr, #16
	sub     $rctr32w, $rctr32w, #1

	b.gt    .L192_dec_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L192_dec_blocks_less_than_1
	.L192_dec_blocks_more_than_3:                             @ blocks left >  3
	rev64   $res0b, $res1b                                    @ GHASH final-3 block
	ld1     { $res1b}, [$input_ptr], #16                      @ AES final-2 block - load ciphertext

	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-3 block  - store result

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	eor     $ctr0b, $res1b, $ctr1b                            @ AES final-2 block - result

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH final-3 block - low
	mov     $output_l0, $ctr0.d[0]                            @ AES final-2 block - mov low
	mov     $rk4d, $res0.d[1]                                 @ GHASH final-3 block - mid

	mov     $output_h0, $ctr0.d[1]                            @ AES final-2 block - mov high

	mov     $acc_md, $h34k.d[1]                               @ GHASH final-3 block - mid
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-3 block - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH final-3 block - high

	eor     $output_l0, $output_l0, $rk12_l                   @ AES final-2 block - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	movi    $t0.8b, #0                                        @ suppress further partial tag feed in

	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                    @ GHASH final-3 block - mid
	eor     $output_h0, $output_h0, $rk12_h                   @ AES final-2 block - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	.L192_dec_blocks_more_than_2:                             @ blocks left >  2

	rev64   $res0b, $res1b                                    @ GHASH final-2 block
	ld1     { $res1b}, [$input_ptr], #16                      @ AES final-1 block - load ciphertext

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in

	eor     $ctr0b, $res1b, $ctr2b                            @ AES final-1 block - result

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                          @ GHASH final-2 block - low

	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-2 block  - store result

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-2 block - mid
	mov     $output_h0, $ctr0.d[1]                            @ AES final-1 block - mov high

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-2 block - low
	mov     $output_l0, $ctr0.d[0]                            @ AES final-1 block - mov low

	pmull2  $rk2q1, $res0.2d, $h3.2d                          @ GHASH final-2 block - high

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                      @ GHASH final-2 block - mid

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-2 block - high
	eor     $output_h0, $output_h0, $rk12_h                   @ AES final-1 block - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $output_l0, $output_l0, $rk12_l                   @ AES final-1 block - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-2 block - mid
	.L192_dec_blocks_more_than_1:                             @ blocks left >  1

	rev64   $res0b, $res1b                                    @ GHASH final-1 block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag
	ld1     { $res1b}, [$input_ptr], #16                      @ AES final block - load ciphertext

	mov     $rk4d, $res0.d[1]                                 @ GHASH final-1 block - mid

	pmull2  $rk2q1, $res0.2d, $h2.2d                          @ GHASH final-1 block - high

	eor     $ctr0b, $res1b, $ctr3b                            @ AES final block - result
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES final-1 block  - store result

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                      @ GHASH final-1 block - mid

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final-1 block - high

	pmull   $rk3q1, $res0.1d, $h2.1d                          @ GHASH final-1 block - low
	mov     $output_h0, $ctr0.d[1]                            @ AES final block - mov high

	ins     $rk4v.d[1], $rk4v.d[0]                            @ GHASH final-1 block - mid
	mov     $output_l0, $ctr0.d[0]                            @ AES final block - mov low

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                      @ GHASH final-1 block - mid

	movi    $t0.8b, #0                                        @ suppress further partial tag feed in
	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final-1 block - low
	eor     $output_h0, $output_h0, $rk12_h                   @ AES final block - round 12 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $output_l0, $output_l0, $rk12_l                   @ AES final block - round 12 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $acc_mb, $acc_mb, $rk4v.16b                       @ GHASH final-1 block - mid
	.L192_dec_blocks_less_than_1:                             @ blocks left <= 1

	mvn     $rk12_l, xzr                                      @ rk12_l = 0xffffffffffffffff
	ldp     $end_input_ptr, $main_end_input_ptr, [$output_ptr]  @ load existing bytes we need to not overwrite
	and     $bit_length, $bit_length, #127                    @ bit_length %= 128

	sub     $bit_length, $bit_length, #128                    @ bit_length -= 128

	neg     $bit_length, $bit_length                          @ bit_length = 128 - #bits in input (in range [1,128])

	and     $bit_length, $bit_length, #127                    @ bit_length %= 128
	mvn     $rk12_h, xzr                                      @ rk12_h = 0xffffffffffffffff

	lsr     $rk12_h, $rk12_h, $bit_length                     @ rk12_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $ctr32x, $rk12_l, $rk12_h, lt
	csel    $ctr96_b64x, $rk12_h, xzr, lt

	fmov    $ctr0d, $ctr32x                                   @ ctr0b is mask for last block
	and     $output_l0, $output_l0, $ctr32x
	bic     $end_input_ptr, $end_input_ptr, $ctr32x           @ mask out low existing bytes

	orr     $output_l0, $output_l0, $end_input_ptr
	mov     $ctr0.d[1], $ctr96_b64x
#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif

	and     $res1b, $res1b, $ctr0b                            @ possibly partial last block has zeroes in highest bits
	str     $ctr32w, [$counter, #12]                          @ store the updated counter

	rev64   $res0b, $res1b                                    @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag
	bic     $main_end_input_ptr, $main_end_input_ptr, $ctr96_b64x @ mask out high existing bytes

	and     $output_h0, $output_h0, $ctr96_b64x

	pmull2  $rk2q1, $res0.2d, $h1.2d                          @ GHASH final block - high
	mov     $t0d, $res0.d[1]                                  @ GHASH final block - mid

	pmull   $rk3q1, $res0.1d, $h1.1d                          @ GHASH final block - low

	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH final block - mid

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final block - high

	pmull   $t0.1q, $t0.1d, $h12k.1d                          @ GHASH final block - mid

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final block - low

	eor     $acc_mb, $acc_mb, $t0.16b                         @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	orr     $output_h0, $output_h0, $main_end_input_ptr
	stp     $output_l0, $output_h0, [$output_ptr]

	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low

	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]

	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

.L192_dec_ret:
	mov w0, #0x0
	ret
.size aes_gcm_dec_192_kernel,.-aes_gcm_dec_192_kernel
___
}

{
my ($end_input_ptr,$main_end_input_ptr,$input_l0,$input_h0)=map("x$_",(4..7));
my ($input_l1,$input_h1,$input_l2,$input_h2,$input_l3,$input_h3)=map("x$_",(19..24));
my ($output_l1,$output_h1,$output_l2,$output_h2,$output_l3,$output_h3)=map("x$_",(19..24));
my ($output_l0,$output_h0)=map("x$_",(6..7));

my $ctr32w="w9";
my ($ctr32x,$ctr96_b64x,$ctr96_t32x,$rctr32x,$rk14_l,$rk14_h,$len)=map("x$_",(9..15));
my ($ctr96_t32w,$rctr32w)=map("w$_",(11..12));

my ($ctr0b,$ctr1b,$ctr2b,$ctr3b,$res0b,$res1b,$res2b,$res3b)=map("v$_.16b",(0..7));
my ($ctr0,$ctr1,$ctr2,$ctr3,$res0,$res1,$res2,$res3)=map("v$_",(0..7));
my ($ctr0d,$ctr1d,$ctr2d,$ctr3d,$res0d,$res1d,$res2d,$res3d)=map("d$_",(0..7));
my ($res0q,$res1q,$res2q,$res3q)=map("q$_",(4..7));

my ($acc_hb,$acc_mb,$acc_lb)=map("v$_.16b",(9..11));
my ($acc_h,$acc_m,$acc_l)=map("v$_",(9..11));
my ($acc_hd,$acc_md,$acc_ld)=map("d$_",(9..11));

my ($h1,$h2,$h3,$h4,$h12k,$h34k)=map("v$_",(12..17));
my ($h1q,$h2q,$h3q,$h4q)=map("q$_",(12..15));
my ($h1b,$h2b,$h3b,$h4b)=map("v$_.16b",(12..15));

my $t0="v8";
my $t0d="d8";
my $t1="v4";
my $t1d="d4";
my $t2="v8";
my $t2d="d8";
my $t3="v4";
my $t3d="d4";
my $t4="v4";
my $t4d="d4";
my $t5="v5";
my $t5d="d5";
my $t6="v8";
my $t6d="d8";
my $t7="v5";
my $t7d="d5";
my $t8="v6";
my $t8d="d6";
my $t9="v4";
my $t9d="d4";

my ($ctr_t0,$ctr_t1,$ctr_t2,$ctr_t3)=map("v$_",(4..7));
my ($ctr_t0d,$ctr_t1d,$ctr_t2d,$ctr_t3d)=map("d$_",(4..7));
my ($ctr_t0b,$ctr_t1b,$ctr_t2b,$ctr_t3b)=map("v$_.16b",(4..7));

my $mod_constantd="d8";
my $mod_constant="v8";
my $mod_t="v7";

my ($rk0,$rk1,$rk2,$rk3,$rk4,$rk5,$rk6,$rk7,$rk8,$rk9,$rk10,$rk11,$rk12,$rk13)=map("v$_.16b",(18..31));
my ($rk0s,$rk1s,$rk2s,$rk3s,$rk4s,$rk5s,$rk6s,$rk7s,$rk8s,$rk9s,$rk10s,$rk11s,$rk12s,$rk13s)=map("v$_.4s",(18..31));
my ($rk0q,$rk1q,$rk2q,$rk3q,$rk4q,$rk5q,$rk6q,$rk7q,$rk8q,$rk9q,$rk10q,$rk11q,$rk12q,$rk13q)=map("q$_",(18..31));
my $rk2q1="v20.1q";
my $rk3q1="v21.1q";
my $rk4v="v22";
my $rk4d="d22";

#########################################################################################
# size_t aes_gcm_enc_256_kernel(const uint8_t * plaintext,
#                               uint64_t plaintext_length,
#                               uint8_t * ciphertext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_enc_256_kernel
.type   aes_gcm_enc_256_kernel,%function
.align  4
aes_gcm_enc_256_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L256_enc_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   @ end_input_ptr
	lsr     $main_end_input_ptr, $bit_length, #3              @ byte_len
	mov     $len, $main_end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk14_l, $rk14_h, [$cc, #224]                     @ load rk14
#ifdef __AARCH64EB__
	ror     $rk14_l, $rk14_l, #32
	ror     $rk14_h, $rk14_h, #32
#endif
	ld1     { $ctr0b}, [$counter]                             @ special case vector load initial counter so we can start first AES block as quickly as possible
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      @ byte_len - 1

	ld1     {$rk0s}, [$cc], #16                               @ load rk0
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0 @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	ld1     {$rk1s}, [$cc], #16                               @ load rk1
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr

	lsr     $rctr32x, $ctr96_t32x, #32
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 2
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w

	rev     $rctr32w, $rctr32w                                @ rev_ctr32
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 4 blocks
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 1

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 0
	add     $rctr32w, $rctr32w, #1                            @ increment rev_ctr32

	rev     $ctr32w, $rctr32w                                 @ CTR block 1
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 3

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 1
	add     $rctr32w, $rctr32w, #1                            @ CTR block 1
	ld1     {$rk2s}, [$cc], #16                               @ load rk2

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 2

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 2
	ld1     {$rk3s}, [$cc], #16                               @ load rk3

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 2
	rev     $ctr32w, $rctr32w                                 @ CTR block 3

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 3

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 3

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 0
	ld1     {$rk4s}, [$cc], #16                               @ load rk4

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 2
	ld1     {$rk5s}, [$cc], #16                               @ load rk5

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 0
	ld1     {$rk6s}, [$cc], #16                               @ load rk6

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 1
	ldr     $h3q, [$current_tag, #80]                         @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 0
	ld1     {$rk7s}, [$cc], #16                               @ load rk7

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 1
	ld1     {$rk8s}, [$cc], #16                               @ load rk8

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 2
	ldr     $h2q, [$current_tag, #64]                         @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 1
	ld1     {$rk9s}, [$cc], #16                               @ load rk9

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 2
	ldr     $h4q, [$current_tag, #112]                        @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 3
	ld1     {$rk10s}, [$cc], #16                              @ load rk10

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 2
	ld1     {$rk11s}, [$cc], #16                              @ load rk11

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 3
	add     $rctr32w, $rctr32w, #1                            @ CTR block 3

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 3

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 3
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 4

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 4

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 4

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 4

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 5

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 5

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 5

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 6
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      @ h4l | h3l

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 6
	ld1     {$rk12s}, [$cc], #16                              @ load rk12

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 6
	ldr     $h1q, [$current_tag, #32]                         @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 6
	ld1     {$rk13s}, [$cc], #16                              @ load rk13

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 7
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      @ h4h | h3h

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 7

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 7
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      @ h2l | h1l

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 8

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 8

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 8

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 9

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 9

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 8

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 10

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 9

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 9

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 10

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 10

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 11

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 11

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 10

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 12

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 12

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 11
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  @ h4k | h3k

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 11

	aese    $ctr2b, $rk13                                     @ AES block 2 - round 13
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      @ h2h | h1h

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 12

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 12

	aese    $ctr1b, $rk13                                     @ AES block 1 - round 13

	aese    $ctr0b, $rk13                                     @ AES block 0 - round 13

	aese    $ctr3b, $rk13                                     @ AES block 3 - round 13
	eor     $h12k.16b, $h12k.16b, $t0.16b                     @ h2k | h1k
	b.ge    .L256_enc_tail                                    @ handle tail

	ldp     $input_l1, $input_h1, [$input_ptr, #16]           @ AES block 1 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	rev     $ctr32w, $rctr32w                                 @ CTR block 4
	ldp     $input_l0, $input_h0, [$input_ptr, #0]            @ AES block 0 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           @ AES block 3 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           @ AES block 2 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	add     $input_ptr, $input_ptr, #64                       @ AES input_ptr update

	eor     $input_l1, $input_l1, $rk14_l                     @ AES block 1 - round 14 low
	eor     $input_h1, $input_h1, $rk14_h                     @ AES block 1 - round 14 high

	fmov    $ctr_t1d, $input_l1                               @ AES block 1 - mov low
	eor     $input_l0, $input_l0, $rk14_l                     @ AES block 0 - round 14 low

	eor     $input_h0, $input_h0, $rk14_h                     @ AES block 0 - round 14 high
	eor     $input_h3, $input_h3, $rk14_h                     @ AES block 3 - round 14 high
	fmov    $ctr_t0d, $input_l0                               @ AES block 0 - mov low

	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 8 blocks
	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 0 - mov high
	eor     $input_l3, $input_l3, $rk14_l                     @ AES block 3 - round 14 low

	eor     $input_l2, $input_l2, $rk14_l                     @ AES block 2 - round 14 low
	fmov    $ctr_t1.d[1], $input_h1                           @ AES block 1 - mov high

	fmov    $ctr_t2d, $input_l2                               @ AES block 2 - mov low
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4
	fmov    $ctr_t3d, $input_l3                               @ AES block 3 - mov low
	eor     $input_h2, $input_h2, $rk14_h                     @ AES block 2 - round 14 high

	fmov    $ctr_t2.d[1], $input_h2                           @ AES block 2 - mov high

	eor     $res0b, $ctr_t0b, $ctr0b                          @ AES block 0 - result
	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4
	rev     $ctr32w, $rctr32w                                 @ CTR block 5
	add     $rctr32w, $rctr32w, #1                            @ CTR block 5

	eor     $res1b, $ctr_t1b, $ctr1b                          @ AES block 1 - result
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 5
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 5

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 5
	rev     $ctr32w, $rctr32w                                 @ CTR block 6
	st1     { $res0b}, [$output_ptr], #16                     @ AES block 0 - store result

	fmov    $ctr_t3.d[1], $input_h3                           @ AES block 3 - mov high
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 6
	eor     $res2b, $ctr_t2b, $ctr2b                          @ AES block 2 - result

	st1     { $res1b}, [$output_ptr], #16                     @ AES block 1 - store result

	add     $rctr32w, $rctr32w, #1                            @ CTR block 6
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 6

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 6
	st1     { $res2b}, [$output_ptr], #16                     @ AES block 2 - store result
	rev     $ctr32w, $rctr32w                                 @ CTR block 7

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 7

	eor     $res3b, $ctr_t3b, $ctr3b                          @ AES block 3 - result
	st1     { $res3b}, [$output_ptr], #16                     @ AES block 3 - store result
	b.ge    L256_enc_prepretail                               @ do prepretail

	.L256_enc_main_loop:                                      @ main loop start
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	rev64   $res0b, $res0b                                    @ GHASH block 4k (only t0 is free)

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+3

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+3

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           @ AES block 4k+7 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l3, $input_l3
	rev     $input_h3, $input_h3
#endif
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           @ AES block 4k+6 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l2, $input_l2
	rev     $input_h2, $input_h2
#endif
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	eor     $input_l3, $input_l3, $rk14_l                     @ AES block 4k+7 - round 14 low

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	eor     $input_h2, $input_h2, $rk14_h                     @ AES block 4k+6 - round 14 high
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	rev64   $res1b, $res1b                                    @ GHASH block 4k+1 (t0 and t1 free)

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3 (t0, t1, t2 and t3 free)

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2 (t0, t1, and t2 free)

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low

	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6
	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	ldp     $input_l1, $input_h1, [$input_ptr, #16]           @ AES block 4k+5 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l1, $input_l1
	rev     $input_h1, $input_h1
#endif
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	eor     $input_l1, $input_l1, $rk14_l                     @ AES block 4k+5 - round 14 low

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	eor     $input_l2, $input_l2, $rk14_l                     @ AES block 4k+6 - round 14 low

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9
	movi    $mod_constant.8b, #0xc2

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high
	fmov    $ctr_t1d, $input_l1                               @ AES block 4k+5 - mov low

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	ldp     $input_l0, $input_h0, [$input_ptr, #0]            @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+3

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 11
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 11
	add     $input_ptr, $input_ptr, #64                       @ AES input_ptr update

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+8
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10
	eor     $input_l0, $input_l0, $rk14_l                     @ AES block 4k+4 - round 14 low

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 12
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10
	eor     $input_h0, $input_h0, $rk14_h                     @ AES block 4k+4 - round 14 high

	fmov    $ctr_t0d, $input_l0                               @ AES block 4k+4 - mov low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+8
	eor     $mod_t.16b, $acc_hb, $mod_t.16b                   @ MODULO - fold into mid

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 12
	eor     $input_h1, $input_h1, $rk14_h                     @ AES block 4k+5 - round 14 high

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 11
	eor     $input_h3, $input_h3, $rk14_h                     @ AES block 4k+7 - round 14 high

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 11
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+8

	aese    $ctr0b, $rk13                                     @ AES block 4k+4 - round 13
	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 4k+4 - mov high
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 12
	fmov    $ctr_t3d, $input_l3                               @ AES block 4k+7 - mov low

	aese    $ctr1b, $rk13                                     @ AES block 4k+5 - round 13
	fmov    $ctr_t1.d[1], $input_h1                           @ AES block 4k+5 - mov high

	fmov    $ctr_t2d, $input_l2                               @ AES block 4k+6 - mov low
	cmp     $input_ptr, $main_end_input_ptr                   @ LOOP CONTROL

	fmov    $ctr_t2.d[1], $input_h2                           @ AES block 4k+6 - mov high

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d            @ MODULO - mid 64b align with low
	eor     $res0b, $ctr_t0b, $ctr0b                          @ AES block 4k+4 - result
	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4k+8

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4k+8
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+9
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+9

	eor     $res1b, $ctr_t1b, $ctr1b                          @ AES block 4k+5 - result
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 4k+9
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+9

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 12
	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 4k+9

	aese    $ctr2b, $rk13                                     @ AES block 4k+6 - round 13
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+10
	st1     { $res0b}, [$output_ptr], #16                     @ AES block 4k+4 - store result

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+10
	eor     $acc_lb, $acc_lb, $acc_hb                         @ MODULO - fold into low
	fmov    $ctr_t3.d[1], $input_h3                           @ AES block 4k+7 - mov high

	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment
	st1     { $res1b}, [$output_ptr], #16                     @ AES block 4k+5 - store result
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+10

	aese    $ctr3b, $rk13                                     @ AES block 4k+7 - round 13
	eor     $res2b, $ctr_t2b, $ctr2b                          @ AES block 4k+6 - result
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+10

	st1     { $res2b}, [$output_ptr], #16                     @ AES block 4k+6 - store result
	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+10
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+11

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+11

	eor     $res3b, $ctr_t3b, $ctr3b                          @ AES block 4k+7 - result
	st1     { $res3b}, [$output_ptr], #16                     @ AES block 4k+7 - store result
	b.lt    L256_enc_main_loop

	.L256_enc_prepretail:                                     @ PREPRETAIL
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2 (t0, t1, and t2 free)

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+3

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	rev64   $res0b, $res0b                                    @ GHASH block 4k (only t0 is free)

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+3
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1

	eor     $res0b, $res0b, $acc_lb                           @ PRE 1
	rev64   $res1b, $res1b                                    @ GHASH block 4k+1 (t0 and t1 free)

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2

	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3

	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3 (t0, t1, t2 and t3 free)

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+3

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high

	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5

	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	movi    $mod_constant.8b, #0xc2

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9

	eor     $acc_mb, $acc_mb, $acc_hb                         @ karatsuba tidy up

	pmull   $t1.1q, $acc_h.1d, $mod_constant.1d
	ext     $acc_hb, $acc_hb, $acc_hb, #8

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	eor     $acc_mb, $acc_mb, $acc_lb

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 11
	eor     $acc_mb, $acc_mb, $t1.16b

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 12

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 11
	eor     $acc_mb, $acc_mb, $acc_hb

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 11

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 12

	pmull   $t1.1q, $acc_m.1d, $mod_constant.1d

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 11
	ext     $acc_mb, $acc_mb, $acc_mb, #8

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 12

	aese    $ctr1b, $rk13                                     @ AES block 4k+5 - round 13
	eor     $acc_lb, $acc_lb, $t1.16b

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 12

	aese    $ctr3b, $rk13                                     @ AES block 4k+7 - round 13

	aese    $ctr0b, $rk13                                     @ AES block 4k+4 - round 13

	aese    $ctr2b, $rk13                                     @ AES block 4k+6 - round 13
	eor     $acc_lb, $acc_lb, $acc_mb
	.L256_enc_tail:                                           @ TAIL

	ext     $t0.16b, $acc_lb, $acc_lb, #8                     @ prepare final partial tag
	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   @ main_end_input_ptr is number of bytes left to process
	ldp     $input_l0, $input_h0, [$input_ptr], #16           @ AES block 4k+4 - load plaintext
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $input_l0, $input_l0, $rk14_l                     @ AES block 4k+4 - round 14 low
	eor     $input_h0, $input_h0, $rk14_h                     @ AES block 4k+4 - round 14 high

	cmp     $main_end_input_ptr, #48
	fmov    $ctr_t0d, $input_l0                               @ AES block 4k+4 - mov low

	fmov    $ctr_t0.d[1], $input_h0                           @ AES block 4k+4 - mov high

	eor     $res1b, $ctr_t0b, $ctr0b                          @ AES block 4k+4 - result
	b.gt    .L256_enc_blocks_more_than_3

	cmp     $main_end_input_ptr, #32
	mov     $ctr3b, $ctr2b
	movi    $acc_l.8b, #0

	movi    $acc_h.8b, #0
	sub     $rctr32w, $rctr32w, #1

	mov     $ctr2b, $ctr1b
	movi    $acc_m.8b, #0
	b.gt    .L256_enc_blocks_more_than_2

	mov     $ctr3b, $ctr1b
	sub     $rctr32w, $rctr32w, #1
	cmp     $main_end_input_ptr, #16

	b.gt    .L256_enc_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L256_enc_blocks_less_than_1
	.L256_enc_blocks_more_than_3:                            @ blocks left >  3
	st1     { $res1b}, [$output_ptr], #16                    @ AES final-3 block  - store result

	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final-2 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	rev64   $res0b, $res1b                                   @ GHASH final-3 block

	eor     $input_l0, $input_l0, $rk14_l                    @ AES final-2 block - round 14 low
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	eor     $input_h0, $input_h0, $rk14_h                    @ AES final-2 block - round 14 high

	mov     $rk4d, $res0.d[1]                                @ GHASH final-3 block - mid
	fmov    $res1d, $input_l0                                @ AES final-2 block - mov low

	fmov    $res1.d[1], $input_h0                            @ AES final-2 block - mov high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-3 block - mid
	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	mov     $acc_md, $h34k.d[1]                              @ GHASH final-3 block - mid

	pmull   $acc_l.1q, $res0.1d, $h4.1d                      @ GHASH final-3 block - low

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      @ GHASH final-3 block - high

	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                   @ GHASH final-3 block - mid
	eor     $res1b, $res1b, $ctr1b                           @ AES final-2 block - result
	.L256_enc_blocks_more_than_2:                            @ blocks left >  2

	st1     { $res1b}, [$output_ptr], #16                    @ AES final-2 block - store result

	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final-1 block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	rev64   $res0b, $res1b                                   @ GHASH final-2 block

	eor     $input_l0, $input_l0, $rk14_l                    @ AES final-1 block - round 14 low
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	fmov    $res1d, $input_l0                                @ AES final-1 block - mov low
	eor     $input_h0, $input_h0, $rk14_h                    @ AES final-1 block - round 14 high

	fmov    $res1.d[1], $input_h0                            @ AES final-1 block - mov high

	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	pmull2  $rk2q1, $res0.2d, $h3.2d                         @ GHASH final-2 block - high
	mov     $rk4d, $res0.d[1]                                @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                         @ GHASH final-2 block - low

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-2 block - mid

	eor     $res1b, $res1b, $ctr2b                           @ AES final-1 block - result

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-2 block - high

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                     @ GHASH final-2 block - mid

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-2 block - low

	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-2 block - mid
	.L256_enc_blocks_more_than_1:                            @ blocks left >  1

	st1     { $res1b}, [$output_ptr], #16                    @ AES final-1 block - store result

	rev64   $res0b, $res1b                                   @ GHASH final-1 block

	ldp     $input_l0, $input_h0, [$input_ptr], #16          @ AES final block - load input low & high
#ifdef __AARCH64EB__
	rev     $input_l0, $input_l0
	rev     $input_h0, $input_h0
#endif
	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	eor     $input_l0, $input_l0, $rk14_l                    @ AES final block - round 14 low
	mov     $rk4d, $res0.d[1]                                @ GHASH final-1 block - mid

	pmull2  $rk2q1, $res0.2d, $h2.2d                         @ GHASH final-1 block - high
	eor     $input_h0, $input_h0, $rk14_h                    @ AES final block - round 14 high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-1 block - mid

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-1 block - high

	ins     $rk4v.d[1], $rk4v.d[0]                           @ GHASH final-1 block - mid
	fmov    $res1d, $input_l0                                @ AES final block - mov low

	fmov    $res1.d[1], $input_h0                            @ AES final block - mov high

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                     @ GHASH final-1 block - mid

	pmull   $rk3q1, $res0.1d, $h2.1d                         @ GHASH final-1 block - low

	eor     $res1b, $res1b, $ctr3b                           @ AES final block - result
	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-1 block - mid

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-1 block - low
	.L256_enc_blocks_less_than_1:                            @ blocks left <= 1

	and     $bit_length, $bit_length, #127                   @ bit_length %= 128

	mvn     $rk14_l, xzr                                     @ rk14_l = 0xffffffffffffffff
	sub     $bit_length, $bit_length, #128                   @ bit_length -= 128

	neg     $bit_length, $bit_length                         @ bit_length = 128 - #bits in input (in range [1,128])
	ld1     { $rk0}, [$output_ptr]                           @ load existing bytes where the possibly partial last block is to be stored

	mvn     $rk14_h, xzr                                     @ rk14_h = 0xffffffffffffffff
	and     $bit_length, $bit_length, #127                   @ bit_length %= 128

	lsr     $rk14_h, $rk14_h, $bit_length                    @ rk14_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $input_l0, $rk14_l, $rk14_h, lt
	csel    $input_h0, $rk14_h, xzr, lt

	fmov    $ctr0d, $input_l0                                @ ctr0b is mask for last block

	fmov    $ctr0.d[1], $input_h0

	and     $res1b, $res1b, $ctr0b                           @ possibly partial last block has zeroes in highest bits

	rev64   $res0b, $res1b                                   @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	bif     $res1b, $rk0, $ctr0b                             @ insert existing bytes in top end of result before storing

	pmull2  $rk2q1, $res0.2d, $h1.2d                         @ GHASH final block - high
	mov     $t0d, $res0.d[1]                                 @ GHASH final block - mid
#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif

	pmull   $rk3q1, $res0.1d, $h1.1d                         @ GHASH final block - low

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final block - high
	eor     $t0.8b, $t0.8b, $res0.8b                         @ GHASH final block - mid

	pmull   $t0.1q, $t0.1d, $h12k.1d                         @ GHASH final block - mid

	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final block - low

	eor     $acc_mb, $acc_mb, $t0.16b                        @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $t9.16b, $acc_lb, $acc_hb                        @ MODULO - karatsuba tidy up

	shl     $mod_constantd, $mod_constantd, #56              @ mod_constant

	eor     $acc_mb, $acc_mb, $t9.16b                        @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d           @ MODULO - top 64b align with mid

	ext     $acc_hb, $acc_hb, $acc_hb, #8                    @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                     @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                        @ MODULO - fold into mid

	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d           @ MODULO - mid 64b align with low

	ext     $acc_mb, $acc_mb, $acc_mb, #8                    @ MODULO - other mid alignment

	str     $ctr32w, [$counter, #12]                         @ store the updated counter

	st1     { $res1b}, [$output_ptr]                         @ store all 16B
	eor     $acc_lb, $acc_lb, $acc_hb                        @ MODULO - fold into low

	eor     $acc_lb, $acc_lb, $acc_mb                        @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]

	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

.L256_enc_ret:
	mov w0, #0x0
	ret
.size aes_gcm_enc_256_kernel,.-aes_gcm_enc_256_kernel
___

{
my $t8="v4";
my $t8d="d4";
my $t9="v6";
my $t9d="d6";
#########################################################################################
# size_t aes_gcm_dec_256_kernel(const uint8_t * ciphertext,
#                               uint64_t plaintext_length,
#                               uint8_t * plaintext,
#                               uint64_t *Xi,
#                               unsigned char ivec[16],
#                               const void *key);
#
$code.=<<___;
.global aes_gcm_dec_256_kernel
.type   aes_gcm_dec_256_kernel,%function
.align  4
aes_gcm_dec_256_kernel:
	AARCH64_VALID_CALL_TARGET
	cbz     x1, .L256_dec_ret
	stp     x19, x20, [sp, #-112]!
	mov     x16, x4
	mov     x8, x5
	stp     x21, x22, [sp, #16]
	stp     x23, x24, [sp, #32]
	stp     d8, d9, [sp, #48]
	stp     d10, d11, [sp, #64]
	stp     d12, d13, [sp, #80]
	stp     d14, d15, [sp, #96]

	lsr     $main_end_input_ptr, $bit_length, #3              @ byte_len
	mov     $len, $main_end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              @ ctr96_b64, ctr96_t32
#ifdef __AARCH64EB__
	rev     $ctr96_b64x, $ctr96_b64x
	rev     $ctr96_t32x, $ctr96_t32x
#endif
	ldp     $rk14_l, $rk14_h, [$cc, #224]                     @ load rk14
#ifdef __AARCH64EB__
	ror     $rk14_h, $rk14_h, #32
	ror     $rk14_l, $rk14_l, #32
#endif
	ld1     {$rk0s}, [$cc], #16                               @ load rk0
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      @ byte_len - 1

	ld1     {$rk1s}, [$cc], #16                               @ load rk1
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0 @ number of bytes to be processed in main loop (at least 1 byte must be handled by tail)

	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   @ end_input_ptr
	ld1     {$rk2s}, [$cc], #16                               @ load rk2

	lsr     $rctr32x, $ctr96_t32x, #32
	ld1     {$rk3s}, [$cc], #16                               @ load rk3
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w

	ld1     {$rk4s}, [$cc], #16                               @ load rk4
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr
	rev     $rctr32w, $rctr32w                                @ rev_ctr32

	add     $rctr32w, $rctr32w, #1                            @ increment rev_ctr32
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 3

	rev     $ctr32w, $rctr32w                                 @ CTR block 1
	add     $rctr32w, $rctr32w, #1                            @ CTR block 1
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 1

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 1
	ld1     { $ctr0b}, [$counter]                             @ special case vector load initial counter so we can start first AES block as quickly as possible

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 2
	add     $rctr32w, $rctr32w, #1                            @ CTR block 2

	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 2

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 2
	rev     $ctr32w, $rctr32w                                 @ CTR block 3

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 3
	ld1     {$rk5s}, [$cc], #16                               @ load rk5

	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 3
	add     $rctr32w, $rctr32w, #1                            @ CTR block 3

	ld1     {$rk6s}, [$cc], #16                               @ load rk6

	ld1     {$rk7s}, [$cc], #16                               @ load rk7

	ld1     {$rk8s}, [$cc], #16                               @ load rk8

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 0
	ldr     $h3q, [$current_tag, #80]                         @ load h3l | h3h
#ifndef __AARCH64EB__
	ext     $h3b, $h3b, $h3b, #8
#endif

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 0
	ldr     $h4q, [$current_tag, #112]                        @ load h4l | h4h
#ifndef __AARCH64EB__
	ext     $h4b, $h4b, $h4b, #8
#endif

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 0
	ldr     $h2q, [$current_tag, #64]                         @ load h2l | h2h
#ifndef __AARCH64EB__
	ext     $h2b, $h2b, $h2b, #8
#endif

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 0
	ld1     {$rk9s}, [$cc], #16                                 @ load rk9

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 1

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 1
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 1
	ld1     {$rk10s}, [$cc], #16                              @ load rk10

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 1
	ld1     {$rk11s}, [$cc], #16                              @ load rk11

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 2
	ldr     $h1q, [$current_tag, #32]                         @ load h1l | h1h
#ifndef __AARCH64EB__
	ext     $h1b, $h1b, $h1b, #8
#endif
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 2
	ld1     {$rk12s}, [$cc], #16                              @ load rk12

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 2

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 3

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 2

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 3

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 4
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 4 blocks

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 3

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 3

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 4

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 4

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 4

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 5

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 5

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 5

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 5

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 6

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 6

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 6

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 6

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 7

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 7

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 7

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 8

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 7

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 8

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 8

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 9

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 8
	ld1     {$rk13s}, [$cc], #16                             @ load rk13

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 9

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 10

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 9

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 10

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 9

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 10

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 11

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 10

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 11

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 11

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 11

	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      @ h4h | h3h

	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      @ h4l | h3l

	trn1    $t0.2d,    $h1.2d,    $h2.2d                      @ h2h | h1h
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      @ h2l | h1l

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 1 - round 12

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 0 - round 12

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 2 - round 12

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 3 - round 12
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  @ h4k | h3k

	aese    $ctr1b, $rk13                                     @ AES block 1 - round 13

	aese    $ctr2b, $rk13                                     @ AES block 2 - round 13
	eor     $h12k.16b, $h12k.16b, $t0.16b                     @ h2k | h1k

	aese    $ctr3b, $rk13                                     @ AES block 3 - round 13

	aese    $ctr0b, $rk13                                     @ AES block 0 - round 13
	b.ge    .L256_dec_tail                                    @ handle tail

	ld1     {$res0b, $res1b}, [$input_ptr], #32               @ AES block 0,1 - load ciphertext

	rev     $ctr32w, $rctr32w                                 @ CTR block 4

	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 0 - result

	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 1 - result
	rev64   $res1b, $res1b                                    @ GHASH block 1
	ld1     {$res2b}, [$input_ptr], #16                       @ AES block 2 - load ciphertext

	mov     $output_h0, $ctr0.d[1]                            @ AES block 0 - mov high

	mov     $output_l0, $ctr0.d[0]                            @ AES block 0 - mov low
	rev64   $res0b, $res0b                                    @ GHASH block 0
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4

	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4

	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4
	rev     $ctr32w, $rctr32w                                 @ CTR block 5
	add     $rctr32w, $rctr32w, #1                            @ CTR block 5

	mov     $output_l1, $ctr1.d[0]                            @ AES block 1 - mov low

	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 5
	mov     $output_h1, $ctr1.d[1]                            @ AES block 1 - mov high
	eor     $output_h0, $output_h0, $rk14_h                   @ AES block 0 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	eor     $output_l0, $output_l0, $rk14_l                   @ AES block 0 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 0 - store result
	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 5

	ld1     {$res3b}, [$input_ptr], #16                       @ AES block 3 - load ciphertext

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 5
	rev     $ctr32w, $rctr32w                                 @ CTR block 6
	add     $rctr32w, $rctr32w, #1                            @ CTR block 6

	eor     $output_l1, $output_l1, $rk14_l                   @ AES block 1 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 6

	eor     $output_h1, $output_h1, $rk14_h                   @ AES block 1 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 1 - store result

	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 2 - result
	cmp     $input_ptr, $main_end_input_ptr                   @ check if we have <= 8 blocks
	b.ge    .L256_dec_prepretail                              @ do prepretail

	.L256_dec_main_loop:                                      @ main loop start
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	eor     $output_h2, $output_h2, $rk14_h                   @ AES block 4k+2 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	rev64   $res2b, $res2b                                    @ GHASH block 4k+2

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0
	eor     $output_l2, $output_l2, $rk14_l                   @ AES block 4k+2 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	eor     $output_l3, $output_l3, $rk14_l                   @ AES block 4k+3 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low
	eor     $output_h3, $output_h3, $rk14_h                   @ AES block 4k+3 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+8

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+8

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7
	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+8
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid
	movi    $mod_constant.8b, #0xc2

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 11

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 12

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9
	ld1     {$res0b}, [$input_ptr], #16                       @ AES block 4k+4 - load ciphertext

	aese    $ctr0b, $rk13                                     @ AES block 4k+4 - round 13
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9
	ld1     {$res1b}, [$input_ptr], #16                       @ AES block 4k+5 - load ciphertext

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8
	eor     $ctr0b, $res0b, $ctr0b                            @ AES block 4k+4 - result

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 11
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9
	ld1     {$res2b}, [$input_ptr], #16                       @ AES block 4k+6 - load ciphertext

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 12
	ld1     {$res3b}, [$input_ptr], #16                       @ AES block 4k+7 - load ciphertext

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 11
	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	aese    $ctr1b, $rk13                                     @ AES block 4k+5 - round 13
	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 12
	fmov    $ctr0d, $ctr96_b64x                               @ CTR block 4k+8

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 11
	fmov    $ctr0.d[1], $ctr32x                               @ CTR block 4k+8

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	eor     $ctr1b, $res1b, $ctr1b                            @ AES block 4k+5 - result
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+9

	aese    $ctr2b, $rk13                                     @ AES block 4k+6 - round 13
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+9
	cmp     $input_ptr, $main_end_input_ptr                   @ LOOP CONTROL

	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+9

	eor     $output_l0, $output_l0, $rk14_l                   @ AES block 4k+4 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $output_h0, $output_h0, $rk14_h                   @ AES block 4k+4 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	mov     $output_h1, $ctr1.d[1]                            @ AES block 4k+5 - mov high
	eor     $ctr2b, $res2b, $ctr2b                            @ AES block 4k+6 - result
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 12
	mov     $output_l1, $ctr1.d[0]                            @ AES block 4k+5 - mov low

	fmov    $ctr1d, $ctr96_b64x                               @ CTR block 4k+9
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	fmov    $ctr1.d[1], $ctr32x                               @ CTR block 4k+9
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+10
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+10

	aese    $ctr3b, $rk13                                     @ AES block 4k+7 - round 13
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+10

	rev64   $res1b, $res1b                                    @ GHASH block 4k+5
	eor     $output_h1, $output_h1, $rk14_h                   @ AES block 4k+5 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h1, $output_h1
#endif
	stp     $output_l0, $output_h0, [$output_ptr], #16        @ AES block 4k+4 - store result

	eor     $output_l1, $output_l1, $rk14_l                   @ AES block 4k+5 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l1, $output_l1
#endif
	stp     $output_l1, $output_h1, [$output_ptr], #16        @ AES block 4k+5 - store result

	rev64   $res0b, $res0b                                    @ GHASH block 4k+4
	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	b.lt    .L256_dec_main_loop


	.L256_dec_prepretail:                                     @ PREPRETAIL
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     @ PRE 0
	mov     $output_l2, $ctr2.d[0]                            @ AES block 4k+2 - mov low
	eor     $ctr3b, $res3b, $ctr3b                            @ AES block 4k+3 - result

	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 0
	mov     $output_h2, $ctr2.d[1]                            @ AES block 4k+2 - mov high

	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 0
	fmov    $ctr2d, $ctr96_b64x                               @ CTR block 4k+6

	fmov    $ctr2.d[1], $ctr32x                               @ CTR block 4k+6
	rev     $ctr32w, $rctr32w                                 @ CTR block 4k+7
	eor     $res0b, $res0b, $acc_lb                           @ PRE 1

	rev64   $res2b, $res2b                                    @ GHASH block 4k+2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            @ CTR block 4k+7
	mov     $output_l3, $ctr3.d[0]                            @ AES block 4k+3 - mov low

	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 1
	mov     $output_h3, $ctr3.d[1]                            @ AES block 4k+3 - mov high

	pmull   $acc_l.1q, $res0.1d, $h4.1d                       @ GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  @ GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               @ CTR block 4k+7

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       @ GHASH block 4k - high
	fmov    $ctr3.d[1], $ctr32x                               @ CTR block 4k+7

	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 0
	mov     $acc_md, $h34k.d[1]                               @ GHASH block 4k - mid

	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 1
	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH block 4k - mid

	pmull2  $t1.1q, $res1.2d, $h3.2d                          @ GHASH block 4k+1 - high

	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 1
	rev64   $res3b, $res3b                                    @ GHASH block 4k+3

	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 0

	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      @ GHASH block 4k - mid
	eor     $acc_hb, $acc_hb, $t1.16b                         @ GHASH block 4k+1 - high

	pmull   $t2.1q, $res1.1d, $h3.1d                          @ GHASH block 4k+1 - low

	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 1
	mov     $t3d, $res1.d[1]                                  @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 2

	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 2
	eor     $acc_lb, $acc_lb, $t2.16b                         @ GHASH block 4k+1 - low

	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 2

	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 3
	mov     $t6d, $res2.d[1]                                  @ GHASH block 4k+2 - mid

	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 2
	eor     $t3.8b, $t3.8b, $res1.8b                          @ GHASH block 4k+1 - mid

	pmull   $t5.1q, $res2.1d, $h2.1d                          @ GHASH block 4k+2 - low

	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 4

	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 3
	eor     $t6.8b, $t6.8b, $res2.8b                          @ GHASH block 4k+2 - mid

	pmull   $t3.1q, $t3.1d, $h34k.1d                          @ GHASH block 4k+1 - mid

	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 5
	eor     $acc_lb, $acc_lb, $t5.16b                         @ GHASH block 4k+2 - low

	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 4

	pmull2  $t7.1q, $res3.2d, $h1.2d                          @ GHASH block 4k+3 - high
	eor     $acc_mb, $acc_mb, $t3.16b                         @ GHASH block 4k+1 - mid

	pmull2  $t4.1q, $res2.2d, $h2.2d                          @ GHASH block 4k+2 - high

	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 5
	ins     $t6.d[1], $t6.d[0]                                @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 3

	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 3
	eor     $acc_hb, $acc_hb, $t4.16b                         @ GHASH block 4k+2 - high

	pmull   $t8.1q, $res3.1d, $h1.1d                          @ GHASH block 4k+3 - low

	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 4
	mov     $t9d, $res3.d[1]                                  @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 4

	pmull2  $t6.1q, $t6.2d, $h12k.2d                          @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 5
	eor     $t9.8b, $t9.8b, $res3.8b                          @ GHASH block 4k+3 - mid

	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 5

	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $t6.16b                         @ GHASH block 4k+2 - mid

	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 6

	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 6
	movi    $mod_constant.8b, #0xc2

	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 6
	eor     $acc_lb, $acc_lb, $t8.16b                         @ GHASH block 4k+3 - low

	pmull   $t9.1q, $t9.1d, $h12k.1d                          @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 7
	eor     $acc_hb, $acc_hb, $t7.16b                         @ GHASH block 4k+3 - high

	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 7

	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 7
	eor     $acc_mb, $acc_mb, $t9.16b                         @ GHASH block 4k+3 - mid

	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 8

	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 7
	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 8

	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 8
	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 8

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 9
	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid

	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 9
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 9

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 9
	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 10

	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 10

	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 10
	eor     $output_h2, $output_h2, $rk14_h                   @ AES block 4k+2 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h2, $output_h2
#endif
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 10
	eor     $output_l3, $output_l3, $rk14_l                   @ AES block 4k+3 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l3, $output_l3
#endif
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 11
	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 11
	add     $rctr32w, $rctr32w, #1                            @ CTR block 4k+7

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 11
	eor     $output_l2, $output_l2, $rk14_l                   @ AES block 4k+2 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l2, $output_l2
#endif

	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          @ AES block 4k+6 - round 12

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low
	eor     $output_h3, $output_h3, $rk14_h                   @ AES block 4k+3 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h3, $output_h3
#endif

	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 11
	stp     $output_l2, $output_h2, [$output_ptr], #16        @ AES block 4k+2 - store result

	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          @ AES block 4k+5 - round 12
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          @ AES block 4k+4 - round 12
	stp     $output_l3, $output_h3, [$output_ptr], #16        @ AES block 4k+3 - store result

	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          @ AES block 4k+7 - round 12
	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	aese    $ctr1b, $rk13                                     @ AES block 4k+5 - round 13

	aese    $ctr0b, $rk13                                     @ AES block 4k+4 - round 13

	aese    $ctr3b, $rk13                                     @ AES block 4k+7 - round 13

	aese    $ctr2b, $rk13                                     @ AES block 4k+6 - round 13
	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	.L256_dec_tail:                                           @ TAIL

	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   @ main_end_input_ptr is number of bytes left to process
	ld1     { $res1b}, [$input_ptr], #16                      @ AES block 4k+4 - load ciphertext

	eor     $ctr0b, $res1b, $ctr0b                            @ AES block 4k+4 - result

	mov     $output_l0, $ctr0.d[0]                            @ AES block 4k+4 - mov low

	mov     $output_h0, $ctr0.d[1]                            @ AES block 4k+4 - mov high
	ext     $t0.16b, $acc_lb, $acc_lb, #8                     @ prepare final partial tag

	cmp     $main_end_input_ptr, #48

	eor     $output_l0, $output_l0, $rk14_l                   @ AES block 4k+4 - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif

	eor     $output_h0, $output_h0, $rk14_h                   @ AES block 4k+4 - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	b.gt    .L256_dec_blocks_more_than_3

	sub     $rctr32w, $rctr32w, #1
	mov     $ctr3b, $ctr2b
	movi    $acc_m.8b, #0

	movi    $acc_l.8b, #0
	cmp     $main_end_input_ptr, #32

	movi    $acc_h.8b, #0
	mov     $ctr2b, $ctr1b
	b.gt    .L256_dec_blocks_more_than_2

	sub     $rctr32w, $rctr32w, #1

	mov     $ctr3b, $ctr1b
	cmp     $main_end_input_ptr, #16
	b.gt    .L256_dec_blocks_more_than_1

	sub     $rctr32w, $rctr32w, #1
	b       .L256_dec_blocks_less_than_1
	.L256_dec_blocks_more_than_3:                            @ blocks left >  3
	rev64   $res0b, $res1b                                   @ GHASH final-3 block
	ld1     { $res1b}, [$input_ptr], #16                     @ AES final-2 block - load ciphertext

	stp     $output_l0, $output_h0, [$output_ptr], #16       @ AES final-3 block  - store result

	mov     $acc_md, $h34k.d[1]                              @ GHASH final-3 block - mid

	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag

	eor     $ctr0b, $res1b, $ctr1b                           @ AES final-2 block - result

	mov     $rk4d, $res0.d[1]                                @ GHASH final-3 block - mid

	mov     $output_l0, $ctr0.d[0]                           @ AES final-2 block - mov low

	mov     $output_h0, $ctr0.d[1]                           @ AES final-2 block - mov high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-3 block - mid

	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      @ GHASH final-3 block - high

	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                   @ GHASH final-3 block - mid
	eor     $output_l0, $output_l0, $rk14_l                  @ AES final-2 block - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif

	pmull   $acc_l.1q, $res0.1d, $h4.1d                      @ GHASH final-3 block - low
	eor     $output_h0, $output_h0, $rk14_h                  @ AES final-2 block - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	.L256_dec_blocks_more_than_2:                            @ blocks left >  2

	rev64   $res0b, $res1b                                   @ GHASH final-2 block
	ld1     { $res1b}, [$input_ptr], #16                     @ AES final-1 block - load ciphertext

	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag
	stp     $output_l0, $output_h0, [$output_ptr], #16       @ AES final-2 block  - store result

	eor     $ctr0b, $res1b, $ctr2b                           @ AES final-1 block - result

	mov     $rk4d, $res0.d[1]                                @ GHASH final-2 block - mid

	pmull   $rk3q1, $res0.1d, $h3.1d                         @ GHASH final-2 block - low

	pmull2  $rk2q1, $res0.2d, $h3.2d                         @ GHASH final-2 block - high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-2 block - mid
	mov     $output_l0, $ctr0.d[0]                           @ AES final-1 block - mov low

	mov     $output_h0, $ctr0.d[1]                           @ AES final-1 block - mov high
	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-2 block - low
	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                     @ GHASH final-2 block - mid

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-2 block - high
	eor     $output_l0, $output_l0, $rk14_l                  @ AES final-1 block - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif

	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-2 block - mid
	eor     $output_h0, $output_h0, $rk14_h                  @ AES final-1 block - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	.L256_dec_blocks_more_than_1:                            @ blocks left >  1

	stp     $output_l0, $output_h0, [$output_ptr], #16       @ AES final-1 block  - store result
	rev64   $res0b, $res1b                                   @ GHASH final-1 block

	ld1     { $res1b}, [$input_ptr], #16                     @ AES final block - load ciphertext

	eor     $res0b, $res0b, $t0.16b                          @ feed in partial tag
	movi    $t0.8b, #0                                       @ suppress further partial tag feed in

	mov     $rk4d, $res0.d[1]                                @ GHASH final-1 block - mid

	eor     $ctr0b, $res1b, $ctr3b                           @ AES final block - result

	pmull2  $rk2q1, $res0.2d, $h2.2d                         @ GHASH final-1 block - high

	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     @ GHASH final-1 block - mid

	pmull   $rk3q1, $res0.1d, $h2.1d                         @ GHASH final-1 block - low
	mov     $output_l0, $ctr0.d[0]                           @ AES final block - mov low

	ins     $rk4v.d[1], $rk4v.d[0]                           @ GHASH final-1 block - mid

	mov     $output_h0, $ctr0.d[1]                           @ AES final block - mov high

	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                     @ GHASH final-1 block - mid
	eor     $output_l0, $output_l0, $rk14_l                  @ AES final block - round 14 low
#ifdef __AARCH64EB__
	rev     $output_l0, $output_l0
#endif
	eor     $acc_lb, $acc_lb, $rk3                           @ GHASH final-1 block - low

	eor     $acc_hb, $acc_hb, $rk2                           @ GHASH final-1 block - high

	eor     $acc_mb, $acc_mb, $rk4v.16b                      @ GHASH final-1 block - mid
	eor     $output_h0, $output_h0, $rk14_h                  @ AES final block - round 14 high
#ifdef __AARCH64EB__
	rev     $output_h0, $output_h0
#endif
	.L256_dec_blocks_less_than_1:                            @ blocks left <= 1

	and     $bit_length, $bit_length, #127                   @ bit_length %= 128
	mvn     $rk14_h, xzr                                     @ rk14_h = 0xffffffffffffffff

	sub     $bit_length, $bit_length, #128                   @ bit_length -= 128
	mvn     $rk14_l, xzr                                     @ rk14_l = 0xffffffffffffffff

	ldp     $end_input_ptr, $main_end_input_ptr, [$output_ptr] @ load existing bytes we need to not overwrite
	neg     $bit_length, $bit_length                         @ bit_length = 128 - #bits in input (in range [1,128])

	and     $bit_length, $bit_length, #127                   @ bit_length %= 128

	lsr     $rk14_h, $rk14_h, $bit_length                    @ rk14_h is mask for top 64b of last block
	cmp     $bit_length, #64

	csel    $ctr32x, $rk14_l, $rk14_h, lt
	csel    $ctr96_b64x, $rk14_h, xzr, lt

	fmov    $ctr0d, $ctr32x                                  @ ctr0b is mask for last block
	and     $output_l0, $output_l0, $ctr32x

	mov     $ctr0.d[1], $ctr96_b64x
	bic     $end_input_ptr, $end_input_ptr, $ctr32x          @ mask out low existing bytes

#ifndef __AARCH64EB__
	rev     $ctr32w, $rctr32w
#else
	mov     $ctr32w, $rctr32w
#endif

	bic     $main_end_input_ptr, $main_end_input_ptr, $ctr96_b64x      @ mask out high existing bytes

	orr     $output_l0, $output_l0, $end_input_ptr

	and     $output_h0, $output_h0, $ctr96_b64x

	orr     $output_h0, $output_h0, $main_end_input_ptr

	and     $res1b, $res1b, $ctr0b                            @ possibly partial last block has zeroes in highest bits

	rev64   $res0b, $res1b                                    @ GHASH final block

	eor     $res0b, $res0b, $t0.16b                           @ feed in partial tag

	pmull   $rk3q1, $res0.1d, $h1.1d                          @ GHASH final block - low

	mov     $t0d, $res0.d[1]                                  @ GHASH final block - mid

	eor     $t0.8b, $t0.8b, $res0.8b                          @ GHASH final block - mid

	pmull2  $rk2q1, $res0.2d, $h1.2d                          @ GHASH final block - high

	pmull   $t0.1q, $t0.1d, $h12k.1d                          @ GHASH final block - mid

	eor     $acc_hb, $acc_hb, $rk2                            @ GHASH final block - high

	eor     $acc_lb, $acc_lb, $rk3                            @ GHASH final block - low

	eor     $acc_mb, $acc_mb, $t0.16b                         @ GHASH final block - mid
	movi    $mod_constant.8b, #0xc2

	eor     $t9.16b, $acc_lb, $acc_hb                         @ MODULO - karatsuba tidy up

	shl     $mod_constantd, $mod_constantd, #56               @ mod_constant

	eor     $acc_mb, $acc_mb, $t9.16b                         @ MODULO - karatsuba tidy up

	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            @ MODULO - top 64b align with mid

	ext     $acc_hb, $acc_hb, $acc_hb, #8                     @ MODULO - other top alignment

	eor     $acc_mb, $acc_mb, $mod_t.16b                      @ MODULO - fold into mid

	eor     $acc_mb, $acc_mb, $acc_hb                         @ MODULO - fold into mid

	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     @ MODULO - mid 64b align with low

	ext     $acc_mb, $acc_mb, $acc_mb, #8                     @ MODULO - other mid alignment

	eor     $acc_lb, $acc_lb, $mod_constant.16b               @ MODULO - fold into low

	stp     $output_l0, $output_h0, [$output_ptr]

	str     $ctr32w, [$counter, #12]                          @ store the updated counter

	eor     $acc_lb, $acc_lb, $acc_mb                         @ MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]

	ldp     x21, x22, [sp, #16]
	ldp     x23, x24, [sp, #32]
	ldp     d8, d9, [sp, #48]
	ldp     d10, d11, [sp, #64]
	ldp     d12, d13, [sp, #80]
	ldp     d14, d15, [sp, #96]
	ldp     x19, x20, [sp], #112
	ret

.L256_dec_ret:
	mov w0, #0x0
	ret
.size aes_gcm_dec_256_kernel,.-aes_gcm_dec_256_kernel
___
}
}

$code.=<<___;
.rodata
.asciz  "GHASH for ARMv8, CRYPTOGAMS by <appro\@openssl.org>"
.align  2
#endif
___

if ($flavour =~ /64/) {         ######## 64-bit code
    sub unvmov {
        my $arg=shift;

        $arg =~ m/q([0-9]+)#(lo|hi),\s*q([0-9]+)#(lo|hi)/o &&
        sprintf "ins    v%d.d[%d],v%d.d[%d]",$1<8?$1:$1+8,($2 eq "lo")?0:1,
                             $3<8?$3:$3+8,($4 eq "lo")?0:1;
    }
    foreach(split("\n",$code)) {
        s/@\s/\/\//o;               # old->new style commentary
        print $_,"\n";
    }
} else {                ######## 32-bit code
    sub unvdup32 {
        my $arg=shift;

        $arg =~ m/q([0-9]+),\s*q([0-9]+)\[([0-3])\]/o &&
        sprintf "vdup.32    q%d,d%d[%d]",$1,2*$2+($3>>1),$3&1;
    }
    sub unvpmullp64 {
        my ($mnemonic,$arg)=@_;

        if ($arg =~ m/q([0-9]+),\s*q([0-9]+),\s*q([0-9]+)/o) {
            my $word = 0xf2a00e00|(($1&7)<<13)|(($1&8)<<19)
                       |(($2&7)<<17)|(($2&8)<<4)
                       |(($3&7)<<1) |(($3&8)<<2);
            $word |= 0x00010001  if ($mnemonic =~ "2");
            # since ARMv7 instructions are always encoded little-endian.
            # correct solution is to use .inst directive, but older%%%%
            # assemblers don't implement it:-(
            sprintf "INST(0x%02x,0x%02x,0x%02x,0x%02x)\t@ %s %s",
                    $word&0xff,($word>>8)&0xff,
                    ($word>>16)&0xff,($word>>24)&0xff,
                    $mnemonic,$arg;
        }
    }

    foreach(split("\n",$code)) {
        s/\b[wx]([0-9]+)\b/r$1/go;      # new->old registers
        s/\bv([0-9])\.[12468]+[bsd]\b/q$1/go;   # new->old registers
        s/\/\/\s?/@ /o;             # new->old style commentary

        # fix up remaining new-style suffixes
        s/\],#[0-9]+/]!/o;

        s/cclr\s+([^,]+),\s*([a-z]+)/mov.$2 $1,#0/o         or
        s/vdup\.32\s+(.*)/unvdup32($1)/geo              or
        s/v?(pmull2?)\.p64\s+(.*)/unvpmullp64($1,$2)/geo        or
        s/\bq([0-9]+)#(lo|hi)/sprintf "d%d",2*$1+($2 eq "hi")/geo   or
        s/^(\s+)b\./$1b/o                       or
        s/^(\s+)ret/$1bx\tlr/o;

        if (s/^(\s+)mov\.([a-z]+)/$1mov$2/) {
            print "     it      $2\n";
        }
        s/__AARCH64E([BL])__/__ARME$1__/go;
        print $_,"\n";
    }
}

close STDOUT or die "error closing STDOUT: $!"; # enforce flush
