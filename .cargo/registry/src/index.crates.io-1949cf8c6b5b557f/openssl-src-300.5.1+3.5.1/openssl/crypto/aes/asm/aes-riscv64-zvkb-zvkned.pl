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
# - RISC-V Vector AES block cipher extension ('Zvkned')
# - RISC-V Zicclsm(Main memory supports misaligned loads/stores)

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

################################################################################
# void rv64i_zvkb_zvkned_ctr32_encrypt_blocks(const unsigned char *in,
#                                             unsigned char *out, size_t blocks,
#                                             const void *key,
#                                             const unsigned char ivec[16]);
{
my ($INP, $OUTP, $BLOCK_NUM, $KEYP, $IVP) = ("a0", "a1", "a2", "a3", "a4");
my ($T0, $T1, $T2, $T3) = ("t0", "t1", "t2", "t3");
my ($VL) = ("t4");
my ($LEN32) = ("t5");
my ($CTR) = ("t6");
my ($MASK) = ("v0");
my ($V0, $V1, $V2, $V3, $V4, $V5, $V6, $V7,
    $V8, $V9, $V10, $V11, $V12, $V13, $V14, $V15,
    $V16, $V17, $V18, $V19, $V20, $V21, $V22, $V23,
    $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map("v$_",(0..31));

# Prepare the AES ctr input data into v16.
sub init_aes_ctr_input {
    my $code=<<___;
    # Setup mask into v0
    # The mask pattern for 4*N-th elements
    # mask v0: [000100010001....]
    # Note:
    #   We could setup the mask just for the maximum element length instead of
    #   the VLMAX.
    li $T0, 0b10001000
    @{[vsetvli $T2, "zero", "e8", "m1", "ta", "ma"]}
    @{[vmv_v_x $MASK, $T0]}
    # Load IV.
    # v31:[IV0, IV1, IV2, big-endian count]
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V31, $IVP]}
    # Convert the big-endian counter into little-endian.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "mu"]}
    @{[vrev8_v $V31, $V31, $MASK]}
    # Splat the IV to v16
    @{[vsetvli "zero", $LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V16, 0]}
    @{[vaesz_vs $V16, $V31]}
    # Prepare the ctr pattern into v20
    # v20: [x, x, x, 0, x, x, x, 1, x, x, x, 2, ...]
    @{[viota_m $V20, $MASK, $MASK]}
    # v16:[IV0, IV1, IV2, count+0, IV0, IV1, IV2, count+1, ...]
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "mu"]}
    @{[vadd_vv $V16, $V16, $V20, $MASK]}
___

    return $code;
}

$code .= <<___;
.p2align 3
.globl rv64i_zvkb_zvkned_ctr32_encrypt_blocks
.type rv64i_zvkb_zvkned_ctr32_encrypt_blocks,\@function
rv64i_zvkb_zvkned_ctr32_encrypt_blocks:
    beqz $BLOCK_NUM, 1f

    # Load number of rounds
    lwu $T0, 240($KEYP)
    li $T1, 14
    li $T2, 12
    li $T3, 10

    slli $LEN32, $BLOCK_NUM, 2

    beq $T0, $T1, ctr32_encrypt_blocks_256
    beq $T0, $T2, ctr32_encrypt_blocks_192
    beq $T0, $T3, ctr32_encrypt_blocks_128

1:
    ret

.size rv64i_zvkb_zvkned_ctr32_encrypt_blocks,.-rv64i_zvkb_zvkned_ctr32_encrypt_blocks
___

$code .= <<___;
.p2align 3
ctr32_encrypt_blocks_128:
    # Load all 11 round keys to v1-v11 registers.
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

    @{[init_aes_ctr_input]}

    ##### AES body
    j 2f
1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "mu"]}
    # Increase ctr in v16.
    @{[vadd_vx $V16, $V16, $CTR, $MASK]}
