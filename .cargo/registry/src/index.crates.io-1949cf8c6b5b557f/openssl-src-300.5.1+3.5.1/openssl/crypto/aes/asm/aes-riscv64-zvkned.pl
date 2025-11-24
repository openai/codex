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
# Copyright (c) 2023, Christoph Müllner <christoph.muellner@vrull.eu>
# Copyright (c) 2023, Phoebe Chen <phoebe.chen@sifive.com>
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
# - RISC-V Vector AES block cipher extension ('Zvkned')

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

my ($V0, $V1, $V2, $V3, $V4, $V5, $V6, $V7,
    $V8, $V9, $V10, $V11, $V12, $V13, $V14, $V15,
    $V16, $V17, $V18, $V19, $V20, $V21, $V22, $V23,
    $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map("v$_",(0..31));

# Load all 11 round keys to v1-v11 registers.
sub aes_128_load_key {
    my $KEYP = shift;

    my $code=<<___;
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
___

    return $code;
}

# Load all 13 round keys to v1-v13 registers.
sub aes_192_load_key {
    my $KEYP = shift;

    my $code=<<___;
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
___

    return $code;
}

# Load all 15 round keys to v1-v15 registers.
sub aes_256_load_key {
    my $KEYP = shift;

    my $code=<<___;
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
___

    return $code;
}

# aes-128 encryption with round keys v1-v11
sub aes_128_encrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V1]}     # with round key w[ 0, 3]
    @{[vaesem_vs $V24, $V2]}    # with round key w[ 4, 7]
    @{[vaesem_vs $V24, $V3]}    # with round key w[ 8,11]
    @{[vaesem_vs $V24, $V4]}    # with round key w[12,15]
    @{[vaesem_vs $V24, $V5]}    # with round key w[16,19]
    @{[vaesem_vs $V24, $V6]}    # with round key w[20,23]
    @{[vaesem_vs $V24, $V7]}    # with round key w[24,27]
    @{[vaesem_vs $V24, $V8]}    # with round key w[28,31]
    @{[vaesem_vs $V24, $V9]}    # with round key w[32,35]
    @{[vaesem_vs $V24, $V10]}   # with round key w[36,39]
    @{[vaesef_vs $V24, $V11]}   # with round key w[40,43]
___

    return $code;
}

# aes-128 decryption with round keys v1-v11
sub aes_128_decrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V11]}   # with round key w[40,43]
    @{[vaesdm_vs $V24, $V10]}  # with round key w[36,39]
    @{[vaesdm_vs $V24, $V9]}   # with round key w[32,35]
    @{[vaesdm_vs $V24, $V8]}   # with round key w[28,31]
    @{[vaesdm_vs $V24, $V7]}   # with round key w[24,27]
    @{[vaesdm_vs $V24, $V6]}   # with round key w[20,23]
    @{[vaesdm_vs $V24, $V5]}   # with round key w[16,19]
    @{[vaesdm_vs $V24, $V4]}   # with round key w[12,15]
    @{[vaesdm_vs $V24, $V3]}   # with round key w[ 8,11]
    @{[vaesdm_vs $V24, $V2]}   # with round key w[ 4, 7]
    @{[vaesdf_vs $V24, $V1]}   # with round key w[ 0, 3]
___

    return $code;
}

# aes-192 encryption with round keys v1-v13
sub aes_192_encrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V1]}     # with round key w[ 0, 3]
    @{[vaesem_vs $V24, $V2]}    # with round key w[ 4, 7]
    @{[vaesem_vs $V24, $V3]}    # with round key w[ 8,11]
    @{[vaesem_vs $V24, $V4]}    # with round key w[12,15]
    @{[vaesem_vs $V24, $V5]}    # with round key w[16,19]
    @{[vaesem_vs $V24, $V6]}    # with round key w[20,23]
    @{[vaesem_vs $V24, $V7]}    # with round key w[24,27]
    @{[vaesem_vs $V24, $V8]}    # with round key w[28,31]
    @{[vaesem_vs $V24, $V9]}    # with round key w[32,35]
    @{[vaesem_vs $V24, $V10]}   # with round key w[36,39]
    @{[vaesem_vs $V24, $V11]}   # with round key w[40,43]
    @{[vaesem_vs $V24, $V12]}   # with round key w[44,47]
    @{[vaesef_vs $V24, $V13]}   # with round key w[48,51]
