#!/usr/bin/env perl
# Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

my $output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;
my $xlate;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; my $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate  ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate ) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

my $code = data();
print $code;

close STDOUT or die "error closing STDOUT: $!"; # enforce flush

sub data
{
    local $/;
    return <DATA>;
}

__END__
// Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
//
// Licensed under the OpenSSL license (the "License").  You may not use
// this file except in compliance with the License.  You can obtain a copy
// in the file LICENSE in the source distribution or at
// https://www.openssl.org/source/license.html
//
// ====================================================================
// Written by Ben Avison <bavison@riscosopen.org> for the OpenSSL
// project. Rights for redistribution and usage in source and binary
// forms are granted according to the OpenSSL license.
// ====================================================================
//
// This implementation is a translation of bsaes-armv7 for AArch64.
// No attempt has been made to carry across the build switches for
// kernel targets, since the Linux kernel crypto support has moved on
// from when it was based on OpenSSL.

// A lot of hand-scheduling has been performed. Consequently, this code
// doesn't factor out neatly into macros in the same way that the
// AArch32 version did, and there is little to be gained by wrapping it
// up in Perl, and it is presented as pure assembly.


#include "crypto/arm_arch.h"

.text

.extern AES_cbc_encrypt
.extern AES_encrypt
.extern AES_decrypt

.type   _bsaes_decrypt8,%function
.align  4
// On entry:
//   x9 -> key (previously expanded using _bsaes_key_convert)
//   x10 = number of rounds
//   v0-v7 input data
// On exit:
//   x9-x11 corrupted
//   other general-purpose registers preserved
//   v0-v7 output data
//   v11-v15 preserved
//   other SIMD registers corrupted
_bsaes_decrypt8:
        ldr     q8, [x9], #16
        adrp    x11, .LM0ISR
        add     x11, x11, #:lo12:.LM0ISR
        movi    v9.16b, #0x55
        ldr     q10, [x11], #16
        movi    v16.16b, #0x33
        movi    v17.16b, #0x0f
        sub     x10, x10, #1
        eor     v0.16b, v0.16b, v8.16b
        eor     v1.16b, v1.16b, v8.16b
        eor     v2.16b, v2.16b, v8.16b
        eor     v4.16b, v4.16b, v8.16b
        eor     v3.16b, v3.16b, v8.16b
        eor     v5.16b, v5.16b, v8.16b
        tbl     v0.16b, {v0.16b}, v10.16b
        tbl     v1.16b, {v1.16b}, v10.16b
        tbl     v2.16b, {v2.16b}, v10.16b
        tbl     v4.16b, {v4.16b}, v10.16b
        eor     v6.16b, v6.16b, v8.16b
        eor     v7.16b, v7.16b, v8.16b
        tbl     v3.16b, {v3.16b}, v10.16b
        tbl     v5.16b, {v5.16b}, v10.16b
        tbl     v6.16b, {v6.16b}, v10.16b
        ushr    v8.2d, v0.2d, #1
        tbl     v7.16b, {v7.16b}, v10.16b
        ushr    v10.2d, v4.2d, #1
        ushr    v18.2d, v2.2d, #1
        eor     v8.16b, v8.16b, v1.16b
        ushr    v19.2d, v6.2d, #1
        eor     v10.16b, v10.16b, v5.16b
        eor     v18.16b, v18.16b, v3.16b
        and     v8.16b, v8.16b, v9.16b
        eor     v19.16b, v19.16b, v7.16b
        and     v10.16b, v10.16b, v9.16b
        and     v18.16b, v18.16b, v9.16b
        eor     v1.16b, v1.16b, v8.16b
        shl     v8.2d, v8.2d, #1
        and     v9.16b, v19.16b, v9.16b
        eor     v5.16b, v5.16b, v10.16b
        shl     v10.2d, v10.2d, #1
        eor     v3.16b, v3.16b, v18.16b
        shl     v18.2d, v18.2d, #1
        eor     v0.16b, v0.16b, v8.16b
        shl     v8.2d, v9.2d, #1
        eor     v7.16b, v7.16b, v9.16b
        eor     v4.16b, v4.16b, v10.16b
        eor     v2.16b, v2.16b, v18.16b
        ushr    v9.2d, v1.2d, #2
        eor     v6.16b, v6.16b, v8.16b
        ushr    v8.2d, v0.2d, #2
        ushr    v10.2d, v5.2d, #2
        ushr    v18.2d, v4.2d, #2
        eor     v9.16b, v9.16b, v3.16b
        eor     v8.16b, v8.16b, v2.16b
        eor     v10.16b, v10.16b, v7.16b
        eor     v18.16b, v18.16b, v6.16b
        and     v9.16b, v9.16b, v16.16b
        and     v8.16b, v8.16b, v16.16b
        and     v10.16b, v10.16b, v16.16b
        and     v16.16b, v18.16b, v16.16b
        eor     v3.16b, v3.16b, v9.16b
        shl     v9.2d, v9.2d, #2
        eor     v2.16b, v2.16b, v8.16b
        shl     v8.2d, v8.2d, #2
        eor     v7.16b, v7.16b, v10.16b
        shl     v10.2d, v10.2d, #2
        eor     v6.16b, v6.16b, v16.16b
        shl     v16.2d, v16.2d, #2
        eor     v1.16b, v1.16b, v9.16b
        eor     v0.16b, v0.16b, v8.16b
        eor     v5.16b, v5.16b, v10.16b
        eor     v4.16b, v4.16b, v16.16b
        ushr    v8.2d, v3.2d, #4
        ushr    v9.2d, v2.2d, #4
        ushr    v10.2d, v1.2d, #4
        ushr    v16.2d, v0.2d, #4
        eor     v8.16b, v8.16b, v7.16b
        eor     v9.16b, v9.16b, v6.16b
        eor     v10.16b, v10.16b, v5.16b
        eor     v16.16b, v16.16b, v4.16b
        and     v8.16b, v8.16b, v17.16b
        and     v9.16b, v9.16b, v17.16b
        and     v10.16b, v10.16b, v17.16b
        and     v16.16b, v16.16b, v17.16b
        eor     v7.16b, v7.16b, v8.16b
        shl     v8.2d, v8.2d, #4
        eor     v6.16b, v6.16b, v9.16b
        shl     v9.2d, v9.2d, #4
        eor     v5.16b, v5.16b, v10.16b
        shl     v10.2d, v10.2d, #4
        eor     v4.16b, v4.16b, v16.16b
        shl     v16.2d, v16.2d, #4
        eor     v3.16b, v3.16b, v8.16b
        eor     v2.16b, v2.16b, v9.16b
        eor     v1.16b, v1.16b, v10.16b
        eor     v0.16b, v0.16b, v16.16b
        b       .Ldec_sbox
.align  4
.Ldec_loop:
        ld1     {v16.16b, v17.16b, v18.16b, v19.16b}, [x9], #64
        ldp     q8, q9, [x9], #32
        eor     v0.16b, v16.16b, v0.16b
        ldr     q10, [x9], #16
        eor     v1.16b, v17.16b, v1.16b
        ldr     q16, [x9], #16
        eor     v2.16b, v18.16b, v2.16b
        eor     v3.16b, v19.16b, v3.16b
        eor     v4.16b, v8.16b, v4.16b
        eor     v5.16b, v9.16b, v5.16b
        eor     v6.16b, v10.16b, v6.16b
        eor     v7.16b, v16.16b, v7.16b
        tbl     v0.16b, {v0.16b}, v28.16b
        tbl     v1.16b, {v1.16b}, v28.16b
        tbl     v2.16b, {v2.16b}, v28.16b
        tbl     v3.16b, {v3.16b}, v28.16b
        tbl     v4.16b, {v4.16b}, v28.16b
        tbl     v5.16b, {v5.16b}, v28.16b
        tbl     v6.16b, {v6.16b}, v28.16b
        tbl     v7.16b, {v7.16b}, v28.16b
