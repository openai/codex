/*
 * Copyright 2017-2018 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_X509_V3_ADMIS_H
# define OSSL_CRYPTO_X509_V3_ADMIS_H

struct NamingAuthority_st {
    ASN1_OBJECT* namingAuthorityId;
    ASN1_IA5STRING* namingAuthorityUrl;
    ASN1_STRING* namingAuthorityText;          /* i.e. DIRECTORYSTRING */
};

struct ProfessionInfo_st {
    NAMING_AUTHORITY* namingAuthority;
    STACK_OF(ASN1_STRING)* professionItems;    /* i.e. DIRECTORYSTRING */
    STACK_OF(ASN1_OBJECT)* professionOIDs;
    ASN1_PRINTABLESTRING* registrationNumber;
    ASN1_OCTET_STRING* addProfessionInfo;
};

struct Admissions_st {
    GENERAL_NAME* admissionAuthority;
    NAMING_AUTHORITY* namingAuthority;
    STACK_OF(PROFESSION_INFO)* professionInfos;
};

struct AdmissionSyntax_st {
    GENERAL_NAME* admissionAuthority;
    STACK_OF(ADMISSIONS)* contentsOfAdmissions;
};

#endif
