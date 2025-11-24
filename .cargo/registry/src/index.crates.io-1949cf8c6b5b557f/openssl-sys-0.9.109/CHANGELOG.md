# Change Log

## [Unreleased]

## [v0.9.109] - 2025-05-28

### Fixed

* Fixed building with `vcpkg`, a statically linked OpenSSL, and rust 1.87.0.
* Fixed building on the latest BoringSSL.

## [v0.9.108] - 2025-04-30

### Added

* Added support for LibreSSL 4.1.x.

## [v0.9.107] - 2025-04-04

### Added

* Support for building with AWS-LC.

## [v0.9.106] - 2025-02-15

### Added

* Support building with `OPENSSL_NO_RC2`.
* Exposed `EVP_rc2_cbc` and `EVP_rc2_40_cbc`.

## [v0.9.105] - 2025-02-02

### Added

* Added `DTLS_server_method` and `DTLS_client_method`.

## [v0.9.104] - 2024-10-15

### Added

* Added support for LibreSSL 4.0.x.
* Added `EVP_KDF_*` and `EVP_KDF_CTX_*` bindings.
* Added `EVP_DigestSqueeze`.
* Added `OSSL_PARAM_construct_octet_string`.
* Added `OSSL_set_max_threads` and `OSSL_get_max_threads`.

### Changed

* `openssl-sys` is now a 2021 edition crate
* Explicitly specify the MSRV in `Cargo.toml`
* Raised the `bindgen` (optional) dependency from 0.65 to 0.69

## [v0.9.103] - 2024-07-20

### Added

* Added several functions and constants for datagram `BIO`s.
* Added `EVP_PKEY_set1_DSA`, `EVP_PKEY_set1_DH`, and `EVP_PKEY_set1_EC_KEY`.
* Added several functions related to QUIC support.

## [v0.9.102] - 2024-03-28

### Added

* Added support for LibreSSL 3.9.x.

## [v0.9.101] - 2024-02-21

### Fixed

* Fixed a bug where, when building with the `vendored` feature, this crate always needed to be rebuilt.

## [v0.9.100] - 2024-02-19

### Added

* Added `OSSL_PARAM`, `OSSL_PARAM_construct_uint` , `OSSL_PARAM_construct_end`.
* Added `EVP_PKEY_CTX_set_params` and `EVP_PKEY_CTX_get_params`.
* Added `X509_alias_get0`.
* Added `EVP_default_properties_enable_fips`.

## [v0.9.99] - 2024-01-19

### Added

* On macOS added Homebrew's `openssl@3.0` to the list of candidates to build against.
* `NID_brainpoolP256r1`, `NID_brainpoolP320r1`, `NID_brainpoolP384r1`, and `NID_brainpoolP512r1` are now available on LibreSSL.

### Changed

* `X509_PURPOSE` is now opaque on LibreSSL 3.9.0+.

## [v0.9.98] - 2023-12-22

### Added

* Added `RAND_priv_bytes`.
* Added `NID_brainpoolP320r1`.

### Changed

* `X509_PURPOSE_get0` now returns a `const` pointer on LibreSSL 3.9.0+.
* `X509V3_EXT_add_alias` is removed on LibreSSL 3.9.0+.

## [v0.9.97] - 2023-12-04

### Changed

* libatomic is no longer dynamically linked for 32 bit ARM targets.

### Added

* Added `SSL_read_ex`, `SSL_peek_ex`, and `SSL_write_ex`.

## [v0.9.96] - 2023-11-22

### Changed

* `EVP_chacha20` is now available on LibreSSL

### Added

* Added `EVP_des_ede3_ecb`, `EVP_des_ede3_cfb8`, `EVP_des_ede3_ofb`, `EVP_camellia_128_ofb`, `EVP_camellia_192_ofb`, `EVP_camellia_256_ofb`, `EVP_cast5_ofb`, `EVP_idea_ofb`
* Added `X509_STORE_get1_all_certs`
* Added `SSL_CTRL_GET_PEER_TMP_KEY`, `SSL_CTRL_GET_TMP_KEY`, `SSL_get_peer_tmp_key`, `SSL_get_tmp_key`

## [v0.9.95] - 2023-11-03

### Changed

