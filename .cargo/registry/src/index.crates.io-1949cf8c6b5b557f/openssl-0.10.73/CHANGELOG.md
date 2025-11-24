# Change Log

## [Unreleased]

## [v0.10.73] - 2025-05-28

### Fixed

* Fixed building on the latest BoringSSL.

### Changed

* Replaced ctest2 with ctest in systest.

## [v0.10.72] - 2025-04-04

### Fixed

* Fixed use-after-free in `Md::fetch` and `Cipher::fetch` when `properties` is `Some(...)`. In practice this use-after-free most likely resulted in OpenSSL treating the `properties` as `b""`.

### Added

* Support for building with AWS-LC.

## [v0.10.71] - 2025-02-15

### Added

* Added `Cipher::rc2_cbc` and `Cipher::rc2_40_cbc`.

## [v0.10.70] - 2025-02-02

### Fixed

* Fixed improper lifetime constraints in `ssl::select_next_proto` that allowed a use after free.

### Added

* Added `SslMethod::dtls_client` and `SslMethod::dtls_server`.

## [v0.10.69] - 2025-01-25

### Fixed

* Fixed the version constraint on `openssl-macros`.

### Added

* Added `SslContextBuilder::load_verify_locations`.
* Added `Hasher::squeeze_xof`.
* Added `SslContextBuilder::set_alpn_select_callback` support for boringssl.

## [v0.10.68] - 2024-10-16

### Fixed

* Fixed building on Rust 1.63.0 (our MSRV) with OpenSSL 3.2 or newer.

## [v0.10.67] - 2024-10-15

### Added

* Added support for LibreSSL 4.0.x.
* Added `argon2id`

### Fixed

* Fixed a case where `MdCtxRef::digest_verify_final` could leave an error on the stack.
* Fixed a case where `RsaRef::check_key` could leave an errror on the stack.

### Changed

* `openssl` is now a 2021 edition crate
* Explicitly specify the MSRV in `Cargo.toml`

## [v0.10.66] - 2024-07-21

### Fixed

- Fixed undefined behavior in `MemBio::get_buf` when the resulting buffer had a length of 0.

## [v0.10.65] - 2024-07-20

### Fixed

* Ensure we are initialized in `MessageDigest::from_nid`, `Md::from_nid`, `Md::fetch`

### Changed

* Expose `SslContextBuilder::set_keylog_callback` on BoringSSL

## [v0.10.64] - 2024-02-19

### Added

* Added `PkeyCtxRef::{nonce_type, set_nonce_type}`.
* Added `X509Ref::alias`.


## [v0.10.63] - 2024-01-19

### Added

* Added `Pkcs7Ref::{type_,signed}`.
* Added `Pkcs7SignedRef::certificates`.
* Added `Cipher::{aes_256_xts,des_ede3_ecb,des_ede3_cfb8,des_ede3_ofb,camellia128_ofb,camellia192_ofb,camellia256_ofb,cast5_ofb,idea_ofb}`
* Added `PKey::from_dhx`
* Added `PKey::{public_key_from_pem_passphrase,public_key_from_pem_callback}`.

### Changed

* `Cipher::aes_128_ofb` is now available on BoringSSL
* `Nid::{BRAINPOOL_P256R1,BRAINPOOL_P320R1,BRAINPOOL_P384R1,BRAINPOOL_P512R1}` are now available on LibreSSL.

## [v0.10.62] - 2023-12-22

### Added

* Added `Nid::BRAINPOOL_P320R1`
* Added `rand_priv_bytes`

### Fixed

* Fixed building on the latest version of BoringSSL

## [v0.10.61] - 2023-12-04

### Changed

* `SslStream` now uses `SSL_read_ex`, `SSL_write_ex`, and `SSL_peek_ex` when available

### Added

* Added `SslStream::{read_uninit, ssl_read_uninit}`.

## [v0.10.60] - 2023-11-22

### Deprecated

* Deprecated `X509StoreRef::objects`. It is unsound. All callers should migrate to using `X509StoreRef::all_certificates` instead.

### Fixed

* Fixed a memory leak when calling `SslContextBuilder::set_ex_data` and `SslRef::set_ex_data` multiple times with the same index.

### Added

* Added `X509StoreRef::all_certificates`
* Added `cipher::Cipher::{camellia128_cbc,camellia192_cbc,camellia256_cbc,cast5_cbc,idea_cbc}`
* Added `symm::Cipher::{des_ede3_ecb,des_ede3_cfb8,des_ede3_ofb,camellia_128_ecb,camellia_128_ofb,camellia_128_cfb128,camellia_192_ecb,camellia_192_ofb,camellia_192_cfb128,camellia_256_ecb,camellia_256_ofb,camellia_256_cfb128,cast5_ecb,cast5_ofb,cast5_cfb64,idea_ecb,idea_ofb,idea_cfb64}`
* Added `Crypter::update_unchecked`
* Added `SslRef::{peer_tmp_key,tmp_key}`

### Changed

* `cipher::Cipher::chacha20` is now available on LibreSSL
* `symm::Cipher::chacha20` is now available on LibreSSL

## [v0.10.59] - 2023-11-03

### Added

* Added `Nid::CHACHA20_POLY1305`

### Changed

* Fixed the availability of `Id::RSA_PSS` on OpenSSL

