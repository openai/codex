#! /usr/bin/env perl
# Author: Min Zhou <zhoumin@loongson.cn>
# Copyright 2023-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

my $code;

# Here is the scalar register layout for LoongArch.
my ($zero,$ra,$tp,$sp,$fp)=map("\$r$_",(0..3,22));
my ($a0,$a1,$a2,$a3,$a4,$a5,$a6,$a7)=map("\$r$_",(4..11));
my ($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$x)=map("\$r$_",(12..21));
my ($s0,$s1,$s2,$s3,$s4,$s5,$s6,$s7,$s8)=map("\$r$_",(23..31));

# The saved floating-point registers in the LP64D ABI.  In LoongArch
# with vector extension, the low 64 bits of a vector register alias with
# the corresponding FPR.  So we must save and restore the corresponding
# FPR if we'll write into a vector register.  The ABI only requires
# saving and restoring the FPR (i.e. 64 bits of the corresponding vector
# register), not the entire vector register.
my ($fs0,$fs1,$fs2,$fs3,$fs4,$fs5,$fs6,$fs7)=map("\$f$_",(24..31));

# Here is the 128-bit vector register layout for LSX extension.
my ($vr0,$vr1,$vr2,$vr3,$vr4,$vr5,$vr6,$vr7,$vr8,$vr9,$vr10,
    $vr11,$vr12,$vr13,$vr14,$vr15,$vr16,$vr17,$vr18,$vr19,
    $vr20,$vr21,$vr22,$vr23,$vr24,$vr25,$vr26,$vr27,$vr28,
    $vr29,$vr30,$vr31)=map("\$vr$_",(0..31));

# Here is the 256-bit vector register layout for LASX extension.
my ($xr0,$xr1,$xr2,$xr3,$xr4,$xr5,$xr6,$xr7,$xr8,$xr9,$xr10,
    $xr11,$xr12,$xr13,$xr14,$xr15,$xr16,$xr17,$xr18,$xr19,
    $xr20,$xr21,$xr22,$xr23,$xr24,$xr25,$xr26,$xr27,$xr28,
    $xr29,$xr30,$xr31)=map("\$xr$_",(0..31));

# $output is the last argument if it looks like a file (it has an extension)
my $output;
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
open STDOUT,">$output";

# Input parameter block
my ($out, $inp, $len, $key, $counter) = ($a0, $a1, $a2, $a3, $a4);

$code .= <<EOF;
#include "loongarch_arch.h"

.text

.extern OPENSSL_loongarch_hwcap_P

.align 6
.Lsigma:
.ascii	"expand 32-byte k"
.Linc8x:
.long	0,1,2,3,4,5,6,7
.Linc4x:
.long	0,1,2,3

.globl	ChaCha20_ctr32
.type	ChaCha20_ctr32 function

.align 6
ChaCha20_ctr32:
	# $a0 = arg #1 (out pointer)
	# $a1 = arg #2 (inp pointer)
	# $a2 = arg #3 (len)
	# $a3 = arg #4 (key array)
	# $a4 = arg #5 (counter array)

	beqz		$len,.Lno_data
	ori			$t3,$zero,64
	la.global	$t0,OPENSSL_loongarch_hwcap_P
	ld.w		$t0,$t0,0

	bleu		$len,$t3,.LChaCha20_1x  # goto 1x when len <= 64

	andi		$t0,$t0,LOONGARCH_HWCAP_LASX | LOONGARCH_HWCAP_LSX
	beqz		$t0,.LChaCha20_1x

	addi.d		$sp,$sp,-64
	fst.d		$fs0,$sp,0
	fst.d		$fs1,$sp,8
	fst.d		$fs2,$sp,16
	fst.d		$fs3,$sp,24
	fst.d		$fs4,$sp,32
	fst.d		$fs5,$sp,40
	fst.d		$fs6,$sp,48
	fst.d		$fs7,$sp,56

	andi		$t1,$t0,LOONGARCH_HWCAP_LASX
	bnez		$t1,.LChaCha20_8x

	b		.LChaCha20_4x

EOF

########################################################################
# Scalar code path that handles all lengths.
{
# Load the initial states in array @x[*] and update directly
my @x = ($t0, $t1, $t2, $t3, $t4, $t5, $t6, $t7,
         $s0, $s1, $s2, $s3, $s4, $s5, $s6, $s7);

sub ROUND {
	my ($a0,$b0,$c0,$d0) = @_;
	my ($a1,$b1,$c1,$d1) = map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
	my ($a2,$b2,$c2,$d2) = map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
	my ($a3,$b3,$c3,$d3) = map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));

$code .= <<EOF;
	add.w		@x[$a0],@x[$a0],@x[$b0]
	xor			@x[$d0],@x[$d0],@x[$a0]
	rotri.w		@x[$d0],@x[$d0],16      # rotate left 16 bits
	add.w		@x[$a1],@x[$a1],@x[$b1]
	xor			@x[$d1],@x[$d1],@x[$a1]
	rotri.w		@x[$d1],@x[$d1],16

	add.w		@x[$c0],@x[$c0],@x[$d0]
	xor			@x[$b0],@x[$b0],@x[$c0]
	rotri.w		@x[$b0],@x[$b0],20      # rotate left 12 bits
	add.w		@x[$c1],@x[$c1],@x[$d1]
	xor			@x[$b1],@x[$b1],@x[$c1]
	rotri.w		@x[$b1],@x[$b1],20

	add.w		@x[$a0],@x[$a0],@x[$b0]
	xor			@x[$d0],@x[$d0],@x[$a0]
	rotri.w		@x[$d0],@x[$d0],24      # rotate left 8 bits
	add.w		@x[$a1],@x[$a1],@x[$b1]
	xor			@x[$d1],@x[$d1],@x[$a1]
	rotri.w		@x[$d1],@x[$d1],24

	add.w		@x[$c0],@x[$c0],@x[$d0]
	xor			@x[$b0],@x[$b0],@x[$c0]
	rotri.w		@x[$b0],@x[$b0],25      # rotate left 7 bits
	add.w		@x[$c1],@x[$c1],@x[$d1]
	xor			@x[$b1],@x[$b1],@x[$c1]
	rotri.w		@x[$b1],@x[$b1],25

	add.w		@x[$a2],@x[$a2],@x[$b2]
	xor			@x[$d2],@x[$d2],@x[$a2]
	rotri.w		@x[$d2],@x[$d2],16
	add.w		@x[$a3],@x[$a3],@x[$b3]
	xor			@x[$d3],@x[$d3],@x[$a3]
	rotri.w		@x[$d3],@x[$d3],16

	add.w		@x[$c2],@x[$c2],@x[$d2]
	xor			@x[$b2],@x[$b2],@x[$c2]
	rotri.w		@x[$b2],@x[$b2],20
	add.w		@x[$c3],@x[$c3],@x[$d3]
	xor			@x[$b3],@x[$b3],@x[$c3]
	rotri.w		@x[$b3],@x[$b3],20

	add.w		@x[$a2],@x[$a2],@x[$b2]
	xor			@x[$d2],@x[$d2],@x[$a2]
	rotri.w		@x[$d2],@x[$d2],24
	add.w		@x[$a3],@x[$a3],@x[$b3]
	xor			@x[$d3],@x[$d3],@x[$a3]
	rotri.w		@x[$d3],@x[$d3],24

	add.w		@x[$c2],@x[$c2],@x[$d2]
	xor			@x[$b2],@x[$b2],@x[$c2]
	rotri.w		@x[$b2],@x[$b2],25
	add.w		@x[$c3],@x[$c3],@x[$d3]
	xor			@x[$b3],@x[$b3],@x[$c3]
	rotri.w		@x[$b3],@x[$b3],25

