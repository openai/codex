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


section	.rdata rdata align=8
ALIGN	64
$L$zero:
	DD	0,0,0,0
$L$one:
	DD	1,0,0,0
$L$inc:
	DD	0,1,2,3
$L$four:
	DD	4,4,4,4
$L$incy:
	DD	0,2,4,6,1,3,5,7
$L$eight:
	DD	8,8,8,8,8,8,8,8
$L$rot16:
	DB	0x2,0x3,0x0,0x1,0x6,0x7,0x4,0x5,0xa,0xb,0x8,0x9,0xe,0xf,0xc,0xd
$L$rot24:
	DB	0x3,0x0,0x1,0x2,0x7,0x4,0x5,0x6,0xb,0x8,0x9,0xa,0xf,0xc,0xd,0xe
$L$sigma:
	DB	101,120,112,97,110,100,32,51,50,45,98,121,116,101,32,107
	DB	0
ALIGN	64
$L$zeroz:
	DD	0,0,0,0,1,0,0,0,2,0,0,0,3,0,0,0
$L$fourz:
	DD	4,0,0,0,4,0,0,0,4,0,0,0,4,0,0,0
$L$incz:
	DD	0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
$L$sixteen:
	DD	16,16,16,16,16,16,16,16,16,16,16,16,16,16,16,16
	DB	67,104,97,67,104,97,50,48,32,102,111,114,32,120,56,54
	DB	95,54,52,44,32,67,82,89,80,84,79,71,65,77,83,32
	DB	98,121,32,60,97,112,112,114,111,64,111,112,101,110,115,115
	DB	108,46,111,114,103,62,0
section	.text

global	ChaCha20_ctr32_nohw

ALIGN	64
ChaCha20_ctr32_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ChaCha20_ctr32_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8
	mov	rcx,r9
	mov	r8,QWORD[40+rsp]



_CET_ENDBR
	push	rbx

	push	rbp

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,64+24

$L$ctr32_body:


	movdqu	xmm1,XMMWORD[rcx]
	movdqu	xmm2,XMMWORD[16+rcx]
	movdqu	xmm3,XMMWORD[r8]
	movdqa	xmm4,XMMWORD[$L$one]


	movdqa	XMMWORD[16+rsp],xmm1
	movdqa	XMMWORD[32+rsp],xmm2
	movdqa	XMMWORD[48+rsp],xmm3
	mov	rbp,rdx
	jmp	NEAR $L$oop_outer

ALIGN	32
$L$oop_outer:
	mov	eax,0x61707865
	mov	ebx,0x3320646e
	mov	ecx,0x79622d32
	mov	edx,0x6b206574
	mov	r8d,DWORD[16+rsp]
	mov	r9d,DWORD[20+rsp]
	mov	r10d,DWORD[24+rsp]
	mov	r11d,DWORD[28+rsp]
	movd	r12d,xmm3
	mov	r13d,DWORD[52+rsp]
	mov	r14d,DWORD[56+rsp]
	mov	r15d,DWORD[60+rsp]

	mov	QWORD[((64+0))+rsp],rbp
	mov	ebp,10
	mov	QWORD[((64+8))+rsp],rsi
DB	102,72,15,126,214
	mov	QWORD[((64+16))+rsp],rdi
	mov	rdi,rsi
	shr	rdi,32
	jmp	NEAR $L$oop

ALIGN	32
$L$oop:
	add	eax,r8d
	xor	r12d,eax
	rol	r12d,16
	add	ebx,r9d
	xor	r13d,ebx
	rol	r13d,16
	add	esi,r12d
	xor	r8d,esi
	rol	r8d,12
	add	edi,r13d
	xor	r9d,edi
	rol	r9d,12
	add	eax,r8d
	xor	r12d,eax
	rol	r12d,8
	add	ebx,r9d
	xor	r13d,ebx
	rol	r13d,8
	add	esi,r12d
	xor	r8d,esi
	rol	r8d,7
	add	edi,r13d
	xor	r9d,edi
	rol	r9d,7
	mov	DWORD[32+rsp],esi
	mov	DWORD[36+rsp],edi
	mov	esi,DWORD[40+rsp]
	mov	edi,DWORD[44+rsp]
	add	ecx,r10d
	xor	r14d,ecx
	rol	r14d,16
	add	edx,r11d
	xor	r15d,edx
	rol	r15d,16
	add	esi,r14d
	xor	r10d,esi
	rol	r10d,12
	add	edi,r15d
	xor	r11d,edi
	rol	r11d,12
	add	ecx,r10d
	xor	r14d,ecx
	rol	r14d,8
	add	edx,r11d
	xor	r15d,edx
	rol	r15d,8
	add	esi,r14d
	xor	r10d,esi
	rol	r10d,7
	add	edi,r15d
	xor	r11d,edi
	rol	r11d,7
	add	eax,r9d
	xor	r15d,eax
	rol	r15d,16
	add	ebx,r10d
	xor	r12d,ebx
	rol	r12d,16
	add	esi,r15d
	xor	r9d,esi
	rol	r9d,12
	add	edi,r12d
	xor	r10d,edi
	rol	r10d,12
	add	eax,r9d
	xor	r15d,eax
	rol	r15d,8
	add	ebx,r10d
	xor	r12d,ebx
	rol	r12d,8
	add	esi,r15d
	xor	r9d,esi
	rol	r9d,7
	add	edi,r12d
	xor	r10d,edi
	rol	r10d,7
	mov	DWORD[40+rsp],esi
	mov	DWORD[44+rsp],edi
	mov	esi,DWORD[32+rsp]
	mov	edi,DWORD[36+rsp]
	add	ecx,r11d
	xor	r13d,ecx
	rol	r13d,16
	add	edx,r8d
	xor	r14d,edx
	rol	r14d,16
	add	esi,r13d
	xor	r11d,esi
	rol	r11d,12
	add	edi,r14d
	xor	r8d,edi
	rol	r8d,12
	add	ecx,r11d
	xor	r13d,ecx
	rol	r13d,8
	add	edx,r8d
	xor	r14d,edx
	rol	r14d,8
	add	esi,r13d
	xor	r11d,esi
	rol	r11d,7
	add	edi,r14d
	xor	r8d,edi
	rol	r8d,7
	dec	ebp
	jnz	NEAR $L$oop
	mov	DWORD[36+rsp],edi
	mov	DWORD[32+rsp],esi
	mov	rbp,QWORD[64+rsp]
	movdqa	xmm1,xmm2
	mov	rsi,QWORD[((64+8))+rsp]
	paddd	xmm3,xmm4
	mov	rdi,QWORD[((64+16))+rsp]

	add	eax,0x61707865
	add	ebx,0x3320646e
	add	ecx,0x79622d32
	add	edx,0x6b206574
	add	r8d,DWORD[16+rsp]
	add	r9d,DWORD[20+rsp]
	add	r10d,DWORD[24+rsp]
	add	r11d,DWORD[28+rsp]
	add	r12d,DWORD[48+rsp]
	add	r13d,DWORD[52+rsp]
	add	r14d,DWORD[56+rsp]
	add	r15d,DWORD[60+rsp]
	paddd	xmm1,XMMWORD[32+rsp]

	cmp	rbp,64
	jb	NEAR $L$tail

	xor	eax,DWORD[rsi]
	xor	ebx,DWORD[4+rsi]
	xor	ecx,DWORD[8+rsi]
	xor	edx,DWORD[12+rsi]
	xor	r8d,DWORD[16+rsi]
	xor	r9d,DWORD[20+rsi]
	xor	r10d,DWORD[24+rsi]
	xor	r11d,DWORD[28+rsi]
	movdqu	xmm0,XMMWORD[32+rsi]
	xor	r12d,DWORD[48+rsi]
	xor	r13d,DWORD[52+rsi]
	xor	r14d,DWORD[56+rsi]
	xor	r15d,DWORD[60+rsi]
	lea	rsi,[64+rsi]
	pxor	xmm0,xmm1

	movdqa	XMMWORD[32+rsp],xmm2
	movd	DWORD[48+rsp],xmm3

	mov	DWORD[rdi],eax
	mov	DWORD[4+rdi],ebx
	mov	DWORD[8+rdi],ecx
	mov	DWORD[12+rdi],edx
	mov	DWORD[16+rdi],r8d
	mov	DWORD[20+rdi],r9d
	mov	DWORD[24+rdi],r10d
	mov	DWORD[28+rdi],r11d
	movdqu	XMMWORD[32+rdi],xmm0
	mov	DWORD[48+rdi],r12d
	mov	DWORD[52+rdi],r13d
	mov	DWORD[56+rdi],r14d
	mov	DWORD[60+rdi],r15d
	lea	rdi,[64+rdi]

	sub	rbp,64
	jnz	NEAR $L$oop_outer

	jmp	NEAR $L$done

