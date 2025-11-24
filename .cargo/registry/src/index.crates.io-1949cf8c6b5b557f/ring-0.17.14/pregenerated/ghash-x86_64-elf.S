// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86_64) && defined(__ELF__)
.text	
.globl	gcm_init_clmul
.hidden gcm_init_clmul
.type	gcm_init_clmul,@function
.align	16
gcm_init_clmul:
.cfi_startproc	

_CET_ENDBR
.L_init_clmul:
	movdqu	(%rsi),%xmm2
	pshufd	$78,%xmm2,%xmm2


	pshufd	$255,%xmm2,%xmm4
	movdqa	%xmm2,%xmm3
	psllq	$1,%xmm2
	pxor	%xmm5,%xmm5
	psrlq	$63,%xmm3
	pcmpgtd	%xmm4,%xmm5
	pslldq	$8,%xmm3
	por	%xmm3,%xmm2


	pand	.L0x1c2_polynomial(%rip),%xmm5
	pxor	%xmm5,%xmm2


	pshufd	$78,%xmm2,%xmm6
	movdqa	%xmm2,%xmm0
	pxor	%xmm2,%xmm6
	movdqa	%xmm0,%xmm1
	pshufd	$78,%xmm0,%xmm3
	pxor	%xmm0,%xmm3
.byte	102,15,58,68,194,0
.byte	102,15,58,68,202,17
.byte	102,15,58,68,222,0
	pxor	%xmm0,%xmm3
	pxor	%xmm1,%xmm3

	movdqa	%xmm3,%xmm4
	psrldq	$8,%xmm3
	pslldq	$8,%xmm4
	pxor	%xmm3,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	pshufd	$78,%xmm2,%xmm3
	pshufd	$78,%xmm0,%xmm4
	pxor	%xmm2,%xmm3
	movdqu	%xmm2,0(%rdi)
	pxor	%xmm0,%xmm4
	movdqu	%xmm0,16(%rdi)
.byte	102,15,58,15,227,8
	movdqu	%xmm4,32(%rdi)
	movdqa	%xmm0,%xmm1
	pshufd	$78,%xmm0,%xmm3
	pxor	%xmm0,%xmm3
.byte	102,15,58,68,194,0
.byte	102,15,58,68,202,17
.byte	102,15,58,68,222,0
	pxor	%xmm0,%xmm3
	pxor	%xmm1,%xmm3

	movdqa	%xmm3,%xmm4
	psrldq	$8,%xmm3
	pslldq	$8,%xmm4
	pxor	%xmm3,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	movdqa	%xmm0,%xmm5
	movdqa	%xmm0,%xmm1
	pshufd	$78,%xmm0,%xmm3
	pxor	%xmm0,%xmm3
.byte	102,15,58,68,194,0
.byte	102,15,58,68,202,17
.byte	102,15,58,68,222,0
	pxor	%xmm0,%xmm3
	pxor	%xmm1,%xmm3

	movdqa	%xmm3,%xmm4
	psrldq	$8,%xmm3
	pslldq	$8,%xmm4
	pxor	%xmm3,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	pshufd	$78,%xmm5,%xmm3
	pshufd	$78,%xmm0,%xmm4
	pxor	%xmm5,%xmm3
	movdqu	%xmm5,48(%rdi)
	pxor	%xmm0,%xmm4
	movdqu	%xmm0,64(%rdi)
.byte	102,15,58,15,227,8
	movdqu	%xmm4,80(%rdi)
	ret
.cfi_endproc	

.size	gcm_init_clmul,.-gcm_init_clmul
.globl	gcm_ghash_clmul
.hidden gcm_ghash_clmul
.type	gcm_ghash_clmul,@function
.align	32
gcm_ghash_clmul:
.cfi_startproc	

_CET_ENDBR
.L_ghash_clmul:
	movdqa	.Lbswap_mask(%rip),%xmm10

	movdqu	(%rdi),%xmm0
	movdqu	(%rsi),%xmm2
	movdqu	32(%rsi),%xmm7