2:
    # Load plaintext into v20
    @{[vle32_v $V20, $INP]}
    slli $T0, $VL, 2
    srli $CTR, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INP, $INP, $T0
    # Prepare the AES ctr input into v24.
    # The ctr data uses big-endian form.
    @{[vmv_v_v $V24, $V16]}
    @{[vrev8_v $V24, $V24, $MASK]}

    @{[vaesz_vs $V24, $V1]}
    @{[vaesem_vs $V24, $V2]}
    @{[vaesem_vs $V24, $V3]}
    @{[vaesem_vs $V24, $V4]}
    @{[vaesem_vs $V24, $V5]}
    @{[vaesem_vs $V24, $V6]}
    @{[vaesem_vs $V24, $V7]}
    @{[vaesem_vs $V24, $V8]}
    @{[vaesem_vs $V24, $V9]}
    @{[vaesem_vs $V24, $V10]}
    @{[vaesef_vs $V24, $V11]}

    # ciphertext
    @{[vxor_vv $V24, $V24, $V20]}

    # Store the ciphertext.
    @{[vse32_v $V24, $OUTP]}
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size ctr32_encrypt_blocks_128,.-ctr32_encrypt_blocks_128
___

$code .= <<___;
.p2align 3
ctr32_encrypt_blocks_192:
    # Load all 13 round keys to v1-v13 registers.
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
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V12, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V13, $KEYP]}

    @{[init_aes_ctr_input]}

    ##### AES body
    j 2f
1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "mu"]}
    # Increase ctr in v16.
    @{[vadd_vx $V16, $V16, $CTR, $MASK]}
2:
    # Load plaintext into v20
    @{[vle32_v $V20, $INP]}
    slli $T0, $VL, 2
    srli $CTR, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INP, $INP, $T0
    # Prepare the AES ctr input into v24.
    # The ctr data uses big-endian form.
    @{[vmv_v_v $V24, $V16]}
    @{[vrev8_v $V24, $V24, $MASK]}

    @{[vaesz_vs $V24, $V1]}
    @{[vaesem_vs $V24, $V2]}
    @{[vaesem_vs $V24, $V3]}
    @{[vaesem_vs $V24, $V4]}
    @{[vaesem_vs $V24, $V5]}
    @{[vaesem_vs $V24, $V6]}
    @{[vaesem_vs $V24, $V7]}
    @{[vaesem_vs $V24, $V8]}
    @{[vaesem_vs $V24, $V9]}
    @{[vaesem_vs $V24, $V10]}
    @{[vaesem_vs $V24, $V11]}
    @{[vaesem_vs $V24, $V12]}
    @{[vaesef_vs $V24, $V13]}

    # ciphertext
    @{[vxor_vv $V24, $V24, $V20]}

    # Store the ciphertext.
    @{[vse32_v $V24, $OUTP]}
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size ctr32_encrypt_blocks_192,.-ctr32_encrypt_blocks_192
___

$code .= <<___;
.p2align 3
ctr32_encrypt_blocks_256:
    # Load all 15 round keys to v1-v15 registers.
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
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V12, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V13, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V14, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V15, $KEYP]}

    @{[init_aes_ctr_input]}

    ##### AES body
    j 2f
1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "mu"]}
    # Increase ctr in v16.
    @{[vadd_vx $V16, $V16, $CTR, $MASK]}
2:
    # Load plaintext into v20
    @{[vle32_v $V20, $INP]}
    slli $T0, $VL, 2
    srli $CTR, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INP, $INP, $T0
    # Prepare the AES ctr input into v24.
    # The ctr data uses big-endian form.
    @{[vmv_v_v $V24, $V16]}
    @{[vrev8_v $V24, $V24, $MASK]}

    @{[vaesz_vs $V24, $V1]}
    @{[vaesem_vs $V24, $V2]}
    @{[vaesem_vs $V24, $V3]}
    @{[vaesem_vs $V24, $V4]}
    @{[vaesem_vs $V24, $V5]}
    @{[vaesem_vs $V24, $V6]}
    @{[vaesem_vs $V24, $V7]}
    @{[vaesem_vs $V24, $V8]}
    @{[vaesem_vs $V24, $V9]}
    @{[vaesem_vs $V24, $V10]}
    @{[vaesem_vs $V24, $V11]}
    @{[vaesem_vs $V24, $V12]}
    @{[vaesem_vs $V24, $V13]}
    @{[vaesem_vs $V24, $V14]}
    @{[vaesef_vs $V24, $V15]}

    # ciphertext
    @{[vxor_vv $V24, $V24, $V20]}

    # Store the ciphertext.
    @{[vse32_v $V24, $OUTP]}
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size ctr32_encrypt_blocks_256,.-ctr32_encrypt_blocks_256
___
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
