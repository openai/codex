/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h> /* strcmp */
#include <openssl/dh.h>
#include "internal/nelem.h"
#include "crypto/dh.h"

typedef struct dh_name2id_st {
    const char *name;
    int id;
    int type;
} DH_GENTYPE_NAME2ID;

/* Indicates that the paramgen_type can be used for either DH or DHX */
#define TYPE_ANY -1
#ifndef OPENSSL_NO_DH
# define TYPE_DH    DH_FLAG_TYPE_DH
# define TYPE_DHX   DH_FLAG_TYPE_DHX
#else
# define TYPE_DH    0
# define TYPE_DHX   0
#endif

static const DH_GENTYPE_NAME2ID dhtype2id[] = {
    { "group", DH_PARAMGEN_TYPE_GROUP, TYPE_ANY },
    { "generator", DH_PARAMGEN_TYPE_GENERATOR, TYPE_DH },
    { "fips186_4", DH_PARAMGEN_TYPE_FIPS_186_4, TYPE_DHX },
    { "fips186_2", DH_PARAMGEN_TYPE_FIPS_186_2, TYPE_DHX },
};

const char *ossl_dh_gen_type_id2name(int id)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dhtype2id); ++i) {
        if (dhtype2id[i].id == id)
            return dhtype2id[i].name;
    }
    return NULL;
}

#ifndef OPENSSL_NO_DH
int ossl_dh_gen_type_name2id(const char *name, int type)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dhtype2id); ++i) {
        if ((dhtype2id[i].type == TYPE_ANY
             || type == dhtype2id[i].type)
            && strcmp(dhtype2id[i].name, name) == 0)
            return dhtype2id[i].id;
    }
    return -1;
}
#endif
