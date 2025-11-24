// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_AARCH64) && defined(_WIN32)
#if __ARM_MAX_ARCH__>=7
.text
.arch	armv8-a+crypto
.section	.rodata
.align	5
Lrcon:
.long	0x01,0x01,0x01,0x01
.long	0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d	// rotate-n-splat
.long	0x1b,0x1b,0x1b,0x1b

.text

.globl	aes_hw_set_encrypt_key

.def aes_hw_set_encrypt_key
   .type 32
.endef
.align	5
aes_hw_set_encrypt_key:
Lenc_key:
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	AARCH64_VALID_CALL_TARGET
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
	mov	x3,#-2
	cmp	w1,#128
	b.lt	Lenc_key_abort
	cmp	w1,#256
	b.gt	Lenc_key_abort
	tst	w1,#0x3f
	b.ne	Lenc_key_abort

	adrp	x3,Lrcon
	add	x3,x3,:lo12:Lrcon
	cmp	w1,#192

	eor	v0.16b,v0.16b,v0.16b
	ld1	{v3.16b},[x0],#16
	mov	w1,#8		// reuse w1
	ld1	{v1.4s,v2.4s},[x3],#32

	b.lt	Loop128
	// 192-bit key support was removed.
	b	L256

.align	4
Loop128:
	tbl	v6.16b,{v3.16b},v2.16b
	ext	v5.16b,v0.16b,v3.16b,#12
	st1	{v3.4s},[x2],#16
	aese	v6.16b,v0.16b
	subs	w1,w1,#1

	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v6.16b,v6.16b,v1.16b
	eor	v3.16b,v3.16b,v5.16b
	shl	v1.16b,v1.16b,#1
	eor	v3.16b,v3.16b,v6.16b
	b.ne	Loop128

	ld1	{v1.4s},[x3]

	tbl	v6.16b,{v3.16b},v2.16b
	ext	v5.16b,v0.16b,v3.16b,#12
	st1	{v3.4s},[x2],#16
	aese	v6.16b,v0.16b

	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v6.16b,v6.16b,v1.16b
	eor	v3.16b,v3.16b,v5.16b
	shl	v1.16b,v1.16b,#1
	eor	v3.16b,v3.16b,v6.16b

	tbl	v6.16b,{v3.16b},v2.16b
	ext	v5.16b,v0.16b,v3.16b,#12
	st1	{v3.4s},[x2],#16
	aese	v6.16b,v0.16b

	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v6.16b,v6.16b,v1.16b
	eor	v3.16b,v3.16b,v5.16b
	eor	v3.16b,v3.16b,v6.16b
	st1	{v3.4s},[x2]
	add	x2,x2,#0x50

	mov	w12,#10
	b	Ldone

// 192-bit key support was removed.

.align	4
L256:
	ld1	{v4.16b},[x0]
	mov	w1,#7
	mov	w12,#14
	st1	{v3.4s},[x2],#16

Loop256:
	tbl	v6.16b,{v4.16b},v2.16b
	ext	v5.16b,v0.16b,v3.16b,#12
	st1	{v4.4s},[x2],#16
	aese	v6.16b,v0.16b
	subs	w1,w1,#1

	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v3.16b,v3.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v6.16b,v6.16b,v1.16b
	eor	v3.16b,v3.16b,v5.16b
	shl	v1.16b,v1.16b,#1
	eor	v3.16b,v3.16b,v6.16b
	st1	{v3.4s},[x2],#16
	b.eq	Ldone

	dup	v6.4s,v3.s[3]		// just splat
	ext	v5.16b,v0.16b,v4.16b,#12
	aese	v6.16b,v0.16b

	eor	v4.16b,v4.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v4.16b,v4.16b,v5.16b
	ext	v5.16b,v0.16b,v5.16b,#12
	eor	v4.16b,v4.16b,v5.16b

	eor	v4.16b,v4.16b,v6.16b
	b	Loop256

Ldone:
	str	w12,[x2]
	mov	x3,#0

