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
%ifdef BORINGSSL_DISPATCH_TEST
extern	_BORINGSSL_function_hit
%endif
align	64
L$_vpaes_consts:
dd	218628480,235210255,168496130,67568393
dd	252381056,17041926,33884169,51187212
dd	252645135,252645135,252645135,252645135
dd	1512730624,3266504856,1377990664,3401244816
dd	830229760,1275146365,2969422977,3447763452
dd	3411033600,2979783055,338359620,2782886510
dd	4209124096,907596821,221174255,1006095553
dd	191964160,3799684038,3164090317,1589111125
dd	182528256,1777043520,2877432650,3265356744
dd	1874708224,3503451415,3305285752,363511674
dd	1606117888,3487855781,1093350906,2384367825
dd	197121,67569157,134941193,202313229
dd	67569157,134941193,202313229,197121
dd	134941193,202313229,197121,67569157
dd	202313229,197121,67569157,134941193
dd	33619971,100992007,168364043,235736079
dd	235736079,33619971,100992007,168364043
dd	168364043,235736079,33619971,100992007
dd	100992007,168364043,235736079,33619971
dd	50462976,117835012,185207048,252579084
dd	252314880,51251460,117574920,184942860
dd	184682752,252054788,50987272,118359308
dd	118099200,185467140,251790600,50727180
dd	2946363062,528716217,1300004225,1881839624
dd	1532713819,1532713819,1532713819,1532713819
dd	3602276352,4288629033,3737020424,4153884961
dd	1354558464,32357713,2958822624,3775749553
dd	1201988352,132424512,1572796698,503232858
dd	2213177600,1597421020,4103937655,675398315
db	86,101,99,116,111,114,32,80,101,114,109,117,116,97,116,105
db	111,110,32,65,69,83,32,102,111,114,32,120,56,54,47,83
db	83,83,69,51,44,32,77,105,107,101,32,72,97,109,98,117
db	114,103,32,40,83,116,97,110,102,111,114,100,32,85,110,105
db	118,101,114,115,105,116,121,41,0
align	64
align	16
__vpaes_preheat:
	add	ebp,DWORD [esp]
	movdqa	xmm7,[ebp-48]
	movdqa	xmm6,[ebp-16]
	ret
align	16
__vpaes_encrypt_core:
	mov	ecx,16
	mov	eax,DWORD [240+edx]
	movdqa	xmm1,xmm6
	movdqa	xmm2,[ebp]
	pandn	xmm1,xmm0
	pand	xmm0,xmm6
	movdqu	xmm5,[edx]
db	102,15,56,0,208
	movdqa	xmm0,[16+ebp]
	pxor	xmm2,xmm5
	psrld	xmm1,4
	add	edx,16
db	102,15,56,0,193
	lea	ebx,[192+ebp]
	pxor	xmm0,xmm2
	jmp	NEAR L$000enc_entry
align	16
L$001enc_loop:
	movdqa	xmm4,[32+ebp]
	movdqa	xmm0,[48+ebp]
db	102,15,56,0,226
db	102,15,56,0,195
	pxor	xmm4,xmm5
	movdqa	xmm5,[64+ebp]
	pxor	xmm0,xmm4
	movdqa	xmm1,[ecx*1+ebx-64]
db	102,15,56,0,234
	movdqa	xmm2,[80+ebp]
	movdqa	xmm4,[ecx*1+ebx]
db	102,15,56,0,211
	movdqa	xmm3,xmm0
	pxor	xmm2,xmm5
db	102,15,56,0,193
	add	edx,16
	pxor	xmm0,xmm2
db	102,15,56,0,220
	add	ecx,16
	pxor	xmm3,xmm0
db	102,15,56,0,193
	and	ecx,48
	sub	eax,1
	pxor	xmm0,xmm3
L$000enc_entry:
	movdqa	xmm1,xmm6
	movdqa	xmm5,[ebp-32]
	pandn	xmm1,xmm0
	psrld	xmm1,4
	pand	xmm0,xmm6
db	102,15,56,0,232
	movdqa	xmm3,xmm7
	pxor	xmm0,xmm1
db	102,15,56,0,217
	movdqa	xmm4,xmm7
	pxor	xmm3,xmm5
db	102,15,56,0,224
	movdqa	xmm2,xmm7
	pxor	xmm4,xmm5
db	102,15,56,0,211
	movdqa	xmm3,xmm7
	pxor	xmm2,xmm0
db	102,15,56,0,220
	movdqu	xmm5,[edx]
	pxor	xmm3,xmm1
	jnz	NEAR L$001enc_loop
	movdqa	xmm4,[96+ebp]
	movdqa	xmm0,[112+ebp]
db	102,15,56,0,226
	pxor	xmm4,xmm5