## [v0.10.58] - 2023-11-01

### Added

* Added `Id::{RSA_PSS,DHX}` constants
* Added `SslContextBuilder::set_security_level`
* Added `SslContextRef::security_level`
* Added `SslRef::set_security_level`, `SslRef::security_level`
* Added `Cipher::{camellia_128_cbc, camellia_192_cbc, camellia_256_cbc, cast5_cbc, idea_cbc}`
* Added `X509CrlRef::extension`
* Added `X509PurposeId::CODE_SIGN`

### Changed

* `Pkey` HKDF functionality now works on LibreSSL
* `BigNum::mod_sqrt` is now available on all OpenSSLs
* `MessageDigest::sha3*` are now available on LibreSSL

## [v0.10.57] - 2023-08-27

### Added
* Added `X509VerifyParam::set_email`
* `Cipher::chacha20_poly1305` is now available on LibreSSL
* Added `CipherCtx::copy`

### Changed
* Updated `bitflags` dependecy to the 2.x series

## [v0.10.56] - 2023-08-06

## Added

* Added `BigNumRef::mod_sqrt`.
* Added `PkeyCtxRef::set_signature_md` and `PkeyCtxRef::set_rsa_pss_saltlen`.
* Added `PkeyCtxRef::verify_recover_init` and `PkeyCtxRef::verify_recover`.
* Added `BigNumRef::is_even` and `BigNumRef::is_odd`.
* Added `EcPointRef::to_hex_str` and `EcPoint::from_hex_str`.
* Added support for AES key wrap and wrap pad.

## [v0.10.55] - 2023-06-20

### Fixed

* Fixed compilation with the latest version of BoringSSL.
* Fixed compilation when OpenSSL is compiled with `OPENSSL_NO_OCB`.
* Fixed a segfault in `X509VerifyParamRef::set_host` when called with an empty string.

### Added

* Added `Deriver::set_peer_ex`.
* Added `EcGroupRef::asn1_flag`.
* Exposed `EcPointRef::affine_coordinates` on BoringSSL and LibreSSL.
* Added `Nid::SM2` and `Id::SM2`

## [v0.10.54] - 2023-05-31

### Fixed

* `PKey::private_key_to_pkcs8_passphrase` no longer panics if a `passphrase` contains a NUL byte.

## [v0.10.53] - 2023-05-30

### Added

* Added `Dsa::from_pqg`, `Dsa::generate_key`, and `Dsa::generate_params`.
* Added `SslRef::bytes_to_cipher_list`.
* Added `SubjectAlternativeName::other_name2`

## [v0.10.52] - 2023-04-24

### Added

* Added `DhRef::check_key`.
* Added `Id::POLY1305`.
* Added `X509Ref::subject_key_id`, `X509Ref::authority_key_id`, `X509Ref::authority_issuer`, and `X509Ref::authority_serial`.


## [v0.10.51] - 2023-04-20

### Added

* Added `X509RevokedRef::issuer_name` and `X509RevokedRef::reason_code`.
* Added `Dh::set_key` and `Dh::set_public_key`
* Added `Asn1OctetString` and `Asn1OctetStringRef1`
* Added `X509Extension::new_from_der`

### Deprecated

* Deprecated `X509Extension::new` and `X509Extension::new_nid` in favor of `X509Extension::new_from_der` and the `extensions` module.
* Deprecated `X509Extension::add_alias`, it is not required with `new_from_der` or the `extensions` module.

## [v0.10.50] - 2023-04-09

### Added

* Added `CipherCtxRef::cipher_update_inplace`.

## [v0.10.49] - 2023-04-01

### Fixed

* `SslConnector` no longer sets the SNI extension when connecting to an IP address.

### Added

* Implemented `Ord`, `PartialOrd`, `Eq`, and `PartialEq` for `Asn1Integer` and `Asn1IntegerRef`.
* Added `X509Ref::crl_distribution_points`, and `DistPoint`.

## [v0.10.48] - 2023-03-23

### Fixed

* Fixed injection vulnerabilities where OpenSSL's configuration mini-language could be used via `x509::extension::SubjectAlternativeName` and `x509::extension::ExtendedKeyUsage`. The mini-language can read arbitrary files amongst other things.
  * As part of fixing this `SubjectAlternativeName::dir_name` and `SubjectAlternativeName::other_name` are deprecated and their implementations always `panic!`. If you have a use case for these, please file an issue.
* Fixed several NULL pointer dereferences in OpenSSL that could be triggered via `x509::X509Extension::new` and `x509::X509Extension::new_nid`. Note that these methods still accept OpenSSL's configuration mini-language, and therefore should not be used with untrusted data.
* Fixed a data-race with `x509::X509Name` that are created with `x509::X509NameBuilder` and then used concurrently.
* Fixed LibreSSL version checking. More functions should now be correctly available on LibreSSL.

## [v0.10.47] - 2023-03-19

### Added

* Added support for X25519 and Ed25519 on LibreSSL and BoringSSL.
* Added `Error::library_code` and `Error::reason_code`.

## [v0.10.46] - 2023-03-14

### Fixed

* Fixed a potential null-pointer deref when parsing a PKCS#12 archive with no identity.
* Fixed builds against OpenSSL built with `no-cast`.
* Fixed debug formatting of `GeneralName`.

