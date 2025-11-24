Behaviour of SSL functions on QUIC SSL objects
==============================================

This document is a companion to the [QUIC API Overview](./quic-api.md) which
lists all SSL functions and controls and notes their behaviour with QUIC SSL
objects.

The Category column is as follows:

- **Global**:
  These API items do not relate to SSL objects. They may be stateless or may
  relate only to global state.

  Can also be used for APIs implemented only in terms of other public libssl APIs.
- **Object**:
  Object management APIs. Some of these may require QUIC-specific implementation.
- **HL**: Handshake layer API.

  These calls should generally be dispatched to the handshake layer, unless
  they are not applicable to QUIC. Modifications inside the handshake layer
  for the QUIC case may or may not be required.
- **CSSM**: Connection/Stream State Machine. API related to lifecycle of a
  connection or stream. Needs QUIC-specific implementation.
- **ADP**: App Data Path. Application-side data path API. QUIC-specific
  implementation.
- **NDP**: Net Data Path. Network-side data path control API. Also includes I/O
  ticking and timeout handling.
- **RL**: Record layer related API. If these API items only relate to the TLS
  record layer, they must be disabled for QUIC; if they are also relevant to the
  QUIC record layer, they will require QUIC-specific implementation.
- **Async**: Relates to the async functionality.
- **0-RTT**: Relates to early data/0-RTT functionality.
- **Special**: Other calls which defy classification.

The Semantics column is as follows:

- **🟩U**: Unchanged. The semantics of the API are not changed for QUIC.
- **🟧C**: Changed. The semantics of the API are changed for QUIC.
- **🟦N**: New. The API is new for QUIC.
- **🟥TBD**: Yet to be determined if semantic changes will be required.

The Applicability column is as follows:

- **🟦U**: Unrelated. Not applicable to QUIC — fully unrelated (e.g. functions for
  other SSL methods).
- **🟥FC**: Not applicable to QUIC (or not currently supported) — fail closed.
- **🟧NO**: Not applicable to QUIC (nor not currently supported) — no-op.
- **🟩A**: Applicable.

The Implementation Requirements column is as follows:

- **🟩NC**: No changes are expected to be needed (where marked **\***, dispatch
  to handshake layer).

  **Note**: Where this value is used with an applicability of **FC** or **NO**,
  this means that the desired behaviour is already an emergent consequence of the
  existing code.
- **🟨C**: Modifications are expected to be needed (where marked **\***,
  dispatch to handshake layer with changes inside the handshake layer).
- **🟧QSI**: QUIC specific implementation.
- **🟥QSA**: QUIC specific API.

The Status column is as follows:

- **🔴Pending Triage**: Have not determined the classification of this API item yet.
- **🟠Design TBD**: It has not yet been determined how this API item will work for
  QUIC.
- **🟡TODO**: It has been determined how this API item should work for QUIC but it
  has not yet been implemented.
- **🟢Done**: No further work is anticipated to be needed for this API item.

Notes:

- †1: Must restrict which ciphers can be used with QUIC; otherwise, no changes.
- †2: ALPN usage must be mandated; otherwise, no changes.
- †3: NPN usage should be forced off as it should never be used with QUIC;
  otherwise, no changes.
- †4: Controls needing changes are listed separately.
- †5: TLS compression and renegotiation must not be used with QUIC, but these
  features are already forbidden in
  TLS 1.3, which is a requirement for QUIC, thus no changes should be needed.
- †6: Callback specified is called for handshake layer messages (TLSv1.3).
- †7: Tickets are issued using `NEW_TOKEN` frames in QUIC and this will
  require handshake layer changes. However these APIs as such do not require
  changes.
- †8: Use of post-handshake authentication is prohibited by QUIC.
- †9: QUIC always uses AES-128-GCM initially. We need to determine when and
  what ciphers we report as being in use.
- †10: Not supporting async for now.
- †11: Since these functions only configure cipher suite lists used for TLSv1.2,
  which is never used for QUIC, they do not require changes, and we can allow
  applications to configure these lists freely, as they will be ignored.