* Fixed the availability of `EVP_PKEY_RSA_PSS` on OpenSSL

### Added

* Added support for LibreSSL 3.8.x.
* Added `NID_chacha20_poly1305`

## [v0.9.94] - 2023-11-01

### Changed

* `X509_ALGOR` is now opaque on new LibreSSL releases

### Added

* Added support for building with `OPENSSL_NO_SCRYPT`
* Added `EVP_PKEY_RSA_PSS` and `EVP_PKEY_DHX`
* Functions and constants for using HKDF `EVP_PKEY` are now available on LibreSSL.
* Added `SSL_CTX_set_security_level`, `SSL_set_security_level`, `SSL_CTX_get_security_level`, `SSL_get_security_level`
* Added `X509_check_host`, `X509_check_email`, `X509_check_ip`, `X509_check_ip_asc`

## [v0.9.93] - 2023-09-04

### Changed

* The `vendored` Cargo feature now builds OpenSSL 3.1, as 1.1.1 is reaching its EOL.

### Added

* Added support for LibreSSL 3.8.1.

## [v0.9.92] - 2023-08-27

### Added

* Added `EVP_CIPHER_CTX_copy`
* Expose `EVP_chacha20_poly1305` on LibreSSL
* Added `X509_VERIFY_PARAM_set1_email`

## [v0.9.91] - 2023-08-06

### Added

* Expose `poly1305_state`, `CRYPTO_poly1305_init`, `CRYPTO_poly1305_update`, and `CRYPTO_poly1305_finish` on BoringSSL and LibreSSL.
* Fix detection of libraries on OpenBSD.
* Added `EC_POINT_point2hex` and `EC_POINT_hex2point`.
* Added `EVP_PKEY_verify_recover_init`, `EVP_PKEY_verify_recover`, and `EVP_PKEY_CTX_set_signature_md`.
* Added `EVP_CIPHER_CTX_FLAG_WRAP_ALLOW` and `EVP_CTX_set_flags`.
* Added `BN_mod_sqrt`.

## [v0.9.90] - 2023-06-20

### Fixed

* Fixed compilation with BoringSSL when building with the bindgen CLI.

## [v0.9.89] - 2023-06-20

### Fixed

* Fixed compilation with recent versions of BoringSSL.

### Added

* Added support for detecting OpenSSL compiled with `OPENSSL_NO_OCB`.
* Added `EVP_PKEY_SM2` and `NID_sm2`.
* Added `EVP_PKEY_assign_RSA`, `EVP_PKEY_assign_DSA`, `EVP_PKEY_assign_DH`, and `EVP_PKEY_assign_EC_KEY`.
* Added `EC_GROUP_get_asn1_flag`.
* Expose `EC_POINT_get_affine_coordinates` on BoringSSL and LibreSSL.
* Added `EVP_PKEY_derive_set_peer_ex`.

## [v0.9.88] - 2023-05-30

### Added

* Added support for the LibreSSL 3.8.0.
* Added support for detecting `OPENSSL_NO_RC4`.
* Added `OBJ_dup`.
* Added `ASN1_TYPE_new`, `ASN1_TYPE_set`, `d2i_ASN1_TYPE`, and `i2d_ASN1_TYPE`.
* Added `SSL_bytes_to_cipher_list`, `SSL_CTX_get_num_tickets`, and `SSL_get_num_tickets`.
* Added `GENERAL_NAME_set0_othername`.
* Added `X509_get_pathlen`.

## [v0.9.87] - 2023-04-24

### Added

* Added `DH_CHECK`.
* Added `CMAC_CTX_new`, `CMAC_CTX_free`, `CMAC_Init`, `CMAC_Update`, `CMAC_Final`, and `CMAC_CTX_copy`.
* Added `EVP_default_properties_is_fips_enabled`.
* Added `X509_get0_subject_key_id`, `X509_get0_authority_key_id`, `X509_get0_authority_issuer`, and `X509_get0_authority_serial`.
* Added `NID_poly1305`.


## [v0.9.86] - 2023-04-20

### Fixed

* Fixed BoringSSL support with the latest bindgen release.

### Added

* Added bindings for PKCS#7 functions and more X.509 functions.


## [v0.9.85] - 2023-04-09

### Added

