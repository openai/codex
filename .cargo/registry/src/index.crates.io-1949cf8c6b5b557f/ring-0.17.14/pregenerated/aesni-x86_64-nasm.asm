; This file is generated from a similarly-named Perl script in the BoringSSL
; source tree. Do not edit by hand.

%ifidn __OUTPUT_FORMAT__, win64
default	rel
%define XMMWORD
%define YMMWORD
%define ZMMWORD
%define _CET_ENDBR

%include "ring_core_generated/prefix_symbols_nasm.inc"
section	.text code align=64


ALIGN	16
_aesni_encrypt2:

	movups	xmm0,XMMWORD[rcx]
	shl	eax,4
	movups	xmm1,XMMWORD[16+rcx]
	xorps	xmm2,xmm0
	xorps	xmm3,xmm0
	movups	xmm0,XMMWORD[32+rcx]
	lea	rcx,[32+rax*1+rcx]
	neg	rax
	add	rax,16

$L$enc_loop2:
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	movups	xmm1,XMMWORD[rax*1+rcx]
	add	rax,32
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	movups	xmm0,XMMWORD[((-16))+rax*1+rcx]
	jnz	NEAR $L$enc_loop2

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,221,208
	DB	102,15,56,221,216
	ret



ALIGN	16
_aesni_encrypt3:

	movups	xmm0,XMMWORD[rcx]
	shl	eax,4
	movups	xmm1,XMMWORD[16+rcx]
	xorps	xmm2,xmm0
	xorps	xmm3,xmm0
	xorps	xmm4,xmm0
	movups	xmm0,XMMWORD[32+rcx]
	lea	rcx,[32+rax*1+rcx]
	neg	rax
	add	rax,16

$L$enc_loop3:
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	movups	xmm1,XMMWORD[rax*1+rcx]
	add	rax,32
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	movups	xmm0,XMMWORD[((-16))+rax*1+rcx]
	jnz	NEAR $L$enc_loop3

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,221,208
	DB	102,15,56,221,216
	DB	102,15,56,221,224
	ret



ALIGN	16
_aesni_encrypt4:

	movups	xmm0,XMMWORD[rcx]
	shl	eax,4
	movups	xmm1,XMMWORD[16+rcx]
	xorps	xmm2,xmm0
	xorps	xmm3,xmm0
	xorps	xmm4,xmm0
	xorps	xmm5,xmm0
	movups	xmm0,XMMWORD[32+rcx]
	lea	rcx,[32+rax*1+rcx]
	neg	rax
	DB	0x0f,0x1f,0x00
	add	rax,16

$L$enc_loop4:
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	movups	xmm1,XMMWORD[rax*1+rcx]
	add	rax,32
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	movups	xmm0,XMMWORD[((-16))+rax*1+rcx]
	jnz	NEAR $L$enc_loop4

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,221,208
	DB	102,15,56,221,216
	DB	102,15,56,221,224
	DB	102,15,56,221,232
	ret



ALIGN	16
_aesni_encrypt6:

	movups	xmm0,XMMWORD[rcx]
	shl	eax,4
	movups	xmm1,XMMWORD[16+rcx]
	xorps	xmm2,xmm0
	pxor	xmm3,xmm0
	pxor	xmm4,xmm0
	DB	102,15,56,220,209
	lea	rcx,[32+rax*1+rcx]
	neg	rax
	DB	102,15,56,220,217
	pxor	xmm5,xmm0
	pxor	xmm6,xmm0
	DB	102,15,56,220,225
	pxor	xmm7,xmm0
	movups	xmm0,XMMWORD[rax*1+rcx]
	add	rax,16
	jmp	NEAR $L$enc_loop6_enter
ALIGN	16
$L$enc_loop6:
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
$L$enc_loop6_enter:
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	movups	xmm1,XMMWORD[rax*1+rcx]
	add	rax,32
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	movups	xmm0,XMMWORD[((-16))+rax*1+rcx]
	jnz	NEAR $L$enc_loop6

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,15,56,221,208
	DB	102,15,56,221,216
	DB	102,15,56,221,224
	DB	102,15,56,221,232
	DB	102,15,56,221,240
	DB	102,15,56,221,248
	ret



