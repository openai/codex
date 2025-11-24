#! /usr/bin/env perl
# This file is dual-licensed, meaning that you can use it under your
# choice of either of the following two licenses:
#
# Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License"). You can obtain
# a copy in the file LICENSE in the source distribution or at
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
# - RISC-V Vector Cryptography Bit-manipulation extension ('Zvkb')
# - RISC-V Vector GCM/GMAC extension ('Zvkg')
# - RISC-V Vector AES Block Cipher extension ('Zvkned')
# - RISC-V Zicclsm(Main memory supports misaligned loads/stores)

# Reference: https://github.com/riscv/riscv-crypto/issues/192#issuecomment-1270447575
#
# Assume we have 12 GCM blocks and we try to parallelize GCM computation for 4 blocks.
# Tag = M0*H^12 + M1*H^11 + M2*H^10 + M3*H^9 +
#       M4*H^8  + M5*H^7  + M6*H^6  + M7*H^5 +
#       M8*H^4  + M9*H^3  + M10*H^2 + M11*H^1
# We could rewrite the formula into:
# T0 = 0
# T1 = (T0+M1)*H^4   T2 = (T0+M2)*H^4    T3 = (T0+M3)*H^4    T4 = (T0+M4)*H^4
# T5 = (T1+M5)*H^4   T6 = (T2+M6)*H^4    T7 = (T3+M7)*H^4    T8 = (T4+M8)*H^4
# T9 = (T5+M9)*H^4  T10 = (T6+M10)*H^3  T11 = (T7+M11)*H^2  T12 = (T8+M12)*H^1
#
# We will multiply with [H^4, H^4, H^4, H^4] in each steps except the last iteration.
# The last iteration will multiply with [H^4, H^3, H^2, H^1].

use strict;
use warnings;

use FindBin qw($Bin);
use lib "$Bin";
use lib "$Bin/../../perlasm";
use riscv;

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
my $output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$output and open STDOUT,">$output";

my $code=<<___;
.text
___

{
my ($INP, $OUTP, $LEN, $KEYP, $IVP, $XIP) = ("a0", "a1", "a2", "a3", "a4", "a5");
my ($T0, $T1, $T2, $T3) = ("t0", "t1", "t2", "t3");
my ($PADDING_LEN32) = ("t4");
my ($LEN32) = ("t5");
my ($CTR) = ("t6");
my ($FULL_BLOCK_LEN32) = ("a6");
my ($ORIGINAL_LEN32) = ("a7");
my ($PROCESSED_LEN) = ("a0");
my ($CTR_MASK) = ("v0");
my ($INPUT_PADDING_MASK) = ("v0");
my ($V0, $V1, $V2, $V3, $V4, $V5, $V6, $V7,
    $V8, $V9, $V10, $V11, $V12, $V13, $V14, $V15,
    $V16, $V17, $V18, $V19, $V20, $V21, $V22, $V23,
    $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map("v$_",(0..31));

# Do aes-128 enc.
sub aes_128_cipher_body {
    my $code=<<___;
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesz_vs $V28, $V1]}
    @{[vaesem_vs $V28, $V2]}
    @{[vaesem_vs $V28, $V3]}
    @{[vaesem_vs $V28, $V4]}
    @{[vaesem_vs $V28, $V5]}
    @{[vaesem_vs $V28, $V6]}
    @{[vaesem_vs $V28, $V7]}
    @{[vaesem_vs $V28, $V8]}
    @{[vaesem_vs $V28, $V9]}
    @{[vaesem_vs $V28, $V10]}
    @{[vaesef_vs $V28, $V11]}
___

    return $code;
}

# Do aes-192 enc.
sub aes_192_cipher_body {
    my $TMP_REG = shift;

    my $code=<<___;
    # Load key 4
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 48
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesz_vs $V28, $V1]}
    @{[vaesem_vs $V28, $V2]}
    @{[vaesem_vs $V28, $V3]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 8
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 112
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V4]}
    @{[vaesem_vs $V28, $V5]}
    @{[vaesem_vs $V28, $V6]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 13
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 192
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V7]}
    @{[vaesem_vs $V28, $V8]}
    @{[vaesem_vs $V28, $V9]}
    @{[vaesem_vs $V28, $V10]}
    @{[vaesef_vs $V28, $V11]}