* Added support for LibreSSL 3.7.x.

## [v0.9.84] - 2023-04-01

### Added

* Added `ASN1_INTEGER_dup` and `ASN1_INTEGER_cmp`.
* Added `stack_st_X509_NAME_ENTRY`.
* Added `DIST_POINT_NAME`, `DIST_POINT`, `stack_st_DIST_POINT`, `DIST_POINT_free`, and `DIST_POINT_NAME_free`.

## [v0.9.83] - 2023-03-23

### Fixed

* Fixed version checks for LibreSSL.

### Added

* Added `i2d_X509_EXTENSION`.
* Added `GENERAL_NAME_new`.

## [v0.9.82] - 2023-03-19

### Added

* Added support for LibreSSL 3.7.1.
* Added support for X25519 and Ed25519 on LibreSSL and BoringSSL.

## [v0.9.81] - 2023-03-14

### Fixed

Fixed builds against OpenSSL built with `no-cast`.

### Added

* Added experimental bindgen support for BoringSSL.
* Added `X509_VERIFY_PARAM_set_auth_level`, `X509_VERIFY_PARAM_get_auth_level`, and `X509_VERIFY_PARAM_set_purpose`.
* Added `X509_PURPOSE_*` consts.
* Added `X509_NAME_add_entry`.
* Added `X509_load_crl_file`.
* Added `SSL_set_cipher_list`, `SSL_set_ssl_method`, `SSL_use_PrivateKey_file`, `SSL_use_PrivateKey`, `SSL_use_certificate`, `SSL_use_certificate_chain_file`, `SSL_set_client_CA_list`, `SSL_add_client_CA`, and `SSL_set0_verify_cert_store`.
* Added `X509_PURPOSE`, `X509_STORE_set_purpose`, and `X509_STORE_set_trust`.
* Added `SSL_CTX_set_num_tickets`, `SSL_set_num_tickets`, `SSL_CTX_get_num_tickets`, and `SSL_get_num_tickets`.
* Added `CMS_verify`.

### Removed

* Removed an unnecessary link to libatomic for 32-bit android targets.

## [v0.9.80] - 2022-12-20

### Fixed

* Added `NO_DEPRECATED_3_0` cfg checks for more APIs.

### Added

* Added support for LibreSSL 3.7.0.
* Added `SSL_CTRL_CHAIN_CERT` and `SSL_add0_chain_cert`.
* Added `EVP_PKEY_get_security_bits` and `EVP_PKEY_security_bits`.
* Added `OSSL_PROVIDER_set_default_search_path`.

## [v0.9.79] - 2022-12-06

### Added

* Added `EVP_CIPHER_CTX_num`.
* Added `X509_LOOKUP_file` and `X509_load_cert_file`.

## [v0.9.78] - 2022-11-23

### Added

* Added support for LibreSSL 3.6.x.
* Added `NID_brainpoolP256r1`, `NID_brainpoolP384r1`, and `NID_brainpool512r1`.
* Added `EVP_camellia_128_cfb128`, `EVP_camellia_128_ecb`, `EVP_camellia_192_cfb128`, `EVP_camellia_192_ecb`,
    `EVP_camellia_256_cfb128`, and `EVP_camellia_256_ecb`.
* Added `EVP_cast5_cfb64` and `EVP_cast5_ecb`.
* Added `EVP_idea_cfb64` and `EVP_idea_ecb`.
* Added `DSA_SIG`, `d2i_DSA_SIG`, `i2d_DSA_SIG`, `DSA_SIG_new`, `DSA_SIG_free`, `DSA_SIG_get0`, and `DSA_SIG_set0`.
* Added `X509_STORE_set1_param`, `X509_VERIFY_PARAM_new`, `X509_VERIFY_PARAM_set_time`, and
    `X509_VERIFY_PARAM_set_depth`.

## [v0.9.77] - 2022-10-22

### Added

* Added support for LibreSSL 3.6.0
* Added `assume_init`.

## [v0.9.76] - 2022-09-26

### Added

* Added `SSL_get_psk_identity_hint` and `SSL_get_psk_identity`.
* Added SHA-3 NID constants.
* Added `SSL_OP_PRIORITIZE_CHACHA`.
* Added `X509_REQ_print`.
* Added `EVP_MD_CTX_size` and `EVP_MD_CTX_get_size`
* Added `EVP_MD_CTX_reset`.
* Added experimental, unstable support for BoringSSL.