ALIGN	16
_aesni_encrypt8:

	movups	xmm0,XMMWORD[rcx]
	shl	eax,4
	movups	xmm1,XMMWORD[16+rcx]
	xorps	xmm2,xmm0
	xorps	xmm3,xmm0
	pxor	xmm4,xmm0
	pxor	xmm5,xmm0
	pxor	xmm6,xmm0
	lea	rcx,[32+rax*1+rcx]
	neg	rax
	DB	102,15,56,220,209
	pxor	xmm7,xmm0
	pxor	xmm8,xmm0
	DB	102,15,56,220,217
	pxor	xmm9,xmm0
	movups	xmm0,XMMWORD[rax*1+rcx]
	add	rax,16
	jmp	NEAR $L$enc_loop8_inner
ALIGN	16
$L$enc_loop8:
	DB	102,15,56,220,209
	DB	102,15,56,220,217
$L$enc_loop8_inner:
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
$L$enc_loop8_enter:
	movups	xmm1,XMMWORD[rax*1+rcx]
	add	rax,32
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((-16))+rax*1+rcx]
	jnz	NEAR $L$enc_loop8

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	DB	102,15,56,221,208
	DB	102,15,56,221,216
	DB	102,15,56,221,224
	DB	102,15,56,221,232
	DB	102,15,56,221,240
	DB	102,15,56,221,248
	DB	102,68,15,56,221,192
	DB	102,68,15,56,221,200
	ret


global	aes_hw_ctr32_encrypt_blocks

ALIGN	16
aes_hw_ctr32_encrypt_blocks:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_aes_hw_ctr32_encrypt_blocks:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8
	mov	rcx,r9
	mov	r8,QWORD[40+rsp]



_CET_ENDBR
%ifdef BORINGSSL_DISPATCH_TEST
	mov	BYTE[BORINGSSL_function_hit],1
%endif
	cmp	rdx,1
	jne	NEAR $L$ctr32_bulk



	movups	xmm2,XMMWORD[r8]
	movups	xmm3,XMMWORD[rdi]
	mov	edx,DWORD[240+rcx]
	movups	xmm0,XMMWORD[rcx]
	movups	xmm1,XMMWORD[16+rcx]
	lea	rcx,[32+rcx]
	xorps	xmm2,xmm0
$L$oop_enc1_1:
	DB	102,15,56,220,209
	dec	edx
	movups	xmm1,XMMWORD[rcx]
	lea	rcx,[16+rcx]
	jnz	NEAR $L$oop_enc1_1
	DB	102,15,56,221,209
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	xorps	xmm2,xmm3
	pxor	xmm3,xmm3
	movups	XMMWORD[rsi],xmm2
	xorps	xmm2,xmm2
	jmp	NEAR $L$ctr32_epilogue

ALIGN	16
$L$ctr32_bulk:
	lea	r11,[rsp]

	push	rbp

	sub	rsp,288
	and	rsp,-16
	movaps	XMMWORD[(-168)+r11],xmm6
	movaps	XMMWORD[(-152)+r11],xmm7
	movaps	XMMWORD[(-136)+r11],xmm8
	movaps	XMMWORD[(-120)+r11],xmm9
	movaps	XMMWORD[(-104)+r11],xmm10
	movaps	XMMWORD[(-88)+r11],xmm11
	movaps	XMMWORD[(-72)+r11],xmm12
	movaps	XMMWORD[(-56)+r11],xmm13
	movaps	XMMWORD[(-40)+r11],xmm14
	movaps	XMMWORD[(-24)+r11],xmm15
$L$ctr32_body:




	movdqu	xmm2,XMMWORD[r8]
	movdqu	xmm0,XMMWORD[rcx]
	mov	r8d,DWORD[12+r8]
	pxor	xmm2,xmm0
	mov	ebp,DWORD[12+rcx]
	movdqa	XMMWORD[rsp],xmm2
	bswap	r8d
	movdqa	xmm3,xmm2
	movdqa	xmm4,xmm2
	movdqa	xmm5,xmm2
	movdqa	XMMWORD[64+rsp],xmm2
	movdqa	XMMWORD[80+rsp],xmm2
	movdqa	XMMWORD[96+rsp],xmm2
	mov	r10,rdx
	movdqa	XMMWORD[112+rsp],xmm2

	lea	rax,[1+r8]
	lea	rdx,[2+r8]
	bswap	eax
	bswap	edx
	xor	eax,ebp
	xor	edx,ebp