___

    return $code;
}

# Do aes-256 enc.
sub aes_256_cipher_body {
    my $TMP_REG = shift;

    my $code=<<___;
    # Load key 3
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 32
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesz_vs $V28, $V1]}
    @{[vaesem_vs $V28, $V2]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 6
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 80
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V3]}
    @{[vaesem_vs $V28, $V4]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 9
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 128
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V5]}
    @{[vaesem_vs $V28, $V6]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 12
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 176
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V7]}
    @{[vaesem_vs $V28, $V8]}
    @{[vaesem_vs $V28, $V11]}
    # Load key 15
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    addi $TMP_REG, $KEYP, 224
    @{[vle32_v $V11, $TMP_REG]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vaesem_vs $V28, $V9]}
    @{[vaesem_vs $V28, $V10]}
    @{[vaesef_vs $V28, $V11]}
___

    return $code;
}

sub handle_padding_in_first_round {
    my $TMP_REG = shift;

    my $code=<<___;
    bnez $PADDING_LEN32, 1f

    ## without padding
    # Store ciphertext/plaintext
    @{[vse32_v $V28, $OUTP]}
    j 2f

    ## with padding
1:
    # Store ciphertext/plaintext using mask
    @{[vse32_v $V28, $OUTP, $INPUT_PADDING_MASK]}

    # Fill zero for the padding blocks
    @{[vsetvli "zero", $PADDING_LEN32, "e32", "m4", "tu", "ma"]}
    @{[vmv_v_i $V28, 0]}

    # We have used mask register for `INPUT_PADDING_MASK` before. We need to
    # setup the ctr mask back.
    # ctr mask : [000100010001....]
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e8", "m1", "ta", "ma"]}
    li $TMP_REG, 0b10001000
    @{[vmv_v_x $CTR_MASK, $TMP_REG]}
2:

___

    return $code;
}


# Do aes-128 enc for first round.
sub aes_128_first_round {
    my $PTR_OFFSET_REG = shift;
    my $TMP_REG = shift;

    my $code=<<___;
    # Load all 11 aes round keys to v1-v11 registers.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V1, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V2, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V3, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V4, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V5, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V6, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V7, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V8, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V9, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V10, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V11, $KEYP]}

    # We already have the ciphertext/plaintext and ctr data for the first round.
    @{[aes_128_cipher_body]}

    # Compute AES ctr result.
    @{[vxor_vv $V28, $V28, $V24]}

    @{[handle_padding_in_first_round $TMP_REG]}

    add $INP, $INP, $PTR_OFFSET_REG
    add $OUTP, $OUTP, $PTR_OFFSET_REG
___

    return $code;
}

# Do aes-192 enc for first round.
sub aes_192_first_round {
    my $PTR_OFFSET_REG = shift;
    my $TMP_REG = shift;

    my $code=<<___;
    # We run out of 32 vector registers, so we just preserve some round keys
    # and load the remaining round keys inside the aes body.
    # We keep the round keys for:
    #   1, 2, 3, 5, 6, 7, 9, 10, 11 and 12th keys.
    # The following keys will be loaded in the aes body:
    #   4, 8 and 13th keys.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    # key 1
    @{[vle32_v $V1, $KEYP]}
    # key 2
    addi $TMP_REG, $KEYP, 16
    @{[vle32_v $V2, $TMP_REG]}
    # key 3
    addi $TMP_REG, $KEYP, 32
    @{[vle32_v $V3, $TMP_REG]}
    # key 5
    addi $TMP_REG, $KEYP, 64
    @{[vle32_v $V4, $TMP_REG]}
    # key 6
    addi $TMP_REG, $KEYP, 80
    @{[vle32_v $V5, $TMP_REG]}
    # key 7
    addi $TMP_REG, $KEYP, 96
    @{[vle32_v $V6, $TMP_REG]}
    # key 9
    addi $TMP_REG, $KEYP, 128
    @{[vle32_v $V7, $TMP_REG]}
    # key 10
    addi $TMP_REG, $KEYP, 144
    @{[vle32_v $V8, $TMP_REG]}
    # key 11
    addi $TMP_REG, $KEYP, 160
    @{[vle32_v $V9, $TMP_REG]}
    # key 12
    addi $TMP_REG, $KEYP, 176
    @{[vle32_v $V10, $TMP_REG]}

    # We already have the ciphertext/plaintext and ctr data for the first round.
    @{[aes_192_cipher_body $TMP_REG]}

    # Compute AES ctr result.
    @{[vxor_vv $V28, $V28, $V24]}

    @{[handle_padding_in_first_round $TMP_REG]}

    add $INP, $INP, $PTR_OFFSET_REG
    add $OUTP, $OUTP, $PTR_OFFSET_REG
___

    return $code;
}