### Fixed

* Fixed the deprecation note on `SSL_CTX_set_alpn_select_cb`.

## [v0.9.75] - 2022-07-09

### Added

* Added SM4 bindings.
* Added `EC_GROUP_set_generator` and `EC_POINT_set_affine_coordinates_GFp`.

## [v0.9.74] - 2022-06-01

### Added

* Added `EVP_MD_block_size`.
* Added `X509V3_EXT_add_alias`.
* Added `X509_V_ERR_INVALID_CA` back when building against OpenSSL 3.0.

## [v0.9.73] - 2022-05-02

### Added

* Added support for installations that place libraries in `$OPENSSL_DIR/lib64` in addition to `$OPENSSL_DIR/lib`.
* Added `X509_issuer_name_hash`.
* Added `ASN1_string_set`.
* Added `X509_CRL_dup`, `X509_REQ_dup`, `X509_NAME_dup`, and `X509_dup`.
* Added `X509_print`.
* Added support for LibreSSL 3.5.x.

## [v0.9.72] - 2021-12-11

### Changed

* Temporarily downgraded the vendored OpenSSL back to 1.1.1 due to significant performance regressions. We will move
    back to 3.0.0 when a future release resolves those issues.

### Added

* Added `PKCS12_set_mac`.
* Added `EVP_PKEY_sign_init`, `EVP_PKEY_sign`, `EVP_PKEY_verify_init`, and `EVP_PKEY_verify`.
* Added support for LibreSSL 3.4.x.

## [v0.9.71]

### Fixed

* Fixed linkage to static OpenSSL 3.0.0 libraries on some 32 bit Android targets.

### Added

* Added support for LibreSSL 3.4.1.
* Added `SSL_get_extms_support` and `SSL_CTRL_GET_EXTMS_SUPPORT`.
* Added `OBJ_create`.
* Added `EVP_CIPHER_CTX_get0_cipher`, `EVP_CIPHER_CTX_get_block_size`, `EVP_CIPHER_CTX_get_key_length`,
    `EVP_CIPHER_CTX_get_iv_length`, and `EVP_CIPHER_CTX_get_tag_length`.
* Added `EVP_CIPHER_free`.
* Added `EVP_CIPHER_CTX_rand_key`.
* Added `OSSL_LIB_CTX_new` and `OSSL_LIB_CTX_free`.
* Added `EVP_CIPHER_fetch`.
* Added `EVP_MD_fetch` and `EVP_MD_free`.
* Added `OPENSSL_malloc` and `OPENSSL_free`.
* Added `EVP_DigestSignUpdate` and `EVP_DigestVerifyUpdate`.

## [v0.9.70] - 2021-10-31

### Fixed

* Fixed linkage to static 3.0.0 OpenSSL libraries on some 32 bit architectures.

## [v0.9.69] - 2021-10-31

### Changed

* Upgraded the vendored OpenSSL to 3.0.0.

### Added

* Added support for automatic detection of Homebrew `openssl@3` installs.
* Added `EVP_PKEY_Q_keygen` and `EVP_EC_gen`.

## [v0.9.68] - 2021-10-27

### Added

* Added `BN_bn2binpad`.
* Added `i2d_X509_NAME` and `d2i_X509_NAME`.
* Added `BN_FLG_MALLOCED`, `BN_FLG_STATIC_DATA`, `BN_FLG_CONSTTIME`, and `BN_FLG_SECURE`.
* Added `BN_CTX_secure_new`, `BN_secure_new`, `BN_set_flags`, and `BN_get_flags`.

## [v0.9.67] - 2021-09-21

### Added

* Added support for LibreSSL 3.4.0

## [v0.9.66] - 2021-08-17

### Added

* Added `EVP_seed_cbc`, `EVP_seed_cfb128`, `EVP_seed_ecb`, and `EVP_seed_ofb`.
* Added `OBJ_length` and `OBJ_get0_data`.
* Added `i2d_PKCS8PrivateKey_bio`.

## [v0.9.65] - 2021-06-21