Lenc_key_abort:
	mov	x0,x3			// return value
	ldr	x29,[sp],#16
	ret

.globl	aes_hw_ctr32_encrypt_blocks

.def aes_hw_ctr32_encrypt_blocks
   .type 32
.endef
.align	5
aes_hw_ctr32_encrypt_blocks:
	// Armv8.3-A PAuth: even though x30 is pushed to stack it is not popped later.
	AARCH64_VALID_CALL_TARGET
	stp	x29,x30,[sp,#-16]!
	add	x29,sp,#0
	ldr	w5,[x3,#240]

	ldr	w8, [x4, #12]
	ld1	{v0.4s},[x4]

	ld1	{v16.4s,v17.4s},[x3]		// load key schedule...
	sub	w5,w5,#4
	mov	x12,#16
	cmp	x2,#2
	add	x7,x3,x5,lsl#4	// pointer to last 5 round keys
	sub	w5,w5,#2
	ld1	{v20.4s,v21.4s},[x7],#32
	ld1	{v22.4s,v23.4s},[x7],#32
	ld1	{v7.4s},[x7]
	add	x7,x3,#32
	mov	w6,w5
	csel	x12,xzr,x12,lo

	// ARM Cortex-A57 and Cortex-A72 cores running in 32-bit mode are
	// affected by silicon errata #1742098 [0] and #1655431 [1],
	// respectively, where the second instruction of an aese/aesmc
	// instruction pair may execute twice if an interrupt is taken right
	// after the first instruction consumes an input register of which a
	// single 32-bit lane has been updated the last time it was modified.
	//
	// This function uses a counter in one 32-bit lane. The vmov lines
	// could write to v1.16b and v18.16b directly, but that trips this bugs.
	// We write to v6.16b and copy to the final register as a workaround.
	//
	// [0] ARM-EPM-049219 v23 Cortex-A57 MPCore Software Developers Errata Notice
	// [1] ARM-EPM-012079 v11.0 Cortex-A72 MPCore Software Developers Errata Notice
#ifndef __AARCH64EB__
	rev	w8, w8
#endif
	add	w10, w8, #1
	orr	v6.16b,v0.16b,v0.16b
	rev	w10, w10
	mov	v6.s[3],w10
	add	w8, w8, #2
	orr	v1.16b,v6.16b,v6.16b
	b.ls	Lctr32_tail
	rev	w12, w8
	mov	v6.s[3],w12
	sub	x2,x2,#3		// bias
	orr	v18.16b,v6.16b,v6.16b
	b	Loop3x_ctr32

.align	4
Loop3x_ctr32:
	aese	v0.16b,v16.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v16.16b
	aesmc	v1.16b,v1.16b
	aese	v18.16b,v16.16b
	aesmc	v18.16b,v18.16b
	ld1	{v16.4s},[x7],#16
	subs	w6,w6,#2
	aese	v0.16b,v17.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v17.16b
	aesmc	v1.16b,v1.16b
	aese	v18.16b,v17.16b
	aesmc	v18.16b,v18.16b
	ld1	{v17.4s},[x7],#16
	b.gt	Loop3x_ctr32

	aese	v0.16b,v16.16b
	aesmc	v4.16b,v0.16b
	aese	v1.16b,v16.16b
	aesmc	v5.16b,v1.16b
	ld1	{v2.16b},[x0],#16
	add	w9,w8,#1
	aese	v18.16b,v16.16b
	aesmc	v18.16b,v18.16b
	ld1	{v3.16b},[x0],#16
	rev	w9,w9
	aese	v4.16b,v17.16b
	aesmc	v4.16b,v4.16b
	aese	v5.16b,v17.16b
	aesmc	v5.16b,v5.16b
	ld1	{v19.16b},[x0],#16
	mov	x7,x3
	aese	v18.16b,v17.16b
	aesmc	v17.16b,v18.16b
	aese	v4.16b,v20.16b
	aesmc	v4.16b,v4.16b
	aese	v5.16b,v20.16b
	aesmc	v5.16b,v5.16b
	eor	v2.16b,v2.16b,v7.16b
	add	w10,w8,#2
	aese	v17.16b,v20.16b
	aesmc	v17.16b,v17.16b
	eor	v3.16b,v3.16b,v7.16b
	add	w8,w8,#3
	aese	v4.16b,v21.16b
	aesmc	v4.16b,v4.16b
	aese	v5.16b,v21.16b
	aesmc	v5.16b,v5.16b
	 // Note the logic to update v0.16b, v1.16b, and v1.16b is written to work
	 // around a bug in ARM Cortex-A57 and Cortex-A72 cores running in
	 // 32-bit mode. See the comment above.
	eor	v19.16b,v19.16b,v7.16b
	mov	v6.s[3], w9
	aese	v17.16b,v21.16b
	aesmc	v17.16b,v17.16b
	orr	v0.16b,v6.16b,v6.16b
	rev	w10,w10
	aese	v4.16b,v22.16b
	aesmc	v4.16b,v4.16b
	mov	v6.s[3], w10
	rev	w12,w8
	aese	v5.16b,v22.16b
	aesmc	v5.16b,v5.16b
	orr	v1.16b,v6.16b,v6.16b
	mov	v6.s[3], w12
	aese	v17.16b,v22.16b
	aesmc	v17.16b,v17.16b
	orr	v18.16b,v6.16b,v6.16b
	subs	x2,x2,#3
	aese	v4.16b,v23.16b
	aese	v5.16b,v23.16b
	aese	v17.16b,v23.16b

	eor	v2.16b,v2.16b,v4.16b
	ld1	{v16.4s},[x7],#16	// re-pre-load rndkey[0]
	st1	{v2.16b},[x1],#16
	eor	v3.16b,v3.16b,v5.16b
	mov	w6,w5
	st1	{v3.16b},[x1],#16
	eor	v19.16b,v19.16b,v17.16b
	ld1	{v17.4s},[x7],#16	// re-pre-load rndkey[1]
	st1	{v19.16b},[x1],#16
	b.hs	Loop3x_ctr32

	adds	x2,x2,#3
	b.eq	Lctr32_done
	cmp	x2,#1
	mov	x12,#16
	csel	x12,xzr,x12,eq

Lctr32_tail:
	aese	v0.16b,v16.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v16.16b
	aesmc	v1.16b,v1.16b
	ld1	{v16.4s},[x7],#16
	subs	w6,w6,#2
	aese	v0.16b,v17.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v17.16b
	aesmc	v1.16b,v1.16b
	ld1	{v17.4s},[x7],#16
	b.gt	Lctr32_tail

	aese	v0.16b,v16.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v16.16b
	aesmc	v1.16b,v1.16b
	aese	v0.16b,v17.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v17.16b
	aesmc	v1.16b,v1.16b
	ld1	{v2.16b},[x0],x12
	aese	v0.16b,v20.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v20.16b
	aesmc	v1.16b,v1.16b
	ld1	{v3.16b},[x0]
	aese	v0.16b,v21.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v21.16b
	aesmc	v1.16b,v1.16b
	eor	v2.16b,v2.16b,v7.16b
	aese	v0.16b,v22.16b
	aesmc	v0.16b,v0.16b
	aese	v1.16b,v22.16b
	aesmc	v1.16b,v1.16b
	eor	v3.16b,v3.16b,v7.16b
	aese	v0.16b,v23.16b
	aese	v1.16b,v23.16b

	cmp	x2,#1
	eor	v2.16b,v2.16b,v0.16b
	eor	v3.16b,v3.16b,v1.16b
	st1	{v2.16b},[x1],#16
	b.eq	Lctr32_done
	st1	{v3.16b},[x1]

Lctr32_done:
	ldr	x29,[sp],#16
	ret

#endif
#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_AARCH64) && defined(_WIN32)
