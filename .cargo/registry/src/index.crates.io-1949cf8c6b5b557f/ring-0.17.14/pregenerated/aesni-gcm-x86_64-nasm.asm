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



ALIGN	32
_aesni_ctr32_ghash_6x:

	vmovdqu	xmm2,XMMWORD[32+r11]
	sub	r8,6
	vpxor	xmm4,xmm4,xmm4
	vmovdqu	xmm15,XMMWORD[((0-128))+r9]
	vpaddb	xmm10,xmm1,xmm2
	vpaddb	xmm11,xmm10,xmm2
	vpaddb	xmm12,xmm11,xmm2
	vpaddb	xmm13,xmm12,xmm2
	vpaddb	xmm14,xmm13,xmm2
	vpxor	xmm9,xmm1,xmm15
	vmovdqu	XMMWORD[(16+8)+rsp],xmm4
	jmp	NEAR $L$oop6x

ALIGN	32
$L$oop6x:
	add	ebx,100663296
	jc	NEAR $L$handle_ctr32
	vmovdqu	xmm3,XMMWORD[((0-32))+rsi]
	vpaddb	xmm1,xmm14,xmm2
	vpxor	xmm10,xmm10,xmm15
	vpxor	xmm11,xmm11,xmm15

$L$resume_ctr32:
	vmovdqu	XMMWORD[rdi],xmm1
	vpclmulqdq	xmm5,xmm7,xmm3,0x10
	vpxor	xmm12,xmm12,xmm15
	vmovups	xmm2,XMMWORD[((16-128))+r9]
	vpclmulqdq	xmm6,xmm7,xmm3,0x01

















	xor	r12,r12
	cmp	r15,r14

	vaesenc	xmm9,xmm9,xmm2
	vmovdqu	xmm0,XMMWORD[((48+8))+rsp]
	vpxor	xmm13,xmm13,xmm15
	vpclmulqdq	xmm1,xmm7,xmm3,0x00
	vaesenc	xmm10,xmm10,xmm2
	vpxor	xmm14,xmm14,xmm15
	setnc	r12b
	vpclmulqdq	xmm7,xmm7,xmm3,0x11
	vaesenc	xmm11,xmm11,xmm2
	vmovdqu	xmm3,XMMWORD[((16-32))+rsi]
	neg	r12
	vaesenc	xmm12,xmm12,xmm2
	vpxor	xmm6,xmm6,xmm5
	vpclmulqdq	xmm5,xmm0,xmm3,0x00
	vpxor	xmm8,xmm8,xmm4
	vaesenc	xmm13,xmm13,xmm2
	vpxor	xmm4,xmm1,xmm5
	and	r12,0x60
	vmovups	xmm15,XMMWORD[((32-128))+r9]
	vpclmulqdq	xmm1,xmm0,xmm3,0x10
	vaesenc	xmm14,xmm14,xmm2

	vpclmulqdq	xmm2,xmm0,xmm3,0x01
	lea	r14,[r12*1+r14]
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm8,xmm8,XMMWORD[((16+8))+rsp]
	vpclmulqdq	xmm3,xmm0,xmm3,0x11
	vmovdqu	xmm0,XMMWORD[((64+8))+rsp]
	vaesenc	xmm10,xmm10,xmm15
	movbe	r13,QWORD[88+r14]
	vaesenc	xmm11,xmm11,xmm15
	movbe	r12,QWORD[80+r14]
	vaesenc	xmm12,xmm12,xmm15
	mov	QWORD[((32+8))+rsp],r13
	vaesenc	xmm13,xmm13,xmm15
	mov	QWORD[((40+8))+rsp],r12
	vmovdqu	xmm5,XMMWORD[((48-32))+rsi]
	vaesenc	xmm14,xmm14,xmm15

	vmovups	xmm15,XMMWORD[((48-128))+r9]
	vpxor	xmm6,xmm6,xmm1
	vpclmulqdq	xmm1,xmm0,xmm5,0x00
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm6,xmm6,xmm2
	vpclmulqdq	xmm2,xmm0,xmm5,0x10
	vaesenc	xmm10,xmm10,xmm15
	vpxor	xmm7,xmm7,xmm3
	vpclmulqdq	xmm3,xmm0,xmm5,0x01
	vaesenc	xmm11,xmm11,xmm15
	vpclmulqdq	xmm5,xmm0,xmm5,0x11
	vmovdqu	xmm0,XMMWORD[((80+8))+rsp]
	vaesenc	xmm12,xmm12,xmm15
	vaesenc	xmm13,xmm13,xmm15
	vpxor	xmm4,xmm4,xmm1
	vmovdqu	xmm1,XMMWORD[((64-32))+rsi]
	vaesenc	xmm14,xmm14,xmm15

	vmovups	xmm15,XMMWORD[((64-128))+r9]
	vpxor	xmm6,xmm6,xmm2
	vpclmulqdq	xmm2,xmm0,xmm1,0x00
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm6,xmm6,xmm3
	vpclmulqdq	xmm3,xmm0,xmm1,0x10
	vaesenc	xmm10,xmm10,xmm15
	movbe	r13,QWORD[72+r14]
	vpxor	xmm7,xmm7,xmm5
	vpclmulqdq	xmm5,xmm0,xmm1,0x01
	vaesenc	xmm11,xmm11,xmm15
	movbe	r12,QWORD[64+r14]
	vpclmulqdq	xmm1,xmm0,xmm1,0x11
	vmovdqu	xmm0,XMMWORD[((96+8))+rsp]
	vaesenc	xmm12,xmm12,xmm15
	mov	QWORD[((48+8))+rsp],r13
	vaesenc	xmm13,xmm13,xmm15
	mov	QWORD[((56+8))+rsp],r12
	vpxor	xmm4,xmm4,xmm2
	vmovdqu	xmm2,XMMWORD[((96-32))+rsi]
	vaesenc	xmm14,xmm14,xmm15

	vmovups	xmm15,XMMWORD[((80-128))+r9]
	vpxor	xmm6,xmm6,xmm3
	vpclmulqdq	xmm3,xmm0,xmm2,0x00
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm6,xmm6,xmm5
	vpclmulqdq	xmm5,xmm0,xmm2,0x10
	vaesenc	xmm10,xmm10,xmm15
	movbe	r13,QWORD[56+r14]
	vpxor	xmm7,xmm7,xmm1
	vpclmulqdq	xmm1,xmm0,xmm2,0x01
	vpxor	xmm8,xmm8,XMMWORD[((112+8))+rsp]
	vaesenc	xmm11,xmm11,xmm15
	movbe	r12,QWORD[48+r14]
	vpclmulqdq	xmm2,xmm0,xmm2,0x11
	vaesenc	xmm12,xmm12,xmm15
	mov	QWORD[((64+8))+rsp],r13
	vaesenc	xmm13,xmm13,xmm15
	mov	QWORD[((72+8))+rsp],r12
	vpxor	xmm4,xmm4,xmm3
	vmovdqu	xmm3,XMMWORD[((112-32))+rsi]
	vaesenc	xmm14,xmm14,xmm15

	vmovups	xmm15,XMMWORD[((96-128))+r9]
	vpxor	xmm6,xmm6,xmm5
	vpclmulqdq	xmm5,xmm8,xmm3,0x10
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm6,xmm6,xmm1
	vpclmulqdq	xmm1,xmm8,xmm3,0x01
	vaesenc	xmm10,xmm10,xmm15
	movbe	r13,QWORD[40+r14]
	vpxor	xmm7,xmm7,xmm2
	vpclmulqdq	xmm2,xmm8,xmm3,0x00
	vaesenc	xmm11,xmm11,xmm15
	movbe	r12,QWORD[32+r14]
	vpclmulqdq	xmm8,xmm8,xmm3,0x11
	vaesenc	xmm12,xmm12,xmm15
	mov	QWORD[((80+8))+rsp],r13
	vaesenc	xmm13,xmm13,xmm15
	mov	QWORD[((88+8))+rsp],r12
	vpxor	xmm6,xmm6,xmm5
	vaesenc	xmm14,xmm14,xmm15
	vpxor	xmm6,xmm6,xmm1

	vmovups	xmm15,XMMWORD[((112-128))+r9]
	vpslldq	xmm5,xmm6,8
	vpxor	xmm4,xmm4,xmm2
	vmovdqu	xmm3,XMMWORD[16+r11]

	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm7,xmm7,xmm8
	vaesenc	xmm10,xmm10,xmm15
	vpxor	xmm4,xmm4,xmm5
	movbe	r13,QWORD[24+r14]
	vaesenc	xmm11,xmm11,xmm15
	movbe	r12,QWORD[16+r14]
	vpalignr	xmm0,xmm4,xmm4,8
	vpclmulqdq	xmm4,xmm4,xmm3,0x10
	mov	QWORD[((96+8))+rsp],r13
	vaesenc	xmm12,xmm12,xmm15
	mov	QWORD[((104+8))+rsp],r12
	vaesenc	xmm13,xmm13,xmm15
	vmovups	xmm1,XMMWORD[((128-128))+r9]
	vaesenc	xmm14,xmm14,xmm15

	vaesenc	xmm9,xmm9,xmm1
	vmovups	xmm15,XMMWORD[((144-128))+r9]
	vaesenc	xmm10,xmm10,xmm1
	vpsrldq	xmm6,xmm6,8
	vaesenc	xmm11,xmm11,xmm1
	vpxor	xmm7,xmm7,xmm6
	vaesenc	xmm12,xmm12,xmm1
	vpxor	xmm4,xmm4,xmm0
	movbe	r13,QWORD[8+r14]
	vaesenc	xmm13,xmm13,xmm1
	movbe	r12,QWORD[r14]
	vaesenc	xmm14,xmm14,xmm1
	vmovups	xmm1,XMMWORD[((160-128))+r9]
	cmp	r10d,11
	jb	NEAR $L$enc_tail

	vaesenc	xmm9,xmm9,xmm15
	vaesenc	xmm10,xmm10,xmm15
	vaesenc	xmm11,xmm11,xmm15
	vaesenc	xmm12,xmm12,xmm15
	vaesenc	xmm13,xmm13,xmm15
	vaesenc	xmm14,xmm14,xmm15

	vaesenc	xmm9,xmm9,xmm1
	vaesenc	xmm10,xmm10,xmm1
	vaesenc	xmm11,xmm11,xmm1
	vaesenc	xmm12,xmm12,xmm1
	vaesenc	xmm13,xmm13,xmm1
	vmovups	xmm15,XMMWORD[((176-128))+r9]
	vaesenc	xmm14,xmm14,xmm1
	vmovups	xmm1,XMMWORD[((192-128))+r9]


	vaesenc	xmm9,xmm9,xmm15
	vaesenc	xmm10,xmm10,xmm15
	vaesenc	xmm11,xmm11,xmm15
	vaesenc	xmm12,xmm12,xmm15
	vaesenc	xmm13,xmm13,xmm15
	vaesenc	xmm14,xmm14,xmm15

	vaesenc	xmm9,xmm9,xmm1
	vaesenc	xmm10,xmm10,xmm1
	vaesenc	xmm11,xmm11,xmm1
	vaesenc	xmm12,xmm12,xmm1
	vaesenc	xmm13,xmm13,xmm1
	vmovups	xmm15,XMMWORD[((208-128))+r9]
	vaesenc	xmm14,xmm14,xmm1
	vmovups	xmm1,XMMWORD[((224-128))+r9]
	jmp	NEAR $L$enc_tail