EOF
}

$code .= <<EOF;
.align 6
.LChaCha20_1x:
	addi.d		$sp,$sp,-256
	st.d		$s0,$sp,0
	st.d		$s1,$sp,8
	st.d		$s2,$sp,16
	st.d		$s3,$sp,24
	st.d		$s4,$sp,32
	st.d		$s5,$sp,40
	st.d		$s6,$sp,48
	st.d		$s7,$sp,56
	st.d		$s8,$sp,64

	# Save the initial block counter in $s8
	ld.w		$s8,$counter,0
	b			.Loop_outer_1x

.align 5
.Loop_outer_1x:
	# Load constants
	la.local	$t8,.Lsigma
	ld.w		@x[0],$t8,0		  # 'expa'
	ld.w		@x[1],$t8,4		  # 'nd 3'
	ld.w		@x[2],$t8,8		  # '2-by'
	ld.w		@x[3],$t8,12	  # 'te k'

	# Load key
	ld.w		@x[4],$key,4*0
	ld.w		@x[5],$key,4*1
	ld.w		@x[6],$key,4*2
	ld.w		@x[7],$key,4*3
	ld.w		@x[8],$key,4*4
	ld.w		@x[9],$key,4*5
	ld.w		@x[10],$key,4*6
	ld.w		@x[11],$key,4*7

	# Load block counter
	move		@x[12],$s8

	# Load nonce
	ld.w		@x[13],$counter,4*1
	ld.w		@x[14],$counter,4*2
	ld.w		@x[15],$counter,4*3

	# Update states in \@x[*] for 20 rounds
	ori			$t8,$zero,10
	b			.Loop_1x

.align 5
.Loop_1x:
EOF

&ROUND (0, 4, 8, 12);
&ROUND (0, 5, 10, 15);

$code .= <<EOF;
	addi.w		$t8,$t8,-1
	bnez		$t8,.Loop_1x

	# Get the final states by adding the initial states
	la.local	$t8,.Lsigma
	ld.w		$a7,$t8,4*0
	ld.w		$a6,$t8,4*1
	ld.w		$a5,$t8,4*2
	add.w		@x[0],@x[0],$a7
	add.w		@x[1],@x[1],$a6
	add.w		@x[2],@x[2],$a5
	ld.w		$a7,$t8,4*3
	add.w		@x[3],@x[3],$a7

	ld.w		$t8,$key,4*0
	ld.w		$a7,$key,4*1
	ld.w		$a6,$key,4*2
	ld.w		$a5,$key,4*3
	add.w		@x[4],@x[4],$t8
	add.w		@x[5],@x[5],$a7
	add.w		@x[6],@x[6],$a6
	add.w		@x[7],@x[7],$a5

	ld.w		$t8,$key,4*4
	ld.w		$a7,$key,4*5
	ld.w		$a6,$key,4*6
	ld.w		$a5,$key,4*7
	add.w		@x[8],@x[8],$t8
	add.w		@x[9],@x[9],$a7
	add.w		@x[10],@x[10],$a6
	add.w		@x[11],@x[11],$a5

	add.w		@x[12],@x[12],$s8

	ld.w		$t8,$counter,4*1
	ld.w		$a7,$counter,4*2
	ld.w		$a6,$counter,4*3
	add.w		@x[13],@x[13],$t8
	add.w		@x[14],@x[14],$a7
	add.w		@x[15],@x[15],$a6

	ori			$t8,$zero,64
	bltu		$len,$t8,.Ltail_1x

	# Get the encrypted message by xor states with plaintext
	ld.w		$t8,$inp,4*0
	ld.w		$a7,$inp,4*1
	ld.w		$a6,$inp,4*2
	ld.w		$a5,$inp,4*3
	xor			$t8,$t8,@x[0]
	xor			$a7,$a7,@x[1]
	xor			$a6,$a6,@x[2]
	xor			$a5,$a5,@x[3]
	st.w		$t8,$out,4*0
	st.w		$a7,$out,4*1
	st.w		$a6,$out,4*2
	st.w		$a5,$out,4*3

	ld.w		$t8,$inp,4*4
	ld.w		$a7,$inp,4*5
	ld.w		$a6,$inp,4*6
	ld.w		$a5,$inp,4*7
	xor			$t8,$t8,@x[4]
	xor			$a7,$a7,@x[5]
	xor			$a6,$a6,@x[6]
	xor			$a5,$a5,@x[7]
	st.w		$t8,$out,4*4
	st.w		$a7,$out,4*5
	st.w		$a6,$out,4*6
	st.w		$a5,$out,4*7

	ld.w		$t8,$inp,4*8
	ld.w		$a7,$inp,4*9
	ld.w		$a6,$inp,4*10
	ld.w		$a5,$inp,4*11
	xor			$t8,$t8,@x[8]
	xor			$a7,$a7,@x[9]
	xor			$a6,$a6,@x[10]
	xor			$a5,$a5,@x[11]
	st.w		$t8,$out,4*8
	st.w		$a7,$out,4*9
	st.w		$a6,$out,4*10
	st.w		$a5,$out,4*11

	ld.w		$t8,$inp,4*12
	ld.w		$a7,$inp,4*13
	ld.w		$a6,$inp,4*14
	ld.w		$a5,$inp,4*15
	xor			$t8,$t8,@x[12]
	xor			$a7,$a7,@x[13]
	xor			$a6,$a6,@x[14]
	xor			$a5,$a5,@x[15]
	st.w		$t8,$out,4*12
	st.w		$a7,$out,4*13
	st.w		$a6,$out,4*14
	st.w		$a5,$out,4*15

	addi.d		$len,$len,-64
	beqz		$len,.Ldone_1x
	addi.d		$inp,$inp,64
	addi.d		$out,$out,64
	addi.w		$s8,$s8,1
	b			.Loop_outer_1x

