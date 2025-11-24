#! /usr/bin/env perl
# This file is dual-licensed, meaning that you can use it under your
# choice of either of the following two licenses:
#
# Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License"). You can obtain
# a copy in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# or
#
# Copyright (c) 2022, Hongren (Zenithal) Zheng <i@zenithal.me>
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

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$output and open STDOUT,">$output";

################################################################################
# Utility functions to help with keeping track of which registers to stack/
# unstack when entering / exiting routines.
################################################################################
{
    # Callee-saved registers
    my @callee_saved = map("x$_",(2,8,9,18..27));
    # Caller-saved registers
    my @caller_saved = map("x$_",(1,5..7,10..17,28..31));
    my @must_save;
    sub use_reg {
        my $reg = shift;
        if (grep(/^$reg$/, @callee_saved)) {
            push(@must_save, $reg);
        } elsif (!grep(/^$reg$/, @caller_saved)) {
            # Register is not usable!
            die("Unusable register ".$reg);
        }
        return $reg;
    }
    sub use_regs {
        return map(use_reg("x$_"), @_);
    }
    sub save_regs {
        my $ret = '';
        my $stack_reservation = ($#must_save + 1) * 8;
        my $stack_offset = $stack_reservation;
        if ($stack_reservation % 16) {
            $stack_reservation += 8;
        }
        $ret.="    addi    sp,sp,-$stack_reservation\n";
        foreach (@must_save) {
            $stack_offset -= 8;
            $ret.="    sw      $_,$stack_offset(sp)\n";
        }
        return $ret;
    }
    sub load_regs {
        my $ret = '';
        my $stack_reservation = ($#must_save + 1) * 8;
        my $stack_offset = $stack_reservation;
        if ($stack_reservation % 16) {
            $stack_reservation += 8;
        }
        foreach (@must_save) {
            $stack_offset -= 8;
            $ret.="    lw      $_,$stack_offset(sp)\n";
        }
        $ret.="    addi    sp,sp,$stack_reservation\n";
        return $ret;
    }
    sub clear_regs {
        @must_save = ();
    }
}

################################################################################
# util for encoding scalar crypto extension instructions
################################################################################

my @regs = map("x$_",(0..31));
my %reglookup;
@reglookup{@regs} = @regs;

# Takes a register name, possibly an alias, and converts it to a register index
# from 0 to 31
sub read_reg {
    my $reg = lc shift;
    if (!exists($reglookup{$reg})) {
        die("Unknown register ".$reg);
    }
    my $regstr = $reglookup{$reg};
    if (!($regstr =~ /^x([0-9]+)$/)) {
        die("Could not process register ".$reg);
    }
    return $1;
}

sub aes32dsi {
    # Encoding for aes32dsi rd, rs1, rs2, bs instruction on RV32
    #                bs_XXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b00_10101_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    my $bs = shift;

    return ".word ".($template | ($bs << 30) | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes32dsmi {
    # Encoding for aes32dsmi rd, rs1, rs2, bs instruction on RV32
    #                bs_XXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b00_10111_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    my $bs = shift;

    return ".word ".($template | ($bs << 30) | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes32esi {
    # Encoding for aes32esi rd, rs1, rs2, bs instruction on RV32
    #                bs_XXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b00_10001_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    my $bs = shift;

    return ".word ".($template | ($bs << 30) | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub aes32esmi {
    # Encoding for aes32esmi rd, rs1, rs2, bs instruction on RV32
    #                bs_XXXXX_ rs2 _ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b00_10011_00000_00000_000_00000_0110011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $rs2 = read_reg shift;
    my $bs = shift;

    return ".word ".($template | ($bs << 30) | ($rs2 << 20) | ($rs1 << 15) | ($rd << 7));
}

sub rori {
    # Encoding for ror rd, rs1, imm instruction on RV64
    #                XXXXXXX_shamt_ rs1 _XXX_ rd  _XXXXXXX
    my $template = 0b0110000_00000_00000_101_00000_0010011;
    my $rd = read_reg shift;
    my $rs1 = read_reg shift;
    my $shamt = shift;

    return ".word ".($template | ($shamt << 20) | ($rs1 << 15) | ($rd << 7));
}

################################################################################
# Register assignment for rv32i_zkne_encrypt and rv32i_zknd_decrypt
################################################################################

# Registers initially to hold AES state (called s0-s3 or y0-y3 elsewhere)
my ($Q0,$Q1,$Q2,$Q3) = use_regs(6..9);

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Input block pointer, output block pointer, key pointer
my ($INP,$OUTP,$KEYP) = use_regs(10..12);

# Registers initially to hold Key
my ($T0,$T1,$T2,$T3) = use_regs(13..16);

# Loop counter
my ($loopcntr) = use_regs(30);

################################################################################
# Utility for rv32i_zkne_encrypt and rv32i_zknd_decrypt
################################################################################

# outer product of whole state into one column of key
sub outer {
    my $inst = shift;
    my $key = shift;
    # state 0 to 3
    my $s0 = shift;
    my $s1 = shift;
    my $s2 = shift;
    my $s3 = shift;
    my $ret = '';
$ret .= <<___;
    @{[$inst->($key,$key,$s0,0)]}
    @{[$inst->($key,$key,$s1,1)]}
    @{[$inst->($key,$key,$s2,2)]}
    @{[$inst->($key,$key,$s3,3)]}
___
    return $ret;
}

sub aes32esmi4 {
    return outer(\&aes32esmi, @_)
}

sub aes32esi4 {
    return outer(\&aes32esi, @_)
}

sub aes32dsmi4 {
    return outer(\&aes32dsmi, @_)
}

sub aes32dsi4 {
    return outer(\&aes32dsi, @_)
}

################################################################################
# void rv32i_zkne_encrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
my $code .= <<___;
.text
.balign 16
.globl rv32i_zkne_encrypt
.type   rv32i_zkne_encrypt,\@function
rv32i_zkne_encrypt:
___

$code .= save_regs();

$code .= <<___;
    # Load input to block cipher
    lw      $Q0,0($INP)
    lw      $Q1,4($INP)
    lw      $Q2,8($INP)
    lw      $Q3,12($INP)

    # Load key
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    # Load number of rounds
    lw      $loopcntr,240($KEYP)

    # initial transformation
    xor     $Q0,$Q0,$T0
    xor     $Q1,$Q1,$T1
    xor     $Q2,$Q2,$T2
    xor     $Q3,$Q3,$T3

    # The main loop only executes the first N-2 rounds, each loop consumes two rounds
    add     $loopcntr,$loopcntr,-2
    srli    $loopcntr,$loopcntr,1
1:
    # Grab next key in schedule
    add     $KEYP,$KEYP,16
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    @{[aes32esmi4 $T0,$Q0,$Q1,$Q2,$Q3]}
    @{[aes32esmi4 $T1,$Q1,$Q2,$Q3,$Q0]}
    @{[aes32esmi4 $T2,$Q2,$Q3,$Q0,$Q1]}
    @{[aes32esmi4 $T3,$Q3,$Q0,$Q1,$Q2]}
    # now T0~T3 hold the new state

    # Grab next key in schedule
    add     $KEYP,$KEYP,16
    lw      $Q0,0($KEYP)
    lw      $Q1,4($KEYP)
    lw      $Q2,8($KEYP)
    lw      $Q3,12($KEYP)

    @{[aes32esmi4 $Q0,$T0,$T1,$T2,$T3]}
    @{[aes32esmi4 $Q1,$T1,$T2,$T3,$T0]}
    @{[aes32esmi4 $Q2,$T2,$T3,$T0,$T1]}
    @{[aes32esmi4 $Q3,$T3,$T0,$T1,$T2]}
    # now Q0~Q3 hold the new state

    add     $loopcntr,$loopcntr,-1
    bgtz    $loopcntr,1b

# final two rounds
    # Grab next key in schedule
    add     $KEYP,$KEYP,16
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    @{[aes32esmi4 $T0,$Q0,$Q1,$Q2,$Q3]}
    @{[aes32esmi4 $T1,$Q1,$Q2,$Q3,$Q0]}
    @{[aes32esmi4 $T2,$Q2,$Q3,$Q0,$Q1]}
    @{[aes32esmi4 $T3,$Q3,$Q0,$Q1,$Q2]}
    # now T0~T3 hold the new state

    # Grab next key in schedule
    add     $KEYP,$KEYP,16
    lw      $Q0,0($KEYP)
    lw      $Q1,4($KEYP)
    lw      $Q2,8($KEYP)
    lw      $Q3,12($KEYP)

    # no mix column now
    @{[aes32esi4 $Q0,$T0,$T1,$T2,$T3]}
    @{[aes32esi4 $Q1,$T1,$T2,$T3,$T0]}
    @{[aes32esi4 $Q2,$T2,$T3,$T0,$T1]}
    @{[aes32esi4 $Q3,$T3,$T0,$T1,$T2]}
    # now Q0~Q3 hold the new state

    sw      $Q0,0($OUTP)
    sw      $Q1,4($OUTP)
    sw      $Q2,8($OUTP)
    sw      $Q3,12($OUTP)

    # Pop registers and return
___

$code .= load_regs();

$code .= <<___;
    ret
___

################################################################################
# void rv32i_zknd_decrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
$code .= <<___;
.text
.balign 16
.globl rv32i_zknd_decrypt
.type   rv32i_zknd_decrypt,\@function
rv32i_zknd_decrypt:
___

$code .= save_regs();

$code .= <<___;
    # Load input to block cipher
    lw      $Q0,0($INP)
    lw      $Q1,4($INP)
    lw      $Q2,8($INP)
    lw      $Q3,12($INP)

    # Load number of rounds
    lw      $loopcntr,240($KEYP)

    # Load the last key
    # use T0 as temporary now
    slli    $T0,$loopcntr,4
    add     $KEYP,$KEYP,$T0
    # Load key
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    # initial transformation
    xor     $Q0,$Q0,$T0
    xor     $Q1,$Q1,$T1
    xor     $Q2,$Q2,$T2
    xor     $Q3,$Q3,$T3

    # The main loop only executes the first N-2 rounds, each loop consumes two rounds
    add     $loopcntr,$loopcntr,-2
    srli    $loopcntr,$loopcntr,1
1:
    # Grab next key in schedule
    add     $KEYP,$KEYP,-16
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    @{[aes32dsmi4 $T0,$Q0,$Q3,$Q2,$Q1]}
    @{[aes32dsmi4 $T1,$Q1,$Q0,$Q3,$Q2]}
    @{[aes32dsmi4 $T2,$Q2,$Q1,$Q0,$Q3]}
    @{[aes32dsmi4 $T3,$Q3,$Q2,$Q1,$Q0]}
    # now T0~T3 hold the new state

    # Grab next key in schedule
    add     $KEYP,$KEYP,-16
    lw      $Q0,0($KEYP)
    lw      $Q1,4($KEYP)
    lw      $Q2,8($KEYP)
    lw      $Q3,12($KEYP)

    @{[aes32dsmi4 $Q0,$T0,$T3,$T2,$T1]}
    @{[aes32dsmi4 $Q1,$T1,$T0,$T3,$T2]}
    @{[aes32dsmi4 $Q2,$T2,$T1,$T0,$T3]}
    @{[aes32dsmi4 $Q3,$T3,$T2,$T1,$T0]}
    # now Q0~Q3 hold the new state

    add     $loopcntr,$loopcntr,-1
    bgtz    $loopcntr,1b

# final two rounds
    # Grab next key in schedule
    add     $KEYP,$KEYP,-16
    lw      $T0,0($KEYP)
    lw      $T1,4($KEYP)
    lw      $T2,8($KEYP)
    lw      $T3,12($KEYP)

    @{[aes32dsmi4 $T0,$Q0,$Q3,$Q2,$Q1]}
    @{[aes32dsmi4 $T1,$Q1,$Q0,$Q3,$Q2]}
    @{[aes32dsmi4 $T2,$Q2,$Q1,$Q0,$Q3]}
    @{[aes32dsmi4 $T3,$Q3,$Q2,$Q1,$Q0]}
    # now T0~T3 hold the new state

    # Grab next key in schedule
    add     $KEYP,$KEYP,-16
    lw      $Q0,0($KEYP)
    lw      $Q1,4($KEYP)
    lw      $Q2,8($KEYP)
    lw      $Q3,12($KEYP)

    # no mix column now
    @{[aes32dsi4 $Q0,$T0,$T3,$T2,$T1]}
    @{[aes32dsi4 $Q1,$T1,$T0,$T3,$T2]}
    @{[aes32dsi4 $Q2,$T2,$T1,$T0,$T3]}
    @{[aes32dsi4 $Q3,$T3,$T2,$T1,$T0]}
    # now Q0~Q3 hold the new state

    sw      $Q0,0($OUTP)
    sw      $Q1,4($OUTP)
    sw      $Q2,8($OUTP)
    sw      $Q3,12($OUTP)

    # Pop registers and return
___

$code .= load_regs();

$code .= <<___;
    ret
___

clear_regs();

################################################################################
# Register assignment for rv32i_zkn[e/d]_set_[en/de]crypt
################################################################################

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Pointer to user key, number of bits in key, key pointer
my ($UKEY,$BITS,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1,$T2,$T3,$T4,$T5,$T6,$T7,$T8) = use_regs(13..17,28..31);

################################################################################
# utility functions for rv32i_zkne_set_encrypt_key
################################################################################

my @rcon = (0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36);

# do 4 sbox on 4 bytes of rs, (possibly mix), then xor with rd
sub sbox4 {
    my $inst = shift;
    my $rd = shift;
    my $rs = shift;
    my $ret = <<___;
    @{[$inst->($rd,$rd,$rs,0)]}
    @{[$inst->($rd,$rd,$rs,1)]}
    @{[$inst->($rd,$rd,$rs,2)]}
    @{[$inst->($rd,$rd,$rs,3)]}
___
    return $ret;
}

sub fwdsbox4 {
    return sbox4(\&aes32esi, @_);
}

sub ke128enc {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
    while($rnum < 10) {
$ret .= <<___;
    # use T4 to store rcon
    li      $T4,$rcon[$rnum]
    # as xor is associative and commutative
    # we fist xor T0 with RCON, then use T0 to
    # xor the result of each SBOX result of T3
    xor     $T0,$T0,$T4
    # use T4 to store rotated T3
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T4,$T3,8]}
___
        } else {
$ret .= <<___;
    srli    $T4,$T3,8
    slli    $T5,$T3,24
    or      $T4,$T4,$T5
___
        }
$ret .= <<___;
    # update T0
    @{[fwdsbox4 $T0,$T4]}

    # update new T1~T3
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    add     $KEYP,$KEYP,16
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
        $rnum++;
    }
    return $ret;
}

sub ke192enc {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)
    lw      $T4,16($UKEY)
    lw      $T5,20($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)
___
    while($rnum < 8) {
$ret .= <<___;
    # see the comment in ke128enc
    li      $T6,$rcon[$rnum]
    xor     $T0,$T0,$T6
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T6,$T5,8]}
___
        } else {
$ret .= <<___;
    srli    $T6,$T5,8
    slli    $T7,$T5,24
    or      $T6,$T6,$T7
___
        }
$ret .= <<___;
    @{[fwdsbox4 $T0,$T6]}
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2
___
        if ($rnum != 7) {
        # note that (8+1)*24 = 216, (12+1)*16 = 208
        # thus the last 8 bytes can be dropped
$ret .= <<___;
    xor     $T4,$T4,$T3
    xor     $T5,$T5,$T4
___
        }
$ret .= <<___;
    add     $KEYP,$KEYP,24
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
        if ($rnum != 7) {
$ret .= <<___;
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke256enc {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)
    lw      $T4,16($UKEY)
    lw      $T5,20($UKEY)
    lw      $T6,24($UKEY)
    lw      $T7,28($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)
    sw      $T6,24($KEYP)
    sw      $T7,28($KEYP)
___
    while($rnum < 7) {
$ret .= <<___;
    # see the comment in ke128enc
    li      $T8,$rcon[$rnum]
    xor     $T0,$T0,$T8
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T8,$T7,8]}
___
        } else {
$ret .= <<___;
    srli    $T8,$T7,8
    slli    $BITS,$T7,24
    or      $T8,$T8,$BITS
___
        }
$ret .= <<___;
    @{[fwdsbox4 $T0,$T8]}
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    add     $KEYP,$KEYP,32
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
        if ($rnum != 6) {
        # note that (7+1)*32 = 256, (14+1)*16 = 240
        # thus the last 16 bytes can be dropped
$ret .= <<___;
    # for aes256, T3->T4 needs 4sbox but no rotate/rcon
    @{[fwdsbox4 $T4,$T3]}
    xor     $T5,$T5,$T4
    xor     $T6,$T6,$T5
    xor     $T7,$T7,$T6
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)
    sw      $T6,24($KEYP)
    sw      $T7,28($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

################################################################################
# void rv32i_zkne_set_encrypt_key(const unsigned char *userKey, const int bits,
#   AES_KEY *key)
################################################################################
sub AES_set_common {
    my ($ke128, $ke192, $ke256) = @_;
    my $ret = '';
$ret .= <<___;
    bnez    $UKEY,1f        # if (!userKey || !key) return -1;
    bnez    $KEYP,1f
    li      a0,-1
    ret
1:
    # Determine number of rounds from key size in bits
    li      $T0,128
    bne     $BITS,$T0,1f
    li      $T1,10          # key->rounds = 10 if bits == 128
    sw      $T1,240($KEYP)  # store key->rounds
$ke128
    j       4f
1:
    li      $T0,192
    bne     $BITS,$T0,2f
    li      $T1,12          # key->rounds = 12 if bits == 192
    sw      $T1,240($KEYP)  # store key->rounds
$ke192
    j       4f
2:
    li      $T1,14          # key->rounds = 14 if bits == 256
    li      $T0,256
    beq     $BITS,$T0,3f
    li      a0,-2           # If bits != 128, 192, or 256, return -2
    j       5f
3:
    sw      $T1,240($KEYP)  # store key->rounds
$ke256
4:  # return 0
    li      a0,0
5:  # return a0
___
    return $ret;
}
$code .= <<___;
.text
.balign 16
.globl rv32i_zkne_set_encrypt_key
.type rv32i_zkne_set_encrypt_key,\@function
rv32i_zkne_set_encrypt_key:
___

$code .= save_regs();
$code .= AES_set_common(ke128enc(0), ke192enc(0),ke256enc(0));
$code .= load_regs();
$code .= <<___;
    ret
___

################################################################################
# void rv32i_zbkb_zkne_set_encrypt_key(const unsigned char *userKey,
#   const int bits, AES_KEY *key)
################################################################################
$code .= <<___;
.text
.balign 16
.globl rv32i_zbkb_zkne_set_encrypt_key
.type rv32i_zbkb_zkne_set_encrypt_key,\@function
rv32i_zbkb_zkne_set_encrypt_key:
___

$code .= save_regs();
$code .= AES_set_common(ke128enc(1), ke192enc(1),ke256enc(1));
$code .= load_regs();
$code .= <<___;
    ret
___

################################################################################
# utility functions for rv32i_zknd_zkne_set_decrypt_key
################################################################################

sub invm4 {
    # fwd sbox then inv sbox then mix column
    # the result is only mix column
    # this simulates aes64im T0
    my $rd = shift;
    my $tmp = shift;
    my $rs = shift;
    my $ret = <<___;
    li      $tmp,0
    li      $rd,0
    @{[fwdsbox4 $tmp,$rs]}
    @{[sbox4(\&aes32dsmi, $rd,$tmp)]}
___
    return $ret;
}

sub ke128dec {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
    while($rnum < 10) {
$ret .= <<___;
    # see comments in ke128enc
    li      $T4,$rcon[$rnum]
    xor     $T0,$T0,$T4
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T4,$T3,8]}
___
        } else {
$ret .= <<___;
    srli    $T4,$T3,8
    slli    $T5,$T3,24
    or      $T4,$T4,$T5
___
        }
$ret .= <<___;
    @{[fwdsbox4 $T0,$T4]}
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2
    add     $KEYP,$KEYP,16
___
    # need to mixcolumn only for [1:N-1] round keys
    # this is from the fact that aes32dsmi subwords first then mix column
    # intuitively decryption needs to first mix column then subwords
    # however, for merging datapaths (encryption first subwords then mix column)
    # aes32dsmi chooses to inverse the order of them, thus
    # transform should then be done on the round key
        if ($rnum < 9) {
$ret .= <<___;
    # T4 and T5 are temp variables
    @{[invm4 $T5,$T4,$T0]}
    sw      $T5,0($KEYP)
    @{[invm4 $T5,$T4,$T1]}
    sw      $T5,4($KEYP)
    @{[invm4 $T5,$T4,$T2]}
    sw      $T5,8($KEYP)
    @{[invm4 $T5,$T4,$T3]}
    sw      $T5,12($KEYP)
___
        } else {
$ret .= <<___;
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke192dec {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)
    lw      $T4,16($UKEY)
    lw      $T5,20($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    # see the comment in ke128dec
    # T7 and T6 are temp variables
    @{[invm4 $T7,$T6,$T4]}
    sw      $T7,16($KEYP)
    @{[invm4 $T7,$T6,$T5]}
    sw      $T7,20($KEYP)
___
    while($rnum < 8) {
$ret .= <<___;
    # see the comment in ke128enc
    li      $T6,$rcon[$rnum]
    xor     $T0,$T0,$T6
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T6,$T5,8]}
___
        } else {
$ret .= <<___;
    srli    $T6,$T5,8
    slli    $T7,$T5,24
    or      $T6,$T6,$T7
___
        }
$ret .= <<___;
    @{[fwdsbox4 $T0,$T6]}
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    add     $KEYP,$KEYP,24
___
        if ($rnum < 7) {
$ret .= <<___;
    xor     $T4,$T4,$T3
    xor     $T5,$T5,$T4

    # see the comment in ke128dec
    # T7 and T6 are temp variables
    @{[invm4 $T7,$T6,$T0]}
    sw      $T7,0($KEYP)
    @{[invm4 $T7,$T6,$T1]}
    sw      $T7,4($KEYP)
    @{[invm4 $T7,$T6,$T2]}
    sw      $T7,8($KEYP)
    @{[invm4 $T7,$T6,$T3]}
    sw      $T7,12($KEYP)
    @{[invm4 $T7,$T6,$T4]}
    sw      $T7,16($KEYP)
    @{[invm4 $T7,$T6,$T5]}
    sw      $T7,20($KEYP)
___
        } else { # rnum == 7
$ret .= <<___;
    # the reason for dropping T4/T5 is in ke192enc
    # the reason for not invm4 is in ke128dec
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke256dec {
    my $zbkb = shift;
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    lw      $T0,0($UKEY)
    lw      $T1,4($UKEY)
    lw      $T2,8($UKEY)
    lw      $T3,12($UKEY)
    lw      $T4,16($UKEY)
    lw      $T5,20($UKEY)
    lw      $T6,24($UKEY)
    lw      $T7,28($UKEY)

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    # see the comment in ke128dec
    # BITS and T8 are temp variables
    # BITS are not used anymore
    @{[invm4 $T8,$BITS,$T4]}
    sw      $T8,16($KEYP)
    @{[invm4 $T8,$BITS,$T5]}
    sw      $T8,20($KEYP)
    @{[invm4 $T8,$BITS,$T6]}
    sw      $T8,24($KEYP)
    @{[invm4 $T8,$BITS,$T7]}
    sw      $T8,28($KEYP)
___
    while($rnum < 7) {
$ret .= <<___;
    # see the comment in ke128enc
    li      $T8,$rcon[$rnum]
    xor     $T0,$T0,$T8
___
        # right rotate by 8
        if ($zbkb) {
$ret .= <<___;
    @{[rori    $T8,$T7,8]}
___
        } else {
$ret .= <<___;
    srli    $T8,$T7,8
    slli    $BITS,$T7,24
    or      $T8,$T8,$BITS
___
        }
$ret .= <<___;
    @{[fwdsbox4 $T0,$T8]}
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    add     $KEYP,$KEYP,32
___
        if ($rnum < 6) {
$ret .= <<___;
    # for aes256, T3->T4 needs 4sbox but no rotate/rcon
    @{[fwdsbox4 $T4,$T3]}
    xor     $T5,$T5,$T4
    xor     $T6,$T6,$T5
    xor     $T7,$T7,$T6

    # see the comment in ke128dec
    # T8 and BITS are temp variables
    @{[invm4 $T8,$BITS,$T0]}
    sw      $T8,0($KEYP)
    @{[invm4 $T8,$BITS,$T1]}
    sw      $T8,4($KEYP)
    @{[invm4 $T8,$BITS,$T2]}
    sw      $T8,8($KEYP)
    @{[invm4 $T8,$BITS,$T3]}
    sw      $T8,12($KEYP)
    @{[invm4 $T8,$BITS,$T4]}
    sw      $T8,16($KEYP)
    @{[invm4 $T8,$BITS,$T5]}
    sw      $T8,20($KEYP)
    @{[invm4 $T8,$BITS,$T6]}
    sw      $T8,24($KEYP)
    @{[invm4 $T8,$BITS,$T7]}
    sw      $T8,28($KEYP)
___
        } else {
$ret .= <<___;
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    # last 16 bytes are dropped
    # see the comment in ke256enc
___
        }
        $rnum++;
    }
    return $ret;
}

################################################################################
# void rv32i_zknd_zkne_set_decrypt_key(const unsigned char *userKey, const int bits,
#   AES_KEY *key)
################################################################################
# a note on naming: set_decrypt_key needs aes32esi thus add zkne on name
$code .= <<___;
.text
.balign 16
.globl rv32i_zknd_zkne_set_decrypt_key
.type   rv32i_zknd_zkne_set_decrypt_key,\@function
rv32i_zknd_zkne_set_decrypt_key:
___
$code .= save_regs();
$code .= AES_set_common(ke128dec(0), ke192dec(0),ke256dec(0));
$code .= load_regs();
$code .= <<___;
    ret
___

################################################################################
# void rv32i_zbkb_zknd_zkne_set_decrypt_key(const unsigned char *userKey,
#   const int bits, AES_KEY *key)
################################################################################
$code .= <<___;
.text
.balign 16
.globl rv32i_zbkb_zknd_zkne_set_decrypt_key
.type rv32i_zbkb_zknd_zkne_set_decrypt_key,\@function
rv32i_zbkb_zknd_zkne_set_decrypt_key:
___

$code .= save_regs();
$code .= AES_set_common(ke128dec(1), ke192dec(1),ke256dec(1));
$code .= load_regs();
$code .= <<___;
    ret
___



print $code;
close STDOUT or die "error closing STDOUT: $!";