# Do aes-256 enc for first round.
sub aes_256_first_round {
    my $PTR_OFFSET_REG = shift;
    my $TMP_REG = shift;

    my $code=<<___;
    # We run out of 32 vector registers, so we just preserve some round keys
    # and load the remaining round keys inside the aes body.
    # We keep the round keys for:
    #   1, 2, 4, 5, 7, 8, 10, 11, 13 and 14th keys.
    # The following keys will be loaded in the aes body:
    #   3, 6, 9, 12 and 15th keys.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    # key 1
    @{[vle32_v $V1, $KEYP]}
    # key 2
    addi $TMP_REG, $KEYP, 16
    @{[vle32_v $V2, $TMP_REG]}
    # key 4
    addi $TMP_REG, $KEYP, 48
    @{[vle32_v $V3, $TMP_REG]}
    # key 5
    addi $TMP_REG, $KEYP, 64
    @{[vle32_v $V4, $TMP_REG]}
    # key 7
    addi $TMP_REG, $KEYP, 96
    @{[vle32_v $V5, $TMP_REG]}
    # key 8
    addi $TMP_REG, $KEYP, 112
    @{[vle32_v $V6, $TMP_REG]}
    # key 10
    addi $TMP_REG, $KEYP, 144
    @{[vle32_v $V7, $TMP_REG]}
    # key 11
    addi $TMP_REG, $KEYP, 160
    @{[vle32_v $V8, $TMP_REG]}
    # key 13
    addi $TMP_REG, $KEYP, 192
    @{[vle32_v $V9, $TMP_REG]}
    # key 14
    addi $TMP_REG, $KEYP, 208
    @{[vle32_v $V10, $TMP_REG]}

    # We already have the ciphertext/plaintext and ctr data for the first round.
    @{[aes_256_cipher_body $TMP_REG]}

    # Compute AES ctr result.
    @{[vxor_vv $V28, $V28, $V24]}

    @{[handle_padding_in_first_round $TMP_REG]}

    add $INP, $INP, $PTR_OFFSET_REG
    add $OUTP, $OUTP, $PTR_OFFSET_REG
___

    return $code;
}

sub aes_gcm_init {
    my $code=<<___;
    # Compute the AES-GCM full-block e32 length for `LMUL=4`. We will handle
    # the multiple AES-GCM blocks at the same time within `LMUL=4` register.
    # The AES-GCM's SEW is e32 and EGW is 128 bits.
    #   FULL_BLOCK_LEN32 = (VLEN*LMUL)/(EGW) * (EGW/SEW) = (VLEN*4)/(32*4) * 4
    #                    = (VLEN*4)/32
    # We could get the block_num using the VL value of `vsetvli with e32, m4`.
    @{[vsetvli $FULL_BLOCK_LEN32, "zero", "e32", "m4", "ta", "ma"]}
    # If `LEN32 % FULL_BLOCK_LEN32` is not equal to zero, we could fill the
    # zero padding data to make sure we could always handle FULL_BLOCK_LEN32
    # blocks for all iterations.

    ## Prepare the H^n multiplier in v16 for GCM multiplier. The `n` is the gcm
    ## block number in a LMUL=4 register group.
    ##   n = ((VLEN*LMUL)/(32*4)) = ((VLEN*4)/(32*4))
    ##     = (VLEN/32)
    ## We could use vsetvli with `e32, m1` to compute the `n` number.
    @{[vsetvli $T0, "zero", "e32", "m1", "ta", "ma"]}

    # The H is at `gcm128_context.Htable[0]`(addr(Xi)+16*2).
    addi $T1, $XIP, 32
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V31, $T1]}

    # Compute the H^n
    li $T1, 1
