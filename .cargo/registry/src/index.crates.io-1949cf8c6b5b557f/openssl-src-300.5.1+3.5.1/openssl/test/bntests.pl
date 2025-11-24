#! /usr/bin/env perl
# Copyright 2008-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Run the tests specified in bntests.txt, as a check against OpenSSL.
use strict;
use warnings;
use Math::BigInt;

my $EXPECTED_FAILURES = 0;
my $failures = 0;

sub bn
{
    my $x = shift;
    my ($sign, $hex) = ($x =~ /^([+\-]?)(.*)$/);

    $hex = '0x' . $hex if $hex !~ /^0x/;
    return Math::BigInt->from_hex($sign.$hex);
}

sub evaluate
{
    my $lineno = shift;
    my %s = @_;

    if ( defined $s{'Sum'} ) {
        # Sum = A + B
        my $sum = bn($s{'Sum'});
        my $a = bn($s{'A'});
        my $b = bn($s{'B'});
        return if $sum == $a + $b;
    } elsif ( defined $s{'LShift1'} ) {
        # LShift1 = A * 2
        my $lshift1 = bn($s{'LShift1'});
        my $a = bn($s{'A'});
        return if $lshift1 == $a->bmul(2);
    } elsif ( defined $s{'LShift'} ) {
        # LShift = A * 2**N
        my $lshift = bn($s{'LShift'});
        my $a = bn($s{'A'});
        my $n = bn($s{'N'});
        return if $lshift == $a->blsft($n);
    } elsif ( defined $s{'RShift'} ) {
        # RShift = A / 2**N
        my $rshift = bn($s{'RShift'});
        my $a = bn($s{'A'});
        my $n = bn($s{'N'});
        return if $rshift == $a->brsft($n);
    } elsif ( defined $s{'Square'} ) {
        # Square = A * A
        my $square = bn($s{'Square'});
        my $a = bn($s{'A'});
        return if $square == $a->bmul($a);
    } elsif ( defined $s{'Product'} ) {
        # Product = A * B
        my $product = bn($s{'Product'});
        my $a = bn($s{'A'});
        my $b = bn($s{'B'});
        return if $product == $a->bmul($b);
    } elsif ( defined $s{'Quotient'} ) {
        # Quotient = A / B
        # Remainder = A - B * Quotient
        my $quotient = bn($s{'Quotient'});
        my $remainder = bn($s{'Remainder'});
        my $a = bn($s{'A'});
        my $b = bn($s{'B'});

        # First the remainder test.
        $b->bmul($quotient);
        my $rempassed = $remainder == $a->bsub($b) ? 1 : 0;

        # Math::BigInt->bdiv() is documented to do floored division,
        # i.e. 1 / -4 = -1, while OpenSSL BN_div does truncated
        # division, i.e. 1 / -4 = 0.  We need to make the operation
        # work like OpenSSL's BN_div to be able to verify.
        $a = bn($s{'A'});
        $b = bn($s{'B'});
        my $neg = $a->is_neg() ? !$b->is_neg() : $b->is_neg();
        $a->babs();
        $b->babs();
        $a->bdiv($b);
        $a->bneg() if $neg;
        return if $rempassed && $quotient == $a;
    } elsif ( defined $s{'ModMul'} ) {
        # ModMul = (A * B) mod M
        my $modmul = bn($s{'ModMul'});
        my $a = bn($s{'A'});
        my $b = bn($s{'B'});
        my $m = bn($s{'M'});
        $a->bmul($b);
        return if $modmul == $a->bmod($m);
    } elsif ( defined $s{'ModExp'} ) {
        # ModExp = (A ** E) mod M
        my $modexp = bn($s{'ModExp'});
        my $a = bn($s{'A'});
        my $e = bn($s{'E'});
        my $m = bn($s{'M'});
        return if $modexp == $a->bmodpow($e, $m);
    } elsif ( defined $s{'Exp'} ) {
        my $exp = bn($s{'Exp'});
        my $a = bn($s{'A'});
        my $e = bn($s{'E'});
        return if $exp == $a ** $e;
    } elsif ( defined $s{'ModSqrt'} ) {
        # (ModSqrt * ModSqrt) mod P = A mod P
        my $modsqrt = bn($s{'ModSqrt'});
        my $a = bn($s{'A'});
        my $p = bn($s{'P'});
        $modsqrt->bmul($modsqrt);
        $modsqrt->bmod($p);
        $a->bmod($p);
        return if $modsqrt == $a;
    } else {
        print "# Unknown test: ";
    }
    $failures++;
    print "# #$failures Test (before line $lineno) failed\n";
    foreach ( keys %s ) {
        print "$_ = $s{$_}\n";
    }
    print "\n";
}

my $infile = shift || 'bntests.txt';
die "No such file, $infile" unless -f $infile;
open my $IN, $infile || die "Can't read $infile, $!\n";

my %stanza = ();
my $l = 0;
while ( <$IN> ) {
    $l++;
    s|\R$||;
    next if /^#/;
    if ( /^$/ ) {
        if ( keys %stanza ) {
            evaluate($l, %stanza);
            %stanza = ();
        }
        next;
    }
    # Parse 'key = value'
    if ( ! /\s*([^\s]*)\s*=\s*(.*)\s*/ ) {
        print "Skipping $_\n";
        next;
    }
    $stanza{$1} = $2;
};
evaluate($l, %stanza) if keys %stanza;
die "Got $failures, expected $EXPECTED_FAILURES"
    if $infile eq 'bntests.txt' and $failures != $EXPECTED_FAILURES;
close($IN)
