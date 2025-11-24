#!/usr/bin/env perl
# Copyright 2024 The BoringSSL Authors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
#------------------------------------------------------------------------------
#
# VAES and VPCLMULQDQ optimized AES-GCM for x86_64 (AVX2 version)
#
# This is similar to aes-gcm-avx10-x86_64.pl, but it uses AVX2 instead of AVX512
# / AVX10.  This means it can only use 16 vector registers instead of 32, the
# maximum vector length is 32 bytes, and some instructions such as vpternlogd
# and masked loads/stores are unavailable.  However, it is able to run on CPUs
# that have VAES without AVX512 / AVX10, namely AMD Zen 3 (including "Milan"
# server processors) and some Intel client CPUs such as Alder Lake.
#
# This implementation also uses Karatsuba multiplication instead of schoolbook
# multiplication for GHASH in its main loop.  This does not help much on Intel,
# but it improves performance by ~5% on AMD Zen 3 which is the main target for
# this implementation.  Other factors weighing slightly in favor of Karatsuba
# multiplication in this implementation are the lower maximum vector length
# (which means there is space left in the Htable array to cache the halves of
# the key powers XOR'd together) and the unavailability of the vpternlogd
# instruction (which helped schoolbook a bit more than Karatsuba).

use strict;

my $flavour = shift;
my $output  = shift;
if ( $flavour =~ /\./ ) { $output = $flavour; undef $flavour; }

my $win64;
my @argregs;
if ( $flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/ ) {
    $win64   = 1;
    @argregs = ( "%rcx", "%rdx", "%r8", "%r9" );
}
else {
    $win64   = 0;
    @argregs = ( "%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9" );
}

$0 =~ m/(.*[\/\\])[^\/\\]+$/;
my $dir = $1;
my $xlate;
( $xlate = "${dir}x86_64-xlate.pl" and -f $xlate )
  or ( $xlate = "${dir}../../../perlasm/x86_64-xlate.pl" and -f $xlate )
  or die "can't locate x86_64-xlate.pl";

open OUT, "| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT = *OUT;

my $g_cur_func_name;
my $g_cur_func_uses_seh;
my @g_cur_func_saved_gpregs;
my @g_cur_func_saved_xmmregs;

sub _begin_func {
    my ( $funcname, $uses_seh ) = @_;
    $g_cur_func_name          = $funcname;
    $g_cur_func_uses_seh      = $uses_seh;
    @g_cur_func_saved_gpregs  = ();
    @g_cur_func_saved_xmmregs = ();
    return <<___;
.globl $funcname
.type $funcname,\@abi-omnipotent
.align 32
$funcname:
    .cfi_startproc
    @{[ $uses_seh ? ".seh_startproc" : "" ]}
    _CET_ENDBR
___
}

# Push a list of general purpose registers onto the stack.
sub _save_gpregs {
    my @gpregs = @_;
    my $code   = "";
    die "_save_gpregs requires uses_seh" unless $g_cur_func_uses_seh;
    die "_save_gpregs can only be called once per function"
      if @g_cur_func_saved_gpregs;
    die "Order must be _save_gpregs, then _save_xmmregs"
      if @g_cur_func_saved_xmmregs;
    @g_cur_func_saved_gpregs = @gpregs;
    for my $reg (@gpregs) {
        $code .= "push $reg\n";
        if ($win64) {
            $code .= ".seh_pushreg $reg\n";
        }
        else {
            $code .= ".cfi_push $reg\n";
        }
    }
    return $code;
}

# Push a list of xmm registers onto the stack if the target is Windows.
sub _save_xmmregs {
    my @xmmregs     = @_;
    my $num_xmmregs = scalar @xmmregs;
    my $code        = "";
    die "_save_xmmregs requires uses_seh" unless $g_cur_func_uses_seh;
    die "_save_xmmregs can only be called once per function"
      if @g_cur_func_saved_xmmregs;
    if ( $win64 and $num_xmmregs > 0 ) {
        @g_cur_func_saved_xmmregs = @xmmregs;
        my $is_misaligned = ( scalar @g_cur_func_saved_gpregs ) % 2 == 0;
        my $alloc_size    = 16 * $num_xmmregs + ( $is_misaligned ? 8 : 0 );
        $code .= "sub \$$alloc_size, %rsp\n";
        $code .= ".seh_stackalloc $alloc_size\n";
        for my $i ( 0 .. $num_xmmregs - 1 ) {
            my $reg_num = $xmmregs[$i];
            my $pos     = 16 * $i;
            $code .= "movdqa %xmm$reg_num, $pos(%rsp)\n";
            $code .= ".seh_savexmm %xmm$reg_num, $pos\n";
        }
    }
    return $code;
}

sub _end_func {
    my $code = "";

    # Restore any xmm registers that were saved earlier.
    my $num_xmmregs = scalar @g_cur_func_saved_xmmregs;
    if ( $win64 and $num_xmmregs > 0 ) {
        my $need_alignment = ( scalar @g_cur_func_saved_gpregs ) % 2 == 0;
        my $alloc_size     = 16 * $num_xmmregs + ( $need_alignment ? 8 : 0 );
        for my $i ( 0 .. $num_xmmregs - 1 ) {
            my $reg_num = $g_cur_func_saved_xmmregs[$i];
            my $pos     = 16 * $i;
            $code .= "movdqa $pos(%rsp), %xmm$reg_num\n";
        }
        $code .= "add \$$alloc_size, %rsp\n";
    }

    # Restore any general purpose registers that were saved earlier.
    for my $reg ( reverse @g_cur_func_saved_gpregs ) {
        $code .= "pop $reg\n";
        if ( !$win64 ) {
            $code .= ".cfi_pop $reg\n";
        }
    }

    $code .= <<___;
    ret
    @{[ $g_cur_func_uses_seh ? ".seh_endproc" : "" ]}
    .cfi_endproc
    .size   $g_cur_func_name, . - $g_cur_func_name
___
    return $code;
}

my $code = <<___;
.section .rodata
.align 16

    # A shuffle mask that reflects the bytes of 16-byte blocks
.Lbswap_mask:
    .quad   0x08090a0b0c0d0e0f, 0x0001020304050607

    # This is the GHASH reducing polynomial without its constant term, i.e.
    # x^128 + x^7 + x^2 + x, represented using the backwards mapping
    # between bits and polynomial coefficients.
    #
    # Alternatively, it can be interpreted as the naturally-ordered
    # representation of the polynomial x^127 + x^126 + x^121 + 1, i.e. the
    # "reversed" GHASH reducing polynomial without its x^128 term.
.Lgfpoly:
    .quad   1, 0xc200000000000000

    # Same as above, but with the (1 << 64) bit set.
.Lgfpoly_and_internal_carrybit:
    .quad   1, 0xc200000000000001

.align 32
    # The below constants are used for incrementing the counter blocks.
.Lctr_pattern:
    .quad   0, 0
    .quad   1, 0
.Linc_2blocks:
    .quad   2, 0
    .quad   2, 0

