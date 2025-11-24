/*
 * Copyright 1999-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This table will be searched using OBJ_bsearch so it *must* kept in order
 * of the ext_nid values.
 */

static const X509V3_EXT_METHOD *standard_exts[] = {
    &ossl_v3_nscert,
    &ossl_v3_ns_ia5_list[0],
    &ossl_v3_ns_ia5_list[1],
    &ossl_v3_ns_ia5_list[2],
    &ossl_v3_ns_ia5_list[3],
    &ossl_v3_ns_ia5_list[4],
    &ossl_v3_ns_ia5_list[5],
    &ossl_v3_ns_ia5_list[6],
    &ossl_v3_skey_id,
    &ossl_v3_key_usage,
    &ossl_v3_pkey_usage_period,
    &ossl_v3_alt[0],
    &ossl_v3_alt[1],
    &ossl_v3_bcons,
    &ossl_v3_crl_num,
    &ossl_v3_cpols,
    &ossl_v3_akey_id,
    &ossl_v3_crld,
    &ossl_v3_ext_ku,
    &ossl_v3_delta_crl,
    &ossl_v3_crl_reason,
#ifndef OPENSSL_NO_OCSP
    &ossl_v3_crl_invdate,
#endif
    &ossl_v3_sxnet,
    &ossl_v3_info,
    &ossl_v3_audit_identity,
#ifndef OPENSSL_NO_RFC3779
    &ossl_v3_addr,
    &ossl_v3_asid,
#endif
#ifndef OPENSSL_NO_OCSP
    &ossl_v3_ocsp_nonce,
    &ossl_v3_ocsp_crlid,
    &ossl_v3_ocsp_accresp,
    &ossl_v3_ocsp_nocheck,
    &ossl_v3_ocsp_acutoff,
    &ossl_v3_ocsp_serviceloc,
#endif
    &ossl_v3_sinfo,
    &ossl_v3_policy_constraints,
    &ossl_v3_targeting_information,
    &ossl_v3_no_rev_avail,
#ifndef OPENSSL_NO_OCSP
    &ossl_v3_crl_hold,
#endif
    &ossl_v3_pci,
    &ossl_v3_name_constraints,
    &ossl_v3_policy_mappings,
    &ossl_v3_inhibit_anyp,
    &ossl_v3_subj_dir_attrs,
    &ossl_v3_idp,
    &ossl_v3_alt[2],
    &ossl_v3_freshest_crl,
#ifndef OPENSSL_NO_CT
    &ossl_v3_ct_scts[0],
    &ossl_v3_ct_scts[1],
    &ossl_v3_ct_scts[2],
#endif
    &ossl_v3_utf8_list[0],
    &ossl_v3_issuer_sign_tool,
    &ossl_v3_tls_feature,
    &ossl_v3_ext_admission,
    &ossl_v3_authority_attribute_identifier,
    &ossl_v3_role_spec_cert_identifier,
    &ossl_v3_battcons,
    &ossl_v3_delegated_name_constraints,
    &ossl_v3_time_specification,
    &ossl_v3_attribute_descriptor,
    &ossl_v3_user_notice,
    &ossl_v3_soa_identifier,
    &ossl_v3_acc_cert_policies,
    &ossl_v3_acc_priv_policies,
    &ossl_v3_indirect_issuer,
    &ossl_v3_no_assertion,
    &ossl_v3_aa_issuing_dist_point,
    &ossl_v3_issued_on_behalf_of,
    &ossl_v3_single_use,
    &ossl_v3_group_ac,
    &ossl_v3_allowed_attribute_assignments,
    &ossl_v3_attribute_mappings,
    &ossl_v3_holder_name_constraints,
    &ossl_v3_associated_info,
};

/* Number of standard extensions */

#define STANDARD_EXTENSION_COUNT OSSL_NELEM(standard_exts)

