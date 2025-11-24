#! /usr/bin/env perl
# Copyright 2008-2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC, "${dir}.", "${dir}../crypto/perlasm");
require "x86asm.pl";

require "uplink-common.pl";

$output = pop and open STDOUT,">$output";

&asm_init($ARGV[0]);

&external_label("OPENSSL_Uplink");
&public_label("OPENSSL_UplinkTable");

for ($i=1;$i<=$N;$i++) {
&function_begin_B("_\$lazy${i}");
	&lea	("eax",&DWP(&label("OPENSSL_UplinkTable")));
	&push	($i);
	&push	("eax");
	&call	(&label("OPENSSL_Uplink"));
	&pop	("eax");
	&add	("esp",4);
	&jmp_ptr(&DWP(4*$i,"eax"));
&function_end_B("_\$lazy${i}");
}

&dataseg();
&align(4);
&set_label("OPENSSL_UplinkTable");
&data_word($N);
for ($i=1;$i<=$N;$i++) {
&data_word(&label("_\$lazy${i}"));
}
&asm_finish();

close STDOUT;
