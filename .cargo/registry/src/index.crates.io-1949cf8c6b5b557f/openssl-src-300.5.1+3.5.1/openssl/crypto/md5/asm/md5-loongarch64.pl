#! /usr/bin/env perl
# Author: Min Zhou <zhoumin@loongson.cn>
# Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the OpenSSL license (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Reference to crypto/md5/asm/md5-x86_64.pl
# MD5 optimized for LoongArch.

use strict;

my $code;

my ($zero,$ra,$tp,$sp,$fp)=map("\$r$_",(0..3,22));
my ($a0,$a1,$a2,$a3,$a4,$a5,$a6,$a7)=map("\$r$_",(4..11));
my ($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$x)=map("\$r$_",(12..21));

# $output is the last argument if it looks like a file (it has an extension)
my $output;
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
open STDOUT,">$output";

# round1_step() does:
#   dst = x + ((dst + F(x,y,z) + X[k] + T_i) <<< s)
#   $t1 = y ^ z
#   $t2 = dst + X[k_next]
sub round1_step
{
    my ($pos, $dst, $x, $y, $z, $k_next, $T_i, $s) = @_;
    my $T_i_h = ($T_i & 0xfffff000) >> 12;
    my $T_i_l = $T_i & 0xfff;

# In LoongArch we have to use two instructions of lu12i.w and ori to load a
# 32-bit immediate into a general register. Meanwhile, the instruction lu12i.w
# treats the 20-bit immediate as a signed number. So if the T_i_h is greater
# than or equal to (1<<19), we need provide lu12i.w a corresponding negative
# number whose complement equals to the sign extension of T_i_h.

# The details of the instruction lu12i.w can be found as following:
# https://loongson.github.io/LoongArch-Documentation/LoongArch-Vol1-EN.html#_lu12i_w_lu32i_d_lu52i_d

    $T_i_h = -((1<<32) - (0xfff00000 | $T_i_h)) if ($T_i_h >= (1<<19));

    $code .= "	ld.w	$t0,$a1,0		/* (NEXT STEP) X[0] */\n" if ($pos == -1);
    $code .= "	xor	$t1,$y,$z		/* y ^ z */\n" if ($pos == -1);
    $code .= "	add.w	$t2,$dst,$t0		/* dst + X[k] */\n" if ($pos == -1);
    $code .= <<EOF;
	lu12i.w	$t8,$T_i_h			/* load bits [31:12] of constant */
	and     $t1,$x,$t1			/* x & ... */
	ori	$t8,$t8,$T_i_l			/* load bits [11:0] of constant */
	xor     $t1,$z,$t1			/* z ^ ... */
	add.w   $t7,$t2,$t8			/* dst + X[k] + Const */
	ld.w	$t0,$a1,$k_next*4		/* (NEXT STEP) X[$k_next] */
	add.w	$dst,$t7,$t1			/* dst += ... */
	add.w	$t2,$z,$t0			/* (NEXT STEP) dst + X[$k_next] */
EOF

    $code .= "	rotri.w	$dst,$dst,32-$s		/* dst <<< s */\n";
    if ($pos != 1) {
        $code .= "	xor	$t1,$x,$y	/* (NEXT STEP) y ^ z */\n";
    } else {
        $code .= "	move	$t0,$a7		/* (NEXT ROUND) $t0 = z' (copy of z) */\n";
        $code .= "	nor	$t1,$zero,$a7	/* (NEXT ROUND) $t1 = not z' (copy of not z) */\n";
    }
    $code .= "	add.w   $dst,$dst,$x		/* dst += x */\n";
}

