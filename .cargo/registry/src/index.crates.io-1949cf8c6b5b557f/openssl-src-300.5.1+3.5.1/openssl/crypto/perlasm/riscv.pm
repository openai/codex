#! /usr/bin/env perl
# This file is dual-licensed, meaning that you can use it under your
# choice of either of the following two licenses:
#
# Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License"). You can obtain
# a copy in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# or
#
# Copyright (c) 2023, Christoph Müllner <christoph.muellner@vrull.eu>
# Copyright (c) 2023, Jerry Shih <jerry.shih@sifive.com>
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

use strict;
use warnings;

# Set $have_stacktrace to 1 if we have Devel::StackTrace
my $have_stacktrace = 0;
if (eval {require Devel::StackTrace;1;}) {
    $have_stacktrace = 1;
}

my @regs = map("x$_",(0..31));
# Mapping from the RISC-V psABI ABI mnemonic names to the register number.
my @regaliases = ('zero','ra','sp','gp','tp','t0','t1','t2','s0','s1',
    map("a$_",(0..7)),
    map("s$_",(2..11)),
    map("t$_",(3..6))
);

my %reglookup;
@reglookup{@regs} = @regs;
@reglookup{@regaliases} = @regs;

# Takes a register name, possibly an alias, and converts it to a register index
# from 0 to 31
sub read_reg {
    my $reg = lc shift;
    if (!exists($reglookup{$reg})) {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unknown register ".$reg."\n".$trace);
    }
    my $regstr = $reglookup{$reg};
    if (!($regstr =~ /^x([0-9]+)$/)) {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Could not process register ".$reg."\n".$trace);
    }
    return $1;
}

# Read the sew setting(8, 16, 32 and 64) and convert to vsew encoding.
sub read_sew {
    my $sew_setting = shift;

    if ($sew_setting eq "e8") {
        return 0;
    } elsif ($sew_setting eq "e16") {
        return 1;
    } elsif ($sew_setting eq "e32") {
        return 2;
    } elsif ($sew_setting eq "e64") {
        return 3;
    } else {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unsupported SEW setting:".$sew_setting."\n".$trace);
    }
}

# Read the LMUL settings and convert to vlmul encoding.
sub read_lmul {
    my $lmul_setting = shift;

    if ($lmul_setting eq "mf8") {
        return 5;
    } elsif ($lmul_setting eq "mf4") {
        return 6;
    } elsif ($lmul_setting eq "mf2") {
        return 7;
    } elsif ($lmul_setting eq "m1") {
        return 0;
    } elsif ($lmul_setting eq "m2") {
        return 1;
    } elsif ($lmul_setting eq "m4") {
        return 2;
    } elsif ($lmul_setting eq "m8") {
        return 3;
    } else {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unsupported LMUL setting:".$lmul_setting."\n".$trace);
    }
}

# Read the tail policy settings and convert to vta encoding.
sub read_tail_policy {
    my $tail_setting = shift;

    if ($tail_setting eq "ta") {
        return 1;
    } elsif ($tail_setting eq "tu") {
        return 0;
    } else {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unsupported tail policy setting:".$tail_setting."\n".$trace);
    }
}

# Read the mask policy settings and convert to vma encoding.
sub read_mask_policy {
    my $mask_setting = shift;

    if ($mask_setting eq "ma") {
        return 1;
    } elsif ($mask_setting eq "mu") {
        return 0;
    } else {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unsupported mask policy setting:".$mask_setting."\n".$trace);
    }
}

my @vregs = map("v$_",(0..31));
my %vreglookup;
@vreglookup{@vregs} = @vregs;

sub read_vreg {
    my $vreg = lc shift;
    if (!exists($vreglookup{$vreg})) {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Unknown vector register ".$vreg."\n".$trace);
    }
    if (!($vreg =~ /^v([0-9]+)$/)) {
        my $trace = "";
        if ($have_stacktrace) {
            $trace = Devel::StackTrace->new->as_string;
        }
        die("Could not process vector register ".$vreg."\n".$trace);
    }
    return $1;
}

# Read the vm settings and convert to mask encoding.
sub read_mask_vreg {
    my $vreg = shift;
    # The default value is unmasked.
    my $mask_bit = 1;

    if (defined($vreg)) {
        my $reg_id = read_vreg $vreg;
        if ($reg_id == 0) {
            $mask_bit = 0;
        } else {
            my $trace = "";
            if ($have_stacktrace) {
                $trace = Devel::StackTrace->new->as_string;
            }
            die("The ".$vreg." is not the mask register v0.\n".$trace);
        }
    }
    return $mask_bit;
}