.text
___

# We use Htable[0..7] to store H^8 through H^1, and Htable[8..11] to store the
# 64-bit halves of the key powers XOR'd together (for Karatsuba multiplication)
# in the order 8,6,7,5,4,2,3,1.  We do not use Htable[12..15].
my $NUM_H_POWERS            = 8;
my $OFFSETOFEND_H_POWERS    = $NUM_H_POWERS * 16;
my $OFFSETOF_H_POWERS_XORED = $OFFSETOFEND_H_POWERS;

# Offset to 'rounds' in AES_KEY struct
my $OFFSETOF_AES_ROUNDS = 240;

# GHASH-multiply the 128-bit lanes of \a by the 128-bit lanes of \b and store
# the reduced products in \dst.  Uses schoolbook multiplication.
sub _ghash_mul {
    my ( $a, $b, $dst, $gfpoly, $t0, $t1, $t2 ) = @_;
    return <<___;
    vpclmulqdq      \$0x00, $a, $b, $t0        # LO = a_L * b_L
    vpclmulqdq      \$0x01, $a, $b, $t1        # MI_0 = a_L * b_H
    vpclmulqdq      \$0x10, $a, $b, $t2        # MI_1 = a_H * b_L
    vpxor           $t2, $t1, $t1              # MI = MI_0 + MI_1
    vpclmulqdq      \$0x01, $t0, $gfpoly, $t2  # LO_L*(x^63 + x^62 + x^57)
    vpshufd         \$0x4e, $t0, $t0           # Swap halves of LO
    vpxor           $t0, $t1, $t1              # Fold LO into MI (part 1)
    vpxor           $t2, $t1, $t1              # Fold LO into MI (part 2)
    vpclmulqdq      \$0x11, $a, $b, $dst       # HI = a_H * b_H
    vpclmulqdq      \$0x01, $t1, $gfpoly, $t0  # MI_L*(x^63 + x^62 + x^57)
    vpshufd         \$0x4e, $t1, $t1           # Swap halves of MI
    vpxor           $t1, $dst, $dst            # Fold MI into HI (part 1)
    vpxor           $t0, $dst, $dst            # Fold MI into HI (part 2)
___
}

# void gcm_init_vpclmulqdq_avx2(u128 Htable[16], const uint64_t H[2]);
#
# Initialize |Htable| with powers of the GHASH subkey |H|.
#
# We use Htable[0..7] to store H^8 through H^1, and Htable[8..11] to store the
# 64-bit halves of the key powers XOR'd together (for Karatsuba multiplication)
# in the order 8,6,7,5,4,2,3,1.  We do not use Htable[12..15].
$code .= _begin_func "gcm_init_vpclmulqdq_avx2", 1;
{
    my ( $HTABLE, $H_PTR ) = @argregs[ 0 .. 1 ];
    my ( $TMP0,   $TMP0_XMM )   = ( "%ymm0", "%xmm0" );
    my ( $TMP1,   $TMP1_XMM )   = ( "%ymm1", "%xmm1" );
    my ( $TMP2,   $TMP2_XMM )   = ( "%ymm2", "%xmm2" );
    my ( $H_CUR,  $H_CUR_XMM )  = ( "%ymm3", "%xmm3" );
    my ( $H_CUR2, $H_CUR2_XMM ) = ( "%ymm4", "%xmm4" );
    my ( $H_INC,  $H_INC_XMM )  = ( "%ymm5", "%xmm5" );
    my ( $GFPOLY, $GFPOLY_XMM ) = ( "%ymm6", "%xmm6" );

    $code .= <<___;
    @{[ _save_xmmregs (6) ]}
    .seh_endprologue

    # Load the byte-reflected hash subkey.  BoringSSL provides it in
    # byte-reflected form except the two halves are in the wrong order.
    vpshufd         \$0x4e, ($H_PTR), $H_CUR_XMM

    # Finish preprocessing the byte-reflected hash subkey by multiplying it by
    # x^-1 ("standard" interpretation of polynomial coefficients) or
    # equivalently x^1 (natural interpretation).  This gets the key into a
    # format that avoids having to bit-reflect the data blocks later.
    vpshufd         \$0xd3, $H_CUR_XMM, $TMP0_XMM
    vpsrad          \$31, $TMP0_XMM, $TMP0_XMM
    vpaddq          $H_CUR_XMM, $H_CUR_XMM, $H_CUR_XMM
    vpand           .Lgfpoly_and_internal_carrybit(%rip), $TMP0_XMM, $TMP0_XMM
    vpxor           $TMP0_XMM, $H_CUR_XMM, $H_CUR_XMM

    vbroadcasti128  .Lgfpoly(%rip), $GFPOLY

    # Square H^1 to get H^2.
    @{[ _ghash_mul  $H_CUR_XMM, $H_CUR_XMM, $H_INC_XMM, $GFPOLY_XMM,
                    $TMP0_XMM, $TMP1_XMM, $TMP2_XMM ]}

    # Create H_CUR = [H^2, H^1] and H_INC = [H^2, H^2].
    vinserti128     \$1, $H_CUR_XMM, $H_INC, $H_CUR
    vinserti128     \$1, $H_INC_XMM, $H_INC, $H_INC

    # Compute H_CUR2 = [H^4, H^3].
    @{[ _ghash_mul  $H_INC, $H_CUR, $H_CUR2, $GFPOLY, $TMP0, $TMP1, $TMP2 ]}

    # Store [H^2, H^1] and [H^4, H^3].
    vmovdqu         $H_CUR, 3*32($HTABLE)
    vmovdqu         $H_CUR2, 2*32($HTABLE)

    # For Karatsuba multiplication: compute and store the two 64-bit halves of
    # each key power XOR'd together.  Order is 4,2,3,1.
    vpunpcklqdq     $H_CUR, $H_CUR2, $TMP0
    vpunpckhqdq     $H_CUR, $H_CUR2, $TMP1
    vpxor           $TMP1, $TMP0, $TMP0
    vmovdqu         $TMP0, $OFFSETOF_H_POWERS_XORED+32($HTABLE)

    # Compute and store H_CUR = [H^6, H^5] and H_CUR2 = [H^8, H^7].
    @{[ _ghash_mul  $H_INC, $H_CUR2, $H_CUR, $GFPOLY, $TMP0, $TMP1, $TMP2 ]}
    @{[ _ghash_mul  $H_INC, $H_CUR, $H_CUR2, $GFPOLY, $TMP0, $TMP1, $TMP2 ]}
    vmovdqu         $H_CUR, 1*32($HTABLE)
    vmovdqu         $H_CUR2, 0*32($HTABLE)

    # Again, compute and store the two 64-bit halves of each key power XOR'd
    # together.  Order is 8,6,7,5.
    vpunpcklqdq     $H_CUR, $H_CUR2, $TMP0
    vpunpckhqdq     $H_CUR, $H_CUR2, $TMP1
    vpxor           $TMP1, $TMP0, $TMP0
    vmovdqu         $TMP0, $OFFSETOF_H_POWERS_XORED($HTABLE)

    vzeroupper
___
}
$code .= _end_func;

