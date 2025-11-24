// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86_64) && defined(__ELF__)
.text	
















.type	_vpaes_encrypt_core,@function
.align	16
_vpaes_encrypt_core:
.cfi_startproc	
	movq	%rdx,%r9
	movq	$16,%r11
	movl	240(%rdx),%eax
	movdqa	%xmm9,%xmm1
	movdqa	.Lk_ipt(%rip),%xmm2
	pandn	%xmm0,%xmm1
	movdqu	(%r9),%xmm5
	psrld	$4,%xmm1
	pand	%xmm9,%xmm0
.byte	102,15,56,0,208
	movdqa	.Lk_ipt+16(%rip),%xmm0
.byte	102,15,56,0,193
	pxor	%xmm5,%xmm2
	addq	$16,%r9
	pxor	%xmm2,%xmm0
	leaq	.Lk_mc_backward(%rip),%r10
	jmp	.Lenc_entry

.align	16
.Lenc_loop:

	movdqa	%xmm13,%xmm4
	movdqa	%xmm12,%xmm0
.byte	102,15,56,0,226
.byte	102,15,56,0,195
	pxor	%xmm5,%xmm4
	movdqa	%xmm15,%xmm5
	pxor	%xmm4,%xmm0
	movdqa	-64(%r11,%r10,1),%xmm1
.byte	102,15,56,0,234
	movdqa	(%r11,%r10,1),%xmm4
	movdqa	%xmm14,%xmm2
.byte	102,15,56,0,211
	movdqa	%xmm0,%xmm3
	pxor	%xmm5,%xmm2
.byte	102,15,56,0,193
	addq	$16,%r9
	pxor	%xmm2,%xmm0
.byte	102,15,56,0,220
	addq	$16,%r11
	pxor	%xmm0,%xmm3
.byte	102,15,56,0,193
	andq	$0x30,%r11
	subq	$1,%rax
	pxor	%xmm3,%xmm0

.Lenc_entry:

	movdqa	%xmm9,%xmm1
	movdqa	%xmm11,%xmm5
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm9,%xmm0
.byte	102,15,56,0,232
	movdqa	%xmm10,%xmm3
	pxor	%xmm1,%xmm0
.byte	102,15,56,0,217
	movdqa	%xmm10,%xmm4
	pxor	%xmm5,%xmm3
.byte	102,15,56,0,224
	movdqa	%xmm10,%xmm2
	pxor	%xmm5,%xmm4
.byte	102,15,56,0,211
	movdqa	%xmm10,%xmm3
	pxor	%xmm0,%xmm2
.byte	102,15,56,0,220
	movdqu	(%r9),%xmm5
	pxor	%xmm1,%xmm3
	jnz	.Lenc_loop


	movdqa	-96(%r10),%xmm4
	movdqa	-80(%r10),%xmm0
.byte	102,15,56,0,226
	pxor	%xmm5,%xmm4
.byte	102,15,56,0,195
	movdqa	64(%r11,%r10,1),%xmm1
	pxor	%xmm4,%xmm0
.byte	102,15,56,0,193
	ret
.cfi_endproc	
.size	_vpaes_encrypt_core,.-_vpaes_encrypt_core






























.type	_vpaes_encrypt_core_2x,@function
.align	16
_vpaes_encrypt_core_2x:
.cfi_startproc	
	movq	%rdx,%r9
	movq	$16,%r11
	movl	240(%rdx),%eax
	movdqa	%xmm9,%xmm1
	movdqa	%xmm9,%xmm7
	movdqa	.Lk_ipt(%rip),%xmm2
	movdqa	%xmm2,%xmm8
	pandn	%xmm0,%xmm1
	pandn	%xmm6,%xmm7
	movdqu	(%r9),%xmm5

	psrld	$4,%xmm1
	psrld	$4,%xmm7
	pand	%xmm9,%xmm0
	pand	%xmm9,%xmm6
.byte	102,15,56,0,208
.byte	102,68,15,56,0,198
	movdqa	.Lk_ipt+16(%rip),%xmm0
	movdqa	%xmm0,%xmm6
.byte	102,15,56,0,193
.byte	102,15,56,0,247
	pxor	%xmm5,%xmm2
	pxor	%xmm5,%xmm8
	addq	$16,%r9
	pxor	%xmm2,%xmm0
	pxor	%xmm8,%xmm6
	leaq	.Lk_mc_backward(%rip),%r10
	jmp	.Lenc2x_entry

