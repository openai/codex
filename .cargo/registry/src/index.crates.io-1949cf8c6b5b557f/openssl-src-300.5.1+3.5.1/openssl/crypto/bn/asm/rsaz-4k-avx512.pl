# Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
# Copyright (c) 2021, Intel Corporation. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
#
# Originally written by Sergey Kirillov and Andrey Matyukov
# Intel Corporation
#
# March 2021
#
# Initial release.
#
# Implementation utilizes 256-bit (ymm) registers to avoid frequency scaling issues.
#
# IceLake-Client @ 1.3GHz
# |---------+-----------------------+---------------+-------------|
# |         | OpenSSL 3.0.0-alpha15 | this          | Unit        |
# |---------+-----------------------+---------------+-------------|
# | rsa4096 | 14 301 4300           | 5 813 953     | cycles/sign |
# |         | 90.9                  | 223.6 / +146% | sign/s      |
# |---------+-----------------------+---------------+-------------|
#

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);
$avx512ifma=0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

if (`$ENV{CC} -Wa,-v -c -o /dev/null -x assembler /dev/null 2>&1`
        =~ /GNU assembler version ([2-9]\.[0-9]+)/) {
    $avx512ifma = ($1>=2.26);
}

if (!$avx512ifma && $win64 && ($flavour =~ /nasm/ || $ENV{ASM} =~ /nasm/) &&
       `nasm -v 2>&1` =~ /NASM version ([2-9]\.[0-9]+)(?:\.([0-9]+))?/) {
    $avx512ifma = ($1==2.11 && $2>=8) + ($1>=2.12);
}

if (!$avx512ifma && `$ENV{CC} -v 2>&1`
    =~ /(Apple)?\s*((?:clang|LLVM) version|.*based on LLVM) ([0-9]+)\.([0-9]+)\.([0-9]+)?/) {
    my $ver = $3 + $4/100.0 + $5/10000.0; # 3.1.0->3.01, 3.10.1->3.1001
    if ($1) {
        # Apple conditions, they use a different version series, see
        # https://en.wikipedia.org/wiki/Xcode#Xcode_7.0_-_10.x_(since_Free_On-Device_Development)_2
        # clang 7.0.0 is Apple clang 10.0.1
        $avx512ifma = ($ver>=10.0001)
    } else {
        $avx512ifma = ($ver>=7.0);
    }
}

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

