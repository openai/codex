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

{
my ($in_a,$in_b,$len,$x,$temp1,$temp2) = ('a0','a1','a2','t0','t1','t2');
$code.=<<___;
################################################################################
# int CRYPTO_memcmp(const void * in_a, const void * in_b, size_t len)
################################################################################
.text
.balign 16
.globl CRYPTO_memcmp
.type   CRYPTO_memcmp,\@function
CRYPTO_memcmp:
    li      $x,0
    beqz    $len,2f   # len == 0
1:
    lbu     $temp1,0($in_a)
    lbu     $temp2,0($in_b)
    addi    $in_a,$in_a,1
    addi    $in_b,$in_b,1
    addi    $len,$len,-1
    xor     $temp1,$temp1,$temp2
    or      $x,$x,$temp1
    bgtz    $len,1b
2:
    mv      a0,$x
    ret
___
}
{
my ($ptr,$len,$temp1,$temp2) = ('a0','a1','t0','t1');
$code.=<<___;
################################################################################
# void OPENSSL_cleanse(void *ptr, size_t len)
################################################################################
.text
.balign 16
.globl OPENSSL_cleanse
.type   OPENSSL_cleanse,\@function
OPENSSL_cleanse:
    beqz    $len,2f         # len == 0, return
    srli    $temp1,$len,4
    bnez    $temp1,3f       # len > 15

1:  # Store <= 15 individual bytes
    sb      x0,0($ptr)
    addi    $ptr,$ptr,1
    addi    $len,$len,-1
    bnez    $len,1b
2:
    ret

3:  # Store individual bytes until we are aligned
    andi    $temp1,$ptr,0x7
    beqz    $temp1,4f
    sb      x0,0($ptr)
    addi    $ptr,$ptr,1
    addi    $len,$len,-1
    j       3b

4:  # Store aligned dwords
    li      $temp2,8
4:
    sd      x0,0($ptr)
    addi    $ptr,$ptr,8
    addi    $len,$len,-8
    bge     $len,$temp2,4b  # if len>=8 loop
    bnez    $len,1b         # if len<8 and len != 0, store remaining bytes
    ret
___
}

{
my ($ret) = ('a0');
$code .= <<___;
################################################################################
# size_t riscv_vlen_asm(void)
# Return VLEN (i.e. the length of a vector register in bits).
.p2align 3
.globl riscv_vlen_asm
.type riscv_vlen_asm,\@function
riscv_vlen_asm:
    csrr $ret, vlenb
    slli $ret, $ret, 3
    ret
.size riscv_vlen_asm,.-riscv_vlen_asm
___
}

print $code;
close STDOUT or die "error closing STDOUT: $!";
