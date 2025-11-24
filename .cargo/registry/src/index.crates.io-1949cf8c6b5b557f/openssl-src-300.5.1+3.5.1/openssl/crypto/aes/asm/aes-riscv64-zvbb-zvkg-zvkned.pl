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
# - RISC-V Vector Bit-manipulation extension ('Zvbb')
# - RISC-V Vector GCM/GMAC extension ('Zvkg')
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

{
################################################################################
# void rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt(const unsigned char *in,
#                                             unsigned char *out, size_t length,
#                                             const AES_KEY *key1,
#                                             const AES_KEY *key2,
#                                             const unsigned char iv[16])
my ($INPUT, $OUTPUT, $LENGTH, $KEY1, $KEY2, $IV) = ("a0", "a1", "a2", "a3", "a4", "a5");
my ($TAIL_LENGTH) = ("a6");
my ($VL) = ("a7");
my ($T0, $T1, $T2) = ("t0", "t1", "t2");
my ($STORE_LEN32) = ("t3");
my ($LEN32) = ("t4");
my ($V0, $V1, $V2, $V3, $V4, $V5, $V6, $V7,
    $V8, $V9, $V10, $V11, $V12, $V13, $V14, $V15,
    $V16, $V17, $V18, $V19, $V20, $V21, $V22, $V23,
    $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map("v$_",(0..31));

sub compute_xts_iv0 {
    my $code=<<___;
    # Load number of rounds
    lwu $T0, 240($KEY2)
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V28, $IV]}
    @{[vle32_v $V29, $KEY2]}
    @{[vaesz_vs $V28, $V29]}
    addi $T0, $T0, -1
    addi $KEY2, $KEY2, 16
1:
    @{[vle32_v $V29, $KEY2]}
    @{[vaesem_vs $V28, $V29]}
    addi $T0, $T0, -1
    addi $KEY2, $KEY2, 16
    bnez $T0, 1b
    @{[vle32_v $V29, $KEY2]}
    @{[vaesef_vs $V28, $V29]}
___

    return $code;
}

# prepare input data(v24), iv(v28), bit-reversed-iv(v16), bit-reversed-iv-multiplier(v20)
sub init_first_round {
    my $code=<<___;
    # load input
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    @{[vle32_v $V24, $INPUT]}

    li $T0, 5
    # We could simplify the initialization steps if we have `block<=1`.
    blt $LEN32, $T0, 1f

    # Note: We use `vgmul` for GF(2^128) multiplication. The `vgmul` uses
    # different order of coefficients. We should use`vbrev8` to reverse the
    # data when we use `vgmul`.
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vbrev8_v $V0, $V28]}
    @{[vsetvli "zero", $LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V16, 0]}
    # v16: [r-IV0, r-IV0, ...]
    @{[vaesz_vs $V16, $V0]}

    # Prepare GF(2^128) multiplier [1, x, x^2, x^3, ...] in v8.
    slli $T0, $LEN32, 2
    @{[vsetvli "zero", $T0, "e32", "m1", "ta", "ma"]}
    # v2: [`1`, `1`, `1`, `1`, ...]
    @{[vmv_v_i $V2, 1]}
    # v3: [`0`, `1`, `2`, `3`, ...]
    @{[vid_v $V3]}
    @{[vsetvli "zero", $T0, "e64", "m2", "ta", "ma"]}
    # v4: [`1`, 0, `1`, 0, `1`, 0, `1`, 0, ...]
    @{[vzext_vf2 $V4, $V2]}
    # v6: [`0`, 0, `1`, 0, `2`, 0, `3`, 0, ...]
    @{[vzext_vf2 $V6, $V3]}
    slli $T0, $LEN32, 1
    @{[vsetvli "zero", $T0, "e32", "m2", "ta", "ma"]}
    # v8: [1<<0=1, 0, 0, 0, 1<<1=x, 0, 0, 0, 1<<2=x^2, 0, 0, 0, ...]
    @{[vwsll_vv $V8, $V4, $V6]}

    # Compute [r-IV0*1, r-IV0*x, r-IV0*x^2, r-IV0*x^3, ...] in v16
    @{[vsetvli "zero", $LEN32, "e32", "m4", "ta", "ma"]}
    @{[vbrev8_v $V8, $V8]}
    @{[vgmul_vv $V16, $V8]}

    # Compute [IV0*1, IV0*x, IV0*x^2, IV0*x^3, ...] in v28.
    # Reverse the bits order back.
    @{[vbrev8_v $V28, $V16]}

    # Prepare the x^n multiplier in v20. The `n` is the aes-xts block number
    # in a LMUL=4 register group.
    #   n = ((VLEN*LMUL)/(32*4)) = ((VLEN*4)/(32*4))
    #     = (VLEN/32)
    # We could use vsetvli with `e32, m1` to compute the `n` number.
    @{[vsetvli $T0, "zero", "e32", "m1", "ta", "ma"]}
    li $T1, 1
    sll $T0, $T1, $T0
    @{[vsetivli "zero", 2, "e64", "m1", "ta", "ma"]}
    @{[vmv_v_i $V0, 0]}
    @{[vsetivli "zero", 1, "e64", "m1", "tu", "ma"]}
    @{[vmv_v_x $V0, $T0]}
    @{[vsetivli "zero", 2, "e64", "m1", "ta", "ma"]}
    @{[vbrev8_v $V0, $V0]}
    @{[vsetvli "zero", $LEN32, "e32", "m4", "ta", "ma"]}
    @{[vmv_v_i $V20, 0]}
    @{[vaesz_vs $V20, $V0]}

    j 2f
1:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vbrev8_v $V16, $V28]}
2:
___

    return $code;
}

# prepare xts enc last block's input(v24) and iv(v28)
sub handle_xts_enc_last_block {
    my $code=<<___;
    bnez $TAIL_LENGTH, 1f
    ret
1:
    # slidedown second to last block
    addi $VL, $VL, -4
    @{[vsetivli "zero", 4, "e32", "m4", "ta", "ma"]}
    # ciphertext
    @{[vslidedown_vx $V24, $V24, $VL]}
    # multiplier
    @{[vslidedown_vx $V16, $V16, $VL]}

    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vmv_v_v $V25, $V24]}

    # load last block into v24
    # note: We should load the last block before store the second to last block
    #       for in-place operation.
    @{[vsetvli "zero", $TAIL_LENGTH, "e8", "m1", "tu", "ma"]}
    @{[vle8_v $V24, $INPUT]}

    # setup `x` multiplier with byte-reversed order
    # 0b00000010 => 0b01000000 (0x40)
    li $T0, 0x40
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vmv_v_i $V28, 0]}
    @{[vsetivli "zero", 1, "e8", "m1", "tu", "ma"]}
    @{[vmv_v_x $V28, $T0]}

    # compute IV for last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vgmul_vv $V16, $V28]}
    @{[vbrev8_v $V28, $V16]}

    # store second to last block
    @{[vsetvli "zero", $TAIL_LENGTH, "e8", "m1", "ta", "ma"]}
    @{[vse8_v $V25, $OUTPUT]}
___

    return $code;
}

# prepare xts dec second to last block's input(v24) and iv(v29) and
# last block's and iv(v28)
sub handle_xts_dec_last_block {
    my $code=<<___;
    bnez $TAIL_LENGTH, 1f
    ret
1:
    # load second to last block's ciphertext
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V24, $INPUT]}
    addi $INPUT, $INPUT, 16

    # setup `x` multiplier with byte-reversed order
    # 0b00000010 => 0b01000000 (0x40)
    li $T0, 0x40
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vmv_v_i $V20, 0]}
    @{[vsetivli "zero", 1, "e8", "m1", "tu", "ma"]}
    @{[vmv_v_x $V20, $T0]}

    beqz $LENGTH, 1f
    # slidedown third to last block
    addi $VL, $VL, -4
    @{[vsetivli "zero", 4, "e32", "m4", "ta", "ma"]}
    # multiplier
    @{[vslidedown_vx $V16, $V16, $VL]}

    # compute IV for last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vgmul_vv $V16, $V20]}
    @{[vbrev8_v $V28, $V16]}

    # compute IV for second to last block
    @{[vgmul_vv $V16, $V20]}
    @{[vbrev8_v $V29, $V16]}
    j 2f
1:
    # compute IV for second to last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vgmul_vv $V16, $V20]}
    @{[vbrev8_v $V29, $V16]}
2:
___

    return $code;
}

# Load all 11 round keys to v1-v11 registers.
sub aes_128_load_key {
    my $code=<<___;
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V1, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V2, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V3, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V4, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V5, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V6, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V7, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V8, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V9, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V10, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V11, $KEY1]}
___

    return $code;
}

# Load all 15 round keys to v1-v15 registers.
sub aes_256_load_key {
    my $code=<<___;
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vle32_v $V1, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V2, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V3, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V4, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V5, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V6, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V7, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V8, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V9, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V10, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V11, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V12, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V13, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V14, $KEY1]}
    addi $KEY1, $KEY1, 16
    @{[vle32_v $V15, $KEY1]}
___

    return $code;
}

# aes-128 enc with round keys v1-v11
sub aes_128_enc {
    my $code=<<___;
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
___

    return $code;
}

