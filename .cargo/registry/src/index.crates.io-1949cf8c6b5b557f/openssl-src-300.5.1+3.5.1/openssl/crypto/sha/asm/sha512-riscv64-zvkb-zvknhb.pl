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

# The generated code of this file depends on the following RISC-V extensions:
# - RV64I
# - RISC-V vector ('V') with VLEN >= 128
# - RISC-V Vector Cryptography Bit-manipulation extension ('Zvkb')
# - RISC-V Vector SHA-2 Secure Hash extension ('Zvknhb')

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

my $K512 = "K512";

# Function arguments
my ($H, $INP, $LEN, $KT, $H2, $INDEX_PATTERN) = ("a0", "a1", "a2", "a3", "t3", "t4");

################################################################################
# void sha512_block_data_order_zvkb_zvknhb(void *c, const void *p, size_t len)
$code .= <<___;
.p2align 2
.globl sha512_block_data_order_zvkb_zvknhb
.type sha512_block_data_order_zvkb_zvknhb,\@function
sha512_block_data_order_zvkb_zvknhb:
    @{[vsetivli "zero", 4, "e64", "m2", "ta", "ma"]}

    # H is stored as {a,b,c,d},{e,f,g,h}, but we need {f,e,b,a},{h,g,d,c}
    # The dst vtype is e64m2 and the index vtype is e8mf4.
    # We use index-load with the following index pattern at v1.
    #   i8 index:
    #     40, 32, 8, 0
    # Instead of setting the i8 index, we could use a single 32bit
    # little-endian value to cover the 4xi8 index.
    #   i32 value:
    #     0x 00 08 20 28
    li $INDEX_PATTERN, 0x00082028
    @{[vsetivli "zero", 1, "e32", "m1", "ta", "ma"]}
    @{[vmv_v_x $V1, $INDEX_PATTERN]}

    addi $H2, $H, 16

    # Use index-load to get {f,e,b,a},{h,g,d,c}
    @{[vsetivli "zero", 4, "e64", "m2", "ta", "ma"]}
    @{[vluxei8_v $V22, $H, $V1]}
    @{[vluxei8_v $V24, $H2, $V1]}

    # Setup v0 mask for the vmerge to replace the first word (idx==0) in key-scheduling.
    # The AVL is 4 in SHA, so we could use a single e8(8 element masking) for masking.
    @{[vsetivli "zero", 1, "e8", "m1", "ta", "ma"]}
    @{[vmv_v_i $V0, 0x01]}

    @{[vsetivli "zero", 4, "e64", "m2", "ta", "ma"]}

L_round_loop:
    # Load round constants K512
    la $KT, $K512

    # Decrement length by 1
    addi $LEN, $LEN, -1

    # Keep the current state as we need it later: H' = H+{a',b',c',...,h'}.
    @{[vmv_v_v $V26, $V22]}
    @{[vmv_v_v $V28, $V24]}

    # Load the 1024-bits of the message block in v10-v16 and perform the endian
    # swap.
    @{[vle64_v $V10, $INP]}
    @{[vrev8_v $V10, $V10]}
    addi $INP, $INP, 32
    @{[vle64_v $V12, $INP]}
    @{[vrev8_v $V12, $V12]}
    addi $INP, $INP, 32
    @{[vle64_v $V14, $INP]}
    @{[vrev8_v $V14, $V14]}
    addi $INP, $INP, 32
    @{[vle64_v $V16, $INP]}
    @{[vrev8_v $V16, $V16]}
    addi $INP, $INP, 32

    .rept 4
    # Quad-round 0 (+0, v10->v12->v14->v16)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V10]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}
    @{[vmerge_vvm $V18, $V14, $V12, $V0]}
    @{[vsha2ms_vv $V10, $V18, $V16]}

    # Quad-round 1 (+1, v12->v14->v16->v10)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V12]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}
    @{[vmerge_vvm $V18, $V16, $V14, $V0]}
    @{[vsha2ms_vv $V12, $V18, $V10]}

    # Quad-round 2 (+2, v14->v16->v10->v12)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V14]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}
    @{[vmerge_vvm $V18, $V10, $V16, $V0]}
    @{[vsha2ms_vv $V14, $V18, $V12]}

    # Quad-round 3 (+3, v16->v10->v12->v14)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V16]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}
    @{[vmerge_vvm $V18, $V12, $V10, $V0]}
    @{[vsha2ms_vv $V16, $V18, $V14]}
    .endr

    # Quad-round 16 (+0, v10->v12->v14->v16)
    # Note that we stop generating new message schedule words (Wt, v10-16)
    # as we already generated all the words we end up consuming (i.e., W[79:76]).
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V10]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}

    # Quad-round 17 (+1, v12->v14->v16->v10)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V12]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}

    # Quad-round 18 (+2, v14->v16->v10->v12)
    @{[vle64_v $V20, ($KT)]}
    addi $KT, $KT, 32
    @{[vadd_vv $V18, $V20, $V14]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}

    # Quad-round 19 (+3, v16->v10->v12->v14)
    @{[vle64_v $V20, ($KT)]}
    # No t1 increment needed.
    @{[vadd_vv $V18, $V20, $V16]}
    @{[vsha2cl_vv $V24, $V22, $V18]}
    @{[vsha2ch_vv $V22, $V24, $V18]}

    # H' = H+{a',b',c',...,h'}
    @{[vadd_vv $V22, $V26, $V22]}
    @{[vadd_vv $V24, $V28, $V24]}
    bnez $LEN, L_round_loop

    # Store {f,e,b,a},{h,g,d,c} back to {a,b,c,d},{e,f,g,h}.
    @{[vsuxei8_v $V22, ($H), $V1]}
    @{[vsuxei8_v $V24, ($H2), $V1]}

    ret
