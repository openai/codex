#! /usr/bin/env perl
# Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
# ====================================================================
# Written by Amitay Isaacs <amitay@ozlabs.org> and Martin Schwenke
# <martin@meltin.net> for the OpenSSL project.
# ====================================================================
#
# p521 lower-level primitives for PPC64 using vector instructions.
#

use strict;
use warnings;

my $flavour = shift;
my $output = "";
while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {}
if (!$output) {
	$output = "-";
}

my ($xlate, $dir);
$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}ppc-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/ppc-xlate.pl" and -f $xlate) or
die "can't locate ppc-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

my $code = "";

my ($sp, $outp, $savelr, $savesp) = ("r1", "r3", "r10", "r12");

my $vzero = "v32";

sub startproc($)
{
    my ($name) = @_;

    $code.=<<___;
    .globl ${name}
    .align 5
${name}:

___
}

sub endproc($)
{
    my ($name) = @_;

    $code.=<<___;
	blr
	    .size	${name},.-${name}

___
}


sub push_vrs($$)
{
	my ($min, $max) = @_;

	my $count = $max - $min + 1;

	$code.=<<___;
	mr		$savesp,$sp
	stdu		$sp,-16*`$count+1`($sp)

___
	    for (my $i = $min; $i <= $max; $i++) {
		    my $mult = $max - $i + 1;
		    $code.=<<___;
	stxv		$i,-16*$mult($savesp)
___

	}

	$code.=<<___;

___
}

sub pop_vrs($$)
{
	my ($min, $max) = @_;

	$code.=<<___;
	ld		$savesp,0($sp)
___
	for (my $i = $min; $i <= $max; $i++) {
		my $mult = $max - $i + 1;
		$code.=<<___;
	lxv		$i,-16*$mult($savesp)
___
	}

	$code.=<<___;
	mr		$sp,$savesp

___
}

sub load_vrs($$)
{
	my ($pointer, $reg_list) = @_;

	for (my $i = 0; $i <= 8; $i++) {
		my $offset = $i * 8;
		$code.=<<___;
	lxsd		$reg_list->[$i],$offset($pointer)
___
	}

	$code.=<<___;

___
}

sub store_vrs($$)
{
	my ($pointer, $reg_list) = @_;

	for (my $i = 0; $i <= 8; $i++) {
		my $offset = $i * 16;
		$code.=<<___;
	stxv		$reg_list->[$i],$offset($pointer)
___
	}

	$code.=<<___;

___
}

$code.=<<___;
.machine	"any"
.text

___