# round2_step() does:
#   dst = x + ((dst + G(x,y,z) + X[k] + T_i) <<< s)
#   $t0 = z' (copy of z for the next step)
#   $t1 = not z' (copy of not z for the next step)
#   $t2 = dst + X[k_next]
sub round2_step
{
    my ($pos, $dst, $x, $y, $z, $k_next, $T_i, $s) = @_;
    my $T_i_h = ($T_i & 0xfffff000) >> 12;
    my $T_i_l = $T_i & 0xfff;
    $T_i_h = -((1<<32) - (0xfff00000 | $T_i_h)) if ($T_i_h >= (1<<19));

    $code .= <<EOF;
	lu12i.w	$t8,$T_i_h			/* load bits [31:12] of Constant */
	and	$t0,$x,$t0			/* x & z */
	ori	$t8,$t8,$T_i_l			/* load bits [11:0] of Constant */
	and	$t1,$y,$t1			/* y & (not z) */
	add.w	$t7,$t2,$t8			/* dst + X[k] + Const */
	or	$t1,$t0,$t1			/* (y & (not z)) | (x & z) */
	ld.w	$t0,$a1,$k_next*4		/* (NEXT STEP) X[$k_next] */
	add.w	$dst,$t7,$t1			/* dst += ... */
	add.w	$t2,$z,$t0			/* (NEXT STEP) dst + X[$k_next] */
EOF

    $code .= "	rotri.w $dst,$dst,32-$s		/* dst <<< s */\n";
    if ($pos != 1) {
        $code .= "	move	$t0,$y		/* (NEXT STEP) z' = $y */\n";
        $code .= "	nor	$t1,$zero,$y	/* (NEXT STEP) not z' = not $y */\n";
    } else {
        $code .= "	xor	$t1,$a6,$a7	/* (NEXT ROUND) $t1 = y ^ z */\n";
    }
    $code .= "	add.w	$dst,$dst,$x		/* dst += x */\n";
}

# round3_step() does:
#   dst = x + ((dst + H(x,y,z) + X[k] + T_i) <<< s)
#   $t1 = y ^ z
#   $t2 = dst + X[k_next]
sub round3_step
{
    my ($pos, $dst, $x, $y, $z, $k_next, $T_i, $s) = @_;
    my $T_i_h = ($T_i & 0xfffff000) >> 12;
    my $T_i_l = $T_i & 0xfff;
    $T_i_h = -((1<<32) - (0xfff00000 | $T_i_h)) if ($T_i_h >= (1<<19));

    $code .= <<EOF;
	lu12i.w	$t8,$T_i_h			/* load bits [31:12] of Constant */
	xor	$t1,$x,$t1			/* x ^ ... */
	ori	$t8,$t8,$T_i_l			/* load bits [11:0] of Constant */
	add.w	$t7,$t2,$t8			/* dst + X[k] + Const */
	ld.w	$t0,$a1,$k_next*4		/* (NEXT STEP) X[$k_next] */
	add.w	$dst,$t7,$t1			/* dst += ... */
	add.w	$t2,$z,$t0			/* (NEXT STEP) dst + X[$k_next] */
EOF

    $code .= "	rotri.w $dst,$dst,32-$s		/* dst <<< s */\n";
    if ($pos != 1) {
	$code .= "	xor	$t1,$x,$y	/* (NEXT STEP) y ^ z */\n";
    } else {
        $code .= "	nor	$t1,$zero,$a7	/* (NEXT ROUND) $t1 = not z */\n";
    }
    $code .= "	add.w	$dst,$dst,$x		/* dst += x */\n";
}

# round4_step() does:
#   dst = x + ((dst + I(x,y,z) + X[k] + T_i) <<< s)
#   $t1 = not z' (copy of not z for the next step)
#   $t2 = dst + X[k_next]
sub round4_step
{
    my ($pos, $dst, $x, $y, $z, $k_next, $T_i, $s) = @_;
    my $T_i_h = ($T_i & 0xfffff000) >> 12;
    my $T_i_l = $T_i & 0xfff;
    $T_i_h = -((1<<32) - (0xfff00000 | $T_i_h)) if ($T_i_h >= (1<<19));

    $code .= <<EOF;
	lu12i.w	$t8,$T_i_h			/* load bits [31:12] of Constant */
	or	$t1,$x,$t1			/* x | ... */
	ori	$t8,$t8,$T_i_l			/* load bits [11:0] of Constant */
	xor	$t1,$y,$t1			/* y ^ ... */
	add.w	$t7,$t2,$t8			/* dst + X[k] + Const */
EOF

    if ($pos != 1) {
        $code .= "	ld.w	$t0,$a1,$k_next*4		/* (NEXT STEP) X[$k_next] */\n";
        $code .= "	add.w	$dst,$t7,$t1			/* dst += ... */\n";
        $code .= "	add.w	$t2,$z,$t0			/* (NEXT STEP) dst + X[$k_next] */\n";
        $code .= "	rotri.w $dst,$dst,32-$s			/* dst <<< s */\n";
        $code .= "	nor	$t1,$zero,$y			/* (NEXT STEP) not z' = not $y */\n";
        $code .= "	add.w	$dst,$dst,$x			/* dst += x */\n";
    } else {
        $code .= "	add.w	$a4,$t3,$a4			/* (NEXT LOOP) add old value of A */\n";
        $code .= "	add.w	$dst,$t7,$t1			/* dst += ... */\n";
        $code .= "	add.w	$a7,$t6,$a7			/* (NEXT LOOP) add old value of D */\n";
        $code .= "	rotri.w $dst,$dst,32-$s			/* dst <<< s */\n";
        $code .= "	addi.d	$a1,$a1,64			/* (NEXT LOOP) ptr += 64 */\n";
        $code .= "	add.w	$dst,$dst,$x			/* dst += x */\n";
    }
}

