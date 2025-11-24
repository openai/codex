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
            $ret.="    sd      $_,$stack_offset(sp)\n";
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
            $ret.="    ld      $_,$stack_offset(sp)\n";
        }
        $ret.="    addi    sp,sp,$stack_reservation\n";
        return $ret;
    }
    sub clear_regs {
        @must_save = ();
    }
}

################################################################################
# Register assignment for rv64i_zkne_encrypt and rv64i_zknd_decrypt
################################################################################

# Registers to hold AES state (called s0-s3 or y0-y3 elsewhere)
my ($Q0,$Q1,$Q2,$Q3) = use_regs(6..9);

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Input block pointer, output block pointer, key pointer
my ($INP,$OUTP,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1) = use_regs(13..14);

# Loop counter
my ($loopcntr) = use_regs(30);

################################################################################
# void rv64i_zkne_encrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
my $code .= <<___;
.text
.balign 16
.globl rv64i_zkne_encrypt
.type   rv64i_zkne_encrypt,\@function
rv64i_zkne_encrypt:
___

$code .= save_regs();

$code .= <<___;

    # Load input to block cipher
    ld      $Q0,0($INP)
    ld      $Q1,8($INP)

    # Load key
    ld      $T0,0($KEYP)
    ld      $T1,8($KEYP)

    # Load number of rounds
    lwu     $loopcntr,240($KEYP)

    # initial transformation
    xor     $Q0,$Q0,$T0
    xor     $Q1,$Q1,$T1

    # The main loop only executes the first N-1 rounds.
    add     $loopcntr,$loopcntr,-1

    # Do Nr - 1 rounds (final round is special)
1:
    @{[aes64esm $Q2,$Q0,$Q1]}
    @{[aes64esm $Q3,$Q1,$Q0]}

    # Update key ptr to point to next key in schedule
    add     $KEYP,$KEYP,16

    # Grab next key in schedule
    ld      $T0,0($KEYP)
    ld      $T1,8($KEYP)
    xor     $Q0,$Q2,$T0
    xor     $Q1,$Q3,$T1

    add     $loopcntr,$loopcntr,-1
    bgtz    $loopcntr,1b

    # final round
    @{[aes64es $Q2,$Q0,$Q1]}
    @{[aes64es $Q3,$Q1,$Q0]}

    # since not added 16 before
    ld      $T0,16($KEYP)
    ld      $T1,24($KEYP)
    xor     $Q0,$Q2,$T0
    xor     $Q1,$Q3,$T1

    sd      $Q0,0($OUTP)
    sd      $Q1,8($OUTP)

    # Pop registers and return
___

$code .= load_regs();

$code .= <<___;
    ret
___

################################################################################
# void rv64i_zknd_decrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
$code .= <<___;
.text
.balign 16
.globl rv64i_zknd_decrypt
.type   rv64i_zknd_decrypt,\@function
rv64i_zknd_decrypt:
___

$code .= save_regs();

$code .= <<___;

    # Load input to block cipher
    ld      $Q0,0($INP)
    ld      $Q1,8($INP)

    # Load number of rounds
    lwu     $loopcntr,240($KEYP)

    # Load the last key
    slli    $T0,$loopcntr,4
    add     $KEYP,$KEYP,$T0
    ld      $T0,0($KEYP)
    ld      $T1,8($KEYP)

    xor     $Q0,$Q0,$T0
    xor     $Q1,$Q1,$T1

    # The main loop only executes the first N-1 rounds.
    add     $loopcntr,$loopcntr,-1

    # Do Nr - 1 rounds (final round is special)
1:
    @{[aes64dsm $Q2,$Q0,$Q1]}
    @{[aes64dsm $Q3,$Q1,$Q0]}

    # Update key ptr to point to next key in schedule
    add     $KEYP,$KEYP,-16

    # Grab next key in schedule
    ld      $T0,0($KEYP)
    ld      $T1,8($KEYP)
    xor     $Q0,$Q2,$T0
    xor     $Q1,$Q3,$T1

    add     $loopcntr,$loopcntr,-1
    bgtz    $loopcntr,1b

    # final round
    @{[aes64ds $Q2,$Q0,$Q1]}
    @{[aes64ds $Q3,$Q1,$Q0]}

    add     $KEYP,$KEYP,-16
    ld      $T0,0($KEYP)
    ld      $T1,8($KEYP)
    xor     $Q0,$Q2,$T0
    xor     $Q1,$Q3,$T1

    sd      $Q0,0($OUTP)
    sd      $Q1,8($OUTP)
    # Pop registers and return
___

$code .= load_regs();

$code .= <<___;
    ret
___

clear_regs();

################################################################################
# Register assignment for rv64i_zkn[e/d]_set_[en/de]crypt_key
################################################################################

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Pointer to user key, number of bits in key, key pointer
my ($UKEY,$BITS,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1,$T2,$T3,$T4) = use_regs(6..8,13..14);