{
	# mul/square common
	my ($t1, $t2, $t3, $t4) = ("v33", "v34", "v44", "v54");
	my ($zero, $one) = ("r8", "r9");
	my @out = map("v$_",(55..63));

	{
		#
		# p521_felem_mul
		#

		my ($in1p, $in2p) = ("r4", "r5");
		my @in1 = map("v$_",(45..53));
		my @in2 = map("v$_",(35..43));

		startproc("p521_felem_mul");

		push_vrs(52, 63);

		$code.=<<___;
	vspltisw	$vzero,0

___

		load_vrs($in1p, \@in1);
		load_vrs($in2p, \@in2);

		$code.=<<___;
	vmsumudm	$out[0],$in1[0],$in2[0],$vzero

	xxpermdi	$t1,$in1[0],$in1[1],0b00
	xxpermdi	$t2,$in2[1],$in2[0],0b00
	vmsumudm	$out[1],$t1,$t2,$vzero

	xxpermdi	$t2,$in2[2],$in2[1],0b00
	vmsumudm	$out[2],$t1,$t2,$vzero
	vmsumudm	$out[2],$in1[2],$in2[0],$out[2]

	xxpermdi	$t2,$in2[3],$in2[2],0b00
	vmsumudm	$out[3],$t1,$t2,$vzero
	xxpermdi	$t3,$in1[2],$in1[3],0b00
	xxpermdi	$t4,$in2[1],$in2[0],0b00
	vmsumudm	$out[3],$t3,$t4,$out[3]

	xxpermdi	$t2,$in2[4],$in2[3],0b00
	vmsumudm	$out[4],$t1,$t2,$vzero
	xxpermdi	$t4,$in2[2],$in2[1],0b00
	vmsumudm	$out[4],$t3,$t4,$out[4]
	vmsumudm	$out[4],$in1[4],$in2[0],$out[4]

	xxpermdi	$t2,$in2[5],$in2[4],0b00
	vmsumudm	$out[5],$t1,$t2,$vzero
	xxpermdi	$t4,$in2[3],$in2[2],0b00
	vmsumudm	$out[5],$t3,$t4,$out[5]

	xxpermdi	$t2,$in2[6],$in2[5],0b00
	vmsumudm	$out[6],$t1,$t2,$vzero
	xxpermdi	$t4,$in2[4],$in2[3],0b00
	vmsumudm	$out[6],$t3,$t4,$out[6]

	xxpermdi	$t2,$in2[7],$in2[6],0b00
	vmsumudm	$out[7],$t1,$t2,$vzero
	xxpermdi	$t4,$in2[5],$in2[4],0b00
	vmsumudm	$out[7],$t3,$t4,$out[7]

	xxpermdi	$t2,$in2[8],$in2[7],0b00
	vmsumudm	$out[8],$t1,$t2,$vzero
	xxpermdi	$t4,$in2[6],$in2[5],0b00
	vmsumudm	$out[8],$t3,$t4,$out[8]

	xxpermdi	$t1,$in1[4],$in1[5],0b00
	xxpermdi	$t2,$in2[1],$in2[0],0b00
	vmsumudm	$out[5],$t1,$t2,$out[5]

	xxpermdi	$t2,$in2[2],$in2[1],0b00
	vmsumudm	$out[6],$t1,$t2,$out[6]
	vmsumudm	$out[6],$in1[6],$in2[0],$out[6]

	xxpermdi	$t2,$in2[3],$in2[2],0b00
	vmsumudm	$out[7],$t1,$t2,$out[7]
	xxpermdi	$t3,$in1[6],$in1[7],0b00
	xxpermdi	$t4,$in2[1],$in2[0],0b00
	vmsumudm	$out[7],$t3,$t4,$out[7]

	xxpermdi	$t2,$in2[4],$in2[3],0b00
	vmsumudm	$out[8],$t1,$t2,$out[8]
	xxpermdi	$t4,$in2[2],$in2[1],0b00
	vmsumudm	$out[8],$t3,$t4,$out[8]
	vmsumudm	$out[8],$in1[8],$in2[0],$out[8]

	li		$zero,0
	li		$one,1
	mtvsrdd		$t1,$one,$zero
___

		for (my $i = 0; $i <= 8; $i++) {
			$code.=<<___;
	vsld		$in2[$i],$in2[$i],$t1
___
		}

		$code.=<<___;

	vmsumudm	$out[7],$in1[8],$in2[8],$out[7]

	xxpermdi	$t2,$in2[8],$in2[7],0b00
	xxpermdi	$t1,$in1[7],$in1[8],0b00
	vmsumudm	$out[6],$t1,$t2,$out[6]

	xxpermdi	$t1,$in1[6],$in1[7],0b00
	vmsumudm	$out[5],$t1,$t2,$out[5]
	vmsumudm	$out[5],$in1[8],$in2[6],$out[5]

	xxpermdi	$t1,$in1[5],$in1[6],0b00
	vmsumudm	$out[4],$t1,$t2,$out[4]
	xxpermdi	$t4,$in2[6],$in2[5],0b00
	xxpermdi	$t3,$in1[7],$in1[8],0b00
	vmsumudm	$out[4],$t3,$t4,$out[4]

	xxpermdi	$t1,$in1[4],$in1[5],0b00
	vmsumudm	$out[3],$t1,$t2,$out[3]
	xxpermdi	$t3,$in1[6],$in1[7],0b00
	vmsumudm	$out[3],$t3,$t4,$out[3]
	vmsumudm	$out[3],$in1[8],$in2[4],$out[3]

	xxpermdi	$t1,$in1[3],$in1[4],0b00
	vmsumudm	$out[2],$t1,$t2,$out[2]
	xxpermdi	$t3,$in1[5],$in1[6],0b00
	vmsumudm	$out[2],$t3,$t4,$out[2]

	xxpermdi	$t1,$in1[2],$in1[3],0b00
	vmsumudm	$out[1],$t1,$t2,$out[1]
	xxpermdi	$t3,$in1[4],$in1[5],0b00
	vmsumudm	$out[1],$t3,$t4,$out[1]

	xxpermdi	$t1,$in1[1],$in1[2],0b00
	vmsumudm	$out[0],$t1,$t2,$out[0]
	xxpermdi	$t3,$in1[3],$in1[4],0b00
	vmsumudm	$out[0],$t3,$t4,$out[0]

	xxpermdi	$t2,$in2[4],$in2[3],0b00
	xxpermdi	$t1,$in1[7],$in1[8],0b00
	vmsumudm	$out[2],$t1,$t2,$out[2]

	xxpermdi	$t1,$in1[6],$in1[7],0b00
	vmsumudm	$out[1],$t1,$t2,$out[1]
	vmsumudm	$out[1],$in1[8],$in2[2],$out[1]

	xxpermdi	$t1,$in1[5],$in1[6],0b00
	vmsumudm	$out[0],$t1,$t2,$out[0]
	xxpermdi	$t4,$in2[2],$in2[1],0b00
	xxpermdi	$t3,$in1[7],$in1[8],0b00
	vmsumudm	$out[0],$t3,$t4,$out[0]

___

		store_vrs($outp, \@out);

		pop_vrs(52, 63);

		endproc("p521_felem_mul");
	}

	{
		#
		# p51_felem_square
		#

		my ($inp) = ("r4");
		my @in = map("v$_",(45..53));
		my @inx2 = map("v$_",(35..43));

		startproc("p521_felem_square");

		push_vrs(52, 63);

		$code.=<<___;
	vspltisw	$vzero,0

___

		load_vrs($inp, \@in);

		$code.=<<___;
	li		$zero,0
	li		$one,1
	mtvsrdd		$t1,$one,$zero
___

		for (my $i = 0; $i <= 8; $i++) {
			$code.=<<___;
	vsld		$inx2[$i],$in[$i],$t1
___
		}

		$code.=<<___;
	vmsumudm	$out[0],$in[0],$in[0],$vzero

	vmsumudm	$out[1],$in[0],$inx2[1],$vzero

	xxpermdi	$t1,$in[0],$in[1],0b00
	xxpermdi	$t2,$inx2[2],$in[1],0b00
	vmsumudm	$out[2],$t1,$t2,$vzero

	xxpermdi	$t2,$inx2[3],$inx2[2],0b00
	vmsumudm	$out[3],$t1,$t2,$vzero

	xxpermdi	$t2,$inx2[4],$inx2[3],0b00
	vmsumudm	$out[4],$t1,$t2,$vzero
	vmsumudm	$out[4],$in[2],$in[2],$out[4]

	xxpermdi	$t2,$inx2[5],$inx2[4],0b00
	vmsumudm	$out[5],$t1,$t2,$vzero
	vmsumudm	$out[5],$in[2],$inx2[3],$out[5]

	xxpermdi	$t2,$inx2[6],$inx2[5],0b00
	vmsumudm	$out[6],$t1,$t2,$vzero
	xxpermdi	$t3,$in[2],$in[3],0b00
	xxpermdi	$t4,$inx2[4],$in[3],0b00
	vmsumudm	$out[6],$t3,$t4,$out[6]

	xxpermdi	$t2,$inx2[7],$inx2[6],0b00
	vmsumudm	$out[7],$t1,$t2,$vzero
	xxpermdi	$t4,$inx2[5],$inx2[4],0b00
	vmsumudm	$out[7],$t3,$t4,$out[7]

	xxpermdi	$t2,$inx2[8],$inx2[7],0b00
	vmsumudm	$out[8],$t1,$t2,$vzero
	xxpermdi	$t4,$inx2[6],$inx2[5],0b00
	vmsumudm	$out[8],$t3,$t4,$out[8]
	vmsumudm	$out[8],$in[4],$in[4],$out[8]

	vmsumudm	$out[1],$in[5],$inx2[5],$out[1]

	vmsumudm	$out[3],$in[6],$inx2[6],$out[3]

	vmsumudm	$out[5],$in[7],$inx2[7],$out[5]

	vmsumudm	$out[7],$in[8],$inx2[8],$out[7]

	mtvsrdd		$t1,$one,$zero
___

		for (my $i = 5; $i <= 8; $i++) {
			$code.=<<___;
	vsld		$inx2[$i],$inx2[$i],$t1
___
		}

		$code.=<<___;

	vmsumudm	$out[6],$in[7],$inx2[8],$out[6]

	vmsumudm	$out[5],$in[6],$inx2[8],$out[5]

	xxpermdi	$t2,$inx2[8],$inx2[7],0b00
	xxpermdi	$t1,$in[5],$in[6],0b00
	vmsumudm	$out[4],$t1,$t2,$out[4]

	xxpermdi	$t1,$in[4],$in[5],0b00
	vmsumudm	$out[3],$t1,$t2,$out[3]

	xxpermdi	$t1,$in[3],$in[4],0b00
	vmsumudm	$out[2],$t1,$t2,$out[2]
	vmsumudm	$out[2],$in[5],$inx2[6],$out[2]

	xxpermdi	$t1,$in[2],$in[3],0b00
	vmsumudm	$out[1],$t1,$t2,$out[1]
	vmsumudm	$out[1],$in[4],$inx2[6],$out[1]

	xxpermdi	$t1,$in[1],$in[2],0b00
	vmsumudm	$out[0],$t1,$t2,$out[0]
	xxpermdi	$t2,$inx2[6],$inx2[5],0b00
	xxpermdi	$t1,$in[3],$in[4],0b00
	vmsumudm	$out[0],$t1,$t2,$out[0]

___

		store_vrs($outp, \@out);

		pop_vrs(52, 63);

		endproc("p521_felem_square");
	}
}

$code =~ s/\`([^\`]*)\`/eval $1/gem;
print $code;
close STDOUT or die "error closing STDOUT: $!";