.byte	102,65,15,56,0,194

	subq	$0x10,%rcx
	jz	.Lodd_tail

	movdqu	16(%rsi),%xmm6
	cmpq	$0x30,%rcx
	jb	.Lskip4x

	subq	$0x30,%rcx
	movq	$0xA040608020C0E000,%rax
	movdqu	48(%rsi),%xmm14
	movdqu	64(%rsi),%xmm15




	movdqu	48(%rdx),%xmm3
	movdqu	32(%rdx),%xmm11
.byte	102,65,15,56,0,218
.byte	102,69,15,56,0,218
	movdqa	%xmm3,%xmm5
	pshufd	$78,%xmm3,%xmm4
	pxor	%xmm3,%xmm4
.byte	102,15,58,68,218,0
.byte	102,15,58,68,234,17
.byte	102,15,58,68,231,0

	movdqa	%xmm11,%xmm13
	pshufd	$78,%xmm11,%xmm12
	pxor	%xmm11,%xmm12
.byte	102,68,15,58,68,222,0
.byte	102,68,15,58,68,238,17
.byte	102,68,15,58,68,231,16
	xorps	%xmm11,%xmm3
	xorps	%xmm13,%xmm5
	movups	80(%rsi),%xmm7
	xorps	%xmm12,%xmm4

	movdqu	16(%rdx),%xmm11
	movdqu	0(%rdx),%xmm8
.byte	102,69,15,56,0,218
.byte	102,69,15,56,0,194
	movdqa	%xmm11,%xmm13
	pshufd	$78,%xmm11,%xmm12
	pxor	%xmm8,%xmm0
	pxor	%xmm11,%xmm12
.byte	102,69,15,58,68,222,0
	movdqa	%xmm0,%xmm1
	pshufd	$78,%xmm0,%xmm8
	pxor	%xmm0,%xmm8
.byte	102,69,15,58,68,238,17
.byte	102,68,15,58,68,231,0
	xorps	%xmm11,%xmm3
	xorps	%xmm13,%xmm5

	leaq	64(%rdx),%rdx
	subq	$0x40,%rcx
	jc	.Ltail4x

	jmp	.Lmod4_loop
.align	32
.Lmod4_loop:
.byte	102,65,15,58,68,199,0
	xorps	%xmm12,%xmm4
	movdqu	48(%rdx),%xmm11
.byte	102,69,15,56,0,218
.byte	102,65,15,58,68,207,17
	xorps	%xmm3,%xmm0
	movdqu	32(%rdx),%xmm3
	movdqa	%xmm11,%xmm13
.byte	102,68,15,58,68,199,16
	pshufd	$78,%xmm11,%xmm12
	xorps	%xmm5,%xmm1
	pxor	%xmm11,%xmm12
.byte	102,65,15,56,0,218
	movups	32(%rsi),%xmm7
	xorps	%xmm4,%xmm8
.byte	102,68,15,58,68,218,0
	pshufd	$78,%xmm3,%xmm4

	pxor	%xmm0,%xmm8
	movdqa	%xmm3,%xmm5
	pxor	%xmm1,%xmm8
	pxor	%xmm3,%xmm4
	movdqa	%xmm8,%xmm9
.byte	102,68,15,58,68,234,17
	pslldq	$8,%xmm8
	psrldq	$8,%xmm9
	pxor	%xmm8,%xmm0
	movdqa	.L7_mask(%rip),%xmm8
	pxor	%xmm9,%xmm1
.byte	102,76,15,110,200

	pand	%xmm0,%xmm8
.byte	102,69,15,56,0,200
	pxor	%xmm0,%xmm9
.byte	102,68,15,58,68,231,0
	psllq	$57,%xmm9
	movdqa	%xmm9,%xmm8
	pslldq	$8,%xmm9
