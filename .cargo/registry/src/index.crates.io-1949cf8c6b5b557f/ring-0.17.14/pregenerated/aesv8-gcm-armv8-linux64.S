// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_AARCH64) && defined(__ELF__)
#if __ARM_MAX_ARCH__ >= 8

.arch	armv8-a+crypto
.text
.globl	aes_gcm_enc_kernel
.hidden	aes_gcm_enc_kernel
.type	aes_gcm_enc_kernel,%function
.align	4
aes_gcm_enc_kernel:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29, x30, [sp, #-128]!
	mov	x29, sp
	stp	x19, x20, [sp, #16]
	mov	x16, x4
	mov	x8, x5
	stp	x21, x22, [sp, #32]
	stp	x23, x24, [sp, #48]
	stp	d8, d9, [sp, #64]
	stp	d10, d11, [sp, #80]
	stp	d12, d13, [sp, #96]
	stp	d14, d15, [sp, #112]
	ldr	w17, [x8, #240]
	add	x19, x8, x17, lsl #4                   // borrow input_l1 for last key
	ldp	x13, x14, [x19]                       // load round N keys
	ldr	q31, [x19, #-16]                        // load round N-1 keys
	add	x4, x0, x1, lsr #3   // end_input_ptr
	lsr	x5, x1, #3              // byte_len
	mov	x15, x5
	ldp	x10, x11, [x16]              // ctr96_b64, ctr96_t32
	ld1	{ v0.16b}, [x16]                             // special case vector load initial counter so we can start first AES block as quickly as possible
	sub	x5, x5, #1      // byte_len - 1
	ldr	q18, [x8, #0]                                  // load rk0
	and	x5, x5, #0xffffffffffffffc0 // number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	ldr	q25, [x8, #112]                                // load rk7
	add	x5, x5, x0
	lsr	x12, x11, #32
	fmov	d2, x10                               // CTR block 2
	orr	w11, w11, w11
	rev	w12, w12                                // rev_ctr32
	fmov	d1, x10                               // CTR block 1
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 0
	add	w12, w12, #1                            // increment rev_ctr32
	rev	w9, w12                                 // CTR block 1
	fmov	d3, x10                               // CTR block 3
	orr	x9, x11, x9, lsl #32            // CTR block 1
	add	w12, w12, #1                            // CTR block 1
	ldr	q19, [x8, #16]                                 // load rk1
	fmov	v1.d[1], x9                               // CTR block 1
	rev	w9, w12                                 // CTR block 2
	add	w12, w12, #1                            // CTR block 2
	orr	x9, x11, x9, lsl #32            // CTR block 2
	ldr	q20, [x8, #32]                                 // load rk2
	fmov	v2.d[1], x9                               // CTR block 2
	rev	w9, w12                                 // CTR block 3
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 1
	orr	x9, x11, x9, lsl #32            // CTR block 3
	fmov	v3.d[1], x9                               // CTR block 3
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 0
	ldr	q21, [x8, #48]                                 // load rk3
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 2
	ldr	q24, [x8, #96]                                 // load rk6
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 0
	ldr	q23, [x8, #80]                                 // load rk5
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 1
	ldr	q14, [x6, #48]                              // load h3l | h3h
	ext	v14.16b, v14.16b, v14.16b, #8
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 0
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 1
	ldr	q22, [x8, #64]                                 // load rk4
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 2
	ldr	q13, [x6, #32]                              // load h2l | h2h
	ext	v13.16b, v13.16b, v13.16b, #8
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 1
	ldr	q30, [x8, #192]                               // load rk12
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 2
	ldr	q15, [x6, #80]                              // load h4l | h4h
	ext	v15.16b, v15.16b, v15.16b, #8
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 3
	ldr	q29, [x8, #176]                               // load rk11
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 2
	ldr	q26, [x8, #128]                                // load rk8
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 3
	add	w12, w12, #1                            // CTR block 3
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 3
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 3
	ld1	{ v11.16b}, [x3]
	ext	v11.16b, v11.16b, v11.16b, #8
	rev64	v11.16b, v11.16b
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 4
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 4
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 4
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 4
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 5
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 5
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 5
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 5
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 6
	trn2	v17.2d,  v14.2d,    v15.2d                      // h4l | h3l
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 6
	ldr	q27, [x8, #144]                                // load rk9
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 6
	ldr	q12, [x6]                                   // load h1l | h1h
	ext	v12.16b, v12.16b, v12.16b, #8
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 6
	ldr	q28, [x8, #160]                               // load rk10
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 7
	trn1	v9.2d, v14.2d,    v15.2d                      // h4h | h3h
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 7
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 7
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 7
	trn2	v16.2d,  v12.2d,    v13.2d                      // h2l | h1l
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 8
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 8
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 8
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 8
	b.lt	.Lenc_finish_first_blocks                         // branch if AES-128

	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 9
	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 9
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 10
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 10
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 10
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 10
	b.eq	.Lenc_finish_first_blocks                         // branch if AES-192

	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 11
	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 11
	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 11
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 11
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 12
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 12
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 12

.Lenc_finish_first_blocks:
	cmp	x0, x5                   // check if we have <= 4 blocks
	eor	v17.16b, v17.16b, v9.16b                  // h4k | h3k
	aese	v2.16b, v31.16b                                    // AES block 2 - round N-1
	trn1	v8.2d,    v12.2d,    v13.2d                      // h2h | h1h
	aese	v1.16b, v31.16b                                    // AES block 1 - round N-1
	aese	v0.16b, v31.16b                                    // AES block 0 - round N-1
	aese	v3.16b, v31.16b                                    // AES block 3 - round N-1
	eor	v16.16b, v16.16b, v8.16b                     // h2k | h1k
	b.ge	.Lenc_tail                                        // handle tail

	ldp	x19, x20, [x0, #16]           // AES block 1 - load plaintext
	rev	w9, w12                                 // CTR block 4
	ldp	x6, x7, [x0, #0]            // AES block 0 - load plaintext
	ldp	x23, x24, [x0, #48]           // AES block 3 - load plaintext
	ldp	x21, x22, [x0, #32]           // AES block 2 - load plaintext
	add	x0, x0, #64                       // AES input_ptr update
	eor	x19, x19, x13                      // AES block 1 - round N low
	eor	x20, x20, x14                      // AES block 1 - round N high
	fmov	d5, x19                               // AES block 1 - mov low
	eor	x6, x6, x13                      // AES block 0 - round N low
	eor	x7, x7, x14                      // AES block 0 - round N high
	eor	x24, x24, x14                      // AES block 3 - round N high
	fmov	d4, x6                               // AES block 0 - mov low
	cmp	x0, x5                   // check if we have <= 8 blocks
	fmov	v4.d[1], x7                           // AES block 0 - mov high
	eor	x23, x23, x13                      // AES block 3 - round N low
	eor	x21, x21, x13                      // AES block 2 - round N low
	fmov	v5.d[1], x20                           // AES block 1 - mov high
	fmov	d6, x21                               // AES block 2 - mov low
	add	w12, w12, #1                            // CTR block 4
	orr	x9, x11, x9, lsl #32            // CTR block 4
	fmov	d7, x23                               // AES block 3 - mov low
	eor	x22, x22, x14                      // AES block 2 - round N high
	fmov	v6.d[1], x22                           // AES block 2 - mov high
	eor	v4.16b, v4.16b, v0.16b                          // AES block 0 - result
	fmov	d0, x10                               // CTR block 4
	fmov	v0.d[1], x9                               // CTR block 4
	rev	w9, w12                                 // CTR block 5
	add	w12, w12, #1                            // CTR block 5
	eor	v5.16b, v5.16b, v1.16b                          // AES block 1 - result
	fmov	d1, x10                               // CTR block 5
	orr	x9, x11, x9, lsl #32            // CTR block 5
	fmov	v1.d[1], x9                               // CTR block 5
	rev	w9, w12                                 // CTR block 6
	st1	{ v4.16b}, [x2], #16                     // AES block 0 - store result
	fmov	v7.d[1], x24                           // AES block 3 - mov high
	orr	x9, x11, x9, lsl #32            // CTR block 6
	eor	v6.16b, v6.16b, v2.16b                          // AES block 2 - result
	st1	{ v5.16b}, [x2], #16                     // AES block 1 - store result
	add	w12, w12, #1                            // CTR block 6
	fmov	d2, x10                               // CTR block 6
	fmov	v2.d[1], x9                               // CTR block 6
	st1	{ v6.16b}, [x2], #16                     // AES block 2 - store result
	rev	w9, w12                                 // CTR block 7
	orr	x9, x11, x9, lsl #32            // CTR block 7
	eor	v7.16b, v7.16b, v3.16b                          // AES block 3 - result
	st1	{ v7.16b}, [x2], #16                     // AES block 3 - store result
	b.ge	.Lenc_prepretail                                  // do prepretail

.Lenc_main_loop:	//	main loop start
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 0
	rev64	v4.16b, v4.16b                                    // GHASH block 4k (only t0 is free)
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 0
	fmov	d3, x10                               // CTR block 4k+3
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 0
	ext	v11.16b, v11.16b, v11.16b, #8                     // PRE 0
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 1
	fmov	v3.d[1], x9                               // CTR block 4k+3
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 1
	ldp	x23, x24, [x0, #48]           // AES block 4k+7 - load plaintext
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 1
	ldp	x21, x22, [x0, #32]           // AES block 4k+6 - load plaintext
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 2
	eor	v4.16b, v4.16b, v11.16b                           // PRE 1
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 2
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 0
	eor	x23, x23, x13                      // AES block 4k+7 - round N low
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 3
	mov	d10, v17.d[1]                               // GHASH block 4k - mid
	pmull2	v9.1q, v4.2d, v15.2d                       // GHASH block 4k - high
	eor	x22, x22, x14                      // AES block 4k+6 - round N high
	mov	d8, v4.d[1]                                  // GHASH block 4k - mid
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 1
	rev64	v5.16b, v5.16b                                    // GHASH block 4k+1 (t0 and t1 free)
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 4
	pmull	v11.1q, v4.1d, v15.1d                       // GHASH block 4k - low
	eor	v8.8b, v8.8b, v4.8b                          // GHASH block 4k - mid
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 2
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 5
	rev64	v7.16b, v7.16b                                    // GHASH block 4k+3 (t0, t1, t2 and t3 free)
	pmull2	v4.1q, v5.2d, v14.2d                          // GHASH block 4k+1 - high
	pmull	v10.1q, v8.1d, v10.1d                      // GHASH block 4k - mid
	rev64	v6.16b, v6.16b                                    // GHASH block 4k+2 (t0, t1, and t2 free)
	pmull	v8.1q, v5.1d, v14.1d                          // GHASH block 4k+1 - low
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+1 - high
	mov	d4, v5.d[1]                                  // GHASH block 4k+1 - mid
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 3
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 2
	eor	v11.16b, v11.16b, v8.16b                         // GHASH block 4k+1 - low
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 3
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 4
	mov	d8, v6.d[1]                                  // GHASH block 4k+2 - mid
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 3
	eor	v4.8b, v4.8b, v5.8b                          // GHASH block 4k+1 - mid
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 4
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 6
	eor	v8.8b, v8.8b, v6.8b                          // GHASH block 4k+2 - mid
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 4
	pmull	v4.1q, v4.1d, v17.1d                          // GHASH block 4k+1 - mid
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 7
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 5
	ins	v8.d[1], v8.d[0]                                // GHASH block 4k+2 - mid
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 5
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 8
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 5
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 6
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+1 - mid
	pmull2	v4.1q, v6.2d, v13.2d                          // GHASH block 4k+2 - high
	pmull	v5.1q, v6.1d, v13.1d                          // GHASH block 4k+2 - low
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 7
	pmull	v6.1q, v7.1d, v12.1d                          // GHASH block 4k+3 - low
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+2 - high
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 6
	ldp	x19, x20, [x0, #16]           // AES block 4k+5 - load plaintext
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 8
	mov	d4, v7.d[1]                                  // GHASH block 4k+3 - mid
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 6
	eor	v11.16b, v11.16b, v5.16b                         // GHASH block 4k+2 - low
	pmull2	v8.1q, v8.2d, v16.2d                          // GHASH block 4k+2 - mid
	pmull2	v5.1q, v7.2d, v12.2d                          // GHASH block 4k+3 - high
	eor	v4.8b, v4.8b, v7.8b                          // GHASH block 4k+3 - mid
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 7
	eor	x19, x19, x13                      // AES block 4k+5 - round N low
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 8
	eor	v10.16b, v10.16b, v8.16b                         // GHASH block 4k+2 - mid
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 7
	eor	x21, x21, x13                      // AES block 4k+6 - round N low
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 8
	movi	v8.8b, #0xc2
	pmull	v4.1q, v4.1d, v16.1d                          // GHASH block 4k+3 - mid
	eor	v9.16b, v9.16b, v5.16b                         // GHASH block 4k+3 - high
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	fmov	d5, x19                               // AES block 4k+5 - mov low
	ldp	x6, x7, [x0, #0]            // AES block 4k+4 - load plaintext
	b.lt	.Lenc_main_loop_continue                          // branch if AES-128

	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 9
	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 9
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 10
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 10
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 10
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 10
	b.eq	.Lenc_main_loop_continue                          // branch if AES-192

	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 11
	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 11
	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 11
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 11
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 12
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 12
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 12

.Lenc_main_loop_continue:
	shl	d8, d8, #56               // mod_constant
	eor	v11.16b, v11.16b, v6.16b                         // GHASH block 4k+3 - low
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+3 - mid
	add	w12, w12, #1                            // CTR block 4k+3
	eor	v4.16b, v11.16b, v9.16b                         // MODULO - karatsuba tidy up
	add	x0, x0, #64                       // AES input_ptr update
	pmull	v7.1q, v9.1d, v8.1d            // MODULO - top 64b align with mid
	rev	w9, w12                                 // CTR block 4k+8
	ext	v9.16b, v9.16b, v9.16b, #8                     // MODULO - other top alignment
	eor	x6, x6, x13                      // AES block 4k+4 - round N low
	eor	v10.16b, v10.16b, v4.16b                         // MODULO - karatsuba tidy up
	eor	x7, x7, x14                      // AES block 4k+4 - round N high
	fmov	d4, x6                               // AES block 4k+4 - mov low
	orr	x9, x11, x9, lsl #32            // CTR block 4k+8
	eor	v7.16b, v9.16b, v7.16b                   // MODULO - fold into mid
	eor	x20, x20, x14                      // AES block 4k+5 - round N high
	eor	x24, x24, x14                      // AES block 4k+7 - round N high
	add	w12, w12, #1                            // CTR block 4k+8
	aese	v0.16b, v31.16b                                    // AES block 4k+4 - round N-1
	fmov	v4.d[1], x7                           // AES block 4k+4 - mov high
	eor	v10.16b, v10.16b, v7.16b                      // MODULO - fold into mid
	fmov	d7, x23                               // AES block 4k+7 - mov low
	aese	v1.16b, v31.16b                                    // AES block 4k+5 - round N-1
	fmov	v5.d[1], x20                           // AES block 4k+5 - mov high
	fmov	d6, x21                               // AES block 4k+6 - mov low
	cmp	x0, x5                   // .LOOP CONTROL
	fmov	v6.d[1], x22                           // AES block 4k+6 - mov high
	pmull	v9.1q, v10.1d, v8.1d            // MODULO - mid 64b align with low
	eor	v4.16b, v4.16b, v0.16b                          // AES block 4k+4 - result
	fmov	d0, x10                               // CTR block 4k+8
	fmov	v0.d[1], x9                               // CTR block 4k+8
	rev	w9, w12                                 // CTR block 4k+9
	add	w12, w12, #1                            // CTR block 4k+9
	eor	v5.16b, v5.16b, v1.16b                          // AES block 4k+5 - result
	fmov	d1, x10                               // CTR block 4k+9
	orr	x9, x11, x9, lsl #32            // CTR block 4k+9
	fmov	v1.d[1], x9                               // CTR block 4k+9
	aese	v2.16b, v31.16b                                    // AES block 4k+6 - round N-1
	rev	w9, w12                                 // CTR block 4k+10
	st1	{ v4.16b}, [x2], #16                     // AES block 4k+4 - store result
	orr	x9, x11, x9, lsl #32            // CTR block 4k+10
	eor	v11.16b, v11.16b, v9.16b                         // MODULO - fold into low
	fmov	v7.d[1], x24                           // AES block 4k+7 - mov high
	ext	v10.16b, v10.16b, v10.16b, #8                     // MODULO - other mid alignment
	st1	{ v5.16b}, [x2], #16                     // AES block 4k+5 - store result
	add	w12, w12, #1                            // CTR block 4k+10
	aese	v3.16b, v31.16b                                    // AES block 4k+7 - round N-1
	eor	v6.16b, v6.16b, v2.16b                          // AES block 4k+6 - result
	fmov	d2, x10                               // CTR block 4k+10
	st1	{ v6.16b}, [x2], #16                     // AES block 4k+6 - store result
	fmov	v2.d[1], x9                               // CTR block 4k+10
	rev	w9, w12                                 // CTR block 4k+11
	eor	v11.16b, v11.16b, v10.16b                         // MODULO - fold into low
	orr	x9, x11, x9, lsl #32            // CTR block 4k+11
	eor	v7.16b, v7.16b, v3.16b                          // AES block 4k+7 - result
	st1	{ v7.16b}, [x2], #16                     // AES block 4k+7 - store result
	b.lt	.Lenc_main_loop

.Lenc_prepretail:	//	PREPRETAIL
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 0
	rev64	v6.16b, v6.16b                                    // GHASH block 4k+2 (t0, t1, and t2 free)
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 0
	fmov	d3, x10                               // CTR block 4k+3
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 0
	rev64	v4.16b, v4.16b                                    // GHASH block 4k (only t0 is free)
	fmov	v3.d[1], x9                               // CTR block 4k+3
	ext	v11.16b, v11.16b, v11.16b, #8                     // PRE 0
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 1
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 1
	eor	v4.16b, v4.16b, v11.16b                           // PRE 1
	rev64	v5.16b, v5.16b                                    // GHASH block 4k+1 (t0 and t1 free)
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 2
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 0
	mov	d10, v17.d[1]                               // GHASH block 4k - mid
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 1
	pmull	v11.1q, v4.1d, v15.1d                       // GHASH block 4k - low
	mov	d8, v4.d[1]                                  // GHASH block 4k - mid
	pmull2	v9.1q, v4.2d, v15.2d                       // GHASH block 4k - high
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 3
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 2
	eor	v8.8b, v8.8b, v4.8b                          // GHASH block 4k - mid
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 2
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 1
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 3
	pmull	v10.1q, v8.1d, v10.1d                      // GHASH block 4k - mid
	pmull2	v4.1q, v5.2d, v14.2d                          // GHASH block 4k+1 - high
	pmull	v8.1q, v5.1d, v14.1d                          // GHASH block 4k+1 - low
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 2
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+1 - high
	mov	d4, v5.d[1]                                  // GHASH block 4k+1 - mid
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 3
	eor	v11.16b, v11.16b, v8.16b                         // GHASH block 4k+1 - low
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 3
	eor	v4.8b, v4.8b, v5.8b                          // GHASH block 4k+1 - mid
	mov	d8, v6.d[1]                                  // GHASH block 4k+2 - mid
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 4
	rev64	v7.16b, v7.16b                                    // GHASH block 4k+3 (t0, t1, t2 and t3 free)
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 4
	pmull	v4.1q, v4.1d, v17.1d                          // GHASH block 4k+1 - mid
	eor	v8.8b, v8.8b, v6.8b                          // GHASH block 4k+2 - mid
	add	w12, w12, #1                            // CTR block 4k+3
	pmull	v5.1q, v6.1d, v13.1d                          // GHASH block 4k+2 - low
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 5
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 4
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+1 - mid
	pmull2	v4.1q, v6.2d, v13.2d                          // GHASH block 4k+2 - high
	eor	v11.16b, v11.16b, v5.16b                         // GHASH block 4k+2 - low
	ins	v8.d[1], v8.d[0]                                // GHASH block 4k+2 - mid
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 5
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+2 - high
	mov	d4, v7.d[1]                                  // GHASH block 4k+3 - mid
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 4
	pmull2	v8.1q, v8.2d, v16.2d                          // GHASH block 4k+2 - mid
	eor	v4.8b, v4.8b, v7.8b                          // GHASH block 4k+3 - mid
	pmull2	v5.1q, v7.2d, v12.2d                          // GHASH block 4k+3 - high
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 5
	pmull	v4.1q, v4.1d, v16.1d                          // GHASH block 4k+3 - mid
	eor	v10.16b, v10.16b, v8.16b                         // GHASH block 4k+2 - mid
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 5
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 6
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 6
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 6
	movi	v8.8b, #0xc2
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 6
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 7
	eor	v9.16b, v9.16b, v5.16b                         // GHASH block 4k+3 - high
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 7
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 7
	shl	d8, d8, #56               // mod_constant
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 8
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+3 - mid
	pmull	v6.1q, v7.1d, v12.1d                          // GHASH block 4k+3 - low
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 8
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 8
	eor	v11.16b, v11.16b, v6.16b                         // GHASH block 4k+3 - low
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 7
	eor	v10.16b, v10.16b, v9.16b                         // karatsuba tidy up
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 8
	pmull	v4.1q, v9.1d, v8.1d
	ext	v9.16b, v9.16b, v9.16b, #8
	eor	v10.16b, v10.16b, v11.16b
	b.lt	.Lenc_finish_prepretail                           // branch if AES-128

	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 9
	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 9
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 10
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 10
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 10
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 10
	b.eq	.Lenc_finish_prepretail                           // branch if AES-192

	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 11
	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 11
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 11
	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 11
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 12
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 12
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 12

.Lenc_finish_prepretail:
	eor	v10.16b, v10.16b, v4.16b
	eor	v10.16b, v10.16b, v9.16b
	pmull	v4.1q, v10.1d, v8.1d
	ext	v10.16b, v10.16b, v10.16b, #8
	aese	v1.16b, v31.16b                                    // AES block 4k+5 - round N-1
	eor	v11.16b, v11.16b, v4.16b
	aese	v3.16b, v31.16b                                    // AES block 4k+7 - round N-1
	aese	v0.16b, v31.16b                                    // AES block 4k+4 - round N-1
	aese	v2.16b, v31.16b                                    // AES block 4k+6 - round N-1
	eor	v11.16b, v11.16b, v10.16b

.Lenc_tail:	//	TAIL
	ext	v8.16b, v11.16b, v11.16b, #8                     // prepare final partial tag
	sub	x5, x4, x0   // main_end_input_ptr is number of bytes left to process
	ldp	x6, x7, [x0], #16           // AES block 4k+4 - load plaintext
	eor	x6, x6, x13                      // AES block 4k+4 - round N low
	eor	x7, x7, x14                      // AES block 4k+4 - round N high
	cmp	x5, #48
	fmov	d4, x6                               // AES block 4k+4 - mov low
	fmov	v4.d[1], x7                           // AES block 4k+4 - mov high
	eor	v5.16b, v4.16b, v0.16b                          // AES block 4k+4 - result
	b.gt	.Lenc_blocks_more_than_3
	cmp	x5, #32
	mov	v3.16b, v2.16b
	movi	v11.8b, #0
	movi	v9.8b, #0
	sub	w12, w12, #1
	mov	v2.16b, v1.16b
	movi	v10.8b, #0
	b.gt	.Lenc_blocks_more_than_2
	mov	v3.16b, v1.16b
	sub	w12, w12, #1
	cmp	x5, #16
	b.gt	.Lenc_blocks_more_than_1
	sub	w12, w12, #1
	b	.Lenc_blocks_less_than_1
.Lenc_blocks_more_than_3:	//	blocks left >  3
	st1	{ v5.16b}, [x2], #16                    // AES final-3 block  - store result
	ldp	x6, x7, [x0], #16          // AES final-2 block - load input low & high
	rev64	v4.16b, v5.16b                                   // GHASH final-3 block
	eor	x6, x6, x13                     // AES final-2 block - round N low
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	eor	x7, x7, x14                     // AES final-2 block - round N high
	mov	d22, v4.d[1]                                // GHASH final-3 block - mid
	fmov	d5, x6                                // AES final-2 block - mov low
	fmov	v5.d[1], x7                            // AES final-2 block - mov high
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-3 block - mid
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	mov	d10, v17.d[1]                              // GHASH final-3 block - mid
	pmull	v11.1q, v4.1d, v15.1d                      // GHASH final-3 block - low
	pmull2	v9.1q, v4.2d, v15.2d                      // GHASH final-3 block - high
	pmull	v10.1q, v22.1d, v10.1d                   // GHASH final-3 block - mid
	eor	v5.16b, v5.16b, v1.16b                           // AES final-2 block - result
.Lenc_blocks_more_than_2:	//	blocks left >  2
	st1	{ v5.16b}, [x2], #16                    // AES final-2 block - store result
	ldp	x6, x7, [x0], #16          // AES final-1 block - load input low & high
	rev64	v4.16b, v5.16b                                   // GHASH final-2 block
	eor	x6, x6, x13                     // AES final-1 block - round N low
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	fmov	d5, x6                                // AES final-1 block - mov low
	eor	x7, x7, x14                     // AES final-1 block - round N high
	fmov	v5.d[1], x7                            // AES final-1 block - mov high
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	pmull2	v20.1q, v4.2d, v14.2d                         // GHASH final-2 block - high
	mov	d22, v4.d[1]                                // GHASH final-2 block - mid
	pmull	v21.1q, v4.1d, v14.1d                         // GHASH final-2 block - low
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-2 block - mid
	eor	v5.16b, v5.16b, v2.16b                           // AES final-1 block - result
	eor	v9.16b, v9.16b, v20.16b                           // GHASH final-2 block - high
	pmull	v22.1q, v22.1d, v17.1d                     // GHASH final-2 block - mid
	eor	v11.16b, v11.16b, v21.16b                           // GHASH final-2 block - low
	eor	v10.16b, v10.16b, v22.16b                      // GHASH final-2 block - mid
.Lenc_blocks_more_than_1:	//	blocks left >  1
	st1	{ v5.16b}, [x2], #16                    // AES final-1 block - store result
	rev64	v4.16b, v5.16b                                   // GHASH final-1 block
	ldp	x6, x7, [x0], #16          // AES final block - load input low & high
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	eor	x6, x6, x13                     // AES final block - round N low
	mov	d22, v4.d[1]                                // GHASH final-1 block - mid
	pmull2	v20.1q, v4.2d, v13.2d                         // GHASH final-1 block - high
	eor	x7, x7, x14                     // AES final block - round N high
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-1 block - mid
	eor	v9.16b, v9.16b, v20.16b                           // GHASH final-1 block - high
	ins	v22.d[1], v22.d[0]                           // GHASH final-1 block - mid
	fmov	d5, x6                                // AES final block - mov low
	fmov	v5.d[1], x7                            // AES final block - mov high
	pmull2	v22.1q, v22.2d, v16.2d                     // GHASH final-1 block - mid
	pmull	v21.1q, v4.1d, v13.1d                         // GHASH final-1 block - low
	eor	v5.16b, v5.16b, v3.16b                           // AES final block - result
	eor	v10.16b, v10.16b, v22.16b                      // GHASH final-1 block - mid
	eor	v11.16b, v11.16b, v21.16b                           // GHASH final-1 block - low
.Lenc_blocks_less_than_1:	//	blocks left <= 1
	and	x1, x1, #127                   // bit_length %= 128
	mvn	x13, xzr                                      // rkN_l = 0xffffffffffffffff
	sub	x1, x1, #128                   // bit_length -= 128
	neg	x1, x1                         // bit_length = 128 - #bits in input (in range [1,128])
	ld1	{ v18.16b}, [x2]                           // load existing bytes where the possibly partial last block is to be stored
	mvn	x14, xzr                                      // rkN_h = 0xffffffffffffffff
	and	x1, x1, #127                   // bit_length %= 128
	lsr	x14, x14, x1                      // rkN_h is mask for top 64b of last block
	cmp	x1, #64
	csel	x6, x13, x14, lt
	csel	x7, x14, xzr, lt
	fmov	d0, x6                                // ctr0b is mask for last block
	fmov	v0.d[1], x7
	and	v5.16b, v5.16b, v0.16b                           // possibly partial last block has zeroes in highest bits
	rev64	v4.16b, v5.16b                                   // GHASH final block
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	bif	v5.16b, v18.16b, v0.16b                             // insert existing bytes in top end of result before storing
	pmull2	v20.1q, v4.2d, v12.2d                         // GHASH final block - high
	mov	d8, v4.d[1]                                 // GHASH final block - mid
	rev	w9, w12
	pmull	v21.1q, v4.1d, v12.1d                         // GHASH final block - low
	eor	v9.16b, v9.16b, v20.16b                           // GHASH final block - high
	eor	v8.8b, v8.8b, v4.8b                         // GHASH final block - mid
	pmull	v8.1q, v8.1d, v16.1d                         // GHASH final block - mid
	eor	v11.16b, v11.16b, v21.16b                           // GHASH final block - low
	eor	v10.16b, v10.16b, v8.16b                        // GHASH final block - mid
	movi	v8.8b, #0xc2
	eor	v4.16b, v11.16b, v9.16b                        // MODULO - karatsuba tidy up
	shl	d8, d8, #56              // mod_constant
	eor	v10.16b, v10.16b, v4.16b                        // MODULO - karatsuba tidy up
	pmull	v7.1q, v9.1d, v8.1d           // MODULO - top 64b align with mid
	ext	v9.16b, v9.16b, v9.16b, #8                    // MODULO - other top alignment
	eor	v10.16b, v10.16b, v7.16b                     // MODULO - fold into mid
	eor	v10.16b, v10.16b, v9.16b                        // MODULO - fold into mid
	pmull	v9.1q, v10.1d, v8.1d           // MODULO - mid 64b align with low
	ext	v10.16b, v10.16b, v10.16b, #8                    // MODULO - other mid alignment
	str	w9, [x16, #12]                         // store the updated counter
	st1	{ v5.16b}, [x2]                         // store all 16B
	eor	v11.16b, v11.16b, v9.16b                        // MODULO - fold into low
	eor	v11.16b, v11.16b, v10.16b                        // MODULO - fold into low
	ext	v11.16b, v11.16b, v11.16b, #8
	rev64	v11.16b, v11.16b
	mov	x0, x15
	st1	{ v11.16b }, [x3]
	ldp	x19, x20, [sp, #16]
	ldp	x21, x22, [sp, #32]
	ldp	x23, x24, [sp, #48]
	ldp	d8, d9, [sp, #64]
	ldp	d10, d11, [sp, #80]
	ldp	d12, d13, [sp, #96]
	ldp	d14, d15, [sp, #112]
	ldp	x29, x30, [sp], #128
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	aes_gcm_enc_kernel,.-aes_gcm_enc_kernel
.globl	aes_gcm_dec_kernel
.hidden	aes_gcm_dec_kernel
.type	aes_gcm_dec_kernel,%function
.align	4
aes_gcm_dec_kernel:
	AARCH64_SIGN_LINK_REGISTER
	stp	x29, x30, [sp, #-128]!
	mov	x29, sp
	stp	x19, x20, [sp, #16]
	mov	x16, x4
	mov	x8, x5
	stp	x21, x22, [sp, #32]
	stp	x23, x24, [sp, #48]
	stp	d8, d9, [sp, #64]
	stp	d10, d11, [sp, #80]
	stp	d12, d13, [sp, #96]
	stp	d14, d15, [sp, #112]
	ldr	w17, [x8, #240]
	add	x19, x8, x17, lsl #4                   // borrow input_l1 for last key
	ldp	x13, x14, [x19]                       // load round N keys
	ldr	q31, [x19, #-16]                        // load round N-1 keys
	lsr	x5, x1, #3              // byte_len
	mov	x15, x5
	ldp	x10, x11, [x16]              // ctr96_b64, ctr96_t32
	ldr	q26, [x8, #128]                                // load rk8
	sub	x5, x5, #1      // byte_len - 1
	ldr	q25, [x8, #112]                                // load rk7
	and	x5, x5, #0xffffffffffffffc0 // number of bytes to be processed in main loop (at least 1 byte must be handled by tail)
	add	x4, x0, x1, lsr #3   // end_input_ptr
	ldr	q24, [x8, #96]                                 // load rk6
	lsr	x12, x11, #32
	ldr	q23, [x8, #80]                                 // load rk5
	orr	w11, w11, w11
	ldr	q21, [x8, #48]                                 // load rk3
	add	x5, x5, x0
	rev	w12, w12                                // rev_ctr32
	add	w12, w12, #1                            // increment rev_ctr32
	fmov	d3, x10                               // CTR block 3
	rev	w9, w12                                 // CTR block 1
	add	w12, w12, #1                            // CTR block 1
	fmov	d1, x10                               // CTR block 1
	orr	x9, x11, x9, lsl #32            // CTR block 1
	ld1	{ v0.16b}, [x16]                             // special case vector load initial counter so we can start first AES block as quickly as possible
	fmov	v1.d[1], x9                               // CTR block 1
	rev	w9, w12                                 // CTR block 2
	add	w12, w12, #1                            // CTR block 2
	fmov	d2, x10                               // CTR block 2
	orr	x9, x11, x9, lsl #32            // CTR block 2
	fmov	v2.d[1], x9                               // CTR block 2
	rev	w9, w12                                 // CTR block 3
	orr	x9, x11, x9, lsl #32            // CTR block 3
	ldr	q18, [x8, #0]                                  // load rk0
	fmov	v3.d[1], x9                               // CTR block 3
	add	w12, w12, #1                            // CTR block 3
	ldr	q22, [x8, #64]                                 // load rk4
	ldr	q19, [x8, #16]                                 // load rk1
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 0
	ldr	q14, [x6, #48]                              // load h3l | h3h
	ext	v14.16b, v14.16b, v14.16b, #8
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 0
	ldr	q15, [x6, #80]                              // load h4l | h4h
	ext	v15.16b, v15.16b, v15.16b, #8
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 0
	ldr	q13, [x6, #32]                              // load h2l | h2h
	ext	v13.16b, v13.16b, v13.16b, #8
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 0
	ldr	q20, [x8, #32]                                 // load rk2
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 1
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 1
	ld1	{ v11.16b}, [x3]
	ext	v11.16b, v11.16b, v11.16b, #8
	rev64	v11.16b, v11.16b
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 1
	ldr	q27, [x8, #144]                                // load rk9
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 1
	ldr	q30, [x8, #192]                               // load rk12
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 2
	ldr	q12, [x6]                                   // load h1l | h1h
	ext	v12.16b, v12.16b, v12.16b, #8
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 2
	ldr	q28, [x8, #160]                               // load rk10
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 2
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 3
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 2
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 3
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 4
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 3
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 3
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 4
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 4
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 4
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 5
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 5
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 5
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 5
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 6
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 6
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 6
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 6
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 7
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 7
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 7
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 8
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 7
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 8
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 8
	ldr	q29, [x8, #176]                               // load rk11
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 8
	b.lt	.Ldec_finish_first_blocks                         // branch if AES-128

	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 9
	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 9
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 10
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 10
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 10
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 10
	b.eq	.Ldec_finish_first_blocks                         // branch if AES-192

	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 11
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 11
	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 11
	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 11
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 1 - round 12
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 0 - round 12
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 2 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 3 - round 12

.Ldec_finish_first_blocks:
	cmp	x0, x5                   // check if we have <= 4 blocks
	trn1	v9.2d, v14.2d,    v15.2d                      // h4h | h3h
	trn2	v17.2d,  v14.2d,    v15.2d                      // h4l | h3l
	trn1	v8.2d,    v12.2d,    v13.2d                      // h2h | h1h
	trn2	v16.2d,  v12.2d,    v13.2d                      // h2l | h1l
	eor	v17.16b, v17.16b, v9.16b                  // h4k | h3k
	aese	v1.16b, v31.16b                                    // AES block 1 - round N-1
	aese	v2.16b, v31.16b                                    // AES block 2 - round N-1
	eor	v16.16b, v16.16b, v8.16b                     // h2k | h1k
	aese	v3.16b, v31.16b                                    // AES block 3 - round N-1
	aese	v0.16b, v31.16b                                    // AES block 0 - round N-1
	b.ge	.Ldec_tail                                        // handle tail

	ldr	q4, [x0, #0]                          // AES block 0 - load ciphertext
	ldr	q5, [x0, #16]                         // AES block 1 - load ciphertext
	rev	w9, w12                                 // CTR block 4
	eor	v0.16b, v4.16b, v0.16b                            // AES block 0 - result
	eor	v1.16b, v5.16b, v1.16b                            // AES block 1 - result
	rev64	v5.16b, v5.16b                                    // GHASH block 1
	ldr	q7, [x0, #48]                         // AES block 3 - load ciphertext
	mov	x7, v0.d[1]                            // AES block 0 - mov high
	mov	x6, v0.d[0]                            // AES block 0 - mov low
	rev64	v4.16b, v4.16b                                    // GHASH block 0
	add	w12, w12, #1                            // CTR block 4
	fmov	d0, x10                               // CTR block 4
	orr	x9, x11, x9, lsl #32            // CTR block 4
	fmov	v0.d[1], x9                               // CTR block 4
	rev	w9, w12                                 // CTR block 5
	add	w12, w12, #1                            // CTR block 5
	mov	x19, v1.d[0]                            // AES block 1 - mov low
	orr	x9, x11, x9, lsl #32            // CTR block 5
	mov	x20, v1.d[1]                            // AES block 1 - mov high
	eor	x7, x7, x14                    // AES block 0 - round N high
	eor	x6, x6, x13                    // AES block 0 - round N low
	stp	x6, x7, [x2], #16        // AES block 0 - store result
	fmov	d1, x10                               // CTR block 5
	ldr	q6, [x0, #32]                         // AES block 2 - load ciphertext
	add	x0, x0, #64                       // AES input_ptr update
	fmov	v1.d[1], x9                               // CTR block 5
	rev	w9, w12                                 // CTR block 6
	add	w12, w12, #1                            // CTR block 6
	eor	x19, x19, x13                    // AES block 1 - round N low
	orr	x9, x11, x9, lsl #32            // CTR block 6
	eor	x20, x20, x14                    // AES block 1 - round N high
	stp	x19, x20, [x2], #16        // AES block 1 - store result
	eor	v2.16b, v6.16b, v2.16b                            // AES block 2 - result
	cmp	x0, x5                   // check if we have <= 8 blocks
	b.ge	.Ldec_prepretail                                  // do prepretail

.Ldec_main_loop:	//	main loop start
	mov	x21, v2.d[0]                            // AES block 4k+2 - mov low
	ext	v11.16b, v11.16b, v11.16b, #8                     // PRE 0
	eor	v3.16b, v7.16b, v3.16b                            // AES block 4k+3 - result
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 0
	mov	x22, v2.d[1]                            // AES block 4k+2 - mov high
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 0
	fmov	d2, x10                               // CTR block 4k+6
	fmov	v2.d[1], x9                               // CTR block 4k+6
	eor	v4.16b, v4.16b, v11.16b                           // PRE 1
	rev	w9, w12                                 // CTR block 4k+7
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 1
	mov	x24, v3.d[1]                            // AES block 4k+3 - mov high
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 1
	mov	x23, v3.d[0]                            // AES block 4k+3 - mov low
	pmull2	v9.1q, v4.2d, v15.2d                       // GHASH block 4k - high
	mov	d8, v4.d[1]                                  // GHASH block 4k - mid
	fmov	d3, x10                               // CTR block 4k+7
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 2
	orr	x9, x11, x9, lsl #32            // CTR block 4k+7
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 0
	fmov	v3.d[1], x9                               // CTR block 4k+7
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 2
	eor	v8.8b, v8.8b, v4.8b                          // GHASH block 4k - mid
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 3
	eor	x22, x22, x14                    // AES block 4k+2 - round N high
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 1
	mov	d10, v17.d[1]                               // GHASH block 4k - mid
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 3
	rev64	v6.16b, v6.16b                                    // GHASH block 4k+2
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 0
	eor	x21, x21, x13                    // AES block 4k+2 - round N low
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 2
	stp	x21, x22, [x2], #16        // AES block 4k+2 - store result
	pmull	v11.1q, v4.1d, v15.1d                       // GHASH block 4k - low
	pmull2	v4.1q, v5.2d, v14.2d                          // GHASH block 4k+1 - high
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 3
	rev64	v7.16b, v7.16b                                    // GHASH block 4k+3
	pmull	v10.1q, v8.1d, v10.1d                      // GHASH block 4k - mid
	eor	x23, x23, x13                    // AES block 4k+3 - round N low
	pmull	v8.1q, v5.1d, v14.1d                          // GHASH block 4k+1 - low
	eor	x24, x24, x14                    // AES block 4k+3 - round N high
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+1 - high
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 4
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 1
	mov	d4, v5.d[1]                                  // GHASH block 4k+1 - mid
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 4
	eor	v11.16b, v11.16b, v8.16b                         // GHASH block 4k+1 - low
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 5
	add	w12, w12, #1                            // CTR block 4k+7
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 2
	mov	d8, v6.d[1]                                  // GHASH block 4k+2 - mid
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 4
	eor	v4.8b, v4.8b, v5.8b                          // GHASH block 4k+1 - mid
	pmull	v5.1q, v6.1d, v13.1d                          // GHASH block 4k+2 - low
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 3
	eor	v8.8b, v8.8b, v6.8b                          // GHASH block 4k+2 - mid
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 5
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 5
	eor	v11.16b, v11.16b, v5.16b                         // GHASH block 4k+2 - low
	pmull	v4.1q, v4.1d, v17.1d                          // GHASH block 4k+1 - mid
	rev	w9, w12                                 // CTR block 4k+8
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 6
	ins	v8.d[1], v8.d[0]                                // GHASH block 4k+2 - mid
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 6
	add	w12, w12, #1                            // CTR block 4k+8
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 4
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 7
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+1 - mid
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 7
	pmull2	v4.1q, v6.2d, v13.2d                          // GHASH block 4k+2 - high
	mov	d6, v7.d[1]                                  // GHASH block 4k+3 - mid
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 5
	pmull2	v8.1q, v8.2d, v16.2d                          // GHASH block 4k+2 - mid
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 8
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+2 - high
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 6
	pmull	v4.1q, v7.1d, v12.1d                          // GHASH block 4k+3 - low
	orr	x9, x11, x9, lsl #32            // CTR block 4k+8
	eor	v10.16b, v10.16b, v8.16b                         // GHASH block 4k+2 - mid
	pmull2	v5.1q, v7.2d, v12.2d                          // GHASH block 4k+3 - high
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	eor	v6.8b, v6.8b, v7.8b                          // GHASH block 4k+3 - mid
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 8
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 6
	eor	v9.16b, v9.16b, v5.16b                         // GHASH block 4k+3 - high
	pmull	v6.1q, v6.1d, v16.1d                          // GHASH block 4k+3 - mid
	movi	v8.8b, #0xc2
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 7
	eor	v11.16b, v11.16b, v4.16b                         // GHASH block 4k+3 - low
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 7
	shl	d8, d8, #56               // mod_constant
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 8
	eor	v10.16b, v10.16b, v6.16b                         // GHASH block 4k+3 - mid
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 8
	b.lt	.Ldec_main_loop_continue                          // branch if AES-128

	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 9
	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 9
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 10
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 10
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 10
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 10
	b.eq	.Ldec_main_loop_continue                          // branch if AES-192

	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 11
	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 11
	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 11
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 11
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 12
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 12
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 12

.Ldec_main_loop_continue:
	pmull	v7.1q, v9.1d, v8.1d            // MODULO - top 64b align with mid
	eor	v6.16b, v11.16b, v9.16b                         // MODULO - karatsuba tidy up
	ldr	q4, [x0, #0]                          // AES block 4k+4 - load ciphertext
	aese	v0.16b, v31.16b                                    // AES block 4k+4 - round N-1
	ext	v9.16b, v9.16b, v9.16b, #8                     // MODULO - other top alignment
	eor	v10.16b, v10.16b, v6.16b                         // MODULO - karatsuba tidy up
	ldr	q5, [x0, #16]                         // AES block 4k+5 - load ciphertext
	eor	v0.16b, v4.16b, v0.16b                            // AES block 4k+4 - result
	stp	x23, x24, [x2], #16        // AES block 4k+3 - store result
	eor	v10.16b, v10.16b, v7.16b                      // MODULO - fold into mid
	ldr	q7, [x0, #48]                         // AES block 4k+7 - load ciphertext
	ldr	q6, [x0, #32]                         // AES block 4k+6 - load ciphertext
	mov	x7, v0.d[1]                            // AES block 4k+4 - mov high
	eor	v10.16b, v10.16b, v9.16b                         // MODULO - fold into mid
	aese	v1.16b, v31.16b                                    // AES block 4k+5 - round N-1
	add	x0, x0, #64                       // AES input_ptr update
	mov	x6, v0.d[0]                            // AES block 4k+4 - mov low
	fmov	d0, x10                               // CTR block 4k+8
	fmov	v0.d[1], x9                               // CTR block 4k+8
	pmull	v8.1q, v10.1d, v8.1d     // MODULO - mid 64b align with low
	eor	v1.16b, v5.16b, v1.16b                            // AES block 4k+5 - result
	rev	w9, w12                                 // CTR block 4k+9
	aese	v2.16b, v31.16b                                    // AES block 4k+6 - round N-1
	orr	x9, x11, x9, lsl #32            // CTR block 4k+9
	cmp	x0, x5                   // .LOOP CONTROL
	add	w12, w12, #1                            // CTR block 4k+9
	eor	x6, x6, x13                    // AES block 4k+4 - round N low
	eor	x7, x7, x14                    // AES block 4k+4 - round N high
	mov	x20, v1.d[1]                            // AES block 4k+5 - mov high
	eor	v2.16b, v6.16b, v2.16b                            // AES block 4k+6 - result
	eor	v11.16b, v11.16b, v8.16b               // MODULO - fold into low
	mov	x19, v1.d[0]                            // AES block 4k+5 - mov low
	fmov	d1, x10                               // CTR block 4k+9
	ext	v10.16b, v10.16b, v10.16b, #8                     // MODULO - other mid alignment
	fmov	v1.d[1], x9                               // CTR block 4k+9
	rev	w9, w12                                 // CTR block 4k+10
	add	w12, w12, #1                            // CTR block 4k+10
	aese	v3.16b, v31.16b                                    // AES block 4k+7 - round N-1
	orr	x9, x11, x9, lsl #32            // CTR block 4k+10
	rev64	v5.16b, v5.16b                                    // GHASH block 4k+5
	eor	x20, x20, x14                    // AES block 4k+5 - round N high
	stp	x6, x7, [x2], #16        // AES block 4k+4 - store result
	eor	x19, x19, x13                    // AES block 4k+5 - round N low
	stp	x19, x20, [x2], #16        // AES block 4k+5 - store result
	rev64	v4.16b, v4.16b                                    // GHASH block 4k+4
	eor	v11.16b, v11.16b, v10.16b                         // MODULO - fold into low
	b.lt	.Ldec_main_loop

.Ldec_prepretail:	//	PREPRETAIL
	ext	v11.16b, v11.16b, v11.16b, #8                     // PRE 0
	mov	x21, v2.d[0]                            // AES block 4k+2 - mov low
	eor	v3.16b, v7.16b, v3.16b                            // AES block 4k+3 - result
	aese	v0.16b, v18.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 0
	mov	x22, v2.d[1]                            // AES block 4k+2 - mov high
	aese	v1.16b, v18.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 0
	fmov	d2, x10                               // CTR block 4k+6
	fmov	v2.d[1], x9                               // CTR block 4k+6
	rev	w9, w12                                 // CTR block 4k+7
	eor	v4.16b, v4.16b, v11.16b                           // PRE 1
	rev64	v6.16b, v6.16b                                    // GHASH block 4k+2
	orr	x9, x11, x9, lsl #32            // CTR block 4k+7
	mov	x23, v3.d[0]                            // AES block 4k+3 - mov low
	aese	v1.16b, v19.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 1
	mov	x24, v3.d[1]                            // AES block 4k+3 - mov high
	pmull	v11.1q, v4.1d, v15.1d                       // GHASH block 4k - low
	mov	d8, v4.d[1]                                  // GHASH block 4k - mid
	fmov	d3, x10                               // CTR block 4k+7
	pmull2	v9.1q, v4.2d, v15.2d                       // GHASH block 4k - high
	fmov	v3.d[1], x9                               // CTR block 4k+7
	aese	v2.16b, v18.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 0
	mov	d10, v17.d[1]                               // GHASH block 4k - mid
	aese	v0.16b, v19.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 1
	eor	v8.8b, v8.8b, v4.8b                          // GHASH block 4k - mid
	pmull2	v4.1q, v5.2d, v14.2d                          // GHASH block 4k+1 - high
	aese	v2.16b, v19.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 1
	rev64	v7.16b, v7.16b                                    // GHASH block 4k+3
	aese	v3.16b, v18.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 0
	pmull	v10.1q, v8.1d, v10.1d                      // GHASH block 4k - mid
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+1 - high
	pmull	v8.1q, v5.1d, v14.1d                          // GHASH block 4k+1 - low
	aese	v3.16b, v19.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 1
	mov	d4, v5.d[1]                                  // GHASH block 4k+1 - mid
	aese	v0.16b, v20.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 2
	aese	v1.16b, v20.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 2
	eor	v11.16b, v11.16b, v8.16b                         // GHASH block 4k+1 - low
	aese	v2.16b, v20.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 2
	aese	v0.16b, v21.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 3
	mov	d8, v6.d[1]                                  // GHASH block 4k+2 - mid
	aese	v3.16b, v20.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 2
	eor	v4.8b, v4.8b, v5.8b                          // GHASH block 4k+1 - mid
	pmull	v5.1q, v6.1d, v13.1d                          // GHASH block 4k+2 - low
	aese	v0.16b, v22.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 4
	aese	v3.16b, v21.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 3
	eor	v8.8b, v8.8b, v6.8b                          // GHASH block 4k+2 - mid
	pmull	v4.1q, v4.1d, v17.1d                          // GHASH block 4k+1 - mid
	aese	v0.16b, v23.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 5
	eor	v11.16b, v11.16b, v5.16b                         // GHASH block 4k+2 - low
	aese	v3.16b, v22.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 4
	pmull2	v5.1q, v7.2d, v12.2d                          // GHASH block 4k+3 - high
	eor	v10.16b, v10.16b, v4.16b                         // GHASH block 4k+1 - mid
	pmull2	v4.1q, v6.2d, v13.2d                          // GHASH block 4k+2 - high
	aese	v3.16b, v23.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 5
	ins	v8.d[1], v8.d[0]                                // GHASH block 4k+2 - mid
	aese	v2.16b, v21.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 3
	aese	v1.16b, v21.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 3
	eor	v9.16b, v9.16b, v4.16b                         // GHASH block 4k+2 - high
	pmull	v4.1q, v7.1d, v12.1d                          // GHASH block 4k+3 - low
	aese	v2.16b, v22.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 4
	mov	d6, v7.d[1]                                  // GHASH block 4k+3 - mid
	aese	v1.16b, v22.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 4
	pmull2	v8.1q, v8.2d, v16.2d                          // GHASH block 4k+2 - mid
	aese	v2.16b, v23.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 5
	eor	v6.8b, v6.8b, v7.8b                          // GHASH block 4k+3 - mid
	aese	v1.16b, v23.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 5
	aese	v3.16b, v24.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 6
	eor	v10.16b, v10.16b, v8.16b                         // GHASH block 4k+2 - mid
	aese	v2.16b, v24.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 6
	aese	v0.16b, v24.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 6
	movi	v8.8b, #0xc2
	aese	v1.16b, v24.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 6
	eor	v11.16b, v11.16b, v4.16b                         // GHASH block 4k+3 - low
	pmull	v6.1q, v6.1d, v16.1d                          // GHASH block 4k+3 - mid
	aese	v3.16b, v25.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 7
	cmp	x17, #12                                      // setup flags for AES-128/192/256 check
	eor	v9.16b, v9.16b, v5.16b                         // GHASH block 4k+3 - high
	aese	v1.16b, v25.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 7
	aese	v0.16b, v25.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 7
	eor	v10.16b, v10.16b, v6.16b                         // GHASH block 4k+3 - mid
	aese	v3.16b, v26.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 8
	aese	v2.16b, v25.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 7
	eor	v6.16b, v11.16b, v9.16b                         // MODULO - karatsuba tidy up
	aese	v1.16b, v26.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 8
	aese	v0.16b, v26.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 8
	shl	d8, d8, #56               // mod_constant
	aese	v2.16b, v26.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 8
	b.lt	.Ldec_finish_prepretail                           // branch if AES-128

	aese	v1.16b, v27.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 9
	aese	v2.16b, v27.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 9
	aese	v3.16b, v27.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 9
	aese	v0.16b, v27.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 9
	aese	v2.16b, v28.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 10
	aese	v3.16b, v28.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 10
	aese	v0.16b, v28.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 10
	aese	v1.16b, v28.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 10
	b.eq	.Ldec_finish_prepretail                           // branch if AES-192

	aese	v2.16b, v29.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 11
	aese	v0.16b, v29.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 11
	aese	v1.16b, v29.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 11
	aese	v2.16b, v30.16b
	aesmc	v2.16b, v2.16b          // AES block 4k+6 - round 12
	aese	v3.16b, v29.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 11
	aese	v1.16b, v30.16b
	aesmc	v1.16b, v1.16b          // AES block 4k+5 - round 12
	aese	v0.16b, v30.16b
	aesmc	v0.16b, v0.16b          // AES block 4k+4 - round 12
	aese	v3.16b, v30.16b
	aesmc	v3.16b, v3.16b          // AES block 4k+7 - round 12

.Ldec_finish_prepretail:
	eor	v10.16b, v10.16b, v6.16b                         // MODULO - karatsuba tidy up
	pmull	v7.1q, v9.1d, v8.1d            // MODULO - top 64b align with mid
	ext	v9.16b, v9.16b, v9.16b, #8                     // MODULO - other top alignment
	eor	v10.16b, v10.16b, v7.16b                      // MODULO - fold into mid
	eor	x22, x22, x14                    // AES block 4k+2 - round N high
	eor	x23, x23, x13                    // AES block 4k+3 - round N low
	eor	v10.16b, v10.16b, v9.16b                         // MODULO - fold into mid
	add	w12, w12, #1                            // CTR block 4k+7
	eor	x21, x21, x13                    // AES block 4k+2 - round N low
	pmull	v8.1q, v10.1d, v8.1d     // MODULO - mid 64b align with low
	eor	x24, x24, x14                    // AES block 4k+3 - round N high
	stp	x21, x22, [x2], #16        // AES block 4k+2 - store result
	ext	v10.16b, v10.16b, v10.16b, #8                     // MODULO - other mid alignment
	stp	x23, x24, [x2], #16        // AES block 4k+3 - store result

	eor	v11.16b, v11.16b, v8.16b               // MODULO - fold into low
	aese	v1.16b, v31.16b                                    // AES block 4k+5 - round N-1
	aese	v0.16b, v31.16b                                    // AES block 4k+4 - round N-1
	aese	v3.16b, v31.16b                                    // AES block 4k+7 - round N-1
	aese	v2.16b, v31.16b                                    // AES block 4k+6 - round N-1
	eor	v11.16b, v11.16b, v10.16b                         // MODULO - fold into low

.Ldec_tail:	//	TAIL
	sub	x5, x4, x0   // main_end_input_ptr is number of bytes left to process
	ld1	{ v5.16b}, [x0], #16                      // AES block 4k+4 - load ciphertext
	eor	v0.16b, v5.16b, v0.16b                            // AES block 4k+4 - result
	mov	x6, v0.d[0]                            // AES block 4k+4 - mov low
	mov	x7, v0.d[1]                            // AES block 4k+4 - mov high
	ext	v8.16b, v11.16b, v11.16b, #8                     // prepare final partial tag
	cmp	x5, #48
	eor	x6, x6, x13                    // AES block 4k+4 - round N low
	eor	x7, x7, x14                    // AES block 4k+4 - round N high
	b.gt	.Ldec_blocks_more_than_3
	sub	w12, w12, #1
	mov	v3.16b, v2.16b
	movi	v10.8b, #0
	movi	v11.8b, #0
	cmp	x5, #32
	movi	v9.8b, #0
	mov	v2.16b, v1.16b
	b.gt	.Ldec_blocks_more_than_2
	sub	w12, w12, #1
	mov	v3.16b, v1.16b
	cmp	x5, #16
	b.gt	.Ldec_blocks_more_than_1
	sub	w12, w12, #1
	b	.Ldec_blocks_less_than_1
.Ldec_blocks_more_than_3:	//	blocks left >  3
	rev64	v4.16b, v5.16b                                   // GHASH final-3 block
	ld1	{ v5.16b}, [x0], #16                     // AES final-2 block - load ciphertext
	stp	x6, x7, [x2], #16       // AES final-3 block  - store result
	mov	d10, v17.d[1]                              // GHASH final-3 block - mid
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	eor	v0.16b, v5.16b, v1.16b                           // AES final-2 block - result
	mov	d22, v4.d[1]                                // GHASH final-3 block - mid
	mov	x6, v0.d[0]                           // AES final-2 block - mov low
	mov	x7, v0.d[1]                           // AES final-2 block - mov high
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-3 block - mid
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	pmull2	v9.1q, v4.2d, v15.2d                      // GHASH final-3 block - high
	pmull	v10.1q, v22.1d, v10.1d                   // GHASH final-3 block - mid
	eor	x6, x6, x13                   // AES final-2 block - round N low
	pmull	v11.1q, v4.1d, v15.1d                      // GHASH final-3 block - low
	eor	x7, x7, x14                   // AES final-2 block - round N high
.Ldec_blocks_more_than_2:	//	blocks left >  2
	rev64	v4.16b, v5.16b                                   // GHASH final-2 block
	ld1	{ v5.16b}, [x0], #16                     // AES final-1 block - load ciphertext
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	stp	x6, x7, [x2], #16       // AES final-2 block  - store result
	eor	v0.16b, v5.16b, v2.16b                           // AES final-1 block - result
	mov	d22, v4.d[1]                                // GHASH final-2 block - mid
	pmull	v21.1q, v4.1d, v14.1d                         // GHASH final-2 block - low
	pmull2	v20.1q, v4.2d, v14.2d                         // GHASH final-2 block - high
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-2 block - mid
	mov	x6, v0.d[0]                           // AES final-1 block - mov low
	mov	x7, v0.d[1]                           // AES final-1 block - mov high
	eor	v11.16b, v11.16b, v21.16b                           // GHASH final-2 block - low
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	pmull	v22.1q, v22.1d, v17.1d                     // GHASH final-2 block - mid
	eor	v9.16b, v9.16b, v20.16b                           // GHASH final-2 block - high
	eor	x6, x6, x13                   // AES final-1 block - round N low
	eor	v10.16b, v10.16b, v22.16b                      // GHASH final-2 block - mid
	eor	x7, x7, x14                   // AES final-1 block - round N high
.Ldec_blocks_more_than_1:	//	blocks left >  1
	stp	x6, x7, [x2], #16       // AES final-1 block  - store result
	rev64	v4.16b, v5.16b                                   // GHASH final-1 block
	ld1	{ v5.16b}, [x0], #16                     // AES final block - load ciphertext
	eor	v4.16b, v4.16b, v8.16b                          // feed in partial tag
	movi	v8.8b, #0                                       // suppress further partial tag feed in
	mov	d22, v4.d[1]                                // GHASH final-1 block - mid
	eor	v0.16b, v5.16b, v3.16b                           // AES final block - result
	pmull2	v20.1q, v4.2d, v13.2d                         // GHASH final-1 block - high
	eor	v22.8b, v22.8b, v4.8b                     // GHASH final-1 block - mid
	pmull	v21.1q, v4.1d, v13.1d                         // GHASH final-1 block - low
	mov	x6, v0.d[0]                           // AES final block - mov low
	ins	v22.d[1], v22.d[0]                           // GHASH final-1 block - mid
	mov	x7, v0.d[1]                           // AES final block - mov high
	pmull2	v22.1q, v22.2d, v16.2d                     // GHASH final-1 block - mid
	eor	x6, x6, x13                   // AES final block - round N low
	eor	v11.16b, v11.16b, v21.16b                           // GHASH final-1 block - low
	eor	v9.16b, v9.16b, v20.16b                           // GHASH final-1 block - high
	eor	v10.16b, v10.16b, v22.16b                      // GHASH final-1 block - mid
	eor	x7, x7, x14                   // AES final block - round N high
.Ldec_blocks_less_than_1:	//	blocks left <= 1
	and	x1, x1, #127                   // bit_length %= 128
	mvn	x14, xzr                                      // rkN_h = 0xffffffffffffffff
	sub	x1, x1, #128                   // bit_length -= 128
	mvn	x13, xzr                                      // rkN_l = 0xffffffffffffffff
	ldp	x4, x5, [x2] // load existing bytes we need to not overwrite
	neg	x1, x1                         // bit_length = 128 - #bits in input (in range [1,128])
	and	x1, x1, #127                   // bit_length %= 128
	lsr	x14, x14, x1                      // rkN_h is mask for top 64b of last block
	cmp	x1, #64
	csel	x9, x13, x14, lt
	csel	x10, x14, xzr, lt
	fmov	d0, x9                                  // ctr0b is mask for last block
	and	x6, x6, x9
	mov	v0.d[1], x10
	bic	x4, x4, x9          // mask out low existing bytes
	rev	w9, w12
	bic	x5, x5, x10      // mask out high existing bytes
	orr	x6, x6, x4
	and	x7, x7, x10
	orr	x7, x7, x5
	and	v5.16b, v5.16b, v0.16b                            // possibly partial last block has zeroes in highest bits
	rev64	v4.16b, v5.16b                                    // GHASH final block
	eor	v4.16b, v4.16b, v8.16b                           // feed in partial tag
	pmull	v21.1q, v4.1d, v12.1d                          // GHASH final block - low
	mov	d8, v4.d[1]                                  // GHASH final block - mid
	eor	v8.8b, v8.8b, v4.8b                          // GHASH final block - mid
	pmull2	v20.1q, v4.2d, v12.2d                          // GHASH final block - high
	pmull	v8.1q, v8.1d, v16.1d                          // GHASH final block - mid
	eor	v9.16b, v9.16b, v20.16b                            // GHASH final block - high
	eor	v11.16b, v11.16b, v21.16b                            // GHASH final block - low
	eor	v10.16b, v10.16b, v8.16b                         // GHASH final block - mid
	movi	v8.8b, #0xc2
	eor	v6.16b, v11.16b, v9.16b                         // MODULO - karatsuba tidy up
	shl	d8, d8, #56               // mod_constant
	eor	v10.16b, v10.16b, v6.16b                         // MODULO - karatsuba tidy up
	pmull	v7.1q, v9.1d, v8.1d            // MODULO - top 64b align with mid
	ext	v9.16b, v9.16b, v9.16b, #8                     // MODULO - other top alignment
	eor	v10.16b, v10.16b, v7.16b                      // MODULO - fold into mid
	eor	v10.16b, v10.16b, v9.16b                         // MODULO - fold into mid
	pmull	v8.1q, v10.1d, v8.1d     // MODULO - mid 64b align with low
	ext	v10.16b, v10.16b, v10.16b, #8                     // MODULO - other mid alignment
	eor	v11.16b, v11.16b, v8.16b               // MODULO - fold into low
	stp	x6, x7, [x2]
	str	w9, [x16, #12]                          // store the updated counter
	eor	v11.16b, v11.16b, v10.16b                         // MODULO - fold into low
	ext	v11.16b, v11.16b, v11.16b, #8
	rev64	v11.16b, v11.16b
	mov	x0, x15
	st1	{ v11.16b }, [x3]
	ldp	x19, x20, [sp, #16]
	ldp	x21, x22, [sp, #32]
	ldp	x23, x24, [sp, #48]
	ldp	d8, d9, [sp, #64]
	ldp	d10, d11, [sp, #80]
	ldp	d12, d13, [sp, #96]
	ldp	d14, d15, [sp, #112]
	ldp	x29, x30, [sp], #128
	AARCH64_VALIDATE_LINK_REGISTER
	ret
.size	aes_gcm_dec_kernel,.-aes_gcm_dec_kernel
#endif
#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_AARCH64) && defined(__ELF__)