### Fixed

* Restored the accidentally deleted `PEM_read_bio_X509_CRL` function.

## [v0.9.64] - 2021-06-18

### Added

* Added support for OpenSSL 3.x.x.
* Added `SSL_peek`.
* Added `ERR_LIB_ASN1` and `ASN1_R_HEADER_TOO_LONG`.
* Added `d2i_X509_bio`.
* Added `OBJ_nid2obj`.
* Added `RAND_add`.
* Added `SSL_CTX_set_post_handshake_auth`.
* Added `COMP_get_type`.
* Added `X509_get_default_cert_file_env`, `X509_get_default_cert_file`, `X509_get_default_cert_dir_env`, and
    `X509_get_default_cirt_dir`.

## [v0.9.63] - 2021-05-06

### Added

* Added support for LibreSSL 3.3.x.

## [v0.9.62] - 2021-04-28

### Added

* Added support for LibreSSL 3.3.2.
* Added `DH_set0_key`.
* Added `EC_POINT_get_affine_coordinates`.

## [v0.9.61] - 2021-03-13

### Added

* Added support for automatic detection of OpenSSL installations via pkgsrc and MacPorts on macOS.
* Added various `V_ASN1_*` constants.
* Added `DH_generate_parameters_ex`.
* Added `EC_POINT_is_at_infinity` and `EC_POINT_is_on_curve`.
* Added `EVP_CIPHER_nid`.
* Added `EVP_sm3`.
* Added `NID_*` constants related to SM3.
* Added `PKCS7_get0_signers`.
* Added `EVP_PKEY_CTX_set0_rsa_oaep_label`.
* Added `ACCESS_DESCRIPTION` and `ACCESS_DESCRIPTION_free`.

## [v0.9.60] - 2020-12-24

### Added

* Added support for the default Homebrew install directory on ARM.
* Added `EVP_PKEY_CTX_set_rsa_oaep_md` and `EVP_PKEY_CTRL_RSA_OAEP_MD`.

## [v0.9.59] - 2020-12-09

### Added

* Added support for LibreSSL 3.2.x, 3.3.0, and 3.3.1.
* Added `DH_generate_parameters`, `DH_generate_key`, `DH_compute_key`, and `DH_size`.
* Added `NID_X25519`, `NID_X448`, `EVP_PKEY_x25519` and `EVP_PKEY_x448`.
* Added `OBJ_txt2obj`.
* Added `d2i_PKCS7` and `i2d_PKCS7`.
* Added `SRTP_AEAD_AES_128_GCM` and `SRTP_AEAD_AES_256_GCM`.

## [v0.9.58] - 2020-06-05

### Added

* Added `SSL_set_mtu`.
* Added support for LibreSSL 3.2.0.
* Added `PEM_read_bio_EC_PUBKEY`, `PEM_write_bio_EC_PUBKEY`, `d2i_EC_PUBKEY`, and `i2d_EC_PUBKEY`.
* Added `EVP_PKEY_encrypt_init`, `EVP_PKEY_encrypt`, `EVP_PKEY_decrypt_init`, `EVP_PKEY_decrypt`,
    `EVP_PKEY_get_raw_public_key`, `EVP_PKEY_new_raw_public_key`, `EVP_PKEY_get_raw_private_key`,
    and `EVP_PKEY_new_raw_private_key`.
* Added `OBJ_sn2nid`.

## [v0.9.57] - 2020-05-24

### Added

* Added support for LibreSSL 3.1.x.

## [v0.9.56] - 2020-05-07

### Fixed

* Fixed vendored builds on windows-gnu targets.

### Added

* Added support for LibreSSL 3.0.0.

## [v0.9.55] - 2020-04-07

### Fixed

* Fixed windows-msvc library names when using OpenSSL from vcpkg.

### Added

* If the `OPENSSL_NO_VENDOR` environment variable is set, vendoring will not be used even if enabled.
* Added `SSL_CTX_get_verify_mode` and `SSL_get_verify_mode`.
* Added `SSL_is_init_finished`.
* Added `SSL_CTX_set_cert_store`.
* Added `TLS_server_method` and `TLS_client_method`.
* Added `X509_STORE_get0_objects`.
* Added `X509_OBJECT_free`, `X509_OBJECT_get_type`, and `X509_OBJECT_get0_X509`.