| API Item                                     | Cat.    | Sema. | Appl. | Impl. Req. | Status       |
|----------------------------------------------|---------|-------|-------|------------|--------------|
| **⇒ Global Information and Functions**       |         |       |       |            |              |
| `OSSL_default_cipher_list`                   | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `OSSL_default_ciphersuites`                  | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `ERR_load_SSL_strings`                       | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `OPENSSL_init_ssl`                           | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `OPENSSL_cipher_name`                        | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_alert_desc_string`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_alert_desc_string_long`                 | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_alert_type_string`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_alert_type_string_long`                 | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_extension_supported`                    | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_add_ssl_module`                         | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_test_functions`                         | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_select_next_proto`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| **⇒ Methods**                                |         |       |       |            |              |
| `SSLv3_method`                               | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSLv3_client_method`                        | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSLv3_server_method`                        | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLS_method`                                 | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLS_client_method`                          | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLS_server_method`                          | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_method`                               | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_client_method`                        | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_server_method`                        | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_1_method`                             | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_1_client_method`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_1_server_method`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_2_client_method`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_2_server_method`                      | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `TLSv1_2_method`                             | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLS_method`                                | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLS_client_method`                         | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLS_server_method`                         | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_client_method`                       | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_server_method`                       | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_method`                              | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_2_method`                            | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_2_client_method`                     | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLSv1_2_server_method`                     | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `OSSL_QUIC_client_method`                    | Global  | 🟩U   | 🟦U   | 🟥QSA      | 🟢Done       |
| `OSSL_QUIC_client_thread_method`             | Global  | 🟩U   | 🟦U   | 🟥QSA      | 🟢Done       |
| `OSSL_QUIC_server_method`                    | Global  | 🟩U   | 🟦U   | 🟥QSA      | 🟠Design TBD |
| **⇒ Instantiation**                          |         |       |       |            |              |
| `BIO_f_ssl`                                  | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `BIO_new_ssl`                                | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_new`                                | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_new_ex`                             | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_up_ref`                             | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_free`                               | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_new`                                    | Object  | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_dup`                                    | Object  | 🟩U   | 🟩A   | 🟥FC       | 🟢Done       |
| `SSL_up_ref`                                 | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_free`                                   | Object  | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_is_dtls`                                | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_get_ex_data`                        | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_CTX_set_ex_data`                        | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_get_ex_data`                            | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_set_ex_data`                            | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_get_SSL_CTX`                            | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_set_SSL_CTX`                            | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| **⇒ Method Manipulation**                    |         |       |       |            |              |
| `SSL_CTX_get_ssl_method`                     | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_get_ssl_method`                         | Object  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_set_ssl_method`                         | Object  | 🟩U   | 🟥FC  | 🟧QSI      | 🟢Done       |
| **⇒ SRTP**                                   |         |       |       |            |              |
| `SSL_get_selected_srtp_profile`              | HL      | 🟩U   | 🟧NO  | 🟨C\*      | 🟢Done       |
| `SSL_get_srtp_profiles`                      | HL      | 🟩U   | 🟧NO  | 🟨C\*      | 🟢Done       |
| `SSL_CTX_set_tlsext_use_srtp`                | HL      | 🟩U   | 🟥FC  | 🟨C\*      | 🟢Done       |
| `SSL_set_tlsext_use_srtp`                    | HL      | 🟩U   | 🟥FC  | 🟩NC\*     | 🟢Done       |
| **⇒ Ciphersuite Configuration**              |         |       |       |            |              |
| `SSL_CTX_set_cipher_list`                    | HL      | 🟩U   | 🟩A   | 🟩NC\* †11 | 🟢Done       |
| `SSL_CTX_set_ciphersuites`                   | HL      | 🟩U   | 🟩A   | 🟨C\* †1   | 🟢Done       |
| `SSL_CTX_get_ciphers`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_ciphersuites`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get1_supported_ciphers`                 | HL      | 🟩U   | 🟩A   | 🟨C\* †1   | 🟢Done       |
| `SSL_bytes_to_cipher_list`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_ciphers`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_cipher_list`                        | HL      | 🟩U   | 🟩A   | 🟩NC\* †11 | 🟢Done       |
| `SSL_set_cipher_list`                        | HL      | 🟩U   | 🟩A   | 🟩NC\* †11 | 🟢Done       |
| **⇒ Negotiated Ciphersuite Queries**         |         |       |       |            |              |
| `SSL_get_current_cipher`                     | HL      | 🟩U   | 🟩A   | 🟩NC\* †9  | 🟢Done       |
| `SSL_get_pending_cipher`                     | HL      | 🟩U   | 🟩A   | 🟩NC\* †9  | 🟢Done       |
| `SSL_get_shared_ciphers`                     | HL      | 🟩U   | 🟩A   | 🟩NC\* †9  | 🟢Done       |
| `SSL_get_client_ciphers`                     | HL      | 🟩U   | 🟩A   | 🟩NC\* †9  | 🟢Done       |
| `SSL_get_current_compression`                | HL      | 🟩U   | 🟩A   | 🟩HLNC     | 🟢Done       |
| `SSL_get_current_expansion`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_shared_sigalgs`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_sigalgs`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_peer_signature_nid`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_peer_signature_type_nid`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_signature_nid`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_signature_type_nid`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ ALPN**                                   | †2      |       |       |            |              |
| `SSL_SESSION_set1_alpn_selected`             | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| `SSL_SESSION_get0_alpn_selected`             | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| `SSL_CTX_set_alpn_select_cb`                 | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| `SSL_set_alpn_protos`                        | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| `SSL_get0_alpn_selected`                     | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| `SSL_CTX_set_alpn_protos`                    | HL      | 🟩U   | 🟩A   | 🟨C\* †2   | 🟢Done       |
| **⇒ NPN**                                    | †3      |       |       |            |              |
| `SSL_CTX_set_next_proto_select_cb`           | HL      | 🟩U   | 🟥FC  | 🟨C\* †3   | 🟢Done       |
| `SSL_CTX_set_next_protos_advertised_cb`      | HL      | 🟩U   | 🟥FC  | 🟨C\* †3   | 🟢Done       |
| `SSL_get0_next_proto_negotiated`             | HL      | 🟩U   | 🟥FC  | 🟩NC\* †3  | 🟢Done       |
| **⇒ Narrow Waist Interface**                 | †4      |       |       |            |              |
| `SSL_CTX_ctrl`                               | Object  | 🟩U   | 🟩A   | 🟩NC\* †4  | 🟢Done       |
| `SSL_ctrl`                                   | Object  | 🟩U   | 🟩A   | 🟩NC\* †4  | 🟢Done       |
| `SSL_CTX_callback_ctrl`                      | Object  | 🟩U   | 🟩A   | 🟩NC\* †4  | 🟢Done       |
| `SSL_callback_ctrl`                          | Object  | 🟩U   | 🟩A   | 🟩NC\* †4  | 🟢Done       |
| **⇒ Miscellaneous Accessors**                |         |       |       |            |              |
| `SSL_get_server_random`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_client_random`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_finished`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_peer_finished`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Ciphersuite Information**                |         |       |       |            |              |
| `SSL_CIPHER_description`                     | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_find`                            | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_auth_nid`                    | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_bits`                        | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_cipher_nid`                  | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_digest_nid`                  | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_handshake_digest`            | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_id`                          | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_kx_nid`                      | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_name`                        | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_protocol_id`                 | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_get_version`                     | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_is_aead`                         | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CIPHER_standard_name`                   | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_group_to_name`                          | Global  | 🟩U   | 🟦U   | 🟩NC\*     | 🟢Done       |
| **⇒ Version Queries**                        |         |       |       |            |              |
| `SSL_get_version`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_version`                                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_version`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Certificate Chain Management**           |         |       |       |            |              |
| `SSL_get_certificate`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_certificate`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_certificate_chain_file`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_certificate_chain_file`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_certificate_file`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_load_verify_file`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_load_verify_dir`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_load_verify_store`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_load_verify_locations`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `CertSSL_use_cert_and_key`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_certificate_ASN1`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_PrivateKey`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_PrivateKey_ASN1`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_PrivateKey_file`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_RSAPrivateKey`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_RSAPrivateKey_ASN1`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_use_RSAPrivateKey_file`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_verify_dir`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_verify_file`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_verify_paths`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_verify_store`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_cert_and_key`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_certificate`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_certificate_ASN1`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_certificate_file`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_PrivateKey`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_PrivateKey_ASN1`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_PrivateKey_file`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_RSAPrivateKey`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_RSAPrivateKey_ASN1`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_RSAPrivateKey_file`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_check_chain`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_check_private_key`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_check_private_key`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add_client_CA`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add1_to_CA_list`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add_dir_cert_subjects_to_stack`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add_file_cert_subjects_to_stack`        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add_store_cert_subjects_to_stack`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_load_client_CA_file`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_load_client_CA_file_ex`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_dup_CA_list`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set0_CA_list`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_CA_list`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_client_CA_list`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_add_client_CA`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_CA_list`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_certificate`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_privatekey`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_cert_store`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set1_cert_store`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_client_CA_list`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_add1_to_CA_list`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set0_CA_list`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_client_cert_cb`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_default_passwd_cb`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_default_passwd_cb_userdata`     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_client_CA_list`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_privatekey`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Certificate Compression**                |         |       |       |            |              |
| `SSL_CTX_set1_cert_comp_preference`          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set1_cert_comp_preference`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_compress_certs`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_compress_certs`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set1_compressed_cert`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set1_compressed_cert`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get1_compressed_cert`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get1_compressed_cert`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Certificate Verification**               |         |       |       |            |              |
| `SSL_set1_host`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_add1_host`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_hostflags`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_verify`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_verify`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_verify_depth`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_verify_result`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_verify_callback`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_verify_depth`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_verify_mode`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_verify_result`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_peer_CA_list`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_peer_certificate`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_verified_chain`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get1_peer_certificate`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_peer_cert_chain`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_peer_certificate`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_certs_clear`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_param`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_param`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_verify_mode`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_verify_depth`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_verify_depth`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_peername`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set1_param`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set1_param`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_param`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_param`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_purpose`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_purpose`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_trust`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_trust`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ PSK**                                    |         |       |       |            |              |
| `SSL_use_psk_identity_hint`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_psk_identity_hint`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_psk_client_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_psk_find_session_callback`          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_psk_server_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_psk_use_session_callback`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_psk_identity`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_psk_identity_hint`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ SRP**                                    |         |       |       |            |              |
| `SSL_SRP_CTX_init`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_SRP_CTX_init`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_SRP_CTX_free`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SRP_CTX_free`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_client_pwd_callback`        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_password`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_srp_g`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_cb_arg`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_srp_N`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_username_callback`          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_srp_username`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_srp_server_param`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_srp_userinfo`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_srp_server_param_with_username`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_strength`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_verify_param_callback`      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_srp_server_param_pw`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_srp_username`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SRP_Calc_A_param`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ DANE**                                   |         |       |       |            |              |
| `SSL_CTX_dane_enable`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_dane_tlsa`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_dane_set_flags`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_dane_set_flags`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_dane_clear_flags`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_dane_clear_flags`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_dane`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_dane_enable`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_dane_authority`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_dane_mtype_set`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_dane_tlsa_add`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Certificate Transparency**               |         |       |       |            |              |
| `SSL_CTX_enable_ct`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_ct_is_enabled`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_ctlog_list_file`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_ctlog_list_file`        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_ct_validation_callback`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set0_ctlog_store`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_ctlog_store`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_enable_ct`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_ct_is_enabled`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_peer_scts`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_ct_validation_callback`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Compression**                            |         |       |       |            |              |
| `SSL_COMP_add_compression_method`            | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| `SSL_COMP_get0_name`                         | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| `SSL_COMP_get_compression_methods`           | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| `SSL_COMP_get_id`                            | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| `SSL_COMP_get_name`                          | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| `SSL_COMP_set0_compression_methods`          | HL      | 🟩U   | 🟩A   | 🟩NC\* †5  | 🟢Done       |
| **⇒ Exporters**                              |         |       |       |            |              |
| `SSL_export_keying_material`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_export_keying_material_early`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Renegotiation**                          |         |       |       |            |              |
| `SSL_renegotiate`                            | HL      | 🟩U   | 🟥FC  | 🟩NC\* †5  | 🟢Done       |
| `SSL_renegotiate_abbreviated`                | HL      | 🟩U   | 🟥FC  | 🟩NC\* †5  | 🟢Done       |
| `SSL_renegotiate_pending`                    | HL      | 🟩U   | 🟧NO  | 🟩NC\* †5  | 🟢Done       |
| **⇒ Options**                                |         |       |       |            |              |
| `SSL_CTX_clear_options`                      | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_CTX_set_options`                        | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_CTX_get_options`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_clear_options`                          | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_set_options`                            | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_get_options`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Configuration**                          |         |       |       |            |              |
| `SSL_CONF_CTX_new`                           | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_free`                          | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_set_ssl`                       | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_set_ssl_ctx`                   | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_set1_prefix`                   | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_set_flags`                     | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_clear_flags`                   | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_CTX_finish`                        | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_cmd`                               | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_cmd_argv`                          | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CONF_cmd_value_type`                    | Global  | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_config`                                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_config`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Callbacks**                              |         |       |       |            |              |
| `SSL_CTX_set_cert_cb`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_cert_store`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_cert_verify_callback`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_client_CA_list`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_client_cert_cb`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_client_cert_engine`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_client_hello_cb`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_cookie_generate_cb`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_cookie_verify_cb`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_passwd_cb`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_passwd_cb_userdata`     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_default_read_buffer_len`        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_info_callback`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_info_callback`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_info_callback`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_info_callback`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_msg_callback`                       | HL      | 🟩U   | 🟩A   | 🟩NC\* †6  | 🟢Done       |
| `SSL_set_cert_cb`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_default_passwd_cb`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_default_passwd_cb_userdata`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_default_passwd_cb`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_default_passwd_cb_userdata`         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_keylog_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_keylog_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_psk_client_callback`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_psk_find_session_callback`      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_psk_server_callback`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_psk_use_session_callback`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_verify_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_not_resumable_session_callback` | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_not_resumable_session_callback`     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_session_secret_cb`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| **⇒ Session Management**                     |         |       |       |            |              |
| `d2i_SSL_SESSION`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `i2d_SSL_SESSION`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `PEM_read_bio_SSL_SESSION`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `PEM_read_SSL_SESSION`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `PEM_write_bio_SSL_SESSION`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `PEM_write_SSL_SESSION`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_new`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_up_ref`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_dup`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_free`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_print`                          | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_print_fp`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_print_keylog`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_cipher`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set_cipher`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_hostname`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set1_hostname`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_id_context`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set1_id_context`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_peer`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_ticket`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_ticket_appdata`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set1_ticket_appdata`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_has_ticket`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_protocol_version`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set_protocol_version`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_compress_id`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_id`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set1_id`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_time`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set_time`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_timeout`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set_timeout`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_ex_data`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set_ex_data`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get0_hostname`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_set1_hostname`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_master_key`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_master_key`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_is_resumable`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_max_early_data`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_max_early_data`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_max_fragment_length`        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_SESSION_get_ticket_lifetime_hint`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_add_session`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_remove_session`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get1_session`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_session`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_session`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_get_get_cb`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_set_get_cb`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_get_new_cb`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_set_new_cb`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_get_remove_cb`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sess_set_remove_cb`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_session_id_context`             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_session_id_context`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_generate_session_id`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_generate_session_id`            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_has_matching_session_id`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_flush_sessions`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_session_reused`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_timeout`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_timeout`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_default_timeout`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_sessions`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Session Ticket Management**              |         |       |       |            |              |
| `SSL_get_num_tickets`                        | HL      | 🟩U   | 🟩A   | 🟩NC\* †7  | 🟢Done       |
| `SSL_set_num_tickets`                        | HL      | 🟩U   | 🟩A   | 🟩NC\* †7  | 🟢Done       |
| `SSL_CTX_get_num_tickets`                    | HL      | 🟩U   | 🟩A   | 🟩NC\* †7  | 🟢Done       |
| `SSL_CTX_set_num_tickets`                    | HL      | 🟩U   | 🟩A   | 🟩NC\* †7  | 🟢Done       |
| `SSL_new_session_ticket`                     | HL      | 🟩U   | 🟩A   | 🟩NC\* †7  | 🟢Done       |
| `SSL_set_session_ticket_ext`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_session_ticket_ext_cb`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_tlsext_ticket_key_evp_cb`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Security Levels**                        |         |       |       |            |              |
| `SSL_CTX_get_security_level`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_security_level`                 | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_security_level`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_security_level`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get_security_callback`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_security_callback`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_security_callback`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_security_callback`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_get0_security_ex_data`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set0_security_ex_data`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get0_security_ex_data`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set0_security_ex_data`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Custom Extensions**                      |         |       |       |            |              |
| `SSL_CTX_add_custom_ext`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_add_client_custom_ext`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_add_server_custom_ext`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_has_client_custom_ext`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Early ClientHello Processing**           |         |       |       |            |              |
| `SSL_client_hello_get_extension_order`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_ciphers`              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_compression_methods`  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_ext`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_legacy_version`       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_random`               | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get0_session_id`           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_get1_extensions_present`   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_client_hello_isv2`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ SNI**                                    |         |       |       |            |              |
| `SSL_get_servername`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_servername_type`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Server Info**                            |         |       |       |            |              |
| `SSL_CTX_use_serverinfo`                     | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_serverinfo_ex`                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_use_serverinfo_file`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Post-Handshake Authentication**          |         |       |       |            |              |
| `SSL_verify_client_post_handshake`           | HL      | 🟩U   | 🟥FC  | 🟨C* †8    | 🟢Done       |
| `SSL_CTX_set_post_handshake_auth`            | HL      | 🟩U   | 🟥FC  | 🟨C* †8    | 🟢Done       |
| `SSL_set_post_handshake_auth`                | HL      | 🟩U   | 🟥FC  | 🟨C* †8    | 🟢Done       |
| **⇒ DH Parameters**                          |         |       |       |            |              |
| `SSL_CTX_set_dh_auto`                        | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_dh_auto`                            | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set0_tmp_dh_pkey`                   | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set0_tmp_dh_pkey`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_tmp_dh_callback`                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_tmp_dh_callback`                    | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_CTX_set_tmp_dh`                         | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_tmp_dh`                             | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ State Queries**                          |         |       |       |            |              |
| `SSL_in_init`                                | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_in_before`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_is_init_finished`                       | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_get_state`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_rstate_string`                          | HL      | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_rstate_string_long`                     | HL      | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_state_string`                           | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_state_string_long`                      | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Data Path and CSSM**                     |         |       |       |            |              |
| `SSL_set_connect_state`                      | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set_accept_state`                       | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_is_server`                              | CSSM    | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_peek`                                   | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_peek_ex`                                | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_read`                                   | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_read_ex`                                | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_write`                                  | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_write_ex`                               | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_sendfile`                               | ADP     | 🟩U   | 🟥FC  | 🟩NC\*     | 🟢Done       |
| `SSL_pending`                                | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_has_pending`                            | ADP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_accept`                                 | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_connect`                                | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_do_handshake`                           | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set0_wbio`                              | NDP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set0_rbio`                              | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set_bio`                                | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_get_wbio`                               | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_get_rbio`                               | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_get_error`                              | NDP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_get_rfd`                                | NDP     | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_get_wfd`                                | NDP     | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_get_fd`                                 | NDP     | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_set_rfd`                                | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set_wfd`                                | NDP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set_fd`                                 | NDP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_key_update`                             | RL      | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_get_key_update_type`                    | RL      | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_clear`  (connection)                    | CSSM    | 🟩U   | 🟥FC  | 🟧QSI      | 🟢Done       |
| `SSL_clear`  (stream)                        | CSSM    | 🟩U   | 🟥FC  | 🟧QSI      | 🟢Done       |
| `SSL_shutdown`                               | CSSM    | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_want`                                   | ADP     | 🟧C   | 🟩A   | 🟧QSI      | 🟢Done       |
| `BIO_new_ssl_connect`                        | Global  | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `BIO_new_buffer_ssl_connect`                 | Global  | 🟩U   | 🟦U   | 🟧QSI      | 🟢Done       |
| `SSL_get_shutdown`                           | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_set_shutdown`                           | CSSM    | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| **⇒ New APIs**                               |         |       |       |            |              |
| `SSL_is_tls`                                 | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_is_quic`                                | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_handle_events`                          | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_event_timeout`                      | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_blocking_mode`                      | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_set_blocking_mode`                      | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_rpoll_descriptor`                   | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_wpoll_descriptor`                   | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_net_read_desired`                       | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_net_write_desired`                      | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_set1_initial_peer_addr`                 | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_shutdown_ex`                            | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_stream_conclude`                        | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_stream_reset`                           | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_read_state`                  | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_write_state`                 | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_read_error_code`             | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_write_error_code`            | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_conn_close_info`                    | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_inject_net_dgram`                       | NDP     | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| **⇒ New APIs for Multi-Stream**              |         |       |       |            |              |
| `SSL_get0_connection`                        | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_is_connection`                          | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_id`                          | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_stream_type`                        | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_is_stream_local`                        | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_new_stream`                             | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_accept_stream`                          | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_get_accept_stream_queue_len`            | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_set_default_stream_mode`                | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| `SSL_set_incoming_stream_policy`             | CSSM    | 🟦N   | 🟩A   | 🟥QSA      | 🟢Done       |
| **⇒ Currently Not Supported**                |         |       |       |            |              |
| `SSL_copy_session_id`                        | Special | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `BIO_ssl_copy_session_id`                    | Special | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_set_quiet_shutdown`                 | CSSM    | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_CTX_get_quiet_shutdown`                 | CSSM    | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_set_quiet_shutdown`                     | CSSM    | 🟩U   | 🟥FC  | 🟨C        | 🟢Done       |
| `SSL_get_quiet_shutdown`                     | CSSM    | 🟩U   | 🟧NO  | 🟨C        | 🟢Done       |
| `SSL_CTX_set_ssl_version`                    | HL      | 🟩U   | 🟥FC  | 🟨C        | 🟢Done       |
| **⇒ Async**                                  |         |       |       |            |              |
| `SSL_CTX_set_async_callback`                 | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_set_async_callback`                     | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_CTX_set_async_callback_arg`             | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_set_async_callback_arg`                 | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_waiting_for_async`                      | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_get_async_status`                       | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_get_all_async_fds`                      | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| `SSL_get_changed_async_fds`                  | Async   | 🟩U   | 🟧NO  | 🟩NC* †10  | 🟢Done       |
| **⇒ Readahead**                              |         |       |       |            |              |
| `SSL_CTX_get_default_read_ahead`             | RL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_get_read_ahead`                     | RL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_read_ahead`                     | RL      | 🟩U   | 🟧NO  | 🟨C*       | 🟢Done       |
| `SSL_get_read_ahead`                         | RL      | 🟩U   | 🟧NO  | 🟨C*       | 🟢Done       |
| `SSL_set_read_ahead`                         | RL      | 🟩U   | 🟧NO  | 🟨C*       | 🟢Done       |
| `SSL_CTX_set_default_read_buffer_len`        | RL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_set_default_read_buffer_len`            | RL      | 🟩U   | 🟧NO  | 🟨C*       | 🟢Done       |
| **⇒ Record Padding and Fragmentation**       |         |       |       |            |              |
| `SSL_CTX_set_record_padding_callback`        | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_record_padding_callback`            | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_get_record_padding_callback_arg`    | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_record_padding_callback_arg`    | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_get_record_padding_callback_arg`        | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_record_padding_callback_arg`        | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_block_padding`                  | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_block_padding`                      | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_set_tlsext_max_fragment_length`     | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_tlsext_max_fragment_length`         | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| **⇒ Stateless/HelloRetryRequest**            |         |       |       |            |              |
| `SSL_stateless`                              | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_set_stateless_cookie_generate_cb`   | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_stateless_cookie_verify_cb`     | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| **⇒ Early Data/0-RTT**                       |         |       |       |            |              |
| `SSL_CTX_set_allow_early_data_cb`            | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_allow_early_data_cb`                | 0-RTT   | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_get_recv_max_early_data`            | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_recv_max_early_data`            | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_get_recv_max_early_data`                | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_recv_max_early_data`                | 0-RTT   | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTX_get_max_early_data`                 | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTX_set_max_early_data`                 | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_get_max_early_data`                     | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_set_max_early_data`                     | 0-RTT   | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_read_early_data`                        | 0-RTT   | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_write_early_data`                       | 0-RTT   | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_get_early_data_status`                  | 0-RTT   | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| **⇒ Miscellaneous**                          |         |       |       |            |              |
| `DTLSv1_listen`                              | RL      | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLS_set_timer_cb`                          | NDP     | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `DTLS_get_data_mtu`                          | NDP     | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `SSL_get_ex_data_X509_STORE_CTX_idx`         | Global  | 🟩U   | 🟦U   | 🟩NC       | 🟢Done       |
| `BIO_ssl_shutdown`                           | Global  | 🟩U   | 🟩A   | 🟩NC       | 🟢Done       |
| `SSL_alloc_buffers`                          | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_free_buffers`                           | HL      | 🟩U   | 🟩A   | 🟨C\*      | 🟢Done       |
| `SSL_trace`                                  | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| `SSL_set_debug`                              | HL      | 🟩U   | 🟩A   | 🟩NC\*     | 🟢Done       |
| **⇒ Controls**                               |         |       |       |            |              |
| `SSL_CTRL_MODE`                              | Special | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_CTRL_CLEAR_MODE`                        | Special | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_CTRL_CLEAR_NUM_RENEGOTIATIONS`          | HL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_NUM_RENEGOTIATIONS`            | HL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TOTAL_RENEGOTIATIONS`          | HL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_RI_SUPPORT`                    | HL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_READ_AHEAD`                    | HL      | 🟩U   | 🟧NO  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_READ_AHEAD`                    | HL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTRL_SET_MAX_PIPELINES`                 | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTRL_SET_MAX_SEND_FRAGMENT`             | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTRL_SET_SPLIT_SEND_FRAGMENT`           | RL      | 🟩U   | 🟥FC  | 🟨C*       | 🟢Done       |
| `SSL_CTRL_SET_MTU`                           | RL      | 🟩U   | 🟥FC  | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_MAX_PROTO_VERSION`             | HL      | 🟩U   | 🟩A   | 🟨C*       | 🟢Done       |
| `SSL_CTRL_SET_MIN_PROTO_VERSION`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_MAX_PROTO_VERSION`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_MIN_PROTO_VERSION`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_BUILD_CERT_CHAIN`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_CERT_FLAGS`                        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_CHAIN`                             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_CHAIN_CERT`                        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_CLEAR_CERT_FLAGS`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_CLEAR_EXTRA_CHAIN_CERTS`           | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_EXTRA_CHAIN_CERT`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_CHAIN_CERTS`                   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_CHAIN_CERT_STORE`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_CLIENT_CERT_REQUEST`           | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_CLIENT_CERT_TYPES`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_EC_POINT_FORMATS`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_EXTMS_SUPPORT`                 | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_EXTRA_CHAIN_CERTS`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_FLAGS`                         | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_GROUPS`                        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_IANA_GROUPS`                   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_MAX_CERT_LIST`                 | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_NEGOTIATED_GROUP`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_PEER_SIGNATURE_NID`            | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_PEER_TMP_KEY`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_RAW_CIPHERLIST`                | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_SESS_CACHE_MODE`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_SESS_CACHE_SIZE`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_SHARED_GROUP`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_SIGNATURE_NID`                 | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_CB`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_CB_ARG`      | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_EXTS`        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_IDS`         | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_OCSP_RESP`   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_STATUS_REQ_TYPE`        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TLSEXT_TICKET_KEYS`            | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_TMP_KEY`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_GET_VERIFY_CERT_STORE`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SELECT_CURRENT_CERT`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_ACCEPT`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_ACCEPT_GOOD`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_ACCEPT_RENEGOTIATE`           | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_CACHE_FULL`                   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_CB_HIT`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_CONNECT`                      | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_CONNECT_GOOD`                 | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_CONNECT_RENEGOTIATE`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_HIT`                          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_MISSES`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_NUMBER`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SESS_TIMEOUTS`                     | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_CHAIN_CERT_STORE`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_CLIENT_CERT_TYPES`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_CLIENT_SIGALGS`                | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_CLIENT_SIGALGS_LIST`           | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_CURRENT_CERT`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_DH_AUTO`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_GROUPS`                        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_GROUPS_LIST`                   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_MAX_CERT_LIST`                 | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_MSG_CALLBACK`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_MSG_CALLBACK_ARG`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_NOT_RESUMABLE_SESS_CB`         | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_RETRY_VERIFY`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SESS_CACHE_MODE`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SESS_CACHE_SIZE`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SIGALGS`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SIGALGS_LIST`                  | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SRP_ARG`                       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SRP_GIVE_CLIENT_PWD_CB`        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_SRP_VERIFY_PARAM_CB`           | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_DEBUG_ARG`              | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_DEBUG_CB`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_HOSTNAME`               | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_SERVERNAME_ARG`         | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_SERVERNAME_CB`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLS_EXT_SRP_PASSWORD`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLS_EXT_SRP_STRENGTH`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLS_EXT_SRP_USERNAME`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLS_EXT_SRP_USERNAME_CB`       | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_CB_ARG`      | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_EXTS`        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_IDS`         | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_OCSP_RESP`   | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_STATUS_REQ_TYPE`        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_TICKET_KEY_CB`          | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TLSEXT_TICKET_KEYS`            | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TMP_DH`                        | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TMP_DH_CB`                     | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_TMP_ECDH`                      | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| `SSL_CTRL_SET_VERIFY_CERT_STORE`             | HL      | 🟩U   | 🟩A   | 🟩NC*      | 🟢Done       |
| **⇒ SSL Modes**                              |         |       |       |            |              |
| `SSL_MODE_ENABLE_PARTIAL_WRITE`              | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER`        | ADP     | 🟩U   | 🟩A   | 🟧QSI      | 🟢Done       |
| `SSL_MODE_RELEASE_BUFFERS`                   | ADP     | 🟩U   | 🟧NO  | 🟩NC       | 🟢Done       |
| `SSL_MODE_ASYNC`                             | ADP     | 🟩U   | 🟧NO  | 🟩NC       | 🟢Done       |
| `SSL_MODE_AUTO_RETRY`                        | ADP     | 🟩U   | 🟧NO  | 🟩NC       | 🟢Done       |
| `SSL_MODE_SEND_FALLBACK_SCSV`                | HL      | 🟩U   | 🟩U   | 🟩NC       | 🟢Done       |

