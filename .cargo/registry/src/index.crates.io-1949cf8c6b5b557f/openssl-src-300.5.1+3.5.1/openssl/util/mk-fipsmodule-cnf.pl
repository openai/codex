#! /usr/bin/env perl
# Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use Getopt::Long;

# Module options for pedantic FIPS mode
# self_test_onload happens if install_mac isn't included, don't add it below
my $conditional_errors = 1;
my $security_checks = 1;
my $ems_check = 1;
my $no_short_mac = 1;
my $drgb_no_trunc_dgst = 1;
my $digest_check = 1;
my $dsa_sign_disabled = 1;
my $tdes_encrypt_disabled = 1;
my $pkcs15_pad_disable = 1;
my $rsa_pss_saltlen_check = 1;
my $rsa_sign_x931_pad_disabled = 1;
my $kdf_key_check = 1;
my $pbkdf2_lower_bound_check = 1;
my $ec_cofactor_check = 1;
my $mac_key_check = 1;

my $activate = 1;
my $version = 1;
my $mac_key;
my $module_name;
my $section_name = "fips_sect";

GetOptions("key=s"              => \$mac_key,
           "module=s"           => \$module_name,
           "section_name=s"     => \$section_name)
    or die "Error when getting command line arguments";

my $mac_keylen = length($mac_key);

use Digest::SHA qw(hmac_sha256_hex);
my $module_size = [ stat($module_name) ]->[7];

open my $fh, "<:raw", $module_name or die "Trying to open $module_name: $!";
read $fh, my $data, $module_size or die "Trying to read $module_name: $!";
close $fh;

# Calculate HMAC-SHA256 in hex, and split it into a list of two character
# chunks, and join the chunks with colons.
my @module_mac
    = ( uc(hmac_sha256_hex($data, pack("H$mac_keylen", $mac_key))) =~ m/../g );
my $module_mac = join(':', @module_mac);

print <<_____;
[$section_name]
activate = $activate
install-version = $version
conditional-errors = $conditional_errors
security-checks = $security_checks
module-mac = $module_mac
tls1-prf-ems-check = $ems_check
no-short-mac = $no_short_mac
drbg-no-trunc-md = $drgb_no_trunc_dgst
signature-digest-check = $digest_check
dsa-sign-disabled = $dsa_sign_disabled
hkdf-digest-check = $digest_check
tls13-kdf-digest-check = $digest_check
tls1-prf-digest-check = $digest_check
sshkdf-digest-check = $digest_check
sskdf-digest-check = $digest_check
x963kdf-digest-check = $digest_check
tdes-encrypt-disabled = $tdes_encrypt_disabled
rsa-pkcs15-pad-disabled = $pkcs15_pad_disable
rsa-pss-saltlen-check = $rsa_pss_saltlen_check
rsa-sign-x931-pad-disabled = $rsa_sign_x931_pad_disabled
hkdf-key-check = $kdf_key_check
kbkdf-key-check = $kdf_key_check
tls13-kdf-key-check = $kdf_key_check
tls1-prf-key-check = $kdf_key_check
sshkdf-key-check = $kdf_key_check
sskdf-key-check = $kdf_key_check
x963kdf-key-check = $kdf_key_check
x942kdf-key-check = $kdf_key_check
pbkdf2-lower-bound-check = $pbkdf2_lower_bound_check
ecdh-cofactor-check = $ec_cofactor_check
hmac-key-check = $mac_key_check
kmac-key-check = $mac_key_check
_____