# Do one step of the GHASH update of four vectors of data blocks.
#   $i: the step to do, 0 through 9
#   $ghashdata_ptr: pointer to the data blocks (ciphertext or AAD)
#   $htable: pointer to the Htable for the key
#   $bswap_mask: mask for reflecting the bytes of blocks
#   $h_pow[2-1]_xored: XOR'd key powers cached from Htable
#   $tmp[0-2]: temporary registers.  $tmp[1-2] must be preserved across steps.
#   $lo, $mi: working state for this macro that must be preserved across steps
#   $ghash_acc: the GHASH accumulator (input/output)
sub _ghash_step_4x {
    my (
        $i,            $ghashdata_ptr, $htable, $bswap_mask,
        $h_pow2_xored, $h_pow1_xored,  $tmp0,   $tmp0_xmm,
        $tmp1,         $tmp2,          $lo,     $mi,
        $ghash_acc,    $ghash_acc_xmm
    ) = @_;
    my ( $hi, $hi_xmm ) = ( $ghash_acc, $ghash_acc_xmm );    # alias
    if ( $i == 0 ) {
        return <<___;
        # First vector
        vmovdqu         0*32($ghashdata_ptr), $tmp1
        vpshufb         $bswap_mask, $tmp1, $tmp1
        vmovdqu         0*32($htable), $tmp2
        vpxor           $ghash_acc, $tmp1, $tmp1
        vpclmulqdq      \$0x00, $tmp2, $tmp1, $lo
        vpclmulqdq      \$0x11, $tmp2, $tmp1, $hi
        vpunpckhqdq     $tmp1, $tmp1, $tmp0
        vpxor           $tmp1, $tmp0, $tmp0
        vpclmulqdq      \$0x00, $h_pow2_xored, $tmp0, $mi
___
    }
    elsif ( $i == 1 ) {
        return <<___;
___
    }
    elsif ( $i == 2 ) {
        return <<___;
        # Second vector
        vmovdqu         1*32($ghashdata_ptr), $tmp1
        vpshufb         $bswap_mask, $tmp1, $tmp1
        vmovdqu         1*32($htable), $tmp2
        vpclmulqdq      \$0x00, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $lo, $lo
        vpclmulqdq      \$0x11, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $hi, $hi
        vpunpckhqdq     $tmp1, $tmp1, $tmp0
        vpxor           $tmp1, $tmp0, $tmp0
        vpclmulqdq      \$0x10, $h_pow2_xored, $tmp0, $tmp0
        vpxor           $tmp0, $mi, $mi
___
    }
    elsif ( $i == 3 ) {
        return <<___;
        # Third vector
        vmovdqu         2*32($ghashdata_ptr), $tmp1
        vpshufb         $bswap_mask, $tmp1, $tmp1
        vmovdqu         2*32($htable), $tmp2
___
    }
    elsif ( $i == 4 ) {
        return <<___;
        vpclmulqdq      \$0x00, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $lo, $lo
        vpclmulqdq      \$0x11, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $hi, $hi
___
    }
    elsif ( $i == 5 ) {
        return <<___;
        vpunpckhqdq     $tmp1, $tmp1, $tmp0
        vpxor           $tmp1, $tmp0, $tmp0
        vpclmulqdq      \$0x00, $h_pow1_xored, $tmp0, $tmp0
        vpxor           $tmp0, $mi, $mi

        # Fourth vector
        vmovdqu         3*32($ghashdata_ptr), $tmp1
        vpshufb         $bswap_mask, $tmp1, $tmp1
___
    }
    elsif ( $i == 6 ) {
        return <<___;
        vmovdqu         3*32($htable), $tmp2
        vpclmulqdq      \$0x00, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $lo, $lo
        vpclmulqdq      \$0x11, $tmp2, $tmp1, $tmp0
        vpxor           $tmp0, $hi, $hi
        vpunpckhqdq     $tmp1, $tmp1, $tmp0
        vpxor           $tmp1, $tmp0, $tmp0
        vpclmulqdq      \$0x10, $h_pow1_xored, $tmp0, $tmp0
        vpxor           $tmp0, $mi, $mi
___
    }
    elsif ( $i == 7 ) {
        return <<___;
        # Finalize 'mi' following Karatsuba multiplication.
        vpxor           $lo, $mi, $mi
        vpxor           $hi, $mi, $mi

        # Fold lo into mi.
        vbroadcasti128  .Lgfpoly(%rip), $tmp2
        vpclmulqdq      \$0x01, $lo, $tmp2, $tmp0
        vpshufd         \$0x4e, $lo, $lo
        vpxor           $lo, $mi, $mi
        vpxor           $tmp0, $mi, $mi
___
    }
    elsif ( $i == 8 ) {
        return <<___;
        # Fold mi into hi.
        vpclmulqdq      \$0x01, $mi, $tmp2, $tmp0
        vpshufd         \$0x4e, $mi, $mi
        vpxor           $mi, $hi, $hi
        vpxor           $tmp0, $hi, $hi
___
    }
    elsif ( $i == 9 ) {
        return <<___;
        vextracti128    \$1, $hi, $tmp0_xmm
        vpxor           $tmp0_xmm, $hi_xmm, $ghash_acc_xmm
___
    }
}

sub _ghash_4x {
    my $code = "";
    for my $i ( 0 .. 9 ) {
        $code .= _ghash_step_4x $i, @_;
    }
    return $code;
}

# void gcm_ghash_vpclmulqdq_avx2(uint8_t Xi[16], const u128 Htable[16],
#                                const uint8_t *in, size_t len);
#
# Using the key |Htable|, update the GHASH accumulator |Xi| with the data given
# by |in| and |len|.  |len| must be exactly 16.
$code .= _begin_func "gcm_ghash_vpclmulqdq_avx2_1", 1;
{
    # Function arguments
    my ( $GHASH_ACC_PTR, $HTABLE, $AAD, $AADLEN ) = @argregs[ 0 .. 3 ];

    # Additional local variables
    my ( $TMP0,       $TMP0_XMM )       = ( "%ymm0", "%xmm0" );
    my ( $TMP1,       $TMP1_XMM )       = ( "%ymm1", "%xmm1" );
    my ( $TMP2,       $TMP2_XMM )       = ( "%ymm2", "%xmm2" );
    my ( $LO,         $LO_XMM )         = ( "%ymm3", "%xmm3" );
    my ( $MI,         $MI_XMM )         = ( "%ymm4", "%xmm4" );
    my ( $GHASH_ACC,  $GHASH_ACC_XMM )  = ( "%ymm5", "%xmm5" );
    my ( $BSWAP_MASK, $BSWAP_MASK_XMM ) = ( "%ymm6", "%xmm6" );
    my ( $GFPOLY,     $GFPOLY_XMM )     = ( "%ymm7", "%xmm7" );
    my $H_POW2_XORED = "%ymm8";
    my $H_POW1_XORED = "%ymm9";

    $code .= <<___;
    @{[ _save_xmmregs (6 .. 9) ]}
    .seh_endprologue

    # Load the bswap_mask and gfpoly constants.  Since AADLEN is usually small,
    # usually only 128-bit vectors will be used.  So as an optimization, don't
    # broadcast these constants to both 128-bit lanes quite yet.
    vmovdqu         .Lbswap_mask(%rip), $BSWAP_MASK_XMM
    vmovdqu         .Lgfpoly(%rip), $GFPOLY_XMM

    # Load the GHASH accumulator.
    vmovdqu         ($GHASH_ACC_PTR), $GHASH_ACC_XMM
    vpshufb         $BSWAP_MASK_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM


    # Update GHASH with the remaining 16-byte block if any.
.Lghash_lastblock:
    vmovdqu         ($AAD), $TMP0_XMM
    vpshufb         $BSWAP_MASK_XMM, $TMP0_XMM, $TMP0_XMM
    vpxor           $TMP0_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM
    vmovdqu         $OFFSETOFEND_H_POWERS-16($HTABLE), $TMP0_XMM
    @{[ _ghash_mul  $TMP0_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM, $GFPOLY_XMM,
                    $TMP1_XMM, $TMP2_XMM, $LO_XMM ]}

.Lghash_done:
    # Store the updated GHASH accumulator back to memory.
    vpshufb         $BSWAP_MASK_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM
    vmovdqu         $GHASH_ACC_XMM, ($GHASH_ACC_PTR)

    vzeroupper
___
}
$code .= _end_func;

