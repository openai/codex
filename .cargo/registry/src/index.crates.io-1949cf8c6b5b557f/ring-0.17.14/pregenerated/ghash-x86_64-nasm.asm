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

global	gcm_init_clmul

ALIGN	16
gcm_init_clmul:

$L$SEH_begin_gcm_init_clmul_1:
_CET_ENDBR
$L$_init_clmul:
	sub	rsp,0x18
$L$SEH_prologue_gcm_init_clmul_2:
	movaps	XMMWORD[rsp],xmm6
$L$SEH_prologue_gcm_init_clmul_3:
$L$SEH_endprologue_gcm_init_clmul_4:
	movdqu	xmm2,XMMWORD[rdx]
	pshufd	xmm2,xmm2,78


	pshufd	xmm4,xmm2,255
	movdqa	xmm3,xmm2
	psllq	xmm2,1
	pxor	xmm5,xmm5
	psrlq	xmm3,63
	pcmpgtd	xmm5,xmm4
	pslldq	xmm3,8
	por	xmm2,xmm3


	pand	xmm5,XMMWORD[$L$0x1c2_polynomial]
	pxor	xmm2,xmm5


	pshufd	xmm6,xmm2,78
	movdqa	xmm0,xmm2
	pxor	xmm6,xmm2
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pxor	xmm3,xmm0
DB	102,15,58,68,194,0
DB	102,15,58,68,202,17
DB	102,15,58,68,222,0
	pxor	xmm3,xmm0
	pxor	xmm3,xmm1

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
	movdqu	XMMWORD[rcx],xmm2
	pxor	xmm4,xmm0
	movdqu	XMMWORD[16+rcx],xmm0
DB	102,15,58,15,227,8
	movdqu	XMMWORD[32+rcx],xmm4
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pxor	xmm3,xmm0
DB	102,15,58,68,194,0
DB	102,15,58,68,202,17
DB	102,15,58,68,222,0
	pxor	xmm3,xmm0
	pxor	xmm3,xmm1

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
	movdqa	xmm5,xmm0
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pxor	xmm3,xmm0
DB	102,15,58,68,194,0
DB	102,15,58,68,202,17
DB	102,15,58,68,222,0
	pxor	xmm3,xmm0
	pxor	xmm3,xmm1

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
	pshufd	xmm3,xmm5,78
	pshufd	xmm4,xmm0,78
	pxor	xmm3,xmm5
	movdqu	XMMWORD[48+rcx],xmm5
	pxor	xmm4,xmm0
	movdqu	XMMWORD[64+rcx],xmm0
DB	102,15,58,15,227,8
	movdqu	XMMWORD[80+rcx],xmm4
	movaps	xmm6,XMMWORD[rsp]
	lea	rsp,[24+rsp]
	ret

$L$SEH_end_gcm_init_clmul_5:

global	gcm_ghash_clmul

ALIGN	32
gcm_ghash_clmul:

$L$SEH_begin_gcm_ghash_clmul_1:
_CET_ENDBR
$L$_ghash_clmul:
	lea	rax,[((-136))+rsp]
	lea	rsp,[((-32))+rax]
$L$SEH_prologue_gcm_ghash_clmul_2:
	movaps	XMMWORD[(-32)+rax],xmm6
$L$SEH_prologue_gcm_ghash_clmul_3:
	movaps	XMMWORD[(-16)+rax],xmm7
$L$SEH_prologue_gcm_ghash_clmul_4:
	movaps	XMMWORD[rax],xmm8
$L$SEH_prologue_gcm_ghash_clmul_5:
	movaps	XMMWORD[16+rax],xmm9
$L$SEH_prologue_gcm_ghash_clmul_6:
	movaps	XMMWORD[32+rax],xmm10
$L$SEH_prologue_gcm_ghash_clmul_7:
	movaps	XMMWORD[48+rax],xmm11
$L$SEH_prologue_gcm_ghash_clmul_8:
	movaps	XMMWORD[64+rax],xmm12
$L$SEH_prologue_gcm_ghash_clmul_9:
	movaps	XMMWORD[80+rax],xmm13
$L$SEH_prologue_gcm_ghash_clmul_10:
	movaps	XMMWORD[96+rax],xmm14
$L$SEH_prologue_gcm_ghash_clmul_11:
	movaps	XMMWORD[112+rax],xmm15
$L$SEH_prologue_gcm_ghash_clmul_12:
$L$SEH_endprologue_gcm_ghash_clmul_13:
	movdqa	xmm10,XMMWORD[$L$bswap_mask]

	movdqu	xmm0,XMMWORD[rcx]
	movdqu	xmm2,XMMWORD[rdx]
	movdqu	xmm7,XMMWORD[32+rdx]