Q&A For TLS-Related Calls
-------------------------

### What should `SSL_get_current_cipher`, `SSL_get_pending_cipher`, etc. do?

QUIC always uses AES-128-GCM for Initial packets. At this time the handshake
layer has not negotiated a ciphersuite so it has no “current” cipher. We could
return AES-128-GCM here, but it seems reasonable to just return NULL as the
encryption is mostly for protection against accidental modification and not
“real” encryption. From the perspective of the Handshake layer encryption is not
active yet. An application using QUIC can always interpret NULL as meaning
AES-128-GCM is being used if needed as this is implied by using QUIC.

A. We return NULL here, because it allows applications to detect if a
ciphersuite has been negotiated and NULL can be used to infer that Initial
encryption is still being used. This also minimises the changes needed to the
implementation.

### What should `SSL_CTX_set_cipher_list` do?

Since this function configures the cipher list for TLSv1.2 and below only, there
is no need to restrict it as TLSv1.3 is required for QUIC. For the sake of
application compatibility, applications can still configure the TLSv1.2 cipher
list; it will always be ignored. This function can still be used to set the
SECLEVEL; no changes are needed to facilitate this.

### What SSL options should be supported?

Options we explicitly want to support:

- `SSL_OP_CIPHER_SERVER_PREFERENCE`
- `SSL_OP_DISABLE_TLSEXT_CA_NAMES`
- `SSL_OP_NO_TX_CERTIFICATE_COMPRESSION`
- `SSL_OP_NO_RX_CERTIFICATE_COMPRESSION`
- `SSL_OP_PRIORITIZE_CHACHA`
- `SSL_OP_NO_TICKET`
- `SSL_OP_CLEANSE_PLAINTEXT`