db	102,15,56,0,195
	movdqa	xmm1,[64+ecx*1+ebx]
	pxor	xmm0,xmm4
db	102,15,56,0,193
	ret
align	16
__vpaes_schedule_core:
	add	ebp,DWORD [esp]
	movdqu	xmm0,[esi]
	movdqa	xmm2,[320+ebp]
	movdqa	xmm3,xmm0
	lea	ebx,[ebp]
	movdqa	[4+esp],xmm2
	call	__vpaes_schedule_transform
	movdqa	xmm7,xmm0
	test	edi,edi
	jnz	NEAR L$002schedule_am_decrypting
	movdqu	[edx],xmm0
	jmp	NEAR L$003schedule_go
L$002schedule_am_decrypting:
	movdqa	xmm1,[256+ecx*1+ebp]
db	102,15,56,0,217
	movdqu	[edx],xmm3
	xor	ecx,48
L$003schedule_go:
	cmp	eax,192
	ja	NEAR L$004schedule_256
L$005schedule_128:
	mov	eax,10
L$006loop_schedule_128:
	call	__vpaes_schedule_round
	dec	eax
	jz	NEAR L$007schedule_mangle_last
	call	__vpaes_schedule_mangle
	jmp	NEAR L$006loop_schedule_128
align	16
L$004schedule_256:
	movdqu	xmm0,[16+esi]
	call	__vpaes_schedule_transform
	mov	eax,7
L$008loop_schedule_256:
	call	__vpaes_schedule_mangle
	movdqa	xmm6,xmm0
	call	__vpaes_schedule_round
	dec	eax
	jz	NEAR L$007schedule_mangle_last
	call	__vpaes_schedule_mangle
	pshufd	xmm0,xmm0,255
	movdqa	[20+esp],xmm7
	movdqa	xmm7,xmm6
	call	L$_vpaes_schedule_low_round
	movdqa	xmm7,[20+esp]
	jmp	NEAR L$008loop_schedule_256
align	16
L$007schedule_mangle_last:
	lea	ebx,[384+ebp]
	test	edi,edi
	jnz	NEAR L$009schedule_mangle_last_dec
	movdqa	xmm1,[256+ecx*1+ebp]
db	102,15,56,0,193
	lea	ebx,[352+ebp]
	add	edx,32
L$009schedule_mangle_last_dec:
	add	edx,-16
	pxor	xmm0,[336+ebp]
	call	__vpaes_schedule_transform
	movdqu	[edx],xmm0
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	pxor	xmm6,xmm6
	pxor	xmm7,xmm7
	ret
align	16
__vpaes_schedule_round:
	movdqa	xmm2,[8+esp]
	pxor	xmm1,xmm1
db	102,15,58,15,202,15
db	102,15,58,15,210,15
	pxor	xmm7,xmm1
	pshufd	xmm0,xmm0,255
db	102,15,58,15,192,1
	movdqa	[8+esp],xmm2
L$_vpaes_schedule_low_round:
	movdqa	xmm1,xmm7
	pslldq	xmm7,4
	pxor	xmm7,xmm1
	movdqa	xmm1,xmm7
	pslldq	xmm7,8
	pxor	xmm7,xmm1
	pxor	xmm7,[336+ebp]
	movdqa	xmm4,[ebp-16]
	movdqa	xmm5,[ebp-48]
	movdqa	xmm1,xmm4
	pandn	xmm1,xmm0
	psrld	xmm1,4
	pand	xmm0,xmm4
	movdqa	xmm2,[ebp-32]
db	102,15,56,0,208
	pxor	xmm0,xmm1
	movdqa	xmm3,xmm5
db	102,15,56,0,217
	pxor	xmm3,xmm2
	movdqa	xmm4,xmm5
db	102,15,56,0,224
	pxor	xmm4,xmm2
	movdqa	xmm2,xmm5
db	102,15,56,0,211
	pxor	xmm2,xmm0
	movdqa	xmm3,xmm5
db	102,15,56,0,220
	pxor	xmm3,xmm1
	movdqa	xmm4,[32+ebp]
db	102,15,56,0,226
	movdqa	xmm0,[48+ebp]
db	102,15,56,0,195
	pxor	xmm0,xmm4
	pxor	xmm0,xmm7
	movdqa	xmm7,xmm0
	ret
align	16
__vpaes_schedule_transform:
	movdqa	xmm2,[ebp-16]
	movdqa	xmm1,xmm2
	pandn	xmm1,xmm0
	psrld	xmm1,4
	pand	xmm0,xmm2
	movdqa	xmm2,[ebx]
db	102,15,56,0,208
	movdqa	xmm0,[16+ebx]