.align	16
.Lenc2x_loop:

	movdqa	.Lk_sb1(%rip),%xmm4
	movdqa	.Lk_sb1+16(%rip),%xmm0
	movdqa	%xmm4,%xmm12
	movdqa	%xmm0,%xmm6
.byte	102,15,56,0,226
.byte	102,69,15,56,0,224
.byte	102,15,56,0,195
.byte	102,65,15,56,0,243
	pxor	%xmm5,%xmm4
	pxor	%xmm5,%xmm12
	movdqa	.Lk_sb2(%rip),%xmm5
	movdqa	%xmm5,%xmm13
	pxor	%xmm4,%xmm0
	pxor	%xmm12,%xmm6
	movdqa	-64(%r11,%r10,1),%xmm1

.byte	102,15,56,0,234
.byte	102,69,15,56,0,232
	movdqa	(%r11,%r10,1),%xmm4

	movdqa	.Lk_sb2+16(%rip),%xmm2
	movdqa	%xmm2,%xmm8
.byte	102,15,56,0,211
.byte	102,69,15,56,0,195
	movdqa	%xmm0,%xmm3
	movdqa	%xmm6,%xmm11
	pxor	%xmm5,%xmm2
	pxor	%xmm13,%xmm8
.byte	102,15,56,0,193
.byte	102,15,56,0,241
	addq	$16,%r9
	pxor	%xmm2,%xmm0
	pxor	%xmm8,%xmm6
.byte	102,15,56,0,220
.byte	102,68,15,56,0,220
	addq	$16,%r11
	pxor	%xmm0,%xmm3
	pxor	%xmm6,%xmm11
.byte	102,15,56,0,193
.byte	102,15,56,0,241
	andq	$0x30,%r11
	subq	$1,%rax
	pxor	%xmm3,%xmm0
	pxor	%xmm11,%xmm6

.Lenc2x_entry:

	movdqa	%xmm9,%xmm1
	movdqa	%xmm9,%xmm7
	movdqa	.Lk_inv+16(%rip),%xmm5
	movdqa	%xmm5,%xmm13
	pandn	%xmm0,%xmm1
	pandn	%xmm6,%xmm7
	psrld	$4,%xmm1
	psrld	$4,%xmm7
	pand	%xmm9,%xmm0
	pand	%xmm9,%xmm6
.byte	102,15,56,0,232
.byte	102,68,15,56,0,238
	movdqa	%xmm10,%xmm3
	movdqa	%xmm10,%xmm11
	pxor	%xmm1,%xmm0
	pxor	%xmm7,%xmm6
.byte	102,15,56,0,217
.byte	102,68,15,56,0,223
	movdqa	%xmm10,%xmm4
	movdqa	%xmm10,%xmm12
	pxor	%xmm5,%xmm3
	pxor	%xmm13,%xmm11
.byte	102,15,56,0,224
.byte	102,68,15,56,0,230
	movdqa	%xmm10,%xmm2
	movdqa	%xmm10,%xmm8
	pxor	%xmm5,%xmm4
	pxor	%xmm13,%xmm12
.byte	102,15,56,0,211
.byte	102,69,15,56,0,195
	movdqa	%xmm10,%xmm3
	movdqa	%xmm10,%xmm11
	pxor	%xmm0,%xmm2
	pxor	%xmm6,%xmm8
.byte	102,15,56,0,220
.byte	102,69,15,56,0,220
	movdqu	(%r9),%xmm5

	pxor	%xmm1,%xmm3
	pxor	%xmm7,%xmm11
	jnz	.Lenc2x_loop


	movdqa	-96(%r10),%xmm4
	movdqa	-80(%r10),%xmm0
	movdqa	%xmm4,%xmm12
	movdqa	%xmm0,%xmm6
.byte	102,15,56,0,226
.byte	102,69,15,56,0,224
	pxor	%xmm5,%xmm4
	pxor	%xmm5,%xmm12
.byte	102,15,56,0,195
.byte	102,65,15,56,0,243
	movdqa	64(%r11,%r10,1),%xmm1

	pxor	%xmm4,%xmm0
	pxor	%xmm12,%xmm6
.byte	102,15,56,0,193
.byte	102,15,56,0,241
	ret
