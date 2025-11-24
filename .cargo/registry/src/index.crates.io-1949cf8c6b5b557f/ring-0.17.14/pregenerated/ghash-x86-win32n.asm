; This file is generated from a similarly-named Perl script in the BoringSSL
; source tree. Do not edit by hand.

%include "ring_core_generated/prefix_symbols_nasm.inc"
%ifidn __OUTPUT_FORMAT__, win32
%ifidn __OUTPUT_FORMAT__,obj
section	code	use32 class=code align=64
%elifidn __OUTPUT_FORMAT__,win32
$@feat.00 equ 1
section	.text	code align=64
%else
section	.text	code
%endif
global	_gcm_init_clmul
align	16
_gcm_init_clmul:
L$_gcm_init_clmul_begin:
	mov	edx,DWORD [4+esp]
	mov	eax,DWORD [8+esp]
	call	L$000pic
L$000pic:
	pop	ecx
	lea	ecx,[(L$bswap-L$000pic)+ecx]
	movdqu	xmm2,[eax]
	pshufd	xmm2,xmm2,78
	pshufd	xmm4,xmm2,255
	movdqa	xmm3,xmm2
	psllq	xmm2,1
	pxor	xmm5,xmm5
	psrlq	xmm3,63
	pcmpgtd	xmm5,xmm4
	pslldq	xmm3,8
	por	xmm2,xmm3
	pand	xmm5,[16+ecx]
	pxor	xmm2,xmm5
	movdqa	xmm0,xmm2
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pshufd	xmm4,xmm2,78
	pxor	xmm3,xmm0
	pxor	xmm4,xmm2
db	102,15,58,68,194,0
db	102,15,58,68,202,17
db	102,15,58,68,220,0
	xorps	xmm3,xmm0
	xorps	xmm3,xmm1
	movdqa	xmm4,xmm3
	psrldq	xmm3,8
	pslldq	xmm4,8
	pxor	xmm1,xmm3
	pxor	xmm0,xmm4
	movdqa	xmm4,xmm0
	movdqa	xmm3,xmm0
	psllq	xmm0,5
	pxor	xmm3,xmm0
	psllq	xmm0,1
	pxor	xmm0,xmm3
	psllq	xmm0,57
	movdqa	xmm3,xmm0
	pslldq	xmm0,8
	psrldq	xmm3,8
	pxor	xmm0,xmm4
	pxor	xmm1,xmm3
	movdqa	xmm4,xmm0
	psrlq	xmm0,1
	pxor	xmm1,xmm4
	pxor	xmm4,xmm0
	psrlq	xmm0,5
	pxor	xmm0,xmm4
	psrlq	xmm0,1
	pxor	xmm0,xmm1
	pshufd	xmm3,xmm2,78
	pshufd	xmm4,xmm0,78
	pxor	xmm3,xmm2
	movdqu	[edx],xmm2
	pxor	xmm4,xmm0
	movdqu	[16+edx],xmm0
db	102,15,58,15,227,8
	movdqu	[32+edx],xmm4
	ret
global	_gcm_ghash_clmul
align	16
_gcm_ghash_clmul:
L$_gcm_ghash_clmul_begin:
	push	ebp
	push	ebx
	push	esi
	push	edi
	mov	eax,DWORD [20+esp]
	mov	edx,DWORD [24+esp]
	mov	esi,DWORD [28+esp]
	mov	ebx,DWORD [32+esp]
	call	L$001pic
L$001pic:
	pop	ecx
	lea	ecx,[(L$bswap-L$001pic)+ecx]
	movdqu	xmm0,[eax]
	movdqa	xmm5,[ecx]
	movdqu	xmm2,[edx]
db	102,15,56,0,197
	sub	ebx,16
	jz	NEAR L$002odd_tail
	movdqu	xmm3,[esi]
	movdqu	xmm6,[16+esi]
db	102,15,56,0,221
db	102,15,56,0,245
	movdqu	xmm5,[32+edx]
	pxor	xmm0,xmm3
	pshufd	xmm3,xmm6,78
	movdqa	xmm7,xmm6
	pxor	xmm3,xmm6
	lea	esi,[32+esi]
db	102,15,58,68,242,0
db	102,15,58,68,250,17
db	102,15,58,68,221,0
	movups	xmm2,[16+edx]
	nop
	sub	ebx,32
	jbe	NEAR L$003even_tail
	jmp	NEAR L$004mod_loop
align	32
L$004mod_loop:
	pshufd	xmm4,xmm0,78
	movdqa	xmm1,xmm0
	pxor	xmm4,xmm0
	nop
db	102,15,58,68,194,0
db	102,15,58,68,202,17
db	102,15,58,68,229,16
	movups	xmm2,[edx]
	xorps	xmm0,xmm6
	movdqa	xmm5,[ecx]
	xorps	xmm1,xmm7
	movdqu	xmm7,[esi]
	pxor	xmm3,xmm0
	movdqu	xmm6,[16+esi]
	pxor	xmm3,xmm1
