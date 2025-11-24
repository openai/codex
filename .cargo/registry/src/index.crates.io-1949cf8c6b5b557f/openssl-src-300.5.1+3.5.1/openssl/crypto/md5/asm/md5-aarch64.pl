#! /usr/bin/env perl
# Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# MD5 optimized for aarch64.

use strict;

my $code;

#no warnings qw(uninitialized);
# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
my $output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; my $dir=$1; my $xlate;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour \"$output\""
    or die "can't call $xlate: $1";
*STDOUT=*OUT;

$code .= <<EOF;
#include "arm_arch.h"

.text
.globl  ossl_md5_block_asm_data_order
.type   ossl_md5_block_asm_data_order,\@function
ossl_md5_block_asm_data_order:
        AARCH64_VALID_CALL_TARGET
        // Save all callee-saved registers
        stp     x19,x20,[sp,#-80]!
        stp     x21,x22,[sp,#16]
        stp     x23,x24,[sp,#32]
        stp     x25,x26,[sp,#48]
        stp     x27,x28,[sp,#64]

        ldp w10, w11, [x0, #0]        // Load MD5 state->A and state->B
        ldp w12, w13, [x0, #8]        // Load MD5 state->C and state->D
.align 5
ossl_md5_blocks_loop:
        eor x17, x12, x13             // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        and x16, x17, x11             // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        ldp w15, w20, [x1]            // Load 2 words of input data0 M[0],M[1]
        ldp w3, w21, [x1, #8]        // Load 2 words of input data0 M[2],M[3]
#ifdef __AARCH64EB__
        rev w15, w15
        rev w20, w20
        rev w3, w3
        rev w21, w21
#endif
        eor x14, x16, x13             // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x9, #0xa478              // Load lower half of constant 0xd76aa478
        movk x9, #0xd76a, lsl #16     // Load upper half of constant 0xd76aa478
        add w8, w10, w15              // Add dest value
        add w7, w8, w9                // Add constant 0xd76aa478
        add w6, w7, w14               // Add aux function result
        ror w6, w6, #25               // Rotate left s=7 bits
        eor x5, x11, x12              // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w4, w11, w6               // Add X parameter round 1 A=FF(A, B, C, D, 0xd76aa478, s=7, M[0])
        and x8, x5, x4                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x17, x8, x12              // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x16, #0xb756             // Load lower half of constant 0xe8c7b756
        movk x16, #0xe8c7, lsl #16    // Load upper half of constant 0xe8c7b756
        add w9, w13, w20              // Add dest value
        add w7, w9, w16               // Add constant 0xe8c7b756
        add w14, w7, w17              // Add aux function result
        ror w14, w14, #20             // Rotate left s=12 bits
        eor x6, x4, x11               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w5, w4, w14               // Add X parameter round 1 D=FF(D, A, B, C, 0xe8c7b756, s=12, M[1])
        and x8, x6, x5                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x9, x8, x11               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x16, #0x70db             // Load lower half of constant 0x242070db
        movk x16, #0x2420, lsl #16    // Load upper half of constant 0x242070db
        add w7, w12, w3               // Add dest value
        add w17, w7, w16              // Add constant 0x242070db
        add w14, w17, w9              // Add aux function result
        ror w14, w14, #15             // Rotate left s=17 bits
        eor x6, x5, x4                // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w8, w5, w14               // Add X parameter round 1 C=FF(C, D, A, B, 0x242070db, s=17, M[2])
        and x7, x6, x8                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x16, x7, x4               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x9, #0xceee              // Load lower half of constant 0xc1bdceee
        movk x9, #0xc1bd, lsl #16     // Load upper half of constant 0xc1bdceee
        add w14, w11, w21             // Add dest value
        add w6, w14, w9               // Add constant 0xc1bdceee
        add w7, w6, w16               // Add aux function result
        ror w7, w7, #10               // Rotate left s=22 bits
        eor x17, x8, x5               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w9, w8, w7                // Add X parameter round 1 B=FF(B, C, D, A, 0xc1bdceee, s=22, M[3])
        ldp w14, w22, [x1, #16]       // Load 2 words of input data0 M[4],M[5]
        ldp w7, w23, [x1, #24]        // Load 2 words of input data0 M[6],M[7]
#ifdef __AARCH64EB__
        rev w14, w14
        rev w22, w22
        rev w7, w7
        rev w23, w23
#endif
        and x16, x17, x9              // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x16, x5               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x16, #0xfaf              // Load lower half of constant 0xf57c0faf
        movk x16, #0xf57c, lsl #16    // Load upper half of constant 0xf57c0faf
        add w17, w4, w14              // Add dest value
        add w16, w17, w16             // Add constant 0xf57c0faf
        add w4, w16, w6               // Add aux function result
        ror w4, w4, #25               // Rotate left s=7 bits
        eor x16, x9, x8               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w17, w9, w4               // Add X parameter round 1 A=FF(A, B, C, D, 0xf57c0faf, s=7, M[4])
        and x16, x16, x17             // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x16, x8               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x4, #0xc62a              // Load lower half of constant 0x4787c62a
        movk x4, #0x4787, lsl #16     // Load upper half of constant 0x4787c62a
        add w16, w5, w22              // Add dest value
        add w16, w16, w4              // Add constant 0x4787c62a
        add w5, w16, w6               // Add aux function result
        ror w5, w5, #20               // Rotate left s=12 bits
        eor x4, x17, x9               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w19, w17, w5              // Add X parameter round 1 D=FF(D, A, B, C, 0x4787c62a, s=12, M[5])
        and x6, x4, x19               // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x5, x6, x9                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x4, #0x4613              // Load lower half of constant 0xa8304613
        movk x4, #0xa830, lsl #16     // Load upper half of constant 0xa8304613
        add w6, w8, w7                // Add dest value
        add w8, w6, w4                // Add constant 0xa8304613
        add w4, w8, w5                // Add aux function result
        ror w4, w4, #15               // Rotate left s=17 bits
        eor x6, x19, x17              // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w8, w19, w4               // Add X parameter round 1 C=FF(C, D, A, B, 0xa8304613, s=17, M[6])
        and x5, x6, x8                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x4, x5, x17               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x6, #0x9501              // Load lower half of constant 0xfd469501
        movk x6, #0xfd46, lsl #16     // Load upper half of constant 0xfd469501
        add w9, w9, w23               // Add dest value
        add w5, w9, w6                // Add constant 0xfd469501
        add w9, w5, w4                // Add aux function result
        ror w9, w9, #10               // Rotate left s=22 bits
        eor x6, x8, x19               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w4, w8, w9                // Add X parameter round 1 B=FF(B, C, D, A, 0xfd469501, s=22, M[7])
        ldp w5, w24, [x1, #32]        // Load 2 words of input data0 M[8],M[9]
        ldp w16, w25, [x1, #40]        // Load 2 words of input data0 M[10],M[11]
#ifdef __AARCH64EB__
        rev w5, w5
        rev w24, w24
        rev w16, w16
        rev w25, w25
#endif
        and x9, x6, x4                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x9, x19               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x9, #0x98d8              // Load lower half of constant 0x698098d8
        movk x9, #0x6980, lsl #16     // Load upper half of constant 0x698098d8
        add w17, w17, w5              // Add dest value
        add w9, w17, w9               // Add constant 0x698098d8
        add w17, w9, w6               // Add aux function result
        ror w17, w17, #25             // Rotate left s=7 bits
        eor x9, x4, x8                // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w6, w4, w17               // Add X parameter round 1 A=FF(A, B, C, D, 0x698098d8, s=7, M[8])
        and x17, x9, x6               // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x9, x17, x8               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x17, #0xf7af             // Load lower half of constant 0x8b44f7af
        movk x17, #0x8b44, lsl #16    // Load upper half of constant 0x8b44f7af
        add w19, w19, w24             // Add dest value
        add w17, w19, w17             // Add constant 0x8b44f7af
        add w19, w17, w9              // Add aux function result
        ror w19, w19, #20             // Rotate left s=12 bits
        eor x9, x6, x4                // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w17, w6, w19              // Add X parameter round 1 D=FF(D, A, B, C, 0x8b44f7af, s=12, M[9])
        and x9, x9, x17               // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x9, x9, x4                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x11, #0x5bb1             // Load lower half of constant 0xffff5bb1
        movk x11, #0xffff, lsl #16    // Load upper half of constant 0xffff5bb1
        add w8, w8, w16               // Add dest value
        add w8, w8, w11               // Add constant 0xffff5bb1
        add w8, w8, w9                // Add aux function result
        ror w8, w8, #15               // Rotate left s=17 bits
        eor x9, x17, x6               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w8, w17, w8               // Add X parameter round 1 C=FF(C, D, A, B, 0xffff5bb1, s=17, M[10])
        and x9, x9, x8                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x9, x9, x6                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x11, #0xd7be             // Load lower half of constant 0x895cd7be
        movk x11, #0x895c, lsl #16    // Load upper half of constant 0x895cd7be
        add w4, w4, w25               // Add dest value
        add w4, w4, w11               // Add constant 0x895cd7be
        add w9, w4, w9                // Add aux function result
        ror w9, w9, #10               // Rotate left s=22 bits
        eor x4, x8, x17               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w9, w8, w9                // Add X parameter round 1 B=FF(B, C, D, A, 0x895cd7be, s=22, M[11])
        ldp w11, w26, [x1, #48]       // Load 2 words of input data0 M[12],M[13]
        ldp w12, w27, [x1, #56]       // Load 2 words of input data0 M[14],M[15]
#ifdef __AARCH64EB__
        rev w11, w11
        rev w26, w26
        rev w12, w12
        rev w27, w27
#endif
        and x4, x4, x9                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x4, x4, x17               // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x19, #0x1122             // Load lower half of constant 0x6b901122
        movk x19, #0x6b90, lsl #16    // Load upper half of constant 0x6b901122
        add w6, w6, w11               // Add dest value
        add w6, w6, w19               // Add constant 0x6b901122
        add w4, w6, w4                // Add aux function result
        ror w4, w4, #25               // Rotate left s=7 bits
        eor x6, x9, x8                // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w4, w9, w4                // Add X parameter round 1 A=FF(A, B, C, D, 0x6b901122, s=7, M[12])
        and x6, x6, x4                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x6, x8                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x19, #0x7193             // Load lower half of constant 0xfd987193
        movk x19, #0xfd98, lsl #16    // Load upper half of constant 0xfd987193
        add w17, w17, w26             // Add dest value
        add w17, w17, w19             // Add constant 0xfd987193
        add w17, w17, w6              // Add aux function result
        ror w17, w17, #20             // Rotate left s=12 bits
        eor x6, x4, x9                // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w17, w4, w17              // Add X parameter round 1 D=FF(D, A, B, C, 0xfd987193, s=12, M[13])
        and x6, x6, x17               // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x6, x9                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x13, #0x438e             // Load lower half of constant 0xa679438e
        movk x13, #0xa679, lsl #16    // Load upper half of constant 0xa679438e
        add w8, w8, w12               // Add dest value
        add w8, w8, w13               // Add constant 0xa679438e
        add w8, w8, w6                // Add aux function result
        ror w8, w8, #15               // Rotate left s=17 bits
        eor x6, x17, x4               // Begin aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        add w8, w17, w8               // Add X parameter round 1 C=FF(C, D, A, B, 0xa679438e, s=17, M[14])
        and x6, x6, x8                // Continue aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        eor x6, x6, x4                // End aux function round 1 F(x,y,z)=(((y^z)&x)^z)
        movz x13, #0x821              // Load lower half of constant 0x49b40821
        movk x13, #0x49b4, lsl #16    // Load upper half of constant 0x49b40821
        add w9, w9, w27               // Add dest value
        add w9, w9, w13               // Add constant 0x49b40821
        add w9, w9, w6                // Add aux function result
        ror w9, w9, #10               // Rotate left s=22 bits
        bic x6, x8, x17               // Aux function round 2 (~z & y)
        add w9, w8, w9                // Add X parameter round 1 B=FF(B, C, D, A, 0x49b40821, s=22, M[15])
        movz x13, #0x2562             // Load lower half of constant 0xf61e2562
        movk x13, #0xf61e, lsl #16    // Load upper half of constant 0xf61e2562
        add w4, w4, w20               // Add dest value
        add w4, w4, w13               // Add constant 0xf61e2562
        and x13, x9, x17              // Aux function round 2 (x & z)
        add w4, w4, w6                // Add (~z & y)
        add w4, w4, w13               // Add (x & z)
        ror w4, w4, #27               // Rotate left s=5 bits
        bic x6, x9, x8                // Aux function round 2 (~z & y)
        add w4, w9, w4                // Add X parameter round 2 A=GG(A, B, C, D, 0xf61e2562, s=5, M[1])
        movz x13, #0xb340             // Load lower half of constant 0xc040b340
        movk x13, #0xc040, lsl #16    // Load upper half of constant 0xc040b340
        add w17, w17, w7              // Add dest value
        add w17, w17, w13             // Add constant 0xc040b340
        and x13, x4, x8               // Aux function round 2 (x & z)
        add w17, w17, w6              // Add (~z & y)
        add w17, w17, w13             // Add (x & z)
        ror w17, w17, #23             // Rotate left s=9 bits
        bic x6, x4, x9                // Aux function round 2 (~z & y)
        add w17, w4, w17              // Add X parameter round 2 D=GG(D, A, B, C, 0xc040b340, s=9, M[6])
        movz x13, #0x5a51             // Load lower half of constant 0x265e5a51
        movk x13, #0x265e, lsl #16    // Load upper half of constant 0x265e5a51
        add w8, w8, w25               // Add dest value
        add w8, w8, w13               // Add constant 0x265e5a51
        and x13, x17, x9              // Aux function round 2 (x & z)
        add w8, w8, w6                // Add (~z & y)
        add w8, w8, w13               // Add (x & z)
        ror w8, w8, #18               // Rotate left s=14 bits
        bic x6, x17, x4               // Aux function round 2 (~z & y)
        add w8, w17, w8               // Add X parameter round 2 C=GG(C, D, A, B, 0x265e5a51, s=14, M[11])
        movz x13, #0xc7aa             // Load lower half of constant 0xe9b6c7aa
        movk x13, #0xe9b6, lsl #16    // Load upper half of constant 0xe9b6c7aa
        add w9, w9, w15               // Add dest value
        add w9, w9, w13               // Add constant 0xe9b6c7aa
        and x13, x8, x4               // Aux function round 2 (x & z)
        add w9, w9, w6                // Add (~z & y)
        add w9, w9, w13               // Add (x & z)
        ror w9, w9, #12               // Rotate left s=20 bits
        bic x6, x8, x17               // Aux function round 2 (~z & y)
        add w9, w8, w9                // Add X parameter round 2 B=GG(B, C, D, A, 0xe9b6c7aa, s=20, M[0])
        movz x13, #0x105d             // Load lower half of constant 0xd62f105d
        movk x13, #0xd62f, lsl #16    // Load upper half of constant 0xd62f105d
        add w4, w4, w22               // Add dest value
        add w4, w4, w13               // Add constant 0xd62f105d
        and x13, x9, x17              // Aux function round 2 (x & z)
        add w4, w4, w6                // Add (~z & y)
        add w4, w4, w13               // Add (x & z)
        ror w4, w4, #27               // Rotate left s=5 bits
        bic x6, x9, x8                // Aux function round 2 (~z & y)
        add w4, w9, w4                // Add X parameter round 2 A=GG(A, B, C, D, 0xd62f105d, s=5, M[5])
        movz x13, #0x1453             // Load lower half of constant 0x2441453
        movk x13, #0x244, lsl #16     // Load upper half of constant 0x2441453
        add w17, w17, w16             // Add dest value
        add w17, w17, w13             // Add constant 0x2441453
        and x13, x4, x8               // Aux function round 2 (x & z)
        add w17, w17, w6              // Add (~z & y)
        add w17, w17, w13             // Add (x & z)
        ror w17, w17, #23             // Rotate left s=9 bits
        bic x6, x4, x9                // Aux function round 2 (~z & y)
        add w17, w4, w17              // Add X parameter round 2 D=GG(D, A, B, C, 0x2441453, s=9, M[10])
        movz x13, #0xe681             // Load lower half of constant 0xd8a1e681
        movk x13, #0xd8a1, lsl #16    // Load upper half of constant 0xd8a1e681
        add w8, w8, w27               // Add dest value
        add w8, w8, w13               // Add constant 0xd8a1e681
        and x13, x17, x9              // Aux function round 2 (x & z)
        add w8, w8, w6                // Add (~z & y)
        add w8, w8, w13               // Add (x & z)
        ror w8, w8, #18               // Rotate left s=14 bits
        bic x6, x17, x4               // Aux function round 2 (~z & y)
        add w8, w17, w8               // Add X parameter round 2 C=GG(C, D, A, B, 0xd8a1e681, s=14, M[15])
        movz x13, #0xfbc8             // Load lower half of constant 0xe7d3fbc8
        movk x13, #0xe7d3, lsl #16    // Load upper half of constant 0xe7d3fbc8
        add w9, w9, w14               // Add dest value
        add w9, w9, w13               // Add constant 0xe7d3fbc8
        and x13, x8, x4               // Aux function round 2 (x & z)
        add w9, w9, w6                // Add (~z & y)
        add w9, w9, w13               // Add (x & z)
        ror w9, w9, #12               // Rotate left s=20 bits
        bic x6, x8, x17               // Aux function round 2 (~z & y)
        add w9, w8, w9                // Add X parameter round 2 B=GG(B, C, D, A, 0xe7d3fbc8, s=20, M[4])
        movz x13, #0xcde6             // Load lower half of constant 0x21e1cde6
        movk x13, #0x21e1, lsl #16    // Load upper half of constant 0x21e1cde6
        add w4, w4, w24               // Add dest value
        add w4, w4, w13               // Add constant 0x21e1cde6
        and x13, x9, x17              // Aux function round 2 (x & z)
        add w4, w4, w6                // Add (~z & y)
        add w4, w4, w13               // Add (x & z)
        ror w4, w4, #27               // Rotate left s=5 bits
        bic x6, x9, x8                // Aux function round 2 (~z & y)
        add w4, w9, w4                // Add X parameter round 2 A=GG(A, B, C, D, 0x21e1cde6, s=5, M[9])
        movz x13, #0x7d6              // Load lower half of constant 0xc33707d6
        movk x13, #0xc337, lsl #16    // Load upper half of constant 0xc33707d6
        add w17, w17, w12             // Add dest value
        add w17, w17, w13             // Add constant 0xc33707d6
        and x13, x4, x8               // Aux function round 2 (x & z)
        add w17, w17, w6              // Add (~z & y)
        add w17, w17, w13             // Add (x & z)
        ror w17, w17, #23             // Rotate left s=9 bits
        bic x6, x4, x9                // Aux function round 2 (~z & y)
        add w17, w4, w17              // Add X parameter round 2 D=GG(D, A, B, C, 0xc33707d6, s=9, M[14])
        movz x13, #0xd87              // Load lower half of constant 0xf4d50d87
        movk x13, #0xf4d5, lsl #16    // Load upper half of constant 0xf4d50d87
        add w8, w8, w21               // Add dest value
        add w8, w8, w13               // Add constant 0xf4d50d87
        and x13, x17, x9              // Aux function round 2 (x & z)
        add w8, w8, w6                // Add (~z & y)
        add w8, w8, w13               // Add (x & z)
        ror w8, w8, #18               // Rotate left s=14 bits
        bic x6, x17, x4               // Aux function round 2 (~z & y)
        add w8, w17, w8               // Add X parameter round 2 C=GG(C, D, A, B, 0xf4d50d87, s=14, M[3])
        movz x13, #0x14ed             // Load lower half of constant 0x455a14ed
        movk x13, #0x455a, lsl #16    // Load upper half of constant 0x455a14ed
        add w9, w9, w5                // Add dest value
        add w9, w9, w13               // Add constant 0x455a14ed
        and x13, x8, x4               // Aux function round 2 (x & z)
        add w9, w9, w6                // Add (~z & y)
        add w9, w9, w13               // Add (x & z)
        ror w9, w9, #12               // Rotate left s=20 bits
        bic x6, x8, x17               // Aux function round 2 (~z & y)
        add w9, w8, w9                // Add X parameter round 2 B=GG(B, C, D, A, 0x455a14ed, s=20, M[8])
        movz x13, #0xe905             // Load lower half of constant 0xa9e3e905
        movk x13, #0xa9e3, lsl #16    // Load upper half of constant 0xa9e3e905
        add w4, w4, w26               // Add dest value
        add w4, w4, w13               // Add constant 0xa9e3e905
        and x13, x9, x17              // Aux function round 2 (x & z)
        add w4, w4, w6                // Add (~z & y)
        add w4, w4, w13               // Add (x & z)
        ror w4, w4, #27               // Rotate left s=5 bits
        bic x6, x9, x8                // Aux function round 2 (~z & y)
        add w4, w9, w4                // Add X parameter round 2 A=GG(A, B, C, D, 0xa9e3e905, s=5, M[13])
        movz x13, #0xa3f8             // Load lower half of constant 0xfcefa3f8
        movk x13, #0xfcef, lsl #16    // Load upper half of constant 0xfcefa3f8
        add w17, w17, w3              // Add dest value
        add w17, w17, w13             // Add constant 0xfcefa3f8
        and x13, x4, x8               // Aux function round 2 (x & z)
        add w17, w17, w6              // Add (~z & y)
        add w17, w17, w13             // Add (x & z)
        ror w17, w17, #23             // Rotate left s=9 bits
        bic x6, x4, x9                // Aux function round 2 (~z & y)
        add w17, w4, w17              // Add X parameter round 2 D=GG(D, A, B, C, 0xfcefa3f8, s=9, M[2])
        movz x13, #0x2d9              // Load lower half of constant 0x676f02d9
        movk x13, #0x676f, lsl #16    // Load upper half of constant 0x676f02d9
        add w8, w8, w23               // Add dest value
        add w8, w8, w13               // Add constant 0x676f02d9
        and x13, x17, x9              // Aux function round 2 (x & z)
        add w8, w8, w6                // Add (~z & y)
        add w8, w8, w13               // Add (x & z)
        ror w8, w8, #18               // Rotate left s=14 bits
        bic x6, x17, x4               // Aux function round 2 (~z & y)
        add w8, w17, w8               // Add X parameter round 2 C=GG(C, D, A, B, 0x676f02d9, s=14, M[7])
        movz x13, #0x4c8a             // Load lower half of constant 0x8d2a4c8a
        movk x13, #0x8d2a, lsl #16    // Load upper half of constant 0x8d2a4c8a
        add w9, w9, w11               // Add dest value
        add w9, w9, w13               // Add constant 0x8d2a4c8a
        and x13, x8, x4               // Aux function round 2 (x & z)
        add w9, w9, w6                // Add (~z & y)
        add w9, w9, w13               // Add (x & z)
        eor x6, x8, x17               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w9, w9, #12               // Rotate left s=20 bits
        movz x10, #0x3942             // Load lower half of constant 0xfffa3942
        add w9, w8, w9                // Add X parameter round 2 B=GG(B, C, D, A, 0x8d2a4c8a, s=20, M[12])
        movk x10, #0xfffa, lsl #16    // Load upper half of constant 0xfffa3942
        add w4, w4, w22               // Add dest value
        eor x6, x6, x9                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w4, w4, w10               // Add constant 0xfffa3942
        add w4, w4, w6                // Add aux function result
        ror w4, w4, #28               // Rotate left s=4 bits
        eor x6, x9, x8                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x10, #0xf681             // Load lower half of constant 0x8771f681
        add w4, w9, w4                // Add X parameter round 3 A=HH(A, B, C, D, 0xfffa3942, s=4, M[5])
        movk x10, #0x8771, lsl #16    // Load upper half of constant 0x8771f681
        add w17, w17, w5              // Add dest value
        eor x6, x6, x4                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w17, w17, w10             // Add constant 0x8771f681
        add w17, w17, w6              // Add aux function result
        eor x6, x4, x9                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w17, w17, #21             // Rotate left s=11 bits
        movz x13, #0x6122             // Load lower half of constant 0x6d9d6122
        add w17, w4, w17              // Add X parameter round 3 D=HH(D, A, B, C, 0x8771f681, s=11, M[8])
        movk x13, #0x6d9d, lsl #16    // Load upper half of constant 0x6d9d6122
        add w8, w8, w25               // Add dest value
        eor x6, x6, x17               // End aux function round 3 H(x,y,z)=(x^y^z)
        add w8, w8, w13               // Add constant 0x6d9d6122
        add w8, w8, w6                // Add aux function result
        ror w8, w8, #16               // Rotate left s=16 bits
        eor x6, x17, x4               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x13, #0x380c             // Load lower half of constant 0xfde5380c
        add w8, w17, w8               // Add X parameter round 3 C=HH(C, D, A, B, 0x6d9d6122, s=16, M[11])
        movk x13, #0xfde5, lsl #16    // Load upper half of constant 0xfde5380c
        add w9, w9, w12               // Add dest value
        eor x6, x6, x8                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w9, w9, w13               // Add constant 0xfde5380c
        add w9, w9, w6                // Add aux function result
        eor x6, x8, x17               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w9, w9, #9                // Rotate left s=23 bits
        movz x10, #0xea44             // Load lower half of constant 0xa4beea44
        add w9, w8, w9                // Add X parameter round 3 B=HH(B, C, D, A, 0xfde5380c, s=23, M[14])
        movk x10, #0xa4be, lsl #16    // Load upper half of constant 0xa4beea44
        add w4, w4, w20               // Add dest value
        eor x6, x6, x9                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w4, w4, w10               // Add constant 0xa4beea44
        add w4, w4, w6                // Add aux function result
        ror w4, w4, #28               // Rotate left s=4 bits
        eor x6, x9, x8                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x10, #0xcfa9             // Load lower half of constant 0x4bdecfa9
        add w4, w9, w4                // Add X parameter round 3 A=HH(A, B, C, D, 0xa4beea44, s=4, M[1])
        movk x10, #0x4bde, lsl #16    // Load upper half of constant 0x4bdecfa9
        add w17, w17, w14             // Add dest value
        eor x6, x6, x4                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w17, w17, w10             // Add constant 0x4bdecfa9
        add w17, w17, w6              // Add aux function result
        eor x6, x4, x9                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w17, w17, #21             // Rotate left s=11 bits
        movz x13, #0x4b60             // Load lower half of constant 0xf6bb4b60
        add w17, w4, w17              // Add X parameter round 3 D=HH(D, A, B, C, 0x4bdecfa9, s=11, M[4])
        movk x13, #0xf6bb, lsl #16    // Load upper half of constant 0xf6bb4b60
        add w8, w8, w23               // Add dest value
        eor x6, x6, x17               // End aux function round 3 H(x,y,z)=(x^y^z)
        add w8, w8, w13               // Add constant 0xf6bb4b60
        add w8, w8, w6                // Add aux function result
        ror w8, w8, #16               // Rotate left s=16 bits
        eor x6, x17, x4               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x13, #0xbc70             // Load lower half of constant 0xbebfbc70
        add w8, w17, w8               // Add X parameter round 3 C=HH(C, D, A, B, 0xf6bb4b60, s=16, M[7])
        movk x13, #0xbebf, lsl #16    // Load upper half of constant 0xbebfbc70
        add w9, w9, w16               // Add dest value
        eor x6, x6, x8                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w9, w9, w13               // Add constant 0xbebfbc70
        add w9, w9, w6                // Add aux function result
        eor x6, x8, x17               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w9, w9, #9                // Rotate left s=23 bits
        movz x10, #0x7ec6             // Load lower half of constant 0x289b7ec6
        add w9, w8, w9                // Add X parameter round 3 B=HH(B, C, D, A, 0xbebfbc70, s=23, M[10])
        movk x10, #0x289b, lsl #16    // Load upper half of constant 0x289b7ec6
        add w4, w4, w26               // Add dest value
        eor x6, x6, x9                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w4, w4, w10               // Add constant 0x289b7ec6
        add w4, w4, w6                // Add aux function result
        ror w4, w4, #28               // Rotate left s=4 bits
        eor x6, x9, x8                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x10, #0x27fa             // Load lower half of constant 0xeaa127fa
        add w4, w9, w4                // Add X parameter round 3 A=HH(A, B, C, D, 0x289b7ec6, s=4, M[13])
        movk x10, #0xeaa1, lsl #16    // Load upper half of constant 0xeaa127fa
        add w17, w17, w15             // Add dest value
        eor x6, x6, x4                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w17, w17, w10             // Add constant 0xeaa127fa
        add w17, w17, w6              // Add aux function result
        eor x6, x4, x9                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w17, w17, #21             // Rotate left s=11 bits
        movz x13, #0x3085             // Load lower half of constant 0xd4ef3085
        add w17, w4, w17              // Add X parameter round 3 D=HH(D, A, B, C, 0xeaa127fa, s=11, M[0])
        movk x13, #0xd4ef, lsl #16    // Load upper half of constant 0xd4ef3085
        add w8, w8, w21               // Add dest value
        eor x6, x6, x17               // End aux function round 3 H(x,y,z)=(x^y^z)
        add w8, w8, w13               // Add constant 0xd4ef3085
        add w8, w8, w6                // Add aux function result
        ror w8, w8, #16               // Rotate left s=16 bits
        eor x6, x17, x4               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x13, #0x1d05             // Load lower half of constant 0x4881d05
        add w8, w17, w8               // Add X parameter round 3 C=HH(C, D, A, B, 0xd4ef3085, s=16, M[3])
        movk x13, #0x488, lsl #16     // Load upper half of constant 0x4881d05
        add w9, w9, w7                // Add dest value
        eor x6, x6, x8                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w9, w9, w13               // Add constant 0x4881d05
        add w9, w9, w6                // Add aux function result
        eor x6, x8, x17               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w9, w9, #9                // Rotate left s=23 bits
        movz x10, #0xd039             // Load lower half of constant 0xd9d4d039
        add w9, w8, w9                // Add X parameter round 3 B=HH(B, C, D, A, 0x4881d05, s=23, M[6])
        movk x10, #0xd9d4, lsl #16    // Load upper half of constant 0xd9d4d039
        add w4, w4, w24               // Add dest value
        eor x6, x6, x9                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w4, w4, w10               // Add constant 0xd9d4d039
        add w4, w4, w6                // Add aux function result
        ror w4, w4, #28               // Rotate left s=4 bits
        eor x6, x9, x8                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x10, #0x99e5             // Load lower half of constant 0xe6db99e5
        add w4, w9, w4                // Add X parameter round 3 A=HH(A, B, C, D, 0xd9d4d039, s=4, M[9])
        movk x10, #0xe6db, lsl #16    // Load upper half of constant 0xe6db99e5
        add w17, w17, w11             // Add dest value
        eor x6, x6, x4                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w17, w17, w10             // Add constant 0xe6db99e5
        add w17, w17, w6              // Add aux function result
        eor x6, x4, x9                // Begin aux function round 3 H(x,y,z)=(x^y^z)
        ror w17, w17, #21             // Rotate left s=11 bits
        movz x13, #0x7cf8             // Load lower half of constant 0x1fa27cf8
        add w17, w4, w17              // Add X parameter round 3 D=HH(D, A, B, C, 0xe6db99e5, s=11, M[12])
        movk x13, #0x1fa2, lsl #16    // Load upper half of constant 0x1fa27cf8
        add w8, w8, w27               // Add dest value
        eor x6, x6, x17               // End aux function round 3 H(x,y,z)=(x^y^z)
        add w8, w8, w13               // Add constant 0x1fa27cf8
        add w8, w8, w6                // Add aux function result
        ror w8, w8, #16               // Rotate left s=16 bits
        eor x6, x17, x4               // Begin aux function round 3 H(x,y,z)=(x^y^z)
        movz x13, #0x5665             // Load lower half of constant 0xc4ac5665
        add w8, w17, w8               // Add X parameter round 3 C=HH(C, D, A, B, 0x1fa27cf8, s=16, M[15])
        movk x13, #0xc4ac, lsl #16    // Load upper half of constant 0xc4ac5665
        add w9, w9, w3                // Add dest value
        eor x6, x6, x8                // End aux function round 3 H(x,y,z)=(x^y^z)
        add w9, w9, w13               // Add constant 0xc4ac5665
        add w9, w9, w6                // Add aux function result
        ror w9, w9, #9                // Rotate left s=23 bits
        movz x6, #0x2244              // Load lower half of constant 0xf4292244
        movk x6, #0xf429, lsl #16     // Load upper half of constant 0xf4292244
        add w9, w8, w9                // Add X parameter round 3 B=HH(B, C, D, A, 0xc4ac5665, s=23, M[2])
        add w4, w4, w15               // Add dest value
        orn x13, x9, x17              // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w4, w4, w6                // Add constant 0xf4292244
        eor x6, x8, x13               // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w4, w4, w6                // Add aux function result
        ror w4, w4, #26               // Rotate left s=6 bits
        movz x6, #0xff97              // Load lower half of constant 0x432aff97
        movk x6, #0x432a, lsl #16     // Load upper half of constant 0x432aff97
        add w4, w9, w4                // Add X parameter round 4 A=II(A, B, C, D, 0xf4292244, s=6, M[0])
        orn x10, x4, x8               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w17, w17, w23             // Add dest value
        eor x10, x9, x10              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w17, w17, w6              // Add constant 0x432aff97
        add w6, w17, w10              // Add aux function result
        ror w6, w6, #22               // Rotate left s=10 bits
        movz x17, #0x23a7             // Load lower half of constant 0xab9423a7
        movk x17, #0xab94, lsl #16    // Load upper half of constant 0xab9423a7
        add w6, w4, w6                // Add X parameter round 4 D=II(D, A, B, C, 0x432aff97, s=10, M[7])
        add w8, w8, w12               // Add dest value
        orn x10, x6, x9               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w8, w17               // Add constant 0xab9423a7
        eor x17, x4, x10              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w8, w17               // Add aux function result
        ror w8, w8, #17               // Rotate left s=15 bits
        movz x17, #0xa039             // Load lower half of constant 0xfc93a039
        movk x17, #0xfc93, lsl #16    // Load upper half of constant 0xfc93a039
        add w8, w6, w8                // Add X parameter round 4 C=II(C, D, A, B, 0xab9423a7, s=15, M[14])
        orn x13, x8, x4               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w9, w22               // Add dest value
        eor x13, x6, x13              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w9, w17               // Add constant 0xfc93a039
        add w17, w9, w13              // Add aux function result
        ror w17, w17, #11             // Rotate left s=21 bits
        movz x9, #0x59c3              // Load lower half of constant 0x655b59c3
        movk x9, #0x655b, lsl #16     // Load upper half of constant 0x655b59c3
        add w17, w8, w17              // Add X parameter round 4 B=II(B, C, D, A, 0xfc93a039, s=21, M[5])
        add w4, w4, w11               // Add dest value
        orn x13, x17, x6              // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w4, w9                // Add constant 0x655b59c3
        eor x4, x8, x13               // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w9, w4                // Add aux function result
        ror w9, w9, #26               // Rotate left s=6 bits
        movz x4, #0xcc92              // Load lower half of constant 0x8f0ccc92
        movk x4, #0x8f0c, lsl #16     // Load upper half of constant 0x8f0ccc92
        add w9, w17, w9               // Add X parameter round 4 A=II(A, B, C, D, 0x655b59c3, s=6, M[12])
        orn x10, x9, x8               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w6, w6, w21               // Add dest value
        eor x10, x17, x10             // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w4, w6, w4                // Add constant 0x8f0ccc92
        add w6, w4, w10               // Add aux function result
        ror w6, w6, #22               // Rotate left s=10 bits
        movz x4, #0xf47d              // Load lower half of constant 0xffeff47d
        movk x4, #0xffef, lsl #16     // Load upper half of constant 0xffeff47d
        add w6, w9, w6                // Add X parameter round 4 D=II(D, A, B, C, 0x8f0ccc92, s=10, M[3])
        add w8, w8, w16               // Add dest value
        orn x10, x6, x17              // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w8, w4                // Add constant 0xffeff47d
        eor x4, x9, x10               // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w8, w4                // Add aux function result
        ror w8, w8, #17               // Rotate left s=15 bits
        movz x4, #0x5dd1              // Load lower half of constant 0x85845dd1
        movk x4, #0x8584, lsl #16     // Load upper half of constant 0x85845dd1
        add w8, w6, w8                // Add X parameter round 4 C=II(C, D, A, B, 0xffeff47d, s=15, M[10])
        orn x10, x8, x9               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w17, w20             // Add dest value
        eor x17, x6, x10              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w15, w4              // Add constant 0x85845dd1
        add w4, w15, w17              // Add aux function result
        ror w4, w4, #11               // Rotate left s=21 bits
        movz x15, #0x7e4f             // Load lower half of constant 0x6fa87e4f
        movk x15, #0x6fa8, lsl #16    // Load upper half of constant 0x6fa87e4f
        add w17, w8, w4               // Add X parameter round 4 B=II(B, C, D, A, 0x85845dd1, s=21, M[1])
        add w4, w9, w5                // Add dest value
        orn x9, x17, x6               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w4, w15              // Add constant 0x6fa87e4f
        eor x4, x8, x9                // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w15, w4               // Add aux function result
        ror w9, w9, #26               // Rotate left s=6 bits
        movz x15, #0xe6e0             // Load lower half of constant 0xfe2ce6e0
        movk x15, #0xfe2c, lsl #16    // Load upper half of constant 0xfe2ce6e0
        add w4, w17, w9               // Add X parameter round 4 A=II(A, B, C, D, 0x6fa87e4f, s=6, M[8])
        orn x9, x4, x8                // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w6, w6, w27               // Add dest value
        eor x9, x17, x9               // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w6, w15              // Add constant 0xfe2ce6e0
        add w6, w15, w9               // Add aux function result
        ror w6, w6, #22               // Rotate left s=10 bits
        movz x9, #0x4314              // Load lower half of constant 0xa3014314
        movk x9, #0xa301, lsl #16     // Load upper half of constant 0xa3014314
        add w15, w4, w6               // Add X parameter round 4 D=II(D, A, B, C, 0xfe2ce6e0, s=10, M[15])
        add w6, w8, w7                // Add dest value
        orn x7, x15, x17              // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w6, w9                // Add constant 0xa3014314
        eor x9, x4, x7                // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w6, w8, w9                // Add aux function result
        ror w6, w6, #17               // Rotate left s=15 bits
        movz x7, #0x11a1              // Load lower half of constant 0x4e0811a1
        movk x7, #0x4e08, lsl #16     // Load upper half of constant 0x4e0811a1
        add w8, w15, w6               // Add X parameter round 4 C=II(C, D, A, B, 0xa3014314, s=15, M[6])
        orn x9, x8, x4                // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w6, w17, w26              // Add dest value
        eor x17, x15, x9              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w9, w6, w7                // Add constant 0x4e0811a1
        add w7, w9, w17               // Add aux function result
        ror w7, w7, #11               // Rotate left s=21 bits
        movz x6, #0x7e82              // Load lower half of constant 0xf7537e82
        movk x6, #0xf753, lsl #16     // Load upper half of constant 0xf7537e82
        add w9, w8, w7                // Add X parameter round 4 B=II(B, C, D, A, 0x4e0811a1, s=21, M[13])
        add w17, w4, w14              // Add dest value
        orn x7, x9, x15               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w14, w17, w6              // Add constant 0xf7537e82
        eor x4, x8, x7                // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w17, w14, w4              // Add aux function result
        ror w17, w17, #26             // Rotate left s=6 bits
        movz x6, #0xf235              // Load lower half of constant 0xbd3af235
        movk x6, #0xbd3a, lsl #16     // Load upper half of constant 0xbd3af235
        add w7, w9, w17               // Add X parameter round 4 A=II(A, B, C, D, 0xf7537e82, s=6, M[4])
        orn x14, x7, x8               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w4, w15, w25              // Add dest value
        eor x17, x9, x14              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w4, w6               // Add constant 0xbd3af235
        add w16, w15, w17             // Add aux function result
        ror w16, w16, #22             // Rotate left s=10 bits
        movz x14, #0xd2bb             // Load lower half of constant 0x2ad7d2bb
        movk x14, #0x2ad7, lsl #16    // Load upper half of constant 0x2ad7d2bb
        add w4, w7, w16               // Add X parameter round 4 D=II(D, A, B, C, 0xbd3af235, s=10, M[11])
        add w6, w8, w3                // Add dest value
        orn x15, x4, x9               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w17, w6, w14              // Add constant 0x2ad7d2bb
        eor x16, x7, x15              // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w8, w17, w16              // Add aux function result
        ror w8, w8, #17               // Rotate left s=15 bits
        movz x3, #0xd391              // Load lower half of constant 0xeb86d391
        movk x3, #0xeb86, lsl #16     // Load upper half of constant 0xeb86d391
        add w14, w4, w8               // Add X parameter round 4 C=II(C, D, A, B, 0x2ad7d2bb, s=15, M[2])
        orn x6, x14, x7               // Begin aux function round 4 I(x,y,z)=((~z|x)^y)
        add w15, w9, w24              // Add dest value
        eor x17, x4, x6               // End aux function round 4 I(x,y,z)=((~z|x)^y)
        add w16, w15, w3              // Add constant 0xeb86d391
        add w8, w16, w17              // Add aux function result
        ror w8, w8, #11               // Rotate left s=21 bits
        ldp w6, w15, [x0]             // Reload MD5 state->A and state->B
        ldp w5, w9, [x0, #8]          // Reload MD5 state->C and state->D
        add w3, w14, w8               // Add X parameter round 4 B=II(B, C, D, A, 0xeb86d391, s=21, M[9])
        add w13, w4, w9               // Add result of MD5 rounds to state->D
        add w12, w14, w5              // Add result of MD5 rounds to state->C
        add w10, w7, w6               // Add result of MD5 rounds to state->A
        add w11, w3, w15              // Add result of MD5 rounds to state->B
        stp w12, w13, [x0, #8]        // Store MD5 states C,D
        stp w10, w11, [x0]            // Store MD5 states A,B
        add x1, x1, #64               // Increment data pointer
        subs w2, w2, #1               // Decrement block counter
        b.ne ossl_md5_blocks_loop

        ldp     x21,x22,[sp,#16]
        ldp     x23,x24,[sp,#32]
        ldp     x25,x26,[sp,#48]
        ldp     x27,x28,[sp,#64]
        ldp     x19,x20,[sp],#80
        ret

EOF

# EXCEPTION_DISPOSITION handler (EXCEPTION_RECORD *rec,ULONG64 frame,
#		CONTEXT *context,DISPATCHER_CONTEXT *disp)

print $code;

close STDOUT or die "error closing STDOUT: $!";