.byte	102,15,58,68,222,0
	psrldq	$8,%xmm8
	pxor	%xmm9,%xmm0
	pxor	%xmm8,%xmm1
	movdqu	0(%rdx),%xmm8

	movdqa	%xmm0,%xmm9
	psrlq	$1,%xmm0
.byte	102,15,58,68,238,17
	xorps	%xmm11,%xmm3
	movdqu	16(%rdx),%xmm11
.byte	102,69,15,56,0,218
.byte	102,15,58,68,231,16
	xorps	%xmm13,%xmm5
	movups	80(%rsi),%xmm7
.byte	102,69,15,56,0,194
	pxor	%xmm9,%xmm1
	pxor	%xmm0,%xmm9
	psrlq	$5,%xmm0

	movdqa	%xmm11,%xmm13
	pxor	%xmm12,%xmm4
	pshufd	$78,%xmm11,%xmm12
	pxor	%xmm9,%xmm0
	pxor	%xmm8,%xmm1
	pxor	%xmm11,%xmm12
.byte	102,69,15,58,68,222,0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	movdqa	%xmm0,%xmm1
.byte	102,69,15,58,68,238,17
	xorps	%xmm11,%xmm3
	pshufd	$78,%xmm0,%xmm8
	pxor	%xmm0,%xmm8

.byte	102,68,15,58,68,231,0
	xorps	%xmm13,%xmm5

	leaq	64(%rdx),%rdx
	subq	$0x40,%rcx
	jnc	.Lmod4_loop

.Ltail4x:
.byte	102,65,15,58,68,199,0
.byte	102,65,15,58,68,207,17
.byte	102,68,15,58,68,199,16
	xorps	%xmm12,%xmm4
	xorps	%xmm3,%xmm0
	xorps	%xmm5,%xmm1
	pxor	%xmm0,%xmm1
	pxor	%xmm4,%xmm8

	pxor	%xmm1,%xmm8
	pxor	%xmm0,%xmm1

	movdqa	%xmm8,%xmm9
	psrldq	$8,%xmm8
	pslldq	$8,%xmm9
	pxor	%xmm8,%xmm1
	pxor	%xmm9,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	addq	$0x40,%rcx
	jz	.Ldone
	movdqu	32(%rsi),%xmm7
	subq	$0x10,%rcx
	jz	.Lodd_tail
.Lskip4x:





	movdqu	(%rdx),%xmm8
	movdqu	16(%rdx),%xmm3
.byte	102,69,15,56,0,194
.byte	102,65,15,56,0,218
	pxor	%xmm8,%xmm0

	movdqa	%xmm3,%xmm5
	pshufd	$78,%xmm3,%xmm4
	pxor	%xmm3,%xmm4
.byte	102,15,58,68,218,0
.byte	102,15,58,68,234,17
.byte	102,15,58,68,231,0

	leaq	32(%rdx),%rdx
	nop
	subq	$0x20,%rcx
	jbe	.Leven_tail
	nop
	jmp	.Lmod_loop

.align	32
.Lmod_loop:
	movdqa	%xmm0,%xmm1
	movdqa	%xmm4,%xmm8
	pshufd	$78,%xmm0,%xmm4
	pxor	%xmm0,%xmm4

.byte	102,15,58,68,198,0
.byte	102,15,58,68,206,17
.byte	102,15,58,68,231,16

	pxor	%xmm3,%xmm0
	pxor	%xmm5,%xmm1
	movdqu	(%rdx),%xmm9
	pxor	%xmm0,%xmm8
.byte	102,69,15,56,0,202
	movdqu	16(%rdx),%xmm3

	pxor	%xmm1,%xmm8
	pxor	%xmm9,%xmm1
	pxor	%xmm8,%xmm4