DB	102,65,15,56,0,194

	sub	r9,0x10
	jz	NEAR $L$odd_tail

	movdqu	xmm6,XMMWORD[16+rdx]
	cmp	r9,0x30
	jb	NEAR $L$skip4x

	sub	r9,0x30
	mov	rax,0xA040608020C0E000
	movdqu	xmm14,XMMWORD[48+rdx]
	movdqu	xmm15,XMMWORD[64+rdx]




	movdqu	xmm3,XMMWORD[48+r8]
	movdqu	xmm11,XMMWORD[32+r8]
DB	102,65,15,56,0,218
DB	102,69,15,56,0,218
	movdqa	xmm5,xmm3
	pshufd	xmm4,xmm3,78
	pxor	xmm4,xmm3
DB	102,15,58,68,218,0
DB	102,15,58,68,234,17
DB	102,15,58,68,231,0

	movdqa	xmm13,xmm11
	pshufd	xmm12,xmm11,78
	pxor	xmm12,xmm11
DB	102,68,15,58,68,222,0
DB	102,68,15,58,68,238,17
DB	102,68,15,58,68,231,16
	xorps	xmm3,xmm11
	xorps	xmm5,xmm13
	movups	xmm7,XMMWORD[80+rdx]
	xorps	xmm4,xmm12

	movdqu	xmm11,XMMWORD[16+r8]
	movdqu	xmm8,XMMWORD[r8]
DB	102,69,15,56,0,218
DB	102,69,15,56,0,194
	movdqa	xmm13,xmm11
	pshufd	xmm12,xmm11,78
	pxor	xmm0,xmm8
	pxor	xmm12,xmm11
DB	102,69,15,58,68,222,0
	movdqa	xmm1,xmm0
	pshufd	xmm8,xmm0,78
	pxor	xmm8,xmm0
DB	102,69,15,58,68,238,17
DB	102,68,15,58,68,231,0
	xorps	xmm3,xmm11
	xorps	xmm5,xmm13

	lea	r8,[64+r8]
	sub	r9,0x40
	jc	NEAR $L$tail4x

	jmp	NEAR $L$mod4_loop
ALIGN	32
$L$mod4_loop:
DB	102,65,15,58,68,199,0
	xorps	xmm4,xmm12
	movdqu	xmm11,XMMWORD[48+r8]
DB	102,69,15,56,0,218
DB	102,65,15,58,68,207,17
	xorps	xmm0,xmm3
	movdqu	xmm3,XMMWORD[32+r8]
	movdqa	xmm13,xmm11
DB	102,68,15,58,68,199,16
	pshufd	xmm12,xmm11,78
	xorps	xmm1,xmm5
	pxor	xmm12,xmm11
DB	102,65,15,56,0,218
	movups	xmm7,XMMWORD[32+rdx]
	xorps	xmm8,xmm4
DB	102,68,15,58,68,218,0
	pshufd	xmm4,xmm3,78

	pxor	xmm8,xmm0
	movdqa	xmm5,xmm3
	pxor	xmm8,xmm1
	pxor	xmm4,xmm3
	movdqa	xmm9,xmm8
DB	102,68,15,58,68,234,17
	pslldq	xmm8,8
	psrldq	xmm9,8
	pxor	xmm0,xmm8
	movdqa	xmm8,XMMWORD[$L$7_mask]
	pxor	xmm1,xmm9
DB	102,76,15,110,200

	pand	xmm8,xmm0
DB	102,69,15,56,0,200
	pxor	xmm9,xmm0
DB	102,68,15,58,68,231,0
	psllq	xmm9,57
	movdqa	xmm8,xmm9
	pslldq	xmm9,8
DB	102,15,58,68,222,0
	psrldq	xmm8,8
	pxor	xmm0,xmm9
	pxor	xmm1,xmm8
	movdqu	xmm8,XMMWORD[r8]

	movdqa	xmm9,xmm0
	psrlq	xmm0,1
DB	102,15,58,68,238,17
	xorps	xmm3,xmm11
	movdqu	xmm11,XMMWORD[16+r8]
DB	102,69,15,56,0,218
DB	102,15,58,68,231,16
	xorps	xmm5,xmm13
	movups	xmm7,XMMWORD[80+rdx]
DB	102,69,15,56,0,194
	pxor	xmm1,xmm9
	pxor	xmm9,xmm0
	psrlq	xmm0,5

	movdqa	xmm13,xmm11
	pxor	xmm4,xmm12
	pshufd	xmm12,xmm11,78
	pxor	xmm0,xmm9
	pxor	xmm1,xmm8
	pxor	xmm12,xmm11
DB	102,69,15,58,68,222,0
	psrlq	xmm0,1
	pxor	xmm0,xmm1
	movdqa	xmm1,xmm0
DB	102,69,15,58,68,238,17
	xorps	xmm3,xmm11
	pshufd	xmm8,xmm0,78
	pxor	xmm8,xmm0

DB	102,68,15,58,68,231,0
	xorps	xmm5,xmm13

	lea	r8,[64+r8]
	sub	r9,0x40
	jnc	NEAR $L$mod4_loop