sub _vaesenc_4x {
    my ( $round_key, $aesdata0, $aesdata1, $aesdata2, $aesdata3 ) = @_;
    return <<___;
    vaesenc         $round_key, $aesdata0, $aesdata0
    vaesenc         $round_key, $aesdata1, $aesdata1
    vaesenc         $round_key, $aesdata2, $aesdata2
    vaesenc         $round_key, $aesdata3, $aesdata3
___
}

sub _ctr_begin_4x {
    my (
        $le_ctr,   $bswap_mask, $rndkey0,  $aesdata0,
        $aesdata1, $aesdata2,   $aesdata3, $tmp
    ) = @_;
    return <<___;
    # Increment le_ctr four times to generate four vectors of little-endian
    # counter blocks, swap each to big-endian, and store them in aesdata[0-3].
    vmovdqu         .Linc_2blocks(%rip), $tmp
    vpshufb         $bswap_mask, $le_ctr, $aesdata0
    vpaddd          $tmp, $le_ctr, $le_ctr
    vpshufb         $bswap_mask, $le_ctr, $aesdata1
    vpaddd          $tmp, $le_ctr, $le_ctr
    vpshufb         $bswap_mask, $le_ctr, $aesdata2
    vpaddd          $tmp, $le_ctr, $le_ctr
    vpshufb         $bswap_mask, $le_ctr, $aesdata3
    vpaddd          $tmp, $le_ctr, $le_ctr

    # AES "round zero": XOR in the zero-th round key.
    vpxor           $rndkey0, $aesdata0, $aesdata0
    vpxor           $rndkey0, $aesdata1, $aesdata1
    vpxor           $rndkey0, $aesdata2, $aesdata2
    vpxor           $rndkey0, $aesdata3, $aesdata3
___
}

# Do the last AES round for four vectors of counter blocks, XOR four vectors of
# source data with the resulting keystream blocks, and write the result to the
# destination buffer.  The implementation differs slightly as it takes advantage
# of the property vaesenclast(key, a) ^ b == vaesenclast(key ^ b, a) to reduce
# latency, but it has the same effect.
sub _aesenclast_and_xor_4x {
    my (
        $src,      $dst,      $rndkeylast, $aesdata0,
        $aesdata1, $aesdata2, $aesdata3,   $t0,
        $t1,       $t2,       $t3
    ) = @_;
    return <<___;
    vpxor           0*32($src), $rndkeylast, $t0
    vpxor           1*32($src), $rndkeylast, $t1
    vpxor           2*32($src), $rndkeylast, $t2
    vpxor           3*32($src), $rndkeylast, $t3
    vaesenclast     $t0, $aesdata0, $aesdata0
    vaesenclast     $t1, $aesdata1, $aesdata1
    vaesenclast     $t2, $aesdata2, $aesdata2
    vaesenclast     $t3, $aesdata3, $aesdata3
    vmovdqu         $aesdata0, 0*32($dst)
    vmovdqu         $aesdata1, 1*32($dst)
    vmovdqu         $aesdata2, 2*32($dst)
    vmovdqu         $aesdata3, 3*32($dst)
___
}

my $g_update_macro_expansion_count = 0;