DB	102,15,58,34,216,3
	lea	rax,[3+r8]
	movdqa	XMMWORD[16+rsp],xmm3
DB	102,15,58,34,226,3
	bswap	eax
	mov	rdx,r10
	lea	r10,[4+r8]
	movdqa	XMMWORD[32+rsp],xmm4
	xor	eax,ebp
	bswap	r10d
DB	102,15,58,34,232,3
	xor	r10d,ebp
	movdqa	XMMWORD[48+rsp],xmm5
	lea	r9,[5+r8]
	mov	DWORD[((64+12))+rsp],r10d
	bswap	r9d
	lea	r10,[6+r8]
	mov	eax,DWORD[240+rcx]
	xor	r9d,ebp
	bswap	r10d
	mov	DWORD[((80+12))+rsp],r9d
	xor	r10d,ebp
	lea	r9,[7+r8]
	mov	DWORD[((96+12))+rsp],r10d
	bswap	r9d
	xor	r9d,ebp
	mov	DWORD[((112+12))+rsp],r9d

	movups	xmm1,XMMWORD[16+rcx]

	movdqa	xmm6,XMMWORD[64+rsp]
	movdqa	xmm7,XMMWORD[80+rsp]

	cmp	rdx,8
	jb	NEAR $L$ctr32_tail

	lea	rcx,[128+rcx]
	sub	rdx,8
	jmp	NEAR $L$ctr32_loop8

