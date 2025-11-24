#! /usr/bin/env perl
# Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

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
# Register assignment for AES_encrypt and AES_decrypt
################################################################################

# Registers to hold AES state (called s0-s3 or y0-y3 elsewhere)
my ($Q0,$Q1,$Q2,$Q3) = use_regs(6..9);

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Input block pointer, output block pointer, key pointer
my ($INP,$OUTP,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1,$T2,$T3) = use_regs(13..16);
my ($T4,$T5,$T6,$T7,$T8,$T9,$T10,$T11) = use_regs(17..24);
my ($T12,$T13,$T14,$T15) = use_regs(25..28);

# Register to hold table offset
my ($I0) = use_regs(29);

# Loop counter
my ($loopcntr) = use_regs(30);

# Lookup table address register
my ($TBL) = use_regs(31);

# Lookup table mask register
my ($MSK) = use_regs(5);

# Aliases for readability
my $K0 = $loopcntr;
my $K1 = $KEYP;

################################################################################
# Table lookup utility functions for AES_encrypt and AES_decrypt
################################################################################

# do_lookup([destination regs], [state regs], [temporary regs], shamt)
# do_lookup loads four entries from an AES encryption/decryption table
# and stores the result in the specified destination register set
# Ds->[0] = Table[Qs->[0] >> shamt]
# Ds->[1] = Table[Qs->[1] >> shamt]
# Ds->[2] = Table[Qs->[2] >> shamt]
# Ds->[3] = Table[Qs->[3] >> shamt]
# Four temporary regs are used to generate these lookups. The temporary regs
# can be equal to the destination regs, but only if they appear in the same
# order. I.e. do_lookup([A,B,C,D],[...],[A,B,C,D],...) is OK
sub do_lookup {
    # (destination regs, state regs, temporary regs, shift amount)
    my ($Ds, $Qs, $Ts, $shamt) = @_;

    my $ret = '';

    # AES encryption/decryption table entries have word-sized (4-byte) entries.
    # To convert the table index into a byte offset, we compute
    # ((Qs->[i] >> shamt) & 0xFF) << 2
    # However, to save work, we compute the equivalent expression
    # (Qs->[i] >> (shamt-2)) & 0x3FC
    if ($shamt < 2) {
$ret .= <<___;

    slli    $Ts->[0],$Qs->[0],$shamt+2
    slli    $Ts->[1],$Qs->[1],$shamt+2
    slli    $Ts->[2],$Qs->[2],$shamt+2
    slli    $Ts->[3],$Qs->[3],$shamt+2
___
    } else {
$ret .= <<___;

    srli    $Ts->[0],$Qs->[0],$shamt-2
    srli    $Ts->[1],$Qs->[1],$shamt-2
    srli    $Ts->[2],$Qs->[2],$shamt-2
    srli    $Ts->[3],$Qs->[3],$shamt-2
___
    }

$ret .= <<___;

    andi    $Ts->[0],$Ts->[0],0x3FC
    andi    $Ts->[1],$Ts->[1],0x3FC
    andi    $Ts->[2],$Ts->[2],0x3FC
    andi    $Ts->[3],$Ts->[3],0x3FC

    # Index into table.
    add     $I0,$TBL,$Ts->[0]
    lwu     $Ds->[0],0($I0)
    add     $I0,$TBL,$Ts->[1]
    lwu     $Ds->[1],0($I0)
    add     $I0,$TBL,$Ts->[2]
    lwu     $Ds->[2],0($I0)
    add     $I0,$TBL,$Ts->[3]
    lwu     $Ds->[3],0($I0)

___

    return $ret;
}

# Identical to do_lookup(), but loads only a single byte into each destination
# register (replaces lwu with lbu). Used in the final round of AES_encrypt.
sub do_lookup_byte {
    my $ret = do_lookup(@_);
    $ret =~ s/lwu/lbu/g;
    return $ret;
}

# do_lookup_Td4([destination regs], [state regs], [temporary regs])
# Used in final phase of AES_decrypt
# Ds->[0] = Table[(Qs->[0])      &0xFF]
# Ds->[1] = Table[(Qs->[1] >> 8 )&0xFF]
# Ds->[2] = Table[(Qs->[2] >> 16)&0xFF]
# Ds->[3] = Table[(Qs->[3] >> 24)&0xFF]
# Four temporary regs are used to generate these lookups. The temporary regs
# can be equal to the destination regs, but only if they appear in the same
# order. I.e. do_lookup([A,B,C,D],[...],[A,B,C,D],...) is OK
sub do_lookup_Td4 {
    my ($Ds, $Qs, $Ts) = @_;

    my $ret = '';

$ret .= <<___;
    srli    $Ts->[1],$Qs->[1],8
    srli    $Ts->[2],$Qs->[2],16
    srli    $Ts->[3],$Qs->[3],24

    andi    $Ts->[0],$Qs->[0],0xFF
    andi    $Ts->[1],$Ts->[1],0xFF
    andi    $Ts->[2],$Ts->[2],0xFF
    andi    $Ts->[3],$Ts->[3],0xFF

    add     $I0,$TBL,$Ts->[0]
    lbu     $Ds->[0],0($I0)
    add     $I0,$TBL,$Ts->[1]
    lbu     $Ds->[1],0($I0)
    add     $I0,$TBL,$Ts->[2]
    lbu     $Ds->[2],0($I0)
    add     $I0,$TBL,$Ts->[3]
    lbu     $Ds->[3],0($I0)

___

    return $ret;
}

################################################################################
# void AES_encrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
my $code .= <<___;
.text
.balign 16
.globl AES_encrypt
.type   AES_encrypt,\@function
AES_encrypt:
___

$code .= save_regs();

$code .= <<___;

    # Load input to block cipher
    ld      $Q0,0($INP)
    ld      $Q2,8($INP)


    # Load key
    ld      $T0,0($KEYP)
    ld      $T2,8($KEYP)


    # Load number of rounds
    lwu     $loopcntr,240($KEYP)

    # Load address of substitution table and wrap-around mask
    la      $TBL,AES_Te0
    li      $MSK,~0xFFF

    # y = n xor k, stored in Q0-Q3

    xor     $Q0,$Q0,$T0
    xor     $Q2,$Q2,$T2
    srli    $Q1,$Q0,32
    srli    $Q3,$Q2,32

    # The main loop only executes the first N-1 rounds.
    add     $loopcntr,$loopcntr,-1

    # Do Nr - 1 rounds (final round is special)

1:
___