# Helper functions

sub brev8_rv64i {
    # brev8 without `brev8` instruction (only in Zbkb)
    # Bit-reverses the first argument and needs two scratch registers
    my $val = shift;
    my $t0 = shift;
    my $t1 = shift;
    my $brev8_const = shift;
    my $seq = <<___;
        la      $brev8_const, Lbrev8_const

        ld      $t0, 0($brev8_const)  # 0xAAAAAAAAAAAAAAAA
        slli    $t1, $val, 1
        and     $t1, $t1, $t0
        and     $val, $val, $t0
        srli    $val, $val, 1
        or      $val, $t1, $val

        ld      $t0, 8($brev8_const)  # 0xCCCCCCCCCCCCCCCC
        slli    $t1, $val, 2
        and     $t1, $t1, $t0
        and     $val, $val, $t0
        srli    $val, $val, 2
        or      $val, $t1, $val

        ld      $t0, 16($brev8_const) # 0xF0F0F0F0F0F0F0F0
        slli    $t1, $val, 4
        and     $t1, $t1, $t0
        and     $val, $val, $t0
        srli    $val, $val, 4
        or      $val, $t1, $val
___
    return $seq;
}

sub sd_rev8_rv64i {
    # rev8 without `rev8` instruction (only in Zbb or Zbkb)
    # Stores the given value byte-reversed and needs one scratch register
    my $val = shift;
    my $addr = shift;
    my $off = shift;
    my $tmp = shift;
    my $off0 = ($off + 0);
    my $off1 = ($off + 1);
    my $off2 = ($off + 2);
    my $off3 = ($off + 3);
    my $off4 = ($off + 4);
    my $off5 = ($off + 5);
    my $off6 = ($off + 6);
    my $off7 = ($off + 7);
    my $seq = <<___;
        sb      $val, $off7($addr)
        srli    $tmp, $val, 8
        sb      $tmp, $off6($addr)
        srli    $tmp, $val, 16
        sb      $tmp, $off5($addr)
        srli    $tmp, $val, 24
        sb      $tmp, $off4($addr)
        srli    $tmp, $val, 32
        sb      $tmp, $off3($addr)
        srli    $tmp, $val, 40
        sb      $tmp, $off2($addr)
        srli    $tmp, $val, 48
        sb      $tmp, $off1($addr)
        srli    $tmp, $val, 56
        sb      $tmp, $off0($addr)
___
    return $seq;
}

# Scalar crypto instructions

sub aes64ds {
    # Encoding for aes64ds rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0011101_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes64dsm {
    # Encoding for aes64dsm rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0011111_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes64es {
    # Encoding for aes64es rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0011001_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes64esm {
    # Encoding for aes64esm rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0011011_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes64im {
    # Encoding for aes64im rd, rs1 instruction on RV64
    #                XXXXXXXXXXXX_ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b001100000000_00000_001_00000_0010011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($rs1 << 15) | ($rd << 7));
}