### Deprecated

* Deprecated `PKcs12Ref::parse` in favor of `Pkcs12Ref::parse2`.
* Deprecated `ParsedPkcs12` in favor of `ParsedPkcs12_2`.
* Deprecated `Pkcs12Builder::build` in favor of `Pkcs12Builder::build2`.

### Added

* Added `X509VerifyParamRef::set_auth_level`, `X509VerifyParamRef::auth_level`, and `X509VerifyParamRef::set_purpose`.
* Added `X509PurposeId` and `X509Purpose`.
* Added `X509NameBuilder::append_entry`.
* Added `PKeyRef::private_key_to_pkcs8`.
* Added `X509LookupRef::load_crl_file`.
* Added `Pkcs12Builder::name`, `Pkcs12Builder::pkey`, and `Pkcs12Builder::cert`.
* Added `SslRef::set_method`, `SslRef::set_private_key_file`, `SslRef::set_private_key`, `SslRef::set_certificate`, `SslRef::set_certificate_chain_file`, `SslRef::add_client_ca`, `SslRef::set_client_ca_list`, `SslRef::set_min_proto_version`, `SslREf::set_max_proto_version`, `SslRef::set_ciphersuites`, `SslRef::set_cipher_list`, `SslRef::set_verify_cert_store`.
* Added `X509NameRef::to_owned`.
* Added `SslContextBuilder::set_num_tickets`, `SslContextRef::num_tickets`, `SslRef::set_num_tickets`, and `SslRef::num_tickets`.
* Added `CmsContentInfo::verify`.

## [v0.10.45] - 2022-12-20

### Fixed

* Removed the newly added `CipherCtxRef::minimal_output_size` method, which did not work properly.
* Added `NO_DEPRECATED_3_0` cfg checks for more APIs.

### Added

* Added `SslRef::add_chain_cert`.
* Added `PKeyRef::security_bits`.
* Added `Provider::set_default_search_path`.
* Added `CipherCtxRef::cipher_final_unchecked`.

## [v0.10.44] - 2022-12-06

### Added

* Added `CipherCtxRef::num`, `CipherCtxRef::minimal_output_size`, and `CipherCtxRef::cipher_update_unchecked`.
* Improved output buffer size checks in `CipherCtxRef::cipher_update`.
* Added `X509Lookup::file` and `X509LookupRef::load_cert_file`.

## [v0.10.43] - 2022-11-23

### Added

* Added `Nid::BRAINPOOL_P256R1`, `Nid::BRAINPOOL_P384R1`, `Nid::BRAINPOOL_P512R1`.
* Added `BigNumRef::copy_from_slice`.
* Added `Cipher` constructors for Camellia, CAST5, and IDEA ciphers.
* Added `DsaSig`.
* Added `X509StoreBuilderRef::set_param`.
* Added `X509VerifyParam::new`, `X509VerifyParamRef::set_time`, and `X509VerifyParamRef::set_depth`.

## [v0.10.42] - 2022-09-26

### Added

* Added `SslRef::psk_identity_hint` and  `SslRef::psk_identity`.
* Added SHA-3 constants to `Nid`.
* Added `SslOptions::PRIORITIZE_CHACHA`.
* Added `X509ReqRef::to_text`.
* Added `MdCtxRef::size`.
* Added `X509NameRef::try_cmp`.
* Added `MdCtxRef::reset`.
* Added experimental, unstable support for BoringSSL.

### Fixed

* Fixed `MdCtxRef::digest_verify_init` to support `PKey`s with only public components.

## [v0.10.41] - 2022-06-09

### Fixed

* Fixed a use-after-free in `Error::function` and `Error::file` with OpenSSL 3.x.

### Added

* Added `MessageDigest::block_size` and `MdRef::block_size`.
* Implemented `Ord` and `Eq` for `X509` and `X509Ref`.
* Added `X509Extension::add_alias`.
* Added SM4 support.
* Added `EcGroup::from_components` `EcGropuRef::set_generator`, and `EcPointRef::set_affine_coordinates_gfp`.

## [v0.10.40] - 2022-05-04

### Fixed

* Fixed the openssl-sys dependency version.

## [v0.10.39] - 2022-05-02

### Deprecated

* Deprecated `SslContextBuilder::set_tmp_ecdh_callback` and `SslRef::set_tmp_ecdh_callback`.

### Added

* Added `SslRef::extms_support`.
* Added `Nid::create`.
* Added `CipherCtx`, which exposes a more direct interface to `EVP_CIPHER_CTX`.
* Added `PkeyCtx`, which exposes a more direct interface to `EVP_PKEY_CTX`.
* Added `MdCtx`, which exposes a more direct interface to `EVP_MD_CTX`.
* Added `Pkcs12Builder::mac_md`.
* Added `Provider`.
* Added `X509Ref::issuer_name_hash`.
* Added `Decrypter::set_rsa_oaep_label`.
* Added `X509Ref::to_text`.

## [v0.10.38] - 2021-10-31

### Added

* Added `Pkey::ec_gen`.

## [v0.10.37] - 2021-10-27

### Fixed

* Fixed linkage against OpenSSL distributions built with `no-chacha`.

### Added

