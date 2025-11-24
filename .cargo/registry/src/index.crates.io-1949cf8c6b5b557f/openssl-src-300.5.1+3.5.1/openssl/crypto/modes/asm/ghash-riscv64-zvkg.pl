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
# - RISC-V Vector GCM/GMAC extension ('Zvkg')
#
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
my $output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$output and open STDOUT,">$output";

my $code=<<___;
.text
___

################################################################################
# void gcm_init_rv64i_zvkg(u128 Htable[16], const u64 H[2]);
# void gcm_init_rv64i_zvkg_zvkb(u128 Htable[16], const u64 H[2]);
#
# input: H: 128-bit H - secret parameter E(K, 0^128)
# output: Htable: Copy of secret parameter (in normalized byte order)
#
# All callers of this function revert the byte-order unconditionally
# on little-endian machines. So we need to revert the byte-order back.
{
my ($Htable,$H,$VAL0,$VAL1,$TMP0) = ("a0","a1","a2","a3","t0");

$code .= <<___;
.p2align 3
.globl gcm_init_rv64i_zvkg
.type gcm_init_rv64i_zvkg,\@function
gcm_init_rv64i_zvkg:
    ld      $VAL0, 0($H)
    ld      $VAL1, 8($H)
    @{[sd_rev8_rv64i $VAL0, $Htable, 0, $TMP0]}
    @{[sd_rev8_rv64i $VAL1, $Htable, 8, $TMP0]}
    ret
.size gcm_init_rv64i_zvkg,.-gcm_init_rv64i_zvkg
___
}

{
my ($Htable,$H,$V0) = ("a0","a1","v0");

$code .= <<___;
.p2align 3
.globl gcm_init_rv64i_zvkg_zvkb
.type gcm_init_rv64i_zvkg_zvkb,\@function
gcm_init_rv64i_zvkg_zvkb:
    @{[vsetivli__x0_2_e64_m1_tu_mu]} # vsetivli x0, 2, e64, m1, ta, ma
    @{[vle64_v $V0, $H]}             # vle64.v v0, (a1)
    @{[vrev8_v $V0, $V0]}            # vrev8.v v0, v0
    @{[vse64_v $V0, $Htable]}        # vse64.v v0, (a0)
    ret
.size gcm_init_rv64i_zvkg_zvkb,.-gcm_init_rv64i_zvkg_zvkb
___
}

################################################################################
# void gcm_gmult_rv64i_zvkg(u64 Xi[2], const u128 Htable[16]);
#
# input: Xi: current hash value
#        Htable: copy of H
# output: Xi: next hash value Xi
{
my ($Xi,$Htable) = ("a0","a1");
my ($VD,$VS2) = ("v1","v2");

$code .= <<___;
.p2align 3
.globl gcm_gmult_rv64i_zvkg
.type gcm_gmult_rv64i_zvkg,\@function
gcm_gmult_rv64i_zvkg:
    @{[vsetivli__x0_4_e32_m1_tu_mu]}
    @{[vle32_v $VS2, $Htable]}
    @{[vle32_v $VD, $Xi]}
    @{[vgmul_vv $VD, $VS2]}
    @{[vse32_v $VD, $Xi]}
    ret
.size gcm_gmult_rv64i_zvkg,.-gcm_gmult_rv64i_zvkg
___
}

################################################################################
# void gcm_ghash_rv64i_zvkg(u64 Xi[2], const u128 Htable[16],
#                           const u8 *inp, size_t len);
#
# input: Xi: current hash value
#        Htable: copy of H
#        inp: pointer to input data
#        len: length of input data in bytes (multiple of block size)
# output: Xi: Xi+1 (next hash value Xi)
{
my ($Xi,$Htable,$inp,$len) = ("a0","a1","a2","a3");
my ($vXi,$vH,$vinp,$Vzero) = ("v1","v2","v3","v4");

$code .= <<___;
.p2align 3
.globl gcm_ghash_rv64i_zvkg
.type gcm_ghash_rv64i_zvkg,\@function
gcm_ghash_rv64i_zvkg:
    @{[vsetivli__x0_4_e32_m1_tu_mu]}
    @{[vle32_v $vH, $Htable]}
    @{[vle32_v $vXi, $Xi]}

Lstep:
    @{[vle32_v $vinp, $inp]}
    add $inp, $inp, 16
    add $len, $len, -16
    @{[vghsh_vv $vXi, $vH, $vinp]}
    bnez $len, Lstep

    @{[vse32_v $vXi, $Xi]}
    ret

.size gcm_ghash_rv64i_zvkg,.-gcm_ghash_rv64i_zvkg
___
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
