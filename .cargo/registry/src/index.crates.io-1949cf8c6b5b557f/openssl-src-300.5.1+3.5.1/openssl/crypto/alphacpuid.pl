#! /usr/bin/env perl
# Copyright 2010-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html


$output = pop and open STDOUT,">$output";

print <<'___';
.text

.set	noat

.globl	OPENSSL_cpuid_setup
.ent	OPENSSL_cpuid_setup
OPENSSL_cpuid_setup:
	.frame	$30,0,$26
	.prologue 0
	ret	($26)
.end	OPENSSL_cpuid_setup

.globl	OPENSSL_wipe_cpu
.ent	OPENSSL_wipe_cpu
OPENSSL_wipe_cpu:
	.frame	$30,0,$26
	.prologue 0
	clr	$1
	clr	$2
	clr	$3
	clr	$4
	clr	$5
	clr	$6
	clr	$7
	clr	$8
	clr	$16
	clr	$17
	clr	$18
	clr	$19
	clr	$20
	clr	$21
	clr	$22
	clr	$23
	clr	$24
	clr	$25
	clr	$27
	clr	$at
	clr	$29
	fclr	$f0
	fclr	$f1
	fclr	$f10
	fclr	$f11
	fclr	$f12
	fclr	$f13
	fclr	$f14
	fclr	$f15
	fclr	$f16
	fclr	$f17
	fclr	$f18
	fclr	$f19
	fclr	$f20
	fclr	$f21
	fclr	$f22
	fclr	$f23
	fclr	$f24
	fclr	$f25
	fclr	$f26
	fclr	$f27
	fclr	$f28
	fclr	$f29
	fclr	$f30
	mov	$sp,$0
	ret	($26)
.end	OPENSSL_wipe_cpu

.globl	OPENSSL_atomic_add
.ent	OPENSSL_atomic_add
OPENSSL_atomic_add:
	.frame	$30,0,$26
	.prologue 0
1:	ldl_l	$0,0($16)
	addl	$0,$17,$1
	stl_c	$1,0($16)
	beq	$1,1b
	addl	$0,$17,$0
	ret	($26)
.end	OPENSSL_atomic_add

.globl	OPENSSL_rdtsc
.ent	OPENSSL_rdtsc
OPENSSL_rdtsc:
	.frame	$30,0,$26
	.prologue 0
	rpcc	$0
	ret	($26)
.end	OPENSSL_rdtsc

.globl	OPENSSL_cleanse
.ent	OPENSSL_cleanse
OPENSSL_cleanse:
	.frame	$30,0,$26
	.prologue 0
	beq	$17,.Ldone
	and	$16,7,$0
	bic	$17,7,$at
	beq	$at,.Little
	beq	$0,.Laligned

.Little:
	subq	$0,8,$0
	ldq_u	$1,0($16)
	mov	$16,$2
.Lalign:
	mskbl	$1,$16,$1
	lda	$16,1($16)
	subq	$17,1,$17
	addq	$0,1,$0
	beq	$17,.Lout
	bne	$0,.Lalign
.Lout:	stq_u	$1,0($2)
	beq	$17,.Ldone
	bic	$17,7,$at
	beq	$at,.Little

.Laligned:
	stq	$31,0($16)
	subq	$17,8,$17
	lda	$16,8($16)
	bic	$17,7,$at
	bne	$at,.Laligned
	bne	$17,.Little
.Ldone: ret	($26)
.end	OPENSSL_cleanse

.globl	CRYPTO_memcmp
.ent	CRYPTO_memcmp
CRYPTO_memcmp:
	.frame	$30,0,$26
	.prologue 0
	xor	$0,$0,$0
	beq	$18,.Lno_data

	xor	$1,$1,$1
	nop
.Loop_cmp:
	ldq_u	$2,0($16)
	subq	$18,1,$18
	ldq_u	$3,0($17)
	extbl	$2,$16,$2
	lda	$16,1($16)
	extbl	$3,$17,$3
	lda	$17,1($17)
	xor	$3,$2,$2
	or	$2,$0,$0
	bne	$18,.Loop_cmp

	subq	$31,$0,$0
	srl	$0,63,$0
.Lno_data:
	ret	($26)
.end	CRYPTO_memcmp
___
{
my ($out,$cnt,$max)=("\$16","\$17","\$18");
my ($tick,$lasttick)=("\$19","\$20");
my ($diff,$lastdiff)=("\$21","\$22");
my ($v0,$ra,$sp,$zero)=("\$0","\$26","\$30","\$31");

print <<___;
.globl	OPENSSL_instrument_bus
.ent	OPENSSL_instrument_bus
OPENSSL_instrument_bus:
	.frame	$sp,0,$ra
	.prologue 0
	mov	$cnt,$v0

	rpcc	$lasttick
	mov	0,$diff

	ecb	($out)
	ldl_l	$tick,0($out)
	addl	$diff,$tick,$tick
	mov	$tick,$diff
	stl_c	$tick,0($out)
	stl	$diff,0($out)

.Loop:	rpcc	$tick
	subq	$tick,$lasttick,$diff
	mov	$tick,$lasttick

	ecb	($out)
	ldl_l	$tick,0($out)
	addl	$diff,$tick,$tick
	mov	$tick,$diff
	stl_c	$tick,0($out)
	stl	$diff,0($out)

	subl	$cnt,1,$cnt
	lda	$out,4($out)
	bne	$cnt,.Loop

	ret	($ra)
.end	OPENSSL_instrument_bus

.globl	OPENSSL_instrument_bus2
.ent	OPENSSL_instrument_bus2
OPENSSL_instrument_bus2:
	.frame	$sp,0,$ra
	.prologue 0
	mov	$cnt,$v0

	rpcc	$lasttick
	mov	0,$diff

	ecb	($out)
	ldl_l	$tick,0($out)
	addl	$diff,$tick,$tick
	mov	$tick,$diff
	stl_c	$tick,0($out)
	stl	$diff,0($out)

	rpcc	$tick
	subq	$tick,$lasttick,$diff
	mov	$tick,$lasttick
	mov	$diff,$lastdiff
.Loop2:
	ecb	($out)
	ldl_l	$tick,0($out)
	addl	$diff,$tick,$tick
	mov	$tick,$diff
	stl_c	$tick,0($out)
	stl	$diff,0($out)

	subl	$max,1,$max
	beq	$max,.Ldone2

	rpcc	$tick
	subq	$tick,$lasttick,$diff
	mov	$tick,$lasttick
	subq	$lastdiff,$diff,$tick
	mov	$diff,$lastdiff
	cmovne	$tick,1,$tick
	subl	$cnt,$tick,$cnt
	s4addq	$tick,$out,$out
	bne	$cnt,.Loop2

.Ldone2:
	subl	$v0,$cnt,$v0
	ret	($ra)
.end	OPENSSL_instrument_bus2
___
}

close STDOUT or die "error closing STDOUT: $!";