* Added `BigNumRef::to_vec_padded`.
* Added `X509Name::from_der` and `X509NameRef::to_der`.
* Added `BigNum::new_secure`, `BigNumReef::set_const_time`, `BigNumref::is_const_time`, and `BigNumRef::is_secure`.

## [v0.10.36] - 2021-08-17

### Added

* Added `Asn1Object::as_slice`.
* Added `PKeyRef::{raw_public_key, raw_private_key, private_key_to_pkcs8_passphrase}` and
    `PKey::{private_key_from_raw_bytes, public_key_from_raw_bytes}`.
* Added `Cipher::{seed_cbc, seed_cfb128, seed_ecb, seed_ofb}`.

## [v0.10.35] - 2021-06-18

### Fixed

* Fixed a memory leak in `Deriver`.

### Added

* Added support for OpenSSL 3.x.x.
* Added `SslStream::peek`.

## [v0.10.34] - 2021-04-28

### Added

* Added `Dh::set_private_key` and `DhRef::private_key`.
* Added `EcPointRef::affine_coordinates`.
* Added `TryFrom` implementations to convert between `PKey` and specific key types.
* Added `X509StoreBuilderRef::set_flags`.

## [v0.10.33] - 2021-03-13

### Fixed

* `Dh::generate_params` now uses `DH_generate_params_ex` rather than the deprecated `DH_generated_params` function.

### Added

* Added `Asn1Type`.
* Added `CmsContentInfoRef::decrypt_without_cert_check`.
* Added `EcPointRef::{is_infinity, is_on_curve}`.
* Added `Encrypter::set_rsa_oaep_label`.
* Added `MessageDigest::sm3`.
* Added `Pkcs7Ref::signers`.
* Added `Cipher::nid`.
* Added `X509Ref::authority_info` and `AccessDescription::{method, location}`.
* Added `X509NameBuilder::{append_entry_by_text_with_type, append_entry_by_nid_with_type}`.

## [v0.10.32] - 2020-12-24

### Fixed

* Fixed `Ssl::new` to take a `&SslContextRef` rather than `&SslContext`.

### Added

* Added the `encrypt` module to support asymmetric encryption and decryption with `PKey`s.
* Added `MessageDigest::from_name`.
* Added `ConnectConfiguration::into_ssl`.
* Added the ability to create unconnected `SslStream`s directly from an `Ssl` and transport stream
    without performing any part of the handshake with `SslStream::new`.
* Added `SslStream::{read_early_data, write_early_data, connect, accept, do_handshake, stateless}`.
* Implemented `ToOwned` for `SslContextRef`.
* Added `SslRef::{set_connect_state, set_accept_state}`.

### Deprecated

* Deprecated `SslStream::from_raw_parts` in favor of `Ssl::from_ptr` and `SslStream::new`.
* Deprecated `SslStreamBuilder` in favor of methods on `Ssl` and `SslStream`.

## [v0.10.31] - 2020-12-09

### Added

* Added `Asn1Object::from_str`.
* Added `Dh::from_pgq`, `DhRef::prime_p`, `DhRef::prime_q`, `DhRef::generator`, `DhRef::generate_params`,
    `DhRef::generate_key`, `DhRef::public_key`, and `DhRef::compute_key`.
* Added `Pkcs7::from_der` and `Pkcs7Ref::to_der`.
* Added `Id::X25519`, `Id::X448`, `PKey::generate_x25519`, and `PKey::generate_x448`.
* Added `SrtpProfileId::SRTP_AEAD_AES_128_GCM` and `SrtpProfileId::SRTP_AEAD_AES_256_GCM`.
* Added `SslContextBuilder::verify_param` and `SslContextBuilder::verify_param_mut`.
* Added `X509Ref::subject_name_hash` and `X509Ref::version`.
* Added `X509StoreBuilderRef::add_lookup`, and the `X509Lookup` type.
* Added `X509VerifyFlags`, `X509VerifyParamRef::set_flags`, `X509VerifyParamRef::clear_flags`
    `X509VerifyParamRef::get_flags`.

## [v0.10.30] - 2020-06-25

### Fixed

* `DsaRef::private_key_to_pem` can no longer be called without a private key.

### Changed

* Improved the `Debug` implementations of many types.

### Added

* Added `is_empty` implementations for `Asn1StringRef` and `Asn1BitStringRef`.
* Added `EcPointRef::{to_pem, to_dir}` and `EcKeyRef::{public_key_from_pem, public_key_from_der}`.
* Added `Default` implementations for many types.
* Added `Debug` implementations for many types.
* Added `SslStream::from_raw_parts`.
* Added `SslRef::set_mtu`.
* Added `Cipher::{aes_128_ocb, aes_192_ocb, aes_256_ocb}`.

### Deprecated

* Deprecated `SslStreamBuilder::set_dtls_mtu_size` in favor of `SslRef::set_mtu`.

## [v0.10.29] - 2020-04-07

### Fixed

* Fixed a memory leak in `X509Builder::append_extension`.

### Added

* Added `SslConnector::into_context` and `SslConnector::context`.
* Added `SslAcceptor::into_context` and `SslAcceptor::context`.
* Added `SslMethod::tls_client` and `SslMethod::tls_server`.
* Added `SslContextBuilder::set_cert_store`.
* Added `SslContextRef::verify_mode` and `SslRef::verify_mode`.
* Added `SslRef::is_init_finished`.
* Added `X509Object`.
* Added `X509StoreRef::objects`.