Options we do not yet support but could support in the future, currently no-ops:

- `SSL_OP_NO_QUERY_MTU`
- `SSL_OP_NO_ANTI_REPLAY`

The following options must be explicitly forbidden:

- `SSL_OP_NO_TLSv1_3` — TLSv1.3 is required for QUIC
- `SSL_OP_ENABLE_MIDDLEBOX_COMPAT` — forbidden by QUIC RFCs
- `SSL_OP_ENABLE_KTLS` — not currently supported for QUIC
- `SSL_OP_SAFARI_ECDHE_ECDSA_BUG`
- `SSL_OP_TLSEXT_PADDING`
- `SSL_OP_TLS_ROLLBACK_BUG`
- `SSL_OP_IGNORE_UNEXPECTED_EOF`
- `SSL_OP_ALLOW_NO_DHE_KEX`

The following options are ignored for TLSv1.3 or otherwise not applicable and
may therefore be settable but ignored. We take this approach on the grounds
that it is harmless and applications might want to see that options have been
correctly set for protocols unrelated to QUIC.

- `SSL_OP_CRYPTOPRO_TLSEXT_BUG`
- `SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS`
- `SSL_OP_ALLOW_CLIENT_RENEGOTIATION`
- `SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION`
- `SSL_OP_CISCO_ANYCONNECT`
- `SSL_OP_COOKIE_EXCHANGE`
- `SSL_OP_LEGACY_SERVER_CONNECT`
- `SSL_OP_NO_COMPRESSION`
- `SSL_OP_NO_ENCRYPT_THEN_MAC`
- `SSL_OP_NO_EXTENDED_MASTER_SECRET`
- `SSL_OP_NO_RENEGOTIATION`
- `SSL_OP_NO_RESSION_RESUMPTION_ON_NEGOTIATION`
- `SSL_OP_NO_SSLv3`
- `SSL_OP_NO_TLSv1`
- `SSL_OP_NO_TLSv1_1`
- `SSL_OP_NO_TLSv1_2`
- `SSL_OP_NO_DTLSv1`
- `SSL_OP_NO_DTLSv1_2`