.byte	102,65,15,56,0,218
	movdqa	%xmm4,%xmm8
	psrldq	$8,%xmm8
	pslldq	$8,%xmm4
	pxor	%xmm8,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm3,%xmm5

	movdqa	%xmm0,%xmm9
	movdqa	%xmm0,%xmm8
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm8
.byte	102,15,58,68,218,0
	psllq	$1,%xmm0
	pxor	%xmm8,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm8
	pslldq	$8,%xmm0
	psrldq	$8,%xmm8
	pxor	%xmm9,%xmm0
	pshufd	$78,%xmm5,%xmm4
	pxor	%xmm8,%xmm1
	pxor	%xmm5,%xmm4

	movdqa	%xmm0,%xmm9
	psrlq	$1,%xmm0
.byte	102,15,58,68,234,17
	pxor	%xmm9,%xmm1
	pxor	%xmm0,%xmm9
	psrlq	$5,%xmm0
	pxor	%xmm9,%xmm0
	leaq	32(%rdx),%rdx
	psrlq	$1,%xmm0
.byte	102,15,58,68,231,0
	pxor	%xmm1,%xmm0

	subq	$0x20,%rcx
	ja	.Lmod_loop

.Leven_tail:
	movdqa	%xmm0,%xmm1
	movdqa	%xmm4,%xmm8
	pshufd	$78,%xmm0,%xmm4
	pxor	%xmm0,%xmm4

.byte	102,15,58,68,198,0
.byte	102,15,58,68,206,17
.byte	102,15,58,68,231,16

	pxor	%xmm3,%xmm0
	pxor	%xmm5,%xmm1
	pxor	%xmm0,%xmm8
	pxor	%xmm1,%xmm8
	pxor	%xmm8,%xmm4
	movdqa	%xmm4,%xmm8
	psrldq	$8,%xmm8
	pslldq	$8,%xmm4
	pxor	%xmm8,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
	testq	%rcx,%rcx
	jnz	.Ldone

.Lodd_tail:
	movdqu	(%rdx),%xmm8
.byte	102,69,15,56,0,194
	pxor	%xmm8,%xmm0
	movdqa	%xmm0,%xmm1
	pshufd	$78,%xmm0,%xmm3
	pxor	%xmm0,%xmm3
.byte	102,15,58,68,194,0
.byte	102,15,58,68,202,17
.byte	102,15,58,68,223,0
	pxor	%xmm0,%xmm3
	pxor	%xmm1,%xmm3

	movdqa	%xmm3,%xmm4
	psrldq	$8,%xmm3
	pslldq	$8,%xmm4
	pxor	%xmm3,%xmm1
	pxor	%xmm4,%xmm0

	movdqa	%xmm0,%xmm4
	movdqa	%xmm0,%xmm3
	psllq	$5,%xmm0
	pxor	%xmm0,%xmm3
	psllq	$1,%xmm0
	pxor	%xmm3,%xmm0
	psllq	$57,%xmm0
	movdqa	%xmm0,%xmm3
	pslldq	$8,%xmm0
	psrldq	$8,%xmm3
	pxor	%xmm4,%xmm0
	pxor	%xmm3,%xmm1


	movdqa	%xmm0,%xmm4
	psrlq	$1,%xmm0
	pxor	%xmm4,%xmm1
	pxor	%xmm0,%xmm4
	psrlq	$5,%xmm0
	pxor	%xmm4,%xmm0
	psrlq	$1,%xmm0
	pxor	%xmm1,%xmm0
.Ldone:
.byte	102,65,15,56,0,194
	movdqu	%xmm0,(%rdi)
	ret
.cfi_endproc	

.size	gcm_ghash_clmul,.-gcm_ghash_clmul
.globl	gcm_init_avx
.hidden gcm_init_avx
.type	gcm_init_avx,@function
.align	32
gcm_init_avx:
.cfi_startproc	

