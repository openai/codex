/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
*/

/*
 * This file is meant to be included from ec_kmgmt.c
 */

static const OSSL_PARAM ec_private_key_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_public_key_types[] = {
    EC_IMEXPORTABLE_PUBLIC_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_key_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_PUBLIC_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_dom_parameters_types[] = {
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_5_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_6_types[] = {
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_key_domp_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_other_parameters_types[] = {
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_9_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_10_types[] = {
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_11_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_all_parameters_types[] = {
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_13_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_14_types[] = {
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM ec_all_types[] = {
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_PUBLIC_KEY,
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};

static const OSSL_PARAM *ec_types[] = {
    NULL,
    ec_private_key_types,
    ec_public_key_types,
    ec_key_types,
    ec_dom_parameters_types,
    ec_5_types,
    ec_6_types,
    ec_key_domp_types,
    ec_other_parameters_types,
    ec_9_types,
    ec_10_types,
    ec_11_types,
    ec_all_parameters_types,
    ec_13_types,
    ec_14_types,
    ec_all_types
};
