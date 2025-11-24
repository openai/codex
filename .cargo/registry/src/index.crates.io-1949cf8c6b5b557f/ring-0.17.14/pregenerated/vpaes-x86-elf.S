// This file is generated from a similarly-named Perl script in the BoringSSL
// source tree. Do not edit by hand.

#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86) && defined(__ELF__)
.text
#ifdef BORINGSSL_DISPATCH_TEST
#endif
.align	64
.L_vpaes_consts:
.long	218628480,235210255,168496130,67568393
.long	252381056,17041926,33884169,51187212
.long	252645135,252645135,252645135,252645135
.long	1512730624,3266504856,1377990664,3401244816
.long	830229760,1275146365,2969422977,3447763452
.long	3411033600,2979783055,338359620,2782886510
.long	4209124096,907596821,221174255,1006095553
.long	191964160,3799684038,3164090317,1589111125
.long	182528256,1777043520,2877432650,3265356744
.long	1874708224,3503451415,3305285752,363511674
.long	1606117888,3487855781,1093350906,2384367825
.long	197121,67569157,134941193,202313229
.long	67569157,134941193,202313229,197121
.long	134941193,202313229,197121,67569157
.long	202313229,197121,67569157,134941193
.long	33619971,100992007,168364043,235736079
.long	235736079,33619971,100992007,168364043
.long	168364043,235736079,33619971,100992007
.long	100992007,168364043,235736079,33619971
.long	50462976,117835012,185207048,252579084
.long	252314880,51251460,117574920,184942860
.long	184682752,252054788,50987272,118359308
.long	118099200,185467140,251790600,50727180
.long	2946363062,528716217,1300004225,1881839624
.long	1532713819,1532713819,1532713819,1532713819
.long	3602276352,4288629033,3737020424,4153884961
.long	1354558464,32357713,2958822624,3775749553
.long	1201988352,132424512,1572796698,503232858
.long	2213177600,1597421020,4103937655,675398315
.byte	86,101,99,116,111,114,32,80,101,114,109,117,116,97,116,105
.byte	111,110,32,65,69,83,32,102,111,114,32,120,56,54,47,83
.byte	83,83,69,51,44,32,77,105,107,101,32,72,97,109,98,117
.byte	114,103,32,40,83,116,97,110,102,111,114,100,32,85,110,105
.byte	118,101,114,115,105,116,121,41,0
.align	64
.hidden	_vpaes_preheat
.type	_vpaes_preheat,@function
.align	16
_vpaes_preheat:
	addl	(%esp),%ebp
	movdqa	-48(%ebp),%xmm7
	movdqa	-16(%ebp),%xmm6
	ret
.size	_vpaes_preheat,.-_vpaes_preheat
.hidden	_vpaes_encrypt_core
.type	_vpaes_encrypt_core,@function
.align	16
_vpaes_encrypt_core:
	movl	$16,%ecx
	movl	240(%edx),%eax
	movdqa	%xmm6,%xmm1
	movdqa	(%ebp),%xmm2
	pandn	%xmm0,%xmm1
	pand	%xmm6,%xmm0
	movdqu	(%edx),%xmm5
.byte	102,15,56,0,208
	movdqa	16(%ebp),%xmm0
	pxor	%xmm5,%xmm2
	psrld	$4,%xmm1
	addl	$16,%edx
.byte	102,15,56,0,193
	leal	192(%ebp),%ebx
	pxor	%xmm2,%xmm0
	jmp	.L000enc_entry
.align	16
.L001enc_loop:
	movdqa	32(%ebp),%xmm4
	movdqa	48(%ebp),%xmm0
.byte	102,15,56,0,226
.byte	102,15,56,0,195
	pxor	%xmm5,%xmm4
	movdqa	64(%ebp),%xmm5
	pxor	%xmm4,%xmm0
	movdqa	-64(%ebx,%ecx,1),%xmm1
.byte	102,15,56,0,234
	movdqa	80(%ebp),%xmm2
	movdqa	(%ebx,%ecx,1),%xmm4
.byte	102,15,56,0,211
	movdqa	%xmm0,%xmm3
	pxor	%xmm5,%xmm2