_CET_ENDBR
	vzeroupper

	vmovdqu	(%rsi),%xmm2
	vpshufd	$78,%xmm2,%xmm2


	vpshufd	$255,%xmm2,%xmm4
	vpsrlq	$63,%xmm2,%xmm3
	vpsllq	$1,%xmm2,%xmm2
	vpxor	%xmm5,%xmm5,%xmm5
	vpcmpgtd	%xmm4,%xmm5,%xmm5
	vpslldq	$8,%xmm3,%xmm3
	vpor	%xmm3,%xmm2,%xmm2


	vpand	.L0x1c2_polynomial(%rip),%xmm5,%xmm5
	vpxor	%xmm5,%xmm2,%xmm2

	vpunpckhqdq	%xmm2,%xmm2,%xmm6
	vmovdqa	%xmm2,%xmm0
	vpxor	%xmm2,%xmm6,%xmm6
	movq	$4,%r10
	jmp	.Linit_start_avx
.align	32
.Linit_loop_avx:
	vpalignr	$8,%xmm3,%xmm4,%xmm5
	vmovdqu	%xmm5,-16(%rdi)
	vpunpckhqdq	%xmm0,%xmm0,%xmm3
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x11,%xmm2,%xmm0,%xmm1
	vpclmulqdq	$0x00,%xmm2,%xmm0,%xmm0
	vpclmulqdq	$0x00,%xmm6,%xmm3,%xmm3
	vpxor	%xmm0,%xmm1,%xmm4
	vpxor	%xmm4,%xmm3,%xmm3

	vpslldq	$8,%xmm3,%xmm4
	vpsrldq	$8,%xmm3,%xmm3
	vpxor	%xmm4,%xmm0,%xmm0
	vpxor	%xmm3,%xmm1,%xmm1
	vpsllq	$57,%xmm0,%xmm3
	vpsllq	$62,%xmm0,%xmm4
	vpxor	%xmm3,%xmm4,%xmm4
	vpsllq	$63,%xmm0,%xmm3
	vpxor	%xmm3,%xmm4,%xmm4
	vpslldq	$8,%xmm4,%xmm3
	vpsrldq	$8,%xmm4,%xmm4
	vpxor	%xmm3,%xmm0,%xmm0
	vpxor	%xmm4,%xmm1,%xmm1

	vpsrlq	$1,%xmm0,%xmm4
	vpxor	%xmm0,%xmm1,%xmm1
	vpxor	%xmm4,%xmm0,%xmm0
	vpsrlq	$5,%xmm4,%xmm4
	vpxor	%xmm4,%xmm0,%xmm0
	vpsrlq	$1,%xmm0,%xmm0
	vpxor	%xmm1,%xmm0,%xmm0
.Linit_start_avx:
	vmovdqa	%xmm0,%xmm5
	vpunpckhqdq	%xmm0,%xmm0,%xmm3
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x11,%xmm2,%xmm0,%xmm1
	vpclmulqdq	$0x00,%xmm2,%xmm0,%xmm0
	vpclmulqdq	$0x00,%xmm6,%xmm3,%xmm3
	vpxor	%xmm0,%xmm1,%xmm4
	vpxor	%xmm4,%xmm3,%xmm3

	vpslldq	$8,%xmm3,%xmm4
	vpsrldq	$8,%xmm3,%xmm3
	vpxor	%xmm4,%xmm0,%xmm0
	vpxor	%xmm3,%xmm1,%xmm1
	vpsllq	$57,%xmm0,%xmm3
	vpsllq	$62,%xmm0,%xmm4
	vpxor	%xmm3,%xmm4,%xmm4
	vpsllq	$63,%xmm0,%xmm3
	vpxor	%xmm3,%xmm4,%xmm4
	vpslldq	$8,%xmm4,%xmm3
	vpsrldq	$8,%xmm4,%xmm4
	vpxor	%xmm3,%xmm0,%xmm0
	vpxor	%xmm4,%xmm1,%xmm1

	vpsrlq	$1,%xmm0,%xmm4
	vpxor	%xmm0,%xmm1,%xmm1
	vpxor	%xmm4,%xmm0,%xmm0
	vpsrlq	$5,%xmm4,%xmm4
	vpxor	%xmm4,%xmm0,%xmm0
	vpsrlq	$1,%xmm0,%xmm0
	vpxor	%xmm1,%xmm0,%xmm0
	vpshufd	$78,%xmm5,%xmm3
	vpshufd	$78,%xmm0,%xmm4
	vpxor	%xmm5,%xmm3,%xmm3
	vmovdqu	%xmm5,0(%rdi)
	vpxor	%xmm0,%xmm4,%xmm4
	vmovdqu	%xmm0,16(%rdi)
	leaq	48(%rdi),%rdi
	subq	$1,%r10
	jnz	.Linit_loop_avx

	vpalignr	$8,%xmm4,%xmm3,%xmm5
	vmovdqu	%xmm5,-16(%rdi)

	vzeroupper
	ret

