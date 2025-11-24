#! /usr/bin/env perl
# Copyright 2008-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

$output = pop and open STDOUT,">$output";

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC,"${dir}.");

require "uplink-common.pl";

local $V=8;	# max number of args uplink functions may accept...
my $loc0 = "r".(32+$V);
print <<___;
.text
.global	OPENSSL_Uplink#
.type	OPENSSL_Uplink#,\@function

___
for ($i=1;$i<=$N;$i++) {
print <<___;
.proc	lazy$i#
lazy$i:
	.prologue
{ .mii;	.save	ar.pfs,$loc0
	alloc	loc0=ar.pfs,$V,3,2,0
	.save	b0,loc1
	mov	loc1=b0
	addl	loc2=\@ltoff(OPENSSL_UplinkTable#),gp	};;
	.body
{ .mmi;	ld8	out0=[loc2]
	mov	out1=$i					};;
{ .mib;	add	loc2=8*$i,out0
	br.call.sptk.many	b0=OPENSSL_Uplink#	};;
{ .mmi;	ld8	r31=[loc2];;
	ld8	r30=[r31],8				};;
{ .mii;	ld8	gp=[r31]
	mov	b6=r30
	mov	b0=loc1					};;
{ .mib;	mov	ar.pfs=loc0
	br.many	b6					};;
.endp	lazy$i#

___
}
print <<___;
.data
.global OPENSSL_UplinkTable#
OPENSSL_UplinkTable:    data8   $N      // amount of following entries
___
for ($i=1;$i<=$N;$i++) {   print "      data8   \@fptr(lazy$i#)\n";   }
print <<___;
.size   OPENSSL_UplinkTable,.-OPENSSL_UplinkTable#
___

close STDOUT;
