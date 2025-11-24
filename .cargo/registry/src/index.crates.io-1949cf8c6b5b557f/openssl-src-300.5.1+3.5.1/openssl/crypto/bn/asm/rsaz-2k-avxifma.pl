# Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
# Copyright (c) 2024, Intel Corporation. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# Written by Zhiguo Zhou <zhiguo.zhou@intel.com> and Wangyang Guo <wangyang.guo@intel.com>.
# Special thanks to Tomasz Kantecki <tomasz.kantecki@intel.com> for his valuable suggestions.
#
# October 2024
#
# Initial release.
#

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);
$avxifma=0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

if (`$ENV{CC} -Wa,-v -c -o /dev/null -x assembler /dev/null 2>&1`
        =~ /GNU assembler version ([2-9]\.[0-9]+)/) {
    $avxifma = ($1>=2.40);
}

if (!$avxifma && `$ENV{CC} -v 2>&1`
    =~ /\s*((?:clang|LLVM) version|.*based on LLVM) ([0-9]+)\.([0-9]+)\.([0-9]+)?/) {
    my $ver = $2 + $3/100.0 + $4/10000.0; # 3.1.0->3.01, 3.10.1->3.1001
    $avxifma = ($ver>=16.0);
}

if ($win64 && ($flavour =~ /nasm/ || $ENV{ASM} =~ /nasm/) &&
       `nasm -v 2>&1` =~ /NASM version ([2-9]\.[0-9]+)(?:\.([0-9]+))?(rc[0-9]+)?/) {
    $avxifma = ($1>2.16) + ($1==2.16 && ((!defined($2) && !defined($3)) || (defined($2))));
}

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