.align 4
.Ltail_1x:
	# Handle the tail for 1x (1 <= tail_len <= 63)
	addi.d		$a7,$sp,72
	st.w		@x[0],$a7,4*0
	st.w		@x[1],$a7,4*1
	st.w		@x[2],$a7,4*2
	st.w		@x[3],$a7,4*3
	st.w		@x[4],$a7,4*4
	st.w		@x[5],$a7,4*5
	st.w		@x[6],$a7,4*6
	st.w		@x[7],$a7,4*7
	st.w		@x[8],$a7,4*8
	st.w		@x[9],$a7,4*9
	st.w		@x[10],$a7,4*10
	st.w		@x[11],$a7,4*11
	st.w		@x[12],$a7,4*12
	st.w		@x[13],$a7,4*13
	st.w		@x[14],$a7,4*14
	st.w		@x[15],$a7,4*15

	move		$t8,$zero

.Loop_tail_1x:
	# Xor input with states byte by byte
	ldx.bu		$a6,$inp,$t8
	ldx.bu		$a5,$a7,$t8
	xor			$a6,$a6,$a5
	stx.b		$a6,$out,$t8
	addi.w		$t8,$t8,1
	addi.d		$len,$len,-1
	bnez		$len,.Loop_tail_1x
	b			.Ldone_1x

.Ldone_1x:
	ld.d		$s0,$sp,0
	ld.d		$s1,$sp,8
	ld.d		$s2,$sp,16
	ld.d		$s3,$sp,24
	ld.d		$s4,$sp,32
	ld.d		$s5,$sp,40
	ld.d		$s6,$sp,48
	ld.d		$s7,$sp,56
	ld.d		$s8,$sp,64
	addi.d		$sp,$sp,256

	b			.Lend

EOF
}

########################################################################
# 128-bit LSX code path that handles all lengths.
{
# Load the initial states in array @x[*] and update directly.
my @x = ($vr0, $vr1, $vr2, $vr3, $vr4, $vr5, $vr6, $vr7,
         $vr8, $vr9, $vr10, $vr11, $vr12, $vr13, $vr14, $vr15);

# Save the initial states in array @y[*]
my @y = ($vr16, $vr17, $vr18, $vr19, $vr20, $vr21, $vr22, $vr23,
         $vr24, $vr25, $vr26, $vr27, $vr28, $vr29, $vr30, $vr31);

sub ROUND_4x {
	my ($a0,$b0,$c0,$d0) = @_;
	my ($a1,$b1,$c1,$d1) = map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
	my ($a2,$b2,$c2,$d2) = map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
	my ($a3,$b3,$c3,$d3) = map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));

$code .= <<EOF;
	vadd.w		@x[$a0],@x[$a0],@x[$b0]
	vxor.v		@x[$d0],@x[$d0],@x[$a0]
	vrotri.w	@x[$d0],@x[$d0],16      # rotate left 16 bits
	vadd.w		@x[$a1],@x[$a1],@x[$b1]
	vxor.v		@x[$d1],@x[$d1],@x[$a1]
	vrotri.w	@x[$d1],@x[$d1],16

	vadd.w		@x[$c0],@x[$c0],@x[$d0]
	vxor.v		@x[$b0],@x[$b0],@x[$c0]
	vrotri.w	@x[$b0],@x[$b0],20      # rotate left 12 bits
	vadd.w		@x[$c1],@x[$c1],@x[$d1]
	vxor.v		@x[$b1],@x[$b1],@x[$c1]
	vrotri.w	@x[$b1],@x[$b1],20

	vadd.w		@x[$a0],@x[$a0],@x[$b0]
	vxor.v		@x[$d0],@x[$d0],@x[$a0]
	vrotri.w	@x[$d0],@x[$d0],24      # rotate left 8 bits
	vadd.w		@x[$a1],@x[$a1],@x[$b1]
	vxor.v		@x[$d1],@x[$d1],@x[$a1]
	vrotri.w	@x[$d1],@x[$d1],24

	vadd.w		@x[$c0],@x[$c0],@x[$d0]
	vxor.v		@x[$b0],@x[$b0],@x[$c0]
	vrotri.w	@x[$b0],@x[$b0],25      # rotate left 7 bits
	vadd.w		@x[$c1],@x[$c1],@x[$d1]
	vxor.v		@x[$b1],@x[$b1],@x[$c1]
	vrotri.w	@x[$b1],@x[$b1],25

	vadd.w		@x[$a2],@x[$a2],@x[$b2]
	vxor.v		@x[$d2],@x[$d2],@x[$a2]
	vrotri.w	@x[$d2],@x[$d2],16
	vadd.w		@x[$a3],@x[$a3],@x[$b3]
	vxor.v		@x[$d3],@x[$d3],@x[$a3]
	vrotri.w	@x[$d3],@x[$d3],16

	vadd.w		@x[$c2],@x[$c2],@x[$d2]
	vxor.v		@x[$b2],@x[$b2],@x[$c2]
	vrotri.w	@x[$b2],@x[$b2],20
	vadd.w		@x[$c3],@x[$c3],@x[$d3]
	vxor.v		@x[$b3],@x[$b3],@x[$c3]
	vrotri.w	@x[$b3],@x[$b3],20

	vadd.w		@x[$a2],@x[$a2],@x[$b2]
	vxor.v		@x[$d2],@x[$d2],@x[$a2]
	vrotri.w	@x[$d2],@x[$d2],24
	vadd.w		@x[$a3],@x[$a3],@x[$b3]
	vxor.v		@x[$d3],@x[$d3],@x[$a3]
	vrotri.w	@x[$d3],@x[$d3],24

	vadd.w		@x[$c2],@x[$c2],@x[$d2]
	vxor.v		@x[$b2],@x[$b2],@x[$c2]
	vrotri.w	@x[$b2],@x[$b2],25
	vadd.w		@x[$c3],@x[$c3],@x[$d3]
	vxor.v		@x[$b3],@x[$b3],@x[$c3]
	vrotri.w	@x[$b3],@x[$b3],25

EOF
}

$code .= <<EOF;
.align 6
.LChaCha20_4x:
	addi.d		$sp,$sp,-128

	# Save the initial block counter in $t4
	ld.w		$t4,$counter,0
	b			.Loop_outer_4x

