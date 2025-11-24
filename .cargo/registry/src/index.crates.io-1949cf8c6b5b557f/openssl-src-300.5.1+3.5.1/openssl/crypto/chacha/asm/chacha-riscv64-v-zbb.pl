#! /usr/bin/env perl
# This file is dual-licensed, meaning that you can use it under your
# choice of either of the following two licenses:
#
# Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# or
#
# Copyright (c) 2023, Jerry Shih <jerry.shih@sifive.com>
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions
# are met:
# 1. Redistributions of source code must retain the above copyright
#    notice, this list of conditions and the following disclaimer.
# 2. Redistributions in binary form must reproduce the above copyright
#    notice, this list of conditions and the following disclaimer in the
#    documentation and/or other materials provided with the distribution.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
# "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
# LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
# A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
# OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
# SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
# LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
# DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
# THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
# (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

# - RV64I
# - RISC-V Vector ('V') with VLEN >= 128
# - RISC-V Basic Bit-manipulation extension ('Zbb')
# - RISC-V Zicclsm(Main memory supports misaligned loads/stores)
# Optional:
# - RISC-V Vector Cryptography Bit-manipulation extension ('Zvkb')

use strict;
use warnings;

use FindBin qw($Bin);
use lib "$Bin";
use lib "$Bin/../../perlasm";
use riscv;

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
my $output  = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop   : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.|          ? shift : undef;

my $use_zvkb = $flavour && $flavour =~ /zvkb/i ? 1 : 0;
my $isaext = "_v_zbb" . ( $use_zvkb ? "_zvkb" : "" );

$output and open STDOUT, ">$output";

my $code = <<___;
.text
___

# void ChaCha20_ctr32@{[$isaext]}(unsigned char *out, const unsigned char *inp,
#                                 size_t len, const unsigned int key[8],
#                                 const unsigned int counter[4]);
################################################################################
my ( $OUTPUT, $INPUT, $LEN, $KEY, $COUNTER ) = ( "a0", "a1", "a2", "a3", "a4" );
my ( $CONST_DATA0, $CONST_DATA1, $CONST_DATA2, $CONST_DATA3 ) = ( "a5", "a6",
  "a7", "s0" );
my ( $KEY0, $KEY1, $KEY2, $KEY3, $KEY4, $KEY5, $KEY6, $KEY7, $COUNTER0,
     $COUNTER1, $NONCE0, $NONCE1) = ( "s1", "s2", "s3", "s4", "s5", "s6", "s7",
  "s8", "s9", "s10", "s11", "t0" );
my ( $STATE0, $STATE1, $STATE2, $STATE3,
     $STATE4, $STATE5, $STATE6, $STATE7,
     $STATE8, $STATE9, $STATE10, $STATE11,
     $STATE12, $STATE13, $STATE14, $STATE15) = (
     $CONST_DATA0, $CONST_DATA1, $CONST_DATA2, $CONST_DATA3,
     $KEY0, $KEY1, $KEY2, $KEY3,
     $KEY4, $KEY5, $KEY6, $KEY7,
     $COUNTER0, $COUNTER1, $NONCE0, $NONCE1 );
