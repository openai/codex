// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_ARM) && defined(__ELF__)
@ Silence ARMv8 deprecated IT instruction warnings. This file is used by both
@ ARMv7 and ARMv8 processors and does not use ARMv8 instructions. (ARMv8 PMULL
@ instructions are in aesv8-armx.pl.)
.arch	armv7-a

.text
#if defined(__thumb2__) || defined(__clang__)
.syntax	unified
#define ldrplb  ldrbpl
#define ldrneb  ldrbne
#endif
#if defined(__thumb2__)
.thumb
#else
.code	32
#endif
#if __ARM_MAX_ARCH__>=7
.arch	armv7-a
.fpu	neon

.globl	gcm_init_neon
.hidden	gcm_init_neon
.type	gcm_init_neon,%function
.align	4
gcm_init_neon:
	vld1.64	d7,[r1]!		@ load H
	vmov.i8	q8,#0xe1
	vld1.64	d6,[r1]
	vshl.i64	d17,#57
	vshr.u64	d16,#63		@ t0=0xc2....01
	vdup.8	q9,d7[7]
	vshr.u64	d26,d6,#63
	vshr.s8	q9,#7			@ broadcast carry bit
	vshl.i64	q3,q3,#1
	vand	q8,q8,q9
	vorr	d7,d26		@ H<<<=1
	veor	q3,q3,q8		@ twisted H
	vstmia	r0,{q3}

	bx	lr					@ bx lr
.size	gcm_init_neon,.-gcm_init_neon

.globl	gcm_gmult_neon
.hidden	gcm_gmult_neon
.type	gcm_gmult_neon,%function
.align	4
gcm_gmult_neon:
	vld1.64	d7,[r0]!		@ load Xi
	vld1.64	d6,[r0]!
	vmov.i64	d29,#0x0000ffffffffffff
	vldmia	r1,{d26,d27}	@ load twisted H
	vmov.i64	d30,#0x00000000ffffffff
#ifdef __ARMEL__
	vrev64.8	q3,q3
#endif
	vmov.i64	d31,#0x000000000000ffff
	veor	d28,d26,d27		@ Karatsuba pre-processing
	mov	r3,#16
	b	.Lgmult_neon
.size	gcm_gmult_neon,.-gcm_gmult_neon

.globl	gcm_ghash_neon
.hidden	gcm_ghash_neon
.type	gcm_ghash_neon,%function
.align	4
gcm_ghash_neon:
	vld1.64	d1,[r0]!		@ load Xi
	vld1.64	d0,[r0]!
	vmov.i64	d29,#0x0000ffffffffffff
	vldmia	r1,{d26,d27}	@ load twisted H
	vmov.i64	d30,#0x00000000ffffffff
#ifdef __ARMEL__
	vrev64.8	q0,q0
#endif
	vmov.i64	d31,#0x000000000000ffff
	veor	d28,d26,d27		@ Karatsuba pre-processing

.Loop_neon:
	vld1.64	d7,[r2]!		@ load inp
	vld1.64	d6,[r2]!
#ifdef __ARMEL__
	vrev64.8	q3,q3
#endif
	veor	q3,q0			@ inp^=Xi
