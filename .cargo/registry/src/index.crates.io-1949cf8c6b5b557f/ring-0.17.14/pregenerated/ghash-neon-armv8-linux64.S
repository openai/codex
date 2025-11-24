// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_AARCH64) && defined(__ELF__)
.text

.globl	gcm_init_neon
.hidden	gcm_init_neon
.type	gcm_init_neon,%function
.align	4
gcm_init_neon:
	AARCH64_VALID_CALL_TARGET
	// This function is adapted from gcm_init_v8. xC2 is t3.
	ld1	{v17.2d}, [x1]			// load H
	movi	v19.16b, #0xe1
	shl	v19.2d, v19.2d, #57		// 0xc2.0
	ext	v3.16b, v17.16b, v17.16b, #8
	ushr	v18.2d, v19.2d, #63
	dup	v17.4s, v17.s[1]
	ext	v16.16b, v18.16b, v19.16b, #8	// t0=0xc2....01
	ushr	v18.2d, v3.2d, #63
	sshr	v17.4s, v17.4s, #31		// broadcast carry bit
	and	v18.16b, v18.16b, v16.16b
	shl	v3.2d, v3.2d, #1
	ext	v18.16b, v18.16b, v18.16b, #8
	and	v16.16b, v16.16b, v17.16b
	orr	v3.16b, v3.16b, v18.16b	// H<<<=1
	eor	v5.16b, v3.16b, v16.16b	// twisted H
	st1	{v5.2d}, [x0]			// store Htable[0]
	ret
.size	gcm_init_neon,.-gcm_init_neon

.globl	gcm_gmult_neon
.hidden	gcm_gmult_neon
.type	gcm_gmult_neon,%function
.align	4
gcm_gmult_neon:
	AARCH64_VALID_CALL_TARGET
	ld1	{v3.16b}, [x0]		// load Xi
	ld1	{v5.1d}, [x1], #8		// load twisted H
	ld1	{v6.1d}, [x1]
	adrp	x9, .Lmasks		// load constants
	add	x9, x9, :lo12:.Lmasks
	ld1	{v24.2d, v25.2d}, [x9]
	rev64	v3.16b, v3.16b		// byteswap Xi
	ext	v3.16b, v3.16b, v3.16b, #8
	eor	v7.8b, v5.8b, v6.8b	// Karatsuba pre-processing

	mov	x3, #16
	b	.Lgmult_neon
.size	gcm_gmult_neon,.-gcm_gmult_neon

.globl	gcm_ghash_neon
.hidden	gcm_ghash_neon
.type	gcm_ghash_neon,%function
.align	4
gcm_ghash_neon:
	AARCH64_VALID_CALL_TARGET
	ld1	{v0.16b}, [x0]		// load Xi
	ld1	{v5.1d}, [x1], #8		// load twisted H
	ld1	{v6.1d}, [x1]
	adrp	x9, .Lmasks		// load constants
	add	x9, x9, :lo12:.Lmasks
	ld1	{v24.2d, v25.2d}, [x9]
	rev64	v0.16b, v0.16b		// byteswap Xi
	ext	v0.16b, v0.16b, v0.16b, #8
	eor	v7.8b, v5.8b, v6.8b	// Karatsuba pre-processing

.Loop_neon:
	ld1	{v3.16b}, [x2], #16	// load inp
	rev64	v3.16b, v3.16b		// byteswap inp
	ext	v3.16b, v3.16b, v3.16b, #8
	eor	v3.16b, v3.16b, v0.16b	// inp ^= Xi