# aes-128 dec with round keys v1-v11
sub aes_128_dec {
    my $code=<<___;
    @{[vaesz_vs $V24, $V11]}
    @{[vaesdm_vs $V24, $V10]}
    @{[vaesdm_vs $V24, $V9]}
    @{[vaesdm_vs $V24, $V8]}
    @{[vaesdm_vs $V24, $V7]}
    @{[vaesdm_vs $V24, $V6]}
    @{[vaesdm_vs $V24, $V5]}
    @{[vaesdm_vs $V24, $V4]}
    @{[vaesdm_vs $V24, $V3]}
    @{[vaesdm_vs $V24, $V2]}
    @{[vaesdf_vs $V24, $V1]}
___

    return $code;
}

# aes-256 enc with round keys v1-v15
sub aes_256_enc {
    my $code=<<___;
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
___

    return $code;
}

# aes-256 dec with round keys v1-v15
sub aes_256_dec {
    my $code=<<___;
    @{[vaesz_vs $V24, $V15]}
    @{[vaesdm_vs $V24, $V14]}
    @{[vaesdm_vs $V24, $V13]}
    @{[vaesdm_vs $V24, $V12]}
    @{[vaesdm_vs $V24, $V11]}
    @{[vaesdm_vs $V24, $V10]}
    @{[vaesdm_vs $V24, $V9]}
    @{[vaesdm_vs $V24, $V8]}
    @{[vaesdm_vs $V24, $V7]}
    @{[vaesdm_vs $V24, $V6]}
    @{[vaesdm_vs $V24, $V5]}
    @{[vaesdm_vs $V24, $V4]}
    @{[vaesdm_vs $V24, $V3]}
    @{[vaesdm_vs $V24, $V2]}
    @{[vaesdf_vs $V24, $V1]}
___

    return $code;
}

$code .= <<___;
.p2align 3
.globl rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt
.type rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt,\@function
rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt:
    @{[compute_xts_iv0]}

    # aes block size is 16
    andi $TAIL_LENGTH, $LENGTH, 15
    mv $STORE_LEN32, $LENGTH
    beqz $TAIL_LENGTH, 1f
    sub $LENGTH, $LENGTH, $TAIL_LENGTH
    addi $STORE_LEN32, $LENGTH, -16
1:
    # We make the `LENGTH` become e32 length here.
    srli $LEN32, $LENGTH, 2
    srli $STORE_LEN32, $STORE_LEN32, 2

    # Load number of rounds
    lwu $T0, 240($KEY1)
    li $T1, 14
    li $T2, 10
    beq $T0, $T1, aes_xts_enc_256
    beq $T0, $T2, aes_xts_enc_128
.size rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt,.-rv64i_zvbb_zvkg_zvkned_aes_xts_encrypt
___

$code .= <<___;
.p2align 3
aes_xts_enc_128:
    @{[init_first_round]}
    @{[aes_128_load_key]}

    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    j 1f

.Lenc_blocks_128:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    # load plaintext into v24
    @{[vle32_v $V24, $INPUT]}
    # update iv
    @{[vgmul_vv $V16, $V20]}
    # reverse the iv's bits order back
    @{[vbrev8_v $V28, $V16]}