.Lgmult_neon:
	vext.8	d16, d26, d26, #1	@ A1
	vmull.p8	q8, d16, d6		@ F = A1*B
	vext.8	d0, d6, d6, #1	@ B1
	vmull.p8	q0, d26, d0		@ E = A*B1
	vext.8	d18, d26, d26, #2	@ A2
	vmull.p8	q9, d18, d6		@ H = A2*B
	vext.8	d22, d6, d6, #2	@ B2
	vmull.p8	q11, d26, d22		@ G = A*B2
	vext.8	d20, d26, d26, #3	@ A3
	veor	q8, q8, q0		@ L = E + F
	vmull.p8	q10, d20, d6		@ J = A3*B
	vext.8	d0, d6, d6, #3	@ B3
	veor	q9, q9, q11		@ M = G + H
	vmull.p8	q0, d26, d0		@ I = A*B3
	veor	d16, d16, d17	@ t0 = (L) (P0 + P1) << 8
	vand	d17, d17, d29
	vext.8	d22, d6, d6, #4	@ B4
	veor	d18, d18, d19	@ t1 = (M) (P2 + P3) << 16
	vand	d19, d19, d30
	vmull.p8	q11, d26, d22		@ K = A*B4
	veor	q10, q10, q0		@ N = I + J
	veor	d16, d16, d17
	veor	d18, d18, d19
	veor	d20, d20, d21	@ t2 = (N) (P4 + P5) << 24
	vand	d21, d21, d31
	vext.8	q8, q8, q8, #15
	veor	d22, d22, d23	@ t3 = (K) (P6 + P7) << 32
	vmov.i64	d23, #0
	vext.8	q9, q9, q9, #14
	veor	d20, d20, d21
	vmull.p8	q0, d26, d6		@ D = A*B
	vext.8	q11, q11, q11, #12
	vext.8	q10, q10, q10, #13
	veor	q8, q8, q9
	veor	q10, q10, q11
	veor	q0, q0, q8
	veor	q0, q0, q10
	veor	d6,d6,d7	@ Karatsuba pre-processing
	vext.8	d16, d28, d28, #1	@ A1
	vmull.p8	q8, d16, d6		@ F = A1*B
	vext.8	d2, d6, d6, #1	@ B1
	vmull.p8	q1, d28, d2		@ E = A*B1
	vext.8	d18, d28, d28, #2	@ A2
	vmull.p8	q9, d18, d6		@ H = A2*B
	vext.8	d22, d6, d6, #2	@ B2
	vmull.p8	q11, d28, d22		@ G = A*B2
	vext.8	d20, d28, d28, #3	@ A3
	veor	q8, q8, q1		@ L = E + F
	vmull.p8	q10, d20, d6		@ J = A3*B
	vext.8	d2, d6, d6, #3	@ B3
	veor	q9, q9, q11		@ M = G + H
	vmull.p8	q1, d28, d2		@ I = A*B3
	veor	d16, d16, d17	@ t0 = (L) (P0 + P1) << 8
	vand	d17, d17, d29
	vext.8	d22, d6, d6, #4	@ B4
	veor	d18, d18, d19	@ t1 = (M) (P2 + P3) << 16
	vand	d19, d19, d30
	vmull.p8	q11, d28, d22		@ K = A*B4
	veor	q10, q10, q1		@ N = I + J
	veor	d16, d16, d17
	veor	d18, d18, d19
	veor	d20, d20, d21	@ t2 = (N) (P4 + P5) << 24
	vand	d21, d21, d31
	vext.8	q8, q8, q8, #15
	veor	d22, d22, d23	@ t3 = (K) (P6 + P7) << 32
	vmov.i64	d23, #0
	vext.8	q9, q9, q9, #14
	veor	d20, d20, d21
	vmull.p8	q1, d28, d6		@ D = A*B
	vext.8	q11, q11, q11, #12
	vext.8	q10, q10, q10, #13
	veor	q8, q8, q9
	veor	q10, q10, q11
	veor	q1, q1, q8
	veor	q1, q1, q10
	vext.8	d16, d27, d27, #1	@ A1
	vmull.p8	q8, d16, d7		@ F = A1*B
	vext.8	d4, d7, d7, #1	@ B1
	vmull.p8	q2, d27, d4		@ E = A*B1
	vext.8	d18, d27, d27, #2	@ A2
	vmull.p8	q9, d18, d7		@ H = A2*B
	vext.8	d22, d7, d7, #2	@ B2
	vmull.p8	q11, d27, d22		@ G = A*B2
	vext.8	d20, d27, d27, #3	@ A3
	veor	q8, q8, q2		@ L = E + F
	vmull.p8	q10, d20, d7		@ J = A3*B
	vext.8	d4, d7, d7, #3	@ B3
	veor	q9, q9, q11		@ M = G + H
	vmull.p8	q2, d27, d4		@ I = A*B3
	veor	d16, d16, d17	@ t0 = (L) (P0 + P1) << 8
	vand	d17, d17, d29
	vext.8	d22, d7, d7, #4	@ B4
	veor	d18, d18, d19	@ t1 = (M) (P2 + P3) << 16
	vand	d19, d19, d30
	vmull.p8	q11, d27, d22		@ K = A*B4
	veor	q10, q10, q2		@ N = I + J
	veor	d16, d16, d17
	veor	d18, d18, d19
	veor	d20, d20, d21	@ t2 = (N) (P4 + P5) << 24
	vand	d21, d21, d31
	vext.8	q8, q8, q8, #15
	veor	d22, d22, d23	@ t3 = (K) (P6 + P7) << 32
	vmov.i64	d23, #0
	vext.8	q9, q9, q9, #14
	veor	d20, d20, d21
	vmull.p8	q2, d27, d7		@ D = A*B
	vext.8	q11, q11, q11, #12
	vext.8	q10, q10, q10, #13
	veor	q8, q8, q9
	veor	q10, q10, q11
	veor	q2, q2, q8
	veor	q2, q2, q10
	veor	q1,q1,q0		@ Karatsuba post-processing
	veor	q1,q1,q2
	veor	d1,d1,d2
	veor	d4,d4,d3	@ Xh|Xl - 256-bit result

	@ equivalent of reduction_avx from ghash-x86_64.pl
	vshl.i64	q9,q0,#57		@ 1st phase
	vshl.i64	q10,q0,#62
	veor	q10,q10,q9		@
	vshl.i64	q9,q0,#63
	veor	q10, q10, q9		@
	veor	d1,d1,d20	@
	veor	d4,d4,d21

	vshr.u64	q10,q0,#1		@ 2nd phase
	veor	q2,q2,q0
	veor	q0,q0,q10		@
	vshr.u64	q10,q10,#6
	vshr.u64	q0,q0,#1		@
	veor	q0,q0,q2		@
	veor	q0,q0,q10		@

	subs	r3,#16
	bne	.Loop_neon

#ifdef __ARMEL__
	vrev64.8	q0,q0
#endif
	sub	r0,#16
	vst1.64	d1,[r0]!		@ write out Xi
	vst1.64	d0,[r0]

	bx	lr					@ bx lr
.size	gcm_ghash_neon,.-gcm_ghash_neon
#endif
.byte	71,72,65,83,72,32,102,111,114,32,65,82,77,118,52,47,78,69,79,78,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
.align	2
.align	2
#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_ARM) && defined(__ELF__)