## [v0.9.54] - 2020-01-29

### Added

* Added `BIO_CTRL_DGRAM_QUERY_MTU`.
* Added `EVP_EncryptInit_ex`, `EVP_EncryptFinal_ex`, `EVP_DecryptInit_ex`, and `EVP_DecryptFinal_ex`.
* Added `EVP_md_null`.
* Added `EVP_PKCS82PKEY`.
* Added `PKCS8_PRIV_KEY_INFO`, `d2i_PKCS8_PRIV_KEY_INFO`, and `PKCS8_PRIV_KEY_INFO_free`.
* Added `SSL_OP_NO_RENEGOTIATION`.

## [v0.9.53] - 2019-11-22

### Added

* Added `ASN1_TIME_diff`.
* Added `EC_GROUP_order_bits`.
* Added `EVP_EncodeBlock` and `EVP_DecodeBlock`.
* Added `SSL_CTRL_SET_GROUPS_LIST`, `SSL_CTRL_SET_SIGALGS_LIST`, `SSL_CTX_set1_groups_list`, and
    `SSL_CTX_set1_sigalgs_list`.
* Added `Clone` implementations to `SHA_CTX`, `SHA256_CTX`, and `SHA512_CTX`.

## [v0.9.52] - 2019-10-19

### Added

* Added support for LibreSSL 3.0.x.

## [v0.9.51] - 2019-10-02

### Added

* Added support for LibreSSL 3.0.1.

## [v0.9.50] - 2019-10-02

### Added

* Added `CRYPTO_LOCK_EVP_PKEY`.
* Added `EVP_PKEY_ED25519` and `EVP_PKEY_ED448`.
* Added `EVP_DigestSign` and `EVP_DigestVerify`.
* Added `EVP_PKEY_up_ref`.
* Added `NID_ED25519` and `NID_ED448`.

## [v0.9.49] - 2019-08-15

### Added

* Added support for LibreSSL 3.0.0.

## [v0.9.48] - 2019-07-19

### Added

* Added `AES_wrap_key` and `AES_unwrap_key`.
* Added `EC_GROUP_get_cofactor`, `EC_GROUP_get0_generator`, and `EC_POINT_dup`.
* Added `EVP_aes_128_ofb`, `EVP_aes_192_ecb`, `EVP_aes_192_cbc`, `EVP_aes_192_cfb1`, `EVP_aes_192_cfb8`,
    `EVP_aes_192_cfb_128`, `EVP_aes_192_ctr`, `EVP_aes_192_ccm`, `EVP_aes_192_gcm`, `EVP_aes_192_ofb`, and
    `EVP_aes_256_ofb`.
* Added `PEM_read_bio_CMS` and `PEM_write_bio_CMS`.

## [v0.9.47] - 2019-05-18

### Added

* Added `SSL_CTX_add_client_CA`.

## [v0.9.46] - 2019-05-08

### Added

* Added support for the LibreSSL 2.9.x series.

## [v0.9.45] - 2019-05-03

### Fixed

* Reverted a change to windows-gnu library names that caused regressions.

## [v0.9.44] - 2019-04-30

### Added

* The `DEP_OPENSSL_VENDORED` environment variable tells downstream build scripts if the vendored feature was enabled.
* Added `EVP_SealInit`, `EVP_SealFinal`, `EVP_EncryptUpdate`, `EVP_OpenInit`, `EVP_OpenFinal`, and `EVP_DecryptUpdate`.
* Added `EVP_PKEY_size`.

### Fixed

* Fixed library names when targeting windows-gnu and pkg-config fails.

## [v0.9.43] - 2019-03-20

### Added

* Added `d2i_CMS_ContentInfo` and `CMS_encrypt`.
* Added `X509_verify` and `X509_REQ_verify`.
* Added `EVP_MD_type` and `EVP_GROUP_get_curve_name`.

