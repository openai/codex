/*
 * Copyright 2018-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef HEADER_ENGINE_LOADER_H
# define HEADER_ENGINE_LOADER_H

# include <openssl/store.h>

/* this is a private URI scheme */
# define ENGINE_SCHEME          "org.openssl.engine"
# define ENGINE_SCHEME_COLON    ENGINE_SCHEME ":"

int setup_engine_loader(void);
void destroy_engine_loader(void);

#endif
