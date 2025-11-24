#! /usr/bin/env perl
# Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use warnings;
use strict;
use Config;

my $expectedsyms=$ARGV[0];

shift(@ARGV);

my $objlist;
my $objfilelist = join(" ", @ARGV);
my $expsyms;
my $exps;
my $OBJFH;
my $cmd;

if ($Config{osname} eq "MSWin32") {
        my $currentdll = "";
        $cmd = "dumpbin /imports " . $objfilelist;
        my @symlist;
        open $expsyms, '<', $expectedsyms or die;
        {
            local $/;
            $exps=<$expsyms>;
        }
        close($expsyms);
        open($OBJFH, "$cmd|") or die "Cannot open process: $!";
        while (<$OBJFH>)
        {
            chomp;
            my $dllfile = $_;
            $dllfile =~ s/( +)(.*)(\.dll)(.*)/DLLFILE \2/;
            if (index($dllfile, "DLLFILE") >= 0) {
                $currentdll = substr($dllfile, 8);
                $currentdll =~ s/^\s+|s+$//g;
            }
            # filter imports from our own library
            if ("$currentdll" ne "libcrypto-3-x64") {
                my $line = $_;
                $line =~ s/                          [0-9a-fA-F]{1,2} /SYMBOL /;
                if (index($line, "SYMBOL") != -1) {
                    $line =~ s/.*SYMBOL //;
                    push(@symlist, $line);
                }
            }
        }
        foreach (@symlist) {
            if (index($exps, $_) < 0) {
                print "Symbol $_ not in the allowed platform symbols list\n";
                exit 1;
            }
        }
        exit 0;
    }
else {
        $cmd = "objdump -t " . $objfilelist . " | grep UND | grep -v \@OPENSSL";
        $cmd = $cmd . " | awk '{print \$NF}' |";
        $cmd = $cmd . " sed -e\"s/@.*\$//\" | sort | uniq";

        open $expsyms, '<', $expectedsyms or die;
        {
            local $/;
            $exps=<$expsyms>;
        }
        close($expsyms);

        open($OBJFH, "$cmd|") or die "Cannot open process: $!";
        while (<$OBJFH>)
        {
                if (index($exps, $_) < 0) {
                    print "Symbol $_ not in the allowed platform symbols list\n";
                    exit 1;
                }
        }
        close($OBJFH);
        exit 0;
    }