my ( $VL ) = ( "t1" );
my ( $CURRENT_COUNTER ) = ( "t2" );
my ( $T0 ) = ( "t3" );
my ( $T1 ) = ( "t4" );
my ( $T2 ) = ( "t5" );
my ( $T3 ) = ( "t6" );
my (
    $V0,  $V1,  $V2,  $V3,  $V4,  $V5,  $V6,  $V7,  $V8,  $V9,  $V10,
    $V11, $V12, $V13, $V14, $V15, $V16, $V17, $V18, $V19, $V20, $V21,
    $V22, $V23, $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map( "v$_", ( 0 .. 31 ) );

sub chacha_sub_round {
    my (
        $A0, $B0, $C0,
        $A1, $B1, $C1,
        $A2, $B2, $C2,
        $A3, $B3, $C3,

        $S_A0, $S_B0, $S_C0,
        $S_A1, $S_B1, $S_C1,
        $S_A2, $S_B2, $S_C2,
        $S_A3, $S_B3, $S_C3,

        $ROL_SHIFT,

        $V_T0, $V_T1, $V_T2, $V_T3,
    ) = @_;

    # a += b; c ^= a;
    my $code = <<___;
    @{[vadd_vv $A0, $A0, $B0]}
    add $S_A0, $S_A0, $S_B0
    @{[vadd_vv $A1, $A1, $B1]}
    add $S_A1, $S_A1, $S_B1
    @{[vadd_vv $A2, $A2, $B2]}
    add $S_A2, $S_A2, $S_B2
    @{[vadd_vv $A3, $A3, $B3]}
    add $S_A3, $S_A3, $S_B3
    @{[vxor_vv $C0, $C0, $A0]}
    xor $S_C0, $S_C0, $S_A0
    @{[vxor_vv $C1, $C1, $A1]}
    xor $S_C1, $S_C1, $S_A1
    @{[vxor_vv $C2, $C2, $A2]}
    xor $S_C2, $S_C2, $S_A2
    @{[vxor_vv $C3, $C3, $A3]}
    xor $S_C3, $S_C3, $S_A3
___

    # c <<<= $ROL_SHIFT;
    if ($use_zvkb) {
        my $ror_part = <<___;
        @{[vror_vi $C0, $C0, 32 - $ROL_SHIFT]}
        @{[roriw $S_C0, $S_C0, 32 - $ROL_SHIFT]}
        @{[vror_vi $C1, $C1, 32 - $ROL_SHIFT]}
        @{[roriw $S_C1, $S_C1, 32 - $ROL_SHIFT]}
        @{[vror_vi $C2, $C2, 32 - $ROL_SHIFT]}
        @{[roriw $S_C2, $S_C2, 32 - $ROL_SHIFT]}
        @{[vror_vi $C3, $C3, 32 - $ROL_SHIFT]}
        @{[roriw $S_C3, $S_C3, 32 - $ROL_SHIFT]}
___

        $code .= $ror_part;
    } else {
        my $ror_part = <<___;
        @{[vsll_vi $V_T0, $C0, $ROL_SHIFT]}
        @{[vsll_vi $V_T1, $C1, $ROL_SHIFT]}
        @{[vsll_vi $V_T2, $C2, $ROL_SHIFT]}
        @{[vsll_vi $V_T3, $C3, $ROL_SHIFT]}
        @{[vsrl_vi $C0, $C0, 32 - $ROL_SHIFT]}
        @{[vsrl_vi $C1, $C1, 32 - $ROL_SHIFT]}
        @{[vsrl_vi $C2, $C2, 32 - $ROL_SHIFT]}
        @{[vsrl_vi $C3, $C3, 32 - $ROL_SHIFT]}
        @{[vor_vv $C0, $C0, $V_T0]}
        @{[roriw $S_C0, $S_C0, 32 - $ROL_SHIFT]}
        @{[vor_vv $C1, $C1, $V_T1]}
        @{[roriw $S_C1, $S_C1, 32 - $ROL_SHIFT]}
        @{[vor_vv $C2, $C2, $V_T2]}
        @{[roriw $S_C2, $S_C2, 32 - $ROL_SHIFT]}
        @{[vor_vv $C3, $C3, $V_T3]}
        @{[roriw $S_C3, $S_C3, 32 - $ROL_SHIFT]}
___

        $code .= $ror_part;
    }

    return $code;
}

sub chacha_quad_round_group {
    my (
        $A0, $B0, $C0, $D0,
        $A1, $B1, $C1, $D1,
        $A2, $B2, $C2, $D2,
        $A3, $B3, $C3, $D3,

        $S_A0, $S_B0, $S_C0, $S_D0,
        $S_A1, $S_B1, $S_C1, $S_D1,
        $S_A2, $S_B2, $S_C2, $S_D2,
        $S_A3, $S_B3, $S_C3, $S_D3,

        $V_T0, $V_T1, $V_T2, $V_T3,
    ) = @_;

    my $code = <<___;
    # a += b; d ^= a; d <<<= 16;
    @{[chacha_sub_round
      $A0, $B0, $D0,
      $A1, $B1, $D1,
      $A2, $B2, $D2,
      $A3, $B3, $D3,
      $S_A0, $S_B0, $S_D0,
      $S_A1, $S_B1, $S_D1,
      $S_A2, $S_B2, $S_D2,
      $S_A3, $S_B3, $S_D3,
      16,
      $V_T0, $V_T1, $V_T2, $V_T3]}
    # c += d; b ^= c; b <<<= 12;
    @{[chacha_sub_round
      $C0, $D0, $B0,
      $C1, $D1, $B1,
      $C2, $D2, $B2,
      $C3, $D3, $B3,
      $S_C0, $S_D0, $S_B0,
      $S_C1, $S_D1, $S_B1,
      $S_C2, $S_D2, $S_B2,
      $S_C3, $S_D3, $S_B3,
      12,
      $V_T0, $V_T1, $V_T2, $V_T3]}
    # a += b; d ^= a; d <<<= 8;
    @{[chacha_sub_round
      $A0, $B0, $D0,
      $A1, $B1, $D1,
      $A2, $B2, $D2,
      $A3, $B3, $D3,
      $S_A0, $S_B0, $S_D0,
      $S_A1, $S_B1, $S_D1,
      $S_A2, $S_B2, $S_D2,
      $S_A3, $S_B3, $S_D3,
      8,
      $V_T0, $V_T1, $V_T2, $V_T3]}
    # c += d; b ^= c; b <<<= 7;
    @{[chacha_sub_round
      $C0, $D0, $B0,
      $C1, $D1, $B1,
      $C2, $D2, $B2,
      $C3, $D3, $B3,
      $S_C0, $S_D0, $S_B0,
      $S_C1, $S_D1, $S_B1,
      $S_C2, $S_D2, $S_B2,
      $S_C3, $S_D3, $S_B3,
      7,
      $V_T0, $V_T1, $V_T2, $V_T3]}
___

    return $code;
}

$code .= <<___;
.p2align 3
.globl ChaCha20_ctr32@{[$isaext]}
.type ChaCha20_ctr32@{[$isaext]},\@function
ChaCha20_ctr32@{[$isaext]}:
    addi sp, sp, -96
    sd s0, 0(sp)
    sd s1, 8(sp)
    sd s2, 16(sp)
    sd s3, 24(sp)
    sd s4, 32(sp)
    sd s5, 40(sp)
    sd s6, 48(sp)
    sd s7, 56(sp)
    sd s8, 64(sp)
    sd s9, 72(sp)
    sd s10, 80(sp)
    sd s11, 88(sp)
    addi sp, sp, -64

    lw $CURRENT_COUNTER, 0($COUNTER)

.Lblock_loop:
    # We will use the scalar ALU for 1 chacha block.
    srli $T0, $LEN, 6
    @{[vsetvli $VL, $T0, "e32", "m1", "ta", "ma"]}
    slli $T1, $VL, 6
    bltu $T1, $LEN, 1f
    # Since there is no more chacha block existed, we need to split 1 block
    # from vector ALU.
    addi $T1, $VL, -1
    @{[vsetvli $VL, $T1, "e32", "m1", "ta", "ma"]}
1:

    #### chacha block data
    # init chacha const states into $V0~$V3
    # "expa" little endian
    li $CONST_DATA0, 0x61707865
    @{[vmv_v_x $V0, $CONST_DATA0]}
    # "nd 3" little endian
    li $CONST_DATA1, 0x3320646e
    @{[vmv_v_x $V1, $CONST_DATA1]}
    # "2-by" little endian
    li $CONST_DATA2, 0x79622d32
    @{[vmv_v_x $V2, $CONST_DATA2]}
    # "te k" little endian
    li $CONST_DATA3, 0x6b206574
    lw $KEY0, 0($KEY)
    @{[vmv_v_x $V3, $CONST_DATA3]}

    # init chacha key states into $V4~$V11
    lw $KEY1, 4($KEY)
    @{[vmv_v_x $V4, $KEY0]}
    lw $KEY2, 8($KEY)
    @{[vmv_v_x $V5, $KEY1]}
    lw $KEY3, 12($KEY)
    @{[vmv_v_x $V6, $KEY2]}
    lw $KEY4, 16($KEY)
    @{[vmv_v_x $V7, $KEY3]}
    lw $KEY5, 20($KEY)
    @{[vmv_v_x $V8, $KEY4]}
    lw $KEY6, 24($KEY)
    @{[vmv_v_x $V9, $KEY5]}
    lw $KEY7, 28($KEY)
    @{[vmv_v_x $V10, $KEY6]}
    @{[vmv_v_x $V11, $KEY7]}

    # init chacha key states into $V12~$V13
    lw $COUNTER1, 4($COUNTER)
    @{[vid_v $V12]}
    lw $NONCE0, 8($COUNTER)
    @{[vadd_vx $V12, $V12, $CURRENT_COUNTER]}
    lw $NONCE1, 12($COUNTER)
    @{[vmv_v_x $V13, $COUNTER1]}
    add $COUNTER0, $CURRENT_COUNTER, $VL

    # init chacha nonce states into $V14~$V15
    @{[vmv_v_x $V14, $NONCE0]}
    @{[vmv_v_x $V15, $NONCE1]}

    li $T0, 64
    # load the top-half of input data into $V16~$V23
    @{[vlsseg_nf_e32_v 8, $V16, $INPUT, $T0]}

    # till now in block_loop, we used:
    # - $V0~$V15 for chacha states.
    # - $V16~$V23 for top-half of input data.
    # - $V24~$V31 haven't been used yet.

    # 20 round groups
    li $T0, 10
.Lround_loop:
    # we can use $V24~$V31 as temporary registers in round_loop.
    addi $T0, $T0, -1
    @{[chacha_quad_round_group
      $V0, $V4, $V8, $V12,
      $V1, $V5, $V9, $V13,
      $V2, $V6, $V10, $V14,
      $V3, $V7, $V11, $V15,
      $STATE0, $STATE4, $STATE8, $STATE12,
      $STATE1, $STATE5, $STATE9, $STATE13,
      $STATE2, $STATE6, $STATE10, $STATE14,
      $STATE3, $STATE7, $STATE11, $STATE15,
      $V24, $V25, $V26, $V27]}
    @{[chacha_quad_round_group
      $V3, $V4, $V9, $V14,
      $V0, $V5, $V10, $V15,
      $V1, $V6, $V11, $V12,
      $V2, $V7, $V8, $V13,
      $STATE3, $STATE4, $STATE9, $STATE14,
      $STATE0, $STATE5, $STATE10, $STATE15,
      $STATE1, $STATE6, $STATE11, $STATE12,
      $STATE2, $STATE7, $STATE8, $STATE13,
      $V24, $V25, $V26, $V27]}
    bnez $T0, .Lround_loop

    li $T0, 64
    # load the bottom-half of input data into $V24~$V31
    addi $T1, $INPUT, 32
    @{[vlsseg_nf_e32_v 8, $V24, $T1, $T0]}

    # now, there are no free vector registers until the round_loop exits.

    # add chacha top-half initial block states
    # "expa" little endian
    li $T0, 0x61707865
    @{[vadd_vx $V0, $V0, $T0]}
    add $STATE0, $STATE0, $T0
    # "nd 3" little endian
    li $T1, 0x3320646e
    @{[vadd_vx $V1, $V1, $T1]}
    add $STATE1, $STATE1, $T1
    lw $T0, 0($KEY)
    # "2-by" little endian
    li $T2, 0x79622d32
    @{[vadd_vx $V2, $V2, $T2]}
    add $STATE2, $STATE2, $T2
    lw $T1, 4($KEY)
    # "te k" little endian
    li $T3, 0x6b206574
    @{[vadd_vx $V3, $V3, $T3]}
    add $STATE3, $STATE3, $T3
    lw $T2, 8($KEY)
    @{[vadd_vx $V4, $V4, $T0]}
    add $STATE4, $STATE4, $T0
    lw $T3, 12($KEY)
    @{[vadd_vx $V5, $V5, $T1]}
    add $STATE5, $STATE5, $T1
    @{[vadd_vx $V6, $V6, $T2]}
    add $STATE6, $STATE6, $T2
    @{[vadd_vx $V7, $V7, $T3]}
    add $STATE7, $STATE7, $T3

    # xor with the top-half input
    @{[vxor_vv $V16, $V16, $V0]}
    sw $STATE0, 0(sp)
    sw $STATE1, 4(sp)
    @{[vxor_vv $V17, $V17, $V1]}
    sw $STATE2, 8(sp)
    sw $STATE3, 12(sp)
    @{[vxor_vv $V18, $V18, $V2]}
    sw $STATE4, 16(sp)
    sw $STATE5, 20(sp)
    @{[vxor_vv $V19, $V19, $V3]}
    sw $STATE6, 24(sp)
    sw $STATE7, 28(sp)
    @{[vxor_vv $V20, $V20, $V4]}
    lw $T0, 16($KEY)
    @{[vxor_vv $V21, $V21, $V5]}
    lw $T1, 20($KEY)
    @{[vxor_vv $V22, $V22, $V6]}
    lw $T2, 24($KEY)
    @{[vxor_vv $V23, $V23, $V7]}

    # save the top-half of output from $V16~$V23
    li $T3, 64
    @{[vssseg_nf_e32_v 8, $V16, $OUTPUT, $T3]}

    # add chacha bottom-half initial block states
    @{[vadd_vx $V8, $V8, $T0]}
    add $STATE8, $STATE8, $T0
    lw $T3, 28($KEY)
    @{[vadd_vx $V9, $V9, $T1]}
    add $STATE9, $STATE9, $T1
    lw $T0, 4($COUNTER)
    @{[vadd_vx $V10, $V10, $T2]}
    add $STATE10, $STATE10, $T2
    lw $T1, 8($COUNTER)
    @{[vadd_vx $V11, $V11, $T3]}
    add $STATE11, $STATE11, $T3
    lw $T2, 12($COUNTER)
    @{[vid_v $V0]}
    add $STATE12, $STATE12, $CURRENT_COUNTER
    @{[vadd_vx $V12, $V12, $CURRENT_COUNTER]}
    add $STATE12, $STATE12, $VL
    @{[vadd_vx $V13, $V13, $T0]}
    add $STATE13, $STATE13, $T0
    @{[vadd_vx $V14, $V14, $T1]}
    add $STATE14, $STATE14, $T1
    @{[vadd_vx $V15, $V15, $T2]}
    add $STATE15, $STATE15, $T2
    @{[vadd_vv $V12, $V12, $V0]}
    # xor with the bottom-half input
    @{[vxor_vv $V24, $V24, $V8]}
    sw $STATE8, 32(sp)
    @{[vxor_vv $V25, $V25, $V9]}
    sw $STATE9, 36(sp)
    @{[vxor_vv $V26, $V26, $V10]}
    sw $STATE10, 40(sp)
    @{[vxor_vv $V27, $V27, $V11]}
    sw $STATE11, 44(sp)
    @{[vxor_vv $V29, $V29, $V13]}
    sw $STATE12, 48(sp)
    @{[vxor_vv $V28, $V28, $V12]}
    sw $STATE13, 52(sp)
    @{[vxor_vv $V30, $V30, $V14]}
    sw $STATE14, 56(sp)
    @{[vxor_vv $V31, $V31, $V15]}
    sw $STATE15, 60(sp)

    # save the bottom-half of output from $V24~$V31
    li $T0, 64
    addi $T1, $OUTPUT, 32
    @{[vssseg_nf_e32_v 8, $V24, $T1, $T0]}

    # the computed vector parts: `64 * VL`
    slli $T0, $VL, 6

    add $INPUT, $INPUT, $T0
    add $OUTPUT, $OUTPUT, $T0
    sub $LEN, $LEN, $T0
    add $CURRENT_COUNTER, $CURRENT_COUNTER, $VL

    # process the scalar data block
    addi $CURRENT_COUNTER, $CURRENT_COUNTER, 1
    li $T0, 64
    @{[minu $T1, $LEN, $T0]}
    sub $LEN, $LEN, $T1
    mv $T2, sp
.Lscalar_data_loop:
    @{[vsetvli $VL, $T1, "e8", "m8", "ta", "ma"]}
    # from this on, vector registers are grouped with lmul = 8
    @{[vle8_v $V8, $INPUT]}
    @{[vle8_v $V16, $T2]}
    @{[vxor_vv $V8, $V8, $V16]}
    @{[vse8_v $V8, $OUTPUT]}
    add $INPUT, $INPUT, $VL
    add $OUTPUT, $OUTPUT, $VL
    add $T2, $T2, $VL
    sub $T1, $T1, $VL
    bnez $T1, .Lscalar_data_loop

    bnez $LEN, .Lblock_loop

    addi sp, sp, 64
    ld s0, 0(sp)
    ld s1, 8(sp)
    ld s2, 16(sp)
    ld s3, 24(sp)
    ld s4, 32(sp)
    ld s5, 40(sp)
    ld s6, 48(sp)
    ld s7, 56(sp)
    ld s8, 64(sp)
    ld s9, 72(sp)
    ld s10, 80(sp)
    ld s11, 88(sp)
    addi sp, sp, 96

    ret
.size ChaCha20_ctr32@{[$isaext]},.-ChaCha20_ctr32@{[$isaext]}
___

print $code;

close STDOUT or die "error closing STDOUT: $!";