$L$tail4x:
DB	102,65,15,58,68,199,0
DB	102,65,15,58,68,207,17
DB	102,68,15,58,68,199,16
	xorps	xmm4,xmm12
	xorps	xmm0,xmm3
	xorps	xmm1,xmm5
	pxor	xmm1,xmm0
	pxor	xmm8,xmm4

	pxor	xmm8,xmm1
	pxor	xmm1,xmm0

	movdqa	xmm9,xmm8
	psrldq	xmm8,8
	pslldq	xmm9,8
	pxor	xmm1,xmm8
	pxor	xmm0,xmm9

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
	add	r9,0x40
	jz	NEAR $L$done
	movdqu	xmm7,XMMWORD[32+rdx]
	sub	r9,0x10
	jz	NEAR $L$odd_tail
$L$skip4x:





	movdqu	xmm8,XMMWORD[r8]
	movdqu	xmm3,XMMWORD[16+r8]
DB	102,69,15,56,0,194
DB	102,65,15,56,0,218
	pxor	xmm0,xmm8

	movdqa	xmm5,xmm3
	pshufd	xmm4,xmm3,78
	pxor	xmm4,xmm3
DB	102,15,58,68,218,0
DB	102,15,58,68,234,17
DB	102,15,58,68,231,0

	lea	r8,[32+r8]
	nop
	sub	r9,0x20
	jbe	NEAR $L$even_tail
	nop
	jmp	NEAR $L$mod_loop

ALIGN	32
$L$mod_loop:
	movdqa	xmm1,xmm0
	movdqa	xmm8,xmm4
	pshufd	xmm4,xmm0,78
	pxor	xmm4,xmm0

DB	102,15,58,68,198,0
DB	102,15,58,68,206,17
DB	102,15,58,68,231,16

	pxor	xmm0,xmm3
	pxor	xmm1,xmm5
	movdqu	xmm9,XMMWORD[r8]
	pxor	xmm8,xmm0
DB	102,69,15,56,0,202
	movdqu	xmm3,XMMWORD[16+r8]

	pxor	xmm8,xmm1
	pxor	xmm1,xmm9
	pxor	xmm4,xmm8
DB	102,65,15,56,0,218
	movdqa	xmm8,xmm4
	psrldq	xmm8,8
	pslldq	xmm4,8
	pxor	xmm1,xmm8
	pxor	xmm0,xmm4

	movdqa	xmm5,xmm3

	movdqa	xmm9,xmm0
	movdqa	xmm8,xmm0
	psllq	xmm0,5
	pxor	xmm8,xmm0
DB	102,15,58,68,218,0
	psllq	xmm0,1
	pxor	xmm0,xmm8
	psllq	xmm0,57
	movdqa	xmm8,xmm0
	pslldq	xmm0,8
	psrldq	xmm8,8
	pxor	xmm0,xmm9
	pshufd	xmm4,xmm5,78
	pxor	xmm1,xmm8
	pxor	xmm4,xmm5

	movdqa	xmm9,xmm0
	psrlq	xmm0,1
DB	102,15,58,68,234,17
	pxor	xmm1,xmm9
	pxor	xmm9,xmm0
	psrlq	xmm0,5
	pxor	xmm0,xmm9
	lea	r8,[32+r8]
	psrlq	xmm0,1
DB	102,15,58,68,231,0
	pxor	xmm0,xmm1

	sub	r9,0x20
	ja	NEAR $L$mod_loop

$L$even_tail:
	movdqa	xmm1,xmm0
	movdqa	xmm8,xmm4
	pshufd	xmm4,xmm0,78
	pxor	xmm4,xmm0

DB	102,15,58,68,198,0
DB	102,15,58,68,206,17
DB	102,15,58,68,231,16

	pxor	xmm0,xmm3
	pxor	xmm1,xmm5
	pxor	xmm8,xmm0
	pxor	xmm8,xmm1
	pxor	xmm4,xmm8
	movdqa	xmm8,xmm4
	psrldq	xmm8,8
	pslldq	xmm4,8
	pxor	xmm1,xmm8
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
	test	r9,r9
	jnz	NEAR $L$done

$L$odd_tail:
	movdqu	xmm8,XMMWORD[r8]
DB	102,69,15,56,0,194
	pxor	xmm0,xmm8
	movdqa	xmm1,xmm0
	pshufd	xmm3,xmm0,78
	pxor	xmm3,xmm0
DB	102,15,58,68,194,0
DB	102,15,58,68,202,17
DB	102,15,58,68,223,0
	pxor	xmm3,xmm0
	pxor	xmm3,xmm1

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
$L$done:
DB	102,65,15,56,0,194
	movdqu	XMMWORD[rcx],xmm0
	movaps	xmm6,XMMWORD[rsp]
	movaps	xmm7,XMMWORD[16+rsp]
	movaps	xmm8,XMMWORD[32+rsp]
	movaps	xmm9,XMMWORD[48+rsp]
	movaps	xmm10,XMMWORD[64+rsp]
	movaps	xmm11,XMMWORD[80+rsp]
	movaps	xmm12,XMMWORD[96+rsp]
	movaps	xmm13,XMMWORD[112+rsp]
	movaps	xmm14,XMMWORD[128+rsp]
	movaps	xmm15,XMMWORD[144+rsp]
	lea	rsp,[168+rsp]
	ret