db	102,15,56,0,253
	pxor	xmm4,xmm3
	movdqa	xmm3,xmm4
	psrldq	xmm4,8
	pslldq	xmm3,8
	pxor	xmm1,xmm4
	pxor	xmm0,xmm3
db	102,15,56,0,245
	pxor	xmm1,xmm7
	movdqa	xmm7,xmm6
	movdqa	xmm4,xmm0
	movdqa	xmm3,xmm0
	psllq	xmm0,5
	pxor	xmm3,xmm0
	psllq	xmm0,1
	pxor	xmm0,xmm3
db	102,15,58,68,242,0
	movups	xmm5,[32+edx]
	psllq	xmm0,57
	movdqa	xmm3,xmm0
	pslldq	xmm0,8
	psrldq	xmm3,8
	pxor	xmm0,xmm4
	pxor	xmm1,xmm3
	pshufd	xmm3,xmm7,78
	movdqa	xmm4,xmm0
	psrlq	xmm0,1
	pxor	xmm3,xmm7
	pxor	xmm1,xmm4
db	102,15,58,68,250,17
	movups	xmm2,[16+edx]
	pxor	xmm4,xmm0
	psrlq	xmm0,5
	pxor	xmm0,xmm4
	psrlq	xmm0,1
	pxor	xmm0,xmm1
db	102,15,58,68,221,0
	lea	esi,[32+esi]
	sub	ebx,32
	ja	NEAR L$004mod_loop
L$003even_tail:
	pshufd	xmm4,xmm0,78
	movdqa	xmm1,xmm0
	pxor	xmm4,xmm0
db	102,15,58,68,194,0
db	102,15,58,68,202,17
db	102,15,58,68,229,16
	movdqa	xmm5,[ecx]
	xorps	xmm0,xmm6
	xorps	xmm1,xmm7
	pxor	xmm3,xmm0
	pxor	xmm3,xmm1
	pxor	xmm4,xmm3
	movdqa	xmm3,xmm4
	psrldq	xmm4,8
	pslldq	xmm3,8
	pxor	xmm1,xmm4
	pxor	xmm0,xmm3
	movdqa	xmm4,xmm0
	movdqa	xmm3,xmm0
	psllq	xmm0,5
	pxor	xmm3,xmm0
	psllq	xmm0,1
	pxor	xmm0,xmm3
	psllq	xmm0,57
	movdqa	xmm3,xmm0
	pslldq	xmm0,8
	psrldq	xmm3,8
	pxor	xmm0,xmm4
	pxor	xmm1,xmm3
	movdqa	xmm4,xmm0
	psrlq	xmm0,1
	pxor	xmm1,xmm4
	pxor	xmm4,xmm0
	psrlq	xmm0,5
	pxor	xmm0,xmm4
	psrlq	xmm0,1
	pxor	xmm0,xmm1
	test	ebx,ebx
	jnz	NEAR L$005done
	movups	xmm2,[edx]
L$002odd_tail:
	movdqu	xmm3,[esi]
db	102,15,56,0,221
	pxor	xmm0,xmm3
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pshufd	xmm4,xmm2,78
	pxor	xmm3,xmm0
	pxor	xmm4,xmm2
db	102,15,58,68,194,0
db	102,15,58,68,202,17
db	102,15,58,68,220,0
	xorps	xmm3,xmm0
	xorps	xmm3,xmm1
	movdqa	xmm4,xmm3
	psrldq	xmm3,8
	pslldq	xmm4,8
	pxor	xmm1,xmm3
	pxor	xmm0,xmm4
	movdqa	xmm4,xmm0
	movdqa	xmm3,xmm0
	psllq	xmm0,5
	pxor	xmm3,xmm0
	psllq	xmm0,1
	pxor	xmm0,xmm3
	psllq	xmm0,57
	movdqa	xmm3,xmm0
	pslldq	xmm0,8
	psrldq	xmm3,8
	pxor	xmm0,xmm4
	pxor	xmm1,xmm3
	movdqa	xmm4,xmm0
	psrlq	xmm0,1
	pxor	xmm1,xmm4
	pxor	xmm4,xmm0
	psrlq	xmm0,5
	pxor	xmm0,xmm4
	psrlq	xmm0,1
	pxor	xmm0,xmm1
L$005done:
db	102,15,56,0,197
	movdqu	[eax],xmm0
	pop	edi
	pop	esi
	pop	ebx
	pop	ebp
	ret
align	64
L$bswap:
db	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
db	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,194
db	71,72,65,83,72,32,102,111,114,32,120,56,54,44,32,67
db	82,89,80,84,79,71,65,77,83,32,98,121,32,60,97,112
db	112,114,111,64,111,112,101,110,115,115,108,46,111,114,103,62
db	0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
