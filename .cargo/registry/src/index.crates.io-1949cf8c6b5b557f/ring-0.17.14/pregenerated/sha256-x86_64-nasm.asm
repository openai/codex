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


global	sha256_block_data_order_nohw

ALIGN	16
sha256_block_data_order_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_sha256_block_data_order_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	mov	rax,rsp

	push	rbx

	push	rbp

	push	r12

	push	r13

	push	r14

	push	r15

	shl	rdx,4
	sub	rsp,16*4+4*8
	lea	rdx,[rdx*4+rsi]
	and	rsp,-64
	mov	QWORD[((64+0))+rsp],rdi
	mov	QWORD[((64+8))+rsp],rsi
	mov	QWORD[((64+16))+rsp],rdx
	mov	QWORD[88+rsp],rax

$L$prologue:

	mov	eax,DWORD[rdi]
	mov	ebx,DWORD[4+rdi]
	mov	ecx,DWORD[8+rdi]
	mov	edx,DWORD[12+rdi]
	mov	r8d,DWORD[16+rdi]
	mov	r9d,DWORD[20+rdi]
	mov	r10d,DWORD[24+rdi]
	mov	r11d,DWORD[28+rdi]
	jmp	NEAR $L$loop

ALIGN	16
$L$loop:
	mov	edi,ebx
	lea	rbp,[K256]
	xor	edi,ecx
	mov	r12d,DWORD[rsi]
	mov	r13d,r8d
	mov	r14d,eax
	bswap	r12d
	ror	r13d,14
	mov	r15d,r9d

	xor	r13d,r8d
	ror	r14d,9
	xor	r15d,r10d

	mov	DWORD[rsp],r12d
	xor	r14d,eax
	and	r15d,r8d

	ror	r13d,5
	add	r12d,r11d
	xor	r15d,r10d

	ror	r14d,11
	xor	r13d,r8d
	add	r12d,r15d

	mov	r15d,eax
	add	r12d,DWORD[rbp]
	xor	r14d,eax

	xor	r15d,ebx
	ror	r13d,6
	mov	r11d,ebx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r11d,edi
	add	edx,r12d
	add	r11d,r12d

	lea	rbp,[4+rbp]
	add	r11d,r14d
	mov	r12d,DWORD[4+rsi]
	mov	r13d,edx
	mov	r14d,r11d
	bswap	r12d
	ror	r13d,14
	mov	edi,r8d

	xor	r13d,edx
	ror	r14d,9
	xor	edi,r9d

	mov	DWORD[4+rsp],r12d
	xor	r14d,r11d
	and	edi,edx

	ror	r13d,5
	add	r12d,r10d
	xor	edi,r9d

	ror	r14d,11
	xor	r13d,edx
	add	r12d,edi

	mov	edi,r11d
	add	r12d,DWORD[rbp]
	xor	r14d,r11d

	xor	edi,eax
	ror	r13d,6
	mov	r10d,eax

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r10d,r15d
	add	ecx,r12d
	add	r10d,r12d

	lea	rbp,[4+rbp]
	add	r10d,r14d
	mov	r12d,DWORD[8+rsi]
	mov	r13d,ecx
	mov	r14d,r10d
	bswap	r12d
	ror	r13d,14
	mov	r15d,edx

	xor	r13d,ecx
	ror	r14d,9
	xor	r15d,r8d

	mov	DWORD[8+rsp],r12d
	xor	r14d,r10d
	and	r15d,ecx

	ror	r13d,5
	add	r12d,r9d
	xor	r15d,r8d

	ror	r14d,11
	xor	r13d,ecx
	add	r12d,r15d

	mov	r15d,r10d
	add	r12d,DWORD[rbp]
	xor	r14d,r10d

	xor	r15d,r11d
	ror	r13d,6
	mov	r9d,r11d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r9d,edi
	add	ebx,r12d
	add	r9d,r12d

	lea	rbp,[4+rbp]
	add	r9d,r14d
	mov	r12d,DWORD[12+rsi]
	mov	r13d,ebx
	mov	r14d,r9d
	bswap	r12d
	ror	r13d,14
	mov	edi,ecx

	xor	r13d,ebx
	ror	r14d,9
	xor	edi,edx

	mov	DWORD[12+rsp],r12d
	xor	r14d,r9d
	and	edi,ebx

	ror	r13d,5
	add	r12d,r8d
	xor	edi,edx

	ror	r14d,11
	xor	r13d,ebx
	add	r12d,edi

	mov	edi,r9d
	add	r12d,DWORD[rbp]
	xor	r14d,r9d

	xor	edi,r10d
	ror	r13d,6
	mov	r8d,r10d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r8d,r15d
	add	eax,r12d
	add	r8d,r12d

	lea	rbp,[20+rbp]
	add	r8d,r14d
	mov	r12d,DWORD[16+rsi]
	mov	r13d,eax
	mov	r14d,r8d
	bswap	r12d
	ror	r13d,14
	mov	r15d,ebx

	xor	r13d,eax
	ror	r14d,9
	xor	r15d,ecx

	mov	DWORD[16+rsp],r12d
	xor	r14d,r8d
	and	r15d,eax

	ror	r13d,5
	add	r12d,edx
	xor	r15d,ecx

	ror	r14d,11
	xor	r13d,eax
	add	r12d,r15d

	mov	r15d,r8d
	add	r12d,DWORD[rbp]
	xor	r14d,r8d

	xor	r15d,r9d
	ror	r13d,6
	mov	edx,r9d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	edx,edi
	add	r11d,r12d
	add	edx,r12d

	lea	rbp,[4+rbp]
	add	edx,r14d
	mov	r12d,DWORD[20+rsi]
	mov	r13d,r11d
	mov	r14d,edx
	bswap	r12d
	ror	r13d,14
	mov	edi,eax

	xor	r13d,r11d
	ror	r14d,9
	xor	edi,ebx

	mov	DWORD[20+rsp],r12d
	xor	r14d,edx
	and	edi,r11d

	ror	r13d,5
	add	r12d,ecx
	xor	edi,ebx

	ror	r14d,11
	xor	r13d,r11d
	add	r12d,edi

	mov	edi,edx
	add	r12d,DWORD[rbp]
	xor	r14d,edx

	xor	edi,r8d
	ror	r13d,6
	mov	ecx,r8d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	ecx,r15d
	add	r10d,r12d
	add	ecx,r12d

	lea	rbp,[4+rbp]
	add	ecx,r14d
	mov	r12d,DWORD[24+rsi]
	mov	r13d,r10d
	mov	r14d,ecx
	bswap	r12d
	ror	r13d,14
	mov	r15d,r11d

	xor	r13d,r10d
	ror	r14d,9
	xor	r15d,eax

	mov	DWORD[24+rsp],r12d
	xor	r14d,ecx
	and	r15d,r10d

	ror	r13d,5
	add	r12d,ebx
	xor	r15d,eax

	ror	r14d,11
	xor	r13d,r10d
	add	r12d,r15d

	mov	r15d,ecx
	add	r12d,DWORD[rbp]
	xor	r14d,ecx

	xor	r15d,edx
	ror	r13d,6
	mov	ebx,edx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	ebx,edi
	add	r9d,r12d
	add	ebx,r12d

	lea	rbp,[4+rbp]
	add	ebx,r14d
	mov	r12d,DWORD[28+rsi]
	mov	r13d,r9d
	mov	r14d,ebx
	bswap	r12d
	ror	r13d,14
	mov	edi,r10d

	xor	r13d,r9d
	ror	r14d,9
	xor	edi,r11d

	mov	DWORD[28+rsp],r12d
	xor	r14d,ebx
	and	edi,r9d

	ror	r13d,5
	add	r12d,eax
	xor	edi,r11d

	ror	r14d,11
	xor	r13d,r9d
	add	r12d,edi

	mov	edi,ebx
	add	r12d,DWORD[rbp]
	xor	r14d,ebx

	xor	edi,ecx
	ror	r13d,6
	mov	eax,ecx

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	eax,r15d
	add	r8d,r12d
	add	eax,r12d

	lea	rbp,[20+rbp]
	add	eax,r14d
	mov	r12d,DWORD[32+rsi]
	mov	r13d,r8d
	mov	r14d,eax
	bswap	r12d
	ror	r13d,14
	mov	r15d,r9d

	xor	r13d,r8d
	ror	r14d,9
	xor	r15d,r10d

	mov	DWORD[32+rsp],r12d
	xor	r14d,eax
	and	r15d,r8d

	ror	r13d,5
	add	r12d,r11d
	xor	r15d,r10d

	ror	r14d,11
	xor	r13d,r8d
	add	r12d,r15d

	mov	r15d,eax
	add	r12d,DWORD[rbp]
	xor	r14d,eax

	xor	r15d,ebx
	ror	r13d,6
	mov	r11d,ebx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r11d,edi
	add	edx,r12d
	add	r11d,r12d

	lea	rbp,[4+rbp]
	add	r11d,r14d
	mov	r12d,DWORD[36+rsi]
	mov	r13d,edx
	mov	r14d,r11d
	bswap	r12d
	ror	r13d,14
	mov	edi,r8d

	xor	r13d,edx
	ror	r14d,9
	xor	edi,r9d

	mov	DWORD[36+rsp],r12d
	xor	r14d,r11d
	and	edi,edx

	ror	r13d,5
	add	r12d,r10d
	xor	edi,r9d

	ror	r14d,11
	xor	r13d,edx
	add	r12d,edi

	mov	edi,r11d
	add	r12d,DWORD[rbp]
	xor	r14d,r11d

	xor	edi,eax
	ror	r13d,6
	mov	r10d,eax

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r10d,r15d
	add	ecx,r12d
	add	r10d,r12d

	lea	rbp,[4+rbp]
	add	r10d,r14d
	mov	r12d,DWORD[40+rsi]
	mov	r13d,ecx
	mov	r14d,r10d
	bswap	r12d
	ror	r13d,14
	mov	r15d,edx

	xor	r13d,ecx
	ror	r14d,9
	xor	r15d,r8d

	mov	DWORD[40+rsp],r12d
	xor	r14d,r10d
	and	r15d,ecx

	ror	r13d,5
	add	r12d,r9d
	xor	r15d,r8d

	ror	r14d,11
	xor	r13d,ecx
	add	r12d,r15d

	mov	r15d,r10d
	add	r12d,DWORD[rbp]
	xor	r14d,r10d

	xor	r15d,r11d
	ror	r13d,6
	mov	r9d,r11d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r9d,edi
	add	ebx,r12d
	add	r9d,r12d

	lea	rbp,[4+rbp]
	add	r9d,r14d
	mov	r12d,DWORD[44+rsi]
	mov	r13d,ebx
	mov	r14d,r9d
	bswap	r12d
	ror	r13d,14
	mov	edi,ecx

	xor	r13d,ebx
	ror	r14d,9
	xor	edi,edx

	mov	DWORD[44+rsp],r12d
	xor	r14d,r9d
	and	edi,ebx

	ror	r13d,5
	add	r12d,r8d
	xor	edi,edx

	ror	r14d,11
	xor	r13d,ebx
	add	r12d,edi

	mov	edi,r9d
	add	r12d,DWORD[rbp]
	xor	r14d,r9d

	xor	edi,r10d
	ror	r13d,6
	mov	r8d,r10d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r8d,r15d
	add	eax,r12d
	add	r8d,r12d

	lea	rbp,[20+rbp]
	add	r8d,r14d
	mov	r12d,DWORD[48+rsi]
	mov	r13d,eax
	mov	r14d,r8d
	bswap	r12d
	ror	r13d,14
	mov	r15d,ebx

	xor	r13d,eax
	ror	r14d,9
	xor	r15d,ecx

	mov	DWORD[48+rsp],r12d
	xor	r14d,r8d
	and	r15d,eax

	ror	r13d,5
	add	r12d,edx
	xor	r15d,ecx

	ror	r14d,11
	xor	r13d,eax
	add	r12d,r15d

	mov	r15d,r8d
	add	r12d,DWORD[rbp]
	xor	r14d,r8d

	xor	r15d,r9d
	ror	r13d,6
	mov	edx,r9d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	edx,edi
	add	r11d,r12d
	add	edx,r12d

	lea	rbp,[4+rbp]
	add	edx,r14d
	mov	r12d,DWORD[52+rsi]
	mov	r13d,r11d
	mov	r14d,edx
	bswap	r12d
	ror	r13d,14
	mov	edi,eax

	xor	r13d,r11d
	ror	r14d,9
	xor	edi,ebx

	mov	DWORD[52+rsp],r12d
	xor	r14d,edx
	and	edi,r11d

	ror	r13d,5
	add	r12d,ecx
	xor	edi,ebx

	ror	r14d,11
	xor	r13d,r11d
	add	r12d,edi

	mov	edi,edx
	add	r12d,DWORD[rbp]
	xor	r14d,edx

	xor	edi,r8d
	ror	r13d,6
	mov	ecx,r8d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	ecx,r15d
	add	r10d,r12d
	add	ecx,r12d

	lea	rbp,[4+rbp]
	add	ecx,r14d
	mov	r12d,DWORD[56+rsi]
	mov	r13d,r10d
	mov	r14d,ecx
	bswap	r12d
	ror	r13d,14
	mov	r15d,r11d

	xor	r13d,r10d
	ror	r14d,9
	xor	r15d,eax

	mov	DWORD[56+rsp],r12d
	xor	r14d,ecx
	and	r15d,r10d

	ror	r13d,5
	add	r12d,ebx
	xor	r15d,eax

	ror	r14d,11
	xor	r13d,r10d
	add	r12d,r15d

	mov	r15d,ecx
	add	r12d,DWORD[rbp]
	xor	r14d,ecx

	xor	r15d,edx
	ror	r13d,6
	mov	ebx,edx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	ebx,edi
	add	r9d,r12d
	add	ebx,r12d

	lea	rbp,[4+rbp]
	add	ebx,r14d
	mov	r12d,DWORD[60+rsi]
	mov	r13d,r9d
	mov	r14d,ebx
	bswap	r12d
	ror	r13d,14
	mov	edi,r10d

	xor	r13d,r9d
	ror	r14d,9
	xor	edi,r11d

	mov	DWORD[60+rsp],r12d
	xor	r14d,ebx
	and	edi,r9d

	ror	r13d,5
	add	r12d,eax
	xor	edi,r11d

	ror	r14d,11
	xor	r13d,r9d
	add	r12d,edi

	mov	edi,ebx
	add	r12d,DWORD[rbp]
	xor	r14d,ebx

	xor	edi,ecx
	ror	r13d,6
	mov	eax,ecx

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	eax,r15d
	add	r8d,r12d
	add	eax,r12d

	lea	rbp,[20+rbp]
	jmp	NEAR $L$rounds_16_xx
