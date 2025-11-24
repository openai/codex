# -*- mode: perl; -*-
# Copyright 2016-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

## SSL test configurations

package ssltests;

sub test_pem
{
    my ($file) = @_;
    my $dir_sep = $^O ne "VMS" ? "/" : "";
    return "\${ENV::TEST_CERTS_DIR}" . $dir_sep . $file,
}

our $fips_mode = 0;
our $no_deflt_libctx = 0;
our $fips_3_4 = 0;
our $fips_3_5 = 0;

our %base_server = (
    "Certificate" => test_pem("servercert.pem"),
    "PrivateKey"  => test_pem("serverkey.pem"),
    "CipherString" => "DEFAULT",
);

our %base_client = (
    "VerifyCAFile" => test_pem("rootcert.pem"),
    "VerifyMode" => "Peer",
    "CipherString" => "DEFAULT",
);
