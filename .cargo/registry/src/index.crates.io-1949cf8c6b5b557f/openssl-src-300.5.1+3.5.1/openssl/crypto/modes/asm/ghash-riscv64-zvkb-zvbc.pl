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
# - RISC-V Vector Carryless Multiplication extension ('Zvbc')

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
# void gcm_init_rv64i_zvkb_zvbc(u128 Htable[16], const u64 H[2]);
#
# input:	H: 128-bit H - secret parameter E(K, 0^128)
# output:	Htable: Preprocessed key data for gcm_gmult_rv64i_zvkb_zvbc and
#                       gcm_ghash_rv64i_zvkb_zvbc
{
my ($Htable,$H,$TMP0,$TMP1,$TMP2) = ("a0","a1","t0","t1","t2");
my ($V0,$V1,$V2,$V3,$V4,$V5,$V6) = ("v0","v1","v2","v3","v4","v5","v6");

$code .= <<___;
.p2align 3
.globl gcm_init_rv64i_zvkb_zvbc
.type gcm_init_rv64i_zvkb_zvbc,\@function
gcm_init_rv64i_zvkb_zvbc:
    # Load/store data in reverse order.
    # This is needed as a part of endianness swap.
    add $H, $H, 8
    li $TMP0, -8
    li $TMP1, 63
    la $TMP2, Lpolymod

    @{[vsetivli__x0_2_e64_m1_tu_mu]} # vsetivli x0, 2, e64, m1, tu, mu

    @{[vlse64_v  $V1, $H, $TMP0]}    # vlse64.v v1, (a1), t0
    @{[vle64_v $V2, $TMP2]}          # vle64.v v2, (t2)

    # Shift one left and get the carry bits.
    @{[vsrl_vx $V3, $V1, $TMP1]}     # vsrl.vx v3, v1, t1
    @{[vsll_vi $V1, $V1, 1]}         # vsll.vi v1, v1, 1

    # Use the fact that the polynomial degree is no more than 128,
    # i.e. only the LSB of the upper half could be set.
    # Thanks to this we don't need to do the full reduction here.
    # Instead simply subtract the reduction polynomial.
    # This idea was taken from x86 ghash implementation in OpenSSL.
    @{[vslideup_vi $V4, $V3, 1]}     # vslideup.vi v4, v3, 1
    @{[vslidedown_vi $V3, $V3, 1]}   # vslidedown.vi v3, v3, 1

    @{[vmv_v_i $V0, 2]}              # vmv.v.i v0, 2
    @{[vor_vv_v0t $V1, $V1, $V4]}    # vor.vv v1, v1, v4, v0.t

    # Need to set the mask to 3, if the carry bit is set.
    @{[vmv_v_v $V0, $V3]}            # vmv.v.v v0, v3
    @{[vmv_v_i $V3, 0]}              # vmv.v.i v3, 0
    @{[vmerge_vim $V3, $V3, 3]}      # vmerge.vim v3, v3, 3, v0
    @{[vmv_v_v $V0, $V3]}            # vmv.v.v v0, v3

    @{[vxor_vv_v0t $V1, $V1, $V2]}   # vxor.vv v1, v1, v2, v0.t

    @{[vse64_v $V1, $Htable]}        # vse64.v v1, (a0)
    ret
.size gcm_init_rv64i_zvkb_zvbc,.-gcm_init_rv64i_zvkb_zvbc
___
}