if ($avx512ifma>0) {{{
@_6_args_universal_ABI = ("%rdi","%rsi","%rdx","%rcx","%r8","%r9");

###############################################################################
# Almost Montgomery Multiplication (AMM) for 40-digit number in radix 2^52.
#
# AMM is defined as presented in the paper [1].
#
# The input and output are presented in 2^52 radix domain, i.e.
#   |res|, |a|, |b|, |m| are arrays of 40 64-bit qwords with 12 high bits zeroed.
#   |k0| is a Montgomery coefficient, which is here k0 = -1/m mod 2^64
#
# NB: the AMM implementation does not perform "conditional" subtraction step
# specified in the original algorithm as according to the Lemma 1 from the paper
# [2], the result will be always < 2*m and can be used as a direct input to
# the next AMM iteration.  This post-condition is true, provided the correct
# parameter |s| (notion of the Lemma 1 from [2]) is chosen, i.e.  s >= n + 2 * k,
# which matches our case: 2080 > 2048 + 2 * 1.
#
# [1] Gueron, S. Efficient software implementations of modular exponentiation.
#     DOI: 10.1007/s13389-012-0031-5
# [2] Gueron, S. Enhanced Montgomery Multiplication.
#     DOI: 10.1007/3-540-36400-5_5
#
# void ossl_rsaz_amm52x40_x1_ifma256(BN_ULONG *res,
#                                    const BN_ULONG *a,
#                                    const BN_ULONG *b,
#                                    const BN_ULONG *m,
#                                    BN_ULONG k0);
###############################################################################
{
# input parameters ("%rdi","%rsi","%rdx","%rcx","%r8")
my ($res,$a,$b,$m,$k0) = @_6_args_universal_ABI;

my $mask52     = "%rax";
my $acc0_0     = "%r9";
my $acc0_0_low = "%r9d";
my $acc0_1     = "%r15";
my $acc0_1_low = "%r15d";
my $b_ptr      = "%r11";

my $iter = "%ebx";

my $zero = "%ymm0";
my $Bi   = "%ymm1";
my $Yi   = "%ymm2";
my ($R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$R2_0h,$R3_0,$R3_0h,$R4_0,$R4_0h) = map("%ymm$_",(3..12));
my ($R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1,$R2_1h,$R3_1,$R3_1h,$R4_1,$R4_1h) = map("%ymm$_",(13..22));

# Registers mapping for normalization
my ($T0,$T0h,$T1,$T1h,$T2,$T2h,$T3,$T3h,$T4,$T4h) = ("$zero", "$Bi", "$Yi", map("%ymm$_", (23..29)));

sub amm52x40_x1() {
# _data_offset - offset in the |a| or |m| arrays pointing to the beginning
#                of data for corresponding AMM operation;
# _b_offset    - offset in the |b| array pointing to the next qword digit;
my ($_data_offset,$_b_offset,$_acc,$_R0,$_R0h,$_R1,$_R1h,$_R2,$_R2h,$_R3,$_R3h,$_R4,$_R4h,$_k0) = @_;
my $_R0_xmm = $_R0;
$_R0_xmm =~ s/%y/%x/;
$code.=<<___;
    movq    $_b_offset($b_ptr), %r13             # b[i]

    vpbroadcastq    %r13, $Bi                    # broadcast b[i]
    movq    $_data_offset($a), %rdx
    mulx    %r13, %r13, %r12                     # a[0]*b[i] = (t0,t2)
    addq    %r13, $_acc                          # acc += t0
    movq    %r12, %r10
    adcq    \$0, %r10                            # t2 += CF

    movq    $_k0, %r13
    imulq   $_acc, %r13                          # acc * k0
    andq    $mask52, %r13                        # yi = (acc * k0) & mask52

    vpbroadcastq    %r13, $Yi                    # broadcast y[i]
    movq    $_data_offset($m), %rdx
    mulx    %r13, %r13, %r12                     # yi * m[0] = (t0,t1)
    addq    %r13, $_acc                          # acc += t0
    adcq    %r12, %r10                           # t2 += (t1 + CF)

    shrq    \$52, $_acc
    salq    \$12, %r10
    or      %r10, $_acc                          # acc = ((acc >> 52) | (t2 << 12))

    vpmadd52luq `$_data_offset+64*0`($a), $Bi, $_R0
    vpmadd52luq `$_data_offset+64*0+32`($a), $Bi, $_R0h
    vpmadd52luq `$_data_offset+64*1`($a), $Bi, $_R1
    vpmadd52luq `$_data_offset+64*1+32`($a), $Bi, $_R1h
    vpmadd52luq `$_data_offset+64*2`($a), $Bi, $_R2
    vpmadd52luq `$_data_offset+64*2+32`($a), $Bi, $_R2h
    vpmadd52luq `$_data_offset+64*3`($a), $Bi, $_R3
    vpmadd52luq `$_data_offset+64*3+32`($a), $Bi, $_R3h
    vpmadd52luq `$_data_offset+64*4`($a), $Bi, $_R4
    vpmadd52luq `$_data_offset+64*4+32`($a), $Bi, $_R4h

    vpmadd52luq `$_data_offset+64*0`($m), $Yi, $_R0
    vpmadd52luq `$_data_offset+64*0+32`($m), $Yi, $_R0h
    vpmadd52luq `$_data_offset+64*1`($m), $Yi, $_R1
    vpmadd52luq `$_data_offset+64*1+32`($m), $Yi, $_R1h
    vpmadd52luq `$_data_offset+64*2`($m), $Yi, $_R2
    vpmadd52luq `$_data_offset+64*2+32`($m), $Yi, $_R2h
    vpmadd52luq `$_data_offset+64*3`($m), $Yi, $_R3
    vpmadd52luq `$_data_offset+64*3+32`($m), $Yi, $_R3h
    vpmadd52luq `$_data_offset+64*4`($m), $Yi, $_R4
    vpmadd52luq `$_data_offset+64*4+32`($m), $Yi, $_R4h

    # Shift accumulators right by 1 qword, zero extending the highest one
    valignq     \$1, $_R0, $_R0h, $_R0
    valignq     \$1, $_R0h, $_R1, $_R0h
    valignq     \$1, $_R1, $_R1h, $_R1
    valignq     \$1, $_R1h, $_R2, $_R1h
    valignq     \$1, $_R2, $_R2h, $_R2
    valignq     \$1, $_R2h, $_R3, $_R2h
    valignq     \$1, $_R3, $_R3h, $_R3
    valignq     \$1, $_R3h, $_R4, $_R3h
    valignq     \$1, $_R4, $_R4h, $_R4
    valignq     \$1, $_R4h, $zero, $_R4h

    vmovq   $_R0_xmm, %r13
    addq    %r13, $_acc    # acc += R0[0]

    vpmadd52huq `$_data_offset+64*0`($a), $Bi, $_R0
    vpmadd52huq `$_data_offset+64*0+32`($a), $Bi, $_R0h
    vpmadd52huq `$_data_offset+64*1`($a), $Bi, $_R1
    vpmadd52huq `$_data_offset+64*1+32`($a), $Bi, $_R1h
    vpmadd52huq `$_data_offset+64*2`($a), $Bi, $_R2
    vpmadd52huq `$_data_offset+64*2+32`($a), $Bi, $_R2h
    vpmadd52huq `$_data_offset+64*3`($a), $Bi, $_R3
    vpmadd52huq `$_data_offset+64*3+32`($a), $Bi, $_R3h
    vpmadd52huq `$_data_offset+64*4`($a), $Bi, $_R4
    vpmadd52huq `$_data_offset+64*4+32`($a), $Bi, $_R4h

    vpmadd52huq `$_data_offset+64*0`($m), $Yi, $_R0
    vpmadd52huq `$_data_offset+64*0+32`($m), $Yi, $_R0h
    vpmadd52huq `$_data_offset+64*1`($m), $Yi, $_R1
    vpmadd52huq `$_data_offset+64*1+32`($m), $Yi, $_R1h
    vpmadd52huq `$_data_offset+64*2`($m), $Yi, $_R2
    vpmadd52huq `$_data_offset+64*2+32`($m), $Yi, $_R2h
    vpmadd52huq `$_data_offset+64*3`($m), $Yi, $_R3
    vpmadd52huq `$_data_offset+64*3+32`($m), $Yi, $_R3h
    vpmadd52huq `$_data_offset+64*4`($m), $Yi, $_R4
    vpmadd52huq `$_data_offset+64*4+32`($m), $Yi, $_R4h
___
}

# Normalization routine: handles carry bits and gets bignum qwords to normalized
# 2^52 representation.
#
# Uses %r8-14,%e[abcd]x
sub amm52x40_x1_norm {
my ($_acc,$_R0,$_R0h,$_R1,$_R1h,$_R2,$_R2h,$_R3,$_R3h,$_R4,$_R4h) = @_;
$code.=<<___;
    # Put accumulator to low qword in R0
    vpbroadcastq    $_acc, $T0
    vpblendd \$3, $T0, $_R0, $_R0

    # Extract "carries" (12 high bits) from each QW of the bignum
    # Save them to LSB of QWs in T0..Tn
    vpsrlq    \$52, $_R0,   $T0
    vpsrlq    \$52, $_R0h,  $T0h
    vpsrlq    \$52, $_R1,   $T1
    vpsrlq    \$52, $_R1h,  $T1h
    vpsrlq    \$52, $_R2,   $T2
    vpsrlq    \$52, $_R2h,  $T2h
    vpsrlq    \$52, $_R3,   $T3
    vpsrlq    \$52, $_R3h,  $T3h
    vpsrlq    \$52, $_R4,   $T4
    vpsrlq    \$52, $_R4h,  $T4h

    # "Shift left" T0..Tn by 1 QW
    valignq \$3, $T4,  $T4h,  $T4h
    valignq \$3, $T3h,  $T4,  $T4
    valignq \$3, $T3,  $T3h,  $T3h
    valignq \$3, $T2h,  $T3,  $T3
    valignq \$3, $T2,  $T2h,  $T2h
    valignq \$3, $T1h,  $T2,  $T2
    valignq \$3, $T1,   $T1h, $T1h
    valignq \$3, $T0h,  $T1,  $T1
    valignq \$3, $T0,   $T0h, $T0h
    valignq \$3, .Lzeros(%rip), $T0,  $T0

    # Drop "carries" from R0..Rn QWs
    vpandq    .Lmask52x4(%rip), $_R0,  $_R0
    vpandq    .Lmask52x4(%rip), $_R0h, $_R0h
    vpandq    .Lmask52x4(%rip), $_R1,  $_R1
    vpandq    .Lmask52x4(%rip), $_R1h, $_R1h
    vpandq    .Lmask52x4(%rip), $_R2,  $_R2
    vpandq    .Lmask52x4(%rip), $_R2h, $_R2h
    vpandq    .Lmask52x4(%rip), $_R3,  $_R3
    vpandq    .Lmask52x4(%rip), $_R3h, $_R3h
    vpandq    .Lmask52x4(%rip), $_R4,  $_R4
    vpandq    .Lmask52x4(%rip), $_R4h, $_R4h

    # Sum R0..Rn with corresponding adjusted carries
    vpaddq  $T0,  $_R0,  $_R0
    vpaddq  $T0h, $_R0h, $_R0h
    vpaddq  $T1,  $_R1,  $_R1
    vpaddq  $T1h, $_R1h, $_R1h
    vpaddq  $T2,  $_R2,  $_R2
    vpaddq  $T2h, $_R2h, $_R2h
    vpaddq  $T3,  $_R3,  $_R3
    vpaddq  $T3h, $_R3h, $_R3h
    vpaddq  $T4,  $_R4,  $_R4
    vpaddq  $T4h, $_R4h, $_R4h

    # Now handle carry bits from this addition
    # Get mask of QWs whose 52-bit parts overflow
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R0},%k1    # OP=nle (i.e. gt)
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R0h},%k2
    kmovb      %k1,%r14d
    kmovb      %k2,%r13d
    shl        \$4,%r13b
    or         %r13b,%r14b

    vpcmpuq    \$6,.Lmask52x4(%rip),${_R1},%k1
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R1h},%k2
    kmovb      %k1,%r13d
    kmovb      %k2,%r12d
    shl        \$4,%r12b
    or         %r12b,%r13b

    vpcmpuq    \$6,.Lmask52x4(%rip),${_R2},%k1
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R2h},%k2
    kmovb      %k1,%r12d
    kmovb      %k2,%r11d
    shl        \$4,%r11b
    or         %r11b,%r12b

    vpcmpuq    \$6,.Lmask52x4(%rip),${_R3},%k1
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R3h},%k2
    kmovb      %k1,%r11d
    kmovb      %k2,%r10d
    shl        \$4,%r10b
    or         %r10b,%r11b

    vpcmpuq    \$6,.Lmask52x4(%rip),${_R4},%k1
    vpcmpuq    \$6,.Lmask52x4(%rip),${_R4h},%k2
    kmovb      %k1,%r10d
    kmovb      %k2,%r9d
    shl        \$4,%r9b
    or         %r9b,%r10b

    addb       %r14b,%r14b
    adcb       %r13b,%r13b
    adcb       %r12b,%r12b
    adcb       %r11b,%r11b
    adcb       %r10b,%r10b

    # Get mask of QWs whose 52-bit parts saturated
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R0},%k1    # OP=eq
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R0h},%k2
    kmovb      %k1,%r9d
    kmovb      %k2,%r8d
    shl        \$4,%r8b
    or         %r8b,%r9b

    vpcmpuq    \$0,.Lmask52x4(%rip),${_R1},%k1
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R1h},%k2
    kmovb      %k1,%r8d
    kmovb      %k2,%edx
    shl        \$4,%dl
    or         %dl,%r8b

    vpcmpuq    \$0,.Lmask52x4(%rip),${_R2},%k1
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R2h},%k2
    kmovb      %k1,%edx
    kmovb      %k2,%ecx
    shl        \$4,%cl
    or         %cl,%dl

    vpcmpuq    \$0,.Lmask52x4(%rip),${_R3},%k1
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R3h},%k2
    kmovb      %k1,%ecx
    kmovb      %k2,%ebx
    shl        \$4,%bl
    or         %bl,%cl

    vpcmpuq    \$0,.Lmask52x4(%rip),${_R4},%k1
    vpcmpuq    \$0,.Lmask52x4(%rip),${_R4h},%k2
    kmovb      %k1,%ebx
    kmovb      %k2,%eax
    shl        \$4,%al
    or         %al,%bl

    addb     %r9b,%r14b
    adcb     %r8b,%r13b
    adcb     %dl,%r12b
    adcb     %cl,%r11b
    adcb     %bl,%r10b

    xor      %r9b,%r14b
    xor      %r8b,%r13b
    xor      %dl,%r12b
    xor      %cl,%r11b
    xor      %bl,%r10b

    kmovb    %r14d,%k1
    shr      \$4,%r14b
    kmovb    %r14d,%k2
    kmovb    %r13d,%k3
    shr      \$4,%r13b
    kmovb    %r13d,%k4
    kmovb    %r12d,%k5
    shr      \$4,%r12b
    kmovb    %r12d,%k6
    kmovb    %r11d,%k7

    vpsubq  .Lmask52x4(%rip), $_R0,  ${_R0}{%k1}
    vpsubq  .Lmask52x4(%rip), $_R0h, ${_R0h}{%k2}
    vpsubq  .Lmask52x4(%rip), $_R1,  ${_R1}{%k3}
    vpsubq  .Lmask52x4(%rip), $_R1h, ${_R1h}{%k4}
    vpsubq  .Lmask52x4(%rip), $_R2,  ${_R2}{%k5}
    vpsubq  .Lmask52x4(%rip), $_R2h, ${_R2h}{%k6}
    vpsubq  .Lmask52x4(%rip), $_R3,  ${_R3}{%k7}

    vpandq  .Lmask52x4(%rip), $_R0,  $_R0
    vpandq  .Lmask52x4(%rip), $_R0h, $_R0h
    vpandq  .Lmask52x4(%rip), $_R1,  $_R1
    vpandq  .Lmask52x4(%rip), $_R1h, $_R1h
    vpandq  .Lmask52x4(%rip), $_R2,  $_R2
    vpandq  .Lmask52x4(%rip), $_R2h, $_R2h
    vpandq  .Lmask52x4(%rip), $_R3,  $_R3

    shr    \$4,%r11b
    kmovb   %r11d,%k1
    kmovb   %r10d,%k2
    shr    \$4,%r10b
    kmovb   %r10d,%k3

    vpsubq  .Lmask52x4(%rip), $_R3h, ${_R3h}{%k1}
    vpsubq  .Lmask52x4(%rip), $_R4,  ${_R4}{%k2}
    vpsubq  .Lmask52x4(%rip), $_R4h, ${_R4h}{%k3}

    vpandq  .Lmask52x4(%rip), $_R3h, $_R3h
    vpandq  .Lmask52x4(%rip), $_R4,  $_R4
    vpandq  .Lmask52x4(%rip), $_R4h, $_R4h
