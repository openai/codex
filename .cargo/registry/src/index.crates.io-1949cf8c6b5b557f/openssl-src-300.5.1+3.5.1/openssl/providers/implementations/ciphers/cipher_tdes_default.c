/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DES low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include "cipher_tdes_default.h"
#include "prov/implementations.h"

/* ossl_tdes_ede3_ofb_functions */
IMPLEMENT_tdes_cipher(ede3, EDE3,  ofb, OFB, TDES_FLAGS, 64*3,  8, 64, stream);
/* ossl_tdes_ede3_cfb_functions */
IMPLEMENT_tdes_cipher(ede3, EDE3,  cfb, CFB, TDES_FLAGS, 64*3,  8, 64, stream);
/* ossl_tdes_ede3_cfb1_functions */
IMPLEMENT_tdes_cipher(ede3, EDE3, cfb1, CFB, TDES_FLAGS, 64*3,  8, 64, stream);
/* ossl_tdes_ede3_cfb8_functions */
IMPLEMENT_tdes_cipher(ede3, EDE3, cfb8, CFB, TDES_FLAGS, 64*3,  8, 64, stream);

/* ossl_tdes_ede2_ecb_functions */
IMPLEMENT_tdes_cipher(ede2, EDE2, ecb, ECB, TDES_FLAGS, 64*2, 64, 0, block);
/* ossl_tdes_ede2_cbc_functions */
IMPLEMENT_tdes_cipher(ede2, EDE2, cbc, CBC, TDES_FLAGS, 64*2, 64, 64, block);
/* ossl_tdes_ede2_ofb_functions */
IMPLEMENT_tdes_cipher(ede2, EDE2, ofb, OFB, TDES_FLAGS, 64*2,  8, 64, stream);
/* ossl_tdes_ede2_cfb_functions */
IMPLEMENT_tdes_cipher(ede2, EDE2, cfb, CFB, TDES_FLAGS, 64*2,  8, 64, stream);
