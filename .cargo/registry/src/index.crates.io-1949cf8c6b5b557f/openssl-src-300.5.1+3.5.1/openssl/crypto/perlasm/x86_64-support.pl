#! /usr/bin/env perl
# Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html


package x86_64support;

# require "x86_64-support.pl";
# $ptr_size=&pointer_size($flavour);
# $ptr_reg=&pointer_register($flavour,$reg);

sub ::pointer_size
{
    my($flavour)=@_;
    my $ptr_size=8; $ptr_size=4 if ($flavour eq "elf32");
    return $ptr_size;
}

sub ::pointer_register
{
    my($flavour,$reg)=@_;
    if ($flavour eq "elf32") {
	if ($reg eq "%rax") {
	    return "%eax";
	} elsif ($reg eq "%rbx") {
	    return "%ebx";
	} elsif ($reg eq "%rcx") {
	    return "%ecx";
	} elsif ($reg eq "%rdx") {
	    return "%edx";
	} elsif ($reg eq "%rdi") {
	    return "%edi";
	} elsif ($reg eq "%rsi") {
	    return "%esi";
	} elsif ($reg eq "%rbp") {
	    return "%ebp";
	} elsif ($reg eq "%rsp") {
	    return "%esp";
	} else {
	    return $reg."d";
	}
    } else {
	return $reg;
    }
}

1;