___
}

$code.=<<___;
.text

.globl  ossl_rsaz_amm52x40_x1_ifma256
.type   ossl_rsaz_amm52x40_x1_ifma256,\@function,5
.align 32
ossl_rsaz_amm52x40_x1_ifma256:
.cfi_startproc
    endbranch
    push    %rbx
.cfi_push   %rbx
    push    %rbp
.cfi_push   %rbp
    push    %r12
.cfi_push   %r12
    push    %r13
.cfi_push   %r13
    push    %r14
.cfi_push   %r14
    push    %r15
.cfi_push   %r15
___
$code.=<<___ if ($win64);
    lea     -168(%rsp),%rsp                 # 16*10 + (8 bytes to get correct 16-byte SIMD alignment)
    vmovdqa64   %xmm6, `0*16`(%rsp)         # save non-volatile registers
    vmovdqa64   %xmm7, `1*16`(%rsp)
    vmovdqa64   %xmm8, `2*16`(%rsp)
    vmovdqa64   %xmm9, `3*16`(%rsp)
    vmovdqa64   %xmm10,`4*16`(%rsp)
    vmovdqa64   %xmm11,`5*16`(%rsp)
    vmovdqa64   %xmm12,`6*16`(%rsp)
    vmovdqa64   %xmm13,`7*16`(%rsp)
    vmovdqa64   %xmm14,`8*16`(%rsp)
    vmovdqa64   %xmm15,`9*16`(%rsp)
