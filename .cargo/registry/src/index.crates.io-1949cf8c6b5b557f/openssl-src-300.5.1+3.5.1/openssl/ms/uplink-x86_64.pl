#! /usr/bin/env perl
# Copyright 2008-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# $output is the last argument if it looks like a file (it has an extension)
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
open OUT,"| \"$^X\" \"${dir}../crypto/perlasm/x86_64-xlate.pl\" $flavour \"$output\""
    or die "can't call ${dir}../crypto/perlasm/x86_64-xlate.pl: $!";
*STDOUT=*OUT;
push(@INC,"${dir}.");

require "uplink-common.pl";

$prefix="_lazy";

print <<___;
.text
.extern	OPENSSL_Uplink
.globl	OPENSSL_UplinkTable
___
for ($i=1;$i<=$N;$i++) {
print <<___;
.type	$prefix${i},\@abi-omnipotent
.align	16
$prefix${i}:
	.byte	0x48,0x83,0xEC,0x28	# sub rsp,40
	mov	%rcx,48(%rsp)
	mov	%rdx,56(%rsp)
	mov	%r8,64(%rsp)
	mov	%r9,72(%rsp)
	lea	OPENSSL_UplinkTable(%rip),%rcx
	mov	\$$i,%rdx
	call	OPENSSL_Uplink
	mov	48(%rsp),%rcx
	mov	56(%rsp),%rdx
	mov	64(%rsp),%r8
	mov	72(%rsp),%r9
	lea	OPENSSL_UplinkTable(%rip),%rax
	add	\$40,%rsp
	jmp	*8*$i(%rax)
$prefix${i}_end:
.size	$prefix${i},.-$prefix${i}
___
}
print <<___;
.data
OPENSSL_UplinkTable:
        .quad   $N
___
for ($i=1;$i<=$N;$i++) {   print "      .quad   $prefix$i\n";   }
print <<___;
.section	.pdata,"r"
.align		4
___
for ($i=1;$i<=$N;$i++) {
print <<___;
	.rva	$prefix${i},$prefix${i}_end,${prefix}_unwind_info
___
}
print <<___;
.section	.xdata,"r"
.align		8
${prefix}_unwind_info:
	.byte	0x01,0x04,0x01,0x00
	.byte	0x04,0x42,0x00,0x00
___

close STDOUT;
