#! /usr/bin/env perl
# Copyright 2021-2022 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# ====================================================================
# Written by Amitay Isaacs <amitay@ozlabs.org>, Martin Schwenke
# <martin@meltin.net> & Alastair D'Silva <alastair@d-silva.org> for
# the OpenSSL project.
# ====================================================================

#
# Fixed length (n=6), unrolled PPC Montgomery Multiplication
#

# 2021
#
# Although this is a generic implementation for unrolling Montgomery
# Multiplication for arbitrary values of n, this is currently only
# used for n = 6 to improve the performance of ECC p384.
#
# Unrolling allows intermediate results to be stored in registers,
# rather than on the stack, improving performance by ~7% compared to
# the existing PPC assembly code.
#
# The ISA 3.0 implementation uses combination multiply/add
# instructions (maddld, maddhdu) to improve performance by an
# additional ~10% on Power 9.
#
# Finally, saving non-volatile registers into volatile vector
# registers instead of onto the stack saves a little more.
#
# On a Power 9 machine we see an overall improvement of ~18%.
#

use strict;
use warnings;

my ($flavour, $output, $dir, $xlate);

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}ppc-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/ppc-xlate.pl" and -f $xlate) or
die "can't locate ppc-xlate.pl";

open STDOUT,"| $^X $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";

if ($flavour !~ /64/) {
	die "bad flavour ($flavour) - only ppc64 permitted";
}

my $SIZE_T= 8;

# Registers are global so the code is remotely readable

# Parameters for Montgomery multiplication
my $ze	= "r0";
my $sp	= "r1";
my $toc	= "r2";
my $rp	= "r3";
my $ap	= "r4";
my $bp	= "r5";
my $np	= "r6";
my $n0	= "r7";
my $num	= "r8";

my $i	= "r9";
my $c0	= "r10";
my $bp0	= "r11";
my $bpi	= "r11";
my $bpj	= "r11";
my $tj	= "r12";
my $apj	= "r12";
my $npj	= "r12";
my $lo	= "r14";
my $c1	= "r14";

# Non-volatile registers used for tp[i]
#
# 12 registers are available but the limit on unrolling is 10,
# since registers from $tp[0] to $tp[$n+1] are used.
my @tp = ("r20" .. "r31");

# volatile VSRs for saving non-volatile GPRs - faster than stack
my @vsrs = ("v32" .. "v46");

package Mont;

sub new($$)
{
	my ($class, $n) = @_;

	if ($n > 10) {
		die "Can't unroll for BN length ${n} (maximum 10)"
	}

	my $self = {
		code => "",
		n => $n,
	};
	bless $self, $class;

	return $self;
}

sub add_code($$)
{
	my ($self, $c) = @_;

	$self->{code} .= $c;
}

sub get_code($)
{
	my ($self) = @_;

	return $self->{code};
}

sub get_function_name($)
{
	my ($self) = @_;

	return "bn_mul_mont_fixed_n" . $self->{n};
}

sub get_label($$)
{
	my ($self, $l) = @_;

	return "L" . $l . "_" . $self->{n};
}

sub get_labels($@)
{
	my ($self, @labels) = @_;

	my %out = ();

	foreach my $l (@labels) {
		$out{"$l"} = $self->get_label("$l");
	}

	return \%out;
}

sub nl($)
{
	my ($self) = @_;

	$self->add_code("\n");
}

sub copy_result($)
{
	my ($self) = @_;

	my ($n) = $self->{n};

	for (my $j = 0; $j < $n; $j++) {
		$self->add_code(<<___);
	std		$tp[$j],`$j*$SIZE_T`($rp)
___
	}

}