$L$SEH_end_gcm_ghash_clmul_14:

global	gcm_init_avx

ALIGN	32
gcm_init_avx:

$L$SEH_begin_gcm_init_avx_1:
_CET_ENDBR
	sub	rsp,0x18
$L$SEH_prologue_gcm_init_avx_2:
	movaps	XMMWORD[rsp],xmm6
$L$SEH_prologue_gcm_init_avx_3:
$L$SEH_endprologue_gcm_init_avx_4:
	vzeroupper

	vmovdqu	xmm2,XMMWORD[rdx]
	vpshufd	xmm2,xmm2,78


	vpshufd	xmm4,xmm2,255
	vpsrlq	xmm3,xmm2,63
	vpsllq	xmm2,xmm2,1
	vpxor	xmm5,xmm5,xmm5
	vpcmpgtd	xmm5,xmm5,xmm4
	vpslldq	xmm3,xmm3,8
	vpor	xmm2,xmm2,xmm3


	vpand	xmm5,xmm5,XMMWORD[$L$0x1c2_polynomial]
	vpxor	xmm2,xmm2,xmm5

	vpunpckhqdq	xmm6,xmm2,xmm2
	vmovdqa	xmm0,xmm2
	vpxor	xmm6,xmm6,xmm2
	mov	r10,4
	jmp	NEAR $L$init_start_avx
ALIGN	32
$L$init_loop_avx:
	vpalignr	xmm5,xmm4,xmm3,8
	vmovdqu	XMMWORD[(-16)+rcx],xmm5
	vpunpckhqdq	xmm3,xmm0,xmm0
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm1,xmm0,xmm2,0x11
	vpclmulqdq	xmm0,xmm0,xmm2,0x00
	vpclmulqdq	xmm3,xmm3,xmm6,0x00
	vpxor	xmm4,xmm1,xmm0
	vpxor	xmm3,xmm3,xmm4

	vpslldq	xmm4,xmm3,8
	vpsrldq	xmm3,xmm3,8
	vpxor	xmm0,xmm0,xmm4
	vpxor	xmm1,xmm1,xmm3
	vpsllq	xmm3,xmm0,57
	vpsllq	xmm4,xmm0,62
	vpxor	xmm4,xmm4,xmm3
	vpsllq	xmm3,xmm0,63
	vpxor	xmm4,xmm4,xmm3
	vpslldq	xmm3,xmm4,8
	vpsrldq	xmm4,xmm4,8
	vpxor	xmm0,xmm0,xmm3
	vpxor	xmm1,xmm1,xmm4

	vpsrlq	xmm4,xmm0,1
	vpxor	xmm1,xmm1,xmm0
	vpxor	xmm0,xmm0,xmm4
	vpsrlq	xmm4,xmm4,5
	vpxor	xmm0,xmm0,xmm4
	vpsrlq	xmm0,xmm0,1
	vpxor	xmm0,xmm0,xmm1
$L$init_start_avx:
	vmovdqa	xmm5,xmm0
	vpunpckhqdq	xmm3,xmm0,xmm0
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm1,xmm0,xmm2,0x11
	vpclmulqdq	xmm0,xmm0,xmm2,0x00
	vpclmulqdq	xmm3,xmm3,xmm6,0x00
	vpxor	xmm4,xmm1,xmm0
	vpxor	xmm3,xmm3,xmm4

	vpslldq	xmm4,xmm3,8
	vpsrldq	xmm3,xmm3,8
	vpxor	xmm0,xmm0,xmm4
	vpxor	xmm1,xmm1,xmm3
	vpsllq	xmm3,xmm0,57
	vpsllq	xmm4,xmm0,62
	vpxor	xmm4,xmm4,xmm3
	vpsllq	xmm3,xmm0,63
	vpxor	xmm4,xmm4,xmm3
	vpslldq	xmm3,xmm4,8
	vpsrldq	xmm4,xmm4,8
	vpxor	xmm0,xmm0,xmm3
	vpxor	xmm1,xmm1,xmm4

	vpsrlq	xmm4,xmm0,1
	vpxor	xmm1,xmm1,xmm0
	vpxor	xmm0,xmm0,xmm4
	vpsrlq	xmm4,xmm4,5
	vpxor	xmm0,xmm0,xmm4
	vpsrlq	xmm0,xmm0,1
	vpxor	xmm0,xmm0,xmm1
	vpshufd	xmm3,xmm5,78
	vpshufd	xmm4,xmm0,78
	vpxor	xmm3,xmm3,xmm5
	vmovdqu	XMMWORD[rcx],xmm5
	vpxor	xmm4,xmm4,xmm0
	vmovdqu	XMMWORD[16+rcx],xmm0
	lea	rcx,[48+rcx]
	sub	r10,1
	jnz	NEAR $L$init_loop_avx

	vpalignr	xmm5,xmm3,xmm4,8
	vmovdqu	XMMWORD[(-16)+rcx],xmm5

	vzeroupper
	movaps	xmm6,XMMWORD[rsp]
	lea	rsp,[24+rsp]
	ret
