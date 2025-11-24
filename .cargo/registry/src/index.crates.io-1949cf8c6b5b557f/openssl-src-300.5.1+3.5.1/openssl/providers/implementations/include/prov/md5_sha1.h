/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_MD5_SHA1_H
# define OSSL_PROV_MD5_SHA1_H

# include <openssl/opensslconf.h>

# ifndef OPENSSL_NO_MD5
#  include <openssl/e_os2.h>
#  include <stddef.h>
#  include <openssl/md5.h>
#  include <openssl/sha.h>

#  define MD5_SHA1_DIGEST_LENGTH (MD5_DIGEST_LENGTH + SHA_DIGEST_LENGTH)
#  define MD5_SHA1_CBLOCK MD5_CBLOCK

typedef struct md5_sha1_st {
    MD5_CTX md5;
    SHA_CTX sha1;
} MD5_SHA1_CTX;

int ossl_md5_sha1_init(MD5_SHA1_CTX *mctx);
int ossl_md5_sha1_update(MD5_SHA1_CTX *mctx, const void *data, size_t count);
int ossl_md5_sha1_final(unsigned char *md, MD5_SHA1_CTX *mctx);
int ossl_md5_sha1_ctrl(MD5_SHA1_CTX *mctx, int cmd, int mslen, void *ms);

# endif /* OPENSSL_NO_MD5 */

#endif /* OSSL_PROV_MD5_SHA1_H */