ALIGN	32
$L$handle_ctr32:
	vmovdqu	xmm0,XMMWORD[r11]
	vpshufb	xmm6,xmm1,xmm0
	vmovdqu	xmm5,XMMWORD[48+r11]
	vpaddd	xmm10,xmm6,XMMWORD[64+r11]
	vpaddd	xmm11,xmm6,xmm5
	vmovdqu	xmm3,XMMWORD[((0-32))+rsi]
	vpaddd	xmm12,xmm10,xmm5
	vpshufb	xmm10,xmm10,xmm0
	vpaddd	xmm13,xmm11,xmm5
	vpshufb	xmm11,xmm11,xmm0
	vpxor	xmm10,xmm10,xmm15
	vpaddd	xmm14,xmm12,xmm5
	vpshufb	xmm12,xmm12,xmm0
	vpxor	xmm11,xmm11,xmm15
	vpaddd	xmm1,xmm13,xmm5
	vpshufb	xmm13,xmm13,xmm0
	vpshufb	xmm14,xmm14,xmm0
	vpshufb	xmm1,xmm1,xmm0
	jmp	NEAR $L$resume_ctr32

ALIGN	32
$L$enc_tail:
	vaesenc	xmm9,xmm9,xmm15
	vmovdqu	XMMWORD[(16+8)+rsp],xmm7
	vpalignr	xmm8,xmm4,xmm4,8
	vaesenc	xmm10,xmm10,xmm15
	vpclmulqdq	xmm4,xmm4,xmm3,0x10
	vpxor	xmm2,xmm1,XMMWORD[rcx]
	vaesenc	xmm11,xmm11,xmm15
	vpxor	xmm0,xmm1,XMMWORD[16+rcx]
	vaesenc	xmm12,xmm12,xmm15
	vpxor	xmm5,xmm1,XMMWORD[32+rcx]
	vaesenc	xmm13,xmm13,xmm15
	vpxor	xmm6,xmm1,XMMWORD[48+rcx]
	vaesenc	xmm14,xmm14,xmm15
	vpxor	xmm7,xmm1,XMMWORD[64+rcx]
	vpxor	xmm3,xmm1,XMMWORD[80+rcx]
	vmovdqu	xmm1,XMMWORD[rdi]

	vaesenclast	xmm9,xmm9,xmm2
	vmovdqu	xmm2,XMMWORD[32+r11]
	vaesenclast	xmm10,xmm10,xmm0
	vpaddb	xmm0,xmm1,xmm2
	mov	QWORD[((112+8))+rsp],r13
	lea	rcx,[96+rcx]

	prefetcht0	[512+rcx]
	prefetcht0	[576+rcx]
	vaesenclast	xmm11,xmm11,xmm5
	vpaddb	xmm5,xmm0,xmm2
	mov	QWORD[((120+8))+rsp],r12
	lea	rdx,[96+rdx]
	vmovdqu	xmm15,XMMWORD[((0-128))+r9]
	vaesenclast	xmm12,xmm12,xmm6
	vpaddb	xmm6,xmm5,xmm2
	vaesenclast	xmm13,xmm13,xmm7
	vpaddb	xmm7,xmm6,xmm2
	vaesenclast	xmm14,xmm14,xmm3
	vpaddb	xmm3,xmm7,xmm2

	add	rax,0x60
	sub	r8,0x6
	jc	NEAR $L$6x_done

	vmovups	XMMWORD[(-96)+rdx],xmm9
	vpxor	xmm9,xmm1,xmm15
	vmovups	XMMWORD[(-80)+rdx],xmm10
	vmovdqa	xmm10,xmm0
	vmovups	XMMWORD[(-64)+rdx],xmm11
	vmovdqa	xmm11,xmm5
	vmovups	XMMWORD[(-48)+rdx],xmm12
	vmovdqa	xmm12,xmm6
	vmovups	XMMWORD[(-32)+rdx],xmm13
	vmovdqa	xmm13,xmm7
	vmovups	XMMWORD[(-16)+rdx],xmm14
	vmovdqa	xmm14,xmm3
	vmovdqu	xmm7,XMMWORD[((32+8))+rsp]
	jmp	NEAR $L$oop6x

