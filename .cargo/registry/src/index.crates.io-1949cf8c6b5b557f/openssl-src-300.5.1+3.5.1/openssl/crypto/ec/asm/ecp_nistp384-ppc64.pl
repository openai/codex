#! /usr/bin/env perl
# Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# ====================================================================
# Written by Danny Tsen <dtsen@us.ibm.com> # for the OpenSSL project.
#
# Copyright 2025- IBM Corp.
# ====================================================================
#
# p384 lower-level primitives for PPC64.
#


use strict;
use warnings;

my $flavour = shift;
my $output = "";
while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {}
if (!$output) {
        $output = "-";
}

my ($xlate, $dir);
$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}ppc-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/ppc-xlate.pl" and -f $xlate) or
die "can't locate ppc-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

my $code = "";

$code.=<<___;
.machine "any"
.text

.globl  p384_felem_mul
.type   p384_felem_mul,\@function
.align	4
p384_felem_mul:

	stdu	1, -176(1)
	mflr	0
	std	14, 56(1)
	std	15, 64(1)
	std	16, 72(1)
	std	17, 80(1)
	std	18, 88(1)
	std	19, 96(1)
	std	20, 104(1)
	std	21, 112(1)
	std	22, 120(1)

	bl	_p384_felem_mul_core

	mtlr	0
	ld	14, 56(1)
	ld	15, 64(1)
	ld	16, 72(1)
	ld	17, 80(1)
	ld	18, 88(1)
	ld	19, 96(1)
	ld	20, 104(1)
	ld	21, 112(1)
	ld	22, 120(1)
	addi	1, 1, 176
	blr
.size   p384_felem_mul,.-p384_felem_mul

.globl  p384_felem_square
.type   p384_felem_square,\@function
.align	4
p384_felem_square:

	stdu	1, -176(1)
	mflr	0
	std	14, 56(1)
	std	15, 64(1)
	std	16, 72(1)
	std	17, 80(1)

	bl	_p384_felem_square_core

	mtlr	0
	ld	14, 56(1)
	ld	15, 64(1)
	ld	16, 72(1)
	ld	17, 80(1)
	addi	1, 1, 176
	blr
.size   p384_felem_square,.-p384_felem_square

#
# Felem mul core function -
# r3, r4 and r5 need to pre-loaded.
#
.type   _p384_felem_mul_core,\@function
.align	4
_p384_felem_mul_core:

	ld	6,0(4)
	ld	14,0(5)
	ld	7,8(4)
	ld	15,8(5)
	ld	8,16(4)
	ld	16,16(5)
	ld	9,24(4)
	ld	17,24(5)
	ld	10,32(4)
	ld	18,32(5)
	ld	11,40(4)
	ld	19,40(5)
	ld	12,48(4)
	ld	20,48(5)

	# out0
	mulld	21, 14, 6
	mulhdu	22, 14, 6
	std	21, 0(3)
	std	22, 8(3)

	vxor	0, 0, 0

	# out1
	mtvsrdd	32+13, 14, 6
	mtvsrdd	32+14, 7, 15
	vmsumudm 1, 13, 14, 0

	# out2
	mtvsrdd	32+15, 15, 6
	mtvsrdd	32+16, 7, 16
	mtvsrdd	32+17, 0, 8
	mtvsrdd	32+18, 0, 14
	vmsumudm 19, 15, 16, 0
	vmsumudm 2, 17, 18, 19

	# out3
	mtvsrdd	32+13, 16, 6
	mtvsrdd	32+14, 7, 17
	mtvsrdd	32+15, 14, 8
	mtvsrdd	32+16, 9, 15
	vmsumudm 19, 13, 14, 0
	vmsumudm 3, 15, 16, 19

	# out4
	mtvsrdd	32+13, 17, 6
	mtvsrdd	32+14, 7, 18
	mtvsrdd	32+15, 15, 8
	mtvsrdd	32+16, 9, 16
	mtvsrdd	32+17, 0, 10
	mtvsrdd	32+18, 0, 14
	vmsumudm 19, 13, 14, 0
	vmsumudm 4, 15, 16, 19
	vmsumudm 4, 17, 18, 4

	# out5
	mtvsrdd	32+13, 18, 6
	mtvsrdd	32+14, 7, 19
	mtvsrdd	32+15, 16, 8
	mtvsrdd	32+16, 9, 17
	mtvsrdd	32+17, 14, 10
	mtvsrdd	32+18, 11, 15
	vmsumudm 19, 13, 14, 0
	vmsumudm 5, 15, 16, 19
	vmsumudm 5, 17, 18, 5

	stxv	32+1, 16(3)
	stxv	32+2, 32(3)
	stxv	32+3, 48(3)
	stxv	32+4, 64(3)
	stxv	32+5, 80(3)

	# out6
	mtvsrdd	32+13, 19, 6
	mtvsrdd	32+14, 7, 20
	mtvsrdd	32+15, 17, 8
	mtvsrdd	32+16, 9, 18
	mtvsrdd	32+17, 15, 10
	mtvsrdd	32+18, 11, 16
	vmsumudm 19, 13, 14, 0
	vmsumudm 6, 15, 16, 19
	mtvsrdd	32+13, 0, 12
	mtvsrdd	32+14, 0, 14
	vmsumudm 19, 17, 18, 6
	vmsumudm 6, 13, 14, 19

	# out7
	mtvsrdd	32+13, 19, 7
	mtvsrdd	32+14, 8, 20
	mtvsrdd	32+15, 17, 9
	mtvsrdd	32+16, 10, 18
	mtvsrdd	32+17, 15, 11
	mtvsrdd	32+18, 12, 16
	vmsumudm 19, 13, 14, 0
	vmsumudm 7, 15, 16, 19
	vmsumudm 7, 17, 18, 7

	# out8
	mtvsrdd	32+13, 19, 8
	mtvsrdd	32+14, 9, 20
	mtvsrdd	32+15, 17, 10
	mtvsrdd	32+16, 11, 18
	mtvsrdd	32+17, 0, 12
	mtvsrdd	32+18, 0, 16
	vmsumudm 19, 13, 14, 0
	vmsumudm 8, 15, 16, 19
	vmsumudm 8, 17, 18, 8

	# out9
	mtvsrdd	32+13, 19, 9
	mtvsrdd	32+14, 10, 20
	mtvsrdd	32+15, 17, 11
	mtvsrdd	32+16, 12, 18
	vmsumudm 19, 13, 14, 0
	vmsumudm 9, 15, 16, 19

	# out10
	mtvsrdd	32+13, 19, 10
	mtvsrdd	32+14, 11, 20
	mtvsrdd	32+15, 0, 12
	mtvsrdd	32+16, 0, 18
	vmsumudm 19, 13, 14, 0
	vmsumudm 10, 15, 16, 19

	# out11
	mtvsrdd	32+17, 19, 11
	mtvsrdd	32+18, 12, 20
	vmsumudm 11, 17, 18, 0

	stxv	32+6, 96(3)
	stxv	32+7, 112(3)
	stxv	32+8, 128(3)
	stxv	32+9, 144(3)
	stxv	32+10, 160(3)
	stxv	32+11, 176(3)

	# out12
	mulld	21, 20, 12
	mulhdu	22, 20, 12	# out12

	std	21, 192(3)
	std	22, 200(3)

	blr