.cfi_endproc	
.size	_vpaes_encrypt_core_2x,.-_vpaes_encrypt_core_2x






.type	_vpaes_schedule_core,@function
.align	16
_vpaes_schedule_core:
.cfi_startproc	





	call	_vpaes_preheat
	movdqa	.Lk_rcon(%rip),%xmm8
	movdqu	(%rdi),%xmm0


	movdqa	%xmm0,%xmm3
	leaq	.Lk_ipt(%rip),%r11
	call	_vpaes_schedule_transform
	movdqa	%xmm0,%xmm7

	leaq	.Lk_sr(%rip),%r10


	movdqu	%xmm0,(%rdx)

.Lschedule_go:
	cmpl	$192,%esi
	ja	.Lschedule_256











.Lschedule_128:
	movl	$10,%esi

.Loop_schedule_128:
	call	_vpaes_schedule_round
	decq	%rsi
	jz	.Lschedule_mangle_last
	call	_vpaes_schedule_mangle
	jmp	.Loop_schedule_128











.align	16
.Lschedule_256:
	movdqu	16(%rdi),%xmm0
	call	_vpaes_schedule_transform
	movl	$7,%esi

.Loop_schedule_256:
	call	_vpaes_schedule_mangle
	movdqa	%xmm0,%xmm6


	call	_vpaes_schedule_round
	decq	%rsi
	jz	.Lschedule_mangle_last
	call	_vpaes_schedule_mangle


	pshufd	$0xFF,%xmm0,%xmm0
	movdqa	%xmm7,%xmm5
	movdqa	%xmm6,%xmm7
	call	_vpaes_schedule_low_round
	movdqa	%xmm5,%xmm7

	jmp	.Loop_schedule_256












.align	16
.Lschedule_mangle_last:

	leaq	.Lk_deskew(%rip),%r11


	movdqa	(%r8,%r10,1),%xmm1
.byte	102,15,56,0,193
	leaq	.Lk_opt(%rip),%r11
	addq	$32,%rdx

.Lschedule_mangle_last_dec:
	addq	$-16,%rdx
	pxor	.Lk_s63(%rip),%xmm0
	call	_vpaes_schedule_transform
	movdqu	%xmm0,(%rdx)


	pxor	%xmm0,%xmm0
	pxor	%xmm1,%xmm1
	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
	pxor	%xmm6,%xmm6
	pxor	%xmm7,%xmm7
	ret
.cfi_endproc	
.size	_vpaes_schedule_core,.-_vpaes_schedule_core



















.type	_vpaes_schedule_round,@function
.align	16
_vpaes_schedule_round:
.cfi_startproc	

	pxor	%xmm1,%xmm1
.byte	102,65,15,58,15,200,15
.byte	102,69,15,58,15,192,15
	pxor	%xmm1,%xmm7


	pshufd	$0xFF,%xmm0,%xmm0
.byte	102,15,58,15,192,1




_vpaes_schedule_low_round:

	movdqa	%xmm7,%xmm1
	pslldq	$4,%xmm7
	pxor	%xmm1,%xmm7
	movdqa	%xmm7,%xmm1
	pslldq	$8,%xmm7
	pxor	%xmm1,%xmm7
	pxor	.Lk_s63(%rip),%xmm7


	movdqa	%xmm9,%xmm1
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm9,%xmm0
	movdqa	%xmm11,%xmm2
.byte	102,15,56,0,208
	pxor	%xmm1,%xmm0
	movdqa	%xmm10,%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
	movdqa	%xmm10,%xmm4
.byte	102,15,56,0,224
	pxor	%xmm2,%xmm4
	movdqa	%xmm10,%xmm2
.byte	102,15,56,0,211
	pxor	%xmm0,%xmm2
	movdqa	%xmm10,%xmm3
.byte	102,15,56,0,220
	pxor	%xmm1,%xmm3
	movdqa	%xmm13,%xmm4
.byte	102,15,56,0,226
	movdqa	%xmm12,%xmm0
.byte	102,15,56,0,195
	pxor	%xmm4,%xmm0


	pxor	%xmm7,%xmm0
	movdqa	%xmm0,%xmm7
	ret
.cfi_endproc	
.size	_vpaes_schedule_round,.-_vpaes_schedule_round










