/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef STORE
# error Macro STORE undefined
#endif

STORE("file", "yes", ossl_file_store_functions)
#ifndef OPENSSL_NO_WINSTORE
STORE("org.openssl.winstore", "yes", ossl_winstore_store_functions)
#endif