.cfi_endproc	
.size	gcm_init_avx,.-gcm_init_avx
.globl	gcm_ghash_avx
.hidden gcm_ghash_avx
.type	gcm_ghash_avx,@function
.align	32
gcm_ghash_avx:
.cfi_startproc	

_CET_ENDBR
	vzeroupper

	vmovdqu	(%rdi),%xmm10
	leaq	.L0x1c2_polynomial(%rip),%r10
	leaq	64(%rsi),%rsi
	vmovdqu	.Lbswap_mask(%rip),%xmm13
	vpshufb	%xmm13,%xmm10,%xmm10
	cmpq	$0x80,%rcx
	jb	.Lshort_avx
	subq	$0x80,%rcx

	vmovdqu	112(%rdx),%xmm14
	vmovdqu	0-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm14
	vmovdqu	32-64(%rsi),%xmm7

	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vmovdqu	96(%rdx),%xmm15
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpxor	%xmm14,%xmm9,%xmm9
	vpshufb	%xmm13,%xmm15,%xmm15
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	16-64(%rsi),%xmm6
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vmovdqu	80(%rdx),%xmm14
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vpxor	%xmm15,%xmm8,%xmm8

	vpshufb	%xmm13,%xmm14,%xmm14
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vmovdqu	48-64(%rsi),%xmm6
	vpxor	%xmm14,%xmm9,%xmm9
	vmovdqu	64(%rdx),%xmm15
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	80-64(%rsi),%xmm7

	vpshufb	%xmm13,%xmm15,%xmm15
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpxor	%xmm1,%xmm4,%xmm4
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	64-64(%rsi),%xmm6
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vpxor	%xmm15,%xmm8,%xmm8

	vmovdqu	48(%rdx),%xmm14
	vpxor	%xmm3,%xmm0,%xmm0
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpxor	%xmm4,%xmm1,%xmm1
	vpshufb	%xmm13,%xmm14,%xmm14
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vmovdqu	96-64(%rsi),%xmm6
	vpxor	%xmm5,%xmm2,%xmm2
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	128-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9

	vmovdqu	32(%rdx),%xmm15
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpxor	%xmm1,%xmm4,%xmm4
	vpshufb	%xmm13,%xmm15,%xmm15
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	112-64(%rsi),%xmm6
	vpxor	%xmm2,%xmm5,%xmm5
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vpxor	%xmm15,%xmm8,%xmm8

	vmovdqu	16(%rdx),%xmm14
	vpxor	%xmm3,%xmm0,%xmm0
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpxor	%xmm4,%xmm1,%xmm1
	vpshufb	%xmm13,%xmm14,%xmm14
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vmovdqu	144-64(%rsi),%xmm6
	vpxor	%xmm5,%xmm2,%xmm2
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	176-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9

	vmovdqu	(%rdx),%xmm15
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpxor	%xmm1,%xmm4,%xmm4
	vpshufb	%xmm13,%xmm15,%xmm15
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	160-64(%rsi),%xmm6
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x10,%xmm7,%xmm9,%xmm2

	leaq	128(%rdx),%rdx
	cmpq	$0x80,%rcx
	jb	.Ltail_avx

	vpxor	%xmm10,%xmm15,%xmm15
	subq	$0x80,%rcx
	jmp	.Loop8x_avx