.Lossl_rsaz_amm52x40_x1_ifma256_body:
___
$code.=<<___;
    # Zeroing accumulators
    vpxord   $zero, $zero, $zero
    vmovdqa64   $zero, $R0_0
    vmovdqa64   $zero, $R0_0h
    vmovdqa64   $zero, $R1_0
    vmovdqa64   $zero, $R1_0h
    vmovdqa64   $zero, $R2_0
    vmovdqa64   $zero, $R2_0h
    vmovdqa64   $zero, $R3_0
    vmovdqa64   $zero, $R3_0h
    vmovdqa64   $zero, $R4_0
    vmovdqa64   $zero, $R4_0h

    xorl    $acc0_0_low, $acc0_0_low

    movq    $b, $b_ptr                       # backup address of b
    movq    \$0xfffffffffffff, $mask52       # 52-bit mask

    # Loop over 40 digits unrolled by 4
    mov     \$10, $iter

.align 32
.Lloop10:
___
    foreach my $idx (0..3) {
        &amm52x40_x1(0,8*$idx,$acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$R2_0h,$R3_0,$R3_0h,$R4_0,$R4_0h,$k0);
    }
$code.=<<___;
    lea    `4*8`($b_ptr), $b_ptr
    dec    $iter
    jne    .Lloop10
