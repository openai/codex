#! /usr/bin/env perl
# Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# This module implements support for Armv8 SM3 instructions

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

$prefix="sm3";
# Message expanding:
#	Wj <- P1(W[j-16]^W[j-9]^(W[j-3]<<<15))^(W[j-13]<<<7)^W[j-6]
# Input: s0, s1, s2, s3
#	s0 = w0  | w1  | w2  | w3
#	s1 = w4  | w5  | w6  | w7
#	s2 = w8  | w9  | w10 | w11
#	s3 = w12 | w13 | w14 | w15
# Output: s4
sub msg_exp () {
my $s0 = shift;
my $s1 = shift;
my $s2 = shift;
my $s3 = shift;
my $s4 = shift;
my $vtmp1 = shift;
my $vtmp2 = shift;
$code.=<<___;
	// s4 = w7  | w8  | w9  | w10
	ext     $s4.16b, $s1.16b, $s2.16b, #12
	// vtmp1 = w3  | w4  | w5  | w6
	ext	$vtmp1.16b, $s0.16b, $s1.16b, #12
	// vtmp2 = w10 | w11 | w12 | w13
	ext     $vtmp2.16b, $s2.16b, $s3.16b, #8
	sm3partw1       $s4.4s, $s0.4s, $s3.4s
	sm3partw2       $s4.4s, $vtmp2.4s, $vtmp1.4s
___
}

# A round of compresson function
# Input:
# 	ab - choose instruction among sm3tt1a, sm3tt1b, sm3tt2a, sm3tt2b
# 	vstate0 - vstate1, store digest status(A - H)
# 	vconst0 - vconst1, interleaved used to store Tj <<< j
# 	vtmp - temporary register
# 	vw - for sm3tt1ab, vw = s0 eor s1
# 	s0 - for sm3tt2ab, just be s0
# 	i, choose wj' or wj from vw
sub round () {
my $ab = shift;
my $vstate0 = shift;
my $vstate1 = shift;
my $vconst0 = shift;
my $vconst1 = shift;
my $vtmp = shift;
my $vw = shift;
my $s0 = shift;
my $i = shift;
$code.=<<___;
	sm3ss1  $vtmp.4s, $vstate0.4s, $vconst0.4s, $vstate1.4s
	shl     $vconst1.4s, $vconst0.4s, #1
	sri     $vconst1.4s, $vconst0.4s, #31
	sm3tt1$ab       $vstate0.4s, $vtmp.4s, $vw.4s[$i]
	sm3tt2$ab       $vstate1.4s, $vtmp.4s, $s0.4s[$i]
___
}

sub qround () {
my $ab = shift;
my $vstate0 = shift;
my $vstate1 = shift;
my $vconst0 = shift;
my $vconst1 = shift;
my $vtmp1 = shift;
my $vtmp2 = shift;
my $s0 = shift;
my $s1 = shift;
my $s2 = shift;
my $s3 = shift;
my $s4 = shift;
	if($s4) {
		&msg_exp($s0, $s1, $s2, $s3, $s4, $vtmp1, $vtmp2);
	}
$code.=<<___;
	eor     $vtmp1.16b, $s0.16b, $s1.16b
___
	&round($ab, $vstate0, $vstate1, $vconst0, $vconst1, $vtmp2,
               $vtmp1, $s0, 0);
	&round($ab, $vstate0, $vstate1, $vconst1, $vconst0, $vtmp2,
               $vtmp1, $s0, 1);
	&round($ab, $vstate0, $vstate1, $vconst0, $vconst1, $vtmp2,
               $vtmp1, $s0, 2);
	&round($ab, $vstate0, $vstate1, $vconst1, $vconst0, $vtmp2,
               $vtmp1, $s0, 3);
}

$code=<<___;
#include "arm_arch.h"
.text
___

