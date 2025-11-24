#! /usr/bin/env perl
# Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

die "No input files" if scalar @ARGV == 0;
$FS = ':.*##';
printf "\nUsage:\n  make <OPTIONS> \033[36m<target>\033[0m\n";

while (<>) {
    chomp;	# strip record separator
    @Fld = split($FS, $_, -1);
    if (/^[a-zA-Z0-9_\-]+:.*?##/) {
	printf "  \033[36m%-15s\033[0m %s\n", $Fld[0], $Fld[1]
    }
    if (/^##@/) {
	printf "\n\033[1m%s\033[0m\n", substr($Fld[$_], (5)-1);
    }
}

printf "\nNote: This list is not all-inclusive\n";