.align 5
.Loop_outer_4x:
	# Load constant
	la.local		$t8,.Lsigma
	vldrepl.w		@x[0],$t8,4*0		# 'expa'
	vldrepl.w		@x[1],$t8,4*1		# 'nd 3'
	vldrepl.w		@x[2],$t8,4*2		# '2-by'
	vldrepl.w		@x[3],$t8,4*3		# 'te k'

	# Load key
	vldrepl.w		@x[4],$key,4*0
	vldrepl.w		@x[5],$key,4*1
	vldrepl.w		@x[6],$key,4*2
	vldrepl.w		@x[7],$key,4*3
	vldrepl.w		@x[8],$key,4*4
	vldrepl.w		@x[9],$key,4*5
	vldrepl.w		@x[10],$key,4*6
	vldrepl.w		@x[11],$key,4*7

	# Load block counter
	vreplgr2vr.w	@x[12],$t4

	# Load nonce
	vldrepl.w		@x[13],$counter,4*1
	vldrepl.w		@x[14],$counter,4*2
	vldrepl.w		@x[15],$counter,4*3

	# Get the correct block counter for each block
	la.local		$t8,.Linc4x
	vld				@y[0],$t8,0
	vadd.w			@x[12],@x[12],@y[0]

	# Copy the initial states from \@x[*] to \@y[*]
	vori.b			@y[0],@x[0],0
	vori.b			@y[1],@x[1],0
	vori.b			@y[2],@x[2],0
	vori.b			@y[3],@x[3],0
	vori.b			@y[4],@x[4],0
	vori.b			@y[5],@x[5],0
	vori.b			@y[6],@x[6],0
	vori.b			@y[7],@x[7],0
	vori.b			@y[8],@x[8],0
	vori.b			@y[9],@x[9],0
	vori.b			@y[10],@x[10],0
	vori.b			@y[11],@x[11],0
	vori.b			@y[12],@x[12],0
	vori.b			@y[13],@x[13],0
	vori.b			@y[14],@x[14],0
	vori.b			@y[15],@x[15],0

	# Update states in \@x[*] for 20 rounds
	ori				$t8,$zero,10
	b				.Loop_4x

.align 5
.Loop_4x:
EOF

&ROUND_4x (0, 4, 8, 12);
&ROUND_4x (0, 5, 10, 15);

$code .= <<EOF;
	addi.w		$t8,$t8,-1
	bnez		$t8,.Loop_4x

	# Get the final states by adding the initial states
	vadd.w		@x[0],@x[0],@y[0]
	vadd.w		@x[1],@x[1],@y[1]
	vadd.w		@x[2],@x[2],@y[2]
	vadd.w		@x[3],@x[3],@y[3]
	vadd.w		@x[4],@x[4],@y[4]
	vadd.w		@x[5],@x[5],@y[5]
	vadd.w		@x[6],@x[6],@y[6]
	vadd.w		@x[7],@x[7],@y[7]
	vadd.w		@x[8],@x[8],@y[8]
	vadd.w		@x[9],@x[9],@y[9]
	vadd.w		@x[10],@x[10],@y[10]
	vadd.w		@x[11],@x[11],@y[11]
	vadd.w		@x[12],@x[12],@y[12]
	vadd.w		@x[13],@x[13],@y[13]
	vadd.w		@x[14],@x[14],@y[14]
	vadd.w		@x[15],@x[15],@y[15]

	# Get the transpose of \@x[*] and save them in \@x[*]
	vilvl.w		@y[0],@x[1],@x[0]
	vilvh.w		@y[1],@x[1],@x[0]
	vilvl.w		@y[2],@x[3],@x[2]
	vilvh.w		@y[3],@x[3],@x[2]
	vilvl.w		@y[4],@x[5],@x[4]
	vilvh.w		@y[5],@x[5],@x[4]
	vilvl.w		@y[6],@x[7],@x[6]
	vilvh.w		@y[7],@x[7],@x[6]
	vilvl.w		@y[8],@x[9],@x[8]
	vilvh.w		@y[9],@x[9],@x[8]
	vilvl.w		@y[10],@x[11],@x[10]
	vilvh.w		@y[11],@x[11],@x[10]
	vilvl.w		@y[12],@x[13],@x[12]
	vilvh.w		@y[13],@x[13],@x[12]
	vilvl.w		@y[14],@x[15],@x[14]
	vilvh.w		@y[15],@x[15],@x[14]

	vilvl.d		@x[0],@y[2],@y[0]
	vilvh.d		@x[1],@y[2],@y[0]
	vilvl.d		@x[2],@y[3],@y[1]
	vilvh.d		@x[3],@y[3],@y[1]
	vilvl.d		@x[4],@y[6],@y[4]
	vilvh.d		@x[5],@y[6],@y[4]
	vilvl.d		@x[6],@y[7],@y[5]
	vilvh.d		@x[7],@y[7],@y[5]
	vilvl.d		@x[8],@y[10],@y[8]
	vilvh.d		@x[9],@y[10],@y[8]
	vilvl.d		@x[10],@y[11],@y[9]
	vilvh.d		@x[11],@y[11],@y[9]
	vilvl.d		@x[12],@y[14],@y[12]
	vilvh.d		@x[13],@y[14],@y[12]
	vilvl.d		@x[14],@y[15],@y[13]
	vilvh.d		@x[15],@y[15],@y[13]
EOF

# Adjust the order of elements in @x[*] for ease of use.
@x = (@x[0],@x[4],@x[8],@x[12],@x[1],@x[5],@x[9],@x[13],
      @x[2],@x[6],@x[10],@x[14],@x[3],@x[7],@x[11],@x[15]);

$code .= <<EOF;
	ori			$t8,$zero,64*4
	bltu		$len,$t8,.Ltail_4x

	# Get the encrypted message by xor states with plaintext
	vld			@y[0],$inp,16*0
	vld			@y[1],$inp,16*1
	vld			@y[2],$inp,16*2
	vld			@y[3],$inp,16*3
	vxor.v		@y[0],@y[0],@x[0]
	vxor.v		@y[1],@y[1],@x[1]
	vxor.v		@y[2],@y[2],@x[2]
	vxor.v		@y[3],@y[3],@x[3]
	vst			@y[0],$out,16*0
	vst			@y[1],$out,16*1
	vst			@y[2],$out,16*2
	vst			@y[3],$out,16*3

	vld			@y[0],$inp,16*4
	vld			@y[1],$inp,16*5
	vld			@y[2],$inp,16*6
	vld			@y[3],$inp,16*7
	vxor.v		@y[0],@y[0],@x[4]
	vxor.v		@y[1],@y[1],@x[5]
	vxor.v		@y[2],@y[2],@x[6]
	vxor.v		@y[3],@y[3],@x[7]
	vst			@y[0],$out,16*4
	vst			@y[1],$out,16*5
	vst			@y[2],$out,16*6
	vst			@y[3],$out,16*7

	vld			@y[0],$inp,16*8
	vld			@y[1],$inp,16*9
	vld			@y[2],$inp,16*10
	vld			@y[3],$inp,16*11
	vxor.v		@y[0],@y[0],@x[8]
	vxor.v		@y[1],@y[1],@x[9]
	vxor.v		@y[2],@y[2],@x[10]
	vxor.v		@y[3],@y[3],@x[11]
	vst			@y[0],$out,16*8
	vst			@y[1],$out,16*9
	vst			@y[2],$out,16*10
	vst			@y[3],$out,16*11

	vld			@y[0],$inp,16*12
	vld			@y[1],$inp,16*13
	vld			@y[2],$inp,16*14
	vld			@y[3],$inp,16*15
	vxor.v		@y[0],@y[0],@x[12]
	vxor.v		@y[1],@y[1],@x[13]
	vxor.v		@y[2],@y[2],@x[14]
	vxor.v		@y[3],@y[3],@x[15]
	vst			@y[0],$out,16*12
	vst			@y[1],$out,16*13
	vst			@y[2],$out,16*14
	vst			@y[3],$out,16*15

	addi.d		$len,$len,-64*4
	beqz		$len,.Ldone_4x
	addi.d		$inp,$inp,64*4
	addi.d		$out,$out,64*4
	addi.w		$t4,$t4,4
	b			.Loop_outer_4x