sub aes64ks1i {
    # Encoding for aes64ks1i rd, rs1, rnum instruction on RV64
    #                XXXXXXXX_rnum_ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b00110001_0000_00000_001_00000_0010011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rnum = shift;
    return ".word ".($template | ($rnum << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes64ks2 {
    # Encoding for aes64ks2 rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0111111_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub brev8 {
    # brev8 rd, rs
    my $template = 0b011010000111_00000_101_00000_0010011;
    my $rd = read_reg shift;
    my $rs = read_reg shift;
    return ".word ".($template | ($rs << 15) | ($rd << 7));
}

sub clmul {
    # Encoding for clmul rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0000101_00000_00000_001_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub clmulh {
    # Encoding for clmulh rd, rs1, rs2 instruction on RV64
    #                XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0000101_00000_00000_011_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub rev8 {
    # Encoding for rev8 rd, rs instruction on RV64
    #               XXXXXXXXXXXXX_ rs  _XXX_ rd  _XXXXXXX
    my $template = 0b011010111000_00000_101_00000_0010011;
    my $rd = read_reg shift;
    my $rs = read_reg shift;
    return ".word ".($template | ($rs << 15) | ($rd << 7));
}

sub roriw {
    # Encoding for roriw rd, rs1, shamt instruction on RV64
    #               XXXXXXX_ shamt _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0110000_00000_00000_101_00000_0011011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $shamt = shift;
    return ".word ".($template | ($shamt << 20) | ($rs1 << 15) | ($rd << 7));
}

sub maxu {
    # Encoding for maxu rd, rs1, rs2 instruction on RV64
    #               XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0000101_00000_00000_111_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub minu {
    # Encoding for minu rd, rs1, rs2 instruction on RV64
    #               XXXXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0000101_00000_00000_101_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

# Vector instructions

sub vadd_vv {
    # vadd.vv vd, vs2, vs1, vm
    my $template = 0b000000_0_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vadd_vx {
    # vadd.vx vd, vs2, rs1, vm
    my $template = 0b000000_0_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vsub_vv {
    # vsub.vv vd, vs2, vs1, vm
    my $template = 0b000010_0_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vsub_vx {
    # vsub.vx vd, vs2, rs1, vm
    my $template = 0b000010_0_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vid_v {
    # vid.v vd
    my $template = 0b0101001_00000_10001_010_00000_1010111;
    my $vd = read_vreg shift;
    return ".word ".($template | ($vd << 7));
}

sub viota_m {
    # viota.m vd, vs2, vm
    my $template = 0b010100_0_00000_10000_010_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vd << 7));
}

sub vle8_v {
    # vle8.v vd, (rs1), vm
    my $template = 0b000000_0_00000_00000_000_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($rs1 << 15) | ($vd << 7));
}

sub vle32_v {
    # vle32.v vd, (rs1), vm
    my $template = 0b000000_0_00000_00000_110_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($rs1 << 15) | ($vd << 7));
}

sub vle64_v {
    # vle64.v vd, (rs1)
    my $template = 0b0000001_00000_00000_111_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($rs1 << 15) | ($vd << 7));
}

sub vlse32_v {
    # vlse32.v vd, (rs1), rs2
    my $template = 0b0000101_00000_00000_110_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vlsseg_nf_e32_v {
    # vlsseg<nf>e32.v vd, (rs1), rs2
    my $template = 0b0000101_00000_00000_110_00000_0000111;
    my $nf = shift;
    $nf -= 1;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($nf << 29) | ($rs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vlse64_v {
    # vlse64.v vd, (rs1), rs2
    my $template = 0b0000101_00000_00000_111_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vluxei8_v {
    # vluxei8.v vd, (rs1), vs2, vm
    my $template = 0b000001_0_00000_00000_000_00000_0000111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vs2 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vmerge_vim {
    # vmerge.vim vd, vs2, imm, v0
    my $template = 0b0101110_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $imm = shift;
    return ".word ".($template | ($vs2 << 20) | ($imm << 15) | ($vd << 7));
}

sub vmerge_vvm {
    # vmerge.vvm vd vs2 vs1
    my $template = 0b0101110_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7))
}

sub vmseq_vi {
    # vmseq.vi vd vs1, imm
    my $template = 0b0110001_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs1 = read_vreg shift;
    my $imm = shift;
    return ".word ".($template | ($vs1 << 20) | ($imm << 15) | ($vd << 7))
}

sub vmsgtu_vx {
    # vmsgtu.vx vd vs2, rs1, vm
    my $template = 0b011110_0_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7))
}

sub vmv_v_i {
    # vmv.v.i vd, imm
    my $template = 0b0101111_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $imm = shift;
    return ".word ".($template | ($imm << 15) | ($vd << 7));
}

sub vmv_v_x {
    # vmv.v.x vd, rs1
    my $template = 0b0101111_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($rs1 << 15) | ($vd << 7));
}

sub vmv_v_v {
    # vmv.v.v vd, vs1
    my $template = 0b0101111_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs1 << 15) | ($vd << 7));
}

sub vor_vv {
    # vor.vv vd, vs2, vs1
    my $template = 0b0010101_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vor_vv_v0t {
    # vor.vv vd, vs2, vs1, v0.t
    my $template = 0b0010100_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vse8_v {
    # vse8.v vd, (rs1), vm
    my $template = 0b000000_0_00000_00000_000_00000_0100111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($rs1 << 15) | ($vd << 7));
}

sub vse32_v {
    # vse32.v vd, (rs1), vm
    my $template = 0b000000_0_00000_00000_110_00000_0100111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($rs1 << 15) | ($vd << 7));
}

sub vssseg_nf_e32_v {
    # vssseg<nf>e32.v vs3, (rs1), rs2
    my $template = 0b0000101_00000_00000_110_00000_0100111;
    my $nf = shift;
    $nf -= 1;
    my $vs3 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($nf << 29) | ($rs2 << 20) | ($rs1 << 15) | ($vs3 << 7));
}

sub vsuxei8_v {
   # vsuxei8.v vs3, (rs1), vs2, vm
   my $template = 0b000001_0_00000_00000_000_00000_0100111;
   my $vs3 = read_vreg shift;
   my $rs1 = read_reg shift;
   my $vs2 = read_vreg shift;
   my $vm = read_mask_vreg shift;
   return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($rs1 << 15) | ($vs3 << 7));
}