1:
    @{[vxor_vv $V24, $V24, $V28]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INPUT, $INPUT, $T0
    @{[aes_128_enc]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store ciphertext
    @{[vsetvli "zero", $STORE_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vse32_v $V24, $OUTPUT]}
    add $OUTPUT, $OUTPUT, $T0
    sub $STORE_LEN32, $STORE_LEN32, $VL

    bnez $LEN32, .Lenc_blocks_128

    @{[handle_xts_enc_last_block]}

    # xts last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V28]}
    @{[aes_128_enc]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store last block ciphertext
    addi $OUTPUT, $OUTPUT, -16
    @{[vse32_v $V24, $OUTPUT]}

    ret
.size aes_xts_enc_128,.-aes_xts_enc_128
___

$code .= <<___;
.p2align 3
aes_xts_enc_256:
    @{[init_first_round]}
    @{[aes_256_load_key]}

    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    j 1f

.Lenc_blocks_256:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    # load plaintext into v24
    @{[vle32_v $V24, $INPUT]}
    # update iv
    @{[vgmul_vv $V16, $V20]}
    # reverse the iv's bits order back
    @{[vbrev8_v $V28, $V16]}
1:
    @{[vxor_vv $V24, $V24, $V28]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INPUT, $INPUT, $T0
    @{[aes_256_enc]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store ciphertext
    @{[vsetvli "zero", $STORE_LEN32, "e32", "m4", "ta", "ma"]}
    @{[vse32_v $V24, $OUTPUT]}
    add $OUTPUT, $OUTPUT, $T0
    sub $STORE_LEN32, $STORE_LEN32, $VL

    bnez $LEN32, .Lenc_blocks_256

    @{[handle_xts_enc_last_block]}

    # xts last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V28]}
    @{[aes_256_enc]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store last block ciphertext
    addi $OUTPUT, $OUTPUT, -16
    @{[vse32_v $V24, $OUTPUT]}

    ret
.size aes_xts_enc_256,.-aes_xts_enc_256
___

################################################################################
# void rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt(const unsigned char *in,
#                                             unsigned char *out, size_t length,
#                                             const AES_KEY *key1,
#                                             const AES_KEY *key2,
#                                             const unsigned char iv[16])
$code .= <<___;
.p2align 3
.globl rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt
.type rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt,\@function
rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt:
    @{[compute_xts_iv0]}

    # aes block size is 16
    andi $TAIL_LENGTH, $LENGTH, 15
    beqz $TAIL_LENGTH, 1f
    sub $LENGTH, $LENGTH, $TAIL_LENGTH
    addi $LENGTH, $LENGTH, -16
1:
    # We make the `LENGTH` become e32 length here.
    srli $LEN32, $LENGTH, 2

    # Load number of rounds
    lwu $T0, 240($KEY1)
    li $T1, 14
    li $T2, 10
    beq $T0, $T1, aes_xts_dec_256
    beq $T0, $T2, aes_xts_dec_128
.size rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt,.-rv64i_zvbb_zvkg_zvkned_aes_xts_decrypt
___

$code .= <<___;
.p2align 3
aes_xts_dec_128:
    @{[init_first_round]}
    @{[aes_128_load_key]}

    beqz $LEN32, 2f

    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    j 1f

.Ldec_blocks_128:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    # load ciphertext into v24
    @{[vle32_v $V24, $INPUT]}
    # update iv
    @{[vgmul_vv $V16, $V20]}
    # reverse the iv's bits order back
    @{[vbrev8_v $V28, $V16]}
1:
    @{[vxor_vv $V24, $V24, $V28]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INPUT, $INPUT, $T0
    @{[aes_128_dec]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store plaintext
    @{[vse32_v $V24, $OUTPUT]}
    add $OUTPUT, $OUTPUT, $T0

    bnez $LEN32, .Ldec_blocks_128

2:
    @{[handle_xts_dec_last_block]}

    ## xts second to last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V29]}
    @{[aes_128_dec]}
    @{[vxor_vv $V24, $V24, $V29]}
    @{[vmv_v_v $V25, $V24]}

    # load last block ciphertext
    @{[vsetvli "zero", $TAIL_LENGTH, "e8", "m1", "tu", "ma"]}
    @{[vle8_v $V24, $INPUT]}

    # store second to last block plaintext
    addi $T0, $OUTPUT, 16
    @{[vse8_v $V25, $T0]}

    ## xts last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V28]}
    @{[aes_128_dec]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store second to last block plaintext
    @{[vse32_v $V24, $OUTPUT]}

    ret
.size aes_xts_dec_128,.-aes_xts_dec_128
___

$code .= <<___;
.p2align 3
aes_xts_dec_256:
    @{[init_first_round]}
    @{[aes_256_load_key]}

    beqz $LEN32, 2f

    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    j 1f

.Ldec_blocks_256:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    # load ciphertext into v24
    @{[vle32_v $V24, $INPUT]}
    # update iv
    @{[vgmul_vv $V16, $V20]}
    # reverse the iv's bits order back
    @{[vbrev8_v $V28, $V16]}
1:
    @{[vxor_vv $V24, $V24, $V28]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL
    add $INPUT, $INPUT, $T0
    @{[aes_256_dec]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store plaintext
    @{[vse32_v $V24, $OUTPUT]}
    add $OUTPUT, $OUTPUT, $T0

    bnez $LEN32, .Ldec_blocks_256

2:
    @{[handle_xts_dec_last_block]}

    ## xts second to last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V29]}
    @{[aes_256_dec]}
    @{[vxor_vv $V24, $V24, $V29]}
    @{[vmv_v_v $V25, $V24]}

    # load last block ciphertext
    @{[vsetvli "zero", $TAIL_LENGTH, "e8", "m1", "tu", "ma"]}
    @{[vle8_v $V24, $INPUT]}

    # store second to last block plaintext
    addi $T0, $OUTPUT, 16
    @{[vse8_v $V25, $T0]}

    ## xts last block
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}
    @{[vxor_vv $V24, $V24, $V28]}
    @{[aes_256_dec]}
    @{[vxor_vv $V24, $V24, $V28]}

    # store second to last block plaintext
    @{[vse32_v $V24, $OUTPUT]}

    ret
.size aes_xts_dec_256,.-aes_xts_dec_256
___
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