.size   _p384_felem_mul_core,.-_p384_felem_mul_core

#
# Felem square core function -
# r3 and r4 need to pre-loaded.
#
.type   _p384_felem_square_core,\@function
.align	4
_p384_felem_square_core:

	ld	6, 0(4)
	ld	7, 8(4)
	ld	8, 16(4)
	ld	9, 24(4)
	ld	10, 32(4)
	ld	11, 40(4)
	ld	12, 48(4)

	vxor	0, 0, 0

	# out0
	mulld	14, 6, 6
	mulhdu	15, 6, 6
	std	14, 0(3)
	std	15, 8(3)

	# out1
	add	14, 6, 6
	mtvsrdd	32+13, 0, 14
	mtvsrdd	32+14, 0, 7
	vmsumudm 1, 13, 14, 0

	# out2
	mtvsrdd	32+15, 7, 14
	mtvsrdd	32+16, 7, 8
	vmsumudm 2, 15, 16, 0

	# out3
	add	15, 7, 7
	mtvsrdd	32+13, 8, 14
	mtvsrdd	32+14, 15, 9
	vmsumudm 3, 13, 14, 0

	# out4
	mtvsrdd	32+13, 9, 14
	mtvsrdd	32+14, 15, 10
	mtvsrdd	32+15, 0, 8
	vmsumudm 4, 13, 14, 0
	vmsumudm 4, 15, 15, 4

	# out5
	mtvsrdd	32+13, 10, 14
	mtvsrdd	32+14, 15, 11
	add	16, 8, 8
	mtvsrdd	32+15, 0, 16
	mtvsrdd	32+16, 0, 9
	vmsumudm 5, 13, 14, 0
	vmsumudm 5, 15, 16, 5

	stxv	32+1, 16(3)
	stxv	32+2, 32(3)
	stxv	32+3, 48(3)
	stxv	32+4, 64(3)

	# out6
	mtvsrdd	32+13, 11, 14
	mtvsrdd	32+14, 15, 12
	mtvsrdd	32+15, 9, 16
	mtvsrdd	32+16, 9, 10
	stxv	32+5, 80(3)
	vmsumudm 19, 13, 14, 0
	vmsumudm 6, 15, 16, 19

	# out7
	add	17, 9, 9
	mtvsrdd	32+13, 11, 15
	mtvsrdd	32+14, 16, 12
	mtvsrdd	32+15, 0, 17
	mtvsrdd	32+16, 0, 10
	vmsumudm 19, 13, 14, 0
	vmsumudm 7, 15, 16, 19

	# out8
	mtvsrdd	32+13, 11, 16
	mtvsrdd	32+14, 17, 12
	mtvsrdd	32+15, 0, 10
	vmsumudm 19, 13, 14, 0
	vmsumudm 8, 15, 15, 19

	# out9
	add	14, 10, 10
	mtvsrdd	32+13, 11, 17
	mtvsrdd	32+14, 14, 12
	vmsumudm 9, 13, 14, 0

	# out10
	mtvsrdd	32+13, 11, 14
	mtvsrdd	32+14, 11, 12
	vmsumudm 10, 13, 14, 0

	stxv	32+6, 96(3)
	stxv	32+7, 112(3)

	# out11
	#add	14, 11, 11
	#mtvsrdd	32+13, 0, 14
	#mtvsrdd	32+14, 0, 12
	#vmsumudm 11, 13, 14, 0

	mulld	6, 12, 11
	mulhdu	7, 12, 11
	addc	8, 6, 6
	adde	9, 7, 7

	stxv	32+8, 128(3)
	stxv	32+9, 144(3)
	stxv	32+10, 160(3)
	#stxv	32+11, 176(3)

	# out12
	mulld	14, 12, 12
	mulhdu	15, 12, 12

	std	8, 176(3)
	std	9, 184(3)
	std	14, 192(3)
	std	15, 200(3)

	blr