# void aes_gcm_{enc,dec}_update_vaes_avx2(const uint8_t *in, uint8_t *out,
#                                         size_t len, const AES_KEY *key,
#                                         const uint8_t ivec[16],
#                                         const u128 Htable[16],
#                                         uint8_t Xi[16]);
#
# This macro generates a GCM encryption or decryption update function with the
# above prototype (with \enc selecting which one).  The function computes the
# next portion of the CTR keystream, XOR's it with |len| bytes from |in|, and
# writes the resulting encrypted or decrypted data to |out|.  It also updates
# the GHASH accumulator |Xi| using the next |len| ciphertext bytes.
#
# |len| must be a multiple of 16.  The caller must do any buffering needed to
# ensure this.  Both in-place and out-of-place en/decryption are supported.
#
# |ivec| must give the current counter in big-endian format.  This function
# loads the counter from |ivec| and increments the loaded counter as needed, but
# it does *not* store the updated counter back to |ivec|.  The caller must
# update |ivec| if any more data segments follow.  Internally, only the low
# 32-bit word of the counter is incremented, following the GCM standard.
sub _aes_gcm_update {
    my $local_label_suffix = "__func" . ++$g_update_macro_expansion_count;
    my ($enc)              = @_;
    my $code               = "";

    # Function arguments
    my ( $SRC, $DST, $DATALEN, $AESKEY, $BE_CTR_PTR, $HTABLE, $GHASH_ACC_PTR )
      = $win64
      ? ( @argregs[ 0 .. 3 ], "%rsi", "%rdi", "%r12" )
      : ( @argregs[ 0 .. 5 ], "%r12" );

    # Additional local variables.
    # %rax is used as a temporary register.  BE_CTR_PTR is also available as a
    # temporary register after the counter is loaded.

    # AES key length in bytes
    my ( $AESKEYLEN, $AESKEYLEN64 ) = ( "%r10d", "%r10" );

    # Pointer to the last AES round key for the chosen AES variant
    my $RNDKEYLAST_PTR = "%r11";

    # BSWAP_MASK is the shuffle mask for byte-reflecting 128-bit values
    # using vpshufb, copied to all 128-bit lanes.
    my ( $BSWAP_MASK, $BSWAP_MASK_XMM ) = ( "%ymm0", "%xmm0" );

    # GHASH_ACC is the accumulator variable for GHASH.  When fully reduced,
    # only the lowest 128-bit lane can be nonzero.  When not fully reduced,
    # more than one lane may be used, and they need to be XOR'd together.
    my ( $GHASH_ACC, $GHASH_ACC_XMM ) = ( "%ymm1", "%xmm1" );

    # TMP[0-2] are temporary registers.
    my ( $TMP0, $TMP0_XMM ) = ( "%ymm2", "%xmm2" );
    my ( $TMP1, $TMP1_XMM ) = ( "%ymm3", "%xmm3" );
    my ( $TMP2, $TMP2_XMM ) = ( "%ymm4", "%xmm4" );

    # LO and MI are used to accumulate unreduced GHASH products.
    my ( $LO, $LO_XMM ) = ( "%ymm5", "%xmm5" );
    my ( $MI, $MI_XMM ) = ( "%ymm6", "%xmm6" );

    # Cached key powers from Htable
    my ( $H_POW2_XORED, $H_POW2_XORED_XMM ) = ( "%ymm7", "%xmm7" );
    my ( $H_POW1_XORED, $H_POW1_XORED_XMM ) = ( "%ymm8", "%xmm8" );

    # RNDKEY0 caches the zero-th round key, and RNDKEYLAST the last one.
    my $RNDKEY0    = "%ymm9";
    my $RNDKEYLAST = "%ymm10";

    # LE_CTR contains the next set of little-endian counter blocks.
    my $LE_CTR = "%ymm11";

    # AESDATA[0-3] hold the counter blocks that are being encrypted by AES.
    my ( $AESDATA0, $AESDATA0_XMM ) = ( "%ymm12", "%xmm12" );
    my ( $AESDATA1, $AESDATA1_XMM ) = ( "%ymm13", "%xmm13" );
    my ( $AESDATA2, $AESDATA2_XMM ) = ( "%ymm14", "%xmm14" );
    my ( $AESDATA3, $AESDATA3_XMM ) = ( "%ymm15", "%xmm15" );
    my @AESDATA = ( $AESDATA0, $AESDATA1, $AESDATA2, $AESDATA3 );

    my @ghash_4x_args = (
        $enc ? $DST : $SRC, $HTABLE, $BSWAP_MASK, $H_POW2_XORED,
        $H_POW1_XORED,      $TMP0,   $TMP0_XMM,   $TMP1,
        $TMP2,              $LO,     $MI,         $GHASH_ACC,
        $GHASH_ACC_XMM
    );

    if ($win64) {
        $code .= <<___;
        @{[ _save_gpregs $BE_CTR_PTR, $HTABLE, $GHASH_ACC_PTR ]}
        mov             64(%rsp), $BE_CTR_PTR     # arg5
        mov             72(%rsp), $HTABLE         # arg6
        mov             80(%rsp), $GHASH_ACC_PTR  # arg7
        @{[ _save_xmmregs (6 .. 15) ]}
        .seh_endprologue
___
    }
    else {
        $code .= <<___;
        @{[ _save_gpregs $GHASH_ACC_PTR ]}
        mov             16(%rsp), $GHASH_ACC_PTR  # arg7
___
    }

    if ($enc) {
        $code .= <<___;
#ifdef BORINGSSL_DISPATCH_TEST
        .extern BORINGSSL_function_hit
        movb \$1,BORINGSSL_function_hit+8(%rip)
#endif
___
    }
    $code .= <<___;
    vbroadcasti128  .Lbswap_mask(%rip), $BSWAP_MASK

    # Load the GHASH accumulator and the starting counter.
    # BoringSSL passes these values in big endian format.
    vmovdqu         ($GHASH_ACC_PTR), $GHASH_ACC_XMM
    vpshufb         $BSWAP_MASK_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM
    vbroadcasti128  ($BE_CTR_PTR), $LE_CTR
    vpshufb         $BSWAP_MASK, $LE_CTR, $LE_CTR

    # Load the AES key length in bytes.  BoringSSL stores number of rounds
    # minus 1, so convert using: AESKEYLEN = 4 * aeskey->rounds - 20.
    movl            $OFFSETOF_AES_ROUNDS($AESKEY), $AESKEYLEN
    lea             -20(,$AESKEYLEN,4), $AESKEYLEN

    # Make RNDKEYLAST_PTR point to the last AES round key.  This is the
    # round key with index 10, 12, or 14 for AES-128, AES-192, or AES-256
    # respectively.  Then load the zero-th and last round keys.
    lea             6*16($AESKEY,$AESKEYLEN64,4), $RNDKEYLAST_PTR
    vbroadcasti128  ($AESKEY), $RNDKEY0
    vbroadcasti128  ($RNDKEYLAST_PTR), $RNDKEYLAST

    # Finish initializing LE_CTR by adding 1 to the second block.
    vpaddd          .Lctr_pattern(%rip), $LE_CTR, $LE_CTR

    # If there are at least 128 bytes of data, then continue into the loop that
    # processes 128 bytes of data at a time.  Otherwise skip it.
    cmp             \$127, $DATALEN
    jbe             .Lcrypt_loop_4x_done$local_label_suffix

    vmovdqu         $OFFSETOF_H_POWERS_XORED($HTABLE), $H_POW2_XORED
    vmovdqu         $OFFSETOF_H_POWERS_XORED+32($HTABLE), $H_POW1_XORED
___

    # Main loop: en/decrypt and hash 4 vectors (128 bytes) at a time.

    if ($enc) {
        $code .= <<___;
        # Encrypt the first 4 vectors of plaintext blocks.
        @{[ _ctr_begin_4x $LE_CTR, $BSWAP_MASK, $RNDKEY0, @AESDATA, $TMP0 ]}
        lea             16($AESKEY), %rax
.Lvaesenc_loop_first_4_vecs$local_label_suffix:
        vbroadcasti128  (%rax), $TMP0
        @{[ _vaesenc_4x $TMP0, @AESDATA ]}
        add             \$16, %rax
        cmp             %rax, $RNDKEYLAST_PTR
        jne             .Lvaesenc_loop_first_4_vecs$local_label_suffix
        @{[ _aesenclast_and_xor_4x $SRC, $DST, $RNDKEYLAST, @AESDATA,
                                   $TMP0, $TMP1, $LO, $MI ]}
        sub             \$-128, $SRC  # 128 is 4 bytes, -128 is 1 byte
        add             \$-128, $DATALEN
        cmp             \$127, $DATALEN
        jbe             .Lghash_last_ciphertext_4x$local_label_suffix
___
    }

    $code .= <<___;
.align 16
.Lcrypt_loop_4x$local_label_suffix:

    # Start the AES encryption of the counter blocks.
    @{[ _ctr_begin_4x $LE_CTR, $BSWAP_MASK, $RNDKEY0, @AESDATA, $TMP0 ]}
    cmp             \$24, $AESKEYLEN
    jl              .Laes128$local_label_suffix
    je              .Laes192$local_label_suffix
    # AES-256
    vbroadcasti128 -13*16($RNDKEYLAST_PTR), $TMP0
    @{[ _vaesenc_4x $TMP0, @AESDATA ]}
    vbroadcasti128 -12*16($RNDKEYLAST_PTR), $TMP0
    @{[ _vaesenc_4x $TMP0, @AESDATA ]}
.Laes192$local_label_suffix:
    vbroadcasti128 -11*16($RNDKEYLAST_PTR), $TMP0
    @{[ _vaesenc_4x $TMP0, @AESDATA ]}
    vbroadcasti128 -10*16($RNDKEYLAST_PTR), $TMP0
    @{[ _vaesenc_4x $TMP0, @AESDATA ]}
.Laes128$local_label_suffix:
___

    # Prefetch the source data 512 bytes ahead into the L1 data cache, to
    # improve performance when the hardware prefetcher is disabled.  Assumes the
    # L1 data cache line size is 64 bytes (de facto standard on x86_64).
    $code .= "prefetcht0 512($SRC)\n";
    $code .= "prefetcht0 512+64($SRC)\n";

    # Finish the AES encryption of the counter blocks in AESDATA[0-3],
    # interleaved with the GHASH update of the ciphertext blocks.
    for my $i ( reverse 1 .. 9 ) {
        $code .= <<___;
        @{[ _ghash_step_4x 9-$i, @ghash_4x_args ]}
        vbroadcasti128  -$i*16($RNDKEYLAST_PTR), $TMP0
        @{[ _vaesenc_4x $TMP0, @AESDATA ]}
___
    }
    $code .= <<___;
    @{[ _ghash_step_4x 9, @ghash_4x_args ]}

    @{[ $enc ? "sub \$-128, $DST" : "" ]}  # 128 is 4 bytes, -128 is 1 byte
    @{[ _aesenclast_and_xor_4x $SRC, $DST, $RNDKEYLAST, @AESDATA,
                               $TMP0, $TMP1, $LO, $MI ]}
    sub             \$-128, $SRC
    @{[ !$enc ? "sub \$-128, $DST" : "" ]}
    add             \$-128, $DATALEN
    cmp             \$127, $DATALEN
    ja              .Lcrypt_loop_4x$local_label_suffix
___

    if ($enc) {

        # Update GHASH with the last set of ciphertext blocks.
        $code .= <<___;
.Lghash_last_ciphertext_4x$local_label_suffix:
        @{[ _ghash_4x @ghash_4x_args ]}
        sub             \$-128, $DST
___
    }

    my $POWERS_PTR = $BE_CTR_PTR;    # BE_CTR_PTR is free to be reused.
    my ( $HI, $HI_XMM ) = ( $H_POW2_XORED, $H_POW2_XORED_XMM );    # reuse

    $code .= <<___;
.Lcrypt_loop_4x_done$local_label_suffix:
    # Check whether any data remains.
    test            $DATALEN, $DATALEN
    jz              .Ldone$local_label_suffix

    # DATALEN is in [16, 32, 48, 64, 80, 96, 112].

    # Make POWERS_PTR point to the key powers [H^N, H^(N-1), ...] where N
    # is the number of blocks that remain.
    lea             $OFFSETOFEND_H_POWERS($HTABLE), $POWERS_PTR
    sub             $DATALEN, $POWERS_PTR

    # Start collecting the unreduced GHASH intermediate value LO, MI, HI.
    vpxor           $LO_XMM, $LO_XMM, $LO_XMM
    vpxor           $MI_XMM, $MI_XMM, $MI_XMM
    vpxor           $HI_XMM, $HI_XMM, $HI_XMM

    cmp             \$64, $DATALEN
    jb              .Llessthan64bytes$local_label_suffix

    # DATALEN is in [64, 80, 96, 112].  Encrypt two vectors of counter blocks.
    vpshufb         $BSWAP_MASK, $LE_CTR, $AESDATA0
    vpaddd          .Linc_2blocks(%rip), $LE_CTR, $LE_CTR
    vpshufb         $BSWAP_MASK, $LE_CTR, $AESDATA1
    vpaddd          .Linc_2blocks(%rip), $LE_CTR, $LE_CTR
    vpxor           $RNDKEY0, $AESDATA0, $AESDATA0
    vpxor           $RNDKEY0, $AESDATA1, $AESDATA1
    lea             16($AESKEY), %rax
.Lvaesenc_loop_tail_1$local_label_suffix:
    vbroadcasti128  (%rax), $TMP0
    vaesenc         $TMP0, $AESDATA0, $AESDATA0
    vaesenc         $TMP0, $AESDATA1, $AESDATA1
    add             \$16, %rax
    cmp             %rax, $RNDKEYLAST_PTR
    jne             .Lvaesenc_loop_tail_1$local_label_suffix
    vaesenclast     $RNDKEYLAST, $AESDATA0, $AESDATA0
    vaesenclast     $RNDKEYLAST, $AESDATA1, $AESDATA1

    # XOR the data with the two vectors of keystream blocks.
    vmovdqu         0($SRC), $TMP0
    vmovdqu         32($SRC), $TMP1
    vpxor           $TMP0, $AESDATA0, $AESDATA0
    vpxor           $TMP1, $AESDATA1, $AESDATA1
    vmovdqu         $AESDATA0, 0($DST)
    vmovdqu         $AESDATA1, 32($DST)

    # Update GHASH with two vectors of ciphertext blocks, without reducing.
    vpshufb         $BSWAP_MASK, @{[ $enc ? $AESDATA0 : $TMP0 ]}, $AESDATA0
    vpshufb         $BSWAP_MASK, @{[ $enc ? $AESDATA1 : $TMP1 ]}, $AESDATA1
    vpxor           $GHASH_ACC, $AESDATA0, $AESDATA0
    vmovdqu         ($POWERS_PTR), $TMP0
    vmovdqu         32($POWERS_PTR), $TMP1
    vpclmulqdq      \$0x00, $TMP0, $AESDATA0, $LO
    vpclmulqdq      \$0x01, $TMP0, $AESDATA0, $MI
    vpclmulqdq      \$0x10, $TMP0, $AESDATA0, $TMP2
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x11, $TMP0, $AESDATA0, $HI
    vpclmulqdq      \$0x00, $TMP1, $AESDATA1, $TMP2
    vpxor           $TMP2, $LO, $LO
    vpclmulqdq      \$0x01, $TMP1, $AESDATA1, $TMP2
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x10, $TMP1, $AESDATA1, $TMP2
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x11, $TMP1, $AESDATA1, $TMP2
    vpxor           $TMP2, $HI, $HI

    add             \$64, $POWERS_PTR
    add             \$64, $SRC
    add             \$64, $DST
    sub             \$64, $DATALEN
    jz              .Lreduce$local_label_suffix

    vpxor           $GHASH_ACC_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM

    # DATALEN is in [16, 32, 48].  Encrypt two last vectors of counter blocks.
.Llessthan64bytes$local_label_suffix:
    vpshufb         $BSWAP_MASK, $LE_CTR, $AESDATA0
    vpaddd          .Linc_2blocks(%rip), $LE_CTR, $LE_CTR
    vpshufb         $BSWAP_MASK, $LE_CTR, $AESDATA1
    vpxor           $RNDKEY0, $AESDATA0, $AESDATA0
    vpxor           $RNDKEY0, $AESDATA1, $AESDATA1
    lea             16($AESKEY), %rax
.Lvaesenc_loop_tail_2$local_label_suffix:
    vbroadcasti128  (%rax), $TMP0
    vaesenc         $TMP0, $AESDATA0, $AESDATA0
    vaesenc         $TMP0, $AESDATA1, $AESDATA1
    add             \$16, %rax
    cmp             %rax, $RNDKEYLAST_PTR
    jne             .Lvaesenc_loop_tail_2$local_label_suffix
    vaesenclast     $RNDKEYLAST, $AESDATA0, $AESDATA0
    vaesenclast     $RNDKEYLAST, $AESDATA1, $AESDATA1

    # XOR the remaining data with the keystream blocks, and update GHASH with
    # the remaining ciphertext blocks without reducing.

    cmp             \$32, $DATALEN
    jb              .Lxor_one_block$local_label_suffix
    je              .Lxor_two_blocks$local_label_suffix

.Lxor_three_blocks$local_label_suffix:
    vmovdqu         0($SRC), $TMP0
    vmovdqu         32($SRC), $TMP1_XMM
    vpxor           $TMP0, $AESDATA0, $AESDATA0
    vpxor           $TMP1_XMM, $AESDATA1_XMM, $AESDATA1_XMM
    vmovdqu         $AESDATA0, 0($DST)
    vmovdqu         $AESDATA1_XMM, 32($DST)

    vpshufb         $BSWAP_MASK, @{[ $enc ? $AESDATA0 : $TMP0 ]}, $AESDATA0
    vpshufb         $BSWAP_MASK_XMM, @{[ $enc ? $AESDATA1_XMM : $TMP1_XMM ]}, $AESDATA1_XMM
    vpxor           $GHASH_ACC, $AESDATA0, $AESDATA0
    vmovdqu         ($POWERS_PTR), $TMP0
    vmovdqu         32($POWERS_PTR), $TMP1_XMM
    vpclmulqdq      \$0x00, $TMP1_XMM, $AESDATA1_XMM, $TMP2_XMM
    vpxor           $TMP2, $LO, $LO
    vpclmulqdq      \$0x01, $TMP1_XMM, $AESDATA1_XMM, $TMP2_XMM
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x10, $TMP1_XMM, $AESDATA1_XMM, $TMP2_XMM
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x11, $TMP1_XMM, $AESDATA1_XMM, $TMP2_XMM
    vpxor           $TMP2, $HI, $HI
    jmp             .Lghash_mul_one_vec_unreduced$local_label_suffix

.Lxor_two_blocks$local_label_suffix:
    vmovdqu         ($SRC), $TMP0
    vpxor           $TMP0, $AESDATA0, $AESDATA0
    vmovdqu         $AESDATA0, ($DST)
    vpshufb         $BSWAP_MASK, @{[ $enc ? $AESDATA0 : $TMP0 ]}, $AESDATA0
    vpxor           $GHASH_ACC, $AESDATA0, $AESDATA0
    vmovdqu         ($POWERS_PTR), $TMP0
    jmp             .Lghash_mul_one_vec_unreduced$local_label_suffix

.Lxor_one_block$local_label_suffix:
    vmovdqu         ($SRC), $TMP0_XMM
    vpxor           $TMP0_XMM, $AESDATA0_XMM, $AESDATA0_XMM
    vmovdqu         $AESDATA0_XMM, ($DST)
    vpshufb         $BSWAP_MASK_XMM, @{[ $enc ? $AESDATA0_XMM : $TMP0_XMM ]}, $AESDATA0_XMM
    vpxor           $GHASH_ACC_XMM, $AESDATA0_XMM, $AESDATA0_XMM
    vmovdqu         ($POWERS_PTR), $TMP0_XMM

.Lghash_mul_one_vec_unreduced$local_label_suffix:
    vpclmulqdq      \$0x00, $TMP0, $AESDATA0, $TMP2
    vpxor           $TMP2, $LO, $LO
    vpclmulqdq      \$0x01, $TMP0, $AESDATA0, $TMP2
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x10, $TMP0, $AESDATA0, $TMP2
    vpxor           $TMP2, $MI, $MI
    vpclmulqdq      \$0x11, $TMP0, $AESDATA0, $TMP2
    vpxor           $TMP2, $HI, $HI

.Lreduce$local_label_suffix:
    # Finally, do the GHASH reduction.
    vbroadcasti128  .Lgfpoly(%rip), $TMP0
    vpclmulqdq      \$0x01, $LO, $TMP0, $TMP1
    vpshufd         \$0x4e, $LO, $LO
    vpxor           $LO, $MI, $MI
    vpxor           $TMP1, $MI, $MI
    vpclmulqdq      \$0x01, $MI, $TMP0, $TMP1
    vpshufd         \$0x4e, $MI, $MI
    vpxor           $MI, $HI, $HI
    vpxor           $TMP1, $HI, $HI
    vextracti128    \$1, $HI, $GHASH_ACC_XMM
    vpxor           $HI_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM

.Ldone$local_label_suffix:
    # Store the updated GHASH accumulator back to memory.
    vpshufb         $BSWAP_MASK_XMM, $GHASH_ACC_XMM, $GHASH_ACC_XMM
    vmovdqu         $GHASH_ACC_XMM, ($GHASH_ACC_PTR)

    vzeroupper
___
    return $code;
}

