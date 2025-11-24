#! /usr/bin/env perl
# Copyright 2006-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;
use lib ".";
use configdata;

my $cversion = "$config{version}";
my $version = "$config{full_version}";

# RC syntax for versions uses commas as separators, rather than period,
# and it must have exactly 4 numbers (16-bit integers).
my @vernums = ( split(/\./, $cversion), 0, 0, 0, 0 );
$cversion = join(',', @vernums[0..3]);

my $filename = $ARGV[0];
my $description = "OpenSSL library";
my $vft = "VFT_DLL";
if ( $filename =~ /openssl/i ) {
    $description = "OpenSSL application";
    $vft = "VFT_APP";
}

my $YEAR = [gmtime($ENV{SOURCE_DATE_EPOCH} || time())]->[5] + 1900;
print <<___;
#include <winver.h>

LANGUAGE 0x09,0x01

1 VERSIONINFO
  FILEVERSION $cversion
  PRODUCTVERSION $cversion
  FILEFLAGSMASK 0x3fL
#ifdef _DEBUG
  FILEFLAGS 0x01L
#else
  FILEFLAGS 0x00L
#endif
  FILEOS VOS__WINDOWS32
  FILETYPE $vft
  FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            // Required:
            VALUE "CompanyName", "The OpenSSL Project, https://www.openssl.org/\\0"
            VALUE "FileDescription", "$description\\0"
            VALUE "FileVersion", "$version\\0"
            VALUE "InternalName", "$filename\\0"
            VALUE "OriginalFilename", "$filename\\0"
            VALUE "ProductName", "The OpenSSL Toolkit\\0"
            VALUE "ProductVersion", "$version\\0"
            // Optional:
            //VALUE "Comments", "\\0"
            VALUE "LegalCopyright", "Copyright 1998-$YEAR The OpenSSL Authors. All rights reserved.\\0"
            //VALUE "LegalTrademarks", "\\0"
            //VALUE "PrivateBuild", "\\0"
            //VALUE "SpecialBuild", "\\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 0x4b0
    END
END
___
