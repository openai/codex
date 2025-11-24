#! /usr/bin/env perl

# Copyright (c) 2022, ARM Inc.
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

#========================================================================
# Written by Fangming Fang <fangming.fang@arm.com> for the OpenSSL project,
# derived from https://github.com/ARM-software/AArch64cryptolib, original
# author Samuel Lee <Samuel.Lee@arm.com>.
#========================================================================
#
# Approach - assume we don't want to reload constants, so reserve ~half of
# vector register file for constants
#
# main loop to act on 4 16B blocks per iteration, and then do modulo of the
# accumulated intermediate hashes from the 4 blocks
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
# PRE: Ensure previous generated intermediate hash is aligned and merged with
# result for GHASH 4k+0
#
# EXT low_acc, low_acc, low_acc, #8
# EOR res_curr (4k+0), res_curr (4k+0), low_acc
#
# CTR block: Increment and byte reverse counter in scalar registers and transfer
# to SIMD registers
#
# REV     ctr32, rev_ctr32
# ORR     ctr64, constctr96_top32, ctr32, LSL #32
# // Keeping this in scalar registers to free up space in SIMD RF
# INS     ctr_next.d[0], constctr96_bottom64
# INS     ctr_next.d[1], ctr64X
# ADD     rev_ctr32, #1
#
# AES block:
#
# Do AES encryption/decryption on CTR block X and EOR it with input block X.
# Take 256 bytes key below for example. Doing small trick here of loading input
# in scalar registers, EORing with last key and then transferring Given we are
# very constrained in our ASIMD registers this is quite important
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
#     Do 128b karatsuba polynomial multiplication on block. We only have
#     64b->128b polynomial multipliers, naively that means we need to do 4 64b
#     multiplies to generate a 128b.
#
# multiplication:
#     Pmull(A,B) == (Pmull(Ah,Bh)<<128 | Pmull(Al,Bl)) ^
#                   (Pmull(Ah,Bl) ^ Pmull(Al,Bh))<<64
#
#     The idea behind Karatsuba multiplication is that we can do just 3 64b
#     multiplies:
#     Pmull(A,B) == (Pmull(Ah,Bh)<<128 | Pmull(Al,Bl)) ^
#                   (Pmull(Ah^Al,Bh^Bl) ^ Pmull(Ah,Bh) ^
#                   Pmull(Al,Bl))<<64
#
#     There is some complication here because the bit order of GHASH's PMULL is
#     reversed compared to elsewhere, so we are multiplying with "twisted"
#     powers of H
#
# Note: We can PMULL directly into the acc_x in first GHASH of the loop
#
# Note: For scheduling big cores we want to split the processing to happen over
#       two loop iterations - otherwise the critical path latency dominates the
#       performance.
#
#       This has a knock on effect on register pressure, so we have to be a bit
#       more clever with our temporary registers than indicated here
#
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
# MODULO: take the partial accumulators (~representing sum of 256b
#         multiplication results), from GHASH and do modulo reduction on them
#         There is some complication here because the bit order of GHASH's
#         PMULL is reversed compared to elsewhere, so we are doing modulo with
#         a reversed constant
#
# EOR     acc_m, acc_m, acc_h
# EOR     acc_m, acc_m, acc_l                // Finish off karatsuba processing
# PMULL   t_mod, acc_h, mod_constant
# EXT     acc_h, acc_h, acc_h, #8
# EOR     acc_m, acc_m, acc_h
# EOR     acc_m, acc_m, t_mod
# PMULL   acc_h, acc_m, mod_constant
# EXT     acc_m, acc_m, acc_m, #8
# EOR     acc_l, acc_l, acc_h
# EOR     acc_l, acc_l, acc_m
#
# This code was then modified to merge the AES-128-GCM, AES-192-GCM, and
# AES-256-GCM implementations into a single function to reduce size. We move the
# last two round keys into consistent registers across all sizes, as they're
# treated special. Then, after rounds 0 through 8, we added some branches to
# conditionally run rounds 9-10 (AES-192 + AES-256) and 11-12 (AES-256), before
# merging back into code which finishes up the last two rounds.
#
# There is a mostly decision to be made around how much parallel work goes
# before or after the conditional part. We attempted to preserve the original
# scheduling where possible, but it's possible other schedulings are more
# optimal with the current ordering.

$flavour = shift;
$output  = shift;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

$code=<<___;
#if __ARM_MAX_ARCH__ >= 8

.arch armv8-a+crypto
.text
___

$input_ptr="x0";  #argument block
$bit_length="x1";
$output_ptr="x2";
$current_tag="x3";
$Htable="x6";
$counter="x16";
$cc="x8";

