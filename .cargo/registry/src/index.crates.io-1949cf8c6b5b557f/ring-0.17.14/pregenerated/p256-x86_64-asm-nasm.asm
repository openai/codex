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
$L$poly:
	DQ	0xffffffffffffffff,0x00000000ffffffff,0x0000000000000000,0xffffffff00000001

$L$One:
	DD	1,1,1,1,1,1,1,1
$L$Two:
	DD	2,2,2,2,2,2,2,2
$L$Three:
	DD	3,3,3,3,3,3,3,3
$L$ONE_mont:
	DQ	0x0000000000000001,0xffffffff00000000,0xffffffffffffffff,0x00000000fffffffe


$L$ord:
	DQ	0xf3b9cac2fc632551,0xbce6faada7179e84,0xffffffffffffffff,0xffffffff00000000
$L$ordK:
	DQ	0xccd1c8aaee00bc4f
section	.text




global	ecp_nistz256_neg

ALIGN	32
ecp_nistz256_neg:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_neg:
	mov	rdi,rcx
	mov	rsi,rdx



_CET_ENDBR
	push	r12

	push	r13

$L$neg_body:

	xor	r8,r8
	xor	r9,r9
	xor	r10,r10
	xor	r11,r11
	xor	r13,r13

	sub	r8,QWORD[rsi]
	sbb	r9,QWORD[8+rsi]
	sbb	r10,QWORD[16+rsi]
	mov	rax,r8
	sbb	r11,QWORD[24+rsi]
	lea	rsi,[$L$poly]
	mov	rdx,r9
	sbb	r13,0

	add	r8,QWORD[rsi]
	mov	rcx,r10
	adc	r9,QWORD[8+rsi]
	adc	r10,QWORD[16+rsi]
	mov	r12,r11
	adc	r11,QWORD[24+rsi]
	test	r13,r13

	cmovz	r8,rax
	cmovz	r9,rdx
	mov	QWORD[rdi],r8
	cmovz	r10,rcx
	mov	QWORD[8+rdi],r9
	cmovz	r11,r12
	mov	QWORD[16+rdi],r10
	mov	QWORD[24+rdi],r11

	mov	r13,QWORD[rsp]

	mov	r12,QWORD[8+rsp]

	lea	rsp,[16+rsp]

$L$neg_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_neg:






global	ecp_nistz256_ord_mul_mont_nohw

ALIGN	32
ecp_nistz256_ord_mul_mont_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_ord_mul_mont_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$ord_mul_body:

	mov	rax,QWORD[rdx]
	mov	rbx,rdx
	lea	r14,[$L$ord]
	mov	r15,QWORD[$L$ordK]


	mov	rcx,rax
	mul	QWORD[rsi]
	mov	r8,rax
	mov	rax,rcx
	mov	r9,rdx

	mul	QWORD[8+rsi]
	add	r9,rax
	mov	rax,rcx
	adc	rdx,0
	mov	r10,rdx

	mul	QWORD[16+rsi]
	add	r10,rax
	mov	rax,rcx
	adc	rdx,0

	mov	r13,r8
	imul	r8,r15

	mov	r11,rdx
	mul	QWORD[24+rsi]
	add	r11,rax
	mov	rax,r8
	adc	rdx,0
	mov	r12,rdx


	mul	QWORD[r14]
	mov	rbp,r8
	add	r13,rax
	mov	rax,r8
	adc	rdx,0
	mov	rcx,rdx

	sub	r10,r8
	sbb	r8,0

	mul	QWORD[8+r14]
	add	r9,rcx
	adc	rdx,0
	add	r9,rax
	mov	rax,rbp
	adc	r10,rdx
	mov	rdx,rbp
	adc	r8,0

	shl	rax,32
	shr	rdx,32
	sub	r11,rax
	mov	rax,QWORD[8+rbx]
	sbb	rbp,rdx

	add	r11,r8
	adc	r12,rbp
	adc	r13,0


	mov	rcx,rax
	mul	QWORD[rsi]
	add	r9,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[8+rsi]
	add	r10,rbp
	adc	rdx,0
	add	r10,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[16+rsi]
	add	r11,rbp
	adc	rdx,0
	add	r11,rax
	mov	rax,rcx
	adc	rdx,0

	mov	rcx,r9
	imul	r9,r15

	mov	rbp,rdx
	mul	QWORD[24+rsi]
	add	r12,rbp
	adc	rdx,0
	xor	r8,r8
	add	r12,rax
	mov	rax,r9
	adc	r13,rdx
	adc	r8,0


	mul	QWORD[r14]
	mov	rbp,r9
	add	rcx,rax
	mov	rax,r9
	adc	rcx,rdx

	sub	r11,r9
	sbb	r9,0

	mul	QWORD[8+r14]
	add	r10,rcx
	adc	rdx,0
	add	r10,rax
	mov	rax,rbp
	adc	r11,rdx
	mov	rdx,rbp
	adc	r9,0

	shl	rax,32
	shr	rdx,32
	sub	r12,rax
	mov	rax,QWORD[16+rbx]
	sbb	rbp,rdx

	add	r12,r9
	adc	r13,rbp
	adc	r8,0


	mov	rcx,rax
	mul	QWORD[rsi]
	add	r10,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[8+rsi]
	add	r11,rbp
	adc	rdx,0
	add	r11,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[16+rsi]
	add	r12,rbp
	adc	rdx,0
	add	r12,rax
	mov	rax,rcx
	adc	rdx,0

	mov	rcx,r10
	imul	r10,r15

	mov	rbp,rdx
	mul	QWORD[24+rsi]
	add	r13,rbp
	adc	rdx,0
	xor	r9,r9
	add	r13,rax
	mov	rax,r10
	adc	r8,rdx
	adc	r9,0


	mul	QWORD[r14]
	mov	rbp,r10
	add	rcx,rax
	mov	rax,r10
	adc	rcx,rdx

	sub	r12,r10
	sbb	r10,0

	mul	QWORD[8+r14]
	add	r11,rcx
	adc	rdx,0
	add	r11,rax
	mov	rax,rbp
	adc	r12,rdx
	mov	rdx,rbp
	adc	r10,0

	shl	rax,32
	shr	rdx,32
	sub	r13,rax
	mov	rax,QWORD[24+rbx]
	sbb	rbp,rdx

	add	r13,r10
	adc	r8,rbp
	adc	r9,0


	mov	rcx,rax
	mul	QWORD[rsi]
	add	r11,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[8+rsi]
	add	r12,rbp
	adc	rdx,0
	add	r12,rax
	mov	rax,rcx
	adc	rdx,0
	mov	rbp,rdx

	mul	QWORD[16+rsi]
	add	r13,rbp
	adc	rdx,0
	add	r13,rax
	mov	rax,rcx
	adc	rdx,0

	mov	rcx,r11
	imul	r11,r15

	mov	rbp,rdx
	mul	QWORD[24+rsi]
	add	r8,rbp
	adc	rdx,0
	xor	r10,r10
	add	r8,rax
	mov	rax,r11
	adc	r9,rdx
	adc	r10,0


	mul	QWORD[r14]
	mov	rbp,r11
	add	rcx,rax
	mov	rax,r11
	adc	rcx,rdx

	sub	r13,r11
	sbb	r11,0

	mul	QWORD[8+r14]
	add	r12,rcx
	adc	rdx,0
	add	r12,rax
	mov	rax,rbp
	adc	r13,rdx
	mov	rdx,rbp
	adc	r11,0

	shl	rax,32
	shr	rdx,32
	sub	r8,rax
	sbb	rbp,rdx

	add	r8,r11
	adc	r9,rbp
	adc	r10,0


	mov	rsi,r12
	sub	r12,QWORD[r14]
	mov	r11,r13
	sbb	r13,QWORD[8+r14]
	mov	rcx,r8
	sbb	r8,QWORD[16+r14]
	mov	rbp,r9
	sbb	r9,QWORD[24+r14]
	sbb	r10,0

	cmovc	r12,rsi
	cmovc	r13,r11
	cmovc	r8,rcx
	cmovc	r9,rbp

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$ord_mul_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_ord_mul_mont_nohw:







global	ecp_nistz256_ord_sqr_mont_nohw

ALIGN	32
ecp_nistz256_ord_sqr_mont_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_ord_sqr_mont_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$ord_sqr_body:

	mov	r8,QWORD[rsi]
	mov	rax,QWORD[8+rsi]
	mov	r14,QWORD[16+rsi]
	mov	r15,QWORD[24+rsi]
	lea	rsi,[$L$ord]
	mov	rbx,rdx
	jmp	NEAR $L$oop_ord_sqr

ALIGN	32
$L$oop_ord_sqr:

	mov	rbp,rax
	mul	r8
	mov	r9,rax
DB	102,72,15,110,205
	mov	rax,r14
	mov	r10,rdx

	mul	r8
	add	r10,rax
	mov	rax,r15
DB	102,73,15,110,214
	adc	rdx,0
	mov	r11,rdx

	mul	r8
	add	r11,rax
	mov	rax,r15
DB	102,73,15,110,223
	adc	rdx,0
	mov	r12,rdx


	mul	r14
	mov	r13,rax
	mov	rax,r14
	mov	r14,rdx


	mul	rbp
	add	r11,rax
	mov	rax,r15
	adc	rdx,0
	mov	r15,rdx

	mul	rbp
	add	r12,rax
	adc	rdx,0

	add	r12,r15
	adc	r13,rdx
	adc	r14,0


	xor	r15,r15
	mov	rax,r8
	add	r9,r9
	adc	r10,r10
	adc	r11,r11
	adc	r12,r12
	adc	r13,r13
	adc	r14,r14
	adc	r15,0


	mul	rax
	mov	r8,rax
DB	102,72,15,126,200
	mov	rbp,rdx

	mul	rax
	add	r9,rbp
	adc	r10,rax
DB	102,72,15,126,208
	adc	rdx,0
	mov	rbp,rdx

	mul	rax
	add	r11,rbp
	adc	r12,rax
DB	102,72,15,126,216
	adc	rdx,0
	mov	rbp,rdx

	mov	rcx,r8
	imul	r8,QWORD[32+rsi]

	mul	rax
	add	r13,rbp
	adc	r14,rax
	mov	rax,QWORD[rsi]
	adc	r15,rdx


	mul	r8
	mov	rbp,r8
	add	rcx,rax
	mov	rax,QWORD[8+rsi]
	adc	rcx,rdx

	sub	r10,r8
	sbb	rbp,0

	mul	r8
	add	r9,rcx
	adc	rdx,0
	add	r9,rax
	mov	rax,r8
	adc	r10,rdx
	mov	rdx,r8
	adc	rbp,0

	mov	rcx,r9
	imul	r9,QWORD[32+rsi]

	shl	rax,32
	shr	rdx,32
	sub	r11,rax
	mov	rax,QWORD[rsi]
	sbb	r8,rdx

	add	r11,rbp
	adc	r8,0


	mul	r9
	mov	rbp,r9
	add	rcx,rax
	mov	rax,QWORD[8+rsi]
	adc	rcx,rdx

	sub	r11,r9
	sbb	rbp,0

	mul	r9
	add	r10,rcx
	adc	rdx,0
	add	r10,rax
	mov	rax,r9
	adc	r11,rdx
	mov	rdx,r9
	adc	rbp,0

	mov	rcx,r10
	imul	r10,QWORD[32+rsi]

	shl	rax,32
	shr	rdx,32
	sub	r8,rax
	mov	rax,QWORD[rsi]
	sbb	r9,rdx

	add	r8,rbp
	adc	r9,0


	mul	r10
	mov	rbp,r10
	add	rcx,rax
	mov	rax,QWORD[8+rsi]
	adc	rcx,rdx

	sub	r8,r10
	sbb	rbp,0

	mul	r10
	add	r11,rcx
	adc	rdx,0
	add	r11,rax
	mov	rax,r10
	adc	r8,rdx
	mov	rdx,r10
	adc	rbp,0

	mov	rcx,r11
	imul	r11,QWORD[32+rsi]

	shl	rax,32
	shr	rdx,32
	sub	r9,rax
	mov	rax,QWORD[rsi]
	sbb	r10,rdx

	add	r9,rbp
	adc	r10,0


	mul	r11
	mov	rbp,r11
	add	rcx,rax
	mov	rax,QWORD[8+rsi]
	adc	rcx,rdx

	sub	r9,r11
	sbb	rbp,0

	mul	r11
	add	r8,rcx
	adc	rdx,0
	add	r8,rax
	mov	rax,r11
	adc	r9,rdx
	mov	rdx,r11
	adc	rbp,0

	shl	rax,32
	shr	rdx,32
	sub	r10,rax
	sbb	r11,rdx

	add	r10,rbp
	adc	r11,0


	xor	rdx,rdx
	add	r8,r12
	adc	r9,r13
	mov	r12,r8
	adc	r10,r14
	adc	r11,r15
	mov	rax,r9
	adc	rdx,0


	sub	r8,QWORD[rsi]
	mov	r14,r10
	sbb	r9,QWORD[8+rsi]
	sbb	r10,QWORD[16+rsi]
	mov	r15,r11
	sbb	r11,QWORD[24+rsi]
	sbb	rdx,0

	cmovc	r8,r12
	cmovnc	rax,r9
	cmovnc	r14,r10
	cmovnc	r15,r11

	dec	rbx
	jnz	NEAR $L$oop_ord_sqr

	mov	QWORD[rdi],r8
	mov	QWORD[8+rdi],rax
	pxor	xmm1,xmm1
	mov	QWORD[16+rdi],r14
	pxor	xmm2,xmm2
	mov	QWORD[24+rdi],r15
	pxor	xmm3,xmm3

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$ord_sqr_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_ord_sqr_mont_nohw:

global	ecp_nistz256_ord_mul_mont_adx

ALIGN	32
ecp_nistz256_ord_mul_mont_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_ord_mul_mont_adx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



$L$ecp_nistz256_ord_mul_mont_adx:
_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$ord_mulx_body:

	mov	rbx,rdx
	mov	rdx,QWORD[rdx]
	mov	r9,QWORD[rsi]
	mov	r10,QWORD[8+rsi]
	mov	r11,QWORD[16+rsi]
	mov	r12,QWORD[24+rsi]
	lea	rsi,[((-128))+rsi]
	lea	r14,[(($L$ord-128))]
	mov	r15,QWORD[$L$ordK]


	mulx	r9,r8,r9
	mulx	r10,rcx,r10
	mulx	r11,rbp,r11
	add	r9,rcx
	mulx	r12,rcx,r12
	mov	rdx,r8
	mulx	rax,rdx,r15
	adc	r10,rbp
	adc	r11,rcx
	adc	r12,0


	xor	r13,r13
	mulx	rbp,rcx,QWORD[((0+128))+r14]
	adcx	r8,rcx
	adox	r9,rbp

	mulx	rbp,rcx,QWORD[((8+128))+r14]
	adcx	r9,rcx
	adox	r10,rbp

	mulx	rbp,rcx,QWORD[((16+128))+r14]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((24+128))+r14]
	mov	rdx,QWORD[8+rbx]
	adcx	r11,rcx
	adox	r12,rbp
	adcx	r12,r8
	adox	r13,r8
	adc	r13,0


	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r9,rcx
	adox	r10,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r9
	mulx	rax,rdx,r15
	adcx	r12,rcx
	adox	r13,rbp

	adcx	r13,r8
	adox	r8,r8
	adc	r8,0


	mulx	rbp,rcx,QWORD[((0+128))+r14]
	adcx	r9,rcx
	adox	r10,rbp

	mulx	rbp,rcx,QWORD[((8+128))+r14]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((16+128))+r14]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((24+128))+r14]
	mov	rdx,QWORD[16+rbx]
	adcx	r12,rcx
	adox	r13,rbp
	adcx	r13,r9
	adox	r8,r9
	adc	r8,0


	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r10
	mulx	rax,rdx,r15
	adcx	r13,rcx
	adox	r8,rbp

	adcx	r8,r9
	adox	r9,r9
	adc	r9,0


	mulx	rbp,rcx,QWORD[((0+128))+r14]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((8+128))+r14]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((16+128))+r14]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((24+128))+r14]
	mov	rdx,QWORD[24+rbx]
	adcx	r13,rcx
	adox	r8,rbp
	adcx	r8,r10
	adox	r9,r10
	adc	r9,0


	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r13,rcx
	adox	r8,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r11
	mulx	rax,rdx,r15
	adcx	r8,rcx
	adox	r9,rbp

	adcx	r9,r10
	adox	r10,r10
	adc	r10,0


	mulx	rbp,rcx,QWORD[((0+128))+r14]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((8+128))+r14]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((16+128))+r14]
	adcx	r13,rcx
	adox	r8,rbp

	mulx	rbp,rcx,QWORD[((24+128))+r14]
	lea	r14,[128+r14]
	mov	rbx,r12
	adcx	r8,rcx
	adox	r9,rbp
	mov	rdx,r13
	adcx	r9,r11
	adox	r10,r11
	adc	r10,0



	mov	rcx,r8
	sub	r12,QWORD[r14]
	sbb	r13,QWORD[8+r14]
	sbb	r8,QWORD[16+r14]
	mov	rbp,r9
	sbb	r9,QWORD[24+r14]
	sbb	r10,0

	cmovc	r12,rbx
	cmovc	r13,rdx
	cmovc	r8,rcx
	cmovc	r9,rbp

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$ord_mulx_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_ord_mul_mont_adx:

global	ecp_nistz256_ord_sqr_mont_adx

ALIGN	32
ecp_nistz256_ord_sqr_mont_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_ord_sqr_mont_adx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
$L$ecp_nistz256_ord_sqr_mont_adx:
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$ord_sqrx_body:

	mov	rbx,rdx
	mov	rdx,QWORD[rsi]
	mov	r14,QWORD[8+rsi]
	mov	r15,QWORD[16+rsi]
	mov	r8,QWORD[24+rsi]
	lea	rsi,[$L$ord]
	jmp	NEAR $L$oop_ord_sqrx

ALIGN	32
$L$oop_ord_sqrx:
	mulx	r10,r9,r14
	mulx	r11,rcx,r15
	mov	rax,rdx
DB	102,73,15,110,206
	mulx	r12,rbp,r8
	mov	rdx,r14
	add	r10,rcx
DB	102,73,15,110,215
	adc	r11,rbp
	adc	r12,0
	xor	r13,r13

	mulx	rbp,rcx,r15
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,r8
	mov	rdx,r15
	adcx	r12,rcx
	adox	r13,rbp
	adc	r13,0

	mulx	r14,rcx,r8
	mov	rdx,rax
DB	102,73,15,110,216
	xor	r15,r15
	adcx	r9,r9
	adox	r13,rcx
	adcx	r10,r10
	adox	r14,r15


	mulx	rbp,r8,rdx
DB	102,72,15,126,202
	adcx	r11,r11
	adox	r9,rbp
	adcx	r12,r12
	mulx	rax,rcx,rdx
DB	102,72,15,126,210
	adcx	r13,r13
	adox	r10,rcx
	adcx	r14,r14
	mulx	rbp,rcx,rdx
	DB	0x67
DB	102,72,15,126,218
	adox	r11,rax
	adcx	r15,r15
	adox	r12,rcx
	adox	r13,rbp
	mulx	rax,rcx,rdx
	adox	r14,rcx
	adox	r15,rax


	mov	rdx,r8
	mulx	rcx,rdx,QWORD[32+rsi]

	xor	rax,rax
	mulx	rbp,rcx,QWORD[rsi]
	adcx	r8,rcx
	adox	r9,rbp
	mulx	rbp,rcx,QWORD[8+rsi]
	adcx	r9,rcx
	adox	r10,rbp
	mulx	rbp,rcx,QWORD[16+rsi]
	adcx	r10,rcx
	adox	r11,rbp
	mulx	rbp,rcx,QWORD[24+rsi]
	adcx	r11,rcx
	adox	r8,rbp
	adcx	r8,rax


	mov	rdx,r9
	mulx	rcx,rdx,QWORD[32+rsi]

	mulx	rbp,rcx,QWORD[rsi]
	adox	r9,rcx
	adcx	r10,rbp
	mulx	rbp,rcx,QWORD[8+rsi]
	adox	r10,rcx
	adcx	r11,rbp
	mulx	rbp,rcx,QWORD[16+rsi]
	adox	r11,rcx
	adcx	r8,rbp
	mulx	rbp,rcx,QWORD[24+rsi]
	adox	r8,rcx
	adcx	r9,rbp
	adox	r9,rax


	mov	rdx,r10
	mulx	rcx,rdx,QWORD[32+rsi]

	mulx	rbp,rcx,QWORD[rsi]
	adcx	r10,rcx
	adox	r11,rbp
	mulx	rbp,rcx,QWORD[8+rsi]
	adcx	r11,rcx
	adox	r8,rbp
	mulx	rbp,rcx,QWORD[16+rsi]
	adcx	r8,rcx
	adox	r9,rbp
	mulx	rbp,rcx,QWORD[24+rsi]
	adcx	r9,rcx
	adox	r10,rbp
	adcx	r10,rax


	mov	rdx,r11
	mulx	rcx,rdx,QWORD[32+rsi]

	mulx	rbp,rcx,QWORD[rsi]
	adox	r11,rcx
	adcx	r8,rbp
	mulx	rbp,rcx,QWORD[8+rsi]
	adox	r8,rcx
	adcx	r9,rbp
	mulx	rbp,rcx,QWORD[16+rsi]
	adox	r9,rcx
	adcx	r10,rbp
	mulx	rbp,rcx,QWORD[24+rsi]
	adox	r10,rcx
	adcx	r11,rbp
	adox	r11,rax


	add	r12,r8
	adc	r9,r13
	mov	rdx,r12
	adc	r10,r14
	adc	r11,r15
	mov	r14,r9
	adc	rax,0


	sub	r12,QWORD[rsi]
	mov	r15,r10
	sbb	r9,QWORD[8+rsi]
	sbb	r10,QWORD[16+rsi]
	mov	r8,r11
	sbb	r11,QWORD[24+rsi]
	sbb	rax,0

	cmovnc	rdx,r12
	cmovnc	r14,r9
	cmovnc	r15,r10
	cmovnc	r8,r11

	dec	rbx
	jnz	NEAR $L$oop_ord_sqrx

	mov	QWORD[rdi],rdx
	mov	QWORD[8+rdi],r14
	pxor	xmm1,xmm1
	mov	QWORD[16+rdi],r15
	pxor	xmm2,xmm2
	mov	QWORD[24+rdi],r8
	pxor	xmm3,xmm3

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$ord_sqrx_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_ord_sqr_mont_adx:






global	ecp_nistz256_mul_mont_nohw

ALIGN	32
ecp_nistz256_mul_mont_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_mul_mont_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$mul_body:
	mov	rbx,rdx
	mov	rax,QWORD[rdx]
	mov	r9,QWORD[rsi]
	mov	r10,QWORD[8+rsi]
	mov	r11,QWORD[16+rsi]
	mov	r12,QWORD[24+rsi]

	call	__ecp_nistz256_mul_montq

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$mul_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_mul_mont_nohw:


ALIGN	32
__ecp_nistz256_mul_montq:



	mov	rbp,rax
	mul	r9
	mov	r14,QWORD[(($L$poly+8))]
	mov	r8,rax
	mov	rax,rbp
	mov	r9,rdx

	mul	r10
	mov	r15,QWORD[(($L$poly+24))]
	add	r9,rax
	mov	rax,rbp
	adc	rdx,0
	mov	r10,rdx

	mul	r11
	add	r10,rax
	mov	rax,rbp
	adc	rdx,0
	mov	r11,rdx

	mul	r12
	add	r11,rax
	mov	rax,r8
	adc	rdx,0
	xor	r13,r13
	mov	r12,rdx










	mov	rbp,r8
	shl	r8,32
	mul	r15
	shr	rbp,32
	add	r9,r8
	adc	r10,rbp
	adc	r11,rax
	mov	rax,QWORD[8+rbx]
	adc	r12,rdx
	adc	r13,0
	xor	r8,r8



	mov	rbp,rax
	mul	QWORD[rsi]
	add	r9,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[8+rsi]
	add	r10,rcx
	adc	rdx,0
	add	r10,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[16+rsi]
	add	r11,rcx
	adc	rdx,0
	add	r11,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[24+rsi]
	add	r12,rcx
	adc	rdx,0
	add	r12,rax
	mov	rax,r9
	adc	r13,rdx
	adc	r8,0



	mov	rbp,r9
	shl	r9,32
	mul	r15
	shr	rbp,32
	add	r10,r9
	adc	r11,rbp
	adc	r12,rax
	mov	rax,QWORD[16+rbx]
	adc	r13,rdx
	adc	r8,0
	xor	r9,r9



	mov	rbp,rax
	mul	QWORD[rsi]
	add	r10,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[8+rsi]
	add	r11,rcx
	adc	rdx,0
	add	r11,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[16+rsi]
	add	r12,rcx
	adc	rdx,0
	add	r12,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[24+rsi]
	add	r13,rcx
	adc	rdx,0
	add	r13,rax
	mov	rax,r10
	adc	r8,rdx
	adc	r9,0



	mov	rbp,r10
	shl	r10,32
	mul	r15
	shr	rbp,32
	add	r11,r10
	adc	r12,rbp
	adc	r13,rax
	mov	rax,QWORD[24+rbx]
	adc	r8,rdx
	adc	r9,0
	xor	r10,r10



	mov	rbp,rax
	mul	QWORD[rsi]
	add	r11,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[8+rsi]
	add	r12,rcx
	adc	rdx,0
	add	r12,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[16+rsi]
	add	r13,rcx
	adc	rdx,0
	add	r13,rax
	mov	rax,rbp
	adc	rdx,0
	mov	rcx,rdx

	mul	QWORD[24+rsi]
	add	r8,rcx
	adc	rdx,0
	add	r8,rax
	mov	rax,r11
	adc	r9,rdx
	adc	r10,0



	mov	rbp,r11
	shl	r11,32
	mul	r15
	shr	rbp,32
	add	r12,r11
	adc	r13,rbp
	mov	rcx,r12
	adc	r8,rax
	adc	r9,rdx
	mov	rbp,r13
	adc	r10,0



	sub	r12,-1
	mov	rbx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	rdx,r9
	sbb	r9,r15
	sbb	r10,0

	cmovc	r12,rcx
	cmovc	r13,rbp
	mov	QWORD[rdi],r12
	cmovc	r8,rbx
	mov	QWORD[8+rdi],r13
	cmovc	r9,rdx
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret










global	ecp_nistz256_sqr_mont_nohw

ALIGN	32
ecp_nistz256_sqr_mont_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_sqr_mont_nohw:
	mov	rdi,rcx
	mov	rsi,rdx



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$sqr_body:
	mov	rax,QWORD[rsi]
	mov	r14,QWORD[8+rsi]
	mov	r15,QWORD[16+rsi]
	mov	r8,QWORD[24+rsi]

	call	__ecp_nistz256_sqr_montq

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$sqr_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_sqr_mont_nohw:


ALIGN	32
__ecp_nistz256_sqr_montq:

	mov	r13,rax
	mul	r14
	mov	r9,rax
	mov	rax,r15
	mov	r10,rdx

	mul	r13
	add	r10,rax
	mov	rax,r8
	adc	rdx,0
	mov	r11,rdx

	mul	r13
	add	r11,rax
	mov	rax,r15
	adc	rdx,0
	mov	r12,rdx


	mul	r14
	add	r11,rax
	mov	rax,r8
	adc	rdx,0
	mov	rbp,rdx

	mul	r14
	add	r12,rax
	mov	rax,r8
	adc	rdx,0
	add	r12,rbp
	mov	r13,rdx
	adc	r13,0


	mul	r15
	xor	r15,r15
	add	r13,rax
	mov	rax,QWORD[rsi]
	mov	r14,rdx
	adc	r14,0

	add	r9,r9
	adc	r10,r10
	adc	r11,r11
	adc	r12,r12
	adc	r13,r13
	adc	r14,r14
	adc	r15,0

	mul	rax
	mov	r8,rax
	mov	rax,QWORD[8+rsi]
	mov	rcx,rdx

	mul	rax
	add	r9,rcx
	adc	r10,rax
	mov	rax,QWORD[16+rsi]
	adc	rdx,0
	mov	rcx,rdx

	mul	rax
	add	r11,rcx
	adc	r12,rax
	mov	rax,QWORD[24+rsi]
	adc	rdx,0
	mov	rcx,rdx

	mul	rax
	add	r13,rcx
	adc	r14,rax
	mov	rax,r8
	adc	r15,rdx

	mov	rsi,QWORD[(($L$poly+8))]
	mov	rbp,QWORD[(($L$poly+24))]




	mov	rcx,r8
	shl	r8,32
	mul	rbp
	shr	rcx,32
	add	r9,r8
	adc	r10,rcx
	adc	r11,rax
	mov	rax,r9
	adc	rdx,0



	mov	rcx,r9
	shl	r9,32
	mov	r8,rdx
	mul	rbp
	shr	rcx,32
	add	r10,r9
	adc	r11,rcx
	adc	r8,rax
	mov	rax,r10
	adc	rdx,0



	mov	rcx,r10
	shl	r10,32
	mov	r9,rdx
	mul	rbp
	shr	rcx,32
	add	r11,r10
	adc	r8,rcx
	adc	r9,rax
	mov	rax,r11
	adc	rdx,0



	mov	rcx,r11
	shl	r11,32
	mov	r10,rdx
	mul	rbp
	shr	rcx,32
	add	r8,r11
	adc	r9,rcx
	adc	r10,rax
	adc	rdx,0
	xor	r11,r11



	add	r12,r8
	adc	r13,r9
	mov	r8,r12
	adc	r14,r10
	adc	r15,rdx
	mov	r9,r13
	adc	r11,0

	sub	r12,-1
	mov	r10,r14
	sbb	r13,rsi
	sbb	r14,0
	mov	rcx,r15
	sbb	r15,rbp
	sbb	r11,0

	cmovc	r12,r8
	cmovc	r13,r9
	mov	QWORD[rdi],r12
	cmovc	r14,r10
	mov	QWORD[8+rdi],r13
	cmovc	r15,rcx
	mov	QWORD[16+rdi],r14
	mov	QWORD[24+rdi],r15

	ret


global	ecp_nistz256_mul_mont_adx

ALIGN	32
ecp_nistz256_mul_mont_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_mul_mont_adx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$mulx_body:
	mov	rbx,rdx
	mov	rdx,QWORD[rdx]
	mov	r9,QWORD[rsi]
	mov	r10,QWORD[8+rsi]
	mov	r11,QWORD[16+rsi]
	mov	r12,QWORD[24+rsi]
	lea	rsi,[((-128))+rsi]

	call	__ecp_nistz256_mul_montx

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$mulx_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_mul_mont_adx:


ALIGN	32
__ecp_nistz256_mul_montx:



	mulx	r9,r8,r9
	mulx	r10,rcx,r10
	mov	r14,32
	xor	r13,r13
	mulx	r11,rbp,r11
	mov	r15,QWORD[(($L$poly+24))]
	adc	r9,rcx
	mulx	r12,rcx,r12
	mov	rdx,r8
	adc	r10,rbp
	shlx	rbp,r8,r14
	adc	r11,rcx
	shrx	rcx,r8,r14
	adc	r12,0



	add	r9,rbp
	adc	r10,rcx

	mulx	rbp,rcx,r15
	mov	rdx,QWORD[8+rbx]
	adc	r11,rcx
	adc	r12,rbp
	adc	r13,0
	xor	r8,r8



	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r9,rcx
	adox	r10,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r9
	adcx	r12,rcx
	shlx	rcx,r9,r14
	adox	r13,rbp
	shrx	rbp,r9,r14

	adcx	r13,r8
	adox	r8,r8
	adc	r8,0



	add	r10,rcx
	adc	r11,rbp

	mulx	rbp,rcx,r15
	mov	rdx,QWORD[16+rbx]
	adc	r12,rcx
	adc	r13,rbp
	adc	r8,0
	xor	r9,r9



	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r10,rcx
	adox	r11,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r10
	adcx	r13,rcx
	shlx	rcx,r10,r14
	adox	r8,rbp
	shrx	rbp,r10,r14

	adcx	r8,r9
	adox	r9,r9
	adc	r9,0



	add	r11,rcx
	adc	r12,rbp

	mulx	rbp,rcx,r15
	mov	rdx,QWORD[24+rbx]
	adc	r13,rcx
	adc	r8,rbp
	adc	r9,0
	xor	r10,r10



	mulx	rbp,rcx,QWORD[((0+128))+rsi]
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,QWORD[((8+128))+rsi]
	adcx	r12,rcx
	adox	r13,rbp

	mulx	rbp,rcx,QWORD[((16+128))+rsi]
	adcx	r13,rcx
	adox	r8,rbp

	mulx	rbp,rcx,QWORD[((24+128))+rsi]
	mov	rdx,r11
	adcx	r8,rcx
	shlx	rcx,r11,r14
	adox	r9,rbp
	shrx	rbp,r11,r14

	adcx	r9,r10
	adox	r10,r10
	adc	r10,0



	add	r12,rcx
	adc	r13,rbp

	mulx	rbp,rcx,r15
	mov	rbx,r12
	mov	r14,QWORD[(($L$poly+8))]
	adc	r8,rcx
	mov	rdx,r13
	adc	r9,rbp
	adc	r10,0



	xor	eax,eax
	mov	rcx,r8
	sbb	r12,-1
	sbb	r13,r14
	sbb	r8,0
	mov	rbp,r9
	sbb	r9,r15
	sbb	r10,0

	cmovc	r12,rbx
	cmovc	r13,rdx
	mov	QWORD[rdi],r12
	cmovc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovc	r9,rbp
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret



global	ecp_nistz256_sqr_mont_adx

ALIGN	32
ecp_nistz256_sqr_mont_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_sqr_mont_adx:
	mov	rdi,rcx
	mov	rsi,rdx



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

$L$sqrx_body:
	mov	rdx,QWORD[rsi]
	mov	r14,QWORD[8+rsi]
	mov	r15,QWORD[16+rsi]
	mov	r8,QWORD[24+rsi]
	lea	rsi,[((-128))+rsi]

	call	__ecp_nistz256_sqr_montx

	mov	r15,QWORD[rsp]

	mov	r14,QWORD[8+rsp]

	mov	r13,QWORD[16+rsp]

	mov	r12,QWORD[24+rsp]

	mov	rbx,QWORD[32+rsp]

	mov	rbp,QWORD[40+rsp]

	lea	rsp,[48+rsp]

$L$sqrx_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_sqr_mont_adx:


ALIGN	32
__ecp_nistz256_sqr_montx:

	mulx	r10,r9,r14
	mulx	r11,rcx,r15
	xor	eax,eax
	adc	r10,rcx
	mulx	r12,rbp,r8
	mov	rdx,r14
	adc	r11,rbp
	adc	r12,0
	xor	r13,r13


	mulx	rbp,rcx,r15
	adcx	r11,rcx
	adox	r12,rbp

	mulx	rbp,rcx,r8
	mov	rdx,r15
	adcx	r12,rcx
	adox	r13,rbp
	adc	r13,0


	mulx	r14,rcx,r8
	mov	rdx,QWORD[((0+128))+rsi]
	xor	r15,r15
	adcx	r9,r9
	adox	r13,rcx
	adcx	r10,r10
	adox	r14,r15

	mulx	rbp,r8,rdx
	mov	rdx,QWORD[((8+128))+rsi]
	adcx	r11,r11
	adox	r9,rbp
	adcx	r12,r12
	mulx	rax,rcx,rdx
	mov	rdx,QWORD[((16+128))+rsi]
	adcx	r13,r13
	adox	r10,rcx
	adcx	r14,r14
	DB	0x67
	mulx	rbp,rcx,rdx
	mov	rdx,QWORD[((24+128))+rsi]
	adox	r11,rax
	adcx	r15,r15
	adox	r12,rcx
	mov	rsi,32
	adox	r13,rbp
	DB	0x67,0x67
	mulx	rax,rcx,rdx
	mov	rdx,QWORD[(($L$poly+24))]
	adox	r14,rcx
	shlx	rcx,r8,rsi
	adox	r15,rax
	shrx	rax,r8,rsi
	mov	rbp,rdx


	add	r9,rcx
	adc	r10,rax

	mulx	r8,rcx,r8
	adc	r11,rcx
	shlx	rcx,r9,rsi
	adc	r8,0
	shrx	rax,r9,rsi


	add	r10,rcx
	adc	r11,rax

	mulx	r9,rcx,r9
	adc	r8,rcx
	shlx	rcx,r10,rsi
	adc	r9,0
	shrx	rax,r10,rsi


	add	r11,rcx
	adc	r8,rax

	mulx	r10,rcx,r10
	adc	r9,rcx
	shlx	rcx,r11,rsi
	adc	r10,0
	shrx	rax,r11,rsi


	add	r8,rcx
	adc	r9,rax

	mulx	r11,rcx,r11
	adc	r10,rcx
	adc	r11,0

	xor	rdx,rdx
	add	r12,r8
	mov	rsi,QWORD[(($L$poly+8))]
	adc	r13,r9
	mov	r8,r12
	adc	r14,r10
	adc	r15,r11
	mov	r9,r13
	adc	rdx,0

	sub	r12,-1
	mov	r10,r14
	sbb	r13,rsi
	sbb	r14,0
	mov	r11,r15
	sbb	r15,rbp
	sbb	rdx,0

	cmovc	r12,r8
	cmovc	r13,r9
	mov	QWORD[rdi],r12
	cmovc	r14,r10
	mov	QWORD[8+rdi],r13
	cmovc	r15,r11
	mov	QWORD[16+rdi],r14
	mov	QWORD[24+rdi],r15

	ret