## [v0.10.28] - 2020-02-04

### Fixed

* Fixed the mutability of `Signer::sign_oneshot` and `Verifier::verify_oneshot`. This is unfortunately a breaking
    change, but a necessary soundness fix.

## [v0.10.27] - 2020-01-29

### Added

* Added `MessageDigest::null`.
* Added `PKey::private_key_from_pkcs8`.
* Added `SslOptions::NO_RENEGOTIATION`.
* Added `SslStreamBuilder::set_dtls_mtu_size`.

## [v0.10.26] - 2019-11-22

### Fixed

* Fixed improper handling of the IV buffer in `envelope::{Seal, Unseal}`.

### Added

* Added `Asn1TimeRef::{diff, compare}`.
* Added `Asn1Time::from_unix`.
* Added `PartialEq` and `PartialOrd` implementations for `Asn1Time` and `Asn1TimeRef`.
* Added `base64::{encode_block, decode_block}`.
* Added `EcGroupRef::order_bits`.
* Added `Clone` implementations for `Sha1`, `Sha224`, `Sha256`, `Sha384`, and `Sha512`.
* Added `SslContextBuilder::{set_sigalgs_list, set_groups_list}`.

## [v0.10.25] - 2019-10-02

### Fixed

* Fixed a memory leak in `EcdsaSig::from_private_components` when using OpenSSL 1.0.x.

### Added

* Added support for Ed25519 and Ed448 keys.
* Implemented `ToOwned` for `PKeyRef` and `Clone` for `PKey`.

## [v0.10.24] - 2019-07-19

### Fixed

* Worked around an OpenSSL 1.0.x bug triggered by code calling `SSL_set_app_data`.

### Added

* Added `aes::{wrap_key, unwrap_key}`.
* Added `CmsContentInfoRef::to_pem` and `CmsContentInfo::from_pem`.
* Added `DsaRef::private_key_to_pem`.
* Added `EcGroupRef::{cofactor, generator}`.
* Added `EcPointRef::to_owned`.
* Added a `Debug` implementation for `EcKey`.
* Added `SslAcceptor::{mozilla_intermediate_v5, mozilla_modern_v5}`.
* Added `Cipher::{aes_128_ofb, aes_192_ecb, aes_192_cbc, aes_192_ctr, aes_192_cfb1, aes_192_cfb128, aes_192_cfb8,
    aes_192_gcm, aes_192_ccm, aes_192_ofb, aes_256_ofb}`.

## [v0.10.23] - 2019-05-18

### Fixed

* Fixed session callbacks when an `Ssl`'s context is replaced.

### Added

* Added `SslContextBuilder::add_client_ca`.

## [v0.10.22] - 2019-05-08

### Added

* Added support for the LibreSSL 2.9.x series.

## [v0.10.21] - 2019-04-30

### Fixed

* Fixed overly conservatifve buffer size checks in `Crypter` when using stream ciphers.

### Added

* Added bindings to envelope encryption APIs.
* Added `PkeyRef::size`.

## [v0.10.20] - 2019-03-20

### Added

* Added `CmsContentInfo::from_der` and `CmsContentInfo::encrypt`.
* Added `X509Ref::verify` and `X509ReqRef::verify`.
* Implemented `PartialEq` and `Eq` for `MessageDigest`.
* Added `MessageDigest::type_` and `EcGroupRef::curve_name`.

## [v0.10.19] - 2019-03-01

### Added

* The openssl-sys build script now logs the values of environment variables.
* Added `ERR_PACK` to openssl-sys.
* The `ERR_*` functions in openssl-sys are const functions when building against newer Rust versions.
* Implemented `Clone` for `Dsa`.
* Added `SslContextRef::add_session` and `SslContextRef::remove_session`.
* Added `SslSessionRef::time`, `SslSessionRef::timeout`, and `SslSessionRef::protocol_version`.
* Added `SslContextBuilder::set_session_cache_size` and `SslContextRef::session_cache_size`.

## [v0.10.18] - 2019-02-22

### Fixed

* Fixed the return type of `ssl::cipher_name`.

## [v0.10.17] - 2019-02-22

### Added

* Implemented `AsRef<str>` and `AsRef<[u8]>` for `OpenSslString`.
* Added `Asn1Integer::from_bn`.
* Added `RsaRef::check_key`.
* Added `Asn1Time::from_str` and `Asn1Time::from_str_x509`.
* Added `Rsa::generate_with_e`.
* Added `Cipher::des_ede3_cfb64`.
* Added `SslCipherRef::standard_name` and `ssl::cipher_name`.

## [v0.10.16] - 2018-12-16

### Added

* Added SHA3 and SHAKE to `MessageDigest`.
* Added `rand::keep_random_devices_open`.
* Added support for LibreSSL 2.9.0.

## [v0.10.15] - 2018-10-22

### Added

* Implemented `DoubleEndedIterator` for stack iterators.

## [v0.10.14] - 2018-10-18

### Fixed

* Made some accidentally exposed internal functions private.

### Added

* Added support for LibreSSL 2.8.

### Changed

* The OpenSSL version used with the `vendored` feature has been upgraded from 1.1.0 to 1.1.1.

## [v0.10.13] - 2018-10-14

### Fixed