{
my ($end_input_ptr,$main_end_input_ptr,$input_l0,$input_h0)=map("x$_",(4..7));
my ($input_l1,$input_h1,$input_l2,$input_h2,$input_l3,$input_h3)=map("x$_",(19..24));
my ($output_l1,$output_h1,$output_l2,$output_h2,$output_l3,$output_h3)=map("x$_",(19..24));
my ($output_l0,$output_h0)=map("x$_",(6..7));

# rkN_l and rkN_h store the final round key, which is handled slightly
# differently because it is EORed through general-purpose registers.
my $ctr32w="w9";
my ($ctr32x,$ctr96_b64x,$ctr96_t32x,$rctr32x,$rkN_l,$rkN_h,$len)=map("x$_",(9..15));
my ($ctr96_t32w,$rctr32w)=map("w$_",(11..12));

my $rounds="x17";
my $roundsw="w17";

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

# rkNm1 stores the second-to-last round key, which is handled slightly
# differently because it uses plain AESE instead of an AESE + AESMC macro-op.
my ($rk0,$rk1,$rk2,$rk3,$rk4,$rk5,$rk6,$rk7,$rk8,$rk9,$rk10,$rk11,$rk12,$rkNm1)=map("v$_.16b",(18..31));
my ($rk0q,$rk1q,$rk2q,$rk3q,$rk4q,$rk5q,$rk6q,$rk7q,$rk8q,$rk9q,$rk10q,$rk11q,$rk12q,$rkNm1q)=map("q$_",(18..31));
my $rk2q1="v20.1q";
my $rk3q1="v21.1q";
my $rk4v="v22";
my $rk4d="d22";

################################################################################
# size_t aes_gcm_enc_kernel(const uint8_t *in,
#                           size_t len_bits,
#                           uint8_t *out,
#                           u64 *Xi,
#                           uint8_t ivec[16],
#                           const void *key,
#                           const void *Htable);
#
$code.=<<___;
.global aes_gcm_enc_kernel
.type   aes_gcm_enc_kernel,%function
.align  4
aes_gcm_enc_kernel:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29, x30, [sp, #-128]!
	mov	x29, sp
	stp     x19, x20, [sp, #16]
	mov     $counter, x4
	mov     $cc, x5
	stp     x21, x22, [sp, #32]
	stp     x23, x24, [sp, #48]
	stp     d8, d9, [sp, #64]
	stp     d10, d11, [sp, #80]
	stp     d12, d13, [sp, #96]
	stp     d14, d15, [sp, #112]
	ldr	$roundsw, [$cc, #240]
	add	$input_l1, $cc, $rounds, lsl #4                   // borrow input_l1 for last key
	ldp     $rkN_l, $rkN_h, [$input_l1]                       // load round N keys
	ldr     $rkNm1q, [$input_l1, #-16]                        // load round N-1 keys
	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   // end_input_ptr
	lsr     $main_end_input_ptr, $bit_length, #3              // byte_len
	mov     $len, $main_end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              // ctr96_b64, ctr96_t32
	ld1     { $ctr0b}, [$counter]                             // special case vector load initial counter so we can start first AES block as quickly as possible
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      // byte_len - 1
	ldr     $rk0q, [$cc, #0]                                  // load rk0
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0 // number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	ldr     $rk7q, [$cc, #112]                                // load rk7
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr
	lsr     $rctr32x, $ctr96_t32x, #32
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 2
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w
	rev     $rctr32w, $rctr32w                                // rev_ctr32
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 1
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 0
	add     $rctr32w, $rctr32w, #1                            // increment rev_ctr32
	rev     $ctr32w, $rctr32w                                 // CTR block 1
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 3
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 1
	add     $rctr32w, $rctr32w, #1                            // CTR block 1
	ldr     $rk1q, [$cc, #16]                                 // load rk1
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 1
	rev     $ctr32w, $rctr32w                                 // CTR block 2
	add     $rctr32w, $rctr32w, #1                            // CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 2
	ldr     $rk2q, [$cc, #32]                                 // load rk2
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 2
	rev     $ctr32w, $rctr32w                                 // CTR block 3
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 3
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 3
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 0
	ldr     $rk3q, [$cc, #48]                                 // load rk3
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 2
	ldr     $rk6q, [$cc, #96]                                 // load rk6
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 0
	ldr     $rk5q, [$cc, #80]                                 // load rk5
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 1
	ldr     $h3q, [$Htable, #48]                              // load h3l | h3h
	ext     $h3b, $h3b, $h3b, #8
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 0
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 1
	ldr     $rk4q, [$cc, #64]                                 // load rk4
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 2
	ldr     $h2q, [$Htable, #32]                              // load h2l | h2h
	ext     $h2b, $h2b, $h2b, #8
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 1
	ldr     $rk12q, [$cc, #192]                               // load rk12
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 2
	ldr     $h4q, [$Htable, #80]                              // load h4l | h4h
	ext     $h4b, $h4b, $h4b, #8
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 3
	ldr     $rk11q, [$cc, #176]                               // load rk11
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 2
	ldr     $rk8q, [$cc, #128]                                // load rk8
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 3
	add     $rctr32w, $rctr32w, #1                            // CTR block 3
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 3
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 3
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 4
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 4
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 4
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 4
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 5
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 5
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 5
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 5
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 6
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      // h4l | h3l
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 6
	ldr     $rk9q, [$cc, #144]                                // load rk9
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 6
	ldr     $h1q, [$Htable]                                   // load h1l | h1h
	ext     $h1b, $h1b, $h1b, #8
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 6
	ldr     $rk10q, [$cc, #160]                               // load rk10
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 7
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      // h4h | h3h
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 7
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 7
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 7
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      // h2l | h1l
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 8
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 8
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 8
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 8
	b.lt	.Lenc_finish_first_blocks                         // branch if AES-128

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 9
	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 9
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 10
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 10
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 10
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 10
	b.eq	.Lenc_finish_first_blocks                         // branch if AES-192

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 11
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 11
	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 11
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 11
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 12
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 12
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 12

.Lenc_finish_first_blocks:
	cmp     $input_ptr, $main_end_input_ptr                   // check if we have <= 4 blocks
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  // h4k | h3k
	aese    $ctr2b, $rkNm1                                    // AES block 2 - round N-1
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      // h2h | h1h
	aese    $ctr1b, $rkNm1                                    // AES block 1 - round N-1
	aese    $ctr0b, $rkNm1                                    // AES block 0 - round N-1
	aese    $ctr3b, $rkNm1                                    // AES block 3 - round N-1
	eor     $h12k.16b, $h12k.16b, $t0.16b                     // h2k | h1k
	b.ge    .Lenc_tail                                        // handle tail

	ldp     $input_l1, $input_h1, [$input_ptr, #16]           // AES block 1 - load plaintext
	rev     $ctr32w, $rctr32w                                 // CTR block 4
	ldp     $input_l0, $input_h0, [$input_ptr, #0]            // AES block 0 - load plaintext
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           // AES block 3 - load plaintext
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           // AES block 2 - load plaintext
	add     $input_ptr, $input_ptr, #64                       // AES input_ptr update
	eor     $input_l1, $input_l1, $rkN_l                      // AES block 1 - round N low
	eor     $input_h1, $input_h1, $rkN_h                      // AES block 1 - round N high
	fmov    $ctr_t1d, $input_l1                               // AES block 1 - mov low
	eor     $input_l0, $input_l0, $rkN_l                      // AES block 0 - round N low
	eor     $input_h0, $input_h0, $rkN_h                      // AES block 0 - round N high
	eor     $input_h3, $input_h3, $rkN_h                      // AES block 3 - round N high
	fmov    $ctr_t0d, $input_l0                               // AES block 0 - mov low
	cmp     $input_ptr, $main_end_input_ptr                   // check if we have <= 8 blocks
	fmov    $ctr_t0.d[1], $input_h0                           // AES block 0 - mov high
	eor     $input_l3, $input_l3, $rkN_l                      // AES block 3 - round N low
	eor     $input_l2, $input_l2, $rkN_l                      // AES block 2 - round N low
	fmov    $ctr_t1.d[1], $input_h1                           // AES block 1 - mov high
	fmov    $ctr_t2d, $input_l2                               // AES block 2 - mov low
	add     $rctr32w, $rctr32w, #1                            // CTR block 4
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4
	fmov    $ctr_t3d, $input_l3                               // AES block 3 - mov low
	eor     $input_h2, $input_h2, $rkN_h                      // AES block 2 - round N high
	fmov    $ctr_t2.d[1], $input_h2                           // AES block 2 - mov high
	eor     $res0b, $ctr_t0b, $ctr0b                          // AES block 0 - result
	fmov    $ctr0d, $ctr96_b64x                               // CTR block 4
	fmov    $ctr0.d[1], $ctr32x                               // CTR block 4
	rev     $ctr32w, $rctr32w                                 // CTR block 5
	add     $rctr32w, $rctr32w, #1                            // CTR block 5
	eor     $res1b, $ctr_t1b, $ctr1b                          // AES block 1 - result
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 5
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 5
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 5
	rev     $ctr32w, $rctr32w                                 // CTR block 6
	st1     { $res0b}, [$output_ptr], #16                     // AES block 0 - store result
	fmov    $ctr_t3.d[1], $input_h3                           // AES block 3 - mov high
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 6
	eor     $res2b, $ctr_t2b, $ctr2b                          // AES block 2 - result
	st1     { $res1b}, [$output_ptr], #16                     // AES block 1 - store result
	add     $rctr32w, $rctr32w, #1                            // CTR block 6
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 6
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 6
	st1     { $res2b}, [$output_ptr], #16                     // AES block 2 - store result
	rev     $ctr32w, $rctr32w                                 // CTR block 7
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 7
	eor     $res3b, $ctr_t3b, $ctr3b                          // AES block 3 - result
	st1     { $res3b}, [$output_ptr], #16                     // AES block 3 - store result
	b.ge    .Lenc_prepretail                                  // do prepretail

.Lenc_main_loop:                                                  // main loop start
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 0
	rev64   $res0b, $res0b                                    // GHASH block 4k (only t0 is free)
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 0
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 4k+3
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 0
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     // PRE 0
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 1
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 4k+3
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 1
	ldp     $input_l3, $input_h3, [$input_ptr, #48]           // AES block 4k+7 - load plaintext
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 1
	ldp     $input_l2, $input_h2, [$input_ptr, #32]           // AES block 4k+6 - load plaintext
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 2
	eor     $res0b, $res0b, $acc_lb                           // PRE 1
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 2
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 0
	eor     $input_l3, $input_l3, $rkN_l                      // AES block 4k+7 - round N low
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 3
	mov     $acc_md, $h34k.d[1]                               // GHASH block 4k - mid
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       // GHASH block 4k - high
	eor     $input_h2, $input_h2, $rkN_h                      // AES block 4k+6 - round N high
	mov     $t0d, $res0.d[1]                                  // GHASH block 4k - mid
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 1
	rev64   $res1b, $res1b                                    // GHASH block 4k+1 (t0 and t1 free)
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 4
	pmull   $acc_l.1q, $res0.1d, $h4.1d                       // GHASH block 4k - low
	eor     $t0.8b, $t0.8b, $res0.8b                          // GHASH block 4k - mid
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 2
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 5
	rev64   $res3b, $res3b                                    // GHASH block 4k+3 (t0, t1, t2 and t3 free)
	pmull2  $t1.1q, $res1.2d, $h3.2d                          // GHASH block 4k+1 - high
	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      // GHASH block 4k - mid
	rev64   $res2b, $res2b                                    // GHASH block 4k+2 (t0, t1, and t2 free)
	pmull   $t2.1q, $res1.1d, $h3.1d                          // GHASH block 4k+1 - low
	eor     $acc_hb, $acc_hb, $t1.16b                         // GHASH block 4k+1 - high
	mov     $t3d, $res1.d[1]                                  // GHASH block 4k+1 - mid
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 3
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 2
	eor     $acc_lb, $acc_lb, $t2.16b                         // GHASH block 4k+1 - low
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 3
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 4
	mov     $t6d, $res2.d[1]                                  // GHASH block 4k+2 - mid
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 3
	eor     $t3.8b, $t3.8b, $res1.8b                          // GHASH block 4k+1 - mid
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 4
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 6
	eor     $t6.8b, $t6.8b, $res2.8b                          // GHASH block 4k+2 - mid
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 4
	pmull   $t3.1q, $t3.1d, $h34k.1d                          // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 7
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 5
	ins     $t6.d[1], $t6.d[0]                                // GHASH block 4k+2 - mid
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 5
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 8
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 5
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 6
	eor     $acc_mb, $acc_mb, $t3.16b                         // GHASH block 4k+1 - mid
	pmull2  $t4.1q, $res2.2d, $h2.2d                          // GHASH block 4k+2 - high
	pmull   $t5.1q, $res2.1d, $h2.1d                          // GHASH block 4k+2 - low
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 7
	pmull   $t8.1q, $res3.1d, $h1.1d                          // GHASH block 4k+3 - low
	eor     $acc_hb, $acc_hb, $t4.16b                         // GHASH block 4k+2 - high
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 6
	ldp     $input_l1, $input_h1, [$input_ptr, #16]           // AES block 4k+5 - load plaintext
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 8
	mov     $t9d, $res3.d[1]                                  // GHASH block 4k+3 - mid
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 6
	eor     $acc_lb, $acc_lb, $t5.16b                         // GHASH block 4k+2 - low
	pmull2  $t6.1q, $t6.2d, $h12k.2d                          // GHASH block 4k+2 - mid
	pmull2  $t7.1q, $res3.2d, $h1.2d                          // GHASH block 4k+3 - high
	eor     $t9.8b, $t9.8b, $res3.8b                          // GHASH block 4k+3 - mid
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 7
	eor     $input_l1, $input_l1, $rkN_l                      // AES block 4k+5 - round N low
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 8
	eor     $acc_mb, $acc_mb, $t6.16b                         // GHASH block 4k+2 - mid
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 7
	eor     $input_l2, $input_l2, $rkN_l                      // AES block 4k+6 - round N low
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 8
	movi    $mod_constant.8b, #0xc2
	pmull   $t9.1q, $t9.1d, $h12k.1d                          // GHASH block 4k+3 - mid
	eor     $acc_hb, $acc_hb, $t7.16b                         // GHASH block 4k+3 - high
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	fmov    $ctr_t1d, $input_l1                               // AES block 4k+5 - mov low
	ldp     $input_l0, $input_h0, [$input_ptr, #0]            // AES block 4k+4 - load plaintext
	b.lt	.Lenc_main_loop_continue                          // branch if AES-128

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 9
	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 9
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 10
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 10
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 10
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 10
	b.eq	.Lenc_main_loop_continue                          // branch if AES-192

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 11
	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 11
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 11
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 11
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 12
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 12
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 12

.Lenc_main_loop_continue:
	shl     $mod_constantd, $mod_constantd, #56               // mod_constant
	eor     $acc_lb, $acc_lb, $t8.16b                         // GHASH block 4k+3 - low
	eor     $acc_mb, $acc_mb, $t9.16b                         // GHASH block 4k+3 - mid
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+3
	eor     $t9.16b, $acc_lb, $acc_hb                         // MODULO - karatsuba tidy up
	add     $input_ptr, $input_ptr, #64                       // AES input_ptr update
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            // MODULO - top 64b align with mid
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+8
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     // MODULO - other top alignment
	eor     $input_l0, $input_l0, $rkN_l                      // AES block 4k+4 - round N low
	eor     $acc_mb, $acc_mb, $t9.16b                         // MODULO - karatsuba tidy up
	eor     $input_h0, $input_h0, $rkN_h                      // AES block 4k+4 - round N high
	fmov    $ctr_t0d, $input_l0                               // AES block 4k+4 - mov low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+8
	eor     $mod_t.16b, $acc_hb, $mod_t.16b                   // MODULO - fold into mid
	eor     $input_h1, $input_h1, $rkN_h                      // AES block 4k+5 - round N high
	eor     $input_h3, $input_h3, $rkN_h                      // AES block 4k+7 - round N high
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+8
	aese    $ctr0b, $rkNm1                                    // AES block 4k+4 - round N-1
	fmov    $ctr_t0.d[1], $input_h0                           // AES block 4k+4 - mov high
	eor     $acc_mb, $acc_mb, $mod_t.16b                      // MODULO - fold into mid
	fmov    $ctr_t3d, $input_l3                               // AES block 4k+7 - mov low
	aese    $ctr1b, $rkNm1                                    // AES block 4k+5 - round N-1
	fmov    $ctr_t1.d[1], $input_h1                           // AES block 4k+5 - mov high
	fmov    $ctr_t2d, $input_l2                               // AES block 4k+6 - mov low
	cmp     $input_ptr, $main_end_input_ptr                   // LOOP CONTROL
	fmov    $ctr_t2.d[1], $input_h2                           // AES block 4k+6 - mov high
	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d            // MODULO - mid 64b align with low
	eor     $res0b, $ctr_t0b, $ctr0b                          // AES block 4k+4 - result
	fmov    $ctr0d, $ctr96_b64x                               // CTR block 4k+8
	fmov    $ctr0.d[1], $ctr32x                               // CTR block 4k+8
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+9
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+9
	eor     $res1b, $ctr_t1b, $ctr1b                          // AES block 4k+5 - result
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 4k+9
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+9
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 4k+9
	aese    $ctr2b, $rkNm1                                    // AES block 4k+6 - round N-1
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+10
	st1     { $res0b}, [$output_ptr], #16                     // AES block 4k+4 - store result
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+10
	eor     $acc_lb, $acc_lb, $acc_hb                         // MODULO - fold into low
	fmov    $ctr_t3.d[1], $input_h3                           // AES block 4k+7 - mov high
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     // MODULO - other mid alignment
	st1     { $res1b}, [$output_ptr], #16                     // AES block 4k+5 - store result
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+10
	aese    $ctr3b, $rkNm1                                    // AES block 4k+7 - round N-1
	eor     $res2b, $ctr_t2b, $ctr2b                          // AES block 4k+6 - result
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 4k+10
	st1     { $res2b}, [$output_ptr], #16                     // AES block 4k+6 - store result
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 4k+10
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+11
	eor     $acc_lb, $acc_lb, $acc_mb                         // MODULO - fold into low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+11
	eor     $res3b, $ctr_t3b, $ctr3b                          // AES block 4k+7 - result
	st1     { $res3b}, [$output_ptr], #16                     // AES block 4k+7 - store result
	b.lt    .Lenc_main_loop

.Lenc_prepretail:                                                 // PREPRETAIL
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 0
	rev64   $res2b, $res2b                                    // GHASH block 4k+2 (t0, t1, and t2 free)
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 0
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 4k+3
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 0
	rev64   $res0b, $res0b                                    // GHASH block 4k (only t0 is free)
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 4k+3
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     // PRE 0
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 1
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 1
	eor     $res0b, $res0b, $acc_lb                           // PRE 1
	rev64   $res1b, $res1b                                    // GHASH block 4k+1 (t0 and t1 free)
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 2
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 0
	mov     $acc_md, $h34k.d[1]                               // GHASH block 4k - mid
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 1
	pmull   $acc_l.1q, $res0.1d, $h4.1d                       // GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  // GHASH block 4k - mid
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       // GHASH block 4k - high
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 3
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 2
	eor     $t0.8b, $t0.8b, $res0.8b                          // GHASH block 4k - mid
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 2
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 1
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 3
	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      // GHASH block 4k - mid
	pmull2  $t1.1q, $res1.2d, $h3.2d                          // GHASH block 4k+1 - high
	pmull   $t2.1q, $res1.1d, $h3.1d                          // GHASH block 4k+1 - low
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 2
	eor     $acc_hb, $acc_hb, $t1.16b                         // GHASH block 4k+1 - high
	mov     $t3d, $res1.d[1]                                  // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 3
	eor     $acc_lb, $acc_lb, $t2.16b                         // GHASH block 4k+1 - low
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 3
	eor     $t3.8b, $t3.8b, $res1.8b                          // GHASH block 4k+1 - mid
	mov     $t6d, $res2.d[1]                                  // GHASH block 4k+2 - mid
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 4
	rev64   $res3b, $res3b                                    // GHASH block 4k+3 (t0, t1, t2 and t3 free)
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 4
	pmull   $t3.1q, $t3.1d, $h34k.1d                          // GHASH block 4k+1 - mid
	eor     $t6.8b, $t6.8b, $res2.8b                          // GHASH block 4k+2 - mid
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+3
	pmull   $t5.1q, $res2.1d, $h2.1d                          // GHASH block 4k+2 - low
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 5
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 4
	eor     $acc_mb, $acc_mb, $t3.16b                         // GHASH block 4k+1 - mid
	pmull2  $t4.1q, $res2.2d, $h2.2d                          // GHASH block 4k+2 - high
	eor     $acc_lb, $acc_lb, $t5.16b                         // GHASH block 4k+2 - low
	ins     $t6.d[1], $t6.d[0]                                // GHASH block 4k+2 - mid
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 5
	eor     $acc_hb, $acc_hb, $t4.16b                         // GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  // GHASH block 4k+3 - mid
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 4
	pmull2  $t6.1q, $t6.2d, $h12k.2d                          // GHASH block 4k+2 - mid
	eor     $t9.8b, $t9.8b, $res3.8b                          // GHASH block 4k+3 - mid
	pmull2  $t7.1q, $res3.2d, $h1.2d                          // GHASH block 4k+3 - high
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 5
	pmull   $t9.1q, $t9.1d, $h12k.1d                          // GHASH block 4k+3 - mid
	eor     $acc_mb, $acc_mb, $t6.16b                         // GHASH block 4k+2 - mid
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 5
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 6
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 6
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 6
	movi    $mod_constant.8b, #0xc2
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 6
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 7
	eor     $acc_hb, $acc_hb, $t7.16b                         // GHASH block 4k+3 - high
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 7
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 7
	shl     $mod_constantd, $mod_constantd, #56               // mod_constant
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 8
	eor     $acc_mb, $acc_mb, $t9.16b                         // GHASH block 4k+3 - mid
	pmull   $t8.1q, $res3.1d, $h1.1d                          // GHASH block 4k+3 - low
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 8
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 8
	eor     $acc_lb, $acc_lb, $t8.16b                         // GHASH block 4k+3 - low
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 7
	eor     $acc_mb, $acc_mb, $acc_hb                         // karatsuba tidy up
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 8
	pmull   $t1.1q, $acc_h.1d, $mod_constant.1d
	ext     $acc_hb, $acc_hb, $acc_hb, #8
	eor     $acc_mb, $acc_mb, $acc_lb
	b.lt	.Lenc_finish_prepretail                           // branch if AES-128

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 9
	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 9
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 10
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 10
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 10
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 10
	b.eq	.Lenc_finish_prepretail                           // branch if AES-192

	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 11
	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 11
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 11
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 11
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 12
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 12
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 12

.Lenc_finish_prepretail:
	eor     $acc_mb, $acc_mb, $t1.16b
	eor     $acc_mb, $acc_mb, $acc_hb
	pmull   $t1.1q, $acc_m.1d, $mod_constant.1d
	ext     $acc_mb, $acc_mb, $acc_mb, #8
	aese    $ctr1b, $rkNm1                                    // AES block 4k+5 - round N-1
	eor     $acc_lb, $acc_lb, $t1.16b
	aese    $ctr3b, $rkNm1                                    // AES block 4k+7 - round N-1
	aese    $ctr0b, $rkNm1                                    // AES block 4k+4 - round N-1
	aese    $ctr2b, $rkNm1                                    // AES block 4k+6 - round N-1
	eor     $acc_lb, $acc_lb, $acc_mb

.Lenc_tail:                                                       // TAIL
	ext     $t0.16b, $acc_lb, $acc_lb, #8                     // prepare final partial tag
	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   // main_end_input_ptr is number of bytes left to process
	ldp     $input_l0, $input_h0, [$input_ptr], #16           // AES block 4k+4 - load plaintext
	eor     $input_l0, $input_l0, $rkN_l                      // AES block 4k+4 - round N low
	eor     $input_h0, $input_h0, $rkN_h                      // AES block 4k+4 - round N high
	cmp     $main_end_input_ptr, #48
	fmov    $ctr_t0d, $input_l0                               // AES block 4k+4 - mov low
	fmov    $ctr_t0.d[1], $input_h0                           // AES block 4k+4 - mov high
	eor     $res1b, $ctr_t0b, $ctr0b                          // AES block 4k+4 - result
	b.gt    .Lenc_blocks_more_than_3
	cmp     $main_end_input_ptr, #32
	mov     $ctr3b, $ctr2b
	movi    $acc_l.8b, #0
	movi    $acc_h.8b, #0
	sub     $rctr32w, $rctr32w, #1
	mov     $ctr2b, $ctr1b
	movi    $acc_m.8b, #0
	b.gt    .Lenc_blocks_more_than_2
	mov     $ctr3b, $ctr1b
	sub     $rctr32w, $rctr32w, #1
	cmp     $main_end_input_ptr, #16
	b.gt    .Lenc_blocks_more_than_1
	sub     $rctr32w, $rctr32w, #1
	b       .Lenc_blocks_less_than_1
.Lenc_blocks_more_than_3:                                        // blocks left >  3
	st1     { $res1b}, [$output_ptr], #16                    // AES final-3 block  - store result
	ldp     $input_l0, $input_h0, [$input_ptr], #16          // AES final-2 block - load input low & high
	rev64   $res0b, $res1b                                   // GHASH final-3 block
	eor     $input_l0, $input_l0, $rkN_l                     // AES final-2 block - round N low
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	eor     $input_h0, $input_h0, $rkN_h                     // AES final-2 block - round N high
	mov     $rk4d, $res0.d[1]                                // GHASH final-3 block - mid
	fmov    $res1d, $input_l0                                // AES final-2 block - mov low
	fmov    $res1.d[1], $input_h0                            // AES final-2 block - mov high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-3 block - mid
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	mov     $acc_md, $h34k.d[1]                              // GHASH final-3 block - mid
	pmull   $acc_l.1q, $res0.1d, $h4.1d                      // GHASH final-3 block - low
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      // GHASH final-3 block - high
	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                   // GHASH final-3 block - mid
	eor     $res1b, $res1b, $ctr1b                           // AES final-2 block - result
.Lenc_blocks_more_than_2:                                        // blocks left >  2
	st1     { $res1b}, [$output_ptr], #16                    // AES final-2 block - store result
	ldp     $input_l0, $input_h0, [$input_ptr], #16          // AES final-1 block - load input low & high
	rev64   $res0b, $res1b                                   // GHASH final-2 block
	eor     $input_l0, $input_l0, $rkN_l                     // AES final-1 block - round N low
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	fmov    $res1d, $input_l0                                // AES final-1 block - mov low
	eor     $input_h0, $input_h0, $rkN_h                     // AES final-1 block - round N high
	fmov    $res1.d[1], $input_h0                            // AES final-1 block - mov high
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	pmull2  $rk2q1, $res0.2d, $h3.2d                         // GHASH final-2 block - high
	mov     $rk4d, $res0.d[1]                                // GHASH final-2 block - mid
	pmull   $rk3q1, $res0.1d, $h3.1d                         // GHASH final-2 block - low
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-2 block - mid
	eor     $res1b, $res1b, $ctr2b                           // AES final-1 block - result
	eor     $acc_hb, $acc_hb, $rk2                           // GHASH final-2 block - high
	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                     // GHASH final-2 block - mid
	eor     $acc_lb, $acc_lb, $rk3                           // GHASH final-2 block - low
	eor     $acc_mb, $acc_mb, $rk4v.16b                      // GHASH final-2 block - mid
.Lenc_blocks_more_than_1:                                        // blocks left >  1
	st1     { $res1b}, [$output_ptr], #16                    // AES final-1 block - store result
	rev64   $res0b, $res1b                                   // GHASH final-1 block
	ldp     $input_l0, $input_h0, [$input_ptr], #16          // AES final block - load input low & high
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	eor     $input_l0, $input_l0, $rkN_l                     // AES final block - round N low
	mov     $rk4d, $res0.d[1]                                // GHASH final-1 block - mid
	pmull2  $rk2q1, $res0.2d, $h2.2d                         // GHASH final-1 block - high
	eor     $input_h0, $input_h0, $rkN_h                     // AES final block - round N high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-1 block - mid
	eor     $acc_hb, $acc_hb, $rk2                           // GHASH final-1 block - high
	ins     $rk4v.d[1], $rk4v.d[0]                           // GHASH final-1 block - mid
	fmov    $res1d, $input_l0                                // AES final block - mov low
	fmov    $res1.d[1], $input_h0                            // AES final block - mov high
	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                     // GHASH final-1 block - mid
	pmull   $rk3q1, $res0.1d, $h2.1d                         // GHASH final-1 block - low
	eor     $res1b, $res1b, $ctr3b                           // AES final block - result
	eor     $acc_mb, $acc_mb, $rk4v.16b                      // GHASH final-1 block - mid
	eor     $acc_lb, $acc_lb, $rk3                           // GHASH final-1 block - low
.Lenc_blocks_less_than_1:                                        // blocks left <= 1
	and     $bit_length, $bit_length, #127                   // bit_length %= 128
	mvn     $rkN_l, xzr                                      // rkN_l = 0xffffffffffffffff
	sub     $bit_length, $bit_length, #128                   // bit_length -= 128
	neg     $bit_length, $bit_length                         // bit_length = 128 - #bits in input (in range [1,128])
	ld1     { $rk0}, [$output_ptr]                           // load existing bytes where the possibly partial last block is to be stored
	mvn     $rkN_h, xzr                                      // rkN_h = 0xffffffffffffffff
	and     $bit_length, $bit_length, #127                   // bit_length %= 128
	lsr     $rkN_h, $rkN_h, $bit_length                      // rkN_h is mask for top 64b of last block
	cmp     $bit_length, #64
	csel    $input_l0, $rkN_l, $rkN_h, lt
	csel    $input_h0, $rkN_h, xzr, lt
	fmov    $ctr0d, $input_l0                                // ctr0b is mask for last block
	fmov    $ctr0.d[1], $input_h0
	and     $res1b, $res1b, $ctr0b                           // possibly partial last block has zeroes in highest bits
	rev64   $res0b, $res1b                                   // GHASH final block
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	bif     $res1b, $rk0, $ctr0b                             // insert existing bytes in top end of result before storing
	pmull2  $rk2q1, $res0.2d, $h1.2d                         // GHASH final block - high
	mov     $t0d, $res0.d[1]                                 // GHASH final block - mid
	rev     $ctr32w, $rctr32w
	pmull   $rk3q1, $res0.1d, $h1.1d                         // GHASH final block - low
	eor     $acc_hb, $acc_hb, $rk2                           // GHASH final block - high
	eor     $t0.8b, $t0.8b, $res0.8b                         // GHASH final block - mid
	pmull   $t0.1q, $t0.1d, $h12k.1d                         // GHASH final block - mid
	eor     $acc_lb, $acc_lb, $rk3                           // GHASH final block - low
	eor     $acc_mb, $acc_mb, $t0.16b                        // GHASH final block - mid
	movi    $mod_constant.8b, #0xc2
	eor     $t9.16b, $acc_lb, $acc_hb                        // MODULO - karatsuba tidy up
	shl     $mod_constantd, $mod_constantd, #56              // mod_constant
	eor     $acc_mb, $acc_mb, $t9.16b                        // MODULO - karatsuba tidy up
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d           // MODULO - top 64b align with mid
	ext     $acc_hb, $acc_hb, $acc_hb, #8                    // MODULO - other top alignment
	eor     $acc_mb, $acc_mb, $mod_t.16b                     // MODULO - fold into mid
	eor     $acc_mb, $acc_mb, $acc_hb                        // MODULO - fold into mid
	pmull   $acc_h.1q, $acc_m.1d, $mod_constant.1d           // MODULO - mid 64b align with low
	ext     $acc_mb, $acc_mb, $acc_mb, #8                    // MODULO - other mid alignment
	str     $ctr32w, [$counter, #12]                         // store the updated counter
	st1     { $res1b}, [$output_ptr]                         // store all 16B
	eor     $acc_lb, $acc_lb, $acc_hb                        // MODULO - fold into low
	eor     $acc_lb, $acc_lb, $acc_mb                        // MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]
	ldp     x19, x20, [sp, #16]
	ldp     x21, x22, [sp, #32]
	ldp     x23, x24, [sp, #48]
	ldp     d8, d9, [sp, #64]
	ldp     d10, d11, [sp, #80]
	ldp     d12, d13, [sp, #96]
	ldp     d14, d15, [sp, #112]
	ldp     x29, x30, [sp], #128
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size aes_gcm_enc_kernel,.-aes_gcm_enc_kernel
___

{
my $t8="v4";
my $t8d="d4";
my $t9="v6";
my $t9d="d6";
################################################################################
# size_t aes_gcm_dec_kernel(const uint8_t *in,
#                           size_t len_bits,
#                           uint8_t *out,
#                           u64 *Xi,
#                           uint8_t ivec[16],
#                           const void *key);
#
$code.=<<___;
.global aes_gcm_dec_kernel
.type   aes_gcm_dec_kernel,%function
.align  4
aes_gcm_dec_kernel:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29, x30, [sp, #-128]!
	mov	x29, sp
	stp     x19, x20, [sp, #16]
	mov     $counter, x4
	mov     $cc, x5
	stp     x21, x22, [sp, #32]
	stp     x23, x24, [sp, #48]
	stp     d8, d9, [sp, #64]
	stp     d10, d11, [sp, #80]
	stp     d12, d13, [sp, #96]
	stp     d14, d15, [sp, #112]
	ldr	$roundsw, [$cc, #240]
	add	$input_l1, $cc, $rounds, lsl #4                   // borrow input_l1 for last key
	ldp     $rkN_l, $rkN_h, [$input_l1]                       // load round N keys
	ldr     $rkNm1q, [$input_l1, #-16]                        // load round N-1 keys
	lsr     $main_end_input_ptr, $bit_length, #3              // byte_len
	mov     $len, $main_end_input_ptr
	ldp     $ctr96_b64x, $ctr96_t32x, [$counter]              // ctr96_b64, ctr96_t32
	ldr     $rk8q, [$cc, #128]                                // load rk8
	sub     $main_end_input_ptr, $main_end_input_ptr, #1      // byte_len - 1
	ldr     $rk7q, [$cc, #112]                                // load rk7
	and     $main_end_input_ptr, $main_end_input_ptr, #0xffffffffffffffc0 // number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	add     $end_input_ptr, $input_ptr, $bit_length, lsr #3   // end_input_ptr
	ldr     $rk6q, [$cc, #96]                                 // load rk6
	lsr     $rctr32x, $ctr96_t32x, #32
	ldr     $rk5q, [$cc, #80]                                 // load rk5
	orr     $ctr96_t32w, $ctr96_t32w, $ctr96_t32w
	ldr     $rk3q, [$cc, #48]                                 // load rk3
	add     $main_end_input_ptr, $main_end_input_ptr, $input_ptr
	rev     $rctr32w, $rctr32w                                // rev_ctr32
	add     $rctr32w, $rctr32w, #1                            // increment rev_ctr32
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 3
	rev     $ctr32w, $rctr32w                                 // CTR block 1
	add     $rctr32w, $rctr32w, #1                            // CTR block 1
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 1
	ld1     { $ctr0b}, [$counter]                             // special case vector load initial counter so we can start first AES block as quickly as possible
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 1
	rev     $ctr32w, $rctr32w                                 // CTR block 2
	add     $rctr32w, $rctr32w, #1                            // CTR block 2
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 2
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 2
	rev     $ctr32w, $rctr32w                                 // CTR block 3
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 3
	ldr     $rk0q, [$cc, #0]                                  // load rk0
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 3
	add     $rctr32w, $rctr32w, #1                            // CTR block 3
	ldr     $rk4q, [$cc, #64]                                 // load rk4
	ldr     $rk1q, [$cc, #16]                                 // load rk1
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 0
	ldr     $h3q, [$Htable, #48]                              // load h3l | h3h
	ext     $h3b, $h3b, $h3b, #8
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 0
	ldr     $h4q, [$Htable, #80]                              // load h4l | h4h
	ext     $h4b, $h4b, $h4b, #8
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 0
	ldr     $h2q, [$Htable, #32]                              // load h2l | h2h
	ext     $h2b, $h2b, $h2b, #8
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 0
	ldr     $rk2q, [$cc, #32]                                 // load rk2
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 1
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 1
	ld1     { $acc_lb}, [$current_tag]
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 1
	ldr     $rk9q, [$cc, #144]                                // load rk9
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 1
	ldr     $rk12q, [$cc, #192]                               // load rk12
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 2
	ldr     $h1q, [$Htable]                                   // load h1l | h1h
	ext     $h1b, $h1b, $h1b, #8
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 2
	ldr     $rk10q, [$cc, #160]                               // load rk10
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 2
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 3
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 2
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 3
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 4
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 3
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 3
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 4
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 4
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 4
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 5
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 5
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 5
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 5
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 6
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 6
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 6
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 6
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 7
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 7
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 7
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 8
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 7
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 8
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 8
	ldr     $rk11q, [$cc, #176]                               // load rk11
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 8
	b.lt	.Ldec_finish_first_blocks                         // branch if AES-128

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 9
	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 9
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 10
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 10
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 10
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 10
	b.eq	.Ldec_finish_first_blocks                         // branch if AES-192

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 11
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 11
	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 11
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 11
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 1 - round 12
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 0 - round 12
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 2 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 3 - round 12

.Ldec_finish_first_blocks:
	cmp     $input_ptr, $main_end_input_ptr                   // check if we have <= 4 blocks
	trn1    $acc_h.2d, $h3.2d,    $h4.2d                      // h4h | h3h
	trn2    $h34k.2d,  $h3.2d,    $h4.2d                      // h4l | h3l
	trn1    $t0.2d,    $h1.2d,    $h2.2d                      // h2h | h1h
	trn2    $h12k.2d,  $h1.2d,    $h2.2d                      // h2l | h1l
	eor     $h34k.16b, $h34k.16b, $acc_h.16b                  // h4k | h3k
	aese    $ctr1b, $rkNm1                                    // AES block 1 - round N-1
	aese    $ctr2b, $rkNm1                                    // AES block 2 - round N-1
	eor     $h12k.16b, $h12k.16b, $t0.16b                     // h2k | h1k
	aese    $ctr3b, $rkNm1                                    // AES block 3 - round N-1
	aese    $ctr0b, $rkNm1                                    // AES block 0 - round N-1
	b.ge    .Ldec_tail                                        // handle tail

	ldr     $res0q, [$input_ptr, #0]                          // AES block 0 - load ciphertext
	ldr     $res1q, [$input_ptr, #16]                         // AES block 1 - load ciphertext
	rev     $ctr32w, $rctr32w                                 // CTR block 4
	eor     $ctr0b, $res0b, $ctr0b                            // AES block 0 - result
	eor     $ctr1b, $res1b, $ctr1b                            // AES block 1 - result
	rev64   $res1b, $res1b                                    // GHASH block 1
	ldr     $res3q, [$input_ptr, #48]                         // AES block 3 - load ciphertext
	mov     $output_h0, $ctr0.d[1]                            // AES block 0 - mov high
	mov     $output_l0, $ctr0.d[0]                            // AES block 0 - mov low
	rev64   $res0b, $res0b                                    // GHASH block 0
	add     $rctr32w, $rctr32w, #1                            // CTR block 4
	fmov    $ctr0d, $ctr96_b64x                               // CTR block 4
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4
	fmov    $ctr0.d[1], $ctr32x                               // CTR block 4
	rev     $ctr32w, $rctr32w                                 // CTR block 5
	add     $rctr32w, $rctr32w, #1                            // CTR block 5
	mov     $output_l1, $ctr1.d[0]                            // AES block 1 - mov low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 5
	mov     $output_h1, $ctr1.d[1]                            // AES block 1 - mov high
	eor     $output_h0, $output_h0, $rkN_h                    // AES block 0 - round N high
	eor     $output_l0, $output_l0, $rkN_l                    // AES block 0 - round N low
	stp     $output_l0, $output_h0, [$output_ptr], #16        // AES block 0 - store result
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 5
	ldr     $res2q, [$input_ptr, #32]                         // AES block 2 - load ciphertext
	add     $input_ptr, $input_ptr, #64                       // AES input_ptr update
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 5
	rev     $ctr32w, $rctr32w                                 // CTR block 6
	add     $rctr32w, $rctr32w, #1                            // CTR block 6
	eor     $output_l1, $output_l1, $rkN_l                    // AES block 1 - round N low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 6
	eor     $output_h1, $output_h1, $rkN_h                    // AES block 1 - round N high
	stp     $output_l1, $output_h1, [$output_ptr], #16        // AES block 1 - store result
	eor     $ctr2b, $res2b, $ctr2b                            // AES block 2 - result
	cmp     $input_ptr, $main_end_input_ptr                   // check if we have <= 8 blocks
	b.ge    .Ldec_prepretail                                  // do prepretail

.Ldec_main_loop:                                                  // main loop start
	mov     $output_l2, $ctr2.d[0]                            // AES block 4k+2 - mov low
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     // PRE 0
	eor     $ctr3b, $res3b, $ctr3b                            // AES block 4k+3 - result
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 0
	mov     $output_h2, $ctr2.d[1]                            // AES block 4k+2 - mov high
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 0
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 4k+6
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 4k+6
	eor     $res0b, $res0b, $acc_lb                           // PRE 1
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+7
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 1
	mov     $output_h3, $ctr3.d[1]                            // AES block 4k+3 - mov high
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 1
	mov     $output_l3, $ctr3.d[0]                            // AES block 4k+3 - mov low
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       // GHASH block 4k - high
	mov     $t0d, $res0.d[1]                                  // GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 4k+7
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+7
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 0
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 4k+7
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 2
	eor     $t0.8b, $t0.8b, $res0.8b                          // GHASH block 4k - mid
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 3
	eor     $output_h2, $output_h2, $rkN_h                    // AES block 4k+2 - round N high
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 1
	mov     $acc_md, $h34k.d[1]                               // GHASH block 4k - mid
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 3
	rev64   $res2b, $res2b                                    // GHASH block 4k+2
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 0
	eor     $output_l2, $output_l2, $rkN_l                    // AES block 4k+2 - round N low
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 2
	stp     $output_l2, $output_h2, [$output_ptr], #16        // AES block 4k+2 - store result
	pmull   $acc_l.1q, $res0.1d, $h4.1d                       // GHASH block 4k - low
	pmull2  $t1.1q, $res1.2d, $h3.2d                          // GHASH block 4k+1 - high
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 3
	rev64   $res3b, $res3b                                    // GHASH block 4k+3
	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      // GHASH block 4k - mid
	eor     $output_l3, $output_l3, $rkN_l                    // AES block 4k+3 - round N low
	pmull   $t2.1q, $res1.1d, $h3.1d                          // GHASH block 4k+1 - low
	eor     $output_h3, $output_h3, $rkN_h                    // AES block 4k+3 - round N high
	eor     $acc_hb, $acc_hb, $t1.16b                         // GHASH block 4k+1 - high
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 4
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 1
	mov     $t3d, $res1.d[1]                                  // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 4
	eor     $acc_lb, $acc_lb, $t2.16b                         // GHASH block 4k+1 - low
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 5
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+7
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 2
	mov     $t6d, $res2.d[1]                                  // GHASH block 4k+2 - mid
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 4
	eor     $t3.8b, $t3.8b, $res1.8b                          // GHASH block 4k+1 - mid
	pmull   $t5.1q, $res2.1d, $h2.1d                          // GHASH block 4k+2 - low
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 3
	eor     $t6.8b, $t6.8b, $res2.8b                          // GHASH block 4k+2 - mid
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 5
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 5
	eor     $acc_lb, $acc_lb, $t5.16b                         // GHASH block 4k+2 - low
	pmull   $t3.1q, $t3.1d, $h34k.1d                          // GHASH block 4k+1 - mid
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+8
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 6
	ins     $t6.d[1], $t6.d[0]                                // GHASH block 4k+2 - mid
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 6
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+8
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 4
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 7
	eor     $acc_mb, $acc_mb, $t3.16b                         // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 7
	pmull2  $t4.1q, $res2.2d, $h2.2d                          // GHASH block 4k+2 - high
	mov     $t9d, $res3.d[1]                                  // GHASH block 4k+3 - mid
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 5
	pmull2  $t6.1q, $t6.2d, $h12k.2d                          // GHASH block 4k+2 - mid
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 8
	eor     $acc_hb, $acc_hb, $t4.16b                         // GHASH block 4k+2 - high
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 6
	pmull   $t8.1q, $res3.1d, $h1.1d                          // GHASH block 4k+3 - low
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+8
	eor     $acc_mb, $acc_mb, $t6.16b                         // GHASH block 4k+2 - mid
	pmull2  $t7.1q, $res3.2d, $h1.2d                          // GHASH block 4k+3 - high
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	eor     $t9.8b, $t9.8b, $res3.8b                          // GHASH block 4k+3 - mid
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 8
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 6
	eor     $acc_hb, $acc_hb, $t7.16b                         // GHASH block 4k+3 - high
	pmull   $t9.1q, $t9.1d, $h12k.1d                          // GHASH block 4k+3 - mid
	movi    $mod_constant.8b, #0xc2
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 7
	eor     $acc_lb, $acc_lb, $t8.16b                         // GHASH block 4k+3 - low
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 7
	shl     $mod_constantd, $mod_constantd, #56               // mod_constant
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 8
	eor     $acc_mb, $acc_mb, $t9.16b                         // GHASH block 4k+3 - mid
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 8
	b.lt	.Ldec_main_loop_continue                          // branch if AES-128

	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 9
	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 9
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 10
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 10
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 10
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 10
	b.eq	.Ldec_main_loop_continue                          // branch if AES-192

	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 11
	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 11
	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 11
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 11
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 12
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 12
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 12

.Ldec_main_loop_continue:
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            // MODULO - top 64b align with mid
	eor     $t9.16b, $acc_lb, $acc_hb                         // MODULO - karatsuba tidy up
	ldr     $res0q, [$input_ptr, #0]                          // AES block 4k+4 - load ciphertext
	aese    $ctr0b, $rkNm1                                    // AES block 4k+4 - round N-1
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     // MODULO - other top alignment
	eor     $acc_mb, $acc_mb, $t9.16b                         // MODULO - karatsuba tidy up
	ldr     $res1q, [$input_ptr, #16]                         // AES block 4k+5 - load ciphertext
	eor     $ctr0b, $res0b, $ctr0b                            // AES block 4k+4 - result
	stp     $output_l3, $output_h3, [$output_ptr], #16        // AES block 4k+3 - store result
	eor     $acc_mb, $acc_mb, $mod_t.16b                      // MODULO - fold into mid
	ldr     $res3q, [$input_ptr, #48]                         // AES block 4k+7 - load ciphertext
	ldr     $res2q, [$input_ptr, #32]                         // AES block 4k+6 - load ciphertext
	mov     $output_h0, $ctr0.d[1]                            // AES block 4k+4 - mov high
	eor     $acc_mb, $acc_mb, $acc_hb                         // MODULO - fold into mid
	aese    $ctr1b, $rkNm1                                    // AES block 4k+5 - round N-1
	add     $input_ptr, $input_ptr, #64                       // AES input_ptr update
	mov     $output_l0, $ctr0.d[0]                            // AES block 4k+4 - mov low
	fmov    $ctr0d, $ctr96_b64x                               // CTR block 4k+8
	fmov    $ctr0.d[1], $ctr32x                               // CTR block 4k+8
	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     // MODULO - mid 64b align with low
	eor     $ctr1b, $res1b, $ctr1b                            // AES block 4k+5 - result
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+9
	aese    $ctr2b, $rkNm1                                    // AES block 4k+6 - round N-1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+9
	cmp     $input_ptr, $main_end_input_ptr                   // LOOP CONTROL
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+9
	eor     $output_l0, $output_l0, $rkN_l                    // AES block 4k+4 - round N low
	eor     $output_h0, $output_h0, $rkN_h                    // AES block 4k+4 - round N high
	mov     $output_h1, $ctr1.d[1]                            // AES block 4k+5 - mov high
	eor     $ctr2b, $res2b, $ctr2b                            // AES block 4k+6 - result
	eor     $acc_lb, $acc_lb, $mod_constant.16b               // MODULO - fold into low
	mov     $output_l1, $ctr1.d[0]                            // AES block 4k+5 - mov low
	fmov    $ctr1d, $ctr96_b64x                               // CTR block 4k+9
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     // MODULO - other mid alignment
	fmov    $ctr1.d[1], $ctr32x                               // CTR block 4k+9
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+10
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+10
	aese    $ctr3b, $rkNm1                                    // AES block 4k+7 - round N-1
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+10
	rev64   $res1b, $res1b                                    // GHASH block 4k+5
	eor     $output_h1, $output_h1, $rkN_h                    // AES block 4k+5 - round N high
	stp     $output_l0, $output_h0, [$output_ptr], #16        // AES block 4k+4 - store result
	eor     $output_l1, $output_l1, $rkN_l                    // AES block 4k+5 - round N low
	stp     $output_l1, $output_h1, [$output_ptr], #16        // AES block 4k+5 - store result
	rev64   $res0b, $res0b                                    // GHASH block 4k+4
	eor     $acc_lb, $acc_lb, $acc_mb                         // MODULO - fold into low
	b.lt    .Ldec_main_loop

.Ldec_prepretail:                                                 // PREPRETAIL
	ext     $acc_lb, $acc_lb, $acc_lb, #8                     // PRE 0
	mov     $output_l2, $ctr2.d[0]                            // AES block 4k+2 - mov low
	eor     $ctr3b, $res3b, $ctr3b                            // AES block 4k+3 - result
	aese    $ctr0b, $rk0  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 0
	mov     $output_h2, $ctr2.d[1]                            // AES block 4k+2 - mov high
	aese    $ctr1b, $rk0  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 0
	fmov    $ctr2d, $ctr96_b64x                               // CTR block 4k+6
	fmov    $ctr2.d[1], $ctr32x                               // CTR block 4k+6
	rev     $ctr32w, $rctr32w                                 // CTR block 4k+7
	eor     $res0b, $res0b, $acc_lb                           // PRE 1
	rev64   $res2b, $res2b                                    // GHASH block 4k+2
	orr     $ctr32x, $ctr96_t32x, $ctr32x, lsl #32            // CTR block 4k+7
	mov     $output_l3, $ctr3.d[0]                            // AES block 4k+3 - mov low
	aese    $ctr1b, $rk1  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 1
	mov     $output_h3, $ctr3.d[1]                            // AES block 4k+3 - mov high
	pmull   $acc_l.1q, $res0.1d, $h4.1d                       // GHASH block 4k - low
	mov     $t0d, $res0.d[1]                                  // GHASH block 4k - mid
	fmov    $ctr3d, $ctr96_b64x                               // CTR block 4k+7
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                       // GHASH block 4k - high
	fmov    $ctr3.d[1], $ctr32x                               // CTR block 4k+7
	aese    $ctr2b, $rk0  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 0
	mov     $acc_md, $h34k.d[1]                               // GHASH block 4k - mid
	aese    $ctr0b, $rk1  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 1
	eor     $t0.8b, $t0.8b, $res0.8b                          // GHASH block 4k - mid
	pmull2  $t1.1q, $res1.2d, $h3.2d                          // GHASH block 4k+1 - high
	aese    $ctr2b, $rk1  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 1
	rev64   $res3b, $res3b                                    // GHASH block 4k+3
	aese    $ctr3b, $rk0  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 0
	pmull   $acc_m.1q, $t0.1d, $acc_m.1d                      // GHASH block 4k - mid
	eor     $acc_hb, $acc_hb, $t1.16b                         // GHASH block 4k+1 - high
	pmull   $t2.1q, $res1.1d, $h3.1d                          // GHASH block 4k+1 - low
	aese    $ctr3b, $rk1  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 1
	mov     $t3d, $res1.d[1]                                  // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk2  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 2
	aese    $ctr1b, $rk2  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 2
	eor     $acc_lb, $acc_lb, $t2.16b                         // GHASH block 4k+1 - low
	aese    $ctr2b, $rk2  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 2
	aese    $ctr0b, $rk3  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 3
	mov     $t6d, $res2.d[1]                                  // GHASH block 4k+2 - mid
	aese    $ctr3b, $rk2  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 2
	eor     $t3.8b, $t3.8b, $res1.8b                          // GHASH block 4k+1 - mid
	pmull   $t5.1q, $res2.1d, $h2.1d                          // GHASH block 4k+2 - low
	aese    $ctr0b, $rk4  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 4
	aese    $ctr3b, $rk3  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 3
	eor     $t6.8b, $t6.8b, $res2.8b                          // GHASH block 4k+2 - mid
	pmull   $t3.1q, $t3.1d, $h34k.1d                          // GHASH block 4k+1 - mid
	aese    $ctr0b, $rk5  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 5
	eor     $acc_lb, $acc_lb, $t5.16b                         // GHASH block 4k+2 - low
	aese    $ctr3b, $rk4  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 4
	pmull2  $t7.1q, $res3.2d, $h1.2d                          // GHASH block 4k+3 - high
	eor     $acc_mb, $acc_mb, $t3.16b                         // GHASH block 4k+1 - mid
	pmull2  $t4.1q, $res2.2d, $h2.2d                          // GHASH block 4k+2 - high
	aese    $ctr3b, $rk5  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 5
	ins     $t6.d[1], $t6.d[0]                                // GHASH block 4k+2 - mid
	aese    $ctr2b, $rk3  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 3
	aese    $ctr1b, $rk3  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 3
	eor     $acc_hb, $acc_hb, $t4.16b                         // GHASH block 4k+2 - high
	pmull   $t8.1q, $res3.1d, $h1.1d                          // GHASH block 4k+3 - low
	aese    $ctr2b, $rk4  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 4
	mov     $t9d, $res3.d[1]                                  // GHASH block 4k+3 - mid
	aese    $ctr1b, $rk4  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 4
	pmull2  $t6.1q, $t6.2d, $h12k.2d                          // GHASH block 4k+2 - mid
	aese    $ctr2b, $rk5  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 5
	eor     $t9.8b, $t9.8b, $res3.8b                          // GHASH block 4k+3 - mid
	aese    $ctr1b, $rk5  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 5
	aese    $ctr3b, $rk6  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 6
	eor     $acc_mb, $acc_mb, $t6.16b                         // GHASH block 4k+2 - mid
	aese    $ctr2b, $rk6  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 6
	aese    $ctr0b, $rk6  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 6
	movi    $mod_constant.8b, #0xc2
	aese    $ctr1b, $rk6  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 6
	eor     $acc_lb, $acc_lb, $t8.16b                         // GHASH block 4k+3 - low
	pmull   $t9.1q, $t9.1d, $h12k.1d                          // GHASH block 4k+3 - mid
	aese    $ctr3b, $rk7  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 7
	cmp     $rounds, #12                                      // setup flags for AES-128/192/256 check
	eor     $acc_hb, $acc_hb, $t7.16b                         // GHASH block 4k+3 - high
	aese    $ctr1b, $rk7  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 7
	aese    $ctr0b, $rk7  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 7
	eor     $acc_mb, $acc_mb, $t9.16b                         // GHASH block 4k+3 - mid
	aese    $ctr3b, $rk8  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 8
	aese    $ctr2b, $rk7  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 7
	eor     $t9.16b, $acc_lb, $acc_hb                         // MODULO - karatsuba tidy up
	aese    $ctr1b, $rk8  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 8
	aese    $ctr0b, $rk8  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 8
	shl     $mod_constantd, $mod_constantd, #56               // mod_constant
	aese    $ctr2b, $rk8  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 8
	b.lt	.Ldec_finish_prepretail                           // branch if AES-128

	aese    $ctr1b, $rk9  \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 9
	aese    $ctr2b, $rk9  \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 9
	aese    $ctr3b, $rk9  \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 9
	aese    $ctr0b, $rk9  \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 9
	aese    $ctr2b, $rk10 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 10
	aese    $ctr3b, $rk10 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 10
	aese    $ctr0b, $rk10 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 10
	aese    $ctr1b, $rk10 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 10
	b.eq	.Ldec_finish_prepretail                           // branch if AES-192

	aese    $ctr2b, $rk11 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 11
	aese    $ctr0b, $rk11 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 11
	aese    $ctr1b, $rk11 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 11
	aese    $ctr2b, $rk12 \n  aesmc   $ctr2b, $ctr2b          // AES block 4k+6 - round 12
	aese    $ctr3b, $rk11 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 11
	aese    $ctr1b, $rk12 \n  aesmc   $ctr1b, $ctr1b          // AES block 4k+5 - round 12
	aese    $ctr0b, $rk12 \n  aesmc   $ctr0b, $ctr0b          // AES block 4k+4 - round 12
	aese    $ctr3b, $rk12 \n  aesmc   $ctr3b, $ctr3b          // AES block 4k+7 - round 12

.Ldec_finish_prepretail:
	eor     $acc_mb, $acc_mb, $t9.16b                         // MODULO - karatsuba tidy up
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            // MODULO - top 64b align with mid
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     // MODULO - other top alignment
	eor     $acc_mb, $acc_mb, $mod_t.16b                      // MODULO - fold into mid
	eor     $output_h2, $output_h2, $rkN_h                    // AES block 4k+2 - round N high
	eor     $output_l3, $output_l3, $rkN_l                    // AES block 4k+3 - round N low
	eor     $acc_mb, $acc_mb, $acc_hb                         // MODULO - fold into mid
	add     $rctr32w, $rctr32w, #1                            // CTR block 4k+7
	eor     $output_l2, $output_l2, $rkN_l                    // AES block 4k+2 - round N low
	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     // MODULO - mid 64b align with low
	eor     $output_h3, $output_h3, $rkN_h                    // AES block 4k+3 - round N high
	stp     $output_l2, $output_h2, [$output_ptr], #16        // AES block 4k+2 - store result
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     // MODULO - other mid alignment
	stp     $output_l3, $output_h3, [$output_ptr], #16        // AES block 4k+3 - store result

	eor     $acc_lb, $acc_lb, $mod_constant.16b               // MODULO - fold into low
	aese    $ctr1b, $rkNm1                                    // AES block 4k+5 - round N-1
	aese    $ctr0b, $rkNm1                                    // AES block 4k+4 - round N-1
	aese    $ctr3b, $rkNm1                                    // AES block 4k+7 - round N-1
	aese    $ctr2b, $rkNm1                                    // AES block 4k+6 - round N-1
	eor     $acc_lb, $acc_lb, $acc_mb                         // MODULO - fold into low

.Ldec_tail:                                                       // TAIL
	sub     $main_end_input_ptr, $end_input_ptr, $input_ptr   // main_end_input_ptr is number of bytes left to process
	ld1     { $res1b}, [$input_ptr], #16                      // AES block 4k+4 - load ciphertext
	eor     $ctr0b, $res1b, $ctr0b                            // AES block 4k+4 - result
	mov     $output_l0, $ctr0.d[0]                            // AES block 4k+4 - mov low
	mov     $output_h0, $ctr0.d[1]                            // AES block 4k+4 - mov high
	ext     $t0.16b, $acc_lb, $acc_lb, #8                     // prepare final partial tag
	cmp     $main_end_input_ptr, #48
	eor     $output_l0, $output_l0, $rkN_l                    // AES block 4k+4 - round N low
	eor     $output_h0, $output_h0, $rkN_h                    // AES block 4k+4 - round N high
	b.gt    .Ldec_blocks_more_than_3
	sub     $rctr32w, $rctr32w, #1
	mov     $ctr3b, $ctr2b
	movi    $acc_m.8b, #0
	movi    $acc_l.8b, #0
	cmp     $main_end_input_ptr, #32
	movi    $acc_h.8b, #0
	mov     $ctr2b, $ctr1b
	b.gt    .Ldec_blocks_more_than_2
	sub     $rctr32w, $rctr32w, #1
	mov     $ctr3b, $ctr1b
	cmp     $main_end_input_ptr, #16
	b.gt    .Ldec_blocks_more_than_1
	sub     $rctr32w, $rctr32w, #1
	b       .Ldec_blocks_less_than_1
.Ldec_blocks_more_than_3:                                    // blocks left >  3
	rev64   $res0b, $res1b                                   // GHASH final-3 block
	ld1     { $res1b}, [$input_ptr], #16                     // AES final-2 block - load ciphertext
	stp     $output_l0, $output_h0, [$output_ptr], #16       // AES final-3 block  - store result
	mov     $acc_md, $h34k.d[1]                              // GHASH final-3 block - mid
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	eor     $ctr0b, $res1b, $ctr1b                           // AES final-2 block - result
	mov     $rk4d, $res0.d[1]                                // GHASH final-3 block - mid
	mov     $output_l0, $ctr0.d[0]                           // AES final-2 block - mov low
	mov     $output_h0, $ctr0.d[1]                           // AES final-2 block - mov high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-3 block - mid
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	pmull2  $acc_h.1q, $res0.2d, $h4.2d                      // GHASH final-3 block - high
	pmull   $acc_m.1q, $rk4v.1d, $acc_m.1d                   // GHASH final-3 block - mid
	eor     $output_l0, $output_l0, $rkN_l                   // AES final-2 block - round N low
	pmull   $acc_l.1q, $res0.1d, $h4.1d                      // GHASH final-3 block - low
	eor     $output_h0, $output_h0, $rkN_h                   // AES final-2 block - round N high
.Ldec_blocks_more_than_2:                                    // blocks left >  2
	rev64   $res0b, $res1b                                   // GHASH final-2 block
	ld1     { $res1b}, [$input_ptr], #16                     // AES final-1 block - load ciphertext
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	stp     $output_l0, $output_h0, [$output_ptr], #16       // AES final-2 block  - store result
	eor     $ctr0b, $res1b, $ctr2b                           // AES final-1 block - result
	mov     $rk4d, $res0.d[1]                                // GHASH final-2 block - mid
	pmull   $rk3q1, $res0.1d, $h3.1d                         // GHASH final-2 block - low
	pmull2  $rk2q1, $res0.2d, $h3.2d                         // GHASH final-2 block - high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-2 block - mid
	mov     $output_l0, $ctr0.d[0]                           // AES final-1 block - mov low
	mov     $output_h0, $ctr0.d[1]                           // AES final-1 block - mov high
	eor     $acc_lb, $acc_lb, $rk3                           // GHASH final-2 block - low
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	pmull   $rk4v.1q, $rk4v.1d, $h34k.1d                     // GHASH final-2 block - mid
	eor     $acc_hb, $acc_hb, $rk2                           // GHASH final-2 block - high
	eor     $output_l0, $output_l0, $rkN_l                   // AES final-1 block - round N low
	eor     $acc_mb, $acc_mb, $rk4v.16b                      // GHASH final-2 block - mid
	eor     $output_h0, $output_h0, $rkN_h                   // AES final-1 block - round N high
.Ldec_blocks_more_than_1:                                        // blocks left >  1
	stp     $output_l0, $output_h0, [$output_ptr], #16       // AES final-1 block  - store result
	rev64   $res0b, $res1b                                   // GHASH final-1 block
	ld1     { $res1b}, [$input_ptr], #16                     // AES final block - load ciphertext
	eor     $res0b, $res0b, $t0.16b                          // feed in partial tag
	movi    $t0.8b, #0                                       // suppress further partial tag feed in
	mov     $rk4d, $res0.d[1]                                // GHASH final-1 block - mid
	eor     $ctr0b, $res1b, $ctr3b                           // AES final block - result
	pmull2  $rk2q1, $res0.2d, $h2.2d                         // GHASH final-1 block - high
	eor     $rk4v.8b, $rk4v.8b, $res0.8b                     // GHASH final-1 block - mid
	pmull   $rk3q1, $res0.1d, $h2.1d                         // GHASH final-1 block - low
	mov     $output_l0, $ctr0.d[0]                           // AES final block - mov low
	ins     $rk4v.d[1], $rk4v.d[0]                           // GHASH final-1 block - mid
	mov     $output_h0, $ctr0.d[1]                           // AES final block - mov high
	pmull2  $rk4v.1q, $rk4v.2d, $h12k.2d                     // GHASH final-1 block - mid
	eor     $output_l0, $output_l0, $rkN_l                   // AES final block - round N low
	eor     $acc_lb, $acc_lb, $rk3                           // GHASH final-1 block - low
	eor     $acc_hb, $acc_hb, $rk2                           // GHASH final-1 block - high
	eor     $acc_mb, $acc_mb, $rk4v.16b                      // GHASH final-1 block - mid
	eor     $output_h0, $output_h0, $rkN_h                   // AES final block - round N high
.Ldec_blocks_less_than_1:                                        // blocks left <= 1
	and     $bit_length, $bit_length, #127                   // bit_length %= 128
	mvn     $rkN_h, xzr                                      // rkN_h = 0xffffffffffffffff
	sub     $bit_length, $bit_length, #128                   // bit_length -= 128
	mvn     $rkN_l, xzr                                      // rkN_l = 0xffffffffffffffff
	ldp     $end_input_ptr, $main_end_input_ptr, [$output_ptr] // load existing bytes we need to not overwrite
	neg     $bit_length, $bit_length                         // bit_length = 128 - #bits in input (in range [1,128])
	and     $bit_length, $bit_length, #127                   // bit_length %= 128
	lsr     $rkN_h, $rkN_h, $bit_length                      // rkN_h is mask for top 64b of last block
	cmp     $bit_length, #64
	csel    $ctr32x, $rkN_l, $rkN_h, lt
	csel    $ctr96_b64x, $rkN_h, xzr, lt
	fmov    $ctr0d, $ctr32x                                  // ctr0b is mask for last block
	and     $output_l0, $output_l0, $ctr32x
	mov     $ctr0.d[1], $ctr96_b64x
	bic     $end_input_ptr, $end_input_ptr, $ctr32x          // mask out low existing bytes
	rev     $ctr32w, $rctr32w
	bic     $main_end_input_ptr, $main_end_input_ptr, $ctr96_b64x      // mask out high existing bytes
	orr     $output_l0, $output_l0, $end_input_ptr
	and     $output_h0, $output_h0, $ctr96_b64x
	orr     $output_h0, $output_h0, $main_end_input_ptr
	and     $res1b, $res1b, $ctr0b                            // possibly partial last block has zeroes in highest bits
	rev64   $res0b, $res1b                                    // GHASH final block
	eor     $res0b, $res0b, $t0.16b                           // feed in partial tag
	pmull   $rk3q1, $res0.1d, $h1.1d                          // GHASH final block - low
	mov     $t0d, $res0.d[1]                                  // GHASH final block - mid
	eor     $t0.8b, $t0.8b, $res0.8b                          // GHASH final block - mid
	pmull2  $rk2q1, $res0.2d, $h1.2d                          // GHASH final block - high
	pmull   $t0.1q, $t0.1d, $h12k.1d                          // GHASH final block - mid
	eor     $acc_hb, $acc_hb, $rk2                            // GHASH final block - high
	eor     $acc_lb, $acc_lb, $rk3                            // GHASH final block - low
	eor     $acc_mb, $acc_mb, $t0.16b                         // GHASH final block - mid
	movi    $mod_constant.8b, #0xc2
	eor     $t9.16b, $acc_lb, $acc_hb                         // MODULO - karatsuba tidy up
	shl     $mod_constantd, $mod_constantd, #56               // mod_constant
	eor     $acc_mb, $acc_mb, $t9.16b                         // MODULO - karatsuba tidy up
	pmull   $mod_t.1q, $acc_h.1d, $mod_constant.1d            // MODULO - top 64b align with mid
	ext     $acc_hb, $acc_hb, $acc_hb, #8                     // MODULO - other top alignment
	eor     $acc_mb, $acc_mb, $mod_t.16b                      // MODULO - fold into mid
	eor     $acc_mb, $acc_mb, $acc_hb                         // MODULO - fold into mid
	pmull   $mod_constant.1q, $acc_m.1d, $mod_constant.1d     // MODULO - mid 64b align with low
	ext     $acc_mb, $acc_mb, $acc_mb, #8                     // MODULO - other mid alignment
	eor     $acc_lb, $acc_lb, $mod_constant.16b               // MODULO - fold into low
	stp     $output_l0, $output_h0, [$output_ptr]
	str     $ctr32w, [$counter, #12]                          // store the updated counter
	eor     $acc_lb, $acc_lb, $acc_mb                         // MODULO - fold into low
	ext     $acc_lb, $acc_lb, $acc_lb, #8
	rev64   $acc_lb, $acc_lb
	mov     x0, $len
	st1     { $acc_l.16b }, [$current_tag]
	ldp     x19, x20, [sp, #16]
	ldp     x21, x22, [sp, #32]
	ldp     x23, x24, [sp, #48]
	ldp     d8, d9, [sp, #64]
	ldp     d10, d11, [sp, #80]
	ldp     d12, d13, [sp, #96]
	ldp     d14, d15, [sp, #112]
	ldp     x29, x30, [sp], #128
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size aes_gcm_dec_kernel,.-aes_gcm_dec_kernel
___
}
}

$code.=<<___;
#endif
___

print $code;
close STDOUT or die "error closing STDOUT: $!"; # enforce flush