.size   _p384_felem_square_core,.-_p384_felem_square_core

#
# widefelem (128 bits) * 8
#
.macro F128_X_8 _off1 _off2
	ld	9,\\_off1(3)
	ld	8,\\_off2(3)
	srdi	10,9,61
	rldimi	10,8,3,0
	sldi	9,9,3
	std	9,\\_off1(3)
	std	10,\\_off2(3)
.endm

.globl p384_felem128_mul_by_8
.type	p384_felem128_mul_by_8, \@function
.align 4
p384_felem128_mul_by_8:

	F128_X_8 0, 8

	F128_X_8 16, 24

	F128_X_8 32, 40

	F128_X_8 48, 56

	F128_X_8 64, 72

	F128_X_8 80, 88

	F128_X_8 96, 104

	F128_X_8 112, 120

	F128_X_8 128, 136

	F128_X_8 144, 152

	F128_X_8 160, 168

	F128_X_8 176, 184

	F128_X_8 192, 200

	blr
.size	p384_felem128_mul_by_8,.-p384_felem128_mul_by_8

#
# widefelem (128 bits) * 2
#
.macro F128_X_2 _off1 _off2
	ld	9,\\_off1(3)
	ld	8,\\_off2(3)
	srdi	10,9,63
	rldimi	10,8,1,0
	sldi	9,9,1
	std	9,\\_off1(3)
	std	10,\\_off2(3)
.endm

.globl p384_felem128_mul_by_2
.type	p384_felem128_mul_by_2, \@function
.align 4
p384_felem128_mul_by_2:

	F128_X_2 0, 8

	F128_X_2 16, 24

	F128_X_2 32, 40

	F128_X_2 48, 56

	F128_X_2 64, 72

	F128_X_2 80, 88

	F128_X_2 96, 104

	F128_X_2 112, 120

	F128_X_2 128, 136

	F128_X_2 144, 152

	F128_X_2 160, 168

	F128_X_2 176, 184

	F128_X_2 192, 200

	blr
.size	p384_felem128_mul_by_2,.-p384_felem128_mul_by_2