$L$6x_done:
	vpxor	xmm8,xmm8,XMMWORD[((16+8))+rsp]
	vpxor	xmm8,xmm8,xmm4

	ret


global	aesni_gcm_decrypt

ALIGN	32
aesni_gcm_decrypt:

$L$SEH_begin_aesni_gcm_decrypt_1:
_CET_ENDBR
	xor	rax,rax



	cmp	r8,0x60
	jb	NEAR $L$gcm_dec_abort

	push	rbp

$L$SEH_prologue_aesni_gcm_decrypt_2:
	mov	rbp,rsp

	push	rbx

$L$SEH_prologue_aesni_gcm_decrypt_3:
	push	r12

$L$SEH_prologue_aesni_gcm_decrypt_4:
	push	r13

$L$SEH_prologue_aesni_gcm_decrypt_5:
	push	r14

$L$SEH_prologue_aesni_gcm_decrypt_6:
	push	r15

$L$SEH_prologue_aesni_gcm_decrypt_7:
	lea	rsp,[((-168))+rsp]
$L$SEH_prologue_aesni_gcm_decrypt_8:
$L$SEH_prologue_aesni_gcm_decrypt_9:



	mov	QWORD[16+rbp],rdi
$L$SEH_prologue_aesni_gcm_decrypt_10:
	mov	QWORD[24+rbp],rsi
$L$SEH_prologue_aesni_gcm_decrypt_11:
	mov	rdi,QWORD[48+rbp]
	mov	rsi,QWORD[56+rbp]

	movaps	XMMWORD[(-208)+rbp],xmm6
$L$SEH_prologue_aesni_gcm_decrypt_12:
	movaps	XMMWORD[(-192)+rbp],xmm7
$L$SEH_prologue_aesni_gcm_decrypt_13:
	movaps	XMMWORD[(-176)+rbp],xmm8
$L$SEH_prologue_aesni_gcm_decrypt_14:
	movaps	XMMWORD[(-160)+rbp],xmm9
$L$SEH_prologue_aesni_gcm_decrypt_15:
	movaps	XMMWORD[(-144)+rbp],xmm10
$L$SEH_prologue_aesni_gcm_decrypt_16:
	movaps	XMMWORD[(-128)+rbp],xmm11
$L$SEH_prologue_aesni_gcm_decrypt_17:
	movaps	XMMWORD[(-112)+rbp],xmm12