sub vse64_v {
    # vse64.v vd, (rs1)
    my $template = 0b0000001_00000_00000_111_00000_0100111;
    my $vd = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($rs1 << 15) | ($vd << 7));
}

sub vsetivli__x0_2_e64_m1_tu_mu {
    # vsetivli x0, 2, e64, m1, tu, mu
    return ".word 0xc1817057";
}

sub vsetivli__x0_4_e32_m1_tu_mu {
    # vsetivli x0, 4, e32, m1, tu, mu
    return ".word 0xc1027057";
}

sub vsetivli__x0_4_e64_m1_tu_mu {
    # vsetivli x0, 4, e64, m1, tu, mu
    return ".word 0xc1827057";
}

sub vsetivli__x0_8_e32_m1_tu_mu {
    # vsetivli x0, 8, e32, m1, tu, mu
    return ".word 0xc1047057";
}

sub vsetvli {
    # vsetvli rd, rs1, vtypei
    my $template = 0b0_00000000000_00000_111_00000_1010111;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $sew = read_sew shift;
    my $lmul = read_lmul shift;
    my $tail_policy = read_tail_policy shift;
    my $mask_policy = read_mask_policy shift;
    my $vtypei = ($mask_policy << 7) | ($tail_policy << 6) | ($sew << 3) | $lmul;

    return ".word ".($template | ($vtypei << 20) | ($rs1 << 15) | ($rd << 7));
}

sub vsetivli {
    # vsetvli rd, uimm, vtypei
    my $template = 0b11_0000000000_00000_111_00000_1010111;
    my $rd = read_reg shift;
    my $uimm = shift;
    my $sew = read_sew shift;
    my $lmul = read_lmul shift;
    my $tail_policy = read_tail_policy shift;
    my $mask_policy = read_mask_policy shift;
    my $vtypei = ($mask_policy << 7) | ($tail_policy << 6) | ($sew << 3) | $lmul;

    return ".word ".($template | ($vtypei << 20) | ($uimm << 15) | ($rd << 7));
}