################################################################################
# void gcm_gmult_rv64i_zvkb_zvbc(u64 Xi[2], const u128 Htable[16]);
#
# input:	Xi: current hash value
#		Htable: preprocessed H
# output:	Xi: next hash value Xi = (Xi * H mod f)
{
my ($Xi,$Htable,$TMP0,$TMP1,$TMP2,$TMP3,$TMP4) = ("a0","a1","t0","t1","t2","t3","t4");
my ($V0,$V1,$V2,$V3,$V4,$V5,$V6) = ("v0","v1","v2","v3","v4","v5","v6");

$code .= <<___;
.text
.p2align 3
.globl gcm_gmult_rv64i_zvkb_zvbc
.type gcm_gmult_rv64i_zvkb_zvbc,\@function
gcm_gmult_rv64i_zvkb_zvbc:
    ld $TMP0, ($Htable)
    ld $TMP1, 8($Htable)
    li $TMP2, 63
    la $TMP3, Lpolymod
    ld $TMP3, 8($TMP3)

    # Load/store data in reverse order.
    # This is needed as a part of endianness swap.
    add $Xi, $Xi, 8
    li $TMP4, -8

    @{[vsetivli__x0_2_e64_m1_tu_mu]} # vsetivli x0, 2, e64, m1, tu, mu

    @{[vlse64_v $V5, $Xi, $TMP4]}    # vlse64.v v5, (a0), t4
    @{[vrev8_v $V5, $V5]}            # vrev8.v v5, v5

    # Multiplication

    # Do two 64x64 multiplications in one go to save some time
    # and simplify things.

    # A = a1a0 (t1, t0)
    # B = b1b0 (v5)
    # C = c1c0 (256 bit)
    # c1 = a1b1 + (a0b1)h + (a1b0)h
    # c0 = a0b0 + (a0b1)l + (a1b0)h

    # v1 = (a0b1)l,(a0b0)l
    @{[vclmul_vx $V1, $V5, $TMP0]}   # vclmul.vx v1, v5, t0
    # v3 = (a0b1)h,(a0b0)h
    @{[vclmulh_vx $V3, $V5, $TMP0]}  # vclmulh.vx v3, v5, t0

    # v4 = (a1b1)l,(a1b0)l
    @{[vclmul_vx $V4, $V5, $TMP1]}   # vclmul.vx v4, v5, t1
    # v2 = (a1b1)h,(a1b0)h
    @{[vclmulh_vx $V2, $V5, $TMP1]}   # vclmulh.vx v2, v5, t1

    # Is there a better way to do this?
    # Would need to swap the order of elements within a vector register.
    @{[vslideup_vi $V5, $V3, 1]}     # vslideup.vi v5, v3, 1
    @{[vslideup_vi $V6, $V4, 1]}     # vslideup.vi v6, v4, 1
    @{[vslidedown_vi $V3, $V3, 1]}   # vslidedown.vi v3, v3, 1
    @{[vslidedown_vi $V4, $V4, 1]}   # vslidedown.vi v4, v4, 1

    @{[vmv_v_i $V0, 1]}              # vmv.v.i v0, 1
    # v2 += (a0b1)h
    @{[vxor_vv_v0t $V2, $V2, $V3]}   # vxor.vv v2, v2, v3, v0.t
    # v2 += (a1b1)l
    @{[vxor_vv_v0t $V2, $V2, $V4]}   # vxor.vv v2, v2, v4, v0.t

    @{[vmv_v_i $V0, 2]}              # vmv.v.i v0, 2
    # v1 += (a0b0)h,0
    @{[vxor_vv_v0t $V1, $V1, $V5]}   # vxor.vv v1, v1, v5, v0.t
    # v1 += (a1b0)l,0
    @{[vxor_vv_v0t $V1, $V1, $V6]}   # vxor.vv v1, v1, v6, v0.t

    # Now the 256bit product should be stored in (v2,v1)
    # v1 = (a0b1)l + (a0b0)h + (a1b0)l, (a0b0)l
    # v2 = (a1b1)h, (a1b0)h + (a0b1)h + (a1b1)l

    # Reduction
    # Let C := A*B = c3,c2,c1,c0 = v2[1],v2[0],v1[1],v1[0]
    # This is a slight variation of the Gueron's Montgomery reduction.
    # The difference being the order of some operations has been changed,
    # to make a better use of vclmul(h) instructions.

    # First step:
    # c1 += (c0 * P)l
    # vmv.v.i v0, 2
    @{[vslideup_vi_v0t $V3, $V1, 1]} # vslideup.vi v3, v1, 1, v0.t
    @{[vclmul_vx_v0t $V3, $V3, $TMP3]} # vclmul.vx v3, v3, t3, v0.t
    @{[vxor_vv_v0t $V1, $V1, $V3]}   # vxor.vv v1, v1, v3, v0.t

    # Second step:
    # D = d1,d0 is final result
    # We want:
    # m1 = c1 + (c1 * P)h
    # m0 = (c1 * P)l + (c0 * P)h + c0
    # d1 = c3 + m1
    # d0 = c2 + m0

    #v3 = (c1 * P)l, 0
    @{[vclmul_vx_v0t $V3, $V1, $TMP3]} # vclmul.vx v3, v1, t3, v0.t
    #v4 = (c1 * P)h, (c0 * P)h
    @{[vclmulh_vx $V4, $V1, $TMP3]}   # vclmulh.vx v4, v1, t3

    @{[vmv_v_i $V0, 1]}              # vmv.v.i v0, 1
    @{[vslidedown_vi $V3, $V3, 1]}   # vslidedown.vi v3, v3, 1

    @{[vxor_vv $V1, $V1, $V4]}       # vxor.vv v1, v1, v4
    @{[vxor_vv_v0t $V1, $V1, $V3]}   # vxor.vv v1, v1, v3, v0.t

    # XOR in the upper upper part of the product
    @{[vxor_vv $V2, $V2, $V1]}       # vxor.vv v2, v2, v1

    @{[vrev8_v $V2, $V2]}            # vrev8.v v2, v2
    @{[vsse64_v $V2, $Xi, $TMP4]}    # vsse64.v v2, (a0), t4
    ret
.size gcm_gmult_rv64i_zvkb_zvbc,.-gcm_gmult_rv64i_zvkb_zvbc
___
}

