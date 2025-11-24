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
align	16
__aesni_encrypt2:
	movups	xmm0,[edx]
	shl	ecx,4
	movups	xmm1,[16+edx]
	xorps	xmm2,xmm0
	pxor	xmm3,xmm0
	movups	xmm0,[32+edx]
	lea	edx,[32+ecx*1+edx]
	neg	ecx
	add	ecx,16
L$000enc2_loop:
db	102,15,56,220,209
db	102,15,56,220,217
	movups	xmm1,[ecx*1+edx]
	add	ecx,32
db	102,15,56,220,208
db	102,15,56,220,216
	movups	xmm0,[ecx*1+edx-16]
	jnz	NEAR L$000enc2_loop
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,221,208
db	102,15,56,221,216
	ret
align	16
__aesni_encrypt3:
	movups	xmm0,[edx]
	shl	ecx,4
	movups	xmm1,[16+edx]
	xorps	xmm2,xmm0
	pxor	xmm3,xmm0
	pxor	xmm4,xmm0
	movups	xmm0,[32+edx]
	lea	edx,[32+ecx*1+edx]
	neg	ecx
	add	ecx,16
L$001enc3_loop:
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
	movups	xmm1,[ecx*1+edx]
	add	ecx,32
db	102,15,56,220,208
db	102,15,56,220,216
db	102,15,56,220,224
	movups	xmm0,[ecx*1+edx-16]
	jnz	NEAR L$001enc3_loop
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
db	102,15,56,221,208
db	102,15,56,221,216
db	102,15,56,221,224
	ret
align	16
__aesni_encrypt4:
	movups	xmm0,[edx]
	movups	xmm1,[16+edx]
	shl	ecx,4
	xorps	xmm2,xmm0
	pxor	xmm3,xmm0
	pxor	xmm4,xmm0
	pxor	xmm5,xmm0
	movups	xmm0,[32+edx]
	lea	edx,[32+ecx*1+edx]
	neg	ecx
db	15,31,64,0
	add	ecx,16
L$002enc4_loop:
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
db	102,15,56,220,233
	movups	xmm1,[ecx*1+edx]
	add	ecx,32
db	102,15,56,220,208
db	102,15,56,220,216
db	102,15,56,220,224
db	102,15,56,220,232
	movups	xmm0,[ecx*1+edx-16]
	jnz	NEAR L$002enc4_loop
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
db	102,15,56,220,233
db	102,15,56,221,208
db	102,15,56,221,216
db	102,15,56,221,224
db	102,15,56,221,232
	ret
align	16
__aesni_encrypt6:
	movups	xmm0,[edx]
	shl	ecx,4
	movups	xmm1,[16+edx]
	xorps	xmm2,xmm0
	pxor	xmm3,xmm0
	pxor	xmm4,xmm0
db	102,15,56,220,209
	pxor	xmm5,xmm0
	pxor	xmm6,xmm0
db	102,15,56,220,217
	lea	edx,[32+ecx*1+edx]
	neg	ecx
db	102,15,56,220,225
	pxor	xmm7,xmm0
	movups	xmm0,[ecx*1+edx]
	add	ecx,16
	jmp	NEAR L$003_aesni_encrypt6_inner
align	16
L$004enc6_loop:
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
L$003_aesni_encrypt6_inner:
db	102,15,56,220,233
db	102,15,56,220,241
db	102,15,56,220,249
L$_aesni_encrypt6_enter:
	movups	xmm1,[ecx*1+edx]
	add	ecx,32
db	102,15,56,220,208
db	102,15,56,220,216
db	102,15,56,220,224
db	102,15,56,220,232
db	102,15,56,220,240
db	102,15,56,220,248
	movups	xmm0,[ecx*1+edx-16]
	jnz	NEAR L$004enc6_loop
db	102,15,56,220,209
db	102,15,56,220,217
db	102,15,56,220,225
db	102,15,56,220,233
db	102,15,56,220,241
db	102,15,56,220,249
db	102,15,56,221,208
db	102,15,56,221,216
db	102,15,56,221,224
db	102,15,56,221,232
db	102,15,56,221,240
db	102,15,56,221,248
	ret