sub mul_mont_fixed($)
{
	my ($self) = @_;

	my ($n) = $self->{n};
	my $fname = $self->get_function_name();
	my $label = $self->get_labels("outer", "enter", "sub", "copy", "end");

	$self->add_code(<<___);

.globl	.${fname}
.align	5
.${fname}:

___

	$self->save_registers();

	$self->add_code(<<___);
	li		$ze,0
	ld		$n0,0($n0)

	ld		$bp0,0($bp)

	ld		$apj,0($ap)
___

	$self->mul_c_0($tp[0], $apj, $bp0, $c0);

	for (my $j = 1; $j < $n - 1; $j++) {
		$self->add_code(<<___);
	ld		$apj,`$j*$SIZE_T`($ap)
___
		$self->mul($tp[$j], $apj, $bp0, $c0);
	}

	$self->add_code(<<___);
	ld		$apj,`($n-1)*$SIZE_T`($ap)
___

	$self->mul_last($tp[$n-1], $tp[$n], $apj, $bp0, $c0);

	$self->add_code(<<___);
	li		$tp[$n+1],0

___

	$self->add_code(<<___);
	li		$i,0
	mtctr		$num
	b		$label->{"enter"}

.align	4
$label->{"outer"}:
	ldx		$bpi,$bp,$i

	ld		$apj,0($ap)
___

	$self->mul_add_c_0($tp[0], $tp[0], $apj, $bpi, $c0);

	for (my $j = 1; $j < $n; $j++) {
		$self->add_code(<<___);
	ld		$apj,`$j*$SIZE_T`($ap)
___
		$self->mul_add($tp[$j], $tp[$j], $apj, $bpi, $c0);
	}

	$self->add_code(<<___);
	addc		$tp[$n],$tp[$n],$c0
	addze		$tp[$n+1],$ze
___

	$self->add_code(<<___);
.align	4
$label->{"enter"}:
	mulld		$bpi,$tp[0],$n0

	ld		$npj,0($np)
___

	$self->mul_add_c_0($lo, $tp[0], $bpi, $npj, $c0);

	for (my $j = 1; $j < $n; $j++) {
		$self->add_code(<<___);
	ld		$npj,`$j*$SIZE_T`($np)
___
		$self->mul_add($tp[$j-1], $tp[$j], $npj, $bpi, $c0);
	}

	$self->add_code(<<___);
	addc		$tp[$n-1],$tp[$n],$c0
	addze		$tp[$n],$tp[$n+1]

	addi		$i,$i,$SIZE_T
	bdnz		$label->{"outer"}

	and.		$tp[$n],$tp[$n],$tp[$n]
	bne		$label->{"sub"}

	cmpld		$tp[$n-1],$npj
	blt		$label->{"copy"}

$label->{"sub"}:
___

	#
	# Reduction
	#

		$self->add_code(<<___);
	ld		$bpj,`0*$SIZE_T`($np)
	subfc		$c1,$bpj,$tp[0]
	std		$c1,`0*$SIZE_T`($rp)

___
	for (my $j = 1; $j < $n - 1; $j++) {
		$self->add_code(<<___);
	ld		$bpj,`$j*$SIZE_T`($np)
	subfe		$c1,$bpj,$tp[$j]
	std		$c1,`$j*$SIZE_T`($rp)

___
	}

		$self->add_code(<<___);
	subfe		$c1,$npj,$tp[$n-1]
	std		$c1,`($n-1)*$SIZE_T`($rp)

___

	$self->add_code(<<___);
	addme.		$tp[$n],$tp[$n]
	beq		$label->{"end"}

$label->{"copy"}:
___

	$self->copy_result();

	$self->add_code(<<___);

$label->{"end"}:
___

	$self->restore_registers();

	$self->add_code(<<___);
	li		r3,1
	blr
.size .${fname},.-.${fname}
___

}

package Mont::GPR;

our @ISA = ('Mont');

sub new($$)
{
    my ($class, $n) = @_;

    return $class->SUPER::new($n);
}

sub save_registers($)
{
	my ($self) = @_;

	my $n = $self->{n};

	$self->add_code(<<___);
	std	$lo,-8($sp)
___

	for (my $j = 0; $j <= $n+1; $j++) {
		$self->{code}.=<<___;
	std	$tp[$j],-`($j+2)*8`($sp)
___
	}

	$self->add_code(<<___);

___
}

sub restore_registers($)
{
	my ($self) = @_;

	my $n = $self->{n};

	$self->add_code(<<___);
	ld	$lo,-8($sp)
___

	for (my $j = 0; $j <= $n+1; $j++) {
		$self->{code}.=<<___;
	ld	$tp[$j],-`($j+2)*8`($sp)
___
	}

	$self->{code} .=<<___;

___
}