___

    return $code;
}

# aes-192 decryption with round keys v1-v13
sub aes_192_decrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V13]}    # with round key w[48,51]
    @{[vaesdm_vs $V24, $V12]}   # with round key w[44,47]
    @{[vaesdm_vs $V24, $V11]}   # with round key w[40,43]
    @{[vaesdm_vs $V24, $V10]}   # with round key w[36,39]
    @{[vaesdm_vs $V24, $V9]}    # with round key w[32,35]
    @{[vaesdm_vs $V24, $V8]}    # with round key w[28,31]
    @{[vaesdm_vs $V24, $V7]}    # with round key w[24,27]
    @{[vaesdm_vs $V24, $V6]}    # with round key w[20,23]
    @{[vaesdm_vs $V24, $V5]}    # with round key w[16,19]
    @{[vaesdm_vs $V24, $V4]}    # with round key w[12,15]
    @{[vaesdm_vs $V24, $V3]}    # with round key w[ 8,11]
    @{[vaesdm_vs $V24, $V2]}    # with round key w[ 4, 7]
    @{[vaesdf_vs $V24, $V1]}    # with round key w[ 0, 3]
___

    return $code;
}

# aes-256 encryption with round keys v1-v15
sub aes_256_encrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V1]}     # with round key w[ 0, 3]
    @{[vaesem_vs $V24, $V2]}    # with round key w[ 4, 7]
    @{[vaesem_vs $V24, $V3]}    # with round key w[ 8,11]
    @{[vaesem_vs $V24, $V4]}    # with round key w[12,15]
    @{[vaesem_vs $V24, $V5]}    # with round key w[16,19]
    @{[vaesem_vs $V24, $V6]}    # with round key w[20,23]
    @{[vaesem_vs $V24, $V7]}    # with round key w[24,27]
    @{[vaesem_vs $V24, $V8]}    # with round key w[28,31]
    @{[vaesem_vs $V24, $V9]}    # with round key w[32,35]
    @{[vaesem_vs $V24, $V10]}   # with round key w[36,39]
    @{[vaesem_vs $V24, $V11]}   # with round key w[40,43]
    @{[vaesem_vs $V24, $V12]}   # with round key w[44,47]
    @{[vaesem_vs $V24, $V13]}   # with round key w[48,51]
    @{[vaesem_vs $V24, $V14]}   # with round key w[52,55]
    @{[vaesef_vs $V24, $V15]}   # with round key w[56,59]
___

    return $code;
}

# aes-256 decryption with round keys v1-v15
sub aes_256_decrypt {
    my $code=<<___;
    @{[vaesz_vs $V24, $V15]}    # with round key w[56,59]
    @{[vaesdm_vs $V24, $V14]}   # with round key w[52,55]
    @{[vaesdm_vs $V24, $V13]}   # with round key w[48,51]
    @{[vaesdm_vs $V24, $V12]}   # with round key w[44,47]
    @{[vaesdm_vs $V24, $V11]}   # with round key w[40,43]
    @{[vaesdm_vs $V24, $V10]}   # with round key w[36,39]
    @{[vaesdm_vs $V24, $V9]}    # with round key w[32,35]
    @{[vaesdm_vs $V24, $V8]}    # with round key w[28,31]
    @{[vaesdm_vs $V24, $V7]}    # with round key w[24,27]
    @{[vaesdm_vs $V24, $V6]}    # with round key w[20,23]
    @{[vaesdm_vs $V24, $V5]}    # with round key w[16,19]
    @{[vaesdm_vs $V24, $V4]}    # with round key w[12,15]
    @{[vaesdm_vs $V24, $V3]}    # with round key w[ 8,11]
    @{[vaesdm_vs $V24, $V2]}    # with round key w[ 4, 7]
    @{[vaesdf_vs $V24, $V1]}    # with round key w[ 0, 3]
___

    return $code;
}

