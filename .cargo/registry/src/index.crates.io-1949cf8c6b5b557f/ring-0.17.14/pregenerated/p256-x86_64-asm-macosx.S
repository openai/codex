// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86_64) && defined(__APPLE__)
.text	


.section	__DATA,__const
.p2align	6
L$poly:
.quad	0xffffffffffffffff, 0x00000000ffffffff, 0x0000000000000000, 0xffffffff00000001

L$One:
.long	1,1,1,1,1,1,1,1
L$Two:
.long	2,2,2,2,2,2,2,2
L$Three:
.long	3,3,3,3,3,3,3,3
L$ONE_mont:
.quad	0x0000000000000001, 0xffffffff00000000, 0xffffffffffffffff, 0x00000000fffffffe


L$ord:
.quad	0xf3b9cac2fc632551, 0xbce6faada7179e84, 0xffffffffffffffff, 0xffffffff00000000
L$ordK:
.quad	0xccd1c8aaee00bc4f
.text	



.globl	_ecp_nistz256_neg
.private_extern _ecp_nistz256_neg

.p2align	5
_ecp_nistz256_neg:

_CET_ENDBR
	pushq	%r12

	pushq	%r13

L$neg_body:

	xorq	%r8,%r8
	xorq	%r9,%r9
	xorq	%r10,%r10
	xorq	%r11,%r11
	xorq	%r13,%r13

	subq	0(%rsi),%r8
	sbbq	8(%rsi),%r9
	sbbq	16(%rsi),%r10
	movq	%r8,%rax
	sbbq	24(%rsi),%r11
	leaq	L$poly(%rip),%rsi
	movq	%r9,%rdx
	sbbq	$0,%r13

	addq	0(%rsi),%r8
	movq	%r10,%rcx
	adcq	8(%rsi),%r9
	adcq	16(%rsi),%r10
	movq	%r11,%r12
	adcq	24(%rsi),%r11
	testq	%r13,%r13

	cmovzq	%rax,%r8
	cmovzq	%rdx,%r9
	movq	%r8,0(%rdi)
	cmovzq	%rcx,%r10
	movq	%r9,8(%rdi)
	cmovzq	%r12,%r11
	movq	%r10,16(%rdi)
	movq	%r11,24(%rdi)

	movq	0(%rsp),%r13

	movq	8(%rsp),%r12

	leaq	16(%rsp),%rsp

L$neg_epilogue:
	ret








.globl	_ecp_nistz256_ord_mul_mont_nohw
.private_extern _ecp_nistz256_ord_mul_mont_nohw

.p2align	5
_ecp_nistz256_ord_mul_mont_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$ord_mul_body:

	movq	0(%rdx),%rax
	movq	%rdx,%rbx
	leaq	L$ord(%rip),%r14
	movq	L$ordK(%rip),%r15


	movq	%rax,%rcx
	mulq	0(%rsi)
	movq	%rax,%r8
	movq	%rcx,%rax
	movq	%rdx,%r9

	mulq	8(%rsi)
	addq	%rax,%r9
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%r10

	mulq	16(%rsi)
	addq	%rax,%r10
	movq	%rcx,%rax
	adcq	$0,%rdx

	movq	%r8,%r13
	imulq	%r15,%r8

	movq	%rdx,%r11
	mulq	24(%rsi)
	addq	%rax,%r11
	movq	%r8,%rax
	adcq	$0,%rdx
	movq	%rdx,%r12


	mulq	0(%r14)
	movq	%r8,%rbp
	addq	%rax,%r13
	movq	%r8,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	subq	%r8,%r10
	sbbq	$0,%r8

	mulq	8(%r14)
	addq	%rcx,%r9
	adcq	$0,%rdx
	addq	%rax,%r9
	movq	%rbp,%rax
	adcq	%rdx,%r10
	movq	%rbp,%rdx
	adcq	$0,%r8

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r11
	movq	8(%rbx),%rax
	sbbq	%rdx,%rbp

	addq	%r8,%r11
	adcq	%rbp,%r12
	adcq	$0,%r13


	movq	%rax,%rcx
	mulq	0(%rsi)
	addq	%rax,%r9
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	8(%rsi)
	addq	%rbp,%r10
	adcq	$0,%rdx
	addq	%rax,%r10
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	16(%rsi)
	addq	%rbp,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%rcx,%rax
	adcq	$0,%rdx

	movq	%r9,%rcx
	imulq	%r15,%r9

	movq	%rdx,%rbp
	mulq	24(%rsi)
	addq	%rbp,%r12
	adcq	$0,%rdx
	xorq	%r8,%r8
	addq	%rax,%r12
	movq	%r9,%rax
	adcq	%rdx,%r13
	adcq	$0,%r8


	mulq	0(%r14)
	movq	%r9,%rbp
	addq	%rax,%rcx
	movq	%r9,%rax
	adcq	%rdx,%rcx

	subq	%r9,%r11
	sbbq	$0,%r9

	mulq	8(%r14)
	addq	%rcx,%r10
	adcq	$0,%rdx
	addq	%rax,%r10
	movq	%rbp,%rax
	adcq	%rdx,%r11
	movq	%rbp,%rdx
	adcq	$0,%r9

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r12
	movq	16(%rbx),%rax
	sbbq	%rdx,%rbp

	addq	%r9,%r12
	adcq	%rbp,%r13
	adcq	$0,%r8


	movq	%rax,%rcx
	mulq	0(%rsi)
	addq	%rax,%r10
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	8(%rsi)
	addq	%rbp,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	16(%rsi)
	addq	%rbp,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%rcx,%rax
	adcq	$0,%rdx

	movq	%r10,%rcx
	imulq	%r15,%r10

	movq	%rdx,%rbp
	mulq	24(%rsi)
	addq	%rbp,%r13
	adcq	$0,%rdx
	xorq	%r9,%r9
	addq	%rax,%r13
	movq	%r10,%rax
	adcq	%rdx,%r8
	adcq	$0,%r9


	mulq	0(%r14)
	movq	%r10,%rbp
	addq	%rax,%rcx
	movq	%r10,%rax
	adcq	%rdx,%rcx

	subq	%r10,%r12
	sbbq	$0,%r10

	mulq	8(%r14)
	addq	%rcx,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%rbp,%rax
	adcq	%rdx,%r12
	movq	%rbp,%rdx
	adcq	$0,%r10

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r13
	movq	24(%rbx),%rax
	sbbq	%rdx,%rbp

	addq	%r10,%r13
	adcq	%rbp,%r8
	adcq	$0,%r9


	movq	%rax,%rcx
	mulq	0(%rsi)
	addq	%rax,%r11
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	8(%rsi)
	addq	%rbp,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%rcx,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	16(%rsi)
	addq	%rbp,%r13
	adcq	$0,%rdx
	addq	%rax,%r13
	movq	%rcx,%rax
	adcq	$0,%rdx

	movq	%r11,%rcx
	imulq	%r15,%r11

	movq	%rdx,%rbp
	mulq	24(%rsi)
	addq	%rbp,%r8
	adcq	$0,%rdx
	xorq	%r10,%r10
	addq	%rax,%r8
	movq	%r11,%rax
	adcq	%rdx,%r9
	adcq	$0,%r10


	mulq	0(%r14)
	movq	%r11,%rbp
	addq	%rax,%rcx
	movq	%r11,%rax
	adcq	%rdx,%rcx

	subq	%r11,%r13
	sbbq	$0,%r11

	mulq	8(%r14)
	addq	%rcx,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%rbp,%rax
	adcq	%rdx,%r13
	movq	%rbp,%rdx
	adcq	$0,%r11

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r8
	sbbq	%rdx,%rbp

	addq	%r11,%r8
	adcq	%rbp,%r9
	adcq	$0,%r10


	movq	%r12,%rsi
	subq	0(%r14),%r12
	movq	%r13,%r11
	sbbq	8(%r14),%r13
	movq	%r8,%rcx
	sbbq	16(%r14),%r8
	movq	%r9,%rbp
	sbbq	24(%r14),%r9
	sbbq	$0,%r10

	cmovcq	%rsi,%r12
	cmovcq	%r11,%r13
	cmovcq	%rcx,%r8
	cmovcq	%rbp,%r9

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$ord_mul_epilogue:
	ret









.globl	_ecp_nistz256_ord_sqr_mont_nohw
.private_extern _ecp_nistz256_ord_sqr_mont_nohw

.p2align	5
_ecp_nistz256_ord_sqr_mont_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$ord_sqr_body:

	movq	0(%rsi),%r8
	movq	8(%rsi),%rax
	movq	16(%rsi),%r14
	movq	24(%rsi),%r15
	leaq	L$ord(%rip),%rsi
	movq	%rdx,%rbx
	jmp	L$oop_ord_sqr

.p2align	5
L$oop_ord_sqr:

	movq	%rax,%rbp
	mulq	%r8
	movq	%rax,%r9
.byte	102,72,15,110,205
	movq	%r14,%rax
	movq	%rdx,%r10

	mulq	%r8
	addq	%rax,%r10
	movq	%r15,%rax
.byte	102,73,15,110,214
	adcq	$0,%rdx
	movq	%rdx,%r11

	mulq	%r8
	addq	%rax,%r11
	movq	%r15,%rax
.byte	102,73,15,110,223
	adcq	$0,%rdx
	movq	%rdx,%r12


	mulq	%r14
	movq	%rax,%r13
	movq	%r14,%rax
	movq	%rdx,%r14


	mulq	%rbp
	addq	%rax,%r11
	movq	%r15,%rax
	adcq	$0,%rdx
	movq	%rdx,%r15

	mulq	%rbp
	addq	%rax,%r12
	adcq	$0,%rdx

	addq	%r15,%r12
	adcq	%rdx,%r13
	adcq	$0,%r14


	xorq	%r15,%r15
	movq	%r8,%rax
	addq	%r9,%r9
	adcq	%r10,%r10
	adcq	%r11,%r11
	adcq	%r12,%r12
	adcq	%r13,%r13
	adcq	%r14,%r14
	adcq	$0,%r15


	mulq	%rax
	movq	%rax,%r8
.byte	102,72,15,126,200
	movq	%rdx,%rbp

	mulq	%rax
	addq	%rbp,%r9
	adcq	%rax,%r10
.byte	102,72,15,126,208
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	%rax
	addq	%rbp,%r11
	adcq	%rax,%r12
