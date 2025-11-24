#! /usr/bin/env perl
# Copyright 2004-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html


use strict;
use warnings;

my @directory_vars = ( "dir", "certs", "crl_dir", "new_certs_dir" );
my @file_vars = ( "database", "certificate", "serial", "crlnumber",
		  "crl", "private_key", "RANDFILE" );
while(<STDIN>) {
    s|\R$||;
    foreach my $d (@directory_vars) {
	if (/^(\s*\#?\s*${d}\s*=\s*)\.\/([^\s\#]*)([\s\#].*)$/) {
	    $_ = "$1sys\\\$disk:\[.$2$3";
	} elsif (/^(\s*\#?\s*${d}\s*=\s*)(\w[^\s\#]*)([\s\#].*)$/) {
	    $_ = "$1sys\\\$disk:\[.$2$3";
	}
	s/^(\s*\#?\s*${d}\s*=\s*\$\w+)\/([^\s\#]*)([\s\#].*)$/$1.$2\]$3/;
	while(/^(\s*\#?\s*${d}\s*=\s*(\$\w+\.|sys\\\$disk:\[\.)[\w\.]+)\/([^\]]*)\](.*)$/) {
	    $_ = "$1.$3]$4";
	}
    }
    foreach my $f (@file_vars) {
	s/^(\s*\#?\s*${f}\s*=\s*)\.\/(.*)$/$1sys\\\$disk:\[\/$2/;
	while(/^(\s*\#?\s*${f}\s*=\s*(\$\w+|sys\\\$disk:\[)[^\/]*)\/(\w+\/[^\s\#]*)([\s\#].*)$/) {
	    $_ = "$1.$3$4";
	}
	if (/^(\s*\#?\s*${f}\s*=\s*(\$\w+|sys\\\$disk:\[)[^\/]*)\/(\w+)([\s\#].*)$/) {
	    $_ = "$1]$3.$4";
	} elsif  (/^(\s*\#?\s*${f}\s*=\s*(\$\w+|sys\\\$disk:\[)[^\/]*)\/([^\s\#]*)([\s\#].*)$/) {
	    $_ = "$1]$3$4";
	}
   }
    print $_,"\n";
}