.Ltail_4x:
	# Handle the tail for 4x (1 <= tail_len <= 255)
	ori			$t8,$zero,192
	bgeu		$len,$t8,.L192_or_more4x
	ori			$t8,$zero,128
	bgeu		$len,$t8,.L128_or_more4x
	ori			$t8,$zero,64
	bgeu		$len,$t8,.L64_or_more4x

	vst			@x[0],$sp,16*0
	vst			@x[1],$sp,16*1
	vst			@x[2],$sp,16*2
	vst			@x[3],$sp,16*3
	move		$t8,$zero
	b			.Loop_tail_4x

.align 5
.L64_or_more4x:
	vld			@y[0],$inp,16*0
	vld			@y[1],$inp,16*1
	vld			@y[2],$inp,16*2
	vld			@y[3],$inp,16*3
	vxor.v		@y[0],@y[0],@x[0]
	vxor.v		@y[1],@y[1],@x[1]
	vxor.v		@y[2],@y[2],@x[2]
	vxor.v		@y[3],@y[3],@x[3]
	vst			@y[0],$out,16*0
	vst			@y[1],$out,16*1
	vst			@y[2],$out,16*2
	vst			@y[3],$out,16*3

	addi.d		$len,$len,-64
	beqz		$len,.Ldone_4x
	addi.d		$inp,$inp,64
	addi.d		$out,$out,64
	vst			@x[4],$sp,16*0
	vst			@x[5],$sp,16*1
	vst			@x[6],$sp,16*2
	vst			@x[7],$sp,16*3
	move		$t8,$zero
	b			.Loop_tail_4x

.align 5
.L128_or_more4x:
	vld			@y[0],$inp,16*0
	vld			@y[1],$inp,16*1
	vld			@y[2],$inp,16*2
	vld			@y[3],$inp,16*3
	vxor.v		@y[0],@y[0],@x[0]
	vxor.v		@y[1],@y[1],@x[1]
	vxor.v		@y[2],@y[2],@x[2]
	vxor.v		@y[3],@y[3],@x[3]
	vst			@y[0],$out,16*0
	vst			@y[1],$out,16*1
	vst			@y[2],$out,16*2
	vst			@y[3],$out,16*3

	vld			@y[0],$inp,16*4
	vld			@y[1],$inp,16*5
	vld			@y[2],$inp,16*6
	vld			@y[3],$inp,16*7
	vxor.v		@y[0],@y[0],@x[4]
	vxor.v		@y[1],@y[1],@x[5]
	vxor.v		@y[2],@y[2],@x[6]
	vxor.v		@y[3],@y[3],@x[7]
	vst			@y[0],$out,16*4
	vst			@y[1],$out,16*5
	vst			@y[2],$out,16*6
	vst			@y[3],$out,16*7

	addi.d		$len,$len,-128
	beqz		$len,.Ldone_4x
	addi.d		$inp,$inp,128
	addi.d		$out,$out,128
	vst			@x[8],$sp,16*0
	vst			@x[9],$sp,16*1
	vst			@x[10],$sp,16*2
	vst			@x[11],$sp,16*3
	move		$t8,$zero
	b			.Loop_tail_4x

.align 5
.L192_or_more4x:
	vld			@y[0],$inp,16*0
	vld			@y[1],$inp,16*1
	vld			@y[2],$inp,16*2
	vld			@y[3],$inp,16*3
	vxor.v		@y[0],@y[0],@x[0]
	vxor.v		@y[1],@y[1],@x[1]
	vxor.v		@y[2],@y[2],@x[2]
	vxor.v		@y[3],@y[3],@x[3]
	vst			@y[0],$out,16*0
	vst			@y[1],$out,16*1
	vst			@y[2],$out,16*2
	vst			@y[3],$out,16*3

	vld			@y[0],$inp,16*4
	vld			@y[1],$inp,16*5
	vld			@y[2],$inp,16*6
	vld			@y[3],$inp,16*7
	vxor.v		@y[0],@y[0],@x[4]
	vxor.v		@y[1],@y[1],@x[5]
	vxor.v		@y[2],@y[2],@x[6]
	vxor.v		@y[3],@y[3],@x[7]
	vst			@y[0],$out,16*4
	vst			@y[1],$out,16*5
	vst			@y[2],$out,16*6
	vst			@y[3],$out,16*7

	vld			@y[0],$inp,16*8
	vld			@y[1],$inp,16*9
	vld			@y[2],$inp,16*10
	vld			@y[3],$inp,16*11
	vxor.v		@y[0],@y[0],@x[8]
	vxor.v		@y[1],@y[1],@x[9]
	vxor.v		@y[2],@y[2],@x[10]
	vxor.v		@y[3],@y[3],@x[11]
	vst			@y[0],$out,16*8
	vst			@y[1],$out,16*9
	vst			@y[2],$out,16*10
	vst			@y[3],$out,16*11

	addi.d		$len,$len,-192
	beqz		$len,.Ldone_4x
	addi.d		$inp,$inp,192
	addi.d		$out,$out,192
	vst			@x[12],$sp,16*0
	vst			@x[13],$sp,16*1
	vst			@x[14],$sp,16*2
	vst			@x[15],$sp,16*3
	move		$t8,$zero
	b			.Loop_tail_4x

.Loop_tail_4x:
	# Xor input with states byte by byte
	ldx.bu		$t5,$inp,$t8
	ldx.bu		$t6,$sp,$t8
	xor			$t5,$t5,$t6
	stx.b		$t5,$out,$t8
	addi.w		$t8,$t8,1
	addi.d		$len,$len,-1
	bnez		$len,.Loop_tail_4x
	b			.Ldone_4x

.Ldone_4x:
	addi.d		$sp,$sp,128
	b			.Lrestore_saved_fpr

EOF
}

########################################################################
# 256-bit LASX code path that handles all lengths.
{
# Load the initial states in array @x[*] and update directly.
my @x = ($xr0, $xr1, $xr2, $xr3, $xr4, $xr5, $xr6, $xr7,
         $xr8, $xr9, $xr10, $xr11, $xr12, $xr13, $xr14, $xr15);

# Save the initial states in array @y[*]
my @y = ($xr16, $xr17, $xr18, $xr19, $xr20, $xr21, $xr22, $xr23,
         $xr24, $xr25, $xr26, $xr27, $xr28, $xr29, $xr30, $xr31);

sub ROUND_8x {
	my ($a0,$b0,$c0,$d0) = @_;
	my ($a1,$b1,$c1,$d1) = map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
	my ($a2,$b2,$c2,$d2) = map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
	my ($a3,$b3,$c3,$d3) = map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));