$L$SEH_end_gcm_init_avx_5:


global	gcm_ghash_avx

ALIGN	32
gcm_ghash_avx:

$L$SEH_begin_gcm_ghash_avx_1:
_CET_ENDBR
	lea	rax,[((-136))+rsp]
	lea	rsp,[((-32))+rax]
$L$SEH_prologue_gcm_ghash_avx_2:
	movaps	XMMWORD[(-32)+rax],xmm6
$L$SEH_prologue_gcm_ghash_avx_3:
	movaps	XMMWORD[(-16)+rax],xmm7
$L$SEH_prologue_gcm_ghash_avx_4:
	movaps	XMMWORD[rax],xmm8
$L$SEH_prologue_gcm_ghash_avx_5:
	movaps	XMMWORD[16+rax],xmm9
$L$SEH_prologue_gcm_ghash_avx_6:
	movaps	XMMWORD[32+rax],xmm10
$L$SEH_prologue_gcm_ghash_avx_7:
	movaps	XMMWORD[48+rax],xmm11
$L$SEH_prologue_gcm_ghash_avx_8:
	movaps	XMMWORD[64+rax],xmm12
$L$SEH_prologue_gcm_ghash_avx_9:
	movaps	XMMWORD[80+rax],xmm13
$L$SEH_prologue_gcm_ghash_avx_10:
	movaps	XMMWORD[96+rax],xmm14
$L$SEH_prologue_gcm_ghash_avx_11:
	movaps	XMMWORD[112+rax],xmm15
$L$SEH_prologue_gcm_ghash_avx_12:
$L$SEH_endprologue_gcm_ghash_avx_13:
	vzeroupper

	vmovdqu	xmm10,XMMWORD[rcx]
	lea	r10,[$L$0x1c2_polynomial]
	lea	rdx,[64+rdx]
	vmovdqu	xmm13,XMMWORD[$L$bswap_mask]
	vpshufb	xmm10,xmm10,xmm13
	cmp	r9,0x80
	jb	NEAR $L$short_avx
	sub	r9,0x80

	vmovdqu	xmm14,XMMWORD[112+r8]
	vmovdqu	xmm6,XMMWORD[((0-64))+rdx]
	vpshufb	xmm14,xmm14,xmm13
	vmovdqu	xmm7,XMMWORD[((32-64))+rdx]

	vpunpckhqdq	xmm9,xmm14,xmm14
	vmovdqu	xmm15,XMMWORD[96+r8]
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpxor	xmm9,xmm9,xmm14
	vpshufb	xmm15,xmm15,xmm13
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((16-64))+rdx]
	vpunpckhqdq	xmm8,xmm15,xmm15
	vmovdqu	xmm14,XMMWORD[80+r8]
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vpxor	xmm8,xmm8,xmm15

	vpshufb	xmm14,xmm14,xmm13
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((48-64))+rdx]
	vpxor	xmm9,xmm9,xmm14
	vmovdqu	xmm15,XMMWORD[64+r8]
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((80-64))+rdx]

	vpshufb	xmm15,xmm15,xmm13
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpxor	xmm4,xmm4,xmm1
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((64-64))+rdx]
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vpxor	xmm8,xmm8,xmm15

	vmovdqu	xmm14,XMMWORD[48+r8]
	vpxor	xmm0,xmm0,xmm3
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpxor	xmm1,xmm1,xmm4
	vpshufb	xmm14,xmm14,xmm13
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((96-64))+rdx]
	vpxor	xmm2,xmm2,xmm5
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((128-64))+rdx]
	vpxor	xmm9,xmm9,xmm14

	vmovdqu	xmm15,XMMWORD[32+r8]
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpxor	xmm4,xmm4,xmm1
	vpshufb	xmm15,xmm15,xmm13
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((112-64))+rdx]
	vpxor	xmm5,xmm5,xmm2
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vpxor	xmm8,xmm8,xmm15

	vmovdqu	xmm14,XMMWORD[16+r8]
	vpxor	xmm0,xmm0,xmm3
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpxor	xmm1,xmm1,xmm4
	vpshufb	xmm14,xmm14,xmm13
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((144-64))+rdx]
	vpxor	xmm2,xmm2,xmm5
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((176-64))+rdx]
	vpxor	xmm9,xmm9,xmm14

	vmovdqu	xmm15,XMMWORD[r8]
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpxor	xmm4,xmm4,xmm1
	vpshufb	xmm15,xmm15,xmm13
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((160-64))+rdx]
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm9,xmm7,0x10

	lea	r8,[128+r8]
	cmp	r9,0x80
	jb	NEAR $L$tail_avx

	vpxor	xmm15,xmm15,xmm10
	sub	r9,0x80
	jmp	NEAR $L$oop8x_avx