___
    &amm52x40_x1_norm($acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$R2_0h,$R3_0,$R3_0h,$R4_0,$R4_0h);
$code.=<<___;

    vmovdqu64   $R0_0,  `0*32`($res)
    vmovdqu64   $R0_0h, `1*32`($res)
    vmovdqu64   $R1_0,  `2*32`($res)
    vmovdqu64   $R1_0h, `3*32`($res)
    vmovdqu64   $R2_0,  `4*32`($res)
    vmovdqu64   $R2_0h, `5*32`($res)
    vmovdqu64   $R3_0,  `6*32`($res)
    vmovdqu64   $R3_0h, `7*32`($res)
    vmovdqu64   $R4_0,  `8*32`($res)
    vmovdqu64   $R4_0h, `9*32`($res)

    vzeroupper
    lea     (%rsp),%rax
.cfi_def_cfa_register   %rax
___
$code.=<<___ if ($win64);
    vmovdqa64   `0*16`(%rax),%xmm6
    vmovdqa64   `1*16`(%rax),%xmm7
    vmovdqa64   `2*16`(%rax),%xmm8
    vmovdqa64   `3*16`(%rax),%xmm9
    vmovdqa64   `4*16`(%rax),%xmm10
    vmovdqa64   `5*16`(%rax),%xmm11
    vmovdqa64   `6*16`(%rax),%xmm12
    vmovdqa64   `7*16`(%rax),%xmm13
    vmovdqa64   `8*16`(%rax),%xmm14
    vmovdqa64   `9*16`(%rax),%xmm15
    lea  168(%rsp),%rax
___
$code.=<<___;
    mov  0(%rax),%r15
.cfi_restore    %r15
    mov  8(%rax),%r14
.cfi_restore    %r14
    mov  16(%rax),%r13
.cfi_restore    %r13
    mov  24(%rax),%r12
.cfi_restore    %r12
    mov  32(%rax),%rbp
.cfi_restore    %rbp
    mov  40(%rax),%rbx
.cfi_restore    %rbx
    lea  48(%rax),%rsp       # restore rsp
.cfi_def_cfa %rsp,8
.Lossl_rsaz_amm52x40_x1_ifma256_epilogue:

    ret
.cfi_endproc
.size   ossl_rsaz_amm52x40_x1_ifma256, .-ossl_rsaz_amm52x40_x1_ifma256
___

$code.=<<___;
.section .rodata align=32
.align 32
.Lmask52x4:
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
___

###############################################################################
# Dual Almost Montgomery Multiplication for 40-digit number in radix 2^52
#
# See description of ossl_rsaz_amm52x40_x1_ifma256() above for details about Almost
# Montgomery Multiplication algorithm and function input parameters description.
#
# This function does two AMMs for two independent inputs, hence dual.
#
# void ossl_rsaz_amm52x40_x2_ifma256(BN_ULONG out[2][40],
#                                    const BN_ULONG a[2][40],
#                                    const BN_ULONG b[2][40],
#                                    const BN_ULONG m[2][40],
#                                    const BN_ULONG k0[2]);
###############################################################################

$code.=<<___;
.text

.globl  ossl_rsaz_amm52x40_x2_ifma256
.type   ossl_rsaz_amm52x40_x2_ifma256,\@function,5
.align 32
ossl_rsaz_amm52x40_x2_ifma256:
.cfi_startproc
    endbranch
    push    %rbx
.cfi_push   %rbx
    push    %rbp
.cfi_push   %rbp
    push    %r12
.cfi_push   %r12
    push    %r13
.cfi_push   %r13
    push    %r14