# Direct translation of C mul()
sub mul($$$$$)
{
	my ($self, $r, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld		$lo,$a,$w
	addc		$r,$lo,$c
	mulhdu		$c,$a,$w
	addze		$c,$c

___
}

# Like mul() but $c is ignored as an input - an optimisation to save a
# preliminary instruction that would set input $c to 0
sub mul_c_0($$$$$)
{
	my ($self, $r, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld		$r,$a,$w
	mulhdu		$c,$a,$w

___
}

# Like mul() but does not to the final addition of CA into $c - an
# optimisation to save an instruction
sub mul_last($$$$$$)
{
	my ($self, $r1, $r2, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld		$lo,$a,$w
	addc		$r1,$lo,$c
	mulhdu		$c,$a,$w

	addze		$r2,$c
___
}

# Like C mul_add() but allow $r_out and $r_in to be different
sub mul_add($$$$$$)
{
	my ($self, $r_out, $r_in, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld		$lo,$a,$w
	addc		$lo,$lo,$c
	mulhdu		$c,$a,$w
	addze		$c,$c
	addc		$r_out,$r_in,$lo
	addze		$c,$c

___
}

# Like mul_add() but $c is ignored as an input - an optimisation to save a
# preliminary instruction that would set input $c to 0
sub mul_add_c_0($$$$$$)
{
	my ($self, $r_out, $r_in, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld		$lo,$a,$w
	addc		$r_out,$r_in,$lo
	mulhdu		$c,$a,$w
	addze		$c,$c

___
}

package Mont::GPR_300;

our @ISA = ('Mont::GPR');

sub new($$)
{
	my ($class, $n) = @_;

	my $mont = $class->SUPER::new($n);

	return $mont;
}

sub get_function_name($)
{
	my ($self) = @_;

	return "bn_mul_mont_300_fixed_n" . $self->{n};
}

sub get_label($$)
{
	my ($self, $l) = @_;

	return "L" . $l . "_300_" . $self->{n};
}

# Direct translation of C mul()
sub mul($$$$$)
{
	my ($self, $r, $a, $w, $c, $last) = @_;

	$self->add_code(<<___);
	maddld		$r,$a,$w,$c
	maddhdu		$c,$a,$w,$c

___
}

# Save the last carry as the final entry
sub mul_last($$$$$)
{
	my ($self, $r1, $r2, $a, $w, $c) = @_;

	$self->add_code(<<___);
	maddld		$r1,$a,$w,$c
	maddhdu		$r2,$a,$w,$c

___
}

# Like mul() but $c is ignored as an input - an optimisation to save a
# preliminary instruction that would set input $c to 0
sub mul_c_0($$$$$)
{
	my ($self, $r, $a, $w, $c) = @_;

	$self->add_code(<<___);
	mulld          $r,$a,$w
	mulhdu          $c,$a,$w

___
}

# Like C mul_add() but allow $r_out and $r_in to be different
sub mul_add($$$$$$)
{
	my ($self, $r_out, $r_in, $a, $w, $c) = @_;

	$self->add_code(<<___);
	maddld		$lo,$a,$w,$c
	maddhdu		$c,$a,$w,$c
	addc		$r_out,$r_in,$lo
	addze		$c,$c

___
}

# Like mul_add() but $c is ignored as an input - an optimisation to save a
# preliminary instruction that would set input $c to 0
sub mul_add_c_0($$$$$$)
{
	my ($self, $r_out, $r_in, $a, $w, $c) = @_;

	$self->add_code(<<___);
	maddld		$lo,$a,$w,$r_in
	maddhdu		$c,$a,$w,$r_in
___

	if ($r_out ne $lo) {
		$self->add_code(<<___);
	mr			$r_out,$lo
___
	}

	$self->nl();
}


package main;

my $code;

$code.=<<___;
.machine "any"
.text
___

my $mont;

$mont = new Mont::GPR(6);
$mont->mul_mont_fixed();
$code .= $mont->get_code();

$mont = new Mont::GPR_300(6);
$mont->mul_mont_fixed();
$code .= $mont->get_code();

$code =~ s/\`([^\`]*)\`/eval $1/gem;

$code.=<<___;
.asciz  "Montgomery Multiplication for PPC by <amitay\@ozlabs.org>, <alastair\@d-silva.org>"
___

print $code;
close STDOUT or die "error closing STDOUT: $!";