1:
    @{[vgmul_vv $V31, $V31]}
    slli $T1, $T1, 1
    bltu $T1, $T0, 1b

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V16, 0]}
    @{[vaesz_vs $V16, $V31]}

    #### Load plaintext into v24 and handle padding. We also load the init tag
    #### data into v20 and prepare the AES ctr input data into v12 and v28.
    @{[vmv_v_i $V20, 0]}

    ## Prepare the AES ctr input data into v12.
    # Setup ctr input mask.
    # ctr mask : [000100010001....]
    # Note: The actual vl should be `FULL_BLOCK_LEN32/4 * 2`, but we just use
    #   `FULL_BLOCK_LEN32` here.
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e8", "m1", "ta", "ma"]}
    li $T0, 0b10001000
    @{[vmv_v_x $CTR_MASK, $T0]}
    # Load IV.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V31, $IVP]}
    # Convert the big-endian counter into little-endian.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "mu"]}
    @{[vrev8_v $V31, $V31, $CTR_MASK]}
    # Splat the `single block of IV` to v12
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V12, 0]}
    @{[vaesz_vs $V12, $V31]}
    # Prepare the ctr counter into v8
    # v8: [x, x, x, 0, x, x, x, 1, x, x, x, 2, ...]
    @{[viota_m $V8, $CTR_MASK, $CTR_MASK]}
    # Merge IV and ctr counter into v12.
    # v12:[x, x, x, count+0, x, x, x, count+1, ...]
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "mu"]}
    @{[vadd_vv $V12, $V12, $V8, $CTR_MASK]}

    li $PADDING_LEN32, 0
    # Get the SEW32 size in the first round.
    # If we have the non-zero value for `LEN32&(FULL_BLOCK_LEN32-1)`, then
    # we will have the leading padding zero.
    addi $T0, $FULL_BLOCK_LEN32, -1
    and $T0, $T0, $LEN32
    beqz $T0, 1f

    ## with padding
    sub $LEN32, $LEN32, $T0
    sub $PADDING_LEN32, $FULL_BLOCK_LEN32, $T0
    # padding block size
    srli $T1, $PADDING_LEN32, 2
    # padding byte size
    slli $T2, $PADDING_LEN32, 2

    # Adjust the ctr counter to make the counter start from `counter+0` for the
    # first non-padding block.
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "mu"]}
    @{[vsub_vx $V12, $V12, $T1, $CTR_MASK]}
    # Prepare the AES ctr input into v28.
    # The ctr data uses big-endian form.
    @{[vmv_v_v $V28, $V12]}
    @{[vrev8_v $V28, $V28, $CTR_MASK]}

    # Prepare the mask for input loading in the first round. We use
    # `VL=FULL_BLOCK_LEN32` with the mask in the first round.
    # Adjust input ptr.
    sub $INP, $INP, $T2
    # Adjust output ptr.
    sub $OUTP, $OUTP, $T2
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e16", "m2", "ta", "ma"]}
    @{[vid_v $V2]}
    # We don't use the pseudo instruction `vmsgeu` here. Use `vmsgtu` instead.
    # The original code is:
    #   vmsgeu.vx $INPUT_PADDING_MASK, $V2, $PADDING_LEN32
    addi $T0, $PADDING_LEN32, -1
    @{[vmsgtu_vx $INPUT_PADDING_MASK, $V2, $T0]}
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V24, 0]}
    # Load the input for length FULL_BLOCK_LEN32 with mask.
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "mu"]}
    @{[vle32_v $V24, $INP, $INPUT_PADDING_MASK]}

    # Load the init `Xi` data to v20 with preceding zero padding.
    # Adjust Xi ptr.
    sub $T0, $XIP, $T2
    # Load for length `zero-padding-e32-length + 4`.
    addi $T1, $PADDING_LEN32, 4
    @{[vsetvli "zero", $T1, "e32", "m4", "tu", "mu"]}
    @{[vle32_v $V20, $T0, $INPUT_PADDING_MASK]}
    j 2f

1:
    ## without padding
    sub $LEN32, $LEN32, $FULL_BLOCK_LEN32

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vle32_v $V24, $INP]}

    # Load the init Xi data to v20.
    @{[vsetivli "zero", 4, "e32", "m1", "tu", "ma"]}
    @{[vle32_v $V20, $XIP]}

    # Prepare the AES ctr input into v28.
    # The ctr data uses big-endian form.
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "mu"]}
    @{[vmv_v_v $V28, $V12]}
    @{[vrev8_v $V28, $V28, $CTR_MASK]}