ALIGN	32
$L$oop8x_avx:
	vpunpckhqdq	xmm8,xmm15,xmm15
	vmovdqu	xmm14,XMMWORD[112+r8]
	vpxor	xmm3,xmm3,xmm0
	vpxor	xmm8,xmm8,xmm15
	vpclmulqdq	xmm10,xmm15,xmm6,0x00
	vpshufb	xmm14,xmm14,xmm13
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm11,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((0-64))+rdx]
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm12,xmm8,xmm7,0x00
	vmovdqu	xmm7,XMMWORD[((32-64))+rdx]
	vpxor	xmm9,xmm9,xmm14

	vmovdqu	xmm15,XMMWORD[96+r8]
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpxor	xmm10,xmm10,xmm3
	vpshufb	xmm15,xmm15,xmm13
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vxorps	xmm11,xmm11,xmm4
	vmovdqu	xmm6,XMMWORD[((16-64))+rdx]
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vpxor	xmm12,xmm12,xmm5
	vxorps	xmm8,xmm8,xmm15

	vmovdqu	xmm14,XMMWORD[80+r8]
	vpxor	xmm12,xmm12,xmm10
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpxor	xmm12,xmm12,xmm11
	vpslldq	xmm9,xmm12,8
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vpsrldq	xmm12,xmm12,8
	vpxor	xmm10,xmm10,xmm9
	vmovdqu	xmm6,XMMWORD[((48-64))+rdx]
	vpshufb	xmm14,xmm14,xmm13
	vxorps	xmm11,xmm11,xmm12
	vpxor	xmm4,xmm4,xmm1
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((80-64))+rdx]
	vpxor	xmm9,xmm9,xmm14
	vpxor	xmm5,xmm5,xmm2

	vmovdqu	xmm15,XMMWORD[64+r8]
	vpalignr	xmm12,xmm10,xmm10,8
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpshufb	xmm15,xmm15,xmm13
	vpxor	xmm0,xmm0,xmm3
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((64-64))+rdx]
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm1,xmm1,xmm4
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vxorps	xmm8,xmm8,xmm15
	vpxor	xmm2,xmm2,xmm5

	vmovdqu	xmm14,XMMWORD[48+r8]
	vpclmulqdq	xmm10,xmm10,XMMWORD[r10],0x10
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpshufb	xmm14,xmm14,xmm13
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((96-64))+rdx]
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((128-64))+rdx]
	vpxor	xmm9,xmm9,xmm14
	vpxor	xmm5,xmm5,xmm2

	vmovdqu	xmm15,XMMWORD[32+r8]
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpshufb	xmm15,xmm15,xmm13
	vpxor	xmm0,xmm0,xmm3
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((112-64))+rdx]
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm1,xmm1,xmm4
	vpclmulqdq	xmm2,xmm9,xmm7,0x00
	vpxor	xmm8,xmm8,xmm15
	vpxor	xmm2,xmm2,xmm5
	vxorps	xmm10,xmm10,xmm12

	vmovdqu	xmm14,XMMWORD[16+r8]
	vpalignr	xmm12,xmm10,xmm10,8
	vpclmulqdq	xmm3,xmm15,xmm6,0x00
	vpshufb	xmm14,xmm14,xmm13
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm4,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((144-64))+rdx]
	vpclmulqdq	xmm10,xmm10,XMMWORD[r10],0x10
	vxorps	xmm12,xmm12,xmm11
	vpunpckhqdq	xmm9,xmm14,xmm14
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm5,xmm8,xmm7,0x10
	vmovdqu	xmm7,XMMWORD[((176-64))+rdx]
	vpxor	xmm9,xmm9,xmm14
	vpxor	xmm5,xmm5,xmm2

	vmovdqu	xmm15,XMMWORD[r8]
	vpclmulqdq	xmm0,xmm14,xmm6,0x00
	vpshufb	xmm15,xmm15,xmm13
	vpclmulqdq	xmm1,xmm14,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((160-64))+rdx]
	vpxor	xmm15,xmm15,xmm12
	vpclmulqdq	xmm2,xmm9,xmm7,0x10
	vpxor	xmm15,xmm15,xmm10

	lea	r8,[128+r8]
	sub	r9,0x80
	jnc	NEAR $L$oop8x_avx

	add	r9,0x80
	jmp	NEAR $L$tail_no_xor_avx

