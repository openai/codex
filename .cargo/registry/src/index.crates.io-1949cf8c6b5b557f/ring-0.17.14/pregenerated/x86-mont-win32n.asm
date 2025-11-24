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
global	_bn_mul_mont
align	16
_bn_mul_mont:
L$_bn_mul_mont_begin:
	push	ebp
	push	ebx
	push	esi
	push	edi
	xor	eax,eax
	mov	edi,DWORD [40+esp]
	lea	esi,[20+esp]
	lea	edx,[24+esp]
	add	edi,2
	neg	edi
	lea	ebp,[edi*4+esp-32]
	neg	edi
	mov	eax,ebp
	sub	eax,edx
	and	eax,2047
	sub	ebp,eax
	xor	edx,ebp
	and	edx,2048
	xor	edx,2048
	sub	ebp,edx
	and	ebp,-64
	mov	eax,esp
	sub	eax,ebp
	and	eax,-4096
	mov	edx,esp
	lea	esp,[eax*1+ebp]
	mov	eax,DWORD [esp]
	cmp	esp,ebp
	ja	NEAR L$000page_walk
	jmp	NEAR L$001page_walk_done
align	16
L$000page_walk:
	lea	esp,[esp-4096]
	mov	eax,DWORD [esp]
	cmp	esp,ebp
	ja	NEAR L$000page_walk
L$001page_walk_done:
	mov	eax,DWORD [esi]
	mov	ebx,DWORD [4+esi]
	mov	ecx,DWORD [8+esi]
	mov	ebp,DWORD [12+esi]
	mov	esi,DWORD [16+esi]
	mov	esi,DWORD [esi]
	mov	DWORD [4+esp],eax
	mov	DWORD [8+esp],ebx
	mov	DWORD [12+esp],ecx
	mov	DWORD [16+esp],ebp
	mov	DWORD [20+esp],esi
	lea	ebx,[edi-3]
	mov	DWORD [24+esp],edx
	mov	eax,-1
	movd	mm7,eax
	mov	esi,DWORD [8+esp]
	mov	edi,DWORD [12+esp]
	mov	ebp,DWORD [16+esp]
	xor	edx,edx
	xor	ecx,ecx
	movd	mm4,DWORD [edi]
	movd	mm5,DWORD [esi]
	movd	mm3,DWORD [ebp]
	pmuludq	mm5,mm4
	movq	mm2,mm5
	movq	mm0,mm5
	pand	mm0,mm7
	pmuludq	mm5,[20+esp]
	pmuludq	mm3,mm5
	paddq	mm3,mm0
	movd	mm1,DWORD [4+ebp]
	movd	mm0,DWORD [4+esi]
	psrlq	mm2,32
	psrlq	mm3,32
	inc	ecx
align	16
L$0021st:
	pmuludq	mm0,mm4
	pmuludq	mm1,mm5
	paddq	mm2,mm0
	paddq	mm3,mm1
	movq	mm0,mm2
	pand	mm0,mm7
	movd	mm1,DWORD [4+ecx*4+ebp]
	paddq	mm3,mm0
	movd	mm0,DWORD [4+ecx*4+esi]
	psrlq	mm2,32
	movd	DWORD [28+ecx*4+esp],mm3
	psrlq	mm3,32
	lea	ecx,[1+ecx]
	cmp	ecx,ebx
	jl	NEAR L$0021st
	pmuludq	mm0,mm4
	pmuludq	mm1,mm5
	paddq	mm2,mm0
	paddq	mm3,mm1
	movq	mm0,mm2
	pand	mm0,mm7
	paddq	mm3,mm0
	movd	DWORD [28+ecx*4+esp],mm3
	psrlq	mm2,32
	psrlq	mm3,32
	paddq	mm3,mm2
	movq	[32+ebx*4+esp],mm3
	inc	edx
L$003outer:
	xor	ecx,ecx
	movd	mm4,DWORD [edx*4+edi]
	movd	mm5,DWORD [esi]
	movd	mm6,DWORD [32+esp]
	movd	mm3,DWORD [ebp]
	pmuludq	mm5,mm4
	paddq	mm5,mm6
	movq	mm0,mm5
	movq	mm2,mm5
	pand	mm0,mm7
	pmuludq	mm5,[20+esp]
	pmuludq	mm3,mm5
	paddq	mm3,mm0
	movd	mm6,DWORD [36+esp]
	movd	mm1,DWORD [4+ebp]
	movd	mm0,DWORD [4+esi]
	psrlq	mm2,32
	psrlq	mm3,32
	paddq	mm2,mm6
	inc	ecx
	dec	ebx
L$004inner:
	pmuludq	mm0,mm4
	pmuludq	mm1,mm5
	paddq	mm2,mm0
	paddq	mm3,mm1
	movq	mm0,mm2
	movd	mm6,DWORD [36+ecx*4+esp]
	pand	mm0,mm7
	movd	mm1,DWORD [4+ecx*4+ebp]
	paddq	mm3,mm0
	movd	mm0,DWORD [4+ecx*4+esi]
	psrlq	mm2,32
	movd	DWORD [28+ecx*4+esp],mm3
	psrlq	mm3,32
	paddq	mm2,mm6
	dec	ebx
	lea	ecx,[1+ecx]
	jnz	NEAR L$004inner
	mov	ebx,ecx
	pmuludq	mm0,mm4
	pmuludq	mm1,mm5
	paddq	mm2,mm0
	paddq	mm3,mm1
	movq	mm0,mm2
	pand	mm0,mm7
	paddq	mm3,mm0
	movd	DWORD [28+ecx*4+esp],mm3
	psrlq	mm2,32
	psrlq	mm3,32
	movd	mm6,DWORD [36+ebx*4+esp]
	paddq	mm3,mm2
	paddq	mm3,mm6
	movq	[32+ebx*4+esp],mm3
	lea	edx,[1+edx]
	cmp	edx,ebx
	jle	NEAR L$003outer
	emms
	jmp	NEAR L$005common_tail
align	16
L$005common_tail:
	mov	ebp,DWORD [16+esp]
	mov	edi,DWORD [4+esp]
	lea	esi,[32+esp]
	mov	eax,DWORD [esi]
	mov	ecx,ebx
	xor	edx,edx
align	16
L$006sub:
	sbb	eax,DWORD [edx*4+ebp]
	mov	DWORD [edx*4+edi],eax
	dec	ecx
	mov	eax,DWORD [4+edx*4+esi]
	lea	edx,[1+edx]
	jge	NEAR L$006sub
	sbb	eax,0
	mov	edx,-1
	xor	edx,eax
	jmp	NEAR L$007copy
align	16
L$007copy:
	mov	esi,DWORD [32+ebx*4+esp]
	mov	ebp,DWORD [ebx*4+edi]
	mov	DWORD [32+ebx*4+esp],ecx
	and	esi,eax
	and	ebp,edx
	or	ebp,esi
	mov	DWORD [ebx*4+edi],ebp
	dec	ebx
	jge	NEAR L$007copy
	mov	esp,DWORD [24+esp]
	mov	eax,1
	pop	edi
	pop	esi
	pop	ebx
	pop	ebp
	ret
db	77,111,110,116,103,111,109,101,114,121,32,77,117,108,116,105
db	112,108,105,99,97,116,105,111,110,32,102,111,114,32,120,56
db	54,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121
db	32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46
db	111,114,103,62,0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