$code .= <<EOF;
	xvadd.w		@x[$a0],@x[$a0],@x[$b0]
	xvxor.v		@x[$d0],@x[$d0],@x[$a0]
	xvrotri.w	@x[$d0],@x[$d0],16      # rotate left 16 bits
	xvadd.w		@x[$a1],@x[$a1],@x[$b1]
	xvxor.v		@x[$d1],@x[$d1],@x[$a1]
	xvrotri.w	@x[$d1],@x[$d1],16

	xvadd.w		@x[$c0],@x[$c0],@x[$d0]
	xvxor.v		@x[$b0],@x[$b0],@x[$c0]
	xvrotri.w	@x[$b0],@x[$b0],20      # rotate left 12 bits
	xvadd.w		@x[$c1],@x[$c1],@x[$d1]
	xvxor.v		@x[$b1],@x[$b1],@x[$c1]
	xvrotri.w	@x[$b1],@x[$b1],20

	xvadd.w		@x[$a0],@x[$a0],@x[$b0]
	xvxor.v		@x[$d0],@x[$d0],@x[$a0]
	xvrotri.w	@x[$d0],@x[$d0],24      # rotate left 8 bits
	xvadd.w		@x[$a1],@x[$a1],@x[$b1]
	xvxor.v		@x[$d1],@x[$d1],@x[$a1]
	xvrotri.w	@x[$d1],@x[$d1],24

	xvadd.w		@x[$c0],@x[$c0],@x[$d0]
	xvxor.v		@x[$b0],@x[$b0],@x[$c0]
	xvrotri.w	@x[$b0],@x[$b0],25      # rotate left 7 bits
	xvadd.w		@x[$c1],@x[$c1],@x[$d1]
	xvxor.v		@x[$b1],@x[$b1],@x[$c1]
	xvrotri.w	@x[$b1],@x[$b1],25

	xvadd.w		@x[$a2],@x[$a2],@x[$b2]
	xvxor.v		@x[$d2],@x[$d2],@x[$a2]
	xvrotri.w	@x[$d2],@x[$d2],16
	xvadd.w		@x[$a3],@x[$a3],@x[$b3]
	xvxor.v		@x[$d3],@x[$d3],@x[$a3]
	xvrotri.w	@x[$d3],@x[$d3],16

	xvadd.w		@x[$c2],@x[$c2],@x[$d2]
	xvxor.v		@x[$b2],@x[$b2],@x[$c2]
	xvrotri.w	@x[$b2],@x[$b2],20
	xvadd.w		@x[$c3],@x[$c3],@x[$d3]
	xvxor.v		@x[$b3],@x[$b3],@x[$c3]
	xvrotri.w	@x[$b3],@x[$b3],20

	xvadd.w		@x[$a2],@x[$a2],@x[$b2]
	xvxor.v		@x[$d2],@x[$d2],@x[$a2]
	xvrotri.w	@x[$d2],@x[$d2],24
	xvadd.w		@x[$a3],@x[$a3],@x[$b3]
	xvxor.v		@x[$d3],@x[$d3],@x[$a3]
	xvrotri.w	@x[$d3],@x[$d3],24

	xvadd.w		@x[$c2],@x[$c2],@x[$d2]
	xvxor.v		@x[$b2],@x[$b2],@x[$c2]
	xvrotri.w	@x[$b2],@x[$b2],25
	xvadd.w		@x[$c3],@x[$c3],@x[$d3]
	xvxor.v		@x[$b3],@x[$b3],@x[$c3]
	xvrotri.w	@x[$b3],@x[$b3],25

EOF
}

$code .= <<EOF;
.align 6
.LChaCha20_8x:
	addi.d		$sp,$sp,-128

	# Save the initial block counter in $t4
	ld.w		$t4,$counter,0
	b			.Loop_outer_8x

.align 5
.Loop_outer_8x:
	# Load constant
	la.local		$t8,.Lsigma
	xvldrepl.w		@x[0],$t8,4*0		# 'expa'
	xvldrepl.w		@x[1],$t8,4*1		# 'nd 3'
	xvldrepl.w		@x[2],$t8,4*2		# '2-by'
	xvldrepl.w		@x[3],$t8,4*3		# 'te k'

	# Load key
	xvldrepl.w		@x[4],$key,4*0
	xvldrepl.w		@x[5],$key,4*1
	xvldrepl.w		@x[6],$key,4*2
	xvldrepl.w		@x[7],$key,4*3
	xvldrepl.w		@x[8],$key,4*4
	xvldrepl.w		@x[9],$key,4*5
	xvldrepl.w		@x[10],$key,4*6
	xvldrepl.w		@x[11],$key,4*7

	# Load block counter
	xvreplgr2vr.w	@x[12],$t4

	# Load nonce
	xvldrepl.w		@x[13],$counter,4*1
	xvldrepl.w		@x[14],$counter,4*2
	xvldrepl.w		@x[15],$counter,4*3

	# Get the correct block counter for each block
	la.local		$t8,.Linc8x
	xvld			@y[0],$t8,0
	xvadd.w			@x[12],@x[12],@y[0]

	# Copy the initial states from \@x[*] to \@y[*]
	xvori.b			@y[0],@x[0],0
	xvori.b			@y[1],@x[1],0
	xvori.b			@y[2],@x[2],0
	xvori.b			@y[3],@x[3],0
	xvori.b			@y[4],@x[4],0
	xvori.b			@y[5],@x[5],0
	xvori.b			@y[6],@x[6],0
	xvori.b			@y[7],@x[7],0
	xvori.b			@y[8],@x[8],0
	xvori.b			@y[9],@x[9],0
	xvori.b			@y[10],@x[10],0
	xvori.b			@y[11],@x[11],0
	xvori.b			@y[12],@x[12],0
	xvori.b			@y[13],@x[13],0
	xvori.b			@y[14],@x[14],0
	xvori.b			@y[15],@x[15],0

	# Update states in \@x[*] for 20 rounds
	ori				$t8,$zero,10
	b				.Loop_8x

.align 5
.Loop_8x:
EOF

&ROUND_8x (0, 4, 8, 12);
&ROUND_8x (0, 5, 10, 15);

