// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_AARCH64) && defined(__APPLE__)
#if __ARM_MAX_ARCH__>=7
.text

.globl	_gcm_init_clmul
.private_extern	_gcm_init_clmul

.align	4
_gcm_init_clmul:
	AARCH64_VALID_CALL_TARGET
	ld1	{v17.2d},[x1]		//load input H
	movi	v19.16b,#0xe1
	shl	v19.2d,v19.2d,#57		//0xc2.0
	ext	v3.16b,v17.16b,v17.16b,#8
	ushr	v18.2d,v19.2d,#63
	dup	v17.4s,v17.s[1]
	ext	v16.16b,v18.16b,v19.16b,#8		//t0=0xc2....01
	ushr	v18.2d,v3.2d,#63
	sshr	v17.4s,v17.4s,#31		//broadcast carry bit
	and	v18.16b,v18.16b,v16.16b
	shl	v3.2d,v3.2d,#1
	ext	v18.16b,v18.16b,v18.16b,#8
	and	v16.16b,v16.16b,v17.16b
	orr	v3.16b,v3.16b,v18.16b		//H<<<=1
	eor	v20.16b,v3.16b,v16.16b		//twisted H
	st1	{v20.2d},[x0],#16		//store Htable[0]

	//calculate H^2
	ext	v16.16b,v20.16b,v20.16b,#8		//Karatsuba pre-processing
	pmull	v0.1q,v20.1d,v20.1d
	eor	v16.16b,v16.16b,v20.16b
	pmull2	v2.1q,v20.2d,v20.2d
	pmull	v1.1q,v16.1d,v16.1d

	ext	v17.16b,v0.16b,v2.16b,#8		//Karatsuba post-processing
	eor	v18.16b,v0.16b,v2.16b
	eor	v1.16b,v1.16b,v17.16b
	eor	v1.16b,v1.16b,v18.16b
	pmull	v18.1q,v0.1d,v19.1d		//1st phase

	ins	v2.d[0],v1.d[1]
	ins	v1.d[1],v0.d[0]
	eor	v0.16b,v1.16b,v18.16b

	ext	v18.16b,v0.16b,v0.16b,#8		//2nd phase
	pmull	v0.1q,v0.1d,v19.1d
	eor	v18.16b,v18.16b,v2.16b
	eor	v22.16b,v0.16b,v18.16b

	ext	v17.16b,v22.16b,v22.16b,#8		//Karatsuba pre-processing
	eor	v17.16b,v17.16b,v22.16b
	ext	v21.16b,v16.16b,v17.16b,#8		//pack Karatsuba pre-processed
	st1	{v21.2d,v22.2d},[x0],#32	//store Htable[1..2]
	//calculate H^3 and H^4
	pmull	v0.1q,v20.1d, v22.1d
	pmull	v5.1q,v22.1d,v22.1d
	pmull2	v2.1q,v20.2d, v22.2d
	pmull2	v7.1q,v22.2d,v22.2d
	pmull	v1.1q,v16.1d,v17.1d
	pmull	v6.1q,v17.1d,v17.1d

	ext	v16.16b,v0.16b,v2.16b,#8		//Karatsuba post-processing
	ext	v17.16b,v5.16b,v7.16b,#8
	eor	v18.16b,v0.16b,v2.16b
	eor	v1.16b,v1.16b,v16.16b
	eor	v4.16b,v5.16b,v7.16b
	eor	v6.16b,v6.16b,v17.16b
	eor	v1.16b,v1.16b,v18.16b
	pmull	v18.1q,v0.1d,v19.1d		//1st phase
	eor	v6.16b,v6.16b,v4.16b
	pmull	v4.1q,v5.1d,v19.1d

	ins	v2.d[0],v1.d[1]
	ins	v7.d[0],v6.d[1]
	ins	v1.d[1],v0.d[0]
	ins	v6.d[1],v5.d[0]
	eor	v0.16b,v1.16b,v18.16b
	eor	v5.16b,v6.16b,v4.16b

	ext	v18.16b,v0.16b,v0.16b,#8		//2nd phase
	ext	v4.16b,v5.16b,v5.16b,#8
	pmull	v0.1q,v0.1d,v19.1d
	pmull	v5.1q,v5.1d,v19.1d
	eor	v18.16b,v18.16b,v2.16b
	eor	v4.16b,v4.16b,v7.16b
	eor	v20.16b, v0.16b,v18.16b		//H^3
	eor	v22.16b,v5.16b,v4.16b		//H^4

	ext	v16.16b,v20.16b, v20.16b,#8		//Karatsuba pre-processing
	ext	v17.16b,v22.16b,v22.16b,#8
	eor	v16.16b,v16.16b,v20.16b
	eor	v17.16b,v17.16b,v22.16b
	ext	v21.16b,v16.16b,v17.16b,#8		//pack Karatsuba pre-processed
	st1	{v20.2d,v21.2d,v22.2d},[x0]		//store Htable[3..5]
	ret

.globl	_gcm_gmult_clmul
.private_extern	_gcm_gmult_clmul

.align	4
_gcm_gmult_clmul:
	AARCH64_VALID_CALL_TARGET
	ld1	{v17.2d},[x0]		//load Xi
	movi	v19.16b,#0xe1
	ld1	{v20.2d,v21.2d},[x1]	//load twisted H, ...
	shl	v19.2d,v19.2d,#57
#ifndef __AARCH64EB__
	rev64	v17.16b,v17.16b
#endif
	ext	v3.16b,v17.16b,v17.16b,#8

	pmull	v0.1q,v20.1d,v3.1d		//H.lo·Xi.lo
	eor	v17.16b,v17.16b,v3.16b		//Karatsuba pre-processing
	pmull2	v2.1q,v20.2d,v3.2d		//H.hi·Xi.hi
	pmull	v1.1q,v21.1d,v17.1d		//(H.lo+H.hi)·(Xi.lo+Xi.hi)

	ext	v17.16b,v0.16b,v2.16b,#8		//Karatsuba post-processing
	eor	v18.16b,v0.16b,v2.16b
	eor	v1.16b,v1.16b,v17.16b
	eor	v1.16b,v1.16b,v18.16b
	pmull	v18.1q,v0.1d,v19.1d		//1st phase of reduction

	ins	v2.d[0],v1.d[1]
	ins	v1.d[1],v0.d[0]
	eor	v0.16b,v1.16b,v18.16b

	ext	v18.16b,v0.16b,v0.16b,#8		//2nd phase of reduction
	pmull	v0.1q,v0.1d,v19.1d
	eor	v18.16b,v18.16b,v2.16b
	eor	v0.16b,v0.16b,v18.16b

#ifndef __AARCH64EB__
	rev64	v0.16b,v0.16b
#endif
	ext	v0.16b,v0.16b,v0.16b,#8
	st1	{v0.2d},[x0]		//write out Xi

	ret

.byte	71,72,65,83,72,32,102,111,114,32,65,82,77,118,56,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
.align	2
.align	2
#endif
#endif  // !OPENSSL_NO_ASM && defined(OPENSSL_AARCH64) && defined(__APPLE__)
