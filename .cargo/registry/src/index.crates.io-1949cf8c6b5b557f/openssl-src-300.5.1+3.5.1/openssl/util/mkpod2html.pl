#! /usr/bin/env perl
# Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

use lib ".";
use Getopt::Std;
use Pod::Html;
use File::Spec::Functions qw(:DEFAULT rel2abs);

# Options.
our($opt_i);    # -i INFILE
our($opt_o);    # -o OUTFILE
our($opt_t);    # -t TITLE
our($opt_r);    # -r PODROOT

getopts('i:o:t:r:');
die "-i flag missing" unless $opt_i;
die "-o flag missing" unless $opt_o;
die "-t flag missing" unless $opt_t;
die "-r flag missing" unless $opt_r;

# We originally used realpath() here, but the Windows implementation appears
# to require that the directory or file exist to be able to process the input,
# so we use rel2abs() instead, which only processes the string without
# looking further.
$opt_i = rel2abs($opt_i) or die "Can't convert to real path: $!";
$opt_o = rel2abs($opt_o) or die "Can't convert to real path: $!";
$opt_r = rel2abs($opt_r) or die "Can't convert to real path: $!";

pod2html
    "--infile=$opt_i",
    "--outfile=$opt_o",
    "--title=$opt_t",
    "--podroot=$opt_r",
    "--podpath=man1:man3:man5:man7",
    "--htmldir=..";

# Read in contents.
open F, "<$opt_o"
    or die "Can't read $opt_o, $!";
my $contents = '';
{
    local $/ = undef;
    $contents = <F>;
}
close F;
unlink $opt_o;

$contents =~
    s|href="http://man\.he\.net/(man\d/[^"]+)(?:\.html)?"|href="../$1.html"|g;
open F, ">$opt_o"
    or die "Can't write $opt_o, $!";
print F $contents;
close F;