ALIGN	16
$L$tail:
	mov	DWORD[rsp],eax
	mov	DWORD[4+rsp],ebx
	xor	rbx,rbx
	mov	DWORD[8+rsp],ecx
	mov	DWORD[12+rsp],edx
	mov	DWORD[16+rsp],r8d
	mov	DWORD[20+rsp],r9d
	mov	DWORD[24+rsp],r10d
	mov	DWORD[28+rsp],r11d
	movdqa	XMMWORD[32+rsp],xmm1
	mov	DWORD[48+rsp],r12d
	mov	DWORD[52+rsp],r13d
	mov	DWORD[56+rsp],r14d
	mov	DWORD[60+rsp],r15d

$L$oop_tail:
	movzx	eax,BYTE[rbx*1+rsi]
	movzx	edx,BYTE[rbx*1+rsp]
	lea	rbx,[1+rbx]
	xor	eax,edx
	mov	BYTE[((-1))+rbx*1+rdi],al
	dec	rbp
	jnz	NEAR $L$oop_tail

$L$done:
	lea	rsi,[((64+24+48))+rsp]
	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbp,QWORD[((-16))+rsi]

	mov	rbx,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$no_data:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ChaCha20_ctr32_nohw:
global	ChaCha20_ctr32_ssse3_4x

ALIGN	32
ChaCha20_ctr32_ssse3_4x:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ChaCha20_ctr32_ssse3_4x:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8
	mov	rcx,r9
	mov	r8,QWORD[40+rsp]



_CET_ENDBR
	mov	r9,rsp

	sub	rsp,0x140+168
	movaps	XMMWORD[(-168)+r9],xmm6
	movaps	XMMWORD[(-152)+r9],xmm7
	movaps	XMMWORD[(-136)+r9],xmm8
	movaps	XMMWORD[(-120)+r9],xmm9
	movaps	XMMWORD[(-104)+r9],xmm10
	movaps	XMMWORD[(-88)+r9],xmm11
	movaps	XMMWORD[(-72)+r9],xmm12
	movaps	XMMWORD[(-56)+r9],xmm13
	movaps	XMMWORD[(-40)+r9],xmm14
	movaps	XMMWORD[(-24)+r9],xmm15
$L$4x_body:
	movdqa	xmm11,XMMWORD[$L$sigma]
	movdqu	xmm15,XMMWORD[rcx]
	movdqu	xmm7,XMMWORD[16+rcx]
	movdqu	xmm3,XMMWORD[r8]
	lea	rcx,[256+rsp]
	lea	r10,[$L$rot16]
	lea	r11,[$L$rot24]

	pshufd	xmm8,xmm11,0x00
	pshufd	xmm9,xmm11,0x55
	movdqa	XMMWORD[64+rsp],xmm8
	pshufd	xmm10,xmm11,0xaa
	movdqa	XMMWORD[80+rsp],xmm9
	pshufd	xmm11,xmm11,0xff
	movdqa	XMMWORD[96+rsp],xmm10
	movdqa	XMMWORD[112+rsp],xmm11

	pshufd	xmm12,xmm15,0x00
	pshufd	xmm13,xmm15,0x55
	movdqa	XMMWORD[(128-256)+rcx],xmm12
	pshufd	xmm14,xmm15,0xaa
	movdqa	XMMWORD[(144-256)+rcx],xmm13
	pshufd	xmm15,xmm15,0xff
	movdqa	XMMWORD[(160-256)+rcx],xmm14
	movdqa	XMMWORD[(176-256)+rcx],xmm15

	pshufd	xmm4,xmm7,0x00
	pshufd	xmm5,xmm7,0x55
	movdqa	XMMWORD[(192-256)+rcx],xmm4
	pshufd	xmm6,xmm7,0xaa
	movdqa	XMMWORD[(208-256)+rcx],xmm5
	pshufd	xmm7,xmm7,0xff
	movdqa	XMMWORD[(224-256)+rcx],xmm6
	movdqa	XMMWORD[(240-256)+rcx],xmm7

	pshufd	xmm0,xmm3,0x00
	pshufd	xmm1,xmm3,0x55
	paddd	xmm0,XMMWORD[$L$inc]
	pshufd	xmm2,xmm3,0xaa
	movdqa	XMMWORD[(272-256)+rcx],xmm1
	pshufd	xmm3,xmm3,0xff
	movdqa	XMMWORD[(288-256)+rcx],xmm2
	movdqa	XMMWORD[(304-256)+rcx],xmm3

	jmp	NEAR $L$oop_enter4x

ALIGN	32
$L$oop_outer4x:
	movdqa	xmm8,XMMWORD[64+rsp]
	movdqa	xmm9,XMMWORD[80+rsp]
	movdqa	xmm10,XMMWORD[96+rsp]
	movdqa	xmm11,XMMWORD[112+rsp]
	movdqa	xmm12,XMMWORD[((128-256))+rcx]
	movdqa	xmm13,XMMWORD[((144-256))+rcx]
	movdqa	xmm14,XMMWORD[((160-256))+rcx]
	movdqa	xmm15,XMMWORD[((176-256))+rcx]
	movdqa	xmm4,XMMWORD[((192-256))+rcx]
	movdqa	xmm5,XMMWORD[((208-256))+rcx]
	movdqa	xmm6,XMMWORD[((224-256))+rcx]
	movdqa	xmm7,XMMWORD[((240-256))+rcx]
	movdqa	xmm0,XMMWORD[((256-256))+rcx]
	movdqa	xmm1,XMMWORD[((272-256))+rcx]
	movdqa	xmm2,XMMWORD[((288-256))+rcx]
	movdqa	xmm3,XMMWORD[((304-256))+rcx]
	paddd	xmm0,XMMWORD[$L$four]

$L$oop_enter4x:
	movdqa	XMMWORD[32+rsp],xmm6
	movdqa	XMMWORD[48+rsp],xmm7
	movdqa	xmm7,XMMWORD[r10]
	mov	eax,10
	movdqa	XMMWORD[(256-256)+rcx],xmm0
	jmp	NEAR $L$oop4x

ALIGN	32
$L$oop4x:
	paddd	xmm8,xmm12
	paddd	xmm9,xmm13
	pxor	xmm0,xmm8
	pxor	xmm1,xmm9
DB	102,15,56,0,199
DB	102,15,56,0,207
	paddd	xmm4,xmm0
	paddd	xmm5,xmm1
	pxor	xmm12,xmm4
	pxor	xmm13,xmm5
	movdqa	xmm6,xmm12
	pslld	xmm12,12
	psrld	xmm6,20
	movdqa	xmm7,xmm13
	pslld	xmm13,12
	por	xmm12,xmm6
	psrld	xmm7,20
	movdqa	xmm6,XMMWORD[r11]
	por	xmm13,xmm7
	paddd	xmm8,xmm12
	paddd	xmm9,xmm13
	pxor	xmm0,xmm8
	pxor	xmm1,xmm9