.cfi_push   %r14
    push    %r15
.cfi_push   %r15
___
$code.=<<___ if ($win64);
    lea     -168(%rsp),%rsp
    vmovdqa64   %xmm6, `0*16`(%rsp)        # save non-volatile registers
    vmovdqa64   %xmm7, `1*16`(%rsp)
    vmovdqa64   %xmm8, `2*16`(%rsp)
    vmovdqa64   %xmm9, `3*16`(%rsp)
    vmovdqa64   %xmm10,`4*16`(%rsp)
    vmovdqa64   %xmm11,`5*16`(%rsp)
    vmovdqa64   %xmm12,`6*16`(%rsp)
    vmovdqa64   %xmm13,`7*16`(%rsp)
    vmovdqa64   %xmm14,`8*16`(%rsp)
    vmovdqa64   %xmm15,`9*16`(%rsp)
.Lossl_rsaz_amm52x40_x2_ifma256_body:
___
$code.=<<___;
    # Zeroing accumulators
    vpxord   $zero, $zero, $zero
    vmovdqa64   $zero, $R0_0
    vmovdqa64   $zero, $R0_0h
    vmovdqa64   $zero, $R1_0
    vmovdqa64   $zero, $R1_0h
    vmovdqa64   $zero, $R2_0
    vmovdqa64   $zero, $R2_0h
    vmovdqa64   $zero, $R3_0
    vmovdqa64   $zero, $R3_0h
    vmovdqa64   $zero, $R4_0
    vmovdqa64   $zero, $R4_0h

    vmovdqa64   $zero, $R0_1
    vmovdqa64   $zero, $R0_1h
    vmovdqa64   $zero, $R1_1
    vmovdqa64   $zero, $R1_1h
    vmovdqa64   $zero, $R2_1
    vmovdqa64   $zero, $R2_1h
    vmovdqa64   $zero, $R3_1
    vmovdqa64   $zero, $R3_1h
    vmovdqa64   $zero, $R4_1
    vmovdqa64   $zero, $R4_1h


    xorl    $acc0_0_low, $acc0_0_low
    xorl    $acc0_1_low, $acc0_1_low

    movq    $b, $b_ptr                       # backup address of b
    movq    \$0xfffffffffffff, $mask52       # 52-bit mask

    mov    \$40, $iter

.align 32
.Lloop40:
___
    &amm52x40_x1(   0,   0,$acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$R2_0h,$R3_0,$R3_0h,$R4_0,$R4_0h,"($k0)");
    # 40*8 = offset of the next dimension in two-dimension array
    &amm52x40_x1(40*8,40*8,$acc0_1,$R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1,$R2_1h,$R3_1,$R3_1h,$R4_1,$R4_1h,"8($k0)");
$code.=<<___;
    lea    8($b_ptr), $b_ptr
    dec    $iter
    jne    .Lloop40
___
    &amm52x40_x1_norm($acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$R2_0h,$R3_0,$R3_0h,$R4_0,$R4_0h);
    &amm52x40_x1_norm($acc0_1,$R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1,$R2_1h,$R3_1,$R3_1h,$R4_1,$R4_1h);
$code.=<<___;

    vmovdqu64   $R0_0,  `0*32`($res)
    vmovdqu64   $R0_0h, `1*32`($res)
    vmovdqu64   $R1_0,  `2*32`($res)
    vmovdqu64   $R1_0h, `3*32`($res)
    vmovdqu64   $R2_0,  `4*32`($res)
    vmovdqu64   $R2_0h, `5*32`($res)
    vmovdqu64   $R3_0,  `6*32`($res)
    vmovdqu64   $R3_0h, `7*32`($res)
    vmovdqu64   $R4_0,  `8*32`($res)
    vmovdqu64   $R4_0h, `9*32`($res)

    vmovdqu64   $R0_1,  `10*32`($res)
    vmovdqu64   $R0_1h, `11*32`($res)
    vmovdqu64   $R1_1,  `12*32`($res)
    vmovdqu64   $R1_1h, `13*32`($res)
    vmovdqu64   $R2_1,  `14*32`($res)
    vmovdqu64   $R2_1h, `15*32`($res)
    vmovdqu64   $R3_1,  `16*32`($res)
    vmovdqu64   $R3_1h, `17*32`($res)
    vmovdqu64   $R4_1,  `18*32`($res)
    vmovdqu64   $R4_1h, `19*32`($res)

    vzeroupper
    lea     (%rsp),%rax
.cfi_def_cfa_register   %rax
___
$code.=<<___ if ($win64);
    vmovdqa64   `0*16`(%rax),%xmm6
    vmovdqa64   `1*16`(%rax),%xmm7
    vmovdqa64   `2*16`(%rax),%xmm8
    vmovdqa64   `3*16`(%rax),%xmm9
    vmovdqa64   `4*16`(%rax),%xmm10
    vmovdqa64   `5*16`(%rax),%xmm11
    vmovdqa64   `6*16`(%rax),%xmm12
    vmovdqa64   `7*16`(%rax),%xmm13
    vmovdqa64   `8*16`(%rax),%xmm14
    vmovdqa64   `9*16`(%rax),%xmm15
    lea     168(%rsp),%rax
___
$code.=<<___;
    mov  0(%rax),%r15