.byte	102,15,56,0,193
	addl	$16,%edx
	pxor	%xmm2,%xmm0
.byte	102,15,56,0,220
	addl	$16,%ecx
	pxor	%xmm0,%xmm3
.byte	102,15,56,0,193
	andl	$48,%ecx
	subl	$1,%eax
	pxor	%xmm3,%xmm0
.L000enc_entry:
	movdqa	%xmm6,%xmm1
	movdqa	-32(%ebp),%xmm5
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm6,%xmm0
.byte	102,15,56,0,232
	movdqa	%xmm7,%xmm3
	pxor	%xmm1,%xmm0
.byte	102,15,56,0,217
	movdqa	%xmm7,%xmm4
	pxor	%xmm5,%xmm3
.byte	102,15,56,0,224
	movdqa	%xmm7,%xmm2
	pxor	%xmm5,%xmm4
.byte	102,15,56,0,211
	movdqa	%xmm7,%xmm3
	pxor	%xmm0,%xmm2
.byte	102,15,56,0,220
	movdqu	(%edx),%xmm5
	pxor	%xmm1,%xmm3
	jnz	.L001enc_loop
	movdqa	96(%ebp),%xmm4
	movdqa	112(%ebp),%xmm0
.byte	102,15,56,0,226
	pxor	%xmm5,%xmm4
.byte	102,15,56,0,195
	movdqa	64(%ebx,%ecx,1),%xmm1
	pxor	%xmm4,%xmm0
.byte	102,15,56,0,193
	ret
.size	_vpaes_encrypt_core,.-_vpaes_encrypt_core
.hidden	_vpaes_schedule_core
.type	_vpaes_schedule_core,@function
.align	16
_vpaes_schedule_core:
	addl	(%esp),%ebp
	movdqu	(%esi),%xmm0
	movdqa	320(%ebp),%xmm2
	movdqa	%xmm0,%xmm3
	leal	(%ebp),%ebx
	movdqa	%xmm2,4(%esp)
	call	_vpaes_schedule_transform
	movdqa	%xmm0,%xmm7
	testl	%edi,%edi
	jnz	.L002schedule_am_decrypting
	movdqu	%xmm0,(%edx)
	jmp	.L003schedule_go
.L002schedule_am_decrypting:
	movdqa	256(%ebp,%ecx,1),%xmm1
.byte	102,15,56,0,217
	movdqu	%xmm3,(%edx)
	xorl	$48,%ecx
.L003schedule_go:
	cmpl	$192,%eax
	ja	.L004schedule_256
.L005schedule_128:
	movl	$10,%eax
.L006loop_schedule_128:
	call	_vpaes_schedule_round
	decl	%eax
	jz	.L007schedule_mangle_last
	call	_vpaes_schedule_mangle
	jmp	.L006loop_schedule_128
.align	16
.L004schedule_256:
	movdqu	16(%esi),%xmm0
	call	_vpaes_schedule_transform
	movl	$7,%eax
.L008loop_schedule_256:
	call	_vpaes_schedule_mangle
	movdqa	%xmm0,%xmm6
	call	_vpaes_schedule_round
	decl	%eax
	jz	.L007schedule_mangle_last
	call	_vpaes_schedule_mangle
	pshufd	$255,%xmm0,%xmm0
	movdqa	%xmm7,20(%esp)
	movdqa	%xmm6,%xmm7
	call	.L_vpaes_schedule_low_round
	movdqa	20(%esp),%xmm7
	jmp	.L008loop_schedule_256
.align	16
.L007schedule_mangle_last:
	leal	384(%ebp),%ebx
	testl	%edi,%edi
	jnz	.L009schedule_mangle_last_dec
	movdqa	256(%ebp,%ecx,1),%xmm1
.byte	102,15,56,0,193
	leal	352(%ebp),%ebx
	addl	$32,%edx