.byte	102,72,15,126,216
	adcq	$0,%rdx
	movq	%rdx,%rbp

	movq	%r8,%rcx
	imulq	32(%rsi),%r8

	mulq	%rax
	addq	%rbp,%r13
	adcq	%rax,%r14
	movq	0(%rsi),%rax
	adcq	%rdx,%r15


	mulq	%r8
	movq	%r8,%rbp
	addq	%rax,%rcx
	movq	8(%rsi),%rax
	adcq	%rdx,%rcx

	subq	%r8,%r10
	sbbq	$0,%rbp

	mulq	%r8
	addq	%rcx,%r9
	adcq	$0,%rdx
	addq	%rax,%r9
	movq	%r8,%rax
	adcq	%rdx,%r10
	movq	%r8,%rdx
	adcq	$0,%rbp

	movq	%r9,%rcx
	imulq	32(%rsi),%r9

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r11
	movq	0(%rsi),%rax
	sbbq	%rdx,%r8

	addq	%rbp,%r11
	adcq	$0,%r8


	mulq	%r9
	movq	%r9,%rbp
	addq	%rax,%rcx
	movq	8(%rsi),%rax
	adcq	%rdx,%rcx

	subq	%r9,%r11
	sbbq	$0,%rbp

	mulq	%r9
	addq	%rcx,%r10
	adcq	$0,%rdx
	addq	%rax,%r10
	movq	%r9,%rax
	adcq	%rdx,%r11
	movq	%r9,%rdx
	adcq	$0,%rbp

	movq	%r10,%rcx
	imulq	32(%rsi),%r10

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r8
	movq	0(%rsi),%rax
	sbbq	%rdx,%r9

	addq	%rbp,%r8
	adcq	$0,%r9


	mulq	%r10
	movq	%r10,%rbp
	addq	%rax,%rcx
	movq	8(%rsi),%rax
	adcq	%rdx,%rcx

	subq	%r10,%r8
	sbbq	$0,%rbp

	mulq	%r10
	addq	%rcx,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%r10,%rax
	adcq	%rdx,%r8
	movq	%r10,%rdx
	adcq	$0,%rbp

	movq	%r11,%rcx
	imulq	32(%rsi),%r11

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r9
	movq	0(%rsi),%rax
	sbbq	%rdx,%r10

	addq	%rbp,%r9
	adcq	$0,%r10


	mulq	%r11
	movq	%r11,%rbp
	addq	%rax,%rcx
	movq	8(%rsi),%rax
	adcq	%rdx,%rcx

	subq	%r11,%r9
	sbbq	$0,%rbp

	mulq	%r11
	addq	%rcx,%r8
	adcq	$0,%rdx
	addq	%rax,%r8
	movq	%r11,%rax
	adcq	%rdx,%r9
	movq	%r11,%rdx
	adcq	$0,%rbp

	shlq	$32,%rax
	shrq	$32,%rdx
	subq	%rax,%r10
	sbbq	%rdx,%r11

	addq	%rbp,%r10
	adcq	$0,%r11


	xorq	%rdx,%rdx
	addq	%r12,%r8
	adcq	%r13,%r9
	movq	%r8,%r12
	adcq	%r14,%r10
	adcq	%r15,%r11
	movq	%r9,%rax
	adcq	$0,%rdx


	subq	0(%rsi),%r8
	movq	%r10,%r14
	sbbq	8(%rsi),%r9
	sbbq	16(%rsi),%r10
	movq	%r11,%r15
	sbbq	24(%rsi),%r11
	sbbq	$0,%rdx

	cmovcq	%r12,%r8
	cmovncq	%r9,%rax
	cmovncq	%r10,%r14
	cmovncq	%r11,%r15

	decq	%rbx
	jnz	L$oop_ord_sqr

	movq	%r8,0(%rdi)
	movq	%rax,8(%rdi)
	pxor	%xmm1,%xmm1
	movq	%r14,16(%rdi)
	pxor	%xmm2,%xmm2
	movq	%r15,24(%rdi)
	pxor	%xmm3,%xmm3

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$ord_sqr_epilogue:
	ret



.globl	_ecp_nistz256_ord_mul_mont_adx
.private_extern _ecp_nistz256_ord_mul_mont_adx

.p2align	5
_ecp_nistz256_ord_mul_mont_adx:

L$ecp_nistz256_ord_mul_mont_adx:
_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$ord_mulx_body:

	movq	%rdx,%rbx
	movq	0(%rdx),%rdx
	movq	0(%rsi),%r9
	movq	8(%rsi),%r10
	movq	16(%rsi),%r11
	movq	24(%rsi),%r12
	leaq	-128(%rsi),%rsi
	leaq	L$ord-128(%rip),%r14
	movq	L$ordK(%rip),%r15


	mulxq	%r9,%r8,%r9
	mulxq	%r10,%rcx,%r10
	mulxq	%r11,%rbp,%r11
	addq	%rcx,%r9
	mulxq	%r12,%rcx,%r12
	movq	%r8,%rdx
	mulxq	%r15,%rdx,%rax
	adcq	%rbp,%r10
	adcq	%rcx,%r11
	adcq	$0,%r12


	xorq	%r13,%r13
	mulxq	0+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r8
	adoxq	%rbp,%r9

	mulxq	8+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10

	mulxq	16+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	24+128(%r14),%rcx,%rbp
	movq	8(%rbx),%rdx
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12
	adcxq	%r8,%r12
	adoxq	%r8,%r13
	adcq	$0,%r13


	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r9,%rdx
	mulxq	%r15,%rdx,%rax
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	adcxq	%r8,%r13
	adoxq	%r8,%r8
	adcq	$0,%r8


	mulxq	0+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10

	mulxq	8+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	16+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	24+128(%r14),%rcx,%rbp
	movq	16(%rbx),%rdx
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13
	adcxq	%r9,%r13
	adoxq	%r9,%r8
	adcq	$0,%r8


	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r10,%rdx
	mulxq	%r15,%rdx,%rax
	adcxq	%rcx,%r13
	adoxq	%rbp,%r8

	adcxq	%r9,%r8
	adoxq	%r9,%r9
	adcq	$0,%r9


	mulxq	0+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	8+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	16+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	24+128(%r14),%rcx,%rbp
	movq	24(%rbx),%rdx
	adcxq	%rcx,%r13
	adoxq	%rbp,%r8
	adcxq	%r10,%r8
	adoxq	%r10,%r9
	adcq	$0,%r9


	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r13
	adoxq	%rbp,%r8

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r11,%rdx
	mulxq	%r15,%rdx,%rax
	adcxq	%rcx,%r8
	adoxq	%rbp,%r9

	adcxq	%r10,%r9
	adoxq	%r10,%r10
	adcq	$0,%r10


	mulxq	0+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	8+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	16+128(%r14),%rcx,%rbp
	adcxq	%rcx,%r13
	adoxq	%rbp,%r8

	mulxq	24+128(%r14),%rcx,%rbp
	leaq	128(%r14),%r14
	movq	%r12,%rbx
	adcxq	%rcx,%r8
	adoxq	%rbp,%r9
	movq	%r13,%rdx
	adcxq	%r11,%r9
	adoxq	%r11,%r10
	adcq	$0,%r10



	movq	%r8,%rcx
	subq	0(%r14),%r12
	sbbq	8(%r14),%r13
	sbbq	16(%r14),%r8
	movq	%r9,%rbp
	sbbq	24(%r14),%r9
	sbbq	$0,%r10

	cmovcq	%rbx,%r12
	cmovcq	%rdx,%r13
	cmovcq	%rcx,%r8
	cmovcq	%rbp,%r9

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$ord_mulx_epilogue:
	ret



.globl	_ecp_nistz256_ord_sqr_mont_adx
.private_extern _ecp_nistz256_ord_sqr_mont_adx

.p2align	5
_ecp_nistz256_ord_sqr_mont_adx:

_CET_ENDBR
L$ecp_nistz256_ord_sqr_mont_adx:
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$ord_sqrx_body:

	movq	%rdx,%rbx
	movq	0(%rsi),%rdx
	movq	8(%rsi),%r14
	movq	16(%rsi),%r15
	movq	24(%rsi),%r8
	leaq	L$ord(%rip),%rsi
	jmp	L$oop_ord_sqrx

.p2align	5
L$oop_ord_sqrx:
	mulxq	%r14,%r9,%r10
	mulxq	%r15,%rcx,%r11
	movq	%rdx,%rax
.byte	102,73,15,110,206
	mulxq	%r8,%rbp,%r12
	movq	%r14,%rdx
	addq	%rcx,%r10
.byte	102,73,15,110,215
	adcq	%rbp,%r11
	adcq	$0,%r12
	xorq	%r13,%r13

	mulxq	%r15,%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	%r8,%rcx,%rbp
	movq	%r15,%rdx
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13
	adcq	$0,%r13

	mulxq	%r8,%rcx,%r14
	movq	%rax,%rdx
.byte	102,73,15,110,216
	xorq	%r15,%r15
	adcxq	%r9,%r9
	adoxq	%rcx,%r13
	adcxq	%r10,%r10
	adoxq	%r15,%r14


	mulxq	%rdx,%r8,%rbp
.byte	102,72,15,126,202
	adcxq	%r11,%r11
	adoxq	%rbp,%r9
	adcxq	%r12,%r12
	mulxq	%rdx,%rcx,%rax
.byte	102,72,15,126,210
	adcxq	%r13,%r13
	adoxq	%rcx,%r10
	adcxq	%r14,%r14
	mulxq	%rdx,%rcx,%rbp