.type	_vpaes_schedule_transform,@function
.align	16
_vpaes_schedule_transform:
.cfi_startproc	
	movdqa	%xmm9,%xmm1
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm9,%xmm0
	movdqa	(%r11),%xmm2
.byte	102,15,56,0,208
	movdqa	16(%r11),%xmm0
.byte	102,15,56,0,193
	pxor	%xmm2,%xmm0
	ret
.cfi_endproc	
.size	_vpaes_schedule_transform,.-_vpaes_schedule_transform
























.type	_vpaes_schedule_mangle,@function
.align	16
_vpaes_schedule_mangle:
.cfi_startproc	
	movdqa	%xmm0,%xmm4
	movdqa	.Lk_mc_forward(%rip),%xmm5


	addq	$16,%rdx
	pxor	.Lk_s63(%rip),%xmm4
.byte	102,15,56,0,229
	movdqa	%xmm4,%xmm3
.byte	102,15,56,0,229
	pxor	%xmm4,%xmm3
.byte	102,15,56,0,229
	pxor	%xmm4,%xmm3

.Lschedule_mangle_both:
	movdqa	(%r8,%r10,1),%xmm1
.byte	102,15,56,0,217
	addq	$-16,%r8
	andq	$0x30,%r8
	movdqu	%xmm3,(%rdx)
	ret
.cfi_endproc	
.size	_vpaes_schedule_mangle,.-_vpaes_schedule_mangle




.globl	vpaes_set_encrypt_key
.hidden vpaes_set_encrypt_key
.type	vpaes_set_encrypt_key,@function
.align	16
vpaes_set_encrypt_key:
.cfi_startproc	
_CET_ENDBR
#ifdef BORINGSSL_DISPATCH_TEST
.extern	BORINGSSL_function_hit
.hidden BORINGSSL_function_hit
	movb	$1,BORINGSSL_function_hit+5(%rip)
#endif

	movl	%esi,%eax
	shrl	$5,%eax
	addl	$5,%eax
	movl	%eax,240(%rdx)

	movl	$0,%ecx
	movl	$0x30,%r8d
	call	_vpaes_schedule_core
	xorl	%eax,%eax
	ret
.cfi_endproc	
.size	vpaes_set_encrypt_key,.-vpaes_set_encrypt_key
.globl	vpaes_ctr32_encrypt_blocks
.hidden vpaes_ctr32_encrypt_blocks
.type	vpaes_ctr32_encrypt_blocks,@function
.align	16
vpaes_ctr32_encrypt_blocks:
.cfi_startproc	
_CET_ENDBR

	xchgq	%rcx,%rdx
	testq	%rcx,%rcx
	jz	.Lctr32_abort
	movdqu	(%r8),%xmm0
	movdqa	.Lctr_add_one(%rip),%xmm8
	subq	%rdi,%rsi
	call	_vpaes_preheat
	movdqa	%xmm0,%xmm6
	pshufb	.Lrev_ctr(%rip),%xmm6

	testq	$1,%rcx
	jz	.Lctr32_prep_loop



	movdqu	(%rdi),%xmm7
	call	_vpaes_encrypt_core
	pxor	%xmm7,%xmm0
	paddd	%xmm8,%xmm6
	movdqu	%xmm0,(%rsi,%rdi,1)
	subq	$1,%rcx
	leaq	16(%rdi),%rdi
	jz	.Lctr32_done

.Lctr32_prep_loop:


	movdqa	%xmm6,%xmm14
	movdqa	%xmm6,%xmm15
	paddd	%xmm8,%xmm15

.Lctr32_loop:
	movdqa	.Lrev_ctr(%rip),%xmm1
	movdqa	%xmm14,%xmm0
	movdqa	%xmm15,%xmm6
.byte	102,15,56,0,193
.byte	102,15,56,0,241
	call	_vpaes_encrypt_core_2x
	movdqu	(%rdi),%xmm1
	movdqu	16(%rdi),%xmm2
	movdqa	.Lctr_add_two(%rip),%xmm3
	pxor	%xmm1,%xmm0
	pxor	%xmm2,%xmm6
	paddd	%xmm3,%xmm14
	paddd	%xmm3,%xmm15
	movdqu	%xmm0,(%rsi,%rdi,1)
	movdqu	%xmm6,16(%rsi,%rdi,1)
	subq	$2,%rcx
	leaq	32(%rdi),%rdi
	jnz	.Lctr32_loop