global	_aes_hw_ctr32_encrypt_blocks
align	16
_aes_hw_ctr32_encrypt_blocks:
L$_aes_hw_ctr32_encrypt_blocks_begin:
	push	ebp
	push	ebx
	push	esi
	push	edi
%ifdef BORINGSSL_DISPATCH_TEST
	push	ebx
	push	edx
	call	L$005pic_for_function_hit
L$005pic_for_function_hit:
	pop	ebx
	lea	ebx,[(_BORINGSSL_function_hit+0-L$005pic_for_function_hit)+ebx]
	mov	edx,1
	mov	BYTE [ebx],dl
	pop	edx
	pop	ebx
%endif
	mov	esi,DWORD [20+esp]
	mov	edi,DWORD [24+esp]
	mov	eax,DWORD [28+esp]
	mov	edx,DWORD [32+esp]
	mov	ebx,DWORD [36+esp]
	mov	ebp,esp
	sub	esp,88
	and	esp,-16
	mov	DWORD [80+esp],ebp
	cmp	eax,1
	je	NEAR L$006ctr32_one_shortcut
	movdqu	xmm7,[ebx]
	mov	DWORD [esp],202182159
	mov	DWORD [4+esp],134810123
	mov	DWORD [8+esp],67438087
	mov	DWORD [12+esp],66051
	mov	ecx,6
	xor	ebp,ebp
	mov	DWORD [16+esp],ecx
	mov	DWORD [20+esp],ecx
	mov	DWORD [24+esp],ecx
	mov	DWORD [28+esp],ebp
db	102,15,58,22,251,3
db	102,15,58,34,253,3
	mov	ecx,DWORD [240+edx]
	bswap	ebx
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	movdqa	xmm2,[esp]
db	102,15,58,34,195,0
	lea	ebp,[3+ebx]
db	102,15,58,34,205,0
	inc	ebx
db	102,15,58,34,195,1
	inc	ebp
db	102,15,58,34,205,1
	inc	ebx
db	102,15,58,34,195,2
	inc	ebp
db	102,15,58,34,205,2
	movdqa	[48+esp],xmm0
db	102,15,56,0,194
	movdqu	xmm6,[edx]
	movdqa	[64+esp],xmm1
db	102,15,56,0,202
	pshufd	xmm2,xmm0,192
	pshufd	xmm3,xmm0,128
	cmp	eax,6
	jb	NEAR L$007ctr32_tail
	pxor	xmm7,xmm6
	shl	ecx,4
	mov	ebx,16
	movdqa	[32+esp],xmm7
	mov	ebp,edx
	sub	ebx,ecx
	lea	edx,[32+ecx*1+edx]
	sub	eax,6
	jmp	NEAR L$008ctr32_loop6
align	16
L$008ctr32_loop6:
	pshufd	xmm4,xmm0,64
	movdqa	xmm0,[32+esp]
	pshufd	xmm5,xmm1,192
	pxor	xmm2,xmm0
	pshufd	xmm6,xmm1,128
	pxor	xmm3,xmm0
	pshufd	xmm7,xmm1,64
	movups	xmm1,[16+ebp]
	pxor	xmm4,xmm0
	pxor	xmm5,xmm0
db	102,15,56,220,209
	pxor	xmm6,xmm0
	pxor	xmm7,xmm0
db	102,15,56,220,217
	movups	xmm0,[32+ebp]
	mov	ecx,ebx
db	102,15,56,220,225
db	102,15,56,220,233
db	102,15,56,220,241
db	102,15,56,220,249
	call	L$_aesni_encrypt6_enter
	movups	xmm1,[esi]
	movups	xmm0,[16+esi]
	xorps	xmm2,xmm1
	movups	xmm1,[32+esi]
	xorps	xmm3,xmm0
	movups	[edi],xmm2
	movdqa	xmm0,[16+esp]
	xorps	xmm4,xmm1
	movdqa	xmm1,[64+esp]
	movups	[16+edi],xmm3
	movups	[32+edi],xmm4
	paddd	xmm1,xmm0
	paddd	xmm0,[48+esp]
	movdqa	xmm2,[esp]
	movups	xmm3,[48+esi]
	movups	xmm4,[64+esi]
	xorps	xmm5,xmm3
	movups	xmm3,[80+esi]
	lea	esi,[96+esi]
	movdqa	[48+esp],xmm0