################################################################################
# utility functions for rv64i_zkne_set_encrypt_key
################################################################################
sub ke128enc {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
___
    while($rnum < 10) {
$ret .= <<___;
    @{[aes64ks1i   $T2,$T1,$rnum]}
    @{[aes64ks2    $T0,$T2,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
    add         $KEYP,$KEYP,16
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
___
        $rnum++;
    }
    return $ret;
}

sub ke192enc {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    ld      $T2,16($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
    sd      $T2,16($KEYP)
___
    while($rnum < 8) {
$ret .= <<___;
    @{[aes64ks1i   $T3,$T2,$rnum]}
    @{[aes64ks2    $T0,$T3,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
___
        if ($rnum != 7) {
        # note that (8+1)*24 = 216, (12+1)*16 = 208
        # thus the last 8 bytes can be dropped
$ret .= <<___;
    @{[aes64ks2    $T2,$T1,$T2]}
___
        }
$ret .= <<___;
    add         $KEYP,$KEYP,24
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
___
        if ($rnum != 7) {
$ret .= <<___;
    sd          $T2,16($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke256enc {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    ld      $T2,16($UKEY)
    ld      $T3,24($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
    sd      $T2,16($KEYP)
    sd      $T3,24($KEYP)
___
    while($rnum < 7) {
$ret .= <<___;
    @{[aes64ks1i   $T4,$T3,$rnum]}
    @{[aes64ks2    $T0,$T4,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
    add         $KEYP,$KEYP,32
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
___
        if ($rnum != 6) {
        # note that (7+1)*32 = 256, (14+1)*16 = 240
        # thus the last 16 bytes can be dropped
$ret .= <<___;
    @{[aes64ks1i   $T4,$T1,0xA]}
    @{[aes64ks2    $T2,$T4,$T2]}
    @{[aes64ks2    $T3,$T2,$T3]}
    sd          $T2,16($KEYP)
    sd          $T3,24($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

################################################################################
# void rv64i_zkne_set_encrypt_key(const unsigned char *userKey, const int bits,
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
.globl rv64i_zkne_set_encrypt_key
.type   rv64i_zkne_set_encrypt_key,\@function
rv64i_zkne_set_encrypt_key:
___
$code .= save_regs();
$code .= AES_set_common(ke128enc(), ke192enc(),ke256enc());
$code .= load_regs();
$code .= <<___;
    ret
___

################################################################################
# utility functions for rv64i_zknd_set_decrypt_key
################################################################################
sub ke128dec {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
___
    while($rnum < 10) {
$ret .= <<___;
    @{[aes64ks1i   $T2,$T1,$rnum]}
    @{[aes64ks2    $T0,$T2,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
    add         $KEYP,$KEYP,16
___
    # need to aes64im for [1:N-1] round keys
    # this is from the fact that aes64dsm subwords first then mix column
    # intuitively decryption needs to first mix column then subwords
    # however, for merging datapaths (encryption first subwords then mix column)
    # aes64dsm chooses to inverse the order of them, thus
    # transform should then be done on the round key
        if ($rnum < 9) {
$ret .= <<___;
    @{[aes64im     $T2,$T0]}
    sd          $T2,0($KEYP)
    @{[aes64im     $T2,$T1]}
    sd          $T2,8($KEYP)
___
        } else {
$ret .= <<___;
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke192dec {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    ld      $T2,16($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
    @{[aes64im $T3,$T2]}
    sd      $T3,16($KEYP)
___
    while($rnum < 8) {
$ret .= <<___;
    @{[aes64ks1i   $T3,$T2,$rnum]}
    @{[aes64ks2    $T0,$T3,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
    add         $KEYP,$KEYP,24
___
        if ($rnum < 7) {
$ret .= <<___;
    @{[aes64im     $T3,$T0]}
    sd          $T3,0($KEYP)
    @{[aes64im     $T3,$T1]}
    sd          $T3,8($KEYP)
    # the reason is in ke192enc
    @{[aes64ks2    $T2,$T1,$T2]}
    @{[aes64im     $T3,$T2]}
    sd          $T3,16($KEYP)
___
        } else { # rnum == 7
$ret .= <<___;
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
___
        }
        $rnum++;
    }
    return $ret;
}

sub ke256dec {
    my $rnum = 0;
    my $ret = '';
$ret .= <<___;
    ld      $T0,0($UKEY)
    ld      $T1,8($UKEY)
    ld      $T2,16($UKEY)
    ld      $T3,24($UKEY)
    sd      $T0,0($KEYP)
    sd      $T1,8($KEYP)
    @{[aes64im $T4,$T2]}
    sd      $T4,16($KEYP)
    @{[aes64im $T4,$T3]}
    sd      $T4,24($KEYP)
___
    while($rnum < 7) {
$ret .= <<___;
    @{[aes64ks1i   $T4,$T3,$rnum]}
    @{[aes64ks2    $T0,$T4,$T0]}
    @{[aes64ks2    $T1,$T0,$T1]}
    add         $KEYP,$KEYP,32
___
        if ($rnum < 6) {
$ret .= <<___;
    @{[aes64ks1i   $T4,$T1,0xA]}
    @{[aes64ks2    $T2,$T4,$T2]}
    @{[aes64ks2    $T3,$T2,$T3]}
    @{[aes64im     $T4,$T0]}
    sd          $T4,0($KEYP)
    @{[aes64im     $T4,$T1]}
    sd          $T4,8($KEYP)
    @{[aes64im     $T4,$T2]}
    sd          $T4,16($KEYP)
    @{[aes64im     $T4,$T3]}
    sd          $T4,24($KEYP)
___
        } else {
$ret .= <<___;
    sd          $T0,0($KEYP)
    sd          $T1,8($KEYP)
    # last two one dropped
___
        }
        $rnum++;
    }
    return $ret;
}

################################################################################
# void rv64i_zknd_set_decrypt_key(const unsigned char *userKey, const int bits,
#   AES_KEY *key)
################################################################################
$code .= <<___;
.text
.balign 16
.globl rv64i_zknd_set_decrypt_key
.type   rv64i_zknd_set_decrypt_key,\@function
rv64i_zknd_set_decrypt_key:
___
$code .= save_regs();
$code .= AES_set_common(ke128dec(), ke192dec(),ke256dec());
$code .= load_regs();
$code .= <<___;
    ret
___

print $code;
close STDOUT or die "error closing STDOUT: $!";