2:
___

    return $code;
}

sub prepare_input_and_ctr {
    my $PTR_OFFSET_REG = shift;

    my $code=<<___;
    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "mu"]}
    # Increase ctr in v12.
    @{[vadd_vx $V12, $V12, $CTR, $CTR_MASK]}
    sub $LEN32, $LEN32, $FULL_BLOCK_LEN32
    # Load plaintext into v24
    @{[vsetvli "zero", "zero", "e32", "m4", "ta", "ma"]}
    @{[vle32_v $V24, $INP]}
    # Prepare the AES ctr input into v28.
    # The ctr data uses big-endian form.
    @{[vmv_v_v $V28, $V12]}
    add $INP, $INP, $PTR_OFFSET_REG
    @{[vsetvli "zero", "zero", "e32", "m4", "ta", "mu"]}
    @{[vrev8_v $V28, $V28, $CTR_MASK]}
___

    return $code;
}

# Store the current CTR back to IV buffer.
sub store_current_ctr {
    my $code=<<___;
    @{[vsetivli "zero", 4, "e32", "m4", "ta", "ma"]}
    # Update current ctr value to v12
    @{[vadd_vx $V12, $V12, $CTR, $CTR_MASK]}
    # Convert ctr to big-endian counter.
    @{[vrev8_v $V12, $V12, $CTR_MASK]}
    @{[vse32_v $V12, $IVP, $CTR_MASK]}
___

    return $code;
}

# Compute the final tag into v0 from the partial tag v20.
sub compute_final_tag {
    my $TMP_REG = shift;

    my $code=<<___;
    # The H is at `gcm128_context.Htable[0]` (addr(Xi)+16*2).
    # Load H to v1
    addi $TMP_REG, $XIP, 32
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V1, $TMP_REG]}
    # Multiply H for each partial tag and XOR them together.
    # Handle 1st partial tag
    @{[vmv_v_v $V0, $V20]}
    @{[vgmul_vv $V0, $V1]}
    # Handle 2nd to N-th partial tags
    li $TMP_REG, 4
1:
    @{[vsetivli "zero", 4, "e32", "m4", "ta", "ma"]}
    @{[vslidedown_vx $V4, $V20, $TMP_REG]}
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vghsh_vv $V0, $V1, $V4]}
    addi $TMP_REG, $TMP_REG, 4
    blt $TMP_REG, $FULL_BLOCK_LEN32, 1b
___

    return $code;
}

################################################################################
# size_t rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt(const unsigned char *in,
#                                               unsigned char *out, size_t len,
#                                               const void *key,
#                                               unsigned char ivec[16], u64 *Xi);
{
$code .= <<___;
.p2align 3
.globl rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt
.type rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt,\@function
rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt:
    srli $T0, $LEN, 4
    beqz $T0, .Lenc_end
    slli $LEN32, $T0, 2

    mv $ORIGINAL_LEN32, $LEN32

    @{[aes_gcm_init]}

    # Load number of rounds
    lwu $T0, 240($KEYP)
    li $T1, 14
    li $T2, 12
    li $T3, 10

    beq $T0, $T1, aes_gcm_enc_blocks_256
    beq $T0, $T2, aes_gcm_enc_blocks_192
    beq $T0, $T3, aes_gcm_enc_blocks_128

.Lenc_end:
    li $PROCESSED_LEN, 0
    ret

.size rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt,.-rv64i_zvkb_zvkg_zvkned_aes_gcm_encrypt
___

$code .= <<___;
.p2align 3
aes_gcm_enc_blocks_128:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_128_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Lenc_blocks_128:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Lenc_blocks_128_end
    @{[vghsh_vv $V20, $V16, $V28]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_128_cipher_body]}

    # Compute AES ctr ciphertext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store ciphertext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Lenc_blocks_128
.Lenc_blocks_128_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V28]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_enc_blocks_128,.-aes_gcm_enc_blocks_128
___

$code .= <<___;
.p2align 3
aes_gcm_enc_blocks_192:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_192_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Lenc_blocks_192:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Lenc_blocks_192_end
    @{[vghsh_vv $V20, $V16, $V28]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_192_cipher_body $T1]}

    # Compute AES ctr ciphertext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store ciphertext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Lenc_blocks_192