db	102,15,56,0,194
	xorps	xmm6,xmm4
	movups	[48+edi],xmm5
	xorps	xmm7,xmm3
	movdqa	[64+esp],xmm1
db	102,15,56,0,202
	movups	[64+edi],xmm6
	pshufd	xmm2,xmm0,192
	movups	[80+edi],xmm7
	lea	edi,[96+edi]
	pshufd	xmm3,xmm0,128
	sub	eax,6
	jnc	NEAR L$008ctr32_loop6
	add	eax,6
	jz	NEAR L$009ctr32_ret
	movdqu	xmm7,[ebp]
	mov	edx,ebp
	pxor	xmm7,[32+esp]
	mov	ecx,DWORD [240+ebp]
L$007ctr32_tail:
	por	xmm2,xmm7
	cmp	eax,2
	jb	NEAR L$010ctr32_one
	pshufd	xmm4,xmm0,64
	por	xmm3,xmm7
	je	NEAR L$011ctr32_two
	pshufd	xmm5,xmm1,192
	por	xmm4,xmm7
	cmp	eax,4
	jb	NEAR L$012ctr32_three
	pshufd	xmm6,xmm1,128
	por	xmm5,xmm7
	je	NEAR L$013ctr32_four
	por	xmm6,xmm7
	call	__aesni_encrypt6
	movups	xmm1,[esi]
	movups	xmm0,[16+esi]
	xorps	xmm2,xmm1
	movups	xmm1,[32+esi]
	xorps	xmm3,xmm0
	movups	xmm0,[48+esi]
	xorps	xmm4,xmm1
	movups	xmm1,[64+esi]
	xorps	xmm5,xmm0
	movups	[edi],xmm2
	xorps	xmm6,xmm1
	movups	[16+edi],xmm3
	movups	[32+edi],xmm4
	movups	[48+edi],xmm5
	movups	[64+edi],xmm6
	jmp	NEAR L$009ctr32_ret
align	16
L$006ctr32_one_shortcut:
	movups	xmm2,[ebx]
	mov	ecx,DWORD [240+edx]
L$010ctr32_one:
	movups	xmm0,[edx]
	movups	xmm1,[16+edx]
	lea	edx,[32+edx]
	xorps	xmm2,xmm0
L$014enc1_loop_1:
db	102,15,56,220,209
	dec	ecx
	movups	xmm1,[edx]
	lea	edx,[16+edx]
	jnz	NEAR L$014enc1_loop_1
db	102,15,56,221,209
	movups	xmm6,[esi]
	xorps	xmm6,xmm2
	movups	[edi],xmm6
	jmp	NEAR L$009ctr32_ret
align	16
L$011ctr32_two:
	call	__aesni_encrypt2
	movups	xmm5,[esi]
	movups	xmm6,[16+esi]
	xorps	xmm2,xmm5
	xorps	xmm3,xmm6
	movups	[edi],xmm2
	movups	[16+edi],xmm3
	jmp	NEAR L$009ctr32_ret
align	16
L$012ctr32_three:
	call	__aesni_encrypt3
	movups	xmm5,[esi]
	movups	xmm6,[16+esi]
	xorps	xmm2,xmm5
	movups	xmm7,[32+esi]
	xorps	xmm3,xmm6
	movups	[edi],xmm2
	xorps	xmm4,xmm7
	movups	[16+edi],xmm3
	movups	[32+edi],xmm4
	jmp	NEAR L$009ctr32_ret
align	16
L$013ctr32_four:
	call	__aesni_encrypt4
	movups	xmm6,[esi]
	movups	xmm7,[16+esi]
	movups	xmm1,[32+esi]
	xorps	xmm2,xmm6
	movups	xmm0,[48+esi]
	xorps	xmm3,xmm7
	movups	[edi],xmm2
	xorps	xmm4,xmm1
	movups	[16+edi],xmm3
	xorps	xmm5,xmm0
	movups	[32+edi],xmm4
	movups	[48+edi],xmm5
L$009ctr32_ret:
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	movdqa	[32+esp],xmm0
	pxor	xmm5,xmm5
	movdqa	[48+esp],xmm0
	pxor	xmm6,xmm6
	movdqa	[64+esp],xmm0
	pxor	xmm7,xmm7
	mov	esp,DWORD [80+esp]
	pop	edi
	pop	esi
	pop	ebx
	pop	ebp
	ret