.Lgmult_neon:
	// Split the input into v3 and v4. (The upper halves are unused,
	// so it is okay to leave them alone.)
	ins	v4.d[0], v3.d[1]
	ext	v16.8b, v5.8b, v5.8b, #1	// A1
	pmull	v16.8h, v16.8b, v3.8b		// F = A1*B
	ext	v0.8b, v3.8b, v3.8b, #1		// B1
	pmull	v0.8h, v5.8b, v0.8b		// E = A*B1
	ext	v17.8b, v5.8b, v5.8b, #2	// A2
	pmull	v17.8h, v17.8b, v3.8b		// H = A2*B
	ext	v19.8b, v3.8b, v3.8b, #2	// B2
	pmull	v19.8h, v5.8b, v19.8b		// G = A*B2
	ext	v18.8b, v5.8b, v5.8b, #3	// A3
	eor	v16.16b, v16.16b, v0.16b	// L = E + F
	pmull	v18.8h, v18.8b, v3.8b		// J = A3*B
	ext	v0.8b, v3.8b, v3.8b, #3		// B3
	eor	v17.16b, v17.16b, v19.16b	// M = G + H
	pmull	v0.8h, v5.8b, v0.8b		// I = A*B3

	// Here we diverge from the 32-bit version. It computes the following
	// (instructions reordered for clarity):
	//
	//     veor	$t0#lo, $t0#lo, $t0#hi	@ t0 = P0 + P1 (L)
	//     vand	$t0#hi, $t0#hi, $k48
	//     veor	$t0#lo, $t0#lo, $t0#hi
	//
	//     veor	$t1#lo, $t1#lo, $t1#hi	@ t1 = P2 + P3 (M)
	//     vand	$t1#hi, $t1#hi, $k32
	//     veor	$t1#lo, $t1#lo, $t1#hi
	//
	//     veor	$t2#lo, $t2#lo, $t2#hi	@ t2 = P4 + P5 (N)
	//     vand	$t2#hi, $t2#hi, $k16
	//     veor	$t2#lo, $t2#lo, $t2#hi
	//
	//     veor	$t3#lo, $t3#lo, $t3#hi	@ t3 = P6 + P7 (K)
	//     vmov.i64	$t3#hi, #0
	//
	// $kN is a mask with the bottom N bits set. AArch64 cannot compute on
	// upper halves of SIMD registers, so we must split each half into
	// separate registers. To compensate, we pair computations up and
	// parallelize.

	ext	v19.8b, v3.8b, v3.8b, #4	// B4
	eor	v18.16b, v18.16b, v0.16b	// N = I + J
	pmull	v19.8h, v5.8b, v19.8b		// K = A*B4

	// This can probably be scheduled more efficiently. For now, we just
	// pair up independent instructions.
	zip1	v20.2d, v16.2d, v17.2d
	zip1	v22.2d, v18.2d, v19.2d
	zip2	v21.2d, v16.2d, v17.2d
	zip2	v23.2d, v18.2d, v19.2d
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	and	v21.16b, v21.16b, v24.16b
	and	v23.16b, v23.16b, v25.16b
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	zip1	v16.2d, v20.2d, v21.2d
	zip1	v18.2d, v22.2d, v23.2d
	zip2	v17.2d, v20.2d, v21.2d
	zip2	v19.2d, v22.2d, v23.2d

	ext	v16.16b, v16.16b, v16.16b, #15	// t0 = t0 << 8
	ext	v17.16b, v17.16b, v17.16b, #14	// t1 = t1 << 16
	pmull	v0.8h, v5.8b, v3.8b		// D = A*B
	ext	v19.16b, v19.16b, v19.16b, #12	// t3 = t3 << 32
	ext	v18.16b, v18.16b, v18.16b, #13	// t2 = t2 << 24
	eor	v16.16b, v16.16b, v17.16b
	eor	v18.16b, v18.16b, v19.16b
	eor	v0.16b, v0.16b, v16.16b
	eor	v0.16b, v0.16b, v18.16b
	eor	v3.8b, v3.8b, v4.8b	// Karatsuba pre-processing
	ext	v16.8b, v7.8b, v7.8b, #1	// A1
	pmull	v16.8h, v16.8b, v3.8b		// F = A1*B
	ext	v1.8b, v3.8b, v3.8b, #1		// B1
	pmull	v1.8h, v7.8b, v1.8b		// E = A*B1
	ext	v17.8b, v7.8b, v7.8b, #2	// A2
	pmull	v17.8h, v17.8b, v3.8b		// H = A2*B
	ext	v19.8b, v3.8b, v3.8b, #2	// B2
	pmull	v19.8h, v7.8b, v19.8b		// G = A*B2
	ext	v18.8b, v7.8b, v7.8b, #3	// A3
	eor	v16.16b, v16.16b, v1.16b	// L = E + F
	pmull	v18.8h, v18.8b, v3.8b		// J = A3*B
	ext	v1.8b, v3.8b, v3.8b, #3		// B3
	eor	v17.16b, v17.16b, v19.16b	// M = G + H
	pmull	v1.8h, v7.8b, v1.8b		// I = A*B3

	// Here we diverge from the 32-bit version. It computes the following
	// (instructions reordered for clarity):
	//
	//     veor	$t0#lo, $t0#lo, $t0#hi	@ t0 = P0 + P1 (L)
	//     vand	$t0#hi, $t0#hi, $k48
	//     veor	$t0#lo, $t0#lo, $t0#hi
	//
	//     veor	$t1#lo, $t1#lo, $t1#hi	@ t1 = P2 + P3 (M)
	//     vand	$t1#hi, $t1#hi, $k32
	//     veor	$t1#lo, $t1#lo, $t1#hi
	//
	//     veor	$t2#lo, $t2#lo, $t2#hi	@ t2 = P4 + P5 (N)
	//     vand	$t2#hi, $t2#hi, $k16
	//     veor	$t2#lo, $t2#lo, $t2#hi
	//
	//     veor	$t3#lo, $t3#lo, $t3#hi	@ t3 = P6 + P7 (K)
	//     vmov.i64	$t3#hi, #0
	//
	// $kN is a mask with the bottom N bits set. AArch64 cannot compute on
	// upper halves of SIMD registers, so we must split each half into
	// separate registers. To compensate, we pair computations up and
	// parallelize.

	ext	v19.8b, v3.8b, v3.8b, #4	// B4
	eor	v18.16b, v18.16b, v1.16b	// N = I + J
	pmull	v19.8h, v7.8b, v19.8b		// K = A*B4

	// This can probably be scheduled more efficiently. For now, we just
	// pair up independent instructions.
	zip1	v20.2d, v16.2d, v17.2d
	zip1	v22.2d, v18.2d, v19.2d
	zip2	v21.2d, v16.2d, v17.2d
	zip2	v23.2d, v18.2d, v19.2d
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	and	v21.16b, v21.16b, v24.16b
	and	v23.16b, v23.16b, v25.16b
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	zip1	v16.2d, v20.2d, v21.2d
	zip1	v18.2d, v22.2d, v23.2d
	zip2	v17.2d, v20.2d, v21.2d
	zip2	v19.2d, v22.2d, v23.2d

	ext	v16.16b, v16.16b, v16.16b, #15	// t0 = t0 << 8
	ext	v17.16b, v17.16b, v17.16b, #14	// t1 = t1 << 16
	pmull	v1.8h, v7.8b, v3.8b		// D = A*B
	ext	v19.16b, v19.16b, v19.16b, #12	// t3 = t3 << 32
	ext	v18.16b, v18.16b, v18.16b, #13	// t2 = t2 << 24
	eor	v16.16b, v16.16b, v17.16b
	eor	v18.16b, v18.16b, v19.16b
	eor	v1.16b, v1.16b, v16.16b
	eor	v1.16b, v1.16b, v18.16b
	ext	v16.8b, v6.8b, v6.8b, #1	// A1
	pmull	v16.8h, v16.8b, v4.8b		// F = A1*B
	ext	v2.8b, v4.8b, v4.8b, #1		// B1
	pmull	v2.8h, v6.8b, v2.8b		// E = A*B1
	ext	v17.8b, v6.8b, v6.8b, #2	// A2
	pmull	v17.8h, v17.8b, v4.8b		// H = A2*B
	ext	v19.8b, v4.8b, v4.8b, #2	// B2
	pmull	v19.8h, v6.8b, v19.8b		// G = A*B2
	ext	v18.8b, v6.8b, v6.8b, #3	// A3
	eor	v16.16b, v16.16b, v2.16b	// L = E + F
	pmull	v18.8h, v18.8b, v4.8b		// J = A3*B
	ext	v2.8b, v4.8b, v4.8b, #3		// B3
	eor	v17.16b, v17.16b, v19.16b	// M = G + H
	pmull	v2.8h, v6.8b, v2.8b		// I = A*B3

	// Here we diverge from the 32-bit version. It computes the following
	// (instructions reordered for clarity):
	//
	//     veor	$t0#lo, $t0#lo, $t0#hi	@ t0 = P0 + P1 (L)
	//     vand	$t0#hi, $t0#hi, $k48
	//     veor	$t0#lo, $t0#lo, $t0#hi
	//
	//     veor	$t1#lo, $t1#lo, $t1#hi	@ t1 = P2 + P3 (M)
	//     vand	$t1#hi, $t1#hi, $k32
	//     veor	$t1#lo, $t1#lo, $t1#hi
	//
	//     veor	$t2#lo, $t2#lo, $t2#hi	@ t2 = P4 + P5 (N)
	//     vand	$t2#hi, $t2#hi, $k16
	//     veor	$t2#lo, $t2#lo, $t2#hi
	//
	//     veor	$t3#lo, $t3#lo, $t3#hi	@ t3 = P6 + P7 (K)
	//     vmov.i64	$t3#hi, #0
	//
	// $kN is a mask with the bottom N bits set. AArch64 cannot compute on
	// upper halves of SIMD registers, so we must split each half into
	// separate registers. To compensate, we pair computations up and
	// parallelize.

	ext	v19.8b, v4.8b, v4.8b, #4	// B4
	eor	v18.16b, v18.16b, v2.16b	// N = I + J
	pmull	v19.8h, v6.8b, v19.8b		// K = A*B4

	// This can probably be scheduled more efficiently. For now, we just
	// pair up independent instructions.
	zip1	v20.2d, v16.2d, v17.2d
	zip1	v22.2d, v18.2d, v19.2d
	zip2	v21.2d, v16.2d, v17.2d
	zip2	v23.2d, v18.2d, v19.2d
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	and	v21.16b, v21.16b, v24.16b
	and	v23.16b, v23.16b, v25.16b
	eor	v20.16b, v20.16b, v21.16b
	eor	v22.16b, v22.16b, v23.16b
	zip1	v16.2d, v20.2d, v21.2d
	zip1	v18.2d, v22.2d, v23.2d
	zip2	v17.2d, v20.2d, v21.2d
	zip2	v19.2d, v22.2d, v23.2d

	ext	v16.16b, v16.16b, v16.16b, #15	// t0 = t0 << 8
	ext	v17.16b, v17.16b, v17.16b, #14	// t1 = t1 << 16
	pmull	v2.8h, v6.8b, v4.8b		// D = A*B
	ext	v19.16b, v19.16b, v19.16b, #12	// t3 = t3 << 32
	ext	v18.16b, v18.16b, v18.16b, #13	// t2 = t2 << 24
	eor	v16.16b, v16.16b, v17.16b
	eor	v18.16b, v18.16b, v19.16b
	eor	v2.16b, v2.16b, v16.16b
	eor	v2.16b, v2.16b, v18.16b
	ext	v16.16b, v0.16b, v2.16b, #8
	eor	v1.16b, v1.16b, v0.16b	// Karatsuba post-processing
	eor	v1.16b, v1.16b, v2.16b
	eor	v1.16b, v1.16b, v16.16b	// Xm overlaps Xh.lo and Xl.hi
	ins	v0.d[1], v1.d[0]		// Xh|Xl - 256-bit result
	// This is a no-op due to the ins instruction below.
	// ins	v2.d[0], v1.d[1]

	// equivalent of reduction_avx from ghash-x86_64.pl
	shl	v17.2d, v0.2d, #57		// 1st phase
	shl	v18.2d, v0.2d, #62
	eor	v18.16b, v18.16b, v17.16b	//
	shl	v17.2d, v0.2d, #63
	eor	v18.16b, v18.16b, v17.16b	//
	// Note Xm contains {Xl.d[1], Xh.d[0]}.
	eor	v18.16b, v18.16b, v1.16b
	ins	v0.d[1], v18.d[0]		// Xl.d[1] ^= t2.d[0]
	ins	v2.d[0], v18.d[1]		// Xh.d[0] ^= t2.d[1]

	ushr	v18.2d, v0.2d, #1		// 2nd phase
	eor	v2.16b, v2.16b,v0.16b
	eor	v0.16b, v0.16b,v18.16b	//
	ushr	v18.2d, v18.2d, #6
	ushr	v0.2d, v0.2d, #1		//
	eor	v0.16b, v0.16b, v2.16b	//
	eor	v0.16b, v0.16b, v18.16b	//

	subs	x3, x3, #16
	bne	.Loop_neon

	rev64	v0.16b, v0.16b		// byteswap Xi and write
	ext	v0.16b, v0.16b, v0.16b, #8
	st1	{v0.16b}, [x0]

	ret
.size	gcm_ghash_neon,.-gcm_ghash_neon

.section	.rodata
.align	4
.Lmasks:
.quad	0x0000ffffffffffff	// k48
.quad	0x00000000ffffffff	// k32
.quad	0x000000000000ffff	// k16
.quad	0x0000000000000000	// k0
.byte	71,72,65,83,72,32,102,111,114,32,65,82,77,118,56,44,32,100,101,114,105,118,101,100,32,102,114,111,109,32,65,82,77,118,52,32,118,101,114,115,105,111,110,32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
.align	2
.align	2
#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_AARCH64) && defined(__ELF__)