$code .= <<EOF;
.text

.globl ossl_md5_block_asm_data_order
.type ossl_md5_block_asm_data_order function
ossl_md5_block_asm_data_order:
	# $a0 = arg #1 (ctx, MD5_CTX pointer)
	# $a1 = arg #2 (ptr, data pointer)
	# $a2 = arg #3 (nbr, number of 16-word blocks to process)
	beqz	$a2,.Lend		# cmp nbr with 0, jmp if nbr == 0

	# ptr is '$a1'
	# end is '$a3'
	slli.d	$t0,$a2,6
	add.d	$a3,$a1,$t0

	# A is '$a4'
	# B is '$a5'
	# C is '$a6'
	# D is '$a7'
	ld.w	$a4,$a0,0	# a4 = ctx->A
	ld.w	$a5,$a0,4	# a5 = ctx->B
	ld.w	$a6,$a0,8	# a6 = ctx->C
	ld.w	$a7,$a0,12	# a7 = ctx->D

# BEGIN of loop over 16-word blocks
.align 6
.Lloop:
	# save old values of A, B, C, D
	move	$t3,$a4
	move	$t4,$a5
	move	$t5,$a6
	move	$t6,$a7

	preld	0,$a1,0
	preld	0,$a1,64
EOF

round1_step(-1, $a4, $a5, $a6, $a7, '1', 0xd76aa478, '7');
round1_step(0, $a7, $a4, $a5, $a6, '2', 0xe8c7b756, '12');
round1_step(0, $a6, $a7, $a4, $a5, '3', 0x242070db, '17');
round1_step(0, $a5, $a6, $a7, $a4, '4', 0xc1bdceee, '22');
round1_step(0, $a4, $a5, $a6, $a7, '5', 0xf57c0faf, '7');
round1_step(0, $a7, $a4, $a5, $a6, '6', 0x4787c62a, '12');
round1_step(0, $a6, $a7, $a4, $a5, '7', 0xa8304613, '17');
round1_step(0, $a5, $a6, $a7, $a4, '8', 0xfd469501, '22');
round1_step(0, $a4, $a5, $a6, $a7, '9', 0x698098d8, '7');
round1_step(0, $a7, $a4, $a5, $a6, '10', 0x8b44f7af, '12');
round1_step(0, $a6, $a7, $a4, $a5, '11', 0xffff5bb1, '17');
round1_step(0, $a5, $a6, $a7, $a4, '12', 0x895cd7be, '22');
round1_step(0, $a4, $a5, $a6, $a7, '13', 0x6b901122, '7');
round1_step(0, $a7, $a4, $a5, $a6, '14', 0xfd987193, '12');
round1_step(0, $a6, $a7, $a4, $a5, '15', 0xa679438e, '17');
round1_step(1, $a5, $a6, $a7, $a4, '1', 0x49b40821, '22');

round2_step(-1, $a4, $a5, $a6, $a7, '6', 0xf61e2562, '5');
round2_step(0, $a7, $a4, $a5, $a6, '11', 0xc040b340, '9');
round2_step(0, $a6, $a7, $a4, $a5, '0', 0x265e5a51, '14');
round2_step(0, $a5, $a6, $a7, $a4, '5', 0xe9b6c7aa, '20');
round2_step(0, $a4, $a5, $a6, $a7, '10', 0xd62f105d, '5');
round2_step(0, $a7, $a4, $a5, $a6, '15', 0x2441453, '9');
round2_step(0, $a6, $a7, $a4, $a5, '4', 0xd8a1e681, '14');
round2_step(0, $a5, $a6, $a7, $a4, '9', 0xe7d3fbc8, '20');
round2_step(0, $a4, $a5, $a6, $a7, '14', 0x21e1cde6, '5');
round2_step(0, $a7, $a4, $a5, $a6, '3', 0xc33707d6, '9');
round2_step(0, $a6, $a7, $a4, $a5, '8', 0xf4d50d87, '14');
round2_step(0, $a5, $a6, $a7, $a4, '13', 0x455a14ed, '20');
round2_step(0, $a4, $a5, $a6, $a7, '2', 0xa9e3e905, '5');
round2_step(0, $a7, $a4, $a5, $a6, '7', 0xfcefa3f8, '9');
round2_step(0, $a6, $a7, $a4, $a5, '12', 0x676f02d9, '14');
round2_step(1, $a5, $a6, $a7, $a4, '5', 0x8d2a4c8a, '20');