.cfi_restore    %r15
    mov  8(%rax),%r14
.cfi_restore    %r14
    mov  16(%rax),%r13
.cfi_restore    %r13
    mov  24(%rax),%r12
.cfi_restore    %r12
    mov  32(%rax),%rbp
.cfi_restore    %rbp
    mov  40(%rax),%rbx
.cfi_restore    %rbx
    lea  48(%rax),%rsp
.cfi_def_cfa    %rsp,8
.Lossl_rsaz_amm52x40_x2_ifma256_epilogue:
    ret
.cfi_endproc
.size   ossl_rsaz_amm52x40_x2_ifma256, .-ossl_rsaz_amm52x40_x2_ifma256
___
}

###############################################################################
# Constant time extraction from the precomputed table of powers base^i, where
#    i = 0..2^EXP_WIN_SIZE-1
#
# The input |red_table| contains precomputations for two independent base values.
# |red_table_idx1| and |red_table_idx2| are corresponding power indexes.
#
# Extracted value (output) is 2 40 digits numbers in 2^52 radix.
#
# void ossl_extract_multiplier_2x40_win5(BN_ULONG *red_Y,
#                                        const BN_ULONG red_table[1 << EXP_WIN_SIZE][2][40],
#                                        int red_table_idx1, int red_table_idx2);
#
# EXP_WIN_SIZE = 5
###############################################################################
{
# input parameters
my ($out,$red_tbl,$red_tbl_idx1,$red_tbl_idx2)=$win64 ? ("%rcx","%rdx","%r8", "%r9") :  # Win64 order
                                                        ("%rdi","%rsi","%rdx","%rcx");  # Unix order

my ($t0,$t1,$t2,$t3,$t4,$t5) = map("%ymm$_", (0..5));
my ($t6,$t7,$t8,$t9) = map("%ymm$_", (16..19));
my ($tmp,$cur_idx,$idx1,$idx2,$ones) = map("%ymm$_", (20..24));

my @t = ($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$t9);
my $t0xmm = $t0;
$t0xmm =~ s/%y/%x/;

sub get_table_value_consttime() {
my ($_idx,$_offset) = @_;
$code.=<<___;
    vpxorq   $cur_idx, $cur_idx, $cur_idx
.align 32
.Lloop_$_offset:
    vpcmpq  \$0, $cur_idx, $_idx, %k1      # mask of (idx == cur_idx)
___
foreach (0..9) {
$code.=<<___;
    vmovdqu64  `$_offset+${_}*32`($red_tbl), $tmp   # load data from red_tbl
    vpblendmq  $tmp, $t[$_], ${t[$_]}{%k1}          # extract data when mask is not zero
___
}
$code.=<<___;
    vpaddq  $ones, $cur_idx, $cur_idx # increment cur_idx
    addq    \$`2*40*8`, $red_tbl
    cmpq    $red_tbl, %rax
    jne .Lloop_$_offset
___
}

$code.=<<___;
.text

.align 32
.globl  ossl_extract_multiplier_2x40_win5
.type   ossl_extract_multiplier_2x40_win5,\@abi-omnipotent
ossl_extract_multiplier_2x40_win5:
.cfi_startproc
    endbranch
    vmovdqa64   .Lones(%rip), $ones         # broadcast ones
    vpbroadcastq    $red_tbl_idx1, $idx1
    vpbroadcastq    $red_tbl_idx2, $idx2
    leaq   `(1<<5)*2*40*8`($red_tbl), %rax  # holds end of the tbl

    # backup red_tbl address
    movq    $red_tbl, %r10

    # zeroing t0..n, cur_idx
    vpxor   $t0xmm, $t0xmm, $t0xmm
___
foreach (1..9) {
    $code.="vmovdqa64   $t0, $t[$_] \n";
}

&get_table_value_consttime($idx1, 0);
foreach (0..9) {
    $code.="vmovdqu64   $t[$_], `(0+$_)*32`($out) \n";
}
$code.="movq    %r10, $red_tbl \n";
&get_table_value_consttime($idx2, 40*8);
foreach (0..9) {
    $code.="vmovdqu64   $t[$_], `(10+$_)*32`($out) \n";
}
$code.=<<___;

    ret
.cfi_endproc
.size   ossl_extract_multiplier_2x40_win5, .-ossl_extract_multiplier_2x40_win5
___
$code.=<<___;
.section .rodata align=32
.align 32
.Lones:
    .quad   1,1,1,1
.Lzeros:
    .quad   0,0,0,0
___
}