.Lenc_blocks_192_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V28]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_enc_blocks_192,.-aes_gcm_enc_blocks_192
___

$code .= <<___;
.p2align 3
aes_gcm_enc_blocks_256:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_256_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Lenc_blocks_256:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Lenc_blocks_256_end
    @{[vghsh_vv $V20, $V16, $V28]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_256_cipher_body $T1]}

    # Compute AES ctr ciphertext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store ciphertext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Lenc_blocks_256
.Lenc_blocks_256_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V28]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_enc_blocks_256,.-aes_gcm_enc_blocks_256
___

}

################################################################################
# size_t rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt(const unsigned char *in,
#                                               unsigned char *out, size_t len,
#                                               const void *key,
#                                               unsigned char ivec[16], u64 *Xi);
{
$code .= <<___;
.p2align 3
.globl rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt
.type rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt,\@function
rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt:
    srli $T0, $LEN, 4
    beqz $T0, .Ldec_end
    slli $LEN32, $T0, 2

    mv $ORIGINAL_LEN32, $LEN32

    @{[aes_gcm_init]}

    # Load number of rounds
    lwu $T0, 240($KEYP)
    li $T1, 14
    li $T2, 12
    li $T3, 10

    beq $T0, $T1, aes_gcm_dec_blocks_256
    beq $T0, $T2, aes_gcm_dec_blocks_192
    beq $T0, $T3, aes_gcm_dec_blocks_128

.Ldec_end:
    li $PROCESSED_LEN, 0
    ret
.size rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt,.-rv64i_zvkb_zvkg_zvkned_aes_gcm_decrypt
___

$code .= <<___;
.p2align 3
aes_gcm_dec_blocks_128:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_128_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Ldec_blocks_128:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Ldec_blocks_256_end
    @{[vghsh_vv $V20, $V16, $V24]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_128_cipher_body]}

    # Compute AES ctr plaintext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store plaintext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Ldec_blocks_128
.Ldec_blocks_128_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V24]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_dec_blocks_128,.-aes_gcm_dec_blocks_128
___

$code .= <<___;
.p2align 3
aes_gcm_dec_blocks_192:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_192_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Ldec_blocks_192:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Ldec_blocks_192_end
    @{[vghsh_vv $V20, $V16, $V24]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_192_cipher_body $T1]}

    # Compute AES ctr plaintext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store plaintext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Ldec_blocks_192
.Ldec_blocks_192_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V24]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_dec_blocks_192,.-aes_gcm_dec_blocks_192
___

$code .= <<___;
.p2align 3
aes_gcm_dec_blocks_256:
    srli $CTR, $FULL_BLOCK_LEN32, 2
    slli $T0, $FULL_BLOCK_LEN32, 2

    @{[aes_256_first_round $T0, $T1]}

    @{[vsetvli "zero", $FULL_BLOCK_LEN32, "e32", "m4", "ta", "ma"]}

.Ldec_blocks_256:
    # Compute the partial tags.
    # The partial tags will multiply with [H^n, H^n, ..., H^n]
    #   [tag0, tag1, ...] =
    #     ([tag0, tag1, ...] + [ciphertext0, ciphertext1, ...] * [H^n, H^n, ..., H^n]
    # We will skip the [H^n, H^n, ..., H^n] multiplication for the last round.
    beqz $LEN32, .Ldec_blocks_256_end
    @{[vghsh_vv $V20, $V16, $V24]}

    @{[prepare_input_and_ctr $T0]}

    @{[aes_256_cipher_body $T1]}

    # Compute AES ctr plaintext result.
    @{[vxor_vv $V28, $V28, $V24]}

    # Store plaintext
    @{[vse32_v $V28, $OUTP]}
    add $OUTP, $OUTP, $T0

    j .Ldec_blocks_256
.Ldec_blocks_256_end:

    # Add ciphertext into partial tag
    @{[vxor_vv $V20, $V20, $V24]}

    @{[store_current_ctr]}

    @{[compute_final_tag $T1]}

    # Save the final tag
    @{[vse32_v $V0, $XIP]}

    # return the processed size.
    slli $PROCESSED_LEN, $ORIGINAL_LEN32, 2
    ret
.size aes_gcm_dec_blocks_256,.-aes_gcm_dec_blocks_256
___

}
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