$L$SEH_prologue_aesni_gcm_decrypt_18:
	movaps	XMMWORD[(-96)+rbp],xmm13
$L$SEH_prologue_aesni_gcm_decrypt_19:
	movaps	XMMWORD[(-80)+rbp],xmm14
$L$SEH_prologue_aesni_gcm_decrypt_20:
	movaps	XMMWORD[(-64)+rbp],xmm15
$L$SEH_prologue_aesni_gcm_decrypt_21:
$L$SEH_endprologue_aesni_gcm_decrypt_22:
	vzeroupper

	mov	r12,QWORD[64+rbp]
	vmovdqu	xmm1,XMMWORD[rdi]
	add	rsp,-128
	mov	ebx,DWORD[12+rdi]
	lea	r11,[$L$bswap_mask]
	lea	r14,[((-128))+r9]
	mov	r15,0xf80
	vmovdqu	xmm8,XMMWORD[r12]
	and	rsp,-128
	vmovdqu	xmm0,XMMWORD[r11]
	lea	r9,[128+r9]
	lea	rsi,[32+rsi]
	mov	r10d,DWORD[((240-128))+r9]
	vpshufb	xmm8,xmm8,xmm0

	and	r14,r15
	and	r15,rsp
	sub	r15,r14
	jc	NEAR $L$dec_no_key_aliasing
	cmp	r15,768
	jnc	NEAR $L$dec_no_key_aliasing
	sub	rsp,r15
$L$dec_no_key_aliasing:

	vmovdqu	xmm7,XMMWORD[80+rcx]
	mov	r14,rcx
	vmovdqu	xmm4,XMMWORD[64+rcx]







	lea	r15,[((-192))+r8*1+rcx]

	vmovdqu	xmm5,XMMWORD[48+rcx]
	shr	r8,4
	xor	rax,rax
	vmovdqu	xmm6,XMMWORD[32+rcx]
	vpshufb	xmm7,xmm7,xmm0
	vmovdqu	xmm2,XMMWORD[16+rcx]
	vpshufb	xmm4,xmm4,xmm0
	vmovdqu	xmm3,XMMWORD[rcx]
	vpshufb	xmm5,xmm5,xmm0
	vmovdqu	XMMWORD[48+rsp],xmm4
	vpshufb	xmm6,xmm6,xmm0
	vmovdqu	XMMWORD[64+rsp],xmm5
	vpshufb	xmm2,xmm2,xmm0
	vmovdqu	XMMWORD[80+rsp],xmm6
	vpshufb	xmm3,xmm3,xmm0
	vmovdqu	XMMWORD[96+rsp],xmm2
	vmovdqu	XMMWORD[112+rsp],xmm3

	call	_aesni_ctr32_ghash_6x

	mov	r12,QWORD[64+rbp]
	vmovups	XMMWORD[(-96)+rdx],xmm9
	vmovups	XMMWORD[(-80)+rdx],xmm10
	vmovups	XMMWORD[(-64)+rdx],xmm11
	vmovups	XMMWORD[(-48)+rdx],xmm12
	vmovups	XMMWORD[(-32)+rdx],xmm13
	vmovups	XMMWORD[(-16)+rdx],xmm14

	vpshufb	xmm8,xmm8,XMMWORD[r11]
	vmovdqu	XMMWORD[r12],xmm8

	vzeroupper
	movaps	xmm6,XMMWORD[((-208))+rbp]
	movaps	xmm7,XMMWORD[((-192))+rbp]
	movaps	xmm8,XMMWORD[((-176))+rbp]
	movaps	xmm9,XMMWORD[((-160))+rbp]
	movaps	xmm10,XMMWORD[((-144))+rbp]
	movaps	xmm11,XMMWORD[((-128))+rbp]
	movaps	xmm12,XMMWORD[((-112))+rbp]
	movaps	xmm13,XMMWORD[((-96))+rbp]
	movaps	xmm14,XMMWORD[((-80))+rbp]
	movaps	xmm15,XMMWORD[((-64))+rbp]
	mov	rdi,QWORD[16+rbp]
	mov	rsi,QWORD[24+rbp]
	lea	rsp,[((-40))+rbp]

	pop	r15

	pop	r14

	pop	r13

	pop	r12

	pop	rbx

	pop	rbp

$L$gcm_dec_abort:
	ret
$L$SEH_end_aesni_gcm_decrypt_23:



ALIGN	32
_aesni_ctr32_6x:

	vmovdqu	xmm4,XMMWORD[((0-128))+r9]
	vmovdqu	xmm2,XMMWORD[32+r11]
	lea	r13,[((-1))+r10]
	vmovups	xmm15,XMMWORD[((16-128))+r9]
	lea	r12,[((32-128))+r9]
	vpxor	xmm9,xmm1,xmm4
	add	ebx,100663296
	jc	NEAR $L$handle_ctr32_2
	vpaddb	xmm10,xmm1,xmm2
	vpaddb	xmm11,xmm10,xmm2
	vpxor	xmm10,xmm10,xmm4
	vpaddb	xmm12,xmm11,xmm2
	vpxor	xmm11,xmm11,xmm4
	vpaddb	xmm13,xmm12,xmm2
	vpxor	xmm12,xmm12,xmm4
	vpaddb	xmm14,xmm13,xmm2
	vpxor	xmm13,xmm13,xmm4
	vpaddb	xmm1,xmm14,xmm2
	vpxor	xmm14,xmm14,xmm4
	jmp	NEAR $L$oop_ctr32