# Lookup in table Te0
$code .= do_lookup(
    [$T4,$T5,$T6,$T7],  # Destination registers
    [$Q0,$Q1,$Q2,$Q3],  # State registers
    [$T0,$T1,$T2,$T3],  # Temporaries
    0                   # Shift amount
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in table Te1
$code .= do_lookup(
    [$T8,$T9,$T10,$T11],
    [$Q1,$Q2,$Q3,$Q0],
    [$T0,$T1,$T2,$T3],
    8
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in table Te2
$code .= do_lookup(
    [$T12,$T13,$T14,$T15],
    [$Q2,$Q3,$Q0,$Q1],
    [$T0,$T1,$T2,$T3],
    16
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in table Te3
$code .= do_lookup(
    [$T0,$T1,$T2,$T3],
    [$Q3,$Q0,$Q1,$Q2],
    [$T0,$T1,$T2,$T3],
    24
);

$code .= <<___;

    # Combine table lookups
    xor     $T4,$T4,$T8
    xor     $T5,$T5,$T9
    xor     $T6,$T6,$T10
    xor     $T7,$T7,$T11

    xor     $T4,$T4,$T12
    xor     $T5,$T5,$T13
    xor     $T6,$T6,$T14
    xor     $T7,$T7,$T15

    xor     $T0,$T0,$T4
    xor     $T1,$T1,$T5
    xor     $T2,$T2,$T6
    xor     $T3,$T3,$T7

    # Update key ptr to point to next key in schedule
    add     $KEYP,$KEYP,16

    # Grab next key in schedule
    ld      $T4,0($KEYP)
    ld      $T6,8($KEYP)

    # Round TBL back to 4k boundary
    and     $TBL,$TBL,$MSK

    add     $loopcntr,$loopcntr,-1

    xor     $Q0,$T0,$T4
    xor     $Q2,$T2,$T6
    srli    $T5,$T4,32
    xor     $Q1,$T1,$T5
    srli    $T7,$T6,32
    xor     $Q3,$T3,$T7

    bgtz    $loopcntr,1b

#================================FINAL ROUND====================================

# In the final round, all lookup table accesses would appear as follows:
#
# ... compute index I0
# add I0,TBL,T0
# lbu T0,1(I0)
#
# Instead of indexing with a 1 offset, we can add 1 to the TBL pointer, and use
# a 0 offset when indexing in the following code. This enables some instruction
# fusion opportunities.

    add     $TBL,$TBL,1

    ld      $K0,16($KEYP)
    ld      $K1,24($KEYP)
___

$code .= do_lookup_byte(
    [$T4,$T5,$T6,$T7],
    [$Q0,$Q1,$Q2,$Q3],
    [$T0,$T1,$T2,$T3],
    0
);

$code .= do_lookup_byte(
    [$T8,$T9,$T10,$T11],
    [$Q1,$Q2,$Q3,$Q0],
    [$T0,$T1,$T2,$T3],
    8
);

$code .= do_lookup_byte(
    [$T12,$T13,$T14,$T15],
    [$Q2,$Q3,$Q0,$Q1],
    [$T0,$T1,$T2,$T3],
    16
);

$code .= do_lookup_byte(
    [$T0,$T1,$T2,$T3],
    [$Q3,$Q0,$Q1,$Q2],
    [$T0,$T1,$T2,$T3],
    24
);

$code .= <<___;

    # Combine table lookups into T0 and T2

    slli    $T5,$T5,32
    slli    $T7,$T7,32
    slli    $T8,$T8,8
    slli    $T9,$T9,8+32
    slli    $T10,$T10,8
    slli    $T11,$T11,8+32
    slli    $T12,$T12,16
    slli    $T13,$T13,16+32
    slli    $T14,$T14,16
    slli    $T15,$T15,16+32

    slli    $T0,$T0,24
    slli    $T1,$T1,24+32
    slli    $T2,$T2,24
    slli    $T3,$T3,24+32

    xor     $T4,$T4,$T0
    xor     $T5,$T5,$T1
    xor     $T6,$T6,$T2
    xor     $T7,$T7,$T3

    xor     $T8,$T8,$T12
    xor     $T9,$T9,$T13
    xor     $T10,$T10,$T14
    xor     $T11,$T11,$T15

    xor     $T0,$T4,$T8
    xor     $T1,$T5,$T9
    xor     $T2,$T6,$T10
    xor     $T3,$T7,$T11


    xor     $T0,$T0,$T1
    # T0 = [T1 T13 T9 T5 T0 T12 T8 T4]
    xor     $T0,$T0,$K0 # XOR in key

    xor     $T2,$T2,$T3
    # T2 = [T3 T15 T11 T7 T2 T14 T10 T6]
    xor     $T2,$T2,$K1 # XOR in key

    sd      $T0,0($OUTP)
    sd      $T2,8($OUTP)

    # Pop registers and return
2:
___

$code .= load_regs();

$code .= <<___;
    ret
___

################################################################################
# void AES_decrypt(const unsigned char *in, unsigned char *out,
#   const AES_KEY *key);
################################################################################
$code .= <<___;
.text
.balign 16
.globl AES_decrypt
.type   AES_decrypt,\@function
AES_decrypt:
___

$code .= save_regs();

$code .= <<___;

    # Load input to block cipher
    ld      $Q0,0($INP)
    ld      $Q2,8($INP)

    # Load key
    # Note that key is assumed in BE byte order
    # (This routine was written against a key scheduling implementation that
    #  placed keys in BE byte order.)
    ld      $T0,0($KEYP)
    ld      $T2,8($KEYP)

    # Load number of rounds
    lwu     $loopcntr,240($KEYP)

    # Load address of substitution table and wrap-around mask
    la      $TBL,AES_Td0
    li      $MSK,~0xFFF

    xor     $Q0,$Q0,$T0
    xor     $Q2,$Q2,$T2
    srli    $Q1,$Q0,32
    srli    $Q3,$Q2,32

    # The main loop only executes the first N-1 rounds.
    add     $loopcntr,$loopcntr,-1

    # Do Nr - 1 rounds (final round is special)
1:
___

# Lookup in Td0
$code .= do_lookup(
    [$T4,$T5,$T6,$T7],  # Destination registers
    [$Q0,$Q1,$Q2,$Q3],  # State registers
    [$T0,$T1,$T2,$T3],  # Temporaries
    0                   # Shift amount
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in Td1
$code .= do_lookup(
    [$T8,$T9,$T10,$T11],
    [$Q3,$Q0,$Q1,$Q2],
    [$T0,$T1,$T2,$T3],
    8
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in Td2
$code .= do_lookup(
    [$T12,$T13,$T14,$T15],
    [$Q2,$Q3,$Q0,$Q1],
    [$T0,$T1,$T2,$T3],
    16
);

$code .= <<___;
    add     $TBL,$TBL,1024
___

# Lookup in Td3
$code .= do_lookup(
    [$T0,$T1,$T2,$T3],
    [$Q1,$Q2,$Q3,$Q0],
    [$T0,$T1,$T2,$T3],
    24
);

$code .= <<___;
    xor     $T4,$T4,$T8
    xor     $T5,$T5,$T9
    xor     $T6,$T6,$T10
    xor     $T7,$T7,$T11

    xor     $T4,$T4,$T12
    xor     $T5,$T5,$T13
    xor     $T6,$T6,$T14
    xor     $T7,$T7,$T15

    xor     $T0,$T0,$T4
    xor     $T1,$T1,$T5
    xor     $T2,$T2,$T6
    xor     $T3,$T3,$T7

    # Update key ptr to point to next key in schedule
    add     $KEYP,$KEYP,16

    # Grab next key in schedule
    ld      $T4,0($KEYP)
    ld      $T6,8($KEYP)

    # Round TBL back to 4k boundary
    and     $TBL,$TBL,$MSK

    add     $loopcntr,$loopcntr,-1

    xor     $Q0,$T0,$T4
    xor     $Q2,$T2,$T6
    srli    $T5,$T4,32
    xor     $Q1,$T1,$T5
    srli    $T7,$T6,32
    xor     $Q3,$T3,$T7

    bgtz    $loopcntr,1b

#================================FINAL ROUND====================================

    la      $TBL,AES_Td4

    # K0,K1 are aliases for loopcntr,KEYP
    # As these registers will no longer be used after these loads, reuse them
    # to store the final key in the schedule.
    ld      $K0,16($KEYP)
    ld      $K1,24($KEYP)
___

$code .= do_lookup_Td4(
    [$T4,$T5,$T6,$T7],
    [$Q0,$Q3,$Q2,$Q1],
    [$T0,$T1,$T2,$T3]
);

$code .= do_lookup_Td4(
    [$T8,$T9,$T10,$T11],
    [$Q1,$Q0,$Q3,$Q2],
    [$T0,$T1,$T2,$T3]
);

$code .= do_lookup_Td4(
    [$T12,$T13,$T14,$T15],
    [$Q2,$Q1,$Q0,$Q3],
    [$T0,$T1,$T2,$T3]
);

$code .= do_lookup_Td4(
    [$T0,$T1,$T2,$T3],
    [$Q3,$Q2,$Q1,$Q0],
    [$T0,$T1,$T2,$T3]
);

$code .= <<___;

    # T0-T15 now contain the decrypted block, minus xoring with the final round
    # key. We pack T0-T15 into the two 64-bit registers T0 and T4, then xor
    # in the key and store.

    slli    $T5,$T5,8
    slli    $T6,$T6,16
    slli    $T7,$T7,24
    slli    $T8,$T8,32
    slli    $T9,$T9,8+32
    slli    $T10,$T10,16+32
    slli    $T11,$T11,32+24
    slli    $T13,$T13,8
    slli    $T14,$T14,16
    slli    $T15,$T15,24
    slli    $T0,$T0,32
    slli    $T1,$T1,8+32
    slli    $T2,$T2,16+32
    slli    $T3,$T3,24+32

    xor     $T4,$T4,$T5
    xor     $T6,$T6,$T7
    xor     $T8,$T8,$T9
    xor     $T10,$T10,$T11

    xor     $T12,$T12,$T13
    xor     $T14,$T14,$T15
    xor     $T0,$T0,$T1
    xor     $T2,$T2,$T3

    xor     $T4,$T4,$T6
    xor     $T8,$T8,$T10
    xor     $T12,$T12,$T14
    xor     $T0,$T0,$T2

    xor     $T4,$T4,$T8
    # T4 = [T11 T10 T9 T8   T7 T6 T5 T4]
    xor     $T4,$T4,$K0 # xor in key

    xor     $T0,$T0,$T12
    # T0 = [T3 T2 T1 T0   T15 T14 T13 T12]
    xor     $T0,$T0,$K1 # xor in key

    sd      $T4,0($OUTP)
    sd      $T0,8($OUTP)

    # Pop registers and return
___

$code .= load_regs();

$code .= <<___;
    ret
___

clear_regs();

################################################################################
# Register assignment for AES_set_encrypt_key
################################################################################

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Pointer to user key, number of bits in key, key pointer
my ($UKEY,$BITS,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1,$T2,$T3) = use_regs(6..8,13);
my ($T4,$T5,$T6,$T7,$T8,$T9,$T10,$T11) = use_regs(14..17,28..31);

# Pointer into rcon table
my ($RCON) = use_regs(9);

# Register to hold table offset and used as a temporary
my ($I0) = use_regs(18);

# Loop counter
my ($loopcntr) = use_regs(19);

# Lookup table address register
my ($TBL) = use_regs(20);

# Calculates dest = [
#   S[(in>>shifts[3])&0xFF],
#   S[(in>>shifts[2])&0xFF],
#   S[(in>>shifts[1])&0xFF],
#   S[(in>>shifts[0])&0xFF]
#   ]
#   This routine spreads accesses across Te0-Te3 to help bring those tables
#   into cache, in anticipation of running AES_[en/de]crypt.
sub do_enc_lookup {
    # (destination reg, input reg, shifts array, temporary regs)
    my ($dest, $in, $shifts, $Ts) = @_;

    my $ret = '';

$ret .= <<___;

    # Round TBL back to 4k boundary
    srli    $TBL,$TBL,12
    slli    $TBL,$TBL,12

    # Offset by 1 byte, since Te0[x] = S[x].[03, 01, 01, 02]
    # So that, later on, a 0-offset lbu yields S[x].01 == S[x]
    addi    $TBL,$TBL,1
___

    for ($i = 0; $i < 4; $i++) {
        if ($shifts->[$i] < 2) {
            $ret .= "    slli    $Ts->[$i],$in,2-$shifts->[$i]\n";
        } else {
            $ret .= "    srli    $Ts->[$i],$in,$shifts->[$i]-2\n";
        }
    }

$ret .= <<___;

    andi    $Ts->[0],$Ts->[0],0x3FC
    andi    $Ts->[1],$Ts->[1],0x3FC
    andi    $Ts->[2],$Ts->[2],0x3FC
    andi    $Ts->[3],$Ts->[3],0x3FC

    # Index into tables Te0-Te3 (spread access across tables to help bring
    # them into cache for later)

    add     $I0,$TBL,$Ts->[0]
    lbu     $Ts->[0],0($I0)

    add     $TBL,$TBL,1025      # yes, 1025
    add     $I0,$TBL,$Ts->[1]
    lbu     $Ts->[1],0($I0)

    add     $TBL,$TBL,1025
    add     $I0,$TBL,$Ts->[2]
    lbu     $Ts->[2],0($I0)

    add     $TBL,$TBL,1022
    add     $I0,$TBL,$Ts->[3]
    lbu     $Ts->[3],0($I0)

    slli    $Ts->[1],$Ts->[1],8
    slli    $Ts->[2],$Ts->[2],16
    slli    $Ts->[3],$Ts->[3],24

    xor     $Ts->[0],$Ts->[0],$Ts->[1]
    xor     $Ts->[2],$Ts->[2],$Ts->[3]
    xor     $dest,$Ts->[0],$Ts->[2]
___

    return $ret;
}

################################################################################
# void AES_set_encrypt_key(const unsigned char *userKey, const int bits,
#   AES_KEY *key)
################################################################################
$code .= <<___;
.text
.balign 16
.globl AES_set_encrypt_key
.type   AES_set_encrypt_key,\@function
AES_set_encrypt_key:
___
$code .= save_regs();
$code .= <<___;
    bnez    $UKEY,1f    # if (!userKey || !key) return -1;
    bnez    $KEYP,1f
    li      a0,-1
    ret
1:
    la      $RCON,AES_rcon
    la      $TBL,AES_Te0
    li      $T8,128
    li      $T9,192
    li      $T10,256

    # Determine number of rounds from key size in bits
    bne     $BITS,$T8,1f
    li      $T3,10          # key->rounds = 10 if bits == 128
    j       3f
1:
    bne     $BITS,$T9,2f
    li      $T3,12          # key->rounds = 12 if bits == 192
    j       3f
2:
    li      $T3,14          # key->rounds = 14 if bits == 256
    beq     $BITS,$T10,3f
    li      a0,-2           # If bits != 128, 192, or 256, return -2
    j       5f
3:
    ld      $T0,0($UKEY)
    ld      $T2,8($UKEY)

    sw      $T3,240($KEYP)

    li      $loopcntr,0     # == i*4

    srli    $T1,$T0,32
    srli    $T3,$T2,32

    sd      $T0,0($KEYP)
    sd      $T2,8($KEYP)

    # if bits == 128
    # jump into loop
    beq     $BITS,$T8,1f

    ld      $T4,16($UKEY)
    srli    $T5,$T4,32
    sd      $T4,16($KEYP)

    # if bits == 192
    # jump into loop
    beq     $BITS,$T9,2f

    ld      $T6,24($UKEY)
    srli    $T7,$T6,32
    sd      $T6,24($KEYP)

    # bits == 256
    j       3f
___

$code .= <<___;
1:
    addi    $KEYP,$KEYP,16
1:
___
$code .= do_enc_lookup($T4,$T3,[8,16,24,0],[$T4,$T5,$T6,$T7]);

$code .= <<___;
    add     $T5,$RCON,$loopcntr # rcon[i] (i increments by 4 so it can double as
                                # a word offset)
    lwu     $T5,0($T5)

    addi    $loopcntr,$loopcntr,4
    li      $I0,10*4

    xor     $T0,$T0,$T4
    xor     $T0,$T0,$T5
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)

    addi    $KEYP,$KEYP,16


    bne     $loopcntr,$I0,1b
    j       4f
___
$code .= <<___;
2:
    addi    $KEYP,$KEYP,24
2:
___
$code .= do_enc_lookup($T6,$T5,[8,16,24,0],[$T6,$T7,$T8,$T9]);

$code .= <<___;
    add     $T7,$RCON,$loopcntr # rcon[i] (i increments by 4 so it can double as
                                # a word offset)
    lwu     $T7,0($T7)

    addi    $loopcntr,$loopcntr,4
    li      $I0,8*4

    xor     $T0,$T0,$T6
    xor     $T0,$T0,$T7
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)

    beq     $loopcntr,$I0,4f

    xor     $T4,$T4,$T3
    xor     $T5,$T5,$T4
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)

    addi    $KEYP,$KEYP,24
    j       2b
___
$code .= <<___;
3:
    addi    $KEYP,$KEYP,32
3:
___
$code .= do_enc_lookup($T8,$T7,[8,16,24,0],[$T8,$T9,$T10,$T11]);

$code .= <<___;
    add     $T9,$RCON,$loopcntr # rcon[i] (i increments by 4 so it can double as
                                # a word offset)
    lwu     $T9,0($T9)

    addi    $loopcntr,$loopcntr,4
    li      $I0,7*4

    xor     $T0,$T0,$T8
    xor     $T0,$T0,$T9
    xor     $T1,$T1,$T0
    xor     $T2,$T2,$T1
    xor     $T3,$T3,$T2

    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)

    beq     $loopcntr,$I0,4f
___
$code .= do_enc_lookup($T8,$T3,[0,8,16,24],[$T8,$T9,$T10,$T11]);
$code .= <<___;
    xor     $T4,$T4,$T8
    xor     $T5,$T5,$T4
    xor     $T6,$T6,$T5
    xor     $T7,$T7,$T6
    sw      $T4,16($KEYP)
    sw      $T5,20($KEYP)
    sw      $T6,24($KEYP)
    sw      $T7,28($KEYP)

    addi    $KEYP,$KEYP,32
    j       3b

4:  # return 0
    li      a0,0
5:  # return a0
___
$code .= load_regs();
$code .= <<___;
    ret
___

clear_regs();

################################################################################
# Register assignment for AES_set_decrypt_key
################################################################################

# Function arguments (x10-x12 are a0-a2 in the ABI)
# Pointer to user key, number of bits in key, key pointer
my ($UKEY,$BITS,$KEYP) = use_regs(10..12);

# Temporaries
my ($T0,$T1,$T2,$T3) = use_regs(6..8,9);
my ($T4,$T5,$T6,$T7,$T8) = use_regs(13..17);

my ($I1) = use_regs(18);

# Register to hold table offset and used as a temporary
my ($I0) = use_regs(19);

# Loop counter
my ($loopcntr) = use_regs(20);

# Lookup table address register
my ($TBL) = use_regs(21);

# Calculates dest = [
#       Td0[Te1[(in >> 24) & 0xff] & 0xff] ^
#       Td1[Te1[(in >> 16) & 0xff] & 0xff] ^
#       Td2[Te1[(in >>  8) & 0xff] & 0xff] ^
#       Td3[Te1[(in      ) & 0xff] & 0xff]
#   ]
sub do_dec_lookup {
    # (destination reg, input reg, temporary regs)
    my ($dest, $in, $Ts) = @_;

    my $ret = '';

$ret .= <<___;

    la      $TBL,AES_Te2

    slli    $Ts->[0],$in,2
    srli    $Ts->[1],$in,8-2
    srli    $Ts->[2],$in,16-2
    srli    $Ts->[3],$in,24-2

    andi    $Ts->[0],$Ts->[0],0x3FC
    andi    $Ts->[1],$Ts->[1],0x3FC
    andi    $Ts->[2],$Ts->[2],0x3FC
    andi    $Ts->[3],$Ts->[3],0x3FC

    # Index into table Te2

    add     $I0,$TBL,$Ts->[0]
    lwu     $Ts->[0],0($I0)

    add     $I0,$TBL,$Ts->[1]
    lwu     $Ts->[1],0($I0)

    add     $I0,$TBL,$Ts->[2]
    lwu     $Ts->[2],0($I0)

    add     $I0,$TBL,$Ts->[3]
    lwu     $Ts->[3],0($I0)

    andi    $Ts->[0],$Ts->[0],0xFF
    andi    $Ts->[1],$Ts->[1],0xFF
    andi    $Ts->[2],$Ts->[2],0xFF
    andi    $Ts->[3],$Ts->[3],0xFF

    slli    $Ts->[0],$Ts->[0],2
    slli    $Ts->[1],$Ts->[1],2
    slli    $Ts->[2],$Ts->[2],2
    slli    $Ts->[3],$Ts->[3],2

    la      $TBL,AES_Td0

    # Lookup in Td0-Td3

    add     $I0,$TBL,$Ts->[0]
    lwu     $Ts->[0],0($I0)

    add     $TBL,$TBL,1024
    add     $I0,$TBL,$Ts->[1]
    lwu     $Ts->[1],0($I0)

    add     $TBL,$TBL,1024
    add     $I0,$TBL,$Ts->[2]
    lwu     $Ts->[2],0($I0)

    add     $TBL,$TBL,1024
    add     $I0,$TBL,$Ts->[3]
    lwu     $Ts->[3],0($I0)

    xor     $Ts->[0],$Ts->[0],$Ts->[1]
    xor     $Ts->[2],$Ts->[2],$Ts->[3]
    xor     $dest,$Ts->[0],$Ts->[2]
___

    return $ret;
}

################################################################################
# void AES_set_decrypt_key(const unsigned char *userKey, const int bits,
#   AES_KEY *key)
################################################################################
$code .= <<___;
.text
.balign 16
.globl AES_set_decrypt_key
.type   AES_set_decrypt_key,\@function
AES_set_decrypt_key:
    # Call AES_set_encrypt_key first
    addi    sp,sp,-16
    sd      $KEYP,0(sp) # We need to hold onto this!
    sd      ra,8(sp)
    jal     ra,AES_set_encrypt_key
    ld      $KEYP,0(sp)
    ld      ra,8(sp)
    addi    sp,sp,16
    bgez    a0,1f   # If error, return error
    ret
1:
___
$code .= save_regs();
$code .= <<___;

    li      $T4,0
    lwu     $T8,240($KEYP)
    slli    $T5,$T8,4
    # Invert order of round keys
1:
    add     $I0,$KEYP,$T4
    ld      $T0,0($I0)
    ld      $T1,8($I0)
    add     $I1,$KEYP,$T5
    ld      $T2,0($I1)
    ld      $T3,8($I1)
    addi    $T4,$T4,16
    addi    $T5,$T5,-16
    sd      $T0,0($I1)
    sd      $T1,8($I1)
    sd      $T2,0($I0)
    sd      $T3,8($I0)
    blt     $T4,$T5,1b

    li      $loopcntr,1

1:
    addi    $KEYP,$KEYP,16
    lwu     $T0,0($KEYP)
    lwu     $T1,4($KEYP)
    lwu     $T2,8($KEYP)
    lwu     $T3,12($KEYP)
___
$code .= do_dec_lookup($T0,$T0,[$T4,$T5,$T6,$T7]);
$code .= do_dec_lookup($T1,$T1,[$T4,$T5,$T6,$T7]);
$code .= do_dec_lookup($T2,$T2,[$T4,$T5,$T6,$T7]);
$code .= do_dec_lookup($T3,$T3,[$T4,$T5,$T6,$T7]);
$code .= <<___;
    sw      $T0,0($KEYP)
    sw      $T1,4($KEYP)
    sw      $T2,8($KEYP)
    sw      $T3,12($KEYP)
    addi    $loopcntr,$loopcntr,1
    blt     $loopcntr,$T8,1b
___
$code .= load_regs();
$code .= <<___;
    li      a0,0
    ret
___
$code .= <<___;

.section .rodata
.p2align    12
.type   AES_Te0,\@object
AES_Te0:
.word 0xa56363c6U, 0x847c7cf8U, 0x997777eeU, 0x8d7b7bf6U
.word 0x0df2f2ffU, 0xbd6b6bd6U, 0xb16f6fdeU, 0x54c5c591U
.word 0x50303060U, 0x03010102U, 0xa96767ceU, 0x7d2b2b56U
.word 0x19fefee7U, 0x62d7d7b5U, 0xe6abab4dU, 0x9a7676ecU
.word 0x45caca8fU, 0x9d82821fU, 0x40c9c989U, 0x877d7dfaU
.word 0x15fafaefU, 0xeb5959b2U, 0xc947478eU, 0x0bf0f0fbU
.word 0xecadad41U, 0x67d4d4b3U, 0xfda2a25fU, 0xeaafaf45U
.word 0xbf9c9c23U, 0xf7a4a453U, 0x967272e4U, 0x5bc0c09bU
.word 0xc2b7b775U, 0x1cfdfde1U, 0xae93933dU, 0x6a26264cU
.word 0x5a36366cU, 0x413f3f7eU, 0x02f7f7f5U, 0x4fcccc83U
.word 0x5c343468U, 0xf4a5a551U, 0x34e5e5d1U, 0x08f1f1f9U
.word 0x937171e2U, 0x73d8d8abU, 0x53313162U, 0x3f15152aU
.word 0x0c040408U, 0x52c7c795U, 0x65232346U, 0x5ec3c39dU
.word 0x28181830U, 0xa1969637U, 0x0f05050aU, 0xb59a9a2fU
.word 0x0907070eU, 0x36121224U, 0x9b80801bU, 0x3de2e2dfU
.word 0x26ebebcdU, 0x6927274eU, 0xcdb2b27fU, 0x9f7575eaU
.word 0x1b090912U, 0x9e83831dU, 0x742c2c58U, 0x2e1a1a34U
.word 0x2d1b1b36U, 0xb26e6edcU, 0xee5a5ab4U, 0xfba0a05bU
.word 0xf65252a4U, 0x4d3b3b76U, 0x61d6d6b7U, 0xceb3b37dU
.word 0x7b292952U, 0x3ee3e3ddU, 0x712f2f5eU, 0x97848413U
.word 0xf55353a6U, 0x68d1d1b9U, 0x00000000U, 0x2cededc1U
.word 0x60202040U, 0x1ffcfce3U, 0xc8b1b179U, 0xed5b5bb6U
.word 0xbe6a6ad4U, 0x46cbcb8dU, 0xd9bebe67U, 0x4b393972U
.word 0xde4a4a94U, 0xd44c4c98U, 0xe85858b0U, 0x4acfcf85U
.word 0x6bd0d0bbU, 0x2aefefc5U, 0xe5aaaa4fU, 0x16fbfbedU
.word 0xc5434386U, 0xd74d4d9aU, 0x55333366U, 0x94858511U
.word 0xcf45458aU, 0x10f9f9e9U, 0x06020204U, 0x817f7ffeU
.word 0xf05050a0U, 0x443c3c78U, 0xba9f9f25U, 0xe3a8a84bU
.word 0xf35151a2U, 0xfea3a35dU, 0xc0404080U, 0x8a8f8f05U
.word 0xad92923fU, 0xbc9d9d21U, 0x48383870U, 0x04f5f5f1U
.word 0xdfbcbc63U, 0xc1b6b677U, 0x75dadaafU, 0x63212142U
.word 0x30101020U, 0x1affffe5U, 0x0ef3f3fdU, 0x6dd2d2bfU
.word 0x4ccdcd81U, 0x140c0c18U, 0x35131326U, 0x2fececc3U
.word 0xe15f5fbeU, 0xa2979735U, 0xcc444488U, 0x3917172eU
.word 0x57c4c493U, 0xf2a7a755U, 0x827e7efcU, 0x473d3d7aU
.word 0xac6464c8U, 0xe75d5dbaU, 0x2b191932U, 0x957373e6U
.word 0xa06060c0U, 0x98818119U, 0xd14f4f9eU, 0x7fdcdca3U
.word 0x66222244U, 0x7e2a2a54U, 0xab90903bU, 0x8388880bU
.word 0xca46468cU, 0x29eeeec7U, 0xd3b8b86bU, 0x3c141428U
.word 0x79dedea7U, 0xe25e5ebcU, 0x1d0b0b16U, 0x76dbdbadU
.word 0x3be0e0dbU, 0x56323264U, 0x4e3a3a74U, 0x1e0a0a14U
.word 0xdb494992U, 0x0a06060cU, 0x6c242448U, 0xe45c5cb8U
.word 0x5dc2c29fU, 0x6ed3d3bdU, 0xefacac43U, 0xa66262c4U
.word 0xa8919139U, 0xa4959531U, 0x37e4e4d3U, 0x8b7979f2U
.word 0x32e7e7d5U, 0x43c8c88bU, 0x5937376eU, 0xb76d6ddaU
.word 0x8c8d8d01U, 0x64d5d5b1U, 0xd24e4e9cU, 0xe0a9a949U
.word 0xb46c6cd8U, 0xfa5656acU, 0x07f4f4f3U, 0x25eaeacfU
.word 0xaf6565caU, 0x8e7a7af4U, 0xe9aeae47U, 0x18080810U
.word 0xd5baba6fU, 0x887878f0U, 0x6f25254aU, 0x722e2e5cU
.word 0x241c1c38U, 0xf1a6a657U, 0xc7b4b473U, 0x51c6c697U
.word 0x23e8e8cbU, 0x7cdddda1U, 0x9c7474e8U, 0x211f1f3eU
.word 0xdd4b4b96U, 0xdcbdbd61U, 0x868b8b0dU, 0x858a8a0fU
.word 0x907070e0U, 0x423e3e7cU, 0xc4b5b571U, 0xaa6666ccU
.word 0xd8484890U, 0x05030306U, 0x01f6f6f7U, 0x120e0e1cU
.word 0xa36161c2U, 0x5f35356aU, 0xf95757aeU, 0xd0b9b969U
.word 0x91868617U, 0x58c1c199U, 0x271d1d3aU, 0xb99e9e27U
.word 0x38e1e1d9U, 0x13f8f8ebU, 0xb398982bU, 0x33111122U
.word 0xbb6969d2U, 0x70d9d9a9U, 0x898e8e07U, 0xa7949433U
.word 0xb69b9b2dU, 0x221e1e3cU, 0x92878715U, 0x20e9e9c9U
.word 0x49cece87U, 0xff5555aaU, 0x78282850U, 0x7adfdfa5U
.word 0x8f8c8c03U, 0xf8a1a159U, 0x80898909U, 0x170d0d1aU
.word 0xdabfbf65U, 0x31e6e6d7U, 0xc6424284U, 0xb86868d0U
.word 0xc3414182U, 0xb0999929U, 0x772d2d5aU, 0x110f0f1eU
.word 0xcbb0b07bU, 0xfc5454a8U, 0xd6bbbb6dU, 0x3a16162cU

.type   AES_Te1,\@object
AES_Te1:
.word 0x6363c6a5U, 0x7c7cf884U, 0x7777ee99U, 0x7b7bf68dU
.word 0xf2f2ff0dU, 0x6b6bd6bdU, 0x6f6fdeb1U, 0xc5c59154U
.word 0x30306050U, 0x01010203U, 0x6767cea9U, 0x2b2b567dU
.word 0xfefee719U, 0xd7d7b562U, 0xabab4de6U, 0x7676ec9aU
.word 0xcaca8f45U, 0x82821f9dU, 0xc9c98940U, 0x7d7dfa87U
.word 0xfafaef15U, 0x5959b2ebU, 0x47478ec9U, 0xf0f0fb0bU
.word 0xadad41ecU, 0xd4d4b367U, 0xa2a25ffdU, 0xafaf45eaU
.word 0x9c9c23bfU, 0xa4a453f7U, 0x7272e496U, 0xc0c09b5bU
.word 0xb7b775c2U, 0xfdfde11cU, 0x93933daeU, 0x26264c6aU
.word 0x36366c5aU, 0x3f3f7e41U, 0xf7f7f502U, 0xcccc834fU
.word 0x3434685cU, 0xa5a551f4U, 0xe5e5d134U, 0xf1f1f908U
.word 0x7171e293U, 0xd8d8ab73U, 0x31316253U, 0x15152a3fU
.word 0x0404080cU, 0xc7c79552U, 0x23234665U, 0xc3c39d5eU
.word 0x18183028U, 0x969637a1U, 0x05050a0fU, 0x9a9a2fb5U
.word 0x07070e09U, 0x12122436U, 0x80801b9bU, 0xe2e2df3dU
.word 0xebebcd26U, 0x27274e69U, 0xb2b27fcdU, 0x7575ea9fU
.word 0x0909121bU, 0x83831d9eU, 0x2c2c5874U, 0x1a1a342eU
.word 0x1b1b362dU, 0x6e6edcb2U, 0x5a5ab4eeU, 0xa0a05bfbU
.word 0x5252a4f6U, 0x3b3b764dU, 0xd6d6b761U, 0xb3b37dceU
.word 0x2929527bU, 0xe3e3dd3eU, 0x2f2f5e71U, 0x84841397U
.word 0x5353a6f5U, 0xd1d1b968U, 0x00000000U, 0xededc12cU
.word 0x20204060U, 0xfcfce31fU, 0xb1b179c8U, 0x5b5bb6edU
.word 0x6a6ad4beU, 0xcbcb8d46U, 0xbebe67d9U, 0x3939724bU
.word 0x4a4a94deU, 0x4c4c98d4U, 0x5858b0e8U, 0xcfcf854aU
.word 0xd0d0bb6bU, 0xefefc52aU, 0xaaaa4fe5U, 0xfbfbed16U
.word 0x434386c5U, 0x4d4d9ad7U, 0x33336655U, 0x85851194U
.word 0x45458acfU, 0xf9f9e910U, 0x02020406U, 0x7f7ffe81U
.word 0x5050a0f0U, 0x3c3c7844U, 0x9f9f25baU, 0xa8a84be3U
.word 0x5151a2f3U, 0xa3a35dfeU, 0x404080c0U, 0x8f8f058aU
.word 0x92923fadU, 0x9d9d21bcU, 0x38387048U, 0xf5f5f104U
.word 0xbcbc63dfU, 0xb6b677c1U, 0xdadaaf75U, 0x21214263U
.word 0x10102030U, 0xffffe51aU, 0xf3f3fd0eU, 0xd2d2bf6dU
.word 0xcdcd814cU, 0x0c0c1814U, 0x13132635U, 0xececc32fU
.word 0x5f5fbee1U, 0x979735a2U, 0x444488ccU, 0x17172e39U
.word 0xc4c49357U, 0xa7a755f2U, 0x7e7efc82U, 0x3d3d7a47U
.word 0x6464c8acU, 0x5d5dbae7U, 0x1919322bU, 0x7373e695U
.word 0x6060c0a0U, 0x81811998U, 0x4f4f9ed1U, 0xdcdca37fU
.word 0x22224466U, 0x2a2a547eU, 0x90903babU, 0x88880b83U
.word 0x46468ccaU, 0xeeeec729U, 0xb8b86bd3U, 0x1414283cU
.word 0xdedea779U, 0x5e5ebce2U, 0x0b0b161dU, 0xdbdbad76U
.word 0xe0e0db3bU, 0x32326456U, 0x3a3a744eU, 0x0a0a141eU
.word 0x494992dbU, 0x06060c0aU, 0x2424486cU, 0x5c5cb8e4U
.word 0xc2c29f5dU, 0xd3d3bd6eU, 0xacac43efU, 0x6262c4a6U
.word 0x919139a8U, 0x959531a4U, 0xe4e4d337U, 0x7979f28bU
.word 0xe7e7d532U, 0xc8c88b43U, 0x37376e59U, 0x6d6ddab7U
.word 0x8d8d018cU, 0xd5d5b164U, 0x4e4e9cd2U, 0xa9a949e0U
.word 0x6c6cd8b4U, 0x5656acfaU, 0xf4f4f307U, 0xeaeacf25U
.word 0x6565caafU, 0x7a7af48eU, 0xaeae47e9U, 0x08081018U
.word 0xbaba6fd5U, 0x7878f088U, 0x25254a6fU, 0x2e2e5c72U
.word 0x1c1c3824U, 0xa6a657f1U, 0xb4b473c7U, 0xc6c69751U
.word 0xe8e8cb23U, 0xdddda17cU, 0x7474e89cU, 0x1f1f3e21U
.word 0x4b4b96ddU, 0xbdbd61dcU, 0x8b8b0d86U, 0x8a8a0f85U
.word 0x7070e090U, 0x3e3e7c42U, 0xb5b571c4U, 0x6666ccaaU
.word 0x484890d8U, 0x03030605U, 0xf6f6f701U, 0x0e0e1c12U
.word 0x6161c2a3U, 0x35356a5fU, 0x5757aef9U, 0xb9b969d0U
.word 0x86861791U, 0xc1c19958U, 0x1d1d3a27U, 0x9e9e27b9U
.word 0xe1e1d938U, 0xf8f8eb13U, 0x98982bb3U, 0x11112233U
.word 0x6969d2bbU, 0xd9d9a970U, 0x8e8e0789U, 0x949433a7U
.word 0x9b9b2db6U, 0x1e1e3c22U, 0x87871592U, 0xe9e9c920U
.word 0xcece8749U, 0x5555aaffU, 0x28285078U, 0xdfdfa57aU
.word 0x8c8c038fU, 0xa1a159f8U, 0x89890980U, 0x0d0d1a17U
.word 0xbfbf65daU, 0xe6e6d731U, 0x424284c6U, 0x6868d0b8U
.word 0x414182c3U, 0x999929b0U, 0x2d2d5a77U, 0x0f0f1e11U
.word 0xb0b07bcbU, 0x5454a8fcU, 0xbbbb6dd6U, 0x16162c3aU

.type   AES_Te2,\@object
AES_Te2:
.word 0x63c6a563U, 0x7cf8847cU, 0x77ee9977U, 0x7bf68d7bU
.word 0xf2ff0df2U, 0x6bd6bd6bU, 0x6fdeb16fU, 0xc59154c5U
.word 0x30605030U, 0x01020301U, 0x67cea967U, 0x2b567d2bU
.word 0xfee719feU, 0xd7b562d7U, 0xab4de6abU, 0x76ec9a76U
.word 0xca8f45caU, 0x821f9d82U, 0xc98940c9U, 0x7dfa877dU
.word 0xfaef15faU, 0x59b2eb59U, 0x478ec947U, 0xf0fb0bf0U
.word 0xad41ecadU, 0xd4b367d4U, 0xa25ffda2U, 0xaf45eaafU
.word 0x9c23bf9cU, 0xa453f7a4U, 0x72e49672U, 0xc09b5bc0U
.word 0xb775c2b7U, 0xfde11cfdU, 0x933dae93U, 0x264c6a26U
.word 0x366c5a36U, 0x3f7e413fU, 0xf7f502f7U, 0xcc834fccU
.word 0x34685c34U, 0xa551f4a5U, 0xe5d134e5U, 0xf1f908f1U
.word 0x71e29371U, 0xd8ab73d8U, 0x31625331U, 0x152a3f15U
.word 0x04080c04U, 0xc79552c7U, 0x23466523U, 0xc39d5ec3U
.word 0x18302818U, 0x9637a196U, 0x050a0f05U, 0x9a2fb59aU
.word 0x070e0907U, 0x12243612U, 0x801b9b80U, 0xe2df3de2U
.word 0xebcd26ebU, 0x274e6927U, 0xb27fcdb2U, 0x75ea9f75U
.word 0x09121b09U, 0x831d9e83U, 0x2c58742cU, 0x1a342e1aU
.word 0x1b362d1bU, 0x6edcb26eU, 0x5ab4ee5aU, 0xa05bfba0U
.word 0x52a4f652U, 0x3b764d3bU, 0xd6b761d6U, 0xb37dceb3U
.word 0x29527b29U, 0xe3dd3ee3U, 0x2f5e712fU, 0x84139784U
.word 0x53a6f553U, 0xd1b968d1U, 0x00000000U, 0xedc12cedU
.word 0x20406020U, 0xfce31ffcU, 0xb179c8b1U, 0x5bb6ed5bU
.word 0x6ad4be6aU, 0xcb8d46cbU, 0xbe67d9beU, 0x39724b39U
.word 0x4a94de4aU, 0x4c98d44cU, 0x58b0e858U, 0xcf854acfU
.word 0xd0bb6bd0U, 0xefc52aefU, 0xaa4fe5aaU, 0xfbed16fbU
.word 0x4386c543U, 0x4d9ad74dU, 0x33665533U, 0x85119485U
.word 0x458acf45U, 0xf9e910f9U, 0x02040602U, 0x7ffe817fU
.word 0x50a0f050U, 0x3c78443cU, 0x9f25ba9fU, 0xa84be3a8U
.word 0x51a2f351U, 0xa35dfea3U, 0x4080c040U, 0x8f058a8fU
.word 0x923fad92U, 0x9d21bc9dU, 0x38704838U, 0xf5f104f5U
.word 0xbc63dfbcU, 0xb677c1b6U, 0xdaaf75daU, 0x21426321U
.word 0x10203010U, 0xffe51affU, 0xf3fd0ef3U, 0xd2bf6dd2U
.word 0xcd814ccdU, 0x0c18140cU, 0x13263513U, 0xecc32fecU
.word 0x5fbee15fU, 0x9735a297U, 0x4488cc44U, 0x172e3917U
.word 0xc49357c4U, 0xa755f2a7U, 0x7efc827eU, 0x3d7a473dU
.word 0x64c8ac64U, 0x5dbae75dU, 0x19322b19U, 0x73e69573U
.word 0x60c0a060U, 0x81199881U, 0x4f9ed14fU, 0xdca37fdcU
.word 0x22446622U, 0x2a547e2aU, 0x903bab90U, 0x880b8388U
.word 0x468cca46U, 0xeec729eeU, 0xb86bd3b8U, 0x14283c14U
.word 0xdea779deU, 0x5ebce25eU, 0x0b161d0bU, 0xdbad76dbU
.word 0xe0db3be0U, 0x32645632U, 0x3a744e3aU, 0x0a141e0aU
.word 0x4992db49U, 0x060c0a06U, 0x24486c24U, 0x5cb8e45cU
.word 0xc29f5dc2U, 0xd3bd6ed3U, 0xac43efacU, 0x62c4a662U
.word 0x9139a891U, 0x9531a495U, 0xe4d337e4U, 0x79f28b79U
.word 0xe7d532e7U, 0xc88b43c8U, 0x376e5937U, 0x6ddab76dU
.word 0x8d018c8dU, 0xd5b164d5U, 0x4e9cd24eU, 0xa949e0a9U
.word 0x6cd8b46cU, 0x56acfa56U, 0xf4f307f4U, 0xeacf25eaU
.word 0x65caaf65U, 0x7af48e7aU, 0xae47e9aeU, 0x08101808U
.word 0xba6fd5baU, 0x78f08878U, 0x254a6f25U, 0x2e5c722eU
.word 0x1c38241cU, 0xa657f1a6U, 0xb473c7b4U, 0xc69751c6U
.word 0xe8cb23e8U, 0xdda17cddU, 0x74e89c74U, 0x1f3e211fU
.word 0x4b96dd4bU, 0xbd61dcbdU, 0x8b0d868bU, 0x8a0f858aU
.word 0x70e09070U, 0x3e7c423eU, 0xb571c4b5U, 0x66ccaa66U
.word 0x4890d848U, 0x03060503U, 0xf6f701f6U, 0x0e1c120eU
.word 0x61c2a361U, 0x356a5f35U, 0x57aef957U, 0xb969d0b9U
.word 0x86179186U, 0xc19958c1U, 0x1d3a271dU, 0x9e27b99eU
.word 0xe1d938e1U, 0xf8eb13f8U, 0x982bb398U, 0x11223311U
.word 0x69d2bb69U, 0xd9a970d9U, 0x8e07898eU, 0x9433a794U
.word 0x9b2db69bU, 0x1e3c221eU, 0x87159287U, 0xe9c920e9U
.word 0xce8749ceU, 0x55aaff55U, 0x28507828U, 0xdfa57adfU
.word 0x8c038f8cU, 0xa159f8a1U, 0x89098089U, 0x0d1a170dU
.word 0xbf65dabfU, 0xe6d731e6U, 0x4284c642U, 0x68d0b868U
.word 0x4182c341U, 0x9929b099U, 0x2d5a772dU, 0x0f1e110fU
.word 0xb07bcbb0U, 0x54a8fc54U, 0xbb6dd6bbU, 0x162c3a16U

.type   AES_Te3,\@object
AES_Te3:
.word 0xc6a56363U, 0xf8847c7cU, 0xee997777U, 0xf68d7b7bU
.word 0xff0df2f2U, 0xd6bd6b6bU, 0xdeb16f6fU, 0x9154c5c5U
.word 0x60503030U, 0x02030101U, 0xcea96767U, 0x567d2b2bU
.word 0xe719fefeU, 0xb562d7d7U, 0x4de6ababU, 0xec9a7676U
.word 0x8f45cacaU, 0x1f9d8282U, 0x8940c9c9U, 0xfa877d7dU
.word 0xef15fafaU, 0xb2eb5959U, 0x8ec94747U, 0xfb0bf0f0U
.word 0x41ecadadU, 0xb367d4d4U, 0x5ffda2a2U, 0x45eaafafU
.word 0x23bf9c9cU, 0x53f7a4a4U, 0xe4967272U, 0x9b5bc0c0U
.word 0x75c2b7b7U, 0xe11cfdfdU, 0x3dae9393U, 0x4c6a2626U
.word 0x6c5a3636U, 0x7e413f3fU, 0xf502f7f7U, 0x834fccccU
.word 0x685c3434U, 0x51f4a5a5U, 0xd134e5e5U, 0xf908f1f1U
.word 0xe2937171U, 0xab73d8d8U, 0x62533131U, 0x2a3f1515U
.word 0x080c0404U, 0x9552c7c7U, 0x46652323U, 0x9d5ec3c3U
.word 0x30281818U, 0x37a19696U, 0x0a0f0505U, 0x2fb59a9aU
.word 0x0e090707U, 0x24361212U, 0x1b9b8080U, 0xdf3de2e2U
.word 0xcd26ebebU, 0x4e692727U, 0x7fcdb2b2U, 0xea9f7575U
.word 0x121b0909U, 0x1d9e8383U, 0x58742c2cU, 0x342e1a1aU
.word 0x362d1b1bU, 0xdcb26e6eU, 0xb4ee5a5aU, 0x5bfba0a0U
.word 0xa4f65252U, 0x764d3b3bU, 0xb761d6d6U, 0x7dceb3b3U
.word 0x527b2929U, 0xdd3ee3e3U, 0x5e712f2fU, 0x13978484U
.word 0xa6f55353U, 0xb968d1d1U, 0x00000000U, 0xc12cededU
.word 0x40602020U, 0xe31ffcfcU, 0x79c8b1b1U, 0xb6ed5b5bU
.word 0xd4be6a6aU, 0x8d46cbcbU, 0x67d9bebeU, 0x724b3939U
.word 0x94de4a4aU, 0x98d44c4cU, 0xb0e85858U, 0x854acfcfU
.word 0xbb6bd0d0U, 0xc52aefefU, 0x4fe5aaaaU, 0xed16fbfbU
.word 0x86c54343U, 0x9ad74d4dU, 0x66553333U, 0x11948585U
.word 0x8acf4545U, 0xe910f9f9U, 0x04060202U, 0xfe817f7fU
.word 0xa0f05050U, 0x78443c3cU, 0x25ba9f9fU, 0x4be3a8a8U
.word 0xa2f35151U, 0x5dfea3a3U, 0x80c04040U, 0x058a8f8fU
.word 0x3fad9292U, 0x21bc9d9dU, 0x70483838U, 0xf104f5f5U
.word 0x63dfbcbcU, 0x77c1b6b6U, 0xaf75dadaU, 0x42632121U
.word 0x20301010U, 0xe51affffU, 0xfd0ef3f3U, 0xbf6dd2d2U
.word 0x814ccdcdU, 0x18140c0cU, 0x26351313U, 0xc32fececU
.word 0xbee15f5fU, 0x35a29797U, 0x88cc4444U, 0x2e391717U
.word 0x9357c4c4U, 0x55f2a7a7U, 0xfc827e7eU, 0x7a473d3dU
.word 0xc8ac6464U, 0xbae75d5dU, 0x322b1919U, 0xe6957373U
.word 0xc0a06060U, 0x19988181U, 0x9ed14f4fU, 0xa37fdcdcU
.word 0x44662222U, 0x547e2a2aU, 0x3bab9090U, 0x0b838888U
.word 0x8cca4646U, 0xc729eeeeU, 0x6bd3b8b8U, 0x283c1414U
.word 0xa779dedeU, 0xbce25e5eU, 0x161d0b0bU, 0xad76dbdbU
.word 0xdb3be0e0U, 0x64563232U, 0x744e3a3aU, 0x141e0a0aU
.word 0x92db4949U, 0x0c0a0606U, 0x486c2424U, 0xb8e45c5cU
.word 0x9f5dc2c2U, 0xbd6ed3d3U, 0x43efacacU, 0xc4a66262U
.word 0x39a89191U, 0x31a49595U, 0xd337e4e4U, 0xf28b7979U
.word 0xd532e7e7U, 0x8b43c8c8U, 0x6e593737U, 0xdab76d6dU
.word 0x018c8d8dU, 0xb164d5d5U, 0x9cd24e4eU, 0x49e0a9a9U
.word 0xd8b46c6cU, 0xacfa5656U, 0xf307f4f4U, 0xcf25eaeaU
.word 0xcaaf6565U, 0xf48e7a7aU, 0x47e9aeaeU, 0x10180808U
.word 0x6fd5babaU, 0xf0887878U, 0x4a6f2525U, 0x5c722e2eU
.word 0x38241c1cU, 0x57f1a6a6U, 0x73c7b4b4U, 0x9751c6c6U
.word 0xcb23e8e8U, 0xa17cddddU, 0xe89c7474U, 0x3e211f1fU
.word 0x96dd4b4bU, 0x61dcbdbdU, 0x0d868b8bU, 0x0f858a8aU
.word 0xe0907070U, 0x7c423e3eU, 0x71c4b5b5U, 0xccaa6666U
.word 0x90d84848U, 0x06050303U, 0xf701f6f6U, 0x1c120e0eU
.word 0xc2a36161U, 0x6a5f3535U, 0xaef95757U, 0x69d0b9b9U
.word 0x17918686U, 0x9958c1c1U, 0x3a271d1dU, 0x27b99e9eU
.word 0xd938e1e1U, 0xeb13f8f8U, 0x2bb39898U, 0x22331111U
.word 0xd2bb6969U, 0xa970d9d9U, 0x07898e8eU, 0x33a79494U
.word 0x2db69b9bU, 0x3c221e1eU, 0x15928787U, 0xc920e9e9U
.word 0x8749ceceU, 0xaaff5555U, 0x50782828U, 0xa57adfdfU
.word 0x038f8c8cU, 0x59f8a1a1U, 0x09808989U, 0x1a170d0dU
.word 0x65dabfbfU, 0xd731e6e6U, 0x84c64242U, 0xd0b86868U
.word 0x82c34141U, 0x29b09999U, 0x5a772d2dU, 0x1e110f0fU
.word 0x7bcbb0b0U, 0xa8fc5454U, 0x6dd6bbbbU, 0x2c3a1616U

.p2align    12
.type   AES_Td0,\@object
AES_Td0:
.word 0x50a7f451U, 0x5365417eU, 0xc3a4171aU, 0x965e273aU
.word 0xcb6bab3bU, 0xf1459d1fU, 0xab58faacU, 0x9303e34bU
.word 0x55fa3020U, 0xf66d76adU, 0x9176cc88U, 0x254c02f5U
.word 0xfcd7e54fU, 0xd7cb2ac5U, 0x80443526U, 0x8fa362b5U
.word 0x495ab1deU, 0x671bba25U, 0x980eea45U, 0xe1c0fe5dU
.word 0x02752fc3U, 0x12f04c81U, 0xa397468dU, 0xc6f9d36bU
.word 0xe75f8f03U, 0x959c9215U, 0xeb7a6dbfU, 0xda595295U
.word 0x2d83bed4U, 0xd3217458U, 0x2969e049U, 0x44c8c98eU
.word 0x6a89c275U, 0x78798ef4U, 0x6b3e5899U, 0xdd71b927U
.word 0xb64fe1beU, 0x17ad88f0U, 0x66ac20c9U, 0xb43ace7dU
.word 0x184adf63U, 0x82311ae5U, 0x60335197U, 0x457f5362U
.word 0xe07764b1U, 0x84ae6bbbU, 0x1ca081feU, 0x942b08f9U
.word 0x58684870U, 0x19fd458fU, 0x876cde94U, 0xb7f87b52U
.word 0x23d373abU, 0xe2024b72U, 0x578f1fe3U, 0x2aab5566U
.word 0x0728ebb2U, 0x03c2b52fU, 0x9a7bc586U, 0xa50837d3U
.word 0xf2872830U, 0xb2a5bf23U, 0xba6a0302U, 0x5c8216edU
.word 0x2b1ccf8aU, 0x92b479a7U, 0xf0f207f3U, 0xa1e2694eU
.word 0xcdf4da65U, 0xd5be0506U, 0x1f6234d1U, 0x8afea6c4U
.word 0x9d532e34U, 0xa055f3a2U, 0x32e18a05U, 0x75ebf6a4U
.word 0x39ec830bU, 0xaaef6040U, 0x069f715eU, 0x51106ebdU
.word 0xf98a213eU, 0x3d06dd96U, 0xae053eddU, 0x46bde64dU
.word 0xb58d5491U, 0x055dc471U, 0x6fd40604U, 0xff155060U
.word 0x24fb9819U, 0x97e9bdd6U, 0xcc434089U, 0x779ed967U
.word 0xbd42e8b0U, 0x888b8907U, 0x385b19e7U, 0xdbeec879U
.word 0x470a7ca1U, 0xe90f427cU, 0xc91e84f8U, 0x00000000U
.word 0x83868009U, 0x48ed2b32U, 0xac70111eU, 0x4e725a6cU
.word 0xfbff0efdU, 0x5638850fU, 0x1ed5ae3dU, 0x27392d36U
.word 0x64d90f0aU, 0x21a65c68U, 0xd1545b9bU, 0x3a2e3624U
.word 0xb1670a0cU, 0x0fe75793U, 0xd296eeb4U, 0x9e919b1bU
.word 0x4fc5c080U, 0xa220dc61U, 0x694b775aU, 0x161a121cU
.word 0x0aba93e2U, 0xe52aa0c0U, 0x43e0223cU, 0x1d171b12U
.word 0x0b0d090eU, 0xadc78bf2U, 0xb9a8b62dU, 0xc8a91e14U
.word 0x8519f157U, 0x4c0775afU, 0xbbdd99eeU, 0xfd607fa3U
.word 0x9f2601f7U, 0xbcf5725cU, 0xc53b6644U, 0x347efb5bU
.word 0x7629438bU, 0xdcc623cbU, 0x68fcedb6U, 0x63f1e4b8U
.word 0xcadc31d7U, 0x10856342U, 0x40229713U, 0x2011c684U
.word 0x7d244a85U, 0xf83dbbd2U, 0x1132f9aeU, 0x6da129c7U
.word 0x4b2f9e1dU, 0xf330b2dcU, 0xec52860dU, 0xd0e3c177U
.word 0x6c16b32bU, 0x99b970a9U, 0xfa489411U, 0x2264e947U
.word 0xc48cfca8U, 0x1a3ff0a0U, 0xd82c7d56U, 0xef903322U
.word 0xc74e4987U, 0xc1d138d9U, 0xfea2ca8cU, 0x360bd498U
.word 0xcf81f5a6U, 0x28de7aa5U, 0x268eb7daU, 0xa4bfad3fU
.word 0xe49d3a2cU, 0x0d927850U, 0x9bcc5f6aU, 0x62467e54U
.word 0xc2138df6U, 0xe8b8d890U, 0x5ef7392eU, 0xf5afc382U
.word 0xbe805d9fU, 0x7c93d069U, 0xa92dd56fU, 0xb31225cfU
.word 0x3b99acc8U, 0xa77d1810U, 0x6e639ce8U, 0x7bbb3bdbU
.word 0x097826cdU, 0xf418596eU, 0x01b79aecU, 0xa89a4f83U
.word 0x656e95e6U, 0x7ee6ffaaU, 0x08cfbc21U, 0xe6e815efU
.word 0xd99be7baU, 0xce366f4aU, 0xd4099feaU, 0xd67cb029U
.word 0xafb2a431U, 0x31233f2aU, 0x3094a5c6U, 0xc066a235U
.word 0x37bc4e74U, 0xa6ca82fcU, 0xb0d090e0U, 0x15d8a733U
.word 0x4a9804f1U, 0xf7daec41U, 0x0e50cd7fU, 0x2ff69117U
.word 0x8dd64d76U, 0x4db0ef43U, 0x544daaccU, 0xdf0496e4U
.word 0xe3b5d19eU, 0x1b886a4cU, 0xb81f2cc1U, 0x7f516546U
.word 0x04ea5e9dU, 0x5d358c01U, 0x737487faU, 0x2e410bfbU
.word 0x5a1d67b3U, 0x52d2db92U, 0x335610e9U, 0x1347d66dU
.word 0x8c61d79aU, 0x7a0ca137U, 0x8e14f859U, 0x893c13ebU
.word 0xee27a9ceU, 0x35c961b7U, 0xede51ce1U, 0x3cb1477aU
.word 0x59dfd29cU, 0x3f73f255U, 0x79ce1418U, 0xbf37c773U
.word 0xeacdf753U, 0x5baafd5fU, 0x146f3ddfU, 0x86db4478U
.word 0x81f3afcaU, 0x3ec468b9U, 0x2c342438U, 0x5f40a3c2U
.word 0x72c31d16U, 0x0c25e2bcU, 0x8b493c28U, 0x41950dffU
.word 0x7101a839U, 0xdeb30c08U, 0x9ce4b4d8U, 0x90c15664U
.word 0x6184cb7bU, 0x70b632d5U, 0x745c6c48U, 0x4257b8d0U

.type   AES_Td1,\@object
AES_Td1:
.word 0xa7f45150U, 0x65417e53U, 0xa4171ac3U, 0x5e273a96U
.word 0x6bab3bcbU, 0x459d1ff1U, 0x58faacabU, 0x03e34b93U
.word 0xfa302055U, 0x6d76adf6U, 0x76cc8891U, 0x4c02f525U
.word 0xd7e54ffcU, 0xcb2ac5d7U, 0x44352680U, 0xa362b58fU
.word 0x5ab1de49U, 0x1bba2567U, 0x0eea4598U, 0xc0fe5de1U
.word 0x752fc302U, 0xf04c8112U, 0x97468da3U, 0xf9d36bc6U
.word 0x5f8f03e7U, 0x9c921595U, 0x7a6dbfebU, 0x595295daU
.word 0x83bed42dU, 0x217458d3U, 0x69e04929U, 0xc8c98e44U
.word 0x89c2756aU, 0x798ef478U, 0x3e58996bU, 0x71b927ddU
.word 0x4fe1beb6U, 0xad88f017U, 0xac20c966U, 0x3ace7db4U
.word 0x4adf6318U, 0x311ae582U, 0x33519760U, 0x7f536245U
.word 0x7764b1e0U, 0xae6bbb84U, 0xa081fe1cU, 0x2b08f994U
.word 0x68487058U, 0xfd458f19U, 0x6cde9487U, 0xf87b52b7U
.word 0xd373ab23U, 0x024b72e2U, 0x8f1fe357U, 0xab55662aU
.word 0x28ebb207U, 0xc2b52f03U, 0x7bc5869aU, 0x0837d3a5U
.word 0x872830f2U, 0xa5bf23b2U, 0x6a0302baU, 0x8216ed5cU
.word 0x1ccf8a2bU, 0xb479a792U, 0xf207f3f0U, 0xe2694ea1U
.word 0xf4da65cdU, 0xbe0506d5U, 0x6234d11fU, 0xfea6c48aU
.word 0x532e349dU, 0x55f3a2a0U, 0xe18a0532U, 0xebf6a475U
.word 0xec830b39U, 0xef6040aaU, 0x9f715e06U, 0x106ebd51U
.word 0x8a213ef9U, 0x06dd963dU, 0x053eddaeU, 0xbde64d46U
.word 0x8d5491b5U, 0x5dc47105U, 0xd406046fU, 0x155060ffU
.word 0xfb981924U, 0xe9bdd697U, 0x434089ccU, 0x9ed96777U
.word 0x42e8b0bdU, 0x8b890788U, 0x5b19e738U, 0xeec879dbU
.word 0x0a7ca147U, 0x0f427ce9U, 0x1e84f8c9U, 0x00000000U
.word 0x86800983U, 0xed2b3248U, 0x70111eacU, 0x725a6c4eU
.word 0xff0efdfbU, 0x38850f56U, 0xd5ae3d1eU, 0x392d3627U
.word 0xd90f0a64U, 0xa65c6821U, 0x545b9bd1U, 0x2e36243aU
.word 0x670a0cb1U, 0xe757930fU, 0x96eeb4d2U, 0x919b1b9eU
.word 0xc5c0804fU, 0x20dc61a2U, 0x4b775a69U, 0x1a121c16U
.word 0xba93e20aU, 0x2aa0c0e5U, 0xe0223c43U, 0x171b121dU
.word 0x0d090e0bU, 0xc78bf2adU, 0xa8b62db9U, 0xa91e14c8U
.word 0x19f15785U, 0x0775af4cU, 0xdd99eebbU, 0x607fa3fdU
.word 0x2601f79fU, 0xf5725cbcU, 0x3b6644c5U, 0x7efb5b34U
.word 0x29438b76U, 0xc623cbdcU, 0xfcedb668U, 0xf1e4b863U
.word 0xdc31d7caU, 0x85634210U, 0x22971340U, 0x11c68420U
.word 0x244a857dU, 0x3dbbd2f8U, 0x32f9ae11U, 0xa129c76dU
.word 0x2f9e1d4bU, 0x30b2dcf3U, 0x52860decU, 0xe3c177d0U
.word 0x16b32b6cU, 0xb970a999U, 0x489411faU, 0x64e94722U
.word 0x8cfca8c4U, 0x3ff0a01aU, 0x2c7d56d8U, 0x903322efU
.word 0x4e4987c7U, 0xd138d9c1U, 0xa2ca8cfeU, 0x0bd49836U
.word 0x81f5a6cfU, 0xde7aa528U, 0x8eb7da26U, 0xbfad3fa4U
.word 0x9d3a2ce4U, 0x9278500dU, 0xcc5f6a9bU, 0x467e5462U
.word 0x138df6c2U, 0xb8d890e8U, 0xf7392e5eU, 0xafc382f5U
.word 0x805d9fbeU, 0x93d0697cU, 0x2dd56fa9U, 0x1225cfb3U
.word 0x99acc83bU, 0x7d1810a7U, 0x639ce86eU, 0xbb3bdb7bU
.word 0x7826cd09U, 0x18596ef4U, 0xb79aec01U, 0x9a4f83a8U
.word 0x6e95e665U, 0xe6ffaa7eU, 0xcfbc2108U, 0xe815efe6U
.word 0x9be7bad9U, 0x366f4aceU, 0x099fead4U, 0x7cb029d6U
.word 0xb2a431afU, 0x233f2a31U, 0x94a5c630U, 0x66a235c0U
.word 0xbc4e7437U, 0xca82fca6U, 0xd090e0b0U, 0xd8a73315U
.word 0x9804f14aU, 0xdaec41f7U, 0x50cd7f0eU, 0xf691172fU
.word 0xd64d768dU, 0xb0ef434dU, 0x4daacc54U, 0x0496e4dfU
.word 0xb5d19ee3U, 0x886a4c1bU, 0x1f2cc1b8U, 0x5165467fU
.word 0xea5e9d04U, 0x358c015dU, 0x7487fa73U, 0x410bfb2eU
.word 0x1d67b35aU, 0xd2db9252U, 0x5610e933U, 0x47d66d13U
.word 0x61d79a8cU, 0x0ca1377aU, 0x14f8598eU, 0x3c13eb89U
.word 0x27a9ceeeU, 0xc961b735U, 0xe51ce1edU, 0xb1477a3cU
.word 0xdfd29c59U, 0x73f2553fU, 0xce141879U, 0x37c773bfU
.word 0xcdf753eaU, 0xaafd5f5bU, 0x6f3ddf14U, 0xdb447886U
.word 0xf3afca81U, 0xc468b93eU, 0x3424382cU, 0x40a3c25fU
.word 0xc31d1672U, 0x25e2bc0cU, 0x493c288bU, 0x950dff41U
.word 0x01a83971U, 0xb30c08deU, 0xe4b4d89cU, 0xc1566490U
.word 0x84cb7b61U, 0xb632d570U, 0x5c6c4874U, 0x57b8d042U

.type   AES_Td2,\@object
AES_Td2:
.word 0xf45150a7U, 0x417e5365U, 0x171ac3a4U, 0x273a965eU
.word 0xab3bcb6bU, 0x9d1ff145U, 0xfaacab58U, 0xe34b9303U
.word 0x302055faU, 0x76adf66dU, 0xcc889176U, 0x02f5254cU
.word 0xe54ffcd7U, 0x2ac5d7cbU, 0x35268044U, 0x62b58fa3U
.word 0xb1de495aU, 0xba25671bU, 0xea45980eU, 0xfe5de1c0U
.word 0x2fc30275U, 0x4c8112f0U, 0x468da397U, 0xd36bc6f9U
.word 0x8f03e75fU, 0x9215959cU, 0x6dbfeb7aU, 0x5295da59U
.word 0xbed42d83U, 0x7458d321U, 0xe0492969U, 0xc98e44c8U
.word 0xc2756a89U, 0x8ef47879U, 0x58996b3eU, 0xb927dd71U
.word 0xe1beb64fU, 0x88f017adU, 0x20c966acU, 0xce7db43aU
.word 0xdf63184aU, 0x1ae58231U, 0x51976033U, 0x5362457fU
.word 0x64b1e077U, 0x6bbb84aeU, 0x81fe1ca0U, 0x08f9942bU
.word 0x48705868U, 0x458f19fdU, 0xde94876cU, 0x7b52b7f8U
.word 0x73ab23d3U, 0x4b72e202U, 0x1fe3578fU, 0x55662aabU
.word 0xebb20728U, 0xb52f03c2U, 0xc5869a7bU, 0x37d3a508U
.word 0x2830f287U, 0xbf23b2a5U, 0x0302ba6aU, 0x16ed5c82U
.word 0xcf8a2b1cU, 0x79a792b4U, 0x07f3f0f2U, 0x694ea1e2U
.word 0xda65cdf4U, 0x0506d5beU, 0x34d11f62U, 0xa6c48afeU
.word 0x2e349d53U, 0xf3a2a055U, 0x8a0532e1U, 0xf6a475ebU
.word 0x830b39ecU, 0x6040aaefU, 0x715e069fU, 0x6ebd5110U
.word 0x213ef98aU, 0xdd963d06U, 0x3eddae05U, 0xe64d46bdU
.word 0x5491b58dU, 0xc471055dU, 0x06046fd4U, 0x5060ff15U
.word 0x981924fbU, 0xbdd697e9U, 0x4089cc43U, 0xd967779eU
.word 0xe8b0bd42U, 0x8907888bU, 0x19e7385bU, 0xc879dbeeU
.word 0x7ca1470aU, 0x427ce90fU, 0x84f8c91eU, 0x00000000U
.word 0x80098386U, 0x2b3248edU, 0x111eac70U, 0x5a6c4e72U
.word 0x0efdfbffU, 0x850f5638U, 0xae3d1ed5U, 0x2d362739U
.word 0x0f0a64d9U, 0x5c6821a6U, 0x5b9bd154U, 0x36243a2eU
.word 0x0a0cb167U, 0x57930fe7U, 0xeeb4d296U, 0x9b1b9e91U
.word 0xc0804fc5U, 0xdc61a220U, 0x775a694bU, 0x121c161aU
.word 0x93e20abaU, 0xa0c0e52aU, 0x223c43e0U, 0x1b121d17U
.word 0x090e0b0dU, 0x8bf2adc7U, 0xb62db9a8U, 0x1e14c8a9U
.word 0xf1578519U, 0x75af4c07U, 0x99eebbddU, 0x7fa3fd60U
.word 0x01f79f26U, 0x725cbcf5U, 0x6644c53bU, 0xfb5b347eU
.word 0x438b7629U, 0x23cbdcc6U, 0xedb668fcU, 0xe4b863f1U
.word 0x31d7cadcU, 0x63421085U, 0x97134022U, 0xc6842011U
.word 0x4a857d24U, 0xbbd2f83dU, 0xf9ae1132U, 0x29c76da1U
.word 0x9e1d4b2fU, 0xb2dcf330U, 0x860dec52U, 0xc177d0e3U
.word 0xb32b6c16U, 0x70a999b9U, 0x9411fa48U, 0xe9472264U
.word 0xfca8c48cU, 0xf0a01a3fU, 0x7d56d82cU, 0x3322ef90U
.word 0x4987c74eU, 0x38d9c1d1U, 0xca8cfea2U, 0xd498360bU
.word 0xf5a6cf81U, 0x7aa528deU, 0xb7da268eU, 0xad3fa4bfU
.word 0x3a2ce49dU, 0x78500d92U, 0x5f6a9bccU, 0x7e546246U
.word 0x8df6c213U, 0xd890e8b8U, 0x392e5ef7U, 0xc382f5afU
.word 0x5d9fbe80U, 0xd0697c93U, 0xd56fa92dU, 0x25cfb312U
.word 0xacc83b99U, 0x1810a77dU, 0x9ce86e63U, 0x3bdb7bbbU
.word 0x26cd0978U, 0x596ef418U, 0x9aec01b7U, 0x4f83a89aU
.word 0x95e6656eU, 0xffaa7ee6U, 0xbc2108cfU, 0x15efe6e8U
.word 0xe7bad99bU, 0x6f4ace36U, 0x9fead409U, 0xb029d67cU
.word 0xa431afb2U, 0x3f2a3123U, 0xa5c63094U, 0xa235c066U
.word 0x4e7437bcU, 0x82fca6caU, 0x90e0b0d0U, 0xa73315d8U
.word 0x04f14a98U, 0xec41f7daU, 0xcd7f0e50U, 0x91172ff6U
.word 0x4d768dd6U, 0xef434db0U, 0xaacc544dU, 0x96e4df04U
.word 0xd19ee3b5U, 0x6a4c1b88U, 0x2cc1b81fU, 0x65467f51U
.word 0x5e9d04eaU, 0x8c015d35U, 0x87fa7374U, 0x0bfb2e41U
.word 0x67b35a1dU, 0xdb9252d2U, 0x10e93356U, 0xd66d1347U
.word 0xd79a8c61U, 0xa1377a0cU, 0xf8598e14U, 0x13eb893cU
.word 0xa9ceee27U, 0x61b735c9U, 0x1ce1ede5U, 0x477a3cb1U
.word 0xd29c59dfU, 0xf2553f73U, 0x141879ceU, 0xc773bf37U
.word 0xf753eacdU, 0xfd5f5baaU, 0x3ddf146fU, 0x447886dbU
.word 0xafca81f3U, 0x68b93ec4U, 0x24382c34U, 0xa3c25f40U
.word 0x1d1672c3U, 0xe2bc0c25U, 0x3c288b49U, 0x0dff4195U
.word 0xa8397101U, 0x0c08deb3U, 0xb4d89ce4U, 0x566490c1U
.word 0xcb7b6184U, 0x32d570b6U, 0x6c48745cU, 0xb8d04257U

.type   AES_Td3,\@object
AES_Td3:
.word 0x5150a7f4U, 0x7e536541U, 0x1ac3a417U, 0x3a965e27U
.word 0x3bcb6babU, 0x1ff1459dU, 0xacab58faU, 0x4b9303e3U
.word 0x2055fa30U, 0xadf66d76U, 0x889176ccU, 0xf5254c02U
.word 0x4ffcd7e5U, 0xc5d7cb2aU, 0x26804435U, 0xb58fa362U
.word 0xde495ab1U, 0x25671bbaU, 0x45980eeaU, 0x5de1c0feU
.word 0xc302752fU, 0x8112f04cU, 0x8da39746U, 0x6bc6f9d3U
.word 0x03e75f8fU, 0x15959c92U, 0xbfeb7a6dU, 0x95da5952U
.word 0xd42d83beU, 0x58d32174U, 0x492969e0U, 0x8e44c8c9U
.word 0x756a89c2U, 0xf478798eU, 0x996b3e58U, 0x27dd71b9U
.word 0xbeb64fe1U, 0xf017ad88U, 0xc966ac20U, 0x7db43aceU
.word 0x63184adfU, 0xe582311aU, 0x97603351U, 0x62457f53U
.word 0xb1e07764U, 0xbb84ae6bU, 0xfe1ca081U, 0xf9942b08U
.word 0x70586848U, 0x8f19fd45U, 0x94876cdeU, 0x52b7f87bU
.word 0xab23d373U, 0x72e2024bU, 0xe3578f1fU, 0x662aab55U
.word 0xb20728ebU, 0x2f03c2b5U, 0x869a7bc5U, 0xd3a50837U
.word 0x30f28728U, 0x23b2a5bfU, 0x02ba6a03U, 0xed5c8216U
.word 0x8a2b1ccfU, 0xa792b479U, 0xf3f0f207U, 0x4ea1e269U
.word 0x65cdf4daU, 0x06d5be05U, 0xd11f6234U, 0xc48afea6U
.word 0x349d532eU, 0xa2a055f3U, 0x0532e18aU, 0xa475ebf6U
.word 0x0b39ec83U, 0x40aaef60U, 0x5e069f71U, 0xbd51106eU
.word 0x3ef98a21U, 0x963d06ddU, 0xddae053eU, 0x4d46bde6U
.word 0x91b58d54U, 0x71055dc4U, 0x046fd406U, 0x60ff1550U
.word 0x1924fb98U, 0xd697e9bdU, 0x89cc4340U, 0x67779ed9U
.word 0xb0bd42e8U, 0x07888b89U, 0xe7385b19U, 0x79dbeec8U
.word 0xa1470a7cU, 0x7ce90f42U, 0xf8c91e84U, 0x00000000U
.word 0x09838680U, 0x3248ed2bU, 0x1eac7011U, 0x6c4e725aU
.word 0xfdfbff0eU, 0x0f563885U, 0x3d1ed5aeU, 0x3627392dU
.word 0x0a64d90fU, 0x6821a65cU, 0x9bd1545bU, 0x243a2e36U
.word 0x0cb1670aU, 0x930fe757U, 0xb4d296eeU, 0x1b9e919bU
.word 0x804fc5c0U, 0x61a220dcU, 0x5a694b77U, 0x1c161a12U
.word 0xe20aba93U, 0xc0e52aa0U, 0x3c43e022U, 0x121d171bU
.word 0x0e0b0d09U, 0xf2adc78bU, 0x2db9a8b6U, 0x14c8a91eU
.word 0x578519f1U, 0xaf4c0775U, 0xeebbdd99U, 0xa3fd607fU
.word 0xf79f2601U, 0x5cbcf572U, 0x44c53b66U, 0x5b347efbU
.word 0x8b762943U, 0xcbdcc623U, 0xb668fcedU, 0xb863f1e4U
.word 0xd7cadc31U, 0x42108563U, 0x13402297U, 0x842011c6U
.word 0x857d244aU, 0xd2f83dbbU, 0xae1132f9U, 0xc76da129U
.word 0x1d4b2f9eU, 0xdcf330b2U, 0x0dec5286U, 0x77d0e3c1U
.word 0x2b6c16b3U, 0xa999b970U, 0x11fa4894U, 0x472264e9U
.word 0xa8c48cfcU, 0xa01a3ff0U, 0x56d82c7dU, 0x22ef9033U
.word 0x87c74e49U, 0xd9c1d138U, 0x8cfea2caU, 0x98360bd4U
.word 0xa6cf81f5U, 0xa528de7aU, 0xda268eb7U, 0x3fa4bfadU
.word 0x2ce49d3aU, 0x500d9278U, 0x6a9bcc5fU, 0x5462467eU
.word 0xf6c2138dU, 0x90e8b8d8U, 0x2e5ef739U, 0x82f5afc3U
.word 0x9fbe805dU, 0x697c93d0U, 0x6fa92dd5U, 0xcfb31225U
.word 0xc83b99acU, 0x10a77d18U, 0xe86e639cU, 0xdb7bbb3bU
.word 0xcd097826U, 0x6ef41859U, 0xec01b79aU, 0x83a89a4fU
.word 0xe6656e95U, 0xaa7ee6ffU, 0x2108cfbcU, 0xefe6e815U
.word 0xbad99be7U, 0x4ace366fU, 0xead4099fU, 0x29d67cb0U
.word 0x31afb2a4U, 0x2a31233fU, 0xc63094a5U, 0x35c066a2U
.word 0x7437bc4eU, 0xfca6ca82U, 0xe0b0d090U, 0x3315d8a7U
.word 0xf14a9804U, 0x41f7daecU, 0x7f0e50cdU, 0x172ff691U
.word 0x768dd64dU, 0x434db0efU, 0xcc544daaU, 0xe4df0496U
.word 0x9ee3b5d1U, 0x4c1b886aU, 0xc1b81f2cU, 0x467f5165U
.word 0x9d04ea5eU, 0x015d358cU, 0xfa737487U, 0xfb2e410bU
.word 0xb35a1d67U, 0x9252d2dbU, 0xe9335610U, 0x6d1347d6U
.word 0x9a8c61d7U, 0x377a0ca1U, 0x598e14f8U, 0xeb893c13U
.word 0xceee27a9U, 0xb735c961U, 0xe1ede51cU, 0x7a3cb147U
.word 0x9c59dfd2U, 0x553f73f2U, 0x1879ce14U, 0x73bf37c7U
.word 0x53eacdf7U, 0x5f5baafdU, 0xdf146f3dU, 0x7886db44U
.word 0xca81f3afU, 0xb93ec468U, 0x382c3424U, 0xc25f40a3U
.word 0x1672c31dU, 0xbc0c25e2U, 0x288b493cU, 0xff41950dU
.word 0x397101a8U, 0x08deb30cU, 0xd89ce4b4U, 0x6490c156U
.word 0x7b6184cbU, 0xd570b632U, 0x48745c6cU, 0xd04257b8U

.type   AES_Td4,\@object
AES_Td4:
.byte   0x52U, 0x09U, 0x6aU, 0xd5U, 0x30U, 0x36U, 0xa5U, 0x38U
.byte   0xbfU, 0x40U, 0xa3U, 0x9eU, 0x81U, 0xf3U, 0xd7U, 0xfbU
.byte   0x7cU, 0xe3U, 0x39U, 0x82U, 0x9bU, 0x2fU, 0xffU, 0x87U
.byte   0x34U, 0x8eU, 0x43U, 0x44U, 0xc4U, 0xdeU, 0xe9U, 0xcbU
.byte   0x54U, 0x7bU, 0x94U, 0x32U, 0xa6U, 0xc2U, 0x23U, 0x3dU
.byte   0xeeU, 0x4cU, 0x95U, 0x0bU, 0x42U, 0xfaU, 0xc3U, 0x4eU
.byte   0x08U, 0x2eU, 0xa1U, 0x66U, 0x28U, 0xd9U, 0x24U, 0xb2U
.byte   0x76U, 0x5bU, 0xa2U, 0x49U, 0x6dU, 0x8bU, 0xd1U, 0x25U
.byte   0x72U, 0xf8U, 0xf6U, 0x64U, 0x86U, 0x68U, 0x98U, 0x16U
.byte   0xd4U, 0xa4U, 0x5cU, 0xccU, 0x5dU, 0x65U, 0xb6U, 0x92U
.byte   0x6cU, 0x70U, 0x48U, 0x50U, 0xfdU, 0xedU, 0xb9U, 0xdaU
.byte   0x5eU, 0x15U, 0x46U, 0x57U, 0xa7U, 0x8dU, 0x9dU, 0x84U
.byte   0x90U, 0xd8U, 0xabU, 0x00U, 0x8cU, 0xbcU, 0xd3U, 0x0aU
.byte   0xf7U, 0xe4U, 0x58U, 0x05U, 0xb8U, 0xb3U, 0x45U, 0x06U
.byte   0xd0U, 0x2cU, 0x1eU, 0x8fU, 0xcaU, 0x3fU, 0x0fU, 0x02U
.byte   0xc1U, 0xafU, 0xbdU, 0x03U, 0x01U, 0x13U, 0x8aU, 0x6bU
.byte   0x3aU, 0x91U, 0x11U, 0x41U, 0x4fU, 0x67U, 0xdcU, 0xeaU
.byte   0x97U, 0xf2U, 0xcfU, 0xceU, 0xf0U, 0xb4U, 0xe6U, 0x73U
.byte   0x96U, 0xacU, 0x74U, 0x22U, 0xe7U, 0xadU, 0x35U, 0x85U
.byte   0xe2U, 0xf9U, 0x37U, 0xe8U, 0x1cU, 0x75U, 0xdfU, 0x6eU
.byte   0x47U, 0xf1U, 0x1aU, 0x71U, 0x1dU, 0x29U, 0xc5U, 0x89U
.byte   0x6fU, 0xb7U, 0x62U, 0x0eU, 0xaaU, 0x18U, 0xbeU, 0x1bU
.byte   0xfcU, 0x56U, 0x3eU, 0x4bU, 0xc6U, 0xd2U, 0x79U, 0x20U
.byte   0x9aU, 0xdbU, 0xc0U, 0xfeU, 0x78U, 0xcdU, 0x5aU, 0xf4U
.byte   0x1fU, 0xddU, 0xa8U, 0x33U, 0x88U, 0x07U, 0xc7U, 0x31U
.byte   0xb1U, 0x12U, 0x10U, 0x59U, 0x27U, 0x80U, 0xecU, 0x5fU
.byte   0x60U, 0x51U, 0x7fU, 0xa9U, 0x19U, 0xb5U, 0x4aU, 0x0dU
.byte   0x2dU, 0xe5U, 0x7aU, 0x9fU, 0x93U, 0xc9U, 0x9cU, 0xefU
.byte   0xa0U, 0xe0U, 0x3bU, 0x4dU, 0xaeU, 0x2aU, 0xf5U, 0xb0U
.byte   0xc8U, 0xebU, 0xbbU, 0x3cU, 0x83U, 0x53U, 0x99U, 0x61U
.byte   0x17U, 0x2bU, 0x04U, 0x7eU, 0xbaU, 0x77U, 0xd6U, 0x26U
.byte   0xe1U, 0x69U, 0x14U, 0x63U, 0x55U, 0x21U, 0x0cU, 0x7dU

.type   AES_rcon,\@object
AES_rcon:
.word 0x00000001U, 0x00000002U, 0x00000004U, 0x00000008U
.word 0x00000010U, 0x00000020U, 0x00000040U, 0x00000080U
.word 0x0000001BU, 0x00000036U
___

print $code;
close STDOUT or die "error closing STDOUT: $!";