DB	102,15,56,0,198
DB	102,15,56,0,206
	paddd	xmm4,xmm0
	paddd	xmm5,xmm1
	pxor	xmm12,xmm4
	pxor	xmm13,xmm5
	movdqa	xmm7,xmm12
	pslld	xmm12,7
	psrld	xmm7,25
	movdqa	xmm6,xmm13
	pslld	xmm13,7
	por	xmm12,xmm7
	psrld	xmm6,25
	movdqa	xmm7,XMMWORD[r10]
	por	xmm13,xmm6
	movdqa	XMMWORD[rsp],xmm4
	movdqa	XMMWORD[16+rsp],xmm5
	movdqa	xmm4,XMMWORD[32+rsp]
	movdqa	xmm5,XMMWORD[48+rsp]
	paddd	xmm10,xmm14
	paddd	xmm11,xmm15
	pxor	xmm2,xmm10
	pxor	xmm3,xmm11
DB	102,15,56,0,215
DB	102,15,56,0,223
	paddd	xmm4,xmm2
	paddd	xmm5,xmm3
	pxor	xmm14,xmm4
	pxor	xmm15,xmm5
	movdqa	xmm6,xmm14
	pslld	xmm14,12
	psrld	xmm6,20
	movdqa	xmm7,xmm15
	pslld	xmm15,12
	por	xmm14,xmm6
	psrld	xmm7,20
	movdqa	xmm6,XMMWORD[r11]
	por	xmm15,xmm7
	paddd	xmm10,xmm14
	paddd	xmm11,xmm15
	pxor	xmm2,xmm10
	pxor	xmm3,xmm11
DB	102,15,56,0,214
DB	102,15,56,0,222
	paddd	xmm4,xmm2
	paddd	xmm5,xmm3
	pxor	xmm14,xmm4
	pxor	xmm15,xmm5
	movdqa	xmm7,xmm14
	pslld	xmm14,7
	psrld	xmm7,25
	movdqa	xmm6,xmm15
	pslld	xmm15,7
	por	xmm14,xmm7
	psrld	xmm6,25
	movdqa	xmm7,XMMWORD[r10]
	por	xmm15,xmm6
	paddd	xmm8,xmm13
	paddd	xmm9,xmm14
	pxor	xmm3,xmm8
	pxor	xmm0,xmm9
DB	102,15,56,0,223
DB	102,15,56,0,199
	paddd	xmm4,xmm3
	paddd	xmm5,xmm0
	pxor	xmm13,xmm4
	pxor	xmm14,xmm5
	movdqa	xmm6,xmm13
	pslld	xmm13,12
	psrld	xmm6,20
	movdqa	xmm7,xmm14
	pslld	xmm14,12
	por	xmm13,xmm6
	psrld	xmm7,20
	movdqa	xmm6,XMMWORD[r11]
	por	xmm14,xmm7
	paddd	xmm8,xmm13
	paddd	xmm9,xmm14
	pxor	xmm3,xmm8
	pxor	xmm0,xmm9
DB	102,15,56,0,222
DB	102,15,56,0,198
	paddd	xmm4,xmm3
	paddd	xmm5,xmm0
	pxor	xmm13,xmm4
	pxor	xmm14,xmm5
	movdqa	xmm7,xmm13
	pslld	xmm13,7
	psrld	xmm7,25
	movdqa	xmm6,xmm14
	pslld	xmm14,7
	por	xmm13,xmm7
	psrld	xmm6,25
	movdqa	xmm7,XMMWORD[r10]
	por	xmm14,xmm6
	movdqa	XMMWORD[32+rsp],xmm4
	movdqa	XMMWORD[48+rsp],xmm5
	movdqa	xmm4,XMMWORD[rsp]
	movdqa	xmm5,XMMWORD[16+rsp]
	paddd	xmm10,xmm15
	paddd	xmm11,xmm12
	pxor	xmm1,xmm10
	pxor	xmm2,xmm11
DB	102,15,56,0,207
DB	102,15,56,0,215
	paddd	xmm4,xmm1
	paddd	xmm5,xmm2
	pxor	xmm15,xmm4
	pxor	xmm12,xmm5
	movdqa	xmm6,xmm15
	pslld	xmm15,12
	psrld	xmm6,20
	movdqa	xmm7,xmm12
	pslld	xmm12,12
	por	xmm15,xmm6
	psrld	xmm7,20
	movdqa	xmm6,XMMWORD[r11]
	por	xmm12,xmm7
	paddd	xmm10,xmm15
	paddd	xmm11,xmm12
	pxor	xmm1,xmm10
	pxor	xmm2,xmm11