.align	32
.Loop8x_avx:
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vmovdqu	112(%rdx),%xmm14
	vpxor	%xmm0,%xmm3,%xmm3
	vpxor	%xmm15,%xmm8,%xmm8
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm10
	vpshufb	%xmm13,%xmm14,%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm11
	vmovdqu	0-64(%rsi),%xmm6
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm12
	vmovdqu	32-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9

	vmovdqu	96(%rdx),%xmm15
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpxor	%xmm3,%xmm10,%xmm10
	vpshufb	%xmm13,%xmm15,%xmm15
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vxorps	%xmm4,%xmm11,%xmm11
	vmovdqu	16-64(%rsi),%xmm6
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vpxor	%xmm5,%xmm12,%xmm12
	vxorps	%xmm15,%xmm8,%xmm8

	vmovdqu	80(%rdx),%xmm14
	vpxor	%xmm10,%xmm12,%xmm12
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpxor	%xmm11,%xmm12,%xmm12
	vpslldq	$8,%xmm12,%xmm9
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vpsrldq	$8,%xmm12,%xmm12
	vpxor	%xmm9,%xmm10,%xmm10
	vmovdqu	48-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm14
	vxorps	%xmm12,%xmm11,%xmm11
	vpxor	%xmm1,%xmm4,%xmm4
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	80-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9
	vpxor	%xmm2,%xmm5,%xmm5

	vmovdqu	64(%rdx),%xmm15
	vpalignr	$8,%xmm10,%xmm10,%xmm12
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpshufb	%xmm13,%xmm15,%xmm15
	vpxor	%xmm3,%xmm0,%xmm0
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	64-64(%rsi),%xmm6
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm4,%xmm1,%xmm1
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vxorps	%xmm15,%xmm8,%xmm8
	vpxor	%xmm5,%xmm2,%xmm2

	vmovdqu	48(%rdx),%xmm14
	vpclmulqdq	$0x10,(%r10),%xmm10,%xmm10
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpshufb	%xmm13,%xmm14,%xmm14
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vmovdqu	96-64(%rsi),%xmm6
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	128-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9
	vpxor	%xmm2,%xmm5,%xmm5

	vmovdqu	32(%rdx),%xmm15
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpshufb	%xmm13,%xmm15,%xmm15
	vpxor	%xmm3,%xmm0,%xmm0
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	112-64(%rsi),%xmm6
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm4,%xmm1,%xmm1
	vpclmulqdq	$0x00,%xmm7,%xmm9,%xmm2
	vpxor	%xmm15,%xmm8,%xmm8
	vpxor	%xmm5,%xmm2,%xmm2
	vxorps	%xmm12,%xmm10,%xmm10

	vmovdqu	16(%rdx),%xmm14
	vpalignr	$8,%xmm10,%xmm10,%xmm12
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm3
	vpshufb	%xmm13,%xmm14,%xmm14
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm4
	vmovdqu	144-64(%rsi),%xmm6
	vpclmulqdq	$0x10,(%r10),%xmm10,%xmm10
	vxorps	%xmm11,%xmm12,%xmm12
	vpunpckhqdq	%xmm14,%xmm14,%xmm9
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x10,%xmm7,%xmm8,%xmm5
	vmovdqu	176-64(%rsi),%xmm7
	vpxor	%xmm14,%xmm9,%xmm9
	vpxor	%xmm2,%xmm5,%xmm5

	vmovdqu	(%rdx),%xmm15
	vpclmulqdq	$0x00,%xmm6,%xmm14,%xmm0
	vpshufb	%xmm13,%xmm15,%xmm15
	vpclmulqdq	$0x11,%xmm6,%xmm14,%xmm1
	vmovdqu	160-64(%rsi),%xmm6
	vpxor	%xmm12,%xmm15,%xmm15
	vpclmulqdq	$0x10,%xmm7,%xmm9,%xmm2
	vpxor	%xmm10,%xmm15,%xmm15

	leaq	128(%rdx),%rdx
	subq	$0x80,%rcx
	jnc	.Loop8x_avx

	addq	$0x80,%rcx
	jmp	.Ltail_no_xor_avx

