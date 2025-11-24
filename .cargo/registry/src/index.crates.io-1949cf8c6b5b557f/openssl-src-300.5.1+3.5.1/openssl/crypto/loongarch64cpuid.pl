#! /usr/bin/env perl
# Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html


# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
($zero,$ra,$tp,$sp)=map("\$r$_",(0..3));
($a0,$a1,$a2,$a3,$a4,$a5,$a6,$a7)=map("\$r$_",(4..11));
($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$t9)=map("\$r$_",(12..21));
($s0,$s1,$s2,$s3,$s4,$s5,$s6,$s7)=map("\$r$_",(23..30));
($vr0,$vr1,$vr2,$vr3,$vr4,$vr5,$vr6,$vr7,$vr8,$vr9,$vr10,$vr11,$vr12,$vr13,$vr14,$vr15,$vr16,$vr17,$vr18,$vr19)=map("\$vr$_",(0..19));
($fp)=map("\$r$_",(22));

# $output is the last argument if it looks like a file (it has an extension)
my $output;
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
open STDOUT,">$output";

{
my ($in_a,$in_b,$len,$m,$temp1,$temp2) = ($a0,$a1,$a2,$t0,$t1,$t2);
$code.=<<___;
################################################################################
# int CRYPTO_memcmp(const void * in_a, const void * in_b, size_t len)
################################################################################
.text
.balign 16
.globl CRYPTO_memcmp
.type   CRYPTO_memcmp,\@function
CRYPTO_memcmp:
    li.d    $m,0
    beqz    $len,2f   # len == 0
1:
    ld.bu   $temp1,$in_a,0
    ld.bu   $temp2,$in_b,0
    addi.d  $in_a,$in_a,1
    addi.d  $in_b,$in_b,1
    addi.d  $len,$len,-1
    xor     $temp1,$temp1,$temp2
    or      $m,$m,$temp1
    blt     $zero,$len,1b
2:
    move    $a0,$m
    jr      $ra
___
}
{
my ($ptr,$len,$temp1,$temp2) = ($a0,$a1,$t0,$t1);
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
    srli.d  $temp1,$len,4
    bnez    $temp1,3f       # len > 15

1:  # Store <= 15 individual bytes
    st.b    $zero,$ptr,0
    addi.d  $ptr,$ptr,1
    addi.d  $len,$len,-1
    bnez    $len,1b
2:
    jr      $ra

3:  # Store individual bytes until we are aligned
    andi    $temp1,$ptr,0x7
    beqz    $temp1,4f
    st.b    $zero,$ptr,0
    addi.d  $ptr,$ptr,1
    addi.d  $len,$len,-1
    b       3b

4:  # Store aligned dwords
    li.d    $temp2,8
4:
    st.d    $zero,$ptr,0
    addi.d  $ptr,$ptr,8
    addi.d  $len,$len,-8
    bge     $len,$temp2,4b  # if len>=8 loop
    bnez    $len,1b         # if len<8 and len != 0, store remaining bytes
    jr      $ra
___
}
{
$code.=<<___;
################################################################################
# uint32_t OPENSSL_rdtsc(void)
################################################################################
.text
.balign 16
.globl OPENSSL_rdtsc
.type   OPENSSL_rdtsc,\@function
OPENSSL_rdtsc:
    rdtimel.w $a0,$zero
    jr        $ra
___
}

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