{
###############################################################################
# void rv64i_zvkned_cbc_encrypt(const unsigned char *in, unsigned char *out,
#                               size_t length, const AES_KEY *key,
#                               unsigned char *ivec, const int enc);
my ($INP, $OUTP, $LEN, $KEYP, $IVP, $ENC) = ("a0", "a1", "a2", "a3", "a4", "a5");
my ($T0, $T1, $ROUNDS) = ("t0", "t1", "t2");

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_cbc_encrypt
.type rv64i_zvkned_cbc_encrypt,\@function
rv64i_zvkned_cbc_encrypt:
    # check whether the length is a multiple of 16 and >= 16
    li $T1, 16
    blt $LEN, $T1, L_end
    andi $T1, $LEN, 15
    bnez $T1, L_end

    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T0, 10
    beq $ROUNDS, $T0, L_cbc_enc_128

    li $T0, 12
    beq $ROUNDS, $T0, L_cbc_enc_192

    li $T0, 14
    beq $ROUNDS, $T0, L_cbc_enc_256

    ret
.size rv64i_zvkned_cbc_encrypt,.-rv64i_zvkned_cbc_encrypt
___

$code .= <<___;
.p2align 3
L_cbc_enc_128:
    # Load all 11 round keys to v1-v11 registers.
    @{[aes_128_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vxor_vv $V24, $V24, $V16]}
    j 2f

1:
    @{[vle32_v $V17, $INP]}
    @{[vxor_vv $V24, $V24, $V17]}

2:
    # AES body
    @{[aes_128_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    addi $INP, $INP, 16
    addi $OUTP, $OUTP, 16
    addi $LEN, $LEN, -16

    bnez $LEN, 1b

    @{[vse32_v $V24, $IVP]}

    ret
.size L_cbc_enc_128,.-L_cbc_enc_128
___

$code .= <<___;
.p2align 3
L_cbc_enc_192:
    # Load all 13 round keys to v1-v13 registers.
    @{[aes_192_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vxor_vv $V24, $V24, $V16]}
    j 2f

1:
    @{[vle32_v $V17, $INP]}
    @{[vxor_vv $V24, $V24, $V17]}

2:
    # AES body
    @{[aes_192_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    addi $INP, $INP, 16
    addi $OUTP, $OUTP, 16
    addi $LEN, $LEN, -16

    bnez $LEN, 1b

    @{[vse32_v $V24, $IVP]}

    ret
.size L_cbc_enc_192,.-L_cbc_enc_192
___

$code .= <<___;
.p2align 3
L_cbc_enc_256:
    # Load all 15 round keys to v1-v15 registers.
    @{[aes_256_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vxor_vv $V24, $V24, $V16]}
    j 2f

1:
    @{[vle32_v $V17, $INP]}
    @{[vxor_vv $V24, $V24, $V17]}

2:
    # AES body
    @{[aes_256_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    addi $INP, $INP, 16
    addi $OUTP, $OUTP, 16
    addi $LEN, $LEN, -16

    bnez $LEN, 1b

    @{[vse32_v $V24, $IVP]}

    ret
.size L_cbc_enc_256,.-L_cbc_enc_256
___

###############################################################################
# void rv64i_zvkned_cbc_decrypt(const unsigned char *in, unsigned char *out,
#                               size_t length, const AES_KEY *key,
#                               unsigned char *ivec, const int enc);

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_cbc_decrypt
.type rv64i_zvkned_cbc_decrypt,\@function
rv64i_zvkned_cbc_decrypt:
    # check whether the length is a multiple of 16 and >= 16
    li $T1, 16
    blt $LEN, $T1, L_end
    andi $T1, $LEN, 15
    bnez $T1, L_end

    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T0, 10
    beq $ROUNDS, $T0, L_cbc_dec_128

    li $T0, 12
    beq $ROUNDS, $T0, L_cbc_dec_192

    li $T0, 14
    beq $ROUNDS, $T0, L_cbc_dec_256

    ret
.size rv64i_zvkned_cbc_decrypt,.-rv64i_zvkned_cbc_decrypt
___

$code .= <<___;
.p2align 3
L_cbc_dec_128:
    # Load all 11 round keys to v1-v11 registers.
    @{[aes_128_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    j 2f

1:
    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    addi $OUTP, $OUTP, 16

2:
    # AES body
    @{[aes_128_decrypt]}

    @{[vxor_vv $V24, $V24, $V16]}
    @{[vse32_v $V24, $OUTP]}
    @{[vmv_v_v $V16, $V17]}

    addi $LEN, $LEN, -16
    addi $INP, $INP, 16

    bnez $LEN, 1b

    @{[vse32_v $V16, $IVP]}

    ret
.size L_cbc_dec_128,.-L_cbc_dec_128
___

$code .= <<___;
.p2align 3
L_cbc_dec_192:
    # Load all 13 round keys to v1-v13 registers.
    @{[aes_192_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    j 2f

1:
    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    addi $OUTP, $OUTP, 16

2:
    # AES body
    @{[aes_192_decrypt]}

    @{[vxor_vv $V24, $V24, $V16]}
    @{[vse32_v $V24, $OUTP]}
    @{[vmv_v_v $V16, $V17]}

    addi $LEN, $LEN, -16
    addi $INP, $INP, 16

    bnez $LEN, 1b

    @{[vse32_v $V16, $IVP]}

    ret
.size L_cbc_dec_192,.-L_cbc_dec_192
___

$code .= <<___;
.p2align 3
L_cbc_dec_256:
    # Load all 15 round keys to v1-v15 registers.
    @{[aes_256_load_key $KEYP]}

    # Load IV.
    @{[vle32_v $V16, $IVP]}

    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    j 2f

1:
    @{[vle32_v $V24, $INP]}
    @{[vmv_v_v $V17, $V24]}
    addi $OUTP, $OUTP, 16

2:
    # AES body
    @{[aes_256_decrypt]}

    @{[vxor_vv $V24, $V24, $V16]}
    @{[vse32_v $V24, $OUTP]}
    @{[vmv_v_v $V16, $V17]}

    addi $LEN, $LEN, -16
    addi $INP, $INP, 16

    bnez $LEN, 1b

    @{[vse32_v $V16, $IVP]}

    ret
.size L_cbc_dec_256,.-L_cbc_dec_256
___
}

{
###############################################################################
# void rv64i_zvkned_ecb_encrypt(const unsigned char *in, unsigned char *out,
#                               size_t length, const AES_KEY *key,
#                               const int enc);
my ($INP, $OUTP, $LEN, $KEYP, $ENC) = ("a0", "a1", "a2", "a3", "a4");
my ($REMAIN_LEN) = ("a5");
my ($VL) = ("a6");
my ($T0, $T1, $ROUNDS) = ("t0", "t1", "t2");
my ($LEN32) = ("t3");

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_ecb_encrypt
.type rv64i_zvkned_ecb_encrypt,\@function
rv64i_zvkned_ecb_encrypt:
    # Make the LEN become e32 length.
    srli $LEN32, $LEN, 2

    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T0, 10
    beq $ROUNDS, $T0, L_ecb_enc_128

    li $T0, 12
    beq $ROUNDS, $T0, L_ecb_enc_192

    li $T0, 14
    beq $ROUNDS, $T0, L_ecb_enc_256

    ret
.size rv64i_zvkned_ecb_encrypt,.-rv64i_zvkned_ecb_encrypt
___

$code .= <<___;
.p2align 3
L_ecb_enc_128:
    # Load all 11 round keys to v1-v11 registers.
    @{[aes_128_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_128_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_enc_128,.-L_ecb_enc_128
___

$code .= <<___;
.p2align 3
L_ecb_enc_192:
    # Load all 13 round keys to v1-v13 registers.
    @{[aes_192_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_192_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_enc_192,.-L_ecb_enc_192
___

$code .= <<___;
.p2align 3
L_ecb_enc_256:
    # Load all 15 round keys to v1-v15 registers.
    @{[aes_256_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_256_encrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_enc_256,.-L_ecb_enc_256
___

###############################################################################
# void rv64i_zvkned_ecb_decrypt(const unsigned char *in, unsigned char *out,
#                               size_t length, const AES_KEY *key,
#                               const int enc);

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_ecb_decrypt
.type rv64i_zvkned_ecb_decrypt,\@function
rv64i_zvkned_ecb_decrypt:
    # Make the LEN become e32 length.
    srli $LEN32, $LEN, 2

    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T0, 10
    beq $ROUNDS, $T0, L_ecb_dec_128

    li $T0, 12
    beq $ROUNDS, $T0, L_ecb_dec_192

    li $T0, 14
    beq $ROUNDS, $T0, L_ecb_dec_256

    ret
.size rv64i_zvkned_ecb_decrypt,.-rv64i_zvkned_ecb_decrypt
___

$code .= <<___;
.p2align 3
L_ecb_dec_128:
    # Load all 11 round keys to v1-v11 registers.
    @{[aes_128_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_128_decrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_dec_128,.-L_ecb_dec_128
___

$code .= <<___;
.p2align 3
L_ecb_dec_192:
    # Load all 13 round keys to v1-v13 registers.
    @{[aes_192_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_192_decrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_dec_192,.-L_ecb_dec_192
___

$code .= <<___;
.p2align 3
L_ecb_dec_256:
    # Load all 15 round keys to v1-v15 registers.
    @{[aes_256_load_key $KEYP]}

1:
    @{[vsetvli $VL, $LEN32, "e32", "m4", "ta", "ma"]}
    slli $T0, $VL, 2
    sub $LEN32, $LEN32, $VL

    @{[vle32_v $V24, $INP]}

    # AES body
    @{[aes_256_decrypt]}

    @{[vse32_v $V24, $OUTP]}

    add $INP, $INP, $T0
    add $OUTP, $OUTP, $T0

    bnez $LEN32, 1b

    ret
.size L_ecb_dec_256,.-L_ecb_dec_256
___

}

{
################################################################################
# int rv64i_zvkned_set_encrypt_key(const unsigned char *userKey, const int bits,
#                                  AES_KEY *key)
# int rv64i_zvkned_set_decrypt_key(const unsigned char *userKey, const int bits,
#                                  AES_KEY *key)
my ($UKEY,$BITS,$KEYP) = ("a0", "a1", "a2");
my ($T0,$T1,$T4) = ("t1", "t2", "t4");

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_set_encrypt_key
.type rv64i_zvkned_set_encrypt_key,\@function
rv64i_zvkned_set_encrypt_key:
    beqz $UKEY, L_fail_m1
    beqz $KEYP, L_fail_m1

    # Get proper routine for key size
    li $T0, 256
    beq $BITS, $T0, L_set_key_256
    li $T0, 128
    beq $BITS, $T0, L_set_key_128

    j L_fail_m2

.size rv64i_zvkned_set_encrypt_key,.-rv64i_zvkned_set_encrypt_key
___

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_set_decrypt_key
.type rv64i_zvkned_set_decrypt_key,\@function
rv64i_zvkned_set_decrypt_key:
    beqz $UKEY, L_fail_m1
    beqz $KEYP, L_fail_m1

    # Get proper routine for key size
    li $T0, 256
    beq $BITS, $T0, L_set_key_256
    li $T0, 128
    beq $BITS, $T0, L_set_key_128

    j L_fail_m2

.size rv64i_zvkned_set_decrypt_key,.-rv64i_zvkned_set_decrypt_key
___

$code .= <<___;
.p2align 3
L_set_key_128:
    # Store the number of rounds
    li $T1, 10
    sw $T1, 240($KEYP)

    @{[vsetivli__x0_4_e32_m1_tu_mu]}

    # Load the key
    @{[vle32_v $V10, ($UKEY)]}

    # Generate keys for round 2-11 into registers v11-v20.
    @{[vaeskf1_vi $V11, $V10, 1]}   # v11 <- rk2  (w[ 4, 7])
    @{[vaeskf1_vi $V12, $V11, 2]}   # v12 <- rk3  (w[ 8,11])
    @{[vaeskf1_vi $V13, $V12, 3]}   # v13 <- rk4  (w[12,15])
    @{[vaeskf1_vi $V14, $V13, 4]}   # v14 <- rk5  (w[16,19])
    @{[vaeskf1_vi $V15, $V14, 5]}   # v15 <- rk6  (w[20,23])
    @{[vaeskf1_vi $V16, $V15, 6]}   # v16 <- rk7  (w[24,27])
    @{[vaeskf1_vi $V17, $V16, 7]}   # v17 <- rk8  (w[28,31])
    @{[vaeskf1_vi $V18, $V17, 8]}   # v18 <- rk9  (w[32,35])
    @{[vaeskf1_vi $V19, $V18, 9]}   # v19 <- rk10 (w[36,39])
    @{[vaeskf1_vi $V20, $V19, 10]}  # v20 <- rk11 (w[40,43])

    # Store the round keys
    @{[vse32_v $V10, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V11, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V12, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V13, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V14, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V15, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V16, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V17, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V18, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V19, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V20, $KEYP]}

    li a0, 1
    ret
.size L_set_key_128,.-L_set_key_128
___

$code .= <<___;
.p2align 3
L_set_key_256:
    # Store the number of rounds
    li $T1, 14
    sw $T1, 240($KEYP)

    @{[vsetivli__x0_4_e32_m1_tu_mu]}

    # Load the key
    @{[vle32_v $V10, ($UKEY)]}
    addi $UKEY, $UKEY, 16
    @{[vle32_v $V11, ($UKEY)]}

    @{[vmv_v_v $V12, $V10]}
    @{[vaeskf2_vi $V12, $V11, 2]}
    @{[vmv_v_v $V13, $V11]}
    @{[vaeskf2_vi $V13, $V12, 3]}
    @{[vmv_v_v $V14, $V12]}
    @{[vaeskf2_vi $V14, $V13, 4]}
    @{[vmv_v_v $V15, $V13]}
    @{[vaeskf2_vi $V15, $V14, 5]}
    @{[vmv_v_v $V16, $V14]}
    @{[vaeskf2_vi $V16, $V15, 6]}
    @{[vmv_v_v $V17, $V15]}
    @{[vaeskf2_vi $V17, $V16, 7]}
    @{[vmv_v_v $V18, $V16]}
    @{[vaeskf2_vi $V18, $V17, 8]}
    @{[vmv_v_v $V19, $V17]}
    @{[vaeskf2_vi $V19, $V18, 9]}
    @{[vmv_v_v $V20, $V18]}
    @{[vaeskf2_vi $V20, $V19, 10]}
    @{[vmv_v_v $V21, $V19]}
    @{[vaeskf2_vi $V21, $V20, 11]}
    @{[vmv_v_v $V22, $V20]}
    @{[vaeskf2_vi $V22, $V21, 12]}
    @{[vmv_v_v $V23, $V21]}
    @{[vaeskf2_vi $V23, $V22, 13]}
    @{[vmv_v_v $V24, $V22]}
    @{[vaeskf2_vi $V24, $V23, 14]}

    @{[vse32_v $V10, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V11, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V12, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V13, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V14, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V15, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V16, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V17, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V18, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V19, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V20, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V21, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V22, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V23, $KEYP]}
    addi $KEYP, $KEYP, 16
    @{[vse32_v $V24, $KEYP]}

    li a0, 1
    ret
.size L_set_key_256,.-L_set_key_256
___
}

{
################################################################################
# void rv64i_zvkned_encrypt(const unsigned char *in, unsigned char *out,
#                           const AES_KEY *key);
my ($INP,$OUTP,$KEYP) = ("a0", "a1", "a2");
my ($T0,$T1, $ROUNDS, $T6) = ("a3", "a4", "t5", "t6");

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_encrypt
.type rv64i_zvkned_encrypt,\@function
rv64i_zvkned_encrypt:
    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T6, 14
    beq $ROUNDS, $T6, L_enc_256
    li $T6, 10
    beq $ROUNDS, $T6, L_enc_128
    li $T6, 12
    beq $ROUNDS, $T6, L_enc_192

    j L_fail_m2
.size rv64i_zvkned_encrypt,.-rv64i_zvkned_encrypt
___

$code .= <<___;
.p2align 3
L_enc_128:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    @{[vle32_v $V10, $KEYP]}
    @{[vaesz_vs $V1, $V10]}    # with round key w[ 0, 3]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesem_vs $V1, $V11]}   # with round key w[ 4, 7]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesem_vs $V1, $V12]}   # with round key w[ 8,11]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesem_vs $V1, $V13]}   # with round key w[12,15]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesem_vs $V1, $V14]}   # with round key w[16,19]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesem_vs $V1, $V15]}   # with round key w[20,23]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesem_vs $V1, $V16]}   # with round key w[24,27]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesem_vs $V1, $V17]}   # with round key w[28,31]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesem_vs $V1, $V18]}   # with round key w[32,35]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesem_vs $V1, $V19]}   # with round key w[36,39]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V20, $KEYP]}
    @{[vaesef_vs $V1, $V20]}   # with round key w[40,43]

    @{[vse32_v $V1, $OUTP]}

    ret
.size L_enc_128,.-L_enc_128
___

$code .= <<___;
.p2align 3
L_enc_192:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    @{[vle32_v $V10, $KEYP]}
    @{[vaesz_vs $V1, $V10]}     # with round key w[ 0, 3]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesem_vs $V1, $V11]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesem_vs $V1, $V12]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesem_vs $V1, $V13]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesem_vs $V1, $V14]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesem_vs $V1, $V15]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesem_vs $V1, $V16]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesem_vs $V1, $V17]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesem_vs $V1, $V18]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesem_vs $V1, $V19]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V20, $KEYP]}
    @{[vaesem_vs $V1, $V20]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V21, $KEYP]}
    @{[vaesem_vs $V1, $V21]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V22, $KEYP]}
    @{[vaesef_vs $V1, $V22]}

    @{[vse32_v $V1, $OUTP]}
    ret
.size L_enc_192,.-L_enc_192
___

$code .= <<___;
.p2align 3
L_enc_256:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    @{[vle32_v $V10, $KEYP]}
    @{[vaesz_vs $V1, $V10]}     # with round key w[ 0, 3]
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesem_vs $V1, $V11]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesem_vs $V1, $V12]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesem_vs $V1, $V13]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesem_vs $V1, $V14]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesem_vs $V1, $V15]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesem_vs $V1, $V16]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesem_vs $V1, $V17]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesem_vs $V1, $V18]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesem_vs $V1, $V19]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V20, $KEYP]}
    @{[vaesem_vs $V1, $V20]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V21, $KEYP]}
    @{[vaesem_vs $V1, $V21]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V22, $KEYP]}
    @{[vaesem_vs $V1, $V22]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V23, $KEYP]}
    @{[vaesem_vs $V1, $V23]}
    addi $KEYP, $KEYP, 16
    @{[vle32_v $V24, $KEYP]}
    @{[vaesef_vs $V1, $V24]}

    @{[vse32_v $V1, $OUTP]}
    ret
.size L_enc_256,.-L_enc_256
___

################################################################################
# void rv64i_zvkned_decrypt(const unsigned char *in, unsigned char *out,
#                           const AES_KEY *key);

$code .= <<___;
.p2align 3
.globl rv64i_zvkned_decrypt
.type rv64i_zvkned_decrypt,\@function
rv64i_zvkned_decrypt:
    # Load number of rounds
    lwu $ROUNDS, 240($KEYP)

    # Get proper routine for key size
    li $T6, 14
    beq $ROUNDS, $T6, L_dec_256
    li $T6, 10
    beq $ROUNDS, $T6, L_dec_128
    li $T6, 12
    beq $ROUNDS, $T6, L_dec_192

    j L_fail_m2
.size rv64i_zvkned_decrypt,.-rv64i_zvkned_decrypt
___

$code .= <<___;
.p2align 3
L_dec_128:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    addi $KEYP, $KEYP, 160
    @{[vle32_v $V20, $KEYP]}
    @{[vaesz_vs $V1, $V20]}    # with round key w[40,43]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesdm_vs $V1, $V19]}   # with round key w[36,39]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesdm_vs $V1, $V18]}   # with round key w[32,35]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesdm_vs $V1, $V17]}   # with round key w[28,31]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesdm_vs $V1, $V16]}   # with round key w[24,27]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesdm_vs $V1, $V15]}   # with round key w[20,23]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesdm_vs $V1, $V14]}   # with round key w[16,19]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesdm_vs $V1, $V13]}   # with round key w[12,15]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesdm_vs $V1, $V12]}   # with round key w[ 8,11]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesdm_vs $V1, $V11]}   # with round key w[ 4, 7]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V10, $KEYP]}
    @{[vaesdf_vs $V1, $V10]}   # with round key w[ 0, 3]

    @{[vse32_v $V1, $OUTP]}

    ret
.size L_dec_128,.-L_dec_128
___

$code .= <<___;
.p2align 3
L_dec_192:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    addi $KEYP, $KEYP, 192
    @{[vle32_v $V22, $KEYP]}
    @{[vaesz_vs $V1, $V22]}    # with round key w[48,51]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V21, $KEYP]}
    @{[vaesdm_vs $V1, $V21]}   # with round key w[44,47]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V20, $KEYP]}
    @{[vaesdm_vs $V1, $V20]}    # with round key w[40,43]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesdm_vs $V1, $V19]}   # with round key w[36,39]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesdm_vs $V1, $V18]}   # with round key w[32,35]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesdm_vs $V1, $V17]}   # with round key w[28,31]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesdm_vs $V1, $V16]}   # with round key w[24,27]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesdm_vs $V1, $V15]}   # with round key w[20,23]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesdm_vs $V1, $V14]}   # with round key w[16,19]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesdm_vs $V1, $V13]}   # with round key w[12,15]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesdm_vs $V1, $V12]}   # with round key w[ 8,11]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesdm_vs $V1, $V11]}   # with round key w[ 4, 7]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V10, $KEYP]}
    @{[vaesdf_vs $V1, $V10]}   # with round key w[ 0, 3]

    @{[vse32_v $V1, $OUTP]}

    ret
.size L_dec_192,.-L_dec_192
___

$code .= <<___;
.p2align 3
L_dec_256:
    @{[vsetivli "zero", 4, "e32", "m1", "ta", "ma"]}

    @{[vle32_v $V1, $INP]}

    addi $KEYP, $KEYP, 224
    @{[vle32_v $V24, $KEYP]}
    @{[vaesz_vs $V1, $V24]}    # with round key w[56,59]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V23, $KEYP]}
    @{[vaesdm_vs $V1, $V23]}   # with round key w[52,55]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V22, $KEYP]}
    @{[vaesdm_vs $V1, $V22]}    # with round key w[48,51]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V21, $KEYP]}
    @{[vaesdm_vs $V1, $V21]}   # with round key w[44,47]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V20, $KEYP]}
    @{[vaesdm_vs $V1, $V20]}    # with round key w[40,43]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V19, $KEYP]}
    @{[vaesdm_vs $V1, $V19]}   # with round key w[36,39]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V18, $KEYP]}
    @{[vaesdm_vs $V1, $V18]}   # with round key w[32,35]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V17, $KEYP]}
    @{[vaesdm_vs $V1, $V17]}   # with round key w[28,31]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V16, $KEYP]}
    @{[vaesdm_vs $V1, $V16]}   # with round key w[24,27]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V15, $KEYP]}
    @{[vaesdm_vs $V1, $V15]}   # with round key w[20,23]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V14, $KEYP]}
    @{[vaesdm_vs $V1, $V14]}   # with round key w[16,19]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V13, $KEYP]}
    @{[vaesdm_vs $V1, $V13]}   # with round key w[12,15]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V12, $KEYP]}
    @{[vaesdm_vs $V1, $V12]}   # with round key w[ 8,11]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V11, $KEYP]}
    @{[vaesdm_vs $V1, $V11]}   # with round key w[ 4, 7]
    addi $KEYP, $KEYP, -16
    @{[vle32_v $V10, $KEYP]}
    @{[vaesdf_vs $V1, $V10]}   # with round key w[ 0, 3]

    @{[vse32_v $V1, $OUTP]}

    ret
.size L_dec_256,.-L_dec_256
___
}

$code .= <<___;
L_fail_m1:
    li a0, -1
    ret
.size L_fail_m1,.-L_fail_m1

L_fail_m2:
    li a0, -2
    ret
.size L_fail_m2,.-L_fail_m2

L_end:
  ret
.size L_end,.-L_end
___

print $code;

close STDOUT or die "error closing STDOUT: $!";