.align	32
.Lshort_avx:
	vmovdqu	-16(%rdx,%rcx,1),%xmm14
	leaq	(%rdx,%rcx,1),%rdx
	vmovdqu	0-64(%rsi),%xmm6
	vmovdqu	32-64(%rsi),%xmm7
	vpshufb	%xmm13,%xmm14,%xmm15

	vmovdqa	%xmm0,%xmm3
	vmovdqa	%xmm1,%xmm4
	vmovdqa	%xmm2,%xmm5
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-32(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	16-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vpsrldq	$8,%xmm7,%xmm7
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-48(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	48-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vmovdqu	80-64(%rsi),%xmm7
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-64(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	64-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vpsrldq	$8,%xmm7,%xmm7
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-80(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	96-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vmovdqu	128-64(%rsi),%xmm7
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-96(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	112-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vpsrldq	$8,%xmm7,%xmm7
	subq	$0x10,%rcx
	jz	.Ltail_avx

	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vmovdqu	-112(%rdx),%xmm14
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vmovdqu	144-64(%rsi),%xmm6
	vpshufb	%xmm13,%xmm14,%xmm15
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2
	vmovq	184-64(%rsi),%xmm7
	subq	$0x10,%rcx
	jmp	.Ltail_avx

.align	32
.Ltail_avx:
	vpxor	%xmm10,%xmm15,%xmm15
.Ltail_no_xor_avx:
	vpunpckhqdq	%xmm15,%xmm15,%xmm8
	vpxor	%xmm0,%xmm3,%xmm3
	vpclmulqdq	$0x00,%xmm6,%xmm15,%xmm0
	vpxor	%xmm15,%xmm8,%xmm8
	vpxor	%xmm1,%xmm4,%xmm4
	vpclmulqdq	$0x11,%xmm6,%xmm15,%xmm1
	vpxor	%xmm2,%xmm5,%xmm5
	vpclmulqdq	$0x00,%xmm7,%xmm8,%xmm2

	vmovdqu	(%r10),%xmm12

	vpxor	%xmm0,%xmm3,%xmm10
	vpxor	%xmm1,%xmm4,%xmm11
	vpxor	%xmm2,%xmm5,%xmm5

	vpxor	%xmm10,%xmm5,%xmm5
	vpxor	%xmm11,%xmm5,%xmm5
	vpslldq	$8,%xmm5,%xmm9
	vpsrldq	$8,%xmm5,%xmm5
	vpxor	%xmm9,%xmm10,%xmm10
	vpxor	%xmm5,%xmm11,%xmm11

	vpclmulqdq	$0x10,%xmm12,%xmm10,%xmm9
	vpalignr	$8,%xmm10,%xmm10,%xmm10
	vpxor	%xmm9,%xmm10,%xmm10

	vpclmulqdq	$0x10,%xmm12,%xmm10,%xmm9
	vpalignr	$8,%xmm10,%xmm10,%xmm10
	vpxor	%xmm11,%xmm10,%xmm10
	vpxor	%xmm9,%xmm10,%xmm10

	cmpq	$0,%rcx
	jne	.Lshort_avx

	vpshufb	%xmm13,%xmm10,%xmm10
	vmovdqu	%xmm10,(%rdi)
	vzeroupper
	ret
.cfi_endproc	

.size	gcm_ghash_avx,.-gcm_ghash_avx
.section	.rodata
.align	64
.Lbswap_mask:
.byte	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
.L0x1c2_polynomial:
.byte	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2
.L7_mask:
.long	7,0,7,0
.align	64

.byte	71,72,65,83,72,32,102,111,114,32,120,56,54,95,54,52,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
.align	64
.text	
#endif