$code .= _begin_func "aes_gcm_enc_update_vaes_avx2", 1;
$code .= _aes_gcm_update 1;
$code .= _end_func;

$code .= _begin_func "aes_gcm_dec_update_vaes_avx2", 1;
$code .= _aes_gcm_update 0;
$code .= _end_func;

sub filter_and_print {
    # This function replaces AVX2 assembly instructions with their assembled forms,
    # to allow the code to work on old versions of binutils (older than 2.30) that do
    # not support these instructions.
    my %asmMap = (
        'vaesenc         %ymm2, %ymm12, %ymm12' => '.byte 0xc4,0x62,0x1d,0xdc,0xe2',
        'vaesenc         %ymm2, %ymm13, %ymm13' => '.byte 0xc4,0x62,0x15,0xdc,0xea',
        'vaesenc         %ymm2, %ymm14, %ymm14' => '.byte 0xc4,0x62,0x0d,0xdc,0xf2',
        'vaesenc         %ymm2, %ymm15, %ymm15' => '.byte 0xc4,0x62,0x05,0xdc,0xfa',
        'vaesenclast     %ymm10, %ymm12, %ymm12' => '.byte 0xc4,0x42,0x1d,0xdd,0xe2',
        'vaesenclast     %ymm10, %ymm13, %ymm13' => '.byte 0xc4,0x42,0x15,0xdd,0xea',
        'vaesenclast     %ymm2, %ymm12, %ymm12' => '.byte 0xc4,0x62,0x1d,0xdd,0xe2',
        'vaesenclast     %ymm3, %ymm13, %ymm13' => '.byte 0xc4,0x62,0x15,0xdd,0xeb',
        'vaesenclast     %ymm5, %ymm14, %ymm14' => '.byte 0xc4,0x62,0x0d,0xdd,0xf5',
        'vaesenclast     %ymm6, %ymm15, %ymm15' => '.byte 0xc4,0x62,0x05,0xdd,0xfe',
        'vpclmulqdq      $0x00, %ymm2, %ymm12, %ymm4' => '.byte 0xc4,0xe3,0x1d,0x44,0xe2,0x00',
        'vpclmulqdq      $0x00, %ymm2, %ymm12, %ymm5' => '.byte 0xc4,0xe3,0x1d,0x44,0xea,0x00',
        'vpclmulqdq      $0x00, %ymm3, %ymm13, %ymm4' => '.byte 0xc4,0xe3,0x15,0x44,0xe3,0x00',
        'vpclmulqdq      $0x00, %ymm4, %ymm3, %ymm2' => '.byte 0xc4,0xe3,0x65,0x44,0xd4,0x00',
        'vpclmulqdq      $0x00, %ymm4, %ymm3, %ymm5' => '.byte 0xc4,0xe3,0x65,0x44,0xec,0x00',
        'vpclmulqdq      $0x00, %ymm5, %ymm3, %ymm0' => '.byte 0xc4,0xe3,0x65,0x44,0xc5,0x00',
        'vpclmulqdq      $0x00, %ymm5, %ymm4, %ymm0' => '.byte 0xc4,0xe3,0x5d,0x44,0xc5,0x00',
        'vpclmulqdq      $0x00, %ymm7, %ymm2, %ymm6' => '.byte 0xc4,0xe3,0x6d,0x44,0xf7,0x00',
        'vpclmulqdq      $0x00, %ymm8, %ymm2, %ymm2' => '.byte 0xc4,0xc3,0x6d,0x44,0xd0,0x00',
        'vpclmulqdq      $0x01, %ymm0, %ymm6, %ymm2' => '.byte 0xc4,0xe3,0x4d,0x44,0xd0,0x01',
        'vpclmulqdq      $0x01, %ymm1, %ymm6, %ymm0' => '.byte 0xc4,0xe3,0x4d,0x44,0xc1,0x01',
        'vpclmulqdq      $0x01, %ymm2, %ymm12, %ymm4' => '.byte 0xc4,0xe3,0x1d,0x44,0xe2,0x01',
        'vpclmulqdq      $0x01, %ymm2, %ymm12, %ymm6' => '.byte 0xc4,0xe3,0x1d,0x44,0xf2,0x01',
        'vpclmulqdq      $0x01, %ymm3, %ymm13, %ymm4' => '.byte 0xc4,0xe3,0x15,0x44,0xe3,0x01',
        'vpclmulqdq      $0x01, %ymm5, %ymm2, %ymm3' => '.byte 0xc4,0xe3,0x6d,0x44,0xdd,0x01',
        'vpclmulqdq      $0x01, %ymm5, %ymm3, %ymm1' => '.byte 0xc4,0xe3,0x65,0x44,0xcd,0x01',
        'vpclmulqdq      $0x01, %ymm5, %ymm4, %ymm1' => '.byte 0xc4,0xe3,0x5d,0x44,0xcd,0x01',
        'vpclmulqdq      $0x01, %ymm5, %ymm4, %ymm2' => '.byte 0xc4,0xe3,0x5d,0x44,0xd5,0x01',
        'vpclmulqdq      $0x01, %ymm6, %ymm2, %ymm3' => '.byte 0xc4,0xe3,0x6d,0x44,0xde,0x01',
        'vpclmulqdq      $0x01, %ymm6, %ymm4, %ymm2' => '.byte 0xc4,0xe3,0x5d,0x44,0xd6,0x01',
        'vpclmulqdq      $0x10, %ymm2, %ymm12, %ymm4' => '.byte 0xc4,0xe3,0x1d,0x44,0xe2,0x10',
        'vpclmulqdq      $0x10, %ymm3, %ymm13, %ymm4' => '.byte 0xc4,0xe3,0x15,0x44,0xe3,0x10',
        'vpclmulqdq      $0x10, %ymm5, %ymm3, %ymm2' => '.byte 0xc4,0xe3,0x65,0x44,0xd5,0x10',
        'vpclmulqdq      $0x10, %ymm5, %ymm4, %ymm2' => '.byte 0xc4,0xe3,0x5d,0x44,0xd5,0x10',
        'vpclmulqdq      $0x10, %ymm7, %ymm2, %ymm2' => '.byte 0xc4,0xe3,0x6d,0x44,0xd7,0x10',
        'vpclmulqdq      $0x10, %ymm8, %ymm2, %ymm2' => '.byte 0xc4,0xc3,0x6d,0x44,0xd0,0x10',
        'vpclmulqdq      $0x11, %ymm2, %ymm12, %ymm4' => '.byte 0xc4,0xe3,0x1d,0x44,0xe2,0x11',
        'vpclmulqdq      $0x11, %ymm2, %ymm12, %ymm7' => '.byte 0xc4,0xe3,0x1d,0x44,0xfa,0x11',
        'vpclmulqdq      $0x11, %ymm3, %ymm13, %ymm4' => '.byte 0xc4,0xe3,0x15,0x44,0xe3,0x11',
        'vpclmulqdq      $0x11, %ymm4, %ymm3, %ymm1' => '.byte 0xc4,0xe3,0x65,0x44,0xcc,0x11',
        'vpclmulqdq      $0x11, %ymm4, %ymm3, %ymm2' => '.byte 0xc4,0xe3,0x65,0x44,0xd4,0x11',
        'vpclmulqdq      $0x11, %ymm5, %ymm3, %ymm4' => '.byte 0xc4,0xe3,0x65,0x44,0xe5,0x11',
        'vpclmulqdq      $0x11, %ymm5, %ymm4, %ymm3' => '.byte 0xc4,0xe3,0x5d,0x44,0xdd,0x11',
    );
    for my $line (split("\n",$code)) {
        my $trimmed;
        $trimmed = $line;
        $trimmed =~ s/^\s+//;
        $trimmed =~ s/\s+(#.*)?$//;
        if (exists $asmMap{$trimmed}) {
            $line = $asmMap{$trimmed};
        } else {
            if($trimmed =~ /(vpclmulqdq|vaes).*%[yz]mm/) {
                die ("found instruction not supported under old binutils, please update asmMap with the results of running\n" .
                     'find target -name "*aes-gcm-avx2*.o" -exec python3 crypto/fipsmodule/aes/asm/make-avx-map-for-old-binutils.py \{\} \; | LC_ALL=C sort | uniq');
            }
        }
        print $line,"\n";
    }
}

filter_and_print();

close STDOUT or die "error closing STDOUT: $!";
exit 0;