* Fixed a double-free in the `SslContextBuilder::set_get_session_callback` API.

### Added

* Added `SslContextBuilder::set_client_hello_callback`.
* Added support for LibreSSL 2.8.1.
* Added `EcdsaSig::from_der` and `EcdsaSig::to_der`.
* Added PKCS#7 support.

## [v0.10.12] - 2018-09-13

### Fixed

* Fixed handling of SNI callbacks during renegotiation.

### Added

* Added `SslRef::get_shutdown` and `SslRef::set_shutdown`.
* Added support for SRTP in DTLS sessions.
* Added support for LibreSSL 2.8.0.

## [v0.10.11] - 2018-08-04

### Added

* The new `vendored` cargo feature will cause openssl-sys to compile and statically link to a
    vendored copy of OpenSSL.
* Added `SslContextBuilder::set_psk_server_callback`.
* Added `DsaRef::pub_key` and `DsaRef::priv_key`.
* Added `Dsa::from_private_components` and `Dsa::from_public_components`.
* Added `X509NameRef::entries`.

### Deprecated

* `SslContextBuilder::set_psk_callback` has been renamed to
    `SslContextBuilder::set_psk_client_callback` and deprecated.

## [v0.10.10] - 2018-06-06

### Added

* Added `SslRef::set_alpn_protos`.
* Added `SslContextBuilder::set_ciphersuites`.

## [v0.10.9] - 2018-06-01

### Fixed

* Fixed a use-after-free in `CmsContentInfo::sign`.
* `SslRef::servername` now returns `None` rather than panicking on a non-UTF8 name.

### Added

* Added `MessageDigest::from_nid`.
* Added `Nid::signature_algorithms`, `Nid::long_name`, and `Nid::short_name`.
* Added early data and early keying material export support for TLS 1.3.
* Added `SslRef::verified_chain`.
* Added `SslRef::servername_raw` which returns a `&[u8]` rather than `&str`.
* Added `SslRef::finished` and `SslRef::peer_finished`.
* Added `X509Ref::digest` to replace `X509Ref::fingerprint`.
* `X509StoreBuilder` and `X509Store` now implement `Sync` and `Send`.

### Deprecated

* `X509Ref::fingerprint` has been deprecated in favor of `X509Ref::digest`.

## [v0.10.8] - 2018-05-20

### Fixed

* `openssl-sys` will now detect Homebrew-installed OpenSSL when installed to a non-default
    directory.
* The `X509_V_ERR_INVALID_CALL`, `X509_V_ERR_STORE_LOOKUP`, and
    `X509_V_ERR_PROXY_SUBJECT_NAME_VIOLATION` constants in `openssl-sys` are now only present when
    building against 1.1.0g and up rather than 1.1.0.
* `SslContextBuilder::max_proto_version` and `SslContextBuilder::min_proto_version` are only present
    when building against 1.1.0g and up rather than 1.1.0.

### Added

* Added `CmsContentInfo::sign`.
* Added `Clone` and `ToOwned` implementations to `Rsa` and `RsaRef` respectively.
* The `min_proto_version` and `max_proto_version` methods are available when linking against
    LibreSSL 2.6.1 and up in addition to OpenSSL.
* `X509VerifyParam` is available when linking against LibreSSL 2.6.1 and up in addition to OpenSSL.
* ALPN support is available when linking against LibreSSL 2.6.1 and up in addition to OpenSSL.
* `Stack` and `StackRef` are now `Sync` and `Send`.

## [v0.10.7] - 2018-04-30

### Added

* Added `X509Req::public_key` and `X509Req::extensions`.
* Added `RsaPrivateKeyBuilder` to allow control over initialization of optional components of an RSA
    private key.
* Added DER encode/decode support to `SslSession`.
* openssl-sys now provides the `DEP_OPENSSL_VERSION_NUMBER` and
    `DEP_OPENSSL_LIBRESSL_VERSION_NUMBER` environment variables to downstream build scripts which
    contains the hex-encoded version number of the OpenSSL or LibreSSL distribution being built
    against. The other variables are deprecated.

## [v0.10.6] - 2018-03-05

### Added

* Added `SslOptions::ENABLE_MIDDLEBOX_COMPAT`.
* Added more `Sync` and `Send` implementations.
* Added `PKeyRef::id`.
* Added `Padding::PKCS1_PSS`.
* Added `Signer::set_rsa_pss_saltlen`, `Signer::set_rsa_mgf1_md`, `Signer::set_rsa_pss_saltlen`, and
    `Signer::set_rsa_mgf1_md`
* Added `X509StoreContextRef::verify` to directly verify certificates.
* Added low level ECDSA support.
* Added support for TLSv1.3 custom extensions. (OpenSSL 1.1.1 only)
* Added AES-CCM support.
* Added `EcKey::from_private_components`.
* Added CMAC support.
* Added support for LibreSSL 2.7.
* Added `X509Ref::serial_number`.
* Added `Asn1IntegerRef::to_bn`.
* Added support for TLSv1.3 stateless handshakes. (OpenSSL 1.1.1 only)

### Changed

* The Cargo features previously used to gate access to version-specific OpenSSL APIs have been
    removed. Those APIs will be available automatically when building against an appropriate OpenSSL
    version.