ALIGN	16
$L$oop_ctr32:
	vaesenc	xmm9,xmm9,xmm15
	vaesenc	xmm10,xmm10,xmm15
	vaesenc	xmm11,xmm11,xmm15
	vaesenc	xmm12,xmm12,xmm15
	vaesenc	xmm13,xmm13,xmm15
	vaesenc	xmm14,xmm14,xmm15
	vmovups	xmm15,XMMWORD[r12]
	lea	r12,[16+r12]
	dec	r13d
	jnz	NEAR $L$oop_ctr32

	vmovdqu	xmm3,XMMWORD[r12]
	vaesenc	xmm9,xmm9,xmm15
	vpxor	xmm4,xmm3,XMMWORD[rcx]
	vaesenc	xmm10,xmm10,xmm15
	vpxor	xmm5,xmm3,XMMWORD[16+rcx]
	vaesenc	xmm11,xmm11,xmm15
	vpxor	xmm6,xmm3,XMMWORD[32+rcx]
	vaesenc	xmm12,xmm12,xmm15
	vpxor	xmm8,xmm3,XMMWORD[48+rcx]
	vaesenc	xmm13,xmm13,xmm15
	vpxor	xmm2,xmm3,XMMWORD[64+rcx]
	vaesenc	xmm14,xmm14,xmm15
	vpxor	xmm3,xmm3,XMMWORD[80+rcx]
	lea	rcx,[96+rcx]

	vaesenclast	xmm9,xmm9,xmm4
	vaesenclast	xmm10,xmm10,xmm5
	vaesenclast	xmm11,xmm11,xmm6
	vaesenclast	xmm12,xmm12,xmm8
	vaesenclast	xmm13,xmm13,xmm2
	vaesenclast	xmm14,xmm14,xmm3
	vmovups	XMMWORD[rdx],xmm9
	vmovups	XMMWORD[16+rdx],xmm10
	vmovups	XMMWORD[32+rdx],xmm11
	vmovups	XMMWORD[48+rdx],xmm12
	vmovups	XMMWORD[64+rdx],xmm13
	vmovups	XMMWORD[80+rdx],xmm14
	lea	rdx,[96+rdx]

	ret
ALIGN	32
$L$handle_ctr32_2:
	vpshufb	xmm6,xmm1,xmm0
	vmovdqu	xmm5,XMMWORD[48+r11]
	vpaddd	xmm10,xmm6,XMMWORD[64+r11]
	vpaddd	xmm11,xmm6,xmm5
	vpaddd	xmm12,xmm10,xmm5
	vpshufb	xmm10,xmm10,xmm0
	vpaddd	xmm13,xmm11,xmm5
	vpshufb	xmm11,xmm11,xmm0
	vpxor	xmm10,xmm10,xmm4
	vpaddd	xmm14,xmm12,xmm5
	vpshufb	xmm12,xmm12,xmm0
	vpxor	xmm11,xmm11,xmm4
	vpaddd	xmm1,xmm13,xmm5
	vpshufb	xmm13,xmm13,xmm0
	vpxor	xmm12,xmm12,xmm4
	vpshufb	xmm14,xmm14,xmm0
	vpxor	xmm13,xmm13,xmm4
	vpshufb	xmm1,xmm1,xmm0
	vpxor	xmm14,xmm14,xmm4
	jmp	NEAR $L$oop_ctr32



global	aesni_gcm_encrypt

ALIGN	32
aesni_gcm_encrypt:

$L$SEH_begin_aesni_gcm_encrypt_1:
_CET_ENDBR
%ifdef BORINGSSL_DISPATCH_TEST
EXTERN	BORINGSSL_function_hit
	mov	BYTE[((BORINGSSL_function_hit+2))],1
%endif
	xor	rax,rax




	cmp	r8,0x60*3
	jb	NEAR $L$gcm_enc_abort

	push	rbp

$L$SEH_prologue_aesni_gcm_encrypt_2:
	mov	rbp,rsp

	push	rbx

$L$SEH_prologue_aesni_gcm_encrypt_3:
	push	r12

$L$SEH_prologue_aesni_gcm_encrypt_4:
	push	r13

$L$SEH_prologue_aesni_gcm_encrypt_5:
	push	r14

$L$SEH_prologue_aesni_gcm_encrypt_6:
	push	r15

$L$SEH_prologue_aesni_gcm_encrypt_7:
	lea	rsp,[((-168))+rsp]
$L$SEH_prologue_aesni_gcm_encrypt_8:
$L$SEH_prologue_aesni_gcm_encrypt_9:



	mov	QWORD[16+rbp],rdi
$L$SEH_prologue_aesni_gcm_encrypt_10:
	mov	QWORD[24+rbp],rsi
$L$SEH_prologue_aesni_gcm_encrypt_11:
	mov	rdi,QWORD[48+rbp]
	mov	rsi,QWORD[56+rbp]

	movaps	XMMWORD[(-208)+rbp],xmm6
$L$SEH_prologue_aesni_gcm_encrypt_12:
	movaps	XMMWORD[(-192)+rbp],xmm7
$L$SEH_prologue_aesni_gcm_encrypt_13:
	movaps	XMMWORD[(-176)+rbp],xmm8
$L$SEH_prologue_aesni_gcm_encrypt_14:
	movaps	XMMWORD[(-160)+rbp],xmm9
$L$SEH_prologue_aesni_gcm_encrypt_15:
	movaps	XMMWORD[(-144)+rbp],xmm10
$L$SEH_prologue_aesni_gcm_encrypt_16:
	movaps	XMMWORD[(-128)+rbp],xmm11
$L$SEH_prologue_aesni_gcm_encrypt_17:
	movaps	XMMWORD[(-112)+rbp],xmm12
$L$SEH_prologue_aesni_gcm_encrypt_18:
	movaps	XMMWORD[(-96)+rbp],xmm13
$L$SEH_prologue_aesni_gcm_encrypt_19:
	movaps	XMMWORD[(-80)+rbp],xmm14
$L$SEH_prologue_aesni_gcm_encrypt_20:
	movaps	XMMWORD[(-64)+rbp],xmm15
$L$SEH_prologue_aesni_gcm_encrypt_21:
$L$SEH_endprologue_aesni_gcm_encrypt_22:
	vzeroupper

	vmovdqu	xmm1,XMMWORD[rdi]
	add	rsp,-128
	mov	ebx,DWORD[12+rdi]
	lea	r11,[$L$bswap_mask]
	lea	r14,[((-128))+r9]
	mov	r15,0xf80
	lea	r9,[128+r9]
	vmovdqu	xmm0,XMMWORD[r11]
	and	rsp,-128
	mov	r10d,DWORD[((240-128))+r9]

	and	r14,r15
	and	r15,rsp
	sub	r15,r14
	jc	NEAR $L$enc_no_key_aliasing
	cmp	r15,768
	jnc	NEAR $L$enc_no_key_aliasing
	sub	rsp,r15