round3_step(-1, $a4, $a5, $a6, $a7, '8', 0xfffa3942, '4');
round3_step(0, $a7, $a4, $a5, $a6, '11', 0x8771f681, '11');
round3_step(0, $a6, $a7, $a4, $a5, '14', 0x6d9d6122, '16');
round3_step(0, $a5, $a6, $a7, $a4, '1', 0xfde5380c, '23');
round3_step(0, $a4, $a5, $a6, $a7, '4', 0xa4beea44, '4');
round3_step(0, $a7, $a4, $a5, $a6, '7', 0x4bdecfa9, '11');
round3_step(0, $a6, $a7, $a4, $a5, '10', 0xf6bb4b60, '16');
round3_step(0, $a5, $a6, $a7, $a4, '13', 0xbebfbc70, '23');
round3_step(0, $a4, $a5, $a6, $a7, '0', 0x289b7ec6, '4');
round3_step(0, $a7, $a4, $a5, $a6, '3', 0xeaa127fa, '11');
round3_step(0, $a6, $a7, $a4, $a5, '6', 0xd4ef3085, '16');
round3_step(0, $a5, $a6, $a7, $a4, '9', 0x4881d05, '23');
round3_step(0, $a4, $a5, $a6, $a7, '12', 0xd9d4d039, '4');
round3_step(0, $a7, $a4, $a5, $a6, '15', 0xe6db99e5, '11');
round3_step(0, $a6, $a7, $a4, $a5, '2', 0x1fa27cf8, '16');
round3_step(1, $a5, $a6, $a7, $a4, '0', 0xc4ac5665, '23');

round4_step(-1, $a4, $a5, $a6, $a7, '7', 0xf4292244, '6');
round4_step(0, $a7, $a4, $a5, $a6, '14', 0x432aff97, '10');
round4_step(0, $a6, $a7, $a4, $a5, '5', 0xab9423a7, '15');
round4_step(0, $a5, $a6, $a7, $a4, '12', 0xfc93a039, '21');
round4_step(0, $a4, $a5, $a6, $a7, '3', 0x655b59c3, '6');
round4_step(0, $a7, $a4, $a5, $a6, '10', 0x8f0ccc92, '10');
round4_step(0, $a6, $a7, $a4, $a5, '1', 0xffeff47d, '15');
round4_step(0, $a5, $a6, $a7, $a4, '8', 0x85845dd1, '21');
round4_step(0, $a4, $a5, $a6, $a7, '15', 0x6fa87e4f, '6');
round4_step(0, $a7, $a4, $a5, $a6, '6', 0xfe2ce6e0, '10');
round4_step(0, $a6, $a7, $a4, $a5, '13', 0xa3014314, '15');
round4_step(0, $a5, $a6, $a7, $a4, '4', 0x4e0811a1, '21');
round4_step(0, $a4, $a5, $a6, $a7, '11', 0xf7537e82, '6');
round4_step(0, $a7, $a4, $a5, $a6, '2', 0xbd3af235, '10');
round4_step(0, $a6, $a7, $a4, $a5, '9', 0x2ad7d2bb, '15');
round4_step(1, $a5, $a6, $a7, $a4, '0', 0xeb86d391, '21');

$code .= <<EOF;
	# add old values of B, C
	add.w	$a5,$t4,$a5
	add.w	$a6,$t5,$a6

	bltu	$a1,$a3,.Lloop	# jmp if ptr < end

	st.w	$a4,$a0,0	# ctx->A = A
	st.w	$a5,$a0,4	# ctx->B = B
	st.w	$a6,$a0,8	# ctx->C = C
	st.w	$a7,$a0,12	# ctx->D = D

.Lend:
	jr	$ra
.size ossl_md5_block_asm_data_order,.-ossl_md5_block_asm_data_order
EOF

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT;