{{{
my ($pstate,$pdata,$num)=("x0","x1","w2");
my ($state1,$state2)=("v5","v6");
my ($sconst1, $sconst2)=("s16","s17");
my ($vconst1, $vconst2)=("v16","v17");
my ($s0,$s1,$s2,$s3,$s4)=map("v$_",(0..4));
my ($bkstate1,$bkstate2)=("v18","v19");
my ($vconst_tmp1,$vconst_tmp2)=("v20","v21");
my ($vtmp1,$vtmp2)=("v22","v23");
my $constaddr="x8";
# void ossl_hwsm3_block_data_order(SM3_CTX *c, const void *p, size_t num)
$code.=<<___;
.globl	ossl_hwsm3_block_data_order
.type	ossl_hwsm3_block_data_order,%function
.align	5
ossl_hwsm3_block_data_order:
	AARCH64_VALID_CALL_TARGET
	// load state
	ld1     {$state1.4s-$state2.4s}, [$pstate]
	rev64   $state1.4s, $state1.4s
	rev64   $state2.4s, $state2.4s
	ext     $state1.16b, $state1.16b, $state1.16b, #8
	ext     $state2.16b, $state2.16b, $state2.16b, #8
___
if ($flavour =~ /linux64/)
{
$code.=<<___;
	adrp    $constaddr, .Tj
	add     $constaddr, $constaddr, #:lo12:.Tj
___
} else {
$code.=<<___;
	adr     $constaddr, .Tj
___
}
$code.=<<___;
	ldp     $sconst1, $sconst2, [$constaddr]

.Loop:
	// load input
	ld1     {$s0.4s-$s3.4s}, [$pdata], #64
	sub     $num, $num, #1

	mov     $bkstate1.16b, $state1.16b
	mov     $bkstate2.16b, $state2.16b

#ifndef __AARCH64EB__
	rev32   $s0.16b, $s0.16b
	rev32   $s1.16b, $s1.16b
	rev32   $s2.16b, $s2.16b
	rev32   $s3.16b, $s3.16b
#endif

	ext     $vconst_tmp1.16b, $vconst1.16b, $vconst1.16b, #4
___
	&qround("a",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s0,$s1,$s2,$s3,$s4);
	&qround("a",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s1,$s2,$s3,$s4,$s0);
	&qround("a",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s2,$s3,$s4,$s0,$s1);
	&qround("a",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s3,$s4,$s0,$s1,$s2);

$code.=<<___;
	ext     $vconst_tmp1.16b, $vconst2.16b, $vconst2.16b, #4
___

	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s4,$s0,$s1,$s2,$s3);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s0,$s1,$s2,$s3,$s4);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s1,$s2,$s3,$s4,$s0);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s2,$s3,$s4,$s0,$s1);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s3,$s4,$s0,$s1,$s2);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s4,$s0,$s1,$s2,$s3);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s0,$s1,$s2,$s3,$s4);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s1,$s2,$s3,$s4,$s0);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s2,$s3,$s4,$s0,$s1);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s3,$s4);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s4,$s0);
	&qround("b",$state1,$state2,$vconst_tmp1,$vconst_tmp2,$vtmp1,$vtmp2,
                $s0,$s1);

$code.=<<___;
	eor     $state1.16b, $state1.16b, $bkstate1.16b
	eor     $state2.16b, $state2.16b, $bkstate2.16b

	// any remained blocks?
	cbnz    $num, .Loop

	// save state
	rev64   $state1.4s, $state1.4s
	rev64   $state2.4s, $state2.4s
	ext     $state1.16b, $state1.16b, $state1.16b, #8
	ext     $state2.16b, $state2.16b, $state2.16b, #8
	st1     {$state1.4s-$state2.4s}, [$pstate]
	ret
.size	ossl_hwsm3_block_data_order,.-ossl_hwsm3_block_data_order
___

$code.=".rodata\n"  if ($flavour =~ /linux64/);

$code.=<<___;

.type	_${prefix}_consts,%object
.align	3
_${prefix}_consts:
.Tj:
.word	0x79cc4519, 0x9d8a7a87
.size _${prefix}_consts,.-_${prefix}_consts
___

$code.=".previous\n"  if ($flavour =~ /linux64/);

}}}

#########################################
my %sm3partopcode = (
	"sm3partw1"         =>   0xce60C000,
        "sm3partw2"         =>   0xce60C400);

my %sm3ss1opcode = (
	"sm3ss1"            =>   0xce400000);

my %sm3ttopcode = (
	"sm3tt1a"           =>   0xce408000,
	"sm3tt1b"           =>   0xce408400,
	"sm3tt2a"           =>   0xce408800,
	"sm3tt2b"           =>   0xce408C00);

sub unsm3part {
	my ($mnemonic,$arg)=@_;

	$arg=~ m/[qv](\d+)[^,]*,\s*[qv](\d+)[^,]*,\s*[qv](\d+)/o
	&&
	sprintf ".inst\t0x%08x\t//%s %s",
			$sm3partopcode{$mnemonic}|$1|($2<<5)|($3<<16),
			$mnemonic,$arg;
}

sub unsm3ss1 {
	my ($mnemonic,$arg)=@_;

	$arg=~ m/[qv](\d+)[^,]*,\s*[qv](\d+)[^,]*,\s*[qv](\d+)[^,]*,\s*[qv](\d+)/o
	&&
	sprintf ".inst\t0x%08x\t//%s %s",
			$sm3ss1opcode{$mnemonic}|$1|($2<<5)|($3<<16)|($4<<10),
			$mnemonic,$arg;
}

sub unsm3tt {
	my ($mnemonic,$arg)=@_;

	$arg=~ m/[qv](\d+)[^,]*,\s*[qv](\d+)[^,]*,\s*[qv](\d+)[^,]*\[([0-3])\]/o
	&&
	sprintf ".inst\t0x%08x\t//%s %s",
			$sm3ttopcode{$mnemonic}|$1|($2<<5)|($3<<16)|($4<<12),
			$mnemonic,$arg;
}

open SELF,$0;
while(<SELF>) {
        next if (/^#!/);
        last if (!s/^#/\/\// and !/^$/);
        print;
}
close SELF;

foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/ge;

	s/\b(sm3partw[1-2])\s+([qv].*)/unsm3part($1,$2)/ge;
	s/\b(sm3ss1)\s+([qv].*)/unsm3ss1($1,$2)/ge;
	s/\b(sm3tt[1-2][a-b])\s+([qv].*)/unsm3tt($1,$2)/ge;
	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