.L009schedule_mangle_last_dec:
	addl	$-16,%edx
	pxor	336(%ebp),%xmm0
	call	_vpaes_schedule_transform
	movdqu	%xmm0,(%edx)
	pxor	%xmm0,%xmm0
	pxor	%xmm1,%xmm1
	pxor	%xmm2,%xmm2
	pxor	%xmm3,%xmm3
	pxor	%xmm4,%xmm4
	pxor	%xmm5,%xmm5
	pxor	%xmm6,%xmm6
	pxor	%xmm7,%xmm7
	ret
.size	_vpaes_schedule_core,.-_vpaes_schedule_core
.hidden	_vpaes_schedule_round
.type	_vpaes_schedule_round,@function
.align	16
_vpaes_schedule_round:
	movdqa	8(%esp),%xmm2
	pxor	%xmm1,%xmm1
.byte	102,15,58,15,202,15
.byte	102,15,58,15,210,15
	pxor	%xmm1,%xmm7
	pshufd	$255,%xmm0,%xmm0
.byte	102,15,58,15,192,1
	movdqa	%xmm2,8(%esp)
.L_vpaes_schedule_low_round:
	movdqa	%xmm7,%xmm1
	pslldq	$4,%xmm7
	pxor	%xmm1,%xmm7
	movdqa	%xmm7,%xmm1
	pslldq	$8,%xmm7
	pxor	%xmm1,%xmm7
	pxor	336(%ebp),%xmm7
	movdqa	-16(%ebp),%xmm4
	movdqa	-48(%ebp),%xmm5
	movdqa	%xmm4,%xmm1
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm4,%xmm0
	movdqa	-32(%ebp),%xmm2
.byte	102,15,56,0,208
	pxor	%xmm1,%xmm0
	movdqa	%xmm5,%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
	movdqa	%xmm5,%xmm4
.byte	102,15,56,0,224
	pxor	%xmm2,%xmm4
	movdqa	%xmm5,%xmm2
.byte	102,15,56,0,211
	pxor	%xmm0,%xmm2
	movdqa	%xmm5,%xmm3
.byte	102,15,56,0,220
	pxor	%xmm1,%xmm3
	movdqa	32(%ebp),%xmm4
.byte	102,15,56,0,226
	movdqa	48(%ebp),%xmm0
.byte	102,15,56,0,195
	pxor	%xmm4,%xmm0
	pxor	%xmm7,%xmm0
	movdqa	%xmm0,%xmm7
	ret
.size	_vpaes_schedule_round,.-_vpaes_schedule_round
.hidden	_vpaes_schedule_transform
.type	_vpaes_schedule_transform,@function
.align	16
_vpaes_schedule_transform:
	movdqa	-16(%ebp),%xmm2
	movdqa	%xmm2,%xmm1
	pandn	%xmm0,%xmm1
	psrld	$4,%xmm1
	pand	%xmm2,%xmm0
	movdqa	(%ebx),%xmm2
.byte	102,15,56,0,208
	movdqa	16(%ebx),%xmm0
.byte	102,15,56,0,193
	pxor	%xmm2,%xmm0
	ret
.size	_vpaes_schedule_transform,.-_vpaes_schedule_transform
.hidden	_vpaes_schedule_mangle
.type	_vpaes_schedule_mangle,@function
.align	16
_vpaes_schedule_mangle:
	movdqa	%xmm0,%xmm4
	movdqa	128(%ebp),%xmm5
	testl	%edi,%edi
	jnz	.L010schedule_mangle_dec
	addl	$16,%edx
	pxor	336(%ebp),%xmm4
.byte	102,15,56,0,229
	movdqa	%xmm4,%xmm3
.byte	102,15,56,0,229
	pxor	%xmm4,%xmm3
.byte	102,15,56,0,229
	pxor	%xmm4,%xmm3
	jmp	.L011schedule_mangle_both
.align	16
.L010schedule_mangle_dec:
	movdqa	-16(%ebp),%xmm2
	leal	(%ebp),%esi
	movdqa	%xmm2,%xmm1
	pandn	%xmm4,%xmm1
	psrld	$4,%xmm1
	pand	%xmm2,%xmm4
	movdqa	(%esi),%xmm2
.byte	102,15,56,0,212
	movdqa	16(%esi),%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
.byte	102,15,56,0,221
	movdqa	32(%esi),%xmm2