################################################################################
# void gcm_ghash_rv64i_zvkb_zvbc(u64 Xi[2], const u128 Htable[16],
#                                const u8 *inp, size_t len);
#
# input:	Xi: current hash value
#		Htable: preprocessed H
#		inp: pointer to input data
#		len: length of input data in bytes (multiple of block size)
# output:	Xi: Xi+1 (next hash value Xi)
{
my ($Xi,$Htable,$inp,$len,$TMP0,$TMP1,$TMP2,$TMP3,$M8,$TMP5,$TMP6) = ("a0","a1","a2","a3","t0","t1","t2","t3","t4","t5","t6");
my ($V0,$V1,$V2,$V3,$V4,$V5,$V6,$Vinp) = ("v0","v1","v2","v3","v4","v5","v6","v7");

$code .= <<___;
.p2align 3
.globl gcm_ghash_rv64i_zvkb_zvbc
.type gcm_ghash_rv64i_zvkb_zvbc,\@function
gcm_ghash_rv64i_zvkb_zvbc:
    ld $TMP0, ($Htable)
    ld $TMP1, 8($Htable)
    li $TMP2, 63
    la $TMP3, Lpolymod
    ld $TMP3, 8($TMP3)

    # Load/store data in reverse order.
    # This is needed as a part of endianness swap.
    add $Xi, $Xi, 8
    add $inp, $inp, 8
    li $M8, -8

    @{[vsetivli__x0_2_e64_m1_tu_mu]} # vsetivli x0, 2, e64, m1, tu, mu

    @{[vlse64_v $V5, $Xi, $M8]}      # vlse64.v v5, (a0), t4

Lstep:
    # Read input data
    @{[vlse64_v $Vinp, $inp, $M8]}   # vle64.v v0, (a2)
    add $inp, $inp, 16
    add $len, $len, -16
    # XOR them into Xi
    @{[vxor_vv $V5, $V5, $Vinp]}       # vxor.vv v0, v0, v1

    @{[vrev8_v $V5, $V5]}            # vrev8.v v5, v5

    # Multiplication

    # Do two 64x64 multiplications in one go to save some time
    # and simplify things.

    # A = a1a0 (t1, t0)
    # B = b1b0 (v5)
    # C = c1c0 (256 bit)
    # c1 = a1b1 + (a0b1)h + (a1b0)h
    # c0 = a0b0 + (a0b1)l + (a1b0)h

    # v1 = (a0b1)l,(a0b0)l
    @{[vclmul_vx $V1, $V5, $TMP0]}   # vclmul.vx v1, v5, t0
    # v3 = (a0b1)h,(a0b0)h
    @{[vclmulh_vx $V3, $V5, $TMP0]}  # vclmulh.vx v3, v5, t0

    # v4 = (a1b1)l,(a1b0)l
    @{[vclmul_vx $V4, $V5, $TMP1]}   # vclmul.vx v4, v5, t1
    # v2 = (a1b1)h,(a1b0)h
    @{[vclmulh_vx $V2, $V5, $TMP1]}   # vclmulh.vx v2, v5, t1

    # Is there a better way to do this?
    # Would need to swap the order of elements within a vector register.
    @{[vslideup_vi $V5, $V3, 1]}     # vslideup.vi v5, v3, 1
    @{[vslideup_vi $V6, $V4, 1]}     # vslideup.vi v6, v4, 1
    @{[vslidedown_vi $V3, $V3, 1]}   # vslidedown.vi v3, v3, 1
    @{[vslidedown_vi $V4, $V4, 1]}   # vslidedown.vi v4, v4, 1

    @{[vmv_v_i $V0, 1]}              # vmv.v.i v0, 1
    # v2 += (a0b1)h
    @{[vxor_vv_v0t $V2, $V2, $V3]}   # vxor.vv v2, v2, v3, v0.t
    # v2 += (a1b1)l
    @{[vxor_vv_v0t $V2, $V2, $V4]}   # vxor.vv v2, v2, v4, v0.t

    @{[vmv_v_i $V0, 2]}              # vmv.v.i v0, 2
    # v1 += (a0b0)h,0
    @{[vxor_vv_v0t $V1, $V1, $V5]}   # vxor.vv v1, v1, v5, v0.t
    # v1 += (a1b0)l,0
    @{[vxor_vv_v0t $V1, $V1, $V6]}   # vxor.vv v1, v1, v6, v0.t

    # Now the 256bit product should be stored in (v2,v1)
    # v1 = (a0b1)l + (a0b0)h + (a1b0)l, (a0b0)l
    # v2 = (a1b1)h, (a1b0)h + (a0b1)h + (a1b1)l

    # Reduction
    # Let C := A*B = c3,c2,c1,c0 = v2[1],v2[0],v1[1],v1[0]
    # This is a slight variation of the Gueron's Montgomery reduction.
    # The difference being the order of some operations has been changed,
    # to make a better use of vclmul(h) instructions.

    # First step:
    # c1 += (c0 * P)l
    # vmv.v.i v0, 2
    @{[vslideup_vi_v0t $V3, $V1, 1]} # vslideup.vi v3, v1, 1, v0.t
    @{[vclmul_vx_v0t $V3, $V3, $TMP3]} # vclmul.vx v3, v3, t3, v0.t
    @{[vxor_vv_v0t $V1, $V1, $V3]}   # vxor.vv v1, v1, v3, v0.t

    # Second step:
    # D = d1,d0 is final result
    # We want:
    # m1 = c1 + (c1 * P)h
    # m0 = (c1 * P)l + (c0 * P)h + c0
    # d1 = c3 + m1
    # d0 = c2 + m0

    #v3 = (c1 * P)l, 0
    @{[vclmul_vx_v0t $V3, $V1, $TMP3]} # vclmul.vx v3, v1, t3, v0.t
    #v4 = (c1 * P)h, (c0 * P)h
    @{[vclmulh_vx $V4, $V1, $TMP3]}   # vclmulh.vx v4, v1, t3

    @{[vmv_v_i $V0, 1]}              # vmv.v.i v0, 1
    @{[vslidedown_vi $V3, $V3, 1]}   # vslidedown.vi v3, v3, 1

    @{[vxor_vv $V1, $V1, $V4]}       # vxor.vv v1, v1, v4
    @{[vxor_vv_v0t $V1, $V1, $V3]}   # vxor.vv v1, v1, v3, v0.t

    # XOR in the upper upper part of the product
    @{[vxor_vv $V2, $V2, $V1]}       # vxor.vv v2, v2, v1

    @{[vrev8_v $V5, $V2]}            # vrev8.v v2, v2

    bnez $len, Lstep

    @{[vsse64_v $V5, $Xi, $M8]}    # vsse64.v v2, (a0), t4
    ret
.size gcm_ghash_rv64i_zvkb_zvbc,.-gcm_ghash_rv64i_zvkb_zvbc
___
}

$code .= <<___;
.p2align 4
Lpolymod:
        .dword 0x0000000000000001
        .dword 0xc200000000000000
.size Lpolymod,.-Lpolymod
___

print $code;

close STDOUT or die "error closing STDOUT: $!";