* Fixed `PKey::private_key_from_der` to return a `PKey<Private>` rather than a `PKey<Public>`. This
    is technically a breaking change but the function was pretty useless previously.

### Deprecated

* `X509CheckFlags::FLAG_NO_WILDCARDS` has been renamed to `X509CheckFlags::NO_WILDCARDS` and the old
    name deprecated.

## [v0.10.5] - 2018-02-28

### Fixed

* `ErrorStack`'s `Display` implementation no longer writes an empty string if it contains no errors.

### Added

* Added `SslRef::version2`.
* Added `Cipher::des_ede3_cbc`.
* Added `SslRef::export_keying_material`.
* Added the ability to push an `Error` or `ErrorStack` back onto OpenSSL's error stack. Various
    callback bindings use this to propagate errors properly.
* Added `SslContextBuilder::set_cookie_generate_cb` and `SslContextBuilder::set_cookie_verify_cb`.
* Added `SslContextBuilder::set_max_proto_version`, `SslContextBuilder::set_min_proto_version`,
    `SslContextBuilder::max_proto_version`, and `SslContextBuilder::min_proto_version`.

### Changed

* Updated `SslConnector`'s default cipher list to match Python's.

### Deprecated

* `SslRef::version` has been deprecated. Use `SslRef::version_str` instead.

## [v0.10.4] - 2018-02-18

### Added

* Added OpenSSL 1.1.1 support.
* Added `Rsa::public_key_from_pem_pkcs1`.
* Added `SslOptions::NO_TLSV1_3`. (OpenSSL 1.1.1 only)
* Added `SslVersion`.
* Added `SslSessionCacheMode` and `SslContextBuilder::set_session_cache_mode`.
* Added `SslContextBuilder::set_new_session_callback`,
    `SslContextBuilder::set_remove_session_callback`, and
    `SslContextBuilder::set_get_session_callback`.
* Added `SslContextBuilder::set_keylog_callback`. (OpenSSL 1.1.1 only)
* Added `SslRef::client_random` and `SslRef::server_random`. (OpenSSL 1.1.0+ only)

### Fixed

* The `SslAcceptorBuilder::mozilla_modern` constructor now disables TLSv1.0 and TLSv1.1 in
    accordance with Mozilla's recommendations.

## [v0.10.3] - 2018-02-12

### Added

* OpenSSL is now automatically detected on FreeBSD systems.
* Added `GeneralName` accessors for `rfc822Name` and `uri` variants.
* Added DES-EDE3 support.

### Fixed

* Fixed a memory leak in `X509StoreBuilder::add_cert`.

## [v0.10.2] - 2018-01-11

### Added

* Added `ConnectConfiguration::set_use_server_name_indication` and
    `ConnectConfiguration::set_verify_hostname` for use in contexts where you don't have ownership
    of the `ConnectConfiguration`.

## [v0.10.1] - 2018-01-10

### Added

* Added a `From<ErrorStack> for ssl::Error` implementation.

## [v0.10.0] - 2018-01-10

### Compatibility

* openssl 0.10 still uses openssl-sys 0.9, so openssl 0.9 and 0.10 can coexist without issue.

### Added

* The `ssl::select_next_proto` function can be used to easily implement the ALPN selection callback
    in a "standard" way.
* FIPS mode support is available in the `fips` module.
* Accessors for the Issuer and Issuer Alternative Name fields of X509 certificates have been added.
* The `X509VerifyResult` can now be set in the certificate verification callback via
    `X509StoreContextRef::set_error`.

### Changed

* All constants have been moved to associated constants of their type. For example, `bn::MSB_ONE`
    is now `bn::MsbOption::ONE`.
* Asymmetric key types are now parameterized over what they contain. In OpenSSL, the same type is
    used for key parameters, public keys, and private keys. Unfortunately, some APIs simply assume
    that certain components are present and will segfault trying to use things that aren't there.

    The `pkey` module contains new tag types named `Params`, `Public`, and `Private`, and the
    `Dh`, `Dsa`, `EcKey`, `Rsa`, and `PKey` have a type parameter set to one of those values. This
    allows the `Signer` constructor to indicate that it requires a private key at compile time for
    example. Previously, `Signer` would simply segfault if provided a key without private
    components.
* ALPN support has been changed to more directly model OpenSSL's own APIs. Instead of a single
    method used for both the server and client sides which performed everything automatically, the
    `SslContextBuilder::set_alpn_protos` and `SslContextBuilder::set_alpn_select_callback` handle
    the client and server sides respectively.
* `SslConnector::danger_connect_without_providing_domain_for_certificate_verification_and_server_name_indication`
    has been removed in favor of new methods which provide more control. The
    `ConnectConfiguration::use_server_name_indication` method controls the use of Server Name
    Indication (SNI), and the `ConnectConfiguration::verify_hostname` method controls the use of
    hostname verification. These can be controlled independently, and if both are disabled, the
    domain argument to `ConnectConfiguration::connect` is ignored.
* Shared secret derivation is now handled by the new `derive::Deriver` type rather than
    `pkey::PKeyContext`, which has been removed.
* `ssl::Error` is now no longer an enum, and provides more direct access to the relevant state.
* `SslConnectorBuilder::new` has been moved and renamed to `SslConnector::builder`.
* `SslAcceptorBuilder::mozilla_intermediate` and `SslAcceptorBuilder::mozilla_modern` have been
    moved to `SslAcceptor` and no longer take the private key and certificate chain. Install those
    manually after creating the builder.