DB	102,15,56,0,206
DB	102,15,56,0,214
	paddd	xmm4,xmm1
	paddd	xmm5,xmm2
	pxor	xmm15,xmm4
	pxor	xmm12,xmm5
	movdqa	xmm7,xmm15
	pslld	xmm15,7
	psrld	xmm7,25
	movdqa	xmm6,xmm12
	pslld	xmm12,7
	por	xmm15,xmm7
	psrld	xmm6,25
	movdqa	xmm7,XMMWORD[r10]
	por	xmm12,xmm6
	dec	eax
	jnz	NEAR $L$oop4x

	paddd	xmm8,XMMWORD[64+rsp]
	paddd	xmm9,XMMWORD[80+rsp]
	paddd	xmm10,XMMWORD[96+rsp]
	paddd	xmm11,XMMWORD[112+rsp]

	movdqa	xmm6,xmm8
	punpckldq	xmm8,xmm9
	movdqa	xmm7,xmm10
	punpckldq	xmm10,xmm11
	punpckhdq	xmm6,xmm9
	punpckhdq	xmm7,xmm11
	movdqa	xmm9,xmm8
	punpcklqdq	xmm8,xmm10
	movdqa	xmm11,xmm6
	punpcklqdq	xmm6,xmm7
	punpckhqdq	xmm9,xmm10
	punpckhqdq	xmm11,xmm7
	paddd	xmm12,XMMWORD[((128-256))+rcx]
	paddd	xmm13,XMMWORD[((144-256))+rcx]
	paddd	xmm14,XMMWORD[((160-256))+rcx]
	paddd	xmm15,XMMWORD[((176-256))+rcx]

	movdqa	XMMWORD[rsp],xmm8
	movdqa	XMMWORD[16+rsp],xmm9
	movdqa	xmm8,XMMWORD[32+rsp]
	movdqa	xmm9,XMMWORD[48+rsp]

	movdqa	xmm10,xmm12
	punpckldq	xmm12,xmm13
	movdqa	xmm7,xmm14
	punpckldq	xmm14,xmm15
	punpckhdq	xmm10,xmm13
	punpckhdq	xmm7,xmm15
	movdqa	xmm13,xmm12
	punpcklqdq	xmm12,xmm14
	movdqa	xmm15,xmm10
	punpcklqdq	xmm10,xmm7
	punpckhqdq	xmm13,xmm14
	punpckhqdq	xmm15,xmm7
	paddd	xmm4,XMMWORD[((192-256))+rcx]
	paddd	xmm5,XMMWORD[((208-256))+rcx]
	paddd	xmm8,XMMWORD[((224-256))+rcx]
	paddd	xmm9,XMMWORD[((240-256))+rcx]

	movdqa	XMMWORD[32+rsp],xmm6
	movdqa	XMMWORD[48+rsp],xmm11

	movdqa	xmm14,xmm4
	punpckldq	xmm4,xmm5
	movdqa	xmm7,xmm8
	punpckldq	xmm8,xmm9
	punpckhdq	xmm14,xmm5
	punpckhdq	xmm7,xmm9
	movdqa	xmm5,xmm4
	punpcklqdq	xmm4,xmm8
	movdqa	xmm9,xmm14
	punpcklqdq	xmm14,xmm7
	punpckhqdq	xmm5,xmm8
	punpckhqdq	xmm9,xmm7
	paddd	xmm0,XMMWORD[((256-256))+rcx]
	paddd	xmm1,XMMWORD[((272-256))+rcx]
	paddd	xmm2,XMMWORD[((288-256))+rcx]
	paddd	xmm3,XMMWORD[((304-256))+rcx]

	movdqa	xmm8,xmm0
	punpckldq	xmm0,xmm1
	movdqa	xmm7,xmm2
	punpckldq	xmm2,xmm3
	punpckhdq	xmm8,xmm1
	punpckhdq	xmm7,xmm3
	movdqa	xmm1,xmm0
	punpcklqdq	xmm0,xmm2
	movdqa	xmm3,xmm8
	punpcklqdq	xmm8,xmm7
	punpckhqdq	xmm1,xmm2
	punpckhqdq	xmm3,xmm7
	cmp	rdx,64*4
	jb	NEAR $L$tail4x

	movdqu	xmm6,XMMWORD[rsi]
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[rsp]
	pxor	xmm11,xmm12
	pxor	xmm2,xmm4
	pxor	xmm7,xmm0

	movdqu	XMMWORD[rdi],xmm6
	movdqu	xmm6,XMMWORD[64+rsi]
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	xmm11,XMMWORD[80+rsi]
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	xmm2,XMMWORD[96+rsi]
	movdqu	XMMWORD[48+rdi],xmm7
	movdqu	xmm7,XMMWORD[112+rsi]
	lea	rsi,[128+rsi]
	pxor	xmm6,XMMWORD[16+rsp]
	pxor	xmm11,xmm13
	pxor	xmm2,xmm5
	pxor	xmm7,xmm1

	movdqu	XMMWORD[64+rdi],xmm6
	movdqu	xmm6,XMMWORD[rsi]
	movdqu	XMMWORD[80+rdi],xmm11
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	XMMWORD[96+rdi],xmm2
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	XMMWORD[112+rdi],xmm7
	lea	rdi,[128+rdi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[32+rsp]
	pxor	xmm11,xmm10
	pxor	xmm2,xmm14
	pxor	xmm7,xmm8

	movdqu	XMMWORD[rdi],xmm6
	movdqu	xmm6,XMMWORD[64+rsi]
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	xmm11,XMMWORD[80+rsi]
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	xmm2,XMMWORD[96+rsi]
	movdqu	XMMWORD[48+rdi],xmm7
	movdqu	xmm7,XMMWORD[112+rsi]
	lea	rsi,[128+rsi]
	pxor	xmm6,XMMWORD[48+rsp]
	pxor	xmm11,xmm15
	pxor	xmm2,xmm9
	pxor	xmm7,xmm3
	movdqu	XMMWORD[64+rdi],xmm6
	movdqu	XMMWORD[80+rdi],xmm11
	movdqu	XMMWORD[96+rdi],xmm2
	movdqu	XMMWORD[112+rdi],xmm7
	lea	rdi,[128+rdi]

	sub	rdx,64*4
	jnz	NEAR $L$oop_outer4x

	jmp	NEAR $L$done4x

$L$tail4x:
	cmp	rdx,192
	jae	NEAR $L$192_or_more4x
	cmp	rdx,128
	jae	NEAR $L$128_or_more4x
	cmp	rdx,64
	jae	NEAR $L$64_or_more4x


	xor	r10,r10

	movdqa	XMMWORD[16+rsp],xmm12
	movdqa	XMMWORD[32+rsp],xmm4
	movdqa	XMMWORD[48+rsp],xmm0
	jmp	NEAR $L$oop_tail4x

ALIGN	32
$L$64_or_more4x:
	movdqu	xmm6,XMMWORD[rsi]
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[rsp]
	pxor	xmm11,xmm12
	pxor	xmm2,xmm4
	pxor	xmm7,xmm0
	movdqu	XMMWORD[rdi],xmm6
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm7
	je	NEAR $L$done4x

	movdqa	xmm6,XMMWORD[16+rsp]
	lea	rsi,[64+rsi]
	xor	r10,r10
	movdqa	XMMWORD[rsp],xmm6
	movdqa	XMMWORD[16+rsp],xmm13
	lea	rdi,[64+rdi]
	movdqa	XMMWORD[32+rsp],xmm5
	sub	rdx,64
	movdqa	XMMWORD[48+rsp],xmm1
	jmp	NEAR $L$oop_tail4x

ALIGN	32
$L$128_or_more4x:
	movdqu	xmm6,XMMWORD[rsi]
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[rsp]
	pxor	xmm11,xmm12
	pxor	xmm2,xmm4
	pxor	xmm7,xmm0

	movdqu	XMMWORD[rdi],xmm6
	movdqu	xmm6,XMMWORD[64+rsi]
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	xmm11,XMMWORD[80+rsi]
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	xmm2,XMMWORD[96+rsi]
	movdqu	XMMWORD[48+rdi],xmm7
	movdqu	xmm7,XMMWORD[112+rsi]
	pxor	xmm6,XMMWORD[16+rsp]
	pxor	xmm11,xmm13
	pxor	xmm2,xmm5
	pxor	xmm7,xmm1
	movdqu	XMMWORD[64+rdi],xmm6
	movdqu	XMMWORD[80+rdi],xmm11
	movdqu	XMMWORD[96+rdi],xmm2
	movdqu	XMMWORD[112+rdi],xmm7
	je	NEAR $L$done4x

	movdqa	xmm6,XMMWORD[32+rsp]
	lea	rsi,[128+rsi]
	xor	r10,r10
	movdqa	XMMWORD[rsp],xmm6
	movdqa	XMMWORD[16+rsp],xmm10
	lea	rdi,[128+rdi]
	movdqa	XMMWORD[32+rsp],xmm14
	sub	rdx,128
	movdqa	XMMWORD[48+rsp],xmm8
	jmp	NEAR $L$oop_tail4x

ALIGN	32
$L$192_or_more4x:
	movdqu	xmm6,XMMWORD[rsi]
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[rsp]
	pxor	xmm11,xmm12
	pxor	xmm2,xmm4
	pxor	xmm7,xmm0

	movdqu	XMMWORD[rdi],xmm6
	movdqu	xmm6,XMMWORD[64+rsi]
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	xmm11,XMMWORD[80+rsi]
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	xmm2,XMMWORD[96+rsi]
	movdqu	XMMWORD[48+rdi],xmm7
	movdqu	xmm7,XMMWORD[112+rsi]
	lea	rsi,[128+rsi]
	pxor	xmm6,XMMWORD[16+rsp]
	pxor	xmm11,xmm13
	pxor	xmm2,xmm5
	pxor	xmm7,xmm1

	movdqu	XMMWORD[64+rdi],xmm6
	movdqu	xmm6,XMMWORD[rsi]
	movdqu	XMMWORD[80+rdi],xmm11
	movdqu	xmm11,XMMWORD[16+rsi]
	movdqu	XMMWORD[96+rdi],xmm2
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	XMMWORD[112+rdi],xmm7
	lea	rdi,[128+rdi]
	movdqu	xmm7,XMMWORD[48+rsi]
	pxor	xmm6,XMMWORD[32+rsp]
	pxor	xmm11,xmm10
	pxor	xmm2,xmm14
	pxor	xmm7,xmm8
	movdqu	XMMWORD[rdi],xmm6
	movdqu	XMMWORD[16+rdi],xmm11
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm7
	je	NEAR $L$done4x

	movdqa	xmm6,XMMWORD[48+rsp]
	lea	rsi,[64+rsi]
	xor	r10,r10
	movdqa	XMMWORD[rsp],xmm6
	movdqa	XMMWORD[16+rsp],xmm15
	lea	rdi,[64+rdi]
	movdqa	XMMWORD[32+rsp],xmm9
	sub	rdx,192
	movdqa	XMMWORD[48+rsp],xmm3

$L$oop_tail4x:
	movzx	eax,BYTE[r10*1+rsi]
	movzx	ecx,BYTE[r10*1+rsp]
	lea	r10,[1+r10]
	xor	eax,ecx
	mov	BYTE[((-1))+r10*1+rdi],al
	dec	rdx
	jnz	NEAR $L$oop_tail4x

$L$done4x:
	movaps	xmm6,XMMWORD[((-168))+r9]
	movaps	xmm7,XMMWORD[((-152))+r9]
	movaps	xmm8,XMMWORD[((-136))+r9]
	movaps	xmm9,XMMWORD[((-120))+r9]
	movaps	xmm10,XMMWORD[((-104))+r9]
	movaps	xmm11,XMMWORD[((-88))+r9]
	movaps	xmm12,XMMWORD[((-72))+r9]
	movaps	xmm13,XMMWORD[((-56))+r9]
	movaps	xmm14,XMMWORD[((-40))+r9]
	movaps	xmm15,XMMWORD[((-24))+r9]
	lea	rsp,[r9]

$L$4x_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ChaCha20_ctr32_ssse3_4x:
global	ChaCha20_ctr32_avx2

ALIGN	32
ChaCha20_ctr32_avx2:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ChaCha20_ctr32_avx2:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8
	mov	rcx,r9
	mov	r8,QWORD[40+rsp]



_CET_ENDBR
	mov	r9,rsp

	sub	rsp,0x280+168
	and	rsp,-32
	movaps	XMMWORD[(-168)+r9],xmm6
	movaps	XMMWORD[(-152)+r9],xmm7
	movaps	XMMWORD[(-136)+r9],xmm8
	movaps	XMMWORD[(-120)+r9],xmm9
	movaps	XMMWORD[(-104)+r9],xmm10
	movaps	XMMWORD[(-88)+r9],xmm11
	movaps	XMMWORD[(-72)+r9],xmm12
	movaps	XMMWORD[(-56)+r9],xmm13
	movaps	XMMWORD[(-40)+r9],xmm14
	movaps	XMMWORD[(-24)+r9],xmm15
$L$8x_body:
	vzeroupper










	vbroadcasti128	ymm11,XMMWORD[$L$sigma]
	vbroadcasti128	ymm3,XMMWORD[rcx]
	vbroadcasti128	ymm15,XMMWORD[16+rcx]
	vbroadcasti128	ymm7,XMMWORD[r8]
	lea	rcx,[256+rsp]
	lea	rax,[512+rsp]
	lea	r10,[$L$rot16]
	lea	r11,[$L$rot24]

	vpshufd	ymm8,ymm11,0x00
	vpshufd	ymm9,ymm11,0x55
	vmovdqa	YMMWORD[(128-256)+rcx],ymm8
	vpshufd	ymm10,ymm11,0xaa
	vmovdqa	YMMWORD[(160-256)+rcx],ymm9
	vpshufd	ymm11,ymm11,0xff
	vmovdqa	YMMWORD[(192-256)+rcx],ymm10
	vmovdqa	YMMWORD[(224-256)+rcx],ymm11

	vpshufd	ymm0,ymm3,0x00
	vpshufd	ymm1,ymm3,0x55
	vmovdqa	YMMWORD[(256-256)+rcx],ymm0
	vpshufd	ymm2,ymm3,0xaa
	vmovdqa	YMMWORD[(288-256)+rcx],ymm1
	vpshufd	ymm3,ymm3,0xff
	vmovdqa	YMMWORD[(320-256)+rcx],ymm2
	vmovdqa	YMMWORD[(352-256)+rcx],ymm3

	vpshufd	ymm12,ymm15,0x00
	vpshufd	ymm13,ymm15,0x55
	vmovdqa	YMMWORD[(384-512)+rax],ymm12
	vpshufd	ymm14,ymm15,0xaa
	vmovdqa	YMMWORD[(416-512)+rax],ymm13
	vpshufd	ymm15,ymm15,0xff
	vmovdqa	YMMWORD[(448-512)+rax],ymm14
	vmovdqa	YMMWORD[(480-512)+rax],ymm15

	vpshufd	ymm4,ymm7,0x00
	vpshufd	ymm5,ymm7,0x55
	vpaddd	ymm4,ymm4,YMMWORD[$L$incy]
	vpshufd	ymm6,ymm7,0xaa
	vmovdqa	YMMWORD[(544-512)+rax],ymm5
	vpshufd	ymm7,ymm7,0xff
	vmovdqa	YMMWORD[(576-512)+rax],ymm6
	vmovdqa	YMMWORD[(608-512)+rax],ymm7

	jmp	NEAR $L$oop_enter8x

ALIGN	32
$L$oop_outer8x:
	vmovdqa	ymm8,YMMWORD[((128-256))+rcx]
	vmovdqa	ymm9,YMMWORD[((160-256))+rcx]
	vmovdqa	ymm10,YMMWORD[((192-256))+rcx]
	vmovdqa	ymm11,YMMWORD[((224-256))+rcx]
	vmovdqa	ymm0,YMMWORD[((256-256))+rcx]
	vmovdqa	ymm1,YMMWORD[((288-256))+rcx]
	vmovdqa	ymm2,YMMWORD[((320-256))+rcx]
	vmovdqa	ymm3,YMMWORD[((352-256))+rcx]
	vmovdqa	ymm12,YMMWORD[((384-512))+rax]
	vmovdqa	ymm13,YMMWORD[((416-512))+rax]
	vmovdqa	ymm14,YMMWORD[((448-512))+rax]
	vmovdqa	ymm15,YMMWORD[((480-512))+rax]
	vmovdqa	ymm4,YMMWORD[((512-512))+rax]
	vmovdqa	ymm5,YMMWORD[((544-512))+rax]
	vmovdqa	ymm6,YMMWORD[((576-512))+rax]
	vmovdqa	ymm7,YMMWORD[((608-512))+rax]
	vpaddd	ymm4,ymm4,YMMWORD[$L$eight]

$L$oop_enter8x:
	vmovdqa	YMMWORD[64+rsp],ymm14
	vmovdqa	YMMWORD[96+rsp],ymm15
	vbroadcasti128	ymm15,XMMWORD[r10]
	vmovdqa	YMMWORD[(512-512)+rax],ymm4
	mov	eax,10
	jmp	NEAR $L$oop8x

ALIGN	32
$L$oop8x:
	vpaddd	ymm8,ymm8,ymm0
	vpxor	ymm4,ymm8,ymm4
	vpshufb	ymm4,ymm4,ymm15
	vpaddd	ymm9,ymm9,ymm1
	vpxor	ymm5,ymm9,ymm5
	vpshufb	ymm5,ymm5,ymm15
	vpaddd	ymm12,ymm12,ymm4
	vpxor	ymm0,ymm12,ymm0
	vpslld	ymm14,ymm0,12
	vpsrld	ymm0,ymm0,20
	vpor	ymm0,ymm14,ymm0
	vbroadcasti128	ymm14,XMMWORD[r11]
	vpaddd	ymm13,ymm13,ymm5
	vpxor	ymm1,ymm13,ymm1
	vpslld	ymm15,ymm1,12
	vpsrld	ymm1,ymm1,20
	vpor	ymm1,ymm15,ymm1
	vpaddd	ymm8,ymm8,ymm0
	vpxor	ymm4,ymm8,ymm4
	vpshufb	ymm4,ymm4,ymm14
	vpaddd	ymm9,ymm9,ymm1
	vpxor	ymm5,ymm9,ymm5
	vpshufb	ymm5,ymm5,ymm14
	vpaddd	ymm12,ymm12,ymm4
	vpxor	ymm0,ymm12,ymm0
	vpslld	ymm15,ymm0,7
	vpsrld	ymm0,ymm0,25
	vpor	ymm0,ymm15,ymm0
	vbroadcasti128	ymm15,XMMWORD[r10]
	vpaddd	ymm13,ymm13,ymm5
	vpxor	ymm1,ymm13,ymm1
	vpslld	ymm14,ymm1,7
	vpsrld	ymm1,ymm1,25
	vpor	ymm1,ymm14,ymm1
	vmovdqa	YMMWORD[rsp],ymm12
	vmovdqa	YMMWORD[32+rsp],ymm13
	vmovdqa	ymm12,YMMWORD[64+rsp]
	vmovdqa	ymm13,YMMWORD[96+rsp]
	vpaddd	ymm10,ymm10,ymm2
	vpxor	ymm6,ymm10,ymm6
	vpshufb	ymm6,ymm6,ymm15
	vpaddd	ymm11,ymm11,ymm3
	vpxor	ymm7,ymm11,ymm7
	vpshufb	ymm7,ymm7,ymm15
	vpaddd	ymm12,ymm12,ymm6
	vpxor	ymm2,ymm12,ymm2
	vpslld	ymm14,ymm2,12
	vpsrld	ymm2,ymm2,20
	vpor	ymm2,ymm14,ymm2
	vbroadcasti128	ymm14,XMMWORD[r11]
	vpaddd	ymm13,ymm13,ymm7
	vpxor	ymm3,ymm13,ymm3
	vpslld	ymm15,ymm3,12
	vpsrld	ymm3,ymm3,20
	vpor	ymm3,ymm15,ymm3
	vpaddd	ymm10,ymm10,ymm2
	vpxor	ymm6,ymm10,ymm6
	vpshufb	ymm6,ymm6,ymm14
	vpaddd	ymm11,ymm11,ymm3
	vpxor	ymm7,ymm11,ymm7
	vpshufb	ymm7,ymm7,ymm14
	vpaddd	ymm12,ymm12,ymm6
	vpxor	ymm2,ymm12,ymm2
	vpslld	ymm15,ymm2,7
	vpsrld	ymm2,ymm2,25
	vpor	ymm2,ymm15,ymm2
	vbroadcasti128	ymm15,XMMWORD[r10]
	vpaddd	ymm13,ymm13,ymm7
	vpxor	ymm3,ymm13,ymm3
	vpslld	ymm14,ymm3,7
	vpsrld	ymm3,ymm3,25
	vpor	ymm3,ymm14,ymm3
	vpaddd	ymm8,ymm8,ymm1
	vpxor	ymm7,ymm8,ymm7
	vpshufb	ymm7,ymm7,ymm15
	vpaddd	ymm9,ymm9,ymm2
	vpxor	ymm4,ymm9,ymm4
	vpshufb	ymm4,ymm4,ymm15
	vpaddd	ymm12,ymm12,ymm7
	vpxor	ymm1,ymm12,ymm1
	vpslld	ymm14,ymm1,12
	vpsrld	ymm1,ymm1,20
	vpor	ymm1,ymm14,ymm1
	vbroadcasti128	ymm14,XMMWORD[r11]
	vpaddd	ymm13,ymm13,ymm4
	vpxor	ymm2,ymm13,ymm2
	vpslld	ymm15,ymm2,12
	vpsrld	ymm2,ymm2,20
	vpor	ymm2,ymm15,ymm2
	vpaddd	ymm8,ymm8,ymm1
	vpxor	ymm7,ymm8,ymm7
	vpshufb	ymm7,ymm7,ymm14
	vpaddd	ymm9,ymm9,ymm2
	vpxor	ymm4,ymm9,ymm4
	vpshufb	ymm4,ymm4,ymm14
	vpaddd	ymm12,ymm12,ymm7
	vpxor	ymm1,ymm12,ymm1
	vpslld	ymm15,ymm1,7
	vpsrld	ymm1,ymm1,25
	vpor	ymm1,ymm15,ymm1
	vbroadcasti128	ymm15,XMMWORD[r10]
	vpaddd	ymm13,ymm13,ymm4
	vpxor	ymm2,ymm13,ymm2
	vpslld	ymm14,ymm2,7
	vpsrld	ymm2,ymm2,25
	vpor	ymm2,ymm14,ymm2
	vmovdqa	YMMWORD[64+rsp],ymm12
	vmovdqa	YMMWORD[96+rsp],ymm13
	vmovdqa	ymm12,YMMWORD[rsp]
	vmovdqa	ymm13,YMMWORD[32+rsp]
	vpaddd	ymm10,ymm10,ymm3
	vpxor	ymm5,ymm10,ymm5
	vpshufb	ymm5,ymm5,ymm15
	vpaddd	ymm11,ymm11,ymm0
	vpxor	ymm6,ymm11,ymm6
	vpshufb	ymm6,ymm6,ymm15
	vpaddd	ymm12,ymm12,ymm5
	vpxor	ymm3,ymm12,ymm3
	vpslld	ymm14,ymm3,12
	vpsrld	ymm3,ymm3,20
	vpor	ymm3,ymm14,ymm3
	vbroadcasti128	ymm14,XMMWORD[r11]
	vpaddd	ymm13,ymm13,ymm6
	vpxor	ymm0,ymm13,ymm0
	vpslld	ymm15,ymm0,12
	vpsrld	ymm0,ymm0,20
	vpor	ymm0,ymm15,ymm0
	vpaddd	ymm10,ymm10,ymm3
	vpxor	ymm5,ymm10,ymm5
	vpshufb	ymm5,ymm5,ymm14
	vpaddd	ymm11,ymm11,ymm0
	vpxor	ymm6,ymm11,ymm6
	vpshufb	ymm6,ymm6,ymm14
	vpaddd	ymm12,ymm12,ymm5
	vpxor	ymm3,ymm12,ymm3
	vpslld	ymm15,ymm3,7
	vpsrld	ymm3,ymm3,25
	vpor	ymm3,ymm15,ymm3
	vbroadcasti128	ymm15,XMMWORD[r10]
	vpaddd	ymm13,ymm13,ymm6
	vpxor	ymm0,ymm13,ymm0
	vpslld	ymm14,ymm0,7
	vpsrld	ymm0,ymm0,25
	vpor	ymm0,ymm14,ymm0
	dec	eax
	jnz	NEAR $L$oop8x

	lea	rax,[512+rsp]
	vpaddd	ymm8,ymm8,YMMWORD[((128-256))+rcx]
	vpaddd	ymm9,ymm9,YMMWORD[((160-256))+rcx]
	vpaddd	ymm10,ymm10,YMMWORD[((192-256))+rcx]
	vpaddd	ymm11,ymm11,YMMWORD[((224-256))+rcx]

	vpunpckldq	ymm14,ymm8,ymm9
	vpunpckldq	ymm15,ymm10,ymm11
	vpunpckhdq	ymm8,ymm8,ymm9
	vpunpckhdq	ymm10,ymm10,ymm11
	vpunpcklqdq	ymm9,ymm14,ymm15
	vpunpckhqdq	ymm14,ymm14,ymm15
	vpunpcklqdq	ymm11,ymm8,ymm10
	vpunpckhqdq	ymm8,ymm8,ymm10
	vpaddd	ymm0,ymm0,YMMWORD[((256-256))+rcx]
	vpaddd	ymm1,ymm1,YMMWORD[((288-256))+rcx]
	vpaddd	ymm2,ymm2,YMMWORD[((320-256))+rcx]
	vpaddd	ymm3,ymm3,YMMWORD[((352-256))+rcx]

	vpunpckldq	ymm10,ymm0,ymm1
	vpunpckldq	ymm15,ymm2,ymm3
	vpunpckhdq	ymm0,ymm0,ymm1
	vpunpckhdq	ymm2,ymm2,ymm3
	vpunpcklqdq	ymm1,ymm10,ymm15
	vpunpckhqdq	ymm10,ymm10,ymm15
	vpunpcklqdq	ymm3,ymm0,ymm2
	vpunpckhqdq	ymm0,ymm0,ymm2
	vperm2i128	ymm15,ymm9,ymm1,0x20
	vperm2i128	ymm1,ymm9,ymm1,0x31
	vperm2i128	ymm9,ymm14,ymm10,0x20
	vperm2i128	ymm10,ymm14,ymm10,0x31
	vperm2i128	ymm14,ymm11,ymm3,0x20
	vperm2i128	ymm3,ymm11,ymm3,0x31
	vperm2i128	ymm11,ymm8,ymm0,0x20
	vperm2i128	ymm0,ymm8,ymm0,0x31
	vmovdqa	YMMWORD[rsp],ymm15
	vmovdqa	YMMWORD[32+rsp],ymm9
	vmovdqa	ymm15,YMMWORD[64+rsp]
	vmovdqa	ymm9,YMMWORD[96+rsp]

	vpaddd	ymm12,ymm12,YMMWORD[((384-512))+rax]
	vpaddd	ymm13,ymm13,YMMWORD[((416-512))+rax]
	vpaddd	ymm15,ymm15,YMMWORD[((448-512))+rax]
	vpaddd	ymm9,ymm9,YMMWORD[((480-512))+rax]

	vpunpckldq	ymm2,ymm12,ymm13
	vpunpckldq	ymm8,ymm15,ymm9
	vpunpckhdq	ymm12,ymm12,ymm13
	vpunpckhdq	ymm15,ymm15,ymm9
	vpunpcklqdq	ymm13,ymm2,ymm8
	vpunpckhqdq	ymm2,ymm2,ymm8
	vpunpcklqdq	ymm9,ymm12,ymm15
	vpunpckhqdq	ymm12,ymm12,ymm15
	vpaddd	ymm4,ymm4,YMMWORD[((512-512))+rax]
	vpaddd	ymm5,ymm5,YMMWORD[((544-512))+rax]
	vpaddd	ymm6,ymm6,YMMWORD[((576-512))+rax]
	vpaddd	ymm7,ymm7,YMMWORD[((608-512))+rax]

	vpunpckldq	ymm15,ymm4,ymm5
	vpunpckldq	ymm8,ymm6,ymm7
	vpunpckhdq	ymm4,ymm4,ymm5
	vpunpckhdq	ymm6,ymm6,ymm7
	vpunpcklqdq	ymm5,ymm15,ymm8
	vpunpckhqdq	ymm15,ymm15,ymm8
	vpunpcklqdq	ymm7,ymm4,ymm6
	vpunpckhqdq	ymm4,ymm4,ymm6
	vperm2i128	ymm8,ymm13,ymm5,0x20
	vperm2i128	ymm5,ymm13,ymm5,0x31
	vperm2i128	ymm13,ymm2,ymm15,0x20
	vperm2i128	ymm15,ymm2,ymm15,0x31
	vperm2i128	ymm2,ymm9,ymm7,0x20
	vperm2i128	ymm7,ymm9,ymm7,0x31
	vperm2i128	ymm9,ymm12,ymm4,0x20
	vperm2i128	ymm4,ymm12,ymm4,0x31
	vmovdqa	ymm6,YMMWORD[rsp]
	vmovdqa	ymm12,YMMWORD[32+rsp]

	cmp	rdx,64*8
	jb	NEAR $L$tail8x

	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	lea	rsi,[128+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	lea	rdi,[128+rdi]

	vpxor	ymm12,ymm12,YMMWORD[rsi]
	vpxor	ymm13,ymm13,YMMWORD[32+rsi]
	vpxor	ymm10,ymm10,YMMWORD[64+rsi]
	vpxor	ymm15,ymm15,YMMWORD[96+rsi]
	lea	rsi,[128+rsi]
	vmovdqu	YMMWORD[rdi],ymm12
	vmovdqu	YMMWORD[32+rdi],ymm13
	vmovdqu	YMMWORD[64+rdi],ymm10
	vmovdqu	YMMWORD[96+rdi],ymm15
	lea	rdi,[128+rdi]

	vpxor	ymm14,ymm14,YMMWORD[rsi]
	vpxor	ymm2,ymm2,YMMWORD[32+rsi]
	vpxor	ymm3,ymm3,YMMWORD[64+rsi]
	vpxor	ymm7,ymm7,YMMWORD[96+rsi]
	lea	rsi,[128+rsi]
	vmovdqu	YMMWORD[rdi],ymm14
	vmovdqu	YMMWORD[32+rdi],ymm2
	vmovdqu	YMMWORD[64+rdi],ymm3
	vmovdqu	YMMWORD[96+rdi],ymm7
	lea	rdi,[128+rdi]

	vpxor	ymm11,ymm11,YMMWORD[rsi]
	vpxor	ymm9,ymm9,YMMWORD[32+rsi]
	vpxor	ymm0,ymm0,YMMWORD[64+rsi]
	vpxor	ymm4,ymm4,YMMWORD[96+rsi]
	lea	rsi,[128+rsi]
	vmovdqu	YMMWORD[rdi],ymm11
	vmovdqu	YMMWORD[32+rdi],ymm9
	vmovdqu	YMMWORD[64+rdi],ymm0
	vmovdqu	YMMWORD[96+rdi],ymm4
	lea	rdi,[128+rdi]

	sub	rdx,64*8
	jnz	NEAR $L$oop_outer8x

	jmp	NEAR $L$done8x

$L$tail8x:
	cmp	rdx,448
	jae	NEAR $L$448_or_more8x
	cmp	rdx,384
	jae	NEAR $L$384_or_more8x
	cmp	rdx,320
	jae	NEAR $L$320_or_more8x
	cmp	rdx,256
	jae	NEAR $L$256_or_more8x
	cmp	rdx,192
	jae	NEAR $L$192_or_more8x
	cmp	rdx,128
	jae	NEAR $L$128_or_more8x
	cmp	rdx,64
	jae	NEAR $L$64_or_more8x

	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm6
	vmovdqa	YMMWORD[32+rsp],ymm8
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$64_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	je	NEAR $L$done8x

	lea	rsi,[64+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm1
	lea	rdi,[64+rdi]
	sub	rdx,64
	vmovdqa	YMMWORD[32+rsp],ymm5
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$128_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	je	NEAR $L$done8x

	lea	rsi,[128+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm12
	lea	rdi,[128+rdi]
	sub	rdx,128
	vmovdqa	YMMWORD[32+rsp],ymm13
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$192_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vpxor	ymm12,ymm12,YMMWORD[128+rsi]
	vpxor	ymm13,ymm13,YMMWORD[160+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	vmovdqu	YMMWORD[128+rdi],ymm12
	vmovdqu	YMMWORD[160+rdi],ymm13
	je	NEAR $L$done8x

	lea	rsi,[192+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm10
	lea	rdi,[192+rdi]
	sub	rdx,192
	vmovdqa	YMMWORD[32+rsp],ymm15
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$256_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vpxor	ymm12,ymm12,YMMWORD[128+rsi]
	vpxor	ymm13,ymm13,YMMWORD[160+rsi]
	vpxor	ymm10,ymm10,YMMWORD[192+rsi]
	vpxor	ymm15,ymm15,YMMWORD[224+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	vmovdqu	YMMWORD[128+rdi],ymm12
	vmovdqu	YMMWORD[160+rdi],ymm13
	vmovdqu	YMMWORD[192+rdi],ymm10
	vmovdqu	YMMWORD[224+rdi],ymm15
	je	NEAR $L$done8x

	lea	rsi,[256+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm14
	lea	rdi,[256+rdi]
	sub	rdx,256
	vmovdqa	YMMWORD[32+rsp],ymm2
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$320_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vpxor	ymm12,ymm12,YMMWORD[128+rsi]
	vpxor	ymm13,ymm13,YMMWORD[160+rsi]
	vpxor	ymm10,ymm10,YMMWORD[192+rsi]
	vpxor	ymm15,ymm15,YMMWORD[224+rsi]
	vpxor	ymm14,ymm14,YMMWORD[256+rsi]
	vpxor	ymm2,ymm2,YMMWORD[288+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	vmovdqu	YMMWORD[128+rdi],ymm12
	vmovdqu	YMMWORD[160+rdi],ymm13
	vmovdqu	YMMWORD[192+rdi],ymm10
	vmovdqu	YMMWORD[224+rdi],ymm15
	vmovdqu	YMMWORD[256+rdi],ymm14
	vmovdqu	YMMWORD[288+rdi],ymm2
	je	NEAR $L$done8x

	lea	rsi,[320+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm3
	lea	rdi,[320+rdi]
	sub	rdx,320
	vmovdqa	YMMWORD[32+rsp],ymm7
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$384_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vpxor	ymm12,ymm12,YMMWORD[128+rsi]
	vpxor	ymm13,ymm13,YMMWORD[160+rsi]
	vpxor	ymm10,ymm10,YMMWORD[192+rsi]
	vpxor	ymm15,ymm15,YMMWORD[224+rsi]
	vpxor	ymm14,ymm14,YMMWORD[256+rsi]
	vpxor	ymm2,ymm2,YMMWORD[288+rsi]
	vpxor	ymm3,ymm3,YMMWORD[320+rsi]
	vpxor	ymm7,ymm7,YMMWORD[352+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	vmovdqu	YMMWORD[128+rdi],ymm12
	vmovdqu	YMMWORD[160+rdi],ymm13
	vmovdqu	YMMWORD[192+rdi],ymm10
	vmovdqu	YMMWORD[224+rdi],ymm15
	vmovdqu	YMMWORD[256+rdi],ymm14
	vmovdqu	YMMWORD[288+rdi],ymm2
	vmovdqu	YMMWORD[320+rdi],ymm3
	vmovdqu	YMMWORD[352+rdi],ymm7
	je	NEAR $L$done8x

	lea	rsi,[384+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm11
	lea	rdi,[384+rdi]
	sub	rdx,384
	vmovdqa	YMMWORD[32+rsp],ymm9
	jmp	NEAR $L$oop_tail8x

ALIGN	32
$L$448_or_more8x:
	vpxor	ymm6,ymm6,YMMWORD[rsi]
	vpxor	ymm8,ymm8,YMMWORD[32+rsi]
	vpxor	ymm1,ymm1,YMMWORD[64+rsi]
	vpxor	ymm5,ymm5,YMMWORD[96+rsi]
	vpxor	ymm12,ymm12,YMMWORD[128+rsi]
	vpxor	ymm13,ymm13,YMMWORD[160+rsi]
	vpxor	ymm10,ymm10,YMMWORD[192+rsi]
	vpxor	ymm15,ymm15,YMMWORD[224+rsi]
	vpxor	ymm14,ymm14,YMMWORD[256+rsi]
	vpxor	ymm2,ymm2,YMMWORD[288+rsi]
	vpxor	ymm3,ymm3,YMMWORD[320+rsi]
	vpxor	ymm7,ymm7,YMMWORD[352+rsi]
	vpxor	ymm11,ymm11,YMMWORD[384+rsi]
	vpxor	ymm9,ymm9,YMMWORD[416+rsi]
	vmovdqu	YMMWORD[rdi],ymm6
	vmovdqu	YMMWORD[32+rdi],ymm8
	vmovdqu	YMMWORD[64+rdi],ymm1
	vmovdqu	YMMWORD[96+rdi],ymm5
	vmovdqu	YMMWORD[128+rdi],ymm12
	vmovdqu	YMMWORD[160+rdi],ymm13
	vmovdqu	YMMWORD[192+rdi],ymm10
	vmovdqu	YMMWORD[224+rdi],ymm15
	vmovdqu	YMMWORD[256+rdi],ymm14
	vmovdqu	YMMWORD[288+rdi],ymm2
	vmovdqu	YMMWORD[320+rdi],ymm3
	vmovdqu	YMMWORD[352+rdi],ymm7
	vmovdqu	YMMWORD[384+rdi],ymm11
	vmovdqu	YMMWORD[416+rdi],ymm9
	je	NEAR $L$done8x

	lea	rsi,[448+rsi]
	xor	r10,r10
	vmovdqa	YMMWORD[rsp],ymm0
	lea	rdi,[448+rdi]
	sub	rdx,448
	vmovdqa	YMMWORD[32+rsp],ymm4

$L$oop_tail8x:
	movzx	eax,BYTE[r10*1+rsi]
	movzx	ecx,BYTE[r10*1+rsp]
	lea	r10,[1+r10]
	xor	eax,ecx
	mov	BYTE[((-1))+r10*1+rdi],al
	dec	rdx
	jnz	NEAR $L$oop_tail8x

$L$done8x:
	vzeroall
	movaps	xmm6,XMMWORD[((-168))+r9]
	movaps	xmm7,XMMWORD[((-152))+r9]
	movaps	xmm8,XMMWORD[((-136))+r9]
	movaps	xmm9,XMMWORD[((-120))+r9]
	movaps	xmm10,XMMWORD[((-104))+r9]
	movaps	xmm11,XMMWORD[((-88))+r9]
	movaps	xmm12,XMMWORD[((-72))+r9]
	movaps	xmm13,XMMWORD[((-56))+r9]
	movaps	xmm14,XMMWORD[((-40))+r9]
	movaps	xmm15,XMMWORD[((-24))+r9]
	lea	rsp,[r9]

$L$8x_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ChaCha20_ctr32_avx2:
EXTERN	__imp_RtlVirtualUnwind

ALIGN	16
se_handler:
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

	lea	r10,[$L$ctr32_body]
	cmp	rbx,r10
	jb	NEAR $L$common_seh_tail

	mov	rax,QWORD[152+r8]

	lea	r10,[$L$no_data]
	cmp	rbx,r10
	jae	NEAR $L$common_seh_tail

	lea	rax,[((64+24+48))+rax]

	mov	rbx,QWORD[((-8))+rax]
	mov	rbp,QWORD[((-16))+rax]
	mov	r12,QWORD[((-24))+rax]
	mov	r13,QWORD[((-32))+rax]
	mov	r14,QWORD[((-40))+rax]
	mov	r15,QWORD[((-48))+rax]
	mov	QWORD[144+r8],rbx
	mov	QWORD[160+r8],rbp
	mov	QWORD[216+r8],r12
	mov	QWORD[224+r8],r13
	mov	QWORD[232+r8],r14
	mov	QWORD[240+r8],r15

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



ALIGN	16
ssse3_handler:
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

	mov	rax,QWORD[192+r8]

	mov	r10d,DWORD[4+r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jae	NEAR $L$common_seh_tail

	lea	rsi,[((-40))+rax]
	lea	rdi,[512+r8]
	mov	ecx,4
	DD	0xa548f3fc

	jmp	NEAR $L$common_seh_tail



ALIGN	16
full_handler:
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

	mov	rax,QWORD[192+r8]

	mov	r10d,DWORD[4+r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jae	NEAR $L$common_seh_tail

	lea	rsi,[((-168))+rax]
	lea	rdi,[512+r8]
	mov	ecx,20
	DD	0xa548f3fc

	jmp	NEAR $L$common_seh_tail


section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_ChaCha20_ctr32_nohw wrt ..imagebase
	DD	$L$SEH_end_ChaCha20_ctr32_nohw wrt ..imagebase
	DD	$L$SEH_info_ChaCha20_ctr32_nohw wrt ..imagebase

	DD	$L$SEH_begin_ChaCha20_ctr32_ssse3_4x wrt ..imagebase
	DD	$L$SEH_end_ChaCha20_ctr32_ssse3_4x wrt ..imagebase
	DD	$L$SEH_info_ChaCha20_ctr32_ssse3_4x wrt ..imagebase
	DD	$L$SEH_begin_ChaCha20_ctr32_avx2 wrt ..imagebase
	DD	$L$SEH_end_ChaCha20_ctr32_avx2 wrt ..imagebase
	DD	$L$SEH_info_ChaCha20_ctr32_avx2 wrt ..imagebase
section	.xdata rdata align=8
ALIGN	8
$L$SEH_info_ChaCha20_ctr32_nohw:
	DB	9,0,0,0
	DD	se_handler wrt ..imagebase

$L$SEH_info_ChaCha20_ctr32_ssse3_4x:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$4x_body wrt ..imagebase,$L$4x_epilogue wrt ..imagebase
$L$SEH_info_ChaCha20_ctr32_avx2:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$8x_body wrt ..imagebase,$L$8x_epilogue wrt ..imagebase
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
