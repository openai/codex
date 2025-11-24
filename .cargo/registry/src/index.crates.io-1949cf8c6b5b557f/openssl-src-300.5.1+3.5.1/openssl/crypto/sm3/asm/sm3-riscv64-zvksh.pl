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

# The generated code of this file depends on the following RISC-V extensions:
# - RV64I
# - RISC-V Vector ('V') with VLEN >= 128
# - RISC-V Vector Cryptography Bit-manipulation extension ('Zvkb')
# - RISC-V Vector SM3 Secure Hash extension ('Zvksh')

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
# ossl_hwsm3_block_data_order_zvksh(SM3_CTX *c, const void *p, size_t num);
{
my ($CTX, $INPUT, $NUM) = ("a0", "a1", "a2");
my ($V0, $V1, $V2, $V3, $V4, $V5, $V6, $V7,
    $V8, $V9, $V10, $V11, $V12, $V13, $V14, $V15,
    $V16, $V17, $V18, $V19, $V20, $V21, $V22, $V23,
    $V24, $V25, $V26, $V27, $V28, $V29, $V30, $V31,
) = map("v$_",(0..31));

$code .= <<___;
.text
.p2align 3
.globl ossl_hwsm3_block_data_order_zvksh
.type ossl_hwsm3_block_data_order_zvksh,\@function
ossl_hwsm3_block_data_order_zvksh:
    @{[vsetivli "zero", 8, "e32", "m2", "ta", "ma"]}

    # Load initial state of hash context (c->A-H).
    @{[vle32_v $V0, $CTX]}
    @{[vrev8_v $V0, $V0]}

L_sm3_loop:
    # Copy the previous state to v2.
    # It will be XOR'ed with the current state at the end of the round.
    @{[vmv_v_v $V2, $V0]}

    # Load the 64B block in 2x32B chunks.
    @{[vle32_v $V6, $INPUT]} # v6 := {w7, ..., w0}
    addi $INPUT, $INPUT, 32

    @{[vle32_v $V8, $INPUT]} # v8 := {w15, ..., w8}
    addi $INPUT, $INPUT, 32

    addi $NUM, $NUM, -1

    # As vsm3c consumes only w0, w1, w4, w5 we need to slide the input
    # 2 elements down so we process elements w2, w3, w6, w7
    # This will be repeated for each odd round.
    @{[vslidedown_vi $V4, $V6, 2]} # v4 := {X, X, w7, ..., w2}

    @{[vsm3c_vi $V0, $V6, 0]}
    @{[vsm3c_vi $V0, $V4, 1]}

    # Prepare a vector with {w11, ..., w4}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w7, ..., w4}
    @{[vslideup_vi $V4, $V8, 4]}   # v4 := {w11, w10, w9, w8, w7, w6, w5, w4}

    @{[vsm3c_vi $V0, $V4, 2]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w11, w10, w9, w8, w7, w6}
    @{[vsm3c_vi $V0, $V4, 3]}

    @{[vsm3c_vi $V0, $V8, 4]}
    @{[vslidedown_vi $V4, $V8, 2]} # v4 := {X, X, w15, w14, w13, w12, w11, w10}
    @{[vsm3c_vi $V0, $V4, 5]}

    @{[vsm3me_vv $V6, $V8, $V6]}   # v6 := {w23, w22, w21, w20, w19, w18, w17, w16}

    # Prepare a register with {w19, w18, w17, w16, w15, w14, w13, w12}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w15, w14, w13, w12}
    @{[vslideup_vi $V4, $V6, 4]}   # v4 := {w19, w18, w17, w16, w15, w14, w13, w12}

    @{[vsm3c_vi $V0, $V4, 6]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w19, w18, w17, w16, w15, w14}
    @{[vsm3c_vi $V0, $V4, 7]}

    @{[vsm3c_vi $V0, $V6, 8]}
    @{[vslidedown_vi $V4, $V6, 2]} # v4 := {X, X, w23, w22, w21, w20, w19, w18}
    @{[vsm3c_vi $V0, $V4, 9]}

    @{[vsm3me_vv $V8, $V6, $V8]}   # v8 := {w31, w30, w29, w28, w27, w26, w25, w24}

    # Prepare a register with {w27, w26, w25, w24, w23, w22, w21, w20}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w23, w22, w21, w20}
    @{[vslideup_vi $V4, $V8, 4]}   # v4 := {w27, w26, w25, w24, w23, w22, w21, w20}

    @{[vsm3c_vi $V0, $V4, 10]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w27, w26, w25, w24, w23, w22}
    @{[vsm3c_vi $V0, $V4, 11]}

    @{[vsm3c_vi $V0, $V8, 12]}
    @{[vslidedown_vi $V4, $V8, 2]} # v4 := {x, X, w31, w30, w29, w28, w27, w26}
    @{[vsm3c_vi $V0, $V4, 13]}

    @{[vsm3me_vv $V6, $V8, $V6]}   # v6 := {w32, w33, w34, w35, w36, w37, w38, w39}

    # Prepare a register with {w35, w34, w33, w32, w31, w30, w29, w28}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w31, w30, w29, w28}
    @{[vslideup_vi $V4, $V6, 4]}   # v4 := {w35, w34, w33, w32, w31, w30, w29, w28}

    @{[vsm3c_vi $V0, $V4, 14]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w35, w34, w33, w32, w31, w30}
    @{[vsm3c_vi $V0, $V4, 15]}

    @{[vsm3c_vi $V0, $V6, 16]}
    @{[vslidedown_vi $V4, $V6, 2]} # v4 := {X, X, w39, w38, w37, w36, w35, w34}
    @{[vsm3c_vi $V0, $V4, 17]}

    @{[vsm3me_vv $V8, $V6, $V8]}   # v8 := {w47, w46, w45, w44, w43, w42, w41, w40}

    # Prepare a register with {w43, w42, w41, w40, w39, w38, w37, w36}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w39, w38, w37, w36}
    @{[vslideup_vi $V4, $V8, 4]}   # v4 := {w43, w42, w41, w40, w39, w38, w37, w36}

    @{[vsm3c_vi $V0, $V4, 18]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w43, w42, w41, w40, w39, w38}
    @{[vsm3c_vi $V0, $V4, 19]}

    @{[vsm3c_vi $V0, $V8, 20]}
    @{[vslidedown_vi $V4, $V8, 2]} # v4 := {X, X, w47, w46, w45, w44, w43, w42}
    @{[vsm3c_vi $V0, $V4, 21]}

    @{[vsm3me_vv $V6, $V8, $V6]}   # v6 := {w55, w54, w53, w52, w51, w50, w49, w48}

    # Prepare a register with {w51, w50, w49, w48, w47, w46, w45, w44}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w47, w46, w45, w44}
    @{[vslideup_vi $V4, $V6, 4]}   # v4 := {w51, w50, w49, w48, w47, w46, w45, w44}

    @{[vsm3c_vi $V0, $V4, 22]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w51, w50, w49, w48, w47, w46}
    @{[vsm3c_vi $V0, $V4, 23]}

    @{[vsm3c_vi $V0, $V6, 24]}
    @{[vslidedown_vi $V4, $V6, 2]} # v4 := {X, X, w55, w54, w53, w52, w51, w50}
    @{[vsm3c_vi $V0, $V4, 25]}

    @{[vsm3me_vv $V8, $V6, $V8]}   # v8 := {w63, w62, w61, w60, w59, w58, w57, w56}

    # Prepare a register with {w59, w58, w57, w56, w55, w54, w53, w52}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w55, w54, w53, w52}
    @{[vslideup_vi $V4, $V8, 4]}   # v4 := {w59, w58, w57, w56, w55, w54, w53, w52}

    @{[vsm3c_vi $V0, $V4, 26]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w59, w58, w57, w56, w55, w54}
    @{[vsm3c_vi $V0, $V4, 27]}

    @{[vsm3c_vi $V0, $V8, 28]}
    @{[vslidedown_vi $V4, $V8, 2]} # v4 := {X, X, w63, w62, w61, w60, w59, w58}
    @{[vsm3c_vi $V0, $V4, 29]}

    @{[vsm3me_vv $V6, $V8, $V6]}   # v6 := {w71, w70, w69, w68, w67, w66, w65, w64}

    # Prepare a register with {w67, w66, w65, w64, w63, w62, w61, w60}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, X, X, w63, w62, w61, w60}
    @{[vslideup_vi $V4, $V6, 4]}   # v4 := {w67, w66, w65, w64, w63, w62, w61, w60}

    @{[vsm3c_vi $V0, $V4, 30]}
    @{[vslidedown_vi $V4, $V4, 2]} # v4 := {X, X, w67, w66, w65, w64, w63, w62}
    @{[vsm3c_vi $V0, $V4, 31]}

    # XOR in the previous state.
    @{[vxor_vv $V0, $V0, $V2]}

    bnez $NUM, L_sm3_loop     # Check if there are any more block to process
L_sm3_end:
    @{[vrev8_v $V0, $V0]}
    @{[vse32_v $V0, $CTX]}
    ret

.size ossl_hwsm3_block_data_order_zvksh,.-ossl_hwsm3_block_data_order_zvksh
___
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