global	_aes_hw_set_encrypt_key_base
align	16
_aes_hw_set_encrypt_key_base:
L$_aes_hw_set_encrypt_key_base_begin:
%ifdef BORINGSSL_DISPATCH_TEST
	push	ebx
	push	edx
	call	L$015pic_for_function_hit
L$015pic_for_function_hit:
	pop	ebx
	lea	ebx,[(_BORINGSSL_function_hit+3-L$015pic_for_function_hit)+ebx]
	mov	edx,1
	mov	BYTE [ebx],dl
	pop	edx
	pop	ebx
%endif
	mov	eax,DWORD [4+esp]
	mov	ecx,DWORD [8+esp]
	mov	edx,DWORD [12+esp]
	push	ebx
	call	L$016pic
L$016pic:
	pop	ebx
	lea	ebx,[(L$key_const-L$016pic)+ebx]
	movups	xmm0,[eax]
	xorps	xmm4,xmm4
	lea	edx,[16+edx]
	cmp	ecx,256
	je	NEAR L$01714rounds
	cmp	ecx,128
	jne	NEAR L$018bad_keybits
align	16
L$01910rounds:
	mov	ecx,9
	movups	[edx-16],xmm0
db	102,15,58,223,200,1
	call	L$020key_128_cold
db	102,15,58,223,200,2
	call	L$021key_128
db	102,15,58,223,200,4
	call	L$021key_128
db	102,15,58,223,200,8
	call	L$021key_128
db	102,15,58,223,200,16
	call	L$021key_128
db	102,15,58,223,200,32
	call	L$021key_128
db	102,15,58,223,200,64
	call	L$021key_128
db	102,15,58,223,200,128
	call	L$021key_128
db	102,15,58,223,200,27
	call	L$021key_128
db	102,15,58,223,200,54
	call	L$021key_128
	movups	[edx],xmm0
	mov	DWORD [80+edx],ecx
	jmp	NEAR L$022good_key
align	16
L$021key_128:
	movups	[edx],xmm0
	lea	edx,[16+edx]
L$020key_128_cold:
	shufps	xmm4,xmm0,16
	xorps	xmm0,xmm4
	shufps	xmm4,xmm0,140
	xorps	xmm0,xmm4
	shufps	xmm1,xmm1,255
	xorps	xmm0,xmm1
	ret
align	16
L$01714rounds:
	movups	xmm2,[16+eax]
	lea	edx,[16+edx]
	mov	ecx,13
	movups	[edx-32],xmm0
	movups	[edx-16],xmm2
db	102,15,58,223,202,1
	call	L$023key_256a_cold
db	102,15,58,223,200,1
	call	L$024key_256b
db	102,15,58,223,202,2
	call	L$025key_256a
db	102,15,58,223,200,2
	call	L$024key_256b
db	102,15,58,223,202,4
	call	L$025key_256a
db	102,15,58,223,200,4
	call	L$024key_256b
db	102,15,58,223,202,8
	call	L$025key_256a
db	102,15,58,223,200,8
	call	L$024key_256b
db	102,15,58,223,202,16
	call	L$025key_256a
db	102,15,58,223,200,16
	call	L$024key_256b
db	102,15,58,223,202,32
	call	L$025key_256a
db	102,15,58,223,200,32
	call	L$024key_256b
db	102,15,58,223,202,64
	call	L$025key_256a
	movups	[edx],xmm0
	mov	DWORD [16+edx],ecx
	xor	eax,eax
	jmp	NEAR L$022good_key
align	16
L$025key_256a:
	movups	[edx],xmm2
	lea	edx,[16+edx]
L$023key_256a_cold:
	shufps	xmm4,xmm0,16
	xorps	xmm0,xmm4
	shufps	xmm4,xmm0,140
	xorps	xmm0,xmm4
	shufps	xmm1,xmm1,255
	xorps	xmm0,xmm1
	ret
align	16
L$024key_256b:
	movups	[edx],xmm0
	lea	edx,[16+edx]
	shufps	xmm4,xmm2,16
	xorps	xmm2,xmm4
	shufps	xmm4,xmm2,140
	xorps	xmm2,xmm4
	shufps	xmm1,xmm1,170
	xorps	xmm2,xmm1
	ret