[Unreleased]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.109..master
[v0.9.109]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.108...openssl-sys-v0.9.109
[v0.9.108]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.107...openssl-sys-v0.9.108
[v0.9.107]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.106...openssl-sys-v0.9.107
[v0.9.106]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.105...openssl-sys-v0.9.106
[v0.9.105]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.104...openssl-sys-v0.9.105
[v0.9.104]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.103...openssl-sys-v0.9.104
[v0.9.103]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.102...openssl-sys-v0.9.103
[v0.9.102]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.101...openssl-sys-v0.9.102
[v0.9.101]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.100...openssl-sys-v0.9.101
[v0.9.100]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.99...openssl-sys-v0.9.100
[v0.9.99]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.98...openssl-sys-v0.9.99
[v0.9.98]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.97...openssl-sys-v0.9.98
[v0.9.97]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.96...openssl-sys-v0.9.97
[v0.9.96]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.95...openssl-sys-v0.9.96
[v0.9.95]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.94...openssl-sys-v0.9.95
[v0.9.94]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.93...openssl-sys-v0.9.94
[v0.9.93]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.92...openssl-sys-v0.9.93
[v0.9.92]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.91...openssl-sys-v0.9.92
[v0.9.91]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.90...openssl-sys-v0.9.91
[v0.9.90]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.89...openssl-sys-v0.9.90
[v0.9.89]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.88...openssl-sys-v0.9.89
[v0.9.88]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.87...openssl-sys-v0.9.88
[v0.9.87]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.86...openssl-sys-v0.9.87
[v0.9.86]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.85...openssl-sys-v0.9.86
[v0.9.85]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.84...openssl-sys-v0.9.85
[v0.9.84]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.83...openssl-sys-v0.9.84
[v0.9.83]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.82...openssl-sys-v0.9.83
[v0.9.82]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.81...openssl-sys-v0.9.82
[v0.9.81]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.80...openssl-sys-v0.9.81
[v0.9.80]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.79...openssl-sys-v0.9.80
[v0.9.79]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.78...openssl-sys-v0.9.79
[v0.9.78]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.77...openssl-sys-v0.9.78
[v0.9.77]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.76...openssl-sys-v0.9.77
[v0.9.76]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.75...openssl-sys-v0.9.76
[v0.9.75]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.74...openssl-sys-v0.9.75
[v0.9.74]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.73...openssl-sys-v0.9.74
[v0.9.73]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.72...openssl-sys-v0.9.73
[v0.9.72]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.71...openssl-sys-v0.9.72
[v0.9.71]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.70...openssl-sys-v0.9.71
[v0.9.70]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.69...openssl-sys-v0.9.70
[v0.9.69]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.68...openssl-sys-v0.9.69
[v0.9.68]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.67...openssl-sys-v0.9.68
[v0.9.67]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.66...openssl-sys-v0.9.67
[v0.9.66]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.65...openssl-sys-v0.9.66
[v0.9.65]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.64...openssl-sys-v0.9.65
[v0.9.64]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.63...openssl-sys-v0.9.64
[v0.9.63]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.62...openssl-sys-v0.9.63
[v0.9.62]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.61...openssl-sys-v0.9.62
[v0.9.61]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.60...openssl-sys-v0.9.61
[v0.9.60]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.59...openssl-sys-v0.9.60
[v0.9.59]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.58...openssl-sys-v0.9.59
[v0.9.58]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.57...openssl-sys-v0.9.58
[v0.9.57]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.56...openssl-sys-v0.9.57
[v0.9.56]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.55...openssl-sys-v0.9.56
[v0.9.55]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.54...openssl-sys-v0.9.55
[v0.9.54]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.53...openssl-sys-v0.9.54
[v0.9.53]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.52...openssl-sys-v0.9.53
[v0.9.52]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.51...openssl-sys-v0.9.52
[v0.9.51]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.50...openssl-sys-v0.9.51
[v0.9.50]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.49...openssl-sys-v0.9.50
[v0.9.49]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.48...openssl-sys-v0.9.49
[v0.9.48]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.47...openssl-sys-v0.9.48
[v0.9.47]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.46...openssl-sys-v0.9.47
[v0.9.46]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.45...openssl-sys-v0.9.46
[v0.9.45]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.44...openssl-sys-v0.9.45
[v0.9.44]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.43...openssl-sys-v0.9.44
[v0.9.43]: https://github.com/sfackler/rust-openssl/compare/openssl-sys-v0.9.42...openssl-sys-v0.9.43