$L$enc_no_key_aliasing:

	mov	r14,rdx








	lea	r15,[((-192))+r8*1+rdx]

	shr	r8,4

	call	_aesni_ctr32_6x
	vpshufb	xmm8,xmm9,xmm0
	vpshufb	xmm2,xmm10,xmm0
	vmovdqu	XMMWORD[112+rsp],xmm8
	vpshufb	xmm4,xmm11,xmm0
	vmovdqu	XMMWORD[96+rsp],xmm2
	vpshufb	xmm5,xmm12,xmm0
	vmovdqu	XMMWORD[80+rsp],xmm4
	vpshufb	xmm6,xmm13,xmm0
	vmovdqu	XMMWORD[64+rsp],xmm5
	vpshufb	xmm7,xmm14,xmm0
	vmovdqu	XMMWORD[48+rsp],xmm6

	call	_aesni_ctr32_6x

	mov	r12,QWORD[64+rbp]
	lea	rsi,[32+rsi]
	vmovdqu	xmm8,XMMWORD[r12]
	sub	r8,12
	mov	rax,0x60*2
	vpshufb	xmm8,xmm8,xmm0

	call	_aesni_ctr32_ghash_6x
	vmovdqu	xmm7,XMMWORD[32+rsp]
	vmovdqu	xmm0,XMMWORD[r11]
	vmovdqu	xmm3,XMMWORD[((0-32))+rsi]
	vpunpckhqdq	xmm1,xmm7,xmm7
	vmovdqu	xmm15,XMMWORD[((32-32))+rsi]
	vmovups	XMMWORD[(-96)+rdx],xmm9
	vpshufb	xmm9,xmm9,xmm0
	vpxor	xmm1,xmm1,xmm7
	vmovups	XMMWORD[(-80)+rdx],xmm10
	vpshufb	xmm10,xmm10,xmm0
	vmovups	XMMWORD[(-64)+rdx],xmm11
	vpshufb	xmm11,xmm11,xmm0
	vmovups	XMMWORD[(-48)+rdx],xmm12
	vpshufb	xmm12,xmm12,xmm0
	vmovups	XMMWORD[(-32)+rdx],xmm13
	vpshufb	xmm13,xmm13,xmm0
	vmovups	XMMWORD[(-16)+rdx],xmm14
	vpshufb	xmm14,xmm14,xmm0
	vmovdqu	XMMWORD[16+rsp],xmm9
	vmovdqu	xmm6,XMMWORD[48+rsp]
	vmovdqu	xmm0,XMMWORD[((16-32))+rsi]
	vpunpckhqdq	xmm2,xmm6,xmm6
	vpclmulqdq	xmm5,xmm7,xmm3,0x00
	vpxor	xmm2,xmm2,xmm6
	vpclmulqdq	xmm7,xmm7,xmm3,0x11
	vpclmulqdq	xmm1,xmm1,xmm15,0x00

	vmovdqu	xmm9,XMMWORD[64+rsp]
	vpclmulqdq	xmm4,xmm6,xmm0,0x00
	vmovdqu	xmm3,XMMWORD[((48-32))+rsi]
	vpxor	xmm4,xmm4,xmm5
	vpunpckhqdq	xmm5,xmm9,xmm9
	vpclmulqdq	xmm6,xmm6,xmm0,0x11
	vpxor	xmm5,xmm5,xmm9
	vpxor	xmm6,xmm6,xmm7
	vpclmulqdq	xmm2,xmm2,xmm15,0x10
	vmovdqu	xmm15,XMMWORD[((80-32))+rsi]
	vpxor	xmm2,xmm2,xmm1

	vmovdqu	xmm1,XMMWORD[80+rsp]
	vpclmulqdq	xmm7,xmm9,xmm3,0x00
	vmovdqu	xmm0,XMMWORD[((64-32))+rsi]
	vpxor	xmm7,xmm7,xmm4
	vpunpckhqdq	xmm4,xmm1,xmm1
	vpclmulqdq	xmm9,xmm9,xmm3,0x11
	vpxor	xmm4,xmm4,xmm1
	vpxor	xmm9,xmm9,xmm6
	vpclmulqdq	xmm5,xmm5,xmm15,0x00
	vpxor	xmm5,xmm5,xmm2

	vmovdqu	xmm2,XMMWORD[96+rsp]
	vpclmulqdq	xmm6,xmm1,xmm0,0x00
	vmovdqu	xmm3,XMMWORD[((96-32))+rsi]
	vpxor	xmm6,xmm6,xmm7
	vpunpckhqdq	xmm7,xmm2,xmm2
	vpclmulqdq	xmm1,xmm1,xmm0,0x11
	vpxor	xmm7,xmm7,xmm2
	vpxor	xmm1,xmm1,xmm9
	vpclmulqdq	xmm4,xmm4,xmm15,0x10
	vmovdqu	xmm15,XMMWORD[((128-32))+rsi]
	vpxor	xmm4,xmm4,xmm5

	vpxor	xmm8,xmm8,XMMWORD[112+rsp]
	vpclmulqdq	xmm5,xmm2,xmm3,0x00
	vmovdqu	xmm0,XMMWORD[((112-32))+rsi]
	vpunpckhqdq	xmm9,xmm8,xmm8
	vpxor	xmm5,xmm5,xmm6
	vpclmulqdq	xmm2,xmm2,xmm3,0x11
	vpxor	xmm9,xmm9,xmm8
	vpxor	xmm2,xmm2,xmm1
	vpclmulqdq	xmm7,xmm7,xmm15,0x00
	vpxor	xmm4,xmm7,xmm4

	vpclmulqdq	xmm6,xmm8,xmm0,0x00
	vmovdqu	xmm3,XMMWORD[((0-32))+rsi]
	vpunpckhqdq	xmm1,xmm14,xmm14
	vpclmulqdq	xmm8,xmm8,xmm0,0x11
	vpxor	xmm1,xmm1,xmm14
	vpxor	xmm5,xmm6,xmm5
	vpclmulqdq	xmm9,xmm9,xmm15,0x10
	vmovdqu	xmm15,XMMWORD[((32-32))+rsi]
	vpxor	xmm7,xmm8,xmm2
	vpxor	xmm6,xmm9,xmm4

	vmovdqu	xmm0,XMMWORD[((16-32))+rsi]
	vpxor	xmm9,xmm7,xmm5
	vpclmulqdq	xmm4,xmm14,xmm3,0x00
	vpxor	xmm6,xmm6,xmm9
	vpunpckhqdq	xmm2,xmm13,xmm13
	vpclmulqdq	xmm14,xmm14,xmm3,0x11
	vpxor	xmm2,xmm2,xmm13
	vpslldq	xmm9,xmm6,8
	vpclmulqdq	xmm1,xmm1,xmm15,0x00
	vpxor	xmm8,xmm5,xmm9
	vpsrldq	xmm6,xmm6,8
	vpxor	xmm7,xmm7,xmm6

	vpclmulqdq	xmm5,xmm13,xmm0,0x00
	vmovdqu	xmm3,XMMWORD[((48-32))+rsi]
	vpxor	xmm5,xmm5,xmm4
	vpunpckhqdq	xmm9,xmm12,xmm12
	vpclmulqdq	xmm13,xmm13,xmm0,0x11
	vpxor	xmm9,xmm9,xmm12
	vpxor	xmm13,xmm13,xmm14
	vpalignr	xmm14,xmm8,xmm8,8
	vpclmulqdq	xmm2,xmm2,xmm15,0x10
	vmovdqu	xmm15,XMMWORD[((80-32))+rsi]
	vpxor	xmm2,xmm2,xmm1

	vpclmulqdq	xmm4,xmm12,xmm3,0x00
	vmovdqu	xmm0,XMMWORD[((64-32))+rsi]
	vpxor	xmm4,xmm4,xmm5
	vpunpckhqdq	xmm1,xmm11,xmm11
	vpclmulqdq	xmm12,xmm12,xmm3,0x11
	vpxor	xmm1,xmm1,xmm11
	vpxor	xmm12,xmm12,xmm13
	vxorps	xmm7,xmm7,XMMWORD[16+rsp]
	vpclmulqdq	xmm9,xmm9,xmm15,0x00
	vpxor	xmm9,xmm9,xmm2

	vpclmulqdq	xmm8,xmm8,XMMWORD[16+r11],0x10
	vxorps	xmm8,xmm8,xmm14

	vpclmulqdq	xmm5,xmm11,xmm0,0x00
	vmovdqu	xmm3,XMMWORD[((96-32))+rsi]
	vpxor	xmm5,xmm5,xmm4
	vpunpckhqdq	xmm2,xmm10,xmm10
	vpclmulqdq	xmm11,xmm11,xmm0,0x11
	vpxor	xmm2,xmm2,xmm10
	vpalignr	xmm14,xmm8,xmm8,8
	vpxor	xmm11,xmm11,xmm12
	vpclmulqdq	xmm1,xmm1,xmm15,0x10
	vmovdqu	xmm15,XMMWORD[((128-32))+rsi]
	vpxor	xmm1,xmm1,xmm9

	vxorps	xmm14,xmm14,xmm7
	vpclmulqdq	xmm8,xmm8,XMMWORD[16+r11],0x10
	vxorps	xmm8,xmm8,xmm14

	vpclmulqdq	xmm4,xmm10,xmm3,0x00
	vmovdqu	xmm0,XMMWORD[((112-32))+rsi]
	vpxor	xmm4,xmm4,xmm5
	vpunpckhqdq	xmm9,xmm8,xmm8
	vpclmulqdq	xmm10,xmm10,xmm3,0x11
	vpxor	xmm9,xmm9,xmm8
	vpxor	xmm10,xmm10,xmm11
	vpclmulqdq	xmm2,xmm2,xmm15,0x00
	vpxor	xmm2,xmm2,xmm1

	vpclmulqdq	xmm5,xmm8,xmm0,0x00
	vpclmulqdq	xmm7,xmm8,xmm0,0x11
	vpxor	xmm5,xmm5,xmm4
	vpclmulqdq	xmm6,xmm9,xmm15,0x10
	vpxor	xmm7,xmm7,xmm10
	vpxor	xmm6,xmm6,xmm2

	vpxor	xmm4,xmm7,xmm5
	vpxor	xmm6,xmm6,xmm4
	vpslldq	xmm1,xmm6,8
	vmovdqu	xmm3,XMMWORD[16+r11]
	vpsrldq	xmm6,xmm6,8
	vpxor	xmm8,xmm5,xmm1
	vpxor	xmm7,xmm7,xmm6

	vpalignr	xmm2,xmm8,xmm8,8
	vpclmulqdq	xmm8,xmm8,xmm3,0x10
	vpxor	xmm8,xmm8,xmm2

	vpalignr	xmm2,xmm8,xmm8,8
	vpclmulqdq	xmm8,xmm8,xmm3,0x10
	vpxor	xmm2,xmm2,xmm7
	vpxor	xmm8,xmm8,xmm2
	mov	r12,QWORD[64+rbp]
	vpshufb	xmm8,xmm8,XMMWORD[r11]
	vmovdqu	XMMWORD[r12],xmm8

	vzeroupper
	movaps	xmm6,XMMWORD[((-208))+rbp]
	movaps	xmm7,XMMWORD[((-192))+rbp]
	movaps	xmm8,XMMWORD[((-176))+rbp]
	movaps	xmm9,XMMWORD[((-160))+rbp]
	movaps	xmm10,XMMWORD[((-144))+rbp]
	movaps	xmm11,XMMWORD[((-128))+rbp]
	movaps	xmm12,XMMWORD[((-112))+rbp]
	movaps	xmm13,XMMWORD[((-96))+rbp]
	movaps	xmm14,XMMWORD[((-80))+rbp]
	movaps	xmm15,XMMWORD[((-64))+rbp]
	mov	rdi,QWORD[16+rbp]
	mov	rsi,QWORD[24+rbp]
	lea	rsp,[((-40))+rbp]

	pop	r15

	pop	r14

	pop	r13

	pop	r12

	pop	rbx

	pop	rbp