.size sha512_block_data_order_zvkb_zvknhb,.-sha512_block_data_order_zvkb_zvknhb

.p2align 3
.type $K512,\@object
$K512:
    .dword 0x428a2f98d728ae22, 0x7137449123ef65cd
    .dword 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc
    .dword 0x3956c25bf348b538, 0x59f111f1b605d019
    .dword 0x923f82a4af194f9b, 0xab1c5ed5da6d8118
    .dword 0xd807aa98a3030242, 0x12835b0145706fbe
    .dword 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2
    .dword 0x72be5d74f27b896f, 0x80deb1fe3b1696b1
    .dword 0x9bdc06a725c71235, 0xc19bf174cf692694
    .dword 0xe49b69c19ef14ad2, 0xefbe4786384f25e3
    .dword 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65
    .dword 0x2de92c6f592b0275, 0x4a7484aa6ea6e483
    .dword 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5
    .dword 0x983e5152ee66dfab, 0xa831c66d2db43210
    .dword 0xb00327c898fb213f, 0xbf597fc7beef0ee4
    .dword 0xc6e00bf33da88fc2, 0xd5a79147930aa725
    .dword 0x06ca6351e003826f, 0x142929670a0e6e70
    .dword 0x27b70a8546d22ffc, 0x2e1b21385c26c926
    .dword 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df
    .dword 0x650a73548baf63de, 0x766a0abb3c77b2a8
    .dword 0x81c2c92e47edaee6, 0x92722c851482353b
    .dword 0xa2bfe8a14cf10364, 0xa81a664bbc423001
    .dword 0xc24b8b70d0f89791, 0xc76c51a30654be30
    .dword 0xd192e819d6ef5218, 0xd69906245565a910
    .dword 0xf40e35855771202a, 0x106aa07032bbd1b8
    .dword 0x19a4c116b8d2d0c8, 0x1e376c085141ab53
    .dword 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8
    .dword 0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb
    .dword 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3
    .dword 0x748f82ee5defb2fc, 0x78a5636f43172f60
    .dword 0x84c87814a1f0ab72, 0x8cc702081a6439ec
    .dword 0x90befffa23631e28, 0xa4506cebde82bde9
    .dword 0xbef9a3f7b2c67915, 0xc67178f2e372532b
    .dword 0xca273eceea26619c, 0xd186b8c721c0c207
    .dword 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178
    .dword 0x06f067aa72176fba, 0x0a637dc5a2c898a6
    .dword 0x113f9804bef90dae, 0x1b710b35131c471b
    .dword 0x28db77f523047d84, 0x32caab7b40c72493
    .dword 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c
    .dword 0x4cc5d4becb3e42b6, 0x597f299cfc657e2a
    .dword 0x5fcb6fab3ad6faec, 0x6c44198c4a475817
.size $K512,.-$K512
___

print $code;

close STDOUT or die "error closing STDOUT: $!";