* `X509VerifyError` is now `X509VerifyResult` and can now have the "ok" value in addition to error
    values.
* `x509::X509FileType` is now `ssl::SslFiletype`.
* Asymmetric key serialization and deserialization methods now document the formats that they
    correspond to, and some have been renamed to better indicate that.

### Removed

* All deprecated APIs have been removed.
* NPN support has been removed. It has been supersceded by ALPN, and is hopefully no longer being
    used in practice. If you still depend on it, please file an issue!
* `SslRef::compression` has been removed.
* Some `ssl::SslOptions` flags have been removed as they no longer do anything.

## Older

Look at the [release tags] for information about older releases.

[Unreleased]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.73...master
[v0.10.73]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.72...openssl-v0.10.73
[v0.10.72]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.71...openssl-v0.10.72
[v0.10.71]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.70...openssl-v0.10.71
[v0.10.70]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.69...openssl-v0.10.70
[v0.10.69]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.68...openssl-v0.10.69
[v0.10.68]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.67...openssl-v0.10.68
[v0.10.67]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.66...openssl-v0.10.67
[v0.10.66]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.65...openssl-v0.10.66
[v0.10.65]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.64...openssl-v0.10.65
[v0.10.64]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.63...openssl-v0.10.64
[v0.10.63]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.62...openssl-v0.10.63
[v0.10.62]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.61...openssl-v0.10.62
[v0.10.61]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.60...openssl-v0.10.61
[v0.10.60]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.59...openssl-v0.10.60
[v0.10.59]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.58...openssl-v0.10.59
[v0.10.58]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.57...openssl-v0.10.58
[v0.10.57]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.56...openssl-v0.10.57
[v0.10.56]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.55...openssl-v0.10.56
[v0.10.55]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.54...openssl-v0.10.55
[v0.10.54]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.53...openssl-v0.10.54
[v0.10.53]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.52...openssl-v0.10.53
[v0.10.52]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.51...openssl-v0.10.52
[v0.10.51]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.50...openssl-v0.10.51
[v0.10.50]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.49...openssl-v0.10.50
[v0.10.49]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.48...openssl-v0.10.49
[v0.10.48]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.47...openssl-v0.10.48
[v0.10.47]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.46...openssl-v0.10.47
[v0.10.46]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.45...openssl-v0.10.46
[v0.10.45]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.44...openssl-v0.10.45
[v0.10.44]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.43...openssl-v0.10.44
[v0.10.43]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.42...openssl-v0.10.43
[v0.10.42]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.41...openssl-v0.10.42
[v0.10.41]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.40...openssl-v0.10.41
[v0.10.40]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.39...openssl-v0.10.40
[v0.10.39]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.38...openssl-v0.10.39
[v0.10.38]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.37...openssl-v0.10.38
[v0.10.37]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.36...openssl-v0.10.37
[v0.10.36]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.35...openssl-v0.10.36
[v0.10.35]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.34...openssl-v0.10.35
[v0.10.34]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.33...openssl-v0.10.34
[v0.10.33]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.32...openssl-v0.10.33
[v0.10.32]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.31...openssl-v0.10.32
[v0.10.31]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.30...openssl-v0.10.31
[v0.10.30]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.29...openssl-v0.10.30
[v0.10.29]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.28...openssl-v0.10.29
[v0.10.28]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.27...openssl-v0.10.28
[v0.10.27]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.26...openssl-v0.10.27
[v0.10.26]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.25...openssl-v0.10.26
[v0.10.25]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.24...openssl-v0.10.25
[v0.10.24]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.23...openssl-v0.10.24
[v0.10.23]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.22...openssl-v0.10.23
[v0.10.22]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.21...openssl-v0.10.22
[v0.10.21]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.20...openssl-v0.10.21
[v0.10.20]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.19...openssl-v0.10.20
[v0.10.19]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.18...openssl-v0.10.19
[v0.10.18]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.17...openssl-v0.10.18
[v0.10.17]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.16...openssl-v0.10.17
[v0.10.16]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.15...openssl-v0.10.16
[v0.10.15]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.14...openssl-v0.10.15
[v0.10.14]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.13...openssl-v0.10.14
[v0.10.13]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.12...openssl-v0.10.13
[v0.10.12]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.11...openssl-v0.10.12
[v0.10.11]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.10...openssl-v0.10.11
[v0.10.10]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.9...openssl-v0.10.10
[v0.10.9]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.8...openssl-v0.10.9
[v0.10.8]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.7...openssl-v0.10.8
[v0.10.7]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.6...openssl-v0.10.7
[v0.10.6]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.5...openssl-v0.10.6
[v0.10.5]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.4...openssl-v0.10.5
[v0.10.4]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.3...openssl-v0.10.4
[v0.10.3]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.2...openssl-v0.10.3
[v0.10.2]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.1...openssl-v0.10.2
[v0.10.1]: https://github.com/sfackler/rust-openssl/compare/openssl-v0.10.0...openssl-v0.10.1
[v0.10.0]: https://github.com/sfackler/rust-openssl/compare/v0.9.23...openssl-v0.10.0
[release tags]: https://github.com/sfackler/rust-openssl/releases