### What should `SSL_rstate_string` and `SSL_state_string` do?

SSL_state_string is highly handshake layer specific, so it makes sense to just
forward to the handshake layer.

SSL_rstate_string is record layer specific. A cursory evaluation of usage via
GitHub code search did not appear to identify much usage of this function other
than for debug output; i.e., there seems to be little usage of this in a way
that depends on the output for the purposes of control flow. Since there is not
really any direct correspondence to the QUIC record layer, we conservatively
define the output of this function as "unknown".

TODO: forbid NPN
TODO: enforce TLSv1.3
TODO: forbid PHA            - DONE
TODO: forbid middlebox compat mode in a deeper way?
TODO: new_session_ticket doesn't need modifying as such, but ticket machinery
      will

### What should `SSL_pending` and `SSL_has_pending` do?

`SSL_pending` traditionally yields the number of bytes buffered inside an SSL
object available for immediate reading. For QUIC, we can just make this report
the current size of the receive stream buffer.

`SSL_has_pending` returns a boolean value indicating whether there is processed
or unprocessed incoming data pending. There is no direct correspondence to
QUIC, so there are various implementation options:

- `SSL_pending() > 0`
- `SSL_pending() > 0 || pending URXEs or RXEs exist`

The latter can probably be viewed as more of a direct correspondence to the
design intent of the API, so we go with this.

