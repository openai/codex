#! /usr/bin/env perl
# Copyright 2016-2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

# First argument is name;
my $name = shift @ARGV;
my $name_uc = uc $name;
# All other arguments are ignored for now

print <<"_____";
/*
 * Generated with test/generate_buildtest.pl, to check that such a simple
 * program builds.
 */
#include <openssl/opensslconf.h>
#ifndef OPENSSL_NO_STDIO
# include <stdio.h>
#endif
#ifndef OPENSSL_NO_${name_uc}
# include <openssl/$name.h>
#endif

int main(void)
{
    return 0;
}
_____