ALIGN	32
$L$ctr32_loop8:
	add	r8d,8
	movdqa	xmm8,XMMWORD[96+rsp]
	DB	102,15,56,220,209
	mov	r9d,r8d
	movdqa	xmm9,XMMWORD[112+rsp]
	DB	102,15,56,220,217
	bswap	r9d
	movups	xmm0,XMMWORD[((32-128))+rcx]
	DB	102,15,56,220,225
	xor	r9d,ebp
	nop
	DB	102,15,56,220,233
	mov	DWORD[((0+12))+rsp],r9d
	lea	r9,[1+r8]
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((48-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	mov	DWORD[((16+12))+rsp],r9d
	lea	r9,[2+r8]
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((64-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	mov	DWORD[((32+12))+rsp],r9d
	lea	r9,[3+r8]
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((80-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	mov	DWORD[((48+12))+rsp],r9d
	lea	r9,[4+r8]
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((96-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	mov	DWORD[((64+12))+rsp],r9d
	lea	r9,[5+r8]
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((112-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	mov	DWORD[((80+12))+rsp],r9d
	lea	r9,[6+r8]
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((128-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	xor	r9d,ebp
	DB	0x66,0x90
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	mov	DWORD[((96+12))+rsp],r9d
	lea	r9,[7+r8]
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((144-128))+rcx]
	bswap	r9d
	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	xor	r9d,ebp
	movdqu	xmm10,XMMWORD[rdi]
	DB	102,15,56,220,232
	mov	DWORD[((112+12))+rsp],r9d
	cmp	eax,11
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((160-128))+rcx]

	jb	NEAR $L$ctr32_enc_done

	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((176-128))+rcx]

	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((192-128))+rcx]


	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movups	xmm1,XMMWORD[((208-128))+rcx]

	DB	102,15,56,220,208
	DB	102,15,56,220,216
	DB	102,15,56,220,224
	DB	102,15,56,220,232
	DB	102,15,56,220,240
	DB	102,15,56,220,248
	DB	102,68,15,56,220,192
	DB	102,68,15,56,220,200
	movups	xmm0,XMMWORD[((224-128))+rcx]
	jmp	NEAR $L$ctr32_enc_done

ALIGN	16
$L$ctr32_enc_done:
	movdqu	xmm11,XMMWORD[16+rdi]
	pxor	xmm10,xmm0
	movdqu	xmm12,XMMWORD[32+rdi]
	pxor	xmm11,xmm0
	movdqu	xmm13,XMMWORD[48+rdi]
	pxor	xmm12,xmm0
	movdqu	xmm14,XMMWORD[64+rdi]
	pxor	xmm13,xmm0
	movdqu	xmm15,XMMWORD[80+rdi]
	pxor	xmm14,xmm0
	prefetcht0	[448+rdi]
	prefetcht0	[512+rdi]
	pxor	xmm15,xmm0
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193
	DB	102,68,15,56,220,201
	movdqu	xmm1,XMMWORD[96+rdi]
	lea	rdi,[128+rdi]

	DB	102,65,15,56,221,210
	pxor	xmm1,xmm0
	movdqu	xmm10,XMMWORD[((112-128))+rdi]
	DB	102,65,15,56,221,219
	pxor	xmm10,xmm0
	movdqa	xmm11,XMMWORD[rsp]
	DB	102,65,15,56,221,228
	DB	102,65,15,56,221,237
	movdqa	xmm12,XMMWORD[16+rsp]
	movdqa	xmm13,XMMWORD[32+rsp]
	DB	102,65,15,56,221,246
	DB	102,65,15,56,221,255
	movdqa	xmm14,XMMWORD[48+rsp]
	movdqa	xmm15,XMMWORD[64+rsp]
	DB	102,68,15,56,221,193
	movdqa	xmm0,XMMWORD[80+rsp]
	movups	xmm1,XMMWORD[((16-128))+rcx]
	DB	102,69,15,56,221,202

	movups	XMMWORD[rsi],xmm2
	movdqa	xmm2,xmm11
	movups	XMMWORD[16+rsi],xmm3
	movdqa	xmm3,xmm12
	movups	XMMWORD[32+rsi],xmm4
	movdqa	xmm4,xmm13
	movups	XMMWORD[48+rsi],xmm5
	movdqa	xmm5,xmm14
	movups	XMMWORD[64+rsi],xmm6
	movdqa	xmm6,xmm15
	movups	XMMWORD[80+rsi],xmm7
	movdqa	xmm7,xmm0
	movups	XMMWORD[96+rsi],xmm8
	movups	XMMWORD[112+rsi],xmm9
	lea	rsi,[128+rsi]

	sub	rdx,8
	jnc	NEAR $L$ctr32_loop8

	add	rdx,8
	jz	NEAR $L$ctr32_done
	lea	rcx,[((-128))+rcx]

$L$ctr32_tail:


	lea	rcx,[16+rcx]
	cmp	rdx,4
	jb	NEAR $L$ctr32_loop3
	je	NEAR $L$ctr32_loop4


	shl	eax,4
	movdqa	xmm8,XMMWORD[96+rsp]
	pxor	xmm9,xmm9

	movups	xmm0,XMMWORD[16+rcx]
	DB	102,15,56,220,209
	DB	102,15,56,220,217
	lea	rcx,[((32-16))+rax*1+rcx]
	neg	rax
	DB	102,15,56,220,225
	add	rax,16
	movups	xmm10,XMMWORD[rdi]
	DB	102,15,56,220,233
	DB	102,15,56,220,241
	movups	xmm11,XMMWORD[16+rdi]
	movups	xmm12,XMMWORD[32+rdi]
	DB	102,15,56,220,249
	DB	102,68,15,56,220,193

	call	$L$enc_loop8_enter

	movdqu	xmm13,XMMWORD[48+rdi]
	pxor	xmm2,xmm10
	movdqu	xmm10,XMMWORD[64+rdi]
	pxor	xmm3,xmm11
	movdqu	XMMWORD[rsi],xmm2
	pxor	xmm4,xmm12
	movdqu	XMMWORD[16+rsi],xmm3
	pxor	xmm5,xmm13
	movdqu	XMMWORD[32+rsi],xmm4
	pxor	xmm6,xmm10
	movdqu	XMMWORD[48+rsi],xmm5
	movdqu	XMMWORD[64+rsi],xmm6
	cmp	rdx,6
	jb	NEAR $L$ctr32_done

	movups	xmm11,XMMWORD[80+rdi]
	xorps	xmm7,xmm11
	movups	XMMWORD[80+rsi],xmm7
	je	NEAR $L$ctr32_done

	movups	xmm12,XMMWORD[96+rdi]
	xorps	xmm8,xmm12
	movups	XMMWORD[96+rsi],xmm8
	jmp	NEAR $L$ctr32_done

ALIGN	32
$L$ctr32_loop4:
	DB	102,15,56,220,209
	lea	rcx,[16+rcx]
	dec	eax
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	DB	102,15,56,220,233
	movups	xmm1,XMMWORD[rcx]
	jnz	NEAR $L$ctr32_loop4
	DB	102,15,56,221,209
	DB	102,15,56,221,217
	movups	xmm10,XMMWORD[rdi]
	movups	xmm11,XMMWORD[16+rdi]
	DB	102,15,56,221,225
	DB	102,15,56,221,233
	movups	xmm12,XMMWORD[32+rdi]
	movups	xmm13,XMMWORD[48+rdi]

	xorps	xmm2,xmm10
	movups	XMMWORD[rsi],xmm2
	xorps	xmm3,xmm11
	movups	XMMWORD[16+rsi],xmm3
	pxor	xmm4,xmm12
	movdqu	XMMWORD[32+rsi],xmm4
	pxor	xmm5,xmm13
	movdqu	XMMWORD[48+rsi],xmm5
	jmp	NEAR $L$ctr32_done

ALIGN	32
$L$ctr32_loop3:
	DB	102,15,56,220,209
	lea	rcx,[16+rcx]
	dec	eax
	DB	102,15,56,220,217
	DB	102,15,56,220,225
	movups	xmm1,XMMWORD[rcx]
	jnz	NEAR $L$ctr32_loop3
	DB	102,15,56,221,209
	DB	102,15,56,221,217
	DB	102,15,56,221,225

	movups	xmm10,XMMWORD[rdi]
	xorps	xmm2,xmm10
	movups	XMMWORD[rsi],xmm2
	cmp	rdx,2
	jb	NEAR $L$ctr32_done

	movups	xmm11,XMMWORD[16+rdi]
	xorps	xmm3,xmm11
	movups	XMMWORD[16+rsi],xmm3
	je	NEAR $L$ctr32_done

	movups	xmm12,XMMWORD[32+rdi]
	xorps	xmm4,xmm12
	movups	XMMWORD[32+rsi],xmm4

$L$ctr32_done:
	xorps	xmm0,xmm0
	xor	ebp,ebp
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	movaps	xmm6,XMMWORD[((-168))+r11]
	movaps	XMMWORD[(-168)+r11],xmm0
	movaps	xmm7,XMMWORD[((-152))+r11]
	movaps	XMMWORD[(-152)+r11],xmm0
	movaps	xmm8,XMMWORD[((-136))+r11]
	movaps	XMMWORD[(-136)+r11],xmm0
	movaps	xmm9,XMMWORD[((-120))+r11]
	movaps	XMMWORD[(-120)+r11],xmm0
	movaps	xmm10,XMMWORD[((-104))+r11]
	movaps	XMMWORD[(-104)+r11],xmm0
	movaps	xmm11,XMMWORD[((-88))+r11]
	movaps	XMMWORD[(-88)+r11],xmm0
	movaps	xmm12,XMMWORD[((-72))+r11]
	movaps	XMMWORD[(-72)+r11],xmm0
	movaps	xmm13,XMMWORD[((-56))+r11]
	movaps	XMMWORD[(-56)+r11],xmm0
	movaps	xmm14,XMMWORD[((-40))+r11]
	movaps	XMMWORD[(-40)+r11],xmm0
	movaps	xmm15,XMMWORD[((-24))+r11]
	movaps	XMMWORD[(-24)+r11],xmm0
	movaps	XMMWORD[rsp],xmm0
	movaps	XMMWORD[16+rsp],xmm0
	movaps	XMMWORD[32+rsp],xmm0
	movaps	XMMWORD[48+rsp],xmm0
	movaps	XMMWORD[64+rsp],xmm0
	movaps	XMMWORD[80+rsp],xmm0
	movaps	XMMWORD[96+rsp],xmm0
	movaps	XMMWORD[112+rsp],xmm0
	mov	rbp,QWORD[((-8))+r11]

	lea	rsp,[r11]

$L$ctr32_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_aes_hw_ctr32_encrypt_blocks:
global	aes_hw_set_encrypt_key_base

ALIGN	16
aes_hw_set_encrypt_key_base:

$L$SEH_begin_aes_hw_set_encrypt_key_base_1:
_CET_ENDBR
%ifdef BORINGSSL_DISPATCH_TEST
	mov	BYTE[((BORINGSSL_function_hit+3))],1
%endif
	sub	rsp,8

$L$SEH_prologue_aes_hw_set_encrypt_key_base_2:
$L$SEH_endprologue_aes_hw_set_encrypt_key_base_3:
	movups	xmm0,XMMWORD[rcx]
	xorps	xmm4,xmm4
	lea	rax,[16+r8]
	cmp	edx,256
	je	NEAR $L$14rounds


	cmp	edx,128
	jne	NEAR $L$bad_keybits

$L$10rounds:
	mov	edx,9

	movups	XMMWORD[r8],xmm0
	DB	102,15,58,223,200,1
	call	$L$key_expansion_128_cold
	DB	102,15,58,223,200,2
	call	$L$key_expansion_128
	DB	102,15,58,223,200,4
	call	$L$key_expansion_128
	DB	102,15,58,223,200,8
	call	$L$key_expansion_128
	DB	102,15,58,223,200,16
	call	$L$key_expansion_128
	DB	102,15,58,223,200,32
	call	$L$key_expansion_128
	DB	102,15,58,223,200,64
	call	$L$key_expansion_128
	DB	102,15,58,223,200,128
	call	$L$key_expansion_128
	DB	102,15,58,223,200,27
	call	$L$key_expansion_128
	DB	102,15,58,223,200,54
	call	$L$key_expansion_128
	movups	XMMWORD[rax],xmm0
	mov	DWORD[80+rax],edx
	xor	eax,eax
	jmp	NEAR $L$enc_key_ret



ALIGN	16
$L$14rounds:
	movups	xmm2,XMMWORD[16+rcx]
	mov	edx,13
	lea	rax,[16+rax]

	movups	XMMWORD[r8],xmm0
	movups	XMMWORD[16+r8],xmm2
	DB	102,15,58,223,202,1
	call	$L$key_expansion_256a_cold
	DB	102,15,58,223,200,1
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,2
	call	$L$key_expansion_256a
	DB	102,15,58,223,200,2
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,4
	call	$L$key_expansion_256a
	DB	102,15,58,223,200,4
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,8
	call	$L$key_expansion_256a
	DB	102,15,58,223,200,8
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,16
	call	$L$key_expansion_256a
	DB	102,15,58,223,200,16
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,32
	call	$L$key_expansion_256a
	DB	102,15,58,223,200,32
	call	$L$key_expansion_256b
	DB	102,15,58,223,202,64
	call	$L$key_expansion_256a
	movups	XMMWORD[rax],xmm0
	mov	DWORD[16+rax],edx
	xor	rax,rax
	jmp	NEAR $L$enc_key_ret

ALIGN	16
$L$bad_keybits:
	mov	rax,-2
$L$enc_key_ret:
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	add	rsp,8

	ret

$L$SEH_end_aes_hw_set_encrypt_key_base_4:

ALIGN	16
$L$key_expansion_128:

	movups	XMMWORD[rax],xmm0
	lea	rax,[16+rax]
$L$key_expansion_128_cold:
	shufps	xmm4,xmm0,16
	xorps	xmm0,xmm4
	shufps	xmm4,xmm0,140
	xorps	xmm0,xmm4
	shufps	xmm1,xmm1,255
	xorps	xmm0,xmm1
	ret


ALIGN	16
$L$key_expansion_256a:

	movups	XMMWORD[rax],xmm2
	lea	rax,[16+rax]
$L$key_expansion_256a_cold:
	shufps	xmm4,xmm0,16
	xorps	xmm0,xmm4
	shufps	xmm4,xmm0,140
	xorps	xmm0,xmm4
	shufps	xmm1,xmm1,255
	xorps	xmm0,xmm1
	ret


ALIGN	16
$L$key_expansion_256b:

	movups	XMMWORD[rax],xmm0
	lea	rax,[16+rax]

	shufps	xmm4,xmm2,16
	xorps	xmm2,xmm4
	shufps	xmm4,xmm2,140
	xorps	xmm2,xmm4
	shufps	xmm1,xmm1,170
	xorps	xmm2,xmm1
	ret



global	aes_hw_set_encrypt_key_alt

ALIGN	16
aes_hw_set_encrypt_key_alt:

$L$SEH_begin_aes_hw_set_encrypt_key_alt_1:
_CET_ENDBR
%ifdef BORINGSSL_DISPATCH_TEST
	mov	BYTE[((BORINGSSL_function_hit+3))],1
%endif
	sub	rsp,8

$L$SEH_prologue_aes_hw_set_encrypt_key_alt_2:
$L$SEH_endprologue_aes_hw_set_encrypt_key_alt_3:
	movups	xmm0,XMMWORD[rcx]
	xorps	xmm4,xmm4
	lea	rax,[16+r8]
	cmp	edx,256
	je	NEAR $L$14rounds_alt

	cmp	edx,128
	jne	NEAR $L$bad_keybits_alt

	mov	edx,9
	movdqa	xmm5,XMMWORD[$L$key_rotate]
	mov	r10d,8
	movdqa	xmm4,XMMWORD[$L$key_rcon1]
	movdqa	xmm2,xmm0
	movdqu	XMMWORD[r8],xmm0
	jmp	NEAR $L$oop_key128

ALIGN	16
$L$oop_key128:
DB	102,15,56,0,197
	DB	102,15,56,221,196
	pslld	xmm4,1
	lea	rax,[16+rax]

	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3

	pxor	xmm0,xmm2
	movdqu	XMMWORD[(-16)+rax],xmm0
	movdqa	xmm2,xmm0

	dec	r10d
	jnz	NEAR $L$oop_key128

	movdqa	xmm4,XMMWORD[$L$key_rcon1b]

DB	102,15,56,0,197
	DB	102,15,56,221,196
	pslld	xmm4,1

	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3

	pxor	xmm0,xmm2
	movdqu	XMMWORD[rax],xmm0

	movdqa	xmm2,xmm0
DB	102,15,56,0,197
	DB	102,15,56,221,196

	movdqa	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm3,xmm2
	pslldq	xmm2,4
	pxor	xmm2,xmm3

	pxor	xmm0,xmm2
	movdqu	XMMWORD[16+rax],xmm0

	mov	DWORD[96+rax],edx
	xor	eax,eax
	jmp	NEAR $L$enc_key_ret_alt



ALIGN	16
$L$14rounds_alt:
	movups	xmm2,XMMWORD[16+rcx]
	mov	edx,13
	lea	rax,[16+rax]
	movdqa	xmm5,XMMWORD[$L$key_rotate]
	movdqa	xmm4,XMMWORD[$L$key_rcon1]
	mov	r10d,7
	movdqu	XMMWORD[r8],xmm0
	movdqa	xmm1,xmm2
	movdqu	XMMWORD[16+r8],xmm2
	jmp	NEAR $L$oop_key256

ALIGN	16
$L$oop_key256:
DB	102,15,56,0,213
	DB	102,15,56,221,212

	movdqa	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm3,xmm0
	pslldq	xmm0,4
	pxor	xmm0,xmm3
	pslld	xmm4,1

	pxor	xmm0,xmm2
	movdqu	XMMWORD[rax],xmm0

	dec	r10d
	jz	NEAR $L$done_key256

	pshufd	xmm2,xmm0,0xff
	pxor	xmm3,xmm3
	DB	102,15,56,221,211

	movdqa	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm3,xmm1
	pslldq	xmm1,4
	pxor	xmm1,xmm3

	pxor	xmm2,xmm1
	movdqu	XMMWORD[16+rax],xmm2
	lea	rax,[32+rax]
	movdqa	xmm1,xmm2

	jmp	NEAR $L$oop_key256

$L$done_key256:
	mov	DWORD[16+rax],edx
	xor	eax,eax
	jmp	NEAR $L$enc_key_ret_alt

ALIGN	16
$L$bad_keybits_alt:
	mov	rax,-2
$L$enc_key_ret_alt:
	pxor	xmm0,xmm0
	pxor	xmm1,xmm1
	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	add	rsp,8

	ret

$L$SEH_end_aes_hw_set_encrypt_key_alt_4:

section	.rdata rdata align=8
ALIGN	64
$L$bswap_mask:
	DB	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
$L$increment32:
	DD	6,6,6,0
$L$increment64:
	DD	1,0,0,0
$L$increment1:
	DB	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1
$L$key_rotate:
	DD	0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d
$L$key_rotate192:
	DD	0x04070605,0x04070605,0x04070605,0x04070605
$L$key_rcon1:
	DD	1,1,1,1
$L$key_rcon1b:
	DD	0x1b,0x1b,0x1b,0x1b

	DB	65,69,83,32,102,111,114,32,73,110,116,101,108,32,65,69
	DB	83,45,78,73,44,32,67,82,89,80,84,79,71,65,77,83
	DB	32,98,121,32,60,97,112,112,114,111,64,111,112,101,110,115
	DB	115,108,46,111,114,103,62,0
ALIGN	64
section	.text

EXTERN	__imp_RtlVirtualUnwind

ALIGN	16
ctr_xts_se_handler:
	push	rsi
	push	rdi
	push	rbx
	push	rbp
	push	r12
	push	r13
	push	r14
	push	r15
	pushfq
	sub	rsp,64

	mov	rax,QWORD[120+r8]
	mov	rbx,QWORD[248+r8]

	mov	rsi,QWORD[8+r9]
	mov	r11,QWORD[56+r9]

	mov	r10d,DWORD[r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jb	NEAR $L$common_seh_tail

	mov	rax,QWORD[152+r8]

	mov	r10d,DWORD[4+r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jae	NEAR $L$common_seh_tail

	mov	rax,QWORD[208+r8]

	lea	rsi,[((-168))+rax]
	lea	rdi,[512+r8]
	mov	ecx,20
	DD	0xa548f3fc

	mov	rbp,QWORD[((-8))+rax]
	mov	QWORD[160+r8],rbp


$L$common_seh_tail:
	mov	rdi,QWORD[8+rax]
	mov	rsi,QWORD[16+rax]
	mov	QWORD[152+r8],rax
	mov	QWORD[168+r8],rsi
	mov	QWORD[176+r8],rdi

	mov	rdi,QWORD[40+r9]
	mov	rsi,r8
	mov	ecx,154
	DD	0xa548f3fc

	mov	rsi,r9
	xor	rcx,rcx
	mov	rdx,QWORD[8+rsi]
	mov	r8,QWORD[rsi]
	mov	r9,QWORD[16+rsi]
	mov	r10,QWORD[40+rsi]
	lea	r11,[56+rsi]
	lea	r12,[24+rsi]
	mov	QWORD[32+rsp],r10
	mov	QWORD[40+rsp],r11
	mov	QWORD[48+rsp],r12
	mov	QWORD[56+rsp],rcx
	call	QWORD[__imp_RtlVirtualUnwind]

	mov	eax,1
	add	rsp,64
	popfq
	pop	r15
	pop	r14
	pop	r13
	pop	r12
	pop	rbp
	pop	rbx
	pop	rdi
	pop	rsi
	ret


section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_aes_hw_ctr32_encrypt_blocks wrt ..imagebase
	DD	$L$SEH_end_aes_hw_ctr32_encrypt_blocks wrt ..imagebase
	DD	$L$SEH_info_ctr32 wrt ..imagebase
section	.xdata rdata align=8
ALIGN	8
$L$SEH_info_ctr32:
	DB	9,0,0,0
	DD	ctr_xts_se_handler wrt ..imagebase
	DD	$L$ctr32_body wrt ..imagebase,$L$ctr32_epilogue wrt ..imagebase
section	.pdata
ALIGN	4
	DD	$L$SEH_begin_aes_hw_set_encrypt_key_base_1 wrt ..imagebase
	DD	$L$SEH_end_aes_hw_set_encrypt_key_base_4 wrt ..imagebase
	DD	$L$SEH_info_aes_hw_set_encrypt_key_base_0 wrt ..imagebase

	DD	$L$SEH_begin_aes_hw_set_encrypt_key_alt_1 wrt ..imagebase
	DD	$L$SEH_end_aes_hw_set_encrypt_key_alt_4 wrt ..imagebase
	DD	$L$SEH_info_aes_hw_set_encrypt_key_alt_0 wrt ..imagebase


section	.xdata
ALIGN	4
$L$SEH_info_aes_hw_set_encrypt_key_base_0:
	DB	1
	DB	$L$SEH_endprologue_aes_hw_set_encrypt_key_base_3-$L$SEH_begin_aes_hw_set_encrypt_key_base_1
	DB	1
	DB	0
	DB	$L$SEH_prologue_aes_hw_set_encrypt_key_base_2-$L$SEH_begin_aes_hw_set_encrypt_key_base_1
	DB	2

	DW	0
$L$SEH_info_aes_hw_set_encrypt_key_alt_0:
	DB	1
	DB	$L$SEH_endprologue_aes_hw_set_encrypt_key_alt_3-$L$SEH_begin_aes_hw_set_encrypt_key_alt_1
	DB	1
	DB	0
	DB	$L$SEH_prologue_aes_hw_set_encrypt_key_alt_2-$L$SEH_begin_aes_hw_set_encrypt_key_alt_1
	DB	2

	DW	0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