global	ecp_nistz256_select_w5_nohw

ALIGN	32
ecp_nistz256_select_w5_nohw:

_CET_ENDBR
	lea	rax,[((-136))+rsp]
$L$SEH_begin_ecp_nistz256_select_w5_nohw:
	DB	0x48,0x8d,0x60,0xe0
	DB	0x0f,0x29,0x70,0xe0
	DB	0x0f,0x29,0x78,0xf0
	DB	0x44,0x0f,0x29,0x00
	DB	0x44,0x0f,0x29,0x48,0x10
	DB	0x44,0x0f,0x29,0x50,0x20
	DB	0x44,0x0f,0x29,0x58,0x30
	DB	0x44,0x0f,0x29,0x60,0x40
	DB	0x44,0x0f,0x29,0x68,0x50
	DB	0x44,0x0f,0x29,0x70,0x60
	DB	0x44,0x0f,0x29,0x78,0x70
	movdqa	xmm0,XMMWORD[$L$One]
	movd	xmm1,r8d

	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5
	pxor	xmm6,xmm6
	pxor	xmm7,xmm7

	movdqa	xmm8,xmm0
	pshufd	xmm1,xmm1,0

	mov	rax,16
$L$select_loop_sse_w5:

	movdqa	xmm15,xmm8
	paddd	xmm8,xmm0
	pcmpeqd	xmm15,xmm1

	movdqa	xmm9,XMMWORD[rdx]
	movdqa	xmm10,XMMWORD[16+rdx]
	movdqa	xmm11,XMMWORD[32+rdx]
	movdqa	xmm12,XMMWORD[48+rdx]
	movdqa	xmm13,XMMWORD[64+rdx]
	movdqa	xmm14,XMMWORD[80+rdx]
	lea	rdx,[96+rdx]

	pand	xmm9,xmm15
	pand	xmm10,xmm15
	por	xmm2,xmm9
	pand	xmm11,xmm15
	por	xmm3,xmm10
	pand	xmm12,xmm15
	por	xmm4,xmm11
	pand	xmm13,xmm15
	por	xmm5,xmm12
	pand	xmm14,xmm15
	por	xmm6,xmm13
	por	xmm7,xmm14

	dec	rax
	jnz	NEAR $L$select_loop_sse_w5

	movdqu	XMMWORD[rcx],xmm2
	movdqu	XMMWORD[16+rcx],xmm3
	movdqu	XMMWORD[32+rcx],xmm4
	movdqu	XMMWORD[48+rcx],xmm5
	movdqu	XMMWORD[64+rcx],xmm6
	movdqu	XMMWORD[80+rcx],xmm7
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

$L$SEH_end_ecp_nistz256_select_w5_nohw:




global	ecp_nistz256_select_w7_nohw

ALIGN	32
ecp_nistz256_select_w7_nohw:

_CET_ENDBR
	lea	rax,[((-136))+rsp]
$L$SEH_begin_ecp_nistz256_select_w7_nohw:
	DB	0x48,0x8d,0x60,0xe0
	DB	0x0f,0x29,0x70,0xe0
	DB	0x0f,0x29,0x78,0xf0
	DB	0x44,0x0f,0x29,0x00
	DB	0x44,0x0f,0x29,0x48,0x10
	DB	0x44,0x0f,0x29,0x50,0x20
	DB	0x44,0x0f,0x29,0x58,0x30
	DB	0x44,0x0f,0x29,0x60,0x40
	DB	0x44,0x0f,0x29,0x68,0x50
	DB	0x44,0x0f,0x29,0x70,0x60
	DB	0x44,0x0f,0x29,0x78,0x70
	movdqa	xmm8,XMMWORD[$L$One]
	movd	xmm1,r8d

	pxor	xmm2,xmm2
	pxor	xmm3,xmm3
	pxor	xmm4,xmm4
	pxor	xmm5,xmm5

	movdqa	xmm0,xmm8
	pshufd	xmm1,xmm1,0
	mov	rax,64

$L$select_loop_sse_w7:
	movdqa	xmm15,xmm8
	paddd	xmm8,xmm0
	movdqa	xmm9,XMMWORD[rdx]
	movdqa	xmm10,XMMWORD[16+rdx]
	pcmpeqd	xmm15,xmm1
	movdqa	xmm11,XMMWORD[32+rdx]
	movdqa	xmm12,XMMWORD[48+rdx]
	lea	rdx,[64+rdx]

	pand	xmm9,xmm15
	pand	xmm10,xmm15
	por	xmm2,xmm9
	pand	xmm11,xmm15
	por	xmm3,xmm10
	pand	xmm12,xmm15
	por	xmm4,xmm11
	prefetcht0	[255+rdx]
	por	xmm5,xmm12

	dec	rax
	jnz	NEAR $L$select_loop_sse_w7

	movdqu	XMMWORD[rcx],xmm2
	movdqu	XMMWORD[16+rcx],xmm3
	movdqu	XMMWORD[32+rcx],xmm4
	movdqu	XMMWORD[48+rcx],xmm5
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

$L$SEH_end_ecp_nistz256_select_w7_nohw:



global	ecp_nistz256_select_w5_avx2

ALIGN	32
ecp_nistz256_select_w5_avx2:

_CET_ENDBR
	vzeroupper
	lea	rax,[((-136))+rsp]
	mov	r11,rsp
$L$SEH_begin_ecp_nistz256_select_w5_avx2:
	DB	0x48,0x8d,0x60,0xe0
	DB	0xc5,0xf8,0x29,0x70,0xe0
	DB	0xc5,0xf8,0x29,0x78,0xf0
	DB	0xc5,0x78,0x29,0x40,0x00
	DB	0xc5,0x78,0x29,0x48,0x10
	DB	0xc5,0x78,0x29,0x50,0x20
	DB	0xc5,0x78,0x29,0x58,0x30
	DB	0xc5,0x78,0x29,0x60,0x40
	DB	0xc5,0x78,0x29,0x68,0x50
	DB	0xc5,0x78,0x29,0x70,0x60
	DB	0xc5,0x78,0x29,0x78,0x70
	vmovdqa	ymm0,YMMWORD[$L$Two]

	vpxor	ymm2,ymm2,ymm2
	vpxor	ymm3,ymm3,ymm3
	vpxor	ymm4,ymm4,ymm4

	vmovdqa	ymm5,YMMWORD[$L$One]
	vmovdqa	ymm10,YMMWORD[$L$Two]

	vmovd	xmm1,r8d
	vpermd	ymm1,ymm2,ymm1

	mov	rax,8
$L$select_loop_avx2_w5:

	vmovdqa	ymm6,YMMWORD[rdx]
	vmovdqa	ymm7,YMMWORD[32+rdx]
	vmovdqa	ymm8,YMMWORD[64+rdx]

	vmovdqa	ymm11,YMMWORD[96+rdx]
	vmovdqa	ymm12,YMMWORD[128+rdx]
	vmovdqa	ymm13,YMMWORD[160+rdx]

	vpcmpeqd	ymm9,ymm5,ymm1
	vpcmpeqd	ymm14,ymm10,ymm1

	vpaddd	ymm5,ymm5,ymm0
	vpaddd	ymm10,ymm10,ymm0
	lea	rdx,[192+rdx]

	vpand	ymm6,ymm6,ymm9
	vpand	ymm7,ymm7,ymm9
	vpand	ymm8,ymm8,ymm9
	vpand	ymm11,ymm11,ymm14
	vpand	ymm12,ymm12,ymm14
	vpand	ymm13,ymm13,ymm14

	vpxor	ymm2,ymm2,ymm6
	vpxor	ymm3,ymm3,ymm7
	vpxor	ymm4,ymm4,ymm8
	vpxor	ymm2,ymm2,ymm11
	vpxor	ymm3,ymm3,ymm12
	vpxor	ymm4,ymm4,ymm13

	dec	rax
	jnz	NEAR $L$select_loop_avx2_w5

	vmovdqu	YMMWORD[rcx],ymm2
	vmovdqu	YMMWORD[32+rcx],ymm3
	vmovdqu	YMMWORD[64+rcx],ymm4
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
	lea	rsp,[r11]
	ret

$L$SEH_end_ecp_nistz256_select_w5_avx2:




global	ecp_nistz256_select_w7_avx2

ALIGN	32
ecp_nistz256_select_w7_avx2:

_CET_ENDBR
	vzeroupper
	mov	r11,rsp
	lea	rax,[((-136))+rsp]
$L$SEH_begin_ecp_nistz256_select_w7_avx2:
	DB	0x48,0x8d,0x60,0xe0
	DB	0xc5,0xf8,0x29,0x70,0xe0
	DB	0xc5,0xf8,0x29,0x78,0xf0
	DB	0xc5,0x78,0x29,0x40,0x00
	DB	0xc5,0x78,0x29,0x48,0x10
	DB	0xc5,0x78,0x29,0x50,0x20
	DB	0xc5,0x78,0x29,0x58,0x30
	DB	0xc5,0x78,0x29,0x60,0x40
	DB	0xc5,0x78,0x29,0x68,0x50
	DB	0xc5,0x78,0x29,0x70,0x60
	DB	0xc5,0x78,0x29,0x78,0x70
	vmovdqa	ymm0,YMMWORD[$L$Three]

	vpxor	ymm2,ymm2,ymm2
	vpxor	ymm3,ymm3,ymm3

	vmovdqa	ymm4,YMMWORD[$L$One]
	vmovdqa	ymm8,YMMWORD[$L$Two]
	vmovdqa	ymm12,YMMWORD[$L$Three]

	vmovd	xmm1,r8d
	vpermd	ymm1,ymm2,ymm1


	mov	rax,21
$L$select_loop_avx2_w7:

	vmovdqa	ymm5,YMMWORD[rdx]
	vmovdqa	ymm6,YMMWORD[32+rdx]

	vmovdqa	ymm9,YMMWORD[64+rdx]
	vmovdqa	ymm10,YMMWORD[96+rdx]

	vmovdqa	ymm13,YMMWORD[128+rdx]
	vmovdqa	ymm14,YMMWORD[160+rdx]

	vpcmpeqd	ymm7,ymm4,ymm1
	vpcmpeqd	ymm11,ymm8,ymm1
	vpcmpeqd	ymm15,ymm12,ymm1

	vpaddd	ymm4,ymm4,ymm0
	vpaddd	ymm8,ymm8,ymm0
	vpaddd	ymm12,ymm12,ymm0
	lea	rdx,[192+rdx]

	vpand	ymm5,ymm5,ymm7
	vpand	ymm6,ymm6,ymm7
	vpand	ymm9,ymm9,ymm11
	vpand	ymm10,ymm10,ymm11
	vpand	ymm13,ymm13,ymm15
	vpand	ymm14,ymm14,ymm15

	vpxor	ymm2,ymm2,ymm5
	vpxor	ymm3,ymm3,ymm6
	vpxor	ymm2,ymm2,ymm9
	vpxor	ymm3,ymm3,ymm10
	vpxor	ymm2,ymm2,ymm13
	vpxor	ymm3,ymm3,ymm14

	dec	rax
	jnz	NEAR $L$select_loop_avx2_w7


	vmovdqa	ymm5,YMMWORD[rdx]
	vmovdqa	ymm6,YMMWORD[32+rdx]

	vpcmpeqd	ymm7,ymm4,ymm1

	vpand	ymm5,ymm5,ymm7
	vpand	ymm6,ymm6,ymm7

	vpxor	ymm2,ymm2,ymm5
	vpxor	ymm3,ymm3,ymm6

	vmovdqu	YMMWORD[rcx],ymm2
	vmovdqu	YMMWORD[32+rcx],ymm3
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
	lea	rsp,[r11]
	ret

$L$SEH_end_ecp_nistz256_select_w7_avx2:


ALIGN	32
__ecp_nistz256_add_toq:

	xor	r11,r11
	add	r12,QWORD[rbx]
	adc	r13,QWORD[8+rbx]
	mov	rax,r12
	adc	r8,QWORD[16+rbx]
	adc	r9,QWORD[24+rbx]
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	cmovc	r13,rbp
	mov	QWORD[rdi],r12
	cmovc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovc	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret




ALIGN	32
__ecp_nistz256_sub_fromq:

	sub	r12,QWORD[rbx]
	sbb	r13,QWORD[8+rbx]
	mov	rax,r12
	sbb	r8,QWORD[16+rbx]
	sbb	r9,QWORD[24+rbx]
	mov	rbp,r13
	sbb	r11,r11

	add	r12,-1
	mov	rcx,r8
	adc	r13,r14
	adc	r8,0
	mov	r10,r9
	adc	r9,r15
	test	r11,r11

	cmovz	r12,rax
	cmovz	r13,rbp
	mov	QWORD[rdi],r12
	cmovz	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovz	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret




ALIGN	32
__ecp_nistz256_subq:

	sub	rax,r12
	sbb	rbp,r13
	mov	r12,rax
	sbb	rcx,r8
	sbb	r10,r9
	mov	r13,rbp
	sbb	r11,r11

	add	rax,-1
	mov	r8,rcx
	adc	rbp,r14
	adc	rcx,0
	mov	r9,r10
	adc	r10,r15
	test	r11,r11

	cmovnz	r12,rax
	cmovnz	r13,rbp
	cmovnz	r8,rcx
	cmovnz	r9,r10

	ret




ALIGN	32
__ecp_nistz256_mul_by_2q:

	xor	r11,r11
	add	r12,r12
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	cmovc	r13,rbp
	mov	QWORD[rdi],r12
	cmovc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovc	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret


global	ecp_nistz256_point_double_nohw

ALIGN	32
ecp_nistz256_point_double_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_double_nohw:
	mov	rdi,rcx
	mov	rsi,rdx



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*5+8

$L$point_doubleq_body:

$L$point_double_shortcutq:
	movdqu	xmm0,XMMWORD[rsi]
	mov	rbx,rsi
	movdqu	xmm1,XMMWORD[16+rsi]
	mov	r12,QWORD[((32+0))+rsi]
	mov	r13,QWORD[((32+8))+rsi]
	mov	r8,QWORD[((32+16))+rsi]
	mov	r9,QWORD[((32+24))+rsi]
	mov	r14,QWORD[(($L$poly+8))]
	mov	r15,QWORD[(($L$poly+24))]
	movdqa	XMMWORD[96+rsp],xmm0
	movdqa	XMMWORD[(96+16)+rsp],xmm1
	lea	r10,[32+rdi]
	lea	r11,[64+rdi]