$code .= <<EOF;
	addi.w		$t8,$t8,-1
	bnez		$t8,.Loop_8x

	# Get the final states by adding the initial states
	xvadd.w		@x[0],@x[0],@y[0]
	xvadd.w		@x[1],@x[1],@y[1]
	xvadd.w		@x[2],@x[2],@y[2]
	xvadd.w		@x[3],@x[3],@y[3]
	xvadd.w		@x[4],@x[4],@y[4]
	xvadd.w		@x[5],@x[5],@y[5]
	xvadd.w		@x[6],@x[6],@y[6]
	xvadd.w		@x[7],@x[7],@y[7]
	xvadd.w		@x[8],@x[8],@y[8]
	xvadd.w		@x[9],@x[9],@y[9]
	xvadd.w		@x[10],@x[10],@y[10]
	xvadd.w		@x[11],@x[11],@y[11]
	xvadd.w		@x[12],@x[12],@y[12]
	xvadd.w		@x[13],@x[13],@y[13]
	xvadd.w		@x[14],@x[14],@y[14]
	xvadd.w		@x[15],@x[15],@y[15]

	# Get the transpose of \@x[*] and save them in \@y[*]
	xvilvl.w	@y[0],@x[1],@x[0]
	xvilvh.w	@y[1],@x[1],@x[0]
	xvilvl.w	@y[2],@x[3],@x[2]
	xvilvh.w	@y[3],@x[3],@x[2]
	xvilvl.w	@y[4],@x[5],@x[4]
	xvilvh.w	@y[5],@x[5],@x[4]
	xvilvl.w	@y[6],@x[7],@x[6]
	xvilvh.w	@y[7],@x[7],@x[6]
	xvilvl.w	@y[8],@x[9],@x[8]
	xvilvh.w	@y[9],@x[9],@x[8]
	xvilvl.w	@y[10],@x[11],@x[10]
	xvilvh.w	@y[11],@x[11],@x[10]
	xvilvl.w	@y[12],@x[13],@x[12]
	xvilvh.w	@y[13],@x[13],@x[12]
	xvilvl.w	@y[14],@x[15],@x[14]
	xvilvh.w	@y[15],@x[15],@x[14]

	xvilvl.d	@x[0],@y[2],@y[0]
	xvilvh.d	@x[1],@y[2],@y[0]
	xvilvl.d	@x[2],@y[3],@y[1]
	xvilvh.d	@x[3],@y[3],@y[1]
	xvilvl.d	@x[4],@y[6],@y[4]
	xvilvh.d	@x[5],@y[6],@y[4]
	xvilvl.d	@x[6],@y[7],@y[5]
	xvilvh.d	@x[7],@y[7],@y[5]
	xvilvl.d	@x[8],@y[10],@y[8]
	xvilvh.d	@x[9],@y[10],@y[8]
	xvilvl.d	@x[10],@y[11],@y[9]
	xvilvh.d	@x[11],@y[11],@y[9]
	xvilvl.d	@x[12],@y[14],@y[12]
	xvilvh.d	@x[13],@y[14],@y[12]
	xvilvl.d	@x[14],@y[15],@y[13]
	xvilvh.d	@x[15],@y[15],@y[13]

	xvori.b		@y[0],@x[4],0
	xvpermi.q	@y[0],@x[0],0x20
	xvori.b		@y[1],@x[5],0
	xvpermi.q	@y[1],@x[1],0x20
	xvori.b		@y[2],@x[6],0
	xvpermi.q	@y[2],@x[2],0x20
	xvori.b		@y[3],@x[7],0
	xvpermi.q	@y[3],@x[3],0x20
	xvori.b		@y[4],@x[4],0
	xvpermi.q	@y[4],@x[0],0x31
	xvori.b		@y[5],@x[5],0
	xvpermi.q	@y[5],@x[1],0x31
	xvori.b		@y[6],@x[6],0
	xvpermi.q	@y[6],@x[2],0x31
	xvori.b		@y[7],@x[7],0
	xvpermi.q	@y[7],@x[3],0x31
	xvori.b		@y[8],@x[12],0
	xvpermi.q	@y[8],@x[8],0x20
	xvori.b		@y[9],@x[13],0
	xvpermi.q	@y[9],@x[9],0x20
	xvori.b		@y[10],@x[14],0
	xvpermi.q	@y[10],@x[10],0x20
	xvori.b		@y[11],@x[15],0
	xvpermi.q	@y[11],@x[11],0x20
	xvori.b		@y[12],@x[12],0
	xvpermi.q	@y[12],@x[8],0x31
	xvori.b		@y[13],@x[13],0
	xvpermi.q	@y[13],@x[9],0x31
	xvori.b		@y[14],@x[14],0
	xvpermi.q	@y[14],@x[10],0x31
	xvori.b		@y[15],@x[15],0
	xvpermi.q	@y[15],@x[11],0x31

EOF

# Adjust the order of elements in @y[*] for ease of use.
@y = (@y[0],@y[8],@y[1],@y[9],@y[2],@y[10],@y[3],@y[11],
      @y[4],@y[12],@y[5],@y[13],@y[6],@y[14],@y[7],@y[15]);

$code .= <<EOF;
	ori			$t8,$zero,64*8
	bltu		$len,$t8,.Ltail_8x

	# Get the encrypted message by xor states with plaintext
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvld		@x[2],$inp,32*6
	xvld		@x[3],$inp,32*7
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvxor.v		@x[2],@x[2],@y[6]
	xvxor.v		@x[3],@x[3],@y[7]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5
	xvst		@x[2],$out,32*6
	xvst		@x[3],$out,32*7

	xvld		@x[0],$inp,32*8
	xvld		@x[1],$inp,32*9
	xvld		@x[2],$inp,32*10
	xvld		@x[3],$inp,32*11
	xvxor.v		@x[0],@x[0],@y[8]
	xvxor.v		@x[1],@x[1],@y[9]
	xvxor.v		@x[2],@x[2],@y[10]
	xvxor.v		@x[3],@x[3],@y[11]
	xvst		@x[0],$out,32*8
	xvst		@x[1],$out,32*9
	xvst		@x[2],$out,32*10
	xvst		@x[3],$out,32*11

	xvld		@x[0],$inp,32*12
	xvld		@x[1],$inp,32*13
	xvld		@x[2],$inp,32*14
	xvld		@x[3],$inp,32*15
	xvxor.v		@x[0],@x[0],@y[12]
	xvxor.v		@x[1],@x[1],@y[13]
	xvxor.v		@x[2],@x[2],@y[14]
	xvxor.v		@x[3],@x[3],@y[15]
	xvst		@x[0],$out,32*12
	xvst		@x[1],$out,32*13
	xvst		@x[2],$out,32*14
	xvst		@x[3],$out,32*15

	addi.d		$len,$len,-64*8
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,64*8
	addi.d		$out,$out,64*8
	addi.w		$t4,$t4,8
	b			.Loop_outer_8x