L$022good_key:
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	xor	eax,eax
	pop	ebx
	ret
align	4
L$018bad_keybits:
	pxor	xmm0,xmm0
	mov	eax,-2
	pop	ebx
	ret
global	_aes_hw_set_encrypt_key_alt
align	16
_aes_hw_set_encrypt_key_alt:
L$_aes_hw_set_encrypt_key_alt_begin:
%ifdef BORINGSSL_DISPATCH_TEST
	push	ebx
	push	edx
	call	L$026pic_for_function_hit
L$026pic_for_function_hit:
	pop	ebx
	lea	ebx,[(_BORINGSSL_function_hit+3-L$026pic_for_function_hit)+ebx]
	mov	edx,1
	mov	BYTE [ebx],dl
	pop	edx
	pop	ebx
%endif
	mov	eax,DWORD [4+esp]
	mov	ecx,DWORD [8+esp]
	mov	edx,DWORD [12+esp]
	push	ebx
	call	L$027pic
L$027pic:
	pop	ebx
	lea	ebx,[(L$key_const-L$027pic)+ebx]
	movups	xmm0,[eax]
	xorps	xmm4,xmm4
	lea	edx,[16+edx]
	cmp	ecx,256
	je	NEAR L$02814rounds_alt
	cmp	ecx,128
	jne	NEAR L$029bad_keybits
align	16
L$03010rounds_alt:
	movdqa	xmm5,[ebx]
	mov	ecx,8
	movdqa	xmm4,[32+ebx]
	movdqa	xmm2,xmm0
	movdqu	[edx-16],xmm0
L$031loop_key128:
db	102,15,56,0,197
db	102,15,56,221,196
	pslld	xmm4,1
	lea	edx,[16+edx]
	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3
	pxor	xmm0,xmm2
	movdqu	[edx-16],xmm0
	movdqa	xmm2,xmm0
	dec	ecx
	jnz	NEAR L$031loop_key128
	movdqa	xmm4,[48+ebx]
db	102,15,56,0,197
db	102,15,56,221,196
	pslld	xmm4,1
	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3
	pxor	xmm0,xmm2
	movdqu	[edx],xmm0
	movdqa	xmm2,xmm0
db	102,15,56,0,197
db	102,15,56,221,196
	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3
	pxor	xmm0,xmm2
	movdqu	[16+edx],xmm0
	mov	ecx,9
	mov	DWORD [96+edx],ecx
	jmp	NEAR L$032good_key
align	16
L$02814rounds_alt:
	movups	xmm2,[16+eax]
	lea	edx,[16+edx]
	movdqa	xmm5,[ebx]
	movdqa	xmm4,[32+ebx]
	mov	ecx,7
	movdqu	[edx-32],xmm0
	movdqa	xmm1,xmm2
	movdqu	[edx-16],xmm2
L$033loop_key256:
db	102,15,56,0,213
db	102,15,56,221,212
	movdqa	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm0,xmm3
	pslld	xmm4,1
	pxor	xmm0,xmm2
	movdqu	[edx],xmm0
	dec	ecx
	jz	NEAR L$034done_key256
	pshufd	xmm2,xmm0,255
	pxor	xmm3,xmm3
db	102,15,56,221,211
	movdqa	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm1,xmm3
	pxor	xmm2,xmm1
	movdqu	[16+edx],xmm2
	lea	edx,[32+edx]
	movdqa	xmm1,xmm2
	jmp	NEAR L$033loop_key256
L$034done_key256:
	mov	ecx,13
	mov	DWORD [16+edx],ecx
L$032good_key:
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	xor	eax,eax
	pop	ebx
	ret
align	4
L$029bad_keybits:
	pxor	xmm0,xmm0
	mov	eax,-2
	pop	ebx
	ret
align	64
L$key_const:
dd	202313229,202313229,202313229,202313229
dd	67569157,67569157,67569157,67569157
dd	1,1,1,1
dd	27,27,27,27
db	65,69,83,32,102,111,114,32,73,110,116,101,108,32,65,69
db	83,45,78,73,44,32,67,82,89,80,84,79,71,65,77,83
db	32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115
db	115,108,46,111,114,103,62,0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