.Lctr32_done:
.Lctr32_abort:
	ret
.cfi_endproc	
.size	vpaes_ctr32_encrypt_blocks,.-vpaes_ctr32_encrypt_blocks






.type	_vpaes_preheat,@function
.align	16
_vpaes_preheat:
.cfi_startproc	
	leaq	.Lk_s0F(%rip),%r10
	movdqa	-32(%r10),%xmm10
	movdqa	-16(%r10),%xmm11
	movdqa	0(%r10),%xmm9
	movdqa	48(%r10),%xmm13
	movdqa	64(%r10),%xmm12
	movdqa	80(%r10),%xmm15
	movdqa	96(%r10),%xmm14
	ret
.cfi_endproc	
.size	_vpaes_preheat,.-_vpaes_preheat





.type	_vpaes_consts,@object
.section	.rodata
.align	64
_vpaes_consts:
.Lk_inv:
.quad	0x0E05060F0D080180, 0x040703090A0B0C02
.quad	0x01040A060F0B0780, 0x030D0E0C02050809

.Lk_s0F:
.quad	0x0F0F0F0F0F0F0F0F, 0x0F0F0F0F0F0F0F0F

.Lk_ipt:
.quad	0xC2B2E8985A2A7000, 0xCABAE09052227808
.quad	0x4C01307D317C4D00, 0xCD80B1FCB0FDCC81

.Lk_sb1:
.quad	0xB19BE18FCB503E00, 0xA5DF7A6E142AF544
.quad	0x3618D415FAE22300, 0x3BF7CCC10D2ED9EF
.Lk_sb2:
.quad	0xE27A93C60B712400, 0x5EB7E955BC982FCD
.quad	0x69EB88400AE12900, 0xC2A163C8AB82234A
.Lk_sbo:
.quad	0xD0D26D176FBDC700, 0x15AABF7AC502A878
.quad	0xCFE474A55FBB6A00, 0x8E1E90D1412B35FA

.Lk_mc_forward:
.quad	0x0407060500030201, 0x0C0F0E0D080B0A09
.quad	0x080B0A0904070605, 0x000302010C0F0E0D
.quad	0x0C0F0E0D080B0A09, 0x0407060500030201
.quad	0x000302010C0F0E0D, 0x080B0A0904070605

.Lk_mc_backward:
.quad	0x0605040702010003, 0x0E0D0C0F0A09080B
.quad	0x020100030E0D0C0F, 0x0A09080B06050407
.quad	0x0E0D0C0F0A09080B, 0x0605040702010003
.quad	0x0A09080B06050407, 0x020100030E0D0C0F

.Lk_sr:
.quad	0x0706050403020100, 0x0F0E0D0C0B0A0908
.quad	0x030E09040F0A0500, 0x0B06010C07020D08
.quad	0x0F060D040B020900, 0x070E050C030A0108
.quad	0x0B0E0104070A0D00, 0x0306090C0F020508

.Lk_rcon:
.quad	0x1F8391B9AF9DEEB6, 0x702A98084D7C7D81

.Lk_s63:
.quad	0x5B5B5B5B5B5B5B5B, 0x5B5B5B5B5B5B5B5B

.Lk_opt:
.quad	0xFF9F4929D6B66000, 0xF7974121DEBE6808
.quad	0x01EDBD5150BCEC00, 0xE10D5DB1B05C0CE0

.Lk_deskew:
.quad	0x07E4A34047A4E300, 0x1DFEB95A5DBEF91A
.quad	0x5F36B5DC83EA6900, 0x2841C2ABF49D1E77


.Lrev_ctr:
.quad	0x0706050403020100, 0x0c0d0e0f0b0a0908


.Lctr_add_one:
.quad	0x0000000000000000, 0x0000000100000000
.Lctr_add_two:
.quad	0x0000000000000000, 0x0000000200000000

.byte	86,101,99,116,111,114,32,80,101,114,109,117,116,97,116,105,111,110,32,65,69,83,32,102,111,114,32,120,56,54,95,54,52,47,83,83,83,69,51,44,32,77,105,107,101,32,72,97,109,98,117,114,103,32,40,83,116,97,110,102,111,114,100,32,85,110,105,118,101,114,115,105,116,121,41,0
.align	64
.size	_vpaes_consts,.-_vpaes_consts
.text	
#endif
