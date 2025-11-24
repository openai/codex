#! /usr/bin/env perl
# Copyright 1995-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

($#ARGV == 1) || die "usage: cmp.pl <file1> <file2>\n";

open(IN0,"<$ARGV[0]") || die "unable to open $ARGV[0]\n";
open(IN1,"<$ARGV[1]") || die "unable to open $ARGV[1]\n";
binmode IN0;
binmode IN1;

$tot=0;
$ret=1;
for (;;)
{
    $n1=sysread(IN0,$b1,4096);
    $n2=sysread(IN1,$b2,4096);

    last if ($n1 != $n2);
    last if ($b1 ne $b2);
    last if ($n1 < 0);
    if ($n1 == 0)
    {
        $ret=0;
        last;
    }
    $tot+=$n1;
}

close(IN0);
close(IN1);
if ($ret)
{
    printf STDERR "$ARGV[0] and $ARGV[1] are different\n";
    @a1=unpack("C*",$b1);
    @a2=unpack("C*",$b2);
    for ($i=0; $i<=$#a1; $i++)
    {
        if ($a1[$i] ne $a2[$i])
        {
            printf "%02X %02X <<\n",$a1[$i],$a2[$i];
            last;
        }
    }
    $nm=$tot+$n1;
    $tot+=$i+1;
    printf STDERR "diff at char $tot of $nm\n";
}
exit($ret);