if ($win64) {
$rec="%rcx";
$frame="%rdx";
$context="%r8";
$disp="%r9";

$code.=<<___;
.extern     __imp_RtlVirtualUnwind
.type   rsaz_avx_handler,\@abi-omnipotent
.align  16
rsaz_avx_handler:
    push    %rsi
    push    %rdi
    push    %rbx
    push    %rbp
    push    %r12
    push    %r13
    push    %r14
    push    %r15
    pushfq
    sub     \$64,%rsp

    mov     120($context),%rax # pull context->Rax
    mov     248($context),%rbx # pull context->Rip

    mov     8($disp),%rsi      # disp->ImageBase
    mov     56($disp),%r11     # disp->HandlerData

    mov     0(%r11),%r10d      # HandlerData[0]
    lea     (%rsi,%r10),%r10   # prologue label
    cmp     %r10,%rbx          # context->Rip<.Lprologue
    jb  .Lcommon_seh_tail

    mov     4(%r11),%r10d      # HandlerData[1]
    lea     (%rsi,%r10),%r10   # epilogue label
    cmp     %r10,%rbx          # context->Rip>=.Lepilogue
    jae     .Lcommon_seh_tail

    mov     152($context),%rax # pull context->Rsp

    lea     (%rax),%rsi         # %xmm save area
    lea     512($context),%rdi  # & context.Xmm6
    mov     \$20,%ecx           # 10*sizeof(%xmm0)/sizeof(%rax)
    .long   0xa548f3fc          # cld; rep movsq

    lea     `48+168`(%rax),%rax

    mov     -8(%rax),%rbx
    mov     -16(%rax),%rbp
    mov     -24(%rax),%r12
    mov     -32(%rax),%r13
    mov     -40(%rax),%r14
    mov     -48(%rax),%r15
    mov     %rbx,144($context) # restore context->Rbx
    mov     %rbp,160($context) # restore context->Rbp
    mov     %r12,216($context) # restore context->R12
    mov     %r13,224($context) # restore context->R13
    mov     %r14,232($context) # restore context->R14
    mov     %r15,240($context) # restore context->R14

.Lcommon_seh_tail:
    mov     8(%rax),%rdi
    mov     16(%rax),%rsi
    mov     %rax,152($context) # restore context->Rsp
    mov     %rsi,168($context) # restore context->Rsi
    mov     %rdi,176($context) # restore context->Rdi

    mov     40($disp),%rdi     # disp->ContextRecord
    mov     $context,%rsi      # context
    mov     \$154,%ecx         # sizeof(CONTEXT)
    .long   0xa548f3fc         # cld; rep movsq

    mov     $disp,%rsi
    xor     %rcx,%rcx          # arg1, UNW_FLAG_NHANDLER
    mov     8(%rsi),%rdx       # arg2, disp->ImageBase
    mov     0(%rsi),%r8        # arg3, disp->ControlPc
    mov     16(%rsi),%r9       # arg4, disp->FunctionEntry
    mov     40(%rsi),%r10      # disp->ContextRecord
    lea     56(%rsi),%r11      # &disp->HandlerData
    lea     24(%rsi),%r12      # &disp->EstablisherFrame
    mov     %r10,32(%rsp)      # arg5
    mov     %r11,40(%rsp)      # arg6
    mov     %r12,48(%rsp)      # arg7
    mov     %rcx,56(%rsp)      # arg8, (NULL)
    call    *__imp_RtlVirtualUnwind(%rip)

    mov     \$1,%eax           # ExceptionContinueSearch
    add     \$64,%rsp
    popfq
    pop     %r15
    pop     %r14
    pop     %r13
    pop     %r12
    pop     %rbp
    pop     %rbx
    pop     %rdi
    pop     %rsi
    ret
.size   rsaz_avx_handler,.-rsaz_avx_handler

.section    .pdata
.align  4
    .rva    .LSEH_begin_ossl_rsaz_amm52x40_x1_ifma256
    .rva    .LSEH_end_ossl_rsaz_amm52x40_x1_ifma256
    .rva    .LSEH_info_ossl_rsaz_amm52x40_x1_ifma256

    .rva    .LSEH_begin_ossl_rsaz_amm52x40_x2_ifma256
    .rva    .LSEH_end_ossl_rsaz_amm52x40_x2_ifma256
    .rva    .LSEH_info_ossl_rsaz_amm52x40_x2_ifma256

.section    .xdata
.align  8
.LSEH_info_ossl_rsaz_amm52x40_x1_ifma256:
    .byte   9,0,0,0
    .rva    rsaz_avx_handler
    .rva    .Lossl_rsaz_amm52x40_x1_ifma256_body,.Lossl_rsaz_amm52x40_x1_ifma256_epilogue
.LSEH_info_ossl_rsaz_amm52x40_x2_ifma256:
    .byte   9,0,0,0
    .rva    rsaz_avx_handler
    .rva    .Lossl_rsaz_amm52x40_x2_ifma256_body,.Lossl_rsaz_amm52x40_x2_ifma256_epilogue
___
}
}}} else {{{                # fallback for old assembler
$code.=<<___;
.text

.globl  ossl_rsaz_amm52x40_x1_ifma256
.globl  ossl_rsaz_amm52x40_x2_ifma256
.globl  ossl_extract_multiplier_2x40_win5
.type   ossl_rsaz_amm52x40_x1_ifma256,\@abi-omnipotent
ossl_rsaz_amm52x40_x1_ifma256:
ossl_rsaz_amm52x40_x2_ifma256:
ossl_extract_multiplier_2x40_win5:
    .byte   0x0f,0x0b    # ud2
    ret
.size   ossl_rsaz_amm52x40_x1_ifma256, .-ossl_rsaz_amm52x40_x1_ifma256
___
}}}

$code =~ s/\`([^\`]*)\`/eval $1/gem;
print $code;
close STDOUT or die "error closing STDOUT: $!";