ALIGN	32
$L$short_avx:
	vmovdqu	xmm14,XMMWORD[((-16))+r9*1+r8]
	lea	r8,[r9*1+r8]
	vmovdqu	xmm6,XMMWORD[((0-64))+rdx]
	vmovdqu	xmm7,XMMWORD[((32-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13

	vmovdqa	xmm3,xmm0
	vmovdqa	xmm4,xmm1
	vmovdqa	xmm5,xmm2
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-32))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((16-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vpsrldq	xmm7,xmm7,8
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-48))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((48-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vmovdqu	xmm7,XMMWORD[((80-64))+rdx]
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-64))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((64-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vpsrldq	xmm7,xmm7,8
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-80))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((96-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vmovdqu	xmm7,XMMWORD[((128-64))+rdx]
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-96))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((112-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vpsrldq	xmm7,xmm7,8
	sub	r9,0x10
	jz	NEAR $L$tail_avx

	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vmovdqu	xmm14,XMMWORD[((-112))+r8]
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vmovdqu	xmm6,XMMWORD[((144-64))+rdx]
	vpshufb	xmm15,xmm14,xmm13
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00
	vmovq	xmm7,QWORD[((184-64))+rdx]
	sub	r9,0x10
	jmp	NEAR $L$tail_avx

ALIGN	32
$L$tail_avx:
	vpxor	xmm15,xmm15,xmm10
$L$tail_no_xor_avx:
	vpunpckhqdq	xmm8,xmm15,xmm15
	vpxor	xmm3,xmm3,xmm0
	vpclmulqdq	xmm0,xmm15,xmm6,0x00
	vpxor	xmm8,xmm8,xmm15
	vpxor	xmm4,xmm4,xmm1
	vpclmulqdq	xmm1,xmm15,xmm6,0x11
	vpxor	xmm5,xmm5,xmm2
	vpclmulqdq	xmm2,xmm8,xmm7,0x00

	vmovdqu	xmm12,XMMWORD[r10]

	vpxor	xmm10,xmm3,xmm0
	vpxor	xmm11,xmm4,xmm1
	vpxor	xmm5,xmm5,xmm2

	vpxor	xmm5,xmm5,xmm10
	vpxor	xmm5,xmm5,xmm11
	vpslldq	xmm9,xmm5,8
	vpsrldq	xmm5,xmm5,8
	vpxor	xmm10,xmm10,xmm9
	vpxor	xmm11,xmm11,xmm5

	vpclmulqdq	xmm9,xmm10,xmm12,0x10
	vpalignr	xmm10,xmm10,xmm10,8
	vpxor	xmm10,xmm10,xmm9

	vpclmulqdq	xmm9,xmm10,xmm12,0x10
	vpalignr	xmm10,xmm10,xmm10,8
	vpxor	xmm10,xmm10,xmm11
	vpxor	xmm10,xmm10,xmm9

	cmp	r9,0
	jne	NEAR $L$short_avx

	vpshufb	xmm10,xmm10,xmm13
	vmovdqu	XMMWORD[rcx],xmm10
	vzeroupper
	movaps	xmm6,XMMWORD[rsp]
	movaps	xmm7,XMMWORD[16+rsp]
	movaps	xmm8,XMMWORD[32+rsp]
	movaps	xmm9,XMMWORD[48+rsp]
	movaps	xmm10,XMMWORD[64+rsp]
	movaps	xmm11,XMMWORD[80+rsp]
	movaps	xmm12,XMMWORD[96+rsp]
	movaps	xmm13,XMMWORD[112+rsp]
	movaps	xmm14,XMMWORD[128+rsp]
	movaps	xmm15,XMMWORD[144+rsp]
	lea	rsp,[168+rsp]
	ret

$L$SEH_end_gcm_ghash_avx_14:

section	.rdata rdata align=8
ALIGN	64
$L$bswap_mask:
	DB	15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0
$L$0x1c2_polynomial:
	DB	1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0xc2
$L$7_mask:
	DD	7,0,7,0
ALIGN	64

	DB	71,72,65,83,72,32,102,111,114,32,120,56,54,95,54,52
	DB	44,32,67,82,89,80,84,79,71,65,77,83,32,98,121,32
	DB	60,97,112,112,114,111,64,111,112,101,110,115,115,108,46,111
	DB	114,103,62,0
ALIGN	64
section	.text

section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_gcm_init_clmul_1 wrt ..imagebase
	DD	$L$SEH_end_gcm_init_clmul_5 wrt ..imagebase
	DD	$L$SEH_info_gcm_init_clmul_0 wrt ..imagebase

	DD	$L$SEH_begin_gcm_ghash_clmul_1 wrt ..imagebase
	DD	$L$SEH_end_gcm_ghash_clmul_14 wrt ..imagebase
	DD	$L$SEH_info_gcm_ghash_clmul_0 wrt ..imagebase

	DD	$L$SEH_begin_gcm_init_avx_1 wrt ..imagebase
	DD	$L$SEH_end_gcm_init_avx_5 wrt ..imagebase
	DD	$L$SEH_info_gcm_init_avx_0 wrt ..imagebase

	DD	$L$SEH_begin_gcm_ghash_avx_1 wrt ..imagebase
	DD	$L$SEH_end_gcm_ghash_avx_14 wrt ..imagebase
	DD	$L$SEH_info_gcm_ghash_avx_0 wrt ..imagebase


section	.xdata rdata align=8
ALIGN	4
$L$SEH_info_gcm_init_clmul_0:
	DB	1
	DB	$L$SEH_endprologue_gcm_init_clmul_4-$L$SEH_begin_gcm_init_clmul_1
	DB	3
	DB	0
	DB	$L$SEH_prologue_gcm_init_clmul_3-$L$SEH_begin_gcm_init_clmul_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_gcm_init_clmul_2-$L$SEH_begin_gcm_init_clmul_1
	DB	34

	DW	0
$L$SEH_info_gcm_ghash_clmul_0:
	DB	1
	DB	$L$SEH_endprologue_gcm_ghash_clmul_13-$L$SEH_begin_gcm_ghash_clmul_1
	DB	22
	DB	0
	DB	$L$SEH_prologue_gcm_ghash_clmul_12-$L$SEH_begin_gcm_ghash_clmul_1
	DB	248
	DW	9
	DB	$L$SEH_prologue_gcm_ghash_clmul_11-$L$SEH_begin_gcm_ghash_clmul_1
	DB	232
	DW	8
	DB	$L$SEH_prologue_gcm_ghash_clmul_10-$L$SEH_begin_gcm_ghash_clmul_1
	DB	216
	DW	7
	DB	$L$SEH_prologue_gcm_ghash_clmul_9-$L$SEH_begin_gcm_ghash_clmul_1
	DB	200
	DW	6
	DB	$L$SEH_prologue_gcm_ghash_clmul_8-$L$SEH_begin_gcm_ghash_clmul_1
	DB	184
	DW	5
	DB	$L$SEH_prologue_gcm_ghash_clmul_7-$L$SEH_begin_gcm_ghash_clmul_1
	DB	168
	DW	4
	DB	$L$SEH_prologue_gcm_ghash_clmul_6-$L$SEH_begin_gcm_ghash_clmul_1
	DB	152
	DW	3
	DB	$L$SEH_prologue_gcm_ghash_clmul_5-$L$SEH_begin_gcm_ghash_clmul_1
	DB	136
	DW	2
	DB	$L$SEH_prologue_gcm_ghash_clmul_4-$L$SEH_begin_gcm_ghash_clmul_1
	DB	120
	DW	1
	DB	$L$SEH_prologue_gcm_ghash_clmul_3-$L$SEH_begin_gcm_ghash_clmul_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_gcm_ghash_clmul_2-$L$SEH_begin_gcm_ghash_clmul_1
	DB	1
	DW	21

$L$SEH_info_gcm_init_avx_0:
	DB	1
	DB	$L$SEH_endprologue_gcm_init_avx_4-$L$SEH_begin_gcm_init_avx_1
	DB	3
	DB	0
	DB	$L$SEH_prologue_gcm_init_avx_3-$L$SEH_begin_gcm_init_avx_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_gcm_init_avx_2-$L$SEH_begin_gcm_init_avx_1
	DB	34

	DW	0
$L$SEH_info_gcm_ghash_avx_0:
	DB	1
	DB	$L$SEH_endprologue_gcm_ghash_avx_13-$L$SEH_begin_gcm_ghash_avx_1
	DB	22
	DB	0
	DB	$L$SEH_prologue_gcm_ghash_avx_12-$L$SEH_begin_gcm_ghash_avx_1
	DB	248
	DW	9
	DB	$L$SEH_prologue_gcm_ghash_avx_11-$L$SEH_begin_gcm_ghash_avx_1
	DB	232
	DW	8
	DB	$L$SEH_prologue_gcm_ghash_avx_10-$L$SEH_begin_gcm_ghash_avx_1
	DB	216
	DW	7
	DB	$L$SEH_prologue_gcm_ghash_avx_9-$L$SEH_begin_gcm_ghash_avx_1
	DB	200
	DW	6
	DB	$L$SEH_prologue_gcm_ghash_avx_8-$L$SEH_begin_gcm_ghash_avx_1
	DB	184
	DW	5
	DB	$L$SEH_prologue_gcm_ghash_avx_7-$L$SEH_begin_gcm_ghash_avx_1
	DB	168
	DW	4
	DB	$L$SEH_prologue_gcm_ghash_avx_6-$L$SEH_begin_gcm_ghash_avx_1
	DB	152
	DW	3
	DB	$L$SEH_prologue_gcm_ghash_avx_5-$L$SEH_begin_gcm_ghash_avx_1
	DB	136
	DW	2
	DB	$L$SEH_prologue_gcm_ghash_avx_4-$L$SEH_begin_gcm_ghash_avx_1
	DB	120
	DW	1
	DB	$L$SEH_prologue_gcm_ghash_avx_3-$L$SEH_begin_gcm_ghash_avx_1
	DB	104
	DW	0
	DB	$L$SEH_prologue_gcm_ghash_avx_2-$L$SEH_begin_gcm_ghash_avx_1
	DB	1
	DW	21
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