ALIGN	16
$L$rounds_16_xx:
	mov	r13d,DWORD[4+rsp]
	mov	r15d,DWORD[56+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	eax,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[36+rsp]

	add	r12d,DWORD[rsp]
	mov	r13d,r8d
	add	r12d,r15d
	mov	r14d,eax
	ror	r13d,14
	mov	r15d,r9d

	xor	r13d,r8d
	ror	r14d,9
	xor	r15d,r10d

	mov	DWORD[rsp],r12d
	xor	r14d,eax
	and	r15d,r8d

	ror	r13d,5
	add	r12d,r11d
	xor	r15d,r10d

	ror	r14d,11
	xor	r13d,r8d
	add	r12d,r15d

	mov	r15d,eax
	add	r12d,DWORD[rbp]
	xor	r14d,eax

	xor	r15d,ebx
	ror	r13d,6
	mov	r11d,ebx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r11d,edi
	add	edx,r12d
	add	r11d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[8+rsp]
	mov	edi,DWORD[60+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r11d,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[40+rsp]

	add	r12d,DWORD[4+rsp]
	mov	r13d,edx
	add	r12d,edi
	mov	r14d,r11d
	ror	r13d,14
	mov	edi,r8d

	xor	r13d,edx
	ror	r14d,9
	xor	edi,r9d

	mov	DWORD[4+rsp],r12d
	xor	r14d,r11d
	and	edi,edx

	ror	r13d,5
	add	r12d,r10d
	xor	edi,r9d

	ror	r14d,11
	xor	r13d,edx
	add	r12d,edi

	mov	edi,r11d
	add	r12d,DWORD[rbp]
	xor	r14d,r11d

	xor	edi,eax
	ror	r13d,6
	mov	r10d,eax

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r10d,r15d
	add	ecx,r12d
	add	r10d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[12+rsp]
	mov	r15d,DWORD[rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r10d,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[44+rsp]

	add	r12d,DWORD[8+rsp]
	mov	r13d,ecx
	add	r12d,r15d
	mov	r14d,r10d
	ror	r13d,14
	mov	r15d,edx

	xor	r13d,ecx
	ror	r14d,9
	xor	r15d,r8d

	mov	DWORD[8+rsp],r12d
	xor	r14d,r10d
	and	r15d,ecx

	ror	r13d,5
	add	r12d,r9d
	xor	r15d,r8d

	ror	r14d,11
	xor	r13d,ecx
	add	r12d,r15d

	mov	r15d,r10d
	add	r12d,DWORD[rbp]
	xor	r14d,r10d

	xor	r15d,r11d
	ror	r13d,6
	mov	r9d,r11d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r9d,edi
	add	ebx,r12d
	add	r9d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[16+rsp]
	mov	edi,DWORD[4+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r9d,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[48+rsp]

	add	r12d,DWORD[12+rsp]
	mov	r13d,ebx
	add	r12d,edi
	mov	r14d,r9d
	ror	r13d,14
	mov	edi,ecx

	xor	r13d,ebx
	ror	r14d,9
	xor	edi,edx

	mov	DWORD[12+rsp],r12d
	xor	r14d,r9d
	and	edi,ebx

	ror	r13d,5
	add	r12d,r8d
	xor	edi,edx

	ror	r14d,11
	xor	r13d,ebx
	add	r12d,edi

	mov	edi,r9d
	add	r12d,DWORD[rbp]
	xor	r14d,r9d

	xor	edi,r10d
	ror	r13d,6
	mov	r8d,r10d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r8d,r15d
	add	eax,r12d
	add	r8d,r12d

	lea	rbp,[20+rbp]
	mov	r13d,DWORD[20+rsp]
	mov	r15d,DWORD[8+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r8d,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[52+rsp]

	add	r12d,DWORD[16+rsp]
	mov	r13d,eax
	add	r12d,r15d
	mov	r14d,r8d
	ror	r13d,14
	mov	r15d,ebx

	xor	r13d,eax
	ror	r14d,9
	xor	r15d,ecx

	mov	DWORD[16+rsp],r12d
	xor	r14d,r8d
	and	r15d,eax

	ror	r13d,5
	add	r12d,edx
	xor	r15d,ecx

	ror	r14d,11
	xor	r13d,eax
	add	r12d,r15d

	mov	r15d,r8d
	add	r12d,DWORD[rbp]
	xor	r14d,r8d

	xor	r15d,r9d
	ror	r13d,6
	mov	edx,r9d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	edx,edi
	add	r11d,r12d
	add	edx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[24+rsp]
	mov	edi,DWORD[12+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	edx,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[56+rsp]

	add	r12d,DWORD[20+rsp]
	mov	r13d,r11d
	add	r12d,edi
	mov	r14d,edx
	ror	r13d,14
	mov	edi,eax

	xor	r13d,r11d
	ror	r14d,9
	xor	edi,ebx

	mov	DWORD[20+rsp],r12d
	xor	r14d,edx
	and	edi,r11d

	ror	r13d,5
	add	r12d,ecx
	xor	edi,ebx

	ror	r14d,11
	xor	r13d,r11d
	add	r12d,edi

	mov	edi,edx
	add	r12d,DWORD[rbp]
	xor	r14d,edx

	xor	edi,r8d
	ror	r13d,6
	mov	ecx,r8d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	ecx,r15d
	add	r10d,r12d
	add	ecx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[28+rsp]
	mov	r15d,DWORD[16+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	ecx,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[60+rsp]

	add	r12d,DWORD[24+rsp]
	mov	r13d,r10d
	add	r12d,r15d
	mov	r14d,ecx
	ror	r13d,14
	mov	r15d,r11d

	xor	r13d,r10d
	ror	r14d,9
	xor	r15d,eax

	mov	DWORD[24+rsp],r12d
	xor	r14d,ecx
	and	r15d,r10d

	ror	r13d,5
	add	r12d,ebx
	xor	r15d,eax

	ror	r14d,11
	xor	r13d,r10d
	add	r12d,r15d

	mov	r15d,ecx
	add	r12d,DWORD[rbp]
	xor	r14d,ecx

	xor	r15d,edx
	ror	r13d,6
	mov	ebx,edx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	ebx,edi
	add	r9d,r12d
	add	ebx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[32+rsp]
	mov	edi,DWORD[20+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	ebx,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[rsp]

	add	r12d,DWORD[28+rsp]
	mov	r13d,r9d
	add	r12d,edi
	mov	r14d,ebx
	ror	r13d,14
	mov	edi,r10d

	xor	r13d,r9d
	ror	r14d,9
	xor	edi,r11d

	mov	DWORD[28+rsp],r12d
	xor	r14d,ebx
	and	edi,r9d

	ror	r13d,5
	add	r12d,eax
	xor	edi,r11d

	ror	r14d,11
	xor	r13d,r9d
	add	r12d,edi

	mov	edi,ebx
	add	r12d,DWORD[rbp]
	xor	r14d,ebx

	xor	edi,ecx
	ror	r13d,6
	mov	eax,ecx

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	eax,r15d
	add	r8d,r12d
	add	eax,r12d

	lea	rbp,[20+rbp]
	mov	r13d,DWORD[36+rsp]
	mov	r15d,DWORD[24+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	eax,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[4+rsp]

	add	r12d,DWORD[32+rsp]
	mov	r13d,r8d
	add	r12d,r15d
	mov	r14d,eax
	ror	r13d,14
	mov	r15d,r9d

	xor	r13d,r8d
	ror	r14d,9
	xor	r15d,r10d

	mov	DWORD[32+rsp],r12d
	xor	r14d,eax
	and	r15d,r8d

	ror	r13d,5
	add	r12d,r11d
	xor	r15d,r10d

	ror	r14d,11
	xor	r13d,r8d
	add	r12d,r15d

	mov	r15d,eax
	add	r12d,DWORD[rbp]
	xor	r14d,eax

	xor	r15d,ebx
	ror	r13d,6
	mov	r11d,ebx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r11d,edi
	add	edx,r12d
	add	r11d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[40+rsp]
	mov	edi,DWORD[28+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r11d,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[8+rsp]

	add	r12d,DWORD[36+rsp]
	mov	r13d,edx
	add	r12d,edi
	mov	r14d,r11d
	ror	r13d,14
	mov	edi,r8d

	xor	r13d,edx
	ror	r14d,9
	xor	edi,r9d

	mov	DWORD[36+rsp],r12d
	xor	r14d,r11d
	and	edi,edx

	ror	r13d,5
	add	r12d,r10d
	xor	edi,r9d

	ror	r14d,11
	xor	r13d,edx
	add	r12d,edi

	mov	edi,r11d
	add	r12d,DWORD[rbp]
	xor	r14d,r11d

	xor	edi,eax
	ror	r13d,6
	mov	r10d,eax

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r10d,r15d
	add	ecx,r12d
	add	r10d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[44+rsp]
	mov	r15d,DWORD[32+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r10d,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[12+rsp]

	add	r12d,DWORD[40+rsp]
	mov	r13d,ecx
	add	r12d,r15d
	mov	r14d,r10d
	ror	r13d,14
	mov	r15d,edx

	xor	r13d,ecx
	ror	r14d,9
	xor	r15d,r8d

	mov	DWORD[40+rsp],r12d
	xor	r14d,r10d
	and	r15d,ecx

	ror	r13d,5
	add	r12d,r9d
	xor	r15d,r8d

	ror	r14d,11
	xor	r13d,ecx
	add	r12d,r15d

	mov	r15d,r10d
	add	r12d,DWORD[rbp]
	xor	r14d,r10d

	xor	r15d,r11d
	ror	r13d,6
	mov	r9d,r11d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	r9d,edi
	add	ebx,r12d
	add	r9d,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[48+rsp]
	mov	edi,DWORD[36+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r9d,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[16+rsp]

	add	r12d,DWORD[44+rsp]
	mov	r13d,ebx
	add	r12d,edi
	mov	r14d,r9d
	ror	r13d,14
	mov	edi,ecx

	xor	r13d,ebx
	ror	r14d,9
	xor	edi,edx

	mov	DWORD[44+rsp],r12d
	xor	r14d,r9d
	and	edi,ebx

	ror	r13d,5
	add	r12d,r8d
	xor	edi,edx

	ror	r14d,11
	xor	r13d,ebx
	add	r12d,edi

	mov	edi,r9d
	add	r12d,DWORD[rbp]
	xor	r14d,r9d

	xor	edi,r10d
	ror	r13d,6
	mov	r8d,r10d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	r8d,r15d
	add	eax,r12d
	add	r8d,r12d

	lea	rbp,[20+rbp]
	mov	r13d,DWORD[52+rsp]
	mov	r15d,DWORD[40+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	r8d,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[20+rsp]

	add	r12d,DWORD[48+rsp]
	mov	r13d,eax
	add	r12d,r15d
	mov	r14d,r8d
	ror	r13d,14
	mov	r15d,ebx

	xor	r13d,eax
	ror	r14d,9
	xor	r15d,ecx

	mov	DWORD[48+rsp],r12d
	xor	r14d,r8d
	and	r15d,eax

	ror	r13d,5
	add	r12d,edx
	xor	r15d,ecx

	ror	r14d,11
	xor	r13d,eax
	add	r12d,r15d

	mov	r15d,r8d
	add	r12d,DWORD[rbp]
	xor	r14d,r8d

	xor	r15d,r9d
	ror	r13d,6
	mov	edx,r9d

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	edx,edi
	add	r11d,r12d
	add	edx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[56+rsp]
	mov	edi,DWORD[44+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	edx,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[24+rsp]

	add	r12d,DWORD[52+rsp]
	mov	r13d,r11d
	add	r12d,edi
	mov	r14d,edx
	ror	r13d,14
	mov	edi,eax

	xor	r13d,r11d
	ror	r14d,9
	xor	edi,ebx

	mov	DWORD[52+rsp],r12d
	xor	r14d,edx
	and	edi,r11d

	ror	r13d,5
	add	r12d,ecx
	xor	edi,ebx

	ror	r14d,11
	xor	r13d,r11d
	add	r12d,edi

	mov	edi,edx
	add	r12d,DWORD[rbp]
	xor	r14d,edx

	xor	edi,r8d
	ror	r13d,6
	mov	ecx,r8d

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	ecx,r15d
	add	r10d,r12d
	add	ecx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[60+rsp]
	mov	r15d,DWORD[48+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	ecx,r14d
	mov	r14d,r15d
	ror	r15d,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	r15d,r14d
	shr	r14d,10

	ror	r15d,17
	xor	r12d,r13d
	xor	r15d,r14d
	add	r12d,DWORD[28+rsp]

	add	r12d,DWORD[56+rsp]
	mov	r13d,r10d
	add	r12d,r15d
	mov	r14d,ecx
	ror	r13d,14
	mov	r15d,r11d

	xor	r13d,r10d
	ror	r14d,9
	xor	r15d,eax

	mov	DWORD[56+rsp],r12d
	xor	r14d,ecx
	and	r15d,r10d

	ror	r13d,5
	add	r12d,ebx
	xor	r15d,eax

	ror	r14d,11
	xor	r13d,r10d
	add	r12d,r15d

	mov	r15d,ecx
	add	r12d,DWORD[rbp]
	xor	r14d,ecx

	xor	r15d,edx
	ror	r13d,6
	mov	ebx,edx

	and	edi,r15d
	ror	r14d,2
	add	r12d,r13d

	xor	ebx,edi
	add	r9d,r12d
	add	ebx,r12d

	lea	rbp,[4+rbp]
	mov	r13d,DWORD[rsp]
	mov	edi,DWORD[52+rsp]

	mov	r12d,r13d
	ror	r13d,11
	add	ebx,r14d
	mov	r14d,edi
	ror	edi,2

	xor	r13d,r12d
	shr	r12d,3
	ror	r13d,7
	xor	edi,r14d
	shr	r14d,10

	ror	edi,17
	xor	r12d,r13d
	xor	edi,r14d
	add	r12d,DWORD[32+rsp]

	add	r12d,DWORD[60+rsp]
	mov	r13d,r9d
	add	r12d,edi
	mov	r14d,ebx
	ror	r13d,14
	mov	edi,r10d

	xor	r13d,r9d
	ror	r14d,9
	xor	edi,r11d

	mov	DWORD[60+rsp],r12d
	xor	r14d,ebx
	and	edi,r9d

	ror	r13d,5
	add	r12d,eax
	xor	edi,r11d

	ror	r14d,11
	xor	r13d,r9d
	add	r12d,edi

	mov	edi,ebx
	add	r12d,DWORD[rbp]
	xor	r14d,ebx

	xor	edi,ecx
	ror	r13d,6
	mov	eax,ecx

	and	r15d,edi
	ror	r14d,2
	add	r12d,r13d

	xor	eax,r15d
	add	r8d,r12d
	add	eax,r12d

	lea	rbp,[20+rbp]
	cmp	BYTE[3+rbp],0
	jnz	NEAR $L$rounds_16_xx

	mov	rdi,QWORD[((64+0))+rsp]
	add	eax,r14d
	lea	rsi,[64+rsi]

	add	eax,DWORD[rdi]
	add	ebx,DWORD[4+rdi]
	add	ecx,DWORD[8+rdi]
	add	edx,DWORD[12+rdi]
	add	r8d,DWORD[16+rdi]
	add	r9d,DWORD[20+rdi]
	add	r10d,DWORD[24+rdi]
	add	r11d,DWORD[28+rdi]

	cmp	rsi,QWORD[((64+16))+rsp]

	mov	DWORD[rdi],eax
	mov	DWORD[4+rdi],ebx
	mov	DWORD[8+rdi],ecx
	mov	DWORD[12+rdi],edx
	mov	DWORD[16+rdi],r8d
	mov	DWORD[20+rdi],r9d
	mov	DWORD[24+rdi],r10d
	mov	DWORD[28+rdi],r11d
	jb	NEAR $L$loop

	mov	rsi,QWORD[88+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbp,QWORD[((-16))+rsi]

	mov	rbx,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_sha256_block_data_order_nohw:
section	.rdata rdata align=8
ALIGN	64

K256:
	DD	0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5
	DD	0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5
	DD	0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5
	DD	0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5
	DD	0xd807aa98,0x12835b01,0x243185be,0x550c7dc3
	DD	0xd807aa98,0x12835b01,0x243185be,0x550c7dc3
	DD	0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174
	DD	0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174
	DD	0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc
	DD	0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc
	DD	0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da
	DD	0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da
	DD	0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7
	DD	0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7
	DD	0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967
	DD	0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967
	DD	0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13
	DD	0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13
	DD	0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85
	DD	0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85
	DD	0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3
	DD	0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3
	DD	0xd192e819,0xd6990624,0xf40e3585,0x106aa070
	DD	0xd192e819,0xd6990624,0xf40e3585,0x106aa070
	DD	0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5
	DD	0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5
	DD	0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3
	DD	0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3
	DD	0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208
	DD	0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208
	DD	0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
	DD	0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2

	DD	0x00010203,0x04050607,0x08090a0b,0x0c0d0e0f
	DD	0x00010203,0x04050607,0x08090a0b,0x0c0d0e0f
	DD	0x03020100,0x0b0a0908,0xffffffff,0xffffffff
	DD	0x03020100,0x0b0a0908,0xffffffff,0xffffffff
	DD	0xffffffff,0xffffffff,0x03020100,0x0b0a0908
	DD	0xffffffff,0xffffffff,0x03020100,0x0b0a0908
	DB	83,72,65,50,53,54,32,98,108,111,99,107,32,116,114,97
	DB	110,115,102,111,114,109,32,102,111,114,32,120,56,54,95,54
	DB	52,44,32,67,82,89,80,84,79,71,65,77,83,32,98,121
	DB	32,60,97,112,112,114,111,64,111,112,101,110,115,115,108,46
	DB	111,114,103,62,0
section	.text

global	sha256_block_data_order_hw

ALIGN	64
sha256_block_data_order_hw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_sha256_block_data_order_hw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	lea	rsp,[((-88))+rsp]
	movaps	XMMWORD[(-8-80)+rax],xmm6
	movaps	XMMWORD[(-8-64)+rax],xmm7
	movaps	XMMWORD[(-8-48)+rax],xmm8
	movaps	XMMWORD[(-8-32)+rax],xmm9
	movaps	XMMWORD[(-8-16)+rax],xmm10
$L$prologue_shaext:
	lea	rcx,[((K256+128))]
	movdqu	xmm1,XMMWORD[rdi]
	movdqu	xmm2,XMMWORD[16+rdi]
	movdqa	xmm7,XMMWORD[((512-128))+rcx]

	pshufd	xmm0,xmm1,0x1b
	pshufd	xmm1,xmm1,0xb1
	pshufd	xmm2,xmm2,0x1b
	movdqa	xmm8,xmm7
DB	102,15,58,15,202,8
	punpcklqdq	xmm2,xmm0
	jmp	NEAR $L$oop_shaext

ALIGN	16
$L$oop_shaext:
	movdqu	xmm3,XMMWORD[rsi]
	movdqu	xmm4,XMMWORD[16+rsi]
	movdqu	xmm5,XMMWORD[32+rsi]
DB	102,15,56,0,223
	movdqu	xmm6,XMMWORD[48+rsi]

	movdqa	xmm0,XMMWORD[((0-128))+rcx]
	paddd	xmm0,xmm3
DB	102,15,56,0,231
	movdqa	xmm10,xmm2
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	nop
	movdqa	xmm9,xmm1
	DB	15,56,203,202

	movdqa	xmm0,XMMWORD[((32-128))+rcx]
	paddd	xmm0,xmm4
DB	102,15,56,0,239
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	lea	rsi,[64+rsi]
	DB	15,56,204,220
	DB	15,56,203,202

	movdqa	xmm0,XMMWORD[((64-128))+rcx]
	paddd	xmm0,xmm5
DB	102,15,56,0,247
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm6
DB	102,15,58,15,253,4
	nop
	paddd	xmm3,xmm7
	DB	15,56,204,229
	DB	15,56,203,202

	movdqa	xmm0,XMMWORD[((96-128))+rcx]
	paddd	xmm0,xmm6
	DB	15,56,205,222
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm3
DB	102,15,58,15,254,4
	nop
	paddd	xmm4,xmm7
	DB	15,56,204,238
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((128-128))+rcx]
	paddd	xmm0,xmm3
	DB	15,56,205,227
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm4
DB	102,15,58,15,251,4
	nop
	paddd	xmm5,xmm7
	DB	15,56,204,243
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((160-128))+rcx]
	paddd	xmm0,xmm4
	DB	15,56,205,236
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm5
DB	102,15,58,15,252,4
	nop
	paddd	xmm6,xmm7
	DB	15,56,204,220
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((192-128))+rcx]
	paddd	xmm0,xmm5
	DB	15,56,205,245
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm6
DB	102,15,58,15,253,4
	nop
	paddd	xmm3,xmm7
	DB	15,56,204,229
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((224-128))+rcx]
	paddd	xmm0,xmm6
	DB	15,56,205,222
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm3
DB	102,15,58,15,254,4
	nop
	paddd	xmm4,xmm7
	DB	15,56,204,238
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((256-128))+rcx]
	paddd	xmm0,xmm3
	DB	15,56,205,227
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm4
DB	102,15,58,15,251,4
	nop
	paddd	xmm5,xmm7
	DB	15,56,204,243
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((288-128))+rcx]
	paddd	xmm0,xmm4
	DB	15,56,205,236
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm5
DB	102,15,58,15,252,4
	nop
	paddd	xmm6,xmm7
	DB	15,56,204,220
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((320-128))+rcx]
	paddd	xmm0,xmm5
	DB	15,56,205,245
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm6
DB	102,15,58,15,253,4
	nop
	paddd	xmm3,xmm7
	DB	15,56,204,229
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((352-128))+rcx]
	paddd	xmm0,xmm6
	DB	15,56,205,222
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm3
DB	102,15,58,15,254,4
	nop
	paddd	xmm4,xmm7
	DB	15,56,204,238
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((384-128))+rcx]
	paddd	xmm0,xmm3
	DB	15,56,205,227
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm4
DB	102,15,58,15,251,4
	nop
	paddd	xmm5,xmm7
	DB	15,56,204,243
	DB	15,56,203,202
	movdqa	xmm0,XMMWORD[((416-128))+rcx]
	paddd	xmm0,xmm4
	DB	15,56,205,236
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	movdqa	xmm7,xmm5
DB	102,15,58,15,252,4
	DB	15,56,203,202
	paddd	xmm6,xmm7

	movdqa	xmm0,XMMWORD[((448-128))+rcx]
	paddd	xmm0,xmm5
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	DB	15,56,205,245
	movdqa	xmm7,xmm8
	DB	15,56,203,202

	movdqa	xmm0,XMMWORD[((480-128))+rcx]
	paddd	xmm0,xmm6
	nop
	DB	15,56,203,209
	pshufd	xmm0,xmm0,0x0e
	dec	rdx
	nop
	DB	15,56,203,202

	paddd	xmm2,xmm10
	paddd	xmm1,xmm9
	jnz	NEAR $L$oop_shaext

	pshufd	xmm2,xmm2,0xb1
	pshufd	xmm7,xmm1,0x1b
	pshufd	xmm1,xmm1,0xb1
	punpckhqdq	xmm1,xmm2
DB	102,15,58,15,215,8

	movdqu	XMMWORD[rdi],xmm1
	movdqu	XMMWORD[16+rdi],xmm2
	movaps	xmm6,XMMWORD[((-8-80))+rax]
	movaps	xmm7,XMMWORD[((-8-64))+rax]
	movaps	xmm8,XMMWORD[((-8-48))+rax]
	movaps	xmm9,XMMWORD[((-8-32))+rax]
	movaps	xmm10,XMMWORD[((-8-16))+rax]
	mov	rsp,rax
$L$epilogue_shaext:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_sha256_block_data_order_hw:
global	sha256_block_data_order_ssse3

ALIGN	64
sha256_block_data_order_ssse3:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_sha256_block_data_order_ssse3:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	mov	rax,rsp

	push	rbx

	push	rbp

	push	r12

	push	r13

	push	r14

	push	r15

	shl	rdx,4
	sub	rsp,160
	lea	rdx,[rdx*4+rsi]
	and	rsp,-64
	mov	QWORD[((64+0))+rsp],rdi
	mov	QWORD[((64+8))+rsp],rsi
	mov	QWORD[((64+16))+rsp],rdx
	mov	QWORD[88+rsp],rax

	movaps	XMMWORD[(64+32)+rsp],xmm6
	movaps	XMMWORD[(64+48)+rsp],xmm7
	movaps	XMMWORD[(64+64)+rsp],xmm8
	movaps	XMMWORD[(64+80)+rsp],xmm9
$L$prologue_ssse3:

	mov	eax,DWORD[rdi]
	mov	ebx,DWORD[4+rdi]
	mov	ecx,DWORD[8+rdi]
	mov	edx,DWORD[12+rdi]
	mov	r8d,DWORD[16+rdi]
	mov	r9d,DWORD[20+rdi]
	mov	r10d,DWORD[24+rdi]
	mov	r11d,DWORD[28+rdi]


	jmp	NEAR $L$loop_ssse3
ALIGN	16
$L$loop_ssse3:
	movdqa	xmm7,XMMWORD[((K256+512))]
	movdqu	xmm0,XMMWORD[rsi]
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
DB	102,15,56,0,199
	movdqu	xmm3,XMMWORD[48+rsi]
	lea	rbp,[K256]
DB	102,15,56,0,207
	movdqa	xmm4,XMMWORD[rbp]
	movdqa	xmm5,XMMWORD[32+rbp]
DB	102,15,56,0,215
	paddd	xmm4,xmm0
	movdqa	xmm6,XMMWORD[64+rbp]
DB	102,15,56,0,223
	movdqa	xmm7,XMMWORD[96+rbp]
	paddd	xmm5,xmm1
	paddd	xmm6,xmm2
	paddd	xmm7,xmm3
	movdqa	XMMWORD[rsp],xmm4
	mov	r14d,eax
	movdqa	XMMWORD[16+rsp],xmm5
	mov	edi,ebx
	movdqa	XMMWORD[32+rsp],xmm6
	xor	edi,ecx
	movdqa	XMMWORD[48+rsp],xmm7
	mov	r13d,r8d
	jmp	NEAR $L$ssse3_00_47

ALIGN	16
$L$ssse3_00_47:
	sub	rbp,-128
	ror	r13d,14
	movdqa	xmm4,xmm1
	mov	eax,r14d
	mov	r12d,r9d
	movdqa	xmm7,xmm3
	ror	r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	ror	r13d,5
	xor	r14d,eax
DB	102,15,58,15,224,4
	and	r12d,r8d
	xor	r13d,r8d
DB	102,15,58,15,250,4
	add	r11d,DWORD[rsp]
	mov	r15d,eax
	xor	r12d,r10d
	ror	r14d,11
	movdqa	xmm5,xmm4
	xor	r15d,ebx
	add	r11d,r12d
	movdqa	xmm6,xmm4
	ror	r13d,6
	and	edi,r15d
	psrld	xmm4,3
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	paddd	xmm0,xmm7
	ror	r14d,2
	add	edx,r11d
	psrld	xmm6,7
	add	r11d,edi
	mov	r13d,edx
	pshufd	xmm7,xmm3,250
	add	r14d,r11d
	ror	r13d,14
	pslld	xmm5,14
	mov	r11d,r14d
	mov	r12d,r8d
	pxor	xmm4,xmm6
	ror	r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	ror	r13d,5
	psrld	xmm6,11
	xor	r14d,r11d
	pxor	xmm4,xmm5
	and	r12d,edx
	xor	r13d,edx
	pslld	xmm5,11
	add	r10d,DWORD[4+rsp]
	mov	edi,r11d
	pxor	xmm4,xmm6
	xor	r12d,r9d
	ror	r14d,11
	movdqa	xmm6,xmm7
	xor	edi,eax
	add	r10d,r12d
	pxor	xmm4,xmm5
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r11d
	psrld	xmm7,10
	add	r10d,r13d
	xor	r15d,eax
	paddd	xmm0,xmm4
	ror	r14d,2
	add	ecx,r10d
	psrlq	xmm6,17
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	pxor	xmm7,xmm6
	ror	r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	ror	r14d,9
	psrlq	xmm6,2
	xor	r13d,ecx
	xor	r12d,r8d
	pxor	xmm7,xmm6
	ror	r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	pshufd	xmm7,xmm7,128
	xor	r13d,ecx
	add	r9d,DWORD[8+rsp]
	mov	r15d,r10d
	psrldq	xmm7,8
	xor	r12d,r8d
	ror	r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	ror	r13d,6
	paddd	xmm0,xmm7
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	pshufd	xmm7,xmm0,80
	xor	edi,r11d
	ror	r14d,2
	add	ebx,r9d
	movdqa	xmm6,xmm7
	add	r9d,edi
	mov	r13d,ebx
	psrld	xmm7,10
	add	r14d,r9d
	ror	r13d,14
	psrlq	xmm6,17
	mov	r9d,r14d
	mov	r12d,ecx
	pxor	xmm7,xmm6
	ror	r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	ror	r13d,5
	xor	r14d,r9d
	psrlq	xmm6,2
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[12+rsp]
	pxor	xmm7,xmm6
	mov	edi,r9d
	xor	r12d,edx
	ror	r14d,11
	pshufd	xmm7,xmm7,8
	xor	edi,r10d
	add	r8d,r12d
	movdqa	xmm6,XMMWORD[rbp]
	ror	r13d,6
	and	r15d,edi
	pslldq	xmm7,8
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	paddd	xmm0,xmm7
	ror	r14d,2
	add	eax,r8d
	add	r8d,r15d
	paddd	xmm6,xmm0
	mov	r13d,eax
	add	r14d,r8d
	movdqa	XMMWORD[rsp],xmm6
	ror	r13d,14
	movdqa	xmm4,xmm2
	mov	r8d,r14d
	mov	r12d,ebx
	movdqa	xmm7,xmm0
	ror	r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	ror	r13d,5
	xor	r14d,r8d
DB	102,15,58,15,225,4
	and	r12d,eax
	xor	r13d,eax
DB	102,15,58,15,251,4
	add	edx,DWORD[16+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	ror	r14d,11
	movdqa	xmm5,xmm4
	xor	r15d,r9d
	add	edx,r12d
	movdqa	xmm6,xmm4
	ror	r13d,6
	and	edi,r15d
	psrld	xmm4,3
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	paddd	xmm1,xmm7
	ror	r14d,2
	add	r11d,edx
	psrld	xmm6,7
	add	edx,edi
	mov	r13d,r11d
	pshufd	xmm7,xmm0,250
	add	r14d,edx
	ror	r13d,14
	pslld	xmm5,14
	mov	edx,r14d
	mov	r12d,eax
	pxor	xmm4,xmm6
	ror	r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	ror	r13d,5
	psrld	xmm6,11
	xor	r14d,edx
	pxor	xmm4,xmm5
	and	r12d,r11d
	xor	r13d,r11d
	pslld	xmm5,11
	add	ecx,DWORD[20+rsp]
	mov	edi,edx
	pxor	xmm4,xmm6
	xor	r12d,ebx
	ror	r14d,11
	movdqa	xmm6,xmm7
	xor	edi,r8d
	add	ecx,r12d
	pxor	xmm4,xmm5
	ror	r13d,6
	and	r15d,edi
	xor	r14d,edx
	psrld	xmm7,10
	add	ecx,r13d
	xor	r15d,r8d
	paddd	xmm1,xmm4
	ror	r14d,2
	add	r10d,ecx
	psrlq	xmm6,17
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	pxor	xmm7,xmm6
	ror	r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	ror	r14d,9
	psrlq	xmm6,2
	xor	r13d,r10d
	xor	r12d,eax
	pxor	xmm7,xmm6
	ror	r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	pshufd	xmm7,xmm7,128
	xor	r13d,r10d
	add	ebx,DWORD[24+rsp]
	mov	r15d,ecx
	psrldq	xmm7,8
	xor	r12d,eax
	ror	r14d,11
	xor	r15d,edx
	add	ebx,r12d
	ror	r13d,6
	paddd	xmm1,xmm7
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	pshufd	xmm7,xmm1,80
	xor	edi,edx
	ror	r14d,2
	add	r9d,ebx
	movdqa	xmm6,xmm7
	add	ebx,edi
	mov	r13d,r9d
	psrld	xmm7,10
	add	r14d,ebx
	ror	r13d,14
	psrlq	xmm6,17
	mov	ebx,r14d
	mov	r12d,r10d
	pxor	xmm7,xmm6
	ror	r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	ror	r13d,5
	xor	r14d,ebx
	psrlq	xmm6,2
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[28+rsp]
	pxor	xmm7,xmm6
	mov	edi,ebx
	xor	r12d,r11d
	ror	r14d,11
	pshufd	xmm7,xmm7,8
	xor	edi,ecx
	add	eax,r12d
	movdqa	xmm6,XMMWORD[32+rbp]
	ror	r13d,6
	and	r15d,edi
	pslldq	xmm7,8
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	paddd	xmm1,xmm7
	ror	r14d,2
	add	r8d,eax
	add	eax,r15d
	paddd	xmm6,xmm1
	mov	r13d,r8d
	add	r14d,eax
	movdqa	XMMWORD[16+rsp],xmm6
	ror	r13d,14
	movdqa	xmm4,xmm3
	mov	eax,r14d
	mov	r12d,r9d
	movdqa	xmm7,xmm1
	ror	r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	ror	r13d,5
	xor	r14d,eax
DB	102,15,58,15,226,4
	and	r12d,r8d
	xor	r13d,r8d
DB	102,15,58,15,248,4
	add	r11d,DWORD[32+rsp]
	mov	r15d,eax
	xor	r12d,r10d
	ror	r14d,11
	movdqa	xmm5,xmm4
	xor	r15d,ebx
	add	r11d,r12d
	movdqa	xmm6,xmm4
	ror	r13d,6
	and	edi,r15d
	psrld	xmm4,3
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	paddd	xmm2,xmm7
	ror	r14d,2
	add	edx,r11d
	psrld	xmm6,7
	add	r11d,edi
	mov	r13d,edx
	pshufd	xmm7,xmm1,250
	add	r14d,r11d
	ror	r13d,14
	pslld	xmm5,14
	mov	r11d,r14d
	mov	r12d,r8d
	pxor	xmm4,xmm6
	ror	r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	ror	r13d,5
	psrld	xmm6,11
	xor	r14d,r11d
	pxor	xmm4,xmm5
	and	r12d,edx
	xor	r13d,edx
	pslld	xmm5,11
	add	r10d,DWORD[36+rsp]
	mov	edi,r11d
	pxor	xmm4,xmm6
	xor	r12d,r9d
	ror	r14d,11
	movdqa	xmm6,xmm7
	xor	edi,eax
	add	r10d,r12d
	pxor	xmm4,xmm5
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r11d
	psrld	xmm7,10
	add	r10d,r13d
	xor	r15d,eax
	paddd	xmm2,xmm4
	ror	r14d,2
	add	ecx,r10d
	psrlq	xmm6,17
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	pxor	xmm7,xmm6
	ror	r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	ror	r14d,9
	psrlq	xmm6,2
	xor	r13d,ecx
	xor	r12d,r8d
	pxor	xmm7,xmm6
	ror	r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	pshufd	xmm7,xmm7,128
	xor	r13d,ecx
	add	r9d,DWORD[40+rsp]
	mov	r15d,r10d
	psrldq	xmm7,8
	xor	r12d,r8d
	ror	r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	ror	r13d,6
	paddd	xmm2,xmm7
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	pshufd	xmm7,xmm2,80
	xor	edi,r11d
	ror	r14d,2
	add	ebx,r9d
	movdqa	xmm6,xmm7
	add	r9d,edi
	mov	r13d,ebx
	psrld	xmm7,10
	add	r14d,r9d
	ror	r13d,14
	psrlq	xmm6,17
	mov	r9d,r14d
	mov	r12d,ecx
	pxor	xmm7,xmm6
	ror	r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	ror	r13d,5
	xor	r14d,r9d
	psrlq	xmm6,2
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[44+rsp]
	pxor	xmm7,xmm6
	mov	edi,r9d
	xor	r12d,edx
	ror	r14d,11
	pshufd	xmm7,xmm7,8
	xor	edi,r10d
	add	r8d,r12d
	movdqa	xmm6,XMMWORD[64+rbp]
	ror	r13d,6
	and	r15d,edi
	pslldq	xmm7,8
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	paddd	xmm2,xmm7
	ror	r14d,2
	add	eax,r8d
	add	r8d,r15d
	paddd	xmm6,xmm2
	mov	r13d,eax
	add	r14d,r8d
	movdqa	XMMWORD[32+rsp],xmm6
	ror	r13d,14
	movdqa	xmm4,xmm0
	mov	r8d,r14d
	mov	r12d,ebx
	movdqa	xmm7,xmm2
	ror	r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	ror	r13d,5
	xor	r14d,r8d
DB	102,15,58,15,227,4
	and	r12d,eax
	xor	r13d,eax
DB	102,15,58,15,249,4
	add	edx,DWORD[48+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	ror	r14d,11
	movdqa	xmm5,xmm4
	xor	r15d,r9d
	add	edx,r12d
	movdqa	xmm6,xmm4
	ror	r13d,6
	and	edi,r15d
	psrld	xmm4,3
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	paddd	xmm3,xmm7
	ror	r14d,2
	add	r11d,edx
	psrld	xmm6,7
	add	edx,edi
	mov	r13d,r11d
	pshufd	xmm7,xmm2,250
	add	r14d,edx
	ror	r13d,14
	pslld	xmm5,14
	mov	edx,r14d
	mov	r12d,eax
	pxor	xmm4,xmm6
	ror	r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	ror	r13d,5
	psrld	xmm6,11
	xor	r14d,edx
	pxor	xmm4,xmm5
	and	r12d,r11d
	xor	r13d,r11d
	pslld	xmm5,11
	add	ecx,DWORD[52+rsp]
	mov	edi,edx
	pxor	xmm4,xmm6
	xor	r12d,ebx
	ror	r14d,11
	movdqa	xmm6,xmm7
	xor	edi,r8d
	add	ecx,r12d
	pxor	xmm4,xmm5
	ror	r13d,6
	and	r15d,edi
	xor	r14d,edx
	psrld	xmm7,10
	add	ecx,r13d
	xor	r15d,r8d
	paddd	xmm3,xmm4
	ror	r14d,2
	add	r10d,ecx
	psrlq	xmm6,17
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	pxor	xmm7,xmm6
	ror	r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	ror	r14d,9
	psrlq	xmm6,2
	xor	r13d,r10d
	xor	r12d,eax
	pxor	xmm7,xmm6
	ror	r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	pshufd	xmm7,xmm7,128
	xor	r13d,r10d
	add	ebx,DWORD[56+rsp]
	mov	r15d,ecx
	psrldq	xmm7,8
	xor	r12d,eax
	ror	r14d,11
	xor	r15d,edx
	add	ebx,r12d
	ror	r13d,6
	paddd	xmm3,xmm7
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	pshufd	xmm7,xmm3,80
	xor	edi,edx
	ror	r14d,2
	add	r9d,ebx
	movdqa	xmm6,xmm7
	add	ebx,edi
	mov	r13d,r9d
	psrld	xmm7,10
	add	r14d,ebx
	ror	r13d,14
	psrlq	xmm6,17
	mov	ebx,r14d
	mov	r12d,r10d
	pxor	xmm7,xmm6
	ror	r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	ror	r13d,5
	xor	r14d,ebx
	psrlq	xmm6,2
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[60+rsp]
	pxor	xmm7,xmm6
	mov	edi,ebx
	xor	r12d,r11d
	ror	r14d,11
	pshufd	xmm7,xmm7,8
	xor	edi,ecx
	add	eax,r12d
	movdqa	xmm6,XMMWORD[96+rbp]
	ror	r13d,6
	and	r15d,edi
	pslldq	xmm7,8
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	paddd	xmm3,xmm7
	ror	r14d,2
	add	r8d,eax
	add	eax,r15d
	paddd	xmm6,xmm3
	mov	r13d,r8d
	add	r14d,eax
	movdqa	XMMWORD[48+rsp],xmm6
	cmp	BYTE[131+rbp],0
	jne	NEAR $L$ssse3_00_47
	ror	r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	ror	r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	ror	r13d,5
	xor	r14d,eax
	and	r12d,r8d
	xor	r13d,r8d
	add	r11d,DWORD[rsp]
	mov	r15d,eax
	xor	r12d,r10d
	ror	r14d,11
	xor	r15d,ebx
	add	r11d,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	ror	r14d,2
	add	edx,r11d
	add	r11d,edi
	mov	r13d,edx
	add	r14d,r11d
	ror	r13d,14
	mov	r11d,r14d
	mov	r12d,r8d
	ror	r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	ror	r13d,5
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	add	r10d,DWORD[4+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	ror	r14d,11
	xor	edi,eax
	add	r10d,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r11d
	add	r10d,r13d
	xor	r15d,eax
	ror	r14d,2
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	ror	r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	ror	r14d,9
	xor	r13d,ecx
	xor	r12d,r8d
	ror	r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[8+rsp]
	mov	r15d,r10d
	xor	r12d,r8d
	ror	r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	xor	edi,r11d
	ror	r14d,2
	add	ebx,r9d
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	ror	r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	ror	r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	ror	r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[12+rsp]
	mov	edi,r9d
	xor	r12d,edx
	ror	r14d,11
	xor	edi,r10d
	add	r8d,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	ror	r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	ror	r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	ror	r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	ror	r13d,5
	xor	r14d,r8d
	and	r12d,eax
	xor	r13d,eax
	add	edx,DWORD[16+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	ror	r14d,11
	xor	r15d,r9d
	add	edx,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	ror	r14d,2
	add	r11d,edx
	add	edx,edi
	mov	r13d,r11d
	add	r14d,edx
	ror	r13d,14
	mov	edx,r14d
	mov	r12d,eax
	ror	r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	ror	r13d,5
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	add	ecx,DWORD[20+rsp]
	mov	edi,edx
	xor	r12d,ebx
	ror	r14d,11
	xor	edi,r8d
	add	ecx,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,edx
	add	ecx,r13d
	xor	r15d,r8d
	ror	r14d,2
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	ror	r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	ror	r14d,9
	xor	r13d,r10d
	xor	r12d,eax
	ror	r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[24+rsp]
	mov	r15d,ecx
	xor	r12d,eax
	ror	r14d,11
	xor	r15d,edx
	add	ebx,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	xor	edi,edx
	ror	r14d,2
	add	r9d,ebx
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	ror	r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	ror	r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	ror	r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[28+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	ror	r14d,11
	xor	edi,ecx
	add	eax,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	ror	r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	ror	r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	ror	r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	ror	r13d,5
	xor	r14d,eax
	and	r12d,r8d
	xor	r13d,r8d
	add	r11d,DWORD[32+rsp]
	mov	r15d,eax
	xor	r12d,r10d
	ror	r14d,11
	xor	r15d,ebx
	add	r11d,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	ror	r14d,2
	add	edx,r11d
	add	r11d,edi
	mov	r13d,edx
	add	r14d,r11d
	ror	r13d,14
	mov	r11d,r14d
	mov	r12d,r8d
	ror	r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	ror	r13d,5
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	add	r10d,DWORD[36+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	ror	r14d,11
	xor	edi,eax
	add	r10d,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r11d
	add	r10d,r13d
	xor	r15d,eax
	ror	r14d,2
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	ror	r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	ror	r14d,9
	xor	r13d,ecx
	xor	r12d,r8d
	ror	r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[40+rsp]
	mov	r15d,r10d
	xor	r12d,r8d
	ror	r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	xor	edi,r11d
	ror	r14d,2
	add	ebx,r9d
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	ror	r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	ror	r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	ror	r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[44+rsp]
	mov	edi,r9d
	xor	r12d,edx
	ror	r14d,11
	xor	edi,r10d
	add	r8d,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	ror	r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	ror	r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	ror	r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	ror	r13d,5
	xor	r14d,r8d
	and	r12d,eax
	xor	r13d,eax
	add	edx,DWORD[48+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	ror	r14d,11
	xor	r15d,r9d
	add	edx,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	ror	r14d,2
	add	r11d,edx
	add	edx,edi
	mov	r13d,r11d
	add	r14d,edx
	ror	r13d,14
	mov	edx,r14d
	mov	r12d,eax
	ror	r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	ror	r13d,5
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	add	ecx,DWORD[52+rsp]
	mov	edi,edx
	xor	r12d,ebx
	ror	r14d,11
	xor	edi,r8d
	add	ecx,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,edx
	add	ecx,r13d
	xor	r15d,r8d
	ror	r14d,2
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	ror	r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	ror	r14d,9
	xor	r13d,r10d
	xor	r12d,eax
	ror	r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[56+rsp]
	mov	r15d,ecx
	xor	r12d,eax
	ror	r14d,11
	xor	r15d,edx
	add	ebx,r12d
	ror	r13d,6
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	xor	edi,edx
	ror	r14d,2
	add	r9d,ebx
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	ror	r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	ror	r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	ror	r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[60+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	ror	r14d,11
	xor	edi,ecx
	add	eax,r12d
	ror	r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	ror	r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	mov	rdi,QWORD[((64+0))+rsp]
	mov	eax,r14d

	add	eax,DWORD[rdi]
	lea	rsi,[64+rsi]
	add	ebx,DWORD[4+rdi]
	add	ecx,DWORD[8+rdi]
	add	edx,DWORD[12+rdi]
	add	r8d,DWORD[16+rdi]
	add	r9d,DWORD[20+rdi]
	add	r10d,DWORD[24+rdi]
	add	r11d,DWORD[28+rdi]

	cmp	rsi,QWORD[((64+16))+rsp]

	mov	DWORD[rdi],eax
	mov	DWORD[4+rdi],ebx
	mov	DWORD[8+rdi],ecx
	mov	DWORD[12+rdi],edx
	mov	DWORD[16+rdi],r8d
	mov	DWORD[20+rdi],r9d
	mov	DWORD[24+rdi],r10d
	mov	DWORD[28+rdi],r11d
	jb	NEAR $L$loop_ssse3

	mov	rsi,QWORD[88+rsp]

	movaps	xmm6,XMMWORD[((64+32))+rsp]
	movaps	xmm7,XMMWORD[((64+48))+rsp]
	movaps	xmm8,XMMWORD[((64+64))+rsp]
	movaps	xmm9,XMMWORD[((64+80))+rsp]
	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbp,QWORD[((-16))+rsi]

	mov	rbx,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$epilogue_ssse3:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_sha256_block_data_order_ssse3:
global	sha256_block_data_order_avx

ALIGN	64
sha256_block_data_order_avx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_sha256_block_data_order_avx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	mov	rax,rsp

	push	rbx

	push	rbp

	push	r12

	push	r13

	push	r14

	push	r15

	shl	rdx,4
	sub	rsp,160
	lea	rdx,[rdx*4+rsi]
	and	rsp,-64
	mov	QWORD[((64+0))+rsp],rdi
	mov	QWORD[((64+8))+rsp],rsi
	mov	QWORD[((64+16))+rsp],rdx
	mov	QWORD[88+rsp],rax

	movaps	XMMWORD[(64+32)+rsp],xmm6
	movaps	XMMWORD[(64+48)+rsp],xmm7
	movaps	XMMWORD[(64+64)+rsp],xmm8
	movaps	XMMWORD[(64+80)+rsp],xmm9
$L$prologue_avx:

	vzeroupper
	mov	eax,DWORD[rdi]
	mov	ebx,DWORD[4+rdi]
	mov	ecx,DWORD[8+rdi]
	mov	edx,DWORD[12+rdi]
	mov	r8d,DWORD[16+rdi]
	mov	r9d,DWORD[20+rdi]
	mov	r10d,DWORD[24+rdi]
	mov	r11d,DWORD[28+rdi]
	vmovdqa	xmm8,XMMWORD[((K256+512+32))]
	vmovdqa	xmm9,XMMWORD[((K256+512+64))]
	jmp	NEAR $L$loop_avx
ALIGN	16
$L$loop_avx:
	vmovdqa	xmm7,XMMWORD[((K256+512))]
	vmovdqu	xmm0,XMMWORD[rsi]
	vmovdqu	xmm1,XMMWORD[16+rsi]
	vmovdqu	xmm2,XMMWORD[32+rsi]
	vmovdqu	xmm3,XMMWORD[48+rsi]
	vpshufb	xmm0,xmm0,xmm7
	lea	rbp,[K256]
	vpshufb	xmm1,xmm1,xmm7
	vpshufb	xmm2,xmm2,xmm7
	vpaddd	xmm4,xmm0,XMMWORD[rbp]
	vpshufb	xmm3,xmm3,xmm7
	vpaddd	xmm5,xmm1,XMMWORD[32+rbp]
	vpaddd	xmm6,xmm2,XMMWORD[64+rbp]
	vpaddd	xmm7,xmm3,XMMWORD[96+rbp]
	vmovdqa	XMMWORD[rsp],xmm4
	mov	r14d,eax
	vmovdqa	XMMWORD[16+rsp],xmm5
	mov	edi,ebx
	vmovdqa	XMMWORD[32+rsp],xmm6
	xor	edi,ecx
	vmovdqa	XMMWORD[48+rsp],xmm7
	mov	r13d,r8d
	jmp	NEAR $L$avx_00_47

ALIGN	16
$L$avx_00_47:
	sub	rbp,-128
	vpalignr	xmm4,xmm1,xmm0,4
	shrd	r13d,r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	vpalignr	xmm7,xmm3,xmm2,4
	shrd	r14d,r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	vpsrld	xmm6,xmm4,7
	shrd	r13d,r13d,5
	xor	r14d,eax
	and	r12d,r8d
	vpaddd	xmm0,xmm0,xmm7
	xor	r13d,r8d
	add	r11d,DWORD[rsp]
	mov	r15d,eax
	vpsrld	xmm7,xmm4,3
	xor	r12d,r10d
	shrd	r14d,r14d,11
	xor	r15d,ebx
	vpslld	xmm5,xmm4,14
	add	r11d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	vpxor	xmm4,xmm7,xmm6
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	vpshufd	xmm7,xmm3,250
	shrd	r14d,r14d,2
	add	edx,r11d
	add	r11d,edi
	vpsrld	xmm6,xmm6,11
	mov	r13d,edx
	add	r14d,r11d
	shrd	r13d,r13d,14
	vpxor	xmm4,xmm4,xmm5
	mov	r11d,r14d
	mov	r12d,r8d
	shrd	r14d,r14d,9
	vpslld	xmm5,xmm5,11
	xor	r13d,edx
	xor	r12d,r9d
	shrd	r13d,r13d,5
	vpxor	xmm4,xmm4,xmm6
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	vpsrld	xmm6,xmm7,10
	add	r10d,DWORD[4+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	vpxor	xmm4,xmm4,xmm5
	shrd	r14d,r14d,11
	xor	edi,eax
	add	r10d,r12d
	vpsrlq	xmm7,xmm7,17
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r11d
	vpaddd	xmm0,xmm0,xmm4
	add	r10d,r13d
	xor	r15d,eax
	shrd	r14d,r14d,2
	vpxor	xmm6,xmm6,xmm7
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	vpsrlq	xmm7,xmm7,2
	add	r14d,r10d
	shrd	r13d,r13d,14
	mov	r10d,r14d
	vpxor	xmm6,xmm6,xmm7
	mov	r12d,edx
	shrd	r14d,r14d,9
	xor	r13d,ecx
	vpshufb	xmm6,xmm6,xmm8
	xor	r12d,r8d
	shrd	r13d,r13d,5
	xor	r14d,r10d
	vpaddd	xmm0,xmm0,xmm6
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[8+rsp]
	vpshufd	xmm7,xmm0,80
	mov	r15d,r10d
	xor	r12d,r8d
	shrd	r14d,r14d,11
	vpsrld	xmm6,xmm7,10
	xor	r15d,r11d
	add	r9d,r12d
	shrd	r13d,r13d,6
	vpsrlq	xmm7,xmm7,17
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	vpxor	xmm6,xmm6,xmm7
	xor	edi,r11d
	shrd	r14d,r14d,2
	add	ebx,r9d
	vpsrlq	xmm7,xmm7,2
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	vpxor	xmm6,xmm6,xmm7
	shrd	r13d,r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	vpshufb	xmm6,xmm6,xmm9
	shrd	r14d,r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	vpaddd	xmm0,xmm0,xmm6
	shrd	r13d,r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	vpaddd	xmm6,xmm0,XMMWORD[rbp]
	xor	r13d,ebx
	add	r8d,DWORD[12+rsp]
	mov	edi,r9d
	xor	r12d,edx
	shrd	r14d,r14d,11
	xor	edi,r10d
	add	r8d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	shrd	r14d,r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	vmovdqa	XMMWORD[rsp],xmm6
	vpalignr	xmm4,xmm2,xmm1,4
	shrd	r13d,r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	vpalignr	xmm7,xmm0,xmm3,4
	shrd	r14d,r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	vpsrld	xmm6,xmm4,7
	shrd	r13d,r13d,5
	xor	r14d,r8d
	and	r12d,eax
	vpaddd	xmm1,xmm1,xmm7
	xor	r13d,eax
	add	edx,DWORD[16+rsp]
	mov	r15d,r8d
	vpsrld	xmm7,xmm4,3
	xor	r12d,ecx
	shrd	r14d,r14d,11
	xor	r15d,r9d
	vpslld	xmm5,xmm4,14
	add	edx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	vpxor	xmm4,xmm7,xmm6
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	vpshufd	xmm7,xmm0,250
	shrd	r14d,r14d,2
	add	r11d,edx
	add	edx,edi
	vpsrld	xmm6,xmm6,11
	mov	r13d,r11d
	add	r14d,edx
	shrd	r13d,r13d,14
	vpxor	xmm4,xmm4,xmm5
	mov	edx,r14d
	mov	r12d,eax
	shrd	r14d,r14d,9
	vpslld	xmm5,xmm5,11
	xor	r13d,r11d
	xor	r12d,ebx
	shrd	r13d,r13d,5
	vpxor	xmm4,xmm4,xmm6
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	vpsrld	xmm6,xmm7,10
	add	ecx,DWORD[20+rsp]
	mov	edi,edx
	xor	r12d,ebx
	vpxor	xmm4,xmm4,xmm5
	shrd	r14d,r14d,11
	xor	edi,r8d
	add	ecx,r12d
	vpsrlq	xmm7,xmm7,17
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,edx
	vpaddd	xmm1,xmm1,xmm4
	add	ecx,r13d
	xor	r15d,r8d
	shrd	r14d,r14d,2
	vpxor	xmm6,xmm6,xmm7
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	vpsrlq	xmm7,xmm7,2
	add	r14d,ecx
	shrd	r13d,r13d,14
	mov	ecx,r14d
	vpxor	xmm6,xmm6,xmm7
	mov	r12d,r11d
	shrd	r14d,r14d,9
	xor	r13d,r10d
	vpshufb	xmm6,xmm6,xmm8
	xor	r12d,eax
	shrd	r13d,r13d,5
	xor	r14d,ecx
	vpaddd	xmm1,xmm1,xmm6
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[24+rsp]
	vpshufd	xmm7,xmm1,80
	mov	r15d,ecx
	xor	r12d,eax
	shrd	r14d,r14d,11
	vpsrld	xmm6,xmm7,10
	xor	r15d,edx
	add	ebx,r12d
	shrd	r13d,r13d,6
	vpsrlq	xmm7,xmm7,17
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	vpxor	xmm6,xmm6,xmm7
	xor	edi,edx
	shrd	r14d,r14d,2
	add	r9d,ebx
	vpsrlq	xmm7,xmm7,2
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	vpxor	xmm6,xmm6,xmm7
	shrd	r13d,r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	vpshufb	xmm6,xmm6,xmm9
	shrd	r14d,r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	vpaddd	xmm1,xmm1,xmm6
	shrd	r13d,r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	vpaddd	xmm6,xmm1,XMMWORD[32+rbp]
	xor	r13d,r9d
	add	eax,DWORD[28+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	shrd	r14d,r14d,11
	xor	edi,ecx
	add	eax,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	shrd	r14d,r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	vmovdqa	XMMWORD[16+rsp],xmm6
	vpalignr	xmm4,xmm3,xmm2,4
	shrd	r13d,r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	vpalignr	xmm7,xmm1,xmm0,4
	shrd	r14d,r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	vpsrld	xmm6,xmm4,7
	shrd	r13d,r13d,5
	xor	r14d,eax
	and	r12d,r8d
	vpaddd	xmm2,xmm2,xmm7
	xor	r13d,r8d
	add	r11d,DWORD[32+rsp]
	mov	r15d,eax
	vpsrld	xmm7,xmm4,3
	xor	r12d,r10d
	shrd	r14d,r14d,11
	xor	r15d,ebx
	vpslld	xmm5,xmm4,14
	add	r11d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	vpxor	xmm4,xmm7,xmm6
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	vpshufd	xmm7,xmm1,250
	shrd	r14d,r14d,2
	add	edx,r11d
	add	r11d,edi
	vpsrld	xmm6,xmm6,11
	mov	r13d,edx
	add	r14d,r11d
	shrd	r13d,r13d,14
	vpxor	xmm4,xmm4,xmm5
	mov	r11d,r14d
	mov	r12d,r8d
	shrd	r14d,r14d,9
	vpslld	xmm5,xmm5,11
	xor	r13d,edx
	xor	r12d,r9d
	shrd	r13d,r13d,5
	vpxor	xmm4,xmm4,xmm6
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	vpsrld	xmm6,xmm7,10
	add	r10d,DWORD[36+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	vpxor	xmm4,xmm4,xmm5
	shrd	r14d,r14d,11
	xor	edi,eax
	add	r10d,r12d
	vpsrlq	xmm7,xmm7,17
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r11d
	vpaddd	xmm2,xmm2,xmm4
	add	r10d,r13d
	xor	r15d,eax
	shrd	r14d,r14d,2
	vpxor	xmm6,xmm6,xmm7
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	vpsrlq	xmm7,xmm7,2
	add	r14d,r10d
	shrd	r13d,r13d,14
	mov	r10d,r14d
	vpxor	xmm6,xmm6,xmm7
	mov	r12d,edx
	shrd	r14d,r14d,9
	xor	r13d,ecx
	vpshufb	xmm6,xmm6,xmm8
	xor	r12d,r8d
	shrd	r13d,r13d,5
	xor	r14d,r10d
	vpaddd	xmm2,xmm2,xmm6
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[40+rsp]
	vpshufd	xmm7,xmm2,80
	mov	r15d,r10d
	xor	r12d,r8d
	shrd	r14d,r14d,11
	vpsrld	xmm6,xmm7,10
	xor	r15d,r11d
	add	r9d,r12d
	shrd	r13d,r13d,6
	vpsrlq	xmm7,xmm7,17
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	vpxor	xmm6,xmm6,xmm7
	xor	edi,r11d
	shrd	r14d,r14d,2
	add	ebx,r9d
	vpsrlq	xmm7,xmm7,2
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	vpxor	xmm6,xmm6,xmm7
	shrd	r13d,r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	vpshufb	xmm6,xmm6,xmm9
	shrd	r14d,r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	vpaddd	xmm2,xmm2,xmm6
	shrd	r13d,r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	vpaddd	xmm6,xmm2,XMMWORD[64+rbp]
	xor	r13d,ebx
	add	r8d,DWORD[44+rsp]
	mov	edi,r9d
	xor	r12d,edx
	shrd	r14d,r14d,11
	xor	edi,r10d
	add	r8d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	shrd	r14d,r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	vmovdqa	XMMWORD[32+rsp],xmm6
	vpalignr	xmm4,xmm0,xmm3,4
	shrd	r13d,r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	vpalignr	xmm7,xmm2,xmm1,4
	shrd	r14d,r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	vpsrld	xmm6,xmm4,7
	shrd	r13d,r13d,5
	xor	r14d,r8d
	and	r12d,eax
	vpaddd	xmm3,xmm3,xmm7
	xor	r13d,eax
	add	edx,DWORD[48+rsp]
	mov	r15d,r8d
	vpsrld	xmm7,xmm4,3
	xor	r12d,ecx
	shrd	r14d,r14d,11
	xor	r15d,r9d
	vpslld	xmm5,xmm4,14
	add	edx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	vpxor	xmm4,xmm7,xmm6
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	vpshufd	xmm7,xmm2,250
	shrd	r14d,r14d,2
	add	r11d,edx
	add	edx,edi
	vpsrld	xmm6,xmm6,11
	mov	r13d,r11d
	add	r14d,edx
	shrd	r13d,r13d,14
	vpxor	xmm4,xmm4,xmm5
	mov	edx,r14d
	mov	r12d,eax
	shrd	r14d,r14d,9
	vpslld	xmm5,xmm5,11
	xor	r13d,r11d
	xor	r12d,ebx
	shrd	r13d,r13d,5
	vpxor	xmm4,xmm4,xmm6
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	vpsrld	xmm6,xmm7,10
	add	ecx,DWORD[52+rsp]
	mov	edi,edx
	xor	r12d,ebx
	vpxor	xmm4,xmm4,xmm5
	shrd	r14d,r14d,11
	xor	edi,r8d
	add	ecx,r12d
	vpsrlq	xmm7,xmm7,17
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,edx
	vpaddd	xmm3,xmm3,xmm4
	add	ecx,r13d
	xor	r15d,r8d
	shrd	r14d,r14d,2
	vpxor	xmm6,xmm6,xmm7
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	vpsrlq	xmm7,xmm7,2
	add	r14d,ecx
	shrd	r13d,r13d,14
	mov	ecx,r14d
	vpxor	xmm6,xmm6,xmm7
	mov	r12d,r11d
	shrd	r14d,r14d,9
	xor	r13d,r10d
	vpshufb	xmm6,xmm6,xmm8
	xor	r12d,eax
	shrd	r13d,r13d,5
	xor	r14d,ecx
	vpaddd	xmm3,xmm3,xmm6
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[56+rsp]
	vpshufd	xmm7,xmm3,80
	mov	r15d,ecx
	xor	r12d,eax
	shrd	r14d,r14d,11
	vpsrld	xmm6,xmm7,10
	xor	r15d,edx
	add	ebx,r12d
	shrd	r13d,r13d,6
	vpsrlq	xmm7,xmm7,17
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	vpxor	xmm6,xmm6,xmm7
	xor	edi,edx
	shrd	r14d,r14d,2
	add	r9d,ebx
	vpsrlq	xmm7,xmm7,2
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	vpxor	xmm6,xmm6,xmm7
	shrd	r13d,r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	vpshufb	xmm6,xmm6,xmm9
	shrd	r14d,r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	vpaddd	xmm3,xmm3,xmm6
	shrd	r13d,r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	vpaddd	xmm6,xmm3,XMMWORD[96+rbp]
	xor	r13d,r9d
	add	eax,DWORD[60+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	shrd	r14d,r14d,11
	xor	edi,ecx
	add	eax,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	shrd	r14d,r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	vmovdqa	XMMWORD[48+rsp],xmm6
	cmp	BYTE[131+rbp],0
	jne	NEAR $L$avx_00_47
	shrd	r13d,r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	shrd	r14d,r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	shrd	r13d,r13d,5
	xor	r14d,eax
	and	r12d,r8d
	xor	r13d,r8d
	add	r11d,DWORD[rsp]
	mov	r15d,eax
	xor	r12d,r10d
	shrd	r14d,r14d,11
	xor	r15d,ebx
	add	r11d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	shrd	r14d,r14d,2
	add	edx,r11d
	add	r11d,edi
	mov	r13d,edx
	add	r14d,r11d
	shrd	r13d,r13d,14
	mov	r11d,r14d
	mov	r12d,r8d
	shrd	r14d,r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	shrd	r13d,r13d,5
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	add	r10d,DWORD[4+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	shrd	r14d,r14d,11
	xor	edi,eax
	add	r10d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r11d
	add	r10d,r13d
	xor	r15d,eax
	shrd	r14d,r14d,2
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	shrd	r13d,r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	shrd	r14d,r14d,9
	xor	r13d,ecx
	xor	r12d,r8d
	shrd	r13d,r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[8+rsp]
	mov	r15d,r10d
	xor	r12d,r8d
	shrd	r14d,r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	xor	edi,r11d
	shrd	r14d,r14d,2
	add	ebx,r9d
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	shrd	r13d,r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	shrd	r14d,r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	shrd	r13d,r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[12+rsp]
	mov	edi,r9d
	xor	r12d,edx
	shrd	r14d,r14d,11
	xor	edi,r10d
	add	r8d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	shrd	r14d,r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	shrd	r13d,r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	shrd	r14d,r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	shrd	r13d,r13d,5
	xor	r14d,r8d
	and	r12d,eax
	xor	r13d,eax
	add	edx,DWORD[16+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	shrd	r14d,r14d,11
	xor	r15d,r9d
	add	edx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	shrd	r14d,r14d,2
	add	r11d,edx
	add	edx,edi
	mov	r13d,r11d
	add	r14d,edx
	shrd	r13d,r13d,14
	mov	edx,r14d
	mov	r12d,eax
	shrd	r14d,r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	shrd	r13d,r13d,5
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	add	ecx,DWORD[20+rsp]
	mov	edi,edx
	xor	r12d,ebx
	shrd	r14d,r14d,11
	xor	edi,r8d
	add	ecx,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,edx
	add	ecx,r13d
	xor	r15d,r8d
	shrd	r14d,r14d,2
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	shrd	r13d,r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	shrd	r14d,r14d,9
	xor	r13d,r10d
	xor	r12d,eax
	shrd	r13d,r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[24+rsp]
	mov	r15d,ecx
	xor	r12d,eax
	shrd	r14d,r14d,11
	xor	r15d,edx
	add	ebx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	xor	edi,edx
	shrd	r14d,r14d,2
	add	r9d,ebx
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	shrd	r13d,r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	shrd	r14d,r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	shrd	r13d,r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[28+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	shrd	r14d,r14d,11
	xor	edi,ecx
	add	eax,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	shrd	r14d,r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	shrd	r13d,r13d,14
	mov	eax,r14d
	mov	r12d,r9d
	shrd	r14d,r14d,9
	xor	r13d,r8d
	xor	r12d,r10d
	shrd	r13d,r13d,5
	xor	r14d,eax
	and	r12d,r8d
	xor	r13d,r8d
	add	r11d,DWORD[32+rsp]
	mov	r15d,eax
	xor	r12d,r10d
	shrd	r14d,r14d,11
	xor	r15d,ebx
	add	r11d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,eax
	add	r11d,r13d
	xor	edi,ebx
	shrd	r14d,r14d,2
	add	edx,r11d
	add	r11d,edi
	mov	r13d,edx
	add	r14d,r11d
	shrd	r13d,r13d,14
	mov	r11d,r14d
	mov	r12d,r8d
	shrd	r14d,r14d,9
	xor	r13d,edx
	xor	r12d,r9d
	shrd	r13d,r13d,5
	xor	r14d,r11d
	and	r12d,edx
	xor	r13d,edx
	add	r10d,DWORD[36+rsp]
	mov	edi,r11d
	xor	r12d,r9d
	shrd	r14d,r14d,11
	xor	edi,eax
	add	r10d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r11d
	add	r10d,r13d
	xor	r15d,eax
	shrd	r14d,r14d,2
	add	ecx,r10d
	add	r10d,r15d
	mov	r13d,ecx
	add	r14d,r10d
	shrd	r13d,r13d,14
	mov	r10d,r14d
	mov	r12d,edx
	shrd	r14d,r14d,9
	xor	r13d,ecx
	xor	r12d,r8d
	shrd	r13d,r13d,5
	xor	r14d,r10d
	and	r12d,ecx
	xor	r13d,ecx
	add	r9d,DWORD[40+rsp]
	mov	r15d,r10d
	xor	r12d,r8d
	shrd	r14d,r14d,11
	xor	r15d,r11d
	add	r9d,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,r10d
	add	r9d,r13d
	xor	edi,r11d
	shrd	r14d,r14d,2
	add	ebx,r9d
	add	r9d,edi
	mov	r13d,ebx
	add	r14d,r9d
	shrd	r13d,r13d,14
	mov	r9d,r14d
	mov	r12d,ecx
	shrd	r14d,r14d,9
	xor	r13d,ebx
	xor	r12d,edx
	shrd	r13d,r13d,5
	xor	r14d,r9d
	and	r12d,ebx
	xor	r13d,ebx
	add	r8d,DWORD[44+rsp]
	mov	edi,r9d
	xor	r12d,edx
	shrd	r14d,r14d,11
	xor	edi,r10d
	add	r8d,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,r9d
	add	r8d,r13d
	xor	r15d,r10d
	shrd	r14d,r14d,2
	add	eax,r8d
	add	r8d,r15d
	mov	r13d,eax
	add	r14d,r8d
	shrd	r13d,r13d,14
	mov	r8d,r14d
	mov	r12d,ebx
	shrd	r14d,r14d,9
	xor	r13d,eax
	xor	r12d,ecx
	shrd	r13d,r13d,5
	xor	r14d,r8d
	and	r12d,eax
	xor	r13d,eax
	add	edx,DWORD[48+rsp]
	mov	r15d,r8d
	xor	r12d,ecx
	shrd	r14d,r14d,11
	xor	r15d,r9d
	add	edx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,r8d
	add	edx,r13d
	xor	edi,r9d
	shrd	r14d,r14d,2
	add	r11d,edx
	add	edx,edi
	mov	r13d,r11d
	add	r14d,edx
	shrd	r13d,r13d,14
	mov	edx,r14d
	mov	r12d,eax
	shrd	r14d,r14d,9
	xor	r13d,r11d
	xor	r12d,ebx
	shrd	r13d,r13d,5
	xor	r14d,edx
	and	r12d,r11d
	xor	r13d,r11d
	add	ecx,DWORD[52+rsp]
	mov	edi,edx
	xor	r12d,ebx
	shrd	r14d,r14d,11
	xor	edi,r8d
	add	ecx,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,edx
	add	ecx,r13d
	xor	r15d,r8d
	shrd	r14d,r14d,2
	add	r10d,ecx
	add	ecx,r15d
	mov	r13d,r10d
	add	r14d,ecx
	shrd	r13d,r13d,14
	mov	ecx,r14d
	mov	r12d,r11d
	shrd	r14d,r14d,9
	xor	r13d,r10d
	xor	r12d,eax
	shrd	r13d,r13d,5
	xor	r14d,ecx
	and	r12d,r10d
	xor	r13d,r10d
	add	ebx,DWORD[56+rsp]
	mov	r15d,ecx
	xor	r12d,eax
	shrd	r14d,r14d,11
	xor	r15d,edx
	add	ebx,r12d
	shrd	r13d,r13d,6
	and	edi,r15d
	xor	r14d,ecx
	add	ebx,r13d
	xor	edi,edx
	shrd	r14d,r14d,2
	add	r9d,ebx
	add	ebx,edi
	mov	r13d,r9d
	add	r14d,ebx
	shrd	r13d,r13d,14
	mov	ebx,r14d
	mov	r12d,r10d
	shrd	r14d,r14d,9
	xor	r13d,r9d
	xor	r12d,r11d
	shrd	r13d,r13d,5
	xor	r14d,ebx
	and	r12d,r9d
	xor	r13d,r9d
	add	eax,DWORD[60+rsp]
	mov	edi,ebx
	xor	r12d,r11d
	shrd	r14d,r14d,11
	xor	edi,ecx
	add	eax,r12d
	shrd	r13d,r13d,6
	and	r15d,edi
	xor	r14d,ebx
	add	eax,r13d
	xor	r15d,ecx
	shrd	r14d,r14d,2
	add	r8d,eax
	add	eax,r15d
	mov	r13d,r8d
	add	r14d,eax
	mov	rdi,QWORD[((64+0))+rsp]
	mov	eax,r14d

	add	eax,DWORD[rdi]
	lea	rsi,[64+rsi]
	add	ebx,DWORD[4+rdi]
	add	ecx,DWORD[8+rdi]
	add	edx,DWORD[12+rdi]
	add	r8d,DWORD[16+rdi]
	add	r9d,DWORD[20+rdi]
	add	r10d,DWORD[24+rdi]
	add	r11d,DWORD[28+rdi]

	cmp	rsi,QWORD[((64+16))+rsp]

	mov	DWORD[rdi],eax
	mov	DWORD[4+rdi],ebx
	mov	DWORD[8+rdi],ecx
	mov	DWORD[12+rdi],edx
	mov	DWORD[16+rdi],r8d
	mov	DWORD[20+rdi],r9d
	mov	DWORD[24+rdi],r10d
	mov	DWORD[28+rdi],r11d
	jb	NEAR $L$loop_avx

	mov	rsi,QWORD[88+rsp]

	vzeroupper
	movaps	xmm6,XMMWORD[((64+32))+rsp]
	movaps	xmm7,XMMWORD[((64+48))+rsp]
	movaps	xmm8,XMMWORD[((64+64))+rsp]
	movaps	xmm9,XMMWORD[((64+80))+rsp]
	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbp,QWORD[((-16))+rsi]

	mov	rbx,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$epilogue_avx:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_sha256_block_data_order_avx:
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

	mov	r10d,DWORD[r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jb	NEAR $L$in_prologue

	mov	rax,QWORD[152+r8]

	mov	r10d,DWORD[4+r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jae	NEAR $L$in_prologue
	mov	rsi,rax
	mov	rax,QWORD[((64+24))+rax]

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

	lea	r10,[$L$epilogue]
	cmp	rbx,r10
	jb	NEAR $L$in_prologue

	lea	rsi,[((64+32))+rsi]
	lea	rdi,[512+r8]
	mov	ecx,8
	DD	0xa548f3fc

$L$in_prologue:
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
shaext_handler:
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

	lea	r10,[$L$prologue_shaext]
	cmp	rbx,r10
	jb	NEAR $L$in_prologue

	lea	r10,[$L$epilogue_shaext]
	cmp	rbx,r10
	jae	NEAR $L$in_prologue

	lea	rsi,[((-8-80))+rax]
	lea	rdi,[512+r8]
	mov	ecx,10
	DD	0xa548f3fc

	jmp	NEAR $L$in_prologue

section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_sha256_block_data_order_nohw wrt ..imagebase
	DD	$L$SEH_end_sha256_block_data_order_nohw wrt ..imagebase
	DD	$L$SEH_info_sha256_block_data_order_nohw wrt ..imagebase
	DD	$L$SEH_begin_sha256_block_data_order_hw wrt ..imagebase
	DD	$L$SEH_end_sha256_block_data_order_hw wrt ..imagebase
	DD	$L$SEH_info_sha256_block_data_order_hw wrt ..imagebase
	DD	$L$SEH_begin_sha256_block_data_order_ssse3 wrt ..imagebase
	DD	$L$SEH_end_sha256_block_data_order_ssse3 wrt ..imagebase
	DD	$L$SEH_info_sha256_block_data_order_ssse3 wrt ..imagebase
	DD	$L$SEH_begin_sha256_block_data_order_avx wrt ..imagebase
	DD	$L$SEH_end_sha256_block_data_order_avx wrt ..imagebase
	DD	$L$SEH_info_sha256_block_data_order_avx wrt ..imagebase
section	.xdata rdata align=8
ALIGN	8
$L$SEH_info_sha256_block_data_order_nohw:
	DB	9,0,0,0
	DD	se_handler wrt ..imagebase
	DD	$L$prologue wrt ..imagebase,$L$epilogue wrt ..imagebase
$L$SEH_info_sha256_block_data_order_hw:
	DB	9,0,0,0
	DD	shaext_handler wrt ..imagebase
$L$SEH_info_sha256_block_data_order_ssse3:
	DB	9,0,0,0
	DD	se_handler wrt ..imagebase
	DD	$L$prologue_ssse3 wrt ..imagebase,$L$epilogue_ssse3 wrt ..imagebase
$L$SEH_info_sha256_block_data_order_avx:
	DB	9,0,0,0
	DD	se_handler wrt ..imagebase
	DD	$L$prologue_avx wrt ..imagebase,$L$epilogue_avx wrt ..imagebase
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