sub vslidedown_vi {
    # vslidedown.vi vd, vs2, uimm
    my $template = 0b0011111_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vslidedown_vx {
    # vslidedown.vx vd, vs2, rs1
    my $template = 0b0011111_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vslideup_vi_v0t {
    # vslideup.vi vd, vs2, uimm, v0.t
    my $template = 0b0011100_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vslideup_vi {
    # vslideup.vi vd, vs2, uimm
    my $template = 0b0011101_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vsll_vi {
    # vsll.vi vd, vs2, uimm, vm
    my $template = 0b1001011_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vsrl_vi {
    # vsrl.vi vd, vs2, uimm, vm
    my $template = 0b1010001_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vsrl_vx {
    # vsrl.vx vd, vs2, rs1
    my $template = 0b1010001_00000_00000_100_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vsse32_v {
    # vse32.v vs3, (rs1), rs2
    my $template = 0b0000101_00000_00000_110_00000_0100111;
    my $vs3 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($vs3 << 7));
}

sub vsse64_v {
    # vsse64.v vs3, (rs1), rs2
    my $template = 0b0000101_00000_00000_111_00000_0100111;
    my $vs3 = read_vreg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    return ".word ".($template | ($rs2 << 20) | ($rs1 << 15) | ($vs3 << 7));
}

sub vxor_vv_v0t {
    # vxor.vv vd, vs2, vs1, v0.t
    my $template = 0b0010110_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vxor_vv {
    # vxor.vv vd, vs2, vs1
    my $template = 0b0010111_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vzext_vf2 {
    # vzext.vf2 vd, vs2, vm
    my $template = 0b010010_0_00000_00110_010_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vd << 7));
}

# Vector crypto instructions

## Zvbb and Zvkb instructions
##
## vandn (also in zvkb)
## vbrev
## vbrev8 (also in zvkb)
## vrev8 (also in zvkb)
## vclz
## vctz
## vcpop
## vrol (also in zvkb)
## vror (also in zvkb)
## vwsll

sub vbrev8_v {
    # vbrev8.v vd, vs2, vm
    my $template = 0b010010_0_00000_01000_010_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vd << 7));
}

sub vrev8_v {
    # vrev8.v vd, vs2, vm
    my $template = 0b010010_0_00000_01001_010_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vd << 7));
}

sub vror_vi {
    # vror.vi vd, vs2, uimm
    my $template = 0b01010_0_1_00000_00000_011_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    my $uimm_i5 = $uimm >> 5;
    my $uimm_i4_0 = $uimm & 0b11111;

    return ".word ".($template | ($uimm_i5 << 26) | ($vs2 << 20) | ($uimm_i4_0 << 15) | ($vd << 7));
}

sub vwsll_vv {
    # vwsll.vv vd, vs2, vs1, vm
    my $template = 0b110101_0_00000_00000_000_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    my $vm = read_mask_vreg shift;
    return ".word ".($template | ($vm << 25) | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

## Zvbc instructions

sub vclmulh_vx {
    # vclmulh.vx vd, vs2, rs1
    my $template = 0b0011011_00000_00000_110_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vclmul_vx_v0t {
    # vclmul.vx vd, vs2, rs1, v0.t
    my $template = 0b0011000_00000_00000_110_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

sub vclmul_vx {
    # vclmul.vx vd, vs2, rs1
    my $template = 0b0011001_00000_00000_110_00000_1010111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $rs1 = read_reg shift;
    return ".word ".($template | ($vs2 << 20) | ($rs1 << 15) | ($vd << 7));
}

## Zvkg instructions

sub vghsh_vv {
    # vghsh.vv vd, vs2, vs1
    my $template = 0b1011001_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15) | ($vd << 7));
}

sub vgmul_vv {
    # vgmul.vv vd, vs2
    my $template = 0b1010001_00000_10001_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

## Zvkned instructions

sub vaesdf_vs {
    # vaesdf.vs vd, vs2
    my $template = 0b101001_1_00000_00001_010_00000_1110111;
    my $vd = read_vreg  shift;
    my $vs2 = read_vreg  shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

sub vaesdm_vs {
    # vaesdm.vs vd, vs2
    my $template = 0b101001_1_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

sub vaesef_vs {
    # vaesef.vs vd, vs2
    my $template = 0b101001_1_00000_00011_010_00000_1110111;
    my $vd = read_vreg  shift;
    my $vs2 = read_vreg  shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

sub vaesem_vs {
    # vaesem.vs vd, vs2
    my $template = 0b101001_1_00000_00010_010_00000_1110111;
    my $vd = read_vreg  shift;
    my $vs2 = read_vreg  shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

sub vaeskf1_vi {
    # vaeskf1.vi vd, vs2, uimmm
    my $template = 0b100010_1_00000_00000_010_00000_1110111;
    my $vd = read_vreg  shift;
    my $vs2 = read_vreg  shift;
    my $uimm = shift;
    return ".word ".($template | ($uimm << 15) | ($vs2 << 20) | ($vd << 7));
}

sub vaeskf2_vi {
    # vaeskf2.vi vd, vs2, uimm
    my $template = 0b101010_1_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vaesz_vs {
    # vaesz.vs vd, vs2
    my $template = 0b101001_1_00000_00111_010_00000_1110111;
    my $vd = read_vreg  shift;
    my $vs2 = read_vreg  shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

## Zvknha and Zvknhb instructions

sub vsha2ms_vv {
    # vsha2ms.vv vd, vs2, vs1
    my $template = 0b1011011_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20)| ($vs1 << 15 )| ($vd << 7));
}

sub vsha2ch_vv {
    # vsha2ch.vv vd, vs2, vs1
    my $template = 0b101110_10000_00000_001_00000_01110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20)| ($vs1 << 15 )| ($vd << 7));
}

sub vsha2cl_vv {
    # vsha2cl.vv vd, vs2, vs1
    my $template = 0b101111_10000_00000_001_00000_01110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20)| ($vs1 << 15 )| ($vd << 7));
}

## Zvksed instructions

sub vsm4k_vi {
    # vsm4k.vi vd, vs2, uimm
    my $template = 0b1000011_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15) | ($vd << 7));
}

sub vsm4r_vs {
    # vsm4r.vs vd, vs2
    my $template = 0b1010011_00000_10000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vd << 7));
}

## zvksh instructions

sub vsm3c_vi {
    # vsm3c.vi vd, vs2, uimm
    my $template = 0b1010111_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $uimm = shift;
    return ".word ".($template | ($vs2 << 20) | ($uimm << 15 ) | ($vd << 7));
}

sub vsm3me_vv {
    # vsm3me.vv vd, vs2, vs1
    my $template = 0b1000001_00000_00000_010_00000_1110111;
    my $vd = read_vreg shift;
    my $vs2 = read_vreg shift;
    my $vs1 = read_vreg shift;
    return ".word ".($template | ($vs2 << 20) | ($vs1 << 15 ) | ($vd << 7));
}

1;