.Ldec_sbox:
        eor     v1.16b, v1.16b, v4.16b
        eor     v3.16b, v3.16b, v4.16b
        subs    x10, x10, #1
        eor     v4.16b, v4.16b, v7.16b
        eor     v2.16b, v2.16b, v7.16b
        eor     v1.16b, v1.16b, v6.16b
        eor     v6.16b, v6.16b, v4.16b
        eor     v2.16b, v2.16b, v5.16b
        eor     v0.16b, v0.16b, v1.16b
        eor     v7.16b, v7.16b, v6.16b
        eor     v8.16b, v6.16b, v2.16b
        and     v9.16b, v4.16b, v6.16b
        eor     v10.16b, v2.16b, v6.16b
        eor     v3.16b, v3.16b, v0.16b
        eor     v5.16b, v5.16b, v0.16b
        eor     v16.16b, v7.16b, v4.16b
        eor     v17.16b, v4.16b, v0.16b
        and     v18.16b, v0.16b, v2.16b
        eor     v19.16b, v7.16b, v4.16b
        eor     v1.16b, v1.16b, v3.16b
        eor     v20.16b, v3.16b, v0.16b
        eor     v21.16b, v5.16b, v2.16b
        eor     v22.16b, v3.16b, v7.16b
        and     v8.16b, v17.16b, v8.16b
        orr     v17.16b, v3.16b, v5.16b
        eor     v23.16b, v1.16b, v6.16b
        eor     v24.16b, v20.16b, v16.16b
        eor     v25.16b, v1.16b, v5.16b
        orr     v26.16b, v20.16b, v21.16b
        and     v20.16b, v20.16b, v21.16b
        and     v27.16b, v7.16b, v1.16b
        eor     v21.16b, v21.16b, v23.16b
        orr     v28.16b, v16.16b, v23.16b
        orr     v29.16b, v22.16b, v25.16b
        eor     v26.16b, v26.16b, v8.16b
        and     v16.16b, v16.16b, v23.16b
        and     v22.16b, v22.16b, v25.16b
        and     v21.16b, v24.16b, v21.16b
        eor     v8.16b, v28.16b, v8.16b
        eor     v23.16b, v5.16b, v2.16b
        eor     v24.16b, v1.16b, v6.16b
        eor     v16.16b, v16.16b, v22.16b
        eor     v22.16b, v3.16b, v0.16b
        eor     v25.16b, v29.16b, v21.16b
        eor     v21.16b, v26.16b, v21.16b
        eor     v8.16b, v8.16b, v20.16b
        eor     v26.16b, v23.16b, v24.16b
        eor     v16.16b, v16.16b, v20.16b
        eor     v28.16b, v22.16b, v19.16b
        eor     v20.16b, v25.16b, v20.16b
        eor     v9.16b, v21.16b, v9.16b
        eor     v8.16b, v8.16b, v18.16b
        eor     v18.16b, v5.16b, v1.16b
        eor     v21.16b, v16.16b, v17.16b
        eor     v16.16b, v16.16b, v17.16b
        eor     v17.16b, v20.16b, v27.16b
        eor     v20.16b, v3.16b, v7.16b
        eor     v25.16b, v9.16b, v8.16b
        eor     v27.16b, v0.16b, v4.16b
        and     v29.16b, v9.16b, v17.16b
        eor     v30.16b, v8.16b, v29.16b
        eor     v31.16b, v21.16b, v29.16b
        eor     v29.16b, v21.16b, v29.16b
        bsl     v30.16b, v17.16b, v21.16b
        bsl     v31.16b, v9.16b, v8.16b
        bsl     v16.16b, v30.16b, v29.16b
        bsl     v21.16b, v29.16b, v30.16b
        eor     v8.16b, v31.16b, v30.16b
        and     v1.16b, v1.16b, v31.16b
        and     v9.16b, v16.16b, v31.16b
        and     v6.16b, v6.16b, v30.16b
        eor     v16.16b, v17.16b, v21.16b
        and     v4.16b, v4.16b, v30.16b
        eor     v17.16b, v8.16b, v30.16b
        and     v21.16b, v24.16b, v8.16b
        eor     v9.16b, v9.16b, v25.16b
        and     v19.16b, v19.16b, v8.16b
        eor     v24.16b, v30.16b, v16.16b
        eor     v25.16b, v30.16b, v16.16b
        and     v7.16b, v7.16b, v17.16b
        and     v10.16b, v10.16b, v16.16b
        eor     v29.16b, v9.16b, v16.16b
        eor     v30.16b, v31.16b, v9.16b
        and     v0.16b, v24.16b, v0.16b
        and     v9.16b, v18.16b, v9.16b
        and     v2.16b, v25.16b, v2.16b
        eor     v10.16b, v10.16b, v6.16b
        eor     v18.16b, v29.16b, v16.16b
        and     v5.16b, v30.16b, v5.16b
        eor     v24.16b, v8.16b, v29.16b
        and     v25.16b, v26.16b, v29.16b
        and     v26.16b, v28.16b, v29.16b
        eor     v8.16b, v8.16b, v29.16b
        eor     v17.16b, v17.16b, v18.16b
        eor     v5.16b, v1.16b, v5.16b
        and     v23.16b, v24.16b, v23.16b
        eor     v21.16b, v21.16b, v25.16b
        eor     v19.16b, v19.16b, v26.16b
        eor     v0.16b, v4.16b, v0.16b
        and     v3.16b, v17.16b, v3.16b
        eor     v1.16b, v9.16b, v1.16b
        eor     v9.16b, v25.16b, v23.16b
        eor     v5.16b, v5.16b, v21.16b
        eor     v2.16b, v6.16b, v2.16b
        and     v6.16b, v8.16b, v22.16b
        eor     v3.16b, v7.16b, v3.16b
        and     v8.16b, v20.16b, v18.16b
        eor     v10.16b, v10.16b, v9.16b
        eor     v0.16b, v0.16b, v19.16b
        eor     v9.16b, v1.16b, v9.16b
        eor     v1.16b, v2.16b, v21.16b
        eor     v3.16b, v3.16b, v19.16b
        and     v16.16b, v27.16b, v16.16b
        eor     v17.16b, v26.16b, v6.16b
        eor     v6.16b, v8.16b, v7.16b
        eor     v7.16b, v1.16b, v9.16b
        eor     v1.16b, v5.16b, v3.16b
        eor     v2.16b, v10.16b, v3.16b
        eor     v4.16b, v16.16b, v4.16b
        eor     v8.16b, v6.16b, v17.16b
        eor     v5.16b, v9.16b, v3.16b
        eor     v9.16b, v0.16b, v1.16b
        eor     v6.16b, v7.16b, v1.16b
        eor     v0.16b, v4.16b, v17.16b
        eor     v4.16b, v8.16b, v7.16b
        eor     v7.16b, v9.16b, v2.16b
        eor     v8.16b, v3.16b, v0.16b
        eor     v7.16b, v7.16b, v5.16b
        eor     v3.16b, v4.16b, v7.16b
        eor     v4.16b, v7.16b, v0.16b
        eor     v7.16b, v8.16b, v3.16b
        bcc     .Ldec_done
        ext     v8.16b, v0.16b, v0.16b, #8
        ext     v9.16b, v1.16b, v1.16b, #8
        ldr     q28, [x11]                  // load from .LISR in common case (x10 > 0)
        ext     v10.16b, v6.16b, v6.16b, #8
        ext     v16.16b, v3.16b, v3.16b, #8
        ext     v17.16b, v5.16b, v5.16b, #8
        ext     v18.16b, v4.16b, v4.16b, #8
        eor     v8.16b, v8.16b, v0.16b
        eor     v9.16b, v9.16b, v1.16b
        eor     v10.16b, v10.16b, v6.16b
        eor     v16.16b, v16.16b, v3.16b
        eor     v17.16b, v17.16b, v5.16b
        ext     v19.16b, v2.16b, v2.16b, #8
        ext     v20.16b, v7.16b, v7.16b, #8
        eor     v18.16b, v18.16b, v4.16b
        eor     v6.16b, v6.16b, v8.16b
        eor     v8.16b, v2.16b, v10.16b
        eor     v4.16b, v4.16b, v9.16b
        eor     v2.16b, v19.16b, v2.16b
        eor     v9.16b, v20.16b, v7.16b
        eor     v0.16b, v0.16b, v16.16b
        eor     v1.16b, v1.16b, v16.16b
        eor     v6.16b, v6.16b, v17.16b
        eor     v8.16b, v8.16b, v16.16b
        eor     v7.16b, v7.16b, v18.16b
        eor     v4.16b, v4.16b, v16.16b
        eor     v2.16b, v3.16b, v2.16b
        eor     v1.16b, v1.16b, v17.16b
        eor     v3.16b, v5.16b, v9.16b
        eor     v5.16b, v8.16b, v17.16b
        eor     v7.16b, v7.16b, v17.16b
        ext     v8.16b, v0.16b, v0.16b, #12
        ext     v9.16b, v6.16b, v6.16b, #12
        ext     v10.16b, v4.16b, v4.16b, #12
        ext     v16.16b, v1.16b, v1.16b, #12
        ext     v17.16b, v5.16b, v5.16b, #12
        ext     v18.16b, v7.16b, v7.16b, #12
        eor     v0.16b, v0.16b, v8.16b
        eor     v6.16b, v6.16b, v9.16b
        eor     v4.16b, v4.16b, v10.16b
        ext     v19.16b, v2.16b, v2.16b, #12
        ext     v20.16b, v3.16b, v3.16b, #12
        eor     v1.16b, v1.16b, v16.16b
        eor     v5.16b, v5.16b, v17.16b
        eor     v7.16b, v7.16b, v18.16b
        eor     v2.16b, v2.16b, v19.16b
        eor     v16.16b, v16.16b, v0.16b
        eor     v3.16b, v3.16b, v20.16b
        eor     v17.16b, v17.16b, v4.16b
        eor     v10.16b, v10.16b, v6.16b
        ext     v0.16b, v0.16b, v0.16b, #8
        eor     v9.16b, v9.16b, v1.16b
        ext     v1.16b, v1.16b, v1.16b, #8
        eor     v8.16b, v8.16b, v3.16b
        eor     v16.16b, v16.16b, v3.16b
        eor     v18.16b, v18.16b, v5.16b
        eor     v19.16b, v19.16b, v7.16b
        ext     v21.16b, v5.16b, v5.16b, #8
        ext     v5.16b, v7.16b, v7.16b, #8
        eor     v7.16b, v20.16b, v2.16b
        ext     v4.16b, v4.16b, v4.16b, #8
        ext     v20.16b, v3.16b, v3.16b, #8
        eor     v17.16b, v17.16b, v3.16b
        ext     v2.16b, v2.16b, v2.16b, #8
        eor     v3.16b, v10.16b, v3.16b
        ext     v10.16b, v6.16b, v6.16b, #8
        eor     v0.16b, v0.16b, v8.16b
        eor     v1.16b, v1.16b, v16.16b
        eor     v5.16b, v5.16b, v18.16b
        eor     v3.16b, v3.16b, v4.16b
        eor     v7.16b, v20.16b, v7.16b
        eor     v6.16b, v2.16b, v19.16b
        eor     v4.16b, v21.16b, v17.16b
        eor     v2.16b, v10.16b, v9.16b
        bne     .Ldec_loop
        ldr     q28, [x11, #16]!            // load from .LISRM0 on last round (x10 == 0)
        b       .Ldec_loop
.align  4
.Ldec_done:
        ushr    v8.2d, v0.2d, #1
        movi    v9.16b, #0x55
        ldr     q10, [x9]
        ushr    v16.2d, v2.2d, #1
        movi    v17.16b, #0x33
        ushr    v18.2d, v6.2d, #1
        movi    v19.16b, #0x0f
        eor     v8.16b, v8.16b, v1.16b
        ushr    v20.2d, v3.2d, #1
        eor     v16.16b, v16.16b, v7.16b
        eor     v18.16b, v18.16b, v4.16b
        and     v8.16b, v8.16b, v9.16b
        eor     v20.16b, v20.16b, v5.16b
        and     v16.16b, v16.16b, v9.16b
        and     v18.16b, v18.16b, v9.16b
        shl     v21.2d, v8.2d, #1
        eor     v1.16b, v1.16b, v8.16b
        and     v8.16b, v20.16b, v9.16b
        eor     v7.16b, v7.16b, v16.16b
        shl     v9.2d, v16.2d, #1
        eor     v4.16b, v4.16b, v18.16b
        shl     v16.2d, v18.2d, #1
        eor     v0.16b, v0.16b, v21.16b
        shl     v18.2d, v8.2d, #1
        eor     v5.16b, v5.16b, v8.16b
        eor     v2.16b, v2.16b, v9.16b
        eor     v6.16b, v6.16b, v16.16b
        ushr    v8.2d, v1.2d, #2
        eor     v3.16b, v3.16b, v18.16b
        ushr    v9.2d, v0.2d, #2
        ushr    v16.2d, v7.2d, #2
        ushr    v18.2d, v2.2d, #2
        eor     v8.16b, v8.16b, v4.16b
        eor     v9.16b, v9.16b, v6.16b
        eor     v16.16b, v16.16b, v5.16b
        eor     v18.16b, v18.16b, v3.16b
        and     v8.16b, v8.16b, v17.16b
        and     v9.16b, v9.16b, v17.16b
        and     v16.16b, v16.16b, v17.16b
        and     v17.16b, v18.16b, v17.16b
        eor     v4.16b, v4.16b, v8.16b
        shl     v8.2d, v8.2d, #2
        eor     v6.16b, v6.16b, v9.16b
        shl     v9.2d, v9.2d, #2
        eor     v5.16b, v5.16b, v16.16b
        shl     v16.2d, v16.2d, #2
        eor     v3.16b, v3.16b, v17.16b
        shl     v17.2d, v17.2d, #2
        eor     v1.16b, v1.16b, v8.16b
        eor     v0.16b, v0.16b, v9.16b
        eor     v7.16b, v7.16b, v16.16b
        eor     v2.16b, v2.16b, v17.16b
        ushr    v8.2d, v4.2d, #4
        ushr    v9.2d, v6.2d, #4
        ushr    v16.2d, v1.2d, #4
        ushr    v17.2d, v0.2d, #4
        eor     v8.16b, v8.16b, v5.16b
        eor     v9.16b, v9.16b, v3.16b
        eor     v16.16b, v16.16b, v7.16b
        eor     v17.16b, v17.16b, v2.16b
        and     v8.16b, v8.16b, v19.16b
        and     v9.16b, v9.16b, v19.16b
        and     v16.16b, v16.16b, v19.16b
        and     v17.16b, v17.16b, v19.16b
        eor     v5.16b, v5.16b, v8.16b
        shl     v8.2d, v8.2d, #4
        eor     v3.16b, v3.16b, v9.16b
        shl     v9.2d, v9.2d, #4
        eor     v7.16b, v7.16b, v16.16b
        shl     v16.2d, v16.2d, #4
        eor     v2.16b, v2.16b, v17.16b
        shl     v17.2d, v17.2d, #4
        eor     v4.16b, v4.16b, v8.16b
        eor     v6.16b, v6.16b, v9.16b
        eor     v7.16b, v7.16b, v10.16b
        eor     v1.16b, v1.16b, v16.16b
        eor     v2.16b, v2.16b, v10.16b
        eor     v0.16b, v0.16b, v17.16b
        eor     v4.16b, v4.16b, v10.16b
        eor     v6.16b, v6.16b, v10.16b
        eor     v3.16b, v3.16b, v10.16b
        eor     v5.16b, v5.16b, v10.16b
        eor     v1.16b, v1.16b, v10.16b
        eor     v0.16b, v0.16b, v10.16b
        ret
.size   _bsaes_decrypt8,.-_bsaes_decrypt8

.rodata
.type   _bsaes_consts,%object
.align  6
_bsaes_consts:
// InvShiftRows constants
// Used in _bsaes_decrypt8, which assumes contiguity
// .LM0ISR used with round 0 key
// .LISR   used with middle round keys
// .LISRM0 used with final round key
.LM0ISR:
.quad   0x0a0e0206070b0f03, 0x0004080c0d010509
.LISR:
.quad   0x0504070602010003, 0x0f0e0d0c080b0a09
.LISRM0:
.quad   0x01040b0e0205080f, 0x0306090c00070a0d

// ShiftRows constants
// Used in _bsaes_encrypt8, which assumes contiguity
// .LM0SR used with round 0 key
// .LSR   used with middle round keys
// .LSRM0 used with final round key
.LM0SR:
.quad   0x0a0e02060f03070b, 0x0004080c05090d01
.LSR:
.quad   0x0504070600030201, 0x0f0e0d0c0a09080b
.LSRM0:
.quad   0x0304090e00050a0f, 0x01060b0c0207080d

.LM0_bigendian:
.quad   0x02060a0e03070b0f, 0x0004080c0105090d
.LM0_littleendian:
.quad   0x0105090d0004080c, 0x03070b0f02060a0e

// Used in ossl_bsaes_ctr32_encrypt_blocks, prior to dropping into
// _bsaes_encrypt8_alt, for round 0 key in place of .LM0SR
.LREVM0SR:
.quad   0x090d01050c000408, 0x03070b0f060a0e02

.align  6
.size   _bsaes_consts,.-_bsaes_consts

.previous

.type   _bsaes_encrypt8,%function
.align  4
// On entry:
//   x9 -> key (previously expanded using _bsaes_key_convert)
//   x10 = number of rounds
//   v0-v7 input data
// On exit:
//   x9-x11 corrupted
//   other general-purpose registers preserved
//   v0-v7 output data
//   v11-v15 preserved
//   other SIMD registers corrupted
_bsaes_encrypt8:
        ldr     q8, [x9], #16
        adrp    x11, .LM0SR
        add     x11, x11, #:lo12:.LM0SR
        ldr     q9, [x11], #16
_bsaes_encrypt8_alt:
        eor     v0.16b, v0.16b, v8.16b
        eor     v1.16b, v1.16b, v8.16b
        sub     x10, x10, #1
        eor     v2.16b, v2.16b, v8.16b
        eor     v4.16b, v4.16b, v8.16b
        eor     v3.16b, v3.16b, v8.16b
        eor     v5.16b, v5.16b, v8.16b
        tbl     v0.16b, {v0.16b}, v9.16b
        tbl     v1.16b, {v1.16b}, v9.16b
        tbl     v2.16b, {v2.16b}, v9.16b
        tbl     v4.16b, {v4.16b}, v9.16b
        eor     v6.16b, v6.16b, v8.16b
        eor     v7.16b, v7.16b, v8.16b
        tbl     v3.16b, {v3.16b}, v9.16b
        tbl     v5.16b, {v5.16b}, v9.16b
        tbl     v6.16b, {v6.16b}, v9.16b
        ushr    v8.2d, v0.2d, #1
        movi    v10.16b, #0x55
        tbl     v7.16b, {v7.16b}, v9.16b
        ushr    v9.2d, v4.2d, #1
        movi    v16.16b, #0x33
        ushr    v17.2d, v2.2d, #1
        eor     v8.16b, v8.16b, v1.16b
        movi    v18.16b, #0x0f
        ushr    v19.2d, v6.2d, #1
        eor     v9.16b, v9.16b, v5.16b
        eor     v17.16b, v17.16b, v3.16b
        and     v8.16b, v8.16b, v10.16b
        eor     v19.16b, v19.16b, v7.16b
        and     v9.16b, v9.16b, v10.16b
        and     v17.16b, v17.16b, v10.16b
        eor     v1.16b, v1.16b, v8.16b
        shl     v8.2d, v8.2d, #1
        and     v10.16b, v19.16b, v10.16b
        eor     v5.16b, v5.16b, v9.16b
        shl     v9.2d, v9.2d, #1
        eor     v3.16b, v3.16b, v17.16b
        shl     v17.2d, v17.2d, #1
        eor     v0.16b, v0.16b, v8.16b
        shl     v8.2d, v10.2d, #1
        eor     v7.16b, v7.16b, v10.16b
        eor     v4.16b, v4.16b, v9.16b
        eor     v2.16b, v2.16b, v17.16b
        ushr    v9.2d, v1.2d, #2
        eor     v6.16b, v6.16b, v8.16b
        ushr    v8.2d, v0.2d, #2
        ushr    v10.2d, v5.2d, #2
        ushr    v17.2d, v4.2d, #2
        eor     v9.16b, v9.16b, v3.16b
        eor     v8.16b, v8.16b, v2.16b
        eor     v10.16b, v10.16b, v7.16b
        eor     v17.16b, v17.16b, v6.16b
        and     v9.16b, v9.16b, v16.16b
        and     v8.16b, v8.16b, v16.16b
        and     v10.16b, v10.16b, v16.16b
        and     v16.16b, v17.16b, v16.16b
        eor     v3.16b, v3.16b, v9.16b
        shl     v9.2d, v9.2d, #2
        eor     v2.16b, v2.16b, v8.16b
        shl     v8.2d, v8.2d, #2
        eor     v7.16b, v7.16b, v10.16b
        shl     v10.2d, v10.2d, #2
        eor     v6.16b, v6.16b, v16.16b
        shl     v16.2d, v16.2d, #2
        eor     v1.16b, v1.16b, v9.16b
        eor     v0.16b, v0.16b, v8.16b
        eor     v5.16b, v5.16b, v10.16b
        eor     v4.16b, v4.16b, v16.16b
        ushr    v8.2d, v3.2d, #4
        ushr    v9.2d, v2.2d, #4
        ushr    v10.2d, v1.2d, #4
        ushr    v16.2d, v0.2d, #4
        eor     v8.16b, v8.16b, v7.16b
        eor     v9.16b, v9.16b, v6.16b
        eor     v10.16b, v10.16b, v5.16b
        eor     v16.16b, v16.16b, v4.16b
        and     v8.16b, v8.16b, v18.16b
        and     v9.16b, v9.16b, v18.16b
        and     v10.16b, v10.16b, v18.16b
        and     v16.16b, v16.16b, v18.16b
        eor     v7.16b, v7.16b, v8.16b
        shl     v8.2d, v8.2d, #4
        eor     v6.16b, v6.16b, v9.16b
        shl     v9.2d, v9.2d, #4
        eor     v5.16b, v5.16b, v10.16b
        shl     v10.2d, v10.2d, #4
        eor     v4.16b, v4.16b, v16.16b
        shl     v16.2d, v16.2d, #4
        eor     v3.16b, v3.16b, v8.16b
        eor     v2.16b, v2.16b, v9.16b
        eor     v1.16b, v1.16b, v10.16b
        eor     v0.16b, v0.16b, v16.16b
        b       .Lenc_sbox
.align  4
.Lenc_loop:
        ld1     {v16.16b, v17.16b, v18.16b, v19.16b}, [x9], #64
        ldp     q8, q9, [x9], #32
        eor     v0.16b, v16.16b, v0.16b
        ldr     q10, [x9], #16
        eor     v1.16b, v17.16b, v1.16b
        ldr     q16, [x9], #16
        eor     v2.16b, v18.16b, v2.16b
        eor     v3.16b, v19.16b, v3.16b
        eor     v4.16b, v8.16b, v4.16b
        eor     v5.16b, v9.16b, v5.16b
        eor     v6.16b, v10.16b, v6.16b
        eor     v7.16b, v16.16b, v7.16b
        tbl     v0.16b, {v0.16b}, v28.16b
        tbl     v1.16b, {v1.16b}, v28.16b
        tbl     v2.16b, {v2.16b}, v28.16b
        tbl     v3.16b, {v3.16b}, v28.16b
        tbl     v4.16b, {v4.16b}, v28.16b
        tbl     v5.16b, {v5.16b}, v28.16b
        tbl     v6.16b, {v6.16b}, v28.16b
        tbl     v7.16b, {v7.16b}, v28.16b
.Lenc_sbox:
        eor     v5.16b, v5.16b, v6.16b
        eor     v3.16b, v3.16b, v0.16b
        subs    x10, x10, #1
        eor     v2.16b, v2.16b, v1.16b
        eor     v5.16b, v5.16b, v0.16b
        eor     v8.16b, v3.16b, v7.16b
        eor     v6.16b, v6.16b, v2.16b
        eor     v7.16b, v7.16b, v5.16b
        eor     v8.16b, v8.16b, v4.16b
        eor     v3.16b, v6.16b, v3.16b
        eor     v4.16b, v4.16b, v5.16b
        eor     v6.16b, v1.16b, v5.16b
        eor     v2.16b, v2.16b, v7.16b
        eor     v1.16b, v8.16b, v1.16b
        eor     v8.16b, v7.16b, v4.16b
        eor     v9.16b, v3.16b, v0.16b
        eor     v10.16b, v7.16b, v6.16b
        eor     v16.16b, v5.16b, v3.16b
        eor     v17.16b, v6.16b, v2.16b
        eor     v18.16b, v5.16b, v1.16b
        eor     v19.16b, v2.16b, v4.16b
        eor     v20.16b, v1.16b, v0.16b
        orr     v21.16b, v8.16b, v9.16b
        orr     v22.16b, v10.16b, v16.16b
        eor     v23.16b, v8.16b, v17.16b
        eor     v24.16b, v9.16b, v18.16b
        and     v19.16b, v19.16b, v20.16b
        orr     v20.16b, v17.16b, v18.16b
        and     v8.16b, v8.16b, v9.16b
        and     v9.16b, v17.16b, v18.16b
        and     v17.16b, v23.16b, v24.16b
        and     v10.16b, v10.16b, v16.16b
        eor     v16.16b, v21.16b, v19.16b
        eor     v18.16b, v20.16b, v19.16b
        and     v19.16b, v2.16b, v1.16b
        and     v20.16b, v6.16b, v5.16b
        eor     v21.16b, v22.16b, v17.16b
        eor     v9.16b, v9.16b, v10.16b
        eor     v10.16b, v16.16b, v17.16b
        eor     v16.16b, v18.16b, v8.16b
        and     v17.16b, v4.16b, v0.16b
        orr     v18.16b, v7.16b, v3.16b
        eor     v21.16b, v21.16b, v8.16b
        eor     v8.16b, v9.16b, v8.16b
        eor     v9.16b, v10.16b, v19.16b
        eor     v10.16b, v3.16b, v0.16b
        eor     v16.16b, v16.16b, v17.16b
        eor     v17.16b, v5.16b, v1.16b
        eor     v19.16b, v21.16b, v20.16b
        eor     v20.16b, v8.16b, v18.16b
        eor     v8.16b, v8.16b, v18.16b
        eor     v18.16b, v7.16b, v4.16b
        eor     v21.16b, v9.16b, v16.16b
        eor     v22.16b, v6.16b, v2.16b
        and     v23.16b, v9.16b, v19.16b
        eor     v24.16b, v10.16b, v17.16b
        eor     v25.16b, v0.16b, v1.16b
        eor     v26.16b, v7.16b, v6.16b
        eor     v27.16b, v18.16b, v22.16b
        eor     v28.16b, v3.16b, v5.16b
        eor     v29.16b, v16.16b, v23.16b
        eor     v30.16b, v20.16b, v23.16b
        eor     v23.16b, v20.16b, v23.16b
        eor     v31.16b, v4.16b, v2.16b
        bsl     v29.16b, v19.16b, v20.16b
        bsl     v30.16b, v9.16b, v16.16b
        bsl     v8.16b, v29.16b, v23.16b
        bsl     v20.16b, v23.16b, v29.16b
        eor     v9.16b, v30.16b, v29.16b
        and     v5.16b, v5.16b, v30.16b
        and     v8.16b, v8.16b, v30.16b
        and     v1.16b, v1.16b, v29.16b
        eor     v16.16b, v19.16b, v20.16b
        and     v2.16b, v2.16b, v29.16b
        eor     v19.16b, v9.16b, v29.16b
        and     v17.16b, v17.16b, v9.16b
        eor     v8.16b, v8.16b, v21.16b
        and     v20.16b, v22.16b, v9.16b
        eor     v21.16b, v29.16b, v16.16b
        eor     v22.16b, v29.16b, v16.16b
        and     v23.16b, v25.16b, v16.16b
        and     v6.16b, v6.16b, v19.16b
        eor     v25.16b, v8.16b, v16.16b
        eor     v29.16b, v30.16b, v8.16b
        and     v4.16b, v21.16b, v4.16b
        and     v8.16b, v28.16b, v8.16b
        and     v0.16b, v22.16b, v0.16b
        eor     v21.16b, v23.16b, v1.16b
        eor     v22.16b, v9.16b, v25.16b
        eor     v9.16b, v9.16b, v25.16b
        eor     v23.16b, v25.16b, v16.16b
        and     v3.16b, v29.16b, v3.16b
        and     v24.16b, v24.16b, v25.16b
        and     v25.16b, v27.16b, v25.16b
        and     v10.16b, v22.16b, v10.16b
        and     v9.16b, v9.16b, v18.16b
        eor     v18.16b, v19.16b, v23.16b
        and     v19.16b, v26.16b, v23.16b
        eor     v3.16b, v5.16b, v3.16b
        eor     v17.16b, v17.16b, v24.16b
        eor     v10.16b, v24.16b, v10.16b
        and     v16.16b, v31.16b, v16.16b
        eor     v20.16b, v20.16b, v25.16b
        eor     v9.16b, v25.16b, v9.16b
        eor     v4.16b, v2.16b, v4.16b
        and     v7.16b, v18.16b, v7.16b
        eor     v18.16b, v19.16b, v6.16b
        eor     v5.16b, v8.16b, v5.16b
        eor     v0.16b, v1.16b, v0.16b
        eor     v1.16b, v21.16b, v10.16b
        eor     v8.16b, v3.16b, v17.16b
        eor     v2.16b, v16.16b, v2.16b
        eor     v3.16b, v6.16b, v7.16b
        eor     v6.16b, v18.16b, v9.16b
        eor     v4.16b, v4.16b, v20.16b
        eor     v10.16b, v5.16b, v10.16b
        eor     v0.16b, v0.16b, v17.16b
        eor     v9.16b, v2.16b, v9.16b
        eor     v3.16b, v3.16b, v20.16b
        eor     v7.16b, v6.16b, v1.16b
        eor     v5.16b, v8.16b, v4.16b
        eor     v6.16b, v10.16b, v1.16b
        eor     v2.16b, v4.16b, v0.16b
        eor     v4.16b, v3.16b, v10.16b
        eor     v9.16b, v9.16b, v7.16b
        eor     v3.16b, v0.16b, v5.16b
        eor     v0.16b, v1.16b, v4.16b
        eor     v1.16b, v4.16b, v8.16b
        eor     v4.16b, v9.16b, v5.16b
        eor     v6.16b, v6.16b, v3.16b
        bcc     .Lenc_done
        ext     v8.16b, v0.16b, v0.16b, #12
        ext     v9.16b, v4.16b, v4.16b, #12
        ldr     q28, [x11]
        ext     v10.16b, v6.16b, v6.16b, #12
        ext     v16.16b, v1.16b, v1.16b, #12
        ext     v17.16b, v3.16b, v3.16b, #12
        ext     v18.16b, v7.16b, v7.16b, #12
        eor     v0.16b, v0.16b, v8.16b
        eor     v4.16b, v4.16b, v9.16b
        eor     v6.16b, v6.16b, v10.16b
        ext     v19.16b, v2.16b, v2.16b, #12
        ext     v20.16b, v5.16b, v5.16b, #12
        eor     v1.16b, v1.16b, v16.16b
        eor     v3.16b, v3.16b, v17.16b
        eor     v7.16b, v7.16b, v18.16b
        eor     v2.16b, v2.16b, v19.16b
        eor     v16.16b, v16.16b, v0.16b
        eor     v5.16b, v5.16b, v20.16b
        eor     v17.16b, v17.16b, v6.16b
        eor     v10.16b, v10.16b, v4.16b
        ext     v0.16b, v0.16b, v0.16b, #8
        eor     v9.16b, v9.16b, v1.16b
        ext     v1.16b, v1.16b, v1.16b, #8
        eor     v8.16b, v8.16b, v5.16b
        eor     v16.16b, v16.16b, v5.16b
        eor     v18.16b, v18.16b, v3.16b
        eor     v19.16b, v19.16b, v7.16b
        ext     v3.16b, v3.16b, v3.16b, #8
        ext     v7.16b, v7.16b, v7.16b, #8
        eor     v20.16b, v20.16b, v2.16b
        ext     v6.16b, v6.16b, v6.16b, #8
        ext     v21.16b, v5.16b, v5.16b, #8
        eor     v17.16b, v17.16b, v5.16b
        ext     v2.16b, v2.16b, v2.16b, #8
        eor     v10.16b, v10.16b, v5.16b
        ext     v22.16b, v4.16b, v4.16b, #8
        eor     v0.16b, v0.16b, v8.16b
        eor     v1.16b, v1.16b, v16.16b
        eor     v5.16b, v7.16b, v18.16b
        eor     v4.16b, v3.16b, v17.16b
        eor     v3.16b, v6.16b, v10.16b
        eor     v7.16b, v21.16b, v20.16b
        eor     v6.16b, v2.16b, v19.16b
        eor     v2.16b, v22.16b, v9.16b
        bne     .Lenc_loop
        ldr     q28, [x11, #16]!            // load from .LSRM0 on last round (x10 == 0)
        b       .Lenc_loop
.align  4
.Lenc_done:
        ushr    v8.2d, v0.2d, #1
        movi    v9.16b, #0x55
        ldr     q10, [x9]
        ushr    v16.2d, v3.2d, #1
        movi    v17.16b, #0x33
        ushr    v18.2d, v4.2d, #1
        movi    v19.16b, #0x0f
        eor     v8.16b, v8.16b, v1.16b
        ushr    v20.2d, v2.2d, #1
        eor     v16.16b, v16.16b, v7.16b
        eor     v18.16b, v18.16b, v6.16b
        and     v8.16b, v8.16b, v9.16b
        eor     v20.16b, v20.16b, v5.16b
        and     v16.16b, v16.16b, v9.16b
        and     v18.16b, v18.16b, v9.16b
        shl     v21.2d, v8.2d, #1
        eor     v1.16b, v1.16b, v8.16b
        and     v8.16b, v20.16b, v9.16b
        eor     v7.16b, v7.16b, v16.16b
        shl     v9.2d, v16.2d, #1
        eor     v6.16b, v6.16b, v18.16b
        shl     v16.2d, v18.2d, #1
        eor     v0.16b, v0.16b, v21.16b
        shl     v18.2d, v8.2d, #1
        eor     v5.16b, v5.16b, v8.16b
        eor     v3.16b, v3.16b, v9.16b
        eor     v4.16b, v4.16b, v16.16b
        ushr    v8.2d, v1.2d, #2
        eor     v2.16b, v2.16b, v18.16b
        ushr    v9.2d, v0.2d, #2
        ushr    v16.2d, v7.2d, #2
        ushr    v18.2d, v3.2d, #2
        eor     v8.16b, v8.16b, v6.16b
        eor     v9.16b, v9.16b, v4.16b
        eor     v16.16b, v16.16b, v5.16b
        eor     v18.16b, v18.16b, v2.16b
        and     v8.16b, v8.16b, v17.16b
        and     v9.16b, v9.16b, v17.16b
        and     v16.16b, v16.16b, v17.16b
        and     v17.16b, v18.16b, v17.16b
        eor     v6.16b, v6.16b, v8.16b
        shl     v8.2d, v8.2d, #2
        eor     v4.16b, v4.16b, v9.16b
        shl     v9.2d, v9.2d, #2
        eor     v5.16b, v5.16b, v16.16b
        shl     v16.2d, v16.2d, #2
        eor     v2.16b, v2.16b, v17.16b
        shl     v17.2d, v17.2d, #2
        eor     v1.16b, v1.16b, v8.16b
        eor     v0.16b, v0.16b, v9.16b
        eor     v7.16b, v7.16b, v16.16b
        eor     v3.16b, v3.16b, v17.16b
        ushr    v8.2d, v6.2d, #4
        ushr    v9.2d, v4.2d, #4
        ushr    v16.2d, v1.2d, #4
        ushr    v17.2d, v0.2d, #4
        eor     v8.16b, v8.16b, v5.16b
        eor     v9.16b, v9.16b, v2.16b
        eor     v16.16b, v16.16b, v7.16b
        eor     v17.16b, v17.16b, v3.16b
        and     v8.16b, v8.16b, v19.16b
        and     v9.16b, v9.16b, v19.16b
        and     v16.16b, v16.16b, v19.16b
        and     v17.16b, v17.16b, v19.16b
        eor     v5.16b, v5.16b, v8.16b
        shl     v8.2d, v8.2d, #4
        eor     v2.16b, v2.16b, v9.16b
        shl     v9.2d, v9.2d, #4
        eor     v7.16b, v7.16b, v16.16b
        shl     v16.2d, v16.2d, #4
        eor     v3.16b, v3.16b, v17.16b
        shl     v17.2d, v17.2d, #4
        eor     v6.16b, v6.16b, v8.16b
        eor     v4.16b, v4.16b, v9.16b
        eor     v7.16b, v7.16b, v10.16b
        eor     v1.16b, v1.16b, v16.16b
        eor     v3.16b, v3.16b, v10.16b
        eor     v0.16b, v0.16b, v17.16b
        eor     v6.16b, v6.16b, v10.16b
        eor     v4.16b, v4.16b, v10.16b
        eor     v2.16b, v2.16b, v10.16b
        eor     v5.16b, v5.16b, v10.16b
        eor     v1.16b, v1.16b, v10.16b
        eor     v0.16b, v0.16b, v10.16b
        ret
.size   _bsaes_encrypt8,.-_bsaes_encrypt8

.type   _bsaes_key_convert,%function
.align  4
// On entry:
//   x9 -> input key (big-endian)
//   x10 = number of rounds
//   x17 -> output key (native endianness)
// On exit:
//   x9, x10 corrupted
//   x11 -> .LM0_bigendian
//   x17 -> last quadword of output key
//   other general-purpose registers preserved
//   v2-v6 preserved
//   v7.16b[] = 0x63
//   v8-v14 preserved
//   v15 = last round key (converted to native endianness)
//   other SIMD registers corrupted
_bsaes_key_convert:
#ifdef __AARCH64EL__
        adrp    x11, .LM0_littleendian
        add     x11, x11, #:lo12:.LM0_littleendian
#else
        adrp    x11, .LM0_bigendian
        add     x11, x11, #:lo12:.LM0_bigendian
#endif
        ldr     q0, [x9], #16               // load round 0 key
        ldr     q1, [x11]                   // .LM0
        ldr     q15, [x9], #16              // load round 1 key

        movi    v7.16b, #0x63               // compose .L63
        movi    v16.16b, #0x01              // bit masks
        movi    v17.16b, #0x02
        movi    v18.16b, #0x04
        movi    v19.16b, #0x08
        movi    v20.16b, #0x10
        movi    v21.16b, #0x20
        movi    v22.16b, #0x40
        movi    v23.16b, #0x80

#ifdef __AARCH64EL__
        rev32   v0.16b, v0.16b
#endif
        sub     x10, x10, #1
        str     q0, [x17], #16              // save round 0 key

.align  4
.Lkey_loop:
        tbl     v0.16b, {v15.16b}, v1.16b
        ldr     q15, [x9], #16              // load next round key

        eor     v0.16b, v0.16b, v7.16b
        cmtst   v24.16b, v0.16b, v16.16b
        cmtst   v25.16b, v0.16b, v17.16b
        cmtst   v26.16b, v0.16b, v18.16b
        cmtst   v27.16b, v0.16b, v19.16b
        cmtst   v28.16b, v0.16b, v20.16b
        cmtst   v29.16b, v0.16b, v21.16b
        cmtst   v30.16b, v0.16b, v22.16b
        cmtst   v31.16b, v0.16b, v23.16b
        sub     x10, x10, #1
        st1     {v24.16b-v27.16b}, [x17], #64 // write bit-sliced round key
        st1     {v28.16b-v31.16b}, [x17], #64
        cbnz    x10, .Lkey_loop

        // don't save last round key
#ifdef __AARCH64EL__
        rev32   v15.16b, v15.16b
        adrp    x11, .LM0_bigendian
        add     x11, x11, #:lo12:.LM0_bigendian
#endif
        ret
.size   _bsaes_key_convert,.-_bsaes_key_convert

.globl  ossl_bsaes_cbc_encrypt
.type   ossl_bsaes_cbc_encrypt,%function
.align  4
// On entry:
//   x0 -> input ciphertext
//   x1 -> output plaintext
//   x2 = size of ciphertext and plaintext in bytes (assumed a multiple of 16)
//   x3 -> key
//   x4 -> 128-bit initialisation vector (or preceding 128-bit block of ciphertext if continuing after an earlier call)
//   w5 must be == 0
// On exit:
//   Output plaintext filled in
//   Initialisation vector overwritten with last quadword of ciphertext
//   No output registers, usual AAPCS64 register preservation
ossl_bsaes_cbc_encrypt:
        AARCH64_VALID_CALL_TARGET
        cmp     x2, #128
        bhs     .Lcbc_do_bsaes
        b       AES_cbc_encrypt
.Lcbc_do_bsaes:

        // it is up to the caller to make sure we are called with enc == 0

        stp     x29, x30, [sp, #-48]!
        stp     d8, d9, [sp, #16]
        stp     d10, d15, [sp, #32]
        lsr     x2, x2, #4                  // len in 16 byte blocks

        ldr     w15, [x3, #240]             // get # of rounds
        mov     x14, sp

        // allocate the key schedule on the stack
        add     x17, sp, #96
        sub     x17, x17, x15, lsl #7       // 128 bytes per inner round key, less 96 bytes

        // populate the key schedule
        mov     x9, x3                      // pass key
        mov     x10, x15                    // pass # of rounds
        mov     sp, x17                     // sp is sp
        bl      _bsaes_key_convert
        ldr     q6,  [sp]
        str     q15, [x17]                  // save last round key
        eor     v6.16b, v6.16b, v7.16b      // fix up round 0 key (by XORing with 0x63)
        str     q6, [sp]

        ldr     q15, [x4]                   // load IV
        b       .Lcbc_dec_loop

.align  4
.Lcbc_dec_loop:
        subs    x2, x2, #0x8
        bmi     .Lcbc_dec_loop_finish

        ldr     q0, [x0], #16               // load input
        mov     x9, sp                      // pass the key
        ldr     q1, [x0], #16
        mov     x10, x15
        ldr     q2, [x0], #16
        ldr     q3, [x0], #16
        ldr     q4, [x0], #16
        ldr     q5, [x0], #16
        ldr     q6, [x0], #16
        ldr     q7, [x0], #-7*16

        bl      _bsaes_decrypt8

        ldr     q16, [x0], #16              // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        eor     v1.16b, v1.16b, v16.16b
        str     q0, [x1], #16               // write output
        ldr     q0, [x0], #16
        str     q1, [x1], #16
        ldr     q1, [x0], #16
        eor     v1.16b, v4.16b, v1.16b
        ldr     q4, [x0], #16
        eor     v2.16b, v2.16b, v4.16b
        eor     v0.16b, v6.16b, v0.16b
        ldr     q4, [x0], #16
        str     q0, [x1], #16
        str     q1, [x1], #16
        eor     v0.16b, v7.16b, v4.16b
        ldr     q1, [x0], #16
        str     q2, [x1], #16
        ldr     q2, [x0], #16
        ldr     q15, [x0], #16
        str     q0, [x1], #16
        eor     v0.16b, v5.16b, v2.16b
        eor     v1.16b, v3.16b, v1.16b
        str     q1, [x1], #16
        str     q0, [x1], #16

        b       .Lcbc_dec_loop

.Lcbc_dec_loop_finish:
        adds    x2, x2, #8
        beq     .Lcbc_dec_done

        ldr     q0, [x0], #16               // load input
        cmp     x2, #2
        blo     .Lcbc_dec_one
        ldr     q1, [x0], #16
        mov     x9, sp                      // pass the key
        mov     x10, x15
        beq     .Lcbc_dec_two
        ldr     q2, [x0], #16
        cmp     x2, #4
        blo     .Lcbc_dec_three
        ldr     q3, [x0], #16
        beq     .Lcbc_dec_four
        ldr     q4, [x0], #16
        cmp     x2, #6
        blo     .Lcbc_dec_five
        ldr     q5, [x0], #16
        beq     .Lcbc_dec_six
        ldr     q6, [x0], #-6*16

        bl      _bsaes_decrypt8

        ldr     q5, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q8, [x0], #16
        ldr     q9, [x0], #16
        ldr     q10, [x0], #16
        str     q0, [x1], #16               // write output
        ldr     q0, [x0], #16
        eor     v1.16b, v1.16b, v5.16b
        ldr     q5, [x0], #16
        eor     v6.16b, v6.16b, v8.16b
        ldr     q15, [x0]
        eor     v4.16b, v4.16b, v9.16b
        eor     v2.16b, v2.16b, v10.16b
        str     q1, [x1], #16
        eor     v0.16b, v7.16b, v0.16b
        str     q6, [x1], #16
        eor     v1.16b, v3.16b, v5.16b
        str     q4, [x1], #16
        str     q2, [x1], #16
        str     q0, [x1], #16
        str     q1, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_six:
        sub     x0, x0, #0x60
        bl      _bsaes_decrypt8
        ldr     q3, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q5, [x0], #16
        ldr     q8, [x0], #16
        ldr     q9, [x0], #16
        str     q0, [x1], #16               // write output
        ldr     q0, [x0], #16
        eor     v1.16b, v1.16b, v3.16b
        ldr     q15, [x0]
        eor     v3.16b, v6.16b, v5.16b
        eor     v4.16b, v4.16b, v8.16b
        eor     v2.16b, v2.16b, v9.16b
        str     q1, [x1], #16
        eor     v0.16b, v7.16b, v0.16b
        str     q3, [x1], #16
        str     q4, [x1], #16
        str     q2, [x1], #16
        str     q0, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_five:
        sub     x0, x0, #0x50
        bl      _bsaes_decrypt8
        ldr     q3, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q5, [x0], #16
        ldr     q7, [x0], #16
        ldr     q8, [x0], #16
        str     q0, [x1], #16               // write output
        ldr     q15, [x0]
        eor     v0.16b, v1.16b, v3.16b
        eor     v1.16b, v6.16b, v5.16b
        eor     v3.16b, v4.16b, v7.16b
        str     q0, [x1], #16
        eor     v0.16b, v2.16b, v8.16b
        str     q1, [x1], #16
        str     q3, [x1], #16
        str     q0, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_four:
        sub     x0, x0, #0x40
        bl      _bsaes_decrypt8
        ldr     q2, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q3, [x0], #16
        ldr     q5, [x0], #16
        str     q0, [x1], #16               // write output
        ldr     q15, [x0]
        eor     v0.16b, v1.16b, v2.16b
        eor     v1.16b, v6.16b, v3.16b
        eor     v2.16b, v4.16b, v5.16b
        str     q0, [x1], #16
        str     q1, [x1], #16
        str     q2, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_three:
        sub     x0, x0, #0x30
        bl      _bsaes_decrypt8
        ldr     q2, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q3, [x0], #16
        ldr     q15, [x0]
        str     q0, [x1], #16               // write output
        eor     v0.16b, v1.16b, v2.16b
        eor     v1.16b, v6.16b, v3.16b
        str     q0, [x1], #16
        str     q1, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_two:
        sub     x0, x0, #0x20
        bl      _bsaes_decrypt8
        ldr     q2, [x0], #16               // reload input
        eor     v0.16b, v0.16b, v15.16b     // ^= IV
        ldr     q15, [x0]
        str     q0, [x1], #16               // write output
        eor     v0.16b, v1.16b, v2.16b
        str     q0, [x1]
        b       .Lcbc_dec_done
.align  4
.Lcbc_dec_one:
        sub     x0, x0, #0x10
        stp     x1, x4, [sp, #-32]!
        str     x14, [sp, #16]
        mov     v8.16b, v15.16b
        mov     v15.16b, v0.16b
        mov     x2, x3
        bl      AES_decrypt
        ldr     x14, [sp, #16]
        ldp     x1, x4, [sp], #32
        ldr     q0, [x1]                    // load result
        eor     v0.16b, v0.16b, v8.16b      // ^= IV
        str     q0, [x1]                    // write output

.align  4
.Lcbc_dec_done:
        movi    v0.16b, #0
        movi    v1.16b, #0
.Lcbc_dec_bzero:// wipe key schedule [if any]
        stp     q0, q1, [sp], #32
        cmp     sp, x14
        bne     .Lcbc_dec_bzero
        str     q15, [x4]                   // return IV
        ldp     d8, d9, [sp, #16]
        ldp     d10, d15, [sp, #32]
        ldp     x29, x30, [sp], #48
        ret
.size   ossl_bsaes_cbc_encrypt,.-ossl_bsaes_cbc_encrypt

.globl  ossl_bsaes_ctr32_encrypt_blocks
.type   ossl_bsaes_ctr32_encrypt_blocks,%function
.align  4
// On entry:
//   x0 -> input text (whole 16-byte blocks)
//   x1 -> output text (whole 16-byte blocks)
//   x2 = number of 16-byte blocks to encrypt/decrypt (> 0)
//   x3 -> key
//   x4 -> initial value of 128-bit counter (stored big-endian) which increments, modulo 2^32, for each block
// On exit:
//   Output text filled in
//   No output registers, usual AAPCS64 register preservation
ossl_bsaes_ctr32_encrypt_blocks:
        AARCH64_VALID_CALL_TARGET
        cmp     x2, #8                      // use plain AES for
        blo     .Lctr_enc_short             // small sizes

        stp     x29, x30, [sp, #-80]!
        stp     d8, d9, [sp, #16]
        stp     d10, d11, [sp, #32]
        stp     d12, d13, [sp, #48]
        stp     d14, d15, [sp, #64]

        ldr     w15, [x3, #240]             // get # of rounds
        mov     x14, sp

        // allocate the key schedule on the stack
        add     x17, sp, #96
        sub     x17, x17, x15, lsl #7       // 128 bytes per inner round key, less 96 bytes

        // populate the key schedule
        mov     x9, x3                      // pass key
        mov     x10, x15                    // pass # of rounds
        mov     sp, x17                     // sp is sp
        bl      _bsaes_key_convert
        eor     v7.16b, v7.16b, v15.16b     // fix up last round key
        str     q7, [x17]                   // save last round key

        ldr     q0, [x4]                    // load counter
        add     x13, x11, #.LREVM0SR-.LM0_bigendian
        ldr     q4, [sp]                    // load round0 key

        movi    v8.4s, #1                   // compose 1<<96
        movi    v9.16b, #0
        rev32   v15.16b, v0.16b
        rev32   v0.16b, v0.16b
        ext     v11.16b, v9.16b, v8.16b, #4
        rev32   v4.16b, v4.16b
        add     v12.4s, v11.4s, v11.4s      // compose 2<<96
        str     q4, [sp]                    // save adjusted round0 key
        add     v13.4s, v11.4s, v12.4s      // compose 3<<96
        add     v14.4s, v12.4s, v12.4s      // compose 4<<96
        b       .Lctr_enc_loop

.align  4
.Lctr_enc_loop:
        // Intermix prologue from _bsaes_encrypt8 to use the opportunity
        // to flip byte order in 32-bit counter

        add     v1.4s, v15.4s, v11.4s       // +1
        add     x9, sp, #0x10               // pass next round key
        add     v2.4s, v15.4s, v12.4s       // +2
        ldr     q9, [x13]                   // .LREVM0SR
        ldr     q8, [sp]                    // load round0 key
        add     v3.4s, v15.4s, v13.4s       // +3
        mov     x10, x15                    // pass rounds
        sub     x11, x13, #.LREVM0SR-.LSR   // pass constants
        add     v6.4s, v2.4s, v14.4s
        add     v4.4s, v15.4s, v14.4s       // +4
        add     v7.4s, v3.4s, v14.4s
        add     v15.4s, v4.4s, v14.4s       // next counter
        add     v5.4s, v1.4s, v14.4s

        bl      _bsaes_encrypt8_alt

        subs    x2, x2, #8
        blo     .Lctr_enc_loop_done

        ldr     q16, [x0], #16
        ldr     q17, [x0], #16
        eor     v1.16b, v1.16b, v17.16b
        ldr     q17, [x0], #16
        eor     v0.16b, v0.16b, v16.16b
        eor     v4.16b, v4.16b, v17.16b
        str     q0, [x1], #16
        ldr     q16, [x0], #16
        str     q1, [x1], #16
        mov     v0.16b, v15.16b
        str     q4, [x1], #16
        ldr     q1, [x0], #16
        eor     v4.16b, v6.16b, v16.16b
        eor     v1.16b, v3.16b, v1.16b
        ldr     q3, [x0], #16
        eor     v3.16b, v7.16b, v3.16b
        ldr     q6, [x0], #16
        eor     v2.16b, v2.16b, v6.16b
        ldr     q6, [x0], #16
        eor     v5.16b, v5.16b, v6.16b
        str     q4, [x1], #16
        str     q1, [x1], #16
        str     q3, [x1], #16
        str     q2, [x1], #16
        str     q5, [x1], #16

        bne     .Lctr_enc_loop
        b       .Lctr_enc_done

.align  4
.Lctr_enc_loop_done:
        add     x2, x2, #8
        ldr     q16, [x0], #16              // load input
        eor     v0.16b, v0.16b, v16.16b
        str     q0, [x1], #16               // write output
        cmp     x2, #2
        blo     .Lctr_enc_done
        ldr     q17, [x0], #16
        eor     v1.16b, v1.16b, v17.16b
        str     q1, [x1], #16
        beq     .Lctr_enc_done
        ldr     q18, [x0], #16
        eor     v4.16b, v4.16b, v18.16b
        str     q4, [x1], #16
        cmp     x2, #4
        blo     .Lctr_enc_done
        ldr     q19, [x0], #16
        eor     v6.16b, v6.16b, v19.16b
        str     q6, [x1], #16
        beq     .Lctr_enc_done
        ldr     q20, [x0], #16
        eor     v3.16b, v3.16b, v20.16b
        str     q3, [x1], #16
        cmp     x2, #6
        blo     .Lctr_enc_done
        ldr     q21, [x0], #16
        eor     v7.16b, v7.16b, v21.16b
        str     q7, [x1], #16
        beq     .Lctr_enc_done
        ldr     q22, [x0]
        eor     v2.16b, v2.16b, v22.16b
        str     q2, [x1], #16

.Lctr_enc_done:
        movi    v0.16b, #0
        movi    v1.16b, #0
.Lctr_enc_bzero: // wipe key schedule [if any]
        stp     q0, q1, [sp], #32
        cmp     sp, x14
        bne     .Lctr_enc_bzero

        ldp     d8, d9, [sp, #16]
        ldp     d10, d11, [sp, #32]
        ldp     d12, d13, [sp, #48]
        ldp     d14, d15, [sp, #64]
        ldp     x29, x30, [sp], #80
        ret

.Lctr_enc_short:
        stp     x29, x30, [sp, #-96]!
        stp     x19, x20, [sp, #16]
        stp     x21, x22, [sp, #32]
        str     x23, [sp, #48]

        mov     x19, x0                     // copy arguments
        mov     x20, x1
        mov     x21, x2
        mov     x22, x3
        ldr     w23, [x4, #12]              // load counter .LSW
        ldr     q1, [x4]                    // load whole counter value
#ifdef __AARCH64EL__
        rev     w23, w23
#endif
        str     q1, [sp, #80]               // copy counter value

.Lctr_enc_short_loop:
        add     x0, sp, #80                 // input counter value
        add     x1, sp, #64                 // output on the stack
        mov     x2, x22                     // key

        bl      AES_encrypt

        ldr     q0, [x19], #16              // load input
        ldr     q1, [sp, #64]               // load encrypted counter
        add     x23, x23, #1
#ifdef __AARCH64EL__
        rev     w0, w23
        str     w0, [sp, #80+12]            // next counter value
#else
        str     w23, [sp, #80+12]           // next counter value
#endif
        eor     v0.16b, v0.16b, v1.16b
        str     q0, [x20], #16              // store output
        subs    x21, x21, #1
        bne     .Lctr_enc_short_loop

        movi    v0.16b, #0
        movi    v1.16b, #0
        stp     q0, q1, [sp, #64]

        ldr     x23, [sp, #48]
        ldp     x21, x22, [sp, #32]
        ldp     x19, x20, [sp, #16]
        ldp     x29, x30, [sp], #96
        ret
.size   ossl_bsaes_ctr32_encrypt_blocks,.-ossl_bsaes_ctr32_encrypt_blocks

.globl  ossl_bsaes_xts_encrypt
.type   ossl_bsaes_xts_encrypt,%function
.align  4
// On entry:
//   x0 -> input plaintext
//   x1 -> output ciphertext
//   x2 -> length of text in bytes (must be at least 16)
//   x3 -> key1 (used to encrypt the XORed plaintext blocks)
//   x4 -> key2 (used to encrypt the initial vector to yield the initial tweak)
//   x5 -> 16-byte initial vector (typically, sector number)
// On exit:
//   Output ciphertext filled in
//   No output registers, usual AAPCS64 register preservation
ossl_bsaes_xts_encrypt:
        AARCH64_VALID_CALL_TARGET
        // Stack layout:
        // sp ->
        //        nrounds*128-96 bytes: key schedule
        // x19 ->
        //        16 bytes: frame record
        //        4*16 bytes: tweak storage across _bsaes_encrypt8
        //        6*8 bytes: storage for 5 callee-saved general-purpose registers
        //        8*8 bytes: storage for 8 callee-saved SIMD registers
        stp     x29, x30, [sp, #-192]!
        stp     x19, x20, [sp, #80]
        stp     x21, x22, [sp, #96]
        str     x23, [sp, #112]
        stp     d8, d9, [sp, #128]
        stp     d10, d11, [sp, #144]
        stp     d12, d13, [sp, #160]
        stp     d14, d15, [sp, #176]

        mov     x19, sp
        mov     x20, x0
        mov     x21, x1
        mov     x22, x2
        mov     x23, x3

        // generate initial tweak
        sub     sp, sp, #16
        mov     x0, x5                      // iv[]
        mov     x1, sp
        mov     x2, x4                      // key2
        bl      AES_encrypt
        ldr     q11, [sp], #16

        ldr     w1, [x23, #240]             // get # of rounds
        // allocate the key schedule on the stack
        add     x17, sp, #96
        sub     x17, x17, x1, lsl #7        // 128 bytes per inner round key, less 96 bytes

        // populate the key schedule
        mov     x9, x23                     // pass key
        mov     x10, x1                     // pass # of rounds
        mov     sp, x17
        bl      _bsaes_key_convert
        eor     v15.16b, v15.16b, v7.16b    // fix up last round key
        str     q15, [x17]                  // save last round key

        subs    x22, x22, #0x80
        blo     .Lxts_enc_short
        b       .Lxts_enc_loop

.align  4
.Lxts_enc_loop:
        ldr     q8, .Lxts_magic
        mov     x10, x1                     // pass rounds
        add     x2, x19, #16
        ldr     q0, [x20], #16
        sshr    v1.2d, v11.2d, #63
        mov     x9, sp                      // pass key schedule
        ldr     q6, .Lxts_magic+16
        add     v2.2d, v11.2d, v11.2d
        cmtst   v3.2d, v11.2d, v6.2d
        and     v1.16b, v1.16b, v8.16b
        ext     v1.16b, v1.16b, v1.16b, #8
        and     v3.16b, v3.16b, v8.16b
        ldr     q4, [x20], #16
        eor     v12.16b, v2.16b, v1.16b
        eor     v1.16b, v4.16b, v12.16b
        eor     v0.16b, v0.16b, v11.16b
        cmtst   v2.2d, v12.2d, v6.2d
        add     v4.2d, v12.2d, v12.2d
        add     x0, x19, #16
        ext     v3.16b, v3.16b, v3.16b, #8
        and     v2.16b, v2.16b, v8.16b
        eor     v13.16b, v4.16b, v3.16b
        ldr     q3, [x20], #16
        ext     v4.16b, v2.16b, v2.16b, #8
        eor     v2.16b, v3.16b, v13.16b
        ldr     q3, [x20], #16
        add     v5.2d, v13.2d, v13.2d
        cmtst   v7.2d, v13.2d, v6.2d
        and     v7.16b, v7.16b, v8.16b
        ldr     q9, [x20], #16
        ext     v7.16b, v7.16b, v7.16b, #8
        ldr     q10, [x20], #16
        eor     v14.16b, v5.16b, v4.16b
        ldr     q16, [x20], #16
        add     v4.2d, v14.2d, v14.2d
        eor     v3.16b, v3.16b, v14.16b
        eor     v15.16b, v4.16b, v7.16b
        add     v5.2d, v15.2d, v15.2d
        ldr     q7, [x20], #16
        cmtst   v4.2d, v14.2d, v6.2d
        and     v17.16b, v4.16b, v8.16b
        cmtst   v18.2d, v15.2d, v6.2d
        eor     v4.16b, v9.16b, v15.16b
        ext     v9.16b, v17.16b, v17.16b, #8
        eor     v9.16b, v5.16b, v9.16b
        add     v17.2d, v9.2d, v9.2d
        and     v18.16b, v18.16b, v8.16b
        eor     v5.16b, v10.16b, v9.16b
        str     q9, [x2], #16
        ext     v10.16b, v18.16b, v18.16b, #8
        cmtst   v9.2d, v9.2d, v6.2d
        and     v9.16b, v9.16b, v8.16b
        eor     v10.16b, v17.16b, v10.16b
        cmtst   v17.2d, v10.2d, v6.2d
        eor     v6.16b, v16.16b, v10.16b
        str     q10, [x2], #16
        ext     v9.16b, v9.16b, v9.16b, #8
        add     v10.2d, v10.2d, v10.2d
        eor     v9.16b, v10.16b, v9.16b
        str     q9, [x2], #16
        eor     v7.16b, v7.16b, v9.16b
        add     v9.2d, v9.2d, v9.2d
        and     v8.16b, v17.16b, v8.16b
        ext     v8.16b, v8.16b, v8.16b, #8
        eor     v8.16b, v9.16b, v8.16b
        str     q8, [x2]                    // next round tweak

        bl      _bsaes_encrypt8

        ldr     q8, [x0], #16
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        ldr     q9, [x0], #16
        eor     v4.16b, v4.16b, v13.16b
        eor     v6.16b, v6.16b, v14.16b
        ldr     q10, [x0], #16
        eor     v3.16b, v3.16b, v15.16b
        subs    x22, x22, #0x80
        str     q0, [x21], #16
        ldr     q11, [x0]                   // next round tweak
        str     q1, [x21], #16
        eor     v0.16b, v7.16b, v8.16b
        eor     v1.16b, v2.16b, v9.16b
        str     q4, [x21], #16
        eor     v2.16b, v5.16b, v10.16b
        str     q6, [x21], #16
        str     q3, [x21], #16
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q2, [x21], #16
        bpl     .Lxts_enc_loop

.Lxts_enc_short:
        adds    x22, x22, #0x70
        bmi     .Lxts_enc_done

        ldr     q8, .Lxts_magic
        sshr    v1.2d, v11.2d, #63
        add     v2.2d, v11.2d, v11.2d
        ldr     q9, .Lxts_magic+16
        subs    x22, x22, #0x10
        ldr     q0, [x20], #16
        and     v1.16b, v1.16b, v8.16b
        cmtst   v3.2d, v11.2d, v9.2d
        ext     v1.16b, v1.16b, v1.16b, #8
        and     v3.16b, v3.16b, v8.16b
        eor     v12.16b, v2.16b, v1.16b
        ext     v1.16b, v3.16b, v3.16b, #8
        add     v2.2d, v12.2d, v12.2d
        cmtst   v3.2d, v12.2d, v9.2d
        eor     v13.16b, v2.16b, v1.16b
        and     v22.16b, v3.16b, v8.16b
        bmi     .Lxts_enc_1

        ext     v2.16b, v22.16b, v22.16b, #8
        add     v3.2d, v13.2d, v13.2d
        ldr     q1, [x20], #16
        cmtst   v4.2d, v13.2d, v9.2d
        subs    x22, x22, #0x10
        eor     v14.16b, v3.16b, v2.16b
        and     v23.16b, v4.16b, v8.16b
        bmi     .Lxts_enc_2

        ext     v3.16b, v23.16b, v23.16b, #8
        add     v4.2d, v14.2d, v14.2d
        ldr     q2, [x20], #16
        cmtst   v5.2d, v14.2d, v9.2d
        eor     v0.16b, v0.16b, v11.16b
        subs    x22, x22, #0x10
        eor     v15.16b, v4.16b, v3.16b
        and     v24.16b, v5.16b, v8.16b
        bmi     .Lxts_enc_3

        ext     v4.16b, v24.16b, v24.16b, #8
        add     v5.2d, v15.2d, v15.2d
        ldr     q3, [x20], #16
        cmtst   v6.2d, v15.2d, v9.2d
        eor     v1.16b, v1.16b, v12.16b
        subs    x22, x22, #0x10
        eor     v16.16b, v5.16b, v4.16b
        and     v25.16b, v6.16b, v8.16b
        bmi     .Lxts_enc_4

        ext     v5.16b, v25.16b, v25.16b, #8
        add     v6.2d, v16.2d, v16.2d
        add     x0, x19, #16
        cmtst   v7.2d, v16.2d, v9.2d
        ldr     q4, [x20], #16
        eor     v2.16b, v2.16b, v13.16b
        str     q16, [x0], #16
        subs    x22, x22, #0x10
        eor     v17.16b, v6.16b, v5.16b
        and     v26.16b, v7.16b, v8.16b
        bmi     .Lxts_enc_5

        ext     v7.16b, v26.16b, v26.16b, #8
        add     v18.2d, v17.2d, v17.2d
        ldr     q5, [x20], #16
        eor     v3.16b, v3.16b, v14.16b
        str     q17, [x0], #16
        subs    x22, x22, #0x10
        eor     v18.16b, v18.16b, v7.16b
        bmi     .Lxts_enc_6

        ldr     q6, [x20], #16
        eor     v4.16b, v4.16b, v15.16b
        eor     v5.16b, v5.16b, v16.16b
        str     q18, [x0]                   // next round tweak
        mov     x9, sp                      // pass key schedule
        mov     x10, x1
        add     x0, x19, #16
        sub     x22, x22, #0x10
        eor     v6.16b, v6.16b, v17.16b

        bl      _bsaes_encrypt8

        ldr     q16, [x0], #16
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        ldr     q17, [x0], #16
        eor     v4.16b, v4.16b, v13.16b
        eor     v6.16b, v6.16b, v14.16b
        eor     v3.16b, v3.16b, v15.16b
        ldr     q11, [x0]                   // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        eor     v0.16b, v7.16b, v16.16b
        eor     v1.16b, v2.16b, v17.16b
        str     q4, [x21], #16
        str     q6, [x21], #16
        str     q3, [x21], #16
        str     q0, [x21], #16
        str     q1, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_6:
        eor     v4.16b, v4.16b, v15.16b
        eor     v5.16b, v5.16b, v16.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_encrypt8

        ldr     q16, [x0], #16
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v4.16b, v4.16b, v13.16b
        eor     v6.16b, v6.16b, v14.16b
        ldr     q11, [x0]                   // next round tweak
        eor     v3.16b, v3.16b, v15.16b
        str     q0, [x21], #16
        str     q1, [x21], #16
        eor     v0.16b, v7.16b, v16.16b
        str     q4, [x21], #16
        str     q6, [x21], #16
        str     q3, [x21], #16
        str     q0, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_5:
        eor     v3.16b, v3.16b, v14.16b
        eor     v4.16b, v4.16b, v15.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_encrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        ldr     q11, [x0]                   // next round tweak
        eor     v4.16b, v4.16b, v13.16b
        eor     v6.16b, v6.16b, v14.16b
        eor     v3.16b, v3.16b, v15.16b
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q4, [x21], #16
        str     q6, [x21], #16
        str     q3, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_4:
        eor     v2.16b, v2.16b, v13.16b
        eor     v3.16b, v3.16b, v14.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_encrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v4.16b, v4.16b, v13.16b
        eor     v6.16b, v6.16b, v14.16b
        mov     v11.16b, v15.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q4, [x21], #16
        str     q6, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_3:
        eor     v1.16b, v1.16b, v12.16b
        eor     v2.16b, v2.16b, v13.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_encrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v4.16b, v4.16b, v13.16b
        mov     v11.16b, v14.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q4, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_2:
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_encrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        mov     v11.16b, v13.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        b       .Lxts_enc_done

.align  4
.Lxts_enc_1:
        eor     v0.16b, v0.16b, v11.16b
        sub     x0, sp, #16
        sub     x1, sp, #16
        mov     x2, x23
        mov     v13.d[0], v11.d[1]          // just in case AES_encrypt corrupts top half of callee-saved SIMD registers
        mov     v14.d[0], v12.d[1]
        str     q0, [sp, #-16]!

        bl      AES_encrypt

        ldr     q0, [sp], #16
        trn1    v13.2d, v11.2d, v13.2d
        trn1    v11.2d, v12.2d, v14.2d      // next round tweak
        eor     v0.16b, v0.16b, v13.16b
        str     q0, [x21], #16

.Lxts_enc_done:
        adds    x22, x22, #0x10
        beq     .Lxts_enc_ret

        sub     x6, x21, #0x10
        // Penultimate plaintext block produces final ciphertext part-block
        // plus remaining part of final plaintext block. Move ciphertext part
        // to final position and reuse penultimate ciphertext block buffer to
        // construct final plaintext block
.Lxts_enc_steal:
        ldrb    w0, [x20], #1
        ldrb    w1, [x21, #-0x10]
        strb    w0, [x21, #-0x10]
        strb    w1, [x21], #1

        subs    x22, x22, #1
        bhi     .Lxts_enc_steal

        // Finally encrypt the penultimate ciphertext block using the
        // last tweak
        ldr     q0, [x6]
        eor     v0.16b, v0.16b, v11.16b
        str     q0, [sp, #-16]!
        mov     x0, sp
        mov     x1, sp
        mov     x2, x23
        mov     x21, x6
        mov     v13.d[0], v11.d[1]          // just in case AES_encrypt corrupts top half of callee-saved SIMD registers

        bl      AES_encrypt

        trn1    v11.2d, v11.2d, v13.2d
        ldr     q0, [sp], #16
        eor     v0.16b, v0.16b, v11.16b
        str     q0, [x21]

.Lxts_enc_ret:

        movi    v0.16b, #0
        movi    v1.16b, #0
.Lxts_enc_bzero: // wipe key schedule
        stp     q0, q1, [sp], #32
        cmp     sp, x19
        bne     .Lxts_enc_bzero

        ldp     x19, x20, [sp, #80]
        ldp     x21, x22, [sp, #96]
        ldr     x23, [sp, #112]
        ldp     d8, d9, [sp, #128]
        ldp     d10, d11, [sp, #144]
        ldp     d12, d13, [sp, #160]
        ldp     d14, d15, [sp, #176]
        ldp     x29, x30, [sp], #192
        ret
.size   ossl_bsaes_xts_encrypt,.-ossl_bsaes_xts_encrypt

// The assembler doesn't seem capable of de-duplicating these when expressed
// using `ldr qd,=` syntax, so assign a symbolic address
.align  5
.Lxts_magic:
.quad   1, 0x87, 0x4000000000000000, 0x4000000000000000

.globl  ossl_bsaes_xts_decrypt
.type   ossl_bsaes_xts_decrypt,%function
.align  4
// On entry:
//   x0 -> input ciphertext
//   x1 -> output plaintext
//   x2 -> length of text in bytes (must be at least 16)
//   x3 -> key1 (used to decrypt the XORed ciphertext blocks)
//   x4 -> key2 (used to encrypt the initial vector to yield the initial tweak)
//   x5 -> 16-byte initial vector (typically, sector number)
// On exit:
//   Output plaintext filled in
//   No output registers, usual AAPCS64 register preservation
ossl_bsaes_xts_decrypt:
        AARCH64_VALID_CALL_TARGET
        // Stack layout:
        // sp ->
        //        nrounds*128-96 bytes: key schedule
        // x19 ->
        //        16 bytes: frame record
        //        4*16 bytes: tweak storage across _bsaes_decrypt8
        //        6*8 bytes: storage for 5 callee-saved general-purpose registers
        //        8*8 bytes: storage for 8 callee-saved SIMD registers
        stp     x29, x30, [sp, #-192]!
        stp     x19, x20, [sp, #80]
        stp     x21, x22, [sp, #96]
        str     x23, [sp, #112]
        stp     d8, d9, [sp, #128]
        stp     d10, d11, [sp, #144]
        stp     d12, d13, [sp, #160]
        stp     d14, d15, [sp, #176]

        mov     x19, sp
        mov     x20, x0
        mov     x21, x1
        mov     x22, x2
        mov     x23, x3

        // generate initial tweak
        sub     sp, sp, #16
        mov     x0, x5                      // iv[]
        mov     x1, sp
        mov     x2, x4                      // key2
        bl      AES_encrypt
        ldr     q11, [sp], #16

        ldr     w1, [x23, #240]             // get # of rounds
        // allocate the key schedule on the stack
        add     x17, sp, #96
        sub     x17, x17, x1, lsl #7        // 128 bytes per inner round key, less 96 bytes

        // populate the key schedule
        mov     x9, x23                     // pass key
        mov     x10, x1                     // pass # of rounds
        mov     sp, x17
        bl      _bsaes_key_convert
        ldr     q6,  [sp]
        str     q15, [x17]                  // save last round key
        eor     v6.16b, v6.16b, v7.16b      // fix up round 0 key (by XORing with 0x63)
        str     q6, [sp]

        sub     x30, x22, #0x10
        tst     x22, #0xf                   // if not multiple of 16
        csel    x22, x30, x22, ne           // subtract another 16 bytes
        subs    x22, x22, #0x80

        blo     .Lxts_dec_short
        b       .Lxts_dec_loop

.align  4
.Lxts_dec_loop:
        ldr     q8, .Lxts_magic
        mov     x10, x1                     // pass rounds
        add     x2, x19, #16
        ldr     q0, [x20], #16
        sshr    v1.2d, v11.2d, #63
        mov     x9, sp                      // pass key schedule
        ldr     q6, .Lxts_magic+16
        add     v2.2d, v11.2d, v11.2d
        cmtst   v3.2d, v11.2d, v6.2d
        and     v1.16b, v1.16b, v8.16b
        ext     v1.16b, v1.16b, v1.16b, #8
        and     v3.16b, v3.16b, v8.16b
        ldr     q4, [x20], #16
        eor     v12.16b, v2.16b, v1.16b
        eor     v1.16b, v4.16b, v12.16b
        eor     v0.16b, v0.16b, v11.16b
        cmtst   v2.2d, v12.2d, v6.2d
        add     v4.2d, v12.2d, v12.2d
        add     x0, x19, #16
        ext     v3.16b, v3.16b, v3.16b, #8
        and     v2.16b, v2.16b, v8.16b
        eor     v13.16b, v4.16b, v3.16b
        ldr     q3, [x20], #16
        ext     v4.16b, v2.16b, v2.16b, #8
        eor     v2.16b, v3.16b, v13.16b
        ldr     q3, [x20], #16
        add     v5.2d, v13.2d, v13.2d
        cmtst   v7.2d, v13.2d, v6.2d
        and     v7.16b, v7.16b, v8.16b
        ldr     q9, [x20], #16
        ext     v7.16b, v7.16b, v7.16b, #8
        ldr     q10, [x20], #16
        eor     v14.16b, v5.16b, v4.16b
        ldr     q16, [x20], #16
        add     v4.2d, v14.2d, v14.2d
        eor     v3.16b, v3.16b, v14.16b
        eor     v15.16b, v4.16b, v7.16b
        add     v5.2d, v15.2d, v15.2d
        ldr     q7, [x20], #16
        cmtst   v4.2d, v14.2d, v6.2d
        and     v17.16b, v4.16b, v8.16b
        cmtst   v18.2d, v15.2d, v6.2d
        eor     v4.16b, v9.16b, v15.16b
        ext     v9.16b, v17.16b, v17.16b, #8
        eor     v9.16b, v5.16b, v9.16b
        add     v17.2d, v9.2d, v9.2d
        and     v18.16b, v18.16b, v8.16b
        eor     v5.16b, v10.16b, v9.16b
        str     q9, [x2], #16
        ext     v10.16b, v18.16b, v18.16b, #8
        cmtst   v9.2d, v9.2d, v6.2d
        and     v9.16b, v9.16b, v8.16b
        eor     v10.16b, v17.16b, v10.16b
        cmtst   v17.2d, v10.2d, v6.2d
        eor     v6.16b, v16.16b, v10.16b
        str     q10, [x2], #16
        ext     v9.16b, v9.16b, v9.16b, #8
        add     v10.2d, v10.2d, v10.2d
        eor     v9.16b, v10.16b, v9.16b
        str     q9, [x2], #16
        eor     v7.16b, v7.16b, v9.16b
        add     v9.2d, v9.2d, v9.2d
        and     v8.16b, v17.16b, v8.16b
        ext     v8.16b, v8.16b, v8.16b, #8
        eor     v8.16b, v9.16b, v8.16b
        str     q8, [x2]                    // next round tweak

        bl      _bsaes_decrypt8

        eor     v6.16b, v6.16b, v13.16b
        eor     v0.16b, v0.16b, v11.16b
        ldr     q8, [x0], #16
        eor     v7.16b, v7.16b, v8.16b
        str     q0, [x21], #16
        eor     v0.16b, v1.16b, v12.16b
        ldr     q1, [x0], #16
        eor     v1.16b, v3.16b, v1.16b
        subs    x22, x22, #0x80
        eor     v2.16b, v2.16b, v15.16b
        eor     v3.16b, v4.16b, v14.16b
        ldr     q4, [x0], #16
        str     q0, [x21], #16
        ldr     q11, [x0]                   // next round tweak
        eor     v0.16b, v5.16b, v4.16b
        str     q6, [x21], #16
        str     q3, [x21], #16
        str     q2, [x21], #16
        str     q7, [x21], #16
        str     q1, [x21], #16
        str     q0, [x21], #16
        bpl     .Lxts_dec_loop

.Lxts_dec_short:
        adds    x22, x22, #0x70
        bmi     .Lxts_dec_done

        ldr     q8, .Lxts_magic
        sshr    v1.2d, v11.2d, #63
        add     v2.2d, v11.2d, v11.2d
        ldr     q9, .Lxts_magic+16
        subs    x22, x22, #0x10
        ldr     q0, [x20], #16
        and     v1.16b, v1.16b, v8.16b
        cmtst   v3.2d, v11.2d, v9.2d
        ext     v1.16b, v1.16b, v1.16b, #8
        and     v3.16b, v3.16b, v8.16b
        eor     v12.16b, v2.16b, v1.16b
        ext     v1.16b, v3.16b, v3.16b, #8
        add     v2.2d, v12.2d, v12.2d
        cmtst   v3.2d, v12.2d, v9.2d
        eor     v13.16b, v2.16b, v1.16b
        and     v22.16b, v3.16b, v8.16b
        bmi     .Lxts_dec_1

        ext     v2.16b, v22.16b, v22.16b, #8
        add     v3.2d, v13.2d, v13.2d
        ldr     q1, [x20], #16
        cmtst   v4.2d, v13.2d, v9.2d
        subs    x22, x22, #0x10
        eor     v14.16b, v3.16b, v2.16b
        and     v23.16b, v4.16b, v8.16b
        bmi     .Lxts_dec_2

        ext     v3.16b, v23.16b, v23.16b, #8
        add     v4.2d, v14.2d, v14.2d
        ldr     q2, [x20], #16
        cmtst   v5.2d, v14.2d, v9.2d
        eor     v0.16b, v0.16b, v11.16b
        subs    x22, x22, #0x10
        eor     v15.16b, v4.16b, v3.16b
        and     v24.16b, v5.16b, v8.16b
        bmi     .Lxts_dec_3

        ext     v4.16b, v24.16b, v24.16b, #8
        add     v5.2d, v15.2d, v15.2d
        ldr     q3, [x20], #16
        cmtst   v6.2d, v15.2d, v9.2d
        eor     v1.16b, v1.16b, v12.16b
        subs    x22, x22, #0x10
        eor     v16.16b, v5.16b, v4.16b
        and     v25.16b, v6.16b, v8.16b
        bmi     .Lxts_dec_4

        ext     v5.16b, v25.16b, v25.16b, #8
        add     v6.2d, v16.2d, v16.2d
        add     x0, x19, #16
        cmtst   v7.2d, v16.2d, v9.2d
        ldr     q4, [x20], #16
        eor     v2.16b, v2.16b, v13.16b
        str     q16, [x0], #16
        subs    x22, x22, #0x10
        eor     v17.16b, v6.16b, v5.16b
        and     v26.16b, v7.16b, v8.16b
        bmi     .Lxts_dec_5

        ext     v7.16b, v26.16b, v26.16b, #8
        add     v18.2d, v17.2d, v17.2d
        ldr     q5, [x20], #16
        eor     v3.16b, v3.16b, v14.16b
        str     q17, [x0], #16
        subs    x22, x22, #0x10
        eor     v18.16b, v18.16b, v7.16b
        bmi     .Lxts_dec_6

        ldr     q6, [x20], #16
        eor     v4.16b, v4.16b, v15.16b
        eor     v5.16b, v5.16b, v16.16b
        str     q18, [x0]                   // next round tweak
        mov     x9, sp                      // pass key schedule
        mov     x10, x1
        add     x0, x19, #16
        sub     x22, x22, #0x10
        eor     v6.16b, v6.16b, v17.16b

        bl      _bsaes_decrypt8

        ldr     q16, [x0], #16
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        ldr     q17, [x0], #16
        eor     v6.16b, v6.16b, v13.16b
        eor     v4.16b, v4.16b, v14.16b
        eor     v2.16b, v2.16b, v15.16b
        ldr     q11, [x0]                   // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        eor     v0.16b, v7.16b, v16.16b
        eor     v1.16b, v3.16b, v17.16b
        str     q6, [x21], #16
        str     q4, [x21], #16
        str     q2, [x21], #16
        str     q0, [x21], #16
        str     q1, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_6:
        eor     v4.16b, v4.16b, v15.16b
        eor     v5.16b, v5.16b, v16.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_decrypt8

        ldr     q16, [x0], #16
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v6.16b, v6.16b, v13.16b
        eor     v4.16b, v4.16b, v14.16b
        ldr     q11, [x0]                   // next round tweak
        eor     v2.16b, v2.16b, v15.16b
        str     q0, [x21], #16
        str     q1, [x21], #16
        eor     v0.16b, v7.16b, v16.16b
        str     q6, [x21], #16
        str     q4, [x21], #16
        str     q2, [x21], #16
        str     q0, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_5:
        eor     v3.16b, v3.16b, v14.16b
        eor     v4.16b, v4.16b, v15.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_decrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        ldr     q11, [x0]                   // next round tweak
        eor     v6.16b, v6.16b, v13.16b
        eor     v4.16b, v4.16b, v14.16b
        eor     v2.16b, v2.16b, v15.16b
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q6, [x21], #16
        str     q4, [x21], #16
        str     q2, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_4:
        eor     v2.16b, v2.16b, v13.16b
        eor     v3.16b, v3.16b, v14.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_decrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v6.16b, v6.16b, v13.16b
        eor     v4.16b, v4.16b, v14.16b
        mov     v11.16b, v15.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q6, [x21], #16
        str     q4, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_3:
        eor     v1.16b, v1.16b, v12.16b
        eor     v2.16b, v2.16b, v13.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_decrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        eor     v6.16b, v6.16b, v13.16b
        mov     v11.16b, v14.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        str     q6, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_2:
        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        mov     x9, sp                      // pass key schedule
        mov     x10, x1                     // pass rounds
        add     x0, x19, #16

        bl      _bsaes_decrypt8

        eor     v0.16b, v0.16b, v11.16b
        eor     v1.16b, v1.16b, v12.16b
        mov     v11.16b, v13.16b            // next round tweak
        str     q0, [x21], #16
        str     q1, [x21], #16
        b       .Lxts_dec_done

.align  4
.Lxts_dec_1:
        eor     v0.16b, v0.16b, v11.16b
        sub     x0, sp, #16
        sub     x1, sp, #16
        mov     x2, x23
        mov     v13.d[0], v11.d[1]          // just in case AES_decrypt corrupts top half of callee-saved SIMD registers
        mov     v14.d[0], v12.d[1]
        str     q0, [sp, #-16]!

        bl      AES_decrypt

        ldr     q0, [sp], #16
        trn1    v13.2d, v11.2d, v13.2d
        trn1    v11.2d, v12.2d, v14.2d      // next round tweak
        eor     v0.16b, v0.16b, v13.16b
        str     q0, [x21], #16

.Lxts_dec_done:
        adds    x22, x22, #0x10
        beq     .Lxts_dec_ret

        // calculate one round of extra tweak for the stolen ciphertext
        ldr     q8, .Lxts_magic
        sshr    v6.2d, v11.2d, #63
        and     v6.16b, v6.16b, v8.16b
        add     v12.2d, v11.2d, v11.2d
        ext     v6.16b, v6.16b, v6.16b, #8
        eor     v12.16b, v12.16b, v6.16b

        // perform the final decryption with the last tweak value
        ldr     q0, [x20], #16
        eor     v0.16b, v0.16b, v12.16b
        str     q0, [sp, #-16]!
        mov     x0, sp
        mov     x1, sp
        mov     x2, x23
        mov     v13.d[0], v11.d[1]          // just in case AES_decrypt corrupts top half of callee-saved SIMD registers
        mov     v14.d[0], v12.d[1]

        bl      AES_decrypt

        trn1    v12.2d, v12.2d, v14.2d
        trn1    v11.2d, v11.2d, v13.2d
        ldr     q0, [sp], #16
        eor     v0.16b, v0.16b, v12.16b
        str     q0, [x21]

        mov     x6, x21
        // Penultimate ciphertext block produces final plaintext part-block
        // plus remaining part of final ciphertext block. Move plaintext part
        // to final position and reuse penultimate plaintext block buffer to
        // construct final ciphertext block
.Lxts_dec_steal:
        ldrb    w1, [x21]
        ldrb    w0, [x20], #1
        strb    w1, [x21, #0x10]
        strb    w0, [x21], #1

        subs    x22, x22, #1
        bhi     .Lxts_dec_steal

        // Finally decrypt the penultimate plaintext block using the
        // penultimate tweak
        ldr     q0, [x6]
        eor     v0.16b, v0.16b, v11.16b
        str     q0, [sp, #-16]!
        mov     x0, sp
        mov     x1, sp
        mov     x2, x23
        mov     x21, x6

        bl      AES_decrypt

        trn1    v11.2d, v11.2d, v13.2d
        ldr     q0, [sp], #16
        eor     v0.16b, v0.16b, v11.16b
        str     q0, [x21]

.Lxts_dec_ret:

        movi    v0.16b, #0
        movi    v1.16b, #0
.Lxts_dec_bzero: // wipe key schedule
        stp     q0, q1, [sp], #32
        cmp     sp, x19
        bne     .Lxts_dec_bzero

        ldp     x19, x20, [sp, #80]
        ldp     x21, x22, [sp, #96]
        ldr     x23, [sp, #112]
        ldp     d8, d9, [sp, #128]
        ldp     d10, d11, [sp, #144]
        ldp     d12, d13, [sp, #160]
        ldp     d14, d15, [sp, #176]
        ldp     x29, x30, [sp], #192
        ret
.size   ossl_bsaes_xts_decrypt,.-ossl_bsaes_xts_decrypt