.byte	102,15,56,0,212
	pxor	%xmm3,%xmm2
	movdqa	48(%esi),%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
.byte	102,15,56,0,221
	movdqa	64(%esi),%xmm2
.byte	102,15,56,0,212
	pxor	%xmm3,%xmm2
	movdqa	80(%esi),%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
.byte	102,15,56,0,221
	movdqa	96(%esi),%xmm2
.byte	102,15,56,0,212
	pxor	%xmm3,%xmm2
	movdqa	112(%esi),%xmm3
.byte	102,15,56,0,217
	pxor	%xmm2,%xmm3
	addl	$-16,%edx
.L011schedule_mangle_both:
	movdqa	256(%ebp,%ecx,1),%xmm1
.byte	102,15,56,0,217
	addl	$-16,%ecx
	andl	$48,%ecx
	movdqu	%xmm3,(%edx)
	ret
.size	_vpaes_schedule_mangle,.-_vpaes_schedule_mangle
.globl	vpaes_set_encrypt_key
.hidden	vpaes_set_encrypt_key
.type	vpaes_set_encrypt_key,@function
.align	16
vpaes_set_encrypt_key:
.L_vpaes_set_encrypt_key_begin:
	pushl	%ebp
	pushl	%ebx
	pushl	%esi
	pushl	%edi
#ifdef BORINGSSL_DISPATCH_TEST
	pushl	%ebx
	pushl	%edx
	call	.L012pic_for_function_hit
.L012pic_for_function_hit:
	popl	%ebx
	leal	BORINGSSL_function_hit+5-.L012pic_for_function_hit(%ebx),%ebx
	movl	$1,%edx
	movb	%dl,(%ebx)
	popl	%edx
	popl	%ebx
#endif
	movl	20(%esp),%esi
	leal	-56(%esp),%ebx
	movl	24(%esp),%eax
	andl	$-16,%ebx
	movl	28(%esp),%edx
	xchgl	%esp,%ebx
	movl	%ebx,48(%esp)
	movl	%eax,%ebx
	shrl	$5,%ebx
	addl	$5,%ebx
	movl	%ebx,240(%edx)
	movl	$48,%ecx
	movl	$0,%edi
	leal	.L_vpaes_consts+0x30-.L013pic_point,%ebp
	call	_vpaes_schedule_core
.L013pic_point:
	movl	48(%esp),%esp
	xorl	%eax,%eax
	popl	%edi
	popl	%esi
	popl	%ebx
	popl	%ebp
	ret
.size	vpaes_set_encrypt_key,.-.L_vpaes_set_encrypt_key_begin
.globl	vpaes_encrypt
.hidden	vpaes_encrypt
.type	vpaes_encrypt,@function
.align	16
vpaes_encrypt:
.L_vpaes_encrypt_begin:
	pushl	%ebp
	pushl	%ebx
	pushl	%esi
	pushl	%edi
#ifdef BORINGSSL_DISPATCH_TEST
	pushl	%ebx
	pushl	%edx
	call	.L014pic_for_function_hit
.L014pic_for_function_hit:
	popl	%ebx
	leal	BORINGSSL_function_hit+4-.L014pic_for_function_hit(%ebx),%ebx
	movl	$1,%edx
	movb	%dl,(%ebx)
	popl	%edx
	popl	%ebx
#endif
	leal	.L_vpaes_consts+0x30-.L015pic_point,%ebp
	call	_vpaes_preheat
.L015pic_point:
	movl	20(%esp),%esi
	leal	-56(%esp),%ebx
	movl	24(%esp),%edi
	andl	$-16,%ebx
	movl	28(%esp),%edx
	xchgl	%esp,%ebx
	movl	%ebx,48(%esp)
	movdqu	(%esi),%xmm0
	call	_vpaes_encrypt_core
	movdqu	%xmm0,(%edi)
	movl	48(%esp),%esp
	popl	%edi
	popl	%esi
	popl	%ebx
	popl	%ebp
	ret
.size	vpaes_encrypt,.-.L_vpaes_encrypt_begin
#endif  // !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86) && defined(__ELF__)