.byte	0x67
.byte	102,72,15,126,218
	adoxq	%rax,%r11
	adcxq	%r15,%r15
	adoxq	%rcx,%r12
	adoxq	%rbp,%r13
	mulxq	%rdx,%rcx,%rax
	adoxq	%rcx,%r14
	adoxq	%rax,%r15


	movq	%r8,%rdx
	mulxq	32(%rsi),%rdx,%rcx

	xorq	%rax,%rax
	mulxq	0(%rsi),%rcx,%rbp
	adcxq	%rcx,%r8
	adoxq	%rbp,%r9
	mulxq	8(%rsi),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10
	mulxq	16(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11
	mulxq	24(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r8
	adcxq	%rax,%r8


	movq	%r9,%rdx
	mulxq	32(%rsi),%rdx,%rcx

	mulxq	0(%rsi),%rcx,%rbp
	adoxq	%rcx,%r9
	adcxq	%rbp,%r10
	mulxq	8(%rsi),%rcx,%rbp
	adoxq	%rcx,%r10
	adcxq	%rbp,%r11
	mulxq	16(%rsi),%rcx,%rbp
	adoxq	%rcx,%r11
	adcxq	%rbp,%r8
	mulxq	24(%rsi),%rcx,%rbp
	adoxq	%rcx,%r8
	adcxq	%rbp,%r9
	adoxq	%rax,%r9


	movq	%r10,%rdx
	mulxq	32(%rsi),%rdx,%rcx

	mulxq	0(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11
	mulxq	8(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r8
	mulxq	16(%rsi),%rcx,%rbp
	adcxq	%rcx,%r8
	adoxq	%rbp,%r9
	mulxq	24(%rsi),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10
	adcxq	%rax,%r10


	movq	%r11,%rdx
	mulxq	32(%rsi),%rdx,%rcx

	mulxq	0(%rsi),%rcx,%rbp
	adoxq	%rcx,%r11
	adcxq	%rbp,%r8
	mulxq	8(%rsi),%rcx,%rbp
	adoxq	%rcx,%r8
	adcxq	%rbp,%r9
	mulxq	16(%rsi),%rcx,%rbp
	adoxq	%rcx,%r9
	adcxq	%rbp,%r10
	mulxq	24(%rsi),%rcx,%rbp
	adoxq	%rcx,%r10
	adcxq	%rbp,%r11
	adoxq	%rax,%r11


	addq	%r8,%r12
	adcq	%r13,%r9
	movq	%r12,%rdx
	adcq	%r14,%r10
	adcq	%r15,%r11
	movq	%r9,%r14
	adcq	$0,%rax


	subq	0(%rsi),%r12
	movq	%r10,%r15
	sbbq	8(%rsi),%r9
	sbbq	16(%rsi),%r10
	movq	%r11,%r8
	sbbq	24(%rsi),%r11
	sbbq	$0,%rax

	cmovncq	%r12,%rdx
	cmovncq	%r9,%r14
	cmovncq	%r10,%r15
	cmovncq	%r11,%r8

	decq	%rbx
	jnz	L$oop_ord_sqrx

	movq	%rdx,0(%rdi)
	movq	%r14,8(%rdi)
	pxor	%xmm1,%xmm1
	movq	%r15,16(%rdi)
	pxor	%xmm2,%xmm2
	movq	%r8,24(%rdi)
	pxor	%xmm3,%xmm3

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$ord_sqrx_epilogue:
	ret








.globl	_ecp_nistz256_mul_mont_nohw
.private_extern _ecp_nistz256_mul_mont_nohw

.p2align	5
_ecp_nistz256_mul_mont_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$mul_body:
	movq	%rdx,%rbx
	movq	0(%rdx),%rax
	movq	0(%rsi),%r9
	movq	8(%rsi),%r10
	movq	16(%rsi),%r11
	movq	24(%rsi),%r12

	call	__ecp_nistz256_mul_montq

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$mul_epilogue:
	ret




.p2align	5
__ecp_nistz256_mul_montq:



	movq	%rax,%rbp
	mulq	%r9
	movq	L$poly+8(%rip),%r14
	movq	%rax,%r8
	movq	%rbp,%rax
	movq	%rdx,%r9

	mulq	%r10
	movq	L$poly+24(%rip),%r15
	addq	%rax,%r9
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%r10

	mulq	%r11
	addq	%rax,%r10
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%r11

	mulq	%r12
	addq	%rax,%r11
	movq	%r8,%rax
	adcq	$0,%rdx
	xorq	%r13,%r13
	movq	%rdx,%r12










	movq	%r8,%rbp
	shlq	$32,%r8
	mulq	%r15
	shrq	$32,%rbp
	addq	%r8,%r9
	adcq	%rbp,%r10
	adcq	%rax,%r11
	movq	8(%rbx),%rax
	adcq	%rdx,%r12
	adcq	$0,%r13
	xorq	%r8,%r8



	movq	%rax,%rbp
	mulq	0(%rsi)
	addq	%rax,%r9
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	8(%rsi)
	addq	%rcx,%r10
	adcq	$0,%rdx
	addq	%rax,%r10
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	16(%rsi)
	addq	%rcx,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	24(%rsi)
	addq	%rcx,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%r9,%rax
	adcq	%rdx,%r13
	adcq	$0,%r8



	movq	%r9,%rbp
	shlq	$32,%r9
	mulq	%r15
	shrq	$32,%rbp
	addq	%r9,%r10
	adcq	%rbp,%r11
	adcq	%rax,%r12
	movq	16(%rbx),%rax
	adcq	%rdx,%r13
	adcq	$0,%r8
	xorq	%r9,%r9



	movq	%rax,%rbp
	mulq	0(%rsi)
	addq	%rax,%r10
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	8(%rsi)
	addq	%rcx,%r11
	adcq	$0,%rdx
	addq	%rax,%r11
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	16(%rsi)
	addq	%rcx,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	24(%rsi)
	addq	%rcx,%r13
	adcq	$0,%rdx
	addq	%rax,%r13
	movq	%r10,%rax
	adcq	%rdx,%r8
	adcq	$0,%r9



	movq	%r10,%rbp
	shlq	$32,%r10
	mulq	%r15
	shrq	$32,%rbp
	addq	%r10,%r11
	adcq	%rbp,%r12
	adcq	%rax,%r13
	movq	24(%rbx),%rax
	adcq	%rdx,%r8
	adcq	$0,%r9
	xorq	%r10,%r10



	movq	%rax,%rbp
	mulq	0(%rsi)
	addq	%rax,%r11
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	8(%rsi)
	addq	%rcx,%r12
	adcq	$0,%rdx
	addq	%rax,%r12
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	16(%rsi)
	addq	%rcx,%r13
	adcq	$0,%rdx
	addq	%rax,%r13
	movq	%rbp,%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	24(%rsi)
	addq	%rcx,%r8
	adcq	$0,%rdx
	addq	%rax,%r8
	movq	%r11,%rax
	adcq	%rdx,%r9
	adcq	$0,%r10



	movq	%r11,%rbp
	shlq	$32,%r11
	mulq	%r15
	shrq	$32,%rbp
	addq	%r11,%r12
	adcq	%rbp,%r13
	movq	%r12,%rcx
	adcq	%rax,%r8
	adcq	%rdx,%r9
	movq	%r13,%rbp
	adcq	$0,%r10



	subq	$-1,%r12
	movq	%r8,%rbx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%rdx
	sbbq	%r15,%r9
	sbbq	$0,%r10

	cmovcq	%rcx,%r12
	cmovcq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rbx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%rdx,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret










.globl	_ecp_nistz256_sqr_mont_nohw
.private_extern _ecp_nistz256_sqr_mont_nohw

.p2align	5
_ecp_nistz256_sqr_mont_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$sqr_body:
	movq	0(%rsi),%rax
	movq	8(%rsi),%r14
	movq	16(%rsi),%r15
	movq	24(%rsi),%r8

	call	__ecp_nistz256_sqr_montq

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$sqr_epilogue:
	ret




.p2align	5
__ecp_nistz256_sqr_montq:

	movq	%rax,%r13
	mulq	%r14
	movq	%rax,%r9
	movq	%r15,%rax
	movq	%rdx,%r10

	mulq	%r13
	addq	%rax,%r10
	movq	%r8,%rax
	adcq	$0,%rdx
	movq	%rdx,%r11

	mulq	%r13
	addq	%rax,%r11
	movq	%r15,%rax
	adcq	$0,%rdx
	movq	%rdx,%r12


	mulq	%r14
	addq	%rax,%r11
	movq	%r8,%rax
	adcq	$0,%rdx
	movq	%rdx,%rbp

	mulq	%r14
	addq	%rax,%r12
	movq	%r8,%rax
	adcq	$0,%rdx
	addq	%rbp,%r12
	movq	%rdx,%r13
	adcq	$0,%r13


	mulq	%r15
	xorq	%r15,%r15
	addq	%rax,%r13
	movq	0(%rsi),%rax
	movq	%rdx,%r14
	adcq	$0,%r14

	addq	%r9,%r9
	adcq	%r10,%r10
	adcq	%r11,%r11
	adcq	%r12,%r12
	adcq	%r13,%r13
	adcq	%r14,%r14
	adcq	$0,%r15

	mulq	%rax
	movq	%rax,%r8
	movq	8(%rsi),%rax
	movq	%rdx,%rcx

	mulq	%rax
	addq	%rcx,%r9
	adcq	%rax,%r10
	movq	16(%rsi),%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	%rax
	addq	%rcx,%r11
	adcq	%rax,%r12
	movq	24(%rsi),%rax
	adcq	$0,%rdx
	movq	%rdx,%rcx

	mulq	%rax
	addq	%rcx,%r13
	adcq	%rax,%r14
	movq	%r8,%rax
	adcq	%rdx,%r15

	movq	L$poly+8(%rip),%rsi
	movq	L$poly+24(%rip),%rbp




	movq	%r8,%rcx
	shlq	$32,%r8
	mulq	%rbp
	shrq	$32,%rcx
	addq	%r8,%r9
	adcq	%rcx,%r10
	adcq	%rax,%r11
	movq	%r9,%rax
	adcq	$0,%rdx



	movq	%r9,%rcx
	shlq	$32,%r9
	movq	%rdx,%r8
	mulq	%rbp
	shrq	$32,%rcx
	addq	%r9,%r10
	adcq	%rcx,%r11
	adcq	%rax,%r8
	movq	%r10,%rax
	adcq	$0,%rdx



	movq	%r10,%rcx
	shlq	$32,%r10
	movq	%rdx,%r9
	mulq	%rbp
	shrq	$32,%rcx
	addq	%r10,%r11
	adcq	%rcx,%r8
	adcq	%rax,%r9
	movq	%r11,%rax
	adcq	$0,%rdx



	movq	%r11,%rcx
	shlq	$32,%r11
	movq	%rdx,%r10
	mulq	%rbp
	shrq	$32,%rcx
	addq	%r11,%r8
	adcq	%rcx,%r9
	adcq	%rax,%r10
	adcq	$0,%rdx
	xorq	%r11,%r11



	addq	%r8,%r12
	adcq	%r9,%r13
	movq	%r12,%r8
	adcq	%r10,%r14
	adcq	%rdx,%r15
	movq	%r13,%r9
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r14,%r10
	sbbq	%rsi,%r13
	sbbq	$0,%r14
	movq	%r15,%rcx
	sbbq	%rbp,%r15
	sbbq	$0,%r11

	cmovcq	%r8,%r12
	cmovcq	%r9,%r13
	movq	%r12,0(%rdi)
	cmovcq	%r10,%r14
	movq	%r13,8(%rdi)
	cmovcq	%rcx,%r15
	movq	%r14,16(%rdi)
	movq	%r15,24(%rdi)

	ret


.globl	_ecp_nistz256_mul_mont_adx
.private_extern _ecp_nistz256_mul_mont_adx

.p2align	5
_ecp_nistz256_mul_mont_adx:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$mulx_body:
	movq	%rdx,%rbx
	movq	0(%rdx),%rdx
	movq	0(%rsi),%r9
	movq	8(%rsi),%r10
	movq	16(%rsi),%r11
	movq	24(%rsi),%r12
	leaq	-128(%rsi),%rsi

	call	__ecp_nistz256_mul_montx

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$mulx_epilogue:
	ret




.p2align	5
__ecp_nistz256_mul_montx:



	mulxq	%r9,%r8,%r9
	mulxq	%r10,%rcx,%r10
	movq	$32,%r14
	xorq	%r13,%r13
	mulxq	%r11,%rbp,%r11
	movq	L$poly+24(%rip),%r15
	adcq	%rcx,%r9
	mulxq	%r12,%rcx,%r12
	movq	%r8,%rdx
	adcq	%rbp,%r10
	shlxq	%r14,%r8,%rbp
	adcq	%rcx,%r11
	shrxq	%r14,%r8,%rcx
	adcq	$0,%r12



	addq	%rbp,%r9
	adcq	%rcx,%r10

	mulxq	%r15,%rcx,%rbp
	movq	8(%rbx),%rdx
	adcq	%rcx,%r11
	adcq	%rbp,%r12
	adcq	$0,%r13
	xorq	%r8,%r8



	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r9
	adoxq	%rbp,%r10

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r9,%rdx
	adcxq	%rcx,%r12
	shlxq	%r14,%r9,%rcx
	adoxq	%rbp,%r13
	shrxq	%r14,%r9,%rbp

	adcxq	%r8,%r13
	adoxq	%r8,%r8
	adcq	$0,%r8



	addq	%rcx,%r10
	adcq	%rbp,%r11

	mulxq	%r15,%rcx,%rbp
	movq	16(%rbx),%rdx
	adcq	%rcx,%r12
	adcq	%rbp,%r13
	adcq	$0,%r8
	xorq	%r9,%r9



	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r10
	adoxq	%rbp,%r11

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r10,%rdx
	adcxq	%rcx,%r13
	shlxq	%r14,%r10,%rcx
	adoxq	%rbp,%r8
	shrxq	%r14,%r10,%rbp

	adcxq	%r9,%r8
	adoxq	%r9,%r9
	adcq	$0,%r9



	addq	%rcx,%r11
	adcq	%rbp,%r12

	mulxq	%r15,%rcx,%rbp
	movq	24(%rbx),%rdx
	adcq	%rcx,%r13
	adcq	%rbp,%r8
	adcq	$0,%r9
	xorq	%r10,%r10



	mulxq	0+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	8+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13

	mulxq	16+128(%rsi),%rcx,%rbp
	adcxq	%rcx,%r13
	adoxq	%rbp,%r8

	mulxq	24+128(%rsi),%rcx,%rbp
	movq	%r11,%rdx
	adcxq	%rcx,%r8
	shlxq	%r14,%r11,%rcx
	adoxq	%rbp,%r9
	shrxq	%r14,%r11,%rbp

	adcxq	%r10,%r9
	adoxq	%r10,%r10
	adcq	$0,%r10



	addq	%rcx,%r12
	adcq	%rbp,%r13

	mulxq	%r15,%rcx,%rbp
	movq	%r12,%rbx
	movq	L$poly+8(%rip),%r14
	adcq	%rcx,%r8
	movq	%r13,%rdx
	adcq	%rbp,%r9
	adcq	$0,%r10



	xorl	%eax,%eax
	movq	%r8,%rcx
	sbbq	$-1,%r12
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%rbp
	sbbq	%r15,%r9
	sbbq	$0,%r10

	cmovcq	%rbx,%r12
	cmovcq	%rdx,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%rbp,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret



.globl	_ecp_nistz256_sqr_mont_adx
.private_extern _ecp_nistz256_sqr_mont_adx

.p2align	5
_ecp_nistz256_sqr_mont_adx:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

L$sqrx_body:
	movq	0(%rsi),%rdx
	movq	8(%rsi),%r14
	movq	16(%rsi),%r15
	movq	24(%rsi),%r8
	leaq	-128(%rsi),%rsi

	call	__ecp_nistz256_sqr_montx

	movq	0(%rsp),%r15

	movq	8(%rsp),%r14

	movq	16(%rsp),%r13

	movq	24(%rsp),%r12

	movq	32(%rsp),%rbx

	movq	40(%rsp),%rbp

	leaq	48(%rsp),%rsp

L$sqrx_epilogue:
	ret




.p2align	5
__ecp_nistz256_sqr_montx:

	mulxq	%r14,%r9,%r10
	mulxq	%r15,%rcx,%r11
	xorl	%eax,%eax
	adcq	%rcx,%r10
	mulxq	%r8,%rbp,%r12
	movq	%r14,%rdx
	adcq	%rbp,%r11
	adcq	$0,%r12
	xorq	%r13,%r13


	mulxq	%r15,%rcx,%rbp
	adcxq	%rcx,%r11
	adoxq	%rbp,%r12

	mulxq	%r8,%rcx,%rbp
	movq	%r15,%rdx
	adcxq	%rcx,%r12
	adoxq	%rbp,%r13
	adcq	$0,%r13


	mulxq	%r8,%rcx,%r14
	movq	0+128(%rsi),%rdx
	xorq	%r15,%r15
	adcxq	%r9,%r9
	adoxq	%rcx,%r13
	adcxq	%r10,%r10
	adoxq	%r15,%r14

	mulxq	%rdx,%r8,%rbp
	movq	8+128(%rsi),%rdx
	adcxq	%r11,%r11
	adoxq	%rbp,%r9
	adcxq	%r12,%r12
	mulxq	%rdx,%rcx,%rax
	movq	16+128(%rsi),%rdx
	adcxq	%r13,%r13
	adoxq	%rcx,%r10
	adcxq	%r14,%r14
.byte	0x67
	mulxq	%rdx,%rcx,%rbp
	movq	24+128(%rsi),%rdx
	adoxq	%rax,%r11
	adcxq	%r15,%r15
	adoxq	%rcx,%r12
	movq	$32,%rsi
	adoxq	%rbp,%r13
.byte	0x67,0x67
	mulxq	%rdx,%rcx,%rax
	movq	L$poly+24(%rip),%rdx
	adoxq	%rcx,%r14
	shlxq	%rsi,%r8,%rcx
	adoxq	%rax,%r15
	shrxq	%rsi,%r8,%rax
	movq	%rdx,%rbp


	addq	%rcx,%r9
	adcq	%rax,%r10

	mulxq	%r8,%rcx,%r8
	adcq	%rcx,%r11
	shlxq	%rsi,%r9,%rcx
	adcq	$0,%r8
	shrxq	%rsi,%r9,%rax


	addq	%rcx,%r10
	adcq	%rax,%r11

	mulxq	%r9,%rcx,%r9
	adcq	%rcx,%r8
	shlxq	%rsi,%r10,%rcx
	adcq	$0,%r9
	shrxq	%rsi,%r10,%rax


	addq	%rcx,%r11
	adcq	%rax,%r8

	mulxq	%r10,%rcx,%r10
	adcq	%rcx,%r9
	shlxq	%rsi,%r11,%rcx
	adcq	$0,%r10
	shrxq	%rsi,%r11,%rax


	addq	%rcx,%r8
	adcq	%rax,%r9

	mulxq	%r11,%rcx,%r11
	adcq	%rcx,%r10
	adcq	$0,%r11

	xorq	%rdx,%rdx
	addq	%r8,%r12
	movq	L$poly+8(%rip),%rsi
	adcq	%r9,%r13
	movq	%r12,%r8
	adcq	%r10,%r14
	adcq	%r11,%r15
	movq	%r13,%r9
	adcq	$0,%rdx

	subq	$-1,%r12
	movq	%r14,%r10
	sbbq	%rsi,%r13
	sbbq	$0,%r14
	movq	%r15,%r11
	sbbq	%rbp,%r15
	sbbq	$0,%rdx

	cmovcq	%r8,%r12
	cmovcq	%r9,%r13
	movq	%r12,0(%rdi)
	cmovcq	%r10,%r14
	movq	%r13,8(%rdi)
	cmovcq	%r11,%r15
	movq	%r14,16(%rdi)
	movq	%r15,24(%rdi)

	ret




.globl	_ecp_nistz256_select_w5_nohw
.private_extern _ecp_nistz256_select_w5_nohw

.p2align	5
_ecp_nistz256_select_w5_nohw:

_CET_ENDBR
	movdqa	L$One(%rip),%xmm0
	movd	%edx,%xmm1

	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
	pxor	%xmm6,%xmm6
	pxor	%xmm7,%xmm7

	movdqa	%xmm0,%xmm8
	pshufd	$0,%xmm1,%xmm1

	movq	$16,%rax
L$select_loop_sse_w5:

	movdqa	%xmm8,%xmm15
	paddd	%xmm0,%xmm8
	pcmpeqd	%xmm1,%xmm15

	movdqa	0(%rsi),%xmm9
	movdqa	16(%rsi),%xmm10
	movdqa	32(%rsi),%xmm11
	movdqa	48(%rsi),%xmm12
	movdqa	64(%rsi),%xmm13
	movdqa	80(%rsi),%xmm14
	leaq	96(%rsi),%rsi

	pand	%xmm15,%xmm9
	pand	%xmm15,%xmm10
	por	%xmm9,%xmm2
	pand	%xmm15,%xmm11
	por	%xmm10,%xmm3
	pand	%xmm15,%xmm12
	por	%xmm11,%xmm4
	pand	%xmm15,%xmm13
	por	%xmm12,%xmm5
	pand	%xmm15,%xmm14
	por	%xmm13,%xmm6
	por	%xmm14,%xmm7

	decq	%rax
	jnz	L$select_loop_sse_w5

	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)
	movdqu	%xmm4,32(%rdi)
	movdqu	%xmm5,48(%rdi)
	movdqu	%xmm6,64(%rdi)
	movdqu	%xmm7,80(%rdi)
	ret

L$SEH_end_ecp_nistz256_select_w5_nohw:




.globl	_ecp_nistz256_select_w7_nohw
.private_extern _ecp_nistz256_select_w7_nohw

.p2align	5
_ecp_nistz256_select_w7_nohw:

_CET_ENDBR
	movdqa	L$One(%rip),%xmm8
	movd	%edx,%xmm1

	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5

	movdqa	%xmm8,%xmm0
	pshufd	$0,%xmm1,%xmm1
	movq	$64,%rax

L$select_loop_sse_w7:
	movdqa	%xmm8,%xmm15
	paddd	%xmm0,%xmm8
	movdqa	0(%rsi),%xmm9
	movdqa	16(%rsi),%xmm10
	pcmpeqd	%xmm1,%xmm15
	movdqa	32(%rsi),%xmm11
	movdqa	48(%rsi),%xmm12
	leaq	64(%rsi),%rsi

	pand	%xmm15,%xmm9
	pand	%xmm15,%xmm10
	por	%xmm9,%xmm2
	pand	%xmm15,%xmm11
	por	%xmm10,%xmm3
	pand	%xmm15,%xmm12
	por	%xmm11,%xmm4
	prefetcht0	255(%rsi)
	por	%xmm12,%xmm5

	decq	%rax
	jnz	L$select_loop_sse_w7

	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)
	movdqu	%xmm4,32(%rdi)
	movdqu	%xmm5,48(%rdi)
	ret

L$SEH_end_ecp_nistz256_select_w7_nohw:



.globl	_ecp_nistz256_select_w5_avx2
.private_extern _ecp_nistz256_select_w5_avx2

.p2align	5
_ecp_nistz256_select_w5_avx2:

_CET_ENDBR
	vzeroupper
	vmovdqa	L$Two(%rip),%ymm0

	vpxor	%ymm2,%ymm2,%ymm2
	vpxor	%ymm3,%ymm3,%ymm3
	vpxor	%ymm4,%ymm4,%ymm4

	vmovdqa	L$One(%rip),%ymm5
	vmovdqa	L$Two(%rip),%ymm10

	vmovd	%edx,%xmm1
	vpermd	%ymm1,%ymm2,%ymm1

	movq	$8,%rax
L$select_loop_avx2_w5:

	vmovdqa	0(%rsi),%ymm6
	vmovdqa	32(%rsi),%ymm7
	vmovdqa	64(%rsi),%ymm8

	vmovdqa	96(%rsi),%ymm11
	vmovdqa	128(%rsi),%ymm12
	vmovdqa	160(%rsi),%ymm13

	vpcmpeqd	%ymm1,%ymm5,%ymm9
	vpcmpeqd	%ymm1,%ymm10,%ymm14

	vpaddd	%ymm0,%ymm5,%ymm5
	vpaddd	%ymm0,%ymm10,%ymm10
	leaq	192(%rsi),%rsi

	vpand	%ymm9,%ymm6,%ymm6
	vpand	%ymm9,%ymm7,%ymm7
	vpand	%ymm9,%ymm8,%ymm8
	vpand	%ymm14,%ymm11,%ymm11
	vpand	%ymm14,%ymm12,%ymm12
	vpand	%ymm14,%ymm13,%ymm13

	vpxor	%ymm6,%ymm2,%ymm2
	vpxor	%ymm7,%ymm3,%ymm3
	vpxor	%ymm8,%ymm4,%ymm4
	vpxor	%ymm11,%ymm2,%ymm2
	vpxor	%ymm12,%ymm3,%ymm3
	vpxor	%ymm13,%ymm4,%ymm4

	decq	%rax
	jnz	L$select_loop_avx2_w5

	vmovdqu	%ymm2,0(%rdi)
	vmovdqu	%ymm3,32(%rdi)
	vmovdqu	%ymm4,64(%rdi)
	vzeroupper
	ret

L$SEH_end_ecp_nistz256_select_w5_avx2:




.globl	_ecp_nistz256_select_w7_avx2
.private_extern _ecp_nistz256_select_w7_avx2

.p2align	5
_ecp_nistz256_select_w7_avx2:

_CET_ENDBR
	vzeroupper
	vmovdqa	L$Three(%rip),%ymm0

	vpxor	%ymm2,%ymm2,%ymm2
	vpxor	%ymm3,%ymm3,%ymm3

	vmovdqa	L$One(%rip),%ymm4
	vmovdqa	L$Two(%rip),%ymm8
	vmovdqa	L$Three(%rip),%ymm12

	vmovd	%edx,%xmm1
	vpermd	%ymm1,%ymm2,%ymm1


	movq	$21,%rax
L$select_loop_avx2_w7:

	vmovdqa	0(%rsi),%ymm5
	vmovdqa	32(%rsi),%ymm6

	vmovdqa	64(%rsi),%ymm9
	vmovdqa	96(%rsi),%ymm10

	vmovdqa	128(%rsi),%ymm13
	vmovdqa	160(%rsi),%ymm14

	vpcmpeqd	%ymm1,%ymm4,%ymm7
	vpcmpeqd	%ymm1,%ymm8,%ymm11
	vpcmpeqd	%ymm1,%ymm12,%ymm15

	vpaddd	%ymm0,%ymm4,%ymm4
	vpaddd	%ymm0,%ymm8,%ymm8
	vpaddd	%ymm0,%ymm12,%ymm12
	leaq	192(%rsi),%rsi

	vpand	%ymm7,%ymm5,%ymm5
	vpand	%ymm7,%ymm6,%ymm6
	vpand	%ymm11,%ymm9,%ymm9
	vpand	%ymm11,%ymm10,%ymm10
	vpand	%ymm15,%ymm13,%ymm13
	vpand	%ymm15,%ymm14,%ymm14

	vpxor	%ymm5,%ymm2,%ymm2
	vpxor	%ymm6,%ymm3,%ymm3
	vpxor	%ymm9,%ymm2,%ymm2
	vpxor	%ymm10,%ymm3,%ymm3
	vpxor	%ymm13,%ymm2,%ymm2
	vpxor	%ymm14,%ymm3,%ymm3

	decq	%rax
	jnz	L$select_loop_avx2_w7


	vmovdqa	0(%rsi),%ymm5
	vmovdqa	32(%rsi),%ymm6

	vpcmpeqd	%ymm1,%ymm4,%ymm7

	vpand	%ymm7,%ymm5,%ymm5
	vpand	%ymm7,%ymm6,%ymm6

	vpxor	%ymm5,%ymm2,%ymm2
	vpxor	%ymm6,%ymm3,%ymm3

	vmovdqu	%ymm2,0(%rdi)
	vmovdqu	%ymm3,32(%rdi)
	vzeroupper
	ret

L$SEH_end_ecp_nistz256_select_w7_avx2:


.p2align	5
__ecp_nistz256_add_toq:

	xorq	%r11,%r11
	addq	0(%rbx),%r12
	adcq	8(%rbx),%r13
	movq	%r12,%rax
	adcq	16(%rbx),%r8
	adcq	24(%rbx),%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	cmovcq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret




.p2align	5
__ecp_nistz256_sub_fromq:

	subq	0(%rbx),%r12
	sbbq	8(%rbx),%r13
	movq	%r12,%rax
	sbbq	16(%rbx),%r8
	sbbq	24(%rbx),%r9
	movq	%r13,%rbp
	sbbq	%r11,%r11

	addq	$-1,%r12
	movq	%r8,%rcx
	adcq	%r14,%r13
	adcq	$0,%r8
	movq	%r9,%r10
	adcq	%r15,%r9
	testq	%r11,%r11

	cmovzq	%rax,%r12
	cmovzq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovzq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovzq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret




.p2align	5
__ecp_nistz256_subq:

	subq	%r12,%rax
	sbbq	%r13,%rbp
	movq	%rax,%r12
	sbbq	%r8,%rcx
	sbbq	%r9,%r10
	movq	%rbp,%r13
	sbbq	%r11,%r11

	addq	$-1,%rax
	movq	%rcx,%r8
	adcq	%r14,%rbp
	adcq	$0,%rcx
	movq	%r10,%r9
	adcq	%r15,%r10
	testq	%r11,%r11

	cmovnzq	%rax,%r12
	cmovnzq	%rbp,%r13
	cmovnzq	%rcx,%r8
	cmovnzq	%r10,%r9

	ret




.p2align	5
__ecp_nistz256_mul_by_2q:

	xorq	%r11,%r11
	addq	%r12,%r12
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	cmovcq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret


.globl	_ecp_nistz256_point_double_nohw
.private_extern _ecp_nistz256_point_double_nohw

.p2align	5
_ecp_nistz256_point_double_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$160+8,%rsp

L$point_doubleq_body:

L$point_double_shortcutq:
	movdqu	0(%rsi),%xmm0
	movq	%rsi,%rbx
	movdqu	16(%rsi),%xmm1
	movq	32+0(%rsi),%r12
	movq	32+8(%rsi),%r13
	movq	32+16(%rsi),%r8
	movq	32+24(%rsi),%r9
	movq	L$poly+8(%rip),%r14
	movq	L$poly+24(%rip),%r15
	movdqa	%xmm0,96(%rsp)
	movdqa	%xmm1,96+16(%rsp)
	leaq	32(%rdi),%r10
	leaq	64(%rdi),%r11
.byte	102,72,15,110,199
.byte	102,73,15,110,202
.byte	102,73,15,110,211

	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2q

	movq	64+0(%rsi),%rax
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	leaq	64-0(%rsi),%rsi
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	0+0(%rsp),%rax
	movq	8+0(%rsp),%r14
	leaq	0+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	32(%rbx),%rax
	movq	64+0(%rbx),%r9
	movq	64+8(%rbx),%r10
	movq	64+16(%rbx),%r11
	movq	64+24(%rbx),%r12
	leaq	64-0(%rbx),%rsi
	leaq	32(%rbx),%rbx
.byte	102,72,15,126,215
	call	__ecp_nistz256_mul_montq
	call	__ecp_nistz256_mul_by_2q

	movq	96+0(%rsp),%r12
	movq	96+8(%rsp),%r13
	leaq	64(%rsp),%rbx
	movq	96+16(%rsp),%r8
	movq	96+24(%rsp),%r9
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_add_toq

	movq	96+0(%rsp),%r12
	movq	96+8(%rsp),%r13
	leaq	64(%rsp),%rbx
	movq	96+16(%rsp),%r8
	movq	96+24(%rsp),%r9
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	movq	0+0(%rsp),%rax
	movq	8+0(%rsp),%r14
	leaq	0+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
.byte	102,72,15,126,207
	call	__ecp_nistz256_sqr_montq
	xorq	%r9,%r9
	movq	%r12,%rax
	addq	$-1,%r12
	movq	%r13,%r10
	adcq	%rsi,%r13
	movq	%r14,%rcx
	adcq	$0,%r14
	movq	%r15,%r8
	adcq	%rbp,%r15
	adcq	$0,%r9
	xorq	%rsi,%rsi
	testq	$1,%rax

	cmovzq	%rax,%r12
	cmovzq	%r10,%r13
	cmovzq	%rcx,%r14
	cmovzq	%r8,%r15
	cmovzq	%rsi,%r9

	movq	%r13,%rax
	shrq	$1,%r12
	shlq	$63,%rax
	movq	%r14,%r10
	shrq	$1,%r13
	orq	%rax,%r12
	shlq	$63,%r10
	movq	%r15,%rcx
	shrq	$1,%r14
	orq	%r10,%r13
	shlq	$63,%rcx
	movq	%r12,0(%rdi)
	shrq	$1,%r15
	movq	%r13,8(%rdi)
	shlq	$63,%r9
	orq	%rcx,%r14
	orq	%r9,%r15
	movq	%r14,16(%rdi)
	movq	%r15,24(%rdi)
	movq	64(%rsp),%rax
	leaq	64(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2q

	leaq	32(%rsp),%rbx
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_add_toq

	movq	96(%rsp),%rax
	leaq	96(%rsp),%rbx
	movq	0+0(%rsp),%r9
	movq	8+0(%rsp),%r10
	leaq	0+0(%rsp),%rsi
	movq	16+0(%rsp),%r11
	movq	24+0(%rsp),%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2q

	movq	0+32(%rsp),%rax
	movq	8+32(%rsp),%r14
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r15
	movq	24+32(%rsp),%r8
.byte	102,72,15,126,199
	call	__ecp_nistz256_sqr_montq

	leaq	128(%rsp),%rbx
	movq	%r14,%r8
	movq	%r15,%r9
	movq	%rsi,%r14
	movq	%rbp,%r15
	call	__ecp_nistz256_sub_fromq

	movq	0+0(%rsp),%rax
	movq	0+8(%rsp),%rbp
	movq	0+16(%rsp),%rcx
	movq	0+24(%rsp),%r10
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_subq

	movq	32(%rsp),%rax
	leaq	32(%rsp),%rbx
	movq	%r12,%r14
	xorl	%ecx,%ecx
	movq	%r12,0+0(%rsp)
	movq	%r13,%r10
	movq	%r13,0+8(%rsp)
	cmovzq	%r8,%r11
	movq	%r8,0+16(%rsp)
	leaq	0-0(%rsp),%rsi
	cmovzq	%r9,%r12
	movq	%r9,0+24(%rsp)
	movq	%r14,%r9
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

.byte	102,72,15,126,203
.byte	102,72,15,126,207
	call	__ecp_nistz256_sub_fromq

	leaq	160+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$point_doubleq_epilogue:
	ret


.globl	_ecp_nistz256_point_add_nohw
.private_extern _ecp_nistz256_point_add_nohw

.p2align	5
_ecp_nistz256_point_add_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$576+8,%rsp

L$point_addq_body:

	movdqu	0(%rsi),%xmm0
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	movdqu	48(%rsi),%xmm3
	movdqu	64(%rsi),%xmm4
	movdqu	80(%rsi),%xmm5
	movq	%rsi,%rbx
	movq	%rdx,%rsi
	movdqa	%xmm0,384(%rsp)
	movdqa	%xmm1,384+16(%rsp)
	movdqa	%xmm2,416(%rsp)
	movdqa	%xmm3,416+16(%rsp)
	movdqa	%xmm4,448(%rsp)
	movdqa	%xmm5,448+16(%rsp)
	por	%xmm4,%xmm5

	movdqu	0(%rsi),%xmm0
	pshufd	$0xb1,%xmm5,%xmm3
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	por	%xmm3,%xmm5
	movdqu	48(%rsi),%xmm3
	movq	64+0(%rsi),%rax
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	movdqa	%xmm0,480(%rsp)
	pshufd	$0x1e,%xmm5,%xmm4
	movdqa	%xmm1,480+16(%rsp)
	movdqu	64(%rsi),%xmm0
	movdqu	80(%rsi),%xmm1
	movdqa	%xmm2,512(%rsp)
	movdqa	%xmm3,512+16(%rsp)
	por	%xmm4,%xmm5
	pxor	%xmm4,%xmm4
	por	%xmm0,%xmm1
.byte	102,72,15,110,199

	leaq	64-0(%rsi),%rsi
	movq	%rax,544+0(%rsp)
	movq	%r14,544+8(%rsp)
	movq	%r15,544+16(%rsp)
	movq	%r8,544+24(%rsp)
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	pcmpeqd	%xmm4,%xmm5
	pshufd	$0xb1,%xmm1,%xmm4
	por	%xmm1,%xmm4
	pshufd	$0,%xmm5,%xmm5
	pshufd	$0x1e,%xmm4,%xmm3
	por	%xmm3,%xmm4
	pxor	%xmm3,%xmm3
	pcmpeqd	%xmm3,%xmm4
	pshufd	$0,%xmm4,%xmm4
	movq	64+0(%rbx),%rax
	movq	64+8(%rbx),%r14
	movq	64+16(%rbx),%r15
	movq	64+24(%rbx),%r8
.byte	102,72,15,110,203

	leaq	64-0(%rbx),%rsi
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	544(%rsp),%rax
	leaq	544(%rsp),%rbx
	movq	0+96(%rsp),%r9
	movq	8+96(%rsp),%r10
	leaq	0+96(%rsp),%rsi
	movq	16+96(%rsp),%r11
	movq	24+96(%rsp),%r12
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	448(%rsp),%rax
	leaq	448(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	416(%rsp),%rax
	leaq	416(%rsp),%rbx
	movq	0+224(%rsp),%r9
	movq	8+224(%rsp),%r10
	leaq	0+224(%rsp),%rsi
	movq	16+224(%rsp),%r11
	movq	24+224(%rsp),%r12
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	512(%rsp),%rax
	leaq	512(%rsp),%rbx
	movq	0+256(%rsp),%r9
	movq	8+256(%rsp),%r10
	leaq	0+256(%rsp),%rsi
	movq	16+256(%rsp),%r11
	movq	24+256(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	224(%rsp),%rbx
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	orq	%r13,%r12
	movdqa	%xmm4,%xmm2
	orq	%r8,%r12
	orq	%r9,%r12
	por	%xmm5,%xmm2
.byte	102,73,15,110,220

	movq	384(%rsp),%rax
	leaq	384(%rsp),%rbx
	movq	0+96(%rsp),%r9
	movq	8+96(%rsp),%r10
	leaq	0+96(%rsp),%rsi
	movq	16+96(%rsp),%r11
	movq	24+96(%rsp),%r12
	leaq	160(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	480(%rsp),%rax
	leaq	480(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	160(%rsp),%rbx
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	orq	%r13,%r12
	orq	%r8,%r12
	orq	%r9,%r12

.byte	102,73,15,126,208
.byte	102,73,15,126,217
	orq	%r8,%r12
.byte	0x3e
	jnz	L$add_proceedq



	testq	%r9,%r9
	jz	L$add_doubleq






.byte	102,72,15,126,199
	pxor	%xmm0,%xmm0
	movdqu	%xmm0,0(%rdi)
	movdqu	%xmm0,16(%rdi)
	movdqu	%xmm0,32(%rdi)
	movdqu	%xmm0,48(%rdi)
	movdqu	%xmm0,64(%rdi)
	movdqu	%xmm0,80(%rdi)
	jmp	L$add_doneq

.p2align	5
L$add_doubleq:
.byte	102,72,15,126,206
.byte	102,72,15,126,199
	addq	$416,%rsp

	jmp	L$point_double_shortcutq


.p2align	5
L$add_proceedq:
	movq	0+64(%rsp),%rax
	movq	8+64(%rsp),%r14
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r15
	movq	24+64(%rsp),%r8
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	448(%rsp),%rax
	leaq	448(%rsp),%rbx
	movq	0+0(%rsp),%r9
	movq	8+0(%rsp),%r10
	leaq	0+0(%rsp),%rsi
	movq	16+0(%rsp),%r11
	movq	24+0(%rsp),%r12
	leaq	352(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	0+0(%rsp),%rax
	movq	8+0(%rsp),%r14
	leaq	0+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	544(%rsp),%rax
	leaq	544(%rsp),%rbx
	movq	0+352(%rsp),%r9
	movq	8+352(%rsp),%r10
	leaq	0+352(%rsp),%rsi
	movq	16+352(%rsp),%r11
	movq	24+352(%rsp),%r12
	leaq	352(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	0(%rsp),%rax
	leaq	0(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	160(%rsp),%rax
	leaq	160(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_mul_montq




	xorq	%r11,%r11
	addq	%r12,%r12
	leaq	96(%rsp),%rsi
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	movq	0(%rsi),%rax
	cmovcq	%rbp,%r13
	movq	8(%rsi),%rbp
	cmovcq	%rcx,%r8
	movq	16(%rsi),%rcx
	cmovcq	%r10,%r9
	movq	24(%rsi),%r10

	call	__ecp_nistz256_subq

	leaq	128(%rsp),%rbx
	leaq	288(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	movq	192+0(%rsp),%rax
	movq	192+8(%rsp),%rbp
	movq	192+16(%rsp),%rcx
	movq	192+24(%rsp),%r10
	leaq	320(%rsp),%rdi

	call	__ecp_nistz256_subq

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)
	movq	128(%rsp),%rax
	leaq	128(%rsp),%rbx
	movq	0+224(%rsp),%r9
	movq	8+224(%rsp),%r10
	leaq	0+224(%rsp),%rsi
	movq	16+224(%rsp),%r11
	movq	24+224(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	320(%rsp),%rax
	leaq	320(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	320(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	256(%rsp),%rbx
	leaq	320(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

.byte	102,72,15,126,199

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	352(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	352+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	544(%rsp),%xmm2
	pand	544+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	448(%rsp),%xmm2
	pand	448+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,64(%rdi)
	movdqu	%xmm3,80(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	288(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	288+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	480(%rsp),%xmm2
	pand	480+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	384(%rsp),%xmm2
	pand	384+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	320(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	320+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	512(%rsp),%xmm2
	pand	512+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	416(%rsp),%xmm2
	pand	416+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,32(%rdi)
	movdqu	%xmm3,48(%rdi)

L$add_doneq:
	leaq	576+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$point_addq_epilogue:
	ret


.globl	_ecp_nistz256_point_add_affine_nohw
.private_extern _ecp_nistz256_point_add_affine_nohw

.p2align	5
_ecp_nistz256_point_add_affine_nohw:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$480+8,%rsp

L$add_affineq_body:

	movdqu	0(%rsi),%xmm0
	movq	%rdx,%rbx
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	movdqu	48(%rsi),%xmm3
	movdqu	64(%rsi),%xmm4
	movdqu	80(%rsi),%xmm5
	movq	64+0(%rsi),%rax
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	movdqa	%xmm0,320(%rsp)
	movdqa	%xmm1,320+16(%rsp)
	movdqa	%xmm2,352(%rsp)
	movdqa	%xmm3,352+16(%rsp)
	movdqa	%xmm4,384(%rsp)
	movdqa	%xmm5,384+16(%rsp)
	por	%xmm4,%xmm5

	movdqu	0(%rbx),%xmm0
	pshufd	$0xb1,%xmm5,%xmm3
	movdqu	16(%rbx),%xmm1
	movdqu	32(%rbx),%xmm2
	por	%xmm3,%xmm5
	movdqu	48(%rbx),%xmm3
	movdqa	%xmm0,416(%rsp)
	pshufd	$0x1e,%xmm5,%xmm4
	movdqa	%xmm1,416+16(%rsp)
	por	%xmm0,%xmm1
.byte	102,72,15,110,199
	movdqa	%xmm2,448(%rsp)
	movdqa	%xmm3,448+16(%rsp)
	por	%xmm2,%xmm3
	por	%xmm4,%xmm5
	pxor	%xmm4,%xmm4
	por	%xmm1,%xmm3

	leaq	64-0(%rsi),%rsi
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	pcmpeqd	%xmm4,%xmm5
	pshufd	$0xb1,%xmm3,%xmm4
	movq	0(%rbx),%rax

	movq	%r12,%r9
	por	%xmm3,%xmm4
	pshufd	$0,%xmm5,%xmm5
	pshufd	$0x1e,%xmm4,%xmm3
	movq	%r13,%r10
	por	%xmm3,%xmm4
	pxor	%xmm3,%xmm3
	movq	%r14,%r11
	pcmpeqd	%xmm3,%xmm4
	pshufd	$0,%xmm4,%xmm4

	leaq	32-0(%rsp),%rsi
	movq	%r15,%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	320(%rsp),%rbx
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	movq	384(%rsp),%rax
	leaq	384(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	384(%rsp),%rax
	leaq	384(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	288(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	448(%rsp),%rax
	leaq	448(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	0+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	352(%rsp),%rbx
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	movq	0+64(%rsp),%rax
	movq	8+64(%rsp),%r14
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r15
	movq	24+64(%rsp),%r8
	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	0+96(%rsp),%rax
	movq	8+96(%rsp),%r14
	leaq	0+96(%rsp),%rsi
	movq	16+96(%rsp),%r15
	movq	24+96(%rsp),%r8
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_sqr_montq

	movq	128(%rsp),%rax
	leaq	128(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	160(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	320(%rsp),%rax
	leaq	320(%rsp),%rbx
	movq	0+128(%rsp),%r9
	movq	8+128(%rsp),%r10
	leaq	0+128(%rsp),%rsi
	movq	16+128(%rsp),%r11
	movq	24+128(%rsp),%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montq




	xorq	%r11,%r11
	addq	%r12,%r12
	leaq	192(%rsp),%rsi
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	movq	0(%rsi),%rax
	cmovcq	%rbp,%r13
	movq	8(%rsi),%rbp
	cmovcq	%rcx,%r8
	movq	16(%rsi),%rcx
	cmovcq	%r10,%r9
	movq	24(%rsi),%r10

	call	__ecp_nistz256_subq

	leaq	160(%rsp),%rbx
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

	movq	0+0(%rsp),%rax
	movq	0+8(%rsp),%rbp
	movq	0+16(%rsp),%rcx
	movq	0+24(%rsp),%r10
	leaq	64(%rsp),%rdi

	call	__ecp_nistz256_subq

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)
	movq	352(%rsp),%rax
	leaq	352(%rsp),%rbx
	movq	0+160(%rsp),%r9
	movq	8+160(%rsp),%r10
	leaq	0+160(%rsp),%rsi
	movq	16+160(%rsp),%r11
	movq	24+160(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	movq	96(%rsp),%rax
	leaq	96(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	0+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_mul_montq

	leaq	32(%rsp),%rbx
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_sub_fromq

.byte	102,72,15,126,199

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	288(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	288+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	L$ONE_mont(%rip),%xmm2
	pand	L$ONE_mont+16(%rip),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	384(%rsp),%xmm2
	pand	384+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,64(%rdi)
	movdqu	%xmm3,80(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	224(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	224+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	416(%rsp),%xmm2
	pand	416+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	320(%rsp),%xmm2
	pand	320+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	256(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	256+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	448(%rsp),%xmm2
	pand	448+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	352(%rsp),%xmm2
	pand	352+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,32(%rdi)
	movdqu	%xmm3,48(%rdi)

	leaq	480+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$add_affineq_epilogue:
	ret



.p2align	5
__ecp_nistz256_add_tox:

	xorq	%r11,%r11
	adcq	0(%rbx),%r12
	adcq	8(%rbx),%r13
	movq	%r12,%rax
	adcq	16(%rbx),%r8
	adcq	24(%rbx),%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	xorq	%r10,%r10
	sbbq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	cmovcq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret




.p2align	5
__ecp_nistz256_sub_fromx:

	xorq	%r11,%r11
	sbbq	0(%rbx),%r12
	sbbq	8(%rbx),%r13
	movq	%r12,%rax
	sbbq	16(%rbx),%r8
	sbbq	24(%rbx),%r9
	movq	%r13,%rbp
	sbbq	$0,%r11

	xorq	%r10,%r10
	adcq	$-1,%r12
	movq	%r8,%rcx
	adcq	%r14,%r13
	adcq	$0,%r8
	movq	%r9,%r10
	adcq	%r15,%r9

	btq	$0,%r11
	cmovncq	%rax,%r12
	cmovncq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovncq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovncq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret




.p2align	5
__ecp_nistz256_subx:

	xorq	%r11,%r11
	sbbq	%r12,%rax
	sbbq	%r13,%rbp
	movq	%rax,%r12
	sbbq	%r8,%rcx
	sbbq	%r9,%r10
	movq	%rbp,%r13
	sbbq	$0,%r11

	xorq	%r9,%r9
	adcq	$-1,%rax
	movq	%rcx,%r8
	adcq	%r14,%rbp
	adcq	$0,%rcx
	movq	%r10,%r9
	adcq	%r15,%r10

	btq	$0,%r11
	cmovcq	%rax,%r12
	cmovcq	%rbp,%r13
	cmovcq	%rcx,%r8
	cmovcq	%r10,%r9

	ret




.p2align	5
__ecp_nistz256_mul_by_2x:

	xorq	%r11,%r11
	adcq	%r12,%r12
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	xorq	%r10,%r10
	sbbq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	cmovcq	%rbp,%r13
	movq	%r12,0(%rdi)
	cmovcq	%rcx,%r8
	movq	%r13,8(%rdi)
	cmovcq	%r10,%r9
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)

	ret


.globl	_ecp_nistz256_point_double_adx
.private_extern _ecp_nistz256_point_double_adx

.p2align	5
_ecp_nistz256_point_double_adx:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$160+8,%rsp

L$point_doublex_body:

L$point_double_shortcutx:
	movdqu	0(%rsi),%xmm0
	movq	%rsi,%rbx
	movdqu	16(%rsi),%xmm1
	movq	32+0(%rsi),%r12
	movq	32+8(%rsi),%r13
	movq	32+16(%rsi),%r8
	movq	32+24(%rsi),%r9
	movq	L$poly+8(%rip),%r14
	movq	L$poly+24(%rip),%r15
	movdqa	%xmm0,96(%rsp)
	movdqa	%xmm1,96+16(%rsp)
	leaq	32(%rdi),%r10
	leaq	64(%rdi),%r11
.byte	102,72,15,110,199
.byte	102,73,15,110,202
.byte	102,73,15,110,211

	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2x

	movq	64+0(%rsi),%rdx
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	leaq	64-128(%rsi),%rsi
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	0+0(%rsp),%rdx
	movq	8+0(%rsp),%r14
	leaq	-128+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	32(%rbx),%rdx
	movq	64+0(%rbx),%r9
	movq	64+8(%rbx),%r10
	movq	64+16(%rbx),%r11
	movq	64+24(%rbx),%r12
	leaq	64-128(%rbx),%rsi
	leaq	32(%rbx),%rbx
.byte	102,72,15,126,215
	call	__ecp_nistz256_mul_montx
	call	__ecp_nistz256_mul_by_2x

	movq	96+0(%rsp),%r12
	movq	96+8(%rsp),%r13
	leaq	64(%rsp),%rbx
	movq	96+16(%rsp),%r8
	movq	96+24(%rsp),%r9
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_add_tox

	movq	96+0(%rsp),%r12
	movq	96+8(%rsp),%r13
	leaq	64(%rsp),%rbx
	movq	96+16(%rsp),%r8
	movq	96+24(%rsp),%r9
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	movq	0+0(%rsp),%rdx
	movq	8+0(%rsp),%r14
	leaq	-128+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
.byte	102,72,15,126,207
	call	__ecp_nistz256_sqr_montx
	xorq	%r9,%r9
	movq	%r12,%rax
	addq	$-1,%r12
	movq	%r13,%r10
	adcq	%rsi,%r13
	movq	%r14,%rcx
	adcq	$0,%r14
	movq	%r15,%r8
	adcq	%rbp,%r15
	adcq	$0,%r9
	xorq	%rsi,%rsi
	testq	$1,%rax

	cmovzq	%rax,%r12
	cmovzq	%r10,%r13
	cmovzq	%rcx,%r14
	cmovzq	%r8,%r15
	cmovzq	%rsi,%r9

	movq	%r13,%rax
	shrq	$1,%r12
	shlq	$63,%rax
	movq	%r14,%r10
	shrq	$1,%r13
	orq	%rax,%r12
	shlq	$63,%r10
	movq	%r15,%rcx
	shrq	$1,%r14
	orq	%r10,%r13
	shlq	$63,%rcx
	movq	%r12,0(%rdi)
	shrq	$1,%r15
	movq	%r13,8(%rdi)
	shlq	$63,%r9
	orq	%rcx,%r14
	orq	%r9,%r15
	movq	%r14,16(%rdi)
	movq	%r15,24(%rdi)
	movq	64(%rsp),%rdx
	leaq	64(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2x

	leaq	32(%rsp),%rbx
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_add_tox

	movq	96(%rsp),%rdx
	leaq	96(%rsp),%rbx
	movq	0+0(%rsp),%r9
	movq	8+0(%rsp),%r10
	leaq	-128+0(%rsp),%rsi
	movq	16+0(%rsp),%r11
	movq	24+0(%rsp),%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_by_2x

	movq	0+32(%rsp),%rdx
	movq	8+32(%rsp),%r14
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r15
	movq	24+32(%rsp),%r8
.byte	102,72,15,126,199
	call	__ecp_nistz256_sqr_montx

	leaq	128(%rsp),%rbx
	movq	%r14,%r8
	movq	%r15,%r9
	movq	%rsi,%r14
	movq	%rbp,%r15
	call	__ecp_nistz256_sub_fromx

	movq	0+0(%rsp),%rax
	movq	0+8(%rsp),%rbp
	movq	0+16(%rsp),%rcx
	movq	0+24(%rsp),%r10
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_subx

	movq	32(%rsp),%rdx
	leaq	32(%rsp),%rbx
	movq	%r12,%r14
	xorl	%ecx,%ecx
	movq	%r12,0+0(%rsp)
	movq	%r13,%r10
	movq	%r13,0+8(%rsp)
	cmovzq	%r8,%r11
	movq	%r8,0+16(%rsp)
	leaq	0-128(%rsp),%rsi
	cmovzq	%r9,%r12
	movq	%r9,0+24(%rsp)
	movq	%r14,%r9
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

.byte	102,72,15,126,203
.byte	102,72,15,126,207
	call	__ecp_nistz256_sub_fromx

	leaq	160+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$point_doublex_epilogue:
	ret


.globl	_ecp_nistz256_point_add_adx
.private_extern _ecp_nistz256_point_add_adx

.p2align	5
_ecp_nistz256_point_add_adx:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$576+8,%rsp

L$point_addx_body:

	movdqu	0(%rsi),%xmm0
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	movdqu	48(%rsi),%xmm3
	movdqu	64(%rsi),%xmm4
	movdqu	80(%rsi),%xmm5
	movq	%rsi,%rbx
	movq	%rdx,%rsi
	movdqa	%xmm0,384(%rsp)
	movdqa	%xmm1,384+16(%rsp)
	movdqa	%xmm2,416(%rsp)
	movdqa	%xmm3,416+16(%rsp)
	movdqa	%xmm4,448(%rsp)
	movdqa	%xmm5,448+16(%rsp)
	por	%xmm4,%xmm5

	movdqu	0(%rsi),%xmm0
	pshufd	$0xb1,%xmm5,%xmm3
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	por	%xmm3,%xmm5
	movdqu	48(%rsi),%xmm3
	movq	64+0(%rsi),%rdx
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	movdqa	%xmm0,480(%rsp)
	pshufd	$0x1e,%xmm5,%xmm4
	movdqa	%xmm1,480+16(%rsp)
	movdqu	64(%rsi),%xmm0
	movdqu	80(%rsi),%xmm1
	movdqa	%xmm2,512(%rsp)
	movdqa	%xmm3,512+16(%rsp)
	por	%xmm4,%xmm5
	pxor	%xmm4,%xmm4
	por	%xmm0,%xmm1
.byte	102,72,15,110,199

	leaq	64-128(%rsi),%rsi
	movq	%rdx,544+0(%rsp)
	movq	%r14,544+8(%rsp)
	movq	%r15,544+16(%rsp)
	movq	%r8,544+24(%rsp)
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	pcmpeqd	%xmm4,%xmm5
	pshufd	$0xb1,%xmm1,%xmm4
	por	%xmm1,%xmm4
	pshufd	$0,%xmm5,%xmm5
	pshufd	$0x1e,%xmm4,%xmm3
	por	%xmm3,%xmm4
	pxor	%xmm3,%xmm3
	pcmpeqd	%xmm3,%xmm4
	pshufd	$0,%xmm4,%xmm4
	movq	64+0(%rbx),%rdx
	movq	64+8(%rbx),%r14
	movq	64+16(%rbx),%r15
	movq	64+24(%rbx),%r8
.byte	102,72,15,110,203

	leaq	64-128(%rbx),%rsi
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	544(%rsp),%rdx
	leaq	544(%rsp),%rbx
	movq	0+96(%rsp),%r9
	movq	8+96(%rsp),%r10
	leaq	-128+96(%rsp),%rsi
	movq	16+96(%rsp),%r11
	movq	24+96(%rsp),%r12
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	448(%rsp),%rdx
	leaq	448(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	416(%rsp),%rdx
	leaq	416(%rsp),%rbx
	movq	0+224(%rsp),%r9
	movq	8+224(%rsp),%r10
	leaq	-128+224(%rsp),%rsi
	movq	16+224(%rsp),%r11
	movq	24+224(%rsp),%r12
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	512(%rsp),%rdx
	leaq	512(%rsp),%rbx
	movq	0+256(%rsp),%r9
	movq	8+256(%rsp),%r10
	leaq	-128+256(%rsp),%rsi
	movq	16+256(%rsp),%r11
	movq	24+256(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	224(%rsp),%rbx
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	orq	%r13,%r12
	movdqa	%xmm4,%xmm2
	orq	%r8,%r12
	orq	%r9,%r12
	por	%xmm5,%xmm2
.byte	102,73,15,110,220

	movq	384(%rsp),%rdx
	leaq	384(%rsp),%rbx
	movq	0+96(%rsp),%r9
	movq	8+96(%rsp),%r10
	leaq	-128+96(%rsp),%rsi
	movq	16+96(%rsp),%r11
	movq	24+96(%rsp),%r12
	leaq	160(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	480(%rsp),%rdx
	leaq	480(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	160(%rsp),%rbx
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	orq	%r13,%r12
	orq	%r8,%r12
	orq	%r9,%r12

.byte	102,73,15,126,208
.byte	102,73,15,126,217
	orq	%r8,%r12
.byte	0x3e
	jnz	L$add_proceedx



	testq	%r9,%r9
	jz	L$add_doublex






.byte	102,72,15,126,199
	pxor	%xmm0,%xmm0
	movdqu	%xmm0,0(%rdi)
	movdqu	%xmm0,16(%rdi)
	movdqu	%xmm0,32(%rdi)
	movdqu	%xmm0,48(%rdi)
	movdqu	%xmm0,64(%rdi)
	movdqu	%xmm0,80(%rdi)
	jmp	L$add_donex

.p2align	5
L$add_doublex:
.byte	102,72,15,126,206
.byte	102,72,15,126,199
	addq	$416,%rsp

	jmp	L$point_double_shortcutx


.p2align	5
L$add_proceedx:
	movq	0+64(%rsp),%rdx
	movq	8+64(%rsp),%r14
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r15
	movq	24+64(%rsp),%r8
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	448(%rsp),%rdx
	leaq	448(%rsp),%rbx
	movq	0+0(%rsp),%r9
	movq	8+0(%rsp),%r10
	leaq	-128+0(%rsp),%rsi
	movq	16+0(%rsp),%r11
	movq	24+0(%rsp),%r12
	leaq	352(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	0+0(%rsp),%rdx
	movq	8+0(%rsp),%r14
	leaq	-128+0(%rsp),%rsi
	movq	16+0(%rsp),%r15
	movq	24+0(%rsp),%r8
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	544(%rsp),%rdx
	leaq	544(%rsp),%rbx
	movq	0+352(%rsp),%r9
	movq	8+352(%rsp),%r10
	leaq	-128+352(%rsp),%rsi
	movq	16+352(%rsp),%r11
	movq	24+352(%rsp),%r12
	leaq	352(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	0(%rsp),%rdx
	leaq	0(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	160(%rsp),%rdx
	leaq	160(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_mul_montx




	xorq	%r11,%r11
	addq	%r12,%r12
	leaq	96(%rsp),%rsi
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	movq	0(%rsi),%rax
	cmovcq	%rbp,%r13
	movq	8(%rsi),%rbp
	cmovcq	%rcx,%r8
	movq	16(%rsi),%rcx
	cmovcq	%r10,%r9
	movq	24(%rsi),%r10

	call	__ecp_nistz256_subx

	leaq	128(%rsp),%rbx
	leaq	288(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	movq	192+0(%rsp),%rax
	movq	192+8(%rsp),%rbp
	movq	192+16(%rsp),%rcx
	movq	192+24(%rsp),%r10
	leaq	320(%rsp),%rdi

	call	__ecp_nistz256_subx

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)
	movq	128(%rsp),%rdx
	leaq	128(%rsp),%rbx
	movq	0+224(%rsp),%r9
	movq	8+224(%rsp),%r10
	leaq	-128+224(%rsp),%rsi
	movq	16+224(%rsp),%r11
	movq	24+224(%rsp),%r12
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	320(%rsp),%rdx
	leaq	320(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	320(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	256(%rsp),%rbx
	leaq	320(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

.byte	102,72,15,126,199

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	352(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	352+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	544(%rsp),%xmm2
	pand	544+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	448(%rsp),%xmm2
	pand	448+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,64(%rdi)
	movdqu	%xmm3,80(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	288(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	288+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	480(%rsp),%xmm2
	pand	480+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	384(%rsp),%xmm2
	pand	384+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	320(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	320+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	512(%rsp),%xmm2
	pand	512+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	416(%rsp),%xmm2
	pand	416+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,32(%rdi)
	movdqu	%xmm3,48(%rdi)

L$add_donex:
	leaq	576+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$point_addx_epilogue:
	ret


.globl	_ecp_nistz256_point_add_affine_adx
.private_extern _ecp_nistz256_point_add_affine_adx

.p2align	5
_ecp_nistz256_point_add_affine_adx:

_CET_ENDBR
	pushq	%rbp

	pushq	%rbx

	pushq	%r12

	pushq	%r13

	pushq	%r14

	pushq	%r15

	subq	$480+8,%rsp

L$add_affinex_body:

	movdqu	0(%rsi),%xmm0
	movq	%rdx,%rbx
	movdqu	16(%rsi),%xmm1
	movdqu	32(%rsi),%xmm2
	movdqu	48(%rsi),%xmm3
	movdqu	64(%rsi),%xmm4
	movdqu	80(%rsi),%xmm5
	movq	64+0(%rsi),%rdx
	movq	64+8(%rsi),%r14
	movq	64+16(%rsi),%r15
	movq	64+24(%rsi),%r8
	movdqa	%xmm0,320(%rsp)
	movdqa	%xmm1,320+16(%rsp)
	movdqa	%xmm2,352(%rsp)
	movdqa	%xmm3,352+16(%rsp)
	movdqa	%xmm4,384(%rsp)
	movdqa	%xmm5,384+16(%rsp)
	por	%xmm4,%xmm5

	movdqu	0(%rbx),%xmm0
	pshufd	$0xb1,%xmm5,%xmm3
	movdqu	16(%rbx),%xmm1
	movdqu	32(%rbx),%xmm2
	por	%xmm3,%xmm5
	movdqu	48(%rbx),%xmm3
	movdqa	%xmm0,416(%rsp)
	pshufd	$0x1e,%xmm5,%xmm4
	movdqa	%xmm1,416+16(%rsp)
	por	%xmm0,%xmm1
.byte	102,72,15,110,199
	movdqa	%xmm2,448(%rsp)
	movdqa	%xmm3,448+16(%rsp)
	por	%xmm2,%xmm3
	por	%xmm4,%xmm5
	pxor	%xmm4,%xmm4
	por	%xmm1,%xmm3

	leaq	64-128(%rsi),%rsi
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	pcmpeqd	%xmm4,%xmm5
	pshufd	$0xb1,%xmm3,%xmm4
	movq	0(%rbx),%rdx

	movq	%r12,%r9
	por	%xmm3,%xmm4
	pshufd	$0,%xmm5,%xmm5
	pshufd	$0x1e,%xmm4,%xmm3
	movq	%r13,%r10
	por	%xmm3,%xmm4
	pxor	%xmm3,%xmm3
	movq	%r14,%r11
	pcmpeqd	%xmm3,%xmm4
	pshufd	$0,%xmm4,%xmm4

	leaq	32-128(%rsp),%rsi
	movq	%r15,%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	320(%rsp),%rbx
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	movq	384(%rsp),%rdx
	leaq	384(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	384(%rsp),%rdx
	leaq	384(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	288(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	448(%rsp),%rdx
	leaq	448(%rsp),%rbx
	movq	0+32(%rsp),%r9
	movq	8+32(%rsp),%r10
	leaq	-128+32(%rsp),%rsi
	movq	16+32(%rsp),%r11
	movq	24+32(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	352(%rsp),%rbx
	leaq	96(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	movq	0+64(%rsp),%rdx
	movq	8+64(%rsp),%r14
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r15
	movq	24+64(%rsp),%r8
	leaq	128(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	0+96(%rsp),%rdx
	movq	8+96(%rsp),%r14
	leaq	-128+96(%rsp),%rsi
	movq	16+96(%rsp),%r15
	movq	24+96(%rsp),%r8
	leaq	192(%rsp),%rdi
	call	__ecp_nistz256_sqr_montx

	movq	128(%rsp),%rdx
	leaq	128(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	160(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	320(%rsp),%rdx
	leaq	320(%rsp),%rbx
	movq	0+128(%rsp),%r9
	movq	8+128(%rsp),%r10
	leaq	-128+128(%rsp),%rsi
	movq	16+128(%rsp),%r11
	movq	24+128(%rsp),%r12
	leaq	0(%rsp),%rdi
	call	__ecp_nistz256_mul_montx




	xorq	%r11,%r11
	addq	%r12,%r12
	leaq	192(%rsp),%rsi
	adcq	%r13,%r13
	movq	%r12,%rax
	adcq	%r8,%r8
	adcq	%r9,%r9
	movq	%r13,%rbp
	adcq	$0,%r11

	subq	$-1,%r12
	movq	%r8,%rcx
	sbbq	%r14,%r13
	sbbq	$0,%r8
	movq	%r9,%r10
	sbbq	%r15,%r9
	sbbq	$0,%r11

	cmovcq	%rax,%r12
	movq	0(%rsi),%rax
	cmovcq	%rbp,%r13
	movq	8(%rsi),%rbp
	cmovcq	%rcx,%r8
	movq	16(%rsi),%rcx
	cmovcq	%r10,%r9
	movq	24(%rsi),%r10

	call	__ecp_nistz256_subx

	leaq	160(%rsp),%rbx
	leaq	224(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

	movq	0+0(%rsp),%rax
	movq	0+8(%rsp),%rbp
	movq	0+16(%rsp),%rcx
	movq	0+24(%rsp),%r10
	leaq	64(%rsp),%rdi

	call	__ecp_nistz256_subx

	movq	%r12,0(%rdi)
	movq	%r13,8(%rdi)
	movq	%r8,16(%rdi)
	movq	%r9,24(%rdi)
	movq	352(%rsp),%rdx
	leaq	352(%rsp),%rbx
	movq	0+160(%rsp),%r9
	movq	8+160(%rsp),%r10
	leaq	-128+160(%rsp),%rsi
	movq	16+160(%rsp),%r11
	movq	24+160(%rsp),%r12
	leaq	32(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	movq	96(%rsp),%rdx
	leaq	96(%rsp),%rbx
	movq	0+64(%rsp),%r9
	movq	8+64(%rsp),%r10
	leaq	-128+64(%rsp),%rsi
	movq	16+64(%rsp),%r11
	movq	24+64(%rsp),%r12
	leaq	64(%rsp),%rdi
	call	__ecp_nistz256_mul_montx

	leaq	32(%rsp),%rbx
	leaq	256(%rsp),%rdi
	call	__ecp_nistz256_sub_fromx

.byte	102,72,15,126,199

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	288(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	288+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	L$ONE_mont(%rip),%xmm2
	pand	L$ONE_mont+16(%rip),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	384(%rsp),%xmm2
	pand	384+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,64(%rdi)
	movdqu	%xmm3,80(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	224(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	224+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	416(%rsp),%xmm2
	pand	416+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	320(%rsp),%xmm2
	pand	320+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,0(%rdi)
	movdqu	%xmm3,16(%rdi)

	movdqa	%xmm5,%xmm0
	movdqa	%xmm5,%xmm1
	pandn	256(%rsp),%xmm0
	movdqa	%xmm5,%xmm2
	pandn	256+16(%rsp),%xmm1
	movdqa	%xmm5,%xmm3
	pand	448(%rsp),%xmm2
	pand	448+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3

	movdqa	%xmm4,%xmm0
	movdqa	%xmm4,%xmm1
	pandn	%xmm2,%xmm0
	movdqa	%xmm4,%xmm2
	pandn	%xmm3,%xmm1
	movdqa	%xmm4,%xmm3
	pand	352(%rsp),%xmm2
	pand	352+16(%rsp),%xmm3
	por	%xmm0,%xmm2
	por	%xmm1,%xmm3
	movdqu	%xmm2,32(%rdi)
	movdqu	%xmm3,48(%rdi)

	leaq	480+56(%rsp),%rsi

	movq	-48(%rsi),%r15

	movq	-40(%rsi),%r14

	movq	-32(%rsi),%r13

	movq	-24(%rsi),%r12

	movq	-16(%rsi),%rbx

	movq	-8(%rsi),%rbp

	leaq	(%rsi),%rsp

L$add_affinex_epilogue:
	ret


#endif