if ($avxifma>0) {{{
@_6_args_universal_ABI = ("%rdi","%rsi","%rdx","%rcx","%r8","%r9");

$code.=<<___;
.text
.extern OPENSSL_ia32cap_P
.globl  ossl_rsaz_avxifma_eligible
.type   ossl_rsaz_avxifma_eligible,\@abi-omnipotent
.align  32
ossl_rsaz_avxifma_eligible:
    mov OPENSSL_ia32cap_P+20(%rip), %ecx
    xor %eax,%eax
    and \$`1<<23`, %ecx     # avxifma
    cmp \$`1<<23`, %ecx
    cmove %ecx,%eax
    ret
.size   ossl_rsaz_avxifma_eligible, .-ossl_rsaz_avxifma_eligible
___

###############################################################################
# Almost Montgomery Multiplication (AMM) for 20-digit number in radix 2^52.
#
# AMM is defined as presented in the paper [1].
#
# The input and output are presented in 2^52 radix domain, i.e.
#   |res|, |a|, |b|, |m| are arrays of 20 64-bit qwords with 12 high bits zeroed.
#   |k0| is a Montgomery coefficient, which is here k0 = -1/m mod 2^64
#
# NB: the AMM implementation does not perform "conditional" subtraction step
# specified in the original algorithm as according to the Lemma 1 from the paper
# [2], the result will be always < 2*m and can be used as a direct input to
# the next AMM iteration.  This post-condition is true, provided the correct
# parameter |s| (notion of the Lemma 1 from [2]) is chosen, i.e.  s >= n + 2 * k,
# which matches our case: 1040 > 1024 + 2 * 1.
#
# [1] Gueron, S. Efficient software implementations of modular exponentiation.
#     DOI: 10.1007/s13389-012-0031-5
# [2] Gueron, S. Enhanced Montgomery Multiplication.
#     DOI: 10.1007/3-540-36400-5_5
#
# void ossl_rsaz_amm52x20_x1_avxifma256(BN_ULONG *res,
#                                       const BN_ULONG *a,
#                                       const BN_ULONG *b,
#                                       const BN_ULONG *m,
#                                       BN_ULONG k0);
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
my $Yi_xmm   = "%xmm2";
my ($R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0) = ("%ymm3",map("%ymm$_",(5..8)));
my ($R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1) = ("%ymm4",map("%ymm$_",(9..12)));

# Registers mapping for normalization.
my ($T0,$T0h,$T1,$T1h,$T2,$tmp) = ("$zero", "$Bi", "$Yi", map("%ymm$_", (13..15)));
my $T0_xmm   = "%xmm0";

sub amm52x20_x1() {
# _data_offset - offset in the |a| or |m| arrays pointing to the beginning
#                of data for corresponding AMM operation;
# _b_offset    - offset in the |b| array pointing to the next qword digit;
my ($_data_offset,$_b_offset,$_acc,$_R0,$_R0h,$_R1,$_R1h,$_R2,$_k0) = @_;
$code.=<<___;
    movq    $_b_offset($b_ptr), %r13             # b[i]

    vpbroadcastq    $_b_offset($b_ptr), $Bi      # broadcast b[i]
    movq    $_data_offset($a), %rdx
    mulx    %r13, %r13, %r12                     # a[0]*b[i] = (t0,t2)
    addq    %r13, $_acc                          # acc += t0
    movq    %r12, %r10
    adcq    \$0, %r10                            # t2 += CF

    movq    $_k0, %r13
    imulq   $_acc, %r13                          # acc * k0
    andq    $mask52, %r13                        # yi = (acc * k0) & mask52

    vmovq   %r13, $Yi_xmm
    vpbroadcastq    $Yi_xmm, $Yi                 # broadcast y[i]
    movq    $_data_offset($m), %rdx
    mulx    %r13, %r13, %r12                     # yi * m[0] = (t0,t1)
    addq    %r13, $_acc                          # acc += t0
    adcq    %r12, %r10                           # t2 += (t1 + CF)

    shrq    \$52, $_acc
    salq    \$12, %r10
    or      %r10, $_acc                          # acc = ((acc >> 52) | (t2 << 12))

    lea -168(%rsp), %rsp
    {vex} vpmadd52luq `$_data_offset`($a), $Bi, $_R0
    {vex} vpmadd52luq `$_data_offset+32`($a), $Bi, $_R0h
    {vex} vpmadd52luq `$_data_offset+64*1`($a), $Bi, $_R1
    {vex} vpmadd52luq `$_data_offset+64*1+32`($a), $Bi, $_R1h
    {vex} vpmadd52luq `$_data_offset+64*2`($a), $Bi, $_R2

    {vex} vpmadd52luq `$_data_offset`($m), $Yi, $_R0
    {vex} vpmadd52luq `$_data_offset+32`($m), $Yi, $_R0h
    {vex} vpmadd52luq `$_data_offset+64*1`($m), $Yi, $_R1
    {vex} vpmadd52luq `$_data_offset+64*1+32`($m), $Yi, $_R1h
    {vex} vpmadd52luq `$_data_offset+64*2`($m), $Yi, $_R2

    # Shift accumulators right by 1 qword, zero extending the highest one
    vmovdqu $_R0,   `32*0`(%rsp)
    vmovdqu $_R0h,  `32*1`(%rsp)
    vmovdqu $_R1,   `32*2`(%rsp)
    vmovdqu $_R1h,  `32*3`(%rsp)
    vmovdqu $_R2,   `32*4`(%rsp)
    movq \$0, `32*5`(%rsp)

    vmovdqu `32*0 + 8`(%rsp), $_R0
    vmovdqu `32*1 + 8`(%rsp), $_R0h
    vmovdqu `32*2 + 8`(%rsp), $_R1
    vmovdqu `32*3 + 8`(%rsp), $_R1h
    vmovdqu `32*4 + 8`(%rsp), $_R2

    addq   8(%rsp), $_acc    # acc += R0[0]

    {vex} vpmadd52huq `$_data_offset`($a), $Bi, $_R0
    {vex} vpmadd52huq `$_data_offset+32`($a), $Bi, $_R0h
    {vex} vpmadd52huq `$_data_offset+64*1`($a), $Bi, $_R1
    {vex} vpmadd52huq `$_data_offset+64*1+32`($a), $Bi, $_R1h
    {vex} vpmadd52huq `$_data_offset+64*2`($a), $Bi, $_R2

    {vex} vpmadd52huq `$_data_offset`($m), $Yi, $_R0
    {vex} vpmadd52huq `$_data_offset+32`($m), $Yi, $_R0h
    {vex} vpmadd52huq `$_data_offset+64*1`($m), $Yi, $_R1
    {vex} vpmadd52huq `$_data_offset+64*1+32`($m), $Yi, $_R1h
    {vex} vpmadd52huq `$_data_offset+64*2`($m), $Yi, $_R2
    lea 168(%rsp),%rsp
___
}

# Normalization routine: handles carry bits and gets bignum qwords to normalized
# 2^52 representation.
#
# Uses %r8-14,%e[bcd]x
sub amm52x20_x1_norm {
my ($_acc,$_R0,$_R0h,$_R1,$_R1h,$_R2) = @_;
$code.=<<___;
    # Put accumulator to low qword in R0
    vmovq $_acc, $T0_xmm
    vpbroadcastq    $T0_xmm, $T0
    vpblendd \$3, $T0, $_R0, $_R0

    # Extract "carries" (12 high bits) from each QW of R0..R2
    # Save them to LSB of QWs in T0..T2
    vpsrlq    \$52, $_R0,   $T0
    vpsrlq    \$52, $_R0h,  $T0h
    vpsrlq    \$52, $_R1,   $T1
    vpsrlq    \$52, $_R1h,  $T1h
    vpsrlq    \$52, $_R2,   $T2

    # "Shift left" T0..T2 by 1 QW
    vpermq      \$144, $T2, $T2
    vpermq      \$3, $T1h, $tmp
    vblendpd    \$1, $tmp, $T2, $T2

    vpermq      \$144, $T1h, $T1h
    vpermq      \$3, $T1, $tmp
    vblendpd    \$1, $tmp, $T1h, $T1h

    vpermq      \$144, $T1, $T1
    vpermq      \$3, $T0h, $tmp
    vblendpd    \$1, $tmp, $T1, $T1

    vpermq      \$144, $T0h, $T0h
    vpermq      \$3, $T0, $tmp
    vblendpd    \$1, $tmp, $T0h, $T0h

    vpermq      \$144, $T0, $T0
    vpand       .Lhigh64x3(%rip), $T0, $T0

    # Drop "carries" from R0..R2 QWs
    vpand    .Lmask52x4(%rip), $_R0,  $_R0
    vpand    .Lmask52x4(%rip), $_R0h, $_R0h
    vpand    .Lmask52x4(%rip), $_R1,  $_R1
    vpand    .Lmask52x4(%rip), $_R1h, $_R1h
    vpand    .Lmask52x4(%rip), $_R2,  $_R2

    # Sum R0..R2 with corresponding adjusted carries
    vpaddq  $T0,  $_R0,  $_R0
    vpaddq  $T0h, $_R0h, $_R0h
    vpaddq  $T1,  $_R1,  $_R1
    vpaddq  $T1h, $_R1h, $_R1h
    vpaddq  $T2,  $_R2,  $_R2

    # Now handle carry bits from this addition
    # Get mask of QWs which 52-bit parts overflow...
    vpcmpgtq .Lmask52x4(%rip), $_R0,  $T0
    vpcmpgtq .Lmask52x4(%rip), $_R0h, $T0h
    vpcmpgtq .Lmask52x4(%rip), $_R1,  $T1
    vpcmpgtq .Lmask52x4(%rip), $_R1h, $T1h
    vpcmpgtq .Lmask52x4(%rip), $_R2,  $T2
    vmovmskpd $T0, %r14d
    vmovmskpd $T0h, %r13d
    vmovmskpd $T1, %r12d
    vmovmskpd $T1h, %r11d
    vmovmskpd $T2, %r10d

    # ...or saturated
    vpcmpeqq .Lmask52x4(%rip), $_R0,  $T0
    vpcmpeqq .Lmask52x4(%rip), $_R0h, $T0h
    vpcmpeqq .Lmask52x4(%rip), $_R1,  $T1
    vpcmpeqq .Lmask52x4(%rip), $_R1h, $T1h
    vpcmpeqq .Lmask52x4(%rip), $_R2,  $T2
    vmovmskpd $T0, %r9d
    vmovmskpd $T0h, %r8d
    vmovmskpd $T1, %ebx
    vmovmskpd $T1h, %ecx
    vmovmskpd $T2, %edx

    # Get mask of QWs where carries shall be propagated to.
    # Merge 4-bit masks to 8-bit values to use add with carry.
    shl   \$4, %r13b
    or    %r13b, %r14b
    shl   \$4, %r11b
    or    %r11b, %r12b

    add   %r14b, %r14b
    adc   %r12b, %r12b
    adc   %r10b, %r10b

    shl   \$4, %r8b
    or    %r8b,%r9b
    shl   \$4, %cl
    or    %cl, %bl

    add   %r9b, %r14b
    adc   %bl, %r12b
    adc   %dl, %r10b

    xor   %r9b, %r14b
    xor   %bl, %r12b
    xor   %dl, %r10b

    lea .Lkmasklut(%rip), %rdx

    mov %r14b, %r13b
    and \$0xf, %r14
    vpsubq  .Lmask52x4(%rip), $_R0,  $T0
    shl \$5, %r14
    vmovapd (%rdx,%r14), $T1
    vblendvpd $T1, $T0, $_R0, $_R0

    shr \$4, %r13b
    and \$0xf, %r13
    vpsubq  .Lmask52x4(%rip), $_R0h,  $T0
    shl \$5, %r13
    vmovapd (%rdx,%r13), $T1
    vblendvpd $T1, $T0, $_R0h, $_R0h

    mov %r12b, %r11b
    and \$0xf, %r12
    vpsubq  .Lmask52x4(%rip), $_R1,  $T0
    shl \$5, %r12
    vmovapd (%rdx,%r12), $T1
    vblendvpd $T1, $T0, $_R1, $_R1

    shr \$4, %r11b
    and \$0xf, %r11
    vpsubq  .Lmask52x4(%rip), $_R1h,  $T0
    shl \$5, %r11
    vmovapd (%rdx,%r11), $T1
    vblendvpd $T1, $T0, $_R1h, $_R1h

    and \$0xf, %r10
    vpsubq  .Lmask52x4(%rip), $_R2,  $T0
    shl \$5, %r10
    vmovapd (%rdx,%r10), $T1
    vblendvpd $T1, $T0, $_R2, $_R2

    # Add carries according to the obtained mask
    vpand   .Lmask52x4(%rip), $_R0,  $_R0
    vpand   .Lmask52x4(%rip), $_R0h, $_R0h
    vpand   .Lmask52x4(%rip), $_R1,  $_R1
    vpand   .Lmask52x4(%rip), $_R1h, $_R1h
    vpand   .Lmask52x4(%rip), $_R2,  $_R2
___
}

$code.=<<___;
.text

.globl  ossl_rsaz_amm52x20_x1_avxifma256
.type   ossl_rsaz_amm52x20_x1_avxifma256,\@function,5
.align 32
ossl_rsaz_amm52x20_x1_avxifma256:
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
.Lossl_rsaz_amm52x20_x1_avxifma256_body:

    # Zeroing accumulators
    vpxor    $zero, $zero, $zero
    vmovapd   $zero, $R0_0
    vmovapd   $zero, $R0_0h
    vmovapd   $zero, $R1_0
    vmovapd   $zero, $R1_0h
    vmovapd   $zero, $R2_0

    xorl    $acc0_0_low, $acc0_0_low

    movq    $b, $b_ptr                       # backup address of b
    movq    \$0xfffffffffffff, $mask52       # 52-bit mask

    # Loop over 20 digits unrolled by 4
    mov     \$5, $iter

.align 32
.Lloop5:
___
    foreach my $idx (0..3) {
        &amm52x20_x1(0,8*$idx,$acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,$k0);
    }
$code.=<<___;
    lea    `4*8`($b_ptr), $b_ptr
    dec    $iter
    jne    .Lloop5
___
    &amm52x20_x1_norm($acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0);
$code.=<<___;

    vmovdqu   $R0_0,  `0*32`($res)
    vmovdqu   $R0_0h, `1*32`($res)
    vmovdqu   $R1_0,  `2*32`($res)
    vmovdqu   $R1_0h, `3*32`($res)
    vmovdqu   $R2_0,  `4*32`($res)

    vzeroupper
    mov  0(%rsp),%r15
.cfi_restore    %r15
    mov  8(%rsp),%r14
.cfi_restore    %r14
    mov  16(%rsp),%r13
.cfi_restore    %r13
    mov  24(%rsp),%r12
.cfi_restore    %r12
    mov  32(%rsp),%rbp
.cfi_restore    %rbp
    mov  40(%rsp),%rbx
.cfi_restore    %rbx
    lea  48(%rsp),%rsp
.cfi_adjust_cfa_offset  -48
.Lossl_rsaz_amm52x20_x1_avxifma256_epilogue:
    ret
.cfi_endproc
.size   ossl_rsaz_amm52x20_x1_avxifma256, .-ossl_rsaz_amm52x20_x1_avxifma256
___

$code.=<<___;
.section .rodata align=32
.align 32
.Lmask52x4:
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
    .quad   0xfffffffffffff
.Lhigh64x3:
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
.Lkmasklut:
    #0000
    .quad   0x0
    .quad   0x0
    .quad   0x0
    .quad   0x0
    #0001
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0x0
    .quad   0x0
    #0010
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0x0
    #0011
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0x0
    #0100
    .quad   0x0
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0x0
    #0101
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0x0
    #0110
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0x0
    #0111
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0x0
    #1000
    .quad   0x0
    .quad   0x0
    .quad   0x0
    .quad   0xffffffffffffffff
    #1001
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0x0
    .quad   0xffffffffffffffff
    #1010
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0xffffffffffffffff
    #1011
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0xffffffffffffffff
    #1100
    .quad   0x0
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    #1101
    .quad   0xffffffffffffffff
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    #1110
    .quad   0x0
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    #1111
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
    .quad   0xffffffffffffffff
___

###############################################################################
# Dual Almost Montgomery Multiplication for 20-digit number in radix 2^52
#
# See description of ossl_rsaz_amm52x20_x1_ifma256() above for details about Almost
# Montgomery Multiplication algorithm and function input parameters description.
#
# This function does two AMMs for two independent inputs, hence dual.
#
# void ossl_rsaz_amm52x20_x2_avxifma256(BN_ULONG out[2][20],
#                                       const BN_ULONG a[2][20],
#                                       const BN_ULONG b[2][20],
#                                       const BN_ULONG m[2][20],
#                                       const BN_ULONG k0[2]);
###############################################################################

$code.=<<___;
.text

.globl  ossl_rsaz_amm52x20_x2_avxifma256
.type   ossl_rsaz_amm52x20_x2_avxifma256,\@function,5
.align 32
ossl_rsaz_amm52x20_x2_avxifma256:
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
.Lossl_rsaz_amm52x20_x2_avxifma256_body:

    # Zeroing accumulators
    vpxor   $zero, $zero, $zero
    vmovapd   $zero, $R0_0
    vmovapd   $zero, $R0_0h
    vmovapd   $zero, $R1_0
    vmovapd   $zero, $R1_0h
    vmovapd   $zero, $R2_0
    vmovapd   $zero, $R0_1
    vmovapd   $zero, $R0_1h
    vmovapd   $zero, $R1_1
    vmovapd   $zero, $R1_1h
    vmovapd   $zero, $R2_1

    xorl    $acc0_0_low, $acc0_0_low
    xorl    $acc0_1_low, $acc0_1_low

    movq    $b, $b_ptr                       # backup address of b
    movq    \$0xfffffffffffff, $mask52       # 52-bit mask

    mov    \$20, $iter

.align 32
.Lloop20:
___
    &amm52x20_x1(   0,   0,$acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0,"($k0)");
    # 20*8 = offset of the next dimension in two-dimension array
    &amm52x20_x1(20*8,20*8,$acc0_1,$R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1,"8($k0)");
$code.=<<___;
    lea    8($b_ptr), $b_ptr
    dec    $iter
    jne    .Lloop20
___
    &amm52x20_x1_norm($acc0_0,$R0_0,$R0_0h,$R1_0,$R1_0h,$R2_0);
    &amm52x20_x1_norm($acc0_1,$R0_1,$R0_1h,$R1_1,$R1_1h,$R2_1);
$code.=<<___;

    vmovdqu   $R0_0,  `0*32`($res)
    vmovdqu   $R0_0h, `1*32`($res)
    vmovdqu   $R1_0,  `2*32`($res)
    vmovdqu   $R1_0h, `3*32`($res)
    vmovdqu   $R2_0,  `4*32`($res)

    vmovdqu   $R0_1,  `5*32`($res)
    vmovdqu   $R0_1h, `6*32`($res)
    vmovdqu   $R1_1,  `7*32`($res)
    vmovdqu   $R1_1h, `8*32`($res)
    vmovdqu   $R2_1,  `9*32`($res)

    vzeroupper
    mov  0(%rsp),%r15
.cfi_restore    %r15
    mov  8(%rsp),%r14
.cfi_restore    %r14
    mov  16(%rsp),%r13
.cfi_restore    %r13
    mov  24(%rsp),%r12
.cfi_restore    %r12
    mov  32(%rsp),%rbp
.cfi_restore    %rbp
    mov  40(%rsp),%rbx
.cfi_restore    %rbx
    lea  48(%rsp),%rsp
.cfi_adjust_cfa_offset  -48
.Lossl_rsaz_amm52x20_x2_avxifma256_epilogue:
    ret
.cfi_endproc
.size   ossl_rsaz_amm52x20_x2_avxifma256, .-ossl_rsaz_amm52x20_x2_avxifma256
___
}

###############################################################################
# Constant time extraction from the precomputed table of powers base^i, where
#    i = 0..2^EXP_WIN_SIZE-1
#
# The input |red_table| contains precomputations for two independent base values.
# |red_table_idx1| and |red_table_idx2| are corresponding power indexes.
#
# Extracted value (output) is 2 20 digit numbers in 2^52 radix.
#
# void ossl_extract_multiplier_2x20_win5_avx(BN_ULONG *red_Y,
#                                            const BN_ULONG red_table[1 << EXP_WIN_SIZE][2][20],
#                                            int red_table_idx1, int red_table_idx2);
#
# EXP_WIN_SIZE = 5
###############################################################################
{
# input parameters
my ($out,$red_tbl,$red_tbl_idx1,$red_tbl_idx2)=$win64 ? ("%rcx","%rdx","%r8", "%r9") :  # Win64 order
                                                        ("%rdi","%rsi","%rdx","%rcx");  # Unix order

my ($t0,$t1,$t2,$t3,$t4,$t5) = map("%ymm$_", (0..5));
my ($t6,$t7,$t8,$t9) = map("%ymm$_", (6..9));
my ($tmp,$cur_idx,$idx1,$idx2,$ones,$mask) = map("%ymm$_", (10..15));

my @t = ($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$t9);
my $t0xmm = $t0;
my $tmp_xmm = "%xmm10";
$t0xmm =~ s/%y/%x/;

$code.=<<___;
.text

.align 32
.globl  ossl_extract_multiplier_2x20_win5_avx
.type   ossl_extract_multiplier_2x20_win5_avx,\@abi-omnipotent
ossl_extract_multiplier_2x20_win5_avx:
.cfi_startproc
    endbranch
    vmovapd   .Lones(%rip), $ones         # broadcast ones
    vmovq $red_tbl_idx1, $tmp_xmm
    vpbroadcastq    $tmp_xmm, $idx1
    vmovq $red_tbl_idx2, $tmp_xmm
    vpbroadcastq    $tmp_xmm, $idx2
    leaq   `(1<<5)*2*20*8`($red_tbl), %rax  # holds end of the tbl

    # zeroing t0..n, cur_idx
    vpxor   $t0xmm, $t0xmm, $t0xmm
    vmovapd   $t0, $cur_idx
___
foreach (1..9) {
    $code.="vmovapd   $t0, $t[$_] \n";
}
$code.=<<___;

.align 32
.Lloop:
    vpcmpeqq  $cur_idx, $idx1, $mask      # mask of (idx1 == cur_idx)
___
foreach (0..4) {
$code.=<<___;
    vmovdqu  `${_}*32`($red_tbl), $tmp     # load data from red_tbl
    vblendvpd  $mask, $tmp, $t[$_], ${t[$_]} # extract data when mask is not zero
___
}
$code.=<<___;
    vpcmpeqq  $cur_idx, $idx2, $mask      # mask of (idx2 == cur_idx)
___
foreach (5..9) {
$code.=<<___;
    vmovdqu  `${_}*32`($red_tbl), $tmp     # load data from red_tbl
    vblendvpd  $mask, $tmp, $t[$_], ${t[$_]} # extract data when mask is not zero
___
}
$code.=<<___;
    vpaddq  $ones, $cur_idx, $cur_idx      # increment cur_idx
    addq    \$`2*20*8`, $red_tbl
    cmpq    $red_tbl, %rax
    jne .Lloop
___
# store t0..n
foreach (0..9) {
    $code.="vmovdqu   $t[$_], `${_}*32`($out) \n";
}
$code.=<<___;
    ret
.cfi_endproc
.size   ossl_extract_multiplier_2x20_win5_avx, .-ossl_extract_multiplier_2x20_win5_avx
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
.type   rsaz_def_handler,\@abi-omnipotent
.align  16
rsaz_def_handler:
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

    mov     152($context),%rax # pull context->Rsp

    mov     4(%r11),%r10d      # HandlerData[1]
    lea     (%rsi,%r10),%r10   # epilogue label
    cmp     %r10,%rbx          # context->Rip>=.Lepilogue
    jae     .Lcommon_seh_tail

    lea     48(%rax),%rax

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
.size   rsaz_def_handler,.-rsaz_def_handler

.section    .pdata
.align  4
    .rva    .LSEH_begin_ossl_rsaz_amm52x20_x1_avxifma256
    .rva    .LSEH_end_ossl_rsaz_amm52x20_x1_avxifma256
    .rva    .LSEH_info_ossl_rsaz_amm52x20_x1_avxifma256

    .rva    .LSEH_begin_ossl_rsaz_amm52x20_x2_avxifma256
    .rva    .LSEH_end_ossl_rsaz_amm52x20_x2_avxifma256
    .rva    .LSEH_info_ossl_rsaz_amm52x20_x2_avxifma256

.section    .xdata
.align  8
.LSEH_info_ossl_rsaz_amm52x20_x1_avxifma256:
    .byte   9,0,0,0
    .rva    rsaz_def_handler
    .rva    .Lossl_rsaz_amm52x20_x1_avxifma256_body,.Lossl_rsaz_amm52x20_x1_avxifma256_epilogue
.LSEH_info_ossl_rsaz_amm52x20_x2_avxifma256:
    .byte   9,0,0,0
    .rva    rsaz_def_handler
    .rva    .Lossl_rsaz_amm52x20_x2_avxifma256_body,.Lossl_rsaz_amm52x20_x2_avxifma256_epilogue
___
}
}}} else {{{                # fallback for old assembler
$code.=<<___;
.text

.globl  ossl_rsaz_avxifma_eligible
.type   ossl_rsaz_avxifma_eligible,\@abi-omnipotent
ossl_rsaz_avxifma_eligible:
    xor     %eax,%eax
    ret
.size   ossl_rsaz_avxifma_eligible, .-ossl_rsaz_avxifma_eligible

.globl  ossl_rsaz_amm52x20_x1_avxifma256
.globl  ossl_rsaz_amm52x20_x2_avxifma256
.globl  ossl_extract_multiplier_2x20_win5_avx
.type   ossl_rsaz_amm52x20_x1_avxifma256,\@abi-omnipotent
ossl_rsaz_amm52x20_x1_avxifma256:
ossl_rsaz_amm52x20_x2_avxifma256:
ossl_extract_multiplier_2x20_win5_avx:
    .byte   0x0f,0x0b    # ud2
    ret
.size   ossl_rsaz_amm52x20_x1_avxifma256, .-ossl_rsaz_amm52x20_x1_avxifma256
___
}}}

$code =~ s/\`([^\`]*)\`/eval $1/gem;
print $code;
close STDOUT or die "error closing STDOUT: $!";