.Ltail_8x:
	# Handle the tail for 8x (1 <= tail_len <= 511)
	ori			$t8,$zero,448
	bgeu		$len,$t8,.L448_or_more8x
	ori			$t8,$zero,384
	bgeu		$len,$t8,.L384_or_more8x
	ori			$t8,$zero,320
	bgeu		$len,$t8,.L320_or_more8x
	ori			$t8,$zero,256
	bgeu		$len,$t8,.L256_or_more8x
	ori			$t8,$zero,192
	bgeu		$len,$t8,.L192_or_more8x
	ori			$t8,$zero,128
	bgeu		$len,$t8,.L128_or_more8x
	ori			$t8,$zero,64
	bgeu		$len,$t8,.L64_or_more8x

	xvst		@y[0],$sp,32*0
	xvst		@y[1],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L64_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1

	addi.d		$len,$len,-64
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,64
	addi.d		$out,$out,64
	xvst		@y[2],$sp,32*0
	xvst		@y[3],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L128_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	addi.d		$len,$len,-128
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,128
	addi.d		$out,$out,128
	xvst		@y[4],$sp,32*0
	xvst		@y[5],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L192_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5

	addi.d		$len,$len,-192
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,192
	addi.d		$out,$out,192
	xvst		@y[6],$sp,32*0
	xvst		@y[7],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L256_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvld		@x[2],$inp,32*6
	xvld		@x[3],$inp,32*7
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvxor.v		@x[2],@x[2],@y[6]
	xvxor.v		@x[3],@x[3],@y[7]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5
	xvst		@x[2],$out,32*6
	xvst		@x[3],$out,32*7

	addi.d		$len,$len,-256
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,256
	addi.d		$out,$out,256
	xvst		@y[8],$sp,32*0
	xvst		@y[9],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L320_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvld		@x[2],$inp,32*6
	xvld		@x[3],$inp,32*7
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvxor.v		@x[2],@x[2],@y[6]
	xvxor.v		@x[3],@x[3],@y[7]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5
	xvst		@x[2],$out,32*6
	xvst		@x[3],$out,32*7

	xvld		@x[0],$inp,32*8
	xvld		@x[1],$inp,32*9
	xvxor.v		@x[0],@x[0],@y[8]
	xvxor.v		@x[1],@x[1],@y[9]
	xvst		@x[0],$out,32*8
	xvst		@x[1],$out,32*9

	addi.d		$len,$len,-320
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,320
	addi.d		$out,$out,320
	xvst		@y[10],$sp,32*0
	xvst		@y[11],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L384_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvld		@x[2],$inp,32*6
	xvld		@x[3],$inp,32*7
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvxor.v		@x[2],@x[2],@y[6]
	xvxor.v		@x[3],@x[3],@y[7]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5
	xvst		@x[2],$out,32*6
	xvst		@x[3],$out,32*7

	xvld		@x[0],$inp,32*8
	xvld		@x[1],$inp,32*9
	xvld		@x[2],$inp,32*10
	xvld		@x[3],$inp,32*11
	xvxor.v		@x[0],@x[0],@y[8]
	xvxor.v		@x[1],@x[1],@y[9]
	xvxor.v		@x[2],@x[2],@y[10]
	xvxor.v		@x[3],@x[3],@y[11]
	xvst		@x[0],$out,32*8
	xvst		@x[1],$out,32*9
	xvst		@x[2],$out,32*10
	xvst		@x[3],$out,32*11

	addi.d		$len,$len,-384
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,384
	addi.d		$out,$out,384
	xvst		@y[12],$sp,32*0
	xvst		@y[13],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.align 5
.L448_or_more8x:
	xvld		@x[0],$inp,32*0
	xvld		@x[1],$inp,32*1
	xvld		@x[2],$inp,32*2
	xvld		@x[3],$inp,32*3
	xvxor.v		@x[0],@x[0],@y[0]
	xvxor.v		@x[1],@x[1],@y[1]
	xvxor.v		@x[2],@x[2],@y[2]
	xvxor.v		@x[3],@x[3],@y[3]
	xvst		@x[0],$out,32*0
	xvst		@x[1],$out,32*1
	xvst		@x[2],$out,32*2
	xvst		@x[3],$out,32*3

	xvld		@x[0],$inp,32*4
	xvld		@x[1],$inp,32*5
	xvld		@x[2],$inp,32*6
	xvld		@x[3],$inp,32*7
	xvxor.v		@x[0],@x[0],@y[4]
	xvxor.v		@x[1],@x[1],@y[5]
	xvxor.v		@x[2],@x[2],@y[6]
	xvxor.v		@x[3],@x[3],@y[7]
	xvst		@x[0],$out,32*4
	xvst		@x[1],$out,32*5
	xvst		@x[2],$out,32*6
	xvst		@x[3],$out,32*7

	xvld		@x[0],$inp,32*8
	xvld		@x[1],$inp,32*9
	xvld		@x[2],$inp,32*10
	xvld		@x[3],$inp,32*11
	xvxor.v		@x[0],@x[0],@y[8]
	xvxor.v		@x[1],@x[1],@y[9]
	xvxor.v		@x[2],@x[2],@y[10]
	xvxor.v		@x[3],@x[3],@y[11]
	xvst		@x[0],$out,32*8
	xvst		@x[1],$out,32*9
	xvst		@x[2],$out,32*10
	xvst		@x[3],$out,32*11

	xvld		@x[0],$inp,32*12
	xvld		@x[1],$inp,32*13
	xvxor.v		@x[0],@x[0],@y[12]
	xvxor.v		@x[1],@x[1],@y[13]
	xvst		@x[0],$out,32*12
	xvst		@x[1],$out,32*13

	addi.d		$len,$len,-448
	beqz		$len,.Ldone_8x
	addi.d		$inp,$inp,448
	addi.d		$out,$out,448
	xvst		@y[14],$sp,32*0
	xvst		@y[15],$sp,32*1
	move		$t8,$zero
	b			.Loop_tail_8x

.Loop_tail_8x:
	# Xor input with states byte by byte
	ldx.bu		$t5,$inp,$t8
	ldx.bu		$t6,$sp,$t8
	xor			$t5,$t5,$t6
	stx.b		$t5,$out,$t8
	addi.w		$t8,$t8,1
	addi.d		$len,$len,-1
	bnez		$len,.Loop_tail_8x
	b			.Ldone_8x

.Ldone_8x:
	addi.d		$sp,$sp,128
	b			.Lrestore_saved_fpr

EOF
}

$code .= <<EOF;
.Lrestore_saved_fpr:
	fld.d		$fs0,$sp,0
	fld.d		$fs1,$sp,8
	fld.d		$fs2,$sp,16
	fld.d		$fs3,$sp,24
	fld.d		$fs4,$sp,32
	fld.d		$fs5,$sp,40
	fld.d		$fs6,$sp,48
	fld.d		$fs7,$sp,56
	addi.d		$sp,$sp,64
.Lno_data:
.Lend:
	jr	$ra
.size ChaCha20_ctr32,.-ChaCha20_ctr32
EOF

$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT;