DB	102,72,15,110,199
DB	102,73,15,110,202
DB	102,73,15,110,211

	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_by_2q

	mov	rax,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	lea	rsi,[((64-0))+rsi]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((0+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[32+rbx]
	mov	r9,QWORD[((64+0))+rbx]
	mov	r10,QWORD[((64+8))+rbx]
	mov	r11,QWORD[((64+16))+rbx]
	mov	r12,QWORD[((64+24))+rbx]
	lea	rsi,[((64-0))+rbx]
	lea	rbx,[32+rbx]
DB	102,72,15,126,215
	call	__ecp_nistz256_mul_montq
	call	__ecp_nistz256_mul_by_2q

	mov	r12,QWORD[((96+0))+rsp]
	mov	r13,QWORD[((96+8))+rsp]
	lea	rbx,[64+rsp]
	mov	r8,QWORD[((96+16))+rsp]
	mov	r9,QWORD[((96+24))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_add_toq

	mov	r12,QWORD[((96+0))+rsp]
	mov	r13,QWORD[((96+8))+rsp]
	lea	rbx,[64+rsp]
	mov	r8,QWORD[((96+16))+rsp]
	mov	r9,QWORD[((96+24))+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((0+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
DB	102,72,15,126,207
	call	__ecp_nistz256_sqr_montq
	xor	r9,r9
	mov	rax,r12
	add	r12,-1
	mov	r10,r13
	adc	r13,rsi
	mov	rcx,r14
	adc	r14,0
	mov	r8,r15
	adc	r15,rbp
	adc	r9,0
	xor	rsi,rsi
	test	rax,1

	cmovz	r12,rax
	cmovz	r13,r10
	cmovz	r14,rcx
	cmovz	r15,r8
	cmovz	r9,rsi

	mov	rax,r13
	shr	r12,1
	shl	rax,63
	mov	r10,r14
	shr	r13,1
	or	r12,rax
	shl	r10,63
	mov	rcx,r15
	shr	r14,1
	or	r13,r10
	shl	rcx,63
	mov	QWORD[rdi],r12
	shr	r15,1
	mov	QWORD[8+rdi],r13
	shl	r9,63
	or	r14,rcx
	or	r15,r9
	mov	QWORD[16+rdi],r14
	mov	QWORD[24+rdi],r15
	mov	rax,QWORD[64+rsp]
	lea	rbx,[64+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_by_2q

	lea	rbx,[32+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_add_toq

	mov	rax,QWORD[96+rsp]
	lea	rbx,[96+rsp]
	mov	r9,QWORD[((0+0))+rsp]
	mov	r10,QWORD[((8+0))+rsp]
	lea	rsi,[((0+0))+rsp]
	mov	r11,QWORD[((16+0))+rsp]
	mov	r12,QWORD[((24+0))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montq

	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_by_2q

	mov	rax,QWORD[((0+32))+rsp]
	mov	r14,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r15,QWORD[((16+32))+rsp]
	mov	r8,QWORD[((24+32))+rsp]
DB	102,72,15,126,199
	call	__ecp_nistz256_sqr_montq

	lea	rbx,[128+rsp]
	mov	r8,r14
	mov	r9,r15
	mov	r14,rsi
	mov	r15,rbp
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[((0+0))+rsp]
	mov	rbp,QWORD[((0+8))+rsp]
	mov	rcx,QWORD[((0+16))+rsp]
	mov	r10,QWORD[((0+24))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_subq

	mov	rax,QWORD[32+rsp]
	lea	rbx,[32+rsp]
	mov	r14,r12
	xor	ecx,ecx
	mov	QWORD[((0+0))+rsp],r12
	mov	r10,r13
	mov	QWORD[((0+8))+rsp],r13
	cmovz	r11,r8
	mov	QWORD[((0+16))+rsp],r8
	lea	rsi,[((0-0))+rsp]
	cmovz	r12,r9
	mov	QWORD[((0+24))+rsp],r9
	mov	r9,r14
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montq

DB	102,72,15,126,203
DB	102,72,15,126,207
	call	__ecp_nistz256_sub_fromq

	lea	rsi,[((160+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$point_doubleq_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_double_nohw:
global	ecp_nistz256_point_add_nohw

ALIGN	32
ecp_nistz256_point_add_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_add_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*18+8

$L$point_addq_body:

	movdqu	xmm0,XMMWORD[rsi]
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm3,XMMWORD[48+rsi]
	movdqu	xmm4,XMMWORD[64+rsi]
	movdqu	xmm5,XMMWORD[80+rsi]
	mov	rbx,rsi
	mov	rsi,rdx
	movdqa	XMMWORD[384+rsp],xmm0
	movdqa	XMMWORD[(384+16)+rsp],xmm1
	movdqa	XMMWORD[416+rsp],xmm2
	movdqa	XMMWORD[(416+16)+rsp],xmm3
	movdqa	XMMWORD[448+rsp],xmm4
	movdqa	XMMWORD[(448+16)+rsp],xmm5
	por	xmm5,xmm4

	movdqu	xmm0,XMMWORD[rsi]
	pshufd	xmm3,xmm5,0xb1
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	por	xmm5,xmm3
	movdqu	xmm3,XMMWORD[48+rsi]
	mov	rax,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	movdqa	XMMWORD[480+rsp],xmm0
	pshufd	xmm4,xmm5,0x1e
	movdqa	XMMWORD[(480+16)+rsp],xmm1
	movdqu	xmm0,XMMWORD[64+rsi]
	movdqu	xmm1,XMMWORD[80+rsi]
	movdqa	XMMWORD[512+rsp],xmm2
	movdqa	XMMWORD[(512+16)+rsp],xmm3
	por	xmm5,xmm4
	pxor	xmm4,xmm4
	por	xmm1,xmm0
DB	102,72,15,110,199

	lea	rsi,[((64-0))+rsi]
	mov	QWORD[((544+0))+rsp],rax
	mov	QWORD[((544+8))+rsp],r14
	mov	QWORD[((544+16))+rsp],r15
	mov	QWORD[((544+24))+rsp],r8
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sqr_montq

	pcmpeqd	xmm5,xmm4
	pshufd	xmm4,xmm1,0xb1
	por	xmm4,xmm1
	pshufd	xmm5,xmm5,0
	pshufd	xmm3,xmm4,0x1e
	por	xmm4,xmm3
	pxor	xmm3,xmm3
	pcmpeqd	xmm4,xmm3
	pshufd	xmm4,xmm4,0
	mov	rax,QWORD[((64+0))+rbx]
	mov	r14,QWORD[((64+8))+rbx]
	mov	r15,QWORD[((64+16))+rbx]
	mov	r8,QWORD[((64+24))+rbx]
DB	102,72,15,110,203

	lea	rsi,[((64-0))+rbx]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[544+rsp]
	lea	rbx,[544+rsp]
	mov	r9,QWORD[((0+96))+rsp]
	mov	r10,QWORD[((8+96))+rsp]
	lea	rsi,[((0+96))+rsp]
	mov	r11,QWORD[((16+96))+rsp]
	mov	r12,QWORD[((24+96))+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[416+rsp]
	lea	rbx,[416+rsp]
	mov	r9,QWORD[((0+224))+rsp]
	mov	r10,QWORD[((8+224))+rsp]
	lea	rsi,[((0+224))+rsp]
	mov	r11,QWORD[((16+224))+rsp]
	mov	r12,QWORD[((24+224))+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[512+rsp]
	lea	rbx,[512+rsp]
	mov	r9,QWORD[((0+256))+rsp]
	mov	r10,QWORD[((8+256))+rsp]
	lea	rsi,[((0+256))+rsp]
	mov	r11,QWORD[((16+256))+rsp]
	mov	r12,QWORD[((24+256))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[224+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromq

	or	r12,r13
	movdqa	xmm2,xmm4
	or	r12,r8
	or	r12,r9
	por	xmm2,xmm5
DB	102,73,15,110,220

	mov	rax,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+96))+rsp]
	mov	r10,QWORD[((8+96))+rsp]
	lea	rsi,[((0+96))+rsp]
	mov	r11,QWORD[((16+96))+rsp]
	mov	r12,QWORD[((24+96))+rsp]
	lea	rdi,[160+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[480+rsp]
	lea	rbx,[480+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[160+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_sub_fromq

	or	r12,r13
	or	r12,r8
	or	r12,r9

DB	102,73,15,126,208
DB	102,73,15,126,217
	or	r12,r8
	DB	0x3e
	jnz	NEAR $L$add_proceedq



	test	r9,r9
	jz	NEAR $L$add_doubleq






DB	102,72,15,126,199
	pxor	xmm0,xmm0
	movdqu	XMMWORD[rdi],xmm0
	movdqu	XMMWORD[16+rdi],xmm0
	movdqu	XMMWORD[32+rdi],xmm0
	movdqu	XMMWORD[48+rdi],xmm0
	movdqu	XMMWORD[64+rdi],xmm0
	movdqu	XMMWORD[80+rdi],xmm0
	jmp	NEAR $L$add_doneq

ALIGN	32
$L$add_doubleq:
DB	102,72,15,126,206
DB	102,72,15,126,199
	add	rsp,416

	jmp	NEAR $L$point_double_shortcutq


ALIGN	32
$L$add_proceedq:
	mov	rax,QWORD[((0+64))+rsp]
	mov	r14,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r15,QWORD[((16+64))+rsp]
	mov	r8,QWORD[((24+64))+rsp]
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+0))+rsp]
	mov	r10,QWORD[((8+0))+rsp]
	lea	rsi,[((0+0))+rsp]
	mov	r11,QWORD[((16+0))+rsp]
	mov	r12,QWORD[((24+0))+rsp]
	lea	rdi,[352+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((0+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[544+rsp]
	lea	rbx,[544+rsp]
	mov	r9,QWORD[((0+352))+rsp]
	mov	r10,QWORD[((8+352))+rsp]
	lea	rsi,[((0+352))+rsp]
	mov	r11,QWORD[((16+352))+rsp]
	mov	r12,QWORD[((24+352))+rsp]
	lea	rdi,[352+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[rsp]
	lea	rbx,[rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[160+rsp]
	lea	rbx,[160+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_mul_montq




	xor	r11,r11
	add	r12,r12
	lea	rsi,[96+rsp]
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	mov	rax,QWORD[rsi]
	cmovc	r13,rbp
	mov	rbp,QWORD[8+rsi]
	cmovc	r8,rcx
	mov	rcx,QWORD[16+rsi]
	cmovc	r9,r10
	mov	r10,QWORD[24+rsi]

	call	__ecp_nistz256_subq

	lea	rbx,[128+rsp]
	lea	rdi,[288+rsp]
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[((192+0))+rsp]
	mov	rbp,QWORD[((192+8))+rsp]
	mov	rcx,QWORD[((192+16))+rsp]
	mov	r10,QWORD[((192+24))+rsp]
	lea	rdi,[320+rsp]

	call	__ecp_nistz256_subq

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9
	mov	rax,QWORD[128+rsp]
	lea	rbx,[128+rsp]
	mov	r9,QWORD[((0+224))+rsp]
	mov	r10,QWORD[((8+224))+rsp]
	lea	rsi,[((0+224))+rsp]
	mov	r11,QWORD[((16+224))+rsp]
	mov	r12,QWORD[((24+224))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[320+rsp]
	lea	rbx,[320+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[320+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[256+rsp]
	lea	rdi,[320+rsp]
	call	__ecp_nistz256_sub_fromq

DB	102,72,15,126,199

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[352+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((352+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[544+rsp]
	pand	xmm3,XMMWORD[((544+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[448+rsp]
	pand	xmm3,XMMWORD[((448+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[64+rdi],xmm2
	movdqu	XMMWORD[80+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[288+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((288+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[480+rsp]
	pand	xmm3,XMMWORD[((480+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[384+rsp]
	pand	xmm3,XMMWORD[((384+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[rdi],xmm2
	movdqu	XMMWORD[16+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[320+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((320+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[512+rsp]
	pand	xmm3,XMMWORD[((512+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[416+rsp]
	pand	xmm3,XMMWORD[((416+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm3

$L$add_doneq:
	lea	rsi,[((576+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$point_addq_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_add_nohw:
global	ecp_nistz256_point_add_affine_nohw

ALIGN	32
ecp_nistz256_point_add_affine_nohw:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_add_affine_nohw:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*15+8

$L$add_affineq_body:

	movdqu	xmm0,XMMWORD[rsi]
	mov	rbx,rdx
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm3,XMMWORD[48+rsi]
	movdqu	xmm4,XMMWORD[64+rsi]
	movdqu	xmm5,XMMWORD[80+rsi]
	mov	rax,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	movdqa	XMMWORD[320+rsp],xmm0
	movdqa	XMMWORD[(320+16)+rsp],xmm1
	movdqa	XMMWORD[352+rsp],xmm2
	movdqa	XMMWORD[(352+16)+rsp],xmm3
	movdqa	XMMWORD[384+rsp],xmm4
	movdqa	XMMWORD[(384+16)+rsp],xmm5
	por	xmm5,xmm4

	movdqu	xmm0,XMMWORD[rbx]
	pshufd	xmm3,xmm5,0xb1
	movdqu	xmm1,XMMWORD[16+rbx]
	movdqu	xmm2,XMMWORD[32+rbx]
	por	xmm5,xmm3
	movdqu	xmm3,XMMWORD[48+rbx]
	movdqa	XMMWORD[416+rsp],xmm0
	pshufd	xmm4,xmm5,0x1e
	movdqa	XMMWORD[(416+16)+rsp],xmm1
	por	xmm1,xmm0
DB	102,72,15,110,199
	movdqa	XMMWORD[448+rsp],xmm2
	movdqa	XMMWORD[(448+16)+rsp],xmm3
	por	xmm3,xmm2
	por	xmm5,xmm4
	pxor	xmm4,xmm4
	por	xmm3,xmm1

	lea	rsi,[((64-0))+rsi]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montq

	pcmpeqd	xmm5,xmm4
	pshufd	xmm4,xmm3,0xb1
	mov	rax,QWORD[rbx]

	mov	r9,r12
	por	xmm4,xmm3
	pshufd	xmm5,xmm5,0
	pshufd	xmm3,xmm4,0x1e
	mov	r10,r13
	por	xmm4,xmm3
	pxor	xmm3,xmm3
	mov	r11,r14
	pcmpeqd	xmm4,xmm3
	pshufd	xmm4,xmm4,0

	lea	rsi,[((32-0))+rsp]
	mov	r12,r15
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[320+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[288+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((0+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[352+rsp]
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[((0+64))+rsp]
	mov	r14,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r15,QWORD[((16+64))+rsp]
	mov	r8,QWORD[((24+64))+rsp]
	lea	rdi,[128+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[((0+96))+rsp]
	mov	r14,QWORD[((8+96))+rsp]
	lea	rsi,[((0+96))+rsp]
	mov	r15,QWORD[((16+96))+rsp]
	mov	r8,QWORD[((24+96))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_sqr_montq

	mov	rax,QWORD[128+rsp]
	lea	rbx,[128+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[160+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[320+rsp]
	lea	rbx,[320+rsp]
	mov	r9,QWORD[((0+128))+rsp]
	mov	r10,QWORD[((8+128))+rsp]
	lea	rsi,[((0+128))+rsp]
	mov	r11,QWORD[((16+128))+rsp]
	mov	r12,QWORD[((24+128))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montq




	xor	r11,r11
	add	r12,r12
	lea	rsi,[192+rsp]
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	mov	rax,QWORD[rsi]
	cmovc	r13,rbp
	mov	rbp,QWORD[8+rsi]
	cmovc	r8,rcx
	mov	rcx,QWORD[16+rsi]
	cmovc	r9,r10
	mov	r10,QWORD[24+rsi]

	call	__ecp_nistz256_subq

	lea	rbx,[160+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_sub_fromq

	mov	rax,QWORD[((0+0))+rsp]
	mov	rbp,QWORD[((0+8))+rsp]
	mov	rcx,QWORD[((0+16))+rsp]
	mov	r10,QWORD[((0+24))+rsp]
	lea	rdi,[64+rsp]

	call	__ecp_nistz256_subq

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9
	mov	rax,QWORD[352+rsp]
	lea	rbx,[352+rsp]
	mov	r9,QWORD[((0+160))+rsp]
	mov	r10,QWORD[((8+160))+rsp]
	lea	rsi,[((0+160))+rsp]
	mov	r11,QWORD[((16+160))+rsp]
	mov	r12,QWORD[((24+160))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montq

	mov	rax,QWORD[96+rsp]
	lea	rbx,[96+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((0+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_mul_montq

	lea	rbx,[32+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_sub_fromq

DB	102,72,15,126,199

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[288+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((288+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[$L$ONE_mont]
	pand	xmm3,XMMWORD[(($L$ONE_mont+16))]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[384+rsp]
	pand	xmm3,XMMWORD[((384+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[64+rdi],xmm2
	movdqu	XMMWORD[80+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[224+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((224+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[416+rsp]
	pand	xmm3,XMMWORD[((416+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[320+rsp]
	pand	xmm3,XMMWORD[((320+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[rdi],xmm2
	movdqu	XMMWORD[16+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[256+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((256+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[448+rsp]
	pand	xmm3,XMMWORD[((448+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[352+rsp]
	pand	xmm3,XMMWORD[((352+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm3

	lea	rsi,[((480+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$add_affineq_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_add_affine_nohw:

ALIGN	32
__ecp_nistz256_add_tox:

	xor	r11,r11
	adc	r12,QWORD[rbx]
	adc	r13,QWORD[8+rbx]
	mov	rax,r12
	adc	r8,QWORD[16+rbx]
	adc	r9,QWORD[24+rbx]
	mov	rbp,r13
	adc	r11,0

	xor	r10,r10
	sbb	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	cmovc	r13,rbp
	mov	QWORD[rdi],r12
	cmovc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovc	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret




ALIGN	32
__ecp_nistz256_sub_fromx:

	xor	r11,r11
	sbb	r12,QWORD[rbx]
	sbb	r13,QWORD[8+rbx]
	mov	rax,r12
	sbb	r8,QWORD[16+rbx]
	sbb	r9,QWORD[24+rbx]
	mov	rbp,r13
	sbb	r11,0

	xor	r10,r10
	adc	r12,-1
	mov	rcx,r8
	adc	r13,r14
	adc	r8,0
	mov	r10,r9
	adc	r9,r15

	bt	r11,0
	cmovnc	r12,rax
	cmovnc	r13,rbp
	mov	QWORD[rdi],r12
	cmovnc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovnc	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret




ALIGN	32
__ecp_nistz256_subx:

	xor	r11,r11
	sbb	rax,r12
	sbb	rbp,r13
	mov	r12,rax
	sbb	rcx,r8
	sbb	r10,r9
	mov	r13,rbp
	sbb	r11,0

	xor	r9,r9
	adc	rax,-1
	mov	r8,rcx
	adc	rbp,r14
	adc	rcx,0
	mov	r9,r10
	adc	r10,r15

	bt	r11,0
	cmovc	r12,rax
	cmovc	r13,rbp
	cmovc	r8,rcx
	cmovc	r9,r10

	ret




ALIGN	32
__ecp_nistz256_mul_by_2x:

	xor	r11,r11
	adc	r12,r12
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	xor	r10,r10
	sbb	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	cmovc	r13,rbp
	mov	QWORD[rdi],r12
	cmovc	r8,rcx
	mov	QWORD[8+rdi],r13
	cmovc	r9,r10
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9

	ret


global	ecp_nistz256_point_double_adx

ALIGN	32
ecp_nistz256_point_double_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_double_adx:
	mov	rdi,rcx
	mov	rsi,rdx



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*5+8

$L$point_doublex_body:

$L$point_double_shortcutx:
	movdqu	xmm0,XMMWORD[rsi]
	mov	rbx,rsi
	movdqu	xmm1,XMMWORD[16+rsi]
	mov	r12,QWORD[((32+0))+rsi]
	mov	r13,QWORD[((32+8))+rsi]
	mov	r8,QWORD[((32+16))+rsi]
	mov	r9,QWORD[((32+24))+rsi]
	mov	r14,QWORD[(($L$poly+8))]
	mov	r15,QWORD[(($L$poly+24))]
	movdqa	XMMWORD[96+rsp],xmm0
	movdqa	XMMWORD[(96+16)+rsp],xmm1
	lea	r10,[32+rdi]
	lea	r11,[64+rdi]
DB	102,72,15,110,199
DB	102,73,15,110,202
DB	102,73,15,110,211

	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_by_2x

	mov	rdx,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	lea	rsi,[((64-128))+rsi]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((-128+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[32+rbx]
	mov	r9,QWORD[((64+0))+rbx]
	mov	r10,QWORD[((64+8))+rbx]
	mov	r11,QWORD[((64+16))+rbx]
	mov	r12,QWORD[((64+24))+rbx]
	lea	rsi,[((64-128))+rbx]
	lea	rbx,[32+rbx]
DB	102,72,15,126,215
	call	__ecp_nistz256_mul_montx
	call	__ecp_nistz256_mul_by_2x

	mov	r12,QWORD[((96+0))+rsp]
	mov	r13,QWORD[((96+8))+rsp]
	lea	rbx,[64+rsp]
	mov	r8,QWORD[((96+16))+rsp]
	mov	r9,QWORD[((96+24))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_add_tox

	mov	r12,QWORD[((96+0))+rsp]
	mov	r13,QWORD[((96+8))+rsp]
	lea	rbx,[64+rsp]
	mov	r8,QWORD[((96+16))+rsp]
	mov	r9,QWORD[((96+24))+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromx

	mov	rdx,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((-128+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
DB	102,72,15,126,207
	call	__ecp_nistz256_sqr_montx
	xor	r9,r9
	mov	rax,r12
	add	r12,-1
	mov	r10,r13
	adc	r13,rsi
	mov	rcx,r14
	adc	r14,0
	mov	r8,r15
	adc	r15,rbp
	adc	r9,0
	xor	rsi,rsi
	test	rax,1

	cmovz	r12,rax
	cmovz	r13,r10
	cmovz	r14,rcx
	cmovz	r15,r8
	cmovz	r9,rsi

	mov	rax,r13
	shr	r12,1
	shl	rax,63
	mov	r10,r14
	shr	r13,1
	or	r12,rax
	shl	r10,63
	mov	rcx,r15
	shr	r14,1
	or	r13,r10
	shl	rcx,63
	mov	QWORD[rdi],r12
	shr	r15,1
	mov	QWORD[8+rdi],r13
	shl	r9,63
	or	r14,rcx
	or	r15,r9
	mov	QWORD[16+rdi],r14
	mov	QWORD[24+rdi],r15
	mov	rdx,QWORD[64+rsp]
	lea	rbx,[64+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_by_2x

	lea	rbx,[32+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_add_tox

	mov	rdx,QWORD[96+rsp]
	lea	rbx,[96+rsp]
	mov	r9,QWORD[((0+0))+rsp]
	mov	r10,QWORD[((8+0))+rsp]
	lea	rsi,[((-128+0))+rsp]
	mov	r11,QWORD[((16+0))+rsp]
	mov	r12,QWORD[((24+0))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montx

	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_by_2x

	mov	rdx,QWORD[((0+32))+rsp]
	mov	r14,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r15,QWORD[((16+32))+rsp]
	mov	r8,QWORD[((24+32))+rsp]
DB	102,72,15,126,199
	call	__ecp_nistz256_sqr_montx

	lea	rbx,[128+rsp]
	mov	r8,r14
	mov	r9,r15
	mov	r14,rsi
	mov	r15,rbp
	call	__ecp_nistz256_sub_fromx

	mov	rax,QWORD[((0+0))+rsp]
	mov	rbp,QWORD[((0+8))+rsp]
	mov	rcx,QWORD[((0+16))+rsp]
	mov	r10,QWORD[((0+24))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_subx

	mov	rdx,QWORD[32+rsp]
	lea	rbx,[32+rsp]
	mov	r14,r12
	xor	ecx,ecx
	mov	QWORD[((0+0))+rsp],r12
	mov	r10,r13
	mov	QWORD[((0+8))+rsp],r13
	cmovz	r11,r8
	mov	QWORD[((0+16))+rsp],r8
	lea	rsi,[((0-128))+rsp]
	cmovz	r12,r9
	mov	QWORD[((0+24))+rsp],r9
	mov	r9,r14
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montx

DB	102,72,15,126,203
DB	102,72,15,126,207
	call	__ecp_nistz256_sub_fromx

	lea	rsi,[((160+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$point_doublex_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_double_adx:
global	ecp_nistz256_point_add_adx

ALIGN	32
ecp_nistz256_point_add_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_add_adx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*18+8

$L$point_addx_body:

	movdqu	xmm0,XMMWORD[rsi]
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm3,XMMWORD[48+rsi]
	movdqu	xmm4,XMMWORD[64+rsi]
	movdqu	xmm5,XMMWORD[80+rsi]
	mov	rbx,rsi
	mov	rsi,rdx
	movdqa	XMMWORD[384+rsp],xmm0
	movdqa	XMMWORD[(384+16)+rsp],xmm1
	movdqa	XMMWORD[416+rsp],xmm2
	movdqa	XMMWORD[(416+16)+rsp],xmm3
	movdqa	XMMWORD[448+rsp],xmm4
	movdqa	XMMWORD[(448+16)+rsp],xmm5
	por	xmm5,xmm4

	movdqu	xmm0,XMMWORD[rsi]
	pshufd	xmm3,xmm5,0xb1
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	por	xmm5,xmm3
	movdqu	xmm3,XMMWORD[48+rsi]
	mov	rdx,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	movdqa	XMMWORD[480+rsp],xmm0
	pshufd	xmm4,xmm5,0x1e
	movdqa	XMMWORD[(480+16)+rsp],xmm1
	movdqu	xmm0,XMMWORD[64+rsi]
	movdqu	xmm1,XMMWORD[80+rsi]
	movdqa	XMMWORD[512+rsp],xmm2
	movdqa	XMMWORD[(512+16)+rsp],xmm3
	por	xmm5,xmm4
	pxor	xmm4,xmm4
	por	xmm1,xmm0
DB	102,72,15,110,199

	lea	rsi,[((64-128))+rsi]
	mov	QWORD[((544+0))+rsp],rdx
	mov	QWORD[((544+8))+rsp],r14
	mov	QWORD[((544+16))+rsp],r15
	mov	QWORD[((544+24))+rsp],r8
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sqr_montx

	pcmpeqd	xmm5,xmm4
	pshufd	xmm4,xmm1,0xb1
	por	xmm4,xmm1
	pshufd	xmm5,xmm5,0
	pshufd	xmm3,xmm4,0x1e
	por	xmm4,xmm3
	pxor	xmm3,xmm3
	pcmpeqd	xmm4,xmm3
	pshufd	xmm4,xmm4,0
	mov	rdx,QWORD[((64+0))+rbx]
	mov	r14,QWORD[((64+8))+rbx]
	mov	r15,QWORD[((64+16))+rbx]
	mov	r8,QWORD[((64+24))+rbx]
DB	102,72,15,110,203

	lea	rsi,[((64-128))+rbx]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[544+rsp]
	lea	rbx,[544+rsp]
	mov	r9,QWORD[((0+96))+rsp]
	mov	r10,QWORD[((8+96))+rsp]
	lea	rsi,[((-128+96))+rsp]
	mov	r11,QWORD[((16+96))+rsp]
	mov	r12,QWORD[((24+96))+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[416+rsp]
	lea	rbx,[416+rsp]
	mov	r9,QWORD[((0+224))+rsp]
	mov	r10,QWORD[((8+224))+rsp]
	lea	rsi,[((-128+224))+rsp]
	mov	r11,QWORD[((16+224))+rsp]
	mov	r12,QWORD[((24+224))+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[512+rsp]
	lea	rbx,[512+rsp]
	mov	r9,QWORD[((0+256))+rsp]
	mov	r10,QWORD[((8+256))+rsp]
	lea	rsi,[((-128+256))+rsp]
	mov	r11,QWORD[((16+256))+rsp]
	mov	r12,QWORD[((24+256))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[224+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromx

	or	r12,r13
	movdqa	xmm2,xmm4
	or	r12,r8
	or	r12,r9
	por	xmm2,xmm5
DB	102,73,15,110,220

	mov	rdx,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+96))+rsp]
	mov	r10,QWORD[((8+96))+rsp]
	lea	rsi,[((-128+96))+rsp]
	mov	r11,QWORD[((16+96))+rsp]
	mov	r12,QWORD[((24+96))+rsp]
	lea	rdi,[160+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[480+rsp]
	lea	rbx,[480+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[160+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_sub_fromx

	or	r12,r13
	or	r12,r8
	or	r12,r9

DB	102,73,15,126,208
DB	102,73,15,126,217
	or	r12,r8
	DB	0x3e
	jnz	NEAR $L$add_proceedx



	test	r9,r9
	jz	NEAR $L$add_doublex






DB	102,72,15,126,199
	pxor	xmm0,xmm0
	movdqu	XMMWORD[rdi],xmm0
	movdqu	XMMWORD[16+rdi],xmm0
	movdqu	XMMWORD[32+rdi],xmm0
	movdqu	XMMWORD[48+rdi],xmm0
	movdqu	XMMWORD[64+rdi],xmm0
	movdqu	XMMWORD[80+rdi],xmm0
	jmp	NEAR $L$add_donex

ALIGN	32
$L$add_doublex:
DB	102,72,15,126,206
DB	102,72,15,126,199
	add	rsp,416

	jmp	NEAR $L$point_double_shortcutx


ALIGN	32
$L$add_proceedx:
	mov	rdx,QWORD[((0+64))+rsp]
	mov	r14,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r15,QWORD[((16+64))+rsp]
	mov	r8,QWORD[((24+64))+rsp]
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+0))+rsp]
	mov	r10,QWORD[((8+0))+rsp]
	lea	rsi,[((-128+0))+rsp]
	mov	r11,QWORD[((16+0))+rsp]
	mov	r12,QWORD[((24+0))+rsp]
	lea	rdi,[352+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[((0+0))+rsp]
	mov	r14,QWORD[((8+0))+rsp]
	lea	rsi,[((-128+0))+rsp]
	mov	r15,QWORD[((16+0))+rsp]
	mov	r8,QWORD[((24+0))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[544+rsp]
	lea	rbx,[544+rsp]
	mov	r9,QWORD[((0+352))+rsp]
	mov	r10,QWORD[((8+352))+rsp]
	lea	rsi,[((-128+352))+rsp]
	mov	r11,QWORD[((16+352))+rsp]
	mov	r12,QWORD[((24+352))+rsp]
	lea	rdi,[352+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[rsp]
	lea	rbx,[rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[128+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[160+rsp]
	lea	rbx,[160+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_mul_montx




	xor	r11,r11
	add	r12,r12
	lea	rsi,[96+rsp]
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	mov	rax,QWORD[rsi]
	cmovc	r13,rbp
	mov	rbp,QWORD[8+rsi]
	cmovc	r8,rcx
	mov	rcx,QWORD[16+rsi]
	cmovc	r9,r10
	mov	r10,QWORD[24+rsi]

	call	__ecp_nistz256_subx

	lea	rbx,[128+rsp]
	lea	rdi,[288+rsp]
	call	__ecp_nistz256_sub_fromx

	mov	rax,QWORD[((192+0))+rsp]
	mov	rbp,QWORD[((192+8))+rsp]
	mov	rcx,QWORD[((192+16))+rsp]
	mov	r10,QWORD[((192+24))+rsp]
	lea	rdi,[320+rsp]

	call	__ecp_nistz256_subx

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9
	mov	rdx,QWORD[128+rsp]
	lea	rbx,[128+rsp]
	mov	r9,QWORD[((0+224))+rsp]
	mov	r10,QWORD[((8+224))+rsp]
	lea	rsi,[((-128+224))+rsp]
	mov	r11,QWORD[((16+224))+rsp]
	mov	r12,QWORD[((24+224))+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[320+rsp]
	lea	rbx,[320+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[320+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[256+rsp]
	lea	rdi,[320+rsp]
	call	__ecp_nistz256_sub_fromx

DB	102,72,15,126,199

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[352+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((352+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[544+rsp]
	pand	xmm3,XMMWORD[((544+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[448+rsp]
	pand	xmm3,XMMWORD[((448+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[64+rdi],xmm2
	movdqu	XMMWORD[80+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[288+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((288+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[480+rsp]
	pand	xmm3,XMMWORD[((480+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[384+rsp]
	pand	xmm3,XMMWORD[((384+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[rdi],xmm2
	movdqu	XMMWORD[16+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[320+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((320+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[512+rsp]
	pand	xmm3,XMMWORD[((512+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[416+rsp]
	pand	xmm3,XMMWORD[((416+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm3

$L$add_donex:
	lea	rsi,[((576+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$point_addx_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_add_adx:
global	ecp_nistz256_point_add_affine_adx

ALIGN	32
ecp_nistz256_point_add_affine_adx:
	mov	QWORD[8+rsp],rdi	;WIN64 prologue
	mov	QWORD[16+rsp],rsi
	mov	rax,rsp
$L$SEH_begin_ecp_nistz256_point_add_affine_adx:
	mov	rdi,rcx
	mov	rsi,rdx
	mov	rdx,r8



_CET_ENDBR
	push	rbp

	push	rbx

	push	r12

	push	r13

	push	r14

	push	r15

	sub	rsp,32*15+8

$L$add_affinex_body:

	movdqu	xmm0,XMMWORD[rsi]
	mov	rbx,rdx
	movdqu	xmm1,XMMWORD[16+rsi]
	movdqu	xmm2,XMMWORD[32+rsi]
	movdqu	xmm3,XMMWORD[48+rsi]
	movdqu	xmm4,XMMWORD[64+rsi]
	movdqu	xmm5,XMMWORD[80+rsi]
	mov	rdx,QWORD[((64+0))+rsi]
	mov	r14,QWORD[((64+8))+rsi]
	mov	r15,QWORD[((64+16))+rsi]
	mov	r8,QWORD[((64+24))+rsi]
	movdqa	XMMWORD[320+rsp],xmm0
	movdqa	XMMWORD[(320+16)+rsp],xmm1
	movdqa	XMMWORD[352+rsp],xmm2
	movdqa	XMMWORD[(352+16)+rsp],xmm3
	movdqa	XMMWORD[384+rsp],xmm4
	movdqa	XMMWORD[(384+16)+rsp],xmm5
	por	xmm5,xmm4

	movdqu	xmm0,XMMWORD[rbx]
	pshufd	xmm3,xmm5,0xb1
	movdqu	xmm1,XMMWORD[16+rbx]
	movdqu	xmm2,XMMWORD[32+rbx]
	por	xmm5,xmm3
	movdqu	xmm3,XMMWORD[48+rbx]
	movdqa	XMMWORD[416+rsp],xmm0
	pshufd	xmm4,xmm5,0x1e
	movdqa	XMMWORD[(416+16)+rsp],xmm1
	por	xmm1,xmm0
DB	102,72,15,110,199
	movdqa	XMMWORD[448+rsp],xmm2
	movdqa	XMMWORD[(448+16)+rsp],xmm3
	por	xmm3,xmm2
	por	xmm5,xmm4
	pxor	xmm4,xmm4
	por	xmm3,xmm1

	lea	rsi,[((64-128))+rsi]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_sqr_montx

	pcmpeqd	xmm5,xmm4
	pshufd	xmm4,xmm3,0xb1
	mov	rdx,QWORD[rbx]

	mov	r9,r12
	por	xmm4,xmm3
	pshufd	xmm5,xmm5,0
	pshufd	xmm3,xmm4,0x1e
	mov	r10,r13
	por	xmm4,xmm3
	pxor	xmm3,xmm3
	mov	r11,r14
	pcmpeqd	xmm4,xmm3
	pshufd	xmm4,xmm4,0

	lea	rsi,[((32-128))+rsp]
	mov	r12,r15
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[320+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_sub_fromx

	mov	rdx,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[384+rsp]
	lea	rbx,[384+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[288+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[448+rsp]
	lea	rbx,[448+rsp]
	mov	r9,QWORD[((0+32))+rsp]
	mov	r10,QWORD[((8+32))+rsp]
	lea	rsi,[((-128+32))+rsp]
	mov	r11,QWORD[((16+32))+rsp]
	mov	r12,QWORD[((24+32))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[352+rsp]
	lea	rdi,[96+rsp]
	call	__ecp_nistz256_sub_fromx

	mov	rdx,QWORD[((0+64))+rsp]
	mov	r14,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r15,QWORD[((16+64))+rsp]
	mov	r8,QWORD[((24+64))+rsp]
	lea	rdi,[128+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[((0+96))+rsp]
	mov	r14,QWORD[((8+96))+rsp]
	lea	rsi,[((-128+96))+rsp]
	mov	r15,QWORD[((16+96))+rsp]
	mov	r8,QWORD[((24+96))+rsp]
	lea	rdi,[192+rsp]
	call	__ecp_nistz256_sqr_montx

	mov	rdx,QWORD[128+rsp]
	lea	rbx,[128+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[160+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[320+rsp]
	lea	rbx,[320+rsp]
	mov	r9,QWORD[((0+128))+rsp]
	mov	r10,QWORD[((8+128))+rsp]
	lea	rsi,[((-128+128))+rsp]
	mov	r11,QWORD[((16+128))+rsp]
	mov	r12,QWORD[((24+128))+rsp]
	lea	rdi,[rsp]
	call	__ecp_nistz256_mul_montx




	xor	r11,r11
	add	r12,r12
	lea	rsi,[192+rsp]
	adc	r13,r13
	mov	rax,r12
	adc	r8,r8
	adc	r9,r9
	mov	rbp,r13
	adc	r11,0

	sub	r12,-1
	mov	rcx,r8
	sbb	r13,r14
	sbb	r8,0
	mov	r10,r9
	sbb	r9,r15
	sbb	r11,0

	cmovc	r12,rax
	mov	rax,QWORD[rsi]
	cmovc	r13,rbp
	mov	rbp,QWORD[8+rsi]
	cmovc	r8,rcx
	mov	rcx,QWORD[16+rsi]
	cmovc	r9,r10
	mov	r10,QWORD[24+rsi]

	call	__ecp_nistz256_subx

	lea	rbx,[160+rsp]
	lea	rdi,[224+rsp]
	call	__ecp_nistz256_sub_fromx

	mov	rax,QWORD[((0+0))+rsp]
	mov	rbp,QWORD[((0+8))+rsp]
	mov	rcx,QWORD[((0+16))+rsp]
	mov	r10,QWORD[((0+24))+rsp]
	lea	rdi,[64+rsp]

	call	__ecp_nistz256_subx

	mov	QWORD[rdi],r12
	mov	QWORD[8+rdi],r13
	mov	QWORD[16+rdi],r8
	mov	QWORD[24+rdi],r9
	mov	rdx,QWORD[352+rsp]
	lea	rbx,[352+rsp]
	mov	r9,QWORD[((0+160))+rsp]
	mov	r10,QWORD[((8+160))+rsp]
	lea	rsi,[((-128+160))+rsp]
	mov	r11,QWORD[((16+160))+rsp]
	mov	r12,QWORD[((24+160))+rsp]
	lea	rdi,[32+rsp]
	call	__ecp_nistz256_mul_montx

	mov	rdx,QWORD[96+rsp]
	lea	rbx,[96+rsp]
	mov	r9,QWORD[((0+64))+rsp]
	mov	r10,QWORD[((8+64))+rsp]
	lea	rsi,[((-128+64))+rsp]
	mov	r11,QWORD[((16+64))+rsp]
	mov	r12,QWORD[((24+64))+rsp]
	lea	rdi,[64+rsp]
	call	__ecp_nistz256_mul_montx

	lea	rbx,[32+rsp]
	lea	rdi,[256+rsp]
	call	__ecp_nistz256_sub_fromx

DB	102,72,15,126,199

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[288+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((288+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[$L$ONE_mont]
	pand	xmm3,XMMWORD[(($L$ONE_mont+16))]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[384+rsp]
	pand	xmm3,XMMWORD[((384+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[64+rdi],xmm2
	movdqu	XMMWORD[80+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[224+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((224+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[416+rsp]
	pand	xmm3,XMMWORD[((416+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[320+rsp]
	pand	xmm3,XMMWORD[((320+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[rdi],xmm2
	movdqu	XMMWORD[16+rdi],xmm3

	movdqa	xmm0,xmm5
	movdqa	xmm1,xmm5
	pandn	xmm0,XMMWORD[256+rsp]
	movdqa	xmm2,xmm5
	pandn	xmm1,XMMWORD[((256+16))+rsp]
	movdqa	xmm3,xmm5
	pand	xmm2,XMMWORD[448+rsp]
	pand	xmm3,XMMWORD[((448+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1

	movdqa	xmm0,xmm4
	movdqa	xmm1,xmm4
	pandn	xmm0,xmm2
	movdqa	xmm2,xmm4
	pandn	xmm1,xmm3
	movdqa	xmm3,xmm4
	pand	xmm2,XMMWORD[352+rsp]
	pand	xmm3,XMMWORD[((352+16))+rsp]
	por	xmm2,xmm0
	por	xmm3,xmm1
	movdqu	XMMWORD[32+rdi],xmm2
	movdqu	XMMWORD[48+rdi],xmm3

	lea	rsi,[((480+56))+rsp]

	mov	r15,QWORD[((-48))+rsi]

	mov	r14,QWORD[((-40))+rsi]

	mov	r13,QWORD[((-32))+rsi]

	mov	r12,QWORD[((-24))+rsi]

	mov	rbx,QWORD[((-16))+rsi]

	mov	rbp,QWORD[((-8))+rsi]

	lea	rsp,[rsi]

$L$add_affinex_epilogue:
	mov	rdi,QWORD[8+rsp]	;WIN64 epilogue
	mov	rsi,QWORD[16+rsp]
	ret

$L$SEH_end_ecp_nistz256_point_add_affine_adx:
EXTERN	__imp_RtlVirtualUnwind


ALIGN	16
short_handler:
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

	lea	rax,[16+rax]

	mov	r12,QWORD[((-8))+rax]
	mov	r13,QWORD[((-16))+rax]
	mov	QWORD[216+r8],r12
	mov	QWORD[224+r8],r13

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

	mov	rax,QWORD[152+r8]

	mov	r10d,DWORD[4+r11]
	lea	r10,[r10*1+rsi]
	cmp	rbx,r10
	jae	NEAR $L$common_seh_tail

	mov	r10d,DWORD[8+r11]
	lea	rax,[r10*1+rax]

	mov	rbp,QWORD[((-8))+rax]
	mov	rbx,QWORD[((-16))+rax]
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


section	.pdata rdata align=4
ALIGN	4
	DD	$L$SEH_begin_ecp_nistz256_neg wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_neg wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_neg wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_ord_mul_mont_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_ord_mul_mont_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_ord_mul_mont_nohw wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_ord_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_ord_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_ord_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_ord_mul_mont_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_ord_mul_mont_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_ord_mul_mont_adx wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_ord_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_ord_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_ord_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_mul_mont_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_mul_mont_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_mul_mont_nohw wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_sqr_mont_nohw wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_mul_mont_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_mul_mont_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_mul_mont_adx wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_sqr_mont_adx wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_select_w5_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_select_w5_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_select_wX_nohw wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_select_w7_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_select_w7_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_select_wX_nohw wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_select_w5_avx2 wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_select_w5_avx2 wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_select_wX_avx2 wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_select_w7_avx2 wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_select_w7_avx2 wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_select_wX_avx2 wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_point_double_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_double_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_double_nohw wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_point_add_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_add_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_add_nohw wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_point_add_affine_nohw wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_add_affine_nohw wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_add_affine_nohw wrt ..imagebase
	DD	$L$SEH_begin_ecp_nistz256_point_double_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_double_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_double_adx wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_point_add_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_add_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_add_adx wrt ..imagebase

	DD	$L$SEH_begin_ecp_nistz256_point_add_affine_adx wrt ..imagebase
	DD	$L$SEH_end_ecp_nistz256_point_add_affine_adx wrt ..imagebase
	DD	$L$SEH_info_ecp_nistz256_point_add_affine_adx wrt ..imagebase

section	.xdata rdata align=8
ALIGN	8
$L$SEH_info_ecp_nistz256_neg:
	DB	9,0,0,0
	DD	short_handler wrt ..imagebase
	DD	$L$neg_body wrt ..imagebase,$L$neg_epilogue wrt ..imagebase
$L$SEH_info_ecp_nistz256_ord_mul_mont_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$ord_mul_body wrt ..imagebase,$L$ord_mul_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_ord_sqr_mont_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$ord_sqr_body wrt ..imagebase,$L$ord_sqr_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_ord_mul_mont_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$ord_mulx_body wrt ..imagebase,$L$ord_mulx_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_ord_sqr_mont_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$ord_sqrx_body wrt ..imagebase,$L$ord_sqrx_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_mul_mont_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$mul_body wrt ..imagebase,$L$mul_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_sqr_mont_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$sqr_body wrt ..imagebase,$L$sqr_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_mul_mont_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$mulx_body wrt ..imagebase,$L$mulx_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_sqr_mont_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$sqrx_body wrt ..imagebase,$L$sqrx_epilogue wrt ..imagebase
	DD	48,0
$L$SEH_info_ecp_nistz256_select_wX_nohw:
	DB	0x01,0x33,0x16,0x00
	DB	0x33,0xf8,0x09,0x00
	DB	0x2e,0xe8,0x08,0x00
	DB	0x29,0xd8,0x07,0x00
	DB	0x24,0xc8,0x06,0x00
	DB	0x1f,0xb8,0x05,0x00
	DB	0x1a,0xa8,0x04,0x00
	DB	0x15,0x98,0x03,0x00
	DB	0x10,0x88,0x02,0x00
	DB	0x0c,0x78,0x01,0x00
	DB	0x08,0x68,0x00,0x00
	DB	0x04,0x01,0x15,0x00
ALIGN	8
$L$SEH_info_ecp_nistz256_select_wX_avx2:
	DB	0x01,0x36,0x17,0x0b
	DB	0x36,0xf8,0x09,0x00
	DB	0x31,0xe8,0x08,0x00
	DB	0x2c,0xd8,0x07,0x00
	DB	0x27,0xc8,0x06,0x00
	DB	0x22,0xb8,0x05,0x00
	DB	0x1d,0xa8,0x04,0x00
	DB	0x18,0x98,0x03,0x00
	DB	0x13,0x88,0x02,0x00
	DB	0x0e,0x78,0x01,0x00
	DB	0x09,0x68,0x00,0x00
	DB	0x04,0x01,0x15,0x00
	DB	0x00,0xb3,0x00,0x00
ALIGN	8
$L$SEH_info_ecp_nistz256_point_double_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$point_doubleq_body wrt ..imagebase,$L$point_doubleq_epilogue wrt ..imagebase
	DD	32*5+56,0
$L$SEH_info_ecp_nistz256_point_add_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$point_addq_body wrt ..imagebase,$L$point_addq_epilogue wrt ..imagebase
	DD	32*18+56,0
$L$SEH_info_ecp_nistz256_point_add_affine_nohw:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$add_affineq_body wrt ..imagebase,$L$add_affineq_epilogue wrt ..imagebase
	DD	32*15+56,0
ALIGN	8
$L$SEH_info_ecp_nistz256_point_double_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$point_doublex_body wrt ..imagebase,$L$point_doublex_epilogue wrt ..imagebase
	DD	32*5+56,0
$L$SEH_info_ecp_nistz256_point_add_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$point_addx_body wrt ..imagebase,$L$point_addx_epilogue wrt ..imagebase
	DD	32*18+56,0
$L$SEH_info_ecp_nistz256_point_add_affine_adx:
	DB	9,0,0,0
	DD	full_handler wrt ..imagebase
	DD	$L$add_affinex_body wrt ..imagebase,$L$add_affinex_epilogue wrt ..imagebase
	DD	32*15+56,0
%else
; Work around https://bugzilla.nasm.us/show_bug.cgi?id=3392738
ret
%endif