db	102,15,56,0,193
	pxor	xmm0,xmm2
	ret
align	16
__vpaes_schedule_mangle:
	movdqa	xmm4,xmm0
	movdqa	xmm5,[128+ebp]
	test	edi,edi
	jnz	NEAR L$010schedule_mangle_dec
	add	edx,16
	pxor	xmm4,[336+ebp]
db	102,15,56,0,229
	movdqa	xmm3,xmm4
db	102,15,56,0,229
	pxor	xmm3,xmm4
db	102,15,56,0,229
	pxor	xmm3,xmm4
	jmp	NEAR L$011schedule_mangle_both
align	16
L$010schedule_mangle_dec:
	movdqa	xmm2,[ebp-16]
	lea	esi,[ebp]
	movdqa	xmm1,xmm2
	pandn	xmm1,xmm4
	psrld	xmm1,4
	pand	xmm4,xmm2
	movdqa	xmm2,[esi]
db	102,15,56,0,212
	movdqa	xmm3,[16+esi]
db	102,15,56,0,217
	pxor	xmm3,xmm2
db	102,15,56,0,221
	movdqa	xmm2,[32+esi]
db	102,15,56,0,212
	pxor	xmm2,xmm3
	movdqa	xmm3,[48+esi]
db	102,15,56,0,217
	pxor	xmm3,xmm2
db	102,15,56,0,221
	movdqa	xmm2,[64+esi]
db	102,15,56,0,212
	pxor	xmm2,xmm3
	movdqa	xmm3,[80+esi]
db	102,15,56,0,217
	pxor	xmm3,xmm2
db	102,15,56,0,221
	movdqa	xmm2,[96+esi]
db	102,15,56,0,212
	pxor	xmm2,xmm3
	movdqa	xmm3,[112+esi]
db	102,15,56,0,217
	pxor	xmm3,xmm2
	add	edx,-16
L$011schedule_mangle_both:
	movdqa	xmm1,[256+ecx*1+ebp]
db	102,15,56,0,217
	add	ecx,-16
	and	ecx,48
	movdqu	[edx],xmm3
	ret
global	_vpaes_set_encrypt_key
align	16
_vpaes_set_encrypt_key:
L$_vpaes_set_encrypt_key_begin:
	push	ebp
	push	ebx
	push	esi
	push	edi
%ifdef BORINGSSL_DISPATCH_TEST
	push	ebx
	push	edx
	call	L$012pic_for_function_hit
L$012pic_for_function_hit:
	pop	ebx
	lea	ebx,[(_BORINGSSL_function_hit+5-L$012pic_for_function_hit)+ebx]
	mov	edx,1
	mov	BYTE [ebx],dl
	pop	edx
	pop	ebx
%endif
	mov	esi,DWORD [20+esp]
	lea	ebx,[esp-56]
	mov	eax,DWORD [24+esp]
	and	ebx,-16
	mov	edx,DWORD [28+esp]
	xchg	ebx,esp
	mov	DWORD [48+esp],ebx
	mov	ebx,eax
	shr	ebx,5
	add	ebx,5
	mov	DWORD [240+edx],ebx
	mov	ecx,48
	mov	edi,0
	lea	ebp,[(L$_vpaes_consts+0x30-L$013pic_point)]
	call	__vpaes_schedule_core
L$013pic_point:
	mov	esp,DWORD [48+esp]
	xor	eax,eax
	pop	edi
	pop	esi
	pop	ebx
	pop	ebp
	ret
global	_vpaes_encrypt
align	16
_vpaes_encrypt:
L$_vpaes_encrypt_begin:
	push	ebp
	push	ebx
	push	esi
	push	edi
%ifdef BORINGSSL_DISPATCH_TEST
	push	ebx
	push	edx
	call	L$014pic_for_function_hit
L$014pic_for_function_hit:
	pop	ebx
	lea	ebx,[(_BORINGSSL_function_hit+4-L$014pic_for_function_hit)+ebx]
	mov	edx,1
	mov	BYTE [ebx],dl
	pop	edx
	pop	ebx
%endif
	lea	ebp,[(L$_vpaes_consts+0x30-L$015pic_point)]
	call	__vpaes_preheat
L$015pic_point:
	mov	esi,DWORD [20+esp]
	lea	ebx,[esp-56]
	mov	edi,DWORD [24+esp]
	and	ebx,-16
	mov	edx,DWORD [28+esp]
	xchg	ebx,esp
	mov	DWORD [48+esp],ebx
	movdqu	xmm0,[esi]
	call	__vpaes_encrypt_core
	movdqu	[edi],xmm0
	mov	esp,DWORD [48+esp]
	pop	edi
	pop	esi
	pop	ebx
	pop	ebp
	ret
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