### What should `SSL_alloc_buffers` and `SSL_free_buffers` do?

These do not really correspond to our internal architecture for QUIC. Since
internal buffers are always available, `SSL_alloc_buffers` can simply always
return 1. `SSL_free_buffers` can always return 0, as though the buffers are in
use, which they generally will be.

### What should `SSL_key_update` and `SSL_get_key_update_type`?

`SSL_key_update` can trigger a TX record layer key update, which will cause the
peer to respond with a key update in turn. The update occurs asynchronously
at next transmission, not immediately.

`SSL_get_key_update_type` returns an enumerated value which is only relevant to
the TLSv1.3 protocol; for QUIC, it will always return `SSL_KEY_UPDATE_NONE`.

### What should `SSL_MODE_AUTO_RETRY` do?

The absence of `SSL_MODE_AUTO_RETRY` causes `SSL_read`/`SSL_write` on a normal
TLS connection to potentially return due to internal handshake message
processing. This does not really make sense for our QUIC implementation,
therefore we always act as though `SSL_MODE_AUTO_RETRY` is on, and this mode is
ignored.

### What should `SSL_MODE_SEND_FALLBACK_SCSV` do?

This is not relevant to QUIC because this functionality relates to protocol
version downgrade attack protection and QUIC only supports TLSv1.3. Thus,
it is ignored.

### What should `SSL_CTX_set_ssl_version` do?

This is a deprecated function, so it needn't be supported for QUIC. Fail closed.

### What should `SSL_set_ssl_method` do?

We do not currently support this for QUIC.

### What should `SSL_set_shutdown` do?

This is not supported and is a no-op for QUIC.

### What should `SSL_dup` and `SSL_clear` do?

These may be tricky to support. Currently they are blocked.
