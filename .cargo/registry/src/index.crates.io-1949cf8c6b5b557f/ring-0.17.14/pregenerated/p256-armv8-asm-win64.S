// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_AARCH64) && defined(_WIN32)
.section	.rodata
.align	5
Lpoly:
.quad	0xffffffffffffffff,0x00000000ffffffff,0x0000000000000000,0xffffffff00000001
LRR:	//	2^512 mod P precomputed for NIST P256 polynomial
.quad	0x0000000000000003,0xfffffffbffffffff,0xfffffffffffffffe,0x00000004fffffffd
Lone_mont:
.quad	0x0000000000000001,0xffffffff00000000,0xffffffffffffffff,0x00000000fffffffe
Lone:
.quad	1,0,0,0
Lord:
.quad	0xf3b9cac2fc632551,0xbce6faada7179e84,0xffffffffffffffff,0xffffffff00000000
LordK:
.quad	0xccd1c8aaee00bc4f
.byte	69,67,80,95,78,73,83,84,90,50,53,54,32,102,111,114,32,65,82,77,118,56,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
.align	2
.text

// void	ecp_nistz256_mul_mont(BN_ULONG x0[4],const BN_ULONG x1[4],
//					     const BN_ULONG x2[4]);
.globl	ecp_nistz256_mul_mont

.def ecp_nistz256_mul_mont
   .type 32
