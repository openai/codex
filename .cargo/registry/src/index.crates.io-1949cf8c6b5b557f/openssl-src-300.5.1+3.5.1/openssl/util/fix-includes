#!/bin/sh
#
# Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

find -name ossl_typ.h -o \( \
	 -name '*.h' -o \
	 -name '*.h.in' -o \
	 -name '*.c' -o \
	 -name '*.ec' -o \
	 -name 'README*' -o \
	 -name '*.pod' -o \
	 -name '*.cnf' -o -name '*.conf' \
	 \) -exec sed -E -i \
	 -f util/fix-includes.sed {} \;