.globl p384_felem_diff128
.type	p384_felem_diff128, \@function
.align 4
p384_felem_diff128:

	addis   5, 2, .LConst_two127\@toc\@ha
	addi    5, 5, .LConst_two127\@toc\@l

	ld	10, 0(3)
	ld	8, 8(3)
	li	9, 0
	addc	10, 10, 9
	li	7, -1
	rldicr	7, 7, 0, 0	# two127
	adde	8, 8, 7
	ld	11, 0(4)
	ld	12, 8(4)
	subfc	11, 11, 10
	subfe	12, 12, 8
	std	11, 0(3)	# out0
	std	12, 8(3)

	# two127m71 = (r10, r9)
	ld	8, 16(3)
	ld	7, 24(3)
	ld	10, 24(5)	# two127m71
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 16(4)
	ld	12, 24(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 16(3)	# out1
	std	12, 24(3)

	ld	8, 32(3)
	ld	7, 40(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 32(4)
	ld	12, 40(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 32(3)	# out2
	std	12, 40(3)

	ld	8, 48(3)
	ld	7, 56(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 48(4)
	ld	12, 56(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 48(3)	# out3
	std	12, 56(3)

	ld	8, 64(3)
	ld	7, 72(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 64(4)
	ld	12, 72(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 64(3)	# out4
	std	12, 72(3)

	ld	8, 80(3)
	ld	7, 88(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 80(4)
	ld	12, 88(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 80(3)	# out5
	std	12, 88(3)

	ld	8, 96(3)
	ld	7, 104(3)
	ld	6, 40(5)	# two127p111m79m71
	addc	8, 8, 9
	adde	7, 7, 6
	ld	11, 96(4)
	ld	12, 104(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 96(3)	# out6
	std	12, 104(3)

	ld	8, 112(3)
	ld	7, 120(3)
	ld	6, 56(5)	# two127m119m71
	addc	8, 8, 9
	adde	7, 7, 6
	ld	11, 112(4)
	ld	12, 120(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 112(3)	# out7
	std	12, 120(3)

	ld	8, 128(3)
	ld	7, 136(3)
	ld	6, 72(5)	# two127m95m71
	addc	8, 8, 9
	adde	7, 7, 6
	ld	11, 128(4)
	ld	12, 136(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 128(3)	# out8
	std	12, 136(3)

	ld	8, 144(3)
	ld	7, 152(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 144(4)
	ld	12, 152(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 144(3)	# out9
	std	12, 152(3)

	ld	8, 160(3)
	ld	7, 168(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 160(4)
	ld	12, 168(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 160(3)	# out10
	std	12, 168(3)

	ld	8, 176(3)
	ld	7, 184(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 176(4)
	ld	12, 184(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 176(3)	# out11
	std	12, 184(3)

	ld	8, 192(3)
	ld	7, 200(3)
	addc	8, 8, 9
	adde	7, 7, 10
	ld	11, 192(4)
	ld	12, 200(4)
	subfc	11, 11, 8
	subfe	12, 12, 7
	std	11, 192(3)	# out12
	std	12, 200(3)

	blr
.size	p384_felem_diff128,.-p384_felem_diff128

.data
.align 4
.LConst_two127:
#two127
.long 0x00000000, 0x00000000, 0x00000000, 0x80000000
#two127m71
.long 0x00000000, 0x00000000, 0xffffff80, 0x7fffffff
#two127p111m79m71
.long 0x00000000, 0x00000000, 0xffff7f80, 0x80007fff
#two127m119m71
.long 0x00000000, 0x00000000, 0xffffff80, 0x7f7fffff
#two127m95m71
.long 0x00000000, 0x00000000, 0x7fffff80, 0x7fffffff

.text

.globl p384_felem_diff_128_64
.type	p384_felem_diff_128_64, \@function
.align 4
p384_felem_diff_128_64:
	addis   5, 2, .LConst_128_two64\@toc\@ha
	addi    5, 5, .LConst_128_two64\@toc\@l

	ld	9, 0(3)
	ld	10, 8(3)
	ld	8, 48(5)	# two64p48m16
	li	7, 0
	addc	9, 9, 8
	li	6, 1
	adde	10, 10, 6
	ld	11, 0(4)
	subfc	8, 11, 9
	subfe	12, 7, 10
	std	8, 0(3)		# out0
	std	12, 8(3)

	ld	9, 16(3)
	ld	10, 24(3)
	ld	8, 0(5)		# two64m56m8
	addc	9, 9, 8
	addze	10, 10
	ld	11, 8(4)
	subfc	11, 11, 9
	subfe	12, 7, 10
	std	11, 16(3)	# out1
	std	12, 24(3)

	ld	9, 32(3)
	ld	10, 40(3)
	ld	8, 16(5)	# two64m32m8
	addc	9, 9, 8
	addze	10, 10
	ld	11, 16(4)
	subfc	11, 11, 9
	subfe	12, 7, 10
	std	11, 32(3)	# out2
	std	12, 40(3)

	ld	10, 48(3)
	ld	8, 56(3)
	#ld	9, 32(5)	# two64m8
	li	9, -256		# two64m8
	addc	10, 10, 9
	addze	8, 8
	ld	11, 24(4)
	subfc	11, 11, 10
	subfe	12, 7, 8
	std	11, 48(3)	# out3
	std	12, 56(3)

	ld	10, 64(3)
	ld	8, 72(3)
	addc	10, 10, 9
	addze	8, 8
	ld	11, 32(4)
	subfc	11, 11, 10
	subfe	12, 7, 8
	std	11, 64(3)	# out4
	std	12, 72(3)

	ld	10, 80(3)
	ld	8, 88(3)
	addc	10, 10, 9
	addze	8, 8
	ld	11, 40(4)
	subfc	11, 11, 10
	subfe	12, 7, 8
	std	11, 80(3)	# out5
	std	12, 88(3)

	ld	10, 96(3)
	ld	8, 104(3)
	addc	10, 10, 9
	addze	9, 8
	ld	11, 48(4)
	subfc	11, 11, 10
	subfe	12, 7, 9
	std	11, 96(3)	# out6
	std	12, 104(3)

	blr
.size	p384_felem_diff_128_64,.-p384_felem_diff_128_64

.data
.align 4
.LConst_128_two64:
#two64m56m8
.long 0xffffff00, 0xfeffffff, 0x00000000, 0x00000000
#two64m32m8
.long 0xffffff00, 0xfffffffe, 0x00000000, 0x00000000
#two64m8
.long 0xffffff00, 0xffffffff, 0x00000000, 0x00000000
#two64p48m16
.long 0xffff0000, 0x0000ffff, 0x00000001, 0x00000000

.LConst_two60:
#two60m52m4
.long 0xfffffff0, 0x0fefffff, 0x0, 0x0
#two60p44m12
.long 0xfffff000, 0x10000fff, 0x0, 0x0
#two60m28m4
.long 0xeffffff0, 0x0fffffff, 0x0, 0x0
#two60m4
.long 0xfffffff0, 0x0fffffff, 0x0, 0x0

.text
#
# static void felem_diff64(felem out, const felem in)
#
.globl p384_felem_diff64
.type	p384_felem_diff64, \@function
.align 4
p384_felem_diff64:
	addis   5, 2, .LConst_two60\@toc\@ha
	addi    5, 5, .LConst_two60\@toc\@l

	ld	9, 0(3)
	ld	8, 16(5)	# two60p44m12
	li	7, 0
	add	9, 9, 8
	ld	11, 0(4)
	subf	8, 11, 9
	std	8, 0(3)		# out0

	ld	9, 8(3)
	ld	8, 0(5)		# two60m52m4
	add	9, 9, 8
	ld	11, 8(4)
	subf	11, 11, 9
	std	11, 8(3)	# out1

	ld	9, 16(3)
	ld	8, 32(5)	# two60m28m4
	add	9, 9, 8
	ld	11, 16(4)
	subf	11, 11, 9
	std	11, 16(3)	# out2

	ld	10, 24(3)
	ld	9, 48(5)	# two60m4
	add	10, 10, 9
	ld	12, 24(4)
	subf	12, 12, 10
	std	12, 24(3)	# out3

	ld	10, 32(3)
	add	10, 10, 9
	ld	11, 32(4)
	subf	11, 11, 10
	std	11, 32(3)	# out4

	ld	10, 40(3)
	add	10, 10, 9
	ld	12, 40(4)
	subf	12, 12, 10
	std	12, 40(3)	# out5

	ld	10, 48(3)
	add	10, 10, 9
	ld	11, 48(4)
	subf	11, 11, 10
	std	11, 48(3)	# out6

	blr
.size	p384_felem_diff64,.-p384_felem_diff64

.text
#
# Shift 128 bits right <nbits>
#
.macro SHR o_h o_l in_h in_l nbits
	srdi	\\o_l, \\in_l, \\nbits		# shift lower right <nbits>
	rldimi	\\o_l, \\in_h, 64-\\nbits, 0	# insert <64-nbits> from hi
	srdi	\\o_h, \\in_h, \\nbits		# shift higher right <nbits>
.endm

#
# static void felem_reduce(felem out, const widefelem in)
#
.global p384_felem_reduce
.type   p384_felem_reduce,\@function
.align 4
p384_felem_reduce:

	stdu    1, -208(1)
	mflr	0
	std     14, 56(1)
	std     15, 64(1)
	std     16, 72(1)
	std     17, 80(1)
	std     18, 88(1)
	std     19, 96(1)
	std     20, 104(1)
	std     21, 112(1)
	std     22, 120(1)
	std     23, 128(1)
	std     24, 136(1)
	std     25, 144(1)
	std     26, 152(1)
	std     27, 160(1)
	std     28, 168(1)
	std     29, 176(1)
	std     30, 184(1)
	std     31, 192(1)

	bl	_p384_felem_reduce_core

	mtlr	0
	ld     14, 56(1)
	ld     15, 64(1)
	ld     16, 72(1)
	ld     17, 80(1)
	ld     18, 88(1)
	ld     19, 96(1)
	ld     20, 104(1)
	ld     21, 112(1)
	ld     22, 120(1)
	ld     23, 128(1)
	ld     24, 136(1)
	ld     25, 144(1)
	ld     26, 152(1)
	ld     27, 160(1)
	ld     28, 168(1)
	ld     29, 176(1)
	ld     30, 184(1)
	ld     31, 192(1)
	addi	1, 1, 208
	blr
.size   p384_felem_reduce,.-p384_felem_reduce

#
# Felem reduction core function -
# r3 and r4 need to pre-loaded.
#
.type   _p384_felem_reduce_core,\@function
.align 4
_p384_felem_reduce_core:
	addis   12, 2, .LConst\@toc\@ha
	addi    12, 12, .LConst\@toc\@l

	# load constat p
	ld	11, 8(12)	# hi - two124m68

	# acc[6] = in[6] + two124m68;
	ld	26, 96(4)	# in[6].l
	ld	27, 96+8(4)	# in[6].h
	add	27, 27, 11

	# acc[5] = in[5] + two124m68;
	ld	24, 80(4)	# in[5].l
	ld	25, 80+8(4)	# in[5].h
	add	25, 25, 11

	# acc[4] = in[4] + two124m68;
	ld	22, 64(4)	# in[4].l
	ld	23, 64+8(4)	# in[4].h
	add	23, 23, 11

	# acc[3] = in[3] + two124m68;
	ld	20, 48(4)	# in[3].l
	ld	21, 48+8(4)	# in[3].h
	add	21, 21, 11

	ld	11, 48+8(12)	# hi - two124m92m68

	# acc[2] = in[2] + two124m92m68;
	ld	18, 32(4)	# in[2].l
	ld	19, 32+8(4)	# in[2].h
	add	19, 19, 11

	ld	11, 16+8(12)	# high - two124m116m68

	# acc[1] = in[1] + two124m116m68;
	ld	16, 16(4)	# in[1].l
	ld	17, 16+8(4)	# in[1].h
	add	17, 17, 11

	ld	11, 32+8(12)	# high - two124p108m76

	# acc[0] = in[0] + two124p108m76;
	ld	14, 0(4)	# in[0].l
	ld	15, 0+8(4)	# in[0].h
	add	15, 15, 11

	# compute mask
	li	7, -1

	# Eliminate in[12]

	# acc[8] += in[12] >> 32;
	ld	5, 192(4)	# in[12].l
	ld	6, 192+8(4)	# in[12].h
	SHR 9, 10, 6, 5, 32
	ld	30, 128(4)	# in[8].l
	ld	31, 136(4)	# in[8].h
	addc	30, 30, 10
	adde	31, 31, 9

	# acc[7] += (in[12] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	ld	28, 112(4)	# in[7].l
	ld	29, 120(4)	# in[7].h
	addc	28, 28, 11
	addze	29, 29

	# acc[7] += in[12] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	28, 28, 10
	adde	29, 29, 9

	# acc[6] += (in[12] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	26, 26, 11
	addze	27, 27

	# acc[6] -= in[12] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	26, 10, 26
	subfe	27, 9, 27

	# acc[5] -= (in[12] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	24, 11, 24
	subfe	25, 9, 25

	# acc[6] += in[12] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	26, 26, 10
	adde	27, 27, 9

	# acc[5] += (in[12] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	24, 24, 11
	addze	25, 25

	# Eliminate in[11]

	# acc[7] += in[11] >> 32;
	ld	5, 176(4)	# in[11].l
	ld	6, 176+8(4)	# in[11].h
	SHR 9, 10, 6, 5, 32
	addc	28, 28, 10
	adde	29, 29, 9

	# acc[6] += (in[11] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	addc	26, 26, 11
	addze	27, 27

	# acc[6] += in[11] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	26, 26, 10
	adde	27, 27, 9

	# acc[5] += (in[11] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	24, 24, 11
	addze	25, 25

	# acc[5] -= in[11] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	24, 10, 24
	subfe	25, 9, 25

	# acc[4] -= (in[11] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	22, 11, 22
	subfe	23, 9, 23

	# acc[5] += in[11] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	24, 24, 10
	adde	25, 25, 9

	# acc[4] += (in[11] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	22, 22, 11
	addze	23, 23

	# Eliminate in[10]

	# acc[6] += in[10] >> 32;
	ld	5, 160(4)	# in[10].l
	ld	6, 160+8(4)	# in[10].h
	SHR 9, 10, 6, 5, 32
	addc	26, 26, 10
	adde	27, 27, 9

	# acc[5] += (in[10] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	addc	24, 24, 11
	addze	25, 25

	# acc[5] += in[10] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	24, 24, 10
	adde	25, 25, 9

	# acc[4] += (in[10] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	22, 22, 11
	addze	23, 23

	# acc[4] -= in[10] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	22, 10, 22
	subfe	23, 9, 23

	# acc[3] -= (in[10] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	20, 11, 20
	subfe	21, 9, 21

	# acc[4] += in[10] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	22, 22, 10
	adde	23, 23, 9

	# acc[3] += (in[10] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	20, 20, 11
	addze	21, 21

	# Eliminate in[9]

	# acc[5] += in[9] >> 32;
	ld	5, 144(4)	# in[9].l
	ld	6, 144+8(4)	# in[9].h
	SHR 9, 10, 6, 5, 32
	addc	24, 24, 10
	adde	25, 25, 9

	# acc[4] += (in[9] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	addc	22, 22, 11
	addze	23, 23

	# acc[4] += in[9] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	22, 22, 10
	adde	23, 23, 9

	# acc[3] += (in[9] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	20, 20, 11
	addze	21, 21

	# acc[3] -= in[9] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	20, 10, 20
	subfe	21, 9, 21

	# acc[2] -= (in[9] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	18, 11, 18
	subfe	19, 9, 19

	# acc[3] += in[9] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	20, 20, 10
	adde	21, 21, 9

	# acc[2] += (in[9] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	18, 18, 11
	addze	19, 19

	# Eliminate acc[8]

	# acc[4] += acc[8] >> 32;
	mr	5, 30		# acc[8].l
	mr	6, 31		# acc[8].h
	SHR 9, 10, 6, 5, 32
	addc	22, 22, 10
	adde	23, 23, 9

	# acc[3] += (acc[8] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	addc	20, 20, 11
	addze	21, 21

	# acc[3] += acc[8] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	20, 20, 10
	adde	21, 21, 9

	# acc[2] += (acc[8] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	18, 18, 11
	addze	19, 19

	# acc[2] -= acc[8] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	18, 10, 18
	subfe	19, 9, 19

	# acc[1] -= (acc[8] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	16, 11, 16
	subfe	17, 9, 17

	#acc[2] += acc[8] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	18, 18, 10
	adde	19, 19, 9

	# acc[1] += (acc[8] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	16, 16, 11
	addze	17, 17

	# Eliminate acc[7]

	# acc[3] += acc[7] >> 32;
	mr	5, 28		# acc[7].l
	mr	6, 29		# acc[7].h
	SHR 9, 10, 6, 5, 32
	addc	20, 20, 10
	adde	21, 21, 9

	# acc[2] += (acc[7] & 0xffffffff) << 24;
	srdi	11, 7, 32	# 0xffffffff
	and	11, 11, 5
	sldi	11, 11, 24	# << 24
	addc	18, 18, 11
	addze	19, 19

	# acc[2] += acc[7] >> 8;
	SHR 9, 10, 6, 5, 8
	addc	18, 18, 10
	adde	19, 19, 9

	# acc[1] += (acc[7] & 0xff) << 48;
	andi.	11, 5, 0xff
	sldi	11, 11, 48
	addc	16, 16, 11
	addze	17, 17

	# acc[1] -= acc[7] >> 16;
	SHR 9, 10, 6, 5, 16
	subfc	16, 10, 16
	subfe	17, 9, 17

	# acc[0] -= (acc[7] & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	11, 11, 5
	sldi	11, 11, 40	# << 40
	li	9, 0
	subfc	14, 11, 14
	subfe	15, 9, 15

	# acc[1] += acc[7] >> 48;
	SHR 9, 10, 6, 5, 48
	addc	16, 16, 10
	adde	17, 17, 9

	# acc[0] += (acc[7] & 0xffffffffffff) << 8;
	srdi	11, 7, 16	# 0xffffffffffff
	and	11, 11, 5
	sldi	11, 11, 8	# << 8
	addc	14, 14, 11
	addze	15, 15

	#
	# Carry 4 -> 5 -> 6
	#
	# acc[5] += acc[4] >> 56;
	# acc[4] &= 0x00ffffffffffffff;
	SHR 9, 10, 23, 22, 56
	addc	24, 24, 10
	adde	25, 25, 9
	srdi	11, 7, 8	# 0x00ffffffffffffff
	and	22, 22, 11
	li	23, 0

	# acc[6] += acc[5] >> 56;
	# acc[5] &= 0x00ffffffffffffff;
	SHR 9, 10, 25, 24, 56
	addc	26, 26, 10
	adde	27, 27, 9
	and	24, 24, 11
	li	25, 0

	# [3]: Eliminate high bits of acc[6] */
	# temp = acc[6] >> 48;
	# acc[6] &= 0x0000ffffffffffff;
	SHR 31, 30, 27, 26, 48	# temp = acc[6] >> 48
	srdi	11, 7, 16	# 0x0000ffffffffffff
	and	26, 26, 11
	li	27, 0

	# temp < 2^80
	# acc[3] += temp >> 40;
	SHR 9, 10, 31, 30, 40
	addc	20, 20, 10
	adde	21, 21, 9

	# acc[2] += (temp & 0xffffffffff) << 16;
	srdi	11, 7, 24	# 0xffffffffff
	and	10, 30, 11
	sldi	10, 10, 16
	addc	18, 18, 10
	addze	19, 19

	# acc[2] += temp >> 16;
	SHR 9, 10, 31, 30, 16
	addc	18, 18, 10
	adde	19, 19, 9

	# acc[1] += (temp & 0xffff) << 40;
	srdi	11, 7, 48	# 0xffff
	and	10, 30, 11
	sldi	10, 10, 40
	addc	16, 16, 10
	addze	17, 17

	# acc[1] -= temp >> 24;
	SHR 9, 10, 31, 30, 24
	subfc	16, 10, 16
	subfe	17, 9, 17

	# acc[0] -= (temp & 0xffffff) << 32;
	srdi	11, 7, 40	# 0xffffff
	and	10, 30, 11
	sldi	10, 10, 32
	li	9, 0
	subfc	14, 10, 14
	subfe	15, 9, 15

	# acc[0] += temp;
	addc	14, 14, 30
	adde	15, 15, 31

	# Carry 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6
	#
	# acc[1] += acc[0] >> 56;   /* acc[1] < acc_old[1] + 2^72 */
	SHR 9, 10, 15, 14, 56
	addc	16, 16, 10
	adde	17, 17, 9

	# acc[0] &= 0x00ffffffffffffff;
	srdi	11, 7, 8	# 0x00ffffffffffffff
	and	14, 14, 11
	li	15, 0

	# acc[2] += acc[1] >> 56;   /* acc[2] < acc_old[2] + 2^72 + 2^16 */
	SHR 9, 10, 17, 16, 56
	addc	18, 18, 10
	adde	19, 19, 9

	# acc[1] &= 0x00ffffffffffffff;
	and	16, 16, 11
	li	17, 0

	# acc[3] += acc[2] >> 56;   /* acc[3] < acc_old[3] + 2^72 + 2^16 */
	SHR 9, 10, 19, 18, 56
	addc	20, 20, 10
	adde	21, 21, 9

	# acc[2] &= 0x00ffffffffffffff;
	and	18, 18, 11
	li	19, 0

	# acc[4] += acc[3] >> 56;
	SHR 9, 10, 21, 20, 56
	addc	22, 22, 10
	adde	23, 23, 9

	# acc[3] &= 0x00ffffffffffffff;
	and	20, 20, 11
	li	21, 0

	# acc[5] += acc[4] >> 56;
	SHR 9, 10, 23, 22, 56
	addc	24, 24, 10
	adde	25, 25, 9

	# acc[4] &= 0x00ffffffffffffff;
	and	22, 22, 11

	# acc[6] += acc[5] >> 56;
	SHR 9, 10, 25, 24, 56
	addc	26, 26, 10
	adde	27, 27, 9

	# acc[5] &= 0x00ffffffffffffff;
	and	24, 24, 11

	std	14, 0(3)
	std	16, 8(3)
	std	18, 16(3)
	std	20, 24(3)
	std	22, 32(3)
	std	24, 40(3)
	std	26, 48(3)
	blr
.size   _p384_felem_reduce_core,.-_p384_felem_reduce_core

.data
.align 4
.LConst:
# two124m68:
.long 0x0, 0x0, 0xfffffff0, 0xfffffff
# two124m116m68:
.long 0x0, 0x0, 0xfffffff0, 0xfefffff
#two124p108m76:
.long 0x0, 0x0, 0xfffff000, 0x10000fff
#two124m92m68:
.long 0x0, 0x0, 0xeffffff0, 0xfffffff

.text

#
# void p384_felem_square_reduce(felem out, const felem in)
#
.global p384_felem_square_reduce
.type   p384_felem_square_reduce,\@function
.align 4
p384_felem_square_reduce:
	stdu    1, -512(1)
	mflr	0
	std     14, 56(1)
	std     15, 64(1)
	std     16, 72(1)
	std     17, 80(1)
	std     18, 88(1)
	std     19, 96(1)
	std     20, 104(1)
	std     21, 112(1)
	std     22, 120(1)
	std     23, 128(1)
	std     24, 136(1)
	std     25, 144(1)
	std     26, 152(1)
	std     27, 160(1)
	std     28, 168(1)
	std     29, 176(1)
	std     30, 184(1)
	std     31, 192(1)

	std	3, 496(1)
	addi	3, 1, 208
	bl _p384_felem_square_core

	mr	4, 3
	ld	3, 496(1)
	bl _p384_felem_reduce_core

	ld     14, 56(1)
	ld     15, 64(1)
	ld     16, 72(1)
	ld     17, 80(1)
	ld     18, 88(1)
	ld     19, 96(1)
	ld     20, 104(1)
	ld     21, 112(1)
	ld     22, 120(1)
	ld     23, 128(1)
	ld     24, 136(1)
	ld     25, 144(1)
	ld     26, 152(1)
	ld     27, 160(1)
	ld     28, 168(1)
	ld     29, 176(1)
	ld     30, 184(1)
	ld     31, 192(1)
	addi	1, 1, 512
	mtlr	0
	blr
.size   p384_felem_square_reduce,.-p384_felem_square_reduce

#
# void p384_felem_mul_reduce(felem out, const felem in1, const felem in2)
#
.global p384_felem_mul_reduce
.type   p384_felem_mul_reduce,\@function
.align 5
p384_felem_mul_reduce:
	stdu    1, -512(1)
	mflr	0
	std     14, 56(1)
	std     15, 64(1)
	std     16, 72(1)
	std     17, 80(1)
	std     18, 88(1)
	std     19, 96(1)
	std     20, 104(1)
	std     21, 112(1)
	std     22, 120(1)
	std     23, 128(1)
	std     24, 136(1)
	std     25, 144(1)
	std     26, 152(1)
	std     27, 160(1)
	std     28, 168(1)
	std     29, 176(1)
	std     30, 184(1)
	std     31, 192(1)

	std	3, 496(1)
	addi	3, 1, 208
	bl _p384_felem_mul_core

	mr	4, 3
	ld	3, 496(1)
	bl _p384_felem_reduce_core

	ld     14, 56(1)
	ld     15, 64(1)
	ld     16, 72(1)
	ld     17, 80(1)
	ld     18, 88(1)
	ld     19, 96(1)
	ld     20, 104(1)
	ld     21, 112(1)
	ld     22, 120(1)
	ld     23, 128(1)
	ld     24, 136(1)
	ld     25, 144(1)
	ld     26, 152(1)
	ld     27, 160(1)
	ld     28, 168(1)
	ld     29, 176(1)
	ld     30, 184(1)
	ld     31, 192(1)
	addi	1, 1, 512
	mtlr	0
	blr
.size   p384_felem_mul_reduce,.-p384_felem_mul_reduce
___

$code =~ s/\`([^\`]*)\`/eval $1/gem;
print $code;
close STDOUT or die "error closing STDOUT: $!";