.endef
.align	4
ecp_nistz256_mul_mont:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-32]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]

	ldr	x3,[x2]		// bp[0]
	ldp	x4,x5,[x1]
	ldp	x6,x7,[x1,#16]
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	ldr	x13,[x13,#24]

	bl	__ecp_nistz256_mul_mont

	ldp	x19,x20,[sp,#16]
	ldp	x29,x30,[sp],#32
	AARCH64_VALIDATE_LINK_REGISTER
	ret


// void	ecp_nistz256_sqr_mont(BN_ULONG x0[4],const BN_ULONG x1[4]);
.globl	ecp_nistz256_sqr_mont

.def ecp_nistz256_sqr_mont
   .type 32
.endef
.align	4
ecp_nistz256_sqr_mont:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-32]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]

	ldp	x4,x5,[x1]
	ldp	x6,x7,[x1,#16]
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	ldr	x13,[x13,#24]

	bl	__ecp_nistz256_sqr_mont

	ldp	x19,x20,[sp,#16]
	ldp	x29,x30,[sp],#32
	AARCH64_VALIDATE_LINK_REGISTER
	ret


// void	ecp_nistz256_neg(BN_ULONG x0[4],const BN_ULONG x1[4]);
.globl	ecp_nistz256_neg

.def ecp_nistz256_neg
   .type 32
.endef
.align	4
ecp_nistz256_neg:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0

	mov	x2,x1
	mov	x14,xzr		// a = 0
	mov	x15,xzr
	mov	x16,xzr
	mov	x17,xzr
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	ldr	x13,[x13,#24]

	bl	__ecp_nistz256_sub_from

	ldp	x29,x30,[sp],#16
	AARCH64_VALIDATE_LINK_REGISTER
	ret


// note that __ecp_nistz256_mul_mont expects a[0-3] input pre-loaded
// to x4-x7 and b[0] - to x3
.def __ecp_nistz256_mul_mont
   .type 32
.endef
.align	4
__ecp_nistz256_mul_mont:
	mul	x14,x4,x3		// a[0]*b[0]
	umulh	x8,x4,x3

	mul	x15,x5,x3		// a[1]*b[0]
	umulh	x9,x5,x3

	mul	x16,x6,x3		// a[2]*b[0]
	umulh	x10,x6,x3

	mul	x17,x7,x3		// a[3]*b[0]
	umulh	x11,x7,x3
	ldr	x3,[x2,#8]		// b[1]

	adds	x15,x15,x8		// accumulate high parts of multiplication
	lsl	x8,x14,#32
	adcs	x16,x16,x9
	lsr	x9,x14,#32
	adcs	x17,x17,x10
	adc	x19,xzr,x11
	mov	x20,xzr
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	mul	x8,x4,x3		// lo(a[0]*b[i])
	adcs	x15,x16,x9
	mul	x9,x5,x3		// lo(a[1]*b[i])
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	mul	x10,x6,x3		// lo(a[2]*b[i])
	adcs	x17,x19,x11
	mul	x11,x7,x3		// lo(a[3]*b[i])
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts of multiplication
	umulh	x8,x4,x3		// hi(a[0]*b[i])
	adcs	x15,x15,x9
	umulh	x9,x5,x3		// hi(a[1]*b[i])
	adcs	x16,x16,x10
	umulh	x10,x6,x3		// hi(a[2]*b[i])
	adcs	x17,x17,x11
	umulh	x11,x7,x3		// hi(a[3]*b[i])
	adc	x19,x19,xzr
	ldr	x3,[x2,#8*(1+1)]	// b[1+1]
	adds	x15,x15,x8		// accumulate high parts of multiplication
	lsl	x8,x14,#32
	adcs	x16,x16,x9
	lsr	x9,x14,#32
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	mul	x8,x4,x3		// lo(a[0]*b[i])
	adcs	x15,x16,x9
	mul	x9,x5,x3		// lo(a[1]*b[i])
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	mul	x10,x6,x3		// lo(a[2]*b[i])
	adcs	x17,x19,x11
	mul	x11,x7,x3		// lo(a[3]*b[i])
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts of multiplication
	umulh	x8,x4,x3		// hi(a[0]*b[i])
	adcs	x15,x15,x9
	umulh	x9,x5,x3		// hi(a[1]*b[i])
	adcs	x16,x16,x10
	umulh	x10,x6,x3		// hi(a[2]*b[i])
	adcs	x17,x17,x11
	umulh	x11,x7,x3		// hi(a[3]*b[i])
	adc	x19,x19,xzr
	ldr	x3,[x2,#8*(2+1)]	// b[2+1]
	adds	x15,x15,x8		// accumulate high parts of multiplication
	lsl	x8,x14,#32
	adcs	x16,x16,x9
	lsr	x9,x14,#32
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	mul	x8,x4,x3		// lo(a[0]*b[i])
	adcs	x15,x16,x9
	mul	x9,x5,x3		// lo(a[1]*b[i])
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	mul	x10,x6,x3		// lo(a[2]*b[i])
	adcs	x17,x19,x11
	mul	x11,x7,x3		// lo(a[3]*b[i])
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts of multiplication
	umulh	x8,x4,x3		// hi(a[0]*b[i])
	adcs	x15,x15,x9
	umulh	x9,x5,x3		// hi(a[1]*b[i])
	adcs	x16,x16,x10
	umulh	x10,x6,x3		// hi(a[2]*b[i])
	adcs	x17,x17,x11
	umulh	x11,x7,x3		// hi(a[3]*b[i])
	adc	x19,x19,xzr
	adds	x15,x15,x8		// accumulate high parts of multiplication
	lsl	x8,x14,#32
	adcs	x16,x16,x9
	lsr	x9,x14,#32
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	// last reduction
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	adcs	x15,x16,x9
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	adcs	x17,x19,x11
	adc	x19,x20,xzr

	adds	x8,x14,#1		// subs	x8,x14,#-1 // tmp = ret-modulus
	sbcs	x9,x15,x12
	sbcs	x10,x16,xzr
	sbcs	x11,x17,x13
	sbcs	xzr,x19,xzr		// did it borrow?

	csel	x14,x14,x8,lo	// ret = borrow ? ret : ret-modulus
	csel	x15,x15,x9,lo
	csel	x16,x16,x10,lo
	stp	x14,x15,[x0]
	csel	x17,x17,x11,lo
	stp	x16,x17,[x0,#16]

	ret


// note that __ecp_nistz256_sqr_mont expects a[0-3] input pre-loaded
// to x4-x7
.def __ecp_nistz256_sqr_mont
   .type 32
.endef
.align	4
__ecp_nistz256_sqr_mont:
	//  |  |  |  |  |  |a1*a0|  |
	//  |  |  |  |  |a2*a0|  |  |
	//  |  |a3*a2|a3*a0|  |  |  |
	//  |  |  |  |a2*a1|  |  |  |
	//  |  |  |a3*a1|  |  |  |  |
	// *|  |  |  |  |  |  |  | 2|
	// +|a3*a3|a2*a2|a1*a1|a0*a0|
	//  |--+--+--+--+--+--+--+--|
	//  |A7|A6|A5|A4|A3|A2|A1|A0|, where Ax is , i.e. follow 
	//
	//  "can't overflow" below mark carrying into high part of
	//  multiplication result, which can't overflow, because it
	//  can never be all ones.

	mul	x15,x5,x4		// a[1]*a[0]
	umulh	x9,x5,x4
	mul	x16,x6,x4		// a[2]*a[0]
	umulh	x10,x6,x4
	mul	x17,x7,x4		// a[3]*a[0]
	umulh	x19,x7,x4

	adds	x16,x16,x9		// accumulate high parts of multiplication
	mul	x8,x6,x5		// a[2]*a[1]
	umulh	x9,x6,x5
	adcs	x17,x17,x10
	mul	x10,x7,x5		// a[3]*a[1]
	umulh	x11,x7,x5
	adc	x19,x19,xzr		// can't overflow

	mul	x20,x7,x6		// a[3]*a[2]
	umulh	x1,x7,x6

	adds	x9,x9,x10		// accumulate high parts of multiplication
	mul	x14,x4,x4		// a[0]*a[0]
	adc	x10,x11,xzr		// can't overflow

	adds	x17,x17,x8		// accumulate low parts of multiplication
	umulh	x4,x4,x4
	adcs	x19,x19,x9
	mul	x9,x5,x5		// a[1]*a[1]
	adcs	x20,x20,x10
	umulh	x5,x5,x5
	adc	x1,x1,xzr		// can't overflow

	adds	x15,x15,x15	// acc[1-6]*=2
	mul	x10,x6,x6		// a[2]*a[2]
	adcs	x16,x16,x16
	umulh	x6,x6,x6
	adcs	x17,x17,x17
	mul	x11,x7,x7		// a[3]*a[3]
	adcs	x19,x19,x19
	umulh	x7,x7,x7
	adcs	x20,x20,x20
	adcs	x1,x1,x1
	adc	x2,xzr,xzr

	adds	x15,x15,x4		// +a[i]*a[i]
	adcs	x16,x16,x9
	adcs	x17,x17,x5
	adcs	x19,x19,x10
	adcs	x20,x20,x6
	lsl	x8,x14,#32
	adcs	x1,x1,x11
	lsr	x9,x14,#32
	adc	x2,x2,x7
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	adcs	x15,x16,x9
	lsl	x8,x14,#32
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	lsr	x9,x14,#32
	adc	x17,x11,xzr		// can't overflow
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	adcs	x15,x16,x9
	lsl	x8,x14,#32
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	lsr	x9,x14,#32
	adc	x17,x11,xzr		// can't overflow
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	adcs	x15,x16,x9
	lsl	x8,x14,#32
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	lsr	x9,x14,#32
	adc	x17,x11,xzr		// can't overflow
	subs	x10,x14,x8		// "*0xffff0001"
	sbc	x11,x14,x9
	adds	x14,x15,x8		// +=acc[0]<<96 and omit acc[0]
	adcs	x15,x16,x9
	adcs	x16,x17,x10		// +=acc[0]*0xffff0001
	adc	x17,x11,xzr		// can't overflow

	adds	x14,x14,x19	// accumulate upper half
	adcs	x15,x15,x20
	adcs	x16,x16,x1
	adcs	x17,x17,x2
	adc	x19,xzr,xzr

	adds	x8,x14,#1		// subs	x8,x14,#-1 // tmp = ret-modulus
	sbcs	x9,x15,x12
	sbcs	x10,x16,xzr
	sbcs	x11,x17,x13
	sbcs	xzr,x19,xzr		// did it borrow?

	csel	x14,x14,x8,lo	// ret = borrow ? ret : ret-modulus
	csel	x15,x15,x9,lo
	csel	x16,x16,x10,lo
	stp	x14,x15,[x0]
	csel	x17,x17,x11,lo
	stp	x16,x17,[x0,#16]

	ret


// Note that __ecp_nistz256_add_to expects both input vectors pre-loaded to
// x4-x7 and x8-x11. This is done because it's used in multiple
// contexts, e.g. in multiplication by 2 and 3...
.def __ecp_nistz256_add_to
   .type 32
.endef
.align	4
__ecp_nistz256_add_to:
	adds	x14,x14,x8		// ret = a+b
	adcs	x15,x15,x9
	adcs	x16,x16,x10
	adcs	x17,x17,x11
	adc	x1,xzr,xzr		// zap x1

	adds	x8,x14,#1		// subs	x8,x4,#-1 // tmp = ret-modulus
	sbcs	x9,x15,x12
	sbcs	x10,x16,xzr
	sbcs	x11,x17,x13
	sbcs	xzr,x1,xzr		// did subtraction borrow?

	csel	x14,x14,x8,lo	// ret = borrow ? ret : ret-modulus
	csel	x15,x15,x9,lo
	csel	x16,x16,x10,lo
	stp	x14,x15,[x0]
	csel	x17,x17,x11,lo
	stp	x16,x17,[x0,#16]

	ret


.def __ecp_nistz256_sub_from
   .type 32
.endef
.align	4
__ecp_nistz256_sub_from:
	ldp	x8,x9,[x2]
	ldp	x10,x11,[x2,#16]
	subs	x14,x14,x8		// ret = a-b
	sbcs	x15,x15,x9
	sbcs	x16,x16,x10
	sbcs	x17,x17,x11
	sbc	x1,xzr,xzr		// zap x1

	subs	x8,x14,#1		// adds	x8,x4,#-1 // tmp = ret+modulus
	adcs	x9,x15,x12
	adcs	x10,x16,xzr
	adc	x11,x17,x13
	cmp	x1,xzr			// did subtraction borrow?

	csel	x14,x14,x8,eq	// ret = borrow ? ret+modulus : ret
	csel	x15,x15,x9,eq
	csel	x16,x16,x10,eq
	stp	x14,x15,[x0]
	csel	x17,x17,x11,eq
	stp	x16,x17,[x0,#16]

	ret


.def __ecp_nistz256_sub_morf
   .type 32
.endef
.align	4
__ecp_nistz256_sub_morf:
	ldp	x8,x9,[x2]
	ldp	x10,x11,[x2,#16]
	subs	x14,x8,x14		// ret = b-a
	sbcs	x15,x9,x15
	sbcs	x16,x10,x16
	sbcs	x17,x11,x17
	sbc	x1,xzr,xzr		// zap x1

	subs	x8,x14,#1		// adds	x8,x4,#-1 // tmp = ret+modulus
	adcs	x9,x15,x12
	adcs	x10,x16,xzr
	adc	x11,x17,x13
	cmp	x1,xzr			// did subtraction borrow?

	csel	x14,x14,x8,eq	// ret = borrow ? ret+modulus : ret
	csel	x15,x15,x9,eq
	csel	x16,x16,x10,eq
	stp	x14,x15,[x0]
	csel	x17,x17,x11,eq
	stp	x16,x17,[x0,#16]

	ret


.def __ecp_nistz256_div_by_2
   .type 32
.endef
.align	4
__ecp_nistz256_div_by_2:
	subs	x8,x14,#1		// adds	x8,x4,#-1 // tmp = a+modulus
	adcs	x9,x15,x12
	adcs	x10,x16,xzr
	adcs	x11,x17,x13
	adc	x1,xzr,xzr		// zap x1
	tst	x14,#1		// is a even?

	csel	x14,x14,x8,eq	// ret = even ? a : a+modulus
	csel	x15,x15,x9,eq
	csel	x16,x16,x10,eq
	csel	x17,x17,x11,eq
	csel	x1,xzr,x1,eq

	lsr	x14,x14,#1		// ret >>= 1
	orr	x14,x14,x15,lsl#63
	lsr	x15,x15,#1
	orr	x15,x15,x16,lsl#63
	lsr	x16,x16,#1
	orr	x16,x16,x17,lsl#63
	lsr	x17,x17,#1
	stp	x14,x15,[x0]
	orr	x17,x17,x1,lsl#63
	stp	x16,x17,[x0,#16]

	ret

.globl	ecp_nistz256_point_double

.def ecp_nistz256_point_double
   .type 32
.endef
.align	5
ecp_nistz256_point_double:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-96]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	sub	sp,sp,#32*4

Ldouble_shortcut:
	ldp	x14,x15,[x1,#32]
	mov	x21,x0
	ldp	x16,x17,[x1,#48]
	mov	x22,x1
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	mov	x8,x14
	ldr	x13,[x13,#24]
	mov	x9,x15
	ldp	x4,x5,[x22,#64]	// forward load for p256_sqr_mont
	mov	x10,x16
	mov	x11,x17
	ldp	x6,x7,[x22,#64+16]
	add	x0,sp,#0
	bl	__ecp_nistz256_add_to	// p256_mul_by_2(S, in_y);

	add	x0,sp,#64
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Zsqr, in_z);

	ldp	x8,x9,[x22]
	ldp	x10,x11,[x22,#16]
	mov	x4,x14		// put Zsqr aside for p256_sub
	mov	x5,x15
	mov	x6,x16
	mov	x7,x17
	add	x0,sp,#32
	bl	__ecp_nistz256_add_to	// p256_add(M, Zsqr, in_x);

	add	x2,x22,#0
	mov	x14,x4		// restore Zsqr
	mov	x15,x5
	ldp	x4,x5,[sp,#0]	// forward load for p256_sqr_mont
	mov	x16,x6
	mov	x17,x7
	ldp	x6,x7,[sp,#0+16]
	add	x0,sp,#64
	bl	__ecp_nistz256_sub_morf	// p256_sub(Zsqr, in_x, Zsqr);

	add	x0,sp,#0
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(S, S);

	ldr	x3,[x22,#32]
	ldp	x4,x5,[x22,#64]
	ldp	x6,x7,[x22,#64+16]
	add	x2,x22,#32
	add	x0,sp,#96
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(tmp0, in_z, in_y);

	mov	x8,x14
	mov	x9,x15
	ldp	x4,x5,[sp,#0]	// forward load for p256_sqr_mont
	mov	x10,x16
	mov	x11,x17
	ldp	x6,x7,[sp,#0+16]
	add	x0,x21,#64
	bl	__ecp_nistz256_add_to	// p256_mul_by_2(res_z, tmp0);

	add	x0,sp,#96
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(tmp0, S);

	ldr	x3,[sp,#64]		// forward load for p256_mul_mont
	ldp	x4,x5,[sp,#32]
	ldp	x6,x7,[sp,#32+16]
	add	x0,x21,#32
	bl	__ecp_nistz256_div_by_2	// p256_div_by_2(res_y, tmp0);

	add	x2,sp,#64
	add	x0,sp,#32
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(M, M, Zsqr);

	mov	x8,x14		// duplicate M
	mov	x9,x15
	mov	x10,x16
	mov	x11,x17
	mov	x4,x14		// put M aside
	mov	x5,x15
	mov	x6,x16
	mov	x7,x17
	add	x0,sp,#32
	bl	__ecp_nistz256_add_to
	mov	x8,x4			// restore M
	mov	x9,x5
	ldr	x3,[x22]		// forward load for p256_mul_mont
	mov	x10,x6
	ldp	x4,x5,[sp,#0]
	mov	x11,x7
	ldp	x6,x7,[sp,#0+16]
	bl	__ecp_nistz256_add_to	// p256_mul_by_3(M, M);

	add	x2,x22,#0
	add	x0,sp,#0
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S, S, in_x);

	mov	x8,x14
	mov	x9,x15
	ldp	x4,x5,[sp,#32]	// forward load for p256_sqr_mont
	mov	x10,x16
	mov	x11,x17
	ldp	x6,x7,[sp,#32+16]
	add	x0,sp,#96
	bl	__ecp_nistz256_add_to	// p256_mul_by_2(tmp0, S);

	add	x0,x21,#0
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(res_x, M);

	add	x2,sp,#96
	bl	__ecp_nistz256_sub_from	// p256_sub(res_x, res_x, tmp0);

	add	x2,sp,#0
	add	x0,sp,#0
	bl	__ecp_nistz256_sub_morf	// p256_sub(S, S, res_x);

	ldr	x3,[sp,#32]
	mov	x4,x14		// copy S
	mov	x5,x15
	mov	x6,x16
	mov	x7,x17
	add	x2,sp,#32
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S, S, M);

	add	x2,x21,#32
	add	x0,x21,#32
	bl	__ecp_nistz256_sub_from	// p256_sub(res_y, S, res_y);

	add	sp,x29,#0		// destroy frame
	ldp	x19,x20,[x29,#16]
	ldp	x21,x22,[x29,#32]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret

.globl	ecp_nistz256_point_add

.def ecp_nistz256_point_add
   .type 32
.endef
.align	5
ecp_nistz256_point_add:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-96]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]
	stp	x25,x26,[sp,#64]
	stp	x27,x28,[sp,#80]
	sub	sp,sp,#32*12

	ldp	x4,x5,[x2,#64]	// in2_z
	ldp	x6,x7,[x2,#64+16]
	mov	x21,x0
	mov	x22,x1
	mov	x23,x2
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	ldr	x13,[x13,#24]
	orr	x8,x4,x5
	orr	x10,x6,x7
	orr	x25,x8,x10
	cmp	x25,#0
	csetm	x25,ne		// ~in2infty
	add	x0,sp,#192
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Z2sqr, in2_z);

	ldp	x4,x5,[x22,#64]	// in1_z
	ldp	x6,x7,[x22,#64+16]
	orr	x8,x4,x5
	orr	x10,x6,x7
	orr	x24,x8,x10
	cmp	x24,#0
	csetm	x24,ne		// ~in1infty
	add	x0,sp,#128
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Z1sqr, in1_z);

	ldr	x3,[x23,#64]
	ldp	x4,x5,[sp,#192]
	ldp	x6,x7,[sp,#192+16]
	add	x2,x23,#64
	add	x0,sp,#320
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S1, Z2sqr, in2_z);

	ldr	x3,[x22,#64]
	ldp	x4,x5,[sp,#128]
	ldp	x6,x7,[sp,#128+16]
	add	x2,x22,#64
	add	x0,sp,#352
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, Z1sqr, in1_z);

	ldr	x3,[x22,#32]
	ldp	x4,x5,[sp,#320]
	ldp	x6,x7,[sp,#320+16]
	add	x2,x22,#32
	add	x0,sp,#320
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S1, S1, in1_y);

	ldr	x3,[x23,#32]
	ldp	x4,x5,[sp,#352]
	ldp	x6,x7,[sp,#352+16]
	add	x2,x23,#32
	add	x0,sp,#352
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, S2, in2_y);

	add	x2,sp,#320
	ldr	x3,[sp,#192]	// forward load for p256_mul_mont
	ldp	x4,x5,[x22]
	ldp	x6,x7,[x22,#16]
	add	x0,sp,#160
	bl	__ecp_nistz256_sub_from	// p256_sub(R, S2, S1);

	orr	x14,x14,x15	// see if result is zero
	orr	x16,x16,x17
	orr	x26,x14,x16	// ~is_equal(S1,S2)

	add	x2,sp,#192
	add	x0,sp,#256
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(U1, in1_x, Z2sqr);

	ldr	x3,[sp,#128]
	ldp	x4,x5,[x23]
	ldp	x6,x7,[x23,#16]
	add	x2,sp,#128
	add	x0,sp,#288
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(U2, in2_x, Z1sqr);

	add	x2,sp,#256
	ldp	x4,x5,[sp,#160]	// forward load for p256_sqr_mont
	ldp	x6,x7,[sp,#160+16]
	add	x0,sp,#96
	bl	__ecp_nistz256_sub_from	// p256_sub(H, U2, U1);

	orr	x14,x14,x15	// see if result is zero
	orr	x16,x16,x17
	orr	x14,x14,x16	// ~is_equal(U1,U2)

	mvn	x27,x24	// -1/0 -> 0/-1
	mvn	x28,x25	// -1/0 -> 0/-1
	orr	x14,x14,x27
	orr	x14,x14,x28
	orr	x14,x14,x26
	cbnz	x14,Ladd_proceed	// if(~is_equal(U1,U2) | in1infty | in2infty | ~is_equal(S1,S2))

Ladd_double:
	mov	x1,x22
	mov	x0,x21
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	add	sp,sp,#256	// #256 is from #32*(12-4). difference in stack frames
	b	Ldouble_shortcut

.align	4
Ladd_proceed:
	add	x0,sp,#192
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Rsqr, R);

	ldr	x3,[x22,#64]
	ldp	x4,x5,[sp,#96]
	ldp	x6,x7,[sp,#96+16]
	add	x2,x22,#64
	add	x0,sp,#64
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(res_z, H, in1_z);

	ldp	x4,x5,[sp,#96]
	ldp	x6,x7,[sp,#96+16]
	add	x0,sp,#128
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Hsqr, H);

	ldr	x3,[x23,#64]
	ldp	x4,x5,[sp,#64]
	ldp	x6,x7,[sp,#64+16]
	add	x2,x23,#64
	add	x0,sp,#64
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(res_z, res_z, in2_z);

	ldr	x3,[sp,#96]
	ldp	x4,x5,[sp,#128]
	ldp	x6,x7,[sp,#128+16]
	add	x2,sp,#96
	add	x0,sp,#224
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(Hcub, Hsqr, H);

	ldr	x3,[sp,#128]
	ldp	x4,x5,[sp,#256]
	ldp	x6,x7,[sp,#256+16]
	add	x2,sp,#128
	add	x0,sp,#288
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(U2, U1, Hsqr);

	mov	x8,x14
	mov	x9,x15
	mov	x10,x16
	mov	x11,x17
	add	x0,sp,#128
	bl	__ecp_nistz256_add_to	// p256_mul_by_2(Hsqr, U2);

	add	x2,sp,#192
	add	x0,sp,#0
	bl	__ecp_nistz256_sub_morf	// p256_sub(res_x, Rsqr, Hsqr);

	add	x2,sp,#224
	bl	__ecp_nistz256_sub_from	//  p256_sub(res_x, res_x, Hcub);

	add	x2,sp,#288
	ldr	x3,[sp,#224]		// forward load for p256_mul_mont
	ldp	x4,x5,[sp,#320]
	ldp	x6,x7,[sp,#320+16]
	add	x0,sp,#32
	bl	__ecp_nistz256_sub_morf	// p256_sub(res_y, U2, res_x);

	add	x2,sp,#224
	add	x0,sp,#352
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, S1, Hcub);

	ldr	x3,[sp,#160]
	ldp	x4,x5,[sp,#32]
	ldp	x6,x7,[sp,#32+16]
	add	x2,sp,#160
	add	x0,sp,#32
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(res_y, res_y, R);

	add	x2,sp,#352
	bl	__ecp_nistz256_sub_from	// p256_sub(res_y, res_y, S2);

	ldp	x4,x5,[sp,#0]		// res
	ldp	x6,x7,[sp,#0+16]
	ldp	x8,x9,[x23]		// in2
	ldp	x10,x11,[x23,#16]
	ldp	x14,x15,[x22,#0]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#0+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	ldp	x4,x5,[sp,#0+0+32]	// res
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	ldp	x6,x7,[sp,#0+0+48]
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	ldp	x8,x9,[x23,#0+32]	// in2
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	ldp	x10,x11,[x23,#0+48]
	stp	x14,x15,[x21,#0]
	stp	x16,x17,[x21,#0+16]
	ldp	x14,x15,[x22,#32]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#32+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	ldp	x4,x5,[sp,#0+32+32]	// res
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	ldp	x6,x7,[sp,#0+32+48]
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	ldp	x8,x9,[x23,#32+32]	// in2
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	ldp	x10,x11,[x23,#32+48]
	stp	x14,x15,[x21,#32]
	stp	x16,x17,[x21,#32+16]
	ldp	x14,x15,[x22,#64]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#64+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	stp	x14,x15,[x21,#64]
	stp	x16,x17,[x21,#64+16]

Ladd_done:
	add	sp,x29,#0		// destroy frame
	ldp	x19,x20,[x29,#16]
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x27,x28,[x29,#80]
	ldp	x29,x30,[sp],#96
	AARCH64_VALIDATE_LINK_REGISTER
	ret

.globl	ecp_nistz256_point_add_affine

.def ecp_nistz256_point_add_affine
   .type 32
.endef
.align	5
ecp_nistz256_point_add_affine:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29,x30,[sp,#-80]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]
	stp	x25,x26,[sp,#64]
	sub	sp,sp,#32*10

	mov	x21,x0
	mov	x22,x1
	mov	x23,x2
	adrp	x13,Lpoly
	add	x13,x13,:lo12:Lpoly
	ldr	x12,[x13,#8]
	ldr	x13,[x13,#24]

	ldp	x4,x5,[x1,#64]	// in1_z
	ldp	x6,x7,[x1,#64+16]
	orr	x8,x4,x5
	orr	x10,x6,x7
	orr	x24,x8,x10
	cmp	x24,#0
	csetm	x24,ne		// ~in1infty

	ldp	x14,x15,[x2]	// in2_x
	ldp	x16,x17,[x2,#16]
	ldp	x8,x9,[x2,#32]	// in2_y
	ldp	x10,x11,[x2,#48]
	orr	x14,x14,x15
	orr	x16,x16,x17
	orr	x8,x8,x9
	orr	x10,x10,x11
	orr	x14,x14,x16
	orr	x8,x8,x10
	orr	x25,x14,x8
	cmp	x25,#0
	csetm	x25,ne		// ~in2infty

	add	x0,sp,#128
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Z1sqr, in1_z);

	mov	x4,x14
	mov	x5,x15
	mov	x6,x16
	mov	x7,x17
	ldr	x3,[x23]
	add	x2,x23,#0
	add	x0,sp,#96
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(U2, Z1sqr, in2_x);

	add	x2,x22,#0
	ldr	x3,[x22,#64]	// forward load for p256_mul_mont
	ldp	x4,x5,[sp,#128]
	ldp	x6,x7,[sp,#128+16]
	add	x0,sp,#160
	bl	__ecp_nistz256_sub_from	// p256_sub(H, U2, in1_x);

	add	x2,x22,#64
	add	x0,sp,#128
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, Z1sqr, in1_z);

	ldr	x3,[x22,#64]
	ldp	x4,x5,[sp,#160]
	ldp	x6,x7,[sp,#160+16]
	add	x2,x22,#64
	add	x0,sp,#64
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(res_z, H, in1_z);

	ldr	x3,[x23,#32]
	ldp	x4,x5,[sp,#128]
	ldp	x6,x7,[sp,#128+16]
	add	x2,x23,#32
	add	x0,sp,#128
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, S2, in2_y);

	add	x2,x22,#32
	ldp	x4,x5,[sp,#160]	// forward load for p256_sqr_mont
	ldp	x6,x7,[sp,#160+16]
	add	x0,sp,#192
	bl	__ecp_nistz256_sub_from	// p256_sub(R, S2, in1_y);

	add	x0,sp,#224
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Hsqr, H);

	ldp	x4,x5,[sp,#192]
	ldp	x6,x7,[sp,#192+16]
	add	x0,sp,#288
	bl	__ecp_nistz256_sqr_mont	// p256_sqr_mont(Rsqr, R);

	ldr	x3,[sp,#160]
	ldp	x4,x5,[sp,#224]
	ldp	x6,x7,[sp,#224+16]
	add	x2,sp,#160
	add	x0,sp,#256
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(Hcub, Hsqr, H);

	ldr	x3,[x22]
	ldp	x4,x5,[sp,#224]
	ldp	x6,x7,[sp,#224+16]
	add	x2,x22,#0
	add	x0,sp,#96
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(U2, in1_x, Hsqr);

	mov	x8,x14
	mov	x9,x15
	mov	x10,x16
	mov	x11,x17
	add	x0,sp,#224
	bl	__ecp_nistz256_add_to	// p256_mul_by_2(Hsqr, U2);

	add	x2,sp,#288
	add	x0,sp,#0
	bl	__ecp_nistz256_sub_morf	// p256_sub(res_x, Rsqr, Hsqr);

	add	x2,sp,#256
	bl	__ecp_nistz256_sub_from	//  p256_sub(res_x, res_x, Hcub);

	add	x2,sp,#96
	ldr	x3,[x22,#32]	// forward load for p256_mul_mont
	ldp	x4,x5,[sp,#256]
	ldp	x6,x7,[sp,#256+16]
	add	x0,sp,#32
	bl	__ecp_nistz256_sub_morf	// p256_sub(res_y, U2, res_x);

	add	x2,x22,#32
	add	x0,sp,#128
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(S2, in1_y, Hcub);

	ldr	x3,[sp,#192]
	ldp	x4,x5,[sp,#32]
	ldp	x6,x7,[sp,#32+16]
	add	x2,sp,#192
	add	x0,sp,#32
	bl	__ecp_nistz256_mul_mont	// p256_mul_mont(res_y, res_y, R);

	add	x2,sp,#128
	bl	__ecp_nistz256_sub_from	// p256_sub(res_y, res_y, S2);

	ldp	x4,x5,[sp,#0]		// res
	ldp	x6,x7,[sp,#0+16]
	ldp	x8,x9,[x23]		// in2
	ldp	x10,x11,[x23,#16]
	ldp	x14,x15,[x22,#0]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#0+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	ldp	x4,x5,[sp,#0+0+32]	// res
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	ldp	x6,x7,[sp,#0+0+48]
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	ldp	x8,x9,[x23,#0+32]	// in2
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	ldp	x10,x11,[x23,#0+48]
	stp	x14,x15,[x21,#0]
	stp	x16,x17,[x21,#0+16]
	adrp	x23,Lone_mont-64
	add	x23,x23,:lo12:Lone_mont-64
	ldp	x14,x15,[x22,#32]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#32+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	ldp	x4,x5,[sp,#0+32+32]	// res
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	ldp	x6,x7,[sp,#0+32+48]
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	ldp	x8,x9,[x23,#32+32]	// in2
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	ldp	x10,x11,[x23,#32+48]
	stp	x14,x15,[x21,#32]
	stp	x16,x17,[x21,#32+16]
	ldp	x14,x15,[x22,#64]	// in1
	cmp	x24,#0			// ~, remember?
	ldp	x16,x17,[x22,#64+16]
	csel	x8,x4,x8,ne
	csel	x9,x5,x9,ne
	csel	x10,x6,x10,ne
	csel	x11,x7,x11,ne
	cmp	x25,#0			// ~, remember?
	csel	x14,x8,x14,ne
	csel	x15,x9,x15,ne
	csel	x16,x10,x16,ne
	csel	x17,x11,x17,ne
	stp	x14,x15,[x21,#64]
	stp	x16,x17,[x21,#64+16]

	add	sp,x29,#0		// destroy frame
	ldp	x19,x20,[x29,#16]
	ldp	x21,x22,[x29,#32]
	ldp	x23,x24,[x29,#48]
	ldp	x25,x26,[x29,#64]
	ldp	x29,x30,[sp],#80
	AARCH64_VALIDATE_LINK_REGISTER
	ret

////////////////////////////////////////////////////////////////////////
// void ecp_nistz256_ord_mul_mont(uint64_t res[4], uint64_t a[4],
//                                uint64_t b[4]);
.globl	ecp_nistz256_ord_mul_mont

.def ecp_nistz256_ord_mul_mont
   .type 32
.endef
.align	4
ecp_nistz256_ord_mul_mont:
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp	x29,x30,[sp,#-64]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]

	adrp	x23,Lord
	add	x23,x23,:lo12:Lord
	ldr	x3,[x2]		// bp[0]
	ldp	x4,x5,[x1]
	ldp	x6,x7,[x1,#16]

	ldp	x12,x13,[x23,#0]
	ldp	x21,x22,[x23,#16]
	ldr	x23,[x23,#32]

	mul	x14,x4,x3		// a[0]*b[0]
	umulh	x8,x4,x3

	mul	x15,x5,x3		// a[1]*b[0]
	umulh	x9,x5,x3

	mul	x16,x6,x3		// a[2]*b[0]
	umulh	x10,x6,x3

	mul	x17,x7,x3		// a[3]*b[0]
	umulh	x19,x7,x3

	mul	x24,x14,x23

	adds	x15,x15,x8		// accumulate high parts of multiplication
	adcs	x16,x16,x9
	adcs	x17,x17,x10
	adc	x19,x19,xzr
	mov	x20,xzr
	ldr	x3,[x2,#8*1]		// b[i]

	lsl	x8,x24,#32
	subs	x16,x16,x24
	lsr	x9,x24,#32
	sbcs	x17,x17,x8
	sbcs	x19,x19,x9
	sbc	x20,x20,xzr

	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	mul	x8,x4,x3
	adc	x11,x11,xzr
	mul	x9,x5,x3

	adds	x14,x15,x10
	mul	x10,x6,x3
	adcs	x15,x16,x11
	mul	x11,x7,x3
	adcs	x16,x17,x24
	adcs	x17,x19,x24
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts
	umulh	x8,x4,x3
	adcs	x15,x15,x9
	umulh	x9,x5,x3
	adcs	x16,x16,x10
	umulh	x10,x6,x3
	adcs	x17,x17,x11
	umulh	x11,x7,x3
	adc	x19,x19,xzr
	mul	x24,x14,x23
	adds	x15,x15,x8		// accumulate high parts
	adcs	x16,x16,x9
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	ldr	x3,[x2,#8*2]		// b[i]

	lsl	x8,x24,#32
	subs	x16,x16,x24
	lsr	x9,x24,#32
	sbcs	x17,x17,x8
	sbcs	x19,x19,x9
	sbc	x20,x20,xzr

	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	mul	x8,x4,x3
	adc	x11,x11,xzr
	mul	x9,x5,x3

	adds	x14,x15,x10
	mul	x10,x6,x3
	adcs	x15,x16,x11
	mul	x11,x7,x3
	adcs	x16,x17,x24
	adcs	x17,x19,x24
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts
	umulh	x8,x4,x3
	adcs	x15,x15,x9
	umulh	x9,x5,x3
	adcs	x16,x16,x10
	umulh	x10,x6,x3
	adcs	x17,x17,x11
	umulh	x11,x7,x3
	adc	x19,x19,xzr
	mul	x24,x14,x23
	adds	x15,x15,x8		// accumulate high parts
	adcs	x16,x16,x9
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	ldr	x3,[x2,#8*3]		// b[i]

	lsl	x8,x24,#32
	subs	x16,x16,x24
	lsr	x9,x24,#32
	sbcs	x17,x17,x8
	sbcs	x19,x19,x9
	sbc	x20,x20,xzr

	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	mul	x8,x4,x3
	adc	x11,x11,xzr
	mul	x9,x5,x3

	adds	x14,x15,x10
	mul	x10,x6,x3
	adcs	x15,x16,x11
	mul	x11,x7,x3
	adcs	x16,x17,x24
	adcs	x17,x19,x24
	adc	x19,x20,xzr

	adds	x14,x14,x8		// accumulate low parts
	umulh	x8,x4,x3
	adcs	x15,x15,x9
	umulh	x9,x5,x3
	adcs	x16,x16,x10
	umulh	x10,x6,x3
	adcs	x17,x17,x11
	umulh	x11,x7,x3
	adc	x19,x19,xzr
	mul	x24,x14,x23
	adds	x15,x15,x8		// accumulate high parts
	adcs	x16,x16,x9
	adcs	x17,x17,x10
	adcs	x19,x19,x11
	adc	x20,xzr,xzr
	lsl	x8,x24,#32		// last reduction
	subs	x16,x16,x24
	lsr	x9,x24,#32
	sbcs	x17,x17,x8
	sbcs	x19,x19,x9
	sbc	x20,x20,xzr

	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	adc	x11,x11,xzr

	adds	x14,x15,x10
	adcs	x15,x16,x11
	adcs	x16,x17,x24
	adcs	x17,x19,x24
	adc	x19,x20,xzr

	subs	x8,x14,x12		// ret -= modulus
	sbcs	x9,x15,x13
	sbcs	x10,x16,x21
	sbcs	x11,x17,x22
	sbcs	xzr,x19,xzr

	csel	x14,x14,x8,lo	// ret = borrow ? ret : ret-modulus
	csel	x15,x15,x9,lo
	csel	x16,x16,x10,lo
	stp	x14,x15,[x0]
	csel	x17,x17,x11,lo
	stp	x16,x17,[x0,#16]

	ldp	x19,x20,[sp,#16]
	ldp	x21,x22,[sp,#32]
	ldp	x23,x24,[sp,#48]
	ldr	x29,[sp],#64
	ret


////////////////////////////////////////////////////////////////////////
// void ecp_nistz256_ord_sqr_mont(uint64_t res[4], uint64_t a[4],
//                                uint64_t rep);
.globl	ecp_nistz256_ord_sqr_mont

.def ecp_nistz256_ord_sqr_mont
   .type 32
.endef
.align	4
ecp_nistz256_ord_sqr_mont:
	AARCH64_VALID_CALL_TARGET
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	stp	x29,x30,[sp,#-64]!
	add	x29,sp,#0
	stp	x19,x20,[sp,#16]
	stp	x21,x22,[sp,#32]
	stp	x23,x24,[sp,#48]

	adrp	x23,Lord
	add	x23,x23,:lo12:Lord
	ldp	x4,x5,[x1]
	ldp	x6,x7,[x1,#16]

	ldp	x12,x13,[x23,#0]
	ldp	x21,x22,[x23,#16]
	ldr	x23,[x23,#32]
	b	Loop_ord_sqr

.align	4
Loop_ord_sqr:
	sub	x2,x2,#1
	////////////////////////////////////////////////////////////////
	//  |  |  |  |  |  |a1*a0|  |
	//  |  |  |  |  |a2*a0|  |  |
	//  |  |a3*a2|a3*a0|  |  |  |
	//  |  |  |  |a2*a1|  |  |  |
	//  |  |  |a3*a1|  |  |  |  |
	// *|  |  |  |  |  |  |  | 2|
	// +|a3*a3|a2*a2|a1*a1|a0*a0|
	//  |--+--+--+--+--+--+--+--|
	//  |A7|A6|A5|A4|A3|A2|A1|A0|, where Ax is , i.e. follow 
	//
	//  "can't overflow" below mark carrying into high part of
	//  multiplication result, which can't overflow, because it
	//  can never be all ones.

	mul	x15,x5,x4		// a[1]*a[0]
	umulh	x9,x5,x4
	mul	x16,x6,x4		// a[2]*a[0]
	umulh	x10,x6,x4
	mul	x17,x7,x4		// a[3]*a[0]
	umulh	x19,x7,x4

	adds	x16,x16,x9		// accumulate high parts of multiplication
	mul	x8,x6,x5		// a[2]*a[1]
	umulh	x9,x6,x5
	adcs	x17,x17,x10
	mul	x10,x7,x5		// a[3]*a[1]
	umulh	x11,x7,x5
	adc	x19,x19,xzr		// can't overflow

	mul	x20,x7,x6		// a[3]*a[2]
	umulh	x1,x7,x6

	adds	x9,x9,x10		// accumulate high parts of multiplication
	mul	x14,x4,x4		// a[0]*a[0]
	adc	x10,x11,xzr		// can't overflow

	adds	x17,x17,x8		// accumulate low parts of multiplication
	umulh	x4,x4,x4
	adcs	x19,x19,x9
	mul	x9,x5,x5		// a[1]*a[1]
	adcs	x20,x20,x10
	umulh	x5,x5,x5
	adc	x1,x1,xzr		// can't overflow

	adds	x15,x15,x15	// acc[1-6]*=2
	mul	x10,x6,x6		// a[2]*a[2]
	adcs	x16,x16,x16
	umulh	x6,x6,x6
	adcs	x17,x17,x17
	mul	x11,x7,x7		// a[3]*a[3]
	adcs	x19,x19,x19
	umulh	x7,x7,x7
	adcs	x20,x20,x20
	adcs	x1,x1,x1
	adc	x3,xzr,xzr

	adds	x15,x15,x4		// +a[i]*a[i]
	mul	x24,x14,x23
	adcs	x16,x16,x9
	adcs	x17,x17,x5
	adcs	x19,x19,x10
	adcs	x20,x20,x6
	adcs	x1,x1,x11
	adc	x3,x3,x7
	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	adc	x11,x11,xzr

	adds	x14,x15,x10
	adcs	x15,x16,x11
	adcs	x16,x17,x24
	adc	x17,xzr,x24		// can't overflow
	mul	x11,x14,x23
	lsl	x8,x24,#32
	subs	x15,x15,x24
	lsr	x9,x24,#32
	sbcs	x16,x16,x8
	sbc	x17,x17,x9		// can't borrow
	subs	xzr,x14,#1
	umulh	x9,x12,x11
	mul	x10,x13,x11
	umulh	x24,x13,x11

	adcs	x10,x10,x9
	adc	x24,x24,xzr

	adds	x14,x15,x10
	adcs	x15,x16,x24
	adcs	x16,x17,x11
	adc	x17,xzr,x11		// can't overflow
	mul	x24,x14,x23
	lsl	x8,x11,#32
	subs	x15,x15,x11
	lsr	x9,x11,#32
	sbcs	x16,x16,x8
	sbc	x17,x17,x9		// can't borrow
	subs	xzr,x14,#1
	umulh	x9,x12,x24
	mul	x10,x13,x24
	umulh	x11,x13,x24

	adcs	x10,x10,x9
	adc	x11,x11,xzr

	adds	x14,x15,x10
	adcs	x15,x16,x11
	adcs	x16,x17,x24
	adc	x17,xzr,x24		// can't overflow
	mul	x11,x14,x23
	lsl	x8,x24,#32
	subs	x15,x15,x24
	lsr	x9,x24,#32
	sbcs	x16,x16,x8
	sbc	x17,x17,x9		// can't borrow
	subs	xzr,x14,#1
	umulh	x9,x12,x11
	mul	x10,x13,x11
	umulh	x24,x13,x11

	adcs	x10,x10,x9
	adc	x24,x24,xzr

	adds	x14,x15,x10
	adcs	x15,x16,x24
	adcs	x16,x17,x11
	adc	x17,xzr,x11		// can't overflow
	lsl	x8,x11,#32
	subs	x15,x15,x11
	lsr	x9,x11,#32
	sbcs	x16,x16,x8
	sbc	x17,x17,x9		// can't borrow
	adds	x14,x14,x19	// accumulate upper half
	adcs	x15,x15,x20
	adcs	x16,x16,x1
	adcs	x17,x17,x3
	adc	x19,xzr,xzr

	subs	x8,x14,x12		// ret -= modulus
	sbcs	x9,x15,x13
	sbcs	x10,x16,x21
	sbcs	x11,x17,x22
	sbcs	xzr,x19,xzr

	csel	x4,x14,x8,lo	// ret = borrow ? ret : ret-modulus
	csel	x5,x15,x9,lo
	csel	x6,x16,x10,lo
	csel	x7,x17,x11,lo

	cbnz	x2,Loop_ord_sqr

	stp	x4,x5,[x0]
	stp	x6,x7,[x0,#16]

	ldp	x19,x20,[sp,#16]
	ldp	x21,x22,[sp,#32]
	ldp	x23,x24,[sp,#48]
	ldr	x29,[sp],#64
	ret

////////////////////////////////////////////////////////////////////////
// void ecp_nistz256_select_w5(uint64_t *val, uint64_t *in_t, int index);
.globl	ecp_nistz256_select_w5

.def ecp_nistz256_select_w5
   .type 32
.endef
.align	4
ecp_nistz256_select_w5:
	AARCH64_VALID_CALL_TARGET

    // x10 := x0
    // w9 := 0; loop counter and incremented internal index
	mov	x10, x0
	mov	w9, #0

    // [v16-v21] := 0
	movi	v16.16b, #0
	movi	v17.16b, #0
	movi	v18.16b, #0
	movi	v19.16b, #0
	movi	v20.16b, #0
	movi	v21.16b, #0

Lselect_w5_loop:
    // Loop 16 times.

    // Increment index (loop counter); tested at the end of the loop
	add	w9, w9, #1

    // [v22-v27] := Load a (3*256-bit = 6*128-bit) table entry starting at x1
    //  and advance x1 to point to the next entry
	ld1	{v22.2d, v23.2d, v24.2d, v25.2d}, [x1],#64

    // x11 := (w9 == w2)? All 1s : All 0s
	cmp	w9, w2
	csetm	x11, eq

    // continue loading ...
	ld1	{v26.2d, v27.2d}, [x1],#32

    // duplicate mask_64 into Mask (all 0s or all 1s)
	dup	v3.2d, x11

    // [v16-v19] := (Mask == all 1s)? [v22-v25] : [v16-v19]
    // i.e., values in output registers will remain the same if w9 != w2
	bit	v16.16b, v22.16b, v3.16b
	bit	v17.16b, v23.16b, v3.16b

	bit	v18.16b, v24.16b, v3.16b
	bit	v19.16b, v25.16b, v3.16b

	bit	v20.16b, v26.16b, v3.16b
	bit	v21.16b, v27.16b, v3.16b

    // If bit #4 is not 0 (i.e. idx_ctr < 16) loop back
	tbz	w9, #4, Lselect_w5_loop

    // Write [v16-v21] to memory at the output pointer
	st1	{v16.2d, v17.2d, v18.2d, v19.2d}, [x10],#64
	st1	{v20.2d, v21.2d}, [x10]

	ret



////////////////////////////////////////////////////////////////////////
// void ecp_nistz256_select_w7(uint64_t *val, uint64_t *in_t, int index);
.globl	ecp_nistz256_select_w7

.def ecp_nistz256_select_w7
   .type 32
.endef
.align	4
ecp_nistz256_select_w7:
	AARCH64_VALID_CALL_TARGET

    // w9 := 0; loop counter and incremented internal index
	mov	w9, #0

    // [v16-v21] := 0
	movi	v16.16b, #0
	movi	v17.16b, #0
	movi	v18.16b, #0
	movi	v19.16b, #0

Lselect_w7_loop:
    // Loop 64 times.

    // Increment index (loop counter); tested at the end of the loop
	add	w9, w9, #1

    // [v22-v25] := Load a (2*256-bit = 4*128-bit) table entry starting at x1
    //  and advance x1 to point to the next entry
	ld1	{v22.2d, v23.2d, v24.2d, v25.2d}, [x1],#64

    // x11 := (w9 == w2)? All 1s : All 0s
	cmp	w9, w2
	csetm	x11, eq

    // duplicate mask_64 into Mask (all 0s or all 1s)
	dup	v3.2d, x11

    // [v16-v19] := (Mask == all 1s)? [v22-v25] : [v16-v19]
    // i.e., values in output registers will remain the same if w9 != w2
	bit	v16.16b, v22.16b, v3.16b
	bit	v17.16b, v23.16b, v3.16b

	bit	v18.16b, v24.16b, v3.16b
	bit	v19.16b, v25.16b, v3.16b

    // If bit #6 is not 0 (i.e. idx_ctr < 64) loop back
	tbz	w9, #6, Lselect_w7_loop

    // Write [v16-v19] to memory at the output pointer
	st1	{v16.2d, v17.2d, v18.2d, v19.2d}, [x0]

	ret

#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_AARCH64) && defined(_WIN32)