$L$gcm_enc_abort:
	ret
$L$SEH_end_aesni_gcm_encrypt_23:


section	.rdata rdata align=8
ALIGN	64
$L$bswap_mask:
	DB	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
$L$poly:
	DB	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2
$L$one_msb:
	DB	0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1
$L$two_lsb:
	DB	2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
$L$one_lsb:
	DB	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0
	DB	65,69,83,45,78,73,32,71,67,77,32,109,111,100,117,108
	DB	101,32,102,111,114,32,120,56,54,95,54,52,44,32,67,82
	DB	89,80,84,79,71,65,77,83,32,98,121,32,60,97,112,112
	DB	114,111,64,111,112,101,110,115,115,108,46,111,114,103,62,0
ALIGN	64
section	.text

section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_aesni_gcm_decrypt_1 wrt ..imagebase
	DD	$L$SEH_end_aesni_gcm_decrypt_23 wrt ..imagebase
	DD	$L$SEH_info_aesni_gcm_decrypt_0 wrt ..imagebase

	DD	$L$SEH_begin_aesni_gcm_encrypt_1 wrt ..imagebase
	DD	$L$SEH_end_aesni_gcm_encrypt_23 wrt ..imagebase
	DD	$L$SEH_info_aesni_gcm_encrypt_0 wrt ..imagebase


