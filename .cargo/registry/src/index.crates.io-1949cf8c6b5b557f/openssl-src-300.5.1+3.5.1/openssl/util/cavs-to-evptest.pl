#! /usr/bin/env perl
# Copyright 2019 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#Convert CCM CAVS test vectors to a format suitable for evp_test

use strict;
use warnings;

my $alg;
my $mode;
my $keylen;
my $key = "";
my $iv = "";
my $aad = "";
my $ct = "";
my $pt = "";
my $tag = "";
my $aadlen = 0;
my $ptlen = 0;
my $taglen = 0;
my $res = "";
my $intest = 0;
my $fixediv = 0;

while (<STDIN>)
{
    chomp;

    # Pull out the cipher mode from the comment at the beginning of the file
    if(/^#\s*"([^-]+)-\w+" information/) {
        $mode = lc($1);
    # Pull out the key length from the comment at the beginning of the file
    } elsif(/^#\s*(\w+) Keylen: (\d+)/) {
        $alg = lc($1);
        $keylen = $2;
    # Some parameters common to many tests appear as a list in square brackets
    # so parse these
    } elsif(/\[(.*)\]/) {
        my @pairs = split(/, /, $1);
        foreach my $pair (@pairs) {
            $pair =~ /(\w+)\s*=\s*(\d+)/;
            # AAD Length
            if ($1 eq "Alen") {
                $aadlen = $2;
            # Plaintext length
            } elsif ($1 eq "Plen") {
                $ptlen = $2;
            # Tag length
            } elsif ($1 eq "Tlen") {
                $taglen = $2;
            }
        }
    # Key/Value pair
    } elsif (/^\s*(\w+)\s*=\s*(\S.*)\r/) {
        if ($1 eq "Key") {
            $key = $2;
        } elsif ($1 eq "Nonce") {
            $iv = $2;
            if ($intest == 0) {
                $fixediv = 1;
            } else {
                $fixediv = 0;
            }
        } elsif ($1 eq "Adata") {
            $aad = $2;
        } elsif ($1 eq "CT") {
            $ct = substr($2, 0, length($2) - ($taglen * 2));
            $tag = substr($2, $taglen * -2);
        } elsif ($1 eq "Payload") {
            $pt = $2;
        } elsif ($1 eq "Result") {
            if ($2 =~ /Fail/) {
                $res = "CIPHERUPDATE_ERROR";
            }
        } elsif ($1 eq "Count") {
            $intest = 1;
        } elsif ($1 eq "Plen") {
            $ptlen = $2;
        } elsif ($1 eq "Tlen") {
            $taglen = $2;
        } elsif ($1 eq "Alen") {
            $aadlen = $2;
        }
    # Something else - probably just a blank line
    } elsif ($intest) {
        print "Cipher = $alg-$keylen-$mode\n";
        print "Key = $key\n";
        print "IV = $iv\n";
        print "AAD =";
        if ($aadlen > 0) {
            print " $aad";
        }
        print "\nTag =";
        if ($taglen > 0) {
            print " $tag";
        }
        print "\nPlaintext =";
        if ($ptlen > 0) {
            print " $pt";
        }
        print "\nCiphertext = $ct\n";
        if ($res ne "") {
            print "Operation = DECRYPT\n";
            print "Result = $res\n";
        }
        print "\n";
        $res = "";
        if ($fixediv == 0) {
            $iv = "";
        }
        $aad = "";
        $tag = "";
        $pt = "";
        $intest = 0;
    }
}