section	.xdata rdata align=8
ALIGN	4
$L$SEH_info_aesni_gcm_decrypt_0:
	DB	1
	DB	$L$SEH_endprologue_aesni_gcm_decrypt_22-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	33
	DB	213
	DB	$L$SEH_prologue_aesni_gcm_decrypt_21-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	248
	DW	9
	DB	$L$SEH_prologue_aesni_gcm_decrypt_20-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	232
	DW	8
	DB	$L$SEH_prologue_aesni_gcm_decrypt_19-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	216
	DW	7
	DB	$L$SEH_prologue_aesni_gcm_decrypt_18-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	200
	DW	6
	DB	$L$SEH_prologue_aesni_gcm_decrypt_17-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	184
	DW	5
	DB	$L$SEH_prologue_aesni_gcm_decrypt_16-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	168
	DW	4
	DB	$L$SEH_prologue_aesni_gcm_decrypt_15-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	152
	DW	3
	DB	$L$SEH_prologue_aesni_gcm_decrypt_14-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	136
	DW	2
	DB	$L$SEH_prologue_aesni_gcm_decrypt_13-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	120
	DW	1
	DB	$L$SEH_prologue_aesni_gcm_decrypt_12-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_aesni_gcm_decrypt_11-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	100
	DW	29
	DB	$L$SEH_prologue_aesni_gcm_decrypt_10-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	116
	DW	28
	DB	$L$SEH_prologue_aesni_gcm_decrypt_9-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	3
	DB	$L$SEH_prologue_aesni_gcm_decrypt_8-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	1
	DW	21
	DB	$L$SEH_prologue_aesni_gcm_decrypt_7-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	240
	DB	$L$SEH_prologue_aesni_gcm_decrypt_6-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	224
	DB	$L$SEH_prologue_aesni_gcm_decrypt_5-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	208
	DB	$L$SEH_prologue_aesni_gcm_decrypt_4-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	192
	DB	$L$SEH_prologue_aesni_gcm_decrypt_3-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	48
	DB	$L$SEH_prologue_aesni_gcm_decrypt_2-$L$SEH_begin_aesni_gcm_decrypt_1
	DB	80

	DW	0
$L$SEH_info_aesni_gcm_encrypt_0:
	DB	1
	DB	$L$SEH_endprologue_aesni_gcm_encrypt_22-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	33
	DB	213
	DB	$L$SEH_prologue_aesni_gcm_encrypt_21-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	248
	DW	9
	DB	$L$SEH_prologue_aesni_gcm_encrypt_20-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	232
	DW	8
	DB	$L$SEH_prologue_aesni_gcm_encrypt_19-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	216
	DW	7
	DB	$L$SEH_prologue_aesni_gcm_encrypt_18-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	200
	DW	6
	DB	$L$SEH_prologue_aesni_gcm_encrypt_17-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	184
	DW	5
	DB	$L$SEH_prologue_aesni_gcm_encrypt_16-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	168
	DW	4
	DB	$L$SEH_prologue_aesni_gcm_encrypt_15-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	152
	DW	3
	DB	$L$SEH_prologue_aesni_gcm_encrypt_14-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	136
	DW	2
	DB	$L$SEH_prologue_aesni_gcm_encrypt_13-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	120
	DW	1
	DB	$L$SEH_prologue_aesni_gcm_encrypt_12-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_aesni_gcm_encrypt_11-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	100
	DW	29
	DB	$L$SEH_prologue_aesni_gcm_encrypt_10-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	116
	DW	28
	DB	$L$SEH_prologue_aesni_gcm_encrypt_9-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	3
	DB	$L$SEH_prologue_aesni_gcm_encrypt_8-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	1
	DW	21
	DB	$L$SEH_prologue_aesni_gcm_encrypt_7-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	240
	DB	$L$SEH_prologue_aesni_gcm_encrypt_6-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	224
	DB	$L$SEH_prologue_aesni_gcm_encrypt_5-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	208
	DB	$L$SEH_prologue_aesni_gcm_encrypt_4-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	192
	DB	$L$SEH_prologue_aesni_gcm_encrypt_3-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	48
	DB	$L$SEH_prologue_aesni_gcm_encrypt_2-$L$SEH_begin_aesni_gcm_encrypt_1
	DB	80

	DW	0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
