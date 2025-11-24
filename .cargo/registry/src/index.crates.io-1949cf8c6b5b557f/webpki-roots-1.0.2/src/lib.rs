//! A compiled-in copy of the root certificates trusted by Mozilla.
//!
//! To use this library with rustls 0.22:
//!
//! ```rust
//! let root_store = rustls::RootCertStore {
//!   roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
//! };
//! ```
//!
//! This library is suitable for use in applications that can always be recompiled and instantly deployed.
//! For applications that are deployed to end-users and cannot be recompiled, or which need certification
//! before deployment, consider a library that uses the platform native certificate verifier such as
//! [rustls-platform-verifier]. This has the additional benefit of supporting OS provided CA constraints
//! and revocation data.
//!
//! [rustls-platform-verifier]: https://docs.rs/rustls-platform-verifier
//
// This library is automatically generated from the Mozilla
// IncludedCACertificateReportPEMCSV report via ccadb.org. Don't edit it.
//
// The generation is done deterministically so you can verify it
// yourself by inspecting and re-running the generation process.

#![no_std]
#![forbid(unsafe_code, unstable_features)]
#![deny(
    elided_lifetimes_in_paths,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_extern_crates,
    unused_qualifications
)]

use pki_types::{Der, TrustAnchor};

pub const TLS_SERVER_ROOTS: &[TrustAnchor<'static>] = &[
  /*
   * Issuer: CN=DigiCert TLS ECC P384 Root G5 O=DigiCert, Inc.
   * Subject: CN=DigiCert TLS ECC P384 Root G5 O=DigiCert, Inc.
   * Label: "DigiCert TLS ECC P384 Root G5"
   * Serial: 13129116028163249804115411775095713523
   * SHA256 Fingerprint: 01:8e:13:f0:77:25:32:cf:80:9b:d1:b1:72:81:86:72:83:fc:48:c6:e1:3b:e9:c6:98:12:85:4a:49:0c:1b:05
   * -----BEGIN CERTIFICATE-----
   * MIICGTCCAZ+gAwIBAgIQCeCTZaz32ci5PhwLBCou8zAKBggqhkjOPQQDAzBOMQsw
   * CQYDVQQGEwJVUzEXMBUGA1UEChMORGlnaUNlcnQsIEluYy4xJjAkBgNVBAMTHURp
   * Z2lDZXJ0IFRMUyBFQ0MgUDM4NCBSb290IEc1MB4XDTIxMDExNTAwMDAwMFoXDTQ2
   * MDExNDIzNTk1OVowTjELMAkGA1UEBhMCVVMxFzAVBgNVBAoTDkRpZ2lDZXJ0LCBJ
   * bmMuMSYwJAYDVQQDEx1EaWdpQ2VydCBUTFMgRUNDIFAzODQgUm9vdCBHNTB2MBAG
   * ByqGSM49AgEGBSuBBAAiA2IABMFEoc8Rl1Ca3iOCNQfN0MsYndLxf3c1TzvdlHJS
   * 7cI7+Oz6e2tYIOyZrsn8aLN1udsJ7MgT9U7GCh1mMEy7H0cKPGEQQil8pQgO4CLp
   * 0zVozptjn4S1mU1YoI71VOeVyaNCMEAwHQYDVR0OBBYEFMFRRVBZqz7nLFr6ICIS
   * B4CIfBFqMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49
   * BAMDA2gAMGUCMQCJao1H5+z8blUD2WdsJk6Dxv3J+ysTvLd6jLRl0mlpYxNjOyZQ
   * LgGheQaRnUi/wr4CMEfDFXuxoJGZSZOoPHzoRgaLLPIxAJSdYsiJvRmEFOml+wG4
   * DXZDjC5Ty3zfDBeWUA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x170\x15\x06\x03U\x04\n\x13\x0eDigiCert, Inc.1&0$\x06\x03U\x04\x03\x13\x1dDigiCert TLS ECC P384 Root G5"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xc1D\xa1\xcf\x11\x97P\x9a\xde#\x825\x07\xcd\xd0\xcb\x18\x9d\xd2\xf1\x7fw5O;\xdd\x94rR\xed\xc2;\xf8\xec\xfa{kX \xec\x99\xae\xc9\xfch\xb3u\xb9\xdb\t\xec\xc8\x13\xf5N\xc6\n\x1df0L\xbb\x1fG\n<a\x10B)|\xa5\x08\x0e\xe0\"\xe9\xd35h\xce\x9bc\x9f\x84\xb5\x99MX\xa0\x8e\xf5T\xe7\x95\xc9"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CommScope Public Trust RSA Root-01 O=CommScope
   * Subject: CN=CommScope Public Trust RSA Root-01 O=CommScope
   * Label: "CommScope Public Trust RSA Root-01"
   * Serial: 354030733275608256394402989253558293562031411421
   * SHA256 Fingerprint: 02:bd:f9:6e:2a:45:dd:9b:f1:8f:c7:e1:db:df:21:a0:37:9b:a3:c9:c2:61:03:44:cf:d8:d6:06:fe:c1:ed:81
   * -----BEGIN CERTIFICATE-----
   * MIIFbDCCA1SgAwIBAgIUPgNJgXUWdDGOTKvVxZAplsU5EN0wDQYJKoZIhvcNAQEL
   * BQAwTjELMAkGA1UEBhMCVVMxEjAQBgNVBAoMCUNvbW1TY29wZTErMCkGA1UEAwwi
   * Q29tbVNjb3BlIFB1YmxpYyBUcnVzdCBSU0EgUm9vdC0wMTAeFw0yMTA0MjgxNjQ1
   * NTRaFw00NjA0MjgxNjQ1NTNaME4xCzAJBgNVBAYTAlVTMRIwEAYDVQQKDAlDb21t
   * U2NvcGUxKzApBgNVBAMMIkNvbW1TY29wZSBQdWJsaWMgVHJ1c3QgUlNBIFJvb3Qt
   * MDEwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCwSGWjDR1C45FtnYSk
   * YZYSwu3D2iM0GXb26v1VWvZVAVMP8syMl0+5UMuzAURWlv2bKOx7dAvnQmtVzslh
   * suitQDy6uUEKBU8bJoWPQ7VAtYXR1HHcg0Hz9kXHgKKEUJdGzqAMxGBWBB0HW0al
   * DrJLpA6lfO741GIDuZNqihS4cPgugkY4Iw50x2tBt9Apo52AsH53k2NC+zSDO3Oj
   * WiE260f6GBfZumbCk6SP/F2krfxQapWsvCQz0b2If4b19bJzKo98rwjyGpg/qYFl
   * P8GMicWWMJoKz/TUyDTtnS+8jTiGU+6Xn6myY5QXjQ/cZip8UlF1y5mO6D1cv547
   * KI2DAg+pn3LiLCuz3GaXAEDQpFSOm117RTYm1nJD68/A6g3czhLmfTifBSeolz7p
   * UcZsBSjBAg/pGG3svZwG1KdJ9FQFa2ww8esD1eo9anbCyxooSU1/ZOD6K9pzg4H/
   * kQO9lLvkuI6cMmPNn7togbGEW682v3fuHX/3SZtS7NJ3Wn2RnU3COS3kuoL4b/JO
   * Hg9O5j9ZpSPcPYeoKFgo0fEbNttPxP/hjFtyjMcmAyejOQoBqsCyMWCDIqFPEgkB
   * Ea801M/XrmLTBQe0MXXgDW1XT2mH+VepuhX2yFJtocucH+X8eKg1mp9BFM6ltM6U
   * CBwJrVbl2rZJmkrqYxhTnCwuwwIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4G
   * A1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUN12mmnQywsL5x6YVEFm45P3luG0wDQYJ
   * KoZIhvcNAQELBQADggIBAK+nz97/4L1CjU3lIpbfaOp9TSp90K09FlxD533Ahuh6
   * NWPxzIHIxgvoLlI1pKZJkGNRrDSsBTtXAOnTYtPZKdVUvhwQkZyybf5Z/Xn36lbQ
   * nmhUQo8mUuJM3y+Xpi/SB5io82BdS5pYV4jvguX6r2yBS5KPQJqTRlnLX3gWsWc+
   * QgvfKNmwrZggvkN80V4aCRckjXtdlemrwWCrWxhkgPut4AZ9HcpZuPN4KWfGVh2v
   * trV0KnahP/t1MJ+UXjulYPPLXAziDslg+MkfFoom3ecnf+slpoq9uC02EJqxWE2a
   * aE9gVOX2RhOOiKy8IUISrcZKiX2bwdgt6ZYD9KJ0DLwAHb/WNyVntHKLr4W96ioD
   * j8z7PEQkguIBpQtZtjSNMgsSDesnwv1B10A8ckYpwIzqug/xBpMu95yo9GA+o/E4
   * Xo4TwbM6l4c/ksp4qRyv0LAbJh6+cOx69TOY6lz/KwsETkPdY34Op054A5U+1C0w
   * lREQKC6/oAI+/15Z0wUOlV9TRe9rh9VIzRamloPh37MG88EU26fsHItdkJANclHn
   * YfkUyq+Dj7+vsQpZXdxc1+SWrVtgHdqul7I52Qb1dgAT+GhMIbA1xNxVssnBQVoc
   * icCMb3SgazNNtQEo/a2tiRc7ppqEvOuM6sRxJKi6KfkIsidWNTJf6jn7MZrVGczw
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x0c\tCommScope1+0)\x06\x03U\x04\x03\x0c\"CommScope Public Trust RSA Root-01"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb0He\xa3\r\x1dB\xe3\x91m\x9d\x84\xa4a\x96\x12\xc2\xed\xc3\xda#4\x19v\xf6\xea\xfdUZ\xf6U\x01S\x0f\xf2\xcc\x8c\x97O\xb9P\xcb\xb3\x01DV\x96\xfd\x9b(\xec{t\x0b\xe7BkU\xce\xc9a\xb2\xe8\xad@<\xba\xb9A\n\x05O\x1b&\x85\x8fC\xb5@\xb5\x85\xd1\xd4q\xdc\x83A\xf3\xf6E\xc7\x80\xa2\x84P\x97F\xce\xa0\x0c\xc4`V\x04\x1d\x07[F\xa5\x0e\xb2K\xa4\x0e\xa5|\xee\xf8\xd4b\x03\xb9\x93j\x8a\x14\xb8p\xf8.\x82F8#\x0et\xc7kA\xb7\xd0)\xa3\x9d\x80\xb0~w\x93cB\xfb4\x83;s\xa3Z!6\xebG\xfa\x18\x17\xd9\xbaf\xc2\x93\xa4\x8f\xfc]\xa4\xad\xfcPj\x95\xac\xbc$3\xd1\xbd\x88\x7f\x86\xf5\xf5\xb2s*\x8f|\xaf\x08\xf2\x1a\x98?\xa9\x81e?\xc1\x8c\x89\xc5\x960\x9a\n\xcf\xf4\xd4\xc84\xed\x9d/\xbc\x8d8\x86S\xee\x97\x9f\xa9\xb2c\x94\x17\x8d\x0f\xdcf*|RQu\xcb\x99\x8e\xe8=\\\xbf\x9e;(\x8d\x83\x02\x0f\xa9\x9fr\xe2,+\xb3\xdcf\x97\x00@\xd0\xa4T\x8e\x9b]{E6&\xd6rC\xeb\xcf\xc0\xea\r\xdc\xce\x12\xe6}8\x9f\x05\'\xa8\x97>\xe9Q\xc6l\x05(\xc1\x02\x0f\xe9\x18m\xec\xbd\x9c\x06\xd4\xa7I\xf4T\x05kl0\xf1\xeb\x03\xd5\xea=jv\xc2\xcb\x1a(IM\x7fd\xe0\xfa+\xdas\x83\x81\xff\x91\x03\xbd\x94\xbb\xe4\xb8\x8e\x9c2c\xcd\x9f\xbbh\x81\xb1\x84[\xaf6\xbfw\xee\x1d\x7f\xf7I\x9bR\xec\xd2wZ}\x91\x9dM\xc29-\xe4\xba\x82\xf8o\xf2N\x1e\x0fN\xe6?Y\xa5#\xdc=\x87\xa8(X(\xd1\xf1\x1b6\xdbO\xc4\xff\xe1\x8c[r\x8c\xc7&\x03\'\xa39\n\x01\xaa\xc0\xb21`\x83\"\xa1O\x12\t\x01\x11\xaf4\xd4\xcf\xd7\xaeb\xd3\x05\x07\xb41u\xe0\rmWOi\x87\xf9W\xa9\xba\x15\xf6\xc8Rm\xa1\xcb\x9c\x1f\xe5\xfcx\xa85\x9a\x9fA\x14\xce\xa5\xb4\xce\x94\x08\x1c\t\xadV\xe5\xda\xb6I\x9aJ\xeac\x18S\x9c,.\xc3\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Entrust Root Certification Authority - EC1 O=Entrust, Inc. OU=See www.entrust.net/legal-terms/(c) 2012 Entrust, Inc. - for authorized use only
   * Subject: CN=Entrust Root Certification Authority - EC1 O=Entrust, Inc. OU=See www.entrust.net/legal-terms/(c) 2012 Entrust, Inc. - for authorized use only
   * Label: "Entrust Root Certification Authority - EC1"
   * Serial: 51543124481930649114116133369
   * SHA256 Fingerprint: 02:ed:0e:b2:8c:14:da:45:16:5c:56:67:91:70:0d:64:51:d7:fb:56:f0:b2:ab:1d:3b:8e:b0:70:e5:6e:df:f5
   * -----BEGIN CERTIFICATE-----
   * MIIC+TCCAoCgAwIBAgINAKaLeSkAAAAAUNCR+TAKBggqhkjOPQQDAzCBvzELMAkG
   * A1UEBhMCVVMxFjAUBgNVBAoTDUVudHJ1c3QsIEluYy4xKDAmBgNVBAsTH1NlZSB3
   * d3cuZW50cnVzdC5uZXQvbGVnYWwtdGVybXMxOTA3BgNVBAsTMChjKSAyMDEyIEVu
   * dHJ1c3QsIEluYy4gLSBmb3IgYXV0aG9yaXplZCB1c2Ugb25seTEzMDEGA1UEAxMq
   * RW50cnVzdCBSb290IENlcnRpZmljYXRpb24gQXV0aG9yaXR5IC0gRUMxMB4XDTEy
   * MTIxODE1MjUzNloXDTM3MTIxODE1NTUzNlowgb8xCzAJBgNVBAYTAlVTMRYwFAYD
   * VQQKEw1FbnRydXN0LCBJbmMuMSgwJgYDVQQLEx9TZWUgd3d3LmVudHJ1c3QubmV0
   * L2xlZ2FsLXRlcm1zMTkwNwYDVQQLEzAoYykgMjAxMiBFbnRydXN0LCBJbmMuIC0g
   * Zm9yIGF1dGhvcml6ZWQgdXNlIG9ubHkxMzAxBgNVBAMTKkVudHJ1c3QgUm9vdCBD
   * ZXJ0aWZpY2F0aW9uIEF1dGhvcml0eSAtIEVDMTB2MBAGByqGSM49AgEGBSuBBAAi
   * A2IABIQTydC6bUF74mzQ61VfZgIaJPRbiWlH47jCffHyAsWfoPZb1YsGGYZPUxBt
   * ByQnoaD41UcZYUx9ypMn6nQM72+WCf5j7HBdNq1nd67JnXxVRDqiY1Ef9eNi1KlH
   * Bz7MIKNCMEAwDgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0O
   * BBYEFLdj5xrdjekIplWDpOBqUEFlEUJJMAoGCCqGSM49BAMDA2cAMGQCMGF52OVC
   * R98crlOZF7ZvHH3hvxGU0QOIdeSNiaSKd0bebWHvAvX7td/M/k7//qnmpwIwW5nX
   * hTcGtXsI/esni0qU+eH6p44mCOh8kmhtc9hvJqwhAriZtyZBWyVgrtBIGu4G
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x160\x14\x06\x03U\x04\n\x13\rEntrust, Inc.1(0&\x06\x03U\x04\x0b\x13\x1fSee www.entrust.net/legal-terms1907\x06\x03U\x04\x0b\x130(c) 2012 Entrust, Inc. - for authorized use only1301\x06\x03U\x04\x03\x13*Entrust Root Certification Authority - EC1"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x84\x13\xc9\xd0\xbamA{\xe2l\xd0\xebU_f\x02\x1a$\xf4[\x89iG\xe3\xb8\xc2}\xf1\xf2\x02\xc5\x9f\xa0\xf6[\xd5\x8b\x06\x19\x86OS\x10m\x07$\'\xa1\xa0\xf8\xd5G\x19aL}\xca\x93\'\xeat\x0c\xefo\x96\t\xfec\xecp]6\xadgw\xae\xc9\x9d|UD:\xa2cQ\x1f\xf5\xe3b\xd4\xa9G\x07>\xcc "),
    name_constraints: None
  },

  /*
   * Issuer: CN=AffirmTrust Commercial O=AffirmTrust
   * Subject: CN=AffirmTrust Commercial O=AffirmTrust
   * Label: "AffirmTrust Commercial"
   * Serial: 8608355977964138876
   * SHA256 Fingerprint: 03:76:ab:1d:54:c5:f9:80:3c:e4:b2:e2:01:a0:ee:7e:ef:7b:57:b6:36:e8:a9:3c:9b:8d:48:60:c9:6f:5f:a7
   * -----BEGIN CERTIFICATE-----
   * MIIDTDCCAjSgAwIBAgIId3cGJyapsXwwDQYJKoZIhvcNAQELBQAwRDELMAkGA1UE
   * BhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MR8wHQYDVQQDDBZBZmZpcm1UcnVz
   * dCBDb21tZXJjaWFsMB4XDTEwMDEyOTE0MDYwNloXDTMwMTIzMTE0MDYwNlowRDEL
   * MAkGA1UEBhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MR8wHQYDVQQDDBZBZmZp
   * cm1UcnVzdCBDb21tZXJjaWFsMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKC
   * AQEA9htPZwcroRX1BiLLHwGy43NFBkRJLLtJJRTWzsO3qyxPxkEylFf6EqdbDuKP
   * Hx6GGaeqtS25Xw2Kwq+FNXkyLbscYjfysVtKPcrNcV/pQr6U6Mje+SJIZMblq8Yr
   * ba0F8PrVC8+a5fBQpIs7R6UjW3p6+DM/uO+Zl+MgwdYoic+U+7lF7eNAFxHUdPAL
   * MeIrJmqbTFeurCA+ukV6BfO9m2kVrn1OIGPENXY6BwLJN/3HR+7o8XYdcxXyl6S1
   * yHp52UKqK39c/s4mT6NmgTWvRLpUHhwwMmWd5jyTXlBOeuM61G7MGvv50jeuJCqr
   * VwMiKA1JdX+3KNp1v47j3A55MQIDAQABo0IwQDAdBgNVHQ4EFgQUnZPGU4teyq8/
   * nx4P5ZmVvCT2lI8wDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwDQYJ
   * KoZIhvcNAQELBQADggEBAFis9AQOzcAN/wr91LoWXym9e2iZWEnStB03TX8nfUYG
   * XUPGhi4+c7ImfU+TqbbEKpqrIZcUsd6M06uJFdhrJNTxFq7YpFzUf1GO7RgBsZNj
   * vbz4YYCanrHOQnDiqX0GJX0nof5v7LMeJNrjS1UaADs1tDvZ110w/YETifLCBivt
   * Z8SOyUOyXGsViQK8YvxO8rUzqrJv0wqiUOP2O+guRMLbZjipM1ZI8W0bM40NjD9g
   * N53Tym1+NH4Nn3J2ixufcv1SNUFFApYvHLKac0khsUlHRUe072o0EclNmsxZt9YC
   * nlpOZbWUrhvfKbAW8b8Angc6F2S1BLUjIZkKlTuXfO8=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x140\x12\x06\x03U\x04\n\x0c\x0bAffirmTrust1\x1f0\x1d\x06\x03U\x04\x03\x0c\x16AffirmTrust Commercial"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xf6\x1bOg\x07+\xa1\x15\xf5\x06\"\xcb\x1f\x01\xb2\xe3sE\x06DI,\xbbI%\x14\xd6\xce\xc3\xb7\xab,O\xc6A2\x94W\xfa\x12\xa7[\x0e\xe2\x8f\x1f\x1e\x86\x19\xa7\xaa\xb5-\xb9_\r\x8a\xc2\xaf\x855y2-\xbb\x1cb7\xf2\xb1[J=\xca\xcdq_\xe9B\xbe\x94\xe8\xc8\xde\xf9\"Hd\xc6\xe5\xab\xc6+m\xad\x05\xf0\xfa\xd5\x0b\xcf\x9a\xe5\xf0P\xa4\x8b;G\xa5#[zz\xf83?\xb8\xef\x99\x97\xe3 \xc1\xd6(\x89\xcf\x94\xfb\xb9E\xed\xe3@\x17\x11\xd4t\xf0\x0b1\xe2+&j\x9bLW\xae\xac >\xbaEz\x05\xf3\xbd\x9bi\x15\xae}N c\xc45v:\x07\x02\xc97\xfd\xc7G\xee\xe8\xf1v\x1ds\x15\xf2\x97\xa4\xb5\xc8zy\xd9B\xaa+\x7f\\\xfe\xce&O\xa3f\x815\xafD\xbaT\x1e\x1c02e\x9d\xe6<\x93^PNz\xe3:\xd4n\xcc\x1a\xfb\xf9\xd27\xae$*\xabW\x03\"(\rIu\x7f\xb7(\xdau\xbf\x8e\xe3\xdc\x0ey1\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=D-TRUST BR Root CA 2 2023 O=D-Trust GmbH
   * Subject: CN=D-TRUST BR Root CA 2 2023 O=D-Trust GmbH
   * Label: "D-TRUST BR Root CA 2 2023"
   * Serial: 153168538924886464690566649552453098598
   * SHA256 Fingerprint: 05:52:e6:f8:3f:df:65:e8:fa:96:70:e6:66:df:28:a4:e2:13:40:b5:10:cb:e5:25:66:f9:7c:4f:b9:4b:2b:d1
   * -----BEGIN CERTIFICATE-----
   * MIIFqTCCA5GgAwIBAgIQczswBEhb2U14LnNLyaHcZjANBgkqhkiG9w0BAQ0FADBI
   * MQswCQYDVQQGEwJERTEVMBMGA1UEChMMRC1UcnVzdCBHbWJIMSIwIAYDVQQDExlE
   * LVRSVVNUIEJSIFJvb3QgQ0EgMiAyMDIzMB4XDTIzMDUwOTA4NTYzMVoXDTM4MDUw
   * OTA4NTYzMFowSDELMAkGA1UEBhMCREUxFTATBgNVBAoTDEQtVHJ1c3QgR21iSDEi
   * MCAGA1UEAxMZRC1UUlVTVCBCUiBSb290IENBIDIgMjAyMzCCAiIwDQYJKoZIhvcN
   * AQEBBQADggIPADCCAgoCggIBAK7/CVmRgApKaOYkP7in5Mg6CjoWzckjYaCTcfKr
   * i3OPoGdlYNJUa2NRb0kz4HIHE304zQaSBylSa053bATTlfrdTIzZXcFhfUvnKLNE
   * gXtRr90zsWh81k5M/itoucpmacTsXld/9w3HnDY25QdgrMBM6ghs7wZ8T1soegj8
   * k12b9py0i4a6Ibn08OhZWiihNIQaJZG2tY/vsvmA+vk9PBFy2OMvhnbFeSzBqZCT
   * Rphny4NqoFAjpzv2gTng7fC5v2Xx2Mt6++9zA84A9H3X4F07ZrjcjrqDy4d2A/wl
   * 2ecjbwb9Z/Pg/4S8R7+1FhhGaRTMBffb00msa8yr5LULQyReS2tNZ9/WtT5PeB+U
   * cSTq3nD88ZP+npNa5JRal1QMNXtfbO4AHyTsA7oC9Xb0n9Sa7YUsOCIvx9gvdhFP
   * /Wxc6PWOJ4d/GUohR5AdeY0cW/jPSoXk7bNbjb7EZChdQcRurDhaTyN0dKkSw/bS
   * uREVMweR2Ds3OmMwBtHFIjYoYiMQ4EbMl6zWK11kJNXuHA7e+whadSr2Y23OC0K+
   * 0bpwHJwh5Q8xaRfX/Aq03u2AnMuStIv13lmiWAmlY0cL4UEyNEHZmrHZqLAbWt4N
   * DfTisl01gLmB1IRpkQLLddCNxbU9CZEJjxShFHR5PtbJFR2kWVki3PaKRT08EtY+
   * XTIvAgMBAAGjgY4wgYswDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUZ5Dw1t61
   * GNVGKX5cq/ieCLxklRAwDgYDVR0PAQH/BAQDAgEGMEkGA1UdHwRCMEAwPqA8oDqG
   * OGh0dHA6Ly9jcmwuZC10cnVzdC5uZXQvY3JsL2QtdHJ1c3RfYnJfcm9vdF9jYV8y
   * XzIwMjMuY3JsMA0GCSqGSIb3DQEBDQUAA4ICAQA097N3U9swFrktpSHxQCF16+tI
   * FoE9c+CeJyrrd6kTpGoKWloUMz1oH4Guaf2Mn2VsNELZLdB/eBaxOqwjMa1ef67n
   * riv6uvw8l5VAk1/DLQOj7aRvU9f6QA4w9QAgLABMjDu0ox+2v5Eyq6+SmNMW5tTR
   * VFxDWy6u71cqqLRvpO8NVhTaIasgdp4D/Ca4nj8+AybmTNudX0KEPUUDAxxZiMrc
   * LmEkWqTqJwtzEr5SswrPMhfiHocaFpVIbVrg0M8JkiZmkdijYQ6qgYF/6FKC0ULn
   * 4B0Y+qSFNueG4A3rvNTJ1jxD8V1Jbn6Bm2m1iWKPiFLY1/4nwSPFyysCu7Ff/vtD
   * hQNGvl3GyiEm/9cCnnRK3PgTFbGBVzbLZVzRHTF36SXDw7IyN9XxmAnkbWOACKsG
   * koHU6XCPpz+y7YaMgmo1yEJagtFSGkUPFaUA8JR7ZSdXOUPPfH/mvTWze/EZTN46
   * ls/pdu4D58JDUjxqgejBWoC9EV2Ta/vH5mQ/u2kc6d0li690yVRAysuTEwrt+2aS
   * Ecr1wPrYg1UDfNPFIkZ1cGt5SAYqgpq/5usWDiJFAbzdNpQ0qTUmiteXue4Icr80
   * knCDgKs4qllo3UCkGJCy89UDyibK79XH4I9TjvAA46jtn/mtd+ArY0+ew+43u3gJ
   * hJ65bvspmZDogNOfJA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x13\x0cD-Trust GmbH1\"0 \x06\x03U\x04\x03\x13\x19D-TRUST BR Root CA 2 2023"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xae\xff\tY\x91\x80\nJh\xe6$?\xb8\xa7\xe4\xc8:\n:\x16\xcd\xc9#a\xa0\x93q\xf2\xab\x8bs\x8f\xa0ge`\xd2TkcQoI3\xe0r\x07\x13}8\xcd\x06\x92\x07)RkNwl\x04\xd3\x95\xfa\xddL\x8c\xd9]\xc1a}K\xe7(\xb3D\x81{Q\xaf\xdd3\xb1h|\xd6NL\xfe+h\xb9\xcafi\xc4\xec^W\x7f\xf7\r\xc7\x9c66\xe5\x07`\xac\xc0L\xea\x08l\xef\x06|O[(z\x08\xfc\x93]\x9b\xf6\x9c\xb4\x8b\x86\xba!\xb9\xf4\xf0\xe8YZ(\xa14\x84\x1a%\x91\xb6\xb5\x8f\xef\xb2\xf9\x80\xfa\xf9=<\x11r\xd8\xe3/\x86v\xc5y,\xc1\xa9\x90\x93F\x98g\xcb\x83j\xa0P#\xa7;\xf6\x819\xe0\xed\xf0\xb9\xbfe\xf1\xd8\xcbz\xfb\xefs\x03\xce\x00\xf4}\xd7\xe0];f\xb8\xdc\x8e\xba\x83\xcb\x87v\x03\xfc%\xd9\xe7#o\x06\xfdg\xf3\xe0\xff\x84\xbcG\xbf\xb5\x16\x18Fi\x14\xcc\x05\xf7\xdb\xd3I\xack\xcc\xab\xe4\xb5\x0bC$^KkMg\xdf\xd6\xb5>Ox\x1f\x94q$\xea\xdep\xfc\xf1\x93\xfe\x9e\x93Z\xe4\x94Z\x97T\x0c5{_l\xee\x00\x1f$\xec\x03\xba\x02\xf5v\xf4\x9f\xd4\x9a\xed\x85,8\"/\xc7\xd8/v\x11O\xfdl\\\xe8\xf5\x8e\'\x87\x7f\x19J!G\x90\x1dy\x8d\x1c[\xf8\xcfJ\x85\xe4\xed\xb3[\x8d\xbe\xc4d(]A\xc4n\xac8ZO#tt\xa9\x12\xc3\xf6\xd2\xb9\x11\x153\x07\x91\xd8;7:c0\x06\xd1\xc5\"6(b#\x10\xe0F\xcc\x97\xac\xd6+]d$\xd5\xee\x1c\x0e\xde\xfb\x08Zu*\xf6cm\xce\x0bB\xbe\xd1\xbap\x1c\x9c!\xe5\x0f1i\x17\xd7\xfc\n\xb4\xde\xed\x80\x9c\xcb\x92\xb4\x8b\xf5\xdeY\xa2X\t\xa5cG\x0b\xe1A24A\xd9\x9a\xb1\xd9\xa8\xb0\x1bZ\xde\r\r\xf4\xe2\xb2]5\x80\xb9\x81\xd4\x84i\x91\x02\xcbu\xd0\x8d\xc5\xb5=\t\x91\t\x8f\x14\xa1\x14ty>\xd6\xc9\x15\x1d\xa4YY\"\xdc\xf6\x8aE=<\x12\xd6>]2/\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TrustAsia TLS RSA Root CA O=TrustAsia Technologies, Inc.
   * Subject: CN=TrustAsia TLS RSA Root CA O=TrustAsia Technologies, Inc.
   * Label: "TrustAsia TLS RSA Root CA"
   * Serial: 160405846464868906657516898462547310235378010780
   * SHA256 Fingerprint: 06:c0:8d:7d:af:d8:76:97:1e:b1:12:4f:e6:7f:84:7e:c0:c7:a1:58:d3:ea:53:cb:e9:40:e2:ea:97:91:f4:c3
   * -----BEGIN CERTIFICATE-----
   * MIIFgDCCA2igAwIBAgIUHBjYz+VTPyI1RlNUJDxsR9FcSpwwDQYJKoZIhvcNAQEM
   * BQAwWDELMAkGA1UEBhMCQ04xJTAjBgNVBAoTHFRydXN0QXNpYSBUZWNobm9sb2dp
   * ZXMsIEluYy4xIjAgBgNVBAMTGVRydXN0QXNpYSBUTFMgUlNBIFJvb3QgQ0EwHhcN
   * MjQwNTE1MDU0MTU3WhcNNDQwNTE1MDU0MTU2WjBYMQswCQYDVQQGEwJDTjElMCMG
   * A1UEChMcVHJ1c3RBc2lhIFRlY2hub2xvZ2llcywgSW5jLjEiMCAGA1UEAxMZVHJ1
   * c3RBc2lhIFRMUyBSU0EgUm9vdCBDQTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCC
   * AgoCggIBAMMWuBtqpERz5dZO9LnPWwvB0ZqB9WOwj0PBuwhaGnrhB3YmH49pVr7+
   * NmDQDIPNlOrnxS1cLwUWAp4KqC/lYCZUlviYQB2srp10Zy9U+5RjmOMmSoPGlbYJ
   * Q1DNDX3eRA5gEk9bNb2/mThtfWza4mhzH/kxpRkQcwUqwzIZheo0qt1CHjCNP561
   * HmHVb70AcnKtEj+qpklz8oYVlQwQX1Fkzv93uMltrOXVmPGZLmzjyUT5tUMnCE32
   * ft5EebuyjBza00tsLtbDeLdM1aTk2tyKjg7/D8OmYCYozza/+lcK7Fs/6TAWe8Tb
   * xNRkoDD75f0dcZLdKY9BWN4ArTr9PXwaqLEX8E40eFgl1oUh63kd0Nyrz2I8sMeX
   * i9bQn9P+PN7F4/w6g3CEIR0JwqH8uyghZVNgepBtljhb//HXeltt08lwSUq6HTrQ
   * UNoyIBnkiz/r1RYmNzz7dZ6wB3C4FGB33PYPXFIKvF1tjVEK2sUYyJtt3LCDs3+j
   * TnhMmCWr8n4uIF6CFabW2I+s5c0yhsj55NqJ4js+k8UTav/H9xj8Z7XvGCxUq0DT
   * bE3txci3OE9kxJRMT6DNrqXGJyV1J23G2pyOsAWZ1SgRxSHUuPzHlqtKZFlhaxP8
   * S8ySpg+kUb8OWJDZgoM5pl+z+m6Ss80zDoWo8SnTq1mt1tve1CuBAgMBAAGjQjBA
   * MA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFLgHkXlcBvRG/XtZylomkadFK/hT
   * MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQwFAAOCAgEAIZtqBSBdGBanEqT3
   * Rz/NyjuujsCCztxIJXgXbODgcMTWltnZ9r96nBO7U5WS/8+S4PPFJzVXqDuiGev4
   * iqME3mmL5Dw8veWv0BIb5Ylrc5tvJQJLkIKvQMKtuppgJFqBTQUYo+IzeXoLH5Pt
   * 7DlK9RME7I10nYEKqG/odv6LTytpEoYKNDbdgptvT+Bz3Ul/KD7JO6NXBNiT2Twp
   * 2xIQaOHEibgGIOcberyxk2GaGUARtWqFVwHxtlotJnMnlvm5P1vQiJ3koP26TpUJ
   * g3933FEFlJ0gcXax7PqJtZwuhfG5WyRasQmr2soaB82G39tp27RIGAAtvKLEiUUj
   * pQ7hRGU+isFqMB3iYPg6qocJQrmBktwliJiJ8Xw18WLK7nn4GS/+X/jbh87qqA8M
   * pugLoDzga5SYnH+tBuYc6kIQX+ImFTw3OffXvO645e8D7r0i+yiGNFjEWn9hongP
   * XvPKnbwbPKfILfanIhHKA9jnZwqKDss1jjQ52MjqjZ9k4DewbNfFj8GQYSbbJIwe
   * SsCI3zWQzj8C9GRh3sfIB5XeMhg6j6JCQCTl1jNdfK7vsU1P1FeQNWrcrgSXSYk0
   * ly4wBOeY99sLAZDBHwo/+ML+TvrbmnNzFrwFuHnYWa8G5z9nODmxfKuU4CkUpijy
   * 323imttUQ/hHWKNddBWcwauwxzQ=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1%0#\x06\x03U\x04\n\x13\x1cTrustAsia Technologies, Inc.1\"0 \x06\x03U\x04\x03\x13\x19TrustAsia TLS RSA Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc3\x16\xb8\x1bj\xa4Ds\xe5\xd6N\xf4\xb9\xcf[\x0b\xc1\xd1\x9a\x81\xf5c\xb0\x8fC\xc1\xbb\x08Z\x1az\xe1\x07v&\x1f\x8fiV\xbe\xfe6`\xd0\x0c\x83\xcd\x94\xea\xe7\xc5-\\/\x05\x16\x02\x9e\n\xa8/\xe5`&T\x96\xf8\x98@\x1d\xac\xae\x9dtg/T\xfb\x94c\x98\xe3&J\x83\xc6\x95\xb6\tCP\xcd\r}\xdeD\x0e`\x12O[5\xbd\xbf\x998m}l\xda\xe2hs\x1f\xf91\xa5\x19\x10s\x05*\xc32\x19\x85\xea4\xaa\xddB\x1e0\x8d?\x9e\xb5\x1ea\xd5o\xbd\x00rr\xad\x12?\xaa\xa6Is\xf2\x86\x15\x95\x0c\x10_Qd\xce\xffw\xb8\xc9m\xac\xe5\xd5\x98\xf1\x99.l\xe3\xc9D\xf9\xb5C\'\x08M\xf6~\xdeDy\xbb\xb2\x8c\x1c\xda\xd3Kl.\xd6\xc3x\xb7L\xd5\xa4\xe4\xda\xdc\x8a\x8e\x0e\xff\x0f\xc3\xa6`&(\xcf6\xbf\xfaW\n\xec[?\xe90\x16{\xc4\xdb\xc4\xd4d\xa00\xfb\xe5\xfd\x1dq\x92\xdd)\x8fAX\xde\x00\xad:\xfd=|\x1a\xa8\xb1\x17\xf0N4xX%\xd6\x85!\xeby\x1d\xd0\xdc\xab\xcfb<\xb0\xc7\x97\x8b\xd6\xd0\x9f\xd3\xfe<\xde\xc5\xe3\xfc:\x83p\x84!\x1d\t\xc2\xa1\xfc\xbb(!eS`z\x90m\x968[\xff\xf1\xd7z[m\xd3\xc9pIJ\xba\x1d:\xd0P\xda2 \x19\xe4\x8b?\xeb\xd5\x16&7<\xfbu\x9e\xb0\x07p\xb8\x14`w\xdc\xf6\x0f\\R\n\xbc]m\x8dQ\n\xda\xc5\x18\xc8\x9bm\xdc\xb0\x83\xb3\x7f\xa3NxL\x98%\xab\xf2~. ^\x82\x15\xa6\xd6\xd8\x8f\xac\xe5\xcd2\x86\xc8\xf9\xe4\xda\x89\xe2;>\x93\xc5\x13j\xff\xc7\xf7\x18\xfcg\xb5\xef\x18,T\xab@\xd3lM\xed\xc5\xc8\xb78Od\xc4\x94LO\xa0\xcd\xae\xa5\xc6\'%u\'m\xc6\xda\x9c\x8e\xb0\x05\x99\xd5(\x11\xc5!\xd4\xb8\xfc\xc7\x96\xabJdYak\x13\xfcK\xcc\x92\xa6\x0f\xa4Q\xbf\x0eX\x90\xd9\x82\x839\xa6_\xb3\xfan\x92\xb3\xcd3\x0e\x85\xa8\xf1)\xd3\xabY\xad\xd6\xdb\xde\xd4+\x81\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=D-TRUST EV Root CA 1 2020 O=D-Trust GmbH
   * Subject: CN=D-TRUST EV Root CA 1 2020 O=D-Trust GmbH
   * Label: "D-TRUST EV Root CA 1 2020"
   * Serial: 126288379621884218666039612629459926992
   * SHA256 Fingerprint: 08:17:0d:1a:a3:64:53:90:1a:2f:95:92:45:e3:47:db:0c:8d:37:ab:aa:bc:56:b8:1a:a1:00:dc:95:89:70:db
   * -----BEGIN CERTIFICATE-----
   * MIIC2zCCAmCgAwIBAgIQXwJB13qHfEwDo6yWjfv/0DAKBggqhkjOPQQDAzBIMQsw
   * CQYDVQQGEwJERTEVMBMGA1UEChMMRC1UcnVzdCBHbWJIMSIwIAYDVQQDExlELVRS
   * VVNUIEVWIFJvb3QgQ0EgMSAyMDIwMB4XDTIwMDIxMTEwMDAwMFoXDTM1MDIxMTA5
   * NTk1OVowSDELMAkGA1UEBhMCREUxFTATBgNVBAoTDEQtVHJ1c3QgR21iSDEiMCAG
   * A1UEAxMZRC1UUlVTVCBFViBSb290IENBIDEgMjAyMDB2MBAGByqGSM49AgEGBSuB
   * BAAiA2IABPEL3YZDIBnfl4XoIkqbz52Yv7QFJsnL46bSj8WeeHsxiamJrSc8ZRCC
   * /N/DnU7wMyPE0jL1HLDfMxddxfCxivnvubcUyilKwg+pf3VlSSowZ/Rk99Yad9rD
   * wpdhQntJraOCAQ0wggEJMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFH8QARY3
   * OqQo5FD4pPfsazK2/umLMA4GA1UdDwEB/wQEAwIBBjCBxgYDVR0fBIG+MIG7MD6g
   * PKA6hjhodHRwOi8vY3JsLmQtdHJ1c3QubmV0L2NybC9kLXRydXN0X2V2X3Jvb3Rf
   * Y2FfMV8yMDIwLmNybDB5oHegdYZzbGRhcDovL2RpcmVjdG9yeS5kLXRydXN0Lm5l
   * dC9DTj1ELVRSVVNUJTIwRVYlMjBSb290JTIwQ0ElMjAxJTIwMjAyMCxPPUQtVHJ1
   * c3QlMjBHbWJILEM9REU/Y2VydGlmaWNhdGVyZXZvY2F0aW9ubGlzdDAKBggqhkjO
   * PQQDAwNpADBmAjEAyjzGKnXCXnViOTYAYFqLwZOZzNnbQTs7h5kXO9XMT8oi96CA
   * y/m0sRtW9XLS/BnRAjEAkfcwkz8QRitxpNA7RJvAKQIFskF3UfN5Wp6OFKBOQtJb
   * gfM0agPnIjhQW+0ZT0MW
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x13\x0cD-Trust GmbH1\"0 \x06\x03U\x04\x03\x13\x19D-TRUST EV Root CA 1 2020"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xf1\x0b\xdd\x86C \x19\xdf\x97\x85\xe8\"J\x9b\xcf\x9d\x98\xbf\xb4\x05&\xc9\xcb\xe3\xa6\xd2\x8f\xc5\x9ex{1\x89\xa9\x89\xad\'<e\x10\x82\xfc\xdf\xc3\x9dN\xf03#\xc4\xd22\xf5\x1c\xb0\xdf3\x17]\xc5\xf0\xb1\x8a\xf9\xef\xb9\xb7\x14\xca)J\xc2\x0f\xa9\x7fueI*0g\xf4d\xf7\xd6\x1aw\xda\xc3\xc2\x97aB{I\xad"),
    name_constraints: None
  },

  /*
   * Issuer: CN=AffirmTrust Networking O=AffirmTrust
   * Subject: CN=AffirmTrust Networking O=AffirmTrust
   * Label: "AffirmTrust Networking"
   * Serial: 8957382827206547757
   * SHA256 Fingerprint: 0a:81:ec:5a:92:97:77:f1:45:90:4a:f3:8d:5d:50:9f:66:b5:e2:c5:8f:cd:b5:31:05:8b:0e:17:f3:f0:b4:1b
   * -----BEGIN CERTIFICATE-----
   * MIIDTDCCAjSgAwIBAgIIfE8EORzUmS0wDQYJKoZIhvcNAQEFBQAwRDELMAkGA1UE
   * BhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MR8wHQYDVQQDDBZBZmZpcm1UcnVz
   * dCBOZXR3b3JraW5nMB4XDTEwMDEyOTE0MDgyNFoXDTMwMTIzMTE0MDgyNFowRDEL
   * MAkGA1UEBhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MR8wHQYDVQQDDBZBZmZp
   * cm1UcnVzdCBOZXR3b3JraW5nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKC
   * AQEAtITMMxcua5Rsa2FSoOujz3mUTOWUgJnLVWREZY9nZOIG41w3SfYvm4SEHi3y
   * YJ0wTsyEheIszx6e/jarM3c1RNg1lho9Nuh6DtjVR6FqaYvZ/Ls6rnla1fTWcbua
   * kCNrmreIdIcMHl+5ni36q1Mr3Lt2PpNMCAiMHqIjHNRqrSK6mQEubWXLviRmVSRL
   * QESxG9fhwoXA3hA/Pe24/PHxI1Pcv2WXb9n5QHGNfb2V1M6+oF4nI979ptAmDgAp
   * 6zxG8D1gvz9Q0twmQVGeFDdCBKNwV6gbh+0t+nvujArjqWaJGctB+d1ENmHP4ndG
   * yH329JKBNv3bNPFyfvMMFr20FQIDAQABo0IwQDAdBgNVHQ4EFgQUBx/S55zawm6i
   * QLSwelAQUHTEyL0wDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwDQYJ
   * KoZIhvcNAQEFBQADggEBAIlXshZ6qML91tmbmzTCnLQyFE2npN/svqe++EPbkTfO
   * tDIuUFUaNU52Q3Eg75N3ThVwLofDwR1t3Mu1J9QsVtFSUzpE0nPIxBsFZVpikpzu
   * QY0x2+c06lkh1QF612S4ZDnNye2v7UsDSKegmQGA3GWjNq5lWUhPgkvIZfFXHeVZ
   * Lgo/bNjR9eUJtGxUAArgFU2HdW23WJZa3W3SAKD0m0i+wzekujbgfIeFlxoVot4u
   * olu9rxj5kFDNcFn4J2dHy8egBzp90SxdbBk6ZrV9/ZFvgrG+CJPbFEfxojfHRZ48
   * x3evZKiT3/Zpg4Jg8klCNO1aAFSFHBY2kgxc+qatv9s=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x140\x12\x06\x03U\x04\n\x0c\x0bAffirmTrust1\x1f0\x1d\x06\x03U\x04\x03\x0c\x16AffirmTrust Networking"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb4\x84\xcc3\x17.k\x94lkaR\xa0\xeb\xa3\xcfy\x94L\xe5\x94\x80\x99\xcbUdDe\x8fgd\xe2\x06\xe3\\7I\xf6/\x9b\x84\x84\x1e-\xf2`\x9d0N\xcc\x84\x85\xe2,\xcf\x1e\x9e\xfe6\xab3w5D\xd85\x96\x1a=6\xe8z\x0e\xd8\xd5G\xa1ji\x8b\xd9\xfc\xbb:\xaeyZ\xd5\xf4\xd6q\xbb\x9a\x90#k\x9a\xb7\x88t\x87\x0c\x1e_\xb9\x9e-\xfa\xabS+\xdc\xbbv>\x93L\x08\x08\x8c\x1e\xa2#\x1c\xd4j\xad\"\xba\x99\x01.me\xcb\xbe$fU$K@D\xb1\x1b\xd7\xe1\xc2\x85\xc0\xde\x10?=\xed\xb8\xfc\xf1\xf1#S\xdc\xbfe\x97o\xd9\xf9@q\x8d}\xbd\x95\xd4\xce\xbe\xa0^\'#\xde\xfd\xa6\xd0&\x0e\x00)\xeb<F\xf0=`\xbf?P\xd2\xdc&AQ\x9e\x147B\x04\xa3pW\xa8\x1b\x87\xed-\xfa{\xee\x8c\n\xe3\xa9f\x89\x19\xcbA\xf9\xddD6a\xcf\xe2wF\xc8}\xf6\xf4\x92\x816\xfd\xdb4\xf1r~\xf3\x0c\x16\xbd\xb4\x15\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=COMODO Certification Authority O=COMODO CA Limited
   * Subject: CN=COMODO Certification Authority O=COMODO CA Limited
   * Label: "COMODO Certification Authority"
   * Serial: 104350513648249232941998508985834464573
   * SHA256 Fingerprint: 0c:2c:d6:3d:f7:80:6f:a3:99:ed:e8:09:11:6b:57:5b:f8:79:89:f0:65:18:f9:80:8c:86:05:03:17:8b:af:66
   * -----BEGIN CERTIFICATE-----
   * MIIEHTCCAwWgAwIBAgIQToEtioJl4AsC7j41AkblPTANBgkqhkiG9w0BAQUFADCB
   * gTELMAkGA1UEBhMCR0IxGzAZBgNVBAgTEkdyZWF0ZXIgTWFuY2hlc3RlcjEQMA4G
   * A1UEBxMHU2FsZm9yZDEaMBgGA1UEChMRQ09NT0RPIENBIExpbWl0ZWQxJzAlBgNV
   * BAMTHkNPTU9ETyBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTAeFw0wNjEyMDEwMDAw
   * MDBaFw0yOTEyMzEyMzU5NTlaMIGBMQswCQYDVQQGEwJHQjEbMBkGA1UECBMSR3Jl
   * YXRlciBNYW5jaGVzdGVyMRAwDgYDVQQHEwdTYWxmb3JkMRowGAYDVQQKExFDT01P
   * RE8gQ0EgTGltaXRlZDEnMCUGA1UEAxMeQ09NT0RPIENlcnRpZmljYXRpb24gQXV0
   * aG9yaXR5MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA0ECLi3LjkRv3
   * UcEbVASY06m/weaKXTuH+7uIzg3jLz8GlvCiKVCZrts7oVewdFFxze1CkU1B/qnI
   * 2GqGd0S7WWaXUF601CxwRM/aN5VCaTwwxHGzUvAhTaHYujl8HJ6jJJ3ygxaYqhZ8
   * Q5sVW7euNJH+1GImGEaaP+vB+fGQV+useg2L23IwambV4EajcNxo2f8ESIl33rXp
   * +2dtQem8Ob0y2WIC8bGoPW43nOIv4tOiJovGuFVDiOEjPqXSJDlqR6sA1KGzqSX+
   * DT+nHbrTUcELpNqsOO9VUCQFZUaTNE8tja3G1CEZ0o7KBWFxB3NH5YoZEr0ETc5O
   * nKVIrLsm9wIDAQABo4GOMIGLMB0GA1UdDgQWBBQLWOWLxkwVN6RAqTCpIb5HNlpW
   * /zAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB/zBJBgNVHR8EQjBAMD6g
   * PKA6hjhodHRwOi8vY3JsLmNvbW9kb2NhLmNvbS9DT01PRE9DZXJ0aWZpY2F0aW9u
   * QXV0aG9yaXR5LmNybDANBgkqhkiG9w0BAQUFAAOCAQEAPpiem/Yb6dc5t3iuHXIY
   * SdOH5EOC6z/JqvWote9VfCFSZfnVDeFs9D6Mk3ORLgLETgdxb8CPOGEIqB6BCsAv
   * IC9Bi5HcSEW88cbeunZrM8gALTFGTO3nnc+IlP8zwFboJIYmuNg4ON8qa90SzMc/
   * RxdMosIGlgnW2/4/PEZB31jiVg88O8EckzXZOFKs7sjsLjBOlDW0JB9LeGna8gI4
   * zJVSk/BwJVmcIGfE7vmLV2H0knZ9P4SNVbfo5azV8fUZVqZa+5Acr5Pr5RzUZ5dd
   * BA6+C4OmF4O5MBKgxTMVBbkN+8cFduPYSo38NBejxiEovjBFMR7HeL5YYTisO+IB
   * ZQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GB1\x1b0\x19\x06\x03U\x04\x08\x13\x12Greater Manchester1\x100\x0e\x06\x03U\x04\x07\x13\x07Salford1\x1a0\x18\x06\x03U\x04\n\x13\x11COMODO CA Limited1\'0%\x06\x03U\x04\x03\x13\x1eCOMODO Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd0@\x8b\x8br\xe3\x91\x1b\xf7Q\xc1\x1bT\x04\x98\xd3\xa9\xbf\xc1\xe6\x8a];\x87\xfb\xbb\x88\xce\r\xe3/?\x06\x96\xf0\xa2)P\x99\xae\xdb;\xa1W\xb0tQq\xcd\xedB\x91MA\xfe\xa9\xc8\xd8j\x86wD\xbbYf\x97P^\xb4\xd4,pD\xcf\xda7\x95Bi<0\xc4q\xb3R\xf0!M\xa1\xd8\xba9|\x1c\x9e\xa3$\x9d\xf2\x83\x16\x98\xaa\x16|C\x9b\x15[\xb7\xae4\x91\xfe\xd4b&\x18F\x9a?\xeb\xc1\xf9\xf1\x90W\xeb\xacz\r\x8b\xdbr0jf\xd5\xe0F\xa3p\xdch\xd9\xff\x04H\x89w\xde\xb5\xe9\xfbgmA\xe9\xbc9\xbd2\xd9b\x02\xf1\xb1\xa8=n7\x9c\xe2/\xe2\xd3\xa2&\x8b\xc6\xb8UC\x88\xe1#>\xa5\xd2$9jG\xab\x00\xd4\xa1\xb3\xa9%\xfe\r?\xa7\x1d\xba\xd3Q\xc1\x0b\xa4\xda\xac8\xefUP$\x05eF\x934O-\x8d\xad\xc6\xd4!\x19\xd2\x8e\xca\x05aq\x07sG\xe5\x8a\x19\x12\xbd\x04M\xceN\x9c\xa5H\xac\xbb&\xf7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CommScope Public Trust ECC Root-01 O=CommScope
   * Subject: CN=CommScope Public Trust ECC Root-01 O=CommScope
   * Label: "CommScope Public Trust ECC Root-01"
   * Serial: 385011430473757362783587124273108818652468453534
   * SHA256 Fingerprint: 11:43:7c:da:7b:b4:5e:41:36:5f:45:b3:9a:38:98:6b:0d:e0:0d:ef:34:8e:0c:7b:b0:87:36:33:80:0b:c3:8b
   * -----BEGIN CERTIFICATE-----
   * MIICHTCCAaOgAwIBAgIUQ3CCd89NXTTxyq4yLzf39H91oJ4wCgYIKoZIzj0EAwMw
   * TjELMAkGA1UEBhMCVVMxEjAQBgNVBAoMCUNvbW1TY29wZTErMCkGA1UEAwwiQ29t
   * bVNjb3BlIFB1YmxpYyBUcnVzdCBFQ0MgUm9vdC0wMTAeFw0yMTA0MjgxNzM1NDNa
   * Fw00NjA0MjgxNzM1NDJaME4xCzAJBgNVBAYTAlVTMRIwEAYDVQQKDAlDb21tU2Nv
   * cGUxKzApBgNVBAMMIkNvbW1TY29wZSBQdWJsaWMgVHJ1c3QgRUNDIFJvb3QtMDEw
   * djAQBgcqhkjOPQIBBgUrgQQAIgNiAARLNumuV16ocNfQj3Rid8NeeqrltqLxeP0C
   * flfdkXmcbLlSiFS8LwS+uM32ENEp7LXQoMPwiXAZu1FlxUOcw5tjnSCDPgYLpkJE
   * hRGnSjot6dZoL0hOUysHP029uax3OVejQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYD
   * VR0PAQH/BAQDAgEGMB0GA1UdDgQWBBSOB2LAUN3GGQYARnQE9/OufXVNMDAKBggq
   * hkjOPQQDAwNoADBlAjEAnDPfQeMjqEI2Jpc1XHvr20v4qotzVRVcrHgpD7oh2MSg
   * 2NED3W3ROT3Ek2DS43KyAjB8xX6I01D1HiXo+k515liWpDVfG2XqYZpwI7UNo5uS
   * Um9poIyNStDuiw7LR47QjRE=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x0c\tCommScope1+0)\x06\x03U\x04\x03\x0c\"CommScope Public Trust ECC Root-01"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04K6\xe9\xaeW^\xa8p\xd7\xd0\x8ftbw\xc3^z\xaa\xe5\xb6\xa2\xf1x\xfd\x02~W\xdd\x91y\x9cl\xb9R\x88T\xbc/\x04\xbe\xb8\xcd\xf6\x10\xd1)\xec\xb5\xd0\xa0\xc3\xf0\x89p\x19\xbbQe\xc5C\x9c\xc3\x9bc\x9d \x83>\x06\x0b\xa6BD\x85\x11\xa7J:-\xe9\xd6h/HNS+\x07?M\xbd\xb9\xacw9W"),
    name_constraints: None
  },

  /*
   * Issuer: CN=emSign Root CA - C1 O=eMudhra Inc OU=emSign PKI
   * Subject: CN=emSign Root CA - C1 O=eMudhra Inc OU=emSign PKI
   * Label: "emSign Root CA - C1"
   * Serial: 825510296613316004955058
   * SHA256 Fingerprint: 12:56:09:aa:30:1d:a0:a2:49:b9:7a:82:39:cb:6a:34:21:6f:44:dc:ac:9f:39:54:b1:42:92:f2:e8:c8:60:8f
   * -----BEGIN CERTIFICATE-----
   * MIIDczCCAlugAwIBAgILAK7PALrEzzL4Q7IwDQYJKoZIhvcNAQELBQAwVjELMAkG
   * A1UEBhMCVVMxEzARBgNVBAsTCmVtU2lnbiBQS0kxFDASBgNVBAoTC2VNdWRocmEg
   * SW5jMRwwGgYDVQQDExNlbVNpZ24gUm9vdCBDQSAtIEMxMB4XDTE4MDIxODE4MzAw
   * MFoXDTQzMDIxODE4MzAwMFowVjELMAkGA1UEBhMCVVMxEzARBgNVBAsTCmVtU2ln
   * biBQS0kxFDASBgNVBAoTC2VNdWRocmEgSW5jMRwwGgYDVQQDExNlbVNpZ24gUm9v
   * dCBDQSAtIEMxMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAz+upufGZ
   * BczYKCFK83M0UYRWEPWgTywS4/oTmifQz/l5GnRfHXk5/Fv4cI7gklL35CX5VIPZ
   * HdPIWoU/Xse2B+4+wM6ar6xWQio5JXDWv7V7Nq2s9nPczdcdioOl+yuQFTdrHCZH
   * 3DspVpNqs8FqOp099cGXOFgFixwR4+S0uF2FHYP+eF8LRWgYSKVGczQ7/g/IdrvH
   * GPMF0Ybzhe3nudkyrVWIzqa2kbBPrH4VI5b2P/AgNBbeCsbEBEV5f6f9vtKppa+c
   * xSMq9zwhbL2vj07FOrLzNBL834AaSaTUqZX3noleoomslMuoaJuvimUnzYnu3Yy1
   * aylwQ6BpC+S5DwIDAQABo0IwQDAdBgNVHQ4EFgQU/qHgcB4qAzlSWkK+XJGFehiq
   * TbUwDgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQEL
   * BQADggEBAMJKVvoVIXsoounlHfv4LcQ5lkFMOycsxGwYFYDGrK9HWS8mC+M2sO87
   * /kOXSTKZEhVb3xEp/6tT+LvBeA+snFOvV71ojD1pM/CjoCNjO2RnIkSt1XHLVip4
   * kqNPEjE2NuLe/gDEo2APJ62gsIq1NnpSob0n9CAnYuhNlCQT5AoE6TyrLshDCUrG
   * YQTlSTR+08TI9Q/Aqum6VF7zYytPT1DU/rl7mYw9wC68AivTxEDkigcxHpvOJpkT
   * +xHqmiIMERnHXhuBUDDIlhJu58tBf5E7oke3VIAb3ADMmpDqw8NQBmIMMMAVSKeo
   * WXzhriKi4gp6D/piq1JM4fHfyr6DDUI=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x130\x11\x06\x03U\x04\x0b\x13\nemSign PKI1\x140\x12\x06\x03U\x04\n\x13\x0beMudhra Inc1\x1c0\x1a\x06\x03U\x04\x03\x13\x13emSign Root CA - C1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xcf\xeb\xa9\xb9\xf1\x99\x05\xcc\xd8(!J\xf3s4Q\x84V\x10\xf5\xa0O,\x12\xe3\xfa\x13\x9a\'\xd0\xcf\xf9y\x1at_\x1dy9\xfc[\xf8p\x8e\xe0\x92R\xf7\xe4%\xf9T\x83\xd9\x1d\xd3\xc8Z\x85?^\xc7\xb6\x07\xee>\xc0\xce\x9a\xaf\xacVB*9%p\xd6\xbf\xb5{6\xad\xac\xf6s\xdc\xcd\xd7\x1d\x8a\x83\xa5\xfb+\x90\x157k\x1c&G\xdc;)V\x93j\xb3\xc1j:\x9d=\xf5\xc1\x978X\x05\x8b\x1c\x11\xe3\xe4\xb4\xb8]\x85\x1d\x83\xfex_\x0bEh\x18H\xa5Fs4;\xfe\x0f\xc8v\xbb\xc7\x18\xf3\x05\xd1\x86\xf3\x85\xed\xe7\xb9\xd92\xadU\x88\xce\xa6\xb6\x91\xb0O\xac~\x15#\x96\xf6?\xf0 4\x16\xde\n\xc6\xc4\x04Ey\x7f\xa7\xfd\xbe\xd2\xa9\xa5\xaf\x9c\xc5#*\xf7<!l\xbd\xaf\x8fN\xc5:\xb2\xf34\x12\xfc\xdf\x80\x1aI\xa4\xd4\xa9\x95\xf7\x9e\x89^\xa2\x89\xac\x94\xcb\xa8h\x9b\xaf\x8ae\'\xcd\x89\xee\xdd\x8c\xb5k)pC\xa0i\x0b\xe4\xb9\x0f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=COMODO ECC Certification Authority O=COMODO CA Limited
   * Subject: CN=COMODO ECC Certification Authority O=COMODO CA Limited
   * Label: "COMODO ECC Certification Authority"
   * Serial: 41578283867086692638256921589707938090
   * SHA256 Fingerprint: 17:93:92:7a:06:14:54:97:89:ad:ce:2f:8f:34:f7:f0:b6:6d:0f:3a:e3:a3:b8:4d:21:ec:15:db:ba:4f:ad:c7
   * -----BEGIN CERTIFICATE-----
   * MIICiTCCAg+gAwIBAgIQH0evqmIAcFBUTAGem2OZKjAKBggqhkjOPQQDAzCBhTEL
   * MAkGA1UEBhMCR0IxGzAZBgNVBAgTEkdyZWF0ZXIgTWFuY2hlc3RlcjEQMA4GA1UE
   * BxMHU2FsZm9yZDEaMBgGA1UEChMRQ09NT0RPIENBIExpbWl0ZWQxKzApBgNVBAMT
   * IkNPTU9ETyBFQ0MgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkwHhcNMDgwMzA2MDAw
   * MDAwWhcNMzgwMTE4MjM1OTU5WjCBhTELMAkGA1UEBhMCR0IxGzAZBgNVBAgTEkdy
   * ZWF0ZXIgTWFuY2hlc3RlcjEQMA4GA1UEBxMHU2FsZm9yZDEaMBgGA1UEChMRQ09N
   * T0RPIENBIExpbWl0ZWQxKzApBgNVBAMTIkNPTU9ETyBFQ0MgQ2VydGlmaWNhdGlv
   * biBBdXRob3JpdHkwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAAQDR3svdcmCFYX7deSR
   * FtSrYpn1PlILBs5BAH+X4QokPB0BBO490o0JlwzgdeT6+3eKKvUDYEs2ixYjFq0J
   * cfRK9ChQtP6IHG4/bC8vCVlbpVsLM5niwz2J+Wos77LTBumjQjBAMB0GA1UdDgQW
   * BBR1cacZSBm8nZ3qQUfflMRId5nTeTAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/
   * BAUwAwEB/zAKBggqhkjOPQQDAwNoADBlAjEA7wNbeqy3eApyt4jf/7VGFAkK+qDm
   * fQjGGoe9GKhzvSbKYAydzpmfz1wPMOG+FDHqAjAU9JM8SaczepBGR7NjfRObTrdv
   * GDeAU/7dIOA1mjbRxwG55tzd8/8dLDoWV9mSOdY=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GB1\x1b0\x19\x06\x03U\x04\x08\x13\x12Greater Manchester1\x100\x0e\x06\x03U\x04\x07\x13\x07Salford1\x1a0\x18\x06\x03U\x04\n\x13\x11COMODO CA Limited1+0)\x06\x03U\x04\x03\x13\"COMODO ECC Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x03G{/u\xc9\x82\x15\x85\xfbu\xe4\x91\x16\xd4\xabb\x99\xf5>R\x0b\x06\xceA\x00\x7f\x97\xe1\n$<\x1d\x01\x04\xee=\xd2\x8d\t\x97\x0c\xe0u\xe4\xfa\xfbw\x8a*\xf5\x03`K6\x8b\x16#\x16\xad\tq\xf4J\xf4(P\xb4\xfe\x88\x1cn?l//\tY[\xa5[\x0b3\x99\xe2\xc3=\x89\xf9j,\xef\xb2\xd3\x06\xe9"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign O=GlobalSign OU=GlobalSign ECC Root CA - R5
   * Subject: CN=GlobalSign O=GlobalSign OU=GlobalSign ECC Root CA - R5
   * Label: "GlobalSign"
   * Serial: 32785792099990507226680698011560947931244
   * SHA256 Fingerprint: 17:9f:bc:14:8a:3d:d0:0f:d2:4e:a1:34:58:cc:43:bf:a7:f5:9c:81:82:d7:83:a5:13:f6:eb:ec:10:0c:89:24
   * -----BEGIN CERTIFICATE-----
   * MIICHjCCAaSgAwIBAgIRYFlJ4CYuu1X5CneKcflK2GwwCgYIKoZIzj0EAwMwUDEk
   * MCIGA1UECxMbR2xvYmFsU2lnbiBFQ0MgUm9vdCBDQSAtIFI1MRMwEQYDVQQKEwpH
   * bG9iYWxTaWduMRMwEQYDVQQDEwpHbG9iYWxTaWduMB4XDTEyMTExMzAwMDAwMFoX
   * DTM4MDExOTAzMTQwN1owUDEkMCIGA1UECxMbR2xvYmFsU2lnbiBFQ0MgUm9vdCBD
   * QSAtIFI1MRMwEQYDVQQKEwpHbG9iYWxTaWduMRMwEQYDVQQDEwpHbG9iYWxTaWdu
   * MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAER0UOlvt9Xb/pOdEh+J8LttV7HpI6SFkc
   * 8GIxLcB6KP4ap1yztsyX50XUWPrRd21DosCHZTQKH3rd6zwzocWdTaRvQZU4f8ke
   * hOvRnkmSh5SHDDqFSmafnVmTTZdhBoZKo0IwQDAOBgNVHQ8BAf8EBAMCAQYwDwYD
   * VR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUPeYpSJvqB8ohREom3m7e0oPQn1kwCgYI
   * KoZIzj0EAwMDaAAwZQIxAOVpEslu28YxuglB4Zf4+/2a4n0Sye18ZNPLBSWLVtmg
   * 515dTguDnFt2KaAJJiFqYgIwcdK1j1zqO+F4CYWodZI7yFz9SO8NdCKoCOJuxUnO
   * xwy8p2Fp8fc74SrL+SvzZpA3
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1$0\"\x06\x03U\x04\x0b\x13\x1bGlobalSign ECC Root CA - R51\x130\x11\x06\x03U\x04\n\x13\nGlobalSign1\x130\x11\x06\x03U\x04\x03\x13\nGlobalSign"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04GE\x0e\x96\xfb}]\xbf\xe99\xd1!\xf8\x9f\x0b\xb6\xd5{\x1e\x92:HY\x1c\xf0b1-\xc0z(\xfe\x1a\xa7\\\xb3\xb6\xcc\x97\xe7E\xd4X\xfa\xd1wmC\xa2\xc0\x87e4\n\x1fz\xdd\xeb<3\xa1\xc5\x9dM\xa4oA\x958\x7f\xc9\x1e\x84\xeb\xd1\x9eI\x92\x87\x94\x87\x0c:\x85Jf\x9f\x9dY\x93M\x97a\x06\x86J"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Amazon Root CA 3 O=Amazon
   * Subject: CN=Amazon Root CA 3 O=Amazon
   * Label: "Amazon Root CA 3"
   * Serial: 143266986699090766294700635381230934788665930
   * SHA256 Fingerprint: 18:ce:6c:fe:7b:f1:4e:60:b2:e3:47:b8:df:e8:68:cb:31:d0:2e:bb:3a:da:27:15:69:f5:03:43:b4:6d:b3:a4
   * -----BEGIN CERTIFICATE-----
   * MIIBtjCCAVugAwIBAgITBmyf1XSXNmY/Owua2eiedgPySjAKBggqhkjOPQQDAjA5
   * MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g
   * Um9vdCBDQSAzMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG
   * A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg
   * Q0EgMzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCmXp8ZBf8ANm+gBG1bG8lKl
   * ui2yEujSLtf6ycXYqm0fc4E7O5hrOXwzpcVOho6AF2hiRVd9RFgdszflZwjrZt6j
   * QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSr
   * ttvXBp43rDCGB5Fwx5zEGbF4wDAKBggqhkjOPQQDAgNJADBGAiEA4IWSoxe3jfkr
   * BqWTrBqYaGFy+uGh0PsceGCmQ5nFuMQCIQCcAu/xlJyzlvnrxir4tiz+OpAUFteM
   * YyRIHN8wfdVoOw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0f0\r\x06\x03U\x04\n\x13\x06Amazon1\x190\x17\x06\x03U\x04\x03\x13\x10Amazon Root CA 3"),
    subject_public_key_info: Der::from_slice(b"0\x13\x06\x07*\x86H\xce=\x02\x01\x06\x08*\x86H\xce=\x03\x01\x07\x03B\x00\x04)\x97\xa7\xc6A\x7f\xc0\r\x9b\xe8\x01\x1bV\xc6\xf2R\xa5\xba-\xb2\x12\xe8\xd2.\xd7\xfa\xc9\xc5\xd8\xaam\x1fs\x81;;\x98k9|3\xa5\xc5N\x86\x8e\x80\x17hbEW}DX\x1d\xb37\xe5g\x08\xebf\xde"),
    name_constraints: None
  },

  /*
   * Issuer: CN=QuoVadis Root CA 3 O=QuoVadis Limited
   * Subject: CN=QuoVadis Root CA 3 O=QuoVadis Limited
   * Label: "QuoVadis Root CA 3"
   * Serial: 1478
   * SHA256 Fingerprint: 18:f1:fc:7f:20:5d:f8:ad:dd:eb:7f:e0:07:dd:57:e3:af:37:5a:9c:4d:8d:73:54:6b:f4:f1:fe:d1:e1:8d:35
   * -----BEGIN CERTIFICATE-----
   * MIIGnTCCBIWgAwIBAgICBcYwDQYJKoZIhvcNAQEFBQAwRTELMAkGA1UEBhMCQk0x
   * GTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxGzAZBgNVBAMTElF1b1ZhZGlzIFJv
   * b3QgQ0EgMzAeFw0wNjExMjQxOTExMjNaFw0zMTExMjQxOTA2NDRaMEUxCzAJBgNV
   * BAYTAkJNMRkwFwYDVQQKExBRdW9WYWRpcyBMaW1pdGVkMRswGQYDVQQDExJRdW9W
   * YWRpcyBSb290IENBIDMwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDM
   * V0IWVJzmmNPTTe7+7cefQzlKZbPoFog02w1ZkXTPkrgEQK0CSzGrvI2RaNggDhoB
   * 4hp7Thdd4oq3P5kazethq8Jlph+3t723j/z9cI8LoGe+AaJZz3HmDyl2/7FWeUUr
   * H556VOijKTVopAFPD6QuN+8bv+OPEKhyq1hX51SGyMnzW9os2l2ObjyjPtr7guXd
   * 8lyyBTNvijbO0BNO/79KDDRMpsMhvVAEVeuxu537RR5kFd5VAYwCdrXLoT9Cabwv
   * vWhDFlaJKjdhkf2mrk7AyxRllDdLkgbvBNDInIjbC3uBr7E9KsRlOni27tyAsdLT
   * mZw67mtaa7ONt9XOnMK+pUsvFrGeaDsGb659n/je7Mwpp5ijJUMv7/FfJuGITfhe
   * btfZFG4ZM2mnO4SJk8RTVROhUXhA+LjJou57ulJCg54U7QVSWllWp5f8nT8KKdjc
   * T5EOE7zelaTfi5m+rJsziO+1ga8bxiJTyPbH7pcUsMV8eFLI8M5ud2CEpukqdiDt
   * WAEXMJPpGovgc2PZapKUSU60rUqFxKMiMPwJ7Wgic6aIDFUhWMXhOp8q3crhkODZ
   * c6tsgLjoC2SToJyMGf+z0gzskSaHirOi4XCPLArlzW1oUevaPwV/izLmE1xr/l9A
   * 4iLItLRkT9a6fUg+qGkM17uGcclzuD87nSVL2v9A6wIDAQABo4IBlTCCAZEwDwYD
   * VR0TAQH/BAUwAwEB/zCB4QYDVR0gBIHZMIHWMIHTBgkrBgEEAb5YAAMwgcUwgZMG
   * CCsGAQUFBwICMIGGGoGDQW55IHVzZSBvZiB0aGlzIENlcnRpZmljYXRlIGNvbnN0
   * aXR1dGVzIGFjY2VwdGFuY2Ugb2YgdGhlIFF1b1ZhZGlzIFJvb3QgQ0EgMyBDZXJ0
   * aWZpY2F0ZSBQb2xpY3kgLyBDZXJ0aWZpY2F0aW9uIFByYWN0aWNlIFN0YXRlbWVu
   * dC4wLQYIKwYBBQUHAgEWIWh0dHA6Ly93d3cucXVvdmFkaXNnbG9iYWwuY29tL2Nw
   * czALBgNVHQ8EBAMCAQYwHQYDVR0OBBYEFPLAE+CCQz777i9nMpY1XNu4ywLQMG4G
   * A1UdIwRnMGWAFPLAE+CCQz777i9nMpY1XNu4ywLQoUmkRzBFMQswCQYDVQQGEwJC
   * TTEZMBcGA1UEChMQUXVvVmFkaXMgTGltaXRlZDEbMBkGA1UEAxMSUXVvVmFkaXMg
   * Um9vdCBDQSAzggIFxjANBgkqhkiG9w0BAQUFAAOCAgEAT62gLEz6wPJv92ZVqyM0
   * 7ucp2sNbtrCD2dDQ4iH782CnO11gUyeim/YIIirnv6By5ZwkajGxkHon24QRiSem
   * d1o417+shvzuXYO8BsbRd2sPbSQvS3pspweWyuOEn62Iix2rFo1bZhfZFvSLgNLd
   * +LJ2w/w4E6oM3kJpK27zPOuAJ9v1pkQNn1pVWQvVDVJIxa6f8i+AxeoyUDUSly7B
   * 4f/xI4hROJ/yZlZ25w9Rl6VSDE1JUZU2Pb+iSwwQHYaZTKrzchGT5Or2m9qoXadN
   * t54CrnMAyNojA+j56hl0YgCUyyIgvpSnWbWCar6ZeXqp8kokUvd0/bpO5qgdAm6x
   * DYBEwa7TIzdfu4V8K5Iu6H6li92Z4b8nby1dqnuH/grdS/yO9SbkbnBCbjPsMZ57
   * k8HkyWkaPcBrTiJt7qtYTcbQQcEr6k8Sh17rRdhs9ZgC06DYVYoGmRmioHfRMJ6s
   * zHXug/WwYjnPbFfiTNKRCw51KBuav/0aQ/HKd/s7j2G4aSgWQgRecCocIdiP4b0j
   * Wy10QJLZYxkNc91pvGJHvOB0K7Lrfb5BG7XARsWhIstfTsEokt4YutUqKLsRixeT
   * mJlglFwjz1onl14LBQaTNx47aTbrqZ5hHY8y2o4M1nQ+ewkk2gF3R8Q7zTSMmfXK
   * 4SVhM7JZG+Ju1zdXtg2pEto=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BM1\x190\x17\x06\x03U\x04\n\x13\x10QuoVadis Limited1\x1b0\x19\x06\x03U\x04\x03\x13\x12QuoVadis Root CA 3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xccWB\x16T\x9c\xe6\x98\xd3\xd3M\xee\xfe\xed\xc7\x9fC9Je\xb3\xe8\x16\x884\xdb\rY\x91t\xcf\x92\xb8\x04@\xad\x02K1\xab\xbc\x8d\x91h\xd8 \x0e\x1a\x01\xe2\x1a{N\x17]\xe2\x8a\xb7?\x99\x1a\xcd\xeba\xab\xc2e\xa6\x1f\xb7\xb7\xbd\xb7\x8f\xfc\xfdp\x8f\x0b\xa0g\xbe\x01\xa2Y\xcfq\xe6\x0f)v\xff\xb1VyE+\x1f\x9ezT\xe8\xa3)5h\xa4\x01O\x0f\xa4.7\xef\x1b\xbf\xe3\x8f\x10\xa8r\xabXW\xe7T\x86\xc8\xc9\xf3[\xda,\xda]\x8en<\xa3>\xda\xfb\x82\xe5\xdd\xf2\\\xb2\x053o\x8a6\xce\xd0\x13N\xff\xbfJ\x0c4L\xa6\xc3!\xbdP\x04U\xeb\xb1\xbb\x9d\xfbE\x1ed\x15\xdeU\x01\x8c\x02v\xb5\xcb\xa1?Bi\xbc/\xbdhC\x16V\x89*7a\x91\xfd\xa6\xaeN\xc0\xcb\x14e\x947K\x92\x06\xef\x04\xd0\xc8\x9c\x88\xdb\x0b{\x81\xaf\xb1=*\xc4e:x\xb6\xee\xdc\x80\xb1\xd2\xd3\x99\x9c:\xeekZk\xb3\x8d\xb7\xd5\xce\x9c\xc2\xbe\xa5K/\x16\xb1\x9eh;\x06o\xae}\x9f\xf8\xde\xec\xcc)\xa7\x98\xa3%C/\xef\xf1_&\xe1\x88M\xf8^n\xd7\xd9\x14n\x193i\xa7;\x84\x89\x93\xc4SU\x13\xa1Qx@\xf8\xb8\xc9\xa2\xee{\xbaRB\x83\x9e\x14\xed\x05RZYV\xa7\x97\xfc\x9d?\n)\xd8\xdcO\x91\x0e\x13\xbc\xde\x95\xa4\xdf\x8b\x99\xbe\xac\x9b3\x88\xef\xb5\x81\xaf\x1b\xc6\"S\xc8\xf6\xc7\xee\x97\x14\xb0\xc5|xR\xc8\xf0\xcenw`\x84\xa6\xe9*v \xedX\x01\x170\x93\xe9\x1a\x8b\xe0sc\xd9j\x92\x94IN\xb4\xadJ\x85\xc4\xa3\"0\xfc\t\xedh\"s\xa6\x88\x0cU!X\xc5\xe1:\x9f*\xdd\xca\xe1\x90\xe0\xd9s\xabl\x80\xb8\xe8\x0bd\x93\xa0\x9c\x8c\x19\xff\xb3\xd2\x0c\xec\x91&\x87\x8a\xb3\xa2\xe1p\x8f,\n\xe5\xcdmhQ\xeb\xda?\x05\x7f\x8b2\xe6\x13\\k\xfe_@\xe2\"\xc8\xb4\xb4dO\xd6\xba}H>\xa8i\x0c\xd7\xbb\x86q\xc9s\xb8?;\x9d%K\xda\xff@\xeb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SwissSign RSA TLS Root CA 2022 - 1 O=SwissSign AG
   * Subject: CN=SwissSign RSA TLS Root CA 2022 - 1 O=SwissSign AG
   * Label: "SwissSign RSA TLS Root CA 2022 - 1"
   * Serial: 388078645722908516278762308316089881486363258315
   * SHA256 Fingerprint: 19:31:44:f4:31:e0:fd:db:74:07:17:d4:de:92:6a:57:11:33:88:4b:43:60:d3:0e:27:29:13:cb:e6:60:ce:41
   * -----BEGIN CERTIFICATE-----
   * MIIFkzCCA3ugAwIBAgIUQ/oMX04bgBhE79G0TzUfRPSA7cswDQYJKoZIhvcNAQEL
   * BQAwUTELMAkGA1UEBhMCQ0gxFTATBgNVBAoTDFN3aXNzU2lnbiBBRzErMCkGA1UE
   * AxMiU3dpc3NTaWduIFJTQSBUTFMgUm9vdCBDQSAyMDIyIC0gMTAeFw0yMjA2MDgx
   * MTA4MjJaFw00NzA2MDgxMTA4MjJaMFExCzAJBgNVBAYTAkNIMRUwEwYDVQQKEwxT
   * d2lzc1NpZ24gQUcxKzApBgNVBAMTIlN3aXNzU2lnbiBSU0EgVExTIFJvb3QgQ0Eg
   * MjAyMiAtIDEwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDLKmjiC8NX
   * vDVjvHClO/OMPE5Xlm7DTjak9gLKHqquuN6orx122ro10JFwB9+zBvKK8i5VUXu7
   * LCTLf5ImgKO0lPaCoaTo+nUdWfMHamFk4saMla+ju45vVs9xzF6BYQ1t8qsCLqSX
   * 5XH8irCRIFucdFJtrhUnWXjyCcplDn/L9Ovn3KlMd/YrFgSVrpxxpT8q2kFC5zyE
   * EPThPYxr4iuRR1VPuFa+Rd4iUU1OKNlfGUEGjw5NBuBwQCMBauTLE5tzrE0USJIt
   * /m2n+IdreXXhvhCxqohAWVTXz8TQm0SzOGlkjIHRI36qOTw7D59Ke4LKa2/KIj4x
   * 0LDQKhySio/YGZxH5D4MucLNvkEM+KRHBdvBFzA4OmnczcNpI/2aDwLOEGrOyvi5
   * KaM2iYauC8BPY7kGWUleDsFpswrzd34unYyzJ5jSmY0lpx+Gs6ZUcDj8fV3oT4MM
   * 0ZPlEuRU2j7yrTrePjxF8CgPBrnh25d7mUWe3f6VWQQvdT/TromZhqwUtKiE+shd
   * OxtYk8EXlFXIC+OCeYSf8wCENO7cMdWP8vpPlkwGqnj73mSiI80fPsWMvDdUDrta
   * clXvyFu1cvh43zcgTFeRc5JzrBh3Q4IgaezprClG5QtO+DdziZaKHG29777YtvTK
   * wP1H8K4LWCDFyB02rpeNUIMmJCn3nTsPBQIDAQABo2MwYTAPBgNVHRMBAf8EBTAD
   * AQH/MA4GA1UdDwEB/wQEAwIBBjAfBgNVHSMEGDAWgBRvjmKLk0Ow4UD2p8P98Q+4
   * DxU4pTAdBgNVHQ4EFgQUb45ii5NDsOFA9qfD/fEPuA8VOKUwDQYJKoZIhvcNAQEL
   * BQADggIBAKwsKUF9+lz1GpUYvyypiqkkVHX1uECry6gkUSsYP2OprphWKwVDIqO3
   * 10aewCoSPY6WlkDfDDOLazeROpW7OSltwAJsipQLBwJNGD77+3v1dj2b9l4wBlgz
   * Hqp41eZUBDqyggmNzhYzWUUo8aWjlw5DI/0LIICQ/+Mmz7hkkeUFjxOgdg3XNwwQ
   * iJb0Pr6VvfHDffCjw3lHC1ySFWPtUnWK50Zpy1FVCypM9fJkT6lc/2cyjlUtMoIc
   * gC9qkfjLvH4YoiaoLqNTKIftV+Vlek4ASltOU8liNr3CjlvrzG4ngRhZi0Rjn9UM
   * ZfQpZX+RLOV/fuiJz48gy20HQhFRJjKKLjpHE7iNvUcNCfAWpO2Whi4Z2L6MOuhF
   * LhG6rlrnub+xzI/goP+4s9GFe3lmozm1O2bYQL7Pt2eLSMkZJVX8vY3PXtpOpvJp
   * zv1/THfQwUY1mFwjmwJFQ5Ra3bxHrSL+ul4vkSkphnsh3m5kt8sNjzdbowhq6/Td
   * Ao9QAwKxuDdollDruF/UKIqlIgyKhPBZLtU30WHlQnNYKoH3dtvi4k0NX/a3vgW0
   * rk4N3hY9A4GzJl5LuEsAz/+MF7psYC0nhzck5npgL7XTgwSqT0N1osGDsieYK7EO
   * gLrAhV5Cud+xYJHT6xh+cHiudoO+cVrQkOPKwRYlZ0rwtnu64ZzZ
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CH1\x150\x13\x06\x03U\x04\n\x13\x0cSwissSign AG1+0)\x06\x03U\x04\x03\x13\"SwissSign RSA TLS Root CA 2022 - 1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xcb*h\xe2\x0b\xc3W\xbc5c\xbcp\xa5;\xf3\x8c<NW\x96n\xc3N6\xa4\xf6\x02\xca\x1e\xaa\xae\xb8\xde\xa8\xaf\x1dv\xda\xba5\xd0\x91p\x07\xdf\xb3\x06\xf2\x8a\xf2.UQ{\xbb,$\xcb\x7f\x92&\x80\xa3\xb4\x94\xf6\x82\xa1\xa4\xe8\xfau\x1dY\xf3\x07jad\xe2\xc6\x8c\x95\xaf\xa3\xbb\x8eoV\xcfq\xcc^\x81a\rm\xf2\xab\x02.\xa4\x97\xe5q\xfc\x8a\xb0\x91 [\x9ctRm\xae\x15\'Yx\xf2\t\xcae\x0e\x7f\xcb\xf4\xeb\xe7\xdc\xa9Lw\xf6+\x16\x04\x95\xae\x9cq\xa5?*\xdaAB\xe7<\x84\x10\xf4\xe1=\x8ck\xe2+\x91GUO\xb8V\xbeE\xde\"QMN(\xd9_\x19A\x06\x8f\x0eM\x06\xe0p@#\x01j\xe4\xcb\x13\x9bs\xacM\x14H\x92-\xfem\xa7\xf8\x87kyu\xe1\xbe\x10\xb1\xaa\x88@YT\xd7\xcf\xc4\xd0\x9bD\xb38id\x8c\x81\xd1#~\xaa9<;\x0f\x9fJ{\x82\xcako\xca\">1\xd0\xb0\xd0*\x1c\x92\x8a\x8f\xd8\x19\x9cG\xe4>\x0c\xb9\xc2\xcd\xbeA\x0c\xf8\xa4G\x05\xdb\xc1\x1708:i\xdc\xcd\xc3i#\xfd\x9a\x0f\x02\xce\x10j\xce\xca\xf8\xb9)\xa36\x89\x86\xae\x0b\xc0Oc\xb9\x06YI^\x0e\xc1i\xb3\n\xf3w~.\x9d\x8c\xb3\'\x98\xd2\x99\x8d%\xa7\x1f\x86\xb3\xa6Tp8\xfc}]\xe8O\x83\x0c\xd1\x93\xe5\x12\xe4T\xda>\xf2\xad:\xde><E\xf0(\x0f\x06\xb9\xe1\xdb\x97{\x99E\x9e\xdd\xfe\x95Y\x04/u?\xd3\xae\x89\x99\x86\xac\x14\xb4\xa8\x84\xfa\xc8];\x1bX\x93\xc1\x17\x94U\xc8\x0b\xe3\x82y\x84\x9f\xf3\x00\x844\xee\xdc1\xd5\x8f\xf2\xfaO\x96L\x06\xaax\xfb\xded\xa2#\xcd\x1f>\xc5\x8c\xbc7T\x0e\xbbZrU\xef\xc8[\xb5r\xf8x\xdf7 LW\x91s\x92s\xac\x18wC\x82 i\xec\xe9\xac)F\xe5\x0bN\xf87s\x89\x96\x8a\x1cm\xbd\xef\xbe\xd8\xb6\xf4\xca\xc0\xfdG\xf0\xae\x0bX \xc5\xc8\x1d6\xae\x97\x8dP\x83&$)\xf7\x9d;\x0f\x05\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Amazon Root CA 2 O=Amazon
   * Subject: CN=Amazon Root CA 2 O=Amazon
   * Label: "Amazon Root CA 2"
   * Serial: 143266982885963551818349160658925006970653239
   * SHA256 Fingerprint: 1b:a5:b2:aa:8c:65:40:1a:82:96:01:18:f8:0b:ec:4f:62:30:4d:83:ce:c4:71:3a:19:c3:9c:01:1e:a4:6d:b4
   * -----BEGIN CERTIFICATE-----
   * MIIFQTCCAymgAwIBAgITBmyf0pY1hp8KD+WGePhbJruKNzANBgkqhkiG9w0BAQwF
   * ADA5MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6
   * b24gUm9vdCBDQSAyMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTEL
   * MAkGA1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJv
   * b3QgQ0EgMjCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK2Wny2cSkxK
   * gXlRmeyKy2tgURO8TW0G/LAIjd0ZEGrHJgw12MBvIITplLGbhQPDW9tK6Mj4kHbZ
   * W0/jTOgGNk3Mmqw9DJArktQGGWCsN0R5hYGCrVo34A3MnaZMUnbqQ523BNFQ9lXg
   * 1dKmSYXpN+nKfq5clU1Imj+uIFptiJXZNLhSGkOQsL9sBbm2eLfq0OQ6PBJTYv9K
   * 8nu+NQWpEjTj82R0Yiw9AElaKP4yRLuH3WUnAnE72kr3H9rN9yFVkE8P7K6C4Z9r
   * 2UXTu/Bfh+08LDmG2j/e7HJV63mjrdvdfLC6HM783k81ds8P+HgfajZRRidhW+me
   * z/CiVX18JYpvL7TFz4QuK/0NURBs+18bvBt+xa47mAExkv8LV/SasrlX6avvDXbR
   * 8O70zoan4G7ptGmh32n2M8ZpLpcTnqWHsFcQgTfJU7O7f/aS0ZzQGPSSbtqDT6Zj
   * mUyl+17vIWR6IF9sZIUVyzfpYgwLKhbcAS4y2j5L9Z469hdAlO+ekQiG+r5jqFoz
   * 7Mt0Q5X5bGlSNscpb/xVA1wf+5+9R+vnSUeVC06JIglJ4PVhHvG/LopyboBZ/1c6
   * +XUyo05f7O0oYtlNc/LMgRdg7c3r3NunysV+Ar3yVAhU/bQtCSwXVEqY0VThUWcI
   * 0u1ufm8/0i2BWSlmy5A5lREedCf+3euvAgMBAAGjQjBAMA8GA1UdEwEB/wQFMAMB
   * Af8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSwDPBMMPQFWAJI/TPlUq9LhONm
   * UjANBgkqhkiG9w0BAQwFAAOCAgEAqqiAjw54o+Ci1M3m9Zh6O+oAA7CXDpO8Wqj2
   * LIxyh6mx/H9z/WNxeKWHWc8w4Q0QshNabYL1auaAn6AFC2jkR2vHat+2/XcycuUY
   * +gn0oJMsXdKMdYV2ZZAMA3m3MSNjrXiDCYZohMr/+c8mmpJ5581LxedhpxfL86kS
   * k5Nrp+gvU5LEYFiwzAJRGFuFjWJZY7attN6a+yb3ACfAXVU3dJnJUH/jWS5E4ywl
   * 7uxMMne0nxrpS10gxdr9HIcWxkPo1LsmmkVwXqkLN1PiRnsn/eBG8om3zEK2yygm
   * btmlyTrIQRNg91CMFa6ybRoVGld45pIq2WWQgj9sAq+uEjonljYE1x2igGOpm/Hl
   * urR8FLBOybEfdF849lHqm/osohHUqS0nGkWxr7JOcQ3AWEbWaQbLU8uz/mtBzUF+
   * fUwPfHJ5elnNXkoOrJupmHN5fLT0zLm4BwyydFy4x2+IoZCn9Kr5v2c69BoVYh63
   * n749sSmvZ6ES8lgQGVMDMBu4Gon2nL2XA46jCfMdiyHxtN/kHNGfZQIG6lzWE7OE
   * 76KlXIx3KadowGuuQNKotOrN8I1LOJwZmhsoVLiJkO/KdYE+HvJkJMcYr07/R54H
   * 9jVlpNMKVv/1F2Rs76giJUmTtt8AF9pYfl3uxRuw0dFfIRDH+fO6AgonB8Xx1sfT
   * 4PsJYGw=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0f0\r\x06\x03U\x04\n\x13\x06Amazon1\x190\x17\x06\x03U\x04\x03\x13\x10Amazon Root CA 2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xad\x96\x9f-\x9cJLJ\x81yQ\x99\xec\x8a\xcbk`Q\x13\xbcMm\x06\xfc\xb0\x08\x8d\xdd\x19\x10j\xc7&\x0c5\xd8\xc0o \x84\xe9\x94\xb1\x9b\x85\x03\xc3[\xdbJ\xe8\xc8\xf8\x90v\xd9[O\xe3L\xe8\x066M\xcc\x9a\xac=\x0c\x90+\x92\xd4\x06\x19`\xac7Dy\x85\x81\x82\xadZ7\xe0\r\xcc\x9d\xa6LRv\xeaC\x9d\xb7\x04\xd1P\xf6U\xe0\xd5\xd2\xa6I\x85\xe97\xe9\xca~\xae\\\x95MH\x9a?\xae Zm\x88\x95\xd94\xb8R\x1aC\x90\xb0\xbfl\x05\xb9\xb6x\xb7\xea\xd0\xe4:<\x12Sb\xffJ\xf2{\xbe5\x05\xa9\x124\xe3\xf3dtb,=\x00IZ(\xfe2D\xbb\x87\xdde\'\x02q;\xdaJ\xf7\x1f\xda\xcd\xf7!U\x90O\x0f\xec\xae\x82\xe1\x9fk\xd9E\xd3\xbb\xf0_\x87\xed<,9\x86\xda?\xde\xecrU\xeby\xa3\xad\xdb\xdd|\xb0\xba\x1c\xce\xfc\xdeO5v\xcf\x0f\xf8x\x1fj6QF\'a[\xe9\x9e\xcf\xf0\xa2U}|%\x8ao/\xb4\xc5\xcf\x84.+\xfd\rQ\x10l\xfb_\x1b\xbc\x1b~\xc5\xae;\x98\x011\x92\xff\x0bW\xf4\x9a\xb2\xb9W\xe9\xab\xef\rv\xd1\xf0\xee\xf4\xce\x86\xa7\xe0n\xe9\xb4i\xa1\xdfi\xf63\xc6i.\x97\x13\x9e\xa5\x87\xb0W\x10\x817\xc9S\xb3\xbb\x7f\xf6\x92\xd1\x9c\xd0\x18\xf4\x92n\xda\x83O\xa6c\x99L\xa5\xfb^\xef!dz _ld\x85\x15\xcb7\xe9b\x0c\x0b*\x16\xdc\x01.2\xda>K\xf5\x9e:\xf6\x17@\x94\xef\x9e\x91\x08\x86\xfa\xbec\xa8Z3\xec\xcbtC\x95\xf9liR6\xc7)o\xfcU\x03\\\x1f\xfb\x9f\xbdG\xeb\xe7IG\x95\x0bN\x89\"\tI\xe0\xf5a\x1e\xf1\xbf.\x8arn\x80Y\xffW:\xf9u2\xa3N_\xec\xed(b\xd9Ms\xf2\xcc\x81\x17`\xed\xcd\xeb\xdc\xdb\xa7\xca\xc5~\x02\xbd\xf2T\x08T\xfd\xb4-\t,\x17TJ\x98\xd1T\xe1Qg\x08\xd2\xedn~o?\xd2-\x81Y)f\xcb\x909\x95\x11\x1et\'\xfe\xdd\xeb\xaf\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com EV Root Certification Authority ECC O=SSL Corporation
   * Subject: CN=SSL.com EV Root Certification Authority ECC O=SSL Corporation
   * Label: "SSL.com EV Root Certification Authority ECC"
   * Serial: 3182246526754555285
   * SHA256 Fingerprint: 22:a2:c1:f7:bd:ed:70:4c:c1:e7:01:b5:f4:08:c3:10:88:0f:e9:56:b5:de:2a:4a:44:f9:9c:87:3a:25:a7:c8
   * -----BEGIN CERTIFICATE-----
   * MIIClDCCAhqgAwIBAgIILCmcWxbtBZUwCgYIKoZIzj0EAwIwfzELMAkGA1UEBhMC
   * VVMxDjAMBgNVBAgMBVRleGFzMRAwDgYDVQQHDAdIb3VzdG9uMRgwFgYDVQQKDA9T
   * U0wgQ29ycG9yYXRpb24xNDAyBgNVBAMMK1NTTC5jb20gRVYgUm9vdCBDZXJ0aWZp
   * Y2F0aW9uIEF1dGhvcml0eSBFQ0MwHhcNMTYwMjEyMTgxNTIzWhcNNDEwMjEyMTgx
   * NTIzWjB/MQswCQYDVQQGEwJVUzEOMAwGA1UECAwFVGV4YXMxEDAOBgNVBAcMB0hv
   * dXN0b24xGDAWBgNVBAoMD1NTTCBDb3Jwb3JhdGlvbjE0MDIGA1UEAwwrU1NMLmNv
   * bSBFViBSb290IENlcnRpZmljYXRpb24gQXV0aG9yaXR5IEVDQzB2MBAGByqGSM49
   * AgEGBSuBBAAiA2IABKoSR5CYG/vvw0AHgyBO8TCCogbR8pKGYfL2IWjKAMTH6kMA
   * VIbc/R/fALhBYlzccBYy3h+Z1MzFB8gIH2EWB1E9fVwHU+M1OIzfzZ/ZLg1Kthku
   * WnBaBu2+8KGwytAJKaNjMGEwHQYDVR0OBBYEFFvKXuXe0oGqzagtZFG22XKbl+ZP
   * MA8GA1UdEwEB/wQFMAMBAf8wHwYDVR0jBBgwFoAUW8pe5d7SgarNqC1kUbbZcpuX
   * 5k8wDgYDVR0PAQH/BAQDAgGGMAoGCCqGSM49BAMCA2gAMGUCMQCK5kCJN+vp1RPZ
   * ytRrJPOwPYdGWBrssd9v+1a6cGvHOMzosYxPD/fxZ3YOg9AeUY8CMD32IygmTMZg
   * h5Mmm7I1HrrW9zzRHM76JTymGoEVW/MSD2zuZYrJh6j5B+BimoxcSg==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0e0\x0c\x06\x03U\x04\x08\x0c\x05Texas1\x100\x0e\x06\x03U\x04\x07\x0c\x07Houston1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation1402\x06\x03U\x04\x03\x0c+SSL.com EV Root Certification Authority ECC"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xaa\x12G\x90\x98\x1b\xfb\xef\xc3@\x07\x83 N\xf10\x82\xa2\x06\xd1\xf2\x92\x86a\xf2\xf6!h\xca\x00\xc4\xc7\xeaC\x00T\x86\xdc\xfd\x1f\xdf\x00\xb8Ab\\\xdcp\x162\xde\x1f\x99\xd4\xcc\xc5\x07\xc8\x08\x1fa\x16\x07Q=}\\\x07S\xe358\x8c\xdf\xcd\x9f\xd9.\rJ\xb6\x19.ZpZ\x06\xed\xbe\xf0\xa1\xb0\xca\xd0\t)"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Telia Root CA v2 O=Telia Finland Oyj
   * Subject: CN=Telia Root CA v2 O=Telia Finland Oyj
   * Label: "Telia Root CA v2"
   * Serial: 7288924052977061235122729490515358
   * SHA256 Fingerprint: 24:2b:69:74:2f:cb:1e:5b:2a:bf:98:89:8b:94:57:21:87:54:4e:5b:4d:99:11:78:65:73:62:1f:6a:74:b8:2c
   * -----BEGIN CERTIFICATE-----
   * MIIFdDCCA1ygAwIBAgIPAWdfJ9b+euPkrL4JWwWeMA0GCSqGSIb3DQEBCwUAMEQx
   * CzAJBgNVBAYTAkZJMRowGAYDVQQKDBFUZWxpYSBGaW5sYW5kIE95ajEZMBcGA1UE
   * AwwQVGVsaWEgUm9vdCBDQSB2MjAeFw0xODExMjkxMTU1NTRaFw00MzExMjkxMTU1
   * NTRaMEQxCzAJBgNVBAYTAkZJMRowGAYDVQQKDBFUZWxpYSBGaW5sYW5kIE95ajEZ
   * MBcGA1UEAwwQVGVsaWEgUm9vdCBDQSB2MjCCAiIwDQYJKoZIhvcNAQEBBQADggIP
   * ADCCAgoCggIBALLQPwe84nvQa5n44ndp586dpAO8gm2h/oFlH0wnrI4AuhZ76zBq
   * AMCzdGh+sq/H1WKzej9Qyow2RCRj0jbpDIX2Q3bVTKFgcmfiKDOlyzG4OiIjNLh9
   * vVYiQJ3q9HsDrWj8soFPmNB06o3lfc1jw6P23pLCWBnglrvFxKk9pXSW/q/5iaq9
   * lRdU2HhE8Qx3FZLgmEKnpNaqIJLNwaCzlrI6hEKNfdWV5Nbb6WLEWLN5xYzTNTOD
   * n3WhUidhOPFZPY5Q4L15POdslv5e2QJltI5c0BE0312/UqeBAMN/mUWZFdUXyApT
   * 7GPzmX3MaRKGwhfwAZ6/hLzRUssbkmbOpFPlob/E2wnW5olWK8jjfN7j/4nlNW4o
   * 6GwLI1GpJQXrSPjdscr6bAhR77cYbETKJuFzxokGgeWKrLDiKca5JLNrRBH0pUPC
   * TEPlcDaMtjNXepUugqD0XBCzYYP2AgWGLnwtbNwDRm41k9V6lS/eINhbfpSQBGq6
   * WT0EBXWdN6IOLj3rwaRSg/7Qa9RmjtzG6RJOHSpXqhC8fF6CfaamyfItufUXJ63R
   * DolUK5X6wK0dmBR4M0KGCqlztft0DbcbMBnEWg4cJ7faGND/isgFuvGqHKI3t+ZI
   * pEYslOqodmJHixBTB0hXbOKSTbauBcvcwUpej6w9GU7C7WB1K9vBykLVAgMBAAGj
   * YzBhMB8GA1UdIwQYMBaAFHKs5DN5qkWH9v2sHZ7Wxy+G2CQ5MB0GA1UdDgQWBBRy
   * rOQzeapFh/b9rB2e1scvhtgkOTAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUw
   * AwEB/zANBgkqhkiG9w0BAQsFAAOCAgEAoDtZpwmUPjaE0n4vOaWWl/oRrfxn83EJ
   * 8rKJhGdEr7nv7ZbsnGTbMjBvZ5qsfl+yqwE2foH65IRe0qw24GtixX1LDoJt0nZi
   * 0f6X+J8wfBj5tFJ3gh1229MdqfDBmgC9bXXYfef6xzijnHDoRnkDry5023X4blMM
   * A8iZGok1GTzTyVR8qPAs5m4HeW9q4ebqkYJpCh3DflminmtGFZhb069GHWLIzoBS
   * SRE/yQQSwxN8PzuKlts8oB4KtItUsiRnDe+Cy748fdHif64W1lZYudogsYMVoe+K
   * TTJvQS8TUoKU1xrBeKJR3Stwbbca+few4GeXVtt8YVMJAygCQMez2P2ccGrGKMOF
   * 6eLtGpOg3kuYooQ+BXcBlj37tCAPnHICehIv1aO6UXivKitEZU61/Qrowc15h2Er
   * 3oBXRb9n8ZuRXqWk7FlIEA04x7D6w0RtBPV4UBySllva9bguulvP5fBqnUsvWHMt
   * Ty3EHD70sz+rFQ47GUGKpMFXEmZxTPpT41frYpUJnlTd0cI8Vzy9OK2YZLe4A5pT
   * VmBds9hCG1xLEooc6+t9xnppxyd/pPiL8uSUZodL6ZQHCRJ5irLrdATczvREWeAW
   * ysUsWNc8e89ihmpQfTU2Zqf7N+cox9jQraVplI/owd8k+BsHMYeB2F326CjYSlKA
   * rBPuUBQemMc=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02FI1\x1a0\x18\x06\x03U\x04\n\x0c\x11Telia Finland Oyj1\x190\x17\x06\x03U\x04\x03\x0c\x10Telia Root CA v2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb2\xd0?\x07\xbc\xe2{\xd0k\x99\xf8\xe2wi\xe7\xce\x9d\xa4\x03\xbc\x82m\xa1\xfe\x81e\x1fL\'\xac\x8e\x00\xba\x16{\xeb0j\x00\xc0\xb3th~\xb2\xaf\xc7\xd5b\xb3z?P\xca\x8c6D$c\xd26\xe9\x0c\x85\xf6Cv\xd5L\xa1`rg\xe2(3\xa5\xcb1\xb8:\"#4\xb8}\xbdV\"@\x9d\xea\xf4{\x03\xadh\xfc\xb2\x81O\x98\xd0t\xea\x8d\xe5}\xcdc\xc3\xa3\xf6\xde\x92\xc2X\x19\xe0\x96\xbb\xc5\xc4\xa9=\xa5t\x96\xfe\xaf\xf9\x89\xaa\xbd\x95\x17T\xd8xD\xf1\x0cw\x15\x92\xe0\x98B\xa7\xa4\xd6\xaa \x92\xcd\xc1\xa0\xb3\x96\xb2:\x84B\x8d}\xd5\x95\xe4\xd6\xdb\xe9b\xc4X\xb3y\xc5\x8c\xd353\x83\x9fu\xa1R\'a8\xf1Y=\x8eP\xe0\xbdy<\xe7l\x96\xfe^\xd9\x02e\xb4\x8e\\\xd0\x114\xdf]\xbfR\xa7\x81\x00\xc3\x7f\x99E\x99\x15\xd5\x17\xc8\nS\xecc\xf3\x99}\xcci\x12\x86\xc2\x17\xf0\x01\x9e\xbf\x84\xbc\xd1R\xcb\x1b\x92f\xce\xa4S\xe5\xa1\xbf\xc4\xdb\t\xd6\xe6\x89V+\xc8\xe3|\xde\xe3\xff\x89\xe55n(\xe8l\x0b#Q\xa9%\x05\xebH\xf8\xdd\xb1\xca\xfal\x08Q\xef\xb7\x18lD\xca&\xe1s\xc6\x89\x06\x81\xe5\x8a\xac\xb0\xe2)\xc6\xb9$\xb3kD\x11\xf4\xa5C\xc2LC\xe5p6\x8c\xb63Wz\x95.\x82\xa0\xf4\\\x10\xb3a\x83\xf6\x02\x05\x86.|-l\xdc\x03Fn5\x93\xd5z\x95/\xde \xd8[~\x94\x90\x04j\xbaY=\x04\x05u\x9d7\xa2\x0e.=\xeb\xc1\xa4R\x83\xfe\xd0k\xd4f\x8e\xdc\xc6\xe9\x12N\x1d*W\xaa\x10\xbc|^\x82}\xa6\xa6\xc9\xf2-\xb9\xf5\x17\'\xad\xd1\x0e\x89T+\x95\xfa\xc0\xad\x1d\x98\x14x3B\x86\n\xa9s\xb5\xfbt\r\xb7\x1b0\x19\xc4Z\x0e\x1c\'\xb7\xda\x18\xd0\xff\x8a\xc8\x05\xba\xf1\xaa\x1c\xa27\xb7\xe6H\xa4F,\x94\xea\xa8vbG\x8b\x10S\x07HWl\xe2\x92M\xb6\xae\x05\xcb\xdc\xc1J^\x8f\xac=\x19N\xc2\xed`u+\xdb\xc1\xcaB\xd5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Izenpe.com O=IZENPE S.A.
   * Subject: CN=Izenpe.com O=IZENPE S.A.
   * Label: "Izenpe.com"
   * Serial: 917563065490389241595536686991402621
   * SHA256 Fingerprint: 25:30:cc:8e:98:32:15:02:ba:d9:6f:9b:1f:ba:1b:09:9e:2d:29:9e:0f:45:48:bb:91:4f:36:3b:c0:d4:53:1f
   * -----BEGIN CERTIFICATE-----
   * MIIF8TCCA9mgAwIBAgIQALC3WhZIX7/hy/WL1xnmfTANBgkqhkiG9w0BAQsFADA4
   * MQswCQYDVQQGEwJFUzEUMBIGA1UECgwLSVpFTlBFIFMuQS4xEzARBgNVBAMMCkl6
   * ZW5wZS5jb20wHhcNMDcxMjEzMTMwODI4WhcNMzcxMjEzMDgyNzI1WjA4MQswCQYD
   * VQQGEwJFUzEUMBIGA1UECgwLSVpFTlBFIFMuQS4xEzARBgNVBAMMCkl6ZW5wZS5j
   * b20wggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDJ03rKDx6sp4boFmVq
   * scIbRTJxldn+EFvMr+eleQGPicPK8lVx93e+d5TzcqQsRNiekpsUOqHnJJAKClaO
   * xdgmlOHZSOEtPtoKct2jmRXagaKH9HtuJneJWK3W6wyyQXpzbm3benhB6QiIEn6H
   * LmYRY2xU+zydcsC8Lv/Ct90NduM61/e0aL6i9eOBbsFGb12N4E3GVFWJGjMxCrFX
   * uaOKmMPsOzTFlUFpfnXCPCDFYbpRR6AgkJOhkEvzTnyFRVSa0QUmQbC1TR0zvsQD
   * yCV8wXDbO/QJLVQnSKwv4cSsPsjLkkxTOTcj7NMB+eAJRE1NZMDhDVqHIrytG6P+
   * JrUV86f8hBnp7KGItERphIPzidF0BqnMC9bC3ieFUCbKF7jJeodWLBoBHmy+E60Q
   * rLUk9TiRodZL2vG70t5HtfG8gfZZa88ZU+mNFctKy6lvROUbQc/hhqfK0GqfvEyN
   * BjNaooXlkDWgYlwWTvDjovoDGrQscbNYLN57C9saD+veIR8GdwYDsMnvmfzAuU8L
   * hij+0rnq49qlw0dpEuDb8PYZi+17cNcC1u2HGCgsBCRMd+RIihrGO5rUD8r6ddIB
   * QFqNeb+Lz0vPqhbBleStTIo+F5HUsWLlguWABKQDfo2/2n+iD5dPDNMN+9fR5XJ+
   * HMh3/1uaD7euBUbl8agW7EekFwIDAQABo4H2MIHzMIGwBgNVHREEgagwgaWBD2lu
   * Zm9AaXplbnBlLmNvbaSBkTCBjjFHMEUGA1UECgw+SVpFTlBFIFMuQS4gLSBDSUYg
   * QTAxMzM3MjYwLVJNZXJjLlZpdG9yaWEtR2FzdGVpeiBUMTA1NSBGNjIgUzgxQzBB
   * BgNVBAkMOkF2ZGEgZGVsIE1lZGl0ZXJyYW5lbyBFdG9yYmlkZWEgMTQgLSAwMTAx
   * MCBWaXRvcmlhLUdhc3RlaXowDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC
   * AQYwHQYDVR0OBBYEFB0cZQ6o8iV7tJHP5LGx5r1VdGwFMA0GCSqGSIb3DQEBCwUA
   * A4ICAQB4pgwWSp9MiDrAyw6lFn2fuUhfGI8NYjb2zRlrrKvV9pF9rnHzP7MOeIWb
   * laQnIUdCSnxIOvVFfLMMjlF4rJUT3sb9fbgakEyrkgPH7UIBzg/YsfqikuFgba56
   * awmqxinuaElnMIAkejEWOVt+8Rwu3WwJrfIxwYJOubv5vr8qhT/AQKM6WfxZSzwo
   * JNu0FXWuDYi6LnPAvViH5ULy617uHjAimcs30cQhbIHsvm0m5hzkQiCeR7Csg1lw
   * LDXWrzY0tM07+DKo7+N4ifuNRSzanLh+QBxh5z6ikixL8s36mLYp//Pye6kfLqCT
   * VyvehQP5aTfLnnhqBbTFMXiJ7HqnheG5ezzevh55hM6fcA5ZwjUukCox2eRFekGk
   * LhObNA5me0mrZJfQRsN5nXJQY6aYWwa9SG3YOYNw6DXwBdGqvOPbyALqfP2C2sJb
   * UjWumDqtujWTI6cfSN01RpiyEGjkpTHCClguGYEQyVB1/OpaFs4R1+7vUIgtYf8/
   * QnMFlEPVjjxOAToZpR9GTnfQXeWBIiGH/pR9hNiTrdZoQ0iy2+tzJOeRf1SktoA+
   * naM8THLCV8Sg1Mw4J87VBp6iSNnpn86CcDaTmjvfliHjWbcM2pE38P1ZWrOZyGls
   * QyYBNWNgVYkDOnXYukrZVP/u3oDYLdE41V4tC5h9Pmzb/CaIxw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02ES1\x140\x12\x06\x03U\x04\n\x0c\x0bIZENPE S.A.1\x130\x11\x06\x03U\x04\x03\x0c\nIzenpe.com"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc9\xd3z\xca\x0f\x1e\xac\xa7\x86\xe8\x16ej\xb1\xc2\x1bE2q\x95\xd9\xfe\x10[\xcc\xaf\xe7\xa5y\x01\x8f\x89\xc3\xca\xf2Uq\xf7w\xbew\x94\xf3r\xa4,D\xd8\x9e\x92\x9b\x14:\xa1\xe7$\x90\n\nV\x8e\xc5\xd8&\x94\xe1\xd9H\xe1->\xda\nr\xdd\xa3\x99\x15\xda\x81\xa2\x87\xf4{n&w\x89X\xad\xd6\xeb\x0c\xb2Azsnm\xdbzxA\xe9\x08\x88\x12~\x87.f\x11clT\xfb<\x9dr\xc0\xbc.\xff\xc2\xb7\xdd\rv\xe3:\xd7\xf7\xb4h\xbe\xa2\xf5\xe3\x81n\xc1Fo]\x8d\xe0M\xc6TU\x89\x1a31\n\xb1W\xb9\xa3\x8a\x98\xc3\xec;4\xc5\x95Ai~u\xc2< \xc5a\xbaQG\xa0 \x90\x93\xa1\x90K\xf3N|\x85ET\x9a\xd1\x05&A\xb0\xb5M\x1d3\xbe\xc4\x03\xc8%|\xc1p\xdb;\xf4\t-T\'H\xac/\xe1\xc4\xac>\xc8\xcb\x92LS97#\xec\xd3\x01\xf9\xe0\tDMMd\xc0\xe1\rZ\x87\"\xbc\xad\x1b\xa3\xfe&\xb5\x15\xf3\xa7\xfc\x84\x19\xe9\xec\xa1\x88\xb4Di\x84\x83\xf3\x89\xd1t\x06\xa9\xcc\x0b\xd6\xc2\xde\'\x85P&\xca\x17\xb8\xc9z\x87V,\x1a\x01\x1el\xbe\x13\xad\x10\xac\xb5$\xf58\x91\xa1\xd6K\xda\xf1\xbb\xd2\xdeG\xb5\xf1\xbc\x81\xf6Yk\xcf\x19S\xe9\x8d\x15\xcbJ\xcb\xa9oD\xe5\x1bA\xcf\xe1\x86\xa7\xca\xd0j\x9f\xbcL\x8d\x063Z\xa2\x85\xe5\x905\xa0b\\\x16N\xf0\xe3\xa2\xfa\x03\x1a\xb4,q\xb3X,\xde{\x0b\xdb\x1a\x0f\xeb\xde!\x1f\x06w\x06\x03\xb0\xc9\xef\x99\xfc\xc0\xb9O\x0b\x86(\xfe\xd2\xb9\xea\xe3\xda\xa5\xc3Gi\x12\xe0\xdb\xf0\xf6\x19\x8b\xed{p\xd7\x02\xd6\xed\x87\x18(,\x04$Lw\xe4H\x8a\x1a\xc6;\x9a\xd4\x0f\xca\xfau\xd2\x01@Z\x8dy\xbf\x8b\xcfK\xcf\xaa\x16\xc1\x95\xe4\xadL\x8a>\x17\x91\xd4\xb1b\xe5\x82\xe5\x80\x04\xa4\x03~\x8d\xbf\xda\x7f\xa2\x0f\x97O\x0c\xd3\r\xfb\xd7\xd1\xe5r~\x1c\xc8w\xff[\x9a\x0f\xb7\xae\x05F\xe5\xf1\xa8\x16\xecG\xa4\x17\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign O=GlobalSign OU=GlobalSign Root CA - R6
   * Subject: CN=GlobalSign O=GlobalSign OU=GlobalSign Root CA - R6
   * Label: "GlobalSign"
   * Serial: 1417766617973444989252670301619537
   * SHA256 Fingerprint: 2c:ab:ea:fe:37:d0:6c:a2:2a:ba:73:91:c0:03:3d:25:98:29:52:c4:53:64:73:49:76:3a:3a:b5:ad:6c:cf:69
   * -----BEGIN CERTIFICATE-----
   * MIIFgzCCA2ugAwIBAgIORea7A4Mzw4VlSOb/RVEwDQYJKoZIhvcNAQEMBQAwTDEg
   * MB4GA1UECxMXR2xvYmFsU2lnbiBSb290IENBIC0gUjYxEzARBgNVBAoTCkdsb2Jh
   * bFNpZ24xEzARBgNVBAMTCkdsb2JhbFNpZ24wHhcNMTQxMjEwMDAwMDAwWhcNMzQx
   * MjEwMDAwMDAwWjBMMSAwHgYDVQQLExdHbG9iYWxTaWduIFJvb3QgQ0EgLSBSNjET
   * MBEGA1UEChMKR2xvYmFsU2lnbjETMBEGA1UEAxMKR2xvYmFsU2lnbjCCAiIwDQYJ
   * KoZIhvcNAQEBBQADggIPADCCAgoCggIBAJUH6HPKZvnsFMp7PPcNCPG0RQssgrRI
   * xutbPK6DuEGSMxSkb3/pKszGsIhrxbaJ0cay/xTOURQh7ErdG1rG1ofuTToVBu1k
   * ZguSgMpE3nOUTvOniX9PeGMIyBJQbUJmL025eShNUhqKGoC3GYEOfsSKvGRMIRxD
   * aNc9PIrFsmbVkJq3MQbFvuJtMgamHvm566qjuL++gmNQ0PAYid/kD3n16qIfKtJw
   * LnvnvJO7bVPiSHyMEAc4/2ayd2F+4OqMPKq0pPbzlUoSB239jLKJz9CgYXfIWHSw
   * 1CM69106yqLbnQneXUQtkPGBzVeS+n68UARjNN9rkxi+azayOeSsJDa38O+2HBNX
   * k7besvjihbdzorg1qkXy4J02oW9UivFyVm4uiMVRQkQVlO6jxTiWm05OWgtH8wY2
   * SXcwvHE35absIQh1/OZhFj931dmRl4QKbNQCTXTAFO39OfuD8l4UoQSwC+n+7o/h
   * bguyCLNhZglqsQY6ZZZZwPA1/cnaKI0aEYdwgQqomnUdnjqGBQCe24DWJfncBZ4n
   * WUx2OVvq+aWh2IMP0f/fMBH5hc8zSPXKbWQULHpYT9NLCEnFlWQaYw55PfWzjMpY
   * rZxCRXluDocZXFSxZba/jJvcE+kNb7gu3GduyYsRtYQUigAZcIN5kZeR1Bonvzce
   * MgfYFGM8KEyvAgMBAAGjYzBhMA4GA1UdDwEB/wQEAwIBBjAPBgNVHRMBAf8EBTAD
   * AQH/MB0GA1UdDgQWBBSubAWjkxPioufi1xzWx/B/yGdToDAfBgNVHSMEGDAWgBSu
   * bAWjkxPioufi1xzWx/B/yGdToDANBgkqhkiG9w0BAQwFAAOCAgEAgyXt6NH9lVLN
   * nsAEoJFp5lzQhN7craJP6Ed41mWYqVuoPId8AorRbrcWc+ZfwFSY1XS+wc3iEZGt
   * Ixg93eFyRJa0lV7Ae46ZeBZDE1ZXs6KzO7V33EByrKPrmzU+sQghoefEQzd5Mr61
   * 55wsTLxDKZmOMNOsIeDjHfrYBzN2VAAiKrlNIC5waNrlU/yDXNOd8v9EDERm8tLj
   * vUYAGm0CuiVdjaExUd1URhxN25mW7xocBFymFe944Hn+Xds+qkxV/ZoVqW/hpvvf
   * cDDpw+5CRu3CkwWJ+n1jez/QcYF8AOiYrg54NMMl+68KnyBr3TsTjxKM4kEaSHpz
   * oHdpx7Zcf4LIHv5YGygrqGytXm3ABdJ7t+uA/iU3/gKbaKxCXcPu9czc8FB10jZp
   * nOZ7BN9uBmm23goJSFmH63sUYHpkqmlD75HHTOwY3WzvUy2MmeFe8nI+z1TIvWfs
   * pA9MRf/TuTAjB0yPEL+GltmZWrSZVxykzLsViVO6LAUP5MSeGbEYNNVMnbrt9x+v
   * JJUEeKgDu+6B5dpffItKoZB0JaezPkvILFa9x8jvOOJckvB595yEunQtYQEgfn7R
   * 8k8HWV+LLUNS60YMlOH1Zkd5d9VUWx+tJDfLRVpOoERIyNiwmcUVhAn21klJwGW4
   * 5hpxbqCo8YLoRT5s1gLXCmeDBVrJpBA=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1 0\x1e\x06\x03U\x04\x0b\x13\x17GlobalSign Root CA - R61\x130\x11\x06\x03U\x04\n\x13\nGlobalSign1\x130\x11\x06\x03U\x04\x03\x13\nGlobalSign"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x95\x07\xe8s\xcaf\xf9\xec\x14\xca{<\xf7\r\x08\xf1\xb4E\x0b,\x82\xb4H\xc6\xeb[<\xae\x83\xb8A\x923\x14\xa4o\x7f\xe9*\xcc\xc6\xb0\x88k\xc5\xb6\x89\xd1\xc6\xb2\xff\x14\xceQ\x14!\xecJ\xdd\x1bZ\xc6\xd6\x87\xeeM:\x15\x06\xeddf\x0b\x92\x80\xcaD\xdes\x94N\xf3\xa7\x89\x7fOxc\x08\xc8\x12PmBf/M\xb9y(MR\x1a\x8a\x1a\x80\xb7\x19\x81\x0e~\xc4\x8a\xbcdL!\x1cCh\xd7=<\x8a\xc5\xb2f\xd5\x90\x9a\xb71\x06\xc5\xbe\xe2m2\x06\xa6\x1e\xf9\xb9\xeb\xaa\xa3\xb8\xbf\xbe\x82cP\xd0\xf0\x18\x89\xdf\xe4\x0fy\xf5\xea\xa2\x1f*\xd2p.{\xe7\xbc\x93\xbbmS\xe2H|\x8c\x10\x078\xfff\xb2wa~\xe0\xea\x8c<\xaa\xb4\xa4\xf6\xf3\x95J\x12\x07m\xfd\x8c\xb2\x89\xcf\xd0\xa0aw\xc8Xt\xb0\xd4#:\xf7]:\xca\xa2\xdb\x9d\t\xde]D-\x90\xf1\x81\xcdW\x92\xfa~\xbcP\x04c4\xdfk\x93\x18\xbek6\xb29\xe4\xac$6\xb7\xf0\xef\xb6\x1c\x13W\x93\xb6\xde\xb2\xf8\xe2\x85\xb7s\xa2\xb85\xaaE\xf2\xe0\x9d6\xa1oT\x8a\xf1rVn.\x88\xc5QBD\x15\x94\xee\xa3\xc58\x96\x9bNNZ\x0bG\xf3\x066Iw0\xbcq7\xe5\xa6\xec!\x08u\xfc\xe6a\x16?w\xd5\xd9\x91\x97\x84\nl\xd4\x02Mt\xc0\x14\xed\xfd9\xfb\x83\xf2^\x14\xa1\x04\xb0\x0b\xe9\xfe\xee\x8f\xe1n\x0b\xb2\x08\xb3af\tj\xb1\x06:e\x96Y\xc0\xf05\xfd\xc9\xda(\x8d\x1a\x11\x87p\x81\n\xa8\x9au\x1d\x9e:\x86\x05\x00\x9e\xdb\x80\xd6%\xf9\xdc\x05\x9e\'YLv9[\xea\xf9\xa5\xa1\xd8\x83\x0f\xd1\xff\xdf0\x11\xf9\x85\xcf3H\xf5\xcamd\x14,zXO\xd3K\x08I\xc5\x95d\x1ac\x0ey=\xf5\xb3\x8c\xcaX\xad\x9cBEyn\x0e\x87\x19\\T\xb1e\xb6\xbf\x8c\x9b\xdc\x13\xe9\ro\xb8.\xdcgn\xc9\x8b\x11\xb5\x84\x14\x8a\x00\x19p\x83y\x91\x97\x91\xd4\x1a\'\xbf7\x1e2\x07\xd8\x14c<(L\xaf\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Starfield Root Certificate Authority - G2 O=Starfield Technologies, Inc.
   * Subject: CN=Starfield Root Certificate Authority - G2 O=Starfield Technologies, Inc.
   * Label: "Starfield Root Certificate Authority - G2"
   * Serial: 0
   * SHA256 Fingerprint: 2c:e1:cb:0b:f9:d2:f9:e1:02:99:3f:be:21:51:52:c3:b2:dd:0c:ab:de:1c:68:e5:31:9b:83:91:54:db:b7:f5
   * -----BEGIN CERTIFICATE-----
   * MIID3TCCAsWgAwIBAgIBADANBgkqhkiG9w0BAQsFADCBjzELMAkGA1UEBhMCVVMx
   * EDAOBgNVBAgTB0FyaXpvbmExEzARBgNVBAcTClNjb3R0c2RhbGUxJTAjBgNVBAoT
   * HFN0YXJmaWVsZCBUZWNobm9sb2dpZXMsIEluYy4xMjAwBgNVBAMTKVN0YXJmaWVs
   * ZCBSb290IENlcnRpZmljYXRlIEF1dGhvcml0eSAtIEcyMB4XDTA5MDkwMTAwMDAw
   * MFoXDTM3MTIzMTIzNTk1OVowgY8xCzAJBgNVBAYTAlVTMRAwDgYDVQQIEwdBcml6
   * b25hMRMwEQYDVQQHEwpTY290dHNkYWxlMSUwIwYDVQQKExxTdGFyZmllbGQgVGVj
   * aG5vbG9naWVzLCBJbmMuMTIwMAYDVQQDEylTdGFyZmllbGQgUm9vdCBDZXJ0aWZp
   * Y2F0ZSBBdXRob3JpdHkgLSBHMjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoC
   * ggEBAL3twQP89o/8ArFvW59I2Z154qK3A2FWGMNHttfKPTUuiUP3oWmb3ooa/RMg
   * nLRJdzIpVv257IzdIvpy3Cdhl+72WoTsbhm5iSzchFvVdPtrX8WJpRBSiUZV9Lh1
   * HOZ/5FSuS/hVclcCGfgXcVnrHigHdMWdSL5stPSksPNkN3mSwOxGXn/hbVNMYq/N
   * Hwtjuzqd+/x5AJhhdM8mgkBj87JyahkNmcrUDnXMN/uLicFZ8WJ/X7NfZTD4p7dN
   * dloedl40wOiWVpmKs/B/pM293DIxfJHP4F8R+GuqSVzRmZTRouNjWwl2tVZi4Ut0
   * HZbUJtQIBFnQmA4O5t78w+wfkPECAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAO
   * BgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFHwMMh+n2TB/xH1oo2Kooc6rB1snMA0G
   * CSqGSIb3DQEBCwUAA4IBAQARWfolTwNvlJk7mh+ChTnUdgWUXuEok21iXQnCoKjU
   * sHU48TRqneSfioYmUeYs0cYtbpUgSpIB7LiKZ3sx4mcujJUDJi5DnUox9g61DLu3
   * 4jd/IroAow57UvtruzvE03lRTs2Q9GcHGcg8RnoNAX3FWOdt5oUwF5okxBDgBPfg
   * 8n/Uqgr/Qh037ZTlZFkSIHc40zI+OIF1lnP6aI+xy84fxez6nH7PfrHxBy22/L/K
   * pL/QlwVKvOoYKAKQvVR4CSFx09F9HdkWsKlhPdAKACL8x3vLCWRFCztAgfd9fDL1
   * mMpYjn0q7pBZc2T5NnReJaH1ZgUufzkVqSr7UIuOhWn0
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x100\x0e\x06\x03U\x04\x08\x13\x07Arizona1\x130\x11\x06\x03U\x04\x07\x13\nScottsdale1%0#\x06\x03U\x04\n\x13\x1cStarfield Technologies, Inc.1200\x06\x03U\x04\x03\x13)Starfield Root Certificate Authority - G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xbd\xed\xc1\x03\xfc\xf6\x8f\xfc\x02\xb1o[\x9fH\xd9\x9dy\xe2\xa2\xb7\x03aV\x18\xc3G\xb6\xd7\xca=5.\x89C\xf7\xa1i\x9b\xde\x8a\x1a\xfd\x13 \x9c\xb4Iw2)V\xfd\xb9\xec\x8c\xdd\"\xfar\xdc\'a\x97\xee\xf6Z\x84\xecn\x19\xb9\x89,\xdc\x84[\xd5t\xfbk_\xc5\x89\xa5\x10R\x89FU\xf4\xb8u\x1c\xe6\x7f\xe4T\xaeK\xf8UrW\x02\x19\xf8\x17qY\xeb\x1e(\x07t\xc5\x9dH\xbel\xb4\xf4\xa4\xb0\xf3d7y\x92\xc0\xecF^\x7f\xe1mSLb\xaf\xcd\x1f\x0bc\xbb:\x9d\xfb\xfcy\x00\x98at\xcf&\x82@c\xf3\xb2rj\x19\r\x99\xca\xd4\x0eu\xcc7\xfb\x8b\x89\xc1Y\xf1b\x7f_\xb3_e0\xf8\xa7\xb7MvZ\x1ev^4\xc0\xe8\x96V\x99\x8a\xb3\xf0\x7f\xa4\xcd\xbd\xdc21|\x91\xcf\xe0_\x11\xf8k\xaaI\\\xd1\x99\x94\xd1\xa2\xe3c[\tv\xb5Vb\xe1Kt\x1d\x96\xd4&\xd4\x08\x04Y\xd0\x98\x0e\x0e\xe6\xde\xfc\xc3\xec\x1f\x90\xf1\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TunTrust Root CA O=Agence Nationale de Certification Electronique
   * Subject: CN=TunTrust Root CA O=Agence Nationale de Certification Electronique
   * Label: "TunTrust Root CA"
   * Serial: 108534058042236574382096126452369648152337120275
   * SHA256 Fingerprint: 2e:44:10:2a:b5:8c:b8:54:19:45:1c:8e:19:d9:ac:f3:66:2c:af:bc:61:4b:6a:53:96:0a:30:f7:d0:e2:eb:41
   * -----BEGIN CERTIFICATE-----
   * MIIFszCCA5ugAwIBAgIUEwLV4kBMkkaGFmddtLu7sms+/BMwDQYJKoZIhvcNAQEL
   * BQAwYTELMAkGA1UEBhMCVE4xNzA1BgNVBAoMLkFnZW5jZSBOYXRpb25hbGUgZGUg
   * Q2VydGlmaWNhdGlvbiBFbGVjdHJvbmlxdWUxGTAXBgNVBAMMEFR1blRydXN0IFJv
   * b3QgQ0EwHhcNMTkwNDI2MDg1NzU2WhcNNDQwNDI2MDg1NzU2WjBhMQswCQYDVQQG
   * EwJUTjE3MDUGA1UECgwuQWdlbmNlIE5hdGlvbmFsZSBkZSBDZXJ0aWZpY2F0aW9u
   * IEVsZWN0cm9uaXF1ZTEZMBcGA1UEAwwQVHVuVHJ1c3QgUm9vdCBDQTCCAiIwDQYJ
   * KoZIhvcNAQEBBQADggIPADCCAgoCggIBAMPN0/y9BFPdDCA61YguBUtB9YOCfvdZ
   * n56eY+hz2vYGqU8ftPkLHzmMmiDQfgbU7DTZhrx1W4eI8NLZ1KMKsmwb60ksPqxd
   * 2JQDoOw05TDENX37Jk0bbjBU2PWARZw5rZzJJQRNmpA+TkBuimvNKWfGzC3gdOgF
   * VwpIUPp6Q9p+7FuaDmJ2/uqdHYVy7BG7NegfJ7/Boce7SBbdVtfMTqDhuazb1YMZ
   * GoXRlJfXyqNlC/M4+QKu3fZnz8k/9YosRxqZbwUN/dAdgjH8KcwAWJeRTIAAHDOF
   * li/LQcKLEITDCSSJH7UP2dl3RxiSlGBcx5kDPP73lad9UKGAwqmDrViWVSHbhlnU
   * r8a83YFuB9tgYv7sEG7aaAH0gxupPqJbI9dkxt/con3YS7qC0lH4Zr8GRuR5KiY2
   * eY8fTpkdso8MDhz/yV3A/ZAQprE38806JG60hZC/gLkMjNWb1sjxVj8agIl6qeIb
   * MlEsPvLfe/ZdeikZjuXIvTZxi11Mwh0/rViizz1wTaZQmCXcI/m4WEEIcb9PuISg
   * jwBUFfyRbVinljvrS5YnzWuioYasDXxU5mZMZl+QviGaAkYt5IPCgLnPSz7ofzwB
   * 7I9ezX/SKEIBlYrilz0QIX32nRzFNKHsLA4KUiwSVXAkPcvCFDVDXSdOvsC9qnyW
   * 5/yeYa1E0wCXAgMBAAGjYzBhMB0GA1UdDgQWBBQGmpsfU33x9aTI04Y+oXNZtPdE
   * ITAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFAaamx9TffH1pMjThj6hc1m0
   * 90QhMA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAqgVutt0Vyb+z
   * xiD2BkewhpMl0425yAA/l/VSJ4hxyXT968pk21vvHl26v9Hr7lxpuhbI87mP0zYu
   * QEkHDVneixCwSQXi/5E/S7fdAo74gShczNxtr18UnH1YeA32gAm56Q6XKRm4t+v4
   * FstVEuTGfbvE7Pi1HE4+Z7/FXxttbUcoqgRYYdZ2vyJ/0Adqp2RT8JeNnYA/u8EH
   * 22Wv5psymsNUk8QcCMNE+3tjEUPRahphanltkE8pjkcFwRJpadbGNjHh/PqAulxP
   * xOu3Mqz4dWEX1xAZufHSCe96Qp1bWgvUxpVOKs7/B9dPfhgGiPEZtdmYu65xxBzn
   * dFlY7wyJz4sfdZMaBBSSSFCp61cpABbjNhzI+L/wM9VBD8TMPN3pM0MBkRArHtG5
   * Xc0yGYuPjCB31yLEQtyEFpslbei0VXF/sHyz03FJuc9SpAQ/3D2gu68zngowYI7b
   * nV2UqL1g52KAdoGDDIzMMEZJ4gzSqK/rYXHv5yJiqfdcZGyfFoxnNidF9Ql7v/YQ
   * CvGwjVRDjAS6oz/v4jXH+XTgbzRB0L9zZVcg+ZtnemZoJE6AZb0QmQZZ8mWvuMZH
   * u/2QeItBcy6vVR/cO5JyboTT0GFMDcx2V+IthSIVNg3rAZ3r2OvEhJn7wAzMMujj
   * d9qDRIueVSjAi1jTkD5OGwDxFa2DK5o=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TN1705\x06\x03U\x04\n\x0c.Agence Nationale de Certification Electronique1\x190\x17\x06\x03U\x04\x03\x0c\x10TunTrust Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc3\xcd\xd3\xfc\xbd\x04S\xdd\x0c :\xd5\x88.\x05KA\xf5\x83\x82~\xf7Y\x9f\x9e\x9ec\xe8s\xda\xf6\x06\xa9O\x1f\xb4\xf9\x0b\x1f9\x8c\x9a \xd0~\x06\xd4\xec4\xd9\x86\xbcu[\x87\x88\xf0\xd2\xd9\xd4\xa3\n\xb2l\x1b\xebI,>\xac]\xd8\x94\x03\xa0\xec4\xe50\xc45}\xfb&M\x1bn0T\xd8\xf5\x80E\x9c9\xad\x9c\xc9%\x04M\x9a\x90>N@n\x8ak\xcd)g\xc6\xcc-\xe0t\xe8\x05W\nHP\xfazC\xda~\xec[\x9a\x0ebv\xfe\xea\x9d\x1d\x85r\xec\x11\xbb5\xe8\x1f\'\xbf\xc1\xa1\xc7\xbbH\x16\xddV\xd7\xccN\xa0\xe1\xb9\xac\xdb\xd5\x83\x19\x1a\x85\xd1\x94\x97\xd7\xca\xa3e\x0b\xf38\xf9\x02\xae\xdd\xf6g\xcf\xc9?\xf5\x8a,G\x1a\x99o\x05\r\xfd\xd0\x1d\x821\xfc)\xcc\x00X\x97\x91L\x80\x00\x1c3\x85\x96/\xcbA\xc2\x8b\x10\x84\xc3\t$\x89\x1f\xb5\x0f\xd9\xd9wG\x18\x92\x94`\\\xc7\x99\x03<\xfe\xf7\x95\xa7}P\xa1\x80\xc2\xa9\x83\xadX\x96U!\xdb\x86Y\xd4\xaf\xc6\xbc\xdd\x81n\x07\xdb`b\xfe\xec\x10n\xdah\x01\xf4\x83\x1b\xa9>\xa2[#\xd7d\xc6\xdf\xdc\xa2}\xd8K\xba\x82\xd2Q\xf8f\xbf\x06F\xe4y*&6y\x8f\x1fN\x99\x1d\xb2\x8f\x0c\x0e\x1c\xff\xc9]\xc0\xfd\x90\x10\xa6\xb17\xf3\xcd:$n\xb4\x85\x90\xbf\x80\xb9\x0c\x8c\xd5\x9b\xd6\xc8\xf1V?\x1a\x80\x89z\xa9\xe2\x1b2Q,>\xf2\xdf{\xf6]z)\x19\x8e\xe5\xc8\xbd6q\x8b]L\xc2\x1d?\xadX\xa2\xcf=pM\xa6P\x98%\xdc#\xf9\xb8XA\x08q\xbfO\xb8\x84\xa0\x8f\x00T\x15\xfc\x91mX\xa7\x96;\xebK\x96\'\xcdk\xa2\xa1\x86\xac\r|T\xe6fLf_\x90\xbe!\x9a\x02F-\xe4\x83\xc2\x80\xb9\xcfK>\xe8\x7f<\x01\xec\x8f^\xcd\x7f\xd2(B\x01\x95\x8a\xe2\x97=\x10!}\xf6\x9d\x1c\xc54\xa1\xec,\x0e\nR,\x12Up$=\xcb\xc2\x145C]\'N\xbe\xc0\xbd\xaa|\x96\xe7\xfc\x9ea\xadD\xd3\x00\x97\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com EV Root Certification Authority RSA R2 O=SSL Corporation
   * Subject: CN=SSL.com EV Root Certification Authority RSA R2 O=SSL Corporation
   * Label: "SSL.com EV Root Certification Authority RSA R2"
   * Serial: 6248227494352943350
   * SHA256 Fingerprint: 2e:7b:f1:6c:c2:24:85:a7:bb:e2:aa:86:96:75:07:61:b0:ae:39:be:3b:2f:e9:d0:cc:6d:4e:f7:34:91:42:5c
   * -----BEGIN CERTIFICATE-----
   * MIIF6zCCA9OgAwIBAgIIVrYpzTS8ePYwDQYJKoZIhvcNAQELBQAwgYIxCzAJBgNV
   * BAYTAlVTMQ4wDAYDVQQIDAVUZXhhczEQMA4GA1UEBwwHSG91c3RvbjEYMBYGA1UE
   * CgwPU1NMIENvcnBvcmF0aW9uMTcwNQYDVQQDDC5TU0wuY29tIEVWIFJvb3QgQ2Vy
   * dGlmaWNhdGlvbiBBdXRob3JpdHkgUlNBIFIyMB4XDTE3MDUzMTE4MTQzN1oXDTQy
   * MDUzMDE4MTQzN1owgYIxCzAJBgNVBAYTAlVTMQ4wDAYDVQQIDAVUZXhhczEQMA4G
   * A1UEBwwHSG91c3RvbjEYMBYGA1UECgwPU1NMIENvcnBvcmF0aW9uMTcwNQYDVQQD
   * DC5TU0wuY29tIEVWIFJvb3QgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkgUlNBIFIy
   * MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAjzZlQOHWTcDXtOlG2mvq
   * M0fNTPl9fb69LT3w23jhhqXZuglXaO1XPqDQCEGD5yhBJB/jchXQARr7XnAjssuf
   * OePPxU7Gkm0mxnu7s9onnQqG6YE3Bf7wcXHswxzpY6IXFJ3vG2fThVUCAtZJycxa
   * 4bH3bzKfydQ7iEGonL3Lq9ttewkfokxykNorCPzPPFTOZw+oz12WGQvE43LrrdF9
   * HSfvkusQv1vrO6/PgN3B0pYEW3p+pKk8OHakYo6gOV7qd89dAFmPZiw+B6KjBSYR
   * aZfqhbcPlgtLyEDhULouisv3D5oi53+aNxPN8k0TayHRwMwi8qFG9kRpnMphNQcA
   * b9ZhCBHqurj26bNg5U257J8UZslXWNvNh2n4ioYSA0e/ZhN2rHd9NCSFg83XqpyQ
   * Gp8hLH94t2S42Oim9HizVcuE0jLEeK6jj2HdzghTreyI/BXkmg3mnxp3zkyPuBQV
   * PWKchjgGAGYS5Fl2WlPAApiiECtoRHuOec4zSnaqW4EWG7WK2NAAe15itAnWhmMO
   * pgWVSbooi4iTsjQc2KRVbrcc0N6ZVTsj9CLg+SlmJuwgUHfbSguPvuUCYHBBXtSu
   * UDkiFCbLsjtzdFVHB3mBOagwE0TlBIqulhMlQg+5U8Sb/M3kHN48+qvWBkofZ6aY
   * MBzdLNvcGJVXZsb/XItW9XcCAwEAAaNjMGEwDwYDVR0TAQH/BAUwAwEB/zAfBgNV
   * HSMEGDAWgBT5YLvU49U09rj1BoAlp3PbRmmonjAdBgNVHQ4EFgQU+WC71OPVNPa4
   * 9QaAJadz20ZpqJ4wDgYDVR0PAQH/BAQDAgGGMA0GCSqGSIb3DQEBCwUAA4ICAQBW
   * s47LCp1Jjr+kxJG7ZhcFUZh1++VQLHqe8RT6q9OKPv+RKY9ji9i0qVQBDb6Thi/5
   * Sm3HXvVX+cpVHBK+Rw82xd9qt9t1wkclf7nxY/hoLVUE0fKNsKTPvDxeH3jnpaAg
   * cLAExbf3cqfeIg29MyVGjGSSJuM+LmOW2puMPfgYCdcDzH2GguDKBAdRUNf/ktUM
   * 79qGn5nX67evaOI5JpS6aLe/g9Pqemc9YmeuJeVy6OLk7K4S9ksrPJ/psEDzOFSz
   * /bdoyNrGj1E8svuR3Bznm53htw1yj+KkxKl4+esUrMZDBcJlOSgYAsOCsp0FvmXt
   * ll9ldDz7CTUue5wT/RsPXcdtgTpWD8w74a8CLyKsRspGPKAcTNZEtF4uXBVmCeEm
   * Kf7GUmG6sXP/wwyc5WxqlD8UykAWlYTzWamsX0xhk23RO8yilQwipmdnRC652dKK
   * QbNmC1r7fSOl8hqw/96bg5Qu0T/fkreRrwU7ZcegbLHNYhLDkBvjJc40vG93drEQ
   * w/cFGsDWr3RiSBd3kmmQYRzelYB0VI8YHMPzA9C/pEN1hlMYegouCRw2n5H9gooi
   * S9EOUCXdywMMF8mDAAhONU2Ki+3wApRmLER/y5UnlhetCTCstnEXbosX9hwJ1C07
   * mKVx01QT2WDz9UtmT/rx7iASjbSsV7FFY6GsdqnC+w==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0e0\x0c\x06\x03U\x04\x08\x0c\x05Texas1\x100\x0e\x06\x03U\x04\x07\x0c\x07Houston1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation1705\x06\x03U\x04\x03\x0c.SSL.com EV Root Certification Authority RSA R2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x8f6e@\xe1\xd6M\xc0\xd7\xb4\xe9F\xdak\xea3G\xcdL\xf9}}\xbe\xbd-=\xf0\xdbx\xe1\x86\xa5\xd9\xba\tWh\xedW>\xa0\xd0\x08A\x83\xe7(A$\x1f\xe3r\x15\xd0\x01\x1a\xfb^p#\xb2\xcb\x9f9\xe3\xcf\xc5N\xc6\x92m&\xc6{\xbb\xb3\xda\'\x9d\n\x86\xe9\x817\x05\xfe\xf0qq\xec\xc3\x1c\xe9c\xa2\x17\x14\x9d\xef\x1bg\xd3\x85U\x02\x02\xd6I\xc9\xccZ\xe1\xb1\xf7o2\x9f\xc9\xd4;\x88A\xa8\x9c\xbd\xcb\xab\xdbm{\t\x1f\xa2Lr\x90\xda+\x08\xfc\xcf<T\xceg\x0f\xa8\xcf]\x96\x19\x0b\xc4\xe3r\xeb\xad\xd1}\x1d\'\xef\x92\xeb\x10\xbf[\xeb;\xaf\xcf\x80\xdd\xc1\xd2\x96\x04[z~\xa4\xa9<8v\xa4b\x8e\xa09^\xeaw\xcf]\x00Y\x8ff,>\x07\xa2\xa3\x05&\x11i\x97\xea\x85\xb7\x0f\x96\x0bK\xc8@\xe1P\xba.\x8a\xcb\xf7\x0f\x9a\"\xe7\x7f\x9a7\x13\xcd\xf2M\x13k!\xd1\xc0\xcc\"\xf2\xa1F\xf6Di\x9c\xcaa5\x07\x00o\xd6a\x08\x11\xea\xba\xb8\xf6\xe9\xb3`\xe5M\xb9\xec\x9f\x14f\xc9WX\xdb\xcd\x87i\xf8\x8a\x86\x12\x03G\xbff\x13v\xacw}4$\x85\x83\xcd\xd7\xaa\x9c\x90\x1a\x9f!,\x7fx\xb7d\xb8\xd8\xe8\xa6\xf4x\xb3U\xcb\x84\xd22\xc4x\xae\xa3\x8fa\xdd\xce\x08S\xad\xec\x88\xfc\x15\xe4\x9a\r\xe6\x9f\x1aw\xceL\x8f\xb8\x14\x15=b\x9c\x868\x06\x00f\x12\xe4YvZS\xc0\x02\x98\xa2\x10+hD{\x8ey\xce3Jv\xaa[\x81\x16\x1b\xb5\x8a\xd8\xd0\x00{^b\xb4\t\xd6\x86c\x0e\xa6\x05\x95I\xba(\x8b\x88\x93\xb24\x1c\xd8\xa4Un\xb7\x1c\xd0\xde\x99U;#\xf4\"\xe0\xf9)f&\xec Pw\xdbJ\x0b\x8f\xbe\xe5\x02`pA^\xd4\xaeP9\"\x14&\xcb\xb2;stUG\x07y\x819\xa80\x13D\xe5\x04\x8a\xae\x96\x13%B\x0f\xb9S\xc4\x9b\xfc\xcd\xe4\x1c\xde<\xfa\xab\xd6\x06J\x1fg\xa6\x980\x1c\xdd,\xdb\xdc\x18\x95Wf\xc6\xff\\\x8bV\xf5w\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CommScope Public Trust ECC Root-02 O=CommScope
   * Subject: CN=CommScope Public Trust ECC Root-02 O=CommScope
   * Label: "CommScope Public Trust ECC Root-02"
   * Serial: 234015080301808452132356021271193974922492992893
   * SHA256 Fingerprint: 2f:fb:7f:81:3b:bb:b3:c8:9a:b4:e8:16:2d:0f:16:d7:15:09:a8:30:cc:9d:73:c2:62:e5:14:08:75:d1:ad:4a
   * -----BEGIN CERTIFICATE-----
   * MIICHDCCAaOgAwIBAgIUKP2ZYEFHpgE6yhR7H+/5aAiDXX0wCgYIKoZIzj0EAwMw
   * TjELMAkGA1UEBhMCVVMxEjAQBgNVBAoMCUNvbW1TY29wZTErMCkGA1UEAwwiQ29t
   * bVNjb3BlIFB1YmxpYyBUcnVzdCBFQ0MgUm9vdC0wMjAeFw0yMTA0MjgxNzQ0NTRa
   * Fw00NjA0MjgxNzQ0NTNaME4xCzAJBgNVBAYTAlVTMRIwEAYDVQQKDAlDb21tU2Nv
   * cGUxKzApBgNVBAMMIkNvbW1TY29wZSBQdWJsaWMgVHJ1c3QgRUNDIFJvb3QtMDIw
   * djAQBgcqhkjOPQIBBgUrgQQAIgNiAAR4MIHoYx7l63FRD/cHB8o5mXxO1Q/MMDAL
   * j2aTPs+9xYa9+bG3tD60B8jzljHz7aRP+KNOjSkVWLjVb3/ubCK1sK9IRQq9qEmU
   * v4RDsNuESgMjGWdqb8FuvAY5N9GIIvejQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYD
   * VR0PAQH/BAQDAgEGMB0GA1UdDgQWBBTmGHX/72DehKT1RsfeSlXjMjZ59TAKBggq
   * hkjOPQQDAwNnADBkAjAmc0l6tqvmSfR9Uj/UQQSugEODZXW5hYA4O9Zv5JOGq4/n
   * ich/m35rChJVYaoR4HkCMHfoMXGsPHED1oQmHhS48zs73u1Z/GtMMH9ZzkXpc2AV
   * mkzw5l4lIhVtwodZ0LKOag==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x0c\tCommScope1+0)\x06\x03U\x04\x03\x0c\"CommScope Public Trust ECC Root-02"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04x0\x81\xe8c\x1e\xe5\xebqQ\x0f\xf7\x07\x07\xca9\x99|N\xd5\x0f\xcc00\x0b\x8ff\x93>\xcf\xbd\xc5\x86\xbd\xf9\xb1\xb7\xb4>\xb4\x07\xc8\xf3\x961\xf3\xed\xa4O\xf8\xa3N\x8d)\x15X\xb8\xd5o\x7f\xeel\"\xb5\xb0\xafHE\n\xbd\xa8I\x94\xbf\x84C\xb0\xdb\x84J\x03#\x19gjo\xc1n\xbc\x0697\xd1\x88\"\xf7"),
    name_constraints: None
  },

  /*
   * Issuer: CN=IdenTrust Public Sector Root CA 1 O=IdenTrust
   * Subject: CN=IdenTrust Public Sector Root CA 1 O=IdenTrust
   * Label: "IdenTrust Public Sector Root CA 1"
   * Serial: 13298821034946342390521976156843933698
   * SHA256 Fingerprint: 30:d0:89:5a:9a:44:8a:26:20:91:63:55:22:d1:f5:20:10:b5:86:7a:ca:e1:2c:78:ef:95:8f:d4:f4:38:9f:2f
   * -----BEGIN CERTIFICATE-----
   * MIIFZjCCA06gAwIBAgIQCgFCgAAAAUUjz0Z8AAAAAjANBgkqhkiG9w0BAQsFADBN
   * MQswCQYDVQQGEwJVUzESMBAGA1UEChMJSWRlblRydXN0MSowKAYDVQQDEyFJZGVu
   * VHJ1c3QgUHVibGljIFNlY3RvciBSb290IENBIDEwHhcNMTQwMTE2MTc1MzMyWhcN
   * MzQwMTE2MTc1MzMyWjBNMQswCQYDVQQGEwJVUzESMBAGA1UEChMJSWRlblRydXN0
   * MSowKAYDVQQDEyFJZGVuVHJ1c3QgUHVibGljIFNlY3RvciBSb290IENBIDEwggIi
   * MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQC2IpT8pEiv6EdrCvsnduTyP4o7
   * ekosMSqMjbCpwzFrqHd2hCa2rIFCDQjrVVi7evi8ZX3yoG2LqEfpYnYeEe4IFNGy
   * RBb06tD6Hi9e28tzQa68ALBKK0CyrOE7S8ItneShm+waOh7wCLPQ5CQ1B5+ctMlS
   * bdsHyo+1W/CD80/HLaXIrcuVIKQxKFdYWuSNG5qrng0M8gozOSI5Cpcu81N3uURF
   * /YTLNiCBWS2ab21ISGHKTN9T0a9SvESfqy9rg3LvdYDaBjMbXcjaY8ZNzaxmMc3R
   * 3j6HEDbhuaR672BQssvKplbgN6+rNBM5Jeg5ZuSYeqoSmJxZZoY+rfGwyj4GD3vw
   * EUs3oERte8uojHH01bWRNszwFcYr3lEXsZdMUD2xlVl8BX0tIdUAvwFnol57plzy
   * 9yLxkA2T26pEUWbMfXYD62qoKjgZl3YNa4ph+bz27nb9cCvdKTz4Ch5bQhyLVi9V
   * GxyhLrXHFub4qjySjmm2AcG1hp2JDws4lFTo6tyePSW8Uybt1as5qsVATFSrsrTZ
   * 2fjXctscvG29ZV/viDUqZi/u9rNl8DONfJhBaUYPQxxp+pu10GFqzcpL2UyQRqsV
   * WaFHVCkugyhfHMKiq3IXAAaOReyL4jM9f9oZRORicsPfIsbyVtTdX5Vy7W1f90gD
   * W/3FKqD2cyOEEBsB5wIDAQABo0IwQDAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/
   * BAUwAwEB/zAdBgNVHQ4EFgQU43HgntinQtnbcZFrlJPrw6PRFKMwDQYJKoZIhvcN
   * AQELBQADggIBAEf63QqwEZE4rU1d9+UOl1QZgkiHVIyqZJnYWv6IAcVYpZmxI1Qj
   * t2odIFflAWJBF9MJ23XLblSQdf4an4EKwt3X9wnQW3IV5B4Jaj0z8yGa5hV+rVHV
   * DRDtfULAj+7AmgjVQdZcDiFpboBhDhXAuM/FSRJSzL46zNQuOAXeNf0fb7iAaJg9
   * TaDKQGXSc3z1i9kKlT/YPyNtGtEqJBnZhbMX73huqVjRI9PHE+1yJX9dsXNw0H8G
   * lwmEKYBhHfpe/3OsoOOJuBxxFcbeMX8S3OFtm6/n6J91eEyrRjuazr8FGF1NFTwW
   * mhlQBJqymm9li1JfPFgEKCXAZmExfrngdbkaqIHWchezxQMxNRF4eKLg6TCMf4Df
   * WN88uieW4oA0beOY02QnrEh+KHdcxiVhJfiFDGX6xDIvpZgF5PgLZxYWxoK4Mhn5
   * +bl53B/N66+rDt0b20XkeucC4pVd/GnwU2lhlXV5C15V5jgclKlZM57IcXR5f1GJ
   * tshquDDIajjDbp7hNxbqBWJMWxJH7ae0s1hWx0nzfxJoCTFx8G34Tkf71oXuxVhA
   * GaQdp/lLQzfcaFpPz+vCZHTetBXZ9FRUGi8c15dxVJCO2SCdUyt/q4/i6jC8UDfv
   * 8Ue1fXwsBOxonbRJRBD0ckscZOf85muQ3Wl9af0AVqW3rLatt8o+Ae+c
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x13\tIdenTrust1*0(\x06\x03U\x04\x03\x13!IdenTrust Public Sector Root CA 1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb6\"\x94\xfc\xa4H\xaf\xe8Gk\n\xfb\'v\xe4\xf2?\x8a;zJ,1*\x8c\x8d\xb0\xa9\xc31k\xa8wv\x84&\xb6\xac\x81B\r\x08\xebUX\xbbz\xf8\xbce}\xf2\xa0m\x8b\xa8G\xe9bv\x1e\x11\xee\x08\x14\xd1\xb2D\x16\xf4\xea\xd0\xfa\x1e/^\xdb\xcbsA\xae\xbc\x00\xb0J+@\xb2\xac\xe1;K\xc2-\x9d\xe4\xa1\x9b\xec\x1a:\x1e\xf0\x08\xb3\xd0\xe4$5\x07\x9f\x9c\xb4\xc9Rm\xdb\x07\xca\x8f\xb5[\xf0\x83\xf3O\xc7-\xa5\xc8\xad\xcb\x95 \xa41(WXZ\xe4\x8d\x1b\x9a\xab\x9e\r\x0c\xf2\n39\"9\n\x97.\xf3Sw\xb9DE\xfd\x84\xcb6 \x81Y-\x9aomHHa\xcaL\xdfS\xd1\xafR\xbcD\x9f\xab/k\x83r\xefu\x80\xda\x063\x1b]\xc8\xdac\xc6M\xcd\xacf1\xcd\xd1\xde>\x87\x106\xe1\xb9\xa4z\xef`P\xb2\xcb\xca\xa6V\xe07\xaf\xab4\x139%\xe89f\xe4\x98z\xaa\x12\x98\x9cYf\x86>\xad\xf1\xb0\xca>\x06\x0f{\xf0\x11K7\xa0Dm{\xcb\xa8\x8cq\xf4\xd5\xb5\x916\xcc\xf0\x15\xc6+\xdeQ\x17\xb1\x97LP=\xb1\x95Y|\x05}-!\xd5\x00\xbf\x01g\xa2^{\xa6\\\xf2\xf7\"\xf1\x90\r\x93\xdb\xaaDQf\xcc}v\x03\xebj\xa8*8\x19\x97v\rk\x8aa\xf9\xbc\xf6\xeev\xfdp+\xdd)<\xf8\n\x1e[B\x1c\x8bV/U\x1b\x1c\xa1.\xb5\xc7\x16\xe6\xf8\xaa<\x92\x8ei\xb6\x01\xc1\xb5\x86\x9d\x89\x0f\x0b8\x94T\xe8\xea\xdc\x9e=%\xbcS&\xed\xd5\xab9\xaa\xc5@LT\xab\xb2\xb4\xd9\xd9\xf8\xd7r\xdb\x1c\xbcm\xbde_\xef\x885*f/\xee\xf6\xb3e\xf03\x8d|\x98AiF\x0fC\x1ci\xfa\x9b\xb5\xd0aj\xcd\xcaK\xd9L\x90F\xab\x15Y\xa1GT).\x83(_\x1c\xc2\xa2\xabr\x17\x00\x06\x8eE\xec\x8b\xe23=\x7f\xda\x19D\xe4br\xc3\xdf\"\xc6\xf2V\xd4\xdd_\x95r\xedm_\xf7H\x03[\xfd\xc5*\xa0\xf6s#\x84\x10\x1b\x01\xe7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=vTrus ECC Root CA O=iTrusChina Co.,Ltd.
   * Subject: CN=vTrus ECC Root CA O=iTrusChina Co.,Ltd.
   * Label: "vTrus ECC Root CA"
   * Serial: 630369271402956006249506845124680065938238527194
   * SHA256 Fingerprint: 30:fb:ba:2c:32:23:8e:2a:98:54:7a:f9:79:31:e5:50:42:8b:9b:3f:1c:8e:eb:66:33:dc:fa:86:c5:b2:7d:d3
   * -----BEGIN CERTIFICATE-----
   * MIICDzCCAZWgAwIBAgIUbmq8WapTvpg5Z6LSa6Q75m0c1towCgYIKoZIzj0EAwMw
   * RzELMAkGA1UEBhMCQ04xHDAaBgNVBAoTE2lUcnVzQ2hpbmEgQ28uLEx0ZC4xGjAY
   * BgNVBAMTEXZUcnVzIEVDQyBSb290IENBMB4XDTE4MDczMTA3MjY0NFoXDTQzMDcz
   * MTA3MjY0NFowRzELMAkGA1UEBhMCQ04xHDAaBgNVBAoTE2lUcnVzQ2hpbmEgQ28u
   * LEx0ZC4xGjAYBgNVBAMTEXZUcnVzIEVDQyBSb290IENBMHYwEAYHKoZIzj0CAQYF
   * K4EEACIDYgAEZVBKrox5lkqqHAjDo6LN/llWQXf9JpRCux3NCNtzslt188+cToL0
   * v/hhJoVs1oVbcnDS/dtitN9Ti72xRFhiQgnH+n9bEOf+QP3A2MMrMudwpremIFUd
   * e4BdS49nTPEQo0IwQDAdBgNVHQ4EFgQUmDnNvtiyjPeyq+GtJK97fKHbH88wDwYD
   * VR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwCgYIKoZIzj0EAwMDaAAwZQIw
   * V53dVvHH4+m4SVBrm2nDb+zDfSXkV5UTQJtS0zvzQBm8JsctBp61ezaf9SXUY2sA
   * AjEA6dPGnlaaKsyh2j/IZivTWJwghfqrkYpwcBE4YGQLYgmRWAD5Tfs0aNoJrSEG
   * GJTO
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1\x1c0\x1a\x06\x03U\x04\n\x13\x13iTrusChina Co.,Ltd.1\x1a0\x18\x06\x03U\x04\x03\x13\x11vTrus ECC Root CA"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04ePJ\xae\x8cy\x96J\xaa\x1c\x08\xc3\xa3\xa2\xcd\xfeYVAw\xfd&\x94B\xbb\x1d\xcd\x08\xdbs\xb2[u\xf3\xcf\x9cN\x82\xf4\xbf\xf8a&\x85l\xd6\x85[rp\xd2\xfd\xdbb\xb4\xdfS\x8b\xbd\xb1DXbB\t\xc7\xfa\x7f[\x10\xe7\xfe@\xfd\xc0\xd8\xc3+2\xe7p\xa6\xb7\xa6 U\x1d{\x80]K\x8fgL\xf1\x10"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Global Root G3 O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Global Root G3 O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Global Root G3"
   * Serial: 7089244469030293291760083333884364146
   * SHA256 Fingerprint: 31:ad:66:48:f8:10:41:38:c7:38:f3:9e:a4:32:01:33:39:3e:3a:18:cc:02:29:6e:f9:7c:2a:c9:ef:67:31:d0
   * -----BEGIN CERTIFICATE-----
   * MIICPzCCAcWgAwIBAgIQBVVWvPJepDU1w6QP1atFcjAKBggqhkjOPQQDAzBhMQsw
   * CQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cu
   * ZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBHMzAe
   * Fw0xMzA4MDExMjAwMDBaFw0zODAxMTUxMjAwMDBaMGExCzAJBgNVBAYTAlVTMRUw
   * EwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAXBgNVBAsTEHd3dy5kaWdpY2VydC5jb20x
   * IDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IEczMHYwEAYHKoZIzj0CAQYF
   * K4EEACIDYgAE3afZu4q4C/sLfyHS8L6+c/MzXRq8NOrexpu80JX28MzQC7phW1FG
   * fp4tn+6OYwwX7Adw9c+ELkCDnOg/QW07rdOkFFk2eJ0DQ+4QE2xy3q6Ip6FrtUPO
   * Z9wj/wMco+I+o0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBhjAd
   * BgNVHQ4EFgQUs9tIpPmhxdiuNkHMEWNpYim8S8YwCgYIKoZIzj0EAwMDaAAwZQIx
   * AK288mw/EkrRLTnDCgmXc/SINoyIJ7vmiI1Qhadj+Z4y3maTD/HMsQmP3Wyr+mt/
   * oAIwOWZbwmSNuJ5Q3KjVSaLtx9zRSX8XAbjIho9OjIgrqJqpisXRAL34VOKa5Vt8
   * sycX
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1 0\x1e\x06\x03U\x04\x03\x13\x17DigiCert Global Root G3"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xdd\xa7\xd9\xbb\x8a\xb8\x0b\xfb\x0b\x7f!\xd2\xf0\xbe\xbes\xf33]\x1a\xbc4\xea\xde\xc6\x9b\xbc\xd0\x95\xf6\xf0\xcc\xd0\x0b\xbaa[QF~\x9e-\x9f\xee\x8ec\x0c\x17\xec\x07p\xf5\xcf\x84.@\x83\x9c\xe8?Am;\xad\xd3\xa4\x14Y6x\x9d\x03C\xee\x10\x13lr\xde\xae\x88\xa7\xa1k\xb5C\xceg\xdc#\xff\x03\x1c\xa3\xe2>"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com Root Certification Authority ECC O=SSL Corporation
   * Subject: CN=SSL.com Root Certification Authority ECC O=SSL Corporation
   * Label: "SSL.com Root Certification Authority ECC"
   * Serial: 8495723813297216424
   * SHA256 Fingerprint: 34:17:bb:06:cc:60:07:da:1b:96:1c:92:0b:8a:b4:ce:3f:ad:82:0e:4a:a3:0b:9a:cb:c4:a7:4e:bd:ce:bc:65
   * -----BEGIN CERTIFICATE-----
   * MIICjTCCAhSgAwIBAgIIdebfy8FoW6gwCgYIKoZIzj0EAwIwfDELMAkGA1UEBhMC
   * VVMxDjAMBgNVBAgMBVRleGFzMRAwDgYDVQQHDAdIb3VzdG9uMRgwFgYDVQQKDA9T
   * U0wgQ29ycG9yYXRpb24xMTAvBgNVBAMMKFNTTC5jb20gUm9vdCBDZXJ0aWZpY2F0
   * aW9uIEF1dGhvcml0eSBFQ0MwHhcNMTYwMjEyMTgxNDAzWhcNNDEwMjEyMTgxNDAz
   * WjB8MQswCQYDVQQGEwJVUzEOMAwGA1UECAwFVGV4YXMxEDAOBgNVBAcMB0hvdXN0
   * b24xGDAWBgNVBAoMD1NTTCBDb3Jwb3JhdGlvbjExMC8GA1UEAwwoU1NMLmNvbSBS
   * b290IENlcnRpZmljYXRpb24gQXV0aG9yaXR5IEVDQzB2MBAGByqGSM49AgEGBSuB
   * BAAiA2IABEVuqVDEpiM2nl8ojRfLliJkP9x6jh3MCLOicSS6jkm5BBtHllirLZXI
   * 7Z4INcgn64mMU1jrYor+8FsPazFSY0E7ic3s7LaNGdM0B9y7xgZ/wkWV7Mt/qCPg
   * CemB+vNH06NjMGEwHQYDVR0OBBYEFILRhXMw5zUE044CkvvlpNHEIejNMA8GA1Ud
   * EwEB/wQFMAMBAf8wHwYDVR0jBBgwFoAUgtGFczDnNQTTjgKS++Wk0cQh6M0wDgYD
   * VR0PAQH/BAQDAgGGMAoGCCqGSM49BAMCA2cAMGQCMG/n61kRpGDPYbCWe+0F+S8T
   * kdzt5fxQaxFGRrMcIQBiu77D5+jNB5n5DQtdcj7EqgIwH7y6C+IwJPt8bYBVCpk+
   * gA0z5Wajs6O7pdWLjwkspl1+4vAHCGht0nxpbl/f5Wpl
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0e0\x0c\x06\x03U\x04\x08\x0c\x05Texas1\x100\x0e\x06\x03U\x04\x07\x0c\x07Houston1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation110/\x06\x03U\x04\x03\x0c(SSL.com Root Certification Authority ECC"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04En\xa9P\xc4\xa6#6\x9e_(\x8d\x17\xcb\x96\"d?\xdcz\x8e\x1d\xcc\x08\xb3\xa2q$\xba\x8eI\xb9\x04\x1bG\x96X\xab-\x95\xc8\xed\x9e\x085\xc8\'\xeb\x89\x8cSX\xebb\x8a\xfe\xf0[\x0fk1RcA;\x89\xcd\xec\xec\xb6\x8d\x19\xd34\x07\xdc\xbb\xc6\x06\x7f\xc2E\x95\xec\xcb\x7f\xa8#\xe0\t\xe9\x81\xfa\xf3G\xd3"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GTS Root R4 O=Google Trust Services LLC
   * Subject: CN=GTS Root R4 O=Google Trust Services LLC
   * Label: "GTS Root R4"
   * Serial: 159662532700760215368942768210
   * SHA256 Fingerprint: 34:9d:fa:40:58:c5:e2:63:12:3b:39:8a:e7:95:57:3c:4e:13:13:c8:3f:e6:8f:93:55:6c:d5:e8:03:1b:3c:7d
   * -----BEGIN CERTIFICATE-----
   * MIICCTCCAY6gAwIBAgINAgPlwGjvYxqccpBQUjAKBggqhkjOPQQDAzBHMQswCQYD
   * VQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2VzIExMQzEUMBIG
   * A1UEAxMLR1RTIFJvb3QgUjQwHhcNMTYwNjIyMDAwMDAwWhcNMzYwNjIyMDAwMDAw
   * WjBHMQswCQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2Vz
   * IExMQzEUMBIGA1UEAxMLR1RTIFJvb3QgUjQwdjAQBgcqhkjOPQIBBgUrgQQAIgNi
   * AATzdHOnaItgrkO4NcWBMHtLSZ37wWHO5t5GvWvVYRg1rkDdc/eJkTBa6zzuhXyi
   * QHY7qca4R9gq55KRanPpsXI5nymfopjTX15YhmUPoYRlBtHci8nHc8iMai/lxKvR
   * HYqjQjBAMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQW
   * BBSATNbrdP9JNqPV2Py1PsVq8JQdjDAKBggqhkjOPQQDAwNpADBmAjEA6ED/g94D
   * 9J+uHXqnLrmvT/aDHQ4thQEd0dlq7A/Cr8deVl5c1RxYIigL9zC2L7F8AjEA8GE8
   * p/SgguMh1YQdc4acLa/KNJvxn7kjNuK8YAOdgLOaVsjh4rsUecrNIdSUtUlD
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\"0 \x06\x03U\x04\n\x13\x19Google Trust Services LLC1\x140\x12\x06\x03U\x04\x03\x13\x0bGTS Root R4"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xf3ts\xa7h\x8b`\xaeC\xb85\xc5\x810{KI\x9d\xfb\xc1a\xce\xe6\xdeF\xbdk\xd5a\x185\xae@\xdds\xf7\x89\x910Z\xeb<\xee\x85|\xa2@v;\xa9\xc6\xb8G\xd8*\xe7\x92\x91js\xe9\xb1r9\x9f)\x9f\xa2\x98\xd3_^X\x86e\x0f\xa1\x84e\x06\xd1\xdc\x8b\xc9\xc7s\xc8\x8cj/\xe5\xc4\xab\xd1\x1d\x8a"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GTS Root R3 O=Google Trust Services LLC
   * Subject: CN=GTS Root R3 O=Google Trust Services LLC
   * Label: "GTS Root R3"
   * Serial: 159662495401136852707857743206
   * SHA256 Fingerprint: 34:d8:a7:3e:e2:08:d9:bc:db:0d:95:65:20:93:4b:4e:40:e6:94:82:59:6e:8b:6f:73:c8:42:6b:01:0a:6f:48
   * -----BEGIN CERTIFICATE-----
   * MIICCTCCAY6gAwIBAgINAgPluILrIPglJ209ZjAKBggqhkjOPQQDAzBHMQswCQYD
   * VQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2VzIExMQzEUMBIG
   * A1UEAxMLR1RTIFJvb3QgUjMwHhcNMTYwNjIyMDAwMDAwWhcNMzYwNjIyMDAwMDAw
   * WjBHMQswCQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2Vz
   * IExMQzEUMBIGA1UEAxMLR1RTIFJvb3QgUjMwdjAQBgcqhkjOPQIBBgUrgQQAIgNi
   * AAQfTzOHMymKoYTey8chWEGJ6ladK0uFxh1MJ7x/JlFyb+Kf1qPKzEUURout736G
   * jOyxfi//qXGdGIRFBEFVbivqJn+7kAHjSxm65FSWRQmx1WyRRK2EE46ajA2ADDL2
   * 4CejQjBAMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQW
   * BBTB8Sa6oC2uhYHP0/EqEr24Cmf9vDAKBggqhkjOPQQDAwNpADBmAjEA9uEglRR7
   * VKOQFhG/hMjqb2sXnh5GmCCbn9MN2azTL818+FsuVbu/3ZL3pAzcMeGiAjEA/Jdm
   * ZuVDFhOD3cffL74UOO0BzrEXGhF16b0DjyZ+hOXJYKaV11RZt+cRLInUue4X
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\"0 \x06\x03U\x04\n\x13\x19Google Trust Services LLC1\x140\x12\x06\x03U\x04\x03\x13\x0bGTS Root R3"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x1fO3\x873)\x8a\xa1\x84\xde\xcb\xc7!XA\x89\xeaV\x9d+K\x85\xc6\x1dL\'\xbc\x7f&Qro\xe2\x9f\xd6\xa3\xca\xccE\x14F\x8b\xad\xef~\x86\x8c\xec\xb1~/\xff\xa9q\x9d\x18\x84E\x04AUn+\xea&\x7f\xbb\x90\x01\xe3K\x19\xba\xe4T\x96E\t\xb1\xd5l\x91D\xad\x84\x13\x8e\x9a\x8c\r\x80\x0c2\xf6\xe0\'"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Microsoft ECC Root Certificate Authority 2017 O=Microsoft Corporation
   * Subject: CN=Microsoft ECC Root Certificate Authority 2017 O=Microsoft Corporation
   * Label: "Microsoft ECC Root Certificate Authority 2017"
   * Serial: 136839042543790627607696632466672567020
   * SHA256 Fingerprint: 35:8d:f3:9d:76:4a:f9:e1:b7:66:e9:c9:72:df:35:2e:e1:5c:fa:c2:27:af:6a:d1:d7:0e:8e:4a:6e:dc:ba:02
   * -----BEGIN CERTIFICATE-----
   * MIICWTCCAd+gAwIBAgIQZvI9r4fei7FK6gxXMQHC7DAKBggqhkjOPQQDAzBlMQsw
   * CQYDVQQGEwJVUzEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMTYwNAYD
   * VQQDEy1NaWNyb3NvZnQgRUNDIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIw
   * MTcwHhcNMTkxMjE4MjMwNjQ1WhcNNDIwNzE4MjMxNjA0WjBlMQswCQYDVQQGEwJV
   * UzEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMTYwNAYDVQQDEy1NaWNy
   * b3NvZnQgRUNDIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTcwdjAQBgcq
   * hkjOPQIBBgUrgQQAIgNiAATUvD0CQnVBEyPNgASGAlEvaqiBYgtlzPbKnR5vSmZR
   * ogPZnZH6thaxjG7efM3beaYvzrvOcS/lpaso7GMEZpn4+vKTEAXhgShC48Zo9OYb
   * hGBKia/teQ87zvH2RPUBeMCjVDBSMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8E
   * BTADAQH/MB0GA1UdDgQWBBTIy5lycFIM+Oa+sgRXKSrPQhDtNTAQBgkrBgEEAYI3
   * FQEEAwIBADAKBggqhkjOPQQDAwNoADBlAjBY8k3qDPlfXu5gKcs68tvWMoQZP3zV
   * L8KxzJOuULsJMsbG7X7JNpQS5GiFBqIb0C8CMQCZ6Ra0DvpWSNSkMBaReNtUjGUB
   * iudQZsIxtzm6uBoiB078a1QWIP8rtedMDE2mT3M=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x1e0\x1c\x06\x03U\x04\n\x13\x15Microsoft Corporation1604\x06\x03U\x04\x03\x13-Microsoft ECC Root Certificate Authority 2017"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xd4\xbc=\x02BuA\x13#\xcd\x80\x04\x86\x02Q/j\xa8\x81b\x0be\xcc\xf6\xca\x9d\x1eoJfQ\xa2\x03\xd9\x9d\x91\xfa\xb6\x16\xb1\x8cn\xde|\xcd\xdby\xa6/\xce\xbb\xceq/\xe5\xa5\xab(\xecc\x04f\x99\xf8\xfa\xf2\x93\x10\x05\xe1\x81(B\xe3\xc6h\xf4\xe6\x1b\x84`J\x89\xaf\xedy\x0f;\xce\xf1\xf6D\xf5\x01x\xc0"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert TLS RSA4096 Root G5 O=DigiCert, Inc.
   * Subject: CN=DigiCert TLS RSA4096 Root G5 O=DigiCert, Inc.
   * Label: "DigiCert TLS RSA4096 Root G5"
   * Serial: 11930366277458970227240571539258396554
   * SHA256 Fingerprint: 37:1a:00:dc:05:33:b3:72:1a:7e:eb:40:e8:41:9e:70:79:9d:2b:0a:0f:2c:1d:80:69:31:65:f7:ce:c4:ad:75
   * -----BEGIN CERTIFICATE-----
   * MIIFZjCCA06gAwIBAgIQCPm0eKj6ftpqMzeJ3nzPijANBgkqhkiG9w0BAQwFADBN
   * MQswCQYDVQQGEwJVUzEXMBUGA1UEChMORGlnaUNlcnQsIEluYy4xJTAjBgNVBAMT
   * HERpZ2lDZXJ0IFRMUyBSU0E0MDk2IFJvb3QgRzUwHhcNMjEwMTE1MDAwMDAwWhcN
   * NDYwMTE0MjM1OTU5WjBNMQswCQYDVQQGEwJVUzEXMBUGA1UEChMORGlnaUNlcnQs
   * IEluYy4xJTAjBgNVBAMTHERpZ2lDZXJ0IFRMUyBSU0E0MDk2IFJvb3QgRzUwggIi
   * MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCz0PTJeRGd/fxmgefM1eS87IE+
   * ajWOLrfn3q/5B03PMJ3qCQuZvWxX2hhKuHisOjmopkisLnLlvevxGs3npAOpPxG0
   * 2C+JFvuUAT27L/gTBaF4HI4o4EXgg/RZG5Wzrn4DReW+wkL+7vI8toUTmDKdFqgp
   * wgscONyfMXdcvyej/Cestyu9dJsXLfKB2l2w4SMXPohKEiPQ6s+d3gMXsUJKoBZM
   * pG2T6T867jp8nVid9E6P/DsjyG244gXazOvswzH016cpVIDPRFtMbzCe88zdH5RD
   * nU1/cHAN1DrRN/BsnZvAFJNY781BOHW8EwOVfH/jXOnVDdXifBBiqmvwPXbzP6Po
   * sMH976pXTayGpxi0KcEsDr9kvimM2AItzVwv8n/vFfQMFawKsPHTDU9qTXeXAaDx
   * Zre3zu/O7Oyldcqs4+Fj97ihBMi8ez9dLRYiVu1ISf6nL3kwJZu6ay0/nTvEF+cd
   * Lvvyz6b84xQslpghjLSR6Rlgg/IwKwZzUNWYOwbpx4oMYIwo+FKbbuH2TbsGJJvX
   * KyY//SovcfXWJL5/MZ4PbeiPT02jP/816t9JXkGPhvnxd3lLG7SjXi/7RgLQZhNe
   * XoVPzthwiHvOAbWWl9fNff2C+MIkwcoBOU+NosEUQB+cZtUMCUbW8tDRSHZWOkPL
   * tgoRObqME2wGtZ7P6wIDAQABo0IwQDAdBgNVHQ4EFgQUUTMc7TZArxfTJc1paPKv
   * TiM+s0EwDgYDVR0PAQH/BAQDAgGGMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcN
   * AQEMBQADggIBAGCmr1tfV9qJ20tQqcQjNSH/0GEwhJG3PxDPJY7Jv0Y02cEhJhxw
   * GXIeo8mH/qlDZJY6yFMECrZBu8RHANmfGBg7sg7zNOok992vIGCukihfNudd5N7H
   * PNtQOa27PShNlnx2xlv0wdsUpasZYgcYQF+Xkdycx6u1UQ3maVNVzDl92sURVXLF
   * O4uJ+DQtpBflF+aZfTCIITfNMBc9uPK8qHWgQ9w+iUuQrm0D4ByjoJYJu32jtyoQ
   * REtGBzRj7TG5BO6jm5qu5jF49OokYTurWGT/u4cnYiWB39yhL/btp/96j1EuMPik
   * AdKFOV8BmZZvWltwGUb+hmA+rYAQCd05JS9Yf7vSdPD3Rh9GOUrYU9DzLjtxpdRv
   * /PNn5AeP3SYZ4Y1b+qOTEZvpyDrDVWiakuFSdjjo4bq9+0/V77PnSIMx8IIh47a+
   * p6tv75/fTM8BuGJqIz3nCU2AG3swpMPdB380vqQmsvZB6Akd4yCYqjdP//fx4ilw
   * MUc/dNAUFvohigLVigmUdy7yWSiLfFCSCmZ4OIN1xLVaqBHG5cGdZlXPU8Sv13WF
   * qUITVuwhd4GTWgzqltlJyqEI8pc7bZsEGCREjnwB8twl2F6GmrE52/WRMmrRpnCK
   * ovfepEWFJqgejF0pW8hL2JpqA15w8oVPbEtoL8pU9ozaMv7Da4M/OMZ+
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x170\x15\x06\x03U\x04\n\x13\x0eDigiCert, Inc.1%0#\x06\x03U\x04\x03\x13\x1cDigiCert TLS RSA4096 Root G5"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb3\xd0\xf4\xc9y\x11\x9d\xfd\xfcf\x81\xe7\xcc\xd5\xe4\xbc\xec\x81>j5\x8e.\xb7\xe7\xde\xaf\xf9\x07M\xcf0\x9d\xea\t\x0b\x99\xbdlW\xda\x18J\xb8x\xac:9\xa8\xa6H\xac.r\xe5\xbd\xeb\xf1\x1a\xcd\xe7\xa4\x03\xa9?\x11\xb4\xd8/\x89\x16\xfb\x94\x01=\xbb/\xf8\x13\x05\xa1x\x1c\x8e(\xe0E\xe0\x83\xf4Y\x1b\x95\xb3\xae~\x03E\xe5\xbe\xc2B\xfe\xee\xf2<\xb6\x85\x13\x982\x9d\x16\xa8)\xc2\x0b\x1c8\xdc\x9f1w\\\xbf\'\xa3\xfc\'\xac\xb7+\xbdt\x9b\x17-\xf2\x81\xda]\xb0\xe1#\x17>\x88J\x12#\xd0\xea\xcf\x9d\xde\x03\x17\xb1BJ\xa0\x16L\xa4m\x93\xe9?:\xee:|\x9dX\x9d\xf4N\x8f\xfc;#\xc8m\xb8\xe2\x05\xda\xcc\xeb\xec\xc31\xf4\xd7\xa7)T\x80\xcfD[Lo0\x9e\xf3\xcc\xdd\x1f\x94C\x9dM\x7fpp\r\xd4:\xd17\xf0l\x9d\x9b\xc0\x14\x93X\xef\xcdA8u\xbc\x13\x03\x95|\x7f\xe3\\\xe9\xd5\r\xd5\xe2|\x10b\xaak\xf0=v\xf3?\xa3\xe8\xb0\xc1\xfd\xef\xaaWM\xac\x86\xa7\x18\xb4)\xc1,\x0e\xbfd\xbe)\x8c\xd8\x02-\xcd\\/\xf2\x7f\xef\x15\xf4\x0c\x15\xac\n\xb0\xf1\xd3\rOjMw\x97\x01\xa0\xf1f\xb7\xb7\xce\xef\xce\xec\xec\xa5u\xca\xac\xe3\xe1c\xf7\xb8\xa1\x04\xc8\xbc{?]-\x16\"V\xedHI\xfe\xa7/y0%\x9b\xbak-?\x9d;\xc4\x17\xe7\x1d.\xfb\xf2\xcf\xa6\xfc\xe3\x14,\x96\x98!\x8c\xb4\x91\xe9\x19`\x83\xf20+\x06sP\xd5\x98;\x06\xe9\xc7\x8a\x0c`\x8c(\xf8R\x9bn\xe1\xf6M\xbb\x06$\x9b\xd7+&?\xfd*/q\xf5\xd6$\xbe\x7f1\x9e\x0fm\xe8\x8fOM\xa3?\xff5\xea\xdfI^A\x8f\x86\xf9\xf1wyK\x1b\xb4\xa3^/\xfbF\x02\xd0f\x13^^\x85O\xce\xd8p\x88{\xce\x01\xb5\x96\x97\xd7\xcd}\xfd\x82\xf8\xc2$\xc1\xca\x019O\x8d\xa2\xc1\x14@\x1f\x9cf\xd5\x0c\tF\xd6\xf2\xd0\xd1HvV:C\xcb\xb6\n\x119\xba\x8c\x13l\x06\xb5\x9e\xcf\xeb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Microsec e-Szigno Root CA 2009 O=Microsec Ltd.
   * Subject: CN=Microsec e-Szigno Root CA 2009 O=Microsec Ltd.
   * Label: "Microsec e-Szigno Root CA 2009"
   * Serial: 14014712776195784473
   * SHA256 Fingerprint: 3c:5f:81:fe:a5:fa:b8:2c:64:bf:a2:ea:ec:af:cd:e8:e0:77:fc:86:20:a7:ca:e5:37:16:3d:f3:6e:db:f3:78
   * -----BEGIN CERTIFICATE-----
   * MIIECjCCAvKgAwIBAgIJAMJ+QwRORz8ZMA0GCSqGSIb3DQEBCwUAMIGCMQswCQYD
   * VQQGEwJIVTERMA8GA1UEBwwIQnVkYXBlc3QxFjAUBgNVBAoMDU1pY3Jvc2VjIEx0
   * ZC4xJzAlBgNVBAMMHk1pY3Jvc2VjIGUtU3ppZ25vIFJvb3QgQ0EgMjAwOTEfMB0G
   * CSqGSIb3DQEJARYQaW5mb0BlLXN6aWduby5odTAeFw0wOTA2MTYxMTMwMThaFw0y
   * OTEyMzAxMTMwMThaMIGCMQswCQYDVQQGEwJIVTERMA8GA1UEBwwIQnVkYXBlc3Qx
   * FjAUBgNVBAoMDU1pY3Jvc2VjIEx0ZC4xJzAlBgNVBAMMHk1pY3Jvc2VjIGUtU3pp
   * Z25vIFJvb3QgQ0EgMjAwOTEfMB0GCSqGSIb3DQEJARYQaW5mb0BlLXN6aWduby5o
   * dTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAOn4j/NjrdqG2KfgQvvP
   * kd6mJviZpWNwrZuuyjNAfW2WbqEORO7hE52UQlKavXWFdCyoDh2Tthi3jCyoz/tc
   * cbna7P7ofo/kLx2yqHWH2Leh5TvPmUpG0IMZfcChEhyVbUr02MelTTMuhTlAdX4U
   * fIASmFDHQWe4oIBhVKZsTh/gnQ4H6cm6M+f+wFUoLAKApxn1ntxVUwOXewdI/5n7
   * N4okxFnMUBBjjqqpGrCEGob5X7uxUG6k0QrM1XF+H6cbfPVTbiJfyyvm1HxdrtbC
   * xkzlBQHZ7Vf8wSN5/PrIJIOV87VqUQHQd9bpEqH5GoP7ghu5sJf0dgYzQ0mg/wu1
   * +rUCAwEAAaOBgDB+MA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0G
   * A1UdDgQWBBTLD8bfQkPMPcu1SCOhGnqmKrs0aDAfBgNVHSMEGDAWgBTLD8bfQkPM
   * Pcu1SCOhGnqmKrs0aDAbBgNVHREEFDASgRBpbmZvQGUtc3ppZ25vLmh1MA0GCSqG
   * SIb3DQEBCwUAA4IBAQDJ0Q5eLtXMs3w+y/w9/w0olZMEyL/azXm4Q5DwpL7v8u8h
   * mLzU1F0G9u5C7DBsoKqpyvGvivo/C3NqPuouQH4frlRheesuCDfXI/OMn74dseGk
   * ddug4lQUsbocKaQY9hK6ohQU4zE1yED/t+AFdlfBHFny+L/k7SViXITwfn4fs775
   * tyERzAMBVnCnEJIeGzSBHq2cGsMEPO0CYdYeBvNfOofyK/FFh+U9rNHHV4S9a67c
   * 2Pm2G2JwCz02yULyMtd6YebS2z3PyKnJm9zbWETXbzivf3jTo60adbocwTZ8jx5t
   * HMN1Rq41Bab2XD0h7lbwyYIiLXpUq3DDfSJlgnCW
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02HU1\x110\x0f\x06\x03U\x04\x07\x0c\x08Budapest1\x160\x14\x06\x03U\x04\n\x0c\rMicrosec Ltd.1\'0%\x06\x03U\x04\x03\x0c\x1eMicrosec e-Szigno Root CA 20091\x1f0\x1d\x06\t*\x86H\x86\xf7\r\x01\t\x01\x16\x10info@e-szigno.hu"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xe9\xf8\x8f\xf3c\xad\xda\x86\xd8\xa7\xe0B\xfb\xcf\x91\xde\xa6&\xf8\x99\xa5cp\xad\x9b\xae\xca3@}m\x96n\xa1\x0eD\xee\xe1\x13\x9d\x94BR\x9a\xbdu\x85t,\xa8\x0e\x1d\x93\xb6\x18\xb7\x8c,\xa8\xcf\xfb\\q\xb9\xda\xec\xfe\xe8~\x8f\xe4/\x1d\xb2\xa8u\x87\xd8\xb7\xa1\xe5;\xcf\x99JF\xd0\x83\x19}\xc0\xa1\x12\x1c\x95mJ\xf4\xd8\xc7\xa5M3.\x859@u~\x14|\x80\x12\x98P\xc7Ag\xb8\xa0\x80aT\xa6lN\x1f\xe0\x9d\x0e\x07\xe9\xc9\xba3\xe7\xfe\xc0U(,\x02\x80\xa7\x19\xf5\x9e\xdcUS\x03\x97{\x07H\xff\x99\xfb7\x8a$\xc4Y\xccP\x10c\x8e\xaa\xa9\x1a\xb0\x84\x1a\x86\xf9_\xbb\xb1Pn\xa4\xd1\n\xcc\xd5q~\x1f\xa7\x1b|\xf5Sn\"_\xcb+\xe6\xd4|]\xae\xd6\xc2\xc6L\xe5\x05\x01\xd9\xedW\xfc\xc1#y\xfc\xfa\xc8$\x83\x95\xf3\xb5jQ\x01\xd0w\xd6\xe9\x12\xa1\xf9\x1a\x83\xfb\x82\x1b\xb9\xb0\x97\xf4v\x063CI\xa0\xff\x0b\xb5\xfa\xb5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Assured ID Root CA O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Assured ID Root CA O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Assured ID Root CA"
   * Serial: 17154717934120587862167794914071425081
   * SHA256 Fingerprint: 3e:90:99:b5:01:5e:8f:48:6c:00:bc:ea:9d:11:1e:e7:21:fa:ba:35:5a:89:bc:f1:df:69:56:1e:3d:c6:32:5c
   * -----BEGIN CERTIFICATE-----
   * MIIDtzCCAp+gAwIBAgIQDOfg5RfYRv6P5WD8G/AwOTANBgkqhkiG9w0BAQUFADBl
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJv
   * b3QgQ0EwHhcNMDYxMTEwMDAwMDAwWhcNMzExMTEwMDAwMDAwWjBlMQswCQYDVQQG
   * EwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNl
   * cnQuY29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJvb3QgQ0EwggEi
   * MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtDhXO5EOAXLGH87dg+XESpa7c
   * JpSIqvTO9SA5KFhgDPiA2qkVlTJhPLWxKISKityfCgyDF3qPkKyK53lTXDGEKvYP
   * mDI2dsze3Tyoou9q+yHyUmHfnyDXH+Kx2f4YZNISW1/5WBg1vEfNoTb5a3/UsDg+
   * wRvDjDPZ2C8Y/igPs6eD1sNuRMBhNZYW/lmci3Zt1/GiSw0r/wty2p5g0I6QNcZ4
   * VYcgoc/lbQrISXwxmDNsIumH0DJaoroTghHtORedmTpyoeb6pNnVFzF1roV9Iq4/
   * AUaG9ih5yLHa5FcXxH4cDrC0kqZWs72yl+2qp/C3xag/lRbQ/6GW6whfGHdPAgMB
   * AAGjYzBhMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQW
   * BBRF66Kv9JLLgjEtUYunpyGd823IDzAfBgNVHSMEGDAWgBRF66Kv9JLLgjEtUYun
   * pyGd823IDzANBgkqhkiG9w0BAQUFAAOCAQEAog683+Lt8ONyc3pklL/3cmbYMuRC
   * dWKuh+vy1dneVrOfzM4UKLkNl2BcEkxY5NM9g0lFWJc1aRqoR+pWxnmrEthngYTf
   * fwk8lOa4JiwgvT2zKIn3X/8i4peEH+ll74fg38FnSbNd67IJKusm7Xi+fT8r87cm
   * NW1fiQG2SVufAQWbqz0lwcy2f8Lxb4bG+mRo64EtlOtCt/qMHt1i8b5QZ7dsvfPx
   * H2sMNgcWfzd8qVttevESRmCD1ycEvkvOl77DZypoEd+A5wwzZr8TDRRu838fYxAe
   * +o0bJW1sj6W3YQGx0qMmoRBxna3iw/nDmVG3KwcIzi7mULKn+gpFL6Lw8g==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1$0\"\x06\x03U\x04\x03\x13\x1bDigiCert Assured ID Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xad\x0e\x15\xce\xe4C\x80\\\xb1\x87\xf3\xb7`\xf9q\x12\xa5\xae\xdc&\x94\x88\xaa\xf4\xce\xf5 9(X`\x0c\xf8\x80\xda\xa9\x15\x952a<\xb5\xb1(\x84\x8a\x8a\xdc\x9f\n\x0c\x83\x17z\x8f\x90\xac\x8a\xe7yS\\1\x84*\xf6\x0f\x9826v\xcc\xde\xdd<\xa8\xa2\xefj\xfb!\xf2Ra\xdf\x9f \xd7\x1f\xe2\xb1\xd9\xfe\x18d\xd2\x12[_\xf9X\x185\xbcG\xcd\xa16\xf9k\x7f\xd4\xb08>\xc1\x1b\xc3\x8c3\xd9\xd8/\x18\xfe(\x0f\xb3\xa7\x83\xd6\xc3nD\xc0a5\x96\x16\xfeY\x9c\x8bvm\xd7\xf1\xa2K\r+\xff\x0br\xda\x9e`\xd0\x8e\x905\xc6xU\x87 \xa1\xcf\xe5m\n\xc8I|1\x983l\"\xe9\x87\xd02Z\xa2\xba\x13\x82\x11\xed9\x17\x9d\x99:r\xa1\xe6\xfa\xa4\xd9\xd5\x171u\xae\x85}\"\xae?\x01F\x86\xf6(y\xc8\xb1\xda\xe4W\x17\xc4~\x1c\x0e\xb0\xb4\x92\xa6V\xb3\xbd\xb2\x97\xed\xaa\xa7\xf0\xb7\xc5\xa8?\x95\x16\xd0\xff\xa1\x96\xeb\x08_\x18wO\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SecureSign Root CA12 O=Cybertrust Japan Co., Ltd.
   * Subject: CN=SecureSign Root CA12 O=Cybertrust Japan Co., Ltd.
   * Label: "SecureSign Root CA12"
   * Serial: 587887345431707215246142177076162061960426065942
   * SHA256 Fingerprint: 3f:03:4b:b5:70:4d:44:b2:d0:85:45:a0:20:57:de:93:eb:f3:90:5f:ce:72:1a:cb:c7:30:c0:6d:da:ee:90:4e
   * -----BEGIN CERTIFICATE-----
   * MIIDcjCCAlqgAwIBAgIUZvnHwa/swlG07VOX5uaCwysckBYwDQYJKoZIhvcNAQEL
   * BQAwUTELMAkGA1UEBhMCSlAxIzAhBgNVBAoTGkN5YmVydHJ1c3QgSmFwYW4gQ28u
   * LCBMdGQuMR0wGwYDVQQDExRTZWN1cmVTaWduIFJvb3QgQ0ExMjAeFw0yMDA0MDgw
   * NTM2NDZaFw00MDA0MDgwNTM2NDZaMFExCzAJBgNVBAYTAkpQMSMwIQYDVQQKExpD
   * eWJlcnRydXN0IEphcGFuIENvLiwgTHRkLjEdMBsGA1UEAxMUU2VjdXJlU2lnbiBS
   * b290IENBMTIwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC6OcE3emhF
   * KxS06+QT61d1I02PJC0W6K6OyX2kVzsqdiUzg2zqMoqUm048luT9Ub+ZyZN+v/mt
   * p7JIKwccJ/VMvHASd6SFVLX9kHrko+RRWAPNEHl57muTH2SOa2SroxPjcf59q5zd
   * J1M3s6oYwlkm7Fsf0uZlfO+TvdhYXAvA42VvPMfKWeP+bl+sg779XSVOKik71gur
   * FzJ4pOE+lEa+Ym6b3kaosRbnhW70CEBFEaCeVESE99g2zvVQR9wsMJvuwPWW0v4J
   * hscGWa5Pro4RmHvzC1KqYiaqId+OJTN5lxZJjfU+1UefNzFJM3IFTQy2VYzxV4+K
   * h9GtxRESOaCtAgMBAAGjQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQD
   * AgEGMB0GA1UdDgQWBBRXNPN0zwRL1SXm8UC2LEzZLemgrTANBgkqhkiG9w0BAQsF
   * AAOCAQEAPrvbFxbS8hQBICw4g0utvsqFepq2m2um4fylOqyttCg6r9cBg0krY6Ld
   * mmQOmFxv3Y67ilQiLUoT865AQ9tPkbeGGuwAtEGBpE/6aouIs3YIcipJQMPTw4WJ
   * mBClnW8Zt7vPemVV2zfrPIpyMpcemik+rY3moxtt9XUa5rBouVui7mlHJzWhhpmA
   * 8zNL4WukJsPvdFlseqJkth5Ew1DgDzk9qTPxpfPSvWKErI4cqc1avTc7bgoitPQV
   * 55FYxTpE05Uo2cBl6XLK0A+9H7MV2anjpEcJnuDLN/v9vZfVvhgaaaI5gdka9at/
   * yOPiZwud9AzqVN/Ssq+xIvEg37xEHA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02JP1#0!\x06\x03U\x04\n\x13\x1aCybertrust Japan Co., Ltd.1\x1d0\x1b\x06\x03U\x04\x03\x13\x14SecureSign Root CA12"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xba9\xc17zhE+\x14\xb4\xeb\xe4\x13\xebWu#M\x8f$-\x16\xe8\xae\x8e\xc9}\xa4W;*v%3\x83l\xea2\x8a\x94\x9bN<\x96\xe4\xfdQ\xbf\x99\xc9\x93~\xbf\xf9\xad\xa7\xb2H+\x07\x1c\'\xf5L\xbcp\x12w\xa4\x85T\xb5\xfd\x90z\xe4\xa3\xe4QX\x03\xcd\x10yy\xeek\x93\x1fd\x8ekd\xab\xa3\x13\xe3q\xfe}\xab\x9c\xdd\'S7\xb3\xaa\x18\xc2Y&\xec[\x1f\xd2\xe6e|\xef\x93\xbd\xd8X\\\x0b\xc0\xe3eo<\xc7\xcaY\xe3\xfen_\xac\x83\xbe\xfd]%N*);\xd6\x0b\xab\x172x\xa4\xe1>\x94F\xbebn\x9b\xdeF\xa8\xb1\x16\xe7\x85n\xf4\x08@E\x11\xa0\x9eTD\x84\xf7\xd86\xce\xf5PG\xdc,0\x9b\xee\xc0\xf5\x96\xd2\xfe\t\x86\xc7\x06Y\xaeO\xae\x8e\x11\x98{\xf3\x0bR\xaab&\xaa!\xdf\x8e%3y\x97\x16I\x8d\xf5>\xd5G\x9f71I3r\x05M\x0c\xb6U\x8c\xf1W\x8f\x8a\x87\xd1\xad\xc5\x11\x129\xa0\xad\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TWCA CYBER Root CA O=TAIWAN-CA OU=Root CA
   * Subject: CN=TWCA CYBER Root CA O=TAIWAN-CA OU=Root CA
   * Label: "TWCA CYBER Root CA"
   * Serial: 85076849864375384482682434040119489222
   * SHA256 Fingerprint: 3f:63:bb:28:14:be:17:4e:c8:b6:43:9c:f0:8d:6d:56:f0:b7:c4:05:88:3a:56:48:a3:34:42:4d:6b:3e:c5:58
   * -----BEGIN CERTIFICATE-----
   * MIIFjTCCA3WgAwIBAgIQQAE0jMIAAAAAAAAAATzyxjANBgkqhkiG9w0BAQwFADBQ
   * MQswCQYDVQQGEwJUVzESMBAGA1UEChMJVEFJV0FOLUNBMRAwDgYDVQQLEwdSb290
   * IENBMRswGQYDVQQDExJUV0NBIENZQkVSIFJvb3QgQ0EwHhcNMjIxMTIyMDY1NDI5
   * WhcNNDcxMTIyMTU1OTU5WjBQMQswCQYDVQQGEwJUVzESMBAGA1UEChMJVEFJV0FO
   * LUNBMRAwDgYDVQQLEwdSb290IENBMRswGQYDVQQDExJUV0NBIENZQkVSIFJvb3Qg
   * Q0EwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDG+Moe2Qkgfh1sTs6P
   * 40czRJzHyWmqOlt47nDSkvgEs1JSHWdyKKHfi12VCv7qze33Kc7wb3+szT3vsxxF
   * avcokPFhV8UMxKNQXd7UtcsZyoC5dc4pztKFIuwCY8xEMCDa6pFbVuYdHNWdZsc/
   * 34bKS1PE2Y2yHer43CdTo0fhYcx9tbD47nORxc5zb87uEB8aBs/pJ2DFTxnk684i
   * JkXXYJndzk834H/nY62wuFm40AZoNWDTNq5xQwTxaWV4fPMf88oon1oglWa0zbfu
   * j3ikRRjpJi+NmykosaS3Om251Bw4ckVYsV7r8Cibt4LK/c/WMw+f+5eesRycnupf
   * Xtuq3VTpMCEobY5583WSjCb+3MX2w7DfRFlDo7YDKPYIMKoNM+HvnKkHIuNZW0CP
   * 2oi3aQiotyMuRAlZN1vH4xfyIutuOVLF3lSnmMlLIJXcRolftBL5hSmO68gnFSDA
   * S9TMfAxsNAwmmyYxpjyn9tnQS6Jk/zuZQXLB4HCX8SS7K8R0IrGsayIyJNN4KsDA
   * oS/xUgXJP+92ZuJF2A09rZXIx4kmyA+upwMu+8Ff+iDhcK2wZSA3M2Cw1a/XDBzC
   * kHDXShi8fgGwsOsVHkQGzaRP6AzRwyAQ4VRlnrZR0Bp2a0JaWHY06rc3Ga4udfmW
   * 5cFZ95RXKSWNOkyrTZpB0F8mAwIDAQABo2MwYTAOBgNVHQ8BAf8EBAMCAQYwDwYD
   * VR0TAQH/BAUwAwEB/zAfBgNVHSMEGDAWgBSdhWEUfMFib5do5E83QOGt4A1WNzAd
   * BgNVHQ4EFgQUnYVhFHzBYm+XaORPN0DhreANVjcwDQYJKoZIhvcNAQEMBQADggIB
   * AGSPesRiDrWIzLjHhg6hShbNcAu3p4ULs3a2D6f/CIsLJc+o1IN1KriWiLb73y0t
   * tGlTITVX1olNc79pj3CjYcya2x6a4CD4bLubIp1dhDGaLIrdaqHXKGnK/nZVekZn
   * 68xDiBaiA9a5F/gZbG0jAn/xX9AKKSM70aoK7akXJlQKTcKlTfjF/biBzysseKNn
   * TKkHmvPfXvt89YnNdJdhEGoHK4Fa0o635yDRIG4kqIQnoVesqlVYL9zZyvpoBJ7t
   * RCT5dEA7IzOrg1oYJkK2bVS1FmAwbLGg+LhBoF1JSdJlBTrq/p1hvIbZv97Tujqx
   * f36SNI7JAG7cmL3c7IAFrQI932XtCwP39xaEBDG6k5TY8hL4iuO/Qq+n1M0RFxbI
   * Qh0UqEL20kCGoE8jypZFVmAGzbdVAaYBlGX+bgUJurSkquLvWL69J1bY73NxW0Qz
   * 8ppy6rBePm6pUlvscG21h483XjyMnM7k8M4MZ0HMzvaAq07MTFb1wWFZk7Q+ptq4
   * NxKfKjLji7gh7MMrZQzvIt6IKTtM1/r+t+FHvpw+PoP7UV31aPcuIYXcv/Fa4nzX
   * xeSDwWrruoBa3lwtcHb4yOWHh8qgnaHlIhInD0Q9HWzq1MKLL295q39QpsQZp6F6
   * t5b5wR9iWqJDB0BeJsas7a5wFsWqynKKTbDPAYsDP27X
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TW1\x120\x10\x06\x03U\x04\n\x13\tTAIWAN-CA1\x100\x0e\x06\x03U\x04\x0b\x13\x07Root CA1\x1b0\x19\x06\x03U\x04\x03\x13\x12TWCA CYBER Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc6\xf8\xca\x1e\xd9\t ~\x1dlN\xce\x8f\xe3G3D\x9c\xc7\xc9i\xaa:[x\xeep\xd2\x92\xf8\x04\xb3RR\x1dgr(\xa1\xdf\x8b]\x95\n\xfe\xea\xcd\xed\xf7)\xce\xf0o\x7f\xac\xcd=\xef\xb3\x1cEj\xf7(\x90\xf1aW\xc5\x0c\xc4\xa3P]\xde\xd4\xb5\xcb\x19\xca\x80\xb9u\xce)\xce\xd2\x85\"\xec\x02c\xccD0 \xda\xea\x91[V\xe6\x1d\x1c\xd5\x9df\xc7?\xdf\x86\xcaKS\xc4\xd9\x8d\xb2\x1d\xea\xf8\xdc\'S\xa3G\xe1a\xcc}\xb5\xb0\xf8\xees\x91\xc5\xceso\xce\xee\x10\x1f\x1a\x06\xcf\xe9\'`\xc5O\x19\xe4\xeb\xce\"&E\xd7`\x99\xdd\xceO7\xe0\x7f\xe7c\xad\xb0\xb8Y\xb8\xd0\x06h5`\xd36\xaeqC\x04\xf1iex|\xf3\x1f\xf3\xca(\x9fZ \x95f\xb4\xcd\xb7\xee\x8fx\xa4E\x18\xe9&/\x8d\x9b)(\xb1\xa4\xb7:m\xb9\xd4\x1c8rEX\xb1^\xeb\xf0(\x9b\xb7\x82\xca\xfd\xcf\xd63\x0f\x9f\xfb\x97\x9e\xb1\x1c\x9c\x9e\xea_^\xdb\xaa\xddT\xe90!(m\x8ey\xf3u\x92\x8c&\xfe\xdc\xc5\xf6\xc3\xb0\xdfDYC\xa3\xb6\x03(\xf6\x080\xaa\r3\xe1\xef\x9c\xa9\x07\"\xe3Y[@\x8f\xda\x88\xb7i\x08\xa8\xb7#.D\tY7[\xc7\xe3\x17\xf2\"\xebn9R\xc5\xdeT\xa7\x98\xc9K \x95\xdcF\x89_\xb4\x12\xf9\x85)\x8e\xeb\xc8\'\x15 \xc0K\xd4\xcc|\x0cl4\x0c&\x9b&1\xa6<\xa7\xf6\xd9\xd0K\xa2d\xff;\x99Ar\xc1\xe0p\x97\xf1$\xbb+\xc4t\"\xb1\xack\"2$\xd3x*\xc0\xc0\xa1/\xf1R\x05\xc9?\xefvf\xe2E\xd8\r=\xad\x95\xc8\xc7\x89&\xc8\x0f\xae\xa7\x03.\xfb\xc1_\xfa \xe1p\xad\xb0e 73`\xb0\xd5\xaf\xd7\x0c\x1c\xc2\x90p\xd7J\x18\xbc~\x01\xb0\xb0\xeb\x15\x1eD\x06\xcd\xa4O\xe8\x0c\xd1\xc3 \x10\xe1Te\x9e\xb6Q\xd0\x1avkBZXv4\xea\xb77\x19\xae.u\xf9\x96\xe5\xc1Y\xf7\x94W)%\x8d:L\xabM\x9aA\xd0_&\x03\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=HARICA TLS ECC Root CA 2021 O=Hellenic Academic and Research Institutions CA
   * Subject: CN=HARICA TLS ECC Root CA 2021 O=Hellenic Academic and Research Institutions CA
   * Label: "HARICA TLS ECC Root CA 2021"
   * Serial: 137515985548005187474074462014555733966
   * SHA256 Fingerprint: 3f:99:cc:47:4a:cf:ce:4d:fe:d5:87:94:66:5e:47:8d:15:47:73:9f:2e:78:0f:1b:b4:ca:9b:13:30:97:d4:01
   * -----BEGIN CERTIFICATE-----
   * MIICVDCCAdugAwIBAgIQZ3SdjXfYO2rbIvT/WeK/zjAKBggqhkjOPQQDAzBsMQsw
   * CQYDVQQGEwJHUjE3MDUGA1UECgwuSGVsbGVuaWMgQWNhZGVtaWMgYW5kIFJlc2Vh
   * cmNoIEluc3RpdHV0aW9ucyBDQTEkMCIGA1UEAwwbSEFSSUNBIFRMUyBFQ0MgUm9v
   * dCBDQSAyMDIxMB4XDTIxMDIxOTExMDExMFoXDTQ1MDIxMzExMDEwOVowbDELMAkG
   * A1UEBhMCR1IxNzA1BgNVBAoMLkhlbGxlbmljIEFjYWRlbWljIGFuZCBSZXNlYXJj
   * aCBJbnN0aXR1dGlvbnMgQ0ExJDAiBgNVBAMMG0hBUklDQSBUTFMgRUNDIFJvb3Qg
   * Q0EgMjAyMTB2MBAGByqGSM49AgEGBSuBBAAiA2IABDgI/rGgltJ6rK9JOtDA4MM7
   * KKrxcm1lAEeIhPyaJmuqS7psBAqIXhfyVYf8MLA04jRYVxqEU+kw2anylnTDUR9Y
   * STHMmE5gEYd103KUkE+bECUqqHgtvpBBWJAVcqeht6NCMEAwDwYDVR0TAQH/BAUw
   * AwEB/zAdBgNVHQ4EFgQUyRtTgRL+BNUW0aq8mm+3oJUZbsowDgYDVR0PAQH/BAQD
   * AgGGMAoGCCqGSM49BAMDA2cAMGQCMBHervjcToiwqfAircJRQO9gcS3ujwLEXQNw
   * SaSS6sUUiHCm0w2wqsosQJz76YJumgIwK0eaB8bRwoF8yguWGEEbo/QwCZ61IygN
   * nxS2PFOiTAZpffpskcYqSUXm7LcT4Tps
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GR1705\x06\x03U\x04\n\x0c.Hellenic Academic and Research Institutions CA1$0\"\x06\x03U\x04\x03\x0c\x1bHARICA TLS ECC Root CA 2021"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x048\x08\xfe\xb1\xa0\x96\xd2z\xac\xafI:\xd0\xc0\xe0\xc3;(\xaa\xf1rme\x00G\x88\x84\xfc\x9a&k\xaaK\xbal\x04\n\x88^\x17\xf2U\x87\xfc0\xb04\xe24XW\x1a\x84S\xe90\xd9\xa9\xf2\x96t\xc3Q\x1fXI1\xcc\x98N`\x11\x87u\xd3r\x94\x90O\x9b\x10%*\xa8x-\xbe\x90AX\x90\x15r\xa7\xa1\xb7"),
    name_constraints: None
  },

  /*
   * Issuer: CN=emSign Root CA - G1 O=eMudhra Technologies Limited OU=emSign PKI
   * Subject: CN=emSign Root CA - G1 O=eMudhra Technologies Limited OU=emSign PKI
   * Label: "emSign Root CA - G1"
   * Serial: 235931866688319308814040
   * SHA256 Fingerprint: 40:f6:af:03:46:a9:9a:a1:cd:1d:55:5a:4e:9c:ce:62:c7:f9:63:46:03:ee:40:66:15:83:3d:c8:c8:d0:03:67
   * -----BEGIN CERTIFICATE-----
   * MIIDlDCCAnygAwIBAgIKMfXkYgxsWO3W2DANBgkqhkiG9w0BAQsFADBnMQswCQYD
   * VQQGEwJJTjETMBEGA1UECxMKZW1TaWduIFBLSTElMCMGA1UEChMcZU11ZGhyYSBU
   * ZWNobm9sb2dpZXMgTGltaXRlZDEcMBoGA1UEAxMTZW1TaWduIFJvb3QgQ0EgLSBH
   * MTAeFw0xODAyMTgxODMwMDBaFw00MzAyMTgxODMwMDBaMGcxCzAJBgNVBAYTAklO
   * MRMwEQYDVQQLEwplbVNpZ24gUEtJMSUwIwYDVQQKExxlTXVkaHJhIFRlY2hub2xv
   * Z2llcyBMaW1pdGVkMRwwGgYDVQQDExNlbVNpZ24gUm9vdCBDQSAtIEcxMIIBIjAN
   * BgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAk0u76WaK7p1b1TST0Bsew+eeuGQz
   * f2N4aLTNLnF115sgxk0pvLZoYIr3IZpWNVrzdr3YzZr/k1ZLpVkGoZM0Kd0WNHVO
   * 8oG0x5ZOrRkVUkr+PHB1cM2vK6sVmjM8qrOLqs1D/fXqcP/tzxE7lM5OMhbTI0Aq
   * d7OvPAEsbO2ZLIvZTmmYsvePQbAyeGHWDV/D+qJAkh1cF+ZwPjXnorfCYuKrpDhM
   * tTk1b+oDafo6VGiFbdbyL0NVHpENDtjVaqSW0RM8LHhQ6DqS0hdW5TUaQBw+jSzt
   * Od9C4INBdN+jzcKGYEho42kLVACL5HZpIQ15TjQIXhTCzLG3rdd8cIrHhQIDAQAB
   * o0IwQDAdBgNVHQ4EFgQU++8Nhp6w492pufEhF38+/PB3KxowDgYDVR0PAQH/BAQD
   * AgEGMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQELBQADggEBAFn/8oz1h31x
   * PaOfG1vR2vjTnGs2vZupYeveFix0PZ7mddrXuqe8QhfnPZHr5X3dPpzxz5KsbEjM
   * wiI/aTvFthUvozXGaCocV685743QNcMYDHsAVhzNixl03r4PEuDQqqE/AjSxcM6d
   * GNYIAwlG7mDgfrbESQRRfXBgvKqy/3lyeqYdPV8q+Mri/Tm3R7nrft8EI6/6nAYH
   * 6ftjk4BAtcZsCjEozgyfz7MjNYBBjWzEN3uBL4ChQEKF6dk4jeihU80Bv2noWgby
   * RQuQ+q7hv53yrlc8pa6yVvSLZUDp/TGBLPQ5Cdjua6e0ph0VpZj3AYHYhX3zUVxx
   * iN66zB+Afko=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02IN1\x130\x11\x06\x03U\x04\x0b\x13\nemSign PKI1%0#\x06\x03U\x04\n\x13\x1ceMudhra Technologies Limited1\x1c0\x1a\x06\x03U\x04\x03\x13\x13emSign Root CA - G1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\x93K\xbb\xe9f\x8a\xee\x9d[\xd54\x93\xd0\x1b\x1e\xc3\xe7\x9e\xb8d3\x7fcxh\xb4\xcd.qu\xd7\x9b \xc6M)\xbc\xb6h`\x8a\xf7!\x9aV5Z\xf3v\xbd\xd8\xcd\x9a\xff\x93VK\xa5Y\x06\xa1\x934)\xdd\x164uN\xf2\x81\xb4\xc7\x96N\xad\x19\x15RJ\xfe<pup\xcd\xaf+\xab\x15\x9a3<\xaa\xb3\x8b\xaa\xcdC\xfd\xf5\xeap\xff\xed\xcf\x11;\x94\xceN2\x16\xd3#@*w\xb3\xaf<\x01,l\xed\x99,\x8b\xd9Ni\x98\xb2\xf7\x8fA\xb02xa\xd6\r_\xc3\xfa\xa2@\x92\x1d\\\x17\xe6p>5\xe7\xa2\xb7\xc2b\xe2\xab\xa48L\xb595o\xea\x03i\xfa:Th\x85m\xd6\xf2/CU\x1e\x91\r\x0e\xd8\xd5j\xa4\x96\xd1\x13<,xP\xe8:\x92\xd2\x17V\xe55\x1a@\x1c>\x8d,\xed9\xdfB\xe0\x83At\xdf\xa3\xcd\xc2\x86`Hh\xe3i\x0bT\x00\x8b\xe4vi!\ryN4\x08^\x14\xc2\xcc\xb1\xb7\xad\xd7|p\x8a\xc7\x85\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Secure Global CA O=SecureTrust Corporation
   * Subject: CN=Secure Global CA O=SecureTrust Corporation
   * Label: "Secure Global CA"
   * Serial: 9751836167731051554232119481456978597
   * SHA256 Fingerprint: 42:00:f5:04:3a:c8:59:0e:bb:52:7d:20:9e:d1:50:30:29:fb:cb:d4:1c:a1:b5:06:ec:27:f1:5a:de:7d:ac:69
   * -----BEGIN CERTIFICATE-----
   * MIIDvDCCAqSgAwIBAgIQB1YipOjUiolN9BPI8PjqpTANBgkqhkiG9w0BAQUFADBK
   * MQswCQYDVQQGEwJVUzEgMB4GA1UEChMXU2VjdXJlVHJ1c3QgQ29ycG9yYXRpb24x
   * GTAXBgNVBAMTEFNlY3VyZSBHbG9iYWwgQ0EwHhcNMDYxMTA3MTk0MjI4WhcNMjkx
   * MjMxMTk1MjA2WjBKMQswCQYDVQQGEwJVUzEgMB4GA1UEChMXU2VjdXJlVHJ1c3Qg
   * Q29ycG9yYXRpb24xGTAXBgNVBAMTEFNlY3VyZSBHbG9iYWwgQ0EwggEiMA0GCSqG
   * SIb3DQEBAQUAA4IBDwAwggEKAoIBAQCvNS7YrGxVaQZx5RNoJLNP2MwhR/jxYDiJ
   * iQPpvepeRlMJ3Fz1Wuj3RSoC6zFh1ykzTM7HfAo3fg+6MpjhHZevj8fcyTiW89sa
   * /FHtaMbQbqR8JNGuQsiWUGMu4P51/pinX0kuleM5M2SOHqRfkNJnPLLZ/kG5VacJ
   * jnIFHovdRIWCQtBJwB1g8NEXLJXr9qXBkqPFwqcIYA1gBBCWeZ4WNOaptvolRTnI
   * HmX5k/Wq8VLcmZg9pYYaDDUz+kulBAYVHDGA76oYa8J719rO+TMg1fW9ajMtgQT7
   * sFzUnKPiXB3jqUJ1XnvUd+85VLrJChgbEplJL4hL/VBi0XPnj3pDAgMBAAGjgZ0w
   * gZowEwYJKwYBBAGCNxQCBAYeBABDAEEwCwYDVR0PBAQDAgGGMA8GA1UdEwEB/wQF
   * MAMBAf8wHQYDVR0OBBYEFK9EBMJBfkiD2045AuzshHrmzsmkMDQGA1UdHwQtMCsw
   * KaAnoCWGI2h0dHA6Ly9jcmwuc2VjdXJldHJ1c3QuY29tL1NHQ0EuY3JsMBAGCSsG
   * AQQBgjcVAQQDAgEAMA0GCSqGSIb3DQEBBQUAA4IBAQBjGghAfaReUw132HquHw0L
   * URYD7xh8yOOvaliTFGCRsoTciE6+OYo68+aCiV0BN7OrJKQVDpI1WkpEXk5X+nXO
   * H0jOZvQ8QCaSmGwb7iRGDBezUqXbpZGRzzfTb+cnCDpOGR86p1hcF895P4vkp9Mm
   * I50mD1hp/Ed+stCNi5O/KU9DaXR2Z0vPB4zmAve14bRDtUstFJ/53CYNv6ZHdAbY
   * iNE6KTCEztI5gGIbqMdXSbxqVVFnFUq+NQfk1XWYN3kwFNspnWzFacxHVaIw98xc
   * f8LDmBxrThaA63p4ZUWiABqvDA1VZDRIuJK58bRQKfJPIx/abKwfROHdI3hRW8cW
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1 0\x1e\x06\x03U\x04\n\x13\x17SecureTrust Corporation1\x190\x17\x06\x03U\x04\x03\x13\x10Secure Global CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xaf5.\xd8\xaclUi\x06q\xe5\x13h$\xb3O\xd8\xcc!G\xf8\xf1`8\x89\x89\x03\xe9\xbd\xea^FS\t\xdc\\\xf5Z\xe8\xf7E*\x02\xeb1a\xd7)3L\xce\xc7|\n7~\x0f\xba2\x98\xe1\x1d\x97\xaf\x8f\xc7\xdc\xc98\x96\xf3\xdb\x1a\xfcQ\xedh\xc6\xd0n\xa4|$\xd1\xaeB\xc8\x96Pc.\xe0\xfeu\xfe\x98\xa7_I.\x95\xe393d\x8e\x1e\xa4_\x90\xd2g<\xb2\xd9\xfeA\xb9U\xa7\t\x8er\x05\x1e\x8b\xddD\x85\x82B\xd0I\xc0\x1d`\xf0\xd1\x17,\x95\xeb\xf6\xa5\xc1\x92\xa3\xc5\xc2\xa7\x08`\r`\x04\x10\x96y\x9e\x164\xe6\xa9\xb6\xfa%E9\xc8\x1ee\xf9\x93\xf5\xaa\xf1R\xdc\x99\x98=\xa5\x86\x1a\x0c53\xfaK\xa5\x04\x06\x15\x1c1\x80\xef\xaa\x18k\xc2{\xd7\xda\xce\xf93 \xd5\xf5\xbdj3-\x81\x04\xfb\xb0\\\xd4\x9c\xa3\xe2\\\x1d\xe3\xa9Bu^{\xd4w\xef9T\xba\xc9\n\x18\x1b\x12\x99I/\x88K\xfdPb\xd1s\xe7\x8fzC\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Global Root CA O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Global Root CA O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Global Root CA"
   * Serial: 10944719598952040374951832963794454346
   * SHA256 Fingerprint: 43:48:a0:e9:44:4c:78:cb:26:5e:05:8d:5e:89:44:b4:d8:4f:96:62:bd:26:db:25:7f:89:34:a4:43:c7:01:61
   * -----BEGIN CERTIFICATE-----
   * MIIDrzCCApegAwIBAgIQCDvgVpBCRrGhdWrJWZHHSjANBgkqhkiG9w0BAQUFADBh
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBD
   * QTAeFw0wNjExMTAwMDAwMDBaFw0zMTExMTAwMDAwMDBaMGExCzAJBgNVBAYTAlVT
   * MRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAXBgNVBAsTEHd3dy5kaWdpY2VydC5j
   * b20xIDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IENBMIIBIjANBgkqhkiG
   * 9w0BAQEFAAOCAQ8AMIIBCgKCAQEA4jvhEXLeqKTTo1eqUKKPC3eQyaKl7hLOllsB
   * CSDMAZOnTjC3U/dDxGkAV53ijSLdhwZAAIEJzs4bg7/fzTtxRuLWZscFs3YnFo97
   * nh6Vfe63SKMI2tavegw5BmV/Sl0fvBf4q77uKNd0f3p4mVmFaG5cIzJLv07A6Fpt
   * 43C/dxC//AH2hdmoRBBYMql1GNXRor5H4idq9Joz+EkIYIvUX7Q6hL+hqkpMfT7P
   * T19sdl6gSzeRntwi5m3OFBqOasv+zbMUZBfHWymeMr/y7vrTC0LUq7dBMtoM1O/4
   * gdW7jVg/tRvoSSiicNoxBN33shbyTApOB6jtSj1etX+jkMOvJwIDAQABo2MwYTAO
   * BgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUA95QNVbR
   * TLtm8KPiGxvDl7I90VUwHwYDVR0jBBgwFoAUA95QNVbRTLtm8KPiGxvDl7I90VUw
   * DQYJKoZIhvcNAQEFBQADggEBAMucN6pIExIK+t1EnE9SsPTfrgT1eXkIoyQY/Esr
   * hMAtudXH/vTBH1jLuG2cenTnmCmrEbXjcKChzUyImZOMkXDiqw8cvpOp/2PV5Adg
   * 06O/nVsJ8dWO41P0jmP6P6fbtGbfYmbW0W5BjfIttep3Sp+dWOIrWcBAI+0tKIJF
   * PnlUkiaY4IBIqDfv8NZ5YBberOgOzW6sRBc4L0na4UU+Krk2U886UAb3LujEV0ls
   * YSEY1QSteDwsOoBrp+uvFRTp2InBuThs4pFsiv9kuXclVzDAGySj4dzp30d8tbQk
   * CAUw7C29C79Fv1C5qfPrmAESrciIxpg0X40KPMbp1ZWVbd4=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1 0\x1e\x06\x03U\x04\x03\x13\x17DigiCert Global Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xe2;\xe1\x11r\xde\xa8\xa4\xd3\xa3W\xaaP\xa2\x8f\x0bw\x90\xc9\xa2\xa5\xee\x12\xce\x96[\x01\t \xcc\x01\x93\xa7N0\xb7S\xf7C\xc4i\x00W\x9d\xe2\x8d\"\xdd\x87\x06@\x00\x81\t\xce\xce\x1b\x83\xbf\xdf\xcd;qF\xe2\xd6f\xc7\x05\xb3v\'\x16\x8f{\x9e\x1e\x95}\xee\xb7H\xa3\x08\xda\xd6\xafz\x0c9\x06e\x7fJ]\x1f\xbc\x17\xf8\xab\xbe\xee(\xd7t\x7fzx\x99Y\x85hn\\#2K\xbfN\xc0\xe8Zm\xe3p\xbfw\x10\xbf\xfc\x01\xf6\x85\xd9\xa8D\x10X2\xa9u\x18\xd5\xd1\xa2\xbeG\xe2\'j\xf4\x9a3\xf8I\x08`\x8b\xd4_\xb4:\x84\xbf\xa1\xaaJL}>\xcfO_lv^\xa0K7\x91\x9e\xdc\"\xe6m\xce\x14\x1a\x8ej\xcb\xfe\xcd\xb3\x14d\x17\xc7[)\x9e2\xbf\xf2\xee\xfa\xd3\x0bB\xd4\xab\xb7A2\xda\x0c\xd4\xef\xf8\x81\xd5\xbb\x8dX?\xb5\x1b\xe8I(\xa2p\xda1\x04\xdd\xf7\xb2\x16\xf2L\nN\x07\xa8\xedJ=^\xb5\x7f\xa3\x90\xc3\xaf\'\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Entrust Root Certification Authority - G2 O=Entrust, Inc. OU=See www.entrust.net/legal-terms/(c) 2009 Entrust, Inc. - for authorized use only
   * Subject: CN=Entrust Root Certification Authority - G2 O=Entrust, Inc. OU=See www.entrust.net/legal-terms/(c) 2009 Entrust, Inc. - for authorized use only
   * Label: "Entrust Root Certification Authority - G2"
   * Serial: 1246989352
   * SHA256 Fingerprint: 43:df:57:74:b0:3e:7f:ef:5f:e4:0d:93:1a:7b:ed:f1:bb:2e:6b:42:73:8c:4e:6d:38:41:10:3d:3a:a7:f3:39
   * -----BEGIN CERTIFICATE-----
   * MIIEPjCCAyagAwIBAgIESlOMKDANBgkqhkiG9w0BAQsFADCBvjELMAkGA1UEBhMC
   * VVMxFjAUBgNVBAoTDUVudHJ1c3QsIEluYy4xKDAmBgNVBAsTH1NlZSB3d3cuZW50
   * cnVzdC5uZXQvbGVnYWwtdGVybXMxOTA3BgNVBAsTMChjKSAyMDA5IEVudHJ1c3Qs
   * IEluYy4gLSBmb3IgYXV0aG9yaXplZCB1c2Ugb25seTEyMDAGA1UEAxMpRW50cnVz
   * dCBSb290IENlcnRpZmljYXRpb24gQXV0aG9yaXR5IC0gRzIwHhcNMDkwNzA3MTcy
   * NTU0WhcNMzAxMjA3MTc1NTU0WjCBvjELMAkGA1UEBhMCVVMxFjAUBgNVBAoTDUVu
   * dHJ1c3QsIEluYy4xKDAmBgNVBAsTH1NlZSB3d3cuZW50cnVzdC5uZXQvbGVnYWwt
   * dGVybXMxOTA3BgNVBAsTMChjKSAyMDA5IEVudHJ1c3QsIEluYy4gLSBmb3IgYXV0
   * aG9yaXplZCB1c2Ugb25seTEyMDAGA1UEAxMpRW50cnVzdCBSb290IENlcnRpZmlj
   * YXRpb24gQXV0aG9yaXR5IC0gRzIwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEK
   * AoIBAQC6hLZy254Ma+KZ6TABp3bqMriVQRrJ2mFOWHLP/vaCeb9zYQYKpSfYs1/T
   * RU4cctZOMvJyig/3gxnQaoCAAEUesMfnmr8SVycco2gvCoe9amsOXmXzHHfV1IWN
   * cCG0szLni6LVhjkCsbjSR87kyUnEO6fe+1R9V77w6G7CebI6C1XiUJgWMhNcL3hW
   * wcKUs/Ja5CeanyTXxuzQmyWC48zCxEXFjJd6BmsqEZ+pCm5IO2/b1BEZQvePB7/1
   * U1+cPvQXLOZprE4yTGJ36rfo5bs0vBmLrpxR57d+tVOxMyLlbc9wPBr64ptntoP0
   * jaWvYkxN4FisZDQSA/i2jZRjJKRxAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAP
   * BgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBRqciZ60B7vfec7aVHUbI2fkBJmqzAN
   * BgkqhkiG9w0BAQsFAAOCAQEAeZ8dlsa2eT8ijYfThwMEYGprmi5ZiXMRrEPR9RP/
   * jTkrwPK9T3CMqS/qF8QLVJ7UG5aYMzyorWKiAHarWWluBh1+xLlEjZivEtRh2woZ
   * Rkfz6/djwUAFQKXSt/S1mja/qYh2iARVBCuch38aNzx+LaUa2NSJXsq9rD1s2G2v
   * 1fN2D807iDginWyTmsQ9v4IbZT+mD12q/OWyFcq1rca8PdCE6OoGcrBNOTJ4vz4R
   * nAuknZoh8/CbCzB428Hch0P+vGOaysXCHMnHjf87ElgI5rY97HosTvuDls4MPGmH
   * VHOkc8KT/1EQrBVUAdj8BbGJoX90g5pJ19xOe4pIb4tF9g==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x160\x14\x06\x03U\x04\n\x13\rEntrust, Inc.1(0&\x06\x03U\x04\x0b\x13\x1fSee www.entrust.net/legal-terms1907\x06\x03U\x04\x0b\x130(c) 2009 Entrust, Inc. - for authorized use only1200\x06\x03U\x04\x03\x13)Entrust Root Certification Authority - G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xba\x84\xb6r\xdb\x9e\x0ck\xe2\x99\xe90\x01\xa7v\xea2\xb8\x95A\x1a\xc9\xdaaNXr\xcf\xfe\xf6\x82y\xbfsa\x06\n\xa5\'\xd8\xb3_\xd3EN\x1cr\xd6N2\xf2r\x8a\x0f\xf7\x83\x19\xd0j\x80\x80\x00E\x1e\xb0\xc7\xe7\x9a\xbf\x12W\'\x1c\xa3h/\n\x87\xbdjk\x0e^e\xf3\x1cw\xd5\xd4\x85\x8dp!\xb4\xb32\xe7\x8b\xa2\xd5\x869\x02\xb1\xb8\xd2G\xce\xe4\xc9I\xc4;\xa7\xde\xfbT}W\xbe\xf0\xe8n\xc2y\xb2:\x0bU\xe2P\x98\x162\x13\\/xV\xc1\xc2\x94\xb3\xf2Z\xe4\'\x9a\x9f$\xd7\xc6\xec\xd0\x9b%\x82\xe3\xcc\xc2\xc4E\xc5\x8c\x97z\x06k*\x11\x9f\xa9\nnH;o\xdb\xd4\x11\x19B\xf7\x8f\x07\xbf\xf5S_\x9c>\xf4\x17,\xe6i\xacN2Lbw\xea\xb7\xe8\xe5\xbb4\xbc\x19\x8b\xae\x9cQ\xe7\xb7~\xb5S\xb13\"\xe5m\xcfp<\x1a\xfa\xe2\x9bg\xb6\x83\xf4\x8d\xa5\xafbLM\xe0X\xacd4\x12\x03\xf8\xb6\x8d\x94c$\xa4q\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Hellenic Academic and Research Institutions ECC RootCA 2015 O=Hellenic Academic and Research Institutions Cert. Authority
   * Subject: CN=Hellenic Academic and Research Institutions ECC RootCA 2015 O=Hellenic Academic and Research Institutions Cert. Authority
   * Label: "Hellenic Academic and Research Institutions ECC RootCA 2015"
   * Serial: 0
   * SHA256 Fingerprint: 44:b5:45:aa:8a:25:e6:5a:73:ca:15:dc:27:fc:36:d2:4c:1c:b9:95:3a:06:65:39:b1:15:82:dc:48:7b:48:33
   * -----BEGIN CERTIFICATE-----
   * MIICwzCCAkqgAwIBAgIBADAKBggqhkjOPQQDAjCBqjELMAkGA1UEBhMCR1IxDzAN
   * BgNVBAcTBkF0aGVuczFEMEIGA1UEChM7SGVsbGVuaWMgQWNhZGVtaWMgYW5kIFJl
   * c2VhcmNoIEluc3RpdHV0aW9ucyBDZXJ0LiBBdXRob3JpdHkxRDBCBgNVBAMTO0hl
   * bGxlbmljIEFjYWRlbWljIGFuZCBSZXNlYXJjaCBJbnN0aXR1dGlvbnMgRUNDIFJv
   * b3RDQSAyMDE1MB4XDTE1MDcwNzEwMzcxMloXDTQwMDYzMDEwMzcxMlowgaoxCzAJ
   * BgNVBAYTAkdSMQ8wDQYDVQQHEwZBdGhlbnMxRDBCBgNVBAoTO0hlbGxlbmljIEFj
   * YWRlbWljIGFuZCBSZXNlYXJjaCBJbnN0aXR1dGlvbnMgQ2VydC4gQXV0aG9yaXR5
   * MUQwQgYDVQQDEztIZWxsZW5pYyBBY2FkZW1pYyBhbmQgUmVzZWFyY2ggSW5zdGl0
   * dXRpb25zIEVDQyBSb290Q0EgMjAxNTB2MBAGByqGSM49AgEGBSuBBAAiA2IABJKg
   * QehLgoRc4vgxEZmGZE4JJS+dQS8KrjVPdJWyUWRrjWvmP3CV8AVER6ZyOFB2lQJa
   * jq4onvktTpnvLEhvTCUp6NFxW98dwXU3tNf6e3pCnGoKVlp8aQuqgAkkbH7BRqNC
   * MEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFLQi
   * C4KZJAEOnLvkDv2/+5cgk5kqMAoGCCqGSM49BAMCA2cAMGQCMGfOFmI4oqxiRaep
   * lSTAGiecMjvAwNW6qef4BENThe5SId6d9SWDPp5YSy/XZxMOIQIwBeF1Ad5o7Sof
   * TUwJCA3sS61kFyjndc5FZXIhF8siQQ6ME5g4mlRtm8rifOoCWCKR
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GR1\x0f0\r\x06\x03U\x04\x07\x13\x06Athens1D0B\x06\x03U\x04\n\x13;Hellenic Academic and Research Institutions Cert. Authority1D0B\x06\x03U\x04\x03\x13;Hellenic Academic and Research Institutions ECC RootCA 2015"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x92\xa0A\xe8K\x82\x84\\\xe2\xf81\x11\x99\x86dN\t%/\x9dA/\n\xae5Ot\x95\xb2Qdk\x8dk\xe6?p\x95\xf0\x05DG\xa6r8Pv\x95\x02Z\x8e\xae(\x9e\xf9-N\x99\xef,HoL%)\xe8\xd1q[\xdf\x1d\xc1u7\xb4\xd7\xfa{zB\x9cj\nVZ|i\x0b\xaa\x80\t$l~\xc1F"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Go Daddy Root Certificate Authority - G2 O=GoDaddy.com, Inc.
   * Subject: CN=Go Daddy Root Certificate Authority - G2 O=GoDaddy.com, Inc.
   * Label: "Go Daddy Root Certificate Authority - G2"
   * Serial: 0
   * SHA256 Fingerprint: 45:14:0b:32:47:eb:9c:c8:c5:b4:f0:d7:b5:30:91:f7:32:92:08:9e:6e:5a:63:e2:74:9d:d3:ac:a9:19:8e:da
   * -----BEGIN CERTIFICATE-----
   * MIIDxTCCAq2gAwIBAgIBADANBgkqhkiG9w0BAQsFADCBgzELMAkGA1UEBhMCVVMx
   * EDAOBgNVBAgTB0FyaXpvbmExEzARBgNVBAcTClNjb3R0c2RhbGUxGjAYBgNVBAoT
   * EUdvRGFkZHkuY29tLCBJbmMuMTEwLwYDVQQDEyhHbyBEYWRkeSBSb290IENlcnRp
   * ZmljYXRlIEF1dGhvcml0eSAtIEcyMB4XDTA5MDkwMTAwMDAwMFoXDTM3MTIzMTIz
   * NTk1OVowgYMxCzAJBgNVBAYTAlVTMRAwDgYDVQQIEwdBcml6b25hMRMwEQYDVQQH
   * EwpTY290dHNkYWxlMRowGAYDVQQKExFHb0RhZGR5LmNvbSwgSW5jLjExMC8GA1UE
   * AxMoR28gRGFkZHkgUm9vdCBDZXJ0aWZpY2F0ZSBBdXRob3JpdHkgLSBHMjCCASIw
   * DQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAL9xYgjx+lk09xvJGKP3gElY6SKD
   * E6bFIEMBO4Tx5oVJnyfq9oQbTqC023CYxzIBsQU+B07u9PpPL1kwIuerGVZr4oAH
   * /PMWdYA5UXvl+TW2dE6pjYIT5LY/qQOD+qK+ihVqf94Lw7YZFAXK6sOoBJQ7Rnwy
   * DfMAZiLIjWltNowRGLfTshxgtDj6AozO091GB94KPutdfMh8+7ArU6SSYmlRJQVh
   * GkSBjCypQ5Yj36w6gZoOKcUcqeldHraenjAKOc7xiID7S13MMuyFYkMlNAJWJwGR
   * tDtwKj9useiciAF9n9T521NtYJ2/LOdYq7hfRvzOxBsDPAnrSTFcaUaz4EcCAwEA
   * AaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYE
   * FDqahQcQZyi27/a9BUFuIMGU2g/eMA0GCSqGSIb3DQEBCwUAA4IBAQCZ21151fmX
   * WWcDYfF+OwYxdS2hII5PZYe096acvNjpL9DbWu7PdIxztDhC2gV7+AJ1uP2lsdeu
   * 9tfeE8tTEH6KRtGX+rcuKxGrkLAngPnon1rpN5+r5N9ss4UXnT3ZJE95kTXWXwTr
   * gIOrmgIttRD02JDHBHNA7XIloKmf7J6raBKZV8aPEjoJpL1E/QYVN8Gb5DKj7Tjo
   * 2GTzLH4U/ALqn83/B2gX2yKQOC16jdFU8WnjXzPKej17CuPKf1855eJ1usV2GDPO
   * LPAvTK33sefOT6jEm0pUBsV/fdUID+Ic/n4XuKxe9tQWskMJDE32p2u0mYRlynqI
   * 4uJEvlz36hz1
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x100\x0e\x06\x03U\x04\x08\x13\x07Arizona1\x130\x11\x06\x03U\x04\x07\x13\nScottsdale1\x1a0\x18\x06\x03U\x04\n\x13\x11GoDaddy.com, Inc.110/\x06\x03U\x04\x03\x13(Go Daddy Root Certificate Authority - G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xbfqb\x08\xf1\xfaY4\xf7\x1b\xc9\x18\xa3\xf7\x80IX\xe9\"\x83\x13\xa6\xc5 C\x01;\x84\xf1\xe6\x85I\x9f\'\xea\xf6\x84\x1bN\xa0\xb4\xdbp\x98\xc72\x01\xb1\x05>\x07N\xee\xf4\xfaO/Y0\"\xe7\xab\x19Vk\xe2\x80\x07\xfc\xf3\x16u\x809Q{\xe5\xf95\xb6tN\xa9\x8d\x82\x13\xe4\xb6?\xa9\x03\x83\xfa\xa2\xbe\x8a\x15j\x7f\xde\x0b\xc3\xb6\x19\x14\x05\xca\xea\xc3\xa8\x04\x94;F|2\r\xf3\x00f\"\xc8\x8dim6\x8c\x11\x18\xb7\xd3\xb2\x1c`\xb48\xfa\x02\x8c\xce\xd3\xddF\x07\xde\n>\xeb]|\xc8|\xfb\xb0+S\xa4\x92biQ%\x05a\x1aD\x81\x8c,\xa9C\x96#\xdf\xac:\x81\x9a\x0e)\xc5\x1c\xa9\xe9]\x1e\xb6\x9e\x9e0\n9\xce\xf1\x88\x80\xfbK]\xcc2\xec\x85bC%4\x02V\'\x01\x91\xb4;p*?n\xb1\xe8\x9c\x88\x01}\x9f\xd4\xf9\xdbSm`\x9d\xbf,\xe7X\xab\xb8_F\xfc\xce\xc4\x1b\x03<\t\xebI1\\iF\xb3\xe0G\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TUBITAK Kamu SM SSL Kok Sertifikasi - Surum 1 O=Turkiye Bilimsel ve Teknolojik Arastirma Kurumu - TUBITAK OU=Kamu Sertifikasyon Merkezi - Kamu SM
   * Subject: CN=TUBITAK Kamu SM SSL Kok Sertifikasi - Surum 1 O=Turkiye Bilimsel ve Teknolojik Arastirma Kurumu - TUBITAK OU=Kamu Sertifikasyon Merkezi - Kamu SM
   * Label: "TUBITAK Kamu SM SSL Kok Sertifikasi - Surum 1"
   * Serial: 1
   * SHA256 Fingerprint: 46:ed:c3:68:90:46:d5:3a:45:3f:b3:10:4a:b8:0d:ca:ec:65:8b:26:60:ea:16:29:dd:7e:86:79:90:64:87:16
   * -----BEGIN CERTIFICATE-----
   * MIIEYzCCA0ugAwIBAgIBATANBgkqhkiG9w0BAQsFADCB0jELMAkGA1UEBhMCVFIx
   * GDAWBgNVBAcTD0dlYnplIC0gS29jYWVsaTFCMEAGA1UEChM5VHVya2l5ZSBCaWxp
   * bXNlbCB2ZSBUZWtub2xvamlrIEFyYXN0aXJtYSBLdXJ1bXUgLSBUVUJJVEFLMS0w
   * KwYDVQQLEyRLYW11IFNlcnRpZmlrYXN5b24gTWVya2V6aSAtIEthbXUgU00xNjA0
   * BgNVBAMTLVRVQklUQUsgS2FtdSBTTSBTU0wgS29rIFNlcnRpZmlrYXNpIC0gU3Vy
   * dW0gMTAeFw0xMzExMjUwODI1NTVaFw00MzEwMjUwODI1NTVaMIHSMQswCQYDVQQG
   * EwJUUjEYMBYGA1UEBxMPR2ViemUgLSBLb2NhZWxpMUIwQAYDVQQKEzlUdXJraXll
   * IEJpbGltc2VsIHZlIFRla25vbG9qaWsgQXJhc3Rpcm1hIEt1cnVtdSAtIFRVQklU
   * QUsxLTArBgNVBAsTJEthbXUgU2VydGlmaWthc3lvbiBNZXJrZXppIC0gS2FtdSBT
   * TTE2MDQGA1UEAxMtVFVCSVRBSyBLYW11IFNNIFNTTCBLb2sgU2VydGlmaWthc2kg
   * LSBTdXJ1bSAxMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAr3UwM6q7
   * a9OZLBI3hNmNe5eA027n/5tQlT6QlVZC1xl8JoSNkvoBHToP4mQ4t4y86Ij5iySr
   * LqP1N+RAjhgleYN1Hzv/bKjFxlb4tO2KRKOrbEz8HdDc72i9z+SqzvBV96I01INr
   * N3wcwv61A+xXzry0tcXtAA9TNypN9E8Mg/uGz8v+jE69h/mniyFXnHrfA2eJLJ2X
   * YacQuFWQfw4tJzh03+f92k4S400VIgLI4OD8D62K18lUUMw7D8oWgITQUVbDjlZ/
   * iSIzL+aFCr2lqBs23tPcLG07xxO9WSMs5uWk99gL7eqQQESolbuT1dCANLZGeA4f
   * AJNG4e7p+exPFwIDAQABo0IwQDAdBgNVHQ4EFgQUZT/HiobGPN08VFw1+DrtUgxH
   * V8gwDgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQEL
   * BQADggEBACo/4fEyjq7hmFxLXs9rHmoJ0iKpEsdeV31zVmSAhHqT5Am5EM2fKifh
   * AHe+SMg1qIGf5LgsyX8OsNJLN13qudULXjS99HMpw+0mFZx+CFOKWI3QSyjfwbPf
   * IPP54+M638yclNhOT8NrF7f3cuitZjO1JVOr4PhMqZ398g26rrnZqsZr+ZO7rqu4
   * lzwDGrpDxpa5RXI4s6ehlj2Re37AIVNMh+3yC1SVUZPVIqUNivGTDj5UDrDYyU7c
   * 8jEyVupk+eq1nRZmQnLzf9OxMUP8pI4X8W0jq5Rm+K37DwhuJi1/FwcJsoz7UMCf
   * lo3Ptv0AnVoUmr8CRPXBwp8iXqIPoeM=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TR1\x180\x16\x06\x03U\x04\x07\x13\x0fGebze - Kocaeli1B0@\x06\x03U\x04\n\x139Turkiye Bilimsel ve Teknolojik Arastirma Kurumu - TUBITAK1-0+\x06\x03U\x04\x0b\x13$Kamu Sertifikasyon Merkezi - Kamu SM1604\x06\x03U\x04\x03\x13-TUBITAK Kamu SM SSL Kok Sertifikasi - Surum 1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xafu03\xaa\xbbk\xd3\x99,\x127\x84\xd9\x8d{\x97\x80\xd3n\xe7\xff\x9bP\x95>\x90\x95VB\xd7\x19|&\x84\x8d\x92\xfa\x01\x1d:\x0f\xe2d8\xb7\x8c\xbc\xe8\x88\xf9\x8b$\xab.\xa3\xf57\xe4@\x8e\x18%y\x83u\x1f;\xffl\xa8\xc5\xc6V\xf8\xb4\xed\x8aD\xa3\xablL\xfc\x1d\xd0\xdc\xefh\xbd\xcf\xe4\xaa\xce\xf0U\xf7\xa24\xd4\x83k7|\x1c\xc2\xfe\xb5\x03\xecW\xce\xbc\xb4\xb5\xc5\xed\x00\x0fS7*M\xf4O\x0c\x83\xfb\x86\xcf\xcb\xfe\x8cN\xbd\x87\xf9\xa7\x8b!W\x9cz\xdf\x03g\x89,\x9d\x97a\xa7\x10\xb8U\x90\x7f\x0e-\'8t\xdf\xe7\xfd\xdaN\x12\xe3M\x15\"\x02\xc8\xe0\xe0\xfc\x0f\xad\x8a\xd7\xc9TP\xcc;\x0f\xca\x16\x80\x84\xd0QV\xc3\x8eV\x7f\x89\"3/\xe6\x85\n\xbd\xa5\xa8\x1b6\xde\xd3\xdc,m;\xc7\x13\xbdY#,\xe6\xe5\xa4\xf7\xd8\x0b\xed\xea\x90@D\xa8\x95\xbb\x93\xd5\xd0\x804\xb6Fx\x0e\x1f\x00\x93F\xe1\xee\xe9\xf9\xecO\x17\x02\x03\x01\x00\x01"),
    name_constraints: Some(Der::from_slice(b"\xa0\x070\x05\x82\x03.tr"))
  },

  /*
   * Issuer: CN=D-TRUST Root Class 3 CA 2 2009 O=D-Trust GmbH
   * Subject: CN=D-TRUST Root Class 3 CA 2 2009 O=D-Trust GmbH
   * Label: "D-TRUST Root Class 3 CA 2 2009"
   * Serial: 623603
   * SHA256 Fingerprint: 49:e7:a4:42:ac:f0:ea:62:87:05:00:54:b5:25:64:b6:50:e4:f4:9e:42:e3:48:d6:aa:38:e0:39:e9:57:b1:c1
   * -----BEGIN CERTIFICATE-----
   * MIIEMzCCAxugAwIBAgIDCYPzMA0GCSqGSIb3DQEBCwUAME0xCzAJBgNVBAYTAkRF
   * MRUwEwYDVQQKDAxELVRydXN0IEdtYkgxJzAlBgNVBAMMHkQtVFJVU1QgUm9vdCBD
   * bGFzcyAzIENBIDIgMjAwOTAeFw0wOTExMDUwODM1NThaFw0yOTExMDUwODM1NTha
   * ME0xCzAJBgNVBAYTAkRFMRUwEwYDVQQKDAxELVRydXN0IEdtYkgxJzAlBgNVBAMM
   * HkQtVFJVU1QgUm9vdCBDbGFzcyAzIENBIDIgMjAwOTCCASIwDQYJKoZIhvcNAQEB
   * BQADggEPADCCAQoCggEBANOySs96R+91myP6Oi/WUEWJNTrGa9v+2wBoqOADER03
   * UAifTUpolDWzU9GUY6cgVq/eUXjsKj3zSEhQPgrfRlWLJ23DEE0NkVJD2IfgXU42
   * tSHKXzlABF9bfsyjxiupQB7ZNoTWSPOSHjRGICTBpFGOShrvUD9pXRl/RcPHAY9R
   * ySPocq60vFYJfxLLHLGvKZAKyVXMD9O0Gu1HNVpK7ZxzBCHQqr0ME7UAyiZsxGsM
   * lFqVlNpQmvH/pStmMaTJOKDfHR+4CS7zp+hnUquVH+BGPtikw8paxTGA6Eian5Rp
   * /hnd2HN8gcqW3o7tszIFZYQ05ub9VxC1X3a/L7AQDcUCAwEAAaOCARowggEWMA8G
   * A1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFP3aFMSfMN4hvR5COfyrYyNJ4PGEMA4G
   * A1UdDwEB/wQEAwIBBjCB0wYDVR0fBIHLMIHIMIGAoH6gfIZ6bGRhcDovL2RpcmVj
   * dG9yeS5kLXRydXN0Lm5ldC9DTj1ELVRSVVNUJTIwUm9vdCUyMENsYXNzJTIwMyUy
   * MENBJTIwMiUyMDIwMDksTz1ELVRydXN0JTIwR21iSCxDPURFP2NlcnRpZmljYXRl
   * cmV2b2NhdGlvbmxpc3QwQ6BBoD+GPWh0dHA6Ly93d3cuZC10cnVzdC5uZXQvY3Js
   * L2QtdHJ1c3Rfcm9vdF9jbGFzc18zX2NhXzJfMjAwOS5jcmwwDQYJKoZIhvcNAQEL
   * BQADggEBAH+X2zDI36ScfSF6gHDOFBJpiBSVYEQBrLLpME+bUMJm2H6NMLVwMeni
   * acfzcNsgFYbQDfC+rAF1hM5+n02/t2A7nPPKHeJeaNijnZflQGDSNiH+0LS4F9p0
   * o3/U37CYAqxva2ssJSRyoWXuJVrl5jLn8t+rSfrzkGkj2wTZ51xY/GXUl77M/C4K
   * zCUqNQT4YJEVdT1B/yMfGchs64JTBKbkTCJNjYy6zltz7GRUUG3RnFX7acM2w4y8
   * PIWmawomDeCTmGCufsYkl4phX5GOZpIJhzbNi5stPvZR1FDUWSi9g/LMKHtThm3Y
   * Johw1+qRzT65ysCQblrGXnRl11z+o+I=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x0c\x0cD-Trust GmbH1\'0%\x06\x03U\x04\x03\x0c\x1eD-TRUST Root Class 3 CA 2 2009"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd3\xb2J\xcfzG\xefu\x9b#\xfa:/\xd6PE\x895:\xc6k\xdb\xfe\xdb\x00h\xa8\xe0\x03\x11\x1d7P\x08\x9fMJh\x945\xb3S\xd1\x94c\xa7 V\xaf\xdeQx\xec*=\xf3HHP>\n\xdfFU\x8b\'m\xc3\x10M\r\x91RC\xd8\x87\xe0]N6\xb5!\xca_9@\x04_[~\xcc\xa3\xc6+\xa9@\x1e\xd96\x84\xd6H\xf3\x92\x1e4F $\xc1\xa4Q\x8eJ\x1a\xefP?i]\x19\x7fE\xc3\xc7\x01\x8fQ\xc9#\xe8r\xae\xb4\xbcV\t\x7f\x12\xcb\x1c\xb1\xaf)\x90\n\xc9U\xcc\x0f\xd3\xb4\x1a\xedG5ZJ\xed\x9cs\x04!\xd0\xaa\xbd\x0c\x13\xb5\x00\xca&l\xc4k\x0c\x94Z\x95\x94\xdaP\x9a\xf1\xff\xa5+f1\xa4\xc98\xa0\xdf\x1d\x1f\xb8\t.\xf3\xa7\xe8gR\xab\x95\x1f\xe0F>\xd8\xa4\xc3\xcaZ\xc51\x80\xe8H\x9a\x9f\x94i\xfe\x19\xdd\xd8s|\x81\xca\x96\xde\x8e\xed\xb32\x05e\x844\xe6\xe6\xfdW\x10\xb5_v\xbf/\xb0\x10\r\xc5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SecureSign Root CA14 O=Cybertrust Japan Co., Ltd.
   * Subject: CN=SecureSign Root CA14 O=Cybertrust Japan Co., Ltd.
   * Label: "SecureSign Root CA14"
   * Serial: 575790784512929437950770173562378038616896959179
   * SHA256 Fingerprint: 4b:00:9c:10:34:49:4f:9a:b5:6b:ba:3b:a1:d6:27:31:fc:4d:20:d8:95:5a:dc:ec:10:a9:25:60:72:61:e3:38
   * -----BEGIN CERTIFICATE-----
   * MIIFcjCCA1qgAwIBAgIUZNtaDCBO6Ncpd8hQJ6JaJ90t8sswDQYJKoZIhvcNAQEM
   * BQAwUTELMAkGA1UEBhMCSlAxIzAhBgNVBAoTGkN5YmVydHJ1c3QgSmFwYW4gQ28u
   * LCBMdGQuMR0wGwYDVQQDExRTZWN1cmVTaWduIFJvb3QgQ0ExNDAeFw0yMDA0MDgw
   * NzA2MTlaFw00NTA0MDgwNzA2MTlaMFExCzAJBgNVBAYTAkpQMSMwIQYDVQQKExpD
   * eWJlcnRydXN0IEphcGFuIENvLiwgTHRkLjEdMBsGA1UEAxMUU2VjdXJlU2lnbiBS
   * b290IENBMTQwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDF0nqh1oq/
   * FjHQmNE6lPxauG4iwWL3pwon71D2LrGeaBLwbCRjOfHw3xDG3rdSINVSW0KZnvOg
   * vlIfX8xnbacuUKLBl422+JX1sLrcneC+y9/3OPJH9aaakpUqYllQC6KxNedlsmGy
   * 6pJxaeQp8E+BgQQ8sqVb1MWoWWd7VRxJq3qdwudzTe/NCcLEVxLbAQ4jeQkHO6Lo
   * /IrPj8BGJJw4J+CDnRugv3gVEOuGTgpa/d/aLIJ+7sr2KeH6caH3iGicnPCNvg9J
   * kdjqOvn90Ghx2+m1K06Ckm9mH+Dw3EzsytHqunQG+bOEkJTRX45zGRBdAuVwpcAQ
   * 0BB8b8VYSbSwbprafZX1zNoCr7gsfXmPvkPx+SgojQlD+Ajda8iLLCSxjVIHvXib
   * y8posqTdDEx5YMaZ0ZPxMBoH064iwurO8YQJzOAUbn8/ftKChazcqRZOhaBgy/ac
   * 18izju3Gm5h1DVXoX+WViwKkrkMpKBGk5hIwAUt1ax5mnXkvpXYvHUC0bcl9eQjs
   * 0Wq2XSqypWa9a4X0dFbD9ed1Uigspf9mR6XU/v6eVL9lfgHWMI+lNpyiUBzuOIAB
   * SMbHdPTGrMNASRZhdCyvjG817XsYAFs2PJxQDcqSMxDxJklt33UkN4Ii1+iW/RVL
   * ApY+B3KVfqs9TC7XyvDf4Fg/LS8EmjijAQIDAQABo0IwQDAPBgNVHRMBAf8EBTAD
   * AQH/MA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUBpOjCl4oaTeqYR3r6/wtbyPk
   * 86AwDQYJKoZIhvcNAQEMBQADggIBAJaAcgkGfpzMkwQWu6A6jZJOtxEaCnFxEM0E
   * rX+lRVAQZk5KQaID2RFPeje5S+LGjzJmdSX7684/AykmjbgWHfYfM25I5uj4V7Ib
   * ed87hwriZLoAymzvftAj63iP/2SbNDefNWWipAA9EiOWWF3KY4fGoweITedpdopT
   * zfFP7ELyk+OZpDc8h7hi2/DsHzc/N19DzFGdtfCXwreFamgLRB7lUe6TzktuhsHS
   * DCRZNhqfLJGP4xjblJUK7ZGqDpncllPjYYPGFrojutzdfhrGe0K22VoF3Jpf1d+4
   * 2kd92jjbrDnVHmtsKheMYc2xbXIBw8MgAGJoFjHVdqqGuw6qnsb58Nn4DSEC5MUo
   * FlkRudlpcyqSeLiSV5sI8jrlL5WwWLdrIBRtFO8KvH7YVdiI2i/6GaX7i+B/OfVy
   * K4XELKzvGUWSTLNhB9xNH27SgRNcmvMSZ4PPmz+Ln52kuaiWA3rF7iDeM9ovnhp6
   * dB7h7sxaOgTdsxoEqBRjrLdHEoOabPXm6RUVkRqEGQ6UROcSjiVbgGcZ3GOTEAtl
   * Lor6CZpO2oYofaphNdgOpygau1LgePhsumywbrmHXumZNTfxPWQrqaA0k89jL9WB
   * 365jJ6UeTo3cKXhZ+PmhIIynJkBugnLNeLLIjzwec+fBH7/PzqUqm9tEZDKgu39c
   * JRNItX+S
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02JP1#0!\x06\x03U\x04\n\x13\x1aCybertrust Japan Co., Ltd.1\x1d0\x1b\x06\x03U\x04\x03\x13\x14SecureSign Root CA14"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc5\xd2z\xa1\xd6\x8a\xbf\x161\xd0\x98\xd1:\x94\xfcZ\xb8n\"\xc1b\xf7\xa7\n\'\xefP\xf6.\xb1\x9eh\x12\xf0l$c9\xf1\xf0\xdf\x10\xc6\xde\xb7R \xd5R[B\x99\x9e\xf3\xa0\xbeR\x1f_\xccgm\xa7.P\xa2\xc1\x97\x8d\xb6\xf8\x95\xf5\xb0\xba\xdc\x9d\xe0\xbe\xcb\xdf\xf78\xf2G\xf5\xa6\x9a\x92\x95*bYP\x0b\xa2\xb15\xe7e\xb2a\xb2\xea\x92qi\xe4)\xf0O\x81\x81\x04<\xb2\xa5[\xd4\xc5\xa8Yg{U\x1cI\xabz\x9d\xc2\xe7sM\xef\xcd\t\xc2\xc4W\x12\xdb\x01\x0e#y\t\x07;\xa2\xe8\xfc\x8a\xcf\x8f\xc0F$\x9c8\'\xe0\x83\x9d\x1b\xa0\xbfx\x15\x10\xeb\x86N\nZ\xfd\xdf\xda,\x82~\xee\xca\xf6)\xe1\xfaq\xa1\xf7\x88h\x9c\x9c\xf0\x8d\xbe\x0fI\x91\xd8\xea:\xf9\xfd\xd0hq\xdb\xe9\xb5+N\x82\x92of\x1f\xe0\xf0\xdcL\xec\xca\xd1\xea\xbat\x06\xf9\xb3\x84\x90\x94\xd1_\x8es\x19\x10]\x02\xe5p\xa5\xc0\x10\xd0\x10|o\xc5XI\xb4\xb0n\x9a\xda}\x95\xf5\xcc\xda\x02\xaf\xb8,}y\x8f\xbeC\xf1\xf9((\x8d\tC\xf8\x08\xddk\xc8\x8b,$\xb1\x8dR\x07\xbdx\x9b\xcb\xcah\xb2\xa4\xdd\x0cLy`\xc6\x99\xd1\x93\xf10\x1a\x07\xd3\xae\"\xc2\xea\xce\xf1\x84\t\xcc\xe0\x14n\x7f?~\xd2\x82\x85\xac\xdc\xa9\x16N\x85\xa0`\xcb\xf6\x9c\xd7\xc8\xb3\x8e\xed\xc6\x9b\x98u\rU\xe8_\xe5\x95\x8b\x02\xa4\xaeC)(\x11\xa4\xe6\x120\x01Kuk\x1ef\x9dy/\xa5v/\x1d@\xb4m\xc9}y\x08\xec\xd1j\xb6]*\xb2\xa5f\xbdk\x85\xf4tV\xc3\xf5\xe7uR(,\xa5\xfffG\xa5\xd4\xfe\xfe\x9eT\xbfe~\x01\xd60\x8f\xa56\x9c\xa2P\x1c\xee8\x80\x01H\xc6\xc7t\xf4\xc6\xac\xc3@I\x16at,\xaf\x8co5\xed{\x18\x00[6<\x9cP\r\xca\x923\x10\xf1&Im\xdfu$7\x82\"\xd7\xe8\x96\xfd\x15K\x02\x96>\x07r\x95~\xab=L.\xd7\xca\xf0\xdf\xe0X?-/\x04\x9a8\xa3\x01\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign Root R46 O=GlobalSign nv-sa
   * Subject: CN=GlobalSign Root R46 O=GlobalSign nv-sa
   * Label: "GlobalSign Root R46"
   * Serial: 1552617688466950547958867513931858518042577
   * SHA256 Fingerprint: 4f:a3:12:6d:8d:3a:11:d1:c4:85:5a:4f:80:7c:ba:d6:cf:91:9d:3a:5a:88:b0:3b:ea:2c:63:72:d9:3c:40:c9
   * -----BEGIN CERTIFICATE-----
   * MIIFWjCCA0KgAwIBAgISEdK7udcjGJ5AXwqdLdDfJWfRMA0GCSqGSIb3DQEBDAUA
   * MEYxCzAJBgNVBAYTAkJFMRkwFwYDVQQKExBHbG9iYWxTaWduIG52LXNhMRwwGgYD
   * VQQDExNHbG9iYWxTaWduIFJvb3QgUjQ2MB4XDTE5MDMyMDAwMDAwMFoXDTQ2MDMy
   * MDAwMDAwMFowRjELMAkGA1UEBhMCQkUxGTAXBgNVBAoTEEdsb2JhbFNpZ24gbnYt
   * c2ExHDAaBgNVBAMTE0dsb2JhbFNpZ24gUm9vdCBSNDYwggIiMA0GCSqGSIb3DQEB
   * AQUAA4ICDwAwggIKAoICAQCsrHQy6LNl5brtQyYdpokNRbopiLKkHWPd08EsCVeJ
   * OaFV6Wc0dwxu5FUdUiXSE2te4R2pt32JMl8Nnp8semNgQB+msLZ4j5lUlghYruQG
   * vGIFAha/r6gjA7aUD7xubMLL1aa7DOn2wQL7Id5m3RerdELv8HQvJfTqa1VbkNud
   * 316HCkD7rRlr+/fKYIje2sGP1q7Vf9Q8g+7XFkyDRTNrJ9CG0Bwta/OrffGFqfUo
   * 0q3v84RLHIf8E6M6cqJaESvWJ3En7YEtbWaBkoe0G1h6zD8K+kZPTXhc+CtI4wSE
   * y132tGqzZfxCnlEmIyDLPRT5ge1lFgBPGmSXZgjPjHvjK8Cd+RTyG/FWaha/LIWF
   * zXg4mutCagI0GIMXTpRW+LaCtfOW3T3zvn8gdz57GSNrLNRyc0NXfeD412lPFzYE
   * +cCQYDdF3uYM2HSNrpyibXRdQr4G9dlkbgIQrImwTDsHTUB+JMWKmIJ5jqSngiCN
   * I/onccnfxkF0oE32kRbcRoxfKWMxWXEM2G/CtjJ9++ZdU6Z+Ffy7dXxd7Pj2Fxzs
   * x2sZy/N78CsHpdlseVR2bJ0cpm4O6XkMqCNqo98bMDGfsVR7/mrLZqrcZdCinkqa
   * ByFrgY/bxFn63iLABJzjqls2k+g9vXqhnQt2sQvHnf3PmKgGwvgqo6GDoLclcqUC
   * 4wIDAQABo0IwQDAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNV
   * HQ4EFgQUA1yrc4GHqMywptWU4jaWSf8FmSwwDQYJKoZIhvcNAQEMBQADggIBAHx4
   * 7PYCLLtbfpIrXTncvtgdokIzTfnvpCo7RGkerNlFo048p9gkUbJUHJNOxO97k4Vg
   * JuoJSOD1u8fpaNK7ajFxzHmuEajwmf3lH7wvqMxX63bEIaZHU1VNaL8FpO7XJqti
   * 2kM3S+LGteWygxk6x9PbTZ4IevPuzz5i+6zoYMzRx6Fcg0XERczzF2sUyQQCPtIk
   * pnnpHs6i58FZFZ8d4kuaPp92CC1r2LpXFNqD6v6MVenQTqnMdzGxRBF6XLE+0xRF
   * FRhiJBPSy03OXIPBNvIQtQ6IbbjhVp+J3pZmOUdkLG5NrmJ7v2B0GbhWrJKsFjLt
   * rWhV/pi60zTe9Mlhww6G9kuEYO4Ne7UyWHmRVSyBQ7N0H3qqJZ4d16GLuc1CLgSk
   * ZoNNiTW2bKg2SnkheCLQQrzRQDGQob4Ez8pn7fXwgNNgyYMqIgXQBztSvwyeqiv5
   * u+YfjyW6hY0XHgL+XVAEV8/+LbzvXMAaq7afJMbfc2hIkCwU9D9SGuTSyxTDYWnP
   * 4vkYxboznxSjBF25cfe1lNj2M8FawTSLfJvdkzrnE6JwYZ+vj+vYxXX4M2bUdGc6
   * N3ec592kD3ZDZopD8p/7DEJ4Y9HiD2971KE9dJeFt0g5QdYg/NA6s/rob8SKunE3
   * vouXsXgxT7PntgMTzlSdriVZzH81Xwj3QEUxeCp6
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BE1\x190\x17\x06\x03U\x04\n\x13\x10GlobalSign nv-sa1\x1c0\x1a\x06\x03U\x04\x03\x13\x13GlobalSign Root R46"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xac\xact2\xe8\xb3e\xe5\xba\xedC&\x1d\xa6\x89\rE\xba)\x88\xb2\xa4\x1dc\xdd\xd3\xc1,\tW\x899\xa1U\xe9g4w\x0cn\xe4U\x1dR%\xd2\x13k^\xe1\x1d\xa9\xb7}\x892_\r\x9e\x9f,zc`@\x1f\xa6\xb0\xb6x\x8f\x99T\x96\x08X\xae\xe4\x06\xbcb\x05\x02\x16\xbf\xaf\xa8#\x03\xb6\x94\x0f\xbcnl\xc2\xcb\xd5\xa6\xbb\x0c\xe9\xf6\xc1\x02\xfb!\xdef\xdd\x17\xabtB\xef\xf0t/%\xf4\xeakU[\x90\xdb\x9d\xdf^\x87\n@\xfb\xad\x19k\xfb\xf7\xca`\x88\xde\xda\xc1\x8f\xd6\xae\xd5\x7f\xd4<\x83\xee\xd7\x16L\x83E3k\'\xd0\x86\xd0\x1c-k\xf3\xab}\xf1\x85\xa9\xf5(\xd2\xad\xef\xf3\x84K\x1c\x87\xfc\x13\xa3:r\xa2Z\x11+\xd6\'q\'\xed\x81-mf\x81\x92\x87\xb4\x1bXz\xcc?\n\xfaFOMx\\\xf8+H\xe3\x04\x84\xcb]\xf6\xb4j\xb3e\xfcB\x9eQ&# \xcb=\x14\xf9\x81\xede\x16\x00O\x1ad\x97f\x08\xcf\x8c{\xe3+\xc0\x9d\xf9\x14\xf2\x1b\xf1Vj\x16\xbf,\x85\x85\xcdx8\x9a\xebBj\x024\x18\x83\x17N\x94V\xf8\xb6\x82\xb5\xf3\x96\xdd=\xf3\xbe\x7f w>{\x19#k,\xd4rsCW}\xe0\xf8\xd7iO\x176\x04\xf9\xc0\x90`7E\xde\xe6\x0c\xd8t\x8d\xae\x9c\xa2mt]B\xbe\x06\xf5\xd9dn\x02\x10\xac\x89\xb0L;\x07M@~$\xc5\x8a\x98\x82y\x8e\xa4\xa7\x82 \x8d#\xfa\'q\xc9\xdf\xc6At\xa0M\xf6\x91\x16\xdcF\x8c_)c1Yq\x0c\xd8o\xc2\xb62}\xfb\xe6]S\xa6~\x15\xfc\xbbu|]\xec\xf8\xf6\x17\x1c\xec\xc7k\x19\xcb\xf3{\xf0+\x07\xa5\xd9lyTvl\x9d\x1c\xa6n\x0e\xe9y\x0c\xa8#j\xa3\xdf\x1b01\x9f\xb1T{\xfej\xcbf\xaa\xdce\xd0\xa2\x9eJ\x9a\x07!k\x81\x8f\xdb\xc4Y\xfa\xde\"\xc0\x04\x9c\xe3\xaa[6\x93\xe8=\xbdz\xa1\x9d\x0bv\xb1\x0b\xc7\x9d\xfd\xcf\x98\xa8\x06\xc2\xf8*\xa3\xa1\x83\xa0\xb7%r\xa5\x02\xe3\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=USERTrust ECC Certification Authority O=The USERTRUST Network
   * Subject: CN=USERTrust ECC Certification Authority O=The USERTRUST Network
   * Label: "USERTrust ECC Certification Authority"
   * Serial: 123013823720199481456569720443997572134
   * SHA256 Fingerprint: 4f:f4:60:d5:4b:9c:86:da:bf:bc:fc:57:12:e0:40:0d:2b:ed:3f:bc:4d:4f:bd:aa:86:e0:6a:dc:d2:a9:ad:7a
   * -----BEGIN CERTIFICATE-----
   * MIICjzCCAhWgAwIBAgIQXIuZxVqUxdJxVt7NiYDMJjAKBggqhkjOPQQDAzCBiDEL
   * MAkGA1UEBhMCVVMxEzARBgNVBAgTCk5ldyBKZXJzZXkxFDASBgNVBAcTC0plcnNl
   * eSBDaXR5MR4wHAYDVQQKExVUaGUgVVNFUlRSVVNUIE5ldHdvcmsxLjAsBgNVBAMT
   * JVVTRVJUcnVzdCBFQ0MgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkwHhcNMTAwMjAx
   * MDAwMDAwWhcNMzgwMTE4MjM1OTU5WjCBiDELMAkGA1UEBhMCVVMxEzARBgNVBAgT
   * Ck5ldyBKZXJzZXkxFDASBgNVBAcTC0plcnNleSBDaXR5MR4wHAYDVQQKExVUaGUg
   * VVNFUlRSVVNUIE5ldHdvcmsxLjAsBgNVBAMTJVVTRVJUcnVzdCBFQ0MgQ2VydGlm
   * aWNhdGlvbiBBdXRob3JpdHkwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAAQarFRaqflo
   * I+d61SRvU8Za2EurxtW20eZzca7dnNYMYf3boIkDuAUU7FfO7l0/4iGzzvfUinng
   * o4N+LZfQYcTxmdwlkWOrfzCjtHDix6EznPO/LlxTsV+zfTJ/ijTjeXmjQjBAMB0G
   * A1UdDgQWBBQ64QmG1M8ZwpZ2dEl23OA1xmNjmjAOBgNVHQ8BAf8EBAMCAQYwDwYD
   * VR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAwNoADBlAjA2Z6EWCNzklwBBHU6+4WMB
   * zzuqQhFkoJ2UOQIReVx7Hfpkue4WQrO/isIJxOzksU0CMQDpKmFHjFJKS04YcPbW
   * RNZu9YO6bVi9JNlWSOrvxKJGgYhqOkbRqZtNyWHa0V1Xahg=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x130\x11\x06\x03U\x04\x08\x13\nNew Jersey1\x140\x12\x06\x03U\x04\x07\x13\x0bJersey City1\x1e0\x1c\x06\x03U\x04\n\x13\x15The USERTRUST Network1.0,\x06\x03U\x04\x03\x13%USERTrust ECC Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x1a\xacTZ\xa9\xf9h#\xe7z\xd5$oS\xc6Z\xd8K\xab\xc6\xd5\xb6\xd1\xe6sq\xae\xdd\x9c\xd6\x0ca\xfd\xdb\xa0\x89\x03\xb8\x05\x14\xecW\xce\xee]?\xe2!\xb3\xce\xf7\xd4\x8ay\xe0\xa3\x83~-\x97\xd0a\xc4\xf1\x99\xdc%\x91c\xab\x7f0\xa3\xb4p\xe2\xc7\xa13\x9c\xf3\xbf.\\S\xb1_\xb3}2\x7f\x8a4\xe3yy"),
    name_constraints: None
  },

  /*
   * Issuer: O=SECOM Trust Systems CO.,LTD. OU=Security Communication RootCA2
   * Subject: O=SECOM Trust Systems CO.,LTD. OU=Security Communication RootCA2
   * Label: "Security Communication RootCA2"
   * Serial: 0
   * SHA256 Fingerprint: 51:3b:2c:ec:b8:10:d4:cd:e5:dd:85:39:1a:df:c6:c2:dd:60:d8:7b:b7:36:d2:b5:21:48:4a:a4:7a:0e:be:f6
   * -----BEGIN CERTIFICATE-----
   * MIIDdzCCAl+gAwIBAgIBADANBgkqhkiG9w0BAQsFADBdMQswCQYDVQQGEwJKUDEl
   * MCMGA1UEChMcU0VDT00gVHJ1c3QgU3lzdGVtcyBDTy4sTFRELjEnMCUGA1UECxMe
   * U2VjdXJpdHkgQ29tbXVuaWNhdGlvbiBSb290Q0EyMB4XDTA5MDUyOTA1MDAzOVoX
   * DTI5MDUyOTA1MDAzOVowXTELMAkGA1UEBhMCSlAxJTAjBgNVBAoTHFNFQ09NIFRy
   * dXN0IFN5c3RlbXMgQ08uLExURC4xJzAlBgNVBAsTHlNlY3VyaXR5IENvbW11bmlj
   * YXRpb24gUm9vdENBMjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBANAV
   * OVKxUrO6xVmCxF1SrjpDZYBLx/KWvNs2l9amZIyoXvDjChz335c9S672XewhtUGr
   * zbl+dp+++T42NKA7wfYxEUV0kz1XgMX5iZnK5atq1LXaQZAQwdbWQonCv/Q4EpVM
   * VAX3NuRFg3sUZdbcDE3R3n4MqzvEFb46VqZab3ZpUql6ucjrappdUtAtCms1FgkQ
   * hNBqyjoGADdH5H5XTz+L62e4iKrFvlNVspHEfbmwhRkGeC7bYRr6hfVKkaHnFtWO
   * ojnflLhwHyg/i/xAXmODPIMqGplrz95Zajv8bxbXH/1KEOtOghY6rCcMU/Gt1SSw
   * awNQwS08Ft1ENCcadfsCAwEAAaNCMEAwHQYDVR0OBBYEFAqFqXdlBZh8QIH4D5cs
   * OPEK7DzPMA4GA1UdDwEB/wQEAwIBBjAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3
   * DQEBCwUAA4IBAQBMOqNErLlFsceTfsgLCkLfZOoc7llsCLqJX2rKSpWeeo8HxdpF
   * coJxDjrSzG+ntKEju/Ykn8sX/oymzsLS28yN/HH8AynBbF0zX2S2ZTuJbxh2ePXc
   * okgfGT+Ok+vx+hfuzU7jBBJV1uXk3fs+BXziHV7Gp7yXT2g69ekuCkO2r1dcYmh8
   * t/2jioSgrGK+KwmHNPBqAbubKVY8/gA3zyNs8U6qtnRGEmyR7jTV7JqR50S+kDFy
   * 1UkC9gLl9B/rfNmWVan/7Ir5mUf/NVoCqgTLiluHcSmRvaS0eg29mvVXIwAHIRc/
   * SjnRBUkLp7Y3gaVdjKozXoEofKd9J+sAro03
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02JP1%0#\x06\x03U\x04\n\x13\x1cSECOM Trust Systems CO.,LTD.1\'0%\x06\x03U\x04\x0b\x13\x1eSecurity Communication RootCA2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd0\x159R\xb1R\xb3\xba\xc5Y\x82\xc4]R\xae:Ce\x80K\xc7\xf2\x96\xbc\xdb6\x97\xd6\xa6d\x8c\xa8^\xf0\xe3\n\x1c\xf7\xdf\x97=K\xae\xf6]\xec!\xb5A\xab\xcd\xb9~v\x9f\xbe\xf9>64\xa0;\xc1\xf61\x11Et\x93=W\x80\xc5\xf9\x89\x99\xca\xe5\xabj\xd4\xb5\xdaA\x90\x10\xc1\xd6\xd6B\x89\xc2\xbf\xf48\x12\x95LT\x05\xf76\xe4E\x83{\x14e\xd6\xdc\x0cM\xd1\xde~\x0c\xab;\xc4\x15\xbe:V\xa6ZoviR\xa9z\xb9\xc8\xebj\x9a]R\xd0-\nk5\x16\t\x10\x84\xd0j\xca:\x06\x007G\xe4~WO?\x8b\xebg\xb8\x88\xaa\xc5\xbeSU\xb2\x91\xc4}\xb9\xb0\x85\x19\x06x.\xdba\x1a\xfa\x85\xf5J\x91\xa1\xe7\x16\xd5\x8e\xa29\xdf\x94\xb8p\x1f(?\x8b\xfc@^c\x83<\x83*\x1a\x99k\xcf\xdeYj;\xfco\x16\xd7\x1f\xfdJ\x10\xebN\x82\x16:\xac\'\x0cS\xf1\xad\xd5$\xb0k\x03P\xc1-<\x16\xddD4\'\x1au\xfb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=COMODO RSA Certification Authority O=COMODO CA Limited
   * Subject: CN=COMODO RSA Certification Authority O=COMODO CA Limited
   * Label: "COMODO RSA Certification Authority"
   * Serial: 101909084537582093308941363524873193117
   * SHA256 Fingerprint: 52:f0:e1:c4:e5:8e:c6:29:29:1b:60:31:7f:07:46:71:b8:5d:7e:a8:0d:5b:07:27:34:63:53:4b:32:b4:02:34
   * -----BEGIN CERTIFICATE-----
   * MIIF2DCCA8CgAwIBAgIQTKr5yttjb+Af907YWwOGnTANBgkqhkiG9w0BAQwFADCB
   * hTELMAkGA1UEBhMCR0IxGzAZBgNVBAgTEkdyZWF0ZXIgTWFuY2hlc3RlcjEQMA4G
   * A1UEBxMHU2FsZm9yZDEaMBgGA1UEChMRQ09NT0RPIENBIExpbWl0ZWQxKzApBgNV
   * BAMTIkNPTU9ETyBSU0EgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkwHhcNMTAwMTE5
   * MDAwMDAwWhcNMzgwMTE4MjM1OTU5WjCBhTELMAkGA1UEBhMCR0IxGzAZBgNVBAgT
   * EkdyZWF0ZXIgTWFuY2hlc3RlcjEQMA4GA1UEBxMHU2FsZm9yZDEaMBgGA1UEChMR
   * Q09NT0RPIENBIExpbWl0ZWQxKzApBgNVBAMTIkNPTU9ETyBSU0EgQ2VydGlmaWNh
   * dGlvbiBBdXRob3JpdHkwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCR
   * 6FSS0gpWsawNJN3Fz0RndJkrN6N9I3AAcbxT38T6KhKPS38QVr2fcHK3YX/JSw8X
   * pz3jsARh7v8Rl8f0hj4K+j5c+ZPmNHrZFGvnnLOFoIJ6dq9xkNfs/Q36nGz637CC
   * 9BR++b7Epi9Pf5l/tfxnQ3K9DADWietrLNPtj5gcFKt+5eNu/Nio5JIk2kNrYrhV
   * /erBvGy2i/MOjZrkm2xpmfh4SDBF1a3hDTxFYPwyllEnvGfDyi62a+pGx8cgoLEf
   * Zd5ICLqkTqnyg0Y3hOvozIFIQ2dOciqbXL1MGyiKXCJ7tKuY2e7gUYPDCUZObT6Z
   * +pUX2nwzV0E8jVHtC7ZcryxjGt9XyD+86V3Em69FmeKjWiS0uqlWPc9vqv9JWL7w
   * qP/0uK3pN/u6uPQLOvnoQ0IeidiEyxPx2bvhiWC4jChWrBQdnArncevPDt09qZah
   * SL0896+1DSJMwBGB7FY79tOi4lu3sgQiUpWAk2nojkxl8ZEDLXB0AuqLZxUpaVIC
   * u9ffUGpVRr+goyhhf3DQw6KqLCGqR84onAZFdr+CGCe01a60y1Dma/RMhnEw6abf
   * Fobg2P9A3fvQQoh/ozM6LlweQRGBY84YcWsr7KaKtzFcOmpH4MN5WdYgGq/yapiq
   * crxXStJLnbsQ/LBMQeXtHT1eKJ2czL+zUdqnR+WEUwIDAQABo0IwQDAdBgNVHQ4E
   * FgQUu69+Aj36pvE8hI6t7jiY7NkyMtQwDgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB
   * /wQFMAMBAf8wDQYJKoZIhvcNAQEMBQADggIBAArx1UaEt65Ru2yyTUEUAJNMnMvl
   * wFTPoCWOAvn9sKIN9SCYPBMtrFaisNZ+EZLpLrqeLppysb0ZRGxhNaKatBYSaVqM
   * 4dc+pBroLwP0rmEdEBsqpIt6xf4FpuHA1sj+nq6PK7o9mfjYcwlYRm6mnPTXJ9OV
   * 2jeDchzTc+CiR5kDOF3VSXkAKRzH7JsgHAckaVd4sjn8OoSgtZx8jb8uk2Intzna
   * FxiuvTwJaP+EmzzV1gsD41eeFPfR60/IvYcjt7ZJQ3mFXLrrkguhxuhoqEwWsRqZ
   * CuhTLJK7oQkYdQxlqHvLI7cawiiFwxv/0Cti76R7CZGYZ4wUAc1oBmpjIXUDgIiK
   * boHGhfKppC3n9KUkEEeDys30jXlYsQab5xoq2Z0B15R97QNKyvDb6KkBPvVWmcke
   * jkk9u+UJueBPSZI9FoJAzMxZxuY67RIuaTxslbH9qh17f4a+Hg4yRvv7E491f0yL
   * S0Zj/gA0QHDBw7mh3aZw4gSzQbzpgJHqZJx64SIDqZxubw5lT2yHh17zbqD5daWb
   * QOhTsiedSrnAdyGN/4fy3ryM7xfft0kL0fJuMAsaDk527RH89elWsn2/x20Kk4yl
   * 0MC2Hb46TpSi125sC8KKfPog88Tk5c0NqMuRkrF8hey1FGlmDoLnzc7ILaZRfyHB
   * NVOFBkpdn627G190
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GB1\x1b0\x19\x06\x03U\x04\x08\x13\x12Greater Manchester1\x100\x0e\x06\x03U\x04\x07\x13\x07Salford1\x1a0\x18\x06\x03U\x04\n\x13\x11COMODO CA Limited1+0)\x06\x03U\x04\x03\x13\"COMODO RSA Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x91\xe8T\x92\xd2\nV\xb1\xac\r$\xdd\xc5\xcfDgt\x99+7\xa3}#p\x00q\xbcS\xdf\xc4\xfa*\x12\x8fK\x7f\x10V\xbd\x9fpr\xb7a\x7f\xc9K\x0f\x17\xa7=\xe3\xb0\x04a\xee\xff\x11\x97\xc7\xf4\x86>\n\xfa>\\\xf9\x93\xe64z\xd9\x14k\xe7\x9c\xb3\x85\xa0\x82zv\xafq\x90\xd7\xec\xfd\r\xfa\x9cl\xfa\xdf\xb0\x82\xf4\x14~\xf9\xbe\xc4\xa6/O\x7f\x99\x7f\xb5\xfcgCr\xbd\x0c\x00\xd6\x89\xebk,\xd3\xed\x8f\x98\x1c\x14\xab~\xe5\xe3n\xfc\xd8\xa8\xe4\x92$\xdaCkb\xb8U\xfd\xea\xc1\xbcl\xb6\x8b\xf3\x0e\x8d\x9a\xe4\x9bli\x99\xf8xH0E\xd5\xad\xe1\r<E`\xfc2\x96Q\'\xbcg\xc3\xca.\xb6k\xeaF\xc7\xc7 \xa0\xb1\x1fe\xdeH\x08\xba\xa4N\xa9\xf2\x83F7\x84\xeb\xe8\xcc\x81HCgNr*\x9b\\\xbdL\x1b(\x8a\\\"{\xb4\xab\x98\xd9\xee\xe0Q\x83\xc3\tFNm>\x99\xfa\x95\x17\xda|3WA<\x8dQ\xed\x0b\xb6\\\xaf,c\x1a\xdfW\xc8?\xbc\xe9]\xc4\x9b\xafE\x99\xe2\xa3Z$\xb4\xba\xa9V=\xcfo\xaa\xffIX\xbe\xf0\xa8\xff\xf4\xb8\xad\xe97\xfb\xba\xb8\xf4\x0b:\xf9\xe8CB\x1e\x89\xd8\x84\xcb\x13\xf1\xd9\xbb\xe1\x89`\xb8\x8c(V\xac\x14\x1d\x9c\n\xe7q\xeb\xcf\x0e\xdd=\xa9\x96\xa1H\xbd<\xf7\xaf\xb5\r\"L\xc0\x11\x81\xecV;\xf6\xd3\xa2\xe2[\xb7\xb2\x04\"R\x95\x80\x93i\xe8\x8eLe\xf1\x91\x03-pt\x02\xea\x8bg\x15)iR\x02\xbb\xd7\xdfPjUF\xbf\xa0\xa3(a\x7fp\xd0\xc3\xa2\xaa,!\xaaG\xce(\x9c\x06Ev\xbf\x82\x18\'\xb4\xd5\xae\xb4\xcbP\xe6k\xf4L\x86q0\xe9\xa6\xdf\x16\x86\xe0\xd8\xff@\xdd\xfb\xd0B\x88\x7f\xa33:.\\\x1eA\x11\x81c\xce\x18qk+\xec\xa6\x8a\xb71\\:jG\xe0\xc3yY\xd6 \x1a\xaf\xf2j\x98\xaar\xbcWJ\xd2K\x9d\xbb\x10\xfc\xb0LA\xe5\xed\x1d=^(\x9d\x9c\xcc\xbf\xb3Q\xda\xa7G\xe5\x84S\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Trusted Root G4 O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Trusted Root G4 O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Trusted Root G4"
   * Serial: 7451500558977370777930084869016614236
   * SHA256 Fingerprint: 55:2f:7b:dc:f1:a7:af:9e:6c:e6:72:01:7f:4f:12:ab:f7:72:40:c7:8e:76:1a:c2:03:d1:d9:d2:0a:c8:99:88
   * -----BEGIN CERTIFICATE-----
   * MIIFkDCCA3igAwIBAgIQBZsbV56OITLiOQe9p3d1XDANBgkqhkiG9w0BAQwFADBi
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSEwHwYDVQQDExhEaWdpQ2VydCBUcnVzdGVkIFJvb3Qg
   * RzQwHhcNMTMwODAxMTIwMDAwWhcNMzgwMTE1MTIwMDAwWjBiMQswCQYDVQQGEwJV
   * UzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNlcnQu
   * Y29tMSEwHwYDVQQDExhEaWdpQ2VydCBUcnVzdGVkIFJvb3QgRzQwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQC/5pBzaN675F1KPDAiMGkz7MKnJS7JIT3y
   * ithZwuEppz1Yq3aaza57G4QNxDAf8xukOBbrVsaXbR2rsnnyyhHS5F/WBTxSD1If
   * xp4VpX6+n6lXFllVcq9ok3DCsrp1mWpzMpTREEQQLt+C8weE5nQ7bXHiLQwb7iDV
   * ySAdYyktzuxeTsiT+CFhmzTrBcZe7FsavOvJz82sNEBfsXpm7nfISKhmV1efVFiO
   * DCu3T6cw2Vbuyntd463JT17lNecxy9qTXtyOj4DatpGYQJB5w3jHtrHEtWoYOAMQ
   * jdjUN6QuBX2I9YI+EJFwq1WCQTLX2wRzKm6RAXwhTNS8rhsDdV14Ztk6MUSaM0C/
   * CNdaSaTC5qmgZ92kJ7yhTzm1EVgX9yRcRo9k98FpiHaYdj1ZXUJ2h4mXaXpI8OCi
   * EhtmmnTK3kse5w5jrubU75KSOp493ADkRSWJtppEGSt+wJS00mFt6zPZxd9LBADM
   * fRyVw4/3IbKyEbe7f/LVjHAsQWCqsWMYRJUadmJ+9oCw++hkpjPRiQfhvbfmQ6QY
   * uKZ3AeEPlAwhHbJUKSWJbOUOUlFHdL4mrLZBdd56rF+NP8m800ERElvlEFDrMcXK
   * chYiCd98THU/Y+whX8QgUWtvsauGi0/C1kVfnSD8oR7FwI+isX4KJpn15GkvmB0t
   * 9dmpsh3lGwIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIB
   * hjAdBgNVHQ4EFgQU7NfjgtJxXWRM3y5nP+e6mK4cD08wDQYJKoZIhvcNAQEMBQAD
   * ggIBALth2X2pbL4XxJEbw6GiAI3jZGgPVs93rnD5/ZpKmbnJeFwMDF/k5hQpVgs2
   * SV1EY+CtnJYYZhsjDT156W1r1lT40jzBQ0CuHVD1UvyQO7uYmWlrx8GnqGikJ9yd
   * +SeuMIW59mdNOj6PWTkiU0TryF0Dyu1Qen1iIQqAyHNm0aAFYF/opbSnr6j3bTWc
   * fFqK1qI4mfN4i/RN0iAL3gTujJtHgXINwBQy7zBZLq7gcfJW5GqXb5JQbZaNaHqa
   * sjYUegbyJLkJEVDXCLG4iXqEI2FCKeWjzaIgQdfRnGTZ6iahixTXTBmyUEFxPT9N
   * cCOGDErcgdLMMpSEDQgJlxxPwO5rIHQw0uA5NBCFIRUBCOhVMt5xSdkoF1BN5r5N
   * 0XWs0Mr7QbhDparTwwVETyw2m+L64kW4I1NsBm9nVX9GtUw/bihaeSbSpKhil9Ie
   * 4u1Ki7wb/UdKDd9nZn6yW0HQO+T0O/QEY+nvwlQAUaCKKsnOeMzV6ocEGLPOr0mI
   * r/OSmbaz5mEP0oUA51Aa5BuVnRmhuZyxm7EAHu/QD09CbMkKvO5D+jpxpchNJqU1
   * /YldvIViHTLSoCtU7ZpXwdv6EM8Zt4tKG48BtieVU+i2iW1bvGjUI+iLUaJW+fCm
   * gKDWHrO8Dw9TdSmq6hN35N6MgSGtBxBHEa2HPQfRdbzP82Z+
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1!0\x1f\x06\x03U\x04\x03\x13\x18DigiCert Trusted Root G4"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xbf\xe6\x90sh\xde\xbb\xe4]J<0\"0i3\xec\xc2\xa7%.\xc9!=\xf2\x8a\xd8Y\xc2\xe1)\xa7=X\xabv\x9a\xcd\xae{\x1b\x84\r\xc40\x1f\xf3\x1b\xa48\x16\xebV\xc6\x97m\x1d\xab\xb2y\xf2\xca\x11\xd2\xe4_\xd6\x05<R\x0fR\x1f\xc6\x9e\x15\xa5~\xbe\x9f\xa9W\x16YUr\xafh\x93p\xc2\xb2\xbau\x99js2\x94\xd1\x10D\x10.\xdf\x82\xf3\x07\x84\xe6t;mq\xe2-\x0c\x1b\xee \xd5\xc9 \x1dc)-\xce\xec^N\xc8\x93\xf8!a\x9b4\xeb\x05\xc6^\xec[\x1a\xbc\xeb\xc9\xcf\xcd\xac4@_\xb1zf\xeew\xc8H\xa8fWW\x9fTX\x8e\x0c+\xb7O\xa70\xd9V\xee\xca{]\xe3\xad\xc9O^\xe55\xe71\xcb\xda\x93^\xdc\x8e\x8f\x80\xda\xb6\x91\x98@\x90y\xc3x\xc7\xb6\xb1\xc4\xb5j\x188\x03\x10\x8d\xd8\xd47\xa4.\x05}\x88\xf5\x82>\x10\x91p\xabU\x82A2\xd7\xdb\x04s*n\x91\x01|!L\xd4\xbc\xae\x1b\x03u]xf\xd9:1D\x9a3@\xbf\x08\xd7ZI\xa4\xc2\xe6\xa9\xa0g\xdd\xa4\'\xbc\xa1O9\xb5\x11X\x17\xf7$\\F\x8fd\xf7\xc1i\x88v\x98v=Y]Bv\x87\x89\x97izH\xf0\xe0\xa2\x12\x1bf\x9at\xca\xdeK\x1e\xe7\x0ec\xae\xe6\xd4\xef\x92\x92:\x9e=\xdc\x00\xe4E%\x89\xb6\x9aD\x19+~\xc0\x94\xb4\xd2am\xeb3\xd9\xc5\xdfK\x04\x00\xcc}\x1c\x95\xc3\x8f\xf7!\xb2\xb2\x11\xb7\xbb\x7f\xf2\xd5\x8cp,A`\xaa\xb1c\x18D\x95\x1avb~\xf6\x80\xb0\xfb\xe8d\xa63\xd1\x89\x07\xe1\xbd\xb7\xe6C\xa4\x18\xb8\xa6w\x01\xe1\x0f\x94\x0c!\x1d\xb2T)%\x89l\xe5\x0eRQGt\xbe&\xac\xb6Au\xdez\xac_\x8d?\xc9\xbc\xd3A\x11\x12[\xe5\x10P\xeb1\xc5\xcar\x16\"\t\xdf|Lu?c\xec!_\xc4 Qko\xb1\xab\x86\x8bO\xc2\xd6E_\x9d \xfc\xa1\x1e\xc5\xc0\x8f\xa2\xb1~\n&\x99\xf5\xe4i/\x98\x1d-\xf5\xd9\xa9\xb2\x1d\xe5\x1b\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=AC RAIZ FNMT-RCM SERVIDORES SEGUROS O=FNMT-RCM OU=Ceres
   * Subject: CN=AC RAIZ FNMT-RCM SERVIDORES SEGUROS O=FNMT-RCM OU=Ceres
   * Label: "AC RAIZ FNMT-RCM SERVIDORES SEGUROS"
   * Serial: 131542671362353147877283741781055151509
   * SHA256 Fingerprint: 55:41:53:b1:3d:2c:f9:dd:b7:53:bf:be:1a:4e:0a:e0:8d:0a:a4:18:70:58:fe:60:a2:b8:62:b2:e4:b8:7b:cb
   * -----BEGIN CERTIFICATE-----
   * MIICbjCCAfOgAwIBAgIQYvYybOXE42hcG2LdnC6dlTAKBggqhkjOPQQDAzB4MQsw
   * CQYDVQQGEwJFUzERMA8GA1UECgwIRk5NVC1SQ00xDjAMBgNVBAsMBUNlcmVzMRgw
   * FgYDVQRhDA9WQVRFUy1RMjgyNjAwNEoxLDAqBgNVBAMMI0FDIFJBSVogRk5NVC1S
   * Q00gU0VSVklET1JFUyBTRUdVUk9TMB4XDTE4MTIyMDA5MzczM1oXDTQzMTIyMDA5
   * MzczM1oweDELMAkGA1UEBhMCRVMxETAPBgNVBAoMCEZOTVQtUkNNMQ4wDAYDVQQL
   * DAVDZXJlczEYMBYGA1UEYQwPVkFURVMtUTI4MjYwMDRKMSwwKgYDVQQDDCNBQyBS
   * QUlaIEZOTVQtUkNNIFNFUlZJRE9SRVMgU0VHVVJPUzB2MBAGByqGSM49AgEGBSuB
   * BAAiA2IABPa6V1PIyqvfNkpSIeSX0oNnnvBlUdBeh8dHsVnyV0ebAAKTRBdp20LH
   * sbI6GA60XYyzZl2hNPk2LEnb80b8s0RpRBNm/dfF/a82Tc4DTQdxz69qBdKiQ1oK
   * Um8BA06Oi6NCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYD
   * VR0OBBYEFAG5L++/EYZg8k/QQW6rcx/n0m5JMAoGCCqGSM49BAMDA2kAMGYCMQCu
   * SuMrQMN0EfKVrRYj3k4MGuZdpSRea0R7/DjiT8ucRRcRTBQnJlU5dUoDzBOQn5IC
   * MQD6SmxgiHPz7riYYqnOK8LZiqZwMR2vsJRM60/G49HzYqc8/5MuB1xJAWdpEgJy
   * v+c=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02ES1\x110\x0f\x06\x03U\x04\n\x0c\x08FNMT-RCM1\x0e0\x0c\x06\x03U\x04\x0b\x0c\x05Ceres1\x180\x16\x06\x03U\x04a\x0c\x0fVATES-Q2826004J1,0*\x06\x03U\x04\x03\x0c#AC RAIZ FNMT-RCM SERVIDORES SEGUROS"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xf6\xbaWS\xc8\xca\xab\xdf6JR!\xe4\x97\xd2\x83g\x9e\xf0eQ\xd0^\x87\xc7G\xb1Y\xf2WG\x9b\x00\x02\x93D\x17i\xdbB\xc7\xb1\xb2:\x18\x0e\xb4]\x8c\xb3f]\xa14\xf96,I\xdb\xf3F\xfc\xb3DiD\x13f\xfd\xd7\xc5\xfd\xaf6M\xce\x03M\x07q\xcf\xafj\x05\xd2\xa2CZ\nRo\x01\x03N\x8e\x8b"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Trustwave Global ECC P384 Certification Authority O=Trustwave Holdings, Inc.
   * Subject: CN=Trustwave Global ECC P384 Certification Authority O=Trustwave Holdings, Inc.
   * Label: "Trustwave Global ECC P384 Certification Authority"
   * Serial: 2704997926503831671788816187
   * SHA256 Fingerprint: 55:90:38:59:c8:c0:c3:eb:b8:75:9e:ce:4e:25:57:22:5f:f5:75:8b:bd:38:eb:d4:82:76:60:1e:1b:d5:80:97
   * -----BEGIN CERTIFICATE-----
   * MIICnTCCAiSgAwIBAgIMCL2Fl2yZJ6SAaEc7MAoGCCqGSM49BAMDMIGRMQswCQYD
   * VQQGEwJVUzERMA8GA1UECBMISWxsaW5vaXMxEDAOBgNVBAcTB0NoaWNhZ28xITAf
   * BgNVBAoTGFRydXN0d2F2ZSBIb2xkaW5ncywgSW5jLjE6MDgGA1UEAxMxVHJ1c3R3
   * YXZlIEdsb2JhbCBFQ0MgUDM4NCBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTAeFw0x
   * NzA4MjMxOTM2NDNaFw00MjA4MjMxOTM2NDNaMIGRMQswCQYDVQQGEwJVUzERMA8G
   * A1UECBMISWxsaW5vaXMxEDAOBgNVBAcTB0NoaWNhZ28xITAfBgNVBAoTGFRydXN0
   * d2F2ZSBIb2xkaW5ncywgSW5jLjE6MDgGA1UEAxMxVHJ1c3R3YXZlIEdsb2JhbCBF
   * Q0MgUDM4NCBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTB2MBAGByqGSM49AgEGBSuB
   * BAAiA2IABGvaDXU1CDFHBa5FmVXxERMuSvgQMSOjfoPTfygIOiYaOs+Xgh+AtycJ
   * j9GOMMQKmw6sWASr9zZ9lCOkmwqKi6vr/TklZvFe/oyujUF5nQlgziip04pt89ZF
   * 1PKYhDhloKNDMEEwDwYDVR0TAQH/BAUwAwEB/zAPBgNVHQ8BAf8EBQMDBwYAMB0G
   * A1UdDgQWBBRVqYSJ0sEyvRjLbKYHTsjnnb6CkDAKBggqhkjOPQQDAwNnADBkAjA3
   * AZKXRRJ+oPM+rRk6ct30UJMDEr5E0k9BpIycnR+j9sKS50gU/k6bpZFXrsY3crsC
   * MGclCrEMXu6pY5Jv5ZAL/mYiykf9ijH3g/56vxC+GCsej/YpHpRZ744hN8tRmKVu
   * Sw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x110\x0f\x06\x03U\x04\x08\x13\x08Illinois1\x100\x0e\x06\x03U\x04\x07\x13\x07Chicago1!0\x1f\x06\x03U\x04\n\x13\x18Trustwave Holdings, Inc.1:08\x06\x03U\x04\x03\x131Trustwave Global ECC P384 Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04k\xda\ru5\x081G\x05\xaeE\x99U\xf1\x11\x13.J\xf8\x101#\xa3~\x83\xd3\x7f(\x08:&\x1a:\xcf\x97\x82\x1f\x80\xb7\'\t\x8f\xd1\x8e0\xc4\n\x9b\x0e\xacX\x04\xab\xf76}\x94#\xa4\x9b\n\x8a\x8b\xab\xeb\xfd9%f\xf1^\xfe\x8c\xae\x8dAy\x9d\t`\xce(\xa9\xd3\x8am\xf3\xd6E\xd4\xf2\x98\x848e\xa0"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Actalis Authentication Root CA O=Actalis S.p.A./03358520967
   * Subject: CN=Actalis Authentication Root CA O=Actalis S.p.A./03358520967
   * Label: "Actalis Authentication Root CA"
   * Serial: 6271844772424770508
   * SHA256 Fingerprint: 55:92:60:84:ec:96:3a:64:b9:6e:2a:be:01:ce:0b:a8:6a:64:fb:fe:bc:c7:aa:b5:af:c1:55:b3:7f:d7:60:66
   * -----BEGIN CERTIFICATE-----
   * MIIFuzCCA6OgAwIBAgIIVwoRl0LE48wwDQYJKoZIhvcNAQELBQAwazELMAkGA1UE
   * BhMCSVQxDjAMBgNVBAcMBU1pbGFuMSMwIQYDVQQKDBpBY3RhbGlzIFMucC5BLi8w
   * MzM1ODUyMDk2NzEnMCUGA1UEAwweQWN0YWxpcyBBdXRoZW50aWNhdGlvbiBSb290
   * IENBMB4XDTExMDkyMjExMjIwMloXDTMwMDkyMjExMjIwMlowazELMAkGA1UEBhMC
   * SVQxDjAMBgNVBAcMBU1pbGFuMSMwIQYDVQQKDBpBY3RhbGlzIFMucC5BLi8wMzM1
   * ODUyMDk2NzEnMCUGA1UEAwweQWN0YWxpcyBBdXRoZW50aWNhdGlvbiBSb290IENB
   * MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAp8bEpSmkLO/lGMWwUKNv
   * UTufClrJwkg4CsIcoBh/kbWHuUA/3R1oHwiD1S0eiKD4j1aPbZkCkpAW1V8IbInX
   * 4ay8IMKx4INRimlNAJZaby/ARH6jDuSRzVju3PvHHkVH3Se5CAGfpiEd9UEtL0z9
   * KK3giq0itFZljoZUj5NDKd45RnijMCO6zfB9E1fAXdKDa0hMxKufgFpbOr3JpyI/
   * gCczWw63igxdBzcIy2zSekciRDXFzMwujt0q7bd9Zg1fYVEiVRvjRuPjPdA1Yprb
   * rxTIW6HMiRvhMCb8oJsfgadHHwTrozmSBp+Z07/T6k9QnBn+locePGX2oxgkg4YQ
   * 51Q+qDp2JE+BIcXjDwL4k5RHILv+1A7TaLndxHqEguNTVHnd25zS8gebLra8Pu2F
   * be8lEfKXGkJh90qX6IuxEAf6ZYGyojnP9zz/GPvG8VqLWeICrHuS0E4UT1lF9gxe
   * KF+w6D9Fz8+vm2/7hNN3WpVvrJSEnu68wEqPSpP4RCHiMUVhUE4Q2OM1fEwZtN4F
   * v6MGn8i1zeQf1xcGDXqVdFUNaBr8EBtiZJ1t4JWgw5QHVw0U5r0F+7if5t+L4sbn
   * fpb2U8WANFAoWPASUHEXMLrmeGO89LKtmyuy/uE5jF66CyCU3nuDuP/jVo23Eek7
   * jPKxwV2dpAtMK9myGPW1n0sCAwEAAaNjMGEwHQYDVR0OBBYEFFLYiDrIn3hm7Ynz
   * ezhwlMkCAjbQMA8GA1UdEwEB/wQFMAMBAf8wHwYDVR0jBBgwFoAUUtiIOsifeGbt
   * ifN7OHCUyQICNtAwDgYDVR0PAQH/BAQDAgEGMA0GCSqGSIb3DQEBCwUAA4ICAQAL
   * e3KHwGCmSUyIWOYdiPcUZEim2FgKDk8TNd81HdTtBjHIgT5q1d07GjLukD0R0i70
   * jsNjLiNmsGe+b7bAEzlgqqI0JZN1Ut6nna0Oh4lScWoWPBkdg/iaKWW+9D+a2fDz
   * WochcYBNy+A4mz+7+uAwTc+G02UQGRjRlwKxK3JCaKygvU5a2hi/a5iB0P2avl4V
   * SM0RFbnAKVy06Ij3Pjaut2L9HmLecHgQHEhb2rykOLpn7VU+Xlff1ANATIGk0k9j
   * pwlCCRT8AKnCgHNPLsBA2RF7SOp6AsDT6ygBJlh0wcBzIm2Tlf05fbsq4/aC4yyX
   * X04fkZT6/iyj2HYauE2yOE+b+h1IYHkm4vP9qdCa6HCPSXrW5b0KDtst842/6+Ok
   * fcvHlXHo2qN8xcL4dJIEG4aspCJTQLas/kx2z/uUMsA1n3Y/buWQbqCmJqK4LL7R
   * K4X9p2jIugErsWx0Hbhzlefut8cl8ABMALJ+tguLHPPAUJ4lueAI3jZm/zel0btU
   * ZCzJJ7VLkn5l/9Mt4blOvH+kQSGQQXemOR/qnuOf0GZvBeyqdn6/axag67XH/JJU
   * LysRJyU3eExRarDzzFhdFPFqSBX/wge2sY0PjlxQRrM9vwGYT7JZVEc+NHt4bVaT
   * LnPqZih4zR0Uv6CPLy64Lo7yFIrM6bV8+2ydDKXhlg==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02IT1\x0e0\x0c\x06\x03U\x04\x07\x0c\x05Milan1#0!\x06\x03U\x04\n\x0c\x1aActalis S.p.A./033585209671\'0%\x06\x03U\x04\x03\x0c\x1eActalis Authentication Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa7\xc6\xc4\xa5)\xa4,\xef\xe5\x18\xc5\xb0P\xa3oQ;\x9f\nZ\xc9\xc2H8\n\xc2\x1c\xa0\x18\x7f\x91\xb5\x87\xb9@?\xdd\x1dh\x1f\x08\x83\xd5-\x1e\x88\xa0\xf8\x8fV\x8fm\x99\x02\x92\x90\x16\xd5_\x08l\x89\xd7\xe1\xac\xbc \xc2\xb1\xe0\x83Q\x8aiM\x00\x96Zo/\xc0D~\xa3\x0e\xe4\x91\xcdX\xee\xdc\xfb\xc7\x1eEG\xdd\'\xb9\x08\x01\x9f\xa6!\x1d\xf5A-/L\xfd(\xad\xe0\x8a\xad\"\xb4Ve\x8e\x86T\x8f\x93C)\xde9Fx\xa30#\xba\xcd\xf0}\x13W\xc0]\xd2\x83kHL\xc4\xab\x9f\x80Z[:\xbd\xc9\xa7\"?\x80\'3[\x0e\xb7\x8a\x0c]\x077\x08\xcbl\xd2zG\"D5\xc5\xcc\xcc.\x8e\xdd*\xed\xb7}f\r_aQ\"U\x1b\xe3F\xe3\xe3=\xd05b\x9a\xdb\xaf\x14\xc8[\xa1\xcc\x89\x1b\xe10&\xfc\xa0\x9b\x1f\x81\xa7G\x1f\x04\xeb\xa39\x92\x06\x9f\x99\xd3\xbf\xd3\xeaOP\x9c\x19\xfe\x96\x87\x1e<e\xf6\xa3\x18$\x83\x86\x10\xe7T>\xa8:v$O\x81!\xc5\xe3\x0f\x02\xf8\x93\x94G \xbb\xfe\xd4\x0e\xd3h\xb9\xdd\xc4z\x84\x82\xe3STy\xdd\xdb\x9c\xd2\xf2\x07\x9b.\xb6\xbc>\xed\x85m\xef%\x11\xf2\x97\x1aBa\xf7J\x97\xe8\x8b\xb1\x10\x07\xfae\x81\xb2\xa29\xcf\xf7<\xff\x18\xfb\xc6\xf1Z\x8bY\xe2\x02\xac{\x92\xd0N\x14OYE\xf6\x0c^(_\xb0\xe8?E\xcf\xcf\xaf\x9bo\xfb\x84\xd3wZ\x95o\xac\x94\x84\x9e\xee\xbc\xc0J\x8fJ\x93\xf8D!\xe21EaPN\x10\xd8\xe35|L\x19\xb4\xde\x05\xbf\xa3\x06\x9f\xc8\xb5\xcd\xe4\x1f\xd7\x17\x06\rz\x95tU\rh\x1a\xfc\x10\x1bbd\x9dm\xe0\x95\xa0\xc3\x94\x07W\r\x14\xe6\xbd\x05\xfb\xb8\x9f\xe6\xdf\x8b\xe2\xc6\xe7~\x96\xf6S\xc5\x804P(X\xf0\x12Pq\x170\xba\xe6xc\xbc\xf4\xb2\xad\x9b+\xb2\xfe\xe19\x8c^\xba\x0b \x94\xde{\x83\xb8\xff\xe3V\x8d\xb7\x11\xe9;\x8c\xf2\xb1\xc1]\x9d\xa4\x0bL+\xd9\xb2\x18\xf5\xb5\x9fK\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Starfield Services Root Certificate Authority - G2 O=Starfield Technologies, Inc.
   * Subject: CN=Starfield Services Root Certificate Authority - G2 O=Starfield Technologies, Inc.
   * Label: "Starfield Services Root Certificate Authority - G2"
   * Serial: 0
   * SHA256 Fingerprint: 56:8d:69:05:a2:c8:87:08:a4:b3:02:51:90:ed:cf:ed:b1:97:4a:60:6a:13:c6:e5:29:0f:cb:2a:e6:3e:da:b5
   * -----BEGIN CERTIFICATE-----
   * MIID7zCCAtegAwIBAgIBADANBgkqhkiG9w0BAQsFADCBmDELMAkGA1UEBhMCVVMx
   * EDAOBgNVBAgTB0FyaXpvbmExEzARBgNVBAcTClNjb3R0c2RhbGUxJTAjBgNVBAoT
   * HFN0YXJmaWVsZCBUZWNobm9sb2dpZXMsIEluYy4xOzA5BgNVBAMTMlN0YXJmaWVs
   * ZCBTZXJ2aWNlcyBSb290IENlcnRpZmljYXRlIEF1dGhvcml0eSAtIEcyMB4XDTA5
   * MDkwMTAwMDAwMFoXDTM3MTIzMTIzNTk1OVowgZgxCzAJBgNVBAYTAlVTMRAwDgYD
   * VQQIEwdBcml6b25hMRMwEQYDVQQHEwpTY290dHNkYWxlMSUwIwYDVQQKExxTdGFy
   * ZmllbGQgVGVjaG5vbG9naWVzLCBJbmMuMTswOQYDVQQDEzJTdGFyZmllbGQgU2Vy
   * dmljZXMgUm9vdCBDZXJ0aWZpY2F0ZSBBdXRob3JpdHkgLSBHMjCCASIwDQYJKoZI
   * hvcNAQEBBQADggEPADCCAQoCggEBANUMOsQq+U7i9b4Zl1+OiFOxHz/Lz58gE20p
   * OsgPfTz3a3Y4Y9k2YKibXlwAgLIvWX/2h/klQ4bnaRtSmpDhcePYLQ1Ob/bISdm2
   * 8xpWriu2dBTrz/sm4xq6HZYuajtYlIlHVv8loJNwU4PahHQUw2eeBGg6345AWh1K
   * Ts9DkTvnVtYAcMtS7nt9rjrnvDH5RfbCYM8TWQIrgMw0R9+53pBlbQLPLJGmpufe
   * hRhJfGZOozptqbXuNC66DQO4M99H67FrjSXZm86B0UVGMpZwh94CDklDhbZsc7tk
   * 6mFBrMnUVN+HL8cisibMn1lUaJ/8viovxFUcdUBgF4UCVTmLfwUCAwEAAaNCMEAw
   * DwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFJxfAN+q
   * AdcwKziIorhtSpzyEZGDMA0GCSqGSIb3DQEBCwUAA4IBAQBLNqaEd2ndOxmfZyMI
   * bw5hyf2E3F/YNoHN2BtBLZ9g3ccaaNnRbobhiCPPE95Dz+I0swSdHynVv/heyNXB
   * ve6SbzJ08pGCL72CQnqtKrcgfU28elUSwhXqvfdqlS5sdJ/PHLTyxQGjhdByPq1z
   * qwubdQxtRbeOlKyWN7Wg0I8VRw7j6IPdj/3vQQF3zCepYoUz8jcI73HPdwbeyBkd
   * iEDPfUYd/x7H4c7/I9vG+o1VTqkC50cRRj70/b17KSa7qWFiNyi2LSr2EIZkyXCn
   * 0q23KXB56jzaYyWf/Wi3MOxw+3WKt21gZ7IeyLnp2KhvAotnDU0mV3HaIPzBSlCN
   * sSi6
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x100\x0e\x06\x03U\x04\x08\x13\x07Arizona1\x130\x11\x06\x03U\x04\x07\x13\nScottsdale1%0#\x06\x03U\x04\n\x13\x1cStarfield Technologies, Inc.1;09\x06\x03U\x04\x03\x132Starfield Services Root Certificate Authority - G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd5\x0c:\xc4*\xf9N\xe2\xf5\xbe\x19\x97_\x8e\x88S\xb1\x1f?\xcb\xcf\x9f \x13m):\xc8\x0f}<\xf7kv8c\xd96`\xa8\x9b^\\\x00\x80\xb2/Y\x7f\xf6\x87\xf9%C\x86\xe7i\x1bR\x9a\x90\xe1q\xe3\xd8-\rNo\xf6\xc8I\xd9\xb6\xf3\x1aV\xae+\xb6t\x14\xeb\xcf\xfb&\xe3\x1a\xba\x1d\x96.j;X\x94\x89GV\xff%\xa0\x93pS\x83\xda\x84t\x14\xc3g\x9e\x04h:\xdf\x8e@Z\x1dJN\xcfC\x91;\xe7V\xd6\x00p\xcbR\xee{}\xae:\xe7\xbc1\xf9E\xf6\xc2`\xcf\x13Y\x02+\x80\xcc4G\xdf\xb9\xde\x90em\x02\xcf,\x91\xa6\xa6\xe7\xde\x85\x18I|fN\xa3:m\xa9\xb5\xee4.\xba\r\x03\xb83\xdfG\xeb\xb1k\x8d%\xd9\x9b\xce\x81\xd1EF2\x96p\x87\xde\x02\x0eIC\x85\xb6ls\xbbd\xeaaA\xac\xc9\xd4T\xdf\x87/\xc7\"\xb2&\xcc\x9fYTh\x9f\xfc\xbe*/\xc4U\x1cu@`\x17\x85\x02U9\x8b\x7f\x05\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=BJCA Global Root CA2 O=BEIJING CERTIFICATE AUTHORITY
   * Subject: CN=BJCA Global Root CA2 O=BEIJING CERTIFICATE AUTHORITY
   * Label: "BJCA Global Root CA2"
   * Serial: 58605626836079930195615843123109055211
   * SHA256 Fingerprint: 57:4d:f6:93:1e:27:80:39:66:7b:72:0a:fd:c1:60:0f:c2:7e:b6:6d:d3:09:29:79:fb:73:85:64:87:21:28:82
   * -----BEGIN CERTIFICATE-----
   * MIICJTCCAaugAwIBAgIQLBcIfWQqwP6FGFkGz7RK6zAKBggqhkjOPQQDAzBUMQsw
   * CQYDVQQGEwJDTjEmMCQGA1UECgwdQkVJSklORyBDRVJUSUZJQ0FURSBBVVRIT1JJ
   * VFkxHTAbBgNVBAMMFEJKQ0EgR2xvYmFsIFJvb3QgQ0EyMB4XDTE5MTIxOTAzMTgy
   * MVoXDTQ0MTIxMjAzMTgyMVowVDELMAkGA1UEBhMCQ04xJjAkBgNVBAoMHUJFSUpJ
   * TkcgQ0VSVElGSUNBVEUgQVVUSE9SSVRZMR0wGwYDVQQDDBRCSkNBIEdsb2JhbCBS
   * b290IENBMjB2MBAGByqGSM49AgEGBSuBBAAiA2IABJ3LgJGNU2e1uVCxA/jlSR9B
   * IgmwUVJY1is0j8USRhTFiy8shP8sbqjV8QnjAyEUxEM9fMEsxEtqSs3ph+B99iK+
   * +kpRuDCK/eHeGBIK9ke35xe/J4rUQUyWPGCWwf0VHKNCMEAwHQYDVR0OBBYEFNJK
   * sVF/BvDRgh9Obl+rg/xI1LCRMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQD
   * AgEGMAoGCCqGSM49BAMDA2gAMGUCMBq8W9f+qdJUDkpd0m2xQNz0Q9XSSpkZElaA
   * 94M04TVOSG0ED1cxMDAtsaqdAzjbBgIxAMvMh1PLet8gUXOQwKhbYdDFUDn9hf7B
   * 43j4ptZLvZuHjw/l1lOWqzzIQNph91Oj9w==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1&0$\x06\x03U\x04\n\x0c\x1dBEIJING CERTIFICATE AUTHORITY1\x1d0\x1b\x06\x03U\x04\x03\x0c\x14BJCA Global Root CA2"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x9d\xcb\x80\x91\x8dSg\xb5\xb9P\xb1\x03\xf8\xe5I\x1fA\"\t\xb0QRX\xd6+4\x8f\xc5\x12F\x14\xc5\x8b/,\x84\xff,n\xa8\xd5\xf1\t\xe3\x03!\x14\xc4C=|\xc1,\xc4KjJ\xcd\xe9\x87\xe0}\xf6\"\xbe\xfaJQ\xb80\x8a\xfd\xe1\xde\x18\x12\n\xf6G\xb7\xe7\x17\xbf\'\x8a\xd4AL\x96<`\x96\xc1\xfd\x15\x1c"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Telekom Security TLS ECC Root 2020 O=Deutsche Telekom Security GmbH
   * Subject: CN=Telekom Security TLS ECC Root 2020 O=Deutsche Telekom Security GmbH
   * Label: "Telekom Security TLS ECC Root 2020"
   * Serial: 72082518505882327255703894282316633856
   * SHA256 Fingerprint: 57:8a:f4:de:d0:85:3f:4e:59:98:db:4a:ea:f9:cb:ea:8d:94:5f:60:b6:20:a3:8d:1a:3c:13:b2:bc:7b:a8:e1
   * -----BEGIN CERTIFICATE-----
   * MIICQjCCAcmgAwIBAgIQNjqWjMlcsljN0AFdxeVXADAKBggqhkjOPQQDAzBjMQsw
   * CQYDVQQGEwJERTEnMCUGA1UECgweRGV1dHNjaGUgVGVsZWtvbSBTZWN1cml0eSBH
   * bWJIMSswKQYDVQQDDCJUZWxla29tIFNlY3VyaXR5IFRMUyBFQ0MgUm9vdCAyMDIw
   * MB4XDTIwMDgyNTA3NDgyMFoXDTQ1MDgyNTIzNTk1OVowYzELMAkGA1UEBhMCREUx
   * JzAlBgNVBAoMHkRldXRzY2hlIFRlbGVrb20gU2VjdXJpdHkgR21iSDErMCkGA1UE
   * AwwiVGVsZWtvbSBTZWN1cml0eSBUTFMgRUNDIFJvb3QgMjAyMDB2MBAGByqGSM49
   * AgEGBSuBBAAiA2IABM6//leov9Wq9xCazbzREaK9Z0LMkOsVGJDZos0MKiXrPk/O
   * tdKPD/M12kOLAoC+b1EkHQ9rK8qfwm9QMuU3ILYg/4gND21Ju9sGpIeQkpT0CdDP
   * f8iAC8GXs7s1J8nCG6NCMEAwHQYDVR0OBBYEFONyzG6VmUex5rNhTNHLq+O6zd6f
   * MA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMAoGCCqGSM49BAMDA2cA
   * MGQCMHVSi7ekEE+uShCLsoRbQuHmKjYC2qBuGT8lv9pZMo7k+5Dck2TOrbRBR2Di
   * z6fLHgIwN0GMZt9Ba9aDAEH9L1r3ULRn0SyocddDypwnJJGDSA3PzfdUga/sf+Rn
   * 27iQ7t0l
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\'0%\x06\x03U\x04\n\x0c\x1eDeutsche Telekom Security GmbH1+0)\x06\x03U\x04\x03\x0c\"Telekom Security TLS ECC Root 2020"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xce\xbf\xfeW\xa8\xbf\xd5\xaa\xf7\x10\x9a\xcd\xbc\xd1\x11\xa2\xbdgB\xcc\x90\xeb\x15\x18\x90\xd9\xa2\xcd\x0c*%\xeb>O\xce\xb5\xd2\x8f\x0f\xf35\xdaC\x8b\x02\x80\xbeoQ$\x1d\x0fk+\xca\x9f\xc2oP2\xe57 \xb6 \xff\x88\r\x0fmI\xbb\xdb\x06\xa4\x87\x90\x92\x94\xf4\t\xd0\xcf\x7f\xc8\x80\x0b\xc1\x97\xb3\xbb5\'\xc9\xc2\x1b"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Autoridad de Certificacion Firmaprofesional CIF A62634068
   * Subject: CN=Autoridad de Certificacion Firmaprofesional CIF A62634068
   * Label: "Autoridad de Certificacion Firmaprofesional CIF A62634068"
   * Serial: 1977337328857672817
   * SHA256 Fingerprint: 57:de:05:83:ef:d2:b2:6e:03:61:da:99:da:9d:f4:64:8d:ef:7e:e8:44:1c:3b:72:8a:fa:9b:cd:e0:f9:b2:6a
   * -----BEGIN CERTIFICATE-----
   * MIIGFDCCA/ygAwIBAgIIG3Dp0v+ubHEwDQYJKoZIhvcNAQELBQAwUTELMAkGA1UE
   * BhMCRVMxQjBABgNVBAMMOUF1dG9yaWRhZCBkZSBDZXJ0aWZpY2FjaW9uIEZpcm1h
   * cHJvZmVzaW9uYWwgQ0lGIEE2MjYzNDA2ODAeFw0xNDA5MjMxNTIyMDdaFw0zNjA1
   * MDUxNTIyMDdaMFExCzAJBgNVBAYTAkVTMUIwQAYDVQQDDDlBdXRvcmlkYWQgZGUg
   * Q2VydGlmaWNhY2lvbiBGaXJtYXByb2Zlc2lvbmFsIENJRiBBNjI2MzQwNjgwggIi
   * MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDKlmuO6vj78aI14H9M2uDDUtd9
   * thDIAl6zQyrET2qyyhxdKJp4ERppWVevtSBC5IsP5t9bpgOSL/UR5GLXMnE42QQM
   * cas9UX4PB99jBVzpv5RvwSmCwLTaUbDBPLutN0pcyvFLNg4kq7/DhHf9qFD0sefG
   * L9ItWY16Ck6WaVICqjaY7Pz6FIMMNx/Jkjd/14Et5cS54D40/mf0PmbR0/RAz15i
   * NA9wBj4gGFrO93IbJWyTdBSTo3OxDqqHECNZXyAFGUftaI6SEspd/NYrspI8IM/h
   * X68gvqB2f3bl7BqGYTM+53u0P6APjqK5am+5hyZvQWyIplD9amML9ZMWGxmPsu2b
   * m8mQ9QEM3xk9Dz44I8kvjwzRAv4bVdZO0I08r0+k8/6vKtMFnXkIoctXMbScyJCy
   * Z/QYFpM6/EfY0XiWMR+6KwxfXZmtY4laJCB22N/9q06mIqqdXuYnin1oKaPnirja
   * EbsXLZmdEyRG98Xi2J+Of8ePdG1asuhy9azuJBCtLxTa/y2aRnFHvkLfuwHb9H/T
   * KI8xWVvTyQKmtFLKbpf7Q8UIJm+K9Lv9nyiqDdVF8xM6HdjAeI9BZzwelGSuewvF
   * 6NkBiDkal4ZkQdU7hwxu+g/GvUgUvzlN1J5Bto+WHWOWk9mVBngxaJ43BjuAiUVh
   * OSPHG0SjFeUc+JIwuwIDAQABo4HvMIHsMB0GA1UdDgQWBBRlzeurNR4APn7VdMAc
   * tHNHDhpkLzASBgNVHRMBAf8ECDAGAQH/AgEBMIGmBgNVHSAEgZ4wgZswgZgGBFUd
   * IAAwgY8wLwYIKwYBBQUHAgEWI2h0dHA6Ly93d3cuZmlybWFwcm9mZXNpb25hbC5j
   * b20vY3BzMFwGCCsGAQUFBwICMFAeTgBQAGEAcwBlAG8AIABkAGUAIABsAGEAIABC
   * AG8AbgBhAG4AbwB2AGEAIAA0ADcAIABCAGEAcgBjAGUAbABvAG4AYQAgADAAOAAw
   * ADEANzAOBgNVHQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQELBQADggIBAHSHKAIrdx9m
   * iWTtj3QuRhy7qPj4Cx2Dtjqn6EWKB7fgPiDL4QjbEwj4KKE1soCzC1HA01aajTNF
   * Sa9J8OA9B3pFE1r/yJfY0xgsfZb43aJlQ3CTkBW6kN/oGbDbLIpgD7dvlAceHabJ
   * hfa9NPhAeGIQcDq+fUs5gakQ1JZBu/hfHAsdCPKxsIl68veg4MSPi3i1O1ilI45P
   * Vf42O+AMt8oqMEEgtIDNrvx2ZnOorm7hfNoD6JQg5iKj0B+QXSBTFCZX2lSX3xZE
   * EAEeiGaPcjiT3SC3NL7X8e5jjkd5KAb881lFJWAiMxujX6i6KtoaPc1A6ozuBRWV
   * 1aUsIC+nmCjuRfzxuIgALI9C2lHVnOUTaHFFQ4ueCyE8S1wF3BqfmI7avSKecs2t
   * CsvMo2ebKHTEm9caPARYpoKdrcd7b/+Alun4jWq9GJAd/0kakFI3ky88Al2CdgtR
   * 5xbHV/g4+afNmyJU72OwFW1TZQNKXkqgsqeOSQBZONXH9IBk9W6VULgRfhVwOEqw
   * f9DEMnDAGf/JOC0ULGb0QkTmVXYbgBVX/8Cnp6o5qtjTcNAuuuuUavpfNIbnYrX9
   * ivAwhZTJryQCL2/W3Wf+47BVTwSYT6RBVuKT0Gro1vP7ZeDOdcQxWQzugsgMYDNK
   * GbqEZycPvEJdvSRUDewdcAZfpLz6IHxV
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02ES1B0@\x06\x03U\x04\x03\x0c9Autoridad de Certificacion Firmaprofesional CIF A62634068"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xca\x96k\x8e\xea\xf8\xfb\xf1\xa25\xe0\x7fL\xda\xe0\xc3R\xd7}\xb6\x10\xc8\x02^\xb3C*\xc4Oj\xb2\xca\x1c](\x9ax\x11\x1aiYW\xaf\xb5 B\xe4\x8b\x0f\xe6\xdf[\xa6\x03\x92/\xf5\x11\xe4b\xd72q8\xd9\x04\x0cq\xab=Q~\x0f\x07\xdfc\x05\\\xe9\xbf\x94o\xc1)\x82\xc0\xb4\xdaQ\xb0\xc1<\xbb\xad7J\\\xca\xf1K6\x0e$\xab\xbf\xc3\x84w\xfd\xa8P\xf4\xb1\xe7\xc6/\xd2-Y\x8dz\nN\x96iR\x02\xaa6\x98\xec\xfc\xfa\x14\x83\x0c7\x1f\xc9\x927\x7f\xd7\x81-\xe5\xc4\xb9\xe0>4\xfeg\xf4>f\xd1\xd3\xf4@\xcf^b4\x0fp\x06> \x18Z\xce\xf7r\x1b%l\x93t\x14\x93\xa3s\xb1\x0e\xaa\x87\x10#Y_ \x05\x19G\xedh\x8e\x92\x12\xca]\xfc\xd6+\xb2\x92< \xcf\xe1_\xaf \xbe\xa0v\x7fv\xe5\xec\x1a\x86a3>\xe7{\xb4?\xa0\x0f\x8e\xa2\xb9jo\xb9\x87&oAl\x88\xa6P\xfdjc\x0b\xf5\x93\x16\x1b\x19\x8f\xb2\xed\x9b\x9b\xc9\x90\xf5\x01\x0c\xdf\x19=\x0f>8#\xc9/\x8f\x0c\xd1\x02\xfe\x1bU\xd6N\xd0\x8d<\xafO\xa4\xf3\xfe\xaf*\xd3\x05\x9dy\x08\xa1\xcbW1\xb4\x9c\xc8\x90\xb2g\xf4\x18\x16\x93:\xfcG\xd8\xd1x\x961\x1f\xba+\x0c_]\x99\xadc\x89Z$ v\xd8\xdf\xfd\xabN\xa6\"\xaa\x9d^\xe6\'\x8a}h)\xa3\xe7\x8a\xb8\xda\x11\xbb\x17-\x99\x9d\x13$F\xf7\xc5\xe2\xd8\x9f\x8e\x7f\xc7\x8ftmZ\xb2\xe8r\xf5\xac\xee$\x10\xad/\x14\xda\xff-\x9aFqG\xbeB\xdf\xbb\x01\xdb\xf4\x7f\xd3(\x8f1Y[\xd3\xc9\x02\xa6\xb4R\xcan\x97\xfbC\xc5\x08&o\x8a\xf4\xbb\xfd\x9f(\xaa\r\xd5E\xf3\x13:\x1d\xd8\xc0x\x8fAg<\x1e\x94d\xae{\x0b\xc5\xe8\xd9\x01\x889\x1a\x97\x86dA\xd5;\x87\x0cn\xfa\x0f\xc6\xbdH\x14\xbf9M\xd4\x9eA\xb6\x8f\x96\x1dc\x96\x93\xd9\x95\x06x1h\x9e7\x06;\x80\x89Ea9#\xc7\x1bD\xa3\x15\xe5\x1c\xf8\x920\xbb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TWCA Global Root CA O=TAIWAN-CA OU=Root CA
   * Subject: CN=TWCA Global Root CA O=TAIWAN-CA OU=Root CA
   * Label: "TWCA Global Root CA"
   * Serial: 3262
   * SHA256 Fingerprint: 59:76:90:07:f7:68:5d:0f:cd:50:87:2f:9f:95:d5:75:5a:5b:2b:45:7d:81:f3:69:2b:61:0a:98:67:2f:0e:1b
   * -----BEGIN CERTIFICATE-----
   * MIIFQTCCAymgAwIBAgICDL4wDQYJKoZIhvcNAQELBQAwUTELMAkGA1UEBhMCVFcx
   * EjAQBgNVBAoTCVRBSVdBTi1DQTEQMA4GA1UECxMHUm9vdCBDQTEcMBoGA1UEAxMT
   * VFdDQSBHbG9iYWwgUm9vdCBDQTAeFw0xMjA2MjcwNjI4MzNaFw0zMDEyMzExNTU5
   * NTlaMFExCzAJBgNVBAYTAlRXMRIwEAYDVQQKEwlUQUlXQU4tQ0ExEDAOBgNVBAsT
   * B1Jvb3QgQ0ExHDAaBgNVBAMTE1RXQ0EgR2xvYmFsIFJvb3QgQ0EwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQCwBdvI64zEbooh745NnHEKH1Jw7W2CnJfF
   * 10xORUnLQEK1EjRsGcJ0pDFfhQKX7EMzClPSnIyOt7h52yvVavKOZsTuKwEHktSz
   * 0ALfUPZVr2YOy+BHYC8rMjk1Ujoog/h7FsYYuGLWRyWRzvAZEk2tY/XTP3VfKfCh
   * MBwqoJimFb3u/Rk28OKRQ4/6ytYQJ0lM793B8YVwm8rqqFpD/G2Gb3PpN0Wp8DbH
   * zIh1HrtsBv+baz4X7GGqcXzGHaL3SekVtTzWoWH1EfcFbx39Eb7QMAfCKbAJTibc
   * 46KokWofwpFFiFzlmLhxpRUZyXx1EcxwdE8tmx2RRP1WKKD+u4ZqyPpcC1jcxkt2
   * yKsi2XMPpfRaAok/T54igu6idFMqPVMnaR1sjjIsZAAmY2E2TqNGtz99sy2sbZCi
   * laLOz9qC5wc0GZbpuCGqKX6mOL6OKUohZnkfs8O1CWfe1tQHRvMq2uYiN2DLgbYP
   * oA/pyJV/v1WRBXrPPRXAb94JlAGD1zQbzECl8LibZ9WYkTunhHiVJqRaCPgrdLQA
   * BDzfuBSO6N+pjWxnkjMdwLfS7JLIvgm/LCkFbwJrnu+8vyq8W8BQj0FwcYeyTbcE
   * qYSjMq+u7msXi7Kx/mzhkIyIqJdIzshNy/MGz19qCkKxHh53L46g5pIOBvwFItIm
   * 4TFRfTLcDwIDAQABoyMwITAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB
   * /zANBgkqhkiG9w0BAQsFAAOCAgEAXzSBdu+WHdXltdkCY4QWwa6gcFGn90xHNcgL
   * 1yg9iXHZqjNB6hQbbCEAwGxCGX6faVsgQt+i0trEfJdLjbDorMjupWkEmQqSpqsn
   * LhpNgb+E1HAerUf+/UqdM+DyucRFCCEK2mlpc3INvjT+lIutwx4116KD7+U4x6WF
   * H6vPNOw/KP4M8VeGTslV9xzU2KV9Bnpv1d8Q34FOIWWxtuEXeZVFBs5fzNxGiWNo
   * RI2T9GRwoD2dKAXDOXC4Ynsg/eTb6QihuJ49CcdP+yz4k3ZB3lLg4VfSnQO8d57+
   * nile98FRYB/e2guyLXW3Q0iT5/Z5xoRdgFlglPx4mI88k1HtQJAH32RjJMtOcQWh
   * 15QaiDLxInQirqWm2BJpTGCjAu4r7NRjkgtevi92a6O2JryPA9gK8kxkRr05YuWW
   * 6zRjESjMlfGt7+/cgFhI6Uu46mWs6fyAtbXIRfmswZ/ZuepiiI7E8UuDEq3mi4TW
   * nsLrgxifarsbJGAzcMzs9zLzXNl5fe+epP7JI8Mk7hWSsT2RTyaGvWZzJBPqpK5j
   * wa19hAM8EHiGG3njxPPyBJUgriOCxLM6AGK/5jYk4Ve6xx6QddVfP5VhK8E7zeWz
   * aGHQRiapIVJpLesux+t3zqY6tQMzT3bR51xUAV3LePTJDL/PEo4XLSNolOer/qmy
   * KwbQBM0=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TW1\x120\x10\x06\x03U\x04\n\x13\tTAIWAN-CA1\x100\x0e\x06\x03U\x04\x0b\x13\x07Root CA1\x1c0\x1a\x06\x03U\x04\x03\x13\x13TWCA Global Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb0\x05\xdb\xc8\xeb\x8c\xc4n\x8a!\xef\x8eM\x9cq\n\x1fRp\xedm\x82\x9c\x97\xc5\xd7LNEI\xcb@B\xb5\x124l\x19\xc2t\xa41_\x85\x02\x97\xecC3\nS\xd2\x9c\x8c\x8e\xb7\xb8y\xdb+\xd5j\xf2\x8ef\xc4\xee+\x01\x07\x92\xd4\xb3\xd0\x02\xdfP\xf6U\xaff\x0e\xcb\xe0G`/+295R:(\x83\xf8{\x16\xc6\x18\xb8b\xd6G%\x91\xce\xf0\x19\x12M\xadc\xf5\xd3?u_)\xf0\xa10\x1c*\xa0\x98\xa6\x15\xbd\xee\xfd\x196\xf0\xe2\x91C\x8f\xfa\xca\xd6\x10\'IL\xef\xdd\xc1\xf1\x85p\x9b\xca\xea\xa8ZC\xfcm\x86os\xe97E\xa9\xf06\xc7\xcc\x88u\x1e\xbbl\x06\xff\x9bk>\x17\xeca\xaaq|\xc6\x1d\xa2\xf7I\xe9\x15\xb5<\xd6\xa1a\xf5\x11\xf7\x05o\x1d\xfd\x11\xbe\xd00\x07\xc2)\xb0\tN&\xdc\xe3\xa2\xa8\x91j\x1f\xc2\x91E\x88\\\xe5\x98\xb8q\xa5\x15\x19\xc9|u\x11\xccptO-\x9b\x1d\x91D\xfdV(\xa0\xfe\xbb\x86j\xc8\xfa\\\x0bX\xdc\xc6Kv\xc8\xab\"\xd9s\x0f\xa5\xf4Z\x02\x89?O\x9e\"\x82\xee\xa2tS*=S\'i\x1dl\x8e2,d\x00&ca6N\xa3F\xb7?}\xb3-\xacm\x90\xa2\x95\xa2\xce\xcf\xda\x82\xe7\x074\x19\x96\xe9\xb8!\xaa)~\xa68\xbe\x8e)J!fy\x1f\xb3\xc3\xb5\tg\xde\xd6\xd4\x07F\xf3*\xda\xe6\"7`\xcb\x81\xb6\x0f\xa0\x0f\xe9\xc8\x95\x7f\xbfU\x91\x05z\xcf=\x15\xc0o\xde\t\x94\x01\x83\xd74\x1b\xcc@\xa5\xf0\xb8\x9bg\xd5\x98\x91;\xa7\x84x\x95&\xa4Z\x08\xf8+t\xb4\x00\x04<\xdf\xb8\x14\x8e\xe8\xdf\xa9\x8dlg\x923\x1d\xc0\xb7\xd2\xec\x92\xc8\xbe\t\xbf,)\x05o\x02k\x9e\xef\xbc\xbf*\xbc[\xc0P\x8fApq\x87\xb2M\xb7\x04\xa9\x84\xa32\xaf\xae\xeek\x17\x8b\xb2\xb1\xfel\xe1\x90\x8c\x88\xa8\x97H\xce\xc8M\xcb\xf3\x06\xcf_j\nB\xb1\x1e\x1ew/\x8e\xa0\xe6\x92\x0e\x06\xfc\x05\"\xd2&\xe11Q}2\xdc\x0f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Hongkong Post Root CA 3 O=Hongkong Post
   * Subject: CN=Hongkong Post Root CA 3 O=Hongkong Post
   * Label: "Hongkong Post Root CA 3"
   * Serial: 46170865288971385588281144162979347873371282084
   * SHA256 Fingerprint: 5a:2f:c0:3f:0c:83:b0:90:bb:fa:40:60:4b:09:88:44:6c:76:36:18:3d:f9:84:6e:17:10:1a:44:7f:b8:ef:d6
   * -----BEGIN CERTIFICATE-----
   * MIIFzzCCA7egAwIBAgIUCBZfikyl7ADJk0DfxMauI7gcWqQwDQYJKoZIhvcNAQEL
   * BQAwbzELMAkGA1UEBhMCSEsxEjAQBgNVBAgTCUhvbmcgS29uZzESMBAGA1UEBxMJ
   * SG9uZyBLb25nMRYwFAYDVQQKEw1Ib25na29uZyBQb3N0MSAwHgYDVQQDExdIb25n
   * a29uZyBQb3N0IFJvb3QgQ0EgMzAeFw0xNzA2MDMwMjI5NDZaFw00MjA2MDMwMjI5
   * NDZaMG8xCzAJBgNVBAYTAkhLMRIwEAYDVQQIEwlIb25nIEtvbmcxEjAQBgNVBAcT
   * CUhvbmcgS29uZzEWMBQGA1UEChMNSG9uZ2tvbmcgUG9zdDEgMB4GA1UEAxMXSG9u
   * Z2tvbmcgUG9zdCBSb290IENBIDMwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIK
   * AoICAQCziNfqzg8gTr7m1gNt7ln8wlffKWihgw4+aMdoWJwcYEuJQwy51BWy7sFO
   * dem1p+/l6TWZ5Mwc50tfjTMwIDNT2aa71T4Tjukfh0mtUC1Qyhi+AViiE3CWu4mI
   * VoBc+L0sPOFMV4i707mV78vH9toxdCim5lSJ9UExyuUmGs2C4HDaOym71QP1mbpV
   * 9WTRYA6ziUm4ii8F0oRFKHyPaFASePwLtVPLwpgchKOesL4jpNrcyCse2m5FHomY
   * 2vkALgbpDDtw1VAliJnLzXNg99X/NWfFobxeq81KuEXryGgeDQ0URhLj0mRiikKY
   * vLTGCAj4/ahMZJx2Ab0vqWwzD9g/KLg8aQFChn5pwckGyuV6RmXpwtZQQS4/t+Tt
   * bNe/JgERohYpSms0BpDsE9K2+2p20jzt8NYt3eEV7KObLyzJPivkaTv/ciWxNoZb
   * x39ri1UbSsUgYT2uy1DhCDq+sI9jQVMwCFk8mB13umOResoQUGC/8Ne8lYePl8X+
   * l2oBlKN8W4UdKjk60FSh0Tlxnf0h+bV78OLgAo9uliQlLKAeLKjEiafv7ZkGL7YK
   * TE/bosw3Gq9HhS2KX8Q0NEwA/RiTZxPRN+ZItIsGxVd7GYYKecsAyVKvQv83j+Gj
   * Hno9UKtjBucVtT+2RTeUN7F+8kjDf8V1/peNRY8apxpyKBpADwIDAQABo2MwYTAP
   * BgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjAfBgNVHSMEGDAWgBQXnc0e
   * i9Y5K3DTXNSguB+wAPzFYTAdBgNVHQ4EFgQUF53NHovWOStw01zUoLgfsAD8xWEw
   * DQYJKoZIhvcNAQELBQADggIBAFbVe27mIgHSQpsY1Q7XZiNc4/6gx5LS6ZStS6LG
   * 7BJ8dNVI0lkUmcDrudHr9EgwW62nV3OZqdPlt9EuWSRY3GguLmLYauRwCy0gUCCk
   * MpXRAJi70/33MvJJrsZ64Ee+bs7Lo3I6LWldy8joRTnU+kLBEUx3XZL7av9YROXr
   * gZ6voJmtvqkBZss4HTzfQx/0TW60uhdG/H39h4F5ag0zD/ov+BS5gLNdTaqX4fnk
   * GMX41TiMJjz98iji7lpJiCzfeT2OnpA8vUFKOt1b9pq0zj8lMH8yfaIDlNDceqFS
   * 3m6TjRgm/VWsvY+b0s+v54Ysyx8Jb6NvqYTUc79NoXQbTiNg8swOqn+knEwlqLJm
   * Ozj/2ZQw9nKEvmhVEA/GcywWaZMH/rFF7buiVWqw2rVKAiUnhde3t4ZEFolsgCs+
   * l6mc1X5VTMbeRRAc6uk7nwNT7u56AQIWeNTowr5GdogTPyK7SBIdUgC0An4hGh6c
   * JfTzPV4e0hz5sy229zdcxsshTrD3mUcYhcErulWuBurQB7Lcq9CClnXO0lD+mefP
   * L5/ndtFhKvshuzHQqp9HpLIiyhY6UFfEW0NnxWViA0kB60PZ2Pierc+xYw5F9KBa
   * LJstxabArahH9CdMOA0uG0k7UvToiIMrVCjU8jVStDKDYmlkDJGcn5fqdBb9HxEG
   * mpv0
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02HK1\x120\x10\x06\x03U\x04\x08\x13\tHong Kong1\x120\x10\x06\x03U\x04\x07\x13\tHong Kong1\x160\x14\x06\x03U\x04\n\x13\rHongkong Post1 0\x1e\x06\x03U\x04\x03\x13\x17Hongkong Post Root CA 3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb3\x88\xd7\xea\xce\x0f N\xbe\xe6\xd6\x03m\xeeY\xfc\xc2W\xdf)h\xa1\x83\x0e>h\xc7hX\x9c\x1c`K\x89C\x0c\xb9\xd4\x15\xb2\xee\xc1Nu\xe9\xb5\xa7\xef\xe5\xe95\x99\xe4\xcc\x1c\xe7K_\x8d30 3S\xd9\xa6\xbb\xd5>\x13\x8e\xe9\x1f\x87I\xadP-P\xca\x18\xbe\x01X\xa2\x13p\x96\xbb\x89\x88V\x80\\\xf8\xbd,<\xe1LW\x88\xbb\xd3\xb9\x95\xef\xcb\xc7\xf6\xda1t(\xa6\xe6T\x89\xf5A1\xca\xe5&\x1a\xcd\x82\xe0p\xda;)\xbb\xd5\x03\xf5\x99\xbaU\xf5d\xd1`\x0e\xb3\x89I\xb8\x8a/\x05\xd2\x84E(|\x8fhP\x12x\xfc\x0b\xb5S\xcb\xc2\x98\x1c\x84\xa3\x9e\xb0\xbe#\xa4\xda\xdc\xc8+\x1e\xdanE\x1e\x89\x98\xda\xf9\x00.\x06\xe9\x0c;p\xd5P%\x88\x99\xcb\xcds`\xf7\xd5\xff5g\xc5\xa1\xbc^\xab\xcdJ\xb8E\xeb\xc8h\x1e\r\r\x14F\x12\xe3\xd2db\x8aB\x98\xbc\xb4\xc6\x08\x08\xf8\xfd\xa8Ld\x9cv\x01\xbd/\xa9l3\x0f\xd8?(\xb8<i\x01B\x86~i\xc1\xc9\x06\xca\xe5zFe\xe9\xc2\xd6PA.?\xb7\xe4\xedl\xd7\xbf&\x01\x11\xa2\x16)Jk4\x06\x90\xec\x13\xd2\xb6\xfbjv\xd2<\xed\xf0\xd6-\xdd\xe1\x15\xec\xa3\x9b/,\xc9>+\xe4i;\xffr%\xb16\x86[\xc7\x7fk\x8bU\x1bJ\xc5 a=\xae\xcbP\xe1\x08:\xbe\xb0\x8fcAS0\x08Y<\x98\x1dw\xbac\x91z\xca\x10P`\xbf\xf0\xd7\xbc\x95\x87\x8f\x97\xc5\xfe\x97j\x01\x94\xa3|[\x85\x1d*9:\xd0T\xa1\xd19q\x9d\xfd!\xf9\xb5{\xf0\xe2\xe0\x02\x8fn\x96$%,\xa0\x1e,\xa8\xc4\x89\xa7\xef\xed\x99\x06/\xb6\nLO\xdb\xa2\xcc7\x1a\xafG\x85-\x8a_\xc444L\x00\xfd\x18\x93g\x13\xd17\xe6H\xb4\x8b\x06\xc5W{\x19\x86\ny\xcb\x00\xc9R\xafB\xff7\x8f\xe1\xa3\x1ez=P\xabc\x06\xe7\x15\xb5?\xb6E7\x947\xb1~\xf2H\xc3\x7f\xc5u\xfe\x97\x8dE\x8f\x1a\xa7\x1ar(\x1a@\x0f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certum Trusted Network CA O=Unizeto Technologies S.A. OU=Certum Certification Authority
   * Subject: CN=Certum Trusted Network CA O=Unizeto Technologies S.A. OU=Certum Certification Authority
   * Label: "Certum Trusted Network CA"
   * Serial: 279744
   * SHA256 Fingerprint: 5c:58:46:8d:55:f5:8e:49:7e:74:39:82:d2:b5:00:10:b6:d1:65:37:4a:cf:83:a7:d4:a3:2d:b7:68:c4:40:8e
   * -----BEGIN CERTIFICATE-----
   * MIIDuzCCAqOgAwIBAgIDBETAMA0GCSqGSIb3DQEBBQUAMH4xCzAJBgNVBAYTAlBM
   * MSIwIAYDVQQKExlVbml6ZXRvIFRlY2hub2xvZ2llcyBTLkEuMScwJQYDVQQLEx5D
   * ZXJ0dW0gQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxIjAgBgNVBAMTGUNlcnR1bSBU
   * cnVzdGVkIE5ldHdvcmsgQ0EwHhcNMDgxMDIyMTIwNzM3WhcNMjkxMjMxMTIwNzM3
   * WjB+MQswCQYDVQQGEwJQTDEiMCAGA1UEChMZVW5pemV0byBUZWNobm9sb2dpZXMg
   * Uy5BLjEnMCUGA1UECxMeQ2VydHVtIENlcnRpZmljYXRpb24gQXV0aG9yaXR5MSIw
   * IAYDVQQDExlDZXJ0dW0gVHJ1c3RlZCBOZXR3b3JrIENBMIIBIjANBgkqhkiG9w0B
   * AQEFAAOCAQ8AMIIBCgKCAQEA4/t9o3K6wvDJFIf1awFO4W5AB7ptJ11/91sts1rH
   * UV+rpDKmYYe2bg+G0jACl/jXaVehGDldamR5xgFZrDwxSjh80gTSSyjoIF87B6LM
   * TXPb865Px1bVWqeWifrzq2jUI4ZZJ88JJ7ysbnKDHDBy3+Ci6dLhdHUZvSqeexVU
   * BBvXQzmtVSjF4hq79MDkrjhJM8x2hZ85RdKknvISjFH4fOQtf/WsX+sWn7Et0brM
   * kUJ3TCXJkDhv2/DM+44el1k+1WBO5gUo7Ul5E0u6SNsv+XLTOcr+H9g0cvW0QM8x
   * AcPs3hEtF10fuFDRXhmnad4HMyjKUJX5p1TLVIZQRan5SQIDAQABo0IwQDAPBgNV
   * HRMBAf8EBTADAQH/MB0GA1UdDgQWBBQIds3LB/8k9sXN7buQvOKEN0Z19zAOBgNV
   * HQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQEFBQADggEBAKaorSLOAT2mo/9i0Eidi15y
   * sHhE49wcrwn9I0j6vSrEuVUEtRCjjSfeC4Jj0O7eDDd5QVsisrCaQVymcODU0HfL
   * I9MA4GxWL+FpDQ3Zqr8hgVDZBqWo/5U30Kr+4rP1mS1FhIrlQgnXdAIv94nYmem8
   * J9RHjboNRhx3zxSkHLmkMcScKHQDNP8zGSal6Q10tz6XxnboJ5ajZt3hrvJBW8qY
   * VoNzcOSGGtIxQbovvi0TWnZvTuhOgQ4/WwMioBK+ZlgRSssDxLQqKi2WF+A5VLxI
   * 03YnnZotBqbJ7DnSq9ufmgsnAjUpsUCV5/nonFWIGUbWtzT1fs45mtk48VH3Tyw=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02PL1\"0 \x06\x03U\x04\n\x13\x19Unizeto Technologies S.A.1\'0%\x06\x03U\x04\x0b\x13\x1eCertum Certification Authority1\"0 \x06\x03U\x04\x03\x13\x19Certum Trusted Network CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xe3\xfb}\xa3r\xba\xc2\xf0\xc9\x14\x87\xf5k\x01N\xe1n@\x07\xbam\']\x7f\xf7[-\xb3Z\xc7Q_\xab\xa42\xa6a\x87\xb6n\x0f\x86\xd20\x02\x97\xf8\xd7iW\xa1\x189]jdy\xc6\x01Y\xac<1J8|\xd2\x04\xd2K(\xe8 _;\x07\xa2\xccMs\xdb\xf3\xaeO\xc7V\xd5Z\xa7\x96\x89\xfa\xf3\xabh\xd4#\x86Y\'\xcf\t\'\xbc\xacnr\x83\x1c0r\xdf\xe0\xa2\xe9\xd2\xe1tu\x19\xbd*\x9e{\x15T\x04\x1b\xd7C9\xadU(\xc5\xe2\x1a\xbb\xf4\xc0\xe4\xae8I3\xccv\x85\x9f9E\xd2\xa4\x9e\xf2\x12\x8cQ\xf8|\xe4-\x7f\xf5\xac_\xeb\x16\x9f\xb1-\xd1\xba\xcc\x91BwL%\xc9\x908o\xdb\xf0\xcc\xfb\x8e\x1e\x97Y>\xd5`N\xe6\x05(\xedIy\x13K\xbaH\xdb/\xf9r\xd39\xca\xfe\x1f\xd84r\xf5\xb4@\xcf1\x01\xc3\xec\xde\x11-\x17]\x1f\xb8P\xd1^\x19\xa7i\xde\x073(\xcaP\x95\xf9\xa7T\xcbT\x86PE\xa9\xf9I\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CFCA EV ROOT O=China Financial Certification Authority
   * Subject: CN=CFCA EV ROOT O=China Financial Certification Authority
   * Label: "CFCA EV ROOT"
   * Serial: 407555286
   * SHA256 Fingerprint: 5c:c3:d7:8e:4e:1d:5e:45:54:7a:04:e6:87:3e:64:f9:0c:f9:53:6d:1c:cc:2e:f8:00:f3:55:c4:c5:fd:70:fd
   * -----BEGIN CERTIFICATE-----
   * MIIFjTCCA3WgAwIBAgIEGErM1jANBgkqhkiG9w0BAQsFADBWMQswCQYDVQQGEwJD
   * TjEwMC4GA1UECgwnQ2hpbmEgRmluYW5jaWFsIENlcnRpZmljYXRpb24gQXV0aG9y
   * aXR5MRUwEwYDVQQDDAxDRkNBIEVWIFJPT1QwHhcNMTIwODA4MDMwNzAxWhcNMjkx
   * MjMxMDMwNzAxWjBWMQswCQYDVQQGEwJDTjEwMC4GA1UECgwnQ2hpbmEgRmluYW5j
   * aWFsIENlcnRpZmljYXRpb24gQXV0aG9yaXR5MRUwEwYDVQQDDAxDRkNBIEVWIFJP
   * T1QwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDXXWvNED8fBVnVBU03
   * sQ7smCuOFR36k0sXgiFxEFLXUWRwFsJVaU2OFW2fvwwbwuCjZ9YMrM8irq93VCpL
   * TIpTUnrD7i7es3ElweldPe6hL6P3KjzJIx1qqx2hp/Hz7KDVRM8Vz3IvHWOX6Jn5
   * /ZOkVIBMUtRSqy5J35DNuF++P96hyk0g1CXohClTt7GIH//62pCfCqktQT+x8Rgp
   * 7hZZLDRJGqgG16iI0gNyejLi6mhNbiyWZXvKWfry4t3uMCz7zEasxGPrb382KzRz
   * EpR/38wmnvFyXVBlWY9ps4deMm/DGIq1lY+wejfeWkU7xzbh72fROdOXW3NiGUgt
   * hxwG+3SYIElz8AXSG7Ggo7cbcNOIabla1jj0Ytwli3i/+Oh+uFzJlU9fpy25IGvP
   * a931DfSCt/SyZi4QKPaXWnuWFo8BGS1sbn85WAZkgwGDg8NNkt0yxoekN+kWzqot
   * aK8KgWU6cMGbrU1tVMoqLUuFG7OA5nBFDWteNfB/O7ic5ARwiRIlk9oKmSJgamNg
   * TnYGmE69g60dWIolhdLHZR4tjsbftsbhf4oEIRUpdPA+nJCdDC7xij5aqgwJHsfV
   * PKPtl8MeNPo4+QgO48BdK4PRVmrJtqhUUy54Mmc9gn900PvhtgVguXDbjgv5E1hv
   * cWAQUhC5wUEJ73IfZzF4/5YFjQIDAQABo2MwYTAfBgNVHSMEGDAWgBTj/i39KNAL
   * tbq2osS/BqoFjJP7LzAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjAd
   * BgNVHQ4EFgQU4/4t/SjQC7W6tqLEvwaqBYyT+y8wDQYJKoZIhvcNAQELBQADggIB
   * ACXGumvrh8vegjmWPfBEp2uEcwPenStPuiB/vHiyz5ewG5zz13ku9Ui20vsXiObT
   * ej/tUxPQ4i9qecsAIyjmHjdXNYmEwnZPNDatZ8POQQaIxffu2Bq41gt/UP+TqhdL
   * jOztUmCypAbqTuv0axn96/Ua4CUqmtzHQTb3yHQFhDmVOdYLO6Qn+gjYXB74BGBS
   * ESgoA//vU2YApUo0FmZ8/Qmkrp5nGm9BC2sGE5uPhnEFtC+NiWYzKXZUmhH4J/qy
   * P5Hgzg0b8zAarb8iXRvTvyUFTeGSGn+ZnzxEk8rUQElsgIfXBDrDMlI1Dlb4pd19
   * xIsNER9Tyx6yF7Zod1rg1MvIB671Oi6ON7fQAUtDKXeMOZePglr4UeWJoBjnaH9d
   * Ci77o0cOPaYjesYBx4/IXr9tgFa+iiS6M+qf4TIRnvHST4D2G0CvOJ4RUHlzEhLN
   * 5mydLIhyPDCBBpEi6lmt2hkuIsKNuYyH4Ga8cyNfIWRjgEj1oDwYPZTISEEdQLpe
   * /v5WOaHIz16eGWRGENoXkbcFgKyLmZJ956LYBws2J+dIeWCKw9cTXPhyQN9Ky8+Z
   * AAoACxGV2lZFA4gKn2fQ1XmxqI1AbQ3CekD6819kR5LLU7m7Wc5P/dAVUwHY3+vZ
   * 5nbv0CO7O6l5s9UCKc2Jo5YPSjXnTkLAdc0Hz+Ys63su
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN100.\x06\x03U\x04\n\x0c\'China Financial Certification Authority1\x150\x13\x06\x03U\x04\x03\x0c\x0cCFCA EV ROOT"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd7]k\xcd\x10?\x1f\x05Y\xd5\x05M7\xb1\x0e\xec\x98+\x8e\x15\x1d\xfa\x93K\x17\x82!q\x10R\xd7Qdp\x16\xc2UiM\x8e\x15m\x9f\xbf\x0c\x1b\xc2\xe0\xa3g\xd6\x0c\xac\xcf\"\xae\xafwT*KL\x8aSRz\xc3\xee.\xde\xb3q%\xc1\xe9]=\xee\xa1/\xa3\xf7*<\xc9#\x1dj\xab\x1d\xa1\xa7\xf1\xf3\xec\xa0\xd5D\xcf\x15\xcfr/\x1dc\x97\xe8\x99\xf9\xfd\x93\xa4T\x80LR\xd4R\xab.I\xdf\x90\xcd\xb8_\xbe?\xde\xa1\xcaM \xd4%\xe8\x84)S\xb7\xb1\x88\x1f\xff\xfa\xda\x90\x9f\n\xa9-A?\xb1\xf1\x18)\xee\x16Y,4I\x1a\xa8\x06\xd7\xa8\x88\xd2\x03rz2\xe2\xeahMn,\x96e{\xcaY\xfa\xf2\xe2\xdd\xee0,\xfb\xccF\xac\xc4c\xebo\x7f6+4s\x12\x94\x7f\xdf\xcc&\x9e\xf1r]PeY\x8fi\xb3\x87^2o\xc3\x18\x8a\xb5\x95\x8f\xb0z7\xdeZE;\xc76\xe1\xefg\xd19\xd3\x97[sb\x19H-\x87\x1c\x06\xfbt\x98 Is\xf0\x05\xd2\x1b\xb1\xa0\xa3\xb7\x1bp\xd3\x88i\xb9Z\xd68\xf4b\xdc%\x8bx\xbf\xf8\xe8~\xb8\\\xc9\x95O_\xa7-\xb9 k\xcfk\xdd\xf5\r\xf4\x82\xb7\xf4\xb2f.\x10(\xf6\x97Z{\x96\x16\x8f\x01\x19-ln\x7f9X\x06d\x83\x01\x83\x83\xc3M\x92\xdd2\xc6\x87\xa47\xe9\x16\xce\xaa-h\xaf\n\x81e:p\xc1\x9b\xadMmT\xca*-K\x85\x1b\xb3\x80\xe6pE\rk^5\xf0\x7f;\xb8\x9c\xe4\x04p\x89\x12%\x93\xda\n\x99\"`jc`Nv\x06\x98N\xbd\x83\xad\x1dX\x8a%\x85\xd2\xc7e\x1e-\x8e\xc6\xdf\xb6\xc6\xe1\x7f\x8a\x04!\x15)t\xf0>\x9c\x90\x9d\x0c.\xf1\x8a>Z\xaa\x0c\t\x1e\xc7\xd5<\xa3\xed\x97\xc3\x1e4\xfa8\xf9\x08\x0e\xe3\xc0]+\x83\xd1Vj\xc9\xb6\xa8TS.x2g=\x82\x7ft\xd0\xfb\xe1\xb6\x05`\xb9p\xdb\x8e\x0b\xf9\x13Xoq`\x10R\x10\xb9\xc1A\t\xefr\x1fg1x\xff\x96\x05\x8d\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=IdenTrust Commercial Root CA 1 O=IdenTrust
   * Subject: CN=IdenTrust Commercial Root CA 1 O=IdenTrust
   * Label: "IdenTrust Commercial Root CA 1"
   * Serial: 13298821034946342390520003877796839426
   * SHA256 Fingerprint: 5d:56:49:9b:e4:d2:e0:8b:cf:ca:d0:8a:3e:38:72:3d:50:50:3b:de:70:69:48:e4:2f:55:60:30:19:e5:28:ae
   * -----BEGIN CERTIFICATE-----
   * MIIFYDCCA0igAwIBAgIQCgFCgAAAAUUjyES1AAAAAjANBgkqhkiG9w0BAQsFADBK
   * MQswCQYDVQQGEwJVUzESMBAGA1UEChMJSWRlblRydXN0MScwJQYDVQQDEx5JZGVu
   * VHJ1c3QgQ29tbWVyY2lhbCBSb290IENBIDEwHhcNMTQwMTE2MTgxMjIzWhcNMzQw
   * MTE2MTgxMjIzWjBKMQswCQYDVQQGEwJVUzESMBAGA1UEChMJSWRlblRydXN0MScw
   * JQYDVQQDEx5JZGVuVHJ1c3QgQ29tbWVyY2lhbCBSb290IENBIDEwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQCnUBneP5k91DNG8W9RYYKyqU+PZ4ldhNlT
   * 3Qwo2dfw/66VQ3KZ+bVdfIrBQuExUHTRgQ18zZshq0PirK1ehm7zCYofWjK9ouuU
   * +ehcCuz/mNKvcbO0U59Oh++SvL3sTzIwiEsXXlfEU8L2ApeN2WIrvyQfYo3fw7gp
   * S0l4PJNgiCL8mdo2yMKi1CxUAGc1bnO/AljwpN3lsKImesrgNqUZFvX9t++uP0D1
   * bVoE/c40yiTcdCMbXTMTEl3EASX2MN0CXZ/g1Ue9tOsbobtJSdifWwLziuQkkORi
   * T0/Br4sOdBeo0XKIanoBScy0RnnGF7HamB4HWfp1IYVl3ZBWzvurpWCdxJ35UrCL
   * vYf5jysjCiN2O/cz4ckA82n5S6LgTrx+kzmEB/dEcH7+B1rlsazRGMzyNeVJSQjK
   * Vsk9+w8YfYs7wRPCTY/JTw436R+hDmrfYi7LNQZReSzIJTj0+kuniVyc0uMNOYZK
   * dHzVWYfCP04MXFL0PfdSgvHqo6z9STQaKPNBiDoT7uje/5kdX7rL6B7yuVBgwDHT
   * c+XvvqDtMwt0viAgxGds8AgDelWAf0ZOlqf0Hj7h9tgJ4TNkK2PXMl6f+cB7D3hv
   * l7yTmvmcEpB4eoCHFddydJxVdHixuuFucAS6T6C6aMN7/zHwcz09lCqxC0EOoP5N
   * iGVreTO01wIDAQABo0IwQDAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB
   * /zAdBgNVHQ4EFgQU7UQZwNPwBovupHu+QucmVMiONnYwDQYJKoZIhvcNAQELBQAD
   * ggIBAA2ukDL2pkt8RHYZYR4nKM1eVO8lvOMIkPkp165oCOGUAFjvLi5+U1KMtlwH
   * 6oi6mYtQlNeCgN9hCQCTrQ0U5s7B8jeUeLBfnLOic7iPBZM4zY0+sLj7wM+x8uwt
   * LRvM7Kqas6pgghstO8OEPVeKlh6cdbjTMM1gCIOQ045U8U1mwF10A0Cj7oV+wh93
   * nAbowacYXVKV7cndJZ5t+qntozo00Fl72u1Q8zW/7esUTTHHYPTa8Yec4kjixsU3
   * +wYQ+nVZZjFHKdp2mhzpgq7vmrlR94gjmmmVYjzlVYA211QC//G5Xc7UI2/YRYRK
   * W2XviQzdFKcgyxilJbQN+QHwotL0AMh0jqEqSI5l2xPE4iUXfeu+h1sXIFRRk0pT
   * AwvsXcoz7WL9RccvW9xYoIA55vrX/hMUpu09lEpCdNTDd1lzzY9GvlU47/rokTLq
   * l1gEIt44w8y8bckzOmoKaT+gyOpyj4xjhiO9bTyWnpXgSUyqorkqG5w2gXjtw+hG
   * 4iZZRHUe2XWJUc0QhJ1hYMtd+ZciTY6Y5uN/9lu7rs3KSoFrXgvzUeF0K+l+J6fZ
   * mUlO+KWA2yUPHGNiiskzZ2s8EIPGrd6ozRaOjfAHN3Gf8qv8QfXBi+wAN10J5U6A
   * 7/qxXDgGpRtK4dw4LTzcqx+QGtVKnO7RcGzM7vRX+Bi6hG6H
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x13\tIdenTrust1\'0%\x06\x03U\x04\x03\x13\x1eIdenTrust Commercial Root CA 1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa7P\x19\xde?\x99=\xd43F\xf1oQa\x82\xb2\xa9O\x8fg\x89]\x84\xd9S\xdd\x0c(\xd9\xd7\xf0\xff\xae\x95Cr\x99\xf9\xb5]|\x8a\xc1B\xe11Pt\xd1\x81\r|\xcd\x9b!\xabC\xe2\xac\xad^\x86n\xf3\t\x8a\x1fZ2\xbd\xa2\xeb\x94\xf9\xe8\\\n\xec\xff\x98\xd2\xafq\xb3\xb4S\x9fN\x87\xef\x92\xbc\xbd\xecO20\x88K\x17^W\xc4S\xc2\xf6\x02\x97\x8d\xd9b+\xbf$\x1fb\x8d\xdf\xc3\xb8)KIx<\x93`\x88\"\xfc\x99\xda6\xc8\xc2\xa2\xd4,T\x00g5ns\xbf\x02X\xf0\xa4\xdd\xe5\xb0\xa2&z\xca\xe06\xa5\x19\x16\xf5\xfd\xb7\xef\xae?@\xf5mZ\x04\xfd\xce4\xca$\xdct#\x1b]3\x13\x12]\xc4\x01%\xf60\xdd\x02]\x9f\xe0\xd5G\xbd\xb4\xeb\x1b\xa1\xbbII\xd8\x9f[\x02\xf3\x8a\xe4$\x90\xe4bOO\xc1\xaf\x8b\x0et\x17\xa8\xd1r\x88jz\x01I\xcc\xb4Fy\xc6\x17\xb1\xda\x98\x1e\x07Y\xfau!\x85e\xdd\x90V\xce\xfb\xab\xa5`\x9d\xc4\x9d\xf9R\xb0\x8b\xbd\x87\xf9\x8f+#\n#v;\xf73\xe1\xc9\x00\xf3i\xf9K\xa2\xe0N\xbc~\x939\x84\x07\xf7Dp~\xfe\x07Z\xe5\xb1\xac\xd1\x18\xcc\xf25\xe5II\x08\xcaV\xc9=\xfb\x0f\x18}\x8b;\xc1\x13\xc2M\x8f\xc9O\x0e7\xe9\x1f\xa1\x0ej\xdfb.\xcb5\x06Qy,\xc8%8\xf4\xfaK\xa7\x89\\\x9c\xd2\xe3\r9\x86Jt|\xd5Y\x87\xc2?N\x0c\\R\xf4=\xf7R\x82\xf1\xea\xa3\xac\xfdI4\x1a(\xf3A\x88:\x13\xee\xe8\xde\xff\x99\x1d_\xba\xcb\xe8\x1e\xf2\xb9P`\xc01\xd3s\xe5\xef\xbe\xa0\xed3\x0bt\xbe  \xc4gl\xf0\x08\x03zU\x80\x7fFN\x96\xa7\xf4\x1e>\xe1\xf6\xd8\t\xe13d+c\xd72^\x9f\xf9\xc0{\x0fxo\x97\xbc\x93\x9a\xf9\x9c\x12\x90xz\x80\x87\x15\xd7rt\x9cUtx\xb1\xba\xe1np\x04\xbaO\xa0\xbah\xc3{\xff1\xf0s==\x94*\xb1\x0bA\x0e\xa0\xfeM\x88eky3\xb4\xd7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SwissSign Gold CA - G2 O=SwissSign AG
   * Subject: CN=SwissSign Gold CA - G2 O=SwissSign AG
   * Label: "SwissSign Gold CA - G2"
   * Serial: 13492815561806991280
   * SHA256 Fingerprint: 62:dd:0b:e9:b9:f5:0a:16:3e:a0:f8:e7:5c:05:3b:1e:ca:57:ea:55:c8:68:8f:64:7c:68:81:f2:c8:35:7b:95
   * -----BEGIN CERTIFICATE-----
   * MIIFujCCA6KgAwIBAgIJALtAHEP1Xk+wMA0GCSqGSIb3DQEBBQUAMEUxCzAJBgNV
   * BAYTAkNIMRUwEwYDVQQKEwxTd2lzc1NpZ24gQUcxHzAdBgNVBAMTFlN3aXNzU2ln
   * biBHb2xkIENBIC0gRzIwHhcNMDYxMDI1MDgzMDM1WhcNMzYxMDI1MDgzMDM1WjBF
   * MQswCQYDVQQGEwJDSDEVMBMGA1UEChMMU3dpc3NTaWduIEFHMR8wHQYDVQQDExZT
   * d2lzc1NpZ24gR29sZCBDQSAtIEcyMIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIIC
   * CgKCAgEAr+TufoskDhJuqVAtFkQ7kpJcyrhdhJJCEyq8ZVeCQD5XJM1QiyUqt2/8
   * 76LQwB8CJEoTlo8jE+YoWACjR8cGp4QjK7u9lit/VcyLwVcfDmJlD909Vopz2q5+
   * bbqBHH5CjCA12UNNhPqE21Is8w4ndwtrvxEvcnifLtg+5hg3Wipy+dpikJKVyh+c
   * 6bM8K8vzARO/Ws/BtQpgvd21mWRTuKCWs2/iJneRjOBiEAKfNA+k1ZIzUd6+jbqE
   * emA8atufK+ze3gE/bk3lUIbLtK/tREDFylqM2tIrfKjuvqblCqoOpd8FUrdVxyJd
   * MmqXl2MT28nbeTZ7hTpKxVKJ+STnnXepgv9VHKVxaSvRAiTysybUa9oEVeXBCsdt
   * MDeQKuSeFDNeFhdVxVu1yzSJkvGdJo+hB9TGsnhQ2wwMC3wLjEHXuendjIj3o02y
   * MszYF9rNt85mndT9Xv+9lz4pded+p2JYryU0pUHHPbwNUMoDAw8IWh+Vc3hiv69y
   * FGkOpeUDDniOJihC8AcLYiAQZzlG+qkDzAQ4embvIIO1jEpWjpEA/I5cgt6IoMPi
   * aG59je883WX0XaxR7ySArqpWl2/5rX3aYT+YdzylkbYcjCbaZaIJbcHiVOO5ykxM
   * gI93e2CaHt+28kgeDrpOVG2Y4OGiGqJ3UM/EY5LsRxmd6+ZrzsECAwEAAaOBrDCB
   * qTAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUWyV7
   * lqRlUX64OfPAeGZe6Drn8O4wHwYDVR0jBBgwFoAUWyV7lqRlUX64OfPAeGZe6Drn
   * 8O4wRgYDVR0gBD8wPTA7BglghXQBWQECAQEwLjAsBggrBgEFBQcCARYgaHR0cDov
   * L3JlcG9zaXRvcnkuc3dpc3NzaWduLmNvbS8wDQYJKoZIhvcNAQEFBQADggIBACe6
   * 45R88a7A3hfm5djV9VSwg/S7zV4Fe0+fdWavPOhWfvxyeDgD2StiGwC5+OlgzczO
   * UYrHUDFu4Up+GC9pWbY9ZIEr44OE5iKHjn3g7gKZYbge9LgriBIWhMIxkziWMaa5
   * O1M/wySTVltpkuzFwbs4AOPsF6m43Md8AYOfMke6UiI0HTJ6CVanfCU2qT1L2sCC
   * bwq7EsiHSycR+R4tx5M/nttfJmtS2S6K8RTGRI0Vqbe/vd6mGu6uLftIdxf+u+yv
   * GPUqUfA5hJeVbG4bwyvEdGB5JbAKJ9/fXtI5z0V9QkvfsywexcZdylU6oJxpmo/a
   * 77KwPJ+HbBIrZXAVUjEaJM9vMSNQH4xPjyPDdEFjHFWoFN0+4FFQz/EbMFYOkrCC
   * hdiDyyJkvC24JdVUorgG6q2SpCSgwYa1ShNqR88uC1aVVMvOmttqtKay20EIhid3
   * 92qgQmwLOM7XdVAyksLfKzAiSNDVQTglXaTpXZ/GlHXQRf0wl0OPkKsKx4ZzYEpp
   * Ld6leNcG2mqeSz53OiATIgHQv2ieY2BrNU0LbbqhPcCT4H8js1WtciVORvnSFu+w
   * ZMEBnunKoGqYDs/YYPIvSbjkQuE4NRb0yG5P94FW6LqjviOvrv1vA+ACOzB2+htt
   * Qc8Bsem4yWb02ybzOqR08kkkW8mw0FfB+j564ZfJ
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CH1\x150\x13\x06\x03U\x04\n\x13\x0cSwissSign AG1\x1f0\x1d\x06\x03U\x04\x03\x13\x16SwissSign Gold CA - G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xaf\xe4\xee~\x8b$\x0e\x12n\xa9P-\x16D;\x92\x92\\\xca\xb8]\x84\x92B\x13*\xbceW\x82@>W$\xcdP\x8b%*\xb7o\xfc\xef\xa2\xd0\xc0\x1f\x02$J\x13\x96\x8f#\x13\xe6(X\x00\xa3G\xc7\x06\xa7\x84#+\xbb\xbd\x96+\x7fU\xcc\x8b\xc1W\x1f\x0ebe\x0f\xdd=V\x8as\xda\xae~m\xba\x81\x1c~B\x8c 5\xd9CM\x84\xfa\x84\xdbR,\xf3\x0e\'w\x0bk\xbf\x11/rx\x9f.\xd8>\xe6\x187Z*r\xf9\xdab\x90\x92\x95\xca\x1f\x9c\xe9\xb3<+\xcb\xf3\x01\x13\xbfZ\xcf\xc1\xb5\n`\xbd\xdd\xb5\x99dS\xb8\xa0\x96\xb3o\xe2&w\x91\x8c\xe0b\x10\x02\x9f4\x0f\xa4\xd5\x923Q\xde\xbe\x8d\xba\x84z`<j\xdb\x9f+\xec\xde\xde\x01?nM\xe5P\x86\xcb\xb4\xaf\xedD@\xc5\xcaZ\x8c\xda\xd2+|\xa8\xee\xbe\xa6\xe5\n\xaa\x0e\xa5\xdf\x05R\xb7U\xc7\"]2j\x97\x97c\x13\xdb\xc9\xdby6{\x85:J\xc5R\x89\xf9$\xe7\x9dw\xa9\x82\xffU\x1c\xa5qi+\xd1\x02$\xf2\xb3&\xd4k\xda\x04U\xe5\xc1\n\xc7m07\x90*\xe4\x9e\x143^\x16\x17U\xc5[\xb5\xcb4\x89\x92\xf1\x9d&\x8f\xa1\x07\xd4\xc6\xb2xP\xdb\x0c\x0c\x0b|\x0b\x8cA\xd7\xb9\xe9\xdd\x8c\x88\xf7\xa3M\xb22\xcc\xd8\x17\xda\xcd\xb7\xcef\x9d\xd4\xfd^\xff\xbd\x97>)u\xe7~\xa7bX\xaf%4\xa5A\xc7=\xbc\rP\xca\x03\x03\x0f\x08Z\x1f\x95sxb\xbf\xafr\x14i\x0e\xa5\xe5\x03\x0ex\x8e&(B\xf0\x07\x0bb \x10g9F\xfa\xa9\x03\xcc\x048zf\xef \x83\xb5\x8cJV\x8e\x91\x00\xfc\x8e\\\x82\xde\x88\xa0\xc3\xe2hn}\x8d\xef<\xdde\xf4]\xacQ\xef$\x80\xae\xaaV\x97o\xf9\xad}\xdaa?\x98w<\xa5\x91\xb6\x1c\x8c&\xdae\xa2\tm\xc1\xe2T\xe3\xb9\xcaLL\x80\x8fw{`\x9a\x1e\xdf\xb6\xf2H\x1e\x0e\xbaNTm\x98\xe0\xe1\xa2\x1a\xa2wP\xcf\xc4c\x92\xecG\x19\x9d\xeb\xe6k\xce\xc1\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: O=CERTSIGN SA OU=certSIGN ROOT CA G2
   * Subject: O=CERTSIGN SA OU=certSIGN ROOT CA G2
   * Label: "certSIGN ROOT CA G2"
   * Serial: 313609486401300475190
   * SHA256 Fingerprint: 65:7c:fe:2f:a7:3f:aa:38:46:25:71:f3:32:a2:36:3a:46:fc:e7:02:09:51:71:07:02:cd:fb:b6:ee:da:33:05
   * -----BEGIN CERTIFICATE-----
   * MIIFRzCCAy+gAwIBAgIJEQA0tk7GNi02MA0GCSqGSIb3DQEBCwUAMEExCzAJBgNV
   * BAYTAlJPMRQwEgYDVQQKEwtDRVJUU0lHTiBTQTEcMBoGA1UECxMTY2VydFNJR04g
   * Uk9PVCBDQSBHMjAeFw0xNzAyMDYwOTI3MzVaFw00MjAyMDYwOTI3MzVaMEExCzAJ
   * BgNVBAYTAlJPMRQwEgYDVQQKEwtDRVJUU0lHTiBTQTEcMBoGA1UECxMTY2VydFNJ
   * R04gUk9PVCBDQSBHMjCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAMDF
   * dRmRfUR0dIf+DjuW3NgBFszuY5HnC2/OOwppGnzC46+CjobXXo9X69MhWf05N0Iw
   * vlDqtg+piNguLWkh59E3GE59kdUWX2tbAMI5Qw02hVK5U2UPHULlj88F0+7cDBrZ
   * uIt4ImfkabBoxTzkbFpG583H+u/E7Eu9aqSs/cwoUe+StCmrqzWaTOTECMYmzPhp
   * n+Sc8CnTXPnGFiWeI8MgwT0PPzhAsP6CRDiqWhqKa2NYOLQV07YRaXseVO6MGiKs
   * cpc/I1mbySKEwQdPzH/iV8oScLumZfNpdWO9lfsbl83kqK/20U6o2YpxJM02PbyW
   * xPFsqa7lzw1uKA2wDrXKUXt4FMMgL3/7FFXhEZn91QqhngLjYl/rNUssuHLoPj1P
   * rCy7Lobio3aP5ZMqz6WryFyNSwb/EkaseMsUBzXgqd+L6a8VTxaJW732jcZZroiF
   * DsGJ6x9nxUWO/203Nit4ZoORUSs9/1F3dmKh7Gc+PoGD4FapUB8fepmrY7+EF3fx
   * DTvf95xhszWYijqy7DwaNz9+j5LP2RIUZNoQAhVB/0/E6xyjyfqZ90bp4RjZsbgy
   * LcsUDFDYg2WD7rlcz8sFWkz6GZdr1l0T08JcVLwyc6B49fFtHsufpaafItzRUZ6C
   * eWRgKRM+o/1Pcmqr4tTluCRVLERLiohEnMqE0yo7AgMBAAGjQjBAMA8GA1UdEwEB
   * /wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBSCIS1mxteg4BXrzkwJ
   * d8RgnlRuAzANBgkqhkiG9w0BAQsFAAOCAgEAYN4auOfyYILVAzOBywaK8SJJ6ejq
   * kX/GM15oGQOGO0MBzwdw5AgeZYWR5hEit/UCI46uuR59H35s5r0l1ZUa8gWmr4UC
   * b6741jH/JclKyMeKqdmfS0mbEVeZkkMR3rYzpMzXjWR91M08KCy0mpbqTfXERMQl
   * qiCA2ClV9+BB/AYm/7k29UMUA2Z44RGx2iBfRgB4ACGlHgAoYXhvqAEBj500mv/0
   * OJD7uNGzcgbJceaBxXntC6Z58hMLnPddDnskk7RI24Zf3lCGeOdA5jGokHZwYa+c
   * NywRtYK3qq4kNFtyDGkNzVmf9nGvnAvRCjj5BiKDUyUM/FHE5r7iOZULJK2v0ZXk
   * ltd0ZGtxTgI8qoXzIKNDOXZbbFD+mpwUHmUUihW9o4JFWklWatKcsWMy5WHgUyIO
   * pwpJ6st+H6jiYoD2EEVSmAYY3qXNL3+q1Ok+CHLsIwMCPKaq2LxndD0UF/tUSxfj
   * 03k9bWtJySgOLnRQvwzZRjoQhsmnP+mg7H/rpXdYaXHmgwo38oZJar55CJD2AhZk
   * PuXaTH4MNMn5X7azKFGnpyuqSfqNZSlO42sTp5SjLVFteAxEy9/eCG/Oo2Sr05WE
   * 1LlSVHJ7liXMvGnjSG4N0MedJ5qq+BOS3R7fY581qRY27Iy4g/Q9iY/NtBde17MX
   * QRBdJ3NghVdJIgc=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02RO1\x140\x12\x06\x03U\x04\n\x13\x0bCERTSIGN SA1\x1c0\x1a\x06\x03U\x04\x0b\x13\x13certSIGN ROOT CA G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc0\xc5u\x19\x91}Dtt\x87\xfe\x0e;\x96\xdc\xd8\x01\x16\xcc\xeec\x91\xe7\x0bo\xce;\ni\x1a|\xc2\xe3\xaf\x82\x8e\x86\xd7^\x8fW\xeb\xd3!Y\xfd97B0\xbeP\xea\xb6\x0f\xa9\x88\xd8.-i!\xe7\xd17\x18N}\x91\xd5\x16_k[\x00\xc29C\r6\x85R\xb9Se\x0f\x1dB\xe5\x8f\xcf\x05\xd3\xee\xdc\x0c\x1a\xd9\xb8\x8bx\"g\xe4i\xb0h\xc5<\xe4lZF\xe7\xcd\xc7\xfa\xef\xc4\xecK\xbdj\xa4\xac\xfd\xcc(Q\xef\x92\xb4)\xab\xab5\x9aL\xe4\xc4\x08\xc6&\xcc\xf8i\x9f\xe4\x9c\xf0)\xd3\\\xf9\xc6\x16%\x9e#\xc3 \xc1=\x0f?8@\xb0\xfe\x82D8\xaaZ\x1a\x8akcX8\xb4\x15\xd3\xb6\x11i{\x1eT\xee\x8c\x1a\"\xacr\x97?#Y\x9b\xc9\"\x84\xc1\x07O\xcc\x7f\xe2W\xca\x12p\xbb\xa6e\xf3iuc\xbd\x95\xfb\x1b\x97\xcd\xe4\xa8\xaf\xf6\xd1N\xa8\xd9\x8aq$\xcd6=\xbc\x96\xc4\xf1l\xa9\xae\xe5\xcf\rn(\r\xb0\x0e\xb5\xcaQ{x\x14\xc3 /\x7f\xfb\x14U\xe1\x11\x99\xfd\xd5\n\xa1\x9e\x02\xe3b_\xeb5K,\xb8r\xe8>=O\xac,\xbb.\x86\xe2\xa3v\x8f\xe5\x93*\xcf\xa5\xab\xc8\\\x8dK\x06\xff\x12F\xacx\xcb\x14\x075\xe0\xa9\xdf\x8b\xe9\xaf\x15O\x16\x89[\xbd\xf6\x8d\xc6Y\xae\x88\x85\x0e\xc1\x89\xeb\x1fg\xc5E\x8e\xffm76+xf\x83\x91Q+=\xffQwvb\xa1\xecg>>\x81\x83\xe0V\xa9P\x1f\x1fz\x99\xabc\xbf\x84\x17w\xf1\r;\xdf\xf7\x9ca\xb35\x98\x8a:\xb2\xec<\x1a7?~\x8f\x92\xcf\xd9\x12\x14d\xda\x10\x02\x15A\xffO\xc4\xeb\x1c\xa3\xc9\xfa\x99\xf7F\xe9\xe1\x18\xd9\xb1\xb82-\xcb\x14\x0cP\xd8\x83e\x83\xee\xb9\\\xcf\xcb\x05ZL\xfa\x19\x97k\xd6]\x13\xd3\xc2\\T\xbc2s\xa0x\xf5\xf1m\x1e\xcb\x9f\xa5\xa6\x9f\"\xdc\xd1Q\x9e\x82yd`)\x13>\xa3\xfdOrj\xab\xe2\xd4\xe5\xb8$U,DK\x8a\x88D\x9c\xca\x84\xd3*;\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=ISRG Root X2 O=Internet Security Research Group
   * Subject: CN=ISRG Root X2 O=Internet Security Research Group
   * Label: "ISRG Root X2"
   * Serial: 87493402998870891108772069816698636114
   * SHA256 Fingerprint: 69:72:9b:8e:15:a8:6e:fc:17:7a:57:af:b7:17:1d:fc:64:ad:d2:8c:2f:ca:8c:f1:50:7e:34:45:3c:cb:14:70
   * -----BEGIN CERTIFICATE-----
   * MIICGzCCAaGgAwIBAgIQQdKd0XLq7qeAwSxs6S+HUjAKBggqhkjOPQQDAzBPMQsw
   * CQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJuZXQgU2VjdXJpdHkgUmVzZWFyY2gg
   * R3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBYMjAeFw0yMDA5MDQwMDAwMDBaFw00
   * MDA5MTcxNjAwMDBaME8xCzAJBgNVBAYTAlVTMSkwJwYDVQQKEyBJbnRlcm5ldCBT
   * ZWN1cml0eSBSZXNlYXJjaCBHcm91cDEVMBMGA1UEAxMMSVNSRyBSb290IFgyMHYw
   * EAYHKoZIzj0CAQYFK4EEACIDYgAEzZvVn4CDCuwJSvMWSj5cz3es3mcFDR0HttwW
   * +1qLFNvicWDEukWVEYmO6gbf9yoWHKS5xcUy4APgHoIYOIvXRdgKam7mAHf7AlF9
   * ItgKbppbd9/w+kHsOdx1ymgHDB/qo0IwQDAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0T
   * AQH/BAUwAwEB/zAdBgNVHQ4EFgQUfEKWrt5LSDv6kviejM9ti6lyN5UwCgYIKoZI
   * zj0EAwMDaAAwZQIwe3lORlCEwkSHRhtFcP9Ymd70/aTSVaYgLXTWNLxBo1BfASdW
   * tL4ndQavEi51mI38AjEAi/V3bNTIZargCyzuFJ0nN6T5U6VR5CmD1/iQMVtCnwr1
   * /q4AaOeMSQ+2b1tbFfLn
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1)0\'\x06\x03U\x04\n\x13 Internet Security Research Group1\x150\x13\x06\x03U\x04\x03\x13\x0cISRG Root X2"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xcd\x9b\xd5\x9f\x80\x83\n\xec\tJ\xf3\x16J>\\\xcfw\xac\xdeg\x05\r\x1d\x07\xb6\xdc\x16\xfbZ\x8b\x14\xdb\xe2q`\xc4\xbaE\x95\x11\x89\x8e\xea\x06\xdf\xf7*\x16\x1c\xa4\xb9\xc5\xc52\xe0\x03\xe0\x1e\x82\x188\x8b\xd7E\xd8\njn\xe6\x00w\xfb\x02Q}\"\xd8\nn\x9a[w\xdf\xf0\xfaA\xec9\xdcu\xcah\x07\x0c\x1f\xea"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certum EC-384 CA O=Asseco Data Systems S.A. OU=Certum Certification Authority
   * Subject: CN=Certum EC-384 CA O=Asseco Data Systems S.A. OU=Certum Certification Authority
   * Label: "Certum EC-384 CA"
   * Serial: 160250656287871593594747141429395092468
   * SHA256 Fingerprint: 6b:32:80:85:62:53:18:aa:50:d1:73:c9:8d:8b:da:09:d5:7e:27:41:3d:11:4c:f7:87:a0:f5:d0:6c:03:0c:f6
   * -----BEGIN CERTIFICATE-----
   * MIICZTCCAeugAwIBAgIQeI8nXIESUiClBNAt3bpz9DAKBggqhkjOPQQDAzB0MQsw
   * CQYDVQQGEwJQTDEhMB8GA1UEChMYQXNzZWNvIERhdGEgU3lzdGVtcyBTLkEuMScw
   * JQYDVQQLEx5DZXJ0dW0gQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxGTAXBgNVBAMT
   * EENlcnR1bSBFQy0zODQgQ0EwHhcNMTgwMzI2MDcyNDU0WhcNNDMwMzI2MDcyNDU0
   * WjB0MQswCQYDVQQGEwJQTDEhMB8GA1UEChMYQXNzZWNvIERhdGEgU3lzdGVtcyBT
   * LkEuMScwJQYDVQQLEx5DZXJ0dW0gQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxGTAX
   * BgNVBAMTEENlcnR1bSBFQy0zODQgQ0EwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAATE
   * KI6rGFtqvm5kN2PkzeyrOvfMobgOgknXhimfoZTy42B4mIF4Bk3y7JoOV2CDn7Tm
   * Fy8as10CW4kjPMIRBSqniBMY81CE1700LCeJVf/OTOffph8oxPBUw7l8t1Ot68Kj
   * QjBAMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFI0GZnQkdjrzife81r1HfS+8
   * EF9LMA4GA1UdDwEB/wQEAwIBBjAKBggqhkjOPQQDAwNoADBlAjADVS2m5hjEfO/J
   * UG7BJw+ch69u1RsIGL2SKcHvlJF40jocVYli5RsJHrpka/F2tNQCMQC0QoSZ/6vn
   * nvuRlydd3LBbMHHOXjgaatkl5+r3YZJW+OraNsKHZZYuciUvf9/DE8k=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02PL1!0\x1f\x06\x03U\x04\n\x13\x18Asseco Data Systems S.A.1\'0%\x06\x03U\x04\x0b\x13\x1eCertum Certification Authority1\x190\x17\x06\x03U\x04\x03\x13\x10Certum EC-384 CA"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xc4(\x8e\xab\x18[j\xbend7c\xe4\xcd\xec\xab:\xf7\xcc\xa1\xb8\x0e\x82I\xd7\x86)\x9f\xa1\x94\xf2\xe3`x\x98\x81x\x06M\xf2\xec\x9a\x0eW`\x83\x9f\xb4\xe6\x17/\x1a\xb3]\x02[\x89#<\xc2\x11\x05*\xa7\x88\x13\x18\xf3P\x84\xd7\xbd4,\'\x89U\xff\xceL\xe7\xdf\xa6\x1f(\xc4\xf0T\xc3\xb9|\xb7S\xad\xeb\xc2"),
    name_constraints: None
  },

  /*
   * Issuer: CN=OISTE WISeKey Global Root GB CA O=WISeKey OU=OISTE Foundation Endorsed
   * Subject: CN=OISTE WISeKey Global Root GB CA O=WISeKey OU=OISTE Foundation Endorsed
   * Label: "OISTE WISeKey Global Root GB CA"
   * Serial: 157768595616588414422159278966750757568
   * SHA256 Fingerprint: 6b:9c:08:e8:6e:b0:f7:67:cf:ad:65:cd:98:b6:21:49:e5:49:4a:67:f5:84:5e:7b:d1:ed:01:9f:27:b8:6b:d6
   * -----BEGIN CERTIFICATE-----
   * MIIDtTCCAp2gAwIBAgIQdrEgUnTwhYdGs/gjGvbCwDANBgkqhkiG9w0BAQsFADBt
   * MQswCQYDVQQGEwJDSDEQMA4GA1UEChMHV0lTZUtleTEiMCAGA1UECxMZT0lTVEUg
   * Rm91bmRhdGlvbiBFbmRvcnNlZDEoMCYGA1UEAxMfT0lTVEUgV0lTZUtleSBHbG9i
   * YWwgUm9vdCBHQiBDQTAeFw0xNDEyMDExNTAwMzJaFw0zOTEyMDExNTEwMzFaMG0x
   * CzAJBgNVBAYTAkNIMRAwDgYDVQQKEwdXSVNlS2V5MSIwIAYDVQQLExlPSVNURSBG
   * b3VuZGF0aW9uIEVuZG9yc2VkMSgwJgYDVQQDEx9PSVNURSBXSVNlS2V5IEdsb2Jh
   * bCBSb290IEdCIENBMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA2Be3
   * HEokKtaXscriHvt9OO+Y9bI5mE4nuBFde9IllIiCFSZqGzG7qFshISvYD06fWvGx
   * WuR51jIjK+FTzJlFXHtPrby/h0oLS5daqPZI7H17Dc0hBt+eFf1Biki3IPShehtX
   * 1F1Q/7pn2COZH8g/497/b1t3sWtuuMlk9+HKQUYOKXHQuSP8yYFfTvdv37+ErXNk
   * u7dCjmn21HYdfp2nuFeKUWdy19SouJVUQHMD9ur06/4oQnc/nSMbsrY9gBQHTC5P
   * 99UKFg29ZkM3fiNDecNAhvVMKdqOmq0NpQSHiB6F4+lT1ZvIiwNjeOvgGUpuuy9r
   * M2RYk61pv48b74JIxwIDAQABo1EwTzALBgNVHQ8EBAMCAYYwDwYDVR0TAQH/BAUw
   * AwEB/zAdBgNVHQ4EFgQUNQ/INmNe4qPs+TtmFc5RUuORmj0wEAYJKwYBBAGCNxUB
   * BAMCAQAwDQYJKoZIhvcNAQELBQADggEBAEBM+4eymYGQfp3FsLAmzYh7KzKNbrgh
   * cViXfa43FK8+5/ea4n32cZiZBKpDdHij40lhPnOMTZTg+XHEthYOU3gf1qKHLwI5
   * gSk8rxWYITD+KJAAjNHhy/peyP34EEY7onhCkRd0VQreUGdNZtGn//3ZwLWoo4rO
   * ZvUPQ82nK1d7Y0Zqqi5S2PTt4W2tKZB4SLrhI6qjiey1q5bAtEuiHZeeevJuQHHf
   * aPFlTc58Bd9TZaml8LGXBHAVRgOY1NK/VLSgWH1Sb9pWJmLU2NuJMW8c8CLC02Ic
   * Nc1MaRVUGpCY3useX8p3x8uOPUNpnJpY0CQ73xtAln41rYHHTnG6iBM=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CH1\x100\x0e\x06\x03U\x04\n\x13\x07WISeKey1\"0 \x06\x03U\x04\x0b\x13\x19OISTE Foundation Endorsed1(0&\x06\x03U\x04\x03\x13\x1fOISTE WISeKey Global Root GB CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd8\x17\xb7\x1cJ$*\xd6\x97\xb1\xca\xe2\x1e\xfb}8\xef\x98\xf5\xb29\x98N\'\xb8\x11]{\xd2%\x94\x88\x82\x15&j\x1b1\xbb\xa8[!!+\xd8\x0fN\x9fZ\xf1\xb1Z\xe4y\xd62#+\xe1S\xcc\x99E\\{O\xad\xbc\xbf\x87J\x0bK\x97Z\xa8\xf6H\xec}{\r\xcd!\x06\xdf\x9e\x15\xfdA\x8aH\xb7 \xf4\xa1z\x1bW\xd4]P\xff\xbag\xd8#\x99\x1f\xc8?\xe3\xde\xffo[w\xb1kn\xb8\xc9d\xf7\xe1\xcaAF\x0e)q\xd0\xb9#\xfc\xc9\x81_N\xf7o\xdf\xbf\x84\xadsd\xbb\xb7B\x8ei\xf6\xd4v\x1d~\x9d\xa7\xb8W\x8aQgr\xd7\xd4\xa8\xb8\x95T@s\x03\xf6\xea\xf4\xeb\xfe(Bw?\x9d#\x1b\xb2\xb6=\x80\x14\x07L.O\xf7\xd5\n\x16\r\xbdfC7~#Cy\xc3@\x86\xf5L)\xda\x8e\x9a\xad\r\xa5\x04\x87\x88\x1e\x85\xe3\xe9S\xd5\x9b\xc8\x8b\x03cx\xeb\xe0\x19Jn\xbb/k3dX\x93\xadi\xbf\x8f\x1b\xef\x82H\xc7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=NetLock Arany (Class Gold) Főtanúsítvány O=NetLock Kft. OU=Tanúsítványkiadók (Certification Services)
   * Subject: CN=NetLock Arany (Class Gold) Főtanúsítvány O=NetLock Kft. OU=Tanúsítványkiadók (Certification Services)
   * Label: "NetLock Arany (Class Gold) Főtanúsítvány"
   * Serial: 80544274841616
   * SHA256 Fingerprint: 6c:61:da:c3:a2:de:f0:31:50:6b:e0:36:d2:a6:fe:40:19:94:fb:d1:3d:f9:c8:d4:66:59:92:74:c4:46:ec:98
   * -----BEGIN CERTIFICATE-----
   * MIIEFTCCAv2gAwIBAgIGSUEs5AAQMA0GCSqGSIb3DQEBCwUAMIGnMQswCQYDVQQG
   * EwJIVTERMA8GA1UEBwwIQnVkYXBlc3QxFTATBgNVBAoMDE5ldExvY2sgS2Z0LjE3
   * MDUGA1UECwwuVGFuw7pzw610dsOhbnlraWFkw7NrIChDZXJ0aWZpY2F0aW9uIFNl
   * cnZpY2VzKTE1MDMGA1UEAwwsTmV0TG9jayBBcmFueSAoQ2xhc3MgR29sZCkgRsWR
   * dGFuw7pzw610dsOhbnkwHhcNMDgxMjExMTUwODIxWhcNMjgxMjA2MTUwODIxWjCB
   * pzELMAkGA1UEBhMCSFUxETAPBgNVBAcMCEJ1ZGFwZXN0MRUwEwYDVQQKDAxOZXRM
   * b2NrIEtmdC4xNzA1BgNVBAsMLlRhbsO6c8OtdHbDoW55a2lhZMOzayAoQ2VydGlm
   * aWNhdGlvbiBTZXJ2aWNlcykxNTAzBgNVBAMMLE5ldExvY2sgQXJhbnkgKENsYXNz
   * IEdvbGQpIEbFkXRhbsO6c8OtdHbDoW55MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8A
   * MIIBCgKCAQEAxCRec75LbRTDofTjl5Bu0jBFHjzuZ9lk4BqKf8owyoPjIMHj9DrT
   * lF8afFttvzBPhCf2nx9JvMaZCpDyD/V/Q4Q3Y1GLeqVw/HpYzY6b7cNGbIRwXdrz
   * AZAj/E4wqX7hJ2Pn7WQ8oLjJM2P+FpD/sLj916jAwJRDC7bVWaaeVtAkH3B5r9s5
   * VA1lddkVQZQBr17s9o3x/61k/iCa11zr/qYfCGSji3ZVrR47KGAuhyXoqq8fxmRG
   * ILdwfzzeSNuWU7c5d+Qa4scWhHaXWy+7GRWF+GmF9ZmnqfI0p6m2pgP8b4Y9VHx2
   * BJtr+UBdADTHLpl1neWIA6pN+APSQnbAGwIDAKiLo0UwQzASBgNVHRMBAf8ECDAG
   * AQH/AgEEMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUzPpnk/C2uNClwB7zU/2M
   * U9+D15YwDQYJKoZIhvcNAQELBQADggEBAKt/7hwWqZw8UQCgwBEIBaeZ5m8BiFRh
   * bvG5GK1Krf6BQCOUL/t1fC8oS2IkgYIL9WHxHG64YTjrgfpioTtaYtOUZcTh5m2C
   * +C8lcLIhJsFyUR+MLMOEkMNaj7rP9KdlpeuY0fsFskZ1FSNqb4VjMIDw1Z4fKRzC
   * bLBQWV2QWzuoDTDPv31/zvGdg73JRm4gpvlhUbohL3u+pRVjodSVh/GeufOJ8z2F
   * uLjbvrW5KfnaNwUASZQDhETnv0Mxz3WLJdH0pmT1kvarBes96aULNmLazAZfNou2
   * XjG4Kvte9nHfRCaexOYNkbQudZWAUWpLMKawYqGT8ZvYzsRjdT9ZR7E=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02HU1\x110\x0f\x06\x03U\x04\x07\x0c\x08Budapest1\x150\x13\x06\x03U\x04\n\x0c\x0cNetLock Kft.1705\x06\x03U\x04\x0b\x0c.Tan\xc3\xbas\xc3\xadtv\xc3\xa1nykiad\xc3\xb3k (Certification Services)1503\x06\x03U\x04\x03\x0c,NetLock Arany (Class Gold) F\xc5\x91tan\xc3\xbas\xc3\xadtv\xc3\xa1ny"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xc4$^s\xbeKm\x14\xc3\xa1\xf4\xe3\x97\x90n\xd20E\x1e<\xeeg\xd9d\xe0\x1a\x8a\x7f\xca0\xca\x83\xe3 \xc1\xe3\xf4:\xd3\x94_\x1a|[m\xbf0O\x84\'\xf6\x9f\x1fI\xbc\xc6\x99\n\x90\xf2\x0f\xf5\x7fC\x847cQ\x8bz\xa5p\xfczX\xcd\x8e\x9b\xed\xc3Fl\x84p]\xda\xf3\x01\x90#\xfcN0\xa9~\xe1\'c\xe7\xedd<\xa0\xb8\xc93c\xfe\x16\x90\xff\xb0\xb8\xfd\xd7\xa8\xc0\xc0\x94C\x0b\xb6\xd5Y\xa6\x9eV\xd0$\x1fpy\xaf\xdb9T\reu\xd9\x15A\x94\x01\xaf^\xec\xf6\x8d\xf1\xff\xadd\xfe \x9a\xd7\\\xeb\xfe\xa6\x1f\x08d\xa3\x8bvU\xad\x1e;(`.\x87%\xe8\xaa\xaf\x1f\xc6dF \xb7p\x7f<\xdeH\xdb\x96S\xb79w\xe4\x1a\xe2\xc7\x16\x84v\x97[/\xbb\x19\x15\x85\xf8i\x85\xf5\x99\xa7\xa9\xf24\xa7\xa9\xb6\xa6\x03\xfco\x86=T|v\x04\x9bk\xf9@]\x004\xc7.\x99u\x9d\xe5\x88\x03\xaaM\xf8\x03\xd2Bv\xc0\x1b\x02\x03\x00\xa8\x8b"),
    name_constraints: None
  },

  /*
   * Issuer: CN=AffirmTrust Premium O=AffirmTrust
   * Subject: CN=AffirmTrust Premium O=AffirmTrust
   * Label: "AffirmTrust Premium"
   * Serial: 7893706540734352110
   * SHA256 Fingerprint: 70:a7:3f:7f:37:6b:60:07:42:48:90:45:34:b1:14:82:d5:bf:0e:69:8e:cc:49:8d:f5:25:77:eb:f2:e9:3b:9a
   * -----BEGIN CERTIFICATE-----
   * MIIFRjCCAy6gAwIBAgIIbYwURrGmCu4wDQYJKoZIhvcNAQEMBQAwQTELMAkGA1UE
   * BhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MRwwGgYDVQQDDBNBZmZpcm1UcnVz
   * dCBQcmVtaXVtMB4XDTEwMDEyOTE0MTAzNloXDTQwMTIzMTE0MTAzNlowQTELMAkG
   * A1UEBhMCVVMxFDASBgNVBAoMC0FmZmlybVRydXN0MRwwGgYDVQQDDBNBZmZpcm1U
   * cnVzdCBQcmVtaXVtMIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAxBLf
   * qV/+Qd3d9Z+K4/as4Tx4mrzY8H96oDMq3I0gW64tb+eT2TZwamjPjlGjhVtnBKAQ
   * JG9dKILBl1fYSCkTtuG+kU3fhQxTGJoeJKJPj/CihQvL9Cl/0qRY7iZNyaqoe5rZ
   * +jjeRFcV5fiMyNlI4g0WJx0eyIOFJbe6qlVBzAMiSy2RjYvmia9mx+n/K+k8rNrS
   * s8PhaJyJ+HoAVt70VZVs+7pk3WKL3wt3MutizCaam7uqYoNMtAZ6MMgpv+0GTZe5
   * HMQxK9VfvFMSF5yZVylmd2EhMQcuJUmdGPLu8ytxjLW6OQdJd/zvLpKQBY0tL3d7
   * 70O/Nbua2Plzpyzy0FfuKE4mX4+QaAkvuPjcBukumj5Rp9EixAqnOEhss/n/fauG
   * V+O61oV4d7pD6kh/9ti+I20ev9E2bFhc8e6kGVQa9QPSdubhjL08s9NIS+LI+H+S
   * qHZGnEJlPqQewQcDWkYtuJfzt9WyVSHvutxMAJf7FJUnM7/oQ0dG0giZFmA7mn7S
   * 5u046uwBHjxIVkkJx0w3AJ6IDsBz4W9m6XJHMD4Q5QsDyZpCAGzFlH5hxIrff4Ia
   * C1nEWTJ3s7xgaVY5/bQGeyzWZDbZvUjthB9+pSKPKrhC9IK31FOQeE4tGv2Bb0TX
   * OwF0lkLgAOIua+rF7nKsu7/+6qqo+Nz2snmKtmcCAwEAAaNCMEAwHQYDVR0OBBYE
   * FJ3AZ6YMItkm9UWrpmVSESfYRaxjMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/
   * BAQDAgEGMA0GCSqGSIb3DQEBDAUAA4ICAQCzV00QYk465KzquByvMiPIs0laUZx2
   * KI15qldGF9X1Uva3ROgIRL8YhNILgM3FEv0AVQVhh0HctSSePMTYyPtwni94loMg
   * Nt58D2kTiKV1NpgIpsbfrM7jWNa3Pt668+s0QNiigfV4Py/VpfzZotReBA4Xrf5B
   * 8OWycvpEgjNC6C1Y91aMYj+6QrCcDFx+LmUmXFNPALJ4fqENmS2NuB2OosSw/WDQ
   * MKSOyARiqcTtNd56l+0OOF6SL5Nwpamcb6d9Ex1+xghIsV5n61EIJenmJWtSKZGc
   * 0jlzCFfemQa0W50QBuHCAKi4HEoCChTQwUHK+4w1IX2COPKpVJEZNZOUbWo6xbLQ
   * u4mGk+ibyQ86p3q4ofB4Rvr8Ny/lioTz3/4E2aFooC8k4gmVBtWVyuEklut89pMF
   * u+1z6S3RdTnX5yTb2E5fQ4+e0BQ5v1VwSJlXMbSc7kqYA5YwH2AG7hsj/oFgIxpH
   * YoWlzBk0gG+zrBrjn/B7SK3VAdlntqlyk+otZrWyuOQ9PLLvTIzq6we/qzWaVYa8
   * GKa1qF60g2xraUDTn9zxw2lrueFtCfTxqlB2Cnp9ehehVZZCmTEJ3WARjQUwfuaO
   * RtGdFNrHF+QFlozEJLUbzxQHskD4o55BhrwE0GuWyCqANP2/7waj3VjFhT0+j/6e
   * KeC2uAloGRwYQw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x140\x12\x06\x03U\x04\n\x0c\x0bAffirmTrust1\x1c0\x1a\x06\x03U\x04\x03\x0c\x13AffirmTrust Premium"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc4\x12\xdf\xa9_\xfeA\xdd\xdd\xf5\x9f\x8a\xe3\xf6\xac\xe1<x\x9a\xbc\xd8\xf0\x7fz\xa03*\xdc\x8d [\xae-o\xe7\x93\xd96pjh\xcf\x8eQ\xa3\x85[g\x04\xa0\x10$o](\x82\xc1\x97W\xd8H)\x13\xb6\xe1\xbe\x91M\xdf\x85\x0cS\x18\x9a\x1e$\xa2O\x8f\xf0\xa2\x85\x0b\xcb\xf4)\x7f\xd2\xa4X\xee&M\xc9\xaa\xa8{\x9a\xd9\xfa8\xdeDW\x15\xe5\xf8\x8c\xc8\xd9H\xe2\r\x16\'\x1d\x1e\xc8\x83\x85%\xb7\xba\xaaUA\xcc\x03\"K-\x91\x8d\x8b\xe6\x89\xaff\xc7\xe9\xff+\xe9<\xac\xda\xd2\xb3\xc3\xe1h\x9c\x89\xf8z\x00V\xde\xf4U\x95l\xfb\xbad\xddb\x8b\xdf\x0bw2\xebb\xcc&\x9a\x9b\xbb\xaab\x83L\xb4\x06z0\xc8)\xbf\xed\x06M\x97\xb9\x1c\xc41+\xd5_\xbcS\x12\x17\x9c\x99W)fwa!1\x07.%I\x9d\x18\xf2\xee\xf3+q\x8c\xb5\xba9\x07Iw\xfc\xef.\x92\x90\x05\x8d-/w{\xefC\xbf5\xbb\x9a\xd8\xf9s\xa7,\xf2\xd0W\xee(N&_\x8f\x90h\t/\xb8\xf8\xdc\x06\xe9.\x9a>Q\xa7\xd1\"\xc4\n\xa78Hl\xb3\xf9\xff}\xab\x86W\xe3\xba\xd6\x85xw\xbaC\xeaH\x7f\xf6\xd8\xbe#m\x1e\xbf\xd16lX\\\xf1\xee\xa4\x19T\x1a\xf5\x03\xd2v\xe6\xe1\x8c\xbd<\xb3\xd3HK\xe2\xc8\xf8\x7f\x92\xa8vF\x9cBe>\xa4\x1e\xc1\x07\x03ZF-\xb8\x97\xf3\xb7\xd5\xb2U!\xef\xba\xdcL\x00\x97\xfb\x14\x95\'3\xbf\xe8CGF\xd2\x08\x99\x16`;\x9a~\xd2\xe6\xed8\xea\xec\x01\x1e<HVI\t\xc7L7\x00\x9e\x88\x0e\xc0s\xe1of\xe9rG0>\x10\xe5\x0b\x03\xc9\x9aB\x00l\xc5\x94~a\xc4\x8a\xdf\x7f\x82\x1a\x0bY\xc4Y2w\xb3\xbc`iV9\xfd\xb4\x06{,\xd6d6\xd9\xbdH\xed\x84\x1f~\xa5\"\x8f*\xb8B\xf4\x82\xb7\xd4S\x90xN-\x1a\xfd\x81oD\xd7;\x01t\x96B\xe0\x00\xe2.k\xea\xc5\xeer\xac\xbb\xbf\xfe\xea\xaa\xa8\xf8\xdc\xf6\xb2y\x8a\xb6g\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Entrust Root Certification Authority O=Entrust, Inc. OU=www.entrust.net/CPS is incorporated by reference/(c) 2006 Entrust, Inc.
   * Subject: CN=Entrust Root Certification Authority O=Entrust, Inc. OU=www.entrust.net/CPS is incorporated by reference/(c) 2006 Entrust, Inc.
   * Label: "Entrust Root Certification Authority"
   * Serial: 1164660820
   * SHA256 Fingerprint: 73:c1:76:43:4f:1b:c6:d5:ad:f4:5b:0e:76:e7:27:28:7c:8d:e5:76:16:c1:e6:e6:14:1a:2b:2c:bc:7d:8e:4c
   * -----BEGIN CERTIFICATE-----
   * MIIEkTCCA3mgAwIBAgIERWtQVDANBgkqhkiG9w0BAQUFADCBsDELMAkGA1UEBhMC
   * VVMxFjAUBgNVBAoTDUVudHJ1c3QsIEluYy4xOTA3BgNVBAsTMHd3dy5lbnRydXN0
   * Lm5ldC9DUFMgaXMgaW5jb3Jwb3JhdGVkIGJ5IHJlZmVyZW5jZTEfMB0GA1UECxMW
   * KGMpIDIwMDYgRW50cnVzdCwgSW5jLjEtMCsGA1UEAxMkRW50cnVzdCBSb290IENl
   * cnRpZmljYXRpb24gQXV0aG9yaXR5MB4XDTA2MTEyNzIwMjM0MloXDTI2MTEyNzIw
   * NTM0MlowgbAxCzAJBgNVBAYTAlVTMRYwFAYDVQQKEw1FbnRydXN0LCBJbmMuMTkw
   * NwYDVQQLEzB3d3cuZW50cnVzdC5uZXQvQ1BTIGlzIGluY29ycG9yYXRlZCBieSBy
   * ZWZlcmVuY2UxHzAdBgNVBAsTFihjKSAyMDA2IEVudHJ1c3QsIEluYy4xLTArBgNV
   * BAMTJEVudHJ1c3QgUm9vdCBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTCCASIwDQYJ
   * KoZIhvcNAQEBBQADggEPADCCAQoCggEBALaVtkNC+sZtKm9I35RMOVcF7sN5EUFo
   * Nu3s/poBj6E4KPz3EEZmLk0eGrEaTsbRwJWIsMn/MYszA9u3g3s+IIRe7bJWKKf4
   * 4LlAcTfFy0cOlypowCKVYhXbR9n10Cv/gkvJrT7eTNuQgFA/CYqEAOwwCj0Yzfv9
   * KlmaI5UXLEWeH25DeW0MXJj+SKfFI0dcXv1u5x609mhF0YaDW6KKjbHjKYD+JXGI
   * rb68j6xSlkuqUY3kEzEZ6E5Nn9uss2rVvDlUccp6en+Q3X0dgNmBu1kmwhH+5pPi
   * 94DkZfs0Nw4pgHBNrziGLp5/V6+eF67rHMsoIV+2HNjnogQi+dPa2MsCAwEAAaOB
   * sDCBrTAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB/zArBgNVHRAEJDAi
   * gA8yMDA2MTEyNzIwMjM0MlqBDzIwMjYxMTI3MjA1MzQyWjAfBgNVHSMEGDAWgBRo
   * kORnpKZTgMeGZqTx90tD+4S9bTAdBgNVHQ4EFgQUaJDkZ6SmU4DHhmak8fdLQ/uE
   * vW0wHQYJKoZIhvZ9B0EABBAwDhsIVjcuMTo0LjADAgSQMA0GCSqGSIb3DQEBBQUA
   * A4IBAQCT1DCw1wMgKtD5Y+iRDAUgqV8ZyntyTtSx29CW+1RaGSwMCPeyvIWonX9t
   * O1KzKtvn1ISMY/YPyyYBkVBs9F8U4pN0wBOeMDpQ47RgxRzwIkSNcUesyBrJ6Zua
   * AGAT/3B+XxFNSRuzFVJ7yVTav52Vr2ua2J7p8eRDjeIRRDq/r72DQnNSi6q7pynP
   * 9WQcCk3RvKqsnyrQ/39/2n3qse0wJcGE2jTSW3iDVuycNsMm4hH2Z0kdkquM++v/
   * eu6FSqdQgPCnXEqULl8FmTxSQeDNtGPPAUO6nIPcj2A781q0tHuu2guQOHXvgR1m
   * 0vdXcDazv/wor3ElhVsT/h5/WrQ8
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x160\x14\x06\x03U\x04\n\x13\rEntrust, Inc.1907\x06\x03U\x04\x0b\x130www.entrust.net/CPS is incorporated by reference1\x1f0\x1d\x06\x03U\x04\x0b\x13\x16(c) 2006 Entrust, Inc.1-0+\x06\x03U\x04\x03\x13$Entrust Root Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb6\x95\xb6CB\xfa\xc6m*oH\xdf\x94L9W\x05\xee\xc3y\x11Ah6\xed\xec\xfe\x9a\x01\x8f\xa18(\xfc\xf7\x10Ff.M\x1e\x1a\xb1\x1aN\xc6\xd1\xc0\x95\x88\xb0\xc9\xff1\x8b3\x03\xdb\xb7\x83{> \x84^\xed\xb2V(\xa7\xf8\xe0\xb9@q7\xc5\xcbG\x0e\x97*h\xc0\"\x95b\x15\xdbG\xd9\xf5\xd0+\xff\x82K\xc9\xad>\xdeL\xdb\x90\x80P?\t\x8a\x84\x00\xec0\n=\x18\xcd\xfb\xfd*Y\x9a#\x95\x17,E\x9e\x1fnCym\x0c\\\x98\xfeH\xa7\xc5#G\\^\xfdn\xe7\x1e\xb4\xf6hE\xd1\x86\x83[\xa2\x8a\x8d\xb1\xe3)\x80\xfe%q\x88\xad\xbe\xbc\x8f\xacR\x96K\xaaQ\x8d\xe4\x131\x19\xe8NM\x9f\xdb\xac\xb3j\xd5\xbc9Tq\xcazz\x7f\x90\xdd}\x1d\x80\xd9\x81\xbbY&\xc2\x11\xfe\xe6\x93\xe2\xf7\x80\xe4e\xfb47\x0e)\x80pM\xaf8\x86.\x9e\x7fW\xaf\x9e\x17\xae\xeb\x1c\xcb(!_\xb6\x1c\xd8\xe7\xa2\x04\"\xf9\xd3\xda\xd8\xcb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert High Assurance EV Root CA O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert High Assurance EV Root CA O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert High Assurance EV Root CA"
   * Serial: 3553400076410547919724730734378100087
   * SHA256 Fingerprint: 74:31:e5:f4:c3:c1:ce:46:90:77:4f:0b:61:e0:54:40:88:3b:a9:a0:1e:d0:0b:a6:ab:d7:80:6e:d3:b1:18:cf
   * -----BEGIN CERTIFICATE-----
   * MIIDxTCCAq2gAwIBAgIQAqxcJmoLQJuPC3nyrkYldzANBgkqhkiG9w0BAQUFADBs
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSswKQYDVQQDEyJEaWdpQ2VydCBIaWdoIEFzc3VyYW5j
   * ZSBFViBSb290IENBMB4XDTA2MTExMDAwMDAwMFoXDTMxMTExMDAwMDAwMFowbDEL
   * MAkGA1UEBhMCVVMxFTATBgNVBAoTDERpZ2lDZXJ0IEluYzEZMBcGA1UECxMQd3d3
   * LmRpZ2ljZXJ0LmNvbTErMCkGA1UEAxMiRGlnaUNlcnQgSGlnaCBBc3N1cmFuY2Ug
   * RVYgUm9vdCBDQTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAMbM5XPm
   * +9S75S0tMqbf5YE/yc0lSbZxKsPVlDRnogocsF9ppkCxxLeyj9CYpKlBWTrT3JTW
   * PNt0OKRKzE0lgvdKpVMSOO7zSW1xkX5jtqumX8OkhPhPYlG++MXs2ziS4wblCJEM
   * xChBVfvLWokVfnHoNb9Ncgk9vjo4UFt3MRuNs8ckRZqnrG0AFFoEt7oT61EKmEFB
   * Ik5lYYeBQVCmeVyJ3hlKV9Uu5l0cUyx+mM0aBhakaHPQNAQTXKFx01p8VdteZOE3
   * hzBWBOURtCmAEvF5OYiiAhF8J2a3iLd48soKqDirCmTCv2ZdlYTBoSUeh10aUAsg
   * EsxBu24LUTi4S8sCAwEAAaNjMGEwDgYDVR0PAQH/BAQDAgGGMA8GA1UdEwEB/wQF
   * MAMBAf8wHQYDVR0OBBYEFLE+w2kD+L9HAdSYJhoIAu9jZCvDMB8GA1UdIwQYMBaA
   * FLE+w2kD+L9HAdSYJhoIAu9jZCvDMA0GCSqGSIb3DQEBBQUAA4IBAQAcGgaX3Nec
   * nzyIZgYIVyHbIUf4KmeqvxgydkAQV8GK83rZEWWONfqe/EW1ntlMMUu4kehDLI6z
   * eM7b41N5cdblIZQB2lWHmiRk9opmzN6cN82oNLFpmyPInngiK3BD41VHMWEZ71jF
   * hS9OMPagMRYjyOfiZRYzy78aG6A9+MpeizGLYAiJLQwGXFK3xPkKmNEVX58Svnw2
   * Yzi9RKR/5CYrCsSXaQ3pjOLAEFe4yHYSkVXySGnYvCoCWw9E1CAx2/S6cCZdkGCe
   * vEsXCS+0yx5DaMkHJ8HSXPfqIbloEpw8nL+e/IBcm2PN7EeqJSdnoDfzAIJ9VNep
   * +OkuE6N36B9K
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1+0)\x06\x03U\x04\x03\x13\"DigiCert High Assurance EV Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xc6\xcc\xe5s\xe6\xfb\xd4\xbb\xe5--2\xa6\xdf\xe5\x81?\xc9\xcd%I\xb6q*\xc3\xd5\x944g\xa2\n\x1c\xb0_i\xa6@\xb1\xc4\xb7\xb2\x8f\xd0\x98\xa4\xa9AY:\xd3\xdc\x94\xd6<\xdbt8\xa4J\xccM%\x82\xf7J\xa5S\x128\xee\xf3Imq\x91~c\xb6\xab\xa6_\xc3\xa4\x84\xf8ObQ\xbe\xf8\xc5\xec\xdb8\x92\xe3\x06\xe5\x08\x91\x0c\xc4(AU\xfb\xcbZ\x89\x15~q\xe85\xbfMr\t=\xbe:8P[w1\x1b\x8d\xb3\xc7$E\x9a\xa7\xacm\x00\x14Z\x04\xb7\xba\x13\xebQ\n\x98AA\"Nea\x87\x81AP\xa6y\\\x89\xde\x19JW\xd5.\xe6]\x1cS,~\x98\xcd\x1a\x06\x16\xa4hs\xd04\x04\x13\\\xa1q\xd3Z|U\xdb^d\xe17\x870V\x04\xe5\x11\xb4)\x80\x12\xf1y9\x88\xa2\x02\x11|\'f\xb7\x88\xb7x\xf2\xca\n\xa88\xab\nd\xc2\xbff]\x95\x84\xc1\xa1%\x1e\x87]\x1aP\x0b \x12\xccA\xbbn\x0bQ8\xb8K\xcb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certainly Root R1 O=Certainly
   * Subject: CN=Certainly Root R1 O=Certainly
   * Label: "Certainly Root R1"
   * Serial: 188833316161142517227353805653483829216
   * SHA256 Fingerprint: 77:b8:2c:d8:64:4c:43:05:f7:ac:c5:cb:15:6b:45:67:50:04:03:3d:51:c6:0c:62:02:a8:e0:c3:34:67:d3:a0
   * -----BEGIN CERTIFICATE-----
   * MIIFRzCCAy+gAwIBAgIRAI4P+UuQcWhlM1T01EQ5t+AwDQYJKoZIhvcNAQELBQAw
   * PTELMAkGA1UEBhMCVVMxEjAQBgNVBAoTCUNlcnRhaW5seTEaMBgGA1UEAxMRQ2Vy
   * dGFpbmx5IFJvb3QgUjEwHhcNMjEwNDAxMDAwMDAwWhcNNDYwNDAxMDAwMDAwWjA9
   * MQswCQYDVQQGEwJVUzESMBAGA1UEChMJQ2VydGFpbmx5MRowGAYDVQQDExFDZXJ0
   * YWlubHkgUm9vdCBSMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBANA2
   * 1B/q3avk0bbm+yLA3RMNansiExyXPGhjZjKcA7WNpIGD2ngwEc/csiu+kr+O5MQT
   * vqRoTNoCaBZ0vrLdBORrKt03H2As2/X3oXyVtwxwhi7xOu9S98zTm/mLvg7fMbed
   * aFySpvXl8wo0tf97ouSHocavFwDvA5HtqRxOcT3Si2yJ9HiG5mpJoM610rCrm/b0
   * 1C7jcvk2xusVtyWMOvwlDbMicyF0yEqWYZL1LwsYpfSt4u5BvQF5+paMjRcCMLT5
   * r3gajLQ2EBAHBXDQ9DGQilHFhiZ5shGIXsXwClTNSaa/ApzSRKft43jvRl5tcdF5
   * cBxGX1HpyTfcX35pe0HfNEXgO4T0oYoKNp43zGJS4YkNKPl6I7ENPT2a/Z2B7yyQ
   * wHtETrtJ4A5KVpK8y7XdeReJkd5hiXSSqOMyhb5OhaRLWcsrxXiOcVTQAjeZjOVJ
   * 6uBUcqQRBi8LjMFbvrWhsFNunLhgkR9Za/kt9JQKl7XsxXYDVBtlUrpMklZRNaBA
   * 2CnbrlJ2Oy0wQJuK0EJWtLeIAaSHO1OWzaMWj/Nmqhexx2DgwUMFDO6bW2BvBlyH
   * Wyf5QBGenDPBt+U1VwV/J84XIIwc/PH72jEpSe31C4SnT8H2TsIonPru4K8H+zMR
   * eiFPCyEQtkA6qyI6BJyLm4SGcprSp6XEtHWRqSsjAgMBAAGjQjBAMA4GA1UdDwEB
   * /wQEAwIBBjAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBTgqj8ljZ9EXME66C6u
   * d0yEPmcM9DANBgkqhkiG9w0BAQsFAAOCAgEAuVevuBLaV4OPaAszHQNTVfSVcOQr
   * PbA56/qJYv331hgELyE03fFo8NWWWt7CgKPBjcZq91l3rhVkz1t5BXdm6ozTaw3d
   * 8VkswTOlMIAVRQdFGjEitpIAq5lNOo93r6kiyi9jyhXWx8bwPWz8HA2YEGGeEaIi
   * 1wrykXprOQ4vMMM2SZ/g6Q8CRFA3lFV96p/2O7qUpUzpvD5RtOjKkjZUbVwlKNrd
   * rRT90+7iIgXr0PK3aBLXWopBGsaSpVo7Y0VPv+E6dyIvXL9G+VoDhRNCX8reU9di
   * taY1BMJH/5n9hN9czulegChB8n3nHpDYT3Y+gjwN/KUD+nsa2UUeYNrEjvn8K8l7
   * lcUq/6qJ34IxD3L/DCfXCh5WAFAeDJDBlrXYFIW7pw0WwfgHJBu6haEaBQmAupVj
   * yTrsJZ9/nbqkRxWbRHDxakvWOF5D8xh+UG7pWijmZeZ3Gzr9Hb4DJqPb1OG7fpYn
   * Kx3upPvaJVQTA945xsMfTZDsjxtK0hzthZU4UHlG1sGQUDGpXJpuHfUzVounmdLy
   * yCwzk5Iwx06MZTMQZBf9JBeW0Y3COmor6xOLRPIh80oat3df1+2IpHLlOR+Vnb5n
   * wXARPbv0+Em34yaXOp/SX3z7wJl8OSngex2/DaeP0ik0biQVy96QXr8axGbqwua6
   * OV+KmalBWQewLK8=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x13\tCertainly1\x1a0\x18\x06\x03U\x04\x03\x13\x11Certainly Root R1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd06\xd4\x1f\xea\xdd\xab\xe4\xd1\xb6\xe6\xfb\"\xc0\xdd\x13\rj{\"\x13\x1c\x97<hcf2\x9c\x03\xb5\x8d\xa4\x81\x83\xdax0\x11\xcf\xdc\xb2+\xbe\x92\xbf\x8e\xe4\xc4\x13\xbe\xa4hL\xda\x02h\x16t\xbe\xb2\xdd\x04\xe4k*\xdd7\x1f`,\xdb\xf5\xf7\xa1|\x95\xb7\x0cp\x86.\xf1:\xefR\xf7\xcc\xd3\x9b\xf9\x8b\xbe\x0e\xdf1\xb7\x9dh\\\x92\xa6\xf5\xe5\xf3\n4\xb5\xff{\xa2\xe4\x87\xa1\xc6\xaf\x17\x00\xef\x03\x91\xed\xa9\x1cNq=\xd2\x8bl\x89\xf4x\x86\xe6jI\xa0\xce\xb5\xd2\xb0\xab\x9b\xf6\xf4\xd4.\xe3r\xf96\xc6\xeb\x15\xb7%\x8c:\xfc%\r\xb3\"s!t\xc8J\x96a\x92\xf5/\x0b\x18\xa5\xf4\xad\xe2\xeeA\xbd\x01y\xfa\x96\x8c\x8d\x17\x020\xb4\xf9\xafx\x1a\x8c\xb46\x10\x10\x07\x05p\xd0\xf41\x90\x8aQ\xc5\x86&y\xb2\x11\x88^\xc5\xf0\nT\xcdI\xa6\xbf\x02\x9c\xd2D\xa7\xed\xe3x\xefF^mq\xd1yp\x1cF_Q\xe9\xc97\xdc_~i{A\xdf4E\xe0;\x84\xf4\xa1\x8a\n6\x9e7\xccbR\xe1\x89\r(\xf9z#\xb1\r==\x9a\xfd\x9d\x81\xef,\x90\xc0{DN\xbbI\xe0\x0eJV\x92\xbc\xcb\xb5\xddy\x17\x89\x91\xdea\x89t\x92\xa8\xe32\x85\xbeN\x85\xa4KY\xcb+\xc5x\x8eqT\xd0\x027\x99\x8c\xe5I\xea\xe0Tr\xa4\x11\x06/\x0b\x8c\xc1[\xbe\xb5\xa1\xb0Sn\x9c\xb8`\x91\x1fYk\xf9-\xf4\x94\n\x97\xb5\xec\xc5v\x03T\x1beR\xbaL\x92VQ5\xa0@\xd8)\xdb\xaeRv;-0@\x9b\x8a\xd0BV\xb4\xb7\x88\x01\xa4\x87;S\x96\xcd\xa3\x16\x8f\xf3f\xaa\x17\xb1\xc7`\xe0\xc1C\x05\x0c\xee\x9b[`o\x06\\\x87[\'\xf9@\x11\x9e\x9c3\xc1\xb7\xe55W\x05\x7f\'\xce\x17 \x8c\x1c\xfc\xf1\xfb\xda1)I\xed\xf5\x0b\x84\xa7O\xc1\xf6N\xc2(\x9c\xfa\xee\xe0\xaf\x07\xfb3\x11z!O\x0b!\x10\xb6@:\xab\":\x04\x9c\x8b\x9b\x84\x86r\x9a\xd2\xa7\xa5\xc4\xb4u\x91\xa9+#\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Sectigo Public Server Authentication Root R46 O=Sectigo Limited
   * Subject: CN=Sectigo Public Server Authentication Root R46 O=Sectigo Limited
   * Label: "Sectigo Public Server Authentication Root R46"
   * Serial: 156256931880233212765902055439220583700
   * SHA256 Fingerprint: 7b:b6:47:a6:2a:ee:ac:88:bf:25:7a:a5:22:d0:1f:fe:a3:95:e0:ab:45:c7:3f:93:f6:56:54:ec:38:f2:5a:06
   * -----BEGIN CERTIFICATE-----
   * MIIFijCCA3KgAwIBAgIQdY39i658BwD6qSWn4cetFDANBgkqhkiG9w0BAQwFADBf
   * MQswCQYDVQQGEwJHQjEYMBYGA1UEChMPU2VjdGlnbyBMaW1pdGVkMTYwNAYDVQQD
   * Ey1TZWN0aWdvIFB1YmxpYyBTZXJ2ZXIgQXV0aGVudGljYXRpb24gUm9vdCBSNDYw
   * HhcNMjEwMzIyMDAwMDAwWhcNNDYwMzIxMjM1OTU5WjBfMQswCQYDVQQGEwJHQjEY
   * MBYGA1UEChMPU2VjdGlnbyBMaW1pdGVkMTYwNAYDVQQDEy1TZWN0aWdvIFB1Ymxp
   * YyBTZXJ2ZXIgQXV0aGVudGljYXRpb24gUm9vdCBSNDYwggIiMA0GCSqGSIb3DQEB
   * AQUAA4ICDwAwggIKAoICAQCTvtU2UnXYASOgHEdCSe5jtrch/cSV1UgrJnwUUxDa
   * ef0rty2k1Cz66jLdScK5vQ9IPXtamFSvnl0xdE8H/FAh3aTPaE8bEmNtJZlMKpnz
   * SDBh+oF8HqcIStw+KxwfGExxqjWMrfhu6DtK2eWUAtaJhBOqbchPM8xQljeSM9xf
   * iOefVNlI8JhD1mb9nxc4Q8UBUQvX4yMPFF1bFOdLvt30yNoDN9HWOaEhUTCDsG3X
   * ME6WW5HwcCSrv0WBZEMNvSE6Lzzpng3LILVCJ8zab5vuZDCQOc2TZYEhMbUjUDM3
   * IuM47fgxMMxF/mL50V0yeUKH32rMVhlATc6qu/m1dkmU8Sf4kaWD5QazYw6A3OAS
   * VYCmO2a0OYctyPDQ0RTp5A1NDvZdV3LFOxxHVp3i1fuBYYzMTYCQNFu31xR13NgE
   * SJ/AwSiItOkcyqex8Va3e0lMWeUgFaiEAin6OJRpmkkGj80feRQXEgyDet4fsZfu
   * +Zd4KKTIRJLpfSYFplhym3kT2BFfrsU4YjRosoYwjviQYZ4ybPUHNs2iTG7sijbt
   * 8uaZFURww3y8nDnAtOFr94MlI1fZEoDlSfB1D++N6xybVCi0ITz8fAr/73trdf+L
   * HaAZBav6+CuBQug4urv7qv094PPK306Xlynt8xhW6aWWrL3DkJiy4Pmi1KZHQ3xt
   * zwIDAQABo0IwQDAdBgNVHQ4EFgQUVnNYZJX5khqwEioEYnmhQBWIIUkwDgYDVR0P
   * AQH/BAQDAgGGMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQEMBQADggIBAC9c
   * mTz8Bl6MlC5w6tIyMY208FHVvArzZJ8HXtXBc2hkeqK5Duj5XYUtqDdFqij0lgVQ
   * YKlJfp/imTYpE0RHap1VIDzYm/EDMrraQKFz6oOht0SmDpkBm+S8f74TlH7Kph52
   * gDY9hAaLMyZlbcp+nv4fjFg4exqDsQ+8FxG75gbMY/qB8oFM2gsQa6H61SilzwZA
   * Fv97fRheORKkU55+MkIQpiGRqRxOF3yEvJ+M0ejf5lG5Nkc/kLnHvALcWxxPDkjB
   * JYOcCj+esQMzEhonrPcibCTRAUH4WAP+JWgiH5paPHxsnnVI84HxZmduTILA7rpX
   * DhjvLpr3Etiga+kFpaHpaPi8TD8SHkXoUsCjvxInebnMMTzD9joiFgOgyY9mpFui
   * TdaBJQbpdqQACj7LzTWb4OE4y2BThihCQRxEV+ioratF4yUQvNs+ZUH7G6aXD+u5
   * dHn5HrwdVw1Hr8Mvn4dGp+smWg9WY7ViYG4A++MnESLn/pmPNPW56MORcr3Ywx65
   * LvKRRFHQV80MNNVIIb/bE/FmJUNS0nAiNs2fxBx1IK1jcmMGDw4nztJqDby1ORrp
   * 0XZ60Vzk50lJLVU3aPAaOpg+VBeHVOmmJ1CJeyAvP/+/oYtKR5j/K3tJPsMpRmAY
   * QqszKbrAKbkTidOIijlBO8n9pu0f9GBj39ItVQGL
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GB1\x180\x16\x06\x03U\x04\n\x13\x0fSectigo Limited1604\x06\x03U\x04\x03\x13-Sectigo Public Server Authentication Root R46"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x93\xbe\xd56Ru\xd8\x01#\xa0\x1cGBI\xeec\xb6\xb7!\xfd\xc4\x95\xd5H+&|\x14S\x10\xday\xfd+\xb7-\xa4\xd4,\xfa\xea2\xddI\xc2\xb9\xbd\x0fH={Z\x98T\xaf\x9e]1tO\x07\xfcP!\xdd\xa4\xcfhO\x1b\x12cm%\x99L*\x99\xf3H0a\xfa\x81|\x1e\xa7\x08J\xdc>+\x1c\x1f\x18Lq\xaa5\x8c\xad\xf8n\xe8;J\xd9\xe5\x94\x02\xd6\x89\x84\x13\xaam\xc8O3\xccP\x967\x923\xdc_\x88\xe7\x9fT\xd9H\xf0\x98C\xd6f\xfd\x9f\x178C\xc5\x01Q\x0b\xd7\xe3#\x0f\x14][\x14\xe7K\xbe\xdd\xf4\xc8\xda\x037\xd1\xd69\xa1!Q0\x83\xb0m\xd70N\x96[\x91\xf0p$\xab\xbfE\x81dC\r\xbd!:/<\xe9\x9e\r\xcb \xb5B\'\xcc\xdao\x9b\xeed0\x909\xcd\x93e\x81!1\xb5#P37\"\xe38\xed\xf810\xccE\xfeb\xf9\xd1]2yB\x87\xdfj\xccV\x19@M\xce\xaa\xbb\xf9\xb5vI\x94\xf1\'\xf8\x91\xa5\x83\xe5\x06\xb3c\x0e\x80\xdc\xe0\x12U\x80\xa6;f\xb49\x87-\xc8\xf0\xd0\xd1\x14\xe9\xe4\rM\x0e\xf6]Wr\xc5;\x1cGV\x9d\xe2\xd5\xfb\x81a\x8c\xccM\x80\x904[\xb7\xd7\x14u\xdc\xd8\x04H\x9f\xc0\xc1(\x88\xb4\xe9\x1c\xca\xa7\xb1\xf1V\xb7{ILY\xe5 \x15\xa8\x84\x02)\xfa8\x94i\x9aI\x06\x8f\xcd\x1fy\x14\x17\x12\x0c\x83z\xde\x1f\xb1\x97\xee\xf9\x97x(\xa4\xc8D\x92\xe9}&\x05\xa6Xr\x9by\x13\xd8\x11_\xae\xc58b4h\xb2\x860\x8e\xf8\x90a\x9e2l\xf5\x076\xcd\xa2Ln\xec\x8a6\xed\xf2\xe6\x99\x15Dp\xc3|\xbc\x9c9\xc0\xb4\xe1k\xf7\x83%#W\xd9\x12\x80\xe5I\xf0u\x0f\xef\x8d\xeb\x1c\x9bT(\xb4!<\xfc|\n\xff\xef{ku\xff\x8b\x1d\xa0\x19\x05\xab\xfa\xf8+\x81B\xe88\xba\xbb\xfb\xaa\xfd=\xe0\xf3\xca\xdfN\x97\x97)\xed\xf3\x18V\xe9\xa5\x96\xac\xbd\xc3\x90\x98\xb2\xe0\xf9\xa2\xd4\xa6GC|m\xcf\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Assured ID Root G2 O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Assured ID Root G2 O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Assured ID Root G2"
   * Serial: 15385348160840213938643033620894905419
   * SHA256 Fingerprint: 7d:05:eb:b6:82:33:9f:8c:94:51:ee:09:4e:eb:fe:fa:79:53:a1:14:ed:b2:f4:49:49:45:2f:ab:7d:2f:c1:85
   * -----BEGIN CERTIFICATE-----
   * MIIDljCCAn6gAwIBAgIQC5McOtY5Z+pnI7/Dr5r0SzANBgkqhkiG9w0BAQsFADBl
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJv
   * b3QgRzIwHhcNMTMwODAxMTIwMDAwWhcNMzgwMTE1MTIwMDAwWjBlMQswCQYDVQQG
   * EwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNl
   * cnQuY29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJvb3QgRzIwggEi
   * MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDZ5ygvUj82ckmIkzTz+GoeMVSA
   * n61UQbVH35ao1K+ALbkKz3X9iaV9JPrjIgwrvJUXCzO/GU1BBpAAvQxNEP4Htecc
   * biJVMWWXvdMX0h5i89vqbFCMP4QMls+3ywPgym2hFEwbid3tALBSfK+RbLE4E9Hp
   * EgjAALAcKxHad3A2m67OeYfcgnDmCXRwVWmvo2ifv922ebPynXApVfSr/5Vh88lA
   * bx3RvpO704gqu52/clpWcTs/1PPRCv4o76Pu2ZmvA9OPYLfykqGxvYmJHzDNw6Yu
   * YjOuFgJ3RFrngQo8p0Quebg/BLxcoIfhG69Rjs3sLPr4/m3wOnyqi+RnlTGNAgMB
   * AAGjQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQW
   * BBTOw0q5mVXyuNtgv6l+vVa1lzan1jANBgkqhkiG9w0BAQsFAAOCAQEAyqVVjOPI
   * QW5pJ6d1Ee88hjZv0p3GeDgdaZaikmkuOGybfQTUiaWxMTeKySHMq2zNixya1r9I
   * 0jJmwYrA8y8678Dj1JGG0VDjA9tzd29KOVPt3ibHtX2vK0LRdWLjSisCx1BL4Gni
   * lmwORGYQRI+tBev4eaymG+g3NJ1TyWGqolKvSnAWhsI6yLETcDbYz+70CjTVW0z9
   * B5yiutkBclzzTcHdDrEcDcRjvq30FPuJ7KJBDkzMyFdA0G4Dqs0MjomZmWzwPDCv
   * ON9vvKO+KSAnq3T/EyJ43pdSVR6DtVQgA+6uwE9W3jfMw3+qBCe703e4YtsXfJwo
   * IhNzbM8m9Yop5w==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1$0\"\x06\x03U\x04\x03\x13\x1bDigiCert Assured ID Root G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xd9\xe7(/R?6rI\x88\x934\xf3\xf8j\x1e1T\x80\x9f\xadTA\xb5G\xdf\x96\xa8\xd4\xaf\x80-\xb9\n\xcfu\xfd\x89\xa5}$\xfa\xe3\"\x0c+\xbc\x95\x17\x0b3\xbf\x19MA\x06\x90\x00\xbd\x0cM\x10\xfe\x07\xb5\xe7\x1cn\"U1e\x97\xbd\xd3\x17\xd2\x1eb\xf3\xdb\xealP\x8c?\x84\x0c\x96\xcf\xb7\xcb\x03\xe0\xcam\xa1\x14L\x1b\x89\xdd\xed\x00\xb0R|\xaf\x91l\xb18\x13\xd1\xe9\x12\x08\xc0\x00\xb0\x1c+\x11\xdawp6\x9b\xae\xcey\x87\xdc\x82p\xe6\ttpUi\xaf\xa3h\x9f\xbf\xdd\xb6y\xb3\xf2\x9dp)U\xf4\xab\xff\x95a\xf3\xc9@o\x1d\xd1\xbe\x93\xbb\xd3\x88*\xbb\x9d\xbfrZVq;?\xd4\xf3\xd1\n\xfe(\xef\xa3\xee\xd9\x99\xaf\x03\xd3\x8f`\xb7\xf2\x92\xa1\xb1\xbd\x89\x89\x1f0\xcd\xc3\xa6.b3\xae\x16\x02wDZ\xe7\x81\n<\xa7D.y\xb8?\x04\xbc\\\xa0\x87\xe1\x1b\xafQ\x8e\xcd\xec,\xfa\xf8\xfem\xf0:|\xaa\x8b\xe4g\x951\x8d\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Assured ID Root G3 O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Assured ID Root G3 O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Assured ID Root G3"
   * Serial: 15459312981008553731928384953135426796
   * SHA256 Fingerprint: 7e:37:cb:8b:4c:47:09:0c:ab:36:55:1b:a6:f4:5d:b8:40:68:0f:ba:16:6a:95:2d:b1:00:71:7f:43:05:3f:c2
   * -----BEGIN CERTIFICATE-----
   * MIICRjCCAc2gAwIBAgIQC6Fa+h3foLVJRK/NJKBs7DAKBggqhkjOPQQDAzBlMQsw
   * CQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cu
   * ZGlnaWNlcnQuY29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJvb3Qg
   * RzMwHhcNMTMwODAxMTIwMDAwWhcNMzgwMTE1MTIwMDAwWjBlMQswCQYDVQQGEwJV
   * UzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3d3cuZGlnaWNlcnQu
   * Y29tMSQwIgYDVQQDExtEaWdpQ2VydCBBc3N1cmVkIElEIFJvb3QgRzMwdjAQBgcq
   * hkjOPQIBBgUrgQQAIgNiAAQZ57ysRGXtzbg/WPuNsVepRC0FFfLvC/8QdJ+1YlJf
   * Zn4f5dwbRXkLzMZTCp2NXQLZqVneAlr2lSoOjThKiknGvMYDOAdfVdp+CW7if17Q
   * RSAPWXYQ1qAk8C3eNvJsKTmjQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/
   * BAQDAgGGMB0GA1UdDgQWBBTL0L2p4ZgFUaFNN6KDec6NHSrkhDAKBggqhkjOPQQD
   * AwNnADBkAjAlpIFFAmsSS3V0T8gj43DydXLefInwz5FyYZ5eEJJZVrmDxxDnOOlY
   * JjZ91eQ0hjkCMHw2U/Aw5WJjOpnitqM7mzT6HtoQknFekROn3aRukswy1vUhZscv
   * 6pZjamVFkpUBtA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1$0\"\x06\x03U\x04\x03\x13\x1bDigiCert Assured ID Root G3"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x19\xe7\xbc\xacDe\xed\xcd\xb8?X\xfb\x8d\xb1W\xa9D-\x05\x15\xf2\xef\x0b\xff\x10t\x9f\xb5bR_f~\x1f\xe5\xdc\x1bEy\x0b\xcc\xc6S\n\x9d\x8d]\x02\xd9\xa9Y\xde\x02Z\xf6\x95*\x0e\x8d8J\x8aI\xc6\xbc\xc6\x038\x07_U\xda~\tn\xe2\x7f^\xd0E \x0fYv\x10\xd6\xa0$\xf0-\xde6\xf2l)9"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Atos TrustedRoot Root CA RSA TLS 2021 O=Atos
   * Subject: CN=Atos TrustedRoot Root CA RSA TLS 2021 O=Atos
   * Label: "Atos TrustedRoot Root CA RSA TLS 2021"
   * Serial: 111436099570196163832749341232207667876
   * SHA256 Fingerprint: 81:a9:08:8e:a5:9f:b3:64:c5:48:a6:f8:55:59:09:9b:6f:04:05:ef:bf:18:e5:32:4e:c9:f4:57:ba:00:11:2f
   * -----BEGIN CERTIFICATE-----
   * MIIFZDCCA0ygAwIBAgIQU9XP5hmTC/srBRLYwiqipDANBgkqhkiG9w0BAQwFADBM
   * MS4wLAYDVQQDDCVBdG9zIFRydXN0ZWRSb290IFJvb3QgQ0EgUlNBIFRMUyAyMDIx
   * MQ0wCwYDVQQKDARBdG9zMQswCQYDVQQGEwJERTAeFw0yMTA0MjIwOTIxMTBaFw00
   * MTA0MTcwOTIxMDlaMEwxLjAsBgNVBAMMJUF0b3MgVHJ1c3RlZFJvb3QgUm9vdCBD
   * QSBSU0EgVExTIDIwMjExDTALBgNVBAoMBEF0b3MxCzAJBgNVBAYTAkRFMIICIjAN
   * BgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAtoAOxHm9BYx9sKOdTSJNy/BBl01Z
   * 4NH+VoyX8te9j2y3I49f1cTYQcvyAh5x5en2XssIKl4w8i1mx4QbZFc4nXUtVsYv
   * Ye+W/CBGvevUez8/fEc4BKkbqlLfEzfTFRVOvV98r61jx3ncCHvVoOX3W3WsgFWZ
   * kmGbzSoXfduP9LVq6hdKZChmFSlsAvFr1bqjM9xaZ6cF4r9lthawEO3NUDPJcFDs
   * GY6wx/J0W2tExn2WuZgIWWbeKQGb9Cpt0xU6kGpn8bRrZtkh68rZYnxGEFzedUln
   * nkL5/nWpo63/dgpnQOPF943HhZpZnmKaau1Fh5hnstVKPNe0OwANwI8f4UDErmwh
   * 3El+fsqyjW22v5MvoVw+j8rtgI5Y4dtXz4U2OLJxpAmMkokIiEjxQGMYsluMWuPD
   * 0xeqqxmjLBvk1cbiZnrXghmmOxYsL3GHX0WelXOTwkKBIROW1527k2gV+p2kHYzy
   * geBYBr3JtuP2iV2J+axEoctr+hbxx1A9JNr3w+SH1VbxT5Aw+kUJWdo0zuATHAR8
   * ANSbhqRAvNncTFd+rrcztl524WWLZt+NyteYr842mIycg5kDcPOvdO3GDjbnvezB
   * c6eUWsuSZIKmAMFwoW4sKeFYV+xafJlrJaSQOoD0IJ2azsct+bJLKZWD6TWNp0lI
   * pw9MGZHQ9b8Q4HECAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQU
   * dEmZ0f+0emhFdcN+tNzMzjkz2ggwDgYDVR0PAQH/BAQDAgGGMA0GCSqGSIb3DQEB
   * DAUAA4ICAQAjQ1MkYlxt/T7Cz1UAbMVWiLkO3TriJQ2VSpfKgInuKs1l+NsW4AmS
   * 4BjHeJi78+xCUvuppILXTdiK/ORO/auQxDh1MoSf/7OwKwIzNsAQkG8dnK/haZPs
   * o0UvFJ/1TCplQ3IM98P4lYsU84UgYt1UU90s3BiVaU+DR3BAM1h3Egyi61IxHkzJ
   * qM7F78PRreBrAwA0JrRUITWXAdxfG/F851X6LWh3e9NpzNMOa7pNdkTWwhWaJuyw
   * xfW70Xp0wmzNxbVe9kzmWy2B27O3Opee7c9GslA9hGCZcbUztVdF5kJHdWoOsAgM
   * rr3e97sPWD2PAzHoPYJQyi9eDF20l74gNAf0xBLh7tew2VktafcxBPTy+av5EzH4
   * AXcOPUIjJsyacmdRIXrMPIWo6iFqO9taPKU0nprALN+AnCng33eU0aKAQv9qTFsR
   * 0PXNor6uzFFcw9VUewyu1rkGd4Di7wcaaMxZUa1+XGdrudviB0JbuAEFWDlN5LuY
   * o7Ey7Nmj1m+UI/87tyll5gfp77YZ6ufCOB0yiJA8EytuzO+rdwY0d4RPcuSBhPm5
   * dDTedk+SKlOxJTnbPP/lPqYO5Wue/9vsL3SD3460s6neFE3/MaNFcyT6lSnMEpcE
   * oji2jbDwN/zIIX8/syQbPYtuzE2wFg2WHYMfRsCbvUOZ58SWLs5fyQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1.0,\x06\x03U\x04\x03\x0c%Atos TrustedRoot Root CA RSA TLS 20211\r0\x0b\x06\x03U\x04\n\x0c\x04Atos1\x0b0\t\x06\x03U\x04\x06\x13\x02DE"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb6\x80\x0e\xc4y\xbd\x05\x8c}\xb0\xa3\x9dM\"M\xcb\xf0A\x97MY\xe0\xd1\xfeV\x8c\x97\xf2\xd7\xbd\x8fl\xb7#\x8f_\xd5\xc4\xd8A\xcb\xf2\x02\x1eq\xe5\xe9\xf6^\xcb\x08*^0\xf2-f\xc7\x84\x1bdW8\x9du-V\xc6/a\xef\x96\xfc F\xbd\xeb\xd4{??|G8\x04\xa9\x1b\xaaR\xdf\x137\xd3\x15\x15N\xbd_|\xaf\xadc\xc7y\xdc\x08{\xd5\xa0\xe5\xf7[u\xac\x80U\x99\x92a\x9b\xcd*\x17}\xdb\x8f\xf4\xb5j\xea\x17Jd(f\x15)l\x02\xf1k\xd5\xba\xa33\xdcZg\xa7\x05\xe2\xbfe\xb6\x16\xb0\x10\xed\xcdP3\xc9pP\xec\x19\x8e\xb0\xc7\xf2t[kD\xc6}\x96\xb9\x98\x08Yf\xde)\x01\x9b\xf4*m\xd3\x15:\x90jg\xf1\xb4kf\xd9!\xeb\xca\xd9b|F\x10\\\xdeuIg\x9eB\xf9\xfeu\xa9\xa3\xad\xffv\ng@\xe3\xc5\xf7\x8d\xc7\x85\x9aY\x9eb\x9aj\xedE\x87\x98g\xb2\xd5J<\xd7\xb4;\x00\r\xc0\x8f\x1f\xe1@\xc4\xael!\xdcI~~\xca\xb2\x8dm\xb6\xbf\x93/\xa1\\>\x8f\xca\xed\x80\x8eX\xe1\xdbW\xcf\x8568\xb2q\xa4\t\x8c\x92\x89\x08\x88H\xf1@c\x18\xb2[\x8cZ\xe3\xc3\xd3\x17\xaa\xab\x19\xa3,\x1b\xe4\xd5\xc6\xe2fz\xd7\x82\x19\xa6;\x16,/q\x87_E\x9e\x95s\x93\xc2B\x81!\x13\x96\xd7\x9d\xbb\x93h\x15\xfa\x9d\xa4\x1d\x8c\xf2\x81\xe0X\x06\xbd\xc9\xb6\xe3\xf6\x89]\x89\xf9\xacD\xa1\xcbk\xfa\x16\xf1\xc7P=$\xda\xf7\xc3\xe4\x87\xd5V\xf1O\x900\xfaE\tY\xda4\xce\xe0\x13\x1c\x04|\x00\xd4\x9b\x86\xa4@\xbc\xd9\xdcLW~\xae\xb73\xb6^v\xe1e\x8bf\xdf\x8d\xca\xd7\x98\xaf\xce6\x98\x8c\x9c\x83\x99\x03p\xf3\xaft\xed\xc6\x0e6\xe7\xbd\xec\xc1s\xa7\x94Z\xcb\x92d\x82\xa6\x00\xc1p\xa1n,)\xe1XW\xecZ|\x99k%\xa4\x90:\x80\xf4 \x9d\x9a\xce\xc7-\xf9\xb2K)\x95\x83\xe95\x8d\xa7IH\xa7\x0fL\x19\x91\xd0\xf5\xbf\x10\xe0q\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=OISTE WISeKey Global Root GC CA O=WISeKey OU=OISTE Foundation Endorsed
   * Subject: CN=OISTE WISeKey Global Root GC CA O=WISeKey OU=OISTE Foundation Endorsed
   * Label: "OISTE WISeKey Global Root GC CA"
   * Serial: 44084345621038548146064804565436152554
   * SHA256 Fingerprint: 85:60:f9:1c:36:24:da:ba:95:70:b5:fe:a0:db:e3:6f:f1:1a:83:23:be:94:86:85:4f:b3:f3:4a:55:71:19:8d
   * -----BEGIN CERTIFICATE-----
   * MIICaTCCAe+gAwIBAgIQISpWDK7aDKtARb8roi066jAKBggqhkjOPQQDAzBtMQsw
   * CQYDVQQGEwJDSDEQMA4GA1UEChMHV0lTZUtleTEiMCAGA1UECxMZT0lTVEUgRm91
   * bmRhdGlvbiBFbmRvcnNlZDEoMCYGA1UEAxMfT0lTVEUgV0lTZUtleSBHbG9iYWwg
   * Um9vdCBHQyBDQTAeFw0xNzA1MDkwOTQ4MzRaFw00MjA1MDkwOTU4MzNaMG0xCzAJ
   * BgNVBAYTAkNIMRAwDgYDVQQKEwdXSVNlS2V5MSIwIAYDVQQLExlPSVNURSBGb3Vu
   * ZGF0aW9uIEVuZG9yc2VkMSgwJgYDVQQDEx9PSVNURSBXSVNlS2V5IEdsb2JhbCBS
   * b290IEdDIENBMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAETOlQwMYPchi82PG6s4ni
   * eUqjFqdrVCTbUf/q9Akkwwsin8tqJ4KBDdLArzHkdIJuyiXZjHWd8dvQmqJLIX4W
   * p2OQ0jnUsYd4XxiWD1AbNTcPasbc2RNNpI6QN+a9WzGRo1QwUjAOBgNVHQ8BAf8E
   * BAMCAQYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUSIcUrOPDnpBgOtfKie7T
   * rYy0UGYwEAYJKwYBBAGCNxUBBAMCAQAwCgYIKoZIzj0EAwMDaAAwZQIwJsdpW9zV
   * 57LnyAyMjMPdeYwbY9XJUpROTYJKcx6ygISpJcBMWm1JKWB4E+J+SOtkAjEA2zQg
   * Mgj/mkkCtojeFK9dbJlxjRo/i9fgojaGHAeCOnZT/cKi7e97sIBPWA9LUzm9
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CH1\x100\x0e\x06\x03U\x04\n\x13\x07WISeKey1\"0 \x06\x03U\x04\x0b\x13\x19OISTE Foundation Endorsed1(0&\x06\x03U\x04\x03\x13\x1fOISTE WISeKey Global Root GC CA"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04L\xe9P\xc0\xc6\x0fr\x18\xbc\xd8\xf1\xba\xb3\x89\xe2yJ\xa3\x16\xa7kT$\xdbQ\xff\xea\xf4\t$\xc3\x0b\"\x9f\xcbj\'\x82\x81\r\xd2\xc0\xaf1\xe4t\x82n\xca%\xd9\x8cu\x9d\xf1\xdb\xd0\x9a\xa2K!~\x16\xa7c\x90\xd29\xd4\xb1\x87x_\x18\x96\x0fP\x1b57\x0fj\xc6\xdc\xd9\x13M\xa4\x8e\x907\xe6\xbd[1\x91"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com Root Certification Authority RSA O=SSL Corporation
   * Subject: CN=SSL.com Root Certification Authority RSA O=SSL Corporation
   * Label: "SSL.com Root Certification Authority RSA"
   * Serial: 8875640296558310041
   * SHA256 Fingerprint: 85:66:6a:56:2e:e0:be:5c:e9:25:c1:d8:89:0a:6f:76:a8:7e:c1:6d:4d:7d:5f:29:ea:74:19:cf:20:12:3b:69
   * -----BEGIN CERTIFICATE-----
   * MIIF3TCCA8WgAwIBAgIIeyyb0xaAMpkwDQYJKoZIhvcNAQELBQAwfDELMAkGA1UE
   * BhMCVVMxDjAMBgNVBAgMBVRleGFzMRAwDgYDVQQHDAdIb3VzdG9uMRgwFgYDVQQK
   * DA9TU0wgQ29ycG9yYXRpb24xMTAvBgNVBAMMKFNTTC5jb20gUm9vdCBDZXJ0aWZp
   * Y2F0aW9uIEF1dGhvcml0eSBSU0EwHhcNMTYwMjEyMTczOTM5WhcNNDEwMjEyMTcz
   * OTM5WjB8MQswCQYDVQQGEwJVUzEOMAwGA1UECAwFVGV4YXMxEDAOBgNVBAcMB0hv
   * dXN0b24xGDAWBgNVBAoMD1NTTCBDb3Jwb3JhdGlvbjExMC8GA1UEAwwoU1NMLmNv
   * bSBSb290IENlcnRpZmljYXRpb24gQXV0aG9yaXR5IFJTQTCCAiIwDQYJKoZIhvcN
   * AQEBBQADggIPADCCAgoCggIBAPkP3aMrfcvQKv7sZ4Wm5y4bunfh4/WvpOz6Sl2R
   * xFdHaxh3a3by/ZPkPQ/CFp4LZsNWlJ4Xg4XOVu/yFv0AYvUiCVToZRdOQbngT0aX
   * qhvIuG5iXmmxX9sqAn78bMrzQdjt0Oj8P2FI7bADFB0QDksZ4LtO7IZl/zbzXmcC
   * C52GVWH9ejjt/uIZALdvoVBidXQ8oPrIJZK0bnoix/geoeOy3ZExqysdBP+lSgQ3
   * 6YWkMyv94tZVNHwZpEpox7Ko07fKoZOI68GXvIz5HdkihCR0xwQ9aqkpk8zruFvh
   * /l8lqjRYyMEjVJ0bmBHDOJx+PYZspQ9AhnwC9FwCTyjLrnGfDzrIM/4RJTXq/LrF
   * YD3ZfBjVsqnTdXgDciLKOsMf7yzlLqn6niy2UUb9rwPW6mBo6oUWNmuF6R7As93E
   * JNyAKoFBbZQ+yODJgUEAnl6/f8UImKIYLEJAs/lvOCdLToD0PYFH4Ih86hzOtXVc
   * US4cK38acijnALXRdMbX5J+tB5O2UzU1/Dfkw/ZdFr4hc96SCvigY2q8lpJqPvi8
   * ZVWb3vUNiSYE/CUapiVpy8JtynziWV+XrOvvLsi81xtZPCvM8hnIk2snYxnP/Okm
   * +Mpxm3+T/jRnhE6Z6/yzeAkzcLpmpnbtG3PrGqUNxCITIJRWCk4sbE6x/c+cCbqi
   * M+2HAgMBAAGjYzBhMB0GA1UdDgQWBBTdBAkHovV6fVJTEpKV7jiAJQ2mWTAPBgNV
   * HRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFN0ECQei9Xp9UlMSkpXuOIAlDaZZMA4G
   * A1UdDwEB/wQEAwIBhjANBgkqhkiG9w0BAQsFAAOCAgEAIBgRlCn7Jp0cHh5wYfGV
   * cpNxJK1ok1iOMq8bs3AD/CUrdIWQPXhq9LmLpZc7tRiRux6n+UBbkflVma8eEdBc
   * Hadm47GUBwwyOabqG7B52B2ccETjit3E+ZUfijhDPwGFpUenPUayvOUiaPd7nNgs
   * PgohyC0zrL/FgZkxdMF1ccW+sfAjRfSda/wZY52jvATGGAslu1OJD7OAUN5F7kR/
   * q5R4ZJjT9ijdh9hwZXT7DrkT66cPYakylszeu+1jTBi7qUD3oFRuIIhxdRjqerQ0
   * cuAjJ3dctpDqhiVAq+8zD8ufgr6iIPv2tS0a5sKFsXQP+8hlAqRSAUfdSSLBv9jr
   * a6x+3uxjMxW3IwiPxg+NQVrdjsW5j+VFP3jbutIbQLH+cU0/4IGiul607BXgk90I
   * H37hVZkLId6Tngr75qNJvTYw/ud3sqB1l7UtgYgXZSD32pAAn8lSzDLKNXz1PQ/Y
   * K9f1JmzJBjSWFupwWRoyeXkLtoh/D1JIPb9s2KJELtFOt3JY04kTlf5Eq/jXixtu
   * nLwsoFvVagCvXzfh1foQC5ichucmj87w7G6KVwuA406ywKBjYZC6VWg3dGq2ktuf
   * oYYitmUnDuy2n0Jg5GfCtdpBC8TTi2EbvPofkSvXRAdeuims2cXp71NIWuuA8ShY
   * Ic2wBlX7Jz9TkHCpBB5XJ7k=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0e0\x0c\x06\x03U\x04\x08\x0c\x05Texas1\x100\x0e\x06\x03U\x04\x07\x0c\x07Houston1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation110/\x06\x03U\x04\x03\x0c(SSL.com Root Certification Authority RSA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xf9\x0f\xdd\xa3+}\xcb\xd0*\xfe\xecg\x85\xa6\xe7.\x1b\xbaw\xe1\xe3\xf5\xaf\xa4\xec\xfaJ]\x91\xc4WGk\x18wkv\xf2\xfd\x93\xe4=\x0f\xc2\x16\x9e\x0bf\xc3V\x94\x9e\x17\x83\x85\xceV\xef\xf2\x16\xfd\x00b\xf5\"\tT\xe8e\x17NA\xb9\xe0OF\x97\xaa\x1b\xc8\xb8nb^i\xb1_\xdb*\x02~\xfcl\xca\xf3A\xd8\xed\xd0\xe8\xfc?aH\xed\xb0\x03\x14\x1d\x10\x0eK\x19\xe0\xbbN\xec\x86e\xff6\xf3^g\x02\x0b\x9d\x86Ua\xfdz8\xed\xfe\xe2\x19\x00\xb7o\xa1Pbut<\xa0\xfa\xc8%\x92\xb4nz\"\xc7\xf8\x1e\xa1\xe3\xb2\xdd\x911\xab+\x1d\x04\xff\xa5J\x047\xe9\x85\xa43+\xfd\xe2\xd6U4|\x19\xa4Jh\xc7\xb2\xa8\xd3\xb7\xca\xa1\x93\x88\xeb\xc1\x97\xbc\x8c\xf9\x1d\xd9\"\x84$t\xc7\x04=j\xa9)\x93\xcc\xeb\xb8[\xe1\xfe_%\xaa4X\xc8\xc1#T\x9d\x1b\x98\x11\xc38\x9c~=\x86l\xa5\x0f@\x86|\x02\xf4\\\x02O(\xcb\xaeq\x9f\x0f:\xc83\xfe\x11%5\xea\xfc\xba\xc5`=\xd9|\x18\xd5\xb2\xa9\xd3ux\x03r\"\xca:\xc3\x1f\xef,\xe5.\xa9\xfa\x9e,\xb6QF\xfd\xaf\x03\xd6\xea`h\xea\x85\x166k\x85\xe9\x1e\xc0\xb3\xdd\xc4$\xdc\x80*\x81Am\x94>\xc8\xe0\xc9\x81A\x00\x9e^\xbf\x7f\xc5\x08\x98\xa2\x18,B@\xb3\xf9o8\'KN\x80\xf4=\x81G\xe0\x88|\xea\x1c\xce\xb5u\\Q.\x1c+\x7f\x1ar(\xe7\x00\xb5\xd1t\xc6\xd7\xe4\x9f\xad\x07\x93\xb6S55\xfc7\xe4\xc3\xf6]\x16\xbe!s\xde\x92\n\xf8\xa0cj\xbc\x96\x92j>\xf8\xbceU\x9b\xde\xf5\r\x89&\x04\xfc%\x1a\xa6%i\xcb\xc2m\xca|\xe2Y_\x97\xac\xeb\xef.\xc8\xbc\xd7\x1bY<+\xcc\xf2\x19\xc8\x93k\'c\x19\xcf\xfc\xe9&\xf8\xcaq\x9b\x7f\x93\xfe4g\x84N\x99\xeb\xfc\xb3x\t3p\xbaf\xa6v\xed\x1bs\xeb\x1a\xa5\r\xc4\"\x13 \x94V\nN,lN\xb1\xfd\xcf\x9c\t\xba\xa23\xed\x87\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=QuoVadis Root CA 2 O=QuoVadis Limited
   * Subject: CN=QuoVadis Root CA 2 O=QuoVadis Limited
   * Label: "QuoVadis Root CA 2"
   * Serial: 1289
   * SHA256 Fingerprint: 85:a0:dd:7d:d7:20:ad:b7:ff:05:f8:3d:54:2b:20:9d:c7:ff:45:28:f7:d6:77:b1:83:89:fe:a5:e5:c4:9e:86
   * -----BEGIN CERTIFICATE-----
   * MIIFtzCCA5+gAwIBAgICBQkwDQYJKoZIhvcNAQEFBQAwRTELMAkGA1UEBhMCQk0x
   * GTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxGzAZBgNVBAMTElF1b1ZhZGlzIFJv
   * b3QgQ0EgMjAeFw0wNjExMjQxODI3MDBaFw0zMTExMjQxODIzMzNaMEUxCzAJBgNV
   * BAYTAkJNMRkwFwYDVQQKExBRdW9WYWRpcyBMaW1pdGVkMRswGQYDVQQDExJRdW9W
   * YWRpcyBSb290IENBIDIwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCa
   * GMpLlA0ALa8DKYrwD4HIrkwZhR0In6spRIXzL4GtMh6QRr+jhiYaHv5+HBg6XJxg
   * Fyo6dIMzMH1hVBHL7avg5tKifvVrbxi3Cgst/ek+7wrGsxDp3MJGF/hd/aTa/55J
   * WpzmM+Yklvc/ulsrHHo1wtZn/qtmUIttKGAr79dgw8eTvI02kfN/+NsRE8Scd3bB
   * rrcCaoF6qUWD4gXmuVbBlDePSHFjIuwXZQeVikvfj8ZaCuWw419eaxGrDPmF60Tp
   * +ARz8un+XJiM9XOva7R+zdRcAitMOeGylZUtQofX1bOQQ7dsE/He3fbE+Ik/0XX1
   * ksOR1YqI0JDs3G3eicJlcZaLDQP9nL9bFqyS2+r+eXyt66/3FsvbzSUr5R/7mp/i
   * Ucw6UwxI5g69ybR2BlLmEROFcmMDBOAENisgGQLodKcftslWZvB1JdxnwQ5hYIiz
   * PtGo/KPaHbDRsSNU30R2be1B2MGyIrZTHN81Hdyhdyox5C315eXbyOD/5YDXC2Og
   * /zOhD7osFRXql7PSorW+8oyWHhqPHWykYTe5hnMz15eWniN9gqRMgeKh0bpnX5UH
   * oycR7hYQe7xFSkyyBNKr79X9DFHOUGoIMfmR2gyPZFwDwzqLID9ujWc9Otb+fVuI
   * yV77zGHcizN300QyNQliBJIWENieJ0f7OyHj+OsdWwIDAQABo4GwMIGtMA8GA1Ud
   * EwEB/wQFMAMBAf8wCwYDVR0PBAQDAgEGMB0GA1UdDgQWBBQahGK8SEwzJQTU7tD2
   * A8QZRtGUazBuBgNVHSMEZzBlgBQahGK8SEwzJQTU7tD2A8QZRtGUa6FJpEcwRTEL
   * MAkGA1UEBhMCQk0xGTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxGzAZBgNVBAMT
   * ElF1b1ZhZGlzIFJvb3QgQ0EgMoICBQkwDQYJKoZIhvcNAQEFBQADggIBAD4KFk2f
   * BluornFdLwUvZ+YTRYPENvbzwCYMDbVHZF34tHLJRqUDGCdViXh9duqWNIAXINzn
   * g/iN/Ae42l9NLmeyhP3ZRPx3UIHmfLTJDQtyU/h2BwdBR5YM++CCJpNVjP4iH2Bl
   * fF/nJrP3MpCYUNQ3cVX2kiF495V5+vgtJodmVjB3pjd4M1IQWK4/YY7yarHvGH5K
   * WWPKjaJW1acvvFYfzznB4vsKqBUsfU16Y8Zsl0Q80m/DShcK+JDSV6IZUaUtl0Ha
   * B0+pUNqQjZRG4T7wlP0QADj1O+hA4bRuVhogzG9Yje0uRY/W6ZM/57Es3zrWIozc
   * hLsib9D45MY56QSIPMO661V6bYCZJPVsAfv4l7CUW+v90m/xd2gNNWQjrLhVoQPR
   * TUIZ3Ph1WVaj+ahJefivDrkRoHy3au000LYmYjgahwz46P0u05B/B5EqHdZ+XIWD
   * mbA4CD/pXvk1B+TJYm5Xf6dQlfe6yJvmjqIBxdZmv3lh8zwc4bmCXF2gw+nYSL0Z
   * ohEUGW6yhhtoPkg3Goi3XZZenMfvJ2II4pEZXNLxId26F0KCl3GBUzGpn/Z9Yr9y
   * 4aOTHcyKJloJONDO1w2AFrR4pTqHTI2KpdVGl/IsELm8VCLAAVBpQ570su9t+Oza
   * 8eOx79+Rj1QqCyXBJhnEUhAFZdWCEOrCMc0u
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BM1\x190\x17\x06\x03U\x04\n\x13\x10QuoVadis Limited1\x1b0\x19\x06\x03U\x04\x03\x13\x12QuoVadis Root CA 2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x9a\x18\xcaK\x94\r\x00-\xaf\x03)\x8a\xf0\x0f\x81\xc8\xaeL\x19\x85\x1d\x08\x9f\xab)D\x85\xf3/\x81\xad2\x1e\x90F\xbf\xa3\x86&\x1a\x1e\xfe~\x1c\x18:\\\x9c`\x17*:t\x8330}aT\x11\xcb\xed\xab\xe0\xe6\xd2\xa2~\xf5ko\x18\xb7\n\x0b-\xfd\xe9>\xef\n\xc6\xb3\x10\xe9\xdc\xc2F\x17\xf8]\xfd\xa4\xda\xff\x9eIZ\x9c\xe63\xe6$\x96\xf7?\xba[+\x1cz5\xc2\xd6g\xfe\xabfP\x8bm(`+\xef\xd7`\xc3\xc7\x93\xbc\x8d6\x91\xf3\x7f\xf8\xdb\x11\x13\xc4\x9cwv\xc1\xae\xb7\x02j\x81z\xa9E\x83\xe2\x05\xe6\xb9V\xc1\x947\x8fHqc\"\xec\x17e\x07\x95\x8aK\xdf\x8f\xc6Z\n\xe5\xb0\xe3_^k\x11\xab\x0c\xf9\x85\xebD\xe9\xf8\x04s\xf2\xe9\xfe\\\x98\x8c\xf5s\xafk\xb4~\xcd\xd4\\\x02+L9\xe1\xb2\x95\x95-B\x87\xd7\xd5\xb3\x90C\xb7l\x13\xf1\xde\xdd\xf6\xc4\xf8\x89?\xd1u\xf5\x92\xc3\x91\xd5\x8a\x88\xd0\x90\xec\xdcm\xde\x89\xc2eq\x96\x8b\r\x03\xfd\x9c\xbf[\x16\xac\x92\xdb\xea\xfey|\xad\xeb\xaf\xf7\x16\xcb\xdb\xcd%+\xe5\x1f\xfb\x9a\x9f\xe2Q\xcc:S\x0cH\xe6\x0e\xbd\xc9\xb4v\x06R\xe6\x11\x13\x85rc\x03\x04\xe0\x046+ \x19\x02\xe8t\xa7\x1f\xb6\xc9Vf\xf0u%\xdcg\xc1\x0ea`\x88\xb3>\xd1\xa8\xfc\xa3\xda\x1d\xb0\xd1\xb1#T\xdfDvm\xedA\xd8\xc1\xb2\"\xb6S\x1c\xdf5\x1d\xdc\xa1w*1\xe4-\xf5\xe5\xe5\xdb\xc8\xe0\xff\xe5\x80\xd7\x0bc\xa0\xff3\xa1\x0f\xba,\x15\x15\xea\x97\xb3\xd2\xa2\xb5\xbe\xf2\x8c\x96\x1e\x1a\x8f\x1dl\xa4a7\xb9\x86s3\xd7\x97\x96\x9e#}\x82\xa4L\x81\xe2\xa1\xd1\xbag_\x95\x07\xa3\'\x11\xee\x16\x10{\xbcEJL\xb2\x04\xd2\xab\xef\xd5\xfd\x0cQ\xcePj\x081\xf9\x91\xda\x0c\x8fd\\\x03\xc3:\x8b ?n\x8dg=:\xd6\xfe}[\x88\xc9^\xfb\xcca\xdc\x8b3w\xd3D25\tb\x04\x92\x16\x10\xd8\x9e\'G\xfb;!\xe3\xf8\xeb\x1d[\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=emSign ECC Root CA - G3 O=eMudhra Technologies Limited OU=emSign PKI
   * Subject: CN=emSign ECC Root CA - G3 O=eMudhra Technologies Limited OU=emSign PKI
   * Label: "emSign ECC Root CA - G3"
   * Serial: 287880440101571086945156
   * SHA256 Fingerprint: 86:a1:ec:ba:08:9c:4a:8d:3b:be:27:34:c6:12:ba:34:1d:81:3e:04:3c:f9:e8:a8:62:cd:5c:57:a3:6b:be:6b
   * -----BEGIN CERTIFICATE-----
   * MIICTjCCAdOgAwIBAgIKPPYHqWhwDtqLhDAKBggqhkjOPQQDAzBrMQswCQYDVQQG
   * EwJJTjETMBEGA1UECxMKZW1TaWduIFBLSTElMCMGA1UEChMcZU11ZGhyYSBUZWNo
   * bm9sb2dpZXMgTGltaXRlZDEgMB4GA1UEAxMXZW1TaWduIEVDQyBSb290IENBIC0g
   * RzMwHhcNMTgwMjE4MTgzMDAwWhcNNDMwMjE4MTgzMDAwWjBrMQswCQYDVQQGEwJJ
   * TjETMBEGA1UECxMKZW1TaWduIFBLSTElMCMGA1UEChMcZU11ZGhyYSBUZWNobm9s
   * b2dpZXMgTGltaXRlZDEgMB4GA1UEAxMXZW1TaWduIEVDQyBSb290IENBIC0gRzMw
   * djAQBgcqhkjOPQIBBgUrgQQAIgNiAAQjpQy4LRL1KPOxst3iAhKAnjlfSU2fySU0
   * WXTsuwYc58Byr+iuL+FBVIcUqEqy6HyC5ltqtdyzdc6LBtCGI79G1Y4PPwT01xyS
   * fvalY8L1X44uT6EYGQIrMgqCZH0Wk9GjQjBAMB0GA1UdDgQWBBR8XQKEE9TMipuB
   * zhccLikenEhjQjAOBgNVHQ8BAf8EBAMCAQYwDwYDVR0TAQH/BAUwAwEB/zAKBggq
   * hkjOPQQDAwNpADBmAjEAvvNhzwIQHWSVB7gYboiFBS+DCBeQyh+KTOgNG3qxrdWB
   * CUfvO6wIBHxcmbHtRwfSAjEAnbpV/KlK6O3t5nYBQnvI+GDZjVGLVTv7jHvrZQnD
   * +JbNR6iC8hZVdyR+EhCVBCyj
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02IN1\x130\x11\x06\x03U\x04\x0b\x13\nemSign PKI1%0#\x06\x03U\x04\n\x13\x1ceMudhra Technologies Limited1 0\x1e\x06\x03U\x04\x03\x13\x17emSign ECC Root CA - G3"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04#\xa5\x0c\xb8-\x12\xf5(\xf3\xb1\xb2\xdd\xe2\x02\x12\x80\x9e9_IM\x9f\xc9%4Yt\xec\xbb\x06\x1c\xe7\xc0r\xaf\xe8\xae/\xe1AT\x87\x14\xa8J\xb2\xe8|\x82\xe6[j\xb5\xdc\xb3u\xce\x8b\x06\xd0\x86#\xbfF\xd5\x8e\x0f?\x04\xf4\xd7\x1c\x92~\xf6\xa5c\xc2\xf5_\x8e.O\xa1\x18\x19\x02+2\n\x82d}\x16\x93\xd1"),
    name_constraints: None
  },

  /*
   * Issuer: CN=QuoVadis Root CA 3 G3 O=QuoVadis Limited
   * Subject: CN=QuoVadis Root CA 3 G3 O=QuoVadis Limited
   * Label: "QuoVadis Root CA 3 G3"
   * Serial: 268090761170461462463995952157327242137089239581
   * SHA256 Fingerprint: 88:ef:81:de:20:2e:b0:18:45:2e:43:f8:64:72:5c:ea:5f:bd:1f:c2:d9:d2:05:73:07:09:c5:d8:b8:69:0f:46
   * -----BEGIN CERTIFICATE-----
   * MIIFYDCCA0igAwIBAgIULvWbAiin23r/1aOp7r0DoM8Sah0wDQYJKoZIhvcNAQEL
   * BQAwSDELMAkGA1UEBhMCQk0xGTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxHjAc
   * BgNVBAMTFVF1b1ZhZGlzIFJvb3QgQ0EgMyBHMzAeFw0xMjAxMTIyMDI2MzJaFw00
   * MjAxMTIyMDI2MzJaMEgxCzAJBgNVBAYTAkJNMRkwFwYDVQQKExBRdW9WYWRpcyBM
   * aW1pdGVkMR4wHAYDVQQDExVRdW9WYWRpcyBSb290IENBIDMgRzMwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQCzyw4QZ47qFJenMioKVjZ/aEzHs286IxSR
   * /xl/pcqs7rN2nXrpixurazHb+gtTTK/FpRp5PIpM/6zfJd5O2YIyC0TeytuMrKNu
   * FoM7pmRLMon7FhY4futD4tN0SsJiCnMK3UmzV9KwCoWdcTzeo8vAMvMBOSBDGzXR
   * U7Ox7sWTaYI+FrUoRqHe6okJ7UO4BUaKhvVZR74bbwEhELn9qdIoyhA5CcoTNs+c
   * ra1AdHkrAj80//ogaX3T7mH1urPnMNA3I4ZyYUUpSFlob3emLoG+B01vr87ERROR
   * FHAGjx+f+IdpsQ7vw4kZ6+ocYfx6bIrc1gMLnia6Et3UVDmrJqMz6nWB2i3ND0/k
   * A9HvFZcba5DFApCTZgIhsUfei5pKgLlVj7WiL8DWM2fafsSntARE60f75li59wzw
   * eyuxwHApw0BiLTtIadwjPEjrewl5qW3aqDCYz4ByA4imW0aucnl8CAMhZa634Ryl
   * sSqiMd5mBPfAdOhx3v89WcyWJhKLhZVXGqtrdQtEPREoPHtht+KPZ0/l7DxMYIBp
   * VzgeAVuNVejH38DMdyM0SXV89pgR6y3e7UEuFAUCf+D+IOs15xGsIs5XPd7JMG0Q
   * A4XN8f+MFrXBsj6IbGB/kE+V9/YtrQE5BwT6dYB9v0lQ7e/JxHwc64B+27bQ3RP+
   * ydOc17KXqQIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIB
   * BjAdBgNVHQ4EFgQUxhfQvKjqAkPyGwaZXSuQILnXnOQwDQYJKoZIhvcNAQELBQAD
   * ggIBADRh2Va1EodVTd2jNTFGu6QHcrxfYWLopfsLN7E8trP6KZ1/AvWkyaiTt3px
   * KGmPc+FSkNrVvjrlt3ZqVoAh313m6Tqe5T72omnHKgqwGEfcIHB9UqM+WXzBusnI
   * FUBhynLWcKzSt/Ac5IYp8M7vaGPQtSCKFWGafoaYtMnCdvvMujAWzKNhxnQT5Wvv
   * oxXqA/4Ti2Tk08HS6IT7SdEQTXlm66r99I0xHnAUrdzeZxNMgRVhvLfZkXdxGYFg
   * u/BYpbWcC/ePIlUnwEsBbTuZDdQdm2NnL9DuDcpmvJRPpq3t/O5jrFc/ZSXPsoaP
   * 0Aj/uHYUbt7lJ+yreLVTubY/6CD50qi+YUbKh4yE8/nxoGibIh6BJpsQBJFxwAYf
   * 3KDTuVan45gtf4Od34wrnDKOMpTwATwiKp9Dwi7DmDkHOHv8XgBCH/MyJnmDhPbl
   * 8MFREsALHgQjDFSlTC9JxUrRtm5gDWv8a4uFJGS3iQ6rJUdbPM9+Sb3H6QrG2vd+
   * DhcI00iX0HGS8A85PjRqHH3Y8iKuu2n0M7SmSFXRDw4m6Oy2Cy2nhTXN/VnIn9HN
   * PlopNLk9hM6xZdRZkZFWdSHBd575euFgndOtBBj0fOtek49TSiIp+EgrPk2GrFt/
   * ywaZWWDYWGWVjUTR939+J399roD1B0y2PpxxVJkES/1Y+Zj0
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BM1\x190\x17\x06\x03U\x04\n\x13\x10QuoVadis Limited1\x1e0\x1c\x06\x03U\x04\x03\x13\x15QuoVadis Root CA 3 G3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb3\xcb\x0e\x10g\x8e\xea\x14\x97\xa72*\nV6\x7fhL\xc7\xb3o:#\x14\x91\xff\x19\x7f\xa5\xca\xac\xee\xb3v\x9dz\xe9\x8b\x1b\xabk1\xdb\xfa\x0bSL\xaf\xc5\xa5\x1ay<\x8aL\xff\xac\xdf%\xdeN\xd9\x822\x0bD\xde\xca\xdb\x8c\xac\xa3n\x16\x83;\xa6dK2\x89\xfb\x16\x168~\xebC\xe2\xd3tJ\xc2b\ns\n\xddI\xb3W\xd2\xb0\n\x85\x9dq<\xde\xa3\xcb\xc02\xf3\x019 C\x1b5\xd1S\xb3\xb1\xee\xc5\x93i\x82>\x16\xb5(F\xa1\xde\xea\x89\t\xedC\xb8\x05F\x8a\x86\xf5YG\xbe\x1bo\x01!\x10\xb9\xfd\xa9\xd2(\xca\x109\t\xca\x136\xcf\x9c\xad\xad@ty+\x02?4\xff\xfa i}\xd3\xeea\xf5\xba\xb3\xe70\xd07#\x86raE)HYhow\xa6.\x81\xbe\x07Mo\xaf\xce\xc4E\x13\x91\x14p\x06\x8f\x1f\x9f\xf8\x87i\xb1\x0e\xef\xc3\x89\x19\xeb\xea\x1ca\xfczl\x8a\xdc\xd6\x03\x0b\x9e&\xba\x12\xdd\xd4T9\xab&\xa33\xeau\x81\xda-\xcd\x0fO\xe4\x03\xd1\xef\x15\x97\x1bk\x90\xc5\x02\x90\x93f\x02!\xb1G\xde\x8b\x9aJ\x80\xb9U\x8f\xb5\xa2/\xc0\xd63g\xda~\xc4\xa7\xb4\x04D\xebG\xfb\xe6X\xb9\xf7\x0c\xf0{+\xb1\xc0p)\xc3@b-;Hi\xdc#<H\xeb{\ty\xa9m\xda\xa80\x98\xcf\x80r\x03\x88\xa6[F\xaery|\x08\x03!e\xae\xb7\xe1\x1c\xa5\xb1*\xa21\xdef\x04\xf7\xc0t\xe8q\xde\xff=Y\xcc\x96&\x12\x8b\x85\x95W\x1a\xabku\x0bD=\x11(<{a\xb7\xe2\x8fgO\xe5\xec<L`\x80iW8\x1e\x01[\x8dU\xe8\xc7\xdf\xc0\xccw#4Iu|\xf6\x98\x11\xeb-\xde\xedA.\x14\x05\x02\x7f\xe0\xfe \xeb5\xe7\x11\xac\"\xceW=\xde\xc90m\x10\x03\x85\xcd\xf1\xff\x8c\x16\xb5\xc1\xb2>\x88l`\x7f\x90O\x95\xf7\xf6-\xad\x019\x07\x04\xfau\x80}\xbfIP\xed\xef\xc9\xc4|\x1c\xeb\x80~\xdb\xb6\xd0\xdd\x13\xfe\xc9\xd3\x9c\xd7\xb2\x97\xa9\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=NAVER Global Root Certification Authority O=NAVER BUSINESS PLATFORM Corp.
   * Subject: CN=NAVER Global Root Certification Authority O=NAVER BUSINESS PLATFORM Corp.
   * Label: "NAVER Global Root Certification Authority"
   * Serial: 9013692873798656336226253319739695165984492813
   * SHA256 Fingerprint: 88:f4:38:dc:f8:ff:d1:fa:8f:42:91:15:ff:e5:f8:2a:e1:e0:6e:0c:70:c3:75:fa:ad:71:7b:34:a4:9e:72:65
   * -----BEGIN CERTIFICATE-----
   * MIIFojCCA4qgAwIBAgIUAZQwHqIL3fXFMyqxQ0Rx+NZQTQ0wDQYJKoZIhvcNAQEM
   * BQAwaTELMAkGA1UEBhMCS1IxJjAkBgNVBAoMHU5BVkVSIEJVU0lORVNTIFBMQVRG
   * T1JNIENvcnAuMTIwMAYDVQQDDClOQVZFUiBHbG9iYWwgUm9vdCBDZXJ0aWZpY2F0
   * aW9uIEF1dGhvcml0eTAeFw0xNzA4MTgwODU4NDJaFw0zNzA4MTgyMzU5NTlaMGkx
   * CzAJBgNVBAYTAktSMSYwJAYDVQQKDB1OQVZFUiBCVVNJTkVTUyBQTEFURk9STSBD
   * b3JwLjEyMDAGA1UEAwwpTkFWRVIgR2xvYmFsIFJvb3QgQ2VydGlmaWNhdGlvbiBB
   * dXRob3JpdHkwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQC21PGTXLVA
   * iQqrDZBbUGOukJR0F0Vy1ntlWilLp1agS7gvQnXp2XskWjFlqxcX0TM62RHcQDaH
   * 38dq6SZeWYp34+hInDEW+j6RscrJo+KfziFTowI2MMtSAuXaMl3Dxeb57hHHi8lE
   * HoSTGEq0n+USZGnQJoViAbbJAh2+g1G7XNr4rRVqmfeSVPc0W+m/6imBEtRTkZaz
   * kVrd/pBzKPswRrXKCAfHcXLJZtM0l/aM9BhK4dA9WkW2aacp+yPOiNgSnABIqKYP
   * szuSjXEOdMWLyEz59JuOuDxp7W87UC9Y7cSw0BwbagzivESq2M0UXZR4Yb8Obtoq
   * vC8MC3GmsxY/nOb5zJ9TNeIDoKAYv7vxvvTWjIcNQvcGufFt7QSUqP620wbGQGHf
   * nZ3zVHbOUzoBppJB7ASjjw2i1QnK1sua8e9DXcCrpUHPXFNwcMmIpi3Ua2FzUCaG
   * YQ5fG8Ir4ozVu53BA0K6lNpfqbDKzE0K70dpAy8i+/Eozr9dUGWokG2zdLAIx6yo
   * 0es+nPxdGoMuK8u180SdOqcXYZaicdNwlhVNt0xz7hlcxVs+Qf6sdWA7G2POAN3a
   * CJBitOUt7kinaxeZVL6HSuOpXgRM6xBtVNbv8ejyYhbLgGvtPe31HzClrkvJE+2K
   * AQHJuFFYwGY6sWZLxNUxAmLpdIQM201GLQIDAQABo0IwQDAdBgNVHQ4EFgQU0p+I
   * 36HNLL3s9TsBAZMzJ7LrYEswDgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB/wQFMAMB
   * Af8wDQYJKoZIhvcNAQEMBQADggIBADLKgLOdPVQG3dLSLvCkASELZ0jKbY7gyKoN
   * qo0hV4/GPnrK21HUUrPUloSlWGB/5QuOH/XcChWB5Tu2tyIvCZwTFrFsDDUIbatj
   * cu3cvuzHV+YwIHHW1xDBE1UBjCpD5EHxzzp6U5LOogMFDTjfArsQLtk70pt6wKGm
   * +LUx5vR1yblTmXVHIloUFcd4G7ad6Qz4G3bxhYTeodoS76TiEJd6eN4MUZeoIUCL
   * hr0N8F5OSza7OyAfikJW4Qsav3vQIkMsRIz75Sq0bBwcupTgE34h5prCy8VCZLQe
   * lHsIJchxzIdFV4XTnyliIoNRlwAYl3dqmJLJfGBs32x9SuRwTMKeuB330DTHD8z7
   * p/8Dvq1wkNoL3chtl1+afwkyQf3NosxabUzyqkn+Zvjp2DXrDige7kgvOtB5CTh8
   * piKCk5XQA76+AqAF3SAi428diDRgxuYKuQl1C/AH6GmWNcf7I4GOODm4RStDeKLR
   * LBT/DShycpWbXgnbiUSYqqFJu3FS8r/2/yehNq+4tneI3TqkbZs0kNwUXTC/t+sX
   * 5Ie3cdCh13cV1ELX8vMxmV2b3RZtP+oGI/hGoiLtk/bdmuYqh7GYVPEi92tF4+KO
   * dh2ajcQGjTa3FPOdVGm3jjzVpG2Tgbet9r1ke8LJaDmgkpzNNIaRkPpkUZ3+/uul
   * 9XXeifdy
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02KR1&0$\x06\x03U\x04\n\x0c\x1dNAVER BUSINESS PLATFORM Corp.1200\x06\x03U\x04\x03\x0c)NAVER Global Root Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb6\xd4\xf1\x93\\\xb5@\x89\n\xab\r\x90[Pc\xae\x90\x94t\x17Er\xd6{eZ)K\xa7V\xa0K\xb8/Bu\xe9\xd9{$Z1e\xab\x17\x17\xd13:\xd9\x11\xdc@6\x87\xdf\xc7j\xe9&^Y\x8aw\xe3\xe8H\x9c1\x16\xfa>\x91\xb1\xca\xc9\xa3\xe2\x9f\xce!S\xa3\x0260\xcbR\x02\xe5\xda2]\xc3\xc5\xe6\xf9\xee\x11\xc7\x8b\xc9D\x1e\x84\x93\x18J\xb4\x9f\xe5\x12di\xd0&\x85b\x01\xb6\xc9\x02\x1d\xbe\x83Q\xbb\\\xda\xf8\xad\x15j\x99\xf7\x92T\xf74[\xe9\xbf\xea)\x81\x12\xd4S\x91\x96\xb3\x91Z\xdd\xfe\x90s(\xfb0F\xb5\xca\x08\x07\xc7qr\xc9f\xd34\x97\xf6\x8c\xf4\x18J\xe1\xd0=ZE\xb6i\xa7)\xfb#\xce\x88\xd8\x12\x9c\x00H\xa8\xa6\x0f\xb3;\x92\x8dq\x0et\xc5\x8b\xc8L\xf9\xf4\x9b\x8e\xb8<i\xedo;P/X\xed\xc4\xb0\xd0\x1c\x1bj\x0c\xe2\xbcD\xaa\xd8\xcd\x14]\x94xa\xbf\x0en\xda*\xbc/\x0c\x0bq\xa6\xb3\x16?\x9c\xe6\xf9\xcc\x9fS5\xe2\x03\xa0\xa0\x18\xbf\xbb\xf1\xbe\xf4\xd6\x8c\x87\rB\xf7\x06\xb9\xf1m\xed\x04\x94\xa8\xfe\xb6\xd3\x06\xc6@a\xdf\x9d\x9d\xf3Tv\xceS:\x01\xa6\x92A\xec\x04\xa3\x8f\r\xa2\xd5\t\xca\xd6\xcb\x9a\xf1\xefC]\xc0\xab\xa5A\xcf\\Spp\xc9\x88\xa6-\xd4kasP&\x86a\x0e_\x1b\xc2+\xe2\x8c\xd5\xbb\x9d\xc1\x03B\xba\x94\xda_\xa9\xb0\xca\xccM\n\xefGi\x03/\"\xfb\xf1(\xce\xbf]Pe\xa8\x90m\xb3t\xb0\x08\xc7\xac\xa8\xd1\xeb>\x9c\xfc]\x1a\x83.+\xcb\xb5\xf3D\x9d:\xa7\x17a\x96\xa2q\xd3p\x96\x15M\xb7Ls\xee\x19\\\xc5[>A\xfe\xacu`;\x1bc\xce\x00\xdd\xda\x08\x90b\xb4\xe5-\xeeH\xa7k\x17\x99T\xbe\x87J\xe3\xa9^\x04L\xeb\x10mT\xd6\xef\xf1\xe8\xf2b\x16\xcb\x80k\xed=\xed\xf5\x1f0\xa5\xaeK\xc9\x13\xed\x8a\x01\x01\xc9\xb8QX\xc0f:\xb1fK\xc4\xd51\x02b\xe9t\x84\x0c\xdbMF-\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=vTrus Root CA O=iTrusChina Co.,Ltd.
   * Subject: CN=vTrus Root CA O=iTrusChina Co.,Ltd.
   * Label: "vTrus Root CA"
   * Serial: 387574501246983434957692974888460947164905180485
   * SHA256 Fingerprint: 8a:71:de:65:59:33:6f:42:6c:26:e5:38:80:d0:0d:88:a1:8d:a4:c6:a9:1f:0d:cb:61:94:e2:06:c5:c9:63:87
   * -----BEGIN CERTIFICATE-----
   * MIIFVjCCAz6gAwIBAgIUQ+NxE9izWRRdt86M/TX9b7wFjUUwDQYJKoZIhvcNAQEL
   * BQAwQzELMAkGA1UEBhMCQ04xHDAaBgNVBAoTE2lUcnVzQ2hpbmEgQ28uLEx0ZC4x
   * FjAUBgNVBAMTDXZUcnVzIFJvb3QgQ0EwHhcNMTgwNzMxMDcyNDA1WhcNNDMwNzMx
   * MDcyNDA1WjBDMQswCQYDVQQGEwJDTjEcMBoGA1UEChMTaVRydXNDaGluYSBDby4s
   * THRkLjEWMBQGA1UEAxMNdlRydXMgUm9vdCBDQTCCAiIwDQYJKoZIhvcNAQEBBQAD
   * ggIPADCCAgoCggIBAL1VfGHTuB0EYgWgrmy3cLRB6ksDXhA/kFocizuwZotsSKYc
   * IrrVQJLuM7IjWcmOvFjai57QGfIvWcaMY1q6n6MLsLOaXLoRuBLpDLvPbmyAhykU
   * AyyNJJrIZIO1aqwTLDPxn9wsYTwaP3BVm60AUn/PBLn+NvqcwBauYv6WTEN+VRS+
   * GrPSbcKvdmaVayqwlHeFXgQPYh1jdfdr58tbmnDsPmcF8P4HCIDPKNsFxhQnL4Z9
   * 8Cfe/+Z+M0jnCx5Y0ScrUw5XSmXX+6KAYPxMvDVTAWqXcoKv8R1w6Jz1717CbMdH
   * flqUhSZNO7rrTOiwCcJlwp2dCZtOtZcFrPUGoPc2BX70kLJrxLT5ZOrpGgrIDajt
   * J8nU57O5q4IikCc9Kuh8kO+8T/3iCiSn3mUkpF3qwHYw03dQ+A0Em5Q2AXPKBlim
   * 0zvc+gRGE1WKyURHuFE5Gi7oNOJ5y1lKCn+8pu8fA2dqWSslYpPZUxlmPCdiKYZN
   * pGvu/9ROutW04o5IWgAZCfEF2c6Rsffr6TlP9m8EQ5pV9T4FFL2/s1m02I4zhKOQ
   * UqqzApVg+QxMaPnu1RcN+HFXtSXkKe5lXa/R7jwXC1pDxaWG6iSe4gUH3DRCEpHW
   * OXSuTEGC2/KmSNGzm/MzqvOmwMVO9fSddmPmAsYiS8GVP1BkLFTltvA8Kc9XAgMB
   * AAGjQjBAMB0GA1UdDgQWBBRUYnBj8XWEQ1iO0RYgscasGrz2iTAPBgNVHRMBAf8E
   * BTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAKbqSSaet
   * 8PFww+SX8J+pJdVrnjT+5hpk9jprUrIQeBqfTNqK2uwcN1LgQkv7bHbKJAs5EhWd
   * nxEt/Hlk3ODg9d3gV8mlsnZwUKT+twpw1aA08XXXTUm6EdGz2OyC/+sOxL9kLX1j
   * bhd47F18iMjrjld22VkE+rxSH0Ws8HqA7Oxvdq6R2xCOBNyS36D25q5J08FsEhvM
   * Kar5CKXiNxTKsbhm7xqC5PD48acWabfbqWE8n/Uxy+QARsIvdLGx14HuqCaVvIiv
   * TDUHKgLKeBRtRytAVunLKmChZwOgzoy8sHJnxDHO2zTlJQNgJXtxmOTAGytfdELS
   * S8VZCAeHvsXDf+eW2eHcKJfWjwXj9ZtOyh1QRwVTsMo554WgicEFOwE30z9J4nfr
   * I8iIZjs9OXYhRvHsXyO466JmdXTBQPfYaJqT4i2pLr0cox7IdMakLXogqzu4sEb9
   * b91fUlV1YvCXoHzXOP0l382gmxDPi7g4Xl7FtKYCNqEeXxzP4padKar9mK5S4fNB
   * UvupLnKWnyfjqnN9+BojZns7q2WwMgFLFT49ok8MKzWixtlnEjUwzXYuFrOZnk1P
   * Ti07NEPhmg4NpGaXutIcSkwsKouLgU9xGqndXHt7CMUADTdA43x7VF8vhV929ven
   * sBxXVsFy6K2ir40zSbofitzmdHxghm+Hl3s=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1\x1c0\x1a\x06\x03U\x04\n\x13\x13iTrusChina Co.,Ltd.1\x160\x14\x06\x03U\x04\x03\x13\rvTrus Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xbdU|a\xd3\xb8\x1d\x04b\x05\xa0\xael\xb7p\xb4A\xeaK\x03^\x10?\x90Z\x1c\x8b;\xb0f\x8blH\xa6\x1c\"\xba\xd5@\x92\xee3\xb2#Y\xc9\x8e\xbcX\xda\x8b\x9e\xd0\x19\xf2/Y\xc6\x8ccZ\xba\x9f\xa3\x0b\xb0\xb3\x9a\\\xba\x11\xb8\x12\xe9\x0c\xbb\xcfnl\x80\x87)\x14\x03,\x8d$\x9a\xc8d\x83\xb5j\xac\x13,3\xf1\x9f\xdc,a<\x1a?pU\x9b\xad\x00R\x7f\xcf\x04\xb9\xfe6\xfa\x9c\xc0\x16\xaeb\xfe\x96LC~U\x14\xbe\x1a\xb3\xd2m\xc2\xafvf\x95k*\xb0\x94w\x85^\x04\x0fb\x1dcu\xf7k\xe7\xcb[\x9ap\xec>g\x05\xf0\xfe\x07\x08\x80\xcf(\xdb\x05\xc6\x14\'/\x86}\xf0\'\xde\xff\xe6~3H\xe7\x0b\x1eX\xd1\'+S\x0eWJe\xd7\xfb\xa2\x80`\xfcL\xbc5S\x01j\x97r\x82\xaf\xf1\x1dp\xe8\x9c\xf5\xef^\xc2l\xc7G~Z\x94\x85&M;\xba\xebL\xe8\xb0\t\xc2e\xc2\x9d\x9d\t\x9bN\xb5\x97\x05\xac\xf5\x06\xa0\xf76\x05~\xf4\x90\xb2k\xc4\xb4\xf9d\xea\xe9\x1a\n\xc8\r\xa8\xed\'\xc9\xd4\xe7\xb3\xb9\xab\x82\"\x90\'=*\xe8|\x90\xef\xbcO\xfd\xe2\n$\xa7\xdee$\xa4]\xea\xc0v0\xd3wP\xf8\r\x04\x9b\x946\x01s\xca\x06X\xa6\xd3;\xdc\xfa\x04F\x13U\x8a\xc9DG\xb8Q9\x1a.\xe84\xe2y\xcbYJ\n\x7f\xbc\xa6\xef\x1f\x03gjY+%b\x93\xd9S\x19f<\'b)\x86M\xa4k\xee\xff\xd4N\xba\xd5\xb4\xe2\x8eHZ\x00\x19\t\xf1\x05\xd9\xce\x91\xb1\xf7\xeb\xe99O\xf6o\x04C\x9aU\xf5>\x05\x14\xbd\xbf\xb3Y\xb4\xd8\x8e3\x84\xa3\x90R\xaa\xb3\x02\x95`\xf9\x0cLh\xf9\xee\xd5\x17\r\xf8qW\xb5%\xe4)\xeee]\xaf\xd1\xee<\x17\x0bZC\xc5\xa5\x86\xea$\x9e\xe2\x05\x07\xdc4B\x12\x91\xd69t\xaeLA\x82\xdb\xf2\xa6H\xd1\xb3\x9b\xf33\xaa\xf3\xa6\xc0\xc5N\xf5\xf4\x9dvc\xe6\x02\xc6\"K\xc1\x95?Pd,T\xe5\xb6\xf0<)\xcfW\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=QuoVadis Root CA 1 G3 O=QuoVadis Limited
   * Subject: CN=QuoVadis Root CA 1 G3 O=QuoVadis Limited
   * Label: "QuoVadis Root CA 1 G3"
   * Serial: 687049649626669250736271037606554624078720034195
   * SHA256 Fingerprint: 8a:86:6f:d1:b2:76:b5:7e:57:8e:92:1c:65:82:8a:2b:ed:58:e9:f2:f2:88:05:41:34:b7:f1:f4:bf:c9:cc:74
   * -----BEGIN CERTIFICATE-----
   * MIIFYDCCA0igAwIBAgIUeFhfLq0sGUvjNwc1NBMotZbUZZMwDQYJKoZIhvcNAQEL
   * BQAwSDELMAkGA1UEBhMCQk0xGTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxHjAc
   * BgNVBAMTFVF1b1ZhZGlzIFJvb3QgQ0EgMSBHMzAeFw0xMjAxMTIxNzI3NDRaFw00
   * MjAxMTIxNzI3NDRaMEgxCzAJBgNVBAYTAkJNMRkwFwYDVQQKExBRdW9WYWRpcyBM
   * aW1pdGVkMR4wHAYDVQQDExVRdW9WYWRpcyBSb290IENBIDEgRzMwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQCgvlAQjunybEC0BJyFuTHK3C3kEakEPBtV
   * wedYMB0ktMPvhd6MLOHBPd+C5k+tR4ds7FtJwUrVu4/sh6x/gpqG7D0DmVIB0jWe
   * rNrwU8lmPNSsAgHaJNM7qAJGr6Qc4/hzWHa39g6QDbXwz8z6+cZM5cOGMAqNF341
   * 68Xfuw6cwI2H44g4hWf6Pser4BOcBRiYz5P1sZK0/CPTz9XEJ0ngnjybCKOLXSoh
   * 4Pw5qlPafX7PGglTvF0FBM+hSo+LdoINofjSxxR3W5A2B4GbPgb6Ul5jxaYA/qXp
   * UhtStZI5cgMJYr2wYBZupt0lwgNm3fME0UDiTouG9G/lg6AnhF4EwfWQvTA9xO+o
   * abw4m6SkltFi2mnAAZauy8RRNOoMqv8hjlmPSlzkYZqn0ukqeI1RPToV7qJZjqlc
   * 3sX5kCLliEVx3ZGZbHqfPT2YfF72vhZooF6uCyP8Wg+qInYtyaEQHeTTRCOQiJ/G
   * KubX9ZqzWB4vMIkIG1SitZgj7Ah3HJVdYdHLiZxfokqRmu8hqkkWCKi9YSgxyXSt
   * hfbZxbGL0eUQMk1fiyA6PEkfM4VZDdvLCXVDaXP7a3F98N/ETH3Goy7IlXnLc6KO
   * Tk0k+17kBL5yG6YnLUlamXrXXAkgt3+UuU/xDRxeiEIbEbfnkduebPRq34wGmAOt
   * zCjvpUfzUwIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIB
   * BjAdBgNVHQ4EFgQUo5fW816iEOGrRZ88F2Q87gFwnMwwDQYJKoZIhvcNAQELBQAD
   * ggIBABj6W3X8PnrHX3fHyt/PX8MSxEBd1DKquGrX1RUVRpgjpeaQWxiZTOOtQqOC
   * MTaIzen7xASWSIsBx40Bz1szBpZGZnQdT+3Btrm0DWHMY37XLneMlhwqI2hrhVd2
   * cDMT/uFPpiN3GPoajOi9ZcnPP/TJF9zrx7zABC4tRi9pZsMbj/7sPtPKlL92CiUN
   * qXsCHKnQO18LwIE6PWThv6ctTr1NxNgpxiIY0MWscgKCP6o6ojoilzHdCGPDdRS5
   * YCgtW2jgFqlmgiNR9etT2DGbe+m3nUvriBbP+V04ikkwj+3x6xn0dxoxGE1nVGwv
   * b2X52z3sIexe9PSLymBlVNFxZPT5pqOBMzYzcfCkeF9OrYMh3jRJjehZrJ3ydlo2
   * 8hP0r+AJx2EqbPfgna67hkooby7utHnNkDPDs3b69fBsnQGQ+p6Q9pxyz0fawx/k
   * NSBT8lTR32GDpgLiJTjehTItXnOQUl1CxM49S+H5GYQd1aJQzEH7QRTDvdbJWqNj
   * ZgKAvQU6O0ec7AAmTPWIUb+oI38YB7AL7YsmoWTTYUrrXJ/es69nA7Mf3W1daWhp
   * q1467HxpvMc7hU6eFbm0FU/DlXpY18ls6Wy58yljXrQs8C097Vpl4KlbQMJImYFt
   * nh8GKjwStIsPm6Ik8KaN1nrgS7ZklmOVhMJKzRwuJIczYOXD
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BM1\x190\x17\x06\x03U\x04\n\x13\x10QuoVadis Limited1\x1e0\x1c\x06\x03U\x04\x03\x13\x15QuoVadis Root CA 1 G3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa0\xbeP\x10\x8e\xe9\xf2l@\xb4\x04\x9c\x85\xb91\xca\xdc-\xe4\x11\xa9\x04<\x1bU\xc1\xe7X0\x1d$\xb4\xc3\xef\x85\xde\x8c,\xe1\xc1=\xdf\x82\xe6O\xadG\x87l\xec[I\xc1J\xd5\xbb\x8f\xec\x87\xac\x7f\x82\x9a\x86\xec=\x03\x99R\x01\xd25\x9e\xac\xda\xf0S\xc9f<\xd4\xac\x02\x01\xda$\xd3;\xa8\x02F\xaf\xa4\x1c\xe3\xf8sXv\xb7\xf6\x0e\x90\r\xb5\xf0\xcf\xcc\xfa\xf9\xc6L\xe5\xc3\x860\n\x8d\x17~5\xeb\xc5\xdf\xbb\x0e\x9c\xc0\x8d\x87\xe3\x888\x85g\xfa>\xc7\xab\xe0\x13\x9c\x05\x18\x98\xcf\x93\xf5\xb1\x92\xb4\xfc#\xd3\xcf\xd5\xc4\'I\xe0\x9e<\x9b\x08\xa3\x8b]*!\xe0\xfc9\xaaS\xda}~\xcf\x1a\tS\xbc]\x05\x04\xcf\xa1J\x8f\x8bv\x82\r\xa1\xf8\xd2\xc7\x14w[\x906\x07\x81\x9b>\x06\xfaR^c\xc5\xa6\x00\xfe\xa5\xe9R\x1bR\xb5\x929r\x03\tb\xbd\xb0`\x16n\xa6\xdd%\xc2\x03f\xdd\xf3\x04\xd1@\xe2N\x8b\x86\xf4o\xe5\x83\xa0\'\x84^\x04\xc1\xf5\x90\xbd0=\xc4\xef\xa8i\xbc8\x9b\xa4\xa4\x96\xd1b\xdai\xc0\x01\x96\xae\xcb\xc4Q4\xea\x0c\xaa\xff!\x8eY\x8fJ\\\xe4a\x9a\xa7\xd2\xe9*x\x8dQ=:\x15\xee\xa2Y\x8e\xa9\\\xde\xc5\xf9\x90\"\xe5\x88Eq\xdd\x91\x99lz\x9f==\x98|^\xf6\xbe\x16h\xa0^\xae\x0b#\xfcZ\x0f\xaa\"v-\xc9\xa1\x10\x1d\xe4\xd3D#\x90\x88\x9f\xc6*\xe6\xd7\xf5\x9a\xb3X\x1e/0\x89\x08\x1bT\xa2\xb5\x98#\xec\x08w\x1c\x95]a\xd1\xcb\x89\x9c_\xa2J\x91\x9a\xef!\xaaI\x16\x08\xa8\xbda(1\xc9t\xad\x85\xf6\xd9\xc5\xb1\x8b\xd1\xe5\x102M_\x8b :<I\x1f3\x85Y\r\xdb\xcb\tuCis\xfbkq}\xf0\xdf\xc4L}\xc6\xa3.\xc8\x95y\xcbs\xa2\x8eNM$\xfb^\xe4\x04\xber\x1b\xa6\'-IZ\x99z\xd7\\\t \xb7\x7f\x94\xb9O\xf1\r\x1c^\x88B\x1b\x11\xb7\xe7\x91\xdb\x9el\xf4j\xdf\x8c\x06\x98\x03\xad\xcc(\xef\xa5G\xf3S\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GTS Root R2 O=Google Trust Services LLC
   * Subject: CN=GTS Root R2 O=Google Trust Services LLC
   * Label: "GTS Root R2"
   * Serial: 159662449406622349769042896298
   * SHA256 Fingerprint: 8d:25:cd:97:22:9d:bf:70:35:6b:da:4e:b3:cc:73:40:31:e2:4c:f0:0f:af:cf:d3:2d:c7:6e:b5:84:1c:7e:a8
   * -----BEGIN CERTIFICATE-----
   * MIIFVzCCAz+gAwIBAgINAgPlrsWNBCUaqxElqjANBgkqhkiG9w0BAQwFADBHMQsw
   * CQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2VzIExMQzEU
   * MBIGA1UEAxMLR1RTIFJvb3QgUjIwHhcNMTYwNjIyMDAwMDAwWhcNMzYwNjIyMDAw
   * MDAwWjBHMQswCQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZp
   * Y2VzIExMQzEUMBIGA1UEAxMLR1RTIFJvb3QgUjIwggIiMA0GCSqGSIb3DQEBAQUA
   * A4ICDwAwggIKAoICAQDO3v2m++zsFDQ8BwZabFn3GTXd98GdVarTzTukk3LvCvpt
   * nfbwhYBboUhSnznFt+4orO/LdmgUud+tAWyZH8QiHZ/+cnfgLFuv5AS/T3KgGjSY
   * 6Dlo7JUle3ah5mm5hRm9iYz+re026nO8/4Piy33B0s5Ks40FnotJk9/BW9BuXvAu
   * MC6C/Pq8tBcKSOWIm8Wba96wyrQD8Nr0kLhlZPdcTK3ofmZemde4wj7I0BOdre7k
   * RXuJVfeKH2JShBKzwkCX44ofR5GmdFrS+LFjKBC4swm4VndAoiaYecb+3yXuPuWg
   * f9RhD1FLPD+M2uFwdNjCaKH5wQzpoeJ/u1U8dgbuak7MkogwTZq9TwtImoS1mKPV
   * +3PBV2HdKFZ1E66HjucMUQkQdYhMvI35ezzUIkgfKtzra7tEscszcTJGr61K8Yzo
   * dDqs5xoic4DSMPclQsciOzsSrZYuxsN2B6ogtzVJV+mSSeh2FnIxZyuWfoqjx5RW
   * Ir9qS34BIbIjMt/kmkRtWVtd9QCgHJvGeJeNkP+byKq0rxFROV7Z+2et1VsRnTKa
   * G73VululycslaVNVJ1zgyjbLiGH7HrfQy+4W+9OmTN6SpdTi3/UGVN4unUu0kzCq
   * gc7dGtxRcw1PcOnlthYhGXmy5okLdWTK1au8CcEYof/UVKGFPP0UJAOyh9OktwID
   * AQABo0IwQDAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4E
   * FgQUu//KjiOfT5nK2+JopqUVJxce2Q4wDQYJKoZIhvcNAQEMBQADggIBAB/Kzt3H
   * vqGf2SdMC9wXmBFqiN495nFWcrKeGk6c1SuYJF2ba3uwM4IJvd8lRuqYnrYb/oM8
   * 0mJhwQTtzuDFycgTE1XnqGOtjHsB/ncw4c5omwX4Eu55MaBBRTUoCnGkJE+M3DyC
   * B19m3H0Q/gxhswWV7uGugQ+o+MePTagjAiZrHYNSVc61LwDKgEDg4XSsYPWHgJ2u
   * NmSRXbBoGOqKYcl3qJfEycel/FVL8/B/uWU9J2jQzGv6U53hkRrJXRqWbTKH7QMg
   * yALOWr7Z6v2yTcQvG99fevX4i8buMTolUVVnjWQye+mew4K6Ki3pHrTgSAai/Gev
   * HyICc/sgCq+dVEuhzf9gR7A/Xe8bVr2XIZYtCtFenTgCR2y59PYjJbigapordwj6
   * xLEokCZYCDzifqrXPW+6MYgKBesntaFJ7qBFVHvmJ2WZICGoo7z7GJa7Um8M7YNR
   * TOlZ4iBgxcJlkoKM8xAfDoqXvneCbT+PHV28SSe9zE8P4c52hgQjxcCMElv924Sg
   * JPFI/2R80L5cFtHvma3AH/vLrrw4IgYmZNralw4/KBVEqE8AyvCazM90arQ+POuV
   * 7LXTWtiBmelDGDfrs7vRWGJB82bSj6p4lVQgw1oudCvV0b4YacCs1aTPObpRhANl
   * 6WLAYv7YTVWW4tAR+kg0Eeye7QUd5MjWHYbL
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\"0 \x06\x03U\x04\n\x13\x19Google Trust Services LLC1\x140\x12\x06\x03U\x04\x03\x13\x0bGTS Root R2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xce\xde\xfd\xa6\xfb\xec\xec\x144<\x07\x06ZlY\xf7\x195\xdd\xf7\xc1\x9dU\xaa\xd3\xcd;\xa4\x93r\xef\n\xfam\x9d\xf6\xf0\x85\x80[\xa1HR\x9f9\xc5\xb7\xee(\xac\xef\xcbvh\x14\xb9\xdf\xad\x01l\x99\x1f\xc4\"\x1d\x9f\xferw\xe0,[\xaf\xe4\x04\xbfOr\xa0\x1a4\x98\xe89h\xec\x95%{v\xa1\xe6i\xb9\x85\x19\xbd\x89\x8c\xfe\xad\xed6\xeas\xbc\xff\x83\xe2\xcb}\xc1\xd2\xceJ\xb3\x8d\x05\x9e\x8bI\x93\xdf\xc1[\xd0n^\xf0.0.\x82\xfc\xfa\xbc\xb4\x17\nH\xe5\x88\x9b\xc5\x9bk\xde\xb0\xca\xb4\x03\xf0\xda\xf4\x90\xb8ed\xf7\\L\xad\xe8~f^\x99\xd7\xb8\xc2>\xc8\xd0\x13\x9d\xad\xee\xe4E{\x89U\xf7\x8a\x1fbR\x84\x12\xb3\xc2@\x97\xe3\x8a\x1fG\x91\xa6tZ\xd2\xf8\xb1c(\x10\xb8\xb3\t\xb8Vw@\xa2&\x98y\xc6\xfe\xdf%\xee>\xe5\xa0\x7f\xd4a\x0fQK<?\x8c\xda\xe1pt\xd8\xc2h\xa1\xf9\xc1\x0c\xe9\xa1\xe2\x7f\xbbU<v\x06\xeejN\xcc\x92\x880M\x9a\xbdO\x0bH\x9a\x84\xb5\x98\xa3\xd5\xfbs\xc1Wa\xdd(Vu\x13\xae\x87\x8e\xe7\x0cQ\t\x10u\x88L\xbc\x8d\xf9{<\xd4\"H\x1f*\xdc\xebk\xbbD\xb1\xcb3q2F\xaf\xadJ\xf1\x8c\xe8t:\xac\xe7\x1a\"s\x80\xd20\xf7%B\xc7\";;\x12\xad\x96.\xc6\xc3v\x07\xaa \xb75IW\xe9\x92I\xe8v\x16r1g+\x96~\x8a\xa3\xc7\x94V\"\xbfjK~\x01!\xb2#2\xdf\xe4\x9aDmY[]\xf5\x00\xa0\x1c\x9b\xc6x\x97\x8d\x90\xff\x9b\xc8\xaa\xb4\xaf\x11Q9^\xd9\xfbg\xad\xd5[\x11\x9d2\x9a\x1b\xbd\xd5\xba[\xa5\xc9\xcb%iSU\'\\\xe0\xca6\xcb\x88a\xfb\x1e\xb7\xd0\xcb\xee\x16\xfb\xd3\xa6L\xde\x92\xa5\xd4\xe2\xdf\xf5\x06T\xde.\x9dK\xb4\x930\xaa\x81\xce\xdd\x1a\xdcQs\rOp\xe9\xe5\xb6\x16!\x19y\xb2\xe6\x89\x0bud\xca\xd5\xab\xbc\t\xc1\x18\xa1\xff\xd4T\xa1\x85<\xfd\x14$\x03\xb2\x87\xd3\xa4\xb7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=D-TRUST EV Root CA 2 2023 O=D-Trust GmbH
   * Subject: CN=D-TRUST EV Root CA 2 2023 O=D-Trust GmbH
   * Label: "D-TRUST EV Root CA 2 2023"
   * Serial: 139766439402180512324132425437959641711
   * SHA256 Fingerprint: 8e:82:21:b2:e7:d4:00:78:36:a1:67:2f:0d:cc:29:9c:33:bc:07:d3:16:f1:32:fa:1a:20:6d:58:71:50:f1:ce
   * -----BEGIN CERTIFICATE-----
   * MIIFqTCCA5GgAwIBAgIQaSYJfoBLTKCnjHhiU19abzANBgkqhkiG9w0BAQ0FADBI
   * MQswCQYDVQQGEwJERTEVMBMGA1UEChMMRC1UcnVzdCBHbWJIMSIwIAYDVQQDExlE
   * LVRSVVNUIEVWIFJvb3QgQ0EgMiAyMDIzMB4XDTIzMDUwOTA5MTAzM1oXDTM4MDUw
   * OTA5MTAzMlowSDELMAkGA1UEBhMCREUxFTATBgNVBAoTDEQtVHJ1c3QgR21iSDEi
   * MCAGA1UEAxMZRC1UUlVTVCBFViBSb290IENBIDIgMjAyMzCCAiIwDQYJKoZIhvcN
   * AQEBBQADggIPADCCAgoCggIBANiOo4mAC7JXUtypU0w3uX9jFxPvp1sjW2l1sJkK
   * F8GLxNuo4MwxusLyzV3pt/gdr2rElYfXR8mV2IIEUD2BCP/kPbOx1sWy/YgJ25yE
   * 7CUXFId/MHibaljJtnMoPDT3mfd/06b4HEV8rSyMlD/YZxBTfiLNTiVR8CUkNRFe
   * EMbsh2aJgWi6zCudR3Mfvc2RpHJqnKIbGKBv7FD0fUDCqDDPvXPIEysQEx6Lmqg6
   * lHPTGGkKSv/BAQP/eX+1SH977ugpbzZMlWGG2Pmic4ruri+W7mjNPU0oQvlFKzIb
   * RlUWaqZLKfm7lVa/Rh3sHZMdwGWyH6FDrlaeoLGPaxK3YG14C8qKXO0elg6DpkiV
   * jTujIcSuWMYAsoS0I6SWhjW42J7YrDRJmGOVxcttSEfi8i4YHtAxq9107PncjLgc
   * jmgjutDzUNzPZY9zOjLHfP7KgiJPvo5iR2blzYfi6NUPGJ/lBHJLRjwQ8kTCZFZx
   * TnXonMkmdMV9WdEKWw9t/p51HBjGGjp82A0EzM23RWV6sY+4roRIPrN6TagD4uJ+
   * ARZZaBhDM7DS3LAaQzXupdqpRlyuhoFBAUp0JuyfBr/CBTdkdXgpaP3F9ev+R/nk
   * hbDhezGdpn9yo7nELC7MmVcOIQxFAZRl62UJxmMiCzNJkkg8/M3OsD6Onov4/knF
   * NXJHAgMBAAGjgY4wgYswDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUqvyREBuH
   * kV8Wub9PS5FeAByxMoAwDgYDVR0PAQH/BAQDAgEGMEkGA1UdHwRCMEAwPqA8oDqG
   * OGh0dHA6Ly9jcmwuZC10cnVzdC5uZXQvY3JsL2QtdHJ1c3RfZXZfcm9vdF9jYV8y
   * XzIwMjMuY3JsMA0GCSqGSIb3DQEBDQUAA4ICAQCTy6UfmRHsmg1fLBWTxj++EI14
   * QvBukEdHjqOSMo1wj/Zbjb6JzkcBahsgIIlbyIIQbODnmaprxiqgYzWRaoUlrRc4
   * pZt+UPJ26oUFKidBK7GB0aL2QHWpDsvxVUjY7NHss+jOFKE17MJeNRqrphYBBo7q
   * 3C+jisosketSjl8MmxfPy3MHGcRqwnNU73xDUmPBEcrCRbH0O1P1aa4846XerOhU
   * t7KR/aypH/KH5BfGSah82ApB9PI+53c0BFLd6IHyTS9URZ0V4U/M5d40VxDJI3IX
   * cI1QcB9WbMy5/zpaT2N6w25lBx2Eof+pDGOJbbJAiDnXH3dotfyc1dZnaVuodNv8
   * ifYbMvekJKZ2t0dT741Jj6m2g1qllpBFYfXeA08mD6iL8AOWsKwV0HFaanuU5nCT
   * 2vFp4LJiTZ6P/4mdm13NRemUAiKN4DV/6PEEeXFsVIP4M7kFMhtYVRFP0OUnR3Hs
   * 7dpn1mKmS00PaaLJvOwiS5THaJQXfuKOKD62xur1NGyfN4gHONuGcfrNlUhDbqNP
   * gofXNJhuS5N5YHVpD/Aa1VP6IQzCP+k/HxiMkl14p3ZnGbuy6n/pcAlWVqOwDAst
   * Nl7F6cTVg8uGF5csbBNvh1qvSaYd2804BC5f4ko1Di1L+KIkBI3Y4WNeApI02phh
   * XBxvWHZks/wCuPWdCg==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x13\x0cD-Trust GmbH1\"0 \x06\x03U\x04\x03\x13\x19D-TRUST EV Root CA 2 2023"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd8\x8e\xa3\x89\x80\x0b\xb2WR\xdc\xa9SL7\xb9\x7fc\x17\x13\xef\xa7[#[iu\xb0\x99\n\x17\xc1\x8b\xc4\xdb\xa8\xe0\xcc1\xba\xc2\xf2\xcd]\xe9\xb7\xf8\x1d\xafj\xc4\x95\x87\xd7G\xc9\x95\xd8\x82\x04P=\x81\x08\xff\xe4=\xb3\xb1\xd6\xc5\xb2\xfd\x88\t\xdb\x9c\x84\xec%\x17\x14\x87\x7f0x\x9bjX\xc9\xb6s(<4\xf7\x99\xf7\x7f\xd3\xa6\xf8\x1cE|\xad,\x8c\x94?\xd8g\x10S~\"\xcdN%Q\xf0%$5\x11^\x10\xc6\xec\x87f\x89\x81h\xba\xcc+\x9dGs\x1f\xbd\xcd\x91\xa4rj\x9c\xa2\x1b\x18\xa0o\xecP\xf4}@\xc2\xa80\xcf\xbds\xc8\x13+\x10\x13\x1e\x8b\x9a\xa8:\x94s\xd3\x18i\nJ\xff\xc1\x01\x03\xffy\x7f\xb5H\x7f{\xee\xe8)o6L\x95a\x86\xd8\xf9\xa2s\x8a\xee\xae/\x96\xeeh\xcd=M(B\xf9E+2\x1bFU\x16j\xa6K)\xf9\xbb\x95V\xbfF\x1d\xec\x1d\x93\x1d\xc0e\xb2\x1f\xa1C\xaeV\x9e\xa0\xb1\x8fk\x12\xb7`mx\x0b\xca\x8a\\\xed\x1e\x96\x0e\x83\xa6H\x95\x8d;\xa3!\xc4\xaeX\xc6\x00\xb2\x84\xb4#\xa4\x96\x865\xb8\xd8\x9e\xd8\xac4I\x98c\x95\xc5\xcbmHG\xe2\xf2.\x18\x1e\xd01\xab\xddt\xec\xf9\xdc\x8c\xb8\x1c\x8eh#\xba\xd0\xf3P\xdc\xcfe\x8fs:2\xc7|\xfe\xca\x82\"O\xbe\x8ebGf\xe5\xcd\x87\xe2\xe8\xd5\x0f\x18\x9f\xe5\x04rKF<\x10\xf2D\xc2dVqNu\xe8\x9c\xc9&t\xc5}Y\xd1\n[\x0fm\xfe\x9eu\x1c\x18\xc6\x1a:|\xd8\r\x04\xcc\xcd\xb7Eez\xb1\x8f\xb8\xae\x84H>\xb3zM\xa8\x03\xe2\xe2~\x01\x16Yh\x18C3\xb0\xd2\xdc\xb0\x1aC5\xee\xa5\xda\xa9F\\\xae\x86\x81A\x01Jt&\xec\x9f\x06\xbf\xc2\x057dux)h\xfd\xc5\xf5\xeb\xfeG\xf9\xe4\x85\xb0\xe1{1\x9d\xa6\x7fr\xa3\xb9\xc4,.\xcc\x99W\x0e!\x0cE\x01\x94e\xebe\t\xc6c\"\x0b3I\x92H<\xfc\xcd\xce\xb0>\x8e\x9e\x8b\xf8\xfeI\xc55rG\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Amazon Root CA 1 O=Amazon
   * Subject: CN=Amazon Root CA 1 O=Amazon
   * Label: "Amazon Root CA 1"
   * Serial: 143266978916655856878034712317230054538369994
   * SHA256 Fingerprint: 8e:cd:e6:88:4f:3d:87:b1:12:5b:a3:1a:c3:fc:b1:3d:70:16:de:7f:57:cc:90:4f:e1:cb:97:c6:ae:98:19:6e
   * -----BEGIN CERTIFICATE-----
   * MIIDQTCCAimgAwIBAgITBmyfz5m/jAo54vB4ikPmljZbyjANBgkqhkiG9w0BAQsF
   * ADA5MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6
   * b24gUm9vdCBDQSAxMB4XDTE1MDUyNjAwMDAwMFoXDTM4MDExNzAwMDAwMFowOTEL
   * MAkGA1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJv
   * b3QgQ0EgMTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALJ4gHHKeNXj
   * ca9HgFB0fW7Y14h29Jlo91ghYPl0hAEvrAIthtOgQ3pOsqTQNroBvo3bSMgHFzZM
   * 9O6II8c+6zf1tRn4SWiw3te5djgdYZ6k/oI2peVKVuRF4fn9tBb6dNqcmzU5L/qw
   * IFAGbHrQgLKm+a/sRxmPUDgH3KKHOVj4utWp+UhnMJbulHheb4mjUcAwhmahRWa6
   * VOujw5H5SNz/0egwLX0tdHA114gk957EWW67c4cX8jJGKLhD+rcdqsq08p8kDi1L
   * 93FcXmn/6pUCyziKrlA4b9v7LWIbxcceVOF34GfID5yHI9Y/QCB/IIDEgEw+OyQm
   * jgSubJrIqg0CAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC
   * AYYwHQYDVR0OBBYEFIQYzIU07LwMlJQuCFmcx7IQTgoIMA0GCSqGSIb3DQEBCwUA
   * A4IBAQCY8jdaQZChGsV2USggNiMOruYou6r4lK5IpDB/G/wkjUu0yKGX9rbxenDI
   * U5PMCCjjmCXPI6T53iHTfIUJrU6adTrCC2qJeHZERxhlbI1Bjjt/msv0tadQ1wUs
   * N+gDS63pYaACbvXy8MWy7Vu33PqUXHeeE6V/Uq2V8viTO96LXFvKWlJbYK8U90vv
   * o/ufQJVtMVT8QtPHRh8jrdkPSHCa2XV4cdFyQzR1bldZwgJcJmApzyMZFo6IQ6XU
   * 5MsI+yMRQ+hDKXJioaldXgjUkK642M4UwtBV8ob2xJNDd2ZhwLnoQdeXeGADbkpy
   * rqXRfboQnoZsG4q5WTP468SQvvG5
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0f0\r\x06\x03U\x04\n\x13\x06Amazon1\x190\x17\x06\x03U\x04\x03\x13\x10Amazon Root CA 1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb2x\x80q\xcax\xd5\xe3q\xafG\x80Pt}n\xd8\xd7\x88v\xf4\x99h\xf7X!`\xf9t\x84\x01/\xac\x02-\x86\xd3\xa0CzN\xb2\xa4\xd06\xba\x01\xbe\x8d\xdbH\xc8\x07\x176L\xf4\xee\x88#\xc7>\xeb7\xf5\xb5\x19\xf8Ih\xb0\xde\xd7\xb9v8\x1da\x9e\xa4\xfe\x826\xa5\xe5JV\xe4E\xe1\xf9\xfd\xb4\x16\xfat\xda\x9c\x9b59/\xfa\xb0 P\x06lz\xd0\x80\xb2\xa6\xf9\xaf\xecG\x19\x8fP8\x07\xdc\xa2\x879X\xf8\xba\xd5\xa9\xf9Hg0\x96\xee\x94x^o\x89\xa3Q\xc00\x86f\xa1Ef\xbaT\xeb\xa3\xc3\x91\xf9H\xdc\xff\xd1\xe80-}-tp5\xd7\x88$\xf7\x9e\xc4Yn\xbbs\x87\x17\xf22F(\xb8C\xfa\xb7\x1d\xaa\xca\xb4\xf2\x9f$\x0e-K\xf7q\\^i\xff\xea\x95\x02\xcb8\x8a\xaeP8o\xdb\xfb-b\x1b\xc5\xc7\x1eT\xe1w\xe0g\xc8\x0f\x9c\x87#\xd6?@ \x7f \x80\xc4\x80L>;$&\x8e\x04\xael\x9a\xc8\xaa\r\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com TLS RSA Root CA 2022 O=SSL Corporation
   * Subject: CN=SSL.com TLS RSA Root CA 2022 O=SSL Corporation
   * Label: "SSL.com TLS RSA Root CA 2022"
   * Serial: 148535279242832292258835760425842727825
   * SHA256 Fingerprint: 8f:af:7d:2e:2c:b4:70:9b:b8:e0:b3:36:66:bf:75:a5:dd:45:b5:de:48:0f:8e:a8:d4:bf:e6:be:bc:17:f2:ed
   * -----BEGIN CERTIFICATE-----
   * MIIFiTCCA3GgAwIBAgIQb77arXO9CEDii02+1PdbkTANBgkqhkiG9w0BAQsFADBO
   * MQswCQYDVQQGEwJVUzEYMBYGA1UECgwPU1NMIENvcnBvcmF0aW9uMSUwIwYDVQQD
   * DBxTU0wuY29tIFRMUyBSU0EgUm9vdCBDQSAyMDIyMB4XDTIyMDgyNTE2MzQyMloX
   * DTQ2MDgxOTE2MzQyMVowTjELMAkGA1UEBhMCVVMxGDAWBgNVBAoMD1NTTCBDb3Jw
   * b3JhdGlvbjElMCMGA1UEAwwcU1NMLmNvbSBUTFMgUlNBIFJvb3QgQ0EgMjAyMjCC
   * AiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBANCkCXJPQIgSYT41I57u9nTP
   * L3tYPc48DRAokC+X94xI2KDYJbFMsBFMF3NQ0CJKY7uB0ylu1bUJPiYYf7ISf5OY
   * t6/wNr/y7hienDtSxUcZXXTzZGbVXcdotL8bHAajvI9AI7YexoS9UcQbOcGV0ins
   * S657Lb85/bRi3pZ7QcacoOAGcvvwB5cJOYF0r/c0WRFXCsJbwST0MXMwgsadugL3
   * PnxEX4MN8/HdIGkWCVDi1FW24IBydm5MR7d1VVm0U3TZlMZBrViKMWYPHqIbKUBO
   * L9975hYsLfy/7PO0+r4Y9ptJ1O4Fbtk085zx7AGL0SDGD6C1vBdOSHtRwvzpXGk3
   * R2azaPgVKPC506QVzFpPulJwoxJF3ca6TvvC0PeoUidtbnm1jPx7jMEWTO6Af77w
   * dr5BUxIzrlo4QqvXDz5BjXYHMtWrifZOZ9mxQnUjbvPNQrL8VfVThxc7wDNY8VLS
   * +YCk8OjwO4s4zKTGkH8PnP2L0aPP2oOnaclQNtVcBdIKQXTbYxE3waWglksejBYS
   * d66UNHsef8JmAOSqg+qKkK3ONkRN0VHpvB/zagX9wHQfJRlAUW7qglFA35u5CCoG
   * AtUjHBPW6dvbxrB6y3snm/vg1UYk7RBLY0ulBY+6uB0rpvqR4pJSvezrZ5dtmi2f
   * gTIFZzL7SAg/2SW4BCUvAgMBAAGjYzBhMA8GA1UdEwEB/wQFMAMBAf8wHwYDVR0j
   * BBgwFoAU+y437uOEeicuzRk1sTN8/9REQrkwHQYDVR0OBBYEFPsuN+7jhHonLs0Z
   * NbEzfP/UREK5MA4GA1UdDwEB/wQEAwIBhjANBgkqhkiG9w0BAQsFAAOCAgEAjYlt
   * hEUY8U+zoO9opMAdrDC8Z2awms22qyIZZtM7QbUQnRC6cm4pJCAcAZli05bg4vsM
   * QtfhWsSWTVTNj8pDU/0quOr4ZcoBwq1gaAafORpR2eCNJvkLTqVTJXojpBzOCBvf
   * R4iyrT7gJ4eLSYwfqUdYe5byiB0YrrPRpgqU+tvT5TgKa3kSM/tKWTcWQA673vWJ
   * DPFs0/dRa1419dvAJuoSc06pkZCmF8NsLzjUo3KUQyxi4U5cMj29TH0ZR6LDSeeW
   * P4+a0zvkEdiLA9z2tmBVGKaBUfPhqBVq6+AL8BQx1rmMRTqoENjwuSfr98t67wVy
   * lrXEj5ZzxOhWc5y8aVFjvO9nHEMaX3cZHxj4HCUp+UmZKbaSPaKDN7EgkaibMOlq
   * bLQjk2UEqxHzDh1TJElTHaE/nUiSEeJ9DU/1172iWD54nR4fK/4huxoTtrEoZP2w
   * AgDHbICivRZQIA9ygV/MlP+7mea6kMvq+cYMwq7FGc4zoWtcu358NFcXrfA/rs3q
   * r5nsLFR+jM4uElZI7xc7P0peYNLcdDa8pUNjyw9bowJWCZ4kLOGGgYz+qxcs+sji
   * Mho6/4UIyYOf8kpIEFR3N+2ivEC+5BB09+Rbu7nzifmPQdjH5FCQNYA+HLhNkNPU
   * 98OwoX6EyneSMSy4kLGCenROmxMmtNVQZlR4rmA=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation1%0#\x06\x03U\x04\x03\x0c\x1cSSL.com TLS RSA Root CA 2022"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd0\xa4\trO@\x88\x12a>5#\x9e\xee\xf6t\xcf/{X=\xce<\r\x10(\x90/\x97\xf7\x8cH\xd8\xa0\xd8%\xb1L\xb0\x11L\x17sP\xd0\"Jc\xbb\x81\xd3)n\xd5\xb5\t>&\x18\x7f\xb2\x12\x7f\x93\x98\xb7\xaf\xf06\xbf\xf2\xee\x18\x9e\x9c;R\xc5G\x19]t\xf3df\xd5]\xc7h\xb4\xbf\x1b\x1c\x06\xa3\xbc\x8f@#\xb6\x1e\xc6\x84\xbdQ\xc4\x1b9\xc1\x95\xd2)\xecK\xae{-\xbf9\xfd\xb4b\xde\x96{A\xc6\x9c\xa0\xe0\x06r\xfb\xf0\x07\x97\t9\x81t\xaf\xf74Y\x11W\n\xc2[\xc1$\xf41s0\x82\xc6\x9d\xba\x02\xf7>|D_\x83\r\xf3\xf1\xdd i\x16\tP\xe2\xd4U\xb6\xe0\x80rvnLG\xb7uUY\xb4St\xd9\x94\xc6A\xadX\x8a1f\x0f\x1e\xa2\x1b)@N/\xdf{\xe6\x16,-\xfc\xbf\xec\xf3\xb4\xfa\xbe\x18\xf6\x9bI\xd4\xee\x05n\xd94\xf3\x9c\xf1\xec\x01\x8b\xd1 \xc6\x0f\xa0\xb5\xbc\x17NH{Q\xc2\xfc\xe9\\i7Gf\xb3h\xf8\x15(\xf0\xb9\xd3\xa4\x15\xccZO\xbaRp\xa3\x12E\xdd\xc6\xbaN\xfb\xc2\xd0\xf7\xa8R\'mny\xb5\x8c\xfc{\x8c\xc1\x16L\xee\x80\x7f\xbe\xf0v\xbeAS\x123\xaeZ8B\xab\xd7\x0f>A\x8dv\x072\xd5\xab\x89\xf6Ng\xd9\xb1Bu#n\xf3\xcdB\xb2\xfcU\xf5S\x87\x17;\xc03X\xf1R\xd2\xf9\x80\xa4\xf0\xe8\xf0;\x8b8\xcc\xa4\xc6\x90\x7f\x0f\x9c\xfd\x8b\xd1\xa3\xcf\xda\x83\xa7i\xc9P6\xd5\\\x05\xd2\nAt\xdbc\x117\xc1\xa5\xa0\x96K\x1e\x8c\x16\x12w\xae\x944{\x1e\x7f\xc2f\x00\xe4\xaa\x83\xea\x8a\x90\xad\xce6DM\xd1Q\xe9\xbc\x1f\xf3j\x05\xfd\xc0t\x1f%\x19@Qn\xea\x82Q@\xdf\x9b\xb9\x08*\x06\x02\xd5#\x1c\x13\xd6\xe9\xdb\xdb\xc6\xb0z\xcb{\'\x9b\xfb\xe0\xd5F$\xed\x10KcK\xa5\x05\x8f\xba\xb8\x1d+\xa6\xfa\x91\xe2\x92R\xbd\xec\xebg\x97m\x9a-\x9f\x812\x05g2\xfbH\x08?\xd9%\xb8\x04%/\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=QuoVadis Root CA 2 G3 O=QuoVadis Limited
   * Subject: CN=QuoVadis Root CA 2 G3 O=QuoVadis Limited
   * Label: "QuoVadis Root CA 2 G3"
   * Serial: 390156079458959257446133169266079962026824725800
   * SHA256 Fingerprint: 8f:e4:fb:0a:f9:3a:4d:0d:67:db:0b:eb:b2:3e:37:c7:1b:f3:25:dc:bc:dd:24:0e:a0:4d:af:58:b4:7e:18:40
   * -----BEGIN CERTIFICATE-----
   * MIIFYDCCA0igAwIBAgIURFc0JFuBiZs18s64KztbpybwdSgwDQYJKoZIhvcNAQEL
   * BQAwSDELMAkGA1UEBhMCQk0xGTAXBgNVBAoTEFF1b1ZhZGlzIExpbWl0ZWQxHjAc
   * BgNVBAMTFVF1b1ZhZGlzIFJvb3QgQ0EgMiBHMzAeFw0xMjAxMTIxODU5MzJaFw00
   * MjAxMTIxODU5MzJaMEgxCzAJBgNVBAYTAkJNMRkwFwYDVQQKExBRdW9WYWRpcyBM
   * aW1pdGVkMR4wHAYDVQQDExVRdW9WYWRpcyBSb290IENBIDIgRzMwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQChriWyARjcV4g/Ruv5r+LrI3HimtFhZiFf
   * qq8nUeVuGxbULX1QsFN3vXg6YOJkApt8hpvWGo6t/x8Vf9WVHhLL5hSEBMHfNrMW
   * n4rjyduYNM7YMxcoRvynyfDStNVNCXJJ+fKH46nafaF9a7I6JaltUkSs+L5u+9ym
   * c5GQYaYDFCDy54ejiK2toIz/pgslUiXnFgHVy7g1gQyjO/Dh4fxaXc6AcW34Sas+
   * O7q414AB+6XrW7PFXmAqMaCvN+ggOp+oMiwMzAkd056OXbxMmO7FGmh77FOm6RQ1
   * o9/NgJ8MSPsc9PG/Srj61YxxSscfrf5BmrODXfKEVu+lV0POKa2Mq1W/xPtbAd0j
   * IaFYAI7D0GoT7RPjEiuA3GfmlbLNHiJuKvhB1PLKFAeNilUSxmn1uIZoL1NesNKq
   * IcGY5jDjZ1XHm26sGahVpkUG0CM62+tlXSoREfA7T8pt9DTEceT/AFr2XK4jYIVz
   * 8eQQsSWu1ZK7E8EM4DnatDlXtas1qnIhO4M15zHfeiFuuDIIfR0ykRVKYnLP43eh
   * vNURG3YBZwjgQQvD6xVu+KQZ2aKrr+InUlYrAoosFCT5v0ICvybIxo/gbjh9Uy3l
   * 7ZizlWNof/k19N+IxWA1ksB8aRxhlRbQ694Lrz4EEEVlWFA4r0jyWbYW8jwNkALG
   * cC4BrTwV1wIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIB
   * BjAdBgNVHQ4EFgQU7edvdlq/YOxJW8ald7tyFnGbxD0wDQYJKoZIhvcNAQELBQAD
   * ggIBAJHfgD9DCX5xwvfrs4iP4VGyvD11+ShdyLyZm3tdquXK4Qr36LLTn91nMX66
   * AarHakE7kNQIXLJgapDwyM4DYvmL7ftuKtwGTTwpD4kWilhMSA/ohGHqPHKmd+RC
   * roijQ1h5fq7KpVMNqT1wvSAZYaRsOPxDMuHBR//47PERIjKWnML2W2mWeyAMQ0Ga
   * W/ZZGYjeVYg3UQt4XAoeo0L9x52ID8DyeAIkVJOviYeIyUqAHerQbj5hLja7NQ4n
   * lv1mNDthcnPxFlxHBlRJAHpYErAK74X9sbgzdWqTHBLmYF5vHX/JHyPLhGGfHoJE
   * +V+tYlUkmlKY7VHnoX6XOuYvHxHaU4AshZ6rNRDbIl9qxV6XU/IyAgkwo1jwDQHV
   * csaxfGl7w/U2Rcxhbl5MlMVerugOXou/983g7aEOGzPuVBj+D77vfoRrQ+NwmNtd
   * dbINWQeFFSM51vHfqSYP1kjHs6Yi9TM3WpVHn3u6GBVv/9YUZINJ0gpnIdsPNWNg
   * KCLjsZWDzYWm3S8P52dSbrsvhXz1SnPnxT7AvSESBT/8twNJAlvIJebiVDj1eYeM
   * HVOyToV7BjjHLPj4sHKNJeV3UvQDHEimUF+IIDBu8oJDqz2XhOdT+yHBTw8imoa4
   * WSr2Rz0ZiC3oheGe7IUIarFsNMkd7EgrO3jtZsSOeWmD3n+M
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BM1\x190\x17\x06\x03U\x04\n\x13\x10QuoVadis Limited1\x1e0\x1c\x06\x03U\x04\x03\x13\x15QuoVadis Root CA 2 G3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa1\xae%\xb2\x01\x18\xdcW\x88?F\xeb\xf9\xaf\xe2\xeb#q\xe2\x9a\xd1af!_\xaa\xaf\'Q\xe5n\x1b\x16\xd4-}P\xb0Sw\xbdx:`\xe2d\x02\x9b|\x86\x9b\xd6\x1a\x8e\xad\xff\x1f\x15\x7f\xd5\x95\x1e\x12\xcb\xe6\x14\x84\x04\xc1\xdf6\xb3\x16\x9f\x8a\xe3\xc9\xdb\x984\xce\xd83\x17(F\xfc\xa7\xc9\xf0\xd2\xb4\xd5M\trI\xf9\xf2\x87\xe3\xa9\xda}\xa1}k\xb2:%\xa9mRD\xac\xf8\xben\xfb\xdc\xa6s\x91\x90a\xa6\x03\x14 \xf2\xe7\x87\xa3\x88\xad\xad\xa0\x8c\xff\xa6\x0b%R%\xe7\x16\x01\xd5\xcb\xb85\x81\x0c\xa3;\xf0\xe1\xe1\xfcZ]\xce\x80qm\xf8I\xab>;\xba\xb8\xd7\x80\x01\xfb\xa5\xeb[\xb3\xc5^`*1\xa0\xaf7\xe8 :\x9f\xa82,\x0c\xcc\t\x1d\xd3\x9e\x8e]\xbcL\x98\xee\xc5\x1ah{\xecS\xa6\xe9\x145\xa3\xdf\xcd\x80\x9f\x0cH\xfb\x1c\xf4\xf1\xbfJ\xb8\xfa\xd5\x8cqJ\xc7\x1f\xad\xfeA\x9a\xb3\x83]\xf2\x84V\xef\xa5WC\xce)\xad\x8c\xabU\xbf\xc4\xfb[\x01\xdd#!\xa1X\x00\x8e\xc3\xd0j\x13\xed\x13\xe3\x12+\x80\xdcg\xe6\x95\xb2\xcd\x1e\"n*\xf8A\xd4\xf2\xca\x14\x07\x8d\x8aU\x12\xc6i\xf5\xb8\x86h/S^\xb0\xd2\xaa!\xc1\x98\xe60\xe3gU\xc7\x9bn\xac\x19\xa8U\xa6E\x06\xd0#:\xdb\xebe]*\x11\x11\xf0;O\xcam\xf44\xc4q\xe4\xff\x00Z\xf6\\\xae#`\x85s\xf1\xe4\x10\xb1%\xae\xd5\x92\xbb\x13\xc1\x0c\xe09\xda\xb49W\xb5\xab5\xaar!;\x835\xe71\xdfz!n\xb82\x08}\x1d2\x91\x15Jbr\xcf\xe3w\xa1\xbc\xd5\x11\x1bv\x01g\x08\xe0A\x0b\xc3\xeb\x15n\xf8\xa4\x19\xd9\xa2\xab\xaf\xe2\'RV+\x02\x8a,\x14$\xf9\xbfB\x02\xbf&\xc8\xc6\x8f\xe0n8}S-\xe5\xed\x98\xb3\x95ch\x7f\xf95\xf4\xdf\x88\xc5`5\x92\xc0|i\x1ca\x95\x16\xd0\xeb\xde\x0b\xaf>\x04\x10EeXP8\xafH\xf2Y\xb6\x16\xf2<\r\x90\x02\xc6p.\x01\xad<\x15\xd7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=T-TeleSec GlobalRoot Class 2 O=T-Systems Enterprise Services GmbH OU=T-Systems Trust Center
   * Subject: CN=T-TeleSec GlobalRoot Class 2 O=T-Systems Enterprise Services GmbH OU=T-Systems Trust Center
   * Label: "T-TeleSec GlobalRoot Class 2"
   * Serial: 1
   * SHA256 Fingerprint: 91:e2:f5:78:8d:58:10:eb:a7:ba:58:73:7d:e1:54:8a:8e:ca:cd:01:45:98:bc:0b:14:3e:04:1b:17:05:25:52
   * -----BEGIN CERTIFICATE-----
   * MIIDwzCCAqugAwIBAgIBATANBgkqhkiG9w0BAQsFADCBgjELMAkGA1UEBhMCREUx
   * KzApBgNVBAoMIlQtU3lzdGVtcyBFbnRlcnByaXNlIFNlcnZpY2VzIEdtYkgxHzAd
   * BgNVBAsMFlQtU3lzdGVtcyBUcnVzdCBDZW50ZXIxJTAjBgNVBAMMHFQtVGVsZVNl
   * YyBHbG9iYWxSb290IENsYXNzIDIwHhcNMDgxMDAxMTA0MDE0WhcNMzMxMDAxMjM1
   * OTU5WjCBgjELMAkGA1UEBhMCREUxKzApBgNVBAoMIlQtU3lzdGVtcyBFbnRlcnBy
   * aXNlIFNlcnZpY2VzIEdtYkgxHzAdBgNVBAsMFlQtU3lzdGVtcyBUcnVzdCBDZW50
   * ZXIxJTAjBgNVBAMMHFQtVGVsZVNlYyBHbG9iYWxSb290IENsYXNzIDIwggEiMA0G
   * CSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCqX9obX+hzkeXaXPSi5kfl82hVYAUd
   * AqSzm1nzHoqvNK38DcLZSBnuaY/JIPwhqgcZ7bBcrGXHX+0CfHt8LRvWurmAwhiC
   * FoT6ZrAIxlQjgeTNuUk/9k9uN0goOA/FvudocP05l03Sx5iRUKrERLMjfTlH6VJi
   * 1hKTXrcxlkIF+3anHqP1wvzpesVsqXFP6st4vGCvx9702cu+fjOlbpSD8DT6Iavq
   * jnKgP6TeMFvvhk1qlVtDRKgQFRzlAVfFmPHmBiiRqiDFt1MmUUOyCxGVWOHAD3bZ
   * wI18gfNycJ5v/hqO2V81xrJvNHy+SE/iWjnX2J14np+GPgNeGYtEotXHAgMBAAGj
   * QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBS/
   * WSA2AHmgoCJrjNXyYdK4LMuCSjANBgkqhkiG9w0BAQsFAAOCAQEAMQOiYQsfdOhy
   * NsZt+U2e+iKo4YFWz827n+qrkRk4r6p8FU3ztqONpfSO9kSpp+ghla0+AGIWiPAC
   * uvxhI+YzmzB6azZie60EI4RYZeLbK4rnJVM3YlNfvNoBYimipidx5joifsFvHZVw
   * IEoHNN/q/xWA5brXethbdXwFeilHfkCoMRN3zUA7tFFHei4R40cR3p1m0IvVVGb6
   * g1XqfMIpiRvpb7PO4gWEyS8+eIVibslfwXhjdFjASBgMmTnrpMwatXlajRWc2BQN
   * 9noHV8cigwUtPJslJj0Ys6lDfMjIq2SPDqO/nBudMNva0Bkuqjzx+zOAduTNrRlP
   * BSeOE6Fuwg==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1+0)\x06\x03U\x04\n\x0c\"T-Systems Enterprise Services GmbH1\x1f0\x1d\x06\x03U\x04\x0b\x0c\x16T-Systems Trust Center1%0#\x06\x03U\x04\x03\x0c\x1cT-TeleSec GlobalRoot Class 2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xaa_\xda\x1b_\xe8s\x91\xe5\xda\\\xf4\xa2\xe6G\xe5\xf3hU`\x05\x1d\x02\xa4\xb3\x9bY\xf3\x1e\x8a\xaf4\xad\xfc\r\xc2\xd9H\x19\xeei\x8f\xc9 \xfc!\xaa\x07\x19\xed\xb0\\\xace\xc7_\xed\x02|{|-\x1b\xd6\xba\xb9\x80\xc2\x18\x82\x16\x84\xfaf\xb0\x08\xc6T#\x81\xe4\xcd\xb9I?\xf6On7H(8\x0f\xc5\xbe\xe7hp\xfd9\x97M\xd2\xc7\x98\x91P\xaa\xc4D\xb3#}9G\xe9Rb\xd6\x12\x93^\xb71\x96B\x05\xfbv\xa7\x1e\xa3\xf5\xc2\xfc\xe9z\xc5l\xa9qO\xea\xcbx\xbc`\xaf\xc7\xde\xf4\xd9\xcb\xbe~3\xa5n\x94\x83\xf04\xfa!\xab\xea\x8er\xa0?\xa4\xde0[\xef\x86Mj\x95[CD\xa8\x10\x15\x1c\xe5\x01W\xc5\x98\xf1\xe6\x06(\x91\xaa \xc5\xb7S&QC\xb2\x0b\x11\x95X\xe1\xc0\x0fv\xd9\xc0\x8d|\x81\xf3rp\x9eo\xfe\x1a\x8e\xd9_5\xc6\xb2o4|\xbeHO\xe2Z9\xd7\xd8\x9dx\x9e\x9f\x86>\x03^\x19\x8bD\xa2\xd5\xc7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Trustwave Global ECC P256 Certification Authority O=Trustwave Holdings, Inc.
   * Subject: CN=Trustwave Global ECC P256 Certification Authority O=Trustwave Holdings, Inc.
   * Label: "Trustwave Global ECC P256 Certification Authority"
   * Serial: 4151900041497450638097112925
   * SHA256 Fingerprint: 94:5b:bc:82:5e:a5:54:f4:89:d1:fd:51:a7:3d:df:2e:a6:24:ac:70:19:a0:52:05:22:5c:22:a7:8c:cf:a8:b4
   * -----BEGIN CERTIFICATE-----
   * MIICYDCCAgegAwIBAgIMDWpfCD8oXD5Rld9dMAoGCCqGSM49BAMCMIGRMQswCQYD
   * VQQGEwJVUzERMA8GA1UECBMISWxsaW5vaXMxEDAOBgNVBAcTB0NoaWNhZ28xITAf
   * BgNVBAoTGFRydXN0d2F2ZSBIb2xkaW5ncywgSW5jLjE6MDgGA1UEAxMxVHJ1c3R3
   * YXZlIEdsb2JhbCBFQ0MgUDI1NiBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTAeFw0x
   * NzA4MjMxOTM1MTBaFw00MjA4MjMxOTM1MTBaMIGRMQswCQYDVQQGEwJVUzERMA8G
   * A1UECBMISWxsaW5vaXMxEDAOBgNVBAcTB0NoaWNhZ28xITAfBgNVBAoTGFRydXN0
   * d2F2ZSBIb2xkaW5ncywgSW5jLjE6MDgGA1UEAxMxVHJ1c3R3YXZlIEdsb2JhbCBF
   * Q0MgUDI1NiBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTBZMBMGByqGSM49AgEGCCqG
   * SM49AwEHA0IABH77bOYj43MyCMpg5lOcunSNGLB4kFKA3TjASh3RqMyTpJcGOMoN
   * FWLGjgEqZZ2q3zSRLoHB5DOSMcT9CTqmP62jQzBBMA8GA1UdEwEB/wQFMAMBAf8w
   * DwYDVR0PAQH/BAUDAwcGADAdBgNVHQ4EFgQUo0EGrJBt0UrrdaVKEJmzsaGLSvcw
   * CgYIKoZIzj0EAwIDRwAwRAIgB+ZU2g6gWrKuEZ+Hxbb/ad4lvvigtwjzRM4q3wgh
   * DDcCIC0mA6AFvWvR9lz4ZcyGbbOcNEhjhAnFjXca4syc4XR7
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x110\x0f\x06\x03U\x04\x08\x13\x08Illinois1\x100\x0e\x06\x03U\x04\x07\x13\x07Chicago1!0\x1f\x06\x03U\x04\n\x13\x18Trustwave Holdings, Inc.1:08\x06\x03U\x04\x03\x131Trustwave Global ECC P256 Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\x13\x06\x07*\x86H\xce=\x02\x01\x06\x08*\x86H\xce=\x03\x01\x07\x03B\x00\x04~\xfbl\xe6#\xe3s2\x08\xca`\xe6S\x9c\xbat\x8d\x18\xb0x\x90R\x80\xdd8\xc0J\x1d\xd1\xa8\xcc\x93\xa4\x97\x068\xca\r\x15b\xc6\x8e\x01*e\x9d\xaa\xdf4\x91.\x81\xc1\xe43\x921\xc4\xfd\t:\xa6?\xad"),
    name_constraints: None
  },

  /*
   * Issuer: CN=ISRG Root X1 O=Internet Security Research Group
   * Subject: CN=ISRG Root X1 O=Internet Security Research Group
   * Label: "ISRG Root X1"
   * Serial: 172886928669790476064670243504169061120
   * SHA256 Fingerprint: 96:bc:ec:06:26:49:76:f3:74:60:77:9a:cf:28:c5:a7:cf:e8:a3:c0:aa:e1:1a:8f:fc:ee:05:c0:bd:df:08:c6
   * -----BEGIN CERTIFICATE-----
   * MIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw
   * TzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh
   * cmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4
   * WhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu
   * ZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY
   * MTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc
   * h77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+
   * 0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U
   * A5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW
   * T8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH
   * B5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC
   * B5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv
   * KBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn
   * OlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn
   * jh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw
   * qHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI
   * rU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV
   * HRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq
   * hkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL
   * ubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ
   * 3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK
   * NFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5
   * ORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur
   * TkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC
   * jNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc
   * oyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq
   * 4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA
   * mRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d
   * emyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1)0\'\x06\x03U\x04\n\x13 Internet Security Research Group1\x150\x13\x06\x03U\x04\x03\x13\x0cISRG Root X1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xad\xe8$s\xf4\x147\xf3\x9b\x9e+W(\x1c\x87\xbe\xdc\xb7\xdf8\x90\x8cn<\xe6W\xa0x\xf7u\xc2\xa2\xfe\xf5jn\xf6\x00O(\xdb\xdeh\x86lD\x93\xb6\xb1c\xfd\x14\x12k\xbf\x1f\xd2\xea1\x9b!~\xd13<\xbaH\xf5\xddy\xdf\xb3\xb8\xff\x12\xf1!\x9aK\xc1\x8a\x86qiJffl\x8f~<p\xbf\xad)\"\x06\xf3\xe4\xc0\xe6\x80\xae\xe2K\x8f\xb7\x99~\x94\x03\x9f\xd3G\x97|\x99H#S\xe88\xaeO\no\x83.\xd1IW\x8c\x80t\xb6\xda/\xd08\x8d{\x03p!\x1bu\xf20<\xfa\x8f\xae\xdd\xdac\xab\xeb\x16O\xc2\x8e\x11K~\xcf\x0b\xe8\xff\xb5w.\xf4\xb2{J\xe0L\x12%\x0cp\x8d\x03)\xa0\xe1S$\xec\x13\xd9\xee\x19\xbf\x10\xb3J\x8c?\x89\xa3aQ\xde\xac\x87\x07\x94\xf4cq\xec.\xe2o[\x98\x81\xe1\x89\\4ylv\xef;\x90by\xe6\xdb\xa4\x9a/&\xc5\xd0\x10\xe1\x0e\xde\xd9\x10\x8e\x16\xfb\xb7\xf7\xa8\xf7\xc7\xe5\x02\x07\x98\x8f6\x08\x95\xe7\xe27\x96\r6u\x9e\xfb\x0er\xb1\x1d\x9b\xbc\x03\xf9I\x05\xd8\x81\xdd\x05\xb4*\xd6A\xe9\xac\x01v\x95\n\x0f\xd8\xdf\xd5\xbd\x12\x1f5/(\x17l\xd2\x98\xc1\xa8\tdwnG7\xba\xce\xacY^h\x9d\x7fr\xd6\x89\xc5\x06A)>Y>\xdd&\xf5$\xc9\x11\xa7Z\xa3L@\x1fF\xa1\x99\xb5\xa7:Qn\x86;\x9e}r\xa7\x12\x05xY\xed>Qx\x15\x0b\x03\x8f\x8d\xd0/\x05\xb2>{J\x1cKs\x05\x12\xfc\xc6\xea\xe0P\x13|C\x93t\xb3\xcat\xe7\x8e\x1f\x01\x08\xd00\xd4[q6\xb4\x07\xba\xc100\\H\xb7\x82;\x98\xa6}`\x8a\xa2\xa3)\x82\xcc\xba\xbd\x83\x04\x1b\xa2\x83\x03A\xa1\xd6\x05\xf1\x1b\xc2\xb6\xf0\xa8|\x86;F\xa8H*\x88\xdcv\x9av\xbf\x1fj\xa5=\x19\x8f\xeb8\xf3d\xde\xc8+\r\n(\xff\xf7\xdb\xe2\x15B\xd4\"\xd0\']\xe1y\xfe\x18\xe7p\x88\xadN\xe6\xd9\x8b:\xc6\xdd\'Qn\xff\xbcd\xf53CO\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Trustwave Global Certification Authority O=Trustwave Holdings, Inc.
   * Subject: CN=Trustwave Global Certification Authority O=Trustwave Holdings, Inc.
   * Label: "Trustwave Global Certification Authority"
   * Serial: 1846098327275375458322922162
   * SHA256 Fingerprint: 97:55:20:15:f5:dd:fc:3c:87:88:c0:06:94:45:55:40:88:94:45:00:84:f1:00:86:70:86:bc:1a:2b:b5:8d:c8
   * -----BEGIN CERTIFICATE-----
   * MIIF2jCCA8KgAwIBAgIMBfcOhtpJ80Y1LrqyMA0GCSqGSIb3DQEBCwUAMIGIMQsw
   * CQYDVQQGEwJVUzERMA8GA1UECAwISWxsaW5vaXMxEDAOBgNVBAcMB0NoaWNhZ28x
   * ITAfBgNVBAoMGFRydXN0d2F2ZSBIb2xkaW5ncywgSW5jLjExMC8GA1UEAwwoVHJ1
   * c3R3YXZlIEdsb2JhbCBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTAeFw0xNzA4MjMx
   * OTM0MTJaFw00MjA4MjMxOTM0MTJaMIGIMQswCQYDVQQGEwJVUzERMA8GA1UECAwI
   * SWxsaW5vaXMxEDAOBgNVBAcMB0NoaWNhZ28xITAfBgNVBAoMGFRydXN0d2F2ZSBI
   * b2xkaW5ncywgSW5jLjExMC8GA1UEAwwoVHJ1c3R3YXZlIEdsb2JhbCBDZXJ0aWZp
   * Y2F0aW9uIEF1dGhvcml0eTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIB
   * ALldUShLPDeS0YLOvR29zd24q88KPuFd5dyqCblXAj7mY2Hf8g+CY66j96xz0Xzn
   * swuvCAAJWX/NKSqIk4cXGIDtiLK0thAfLdZfVaITXdHG6wZWiYj+rDKd/VzDBcdu
   * 7oaJuogDnXIhhpCujwOl3J+IKMujkkkP7NAP4m1ET4BqstTnoApTAbqOl5F2brz8
   * 1Ws25kCI1nsvXwXoLG0R8+eyvpJETNKXpP7ScoFDB5zpET71ixpZfR9oWN0EACyW
   * 80OzfpgZdNmcc9kYvkHHNHnZ9GLCQ7mzJ7Aiy/k9UscwR7PJPrhq4ufogXBeQotP
   * JqX+OsIgbrv4Fo7NDKm0G2x2EOFYeUY+VM6AqFcJNykbmROPDMjWLBz7BegIlT1l
   * RtzuzWniTY+HKE40Cz7PFNm73bZQmq131BnW2hqIyE4bJ3XYsgjxroMwuREOzYfw
   * hI0Vcnyh78zyiGG69Gm7DIwLdVcEuE4qFC49DxweMqZiNu5m4iK4BUBjECLzMx10
   * coos9TkpoNPnG4CELcU9402x/RpvumUHO1jsQkUm+9jaJXLE9gCxInm943xZYkqc
   * BW89zubWR2OZxiRvchLIrH+QtAuRcOi35hYQcRfO3gZPSEF9NUqjifLJS3tBEW1n
   * twiYTOURGa5CgNz7kAXU+FDKvuStx8KU1xad5hePrzb7AgMBAAGjQjBAMA8GA1Ud
   * EwEB/wQFMAMBAf8wHQYDVR0OBBYEFJngGWcNYtt2s9o9uFvo/ULSMQ6HMA4GA1Ud
   * DwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAmHNw4rDT7TnsTGDZqRKGFx6W
   * 0OhUKDtkLSGm+J1WE2pIPU/HPinbbViDVD2HfSMF1OQc3Og4ZYbFdada2zUFvXfe
   * uyk3QAUHw5RSn8pk3fEbK9xGChACMf1KaA0HZJDmHvUqoai7PF35owgLEQzxPy0Q
   * lG/+4jSHg9bP5Rs1bdID4bANqKCqRieCNqcVtgimQlRXtpla4gt5kNdXElE1GYhB
   * aCXUNxeEFfsBctyV3lImIJgm4nb1J2/6ADtKYdkNy1GTKv0WBpanI5ojSP5RvbbE
   * sLFUzt5sQa0WZ37b/TjNuThOssFgy50X31ieemKyJo90lZvkWx3SD92YHJtZuSPT
   * MaCm/zjdzyBP6VhWOmfD0faZmZ26NraAL4hHT4a/RDqA5Dccprrql5gR0IRiR2Qe
   * qu5AvzSxnI9O4fKSTx+O856X3vOmeWqJcU9LJxdI/uz0UA9PSX3MReO9ekDFQdxh
   * VicGaeVyQYHTtgGJoC86cnn+OjC/QezHYj6RS8fZMXZC+fc8Y+wmjHMMfRod6qh8
   * h6jCJ3zhM0EPz8/8AKAigJ5Kp28AsEFFtyLKaEjFQqKu3R3y4G5OBVixwJAWKqQ9
   * EEC+j2Jjg6mcgn0tAumDMHzLJ8n9HmYAsC7TIS+OMxZsmO0QqAfWzJPP29FpHOTK
   * yeC2nOnOcXHebD8WpHk=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x110\x0f\x06\x03U\x04\x08\x0c\x08Illinois1\x100\x0e\x06\x03U\x04\x07\x0c\x07Chicago1!0\x1f\x06\x03U\x04\n\x0c\x18Trustwave Holdings, Inc.110/\x06\x03U\x04\x03\x0c(Trustwave Global Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb9]Q(K<7\x92\xd1\x82\xce\xbd\x1d\xbd\xcd\xdd\xb8\xab\xcf\n>\xe1]\xe5\xdc\xaa\t\xb9W\x02>\xe6ca\xdf\xf2\x0f\x82c\xae\xa3\xf7\xacs\xd1|\xe7\xb3\x0b\xaf\x08\x00\tY\x7f\xcd)*\x88\x93\x87\x17\x18\x80\xed\x88\xb2\xb4\xb6\x10\x1f-\xd6_U\xa2\x13]\xd1\xc6\xeb\x06V\x89\x88\xfe\xac2\x9d\xfd\\\xc3\x05\xc7n\xee\x86\x89\xba\x88\x03\x9dr!\x86\x90\xae\x8f\x03\xa5\xdc\x9f\x88(\xcb\xa3\x92I\x0f\xec\xd0\x0f\xe2mDO\x80j\xb2\xd4\xe7\xa0\nS\x01\xba\x8e\x97\x91vn\xbc\xfc\xd5k6\xe6@\x88\xd6{/_\x05\xe8,m\x11\xf3\xe7\xb2\xbe\x92DL\xd2\x97\xa4\xfe\xd2r\x81C\x07\x9c\xe9\x11>\xf5\x8b\x1aY}\x1fhX\xdd\x04\x00,\x96\xf3C\xb3~\x98\x19t\xd9\x9cs\xd9\x18\xbeA\xc74y\xd9\xf4b\xc2C\xb9\xb3\'\xb0\"\xcb\xf9=R\xc70G\xb3\xc9>\xb8j\xe2\xe7\xe8\x81p^B\x8bO&\xa5\xfe:\xc2 n\xbb\xf8\x16\x8e\xcd\x0c\xa9\xb4\x1blv\x10\xe1XyF>T\xce\x80\xa8W\t7)\x1b\x99\x13\x8f\x0c\xc8\xd6,\x1c\xfb\x05\xe8\x08\x95=eF\xdc\xee\xcdi\xe2M\x8f\x87(N4\x0b>\xcf\x14\xd9\xbb\xdd\xb6P\x9a\xadw\xd4\x19\xd6\xda\x1a\x88\xc8N\x1b\'u\xd8\xb2\x08\xf1\xae\x830\xb9\x11\x0e\xcd\x87\xf0\x84\x8d\x15r|\xa1\xef\xcc\xf2\x88a\xba\xf4i\xbb\x0c\x8c\x0buW\x04\xb8N*\x14.=\x0f\x1c\x1e2\xa6b6\xeef\xe2\"\xb8\x05@c\x10\"\xf33\x1dtr\x8a,\xf59)\xa0\xd3\xe7\x1b\x80\x84-\xc5=\xe3M\xb1\xfd\x1ao\xbae\x07;X\xecBE&\xfb\xd8\xda%r\xc4\xf6\x00\xb1\"y\xbd\xe3|YbJ\x9c\x05o=\xce\xe6\xd6Gc\x99\xc6$or\x12\xc8\xac\x7f\x90\xb4\x0b\x91p\xe8\xb7\xe6\x16\x10q\x17\xce\xde\x06OHA}5J\xa3\x89\xf2\xc9K{A\x11mg\xb7\x08\x98L\xe5\x11\x19\xaeB\x80\xdc\xfb\x90\x05\xd4\xf8P\xca\xbe\xe4\xad\xc7\xc2\x94\xd7\x16\x9d\xe6\x17\x8f\xaf6\xfb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Buypass Class 2 Root CA O=Buypass AS-983163327
   * Subject: CN=Buypass Class 2 Root CA O=Buypass AS-983163327
   * Label: "Buypass Class 2 Root CA"
   * Serial: 2
   * SHA256 Fingerprint: 9a:11:40:25:19:7c:5b:b9:5d:94:e6:3d:55:cd:43:79:08:47:b6:46:b2:3c:df:11:ad:a4:a0:0e:ff:15:fb:48
   * -----BEGIN CERTIFICATE-----
   * MIIFWTCCA0GgAwIBAgIBAjANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJOTzEd
   * MBsGA1UECgwUQnV5cGFzcyBBUy05ODMxNjMzMjcxIDAeBgNVBAMMF0J1eXBhc3Mg
   * Q2xhc3MgMiBSb290IENBMB4XDTEwMTAyNjA4MzgwM1oXDTQwMTAyNjA4MzgwM1ow
   * TjELMAkGA1UEBhMCTk8xHTAbBgNVBAoMFEJ1eXBhc3MgQVMtOTgzMTYzMzI3MSAw
   * HgYDVQQDDBdCdXlwYXNzIENsYXNzIDIgUm9vdCBDQTCCAiIwDQYJKoZIhvcNAQEB
   * BQADggIPADCCAgoCggIBANfHXvfBB9R3+0Mh9PT1aeTuMgHbo4Yf5FkNuud1g1Lr
   * 6hxhFUi7HQfKjK6w3Jad6sNgkoaCKHOcVgb/S2TwDCo3SbXlzwx87vFKu3MwZfPV
   * L4O2fuPn9Z6rYPnT8Z2SdIrkHJasW4DptfQxh6NR/Md+oW+OU3fUl8FVM5I+GC91
   * 1K2GScuVr1QGbNgGE41b/+EmGVnAJLqBcXmQRFBoJJRfuLMR8SlBYaNByyM21cHx
   * MlAQTn/0hpPshNOOvEu/XAFOBz3cFIqUCqTqc/sLUegTBxj6DvEr0VQVfTzh97QZ
   * QmdiXnfgolXsttlpF9U6r0TtSsWe5HonfOV116rLJeffawrbD02TTqigzXsu8lkB
   * arcNuAeBfos4GzjmCleZPe4h6KP1DBbdi+w0jpwqHAAVF41og9JwnxgIzRFo1clr
   * Us3ERo/ctfPYV3Me6ZQ5BL/T3jjetFPsaRyifsSP5BtwrfKi+fv3FmRmaZ9JUaLi
   * FRhnBkp/1Wy1TbMz4GHrXb7pmA8y1x1LPC5aAVKRCfLf6o3YBkBjqhHk/sM3nhRS
   * P/TizPJhk9H9Z2vXUq6/aKtAQ6BXNVN48FP4YUIHZMbXb5tMOA1jrGKvNouicwoN
   * 9SG9dKpN6nIDSdvHXx1iY8f93ZHsM+71bbRuMGjeyNYmsHVee7QHIJihdjK4TWxP
   * AgMBAAGjQjBAMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFMmAd+BikoL1Rpzz
   * uvdMw964o605MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAU18h
   * 9bqwOlI5LJKwbADJ784g7wbylp7ppHR/ehb8t/W2+xUbP6umwHJdELFx7rxP462s
   * A20ucS6vxOOto70MEae0/0qyexAQH6dXQbLArvQsWdZHEIjzIVEpMMpghq9Gqx3t
   * OluwlN5E40EIosHsHdb9T7bWR9AUC8rmyrV7d35BH16Dx7aMOZawP5aBQW9gkOLo
   * +fsicdl9sz1Gv7SEr5AcD48Saq/v7h56rgJKihcrdv6sVIkkLE8/trKnToyokZf7
   * KcZ7XC25y2a2t6hbElGFtQl+Ynhw/qlqYLYdDnkM/crqJIByw5c/8nerQyIKx+u2
   * DISCLIBrQYoIwOula9+ZEsuK1V6ADJHgJgg2SMX6OBE1/yWDLfJ6v9r9jv6ly0Us
   * H8SIU653DtmadsWOLB2jutXsMq7Aqqz30XpN69QH4kj3Io6wpJ9qzo6ysmD0oyLQ
   * I+uUWnpp3Q+/QFesa1lQ2aOZ4W7+jQF5JyMV3pKdewlNWudLSDBaGOYKbeaP4NK7
   * 5t98biGCwWg5TbSYWGZizEqQXsP6JwSxeRV0mcy+rSDeJmAc61ZRpqPq5KM/p/9h
   * 3PFaTWwyI0PurKju7koSCTxdccK+efrCh2gdC/1cacwG0Jp9VJkqyTkaGa9LKkPz
   * Y11aWOIv4x3kqdbQCtCev9eBCfHJxyYNrJgWVqA=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02NO1\x1d0\x1b\x06\x03U\x04\n\x0c\x14Buypass AS-9831633271 0\x1e\x06\x03U\x04\x03\x0c\x17Buypass Class 2 Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd7\xc7^\xf7\xc1\x07\xd4w\xfbC!\xf4\xf4\xf5i\xe4\xee2\x01\xdb\xa3\x86\x1f\xe4Y\r\xba\xe7u\x83R\xeb\xea\x1ca\x15H\xbb\x1d\x07\xca\x8c\xae\xb0\xdc\x96\x9d\xea\xc3`\x92\x86\x82(s\x9cV\x06\xffKd\xf0\x0c*7I\xb5\xe5\xcf\x0c|\xee\xf1J\xbbs0e\xf3\xd5/\x83\xb6~\xe3\xe7\xf5\x9e\xab`\xf9\xd3\xf1\x9d\x92t\x8a\xe4\x1c\x96\xac[\x80\xe9\xb5\xf41\x87\xa3Q\xfc\xc7~\xa1o\x8eSw\xd4\x97\xc1U3\x92>\x18/u\xd4\xad\x86I\xcb\x95\xafT\x06l\xd8\x06\x13\x8d[\xff\xe1&\x19Y\xc0$\xba\x81qy\x90DPh$\x94_\xb8\xb3\x11\xf1)Aa\xa3A\xcb#6\xd5\xc1\xf12P\x10N\x7f\xf4\x86\x93\xec\x84\xd3\x8e\xbcK\xbf\\\x01N\x07=\xdc\x14\x8a\x94\n\xa4\xeas\xfb\x0bQ\xe8\x13\x07\x18\xfa\x0e\xf1+\xd1T\x15}<\xe1\xf7\xb4\x19Bgb^w\xe0\xa2U\xec\xb6\xd9i\x17\xd5:\xafD\xedJ\xc5\x9e\xe4z\'|\xe5u\xd7\xaa\xcb%\xe7\xdfk\n\xdb\x0fM\x93N\xa8\xa0\xcd{.\xf2Y\x01j\xb7\r\xb8\x07\x81~\x8b8\x1b8\xe6\nW\x99=\xee!\xe8\xa3\xf5\x0c\x16\xdd\x8b\xec4\x8e\x9c*\x1c\x00\x15\x17\x8dh\x83\xd2p\x9f\x18\x08\xcd\x11h\xd5\xc9kR\xcd\xc4F\x8f\xdc\xb5\xf3\xd8Ws\x1e\xe9\x949\x04\xbf\xd3\xde8\xde\xb4S\xeci\x1c\xa2~\xc4\x8f\xe4\x1bp\xad\xf2\xa2\xf9\xfb\xf7\x16dfi\x9fIQ\xa2\xe2\x15\x18g\x06J\x7f\xd5l\xb5M\xb33\xe0a\xeb]\xbe\xe9\x98\x0f2\xd7\x1dK<.Z\x01R\x91\t\xf2\xdf\xea\x8d\xd8\x06@c\xaa\x11\xe4\xfe\xc37\x9e\x14R?\xf4\xe2\xcc\xf2a\x93\xd1\xfdgk\xd7R\xae\xbfh\xab@C\xa0W5Sx\xf0S\xf8aB\x07d\xc6\xd7o\x9bL8\rc\xacb\xaf6\x8b\xa2s\n\r\xf5!\xbdt\xaaM\xear\x03I\xdb\xc7_\x1dbc\xc7\xfd\xdd\x91\xec3\xee\xf5m\xb4n0h\xde\xc8\xd6&\xb0u^{\xb4\x07 \x98\xa1v2\xb8MlO\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=ACCVRAIZ1 O=ACCV OU=PKIACCV
   * Subject: CN=ACCVRAIZ1 O=ACCV OU=PKIACCV
   * Label: "ACCVRAIZ1"
   * Serial: 6828503384748696800
   * SHA256 Fingerprint: 9a:6e:c0:12:e1:a7:da:9d:be:34:19:4d:47:8a:d7:c0:db:18:22:fb:07:1d:f1:29:81:49:6e:d1:04:38:41:13
   * -----BEGIN CERTIFICATE-----
   * MIIH0zCCBbugAwIBAgIIXsO3pkN/pOAwDQYJKoZIhvcNAQEFBQAwQjESMBAGA1UE
   * AwwJQUNDVlJBSVoxMRAwDgYDVQQLDAdQS0lBQ0NWMQ0wCwYDVQQKDARBQ0NWMQsw
   * CQYDVQQGEwJFUzAeFw0xMTA1MDUwOTM3MzdaFw0zMDEyMzEwOTM3MzdaMEIxEjAQ
   * BgNVBAMMCUFDQ1ZSQUlaMTEQMA4GA1UECwwHUEtJQUNDVjENMAsGA1UECgwEQUND
   * VjELMAkGA1UEBhMCRVMwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCb
   * qau/YUqXry+XZpp0X9DZlv3P4uRm7x8fRzPCRKPfmt4ftVTdFXxpNRFvu8gMjmoY
   * HtiP2Ra8EEg2XPBjs5BaXCQ316PWywlxufEBcoSwfdtNgM3802/J+Nq2DoLSRYWo
   * G2ioPej0RGy9ocLLA76MPhMAhN9KSMDjIgro6TenGEyxCQ0jVn8ETdkXhBilyNpA
   * lHPrzg5XPAOBOp0KoVdDaaxXbXmQeOW1tDvYvEyNKKGno6e6Ak4l0Squ7a4DIrhr
   * IA8wKFSVf+DuzgpmndFALW4ir50awQUZ0m/A8p/4e7MCQvtQqR0tkw8jq8bBD5L/
   * 0KIV9VMJcRz/RROE5iZe+OCIHAr8Fraocwa48GOEAqDGWuzndN9wrqODJerWx5eH
   * k6fGioozl2A3ED6XPm4pFdahD9GILBKfb6qkxkLrQaLjlUPTAYVtjrs78yM2x/47
   * 4KElB0iryYl0/wiPgL/AlmXz7uxLaL2diMMxs0Dx6M/2OLuc5NF/1OVYm3z61PMO
   * m3WR5LpSLhl+0fXNWhn8ugb2+1KoS5kE3fj5tItQo05iifCHJPqDQsGH+tUtKSpa
   * cXpkatcnYGMN285J9Y0fkIkyF/hzQ7jSWpOGYdbhdQrqeWZ2iE9x6wQl1gpaepPl
   * uUsXQA+xtrn13k/c4LOsOxFwYIRKQ26ZIMApcQrAZQIDAQABo4ICyzCCAscwfQYI
   * KwYBBQUHAQEEcTBvMEwGCCsGAQUFBzAChkBodHRwOi8vd3d3LmFjY3YuZXMvZmls
   * ZWFkbWluL0FyY2hpdm9zL2NlcnRpZmljYWRvcy9yYWl6YWNjdjEuY3J0MB8GCCsG
   * AQUFBzABhhNodHRwOi8vb2NzcC5hY2N2LmVzMB0GA1UdDgQWBBTSh7Tj3zcnk1X2
   * VuqB5TbMjB4/vTAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFNKHtOPfNyeT
   * VfZW6oHlNsyMHj+9MIIBcwYDVR0gBIIBajCCAWYwggFiBgRVHSAAMIIBWDCCASIG
   * CCsGAQUFBwICMIIBFB6CARAAQQB1AHQAbwByAGkAZABhAGQAIABkAGUAIABDAGUA
   * cgB0AGkAZgBpAGMAYQBjAGkA8wBuACAAUgBhAO0AegAgAGQAZQAgAGwAYQAgAEEA
   * QwBDAFYAIAAoAEEAZwBlAG4AYwBpAGEAIABkAGUAIABUAGUAYwBuAG8AbABvAGcA
   * 7QBhACAAeQAgAEMAZQByAHQAaQBmAGkAYwBhAGMAaQDzAG4AIABFAGwAZQBjAHQA
   * cgDzAG4AaQBjAGEALAAgAEMASQBGACAAUQA0ADYAMAAxADEANQA2AEUAKQAuACAA
   * QwBQAFMAIABlAG4AIABoAHQAdABwADoALwAvAHcAdwB3AC4AYQBjAGMAdgAuAGUA
   * czAwBggrBgEFBQcCARYkaHR0cDovL3d3dy5hY2N2LmVzL2xlZ2lzbGFjaW9uX2Mu
   * aHRtMFUGA1UdHwROMEwwSqBIoEaGRGh0dHA6Ly93d3cuYWNjdi5lcy9maWxlYWRt
   * aW4vQXJjaGl2b3MvY2VydGlmaWNhZG9zL3JhaXphY2N2MV9kZXIuY3JsMA4GA1Ud
   * DwEB/wQEAwIBBjAXBgNVHREEEDAOgQxhY2N2QGFjY3YuZXMwDQYJKoZIhvcNAQEF
   * BQADggIBAJcxAp/n/UNnSEQU5CmH7UwoZtCPNdpNYbdKl02125DgBS4OxnnQ8pdp
   * D70ER9m+27Up2pvZrqmZ1dM8MJP1jaGo/AaNRPTKFpV8M9xii6g3+CfYCS0b78gU
   * JyCpZET/LtZ1qmxNYEAZSUNUY9rizLpm5U9EelvZaoErQNV/+QEnWCzI7UiRfD+m
   * AM/EKXMRNt6GGT6d7hmKG9Ww7Y49nCrADdg9ZuM8Db3VlFzi4qc1GwQA9j9ajepD
   * vV+JHanBsMyZ4k0ACtrJJ1vnE5Bc5PUzolVt3OAJTS+xJlsndQAJxGJ3KQhfnlms
   * tn6tn1QwIgPBHnFk/vk4CpYY3QIUrCPLBhwepH2NDd4nQeit2hW3sCPdK6jT2iWH
   * 7ehVRE2I9DZ+hJp4rPcOVkkO1jMl1oRQQmwgEh0q1b688nCBpHBgvgW1m54ERL5h
   * I6zppSSMEYCUWqKiuUnSwdzRp+0xESyeGabu4VXhwOrPDYTkF7eifKXeVSUG7szA
   * h1xA2syVP1XgNce4hL60Xc16gwFy7ofmXx2utYXGJt/mwZrpHgJHnyqobalbz+xF
   * d3+YJ5oyXSrjhO7FmGYvliAd3djDJ9ew+f7Zfc3Qn48LFFhRny+Lwzgt3uiP1o2H
   * pPVWQxaZLPSkVrQ0uGE3ycJYgBugl6H8WY3pEfbRD0tVNEYqi4Y7
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x120\x10\x06\x03U\x04\x03\x0c\tACCVRAIZ11\x100\x0e\x06\x03U\x04\x0b\x0c\x07PKIACCV1\r0\x0b\x06\x03U\x04\n\x0c\x04ACCV1\x0b0\t\x06\x03U\x04\x06\x13\x02ES"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x9b\xa9\xab\xbfaJ\x97\xaf/\x97f\x9at_\xd0\xd9\x96\xfd\xcf\xe2\xe4f\xef\x1f\x1fG3\xc2D\xa3\xdf\x9a\xde\x1f\xb5T\xdd\x15|i5\x11o\xbb\xc8\x0c\x8ej\x18\x1e\xd8\x8f\xd9\x16\xbc\x10H6\\\xf0c\xb3\x90Z\\$7\xd7\xa3\xd6\xcb\tq\xb9\xf1\x01r\x84\xb0}\xdbM\x80\xcd\xfc\xd3o\xc9\xf8\xda\xb6\x0e\x82\xd2E\x85\xa8\x1bh\xa8=\xe8\xf4Dl\xbd\xa1\xc2\xcb\x03\xbe\x8c>\x13\x00\x84\xdfJH\xc0\xe3\"\n\xe8\xe97\xa7\x18L\xb1\t\r#V\x7f\x04M\xd9\x17\x84\x18\xa5\xc8\xda@\x94s\xeb\xce\x0eW<\x03\x81:\x9d\n\xa1WCi\xacWmy\x90x\xe5\xb5\xb4;\xd8\xbcL\x8d(\xa1\xa7\xa3\xa7\xba\x02N%\xd1*\xae\xed\xae\x03\"\xb8k \x0f0(T\x95\x7f\xe0\xee\xce\nf\x9d\xd1@-n\"\xaf\x9d\x1a\xc1\x05\x19\xd2o\xc0\xf2\x9f\xf8{\xb3\x02B\xfbP\xa9\x1d-\x93\x0f#\xab\xc6\xc1\x0f\x92\xff\xd0\xa2\x15\xf5S\tq\x1c\xffE\x13\x84\xe6&^\xf8\xe0\x88\x1c\n\xfc\x16\xb6\xa8s\x06\xb8\xf0c\x84\x02\xa0\xc6Z\xec\xe7t\xdfp\xae\xa3\x83%\xea\xd6\xc7\x97\x87\x93\xa7\xc6\x8a\x8a3\x97`7\x10>\x97>n)\x15\xd6\xa1\x0f\xd1\x88,\x12\x9fo\xaa\xa4\xc6B\xebA\xa2\xe3\x95C\xd3\x01\x85m\x8e\xbb;\xf3#6\xc7\xfe;\xe0\xa1%\x07H\xab\xc9\x89t\xff\x08\x8f\x80\xbf\xc0\x96e\xf3\xee\xecKh\xbd\x9d\x88\xc31\xb3@\xf1\xe8\xcf\xf68\xbb\x9c\xe4\xd1\x7f\xd4\xe5X\x9b|\xfa\xd4\xf3\x0e\x9bu\x91\xe4\xbaR.\x19~\xd1\xf5\xcdZ\x19\xfc\xba\x06\xf6\xfbR\xa8K\x99\x04\xdd\xf8\xf9\xb4\x8bP\xa3Nb\x89\xf0\x87$\xfa\x83B\xc1\x87\xfa\xd5-)*Zqzdj\xd7\'`c\r\xdb\xceI\xf5\x8d\x1f\x90\x892\x17\xf8sC\xb8\xd2Z\x93\x86a\xd6\xe1u\n\xeayfv\x88Oq\xeb\x04%\xd6\nZz\x93\xe5\xb9K\x17@\x0f\xb1\xb6\xb9\xf5\xdeO\xdc\xe0\xb3\xac;\x11p`\x84JCn\x99 \xc0)q\n\xc0e\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=UCA Global G2 Root O=UniTrust
   * Subject: CN=UCA Global G2 Root O=UniTrust
   * Label: "UCA Global G2 Root"
   * Serial: 124779693093741543919145257850076631279
   * SHA256 Fingerprint: 9b:ea:11:c9:76:fe:01:47:64:c1:be:56:a6:f9:14:b5:a5:60:31:7a:bd:99:88:39:33:82:e5:16:1a:a0:49:3c
   * -----BEGIN CERTIFICATE-----
   * MIIFRjCCAy6gAwIBAgIQXd+x2lqj7V2+WmUgZQOQ7zANBgkqhkiG9w0BAQsFADA9
   * MQswCQYDVQQGEwJDTjERMA8GA1UECgwIVW5pVHJ1c3QxGzAZBgNVBAMMElVDQSBH
   * bG9iYWwgRzIgUm9vdDAeFw0xNjAzMTEwMDAwMDBaFw00MDEyMzEwMDAwMDBaMD0x
   * CzAJBgNVBAYTAkNOMREwDwYDVQQKDAhVbmlUcnVzdDEbMBkGA1UEAwwSVUNBIEds
   * b2JhbCBHMiBSb290MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAxeYr
   * b3zvJgUno4Ek2m/LAfmZmqkywiKHYUGRO8vDaBsGxUypK8FnFyIdK+35KYmToni9
   * kmugow2ifsqTs6bRjDXVdfkX9s9FxeV67HeToI8jrg4aA3++1NDtLnurRiNb/yzm
   * VHqUwCoV8MmNsHo7JOHXaOIxPAYzRrZUEaalLyJUKlgNAQLx+hVRZ2zA+te2G3/R
   * VogvGjqNO7uCEeBHANBSh6v7hn4PJGtAnTRnvI3HLYZveT6OqTwXS3+wmeOwcWDc
   * C/Vkw85DvG1xudLeJ1uK6NjGruFZfc8oLTW4lVYa8bJYS7cSN8h8s+1LgOGN+jIj
   * tm+3SJUIsUROhYw6AlQgL9+/V087OpAh18EmNVQg7Mc/R+zvWr9LesGtOxdQXGLY
   * D0tK3Cv6brxzks3sx1DoQZbXqX5t2Okdj4q1uViSukqSKwxW/YDrCPBeKW4bHAyv
   * j5OJrdu9o54hyokZ7N+1wxrrFv54NkzWbtA+FxyQF2smuvt6L78RHBgOLXMDj6Dl
   * NaBa4kx1HXHhOThTeEDMg5PXCp6dW4+K5OXgSORIskfNTip1KnvyIvbJvgmRlld6
   * iIis7nCs+dwp4wwcOxJORNanTrAmyPPZGpeRaOrvjUYG0lZFWJo8DA+DuAUlwznP
   * O6Q0ibd5Ei9Hxeepl2n8pndntd978XplFeRhVmUCAwEAAaNCMEAwDgYDVR0PAQH/
   * BAQDAgEGMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFIHEjMz15DD/pQwIX4wV
   * ZyF0Ad/fMA0GCSqGSIb3DQEBCwUAA4ICAQATZSL1jiutROTL/7lo5sOASD0Ee/oj
   * L3rtNtqyzm325p7lX1iPyzcyochltq44PTUbPrw7tgTQvPlJ9Zv3hcU2tsu8+Mg5
   * 1eRfB70VVJd0ysrtT7q6ZHafgbiERUlMjW+i67HM0cOU2kTC5uLqGOiiHycFutfl
   * 1qnN3e92mI0ADs0b+gO3joBYDic/UvuUospeZcnWhNq5NXHzJsBPd+aBJ9J3O5oU
   * b3n09tDh05S60FdRvScFDcH9yBIw7m+NESsIndTUv4BFFJqIRNow6rSn4+7vW4LV
   * PtateJLbXDzz2K36uGt/xDYotgIVilQsnLAXc47QN6MUPJiVAAwpBVueSUmxX8fj
   * y88nZY41F7dXyDDZQVu5FLbowg+UMaeUmMxq67XhJ/UQqAHojhJi6IjMtX9Gl8Cb
   * EGY4GjZGXyJoPd/JxhMnq1MGrKI8hgZlb7F+sSlEmqO6SWkoaY/X5V+tBIZkbxqg
   * DMUIYs6Ao9Dz7GjevjPHF1t/gMRMTLGmhIrDO7gJzRSBuhjjVFc2/tsvfEehOjPI
   * +Vg7RE+xygKJBJYoaMVLuCaJu9YzL1DV/pqJuhgyklTGW+Cd+V7lDSKb9triyCGy
   * YiGqhkCyLmTTX8jjfhFnRR8F/uOi77Oos/N9j/gMHyIfLXC0uAE0djAA5SN4p1bX
   * UB+K+wb1whnw0A==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1\x110\x0f\x06\x03U\x04\n\x0c\x08UniTrust1\x1b0\x19\x06\x03U\x04\x03\x0c\x12UCA Global G2 Root"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc5\xe6+o|\xef&\x05\'\xa3\x81$\xdao\xcb\x01\xf9\x99\x9a\xa92\xc2\"\x87aA\x91;\xcb\xc3h\x1b\x06\xc5L\xa9+\xc1g\x17\"\x1d+\xed\xf9)\x89\x93\xa2x\xbd\x92k\xa0\xa3\r\xa2~\xca\x93\xb3\xa6\xd1\x8c5\xd5u\xf9\x17\xf6\xcfE\xc5\xe5z\xecw\x93\xa0\x8f#\xae\x0e\x1a\x03\x7f\xbe\xd4\xd0\xed.{\xabF#[\xff,\xe6Tz\x94\xc0*\x15\xf0\xc9\x8d\xb0z;$\xe1\xd7h\xe21<\x063F\xb6T\x11\xa6\xa5/\"T*X\r\x01\x02\xf1\xfa\x15Qgl\xc0\xfa\xd7\xb6\x1b\x7f\xd1V\x88/\x1a:\x8d;\xbb\x82\x11\xe0G\x00\xd0R\x87\xab\xfb\x86~\x0f$k@\x9d4g\xbc\x8d\xc7-\x86oy>\x8e\xa9<\x17K\x7f\xb0\x99\xe3\xb0q`\xdc\x0b\xf5d\xc3\xceC\xbcmq\xb9\xd2\xde\'[\x8a\xe8\xd8\xc6\xae\xe1Y}\xcf(-5\xb8\x95V\x1a\xf1\xb2XK\xb7\x127\xc8|\xb3\xedK\x80\xe1\x8d\xfa2#\xb6o\xb7H\x95\x08\xb1DN\x85\x8c:\x02T /\xdf\xbfWO;:\x90!\xd7\xc1&5T \xec\xc7?G\xec\xefZ\xbfKz\xc1\xad;\x17P\\b\xd8\x0fKJ\xdc+\xfan\xbcs\x92\xcd\xec\xc7P\xe8A\x96\xd7\xa9~m\xd8\xe9\x1d\x8f\x8a\xb5\xb9X\x92\xbaJ\x92+\x0cV\xfd\x80\xeb\x08\xf0^)n\x1b\x1c\x0c\xaf\x8f\x93\x89\xad\xdb\xbd\xa3\x9e!\xca\x89\x19\xec\xdf\xb5\xc3\x1a\xeb\x16\xfex6L\xd6n\xd0>\x17\x1c\x90\x17k&\xba\xfbz/\xbf\x11\x1c\x18\x0e-s\x03\x8f\xa0\xe55\xa0Z\xe2Lu\x1dq\xe198Sx@\xcc\x83\x93\xd7\n\x9e\x9d[\x8f\x8a\xe4\xe5\xe0H\xe4H\xb2G\xcdN*u*{\xf2\"\xf6\xc9\xbe\t\x91\x96Wz\x88\x88\xac\xeep\xac\xf9\xdc)\xe3\x0c\x1c;\x12ND\xd6\xa7N\xb0&\xc8\xf3\xd9\x1a\x97\x91h\xea\xef\x8dF\x06\xd2VEX\x9a<\x0c\x0f\x83\xb8\x05%\xc39\xcf;\xa44\x89\xb7y\x12/G\xc5\xe7\xa9\x97i\xfc\xa6wg\xb5\xdf{\xf1ze\x15\xe4aVe\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Hellenic Academic and Research Institutions RootCA 2015 O=Hellenic Academic and Research Institutions Cert. Authority
   * Subject: CN=Hellenic Academic and Research Institutions RootCA 2015 O=Hellenic Academic and Research Institutions Cert. Authority
   * Label: "Hellenic Academic and Research Institutions RootCA 2015"
   * Serial: 0
   * SHA256 Fingerprint: a0:40:92:9a:02:ce:53:b4:ac:f4:f2:ff:c6:98:1c:e4:49:6f:75:5e:6d:45:fe:0b:2a:69:2b:cd:52:52:3f:36
   * -----BEGIN CERTIFICATE-----
   * MIIGCzCCA/OgAwIBAgIBADANBgkqhkiG9w0BAQsFADCBpjELMAkGA1UEBhMCR1Ix
   * DzANBgNVBAcTBkF0aGVuczFEMEIGA1UEChM7SGVsbGVuaWMgQWNhZGVtaWMgYW5k
   * IFJlc2VhcmNoIEluc3RpdHV0aW9ucyBDZXJ0LiBBdXRob3JpdHkxQDA+BgNVBAMT
   * N0hlbGxlbmljIEFjYWRlbWljIGFuZCBSZXNlYXJjaCBJbnN0aXR1dGlvbnMgUm9v
   * dENBIDIwMTUwHhcNMTUwNzA3MTAxMTIxWhcNNDAwNjMwMTAxMTIxWjCBpjELMAkG
   * A1UEBhMCR1IxDzANBgNVBAcTBkF0aGVuczFEMEIGA1UEChM7SGVsbGVuaWMgQWNh
   * ZGVtaWMgYW5kIFJlc2VhcmNoIEluc3RpdHV0aW9ucyBDZXJ0LiBBdXRob3JpdHkx
   * QDA+BgNVBAMTN0hlbGxlbmljIEFjYWRlbWljIGFuZCBSZXNlYXJjaCBJbnN0aXR1
   * dGlvbnMgUm9vdENBIDIwMTUwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoIC
   * AQDC+Kk/G4n8PDwEXT2QNrCROnk8ZlrvbTkBSRq0t89/TSNTt5AA4xMqKKYx8ZEA
   * 4yjsriFBzh/a/X0SWwGDD7mwX5nh8hKDgE0GPt+sr+ehiGsxr/CL0BgzuNtFajT0
   * AoAkKAoCFZVedioNmToUW/bLy1O8E00BiDeUJRtCvCLYjqOWXjrZMts+6PAQZe10
   * 4S+nfK8nNLspfZu2zwnI5dMK/IhlZXQK3HMcXM1AsRzUtoSMTFDPaI6oWa7CJ06C
   * ojXdFPQf/7J31Ycvqm59JCfnxssm5uX+Zwdj2EUN3TpZZTlYepKZcj2chF6IIbjV
   * 9Cz82XBST3i4vTwri5WY9bPRaM8gFH5MXF/ni+X1NYEZN9cRCLdmvtNKzoNXADrD
   * gfgXy5I2XdGj2HUb4Ysn6npIQf1FGQatJ5lOwXBH3bWfgVMS5bGMSF0xQxfjjMZ6
   * Y5ZLKTBOhE5iGV48zpeQpX8B653g+IuJ3SWYPZK2fu/Z8VFRfS0myGlZYeCsargq
   * NhEEelC9MoS+L9xy1dcdFkfkR2YgP/SWxa+OAXqlD3pk9Q0Yh9muiNX6hME6wGko
   * LfINaFGq46V3xqSQDqE3izEjR8EJCOtu93ib14L8hCCZSRm2Ekax+0VVFqmjZayc
   * Bw/qa9wfLgZy7IaIEuQt218FL+TwA9MmM+eAws1CoRc0CwIDAQABo0IwQDAPBgNV
   * HRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUcRVnyMjJvXVd
   * ctA4GGqd83EkVAswDQYJKoZIhvcNAQELBQADggIBAHW7bVRLqhBYRjTyYtcWNl0I
   * XtVsyIe9tC5G8jH4fOpCtZMWVdyhDBKg2mF+D1hYc2Ryx+hFjtyp8iY/xnmMsVMI
   * M4GwVhO+5lFc2JsKT0ucVlMC6U/2DWDqTUJV6HwbISHTGzrMd/K4kPFox/la/vot
   * 9L/J9UUbzjgQKjeKeaO04wlshYaT/4mWJ3iBj2fjRnRUjtkNaeJK9E10A/+yd+2V
   * Z5fkscWrv2oj6NSU4kQoYsRL4vDY4ilrGnB+JGGTe08DMiUNRSQrlrRGar9KC/ea
   * j8GsGsVn82800vpzY4zvFrCopEYq+OsS7HK07/grfoxSwIuEVPkvPuNVqNxmsdnh
   * X9izjFk0WaSrT2y7HxjbdavYy5LNlDhhDgcGH0tGEPEVvo2FXDtKK4F5D7Rpn0lQ
   * l033DlZdwJVqwjbDG2jJ9SrcR5q+ss7FJej6A7na+RZukYT1HCjI/CbM1xyQVqdf
   * bzoEvM14iQuODy+jqk+iGxI9FghAD/FGTNeqewjBCvVtJ94Cj8rDtSvK6evIIVM4
   * pcw72Hc3MKJP2W/R8kCtQXoXxdZKNYm3QdV8hn9VTYNKpXMgwDqvkPGaJI7ZjnHK
   * e7iG2rKPmT4dEw0SEe7Uq/DpFXYC5ODfqiAeW2GFZECpkJcNrVPSWh2HagCXZWK0
   * vm9qp/UsQu0yrbYhnr68
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GR1\x0f0\r\x06\x03U\x04\x07\x13\x06Athens1D0B\x06\x03U\x04\n\x13;Hellenic Academic and Research Institutions Cert. Authority1@0>\x06\x03U\x04\x03\x137Hellenic Academic and Research Institutions RootCA 2015"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc2\xf8\xa9?\x1b\x89\xfc<<\x04]=\x906\xb0\x91:y<fZ\xefm9\x01I\x1a\xb4\xb7\xcf\x7fM#S\xb7\x90\x00\xe3\x13*(\xa61\xf1\x91\x00\xe3(\xec\xae!A\xce\x1f\xda\xfd}\x12[\x01\x83\x0f\xb9\xb0_\x99\xe1\xf2\x12\x83\x80M\x06>\xdf\xac\xaf\xe7\xa1\x88k1\xaf\xf0\x8b\xd0\x183\xb8\xdbEj4\xf4\x02\x80$(\n\x02\x15\x95^v*\r\x99:\x14[\xf6\xcb\xcbS\xbc\x13M\x01\x887\x94%\x1bB\xbc\"\xd8\x8e\xa3\x96^:\xd92\xdb>\xe8\xf0\x10e\xedt\xe1/\xa7|\xaf\'4\xbb)}\x9b\xb6\xcf\t\xc8\xe5\xd3\n\xfc\x88eet\n\xdcs\x1c\\\xcd@\xb1\x1c\xd4\xb6\x84\x8cLP\xcfh\x8e\xa8Y\xae\xc2\'N\x82\xa25\xdd\x14\xf4\x1f\xff\xb2w\xd5\x87/\xaan}$\'\xe7\xc6\xcb&\xe6\xe5\xfeg\x07c\xd8E\r\xdd:Ye9Xz\x92\x99r=\x9c\x84^\x88!\xb8\xd5\xf4,\xfc\xd9pROx\xb8\xbd<+\x8b\x95\x98\xf5\xb3\xd1h\xcf \x14~L\\_\xe7\x8b\xe5\xf55\x81\x197\xd7\x11\x08\xb7f\xbe\xd3J\xce\x83W\x00:\xc3\x81\xf8\x17\xcb\x926]\xd1\xa3\xd8u\x1b\xe1\x8b\'\xeazHA\xfdE\x19\x06\xad\'\x99N\xc1pG\xdd\xb5\x9f\x81S\x12\xe5\xb1\x8cH]1C\x17\xe3\x8c\xc6zc\x96K)0N\x84Nb\x19^<\xce\x97\x90\xa5\x7f\x01\xeb\x9d\xe0\xf8\x8b\x89\xdd%\x98=\x92\xb6~\xef\xd9\xf1QQ}-&\xc8iYa\xe0\xacj\xb8*6\x11\x04zP\xbd2\x84\xbe/\xdcr\xd5\xd7\x1d\x16G\xe4Gf ?\xf4\x96\xc5\xaf\x8e\x01z\xa5\x0fzd\xf5\r\x18\x87\xd9\xae\x88\xd5\xfa\x84\xc1:\xc0i(-\xf2\rhQ\xaa\xe3\xa5w\xc6\xa4\x90\x0e\xa17\x8b1#G\xc1\t\x08\xebn\xf7x\x9b\xd7\x82\xfc\x84 \x99I\x19\xb6\x12F\xb1\xfbEU\x16\xa9\xa3e\xac\x9c\x07\x0f\xeak\xdc\x1f.\x06r\xec\x86\x88\x12\xe4-\xdb_\x05/\xe4\xf0\x03\xd3&3\xe7\x80\xc2\xcdB\xa1\x174\x0b\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SZAFIR ROOT CA2 O=Krajowa Izba Rozliczeniowa S.A.
   * Subject: CN=SZAFIR ROOT CA2 O=Krajowa Izba Rozliczeniowa S.A.
   * Label: "SZAFIR ROOT CA2"
   * Serial: 357043034767186914217277344587386743377558296292
   * SHA256 Fingerprint: a1:33:9d:33:28:1a:0b:56:e5:57:d3:d3:2b:1c:e7:f9:36:7e:b0:94:bd:5f:a7:2a:7e:50:04:c8:de:d7:ca:fe
   * -----BEGIN CERTIFICATE-----
   * MIIDcjCCAlqgAwIBAgIUPopdB+xV0jLVt+O2XwHrLdzk1uQwDQYJKoZIhvcNAQEL
   * BQAwUTELMAkGA1UEBhMCUEwxKDAmBgNVBAoMH0tyYWpvd2EgSXpiYSBSb3psaWN6
   * ZW5pb3dhIFMuQS4xGDAWBgNVBAMMD1NaQUZJUiBST09UIENBMjAeFw0xNTEwMTkw
   * NzQzMzBaFw0zNTEwMTkwNzQzMzBaMFExCzAJBgNVBAYTAlBMMSgwJgYDVQQKDB9L
   * cmFqb3dhIEl6YmEgUm96bGljemVuaW93YSBTLkEuMRgwFgYDVQQDDA9TWkFGSVIg
   * Uk9PVCBDQTIwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC3vD5QqEvN
   * QLXOYeeWyrSh2gwisPq1e3YAd4wLz32ohswmUeQgPYUM1ljj5/QqGJ3a0a4m7utT
   * 3PSQ1hNKDJA8w/Ta0o4NkjrcsbH/ON7Dui1fgLkCvUqdGw+0w8LBZwPd3BucPbOw
   * 3gAeqDRHu5rr/gsUvTaE2g0gv/pby6kWIK05YO4vdbbnl5z5Pv1+TW9NL++IDWr6
   * 3fE9biCloBK0TXC5ztdyO4mTp4CEHCdJckm1/zuVnsHMyAHs6A6KCpbns6aH5db5
   * BSsNl0BwPLqsdVqc1U2dAgrSS5tmS0YHF2Wtn2yIANwiieDhZNRnvDF5YTy7ykHN
   * XGoAyDw4jlivAgMBAAGjQjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQD
   * AgEGMB0GA1UdDgQWBBQuFqlKGLXLzPVvUPMjX/hd56zwyDANBgkqhkiG9w0BAQsF
   * AAOCAQEAtXP4A9xZWx126aMqe5Aosk3AM0+qmrHUuOQn/6mWmc5G4G18TKI4pAZw
   * 8PRBEew/R40/cof5O/2kbytTAOD/OblqBw7rHRz2onKQy4I9EYKL0rufKq8h5mOG
   * nXkZ7/e7DDWQw4rtTw/1zBLZpD67oPwglV9PJi8RI4NOdQcPv5vRtB3pEAT+ymCP
   * oky4rc/hkA/NrgrHXXu3UNLUYfrVFdvXn4dRVOul4+vJhaAlIDf7js4MNIThPIGy
   * d05DpYhfhmehPea0XGG2Ptv+tyjFogeutcrKjSoS75ftwjCkySp6+/NNIxuZMzSg
   * LvWpCz/UXeHPhJ/iGcJfitYgHuNztw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02PL1(0&\x06\x03U\x04\n\x0c\x1fKrajowa Izba Rozliczeniowa S.A.1\x180\x16\x06\x03U\x04\x03\x0c\x0fSZAFIR ROOT CA2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb7\xbc>P\xa8K\xcd@\xb5\xcea\xe7\x96\xca\xb4\xa1\xda\x0c\"\xb0\xfa\xb5{v\x00w\x8c\x0b\xcf}\xa8\x86\xcc&Q\xe4 =\x85\x0c\xd6X\xe3\xe7\xf4*\x18\x9d\xda\xd1\xae&\xee\xebS\xdc\xf4\x90\xd6\x13J\x0c\x90<\xc3\xf4\xda\xd2\x8e\r\x92:\xdc\xb1\xb1\xff8\xde\xc3\xba-_\x80\xb9\x02\xbdJ\x9d\x1b\x0f\xb4\xc3\xc2\xc1g\x03\xdd\xdc\x1b\x9c=\xb3\xb0\xde\x00\x1e\xa84G\xbb\x9a\xeb\xfe\x0b\x14\xbd6\x84\xda\r \xbf\xfa[\xcb\xa9\x16 \xad9`\xee/u\xb6\xe7\x97\x9c\xf9>\xfd~MoM/\xef\x88\rj\xfa\xdd\xf1=n \xa5\xa0\x12\xb4Mp\xb9\xce\xd7r;\x89\x93\xa7\x80\x84\x1c\'IrI\xb5\xff;\x95\x9e\xc1\xcc\xc8\x01\xec\xe8\x0e\x8a\n\x96\xe7\xb3\xa6\x87\xe5\xd6\xf9\x05+\r\x97@p<\xba\xacuZ\x9c\xd5M\x9d\x02\n\xd2K\x9bfKF\x07\x17e\xad\x9fl\x88\x00\xdc\"\x89\xe0\xe1d\xd4g\xbc1ya<\xbb\xcaA\xcd\\j\x00\xc8<8\x8eX\xaf\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign O=GlobalSign OU=GlobalSign ECC Root CA - R4
   * Subject: CN=GlobalSign O=GlobalSign OU=GlobalSign ECC Root CA - R4
   * Label: "GlobalSign"
   * Serial: 159662223612894884239637590694
   * SHA256 Fingerprint: b0:85:d7:0b:96:4f:19:1a:73:e4:af:0d:54:ae:7a:0e:07:aa:fd:af:9b:71:dd:08:62:13:8a:b7:32:5a:24:a2
   * -----BEGIN CERTIFICATE-----
   * MIIB3DCCAYOgAwIBAgINAgPlfvU/k/2lCSGypjAKBggqhkjOPQQDAjBQMSQwIgYD
   * VQQLExtHbG9iYWxTaWduIEVDQyBSb290IENBIC0gUjQxEzARBgNVBAoTCkdsb2Jh
   * bFNpZ24xEzARBgNVBAMTCkdsb2JhbFNpZ24wHhcNMTIxMTEzMDAwMDAwWhcNMzgw
   * MTE5MDMxNDA3WjBQMSQwIgYDVQQLExtHbG9iYWxTaWduIEVDQyBSb290IENBIC0g
   * UjQxEzARBgNVBAoTCkdsb2JhbFNpZ24xEzARBgNVBAMTCkdsb2JhbFNpZ24wWTAT
   * BgcqhkjOPQIBBggqhkjOPQMBBwNCAAS4xnnTj2wlDp8uORkcA6SumuU5BwkWymOx
   * uYb4ilfBV85C+nOh92VC/x7BALJucw7/xyHlGKSq2XE/qNS5zowdo0IwQDAOBgNV
   * HQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUVLB7rUW44kB/
   * +wpu+74zyTyjhNUwCgYIKoZIzj0EAwIDRwAwRAIgIk90crlgr/HmnKAWBVBfw147
   * bmF0774BxL4YSFlhgjICICadVGNA3jdgUM/I2O2dgq43mLyjj0xMqTQrbO/7lZsm
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1$0\"\x06\x03U\x04\x0b\x13\x1bGlobalSign ECC Root CA - R41\x130\x11\x06\x03U\x04\n\x13\nGlobalSign1\x130\x11\x06\x03U\x04\x03\x13\nGlobalSign"),
    subject_public_key_info: Der::from_slice(b"0\x13\x06\x07*\x86H\xce=\x02\x01\x06\x08*\x86H\xce=\x03\x01\x07\x03B\x00\x04\xb8\xc6y\xd3\x8fl%\x0e\x9f.9\x19\x1c\x03\xa4\xae\x9a\xe59\x07\t\x16\xcac\xb1\xb9\x86\xf8\x8aW\xc1W\xceB\xfas\xa1\xf7eB\xff\x1e\xc1\x00\xb2ns\x0e\xff\xc7!\xe5\x18\xa4\xaa\xd9q?\xa8\xd4\xb9\xce\x8c\x1d"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Atos TrustedRoot Root CA ECC TLS 2021 O=Atos
   * Subject: CN=Atos TrustedRoot Root CA ECC TLS 2021 O=Atos
   * Label: "Atos TrustedRoot Root CA ECC TLS 2021"
   * Serial: 81873346711060652204712539181482831616
   * SHA256 Fingerprint: b2:fa:e5:3e:14:cc:d7:ab:92:12:06:47:01:ae:27:9c:1d:89:88:fa:cb:77:5f:a8:a0:08:91:4e:66:39:88:a8
   * -----BEGIN CERTIFICATE-----
   * MIICFTCCAZugAwIBAgIQPZg7pmY9kGP3fiZXOATvADAKBggqhkjOPQQDAzBMMS4w
   * LAYDVQQDDCVBdG9zIFRydXN0ZWRSb290IFJvb3QgQ0EgRUNDIFRMUyAyMDIxMQ0w
   * CwYDVQQKDARBdG9zMQswCQYDVQQGEwJERTAeFw0yMTA0MjIwOTI2MjNaFw00MTA0
   * MTcwOTI2MjJaMEwxLjAsBgNVBAMMJUF0b3MgVHJ1c3RlZFJvb3QgUm9vdCBDQSBF
   * Q0MgVExTIDIwMjExDTALBgNVBAoMBEF0b3MxCzAJBgNVBAYTAkRFMHYwEAYHKoZI
   * zj0CAQYFK4EEACIDYgAEloZYKDcKZ9Cg3iQZGeHkBQcfl+3oZIK59sRxUM6KDP/X
   * tXa7oWyTbIOiaG6l2b4siJVBzV3dscqDY4PMwL502eCdpO5KTlbgmClBk1IQ1SQ4
   * AjJn8ZQSb+/Xxd4u/RmAo0IwQDAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBR2
   * KCXWfeBmmnoJsmo7jjPXNtNPojAOBgNVHQ8BAf8EBAMCAYYwCgYIKoZIzj0EAwMD
   * aAAwZQIwW5kp85wxtolrbNa9d+F851F+uDrNozZffPc8dz7kUK2o59JZDCaOMDtu
   * CCrCp1rIAjEAmeMM56PDr9NJLkaCI2ZdyQAUEv049OGYa3cpetskz2VAv9LcjBHo
   * 9H1/IISpQuQo
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1.0,\x06\x03U\x04\x03\x0c%Atos TrustedRoot Root CA ECC TLS 20211\r0\x0b\x06\x03U\x04\n\x0c\x04Atos1\x0b0\t\x06\x03U\x04\x06\x13\x02DE"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x96\x86X(7\ng\xd0\xa0\xde$\x19\x19\xe1\xe4\x05\x07\x1f\x97\xed\xe8d\x82\xb9\xf6\xc4qP\xce\x8a\x0c\xff\xd7\xb5v\xbb\xa1l\x93l\x83\xa2hn\xa5\xd9\xbe,\x88\x95A\xcd]\xdd\xb1\xca\x83c\x83\xcc\xc0\xbet\xd9\xe0\x9d\xa4\xeeJNV\xe0\x98)A\x93R\x10\xd5$8\x022g\xf1\x94\x12o\xef\xd7\xc5\xde.\xfd\x19\x80"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certainly Root E1 O=Certainly
   * Subject: CN=Certainly Root E1 O=Certainly
   * Label: "Certainly Root E1"
   * Serial: 8168531406727139161245376702891150584
   * SHA256 Fingerprint: b4:58:5f:22:e4:ac:75:6a:4e:86:12:a1:36:1c:5d:9d:03:1a:93:fd:84:fe:bb:77:8f:a3:06:8b:0f:c4:2d:c2
   * -----BEGIN CERTIFICATE-----
   * MIIB9zCCAX2gAwIBAgIQBiUzsUcDMydc+Y2aub/M+DAKBggqhkjOPQQDAzA9MQsw
   * CQYDVQQGEwJVUzESMBAGA1UEChMJQ2VydGFpbmx5MRowGAYDVQQDExFDZXJ0YWlu
   * bHkgUm9vdCBFMTAeFw0yMTA0MDEwMDAwMDBaFw00NjA0MDEwMDAwMDBaMD0xCzAJ
   * BgNVBAYTAlVTMRIwEAYDVQQKEwlDZXJ0YWlubHkxGjAYBgNVBAMTEUNlcnRhaW5s
   * eSBSb290IEUxMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAE3m/4fxzf7flHh4axpMCK
   * +IKXgOqPyEpeKn2IaKcBYhSRJHpcnqMXfYqGITQYUBsQ3tA3SybHGWCA6TS9YBk2
   * QNYphwk8kXr2vBMj3VlOBF7PyAIcGFPBMdjaIOlEjeR2o0IwQDAOBgNVHQ8BAf8E
   * BAMCAQYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQU8ygYy2R17ikq6+2uI1g4
   * hevIIgcwCgYIKoZIzj0EAwMDaAAwZQIxALGOWiDDshliTd6wT99u0nCK8Z9+aozm
   * ut6Dacpps6kFtZaSF4fC0urQe87YQVt8rgIwRt7qy12a7DLCZRawTDBcMPPaTnOG
   * BtjOiQRINzf43TNRnXCve1XYAS59BWQOhriR
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x13\tCertainly1\x1a0\x18\x06\x03U\x04\x03\x13\x11Certainly Root E1"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xdeo\xf8\x7f\x1c\xdf\xed\xf9G\x87\x86\xb1\xa4\xc0\x8a\xf8\x82\x97\x80\xea\x8f\xc8J^*}\x88h\xa7\x01b\x14\x91$z\\\x9e\xa3\x17}\x8a\x86!4\x18P\x1b\x10\xde\xd07K&\xc7\x19`\x80\xe94\xbd`\x196@\xd6)\x87\t<\x91z\xf6\xbc\x13#\xddYN\x04^\xcf\xc8\x02\x1c\x18S\xc11\xd8\xda \xe9D\x8d\xe4v"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certum Trusted Network CA 2 O=Unizeto Technologies S.A. OU=Certum Certification Authority
   * Subject: CN=Certum Trusted Network CA 2 O=Unizeto Technologies S.A. OU=Certum Certification Authority
   * Label: "Certum Trusted Network CA 2"
   * Serial: 44979900017204383099463764357512596969
   * SHA256 Fingerprint: b6:76:f2:ed:da:e8:77:5c:d3:6c:b0:f6:3c:d1:d4:60:39:61:f4:9e:62:65:ba:01:3a:2f:03:07:b6:d0:b8:04
   * -----BEGIN CERTIFICATE-----
   * MIIF0jCCA7qgAwIBAgIQIdbQSk8lD8kyN/yqXhKN6TANBgkqhkiG9w0BAQ0FADCB
   * gDELMAkGA1UEBhMCUEwxIjAgBgNVBAoTGVVuaXpldG8gVGVjaG5vbG9naWVzIFMu
   * QS4xJzAlBgNVBAsTHkNlcnR1bSBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTEkMCIG
   * A1UEAxMbQ2VydHVtIFRydXN0ZWQgTmV0d29yayBDQSAyMCIYDzIwMTExMDA2MDgz
   * OTU2WhgPMjA0NjEwMDYwODM5NTZaMIGAMQswCQYDVQQGEwJQTDEiMCAGA1UEChMZ
   * VW5pemV0byBUZWNobm9sb2dpZXMgUy5BLjEnMCUGA1UECxMeQ2VydHVtIENlcnRp
   * ZmljYXRpb24gQXV0aG9yaXR5MSQwIgYDVQQDExtDZXJ0dW0gVHJ1c3RlZCBOZXR3
   * b3JrIENBIDIwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQC9+Xj45tWA
   * DGSdhhuWZGc/IjoedQF97/tcZ4zJzFxrqZHmuULlIEub2pt7uZld2ZuAS9eEQCsn
   * 0+i6MLs+CRqnSZXvK0AkwpfHp+6bJe+oCgCXhVqqndwpyeI1B+twTUrWwbNWuKFB
   * OJvR+zF/j+Bf4bE/D44WSWDXBo0Y+aomEKsq09DRZ40bRr5HMNUuctHFY9rnY3lE
   * fktjJImGLjQ/KUxSiyqnwOKRKIm5wFv5HdnnJ63/mgKXwcZQkpsCLL2puTRZCr+E
   * Sv/f/rOf69me4Jgj7KZrdxYq28ytOxykh9xGc14ZYmhFV+SQgkK7QtbwYeDBoz1m
   * o130GO6IyY0XRSmZMnUCMe4pJshrAua1YkV/NxVaI2iJ1D7eTiew8EAMvE0Xy02i
   * sx7QBlrd9pPPV3WZ9fqGGmd4s7+W/jTcvedSVuWz5XV710GRBdxdaeOVDUO5/IOW
   * OZV7bIBaTxNyxtd9KXpEulKkKtVBRgkg/iKgtlswjbyJDNXXcPiHUv3a76xRLgez
   * Tv7QCdpw75j6VuZt27VXS9zlLCUVyJ4ueE742pyehizKV/Ma5ciSixqClnrDvFAS
   * adgOWkaLOusm+iPJtrCBvkIApPjW/jAux9JG9uWOdf3yzLnQh1vMBhBgu4M1t15n
   * 3kfsmUjxpKEV/q2MYo45VU85FrmxY53/twIDAQABo0IwQDAPBgNVHRMBAf8EBTAD
   * AQH/MB0GA1UdDgQWBBS2oVQ5AsOgP46KvPrU+Bym0ToO/TAOBgNVHQ8BAf8EBAMC
   * AQYwDQYJKoZIhvcNAQENBQADggIBAHGlDs7k6b8/ONWJWsQCYftMxRQXLYtPU2sQ
   * F/xlhMcQSZDe28cmk4gmb3DWAl45oPePq5a1pRNcgRRtDoGCERuKTsZPpd1iHkTf
   * CVn0W3cLN+mLIMb4Ck4uWBzrM9DPhmDJ2vuAL55MYIR4PSFk1vtBHxgP58l1cb29
   * XN40hz5BsA72udY/CROWFC/emh1auVbONTqwX3BNXuMp8SMoclm2q8KMZiYcdywm
   * djWLKKdpoPk79SPdhRB0yZADVpHnr7pH1BKXESLjokmUbOe3lEu6LaTaM4tMpkT/
   * WjzGHWTYtTHkpjx6qFcL2+1hGsvxznN3Y6SHb0xRONbkX8eftoEq5IVIeVheO/jb
   * AoJnwTnbw3RLPTYe+SmTiGhbqEQZIfCn6IENLOiTNrQ3ssqwGyZ6miUfmpqAnksq
   * P/ujmv5zMnHCnsZy4YpoJ/HkD7TETKVhk/iXEAcqMCWpuchxuO9ozC1+9eB+D4Ko
   * b7a6bINDd82Kkhehnlt4Fj1F4jNy3eFmypnTycUm/Q1oBEauttmbjL4ZvrHG8hnj
   * XALKLNhvSgfZyTXaQHXyxKcZb55CEJh15pWLYLztxRLXis7VmFxWlgPF7ncGNf/P
   * 5O4/E2Hu29othfDNrp2yGAlFw5Khchf8R7agCyzxxN5DaAhqXzvwdmP7zAYspsbi
   * DrW5viSP
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02PL1\"0 \x06\x03U\x04\n\x13\x19Unizeto Technologies S.A.1\'0%\x06\x03U\x04\x0b\x13\x1eCertum Certification Authority1$0\"\x06\x03U\x04\x03\x13\x1bCertum Trusted Network CA 2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xbd\xf9x\xf8\xe6\xd5\x80\x0cd\x9d\x86\x1b\x96dg?\":\x1eu\x01}\xef\xfb\\g\x8c\xc9\xcc\\k\xa9\x91\xe6\xb9B\xe5 K\x9b\xda\x9b{\xb9\x99]\xd9\x9b\x80K\xd7\x84@+\'\xd3\xe8\xba0\xbb>\t\x1a\xa7I\x95\xef+@$\xc2\x97\xc7\xa7\xee\x9b%\xef\xa8\n\x00\x97\x85Z\xaa\x9d\xdc)\xc9\xe25\x07\xebpMJ\xd6\xc1\xb3V\xb8\xa1A8\x9b\xd1\xfb1\x7f\x8f\xe0_\xe1\xb1?\x0f\x8e\x16I`\xd7\x06\x8d\x18\xf9\xaa&\x10\xab*\xd3\xd0\xd1g\x8d\x1bF\xbeG0\xd5.r\xd1\xc5c\xda\xe7cyD~Kc$\x89\x86.4?)LR\x8b*\xa7\xc0\xe2\x91(\x89\xb9\xc0[\xf9\x1d\xd9\xe7\'\xad\xff\x9a\x02\x97\xc1\xc6P\x92\x9b\x02,\xbd\xa9\xb94Y\n\xbf\x84J\xff\xdf\xfe\xb3\x9f\xeb\xd9\x9e\xe0\x98#\xec\xa6kw\x16*\xdb\xcc\xad;\x1c\xa4\x87\xdcFs^\x19bhEW\xe4\x90\x82B\xbbB\xd6\xf0a\xe0\xc1\xa3=f\xa3]\xf4\x18\xee\x88\xc9\x8d\x17E)\x992u\x021\xee)&\xc8k\x02\xe6\xb5bE\x7f7\x15Z#h\x89\xd4>\xdeN\'\xb0\xf0@\x0c\xbcM\x17\xcbM\xa2\xb3\x1e\xd0\x06Z\xdd\xf6\x93\xcfWu\x99\xf5\xfa\x86\x1agx\xb3\xbf\x96\xfe4\xdc\xbd\xe7RV\xe5\xb3\xe5u{\xd7A\x91\x05\xdc]i\xe3\x95\rC\xb9\xfc\x83\x969\x95{l\x80ZO\x13r\xc6\xd7})zD\xbaR\xa4*\xd5AF\t \xfe\"\xa0\xb6[0\x8d\xbc\x89\x0c\xd5\xd7p\xf8\x87R\xfd\xda\xef\xacQ.\x07\xb3N\xfe\xd0\t\xdap\xef\x98\xfaV\xe6m\xdb\xb5WK\xdc\xe5,%\x15\xc8\x9e.xN\xf8\xda\x9c\x9e\x86,\xcaW\xf3\x1a\xe5\xc8\x92\x8b\x1a\x82\x96z\xc3\xbcP\x12i\xd8\x0eZF\x8b:\xeb&\xfa#\xc9\xb6\xb0\x81\xbeB\x00\xa4\xf8\xd6\xfe0.\xc7\xd2F\xf6\xe5\x8eu\xfd\xf2\xcc\xb9\xd0\x87[\xcc\x06\x10`\xbb\x835\xb7^g\xdeG\xec\x99H\xf1\xa4\xa1\x15\xfe\xad\x8cb\x8e9UO9\x16\xb9\xb1c\x9d\xff\xb7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=emSign ECC Root CA - C3 O=eMudhra Inc OU=emSign PKI
   * Subject: CN=emSign ECC Root CA - C3 O=eMudhra Inc OU=emSign PKI
   * Label: "emSign ECC Root CA - C3"
   * Serial: 582948710642506000014504
   * SHA256 Fingerprint: bc:4d:80:9b:15:18:9d:78:db:3e:1d:8c:f4:f9:72:6a:79:5d:a1:64:3c:a5:f1:35:8e:1d:db:0e:dc:0d:7e:b3
   * -----BEGIN CERTIFICATE-----
   * MIICKzCCAbGgAwIBAgIKe3G2gla4EnycqDAKBggqhkjOPQQDAzBaMQswCQYDVQQG
   * EwJVUzETMBEGA1UECxMKZW1TaWduIFBLSTEUMBIGA1UEChMLZU11ZGhyYSBJbmMx
   * IDAeBgNVBAMTF2VtU2lnbiBFQ0MgUm9vdCBDQSAtIEMzMB4XDTE4MDIxODE4MzAw
   * MFoXDTQzMDIxODE4MzAwMFowWjELMAkGA1UEBhMCVVMxEzARBgNVBAsTCmVtU2ln
   * biBQS0kxFDASBgNVBAoTC2VNdWRocmEgSW5jMSAwHgYDVQQDExdlbVNpZ24gRUND
   * IFJvb3QgQ0EgLSBDMzB2MBAGByqGSM49AgEGBSuBBAAiA2IABP2lYa57JhAd6bci
   * MK4G9IGzsUJxlTm801Ljr6/58pc1kjZGDoeVjbk5Wum739D+yAdBPLtVb4Ojavti
   * sIGJAnB9SMVK4+kiVCJNk7tCDK93nCOmfddhEc5lx/h//vXyqaNCMEAwHQYDVR0O
   * BBYEFPtaSNCAIEDyqOkAB2kZd6fmw/TPMA4GA1UdDwEB/wQEAwIBBjAPBgNVHRMB
   * Af8EBTADAQH/MAoGCCqGSM49BAMDA2gAMGUCMQC02C8Cif22TGK6Q04ThHK1rt0c
   * 3ta13FaPWEBaLd4gTCKDypOofu4SQMfWh0/434UCMBwUZOR8loMRnLDRWmFLpg9J
   * 0wD8ofzkpf9/rdcw0Md3f76BB1UwUCAU9Vc4CqgxUQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x130\x11\x06\x03U\x04\x0b\x13\nemSign PKI1\x140\x12\x06\x03U\x04\n\x13\x0beMudhra Inc1 0\x1e\x06\x03U\x04\x03\x13\x17emSign ECC Root CA - C3"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xfd\xa5a\xae{&\x10\x1d\xe9\xb7\"0\xae\x06\xf4\x81\xb3\xb1Bq\x959\xbc\xd3R\xe3\xaf\xaf\xf9\xf2\x975\x926F\x0e\x87\x95\x8d\xb99Z\xe9\xbb\xdf\xd0\xfe\xc8\x07A<\xbbUo\x83\xa3j\xfbb\xb0\x81\x89\x02p}H\xc5J\xe3\xe9\"T\"M\x93\xbbB\x0c\xafw\x9c#\xa6}\xd7a\x11\xcee\xc7\xf8\x7f\xfe\xf5\xf2\xa9"),
    name_constraints: None
  },

  /*
   * Issuer: CN=AffirmTrust Premium ECC O=AffirmTrust
   * Subject: CN=AffirmTrust Premium ECC O=AffirmTrust
   * Label: "AffirmTrust Premium ECC"
   * Serial: 8401224907861490260
   * SHA256 Fingerprint: bd:71:fd:f6:da:97:e4:cf:62:d1:64:7a:dd:25:81:b0:7d:79:ad:f8:39:7e:b4:ec:ba:9c:5e:84:88:82:14:23
   * -----BEGIN CERTIFICATE-----
   * MIIB/jCCAYWgAwIBAgIIdJclisc/elQwCgYIKoZIzj0EAwMwRTELMAkGA1UEBhMC
   * VVMxFDASBgNVBAoMC0FmZmlybVRydXN0MSAwHgYDVQQDDBdBZmZpcm1UcnVzdCBQ
   * cmVtaXVtIEVDQzAeFw0xMDAxMjkxNDIwMjRaFw00MDEyMzExNDIwMjRaMEUxCzAJ
   * BgNVBAYTAlVTMRQwEgYDVQQKDAtBZmZpcm1UcnVzdDEgMB4GA1UEAwwXQWZmaXJt
   * VHJ1c3QgUHJlbWl1bSBFQ0MwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAAQNMF4bFZ0D
   * 0KF5Nbc6PJJ6yhUczWLznCZcBz3lVPqj1swS6vQUX+iOGasvLkjmrBhDeKzQN8O9
   * ss0s5kfiGuZjuD0uL3jET9v0D6RoTFVya5UdThhClXjMNzyR4ptlKymjQjBAMB0G
   * A1UdDgQWBBSaryl6wBE1NSZRMADDav5A1a7WPDAPBgNVHRMBAf8EBTADAQH/MA4G
   * A1UdDwEB/wQEAwIBBjAKBggqhkjOPQQDAwNnADBkAjAXCfOHiFBar8jAQr9HX/Vs
   * aobgxCd05DhT1wV/GzTjxi+zygk8N53X57hG8f2h4nECMEJZh0PUUd+60wkyWs6I
   * flc9nF9Ca/UHLbXwgpP5WW+uZPpY5Yse42O+tYHNbwKMeQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x140\x12\x06\x03U\x04\n\x0c\x0bAffirmTrust1 0\x1e\x06\x03U\x04\x03\x0c\x17AffirmTrust Premium ECC"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\r0^\x1b\x15\x9d\x03\xd0\xa1y5\xb7:<\x92z\xca\x15\x1c\xcdb\xf3\x9c&\\\x07=\xe5T\xfa\xa3\xd6\xcc\x12\xea\xf4\x14_\xe8\x8e\x19\xab/.H\xe6\xac\x18Cx\xac\xd07\xc3\xbd\xb2\xcd,\xe6G\xe2\x1a\xe6c\xb8=./x\xc4O\xdb\xf4\x0f\xa4hLUrk\x95\x1dN\x18B\x95x\xcc7<\x91\xe2\x9be+)"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TrustAsia Global Root CA G4 O=TrustAsia Technologies, Inc.
   * Subject: CN=TrustAsia Global Root CA G4 O=TrustAsia Technologies, Inc.
   * Label: "TrustAsia Global Root CA G4"
   * Serial: 451799571007117016466790293371524403291602933463
   * SHA256 Fingerprint: be:4b:56:cb:50:56:c0:13:6a:52:6d:f4:44:50:8d:aa:36:a0:b5:4f:42:e4:ac:38:f7:2a:f4:70:e4:79:65:4c
   * -----BEGIN CERTIFICATE-----
   * MIICVTCCAdygAwIBAgIUTyNkuI6XY57GU4HBdk7LKnQV1tcwCgYIKoZIzj0EAwMw
   * WjELMAkGA1UEBhMCQ04xJTAjBgNVBAoMHFRydXN0QXNpYSBUZWNobm9sb2dpZXMs
   * IEluYy4xJDAiBgNVBAMMG1RydXN0QXNpYSBHbG9iYWwgUm9vdCBDQSBHNDAeFw0y
   * MTA1MjAwMjEwMjJaFw00NjA1MTkwMjEwMjJaMFoxCzAJBgNVBAYTAkNOMSUwIwYD
   * VQQKDBxUcnVzdEFzaWEgVGVjaG5vbG9naWVzLCBJbmMuMSQwIgYDVQQDDBtUcnVz
   * dEFzaWEgR2xvYmFsIFJvb3QgQ0EgRzQwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAATx
   * s8045CVD5d4ZCbuBeaIVXxVjAd7Cq92zphtnS4CDr5nLrBfbK5bKfFJV4hrhPVbw
   * LxYI+hW8m7tH5j/uqOFMjPXTNvk4XatwmkcN4oFBButJ+bAp3TPsUKV/eSm4IJij
   * YzBhMA8GA1UdEwEB/wQFMAMBAf8wHwYDVR0jBBgwFoAUpbtKl86zK3+kMd6Xg1mD
   * pm9xy94wHQYDVR0OBBYEFKW7SpfOsyt/pDHel4NZg6ZvccveMA4GA1UdDwEB/wQE
   * AwIBBjAKBggqhkjOPQQDAwNnADBkAjBe8usGzEkxn0AAbbd+NvBNEU/zy4k6LHiR
   * UKNbwMp1JvK/kF0LgoxgKJ/GcJpo5PECMFxYDlZ2z1jD1xCMuo6u47xkdUfFVZDj
   * /bpV6wfEU6s3qe4hsiFbYI89MvHVI5TWWA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1%0#\x06\x03U\x04\n\x0c\x1cTrustAsia Technologies, Inc.1$0\"\x06\x03U\x04\x03\x0c\x1bTrustAsia Global Root CA G4"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xf1\xb3\xcd8\xe4%C\xe5\xde\x19\t\xbb\x81y\xa2\x15_\x15c\x01\xde\xc2\xab\xdd\xb3\xa6\x1bgK\x80\x83\xaf\x99\xcb\xac\x17\xdb+\x96\xca|RU\xe2\x1a\xe1=V\xf0/\x16\x08\xfa\x15\xbc\x9b\xbbG\xe6?\xee\xa8\xe1L\x8c\xf5\xd36\xf98]\xabp\x9aG\r\xe2\x81A\x06\xebI\xf9\xb0)\xdd3\xecP\xa5\x7fy)\xb8 \x98"),
    name_constraints: None
  },

  /*
   * Issuer: CN=e-Szigno Root CA 2017 O=Microsec Ltd.
   * Subject: CN=e-Szigno Root CA 2017 O=Microsec Ltd.
   * Label: "e-Szigno Root CA 2017"
   * Serial: 411379200276854331539784714
   * SHA256 Fingerprint: be:b0:0b:30:83:9b:9b:c3:2c:32:e4:44:79:05:95:06:41:f2:64:21:b1:5e:d0:89:19:8b:51:8a:e2:ea:1b:99
   * -----BEGIN CERTIFICATE-----
   * MIICQDCCAeWgAwIBAgIMAVRI7yH9l1kN9QQKMAoGCCqGSM49BAMCMHExCzAJBgNV
   * BAYTAkhVMREwDwYDVQQHDAhCdWRhcGVzdDEWMBQGA1UECgwNTWljcm9zZWMgTHRk
   * LjEXMBUGA1UEYQwOVkFUSFUtMjM1ODQ0OTcxHjAcBgNVBAMMFWUtU3ppZ25vIFJv
   * b3QgQ0EgMjAxNzAeFw0xNzA4MjIxMjA3MDZaFw00MjA4MjIxMjA3MDZaMHExCzAJ
   * BgNVBAYTAkhVMREwDwYDVQQHDAhCdWRhcGVzdDEWMBQGA1UECgwNTWljcm9zZWMg
   * THRkLjEXMBUGA1UEYQwOVkFUSFUtMjM1ODQ0OTcxHjAcBgNVBAMMFWUtU3ppZ25v
   * IFJvb3QgQ0EgMjAxNzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABJbcPYrYsHtv
   * xie+RJCxs1YVe45DJH0ahFnuY2iyxl6H0BVIHqiQrb1TotreOpCmYF9oMrWGQd+H
   * Wyx7xf58etqjYzBhMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0G
   * A1UdDgQWBBSHERUI0arBeAyxr87GyZDvvzAEwDAfBgNVHSMEGDAWgBSHERUI0arB
   * eAyxr87GyZDvvzAEwDAKBggqhkjOPQQDAgNJADBGAiEAtVfd14pVCzbhhkT61Nlo
   * jbjcI4qKDdQvfepz7L9NbKgCIQDLpbQS+ue16M9+k/zzNY9vTlp8tLxOsvxyqltZ
   * +efcMQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02HU1\x110\x0f\x06\x03U\x04\x07\x0c\x08Budapest1\x160\x14\x06\x03U\x04\n\x0c\rMicrosec Ltd.1\x170\x15\x06\x03U\x04a\x0c\x0eVATHU-235844971\x1e0\x1c\x06\x03U\x04\x03\x0c\x15e-Szigno Root CA 2017"),
    subject_public_key_info: Der::from_slice(b"0\x13\x06\x07*\x86H\xce=\x02\x01\x06\x08*\x86H\xce=\x03\x01\x07\x03B\x00\x04\x96\xdc=\x8a\xd8\xb0{o\xc6\'\xbeD\x90\xb1\xb3V\x15{\x8eC$}\x1a\x84Y\xeech\xb2\xc6^\x87\xd0\x15H\x1e\xa8\x90\xad\xbdS\xa2\xda\xde:\x90\xa6`_h2\xb5\x86A\xdf\x87[,{\xc5\xfe|z\xda"),
    name_constraints: None
  },

  /*
   * Issuer: CN=FIRMAPROFESIONAL CA ROOT-A WEB O=Firmaprofesional SA
   * Subject: CN=FIRMAPROFESIONAL CA ROOT-A WEB O=Firmaprofesional SA
   * Label: "FIRMAPROFESIONAL CA ROOT-A WEB"
   * Serial: 65916896770016886708751106294915943533
   * SHA256 Fingerprint: be:f2:56:da:f2:6e:9c:69:bd:ec:16:02:35:97:98:f3:ca:f7:18:21:a0:3e:01:82:57:c5:3c:65:61:7f:3d:4a
   * -----BEGIN CERTIFICATE-----
   * MIICejCCAgCgAwIBAgIQMZch7a+JQn81QYehZ1ZMbTAKBggqhkjOPQQDAzBuMQsw
   * CQYDVQQGEwJFUzEcMBoGA1UECgwTRmlybWFwcm9mZXNpb25hbCBTQTEYMBYGA1UE
   * YQwPVkFURVMtQTYyNjM0MDY4MScwJQYDVQQDDB5GSVJNQVBST0ZFU0lPTkFMIENB
   * IFJPT1QtQSBXRUIwHhcNMjIwNDA2MDkwMTM2WhcNNDcwMzMxMDkwMTM2WjBuMQsw
   * CQYDVQQGEwJFUzEcMBoGA1UECgwTRmlybWFwcm9mZXNpb25hbCBTQTEYMBYGA1UE
   * YQwPVkFURVMtQTYyNjM0MDY4MScwJQYDVQQDDB5GSVJNQVBST0ZFU0lPTkFMIENB
   * IFJPT1QtQSBXRUIwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAARHU+osEaR3xyrq89Zf
   * e9MEkVz6iMYiuYMQYneEMy3pA4jU4DP37XcsSmDq5G+tbbT4TIqk5B/K6k84Si6C
   * cyvHZpsKjECcfIr28jlgst7L7Ljkb+qbXbdTkBgyVcUgt5SjYzBhMA8GA1UdEwEB
   * /wQFMAMBAf8wHwYDVR0jBBgwFoAUk+FDY1w8ndYn81LsF7Kpryz3dvgwHQYDVR0O
   * BBYEFJPhQ2NcPJ3WJ/NS7Beyqa8s93b4MA4GA1UdDwEB/wQEAwIBBjAKBggqhkjO
   * PQQDAwNoADBlAjAdfKR7w4l1M+E7qUW/Runpod3JIha3RxEL2Jq68cgLcFBTApFw
   * hVmpHqTm6iMxoAACMQD94vizrxa5HnPEluPBMBnYfubDl94cT7iJLzPrSA8Z94dG
   * XSaQpYXFuXqUPoeovQA=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02ES1\x1c0\x1a\x06\x03U\x04\n\x0c\x13Firmaprofesional SA1\x180\x16\x06\x03U\x04a\x0c\x0fVATES-A626340681\'0%\x06\x03U\x04\x03\x0c\x1eFIRMAPROFESIONAL CA ROOT-A WEB"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04GS\xea,\x11\xa4w\xc7*\xea\xf3\xd6_{\xd3\x04\x91\\\xfa\x88\xc6\"\xb9\x83\x10bw\x843-\xe9\x03\x88\xd4\xe03\xf7\xedw,J`\xea\xe4o\xadm\xb4\xf8L\x8a\xa4\xe4\x1f\xca\xeaO8J.\x82s+\xc7f\x9b\n\x8c@\x9c|\x8a\xf6\xf29`\xb2\xde\xcb\xec\xb8\xe4o\xea\x9b]\xb7S\x90\x182U\xc5 \xb7\x94"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TWCA Root Certification Authority O=TAIWAN-CA OU=Root CA
   * Subject: CN=TWCA Root Certification Authority O=TAIWAN-CA OU=Root CA
   * Label: "TWCA Root Certification Authority"
   * Serial: 1
   * SHA256 Fingerprint: bf:d8:8f:e1:10:1c:41:ae:3e:80:1b:f8:be:56:35:0e:e9:ba:d1:a6:b9:bd:51:5e:dc:5c:6d:5b:87:11:ac:44
   * -----BEGIN CERTIFICATE-----
   * MIIDezCCAmOgAwIBAgIBATANBgkqhkiG9w0BAQUFADBfMQswCQYDVQQGEwJUVzES
   * MBAGA1UECgwJVEFJV0FOLUNBMRAwDgYDVQQLDAdSb290IENBMSowKAYDVQQDDCFU
   * V0NBIFJvb3QgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkwHhcNMDgwODI4MDcyNDMz
   * WhcNMzAxMjMxMTU1OTU5WjBfMQswCQYDVQQGEwJUVzESMBAGA1UECgwJVEFJV0FO
   * LUNBMRAwDgYDVQQLDAdSb290IENBMSowKAYDVQQDDCFUV0NBIFJvb3QgQ2VydGlm
   * aWNhdGlvbiBBdXRob3JpdHkwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIB
   * AQCwfnK4pAOU5qfeCTiRShFAh6d8WWQUe7UREN3+v9XAu1bihSX0NXIP+FPQQeFE
   * AcK0HMMxQhZHhTMidrIKbw/lJVBPhYa+v5guEGcevhEFhgWQxFnQfHgQsIBct+HH
   * K3XLfJ+utdGdIzdjp9xCoi2SBBtQwXu4PhvJVgSLL1KbralW6cH/ralYhzC2gfeX
   * RfwZVzsrb+RH9JlF/h3x+JejiB03HFyP4HYlmlD4oFT/RJB2I9IyxsOrBr/8+7/z
   * rX2SYgJbKdM1o5OaQ2RgXbL6Mv87BK9NQGr5x+PvI/1ry+UPizgN7gr8/g+YnzAx
   * 3WxSZfmLgb4i4RxYA7qRG4kHAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV
   * HRMBAf8EBTADAQH/MB0GA1UdDgQWBBRqOFsmjd6LWvJPelSDGRjjCDWmujANBgkq
   * hkiG9w0BAQUFAAOCAQEAPNV3PdrfibqHDAhUaiBQkr6wQT25JmSDCi/oQMCXKCeC
   * MErJk/9q56YAf4lCmtYR5VPOL8zy2gXE/uJQxDqGfczafhAJO5I1KlOy/usrBdls
   * XebQ79NqZp4VKIV66IIArB6nCWlWQtNoURi+VJq/REG6Sb4gumlc7rh3zc5sH62D
   * lhh9DrUUOYTxKOkto557HnpyWoOzeW/vtPzQCqVYT0bf+215WfKEIlKuD8z7fDvn
   * aspHYcN6+NOSBB+4IIThNlQWx0DeO4pz3N/GCUzf7Nr/1FNCocnyYh0igzyXxfkZ
   * YiesZSLX0zzG5Y6yU8xJzrww/nsOM5D77dIUkR8Hrw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TW1\x120\x10\x06\x03U\x04\n\x0c\tTAIWAN-CA1\x100\x0e\x06\x03U\x04\x0b\x0c\x07Root CA1*0(\x06\x03U\x04\x03\x0c!TWCA Root Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb0~r\xb8\xa4\x03\x94\xe6\xa7\xde\t8\x91J\x11@\x87\xa7|Yd\x14{\xb5\x11\x10\xdd\xfe\xbf\xd5\xc0\xbbV\xe2\x85%\xf45r\x0f\xf8S\xd0A\xe1D\x01\xc2\xb4\x1c\xc31B\x16G\x853\"v\xb2\no\x0f\xe5%PO\x85\x86\xbe\xbf\x98.\x10g\x1e\xbe\x11\x05\x86\x05\x90\xc4Y\xd0|x\x10\xb0\x80\\\xb7\xe1\xc7+u\xcb|\x9f\xae\xb5\xd1\x9d#7c\xa7\xdcB\xa2-\x92\x04\x1bP\xc1{\xb8>\x1b\xc9V\x04\x8b/R\x9b\xad\xa9V\xe9\xc1\xff\xad\xa9X\x870\xb6\x81\xf7\x97E\xfc\x19W;+o\xe4G\xf4\x99E\xfe\x1d\xf1\xf8\x97\xa3\x88\x1d7\x1c\\\x8f\xe0v%\x9aP\xf8\xa0T\xffD\x90v#\xd22\xc6\xc3\xab\x06\xbf\xfc\xfb\xbf\xf3\xad}\x92b\x02[)\xd35\xa3\x93\x9aCd`]\xb2\xfa2\xff;\x04\xafM@j\xf9\xc7\xe3\xef#\xfdk\xcb\xe5\x0f\x8b8\r\xee\n\xfc\xfe\x0f\x98\x9f01\xddlRe\xf9\x8b\x81\xbe\"\xe1\x1cX\x03\xba\x91\x1b\x89\x07\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GDCA TrustAUTH R5 ROOT O=GUANG DONG CERTIFICATE AUTHORITY CO.,LTD.
   * Subject: CN=GDCA TrustAUTH R5 ROOT O=GUANG DONG CERTIFICATE AUTHORITY CO.,LTD.
   * Label: "GDCA TrustAUTH R5 ROOT"
   * Serial: 9009899650740120186
   * SHA256 Fingerprint: bf:ff:8f:d0:44:33:48:7d:6a:8a:a6:0c:1a:29:76:7a:9f:c2:bb:b0:5e:42:0f:71:3a:13:b9:92:89:1d:38:93
   * -----BEGIN CERTIFICATE-----
   * MIIFiDCCA3CgAwIBAgIIfQmX/vBH6nowDQYJKoZIhvcNAQELBQAwYjELMAkGA1UE
   * BhMCQ04xMjAwBgNVBAoMKUdVQU5HIERPTkcgQ0VSVElGSUNBVEUgQVVUSE9SSVRZ
   * IENPLixMVEQuMR8wHQYDVQQDDBZHRENBIFRydXN0QVVUSCBSNSBST09UMB4XDTE0
   * MTEyNjA1MTMxNVoXDTQwMTIzMTE1NTk1OVowYjELMAkGA1UEBhMCQ04xMjAwBgNV
   * BAoMKUdVQU5HIERPTkcgQ0VSVElGSUNBVEUgQVVUSE9SSVRZIENPLixMVEQuMR8w
   * HQYDVQQDDBZHRENBIFRydXN0QVVUSCBSNSBST09UMIICIjANBgkqhkiG9w0BAQEF
   * AAOCAg8AMIICCgKCAgEA2aMW8Mh0dHeb7zMNOwZ+Vfy1YI92hhJCfVZmPoiC7XJj
   * Dp6L3TQsAlFRwxn9WVSEyfFrs0yw6ehGXTjGoqcuEVe6ghWinI9tsJlKCvLriXBj
   * TnnEt1u9ol2x8kECK62pOqPseQrsXzrj/e+APK00mxqriCZ7VqKChh/rNYmDf1+u
   * KU49tm7srsHwJ5uu4/Ts765/94Y9cnrrpftZTqfrlYwiOXnhLQiPzLyRuEH3FMEj
   * qcOtmkVEs7LXLM3GKeJQEK5cy4KOFxg2fZfmiJqwTTQJ9Cy5WmYqsBebnh52nUpm
   * MUHfP/vFBu8btn4aRjb3ZGM74zkYI+dndRTVdVeSN72+ahsmUPI2JgaQxXABZG12
   * ZuGR224HwGGALrIuL4xwp9E7PLOR5G62xDtw8mySlwnNR30YwPO7ng/Wi64HtloP
   * zgsMR6flPri9fcebNaBhlzpBdRfMK5Z3KpIhHtmVdiBnaM8Nvd/WHwlqmuLMc3Gk
   * L30SgLdTMEZeS1SZD2fJpcjyIMGC7J0R38IC+xo70e0gmu9lZJIQDSri3nDxGGeC
   * jGHeuLzRL5z7D9Ar7Rt2ueQ5Vfj4oR24qoAATILnsn8JuLwwoC8N9VKejveSswoA
   * HQBUlwbgsQfZxw9cZX08bVlX5O2ljelAU58VS6Bx9hoh49pwBiFYFIeFd3mqgnkC
   * AwEAAaNCMEAwHQYDVR0OBBYEFOLJQJ9NzuiaoXzPDj9lxSmIahlRMA8GA1UdEwEB
   * /wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMA0GCSqGSIb3DQEBCwUAA4ICAQDRSVfg
   * p8xoWLoBDysZzY2wYUWsEe1jUGn4H3++Fo/9nesLqjJHdtJnJO29fDMylyrHBYZm
   * DRd9FBUb1Ov9H5r2XpdptxolpAqzkT9fNqyL7FeoPueBihhXOYV0GkLH6VsTX4/5
   * COmSdI31R9KrO9b7eGZONn356ZLpBN79SWP8bfsUcZNnL0dKt7n/HipzcEYwv1ry
   * L3ml4Y0M2fmyYzeMN2WFcGpcWwlyua1jPLHd+PwyvzeG5LuOmCd+uh8W4XAR8gPf
   * JWIyJyYYMoSf/wA6E7qaTfRPuBRwIrHKK5DOKcFw9C+df/KQHtZa37dG/OaG+svg
   * IHZ6uqbL9XzeYqWxi+7egmaKTjowHz+Ay60nugxe19CxVsp3cbK1daFQqUBDF8Io
   * 2c9Si1vIY9RCPqAzekYu9wogRlR+ak8x8YF+QnQ4ZXMn7sZ8uI7XpTrXmKGcjBBV
   * 09tL7ECQ8s1uV9JiDnxXk7Gnbc2dg7sq5+W2O3FYrf3RRbxake5TFW/TRQl1brqQ
   * XR4EzzffHqhmsYzmIGrv/EhOdJhCrylvLmrH+33RZjEizIYAfmaDDEL0vTSSwxrq
   * T8p+ck0LcIymSLumoRT2+1hEmRSuqguTaaApJUqlyyvdimYHFngVV3Eb7PVHhPOe
   * MTd61X8kreS8/f3MboPoDKi3QWwH3b08hpcv0g==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1200\x06\x03U\x04\n\x0c)GUANG DONG CERTIFICATE AUTHORITY CO.,LTD.1\x1f0\x1d\x06\x03U\x04\x03\x0c\x16GDCA TrustAUTH R5 ROOT"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd9\xa3\x16\xf0\xc8ttw\x9b\xef3\r;\x06~U\xfc\xb5`\x8fv\x86\x12B}Vf>\x88\x82\xedrc\x0e\x9e\x8b\xdd4,\x02QQ\xc3\x19\xfdYT\x84\xc9\xf1k\xb3L\xb0\xe9\xe8F]8\xc6\xa2\xa7.\x11W\xba\x82\x15\xa2\x9c\x8fm\xb0\x99J\n\xf2\xeb\x89pcNy\xc4\xb7[\xbd\xa2]\xb1\xf2A\x02+\xad\xa9:\xa3\xecy\n\xec_:\xe3\xfd\xef\x80<\xad4\x9b\x1a\xab\x88&{V\xa2\x82\x86\x1f\xeb5\x89\x83\x7f_\xae)N=\xb6n\xec\xae\xc1\xf0\'\x9b\xae\xe3\xf4\xec\xef\xae\x7f\xf7\x86=rz\xeb\xa5\xfbYN\xa7\xeb\x95\x8c\"9y\xe1-\x08\x8f\xcc\xbc\x91\xb8A\xf7\x14\xc1#\xa9\xc3\xad\x9aED\xb3\xb2\xd7,\xcd\xc6)\xe2P\x10\xae\\\xcb\x82\x8e\x17\x186}\x97\xe6\x88\x9a\xb0M4\t\xf4,\xb9Zf*\xb0\x17\x9b\x9e\x1ev\x9dJf1A\xdf?\xfb\xc5\x06\xef\x1b\xb6~\x1aF6\xf7dc;\xe39\x18#\xe7gu\x14\xd5uW\x927\xbd\xbej\x1b&P\xf26&\x06\x90\xc5p\x01dmvf\xe1\x91\xdbn\x07\xc0a\x80.\xb2./\x8cp\xa7\xd1;<\xb3\x91\xe4n\xb6\xc4;p\xf2l\x92\x97\t\xcdG}\x18\xc0\xf3\xbb\x9e\x0f\xd6\x8b\xae\x07\xb6Z\x0f\xce\x0b\x0cG\xa7\xe5>\xb8\xbd}\xc7\x9b5\xa0a\x97:Au\x17\xcc+\x96w*\x92!\x1e\xd9\x95v gh\xcf\r\xbd\xdf\xd6\x1f\tj\x9a\xe2\xccsq\xa4/}\x12\x80\xb7S0F^KT\x99\x0fg\xc9\xa5\xc8\xf2 \xc1\x82\xec\x9d\x11\xdf\xc2\x02\xfb\x1a;\xd1\xed \x9a\xefed\x92\x10\r*\xe2\xdep\xf1\x18g\x82\x8ca\xde\xb8\xbc\xd1/\x9c\xfb\x0f\xd0+\xed\x1bv\xb9\xe49U\xf8\xf8\xa1\x1d\xb8\xaa\x80\x00L\x82\xe7\xb2\x7f\t\xb8\xbc0\xa0/\r\xf5R\x9e\x8e\xf7\x92\xb3\n\x00\x1d\x00T\x97\x06\xe0\xb1\x07\xd9\xc7\x0f\\e}<mYW\xe4\xed\xa5\x8d\xe9@S\x9f\x15K\xa0q\xf6\x1a!\xe3\xdap\x06!X\x14\x87\x85wy\xaa\x82y\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TrustAsia TLS ECC Root CA O=TrustAsia Technologies, Inc.
   * Subject: CN=TrustAsia TLS ECC Root CA O=TrustAsia Technologies, Inc.
   * Label: "TrustAsia TLS ECC Root CA"
   * Serial: 310892014698942880364840003424242768478804666567
   * SHA256 Fingerprint: c0:07:6b:9e:f0:53:1f:b1:a6:56:d6:7c:4e:be:97:cd:5d:ba:a4:1e:f4:45:98:ac:c2:48:98:78:c9:2d:87:11
   * -----BEGIN CERTIFICATE-----
   * MIICMTCCAbegAwIBAgIUNnThTXxlE8msg1UloD5Sfi9QaMcwCgYIKoZIzj0EAwMw
   * WDELMAkGA1UEBhMCQ04xJTAjBgNVBAoTHFRydXN0QXNpYSBUZWNobm9sb2dpZXMs
   * IEluYy4xIjAgBgNVBAMTGVRydXN0QXNpYSBUTFMgRUNDIFJvb3QgQ0EwHhcNMjQw
   * NTE1MDU0MTU2WhcNNDQwNTE1MDU0MTU1WjBYMQswCQYDVQQGEwJDTjElMCMGA1UE
   * ChMcVHJ1c3RBc2lhIFRlY2hub2xvZ2llcywgSW5jLjEiMCAGA1UEAxMZVHJ1c3RB
   * c2lhIFRMUyBFQ0MgUm9vdCBDQTB2MBAGByqGSM49AgEGBSuBBAAiA2IABLh/pVs/
   * AT598IhtrimY4ZtcU5nb9wj/1WrgjstEpvDBjL1P1M7UiFPoXlfXTr4sP/MSpwDp
   * guMqWzJ8S5sUKZ74LYO1644xST0mYekdcouJtgq7nDM1D9rs3qlKH8kzsaNCMEAw
   * DwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQULIVTu7FDzTLqnqOH/qKYqKaT6RAw
   * DgYDVR0PAQH/BAQDAgEGMAoGCCqGSM49BAMDA2gAMGUCMFRH18MtYYZI9HlaVQ01
   * L18N9mdsd0AaRuf4aFtOJx24mH1/k78ITcTaRTChD15KeAIxAKORh/IRM4PDwYqR
   * OkwrULG9IpRdNYlzg8WbGf60oenUoWa2AaU2+dhoYSi3dOGiMQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1%0#\x06\x03U\x04\n\x13\x1cTrustAsia Technologies, Inc.1\"0 \x06\x03U\x04\x03\x13\x19TrustAsia TLS ECC Root CA"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xb8\x7f\xa5[?\x01>}\xf0\x88m\xae)\x98\xe1\x9b\\S\x99\xdb\xf7\x08\xff\xd5j\xe0\x8e\xcbD\xa6\xf0\xc1\x8c\xbdO\xd4\xce\xd4\x88S\xe8^W\xd7N\xbe,?\xf3\x12\xa7\x00\xe9\x82\xe3*[2|K\x9b\x14)\x9e\xf8-\x83\xb5\xeb\x8e1I=&a\xe9\x1dr\x8b\x89\xb6\n\xbb\x9c35\x0f\xda\xec\xde\xa9J\x1f\xc93\xb1"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SSL.com TLS ECC Root CA 2022 O=SSL Corporation
   * Subject: CN=SSL.com TLS ECC Root CA 2022 O=SSL Corporation
   * Label: "SSL.com TLS ECC Root CA 2022"
   * Serial: 26605119622390491762507526719404364228
   * SHA256 Fingerprint: c3:2f:fd:9f:46:f9:36:d1:6c:36:73:99:09:59:43:4b:9a:d6:0a:af:bb:9e:7c:f3:36:54:f1:44:cc:1b:a1:43
   * -----BEGIN CERTIFICATE-----
   * MIICOjCCAcCgAwIBAgIQFAP1q/s3ixdAW+JDsqXRxDAKBggqhkjOPQQDAzBOMQsw
   * CQYDVQQGEwJVUzEYMBYGA1UECgwPU1NMIENvcnBvcmF0aW9uMSUwIwYDVQQDDBxT
   * U0wuY29tIFRMUyBFQ0MgUm9vdCBDQSAyMDIyMB4XDTIyMDgyNTE2MzM0OFoXDTQ2
   * MDgxOTE2MzM0N1owTjELMAkGA1UEBhMCVVMxGDAWBgNVBAoMD1NTTCBDb3Jwb3Jh
   * dGlvbjElMCMGA1UEAwwcU1NMLmNvbSBUTFMgRUNDIFJvb3QgQ0EgMjAyMjB2MBAG
   * ByqGSM49AgEGBSuBBAAiA2IABEUpNXP6wrgjzhR9qLFNoFs27iosU8NgCTWyJGYm
   * acCzldZdkkAZDsalE3D07xJRKF3nzL35PIXBz5SQySvOkkJYWWf9lCcQZIxPBLFN
   * SeR7T5v15wj4A4j3p8OSSxlUgaNjMGEwDwYDVR0TAQH/BAUwAwEB/zAfBgNVHSME
   * GDAWgBSJjy+j6CugFFR781a4Jl9nOAuc0DAdBgNVHQ4EFgQUiY8vo+groBRUe/NW
   * uCZfZzgLnNAwDgYDVR0PAQH/BAQDAgGGMAoGCCqGSM49BAMDA2gAMGUCMFXjIlbp
   * 15IkWE8elDIPDAI2wv2sdDJO4fscgIijzPvX6yv/N33w7deedWo1dlJF4AIxAMeN
   * b0Igj762TVntd00pxCAgRWSGOlDGxK0tk/UYfXLtqc/ErFc2KAhl3zx5Zn6g6g==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x180\x16\x06\x03U\x04\n\x0c\x0fSSL Corporation1%0#\x06\x03U\x04\x03\x0c\x1cSSL.com TLS ECC Root CA 2022"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04E)5s\xfa\xc2\xb8#\xce\x14}\xa8\xb1M\xa0[6\xee*,S\xc3`\t5\xb2$f&i\xc0\xb3\x95\xd6]\x92@\x19\x0e\xc6\xa5\x13p\xf4\xef\x12Q(]\xe7\xcc\xbd\xf9<\x85\xc1\xcf\x94\x90\xc9+\xce\x92BXYg\xfd\x94\'\x10d\x8cO\x04\xb1MI\xe4{O\x9b\xf5\xe7\x08\xf8\x03\x88\xf7\xa7\xc3\x92K\x19T\x81"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Microsoft RSA Root Certificate Authority 2017 O=Microsoft Corporation
   * Subject: CN=Microsoft RSA Root Certificate Authority 2017 O=Microsoft Corporation
   * Label: "Microsoft RSA Root Certificate Authority 2017"
   * Serial: 40975477897264996090493496164228220339
   * SHA256 Fingerprint: c7:41:f7:0f:4b:2a:8d:88:bf:2e:71:c1:41:22:ef:53:ef:10:eb:a0:cf:a5:e6:4c:fa:20:f4:18:85:30:73:e0
   * -----BEGIN CERTIFICATE-----
   * MIIFqDCCA5CgAwIBAgIQHtOXCV/YtLNHcB6qvn9FszANBgkqhkiG9w0BAQwFADBl
   * MQswCQYDVQQGEwJVUzEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMTYw
   * NAYDVQQDEy1NaWNyb3NvZnQgUlNBIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5
   * IDIwMTcwHhcNMTkxMjE4MjI1MTIyWhcNNDIwNzE4MjMwMDIzWjBlMQswCQYDVQQG
   * EwJVUzEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMTYwNAYDVQQDEy1N
   * aWNyb3NvZnQgUlNBIFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTcwggIi
   * MA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDKW76UM4wplZEWCpW9R2LBifOZ
   * Nt9GkMml7Xhqb0eRaPgnZ1AzHaGm++DlQ6OEAlcBXZxIQIJTELy/xztokLaCLeX0
   * ZdDMbRnMlfl7rEqUrQ7eS0MdhweSE5CAg2Q1OQT85elss7YfUJQ4ZVBcF0a5toW1
   * HLUX6NZFndiyJrDKxHBKrmCk3bPZ7Pw71VdyvD/IybLeS2v4I2wDwAW9lcfNcztm
   * gGTjGqwu+UcF8ga2m3P1eDNbx6H7JyqhtJqRjJHTOoI+dkC0zVJhUXAoP8XFWvLJ
   * jEm7FFtNyP9nTUwSlq31/niol4fX/V4ggNyhSyL71Imtus5Hl0dVe49FyGcohJUc
   * aDDv70ngNXtk55iwlNpNhTs+VcQor1fznhPbRiefHqJeRIOkpcrVE7NLP8TjwuaG
   * YaRSMLl6IE9vDzhTyzMMEyuP1pq9KsgtsRx9S1HKR9FIJ3Jdh+vVReZIZZ2vUpC6
   * W6IYZVcSn2i51BVrlMRpIpj0M+Dt+VGOQVDJNE92kKz8OMHY4Xu54+OU4UZpyw4K
   * UGsTuqwPN1q3ErWQgR5WrlcihtnJ0tHXUeOrO8ZV/R4O03QK0dqq6mm4lyiPSMQH
   * +FJDOvTKVTUssKZqwJz58oHhEmrARdlns87/I6KJClTUFLkqqNfs+avNJVgyeY+Q
   * W5g5xAgGwax/Dj0ApQIDAQABo1QwUjAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/
   * BAUwAwEB/zAdBgNVHQ4EFgQUCctZf4aycI8awznjwNnpv7tNsiMwEAYJKwYBBAGC
   * NxUBBAMCAQAwDQYJKoZIhvcNAQEMBQADggIBAKyvPl3CEZaJjqPnktaXFbgToqZC
   * LgLNFgVZJ8og6Lq46BrsTaiXVq5lQ7GPAJtSzVXNUzltYkyLDVt8LkS/gxCP81OC
   * gMNPOsduET/m4xaRhPtthH80dK2Jp86519efhGSSvpWhrQlTM93uCupKUY5vVau6
   * tZRGrox/2KJQJWVggEbbMwSubLWYdFQl3JPk+ONVFT24bcMKpBLBaYVu32TxU5nh
   * SnUgnZUP5NbcA/FZGOhHibJXWpS2qdgXKxdJ5XbLwVaZOjex/2kskZGT4d9Mozd2
   * TaGf+G0eHdP67Pv0RR0Tbc/3WeUiJ3IrhvNXuzDtJE3cfVa7o7P4NHmJweDyAmH3
   * pvwPuxwXC65B2Xy9J6P9LjrRk5Sxcx0ki69bIImtt2dmefU6xqaWM/5TkshGsRGR
   * xpl/j8nWZjEgQRCHLQzWwa80mMpkg/sTV9HB8Dx6jKXB/ZUhoHHBk2dxEuqPiApp
   * GWSZI1b7rCoucL5mxAyE7+WL85MB+GqQk2dLsmijtWKP6T+MejteD+eMuMZ87zf9
   * dOLITzNy4ZQ5bb0Sr74MTnB8G2+NszKTc0QWbej09+CVgI+WXTik9KveCjCHk9hN
   * AHFiRSdLOkKEW39lt2c0Ui2cFmuqqNh7o0JMcccMyj6D5KbvtwEwXlGjefVwaaZB
   * RA+GsCyRxj3qrg+E
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x1e0\x1c\x06\x03U\x04\n\x13\x15Microsoft Corporation1604\x06\x03U\x04\x03\x13-Microsoft RSA Root Certificate Authority 2017"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xca[\xbe\x943\x8c)\x95\x91\x16\n\x95\xbdGb\xc1\x89\xf3\x996\xdfF\x90\xc9\xa5\xedxjoG\x91h\xf8\'gP3\x1d\xa1\xa6\xfb\xe0\xe5C\xa3\x84\x02W\x01]\x9cH@\x82S\x10\xbc\xbf\xc7;h\x90\xb6\x82-\xe5\xf4e\xd0\xccm\x19\xcc\x95\xf9{\xacJ\x94\xad\x0e\xdeKC\x1d\x87\x07\x92\x13\x90\x80\x83d59\x04\xfc\xe5\xe9l\xb3\xb6\x1fP\x948eP\\\x17F\xb9\xb6\x85\xb5\x1c\xb5\x17\xe8\xd6E\x9d\xd8\xb2&\xb0\xca\xc4pJ\xae`\xa4\xdd\xb3\xd9\xec\xfc;\xd5Wr\xbc?\xc8\xc9\xb2\xdeKk\xf8#l\x03\xc0\x05\xbd\x95\xc7\xcds;f\x80d\xe3\x1a\xac.\xf9G\x05\xf2\x06\xb6\x9bs\xf5x3[\xc7\xa1\xfb\'*\xa1\xb4\x9a\x91\x8c\x91\xd3:\x82>v@\xb4\xcdRaQp(?\xc5\xc5Z\xf2\xc9\x8cI\xbb\x14[M\xc8\xffgML\x12\x96\xad\xf5\xfex\xa8\x97\x87\xd7\xfd^ \x80\xdc\xa1K\"\xfb\xd4\x89\xad\xba\xceG\x97GU{\x8fE\xc8g(\x84\x95\x1ch0\xef\xefI\xe05{d\xe7\x98\xb0\x94\xdaM\x85;>U\xc4(\xafW\xf3\x9e\x13\xdbF\'\x9f\x1e\xa2^D\x83\xa4\xa5\xca\xd5\x13\xb3K?\xc4\xe3\xc2\xe6\x86a\xa4R0\xb9z Oo\x0f8S\xcb3\x0c\x13+\x8f\xd6\x9a\xbd*\xc8-\xb1\x1c}KQ\xcaG\xd1H\'r]\x87\xeb\xd5E\xe6He\x9d\xafR\x90\xba[\xa2\x18eW\x12\x9fh\xb9\xd4\x15k\x94\xc4i\"\x98\xf43\xe0\xed\xf9Q\x8eAP\xc94Ov\x90\xac\xfc8\xc1\xd8\xe1{\xb9\xe3\xe3\x94\xe1Fi\xcb\x0e\nPk\x13\xba\xac\x0f7Z\xb7\x12\xb5\x90\x81\x1eV\xaeW\"\x86\xd9\xc9\xd2\xd1\xd7Q\xe3\xab;\xc6U\xfd\x1e\x0e\xd3t\n\xd1\xda\xaa\xeai\xb8\x97(\x8fH\xc4\x07\xf8RC:\xf4\xcaU5,\xb0\xa6j\xc0\x9c\xf9\xf2\x81\xe1\x12j\xc0E\xd9g\xb3\xce\xff#\xa2\x89\nT\xd4\x14\xb9*\xa8\xd7\xec\xf9\xab\xcd%X2y\x8f\x90[\x989\xc4\x08\x06\xc1\xac\x7f\x0e=\x00\xa5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Sectigo Public Server Authentication Root E46 O=Sectigo Limited
   * Subject: CN=Sectigo Public Server Authentication Root E46 O=Sectigo Limited
   * Label: "Sectigo Public Server Authentication Root E46"
   * Serial: 88989738453351742415770396670917916916
   * SHA256 Fingerprint: c9:0f:26:f0:fb:1b:40:18:b2:22:27:51:9b:5c:a2:b5:3e:2c:a5:b3:be:5c:f1:8e:fe:1b:ef:47:38:0c:53:83
   * -----BEGIN CERTIFICATE-----
   * MIICOjCCAcGgAwIBAgIQQvLM2htpN0RfFf51KBC49DAKBggqhkjOPQQDAzBfMQsw
   * CQYDVQQGEwJHQjEYMBYGA1UEChMPU2VjdGlnbyBMaW1pdGVkMTYwNAYDVQQDEy1T
   * ZWN0aWdvIFB1YmxpYyBTZXJ2ZXIgQXV0aGVudGljYXRpb24gUm9vdCBFNDYwHhcN
   * MjEwMzIyMDAwMDAwWhcNNDYwMzIxMjM1OTU5WjBfMQswCQYDVQQGEwJHQjEYMBYG
   * A1UEChMPU2VjdGlnbyBMaW1pdGVkMTYwNAYDVQQDEy1TZWN0aWdvIFB1YmxpYyBT
   * ZXJ2ZXIgQXV0aGVudGljYXRpb24gUm9vdCBFNDYwdjAQBgcqhkjOPQIBBgUrgQQA
   * IgNiAAR2+pmpbiDt+dd34wc7qNs9Xzjoq1WmVk/WSOrsfy2qw7LFeeyZYX8QeccC
   * WvkEN/U0NSt3zn8gj1KjAIns1aeibVvjS5KToID1AZTc8GgHHs3u/iVStSBDHBv+
   * 6xnOQ6OjQjBAMB0GA1UdDgQWBBTRItpMWfFLXyY4qp3W7usNw/upYTAOBgNVHQ8B
   * Af8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAwNnADBkAjAn7qRa
   * qCG76UeXlImldCBteU/IvZNeWBj7LRoAasm4PdCkT0RHlAFWovgzJQxC36oCMB3q
   * 4S6ILuH5px0CMk7yn2xVdOOurvulGu7t0vzCAxHrRVxgED1cf5kDW21USAGKcw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GB1\x180\x16\x06\x03U\x04\n\x13\x0fSectigo Limited1604\x06\x03U\x04\x03\x13-Sectigo Public Server Authentication Root E46"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04v\xfa\x99\xa9n \xed\xf9\xd7w\xe3\x07;\xa8\xdb=_8\xe8\xabU\xa6VO\xd6H\xea\xec\x7f-\xaa\xc3\xb2\xc5y\xec\x99a\x7f\x10y\xc7\x02Z\xf9\x047\xf545+w\xce\x7f \x8fR\xa3\x00\x89\xec\xd5\xa7\xa2m[\xe3K\x92\x93\xa0\x80\xf5\x01\x94\xdc\xf0h\x07\x1e\xcd\xee\xfe%R\xb5 C\x1c\x1b\xfe\xeb\x19\xceC\xa3"),
    name_constraints: None
  },

  /*
   * Issuer: CN=DigiCert Global Root G2 O=DigiCert Inc OU=www.digicert.com
   * Subject: CN=DigiCert Global Root G2 O=DigiCert Inc OU=www.digicert.com
   * Label: "DigiCert Global Root G2"
   * Serial: 4293743540046975378534879503202253541
   * SHA256 Fingerprint: cb:3c:cb:b7:60:31:e5:e0:13:8f:8d:d3:9a:23:f9:de:47:ff:c3:5e:43:c1:14:4c:ea:27:d4:6a:5a:b1:cb:5f
   * -----BEGIN CERTIFICATE-----
   * MIIDjjCCAnagAwIBAgIQAzrx5qcRqaC7KGSxHQn65TANBgkqhkiG9w0BAQsFADBh
   * MQswCQYDVQQGEwJVUzEVMBMGA1UEChMMRGlnaUNlcnQgSW5jMRkwFwYDVQQLExB3
   * d3cuZGlnaWNlcnQuY29tMSAwHgYDVQQDExdEaWdpQ2VydCBHbG9iYWwgUm9vdCBH
   * MjAeFw0xMzA4MDExMjAwMDBaFw0zODAxMTUxMjAwMDBaMGExCzAJBgNVBAYTAlVT
   * MRUwEwYDVQQKEwxEaWdpQ2VydCBJbmMxGTAXBgNVBAsTEHd3dy5kaWdpY2VydC5j
   * b20xIDAeBgNVBAMTF0RpZ2lDZXJ0IEdsb2JhbCBSb290IEcyMIIBIjANBgkqhkiG
   * 9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuzfNNNx7a8myaJCtSnX/RrohCgiN9RlUyfuI
   * 2/Ou8jqJkTx65qsGGmvPrC3oXgkkRLpimn7Wo6h+4FR1IAWsULecYxpsMNzaHxmx
   * 1x7e/dfgy5SDN67sH0NO3Xss0r0upS/kqbitOtSZpLYl6ZtrAGCSYP9PIUkY92eQ
   * q2EGnI/yuum06ZIya7XzV+hdG82MHauVBJVJ8zUtluNJbd134/tJS7SsVQepj5Wz
   * tCO7TG1F8PapspUwtP1MVYwnSlcUfIKdzXOS0xZKBgyMUNGPHgm+F6HmIcr9g+UQ
   * vIOlCsRnKPZzFBQ9RnbDhxSJITRNrw9FDKZJobq7nMWxM4MphQIDAQABo0IwQDAP
   * BgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBhjAdBgNVHQ4EFgQUTiJUIBiV
   * 5uNu5g/6+rkS7QYXjzkwDQYJKoZIhvcNAQELBQADggEBAGBnKJRvDkhj6zHd6mcY
   * 1Yl9PMWLSn/pvtsrF9+wX3N3KjITOYFnQoQj8kVnNeyIv/iPsGEMNKSuIEyExtv4
   * NeF22d+mQrvHRAiGfzZ0JFrabA0UWTW98kndth/Jsw1HKj2ZL7tcu7XUIOGZX1NG
   * Fdtom/DzMNU+MeKNhJ7jitralj41E6Vf8PlwUHBHQRFXGU7Aj64GxJUTFy8bJZ91
   * 8rGOmaFvE7FBcf6IKshPECBV1/MUReXgRPTqh5Uykw7+U0b6LJ3/iyK5S9kJRaTe
   * pLiaWN0bfVKfjllDiIGknibVb63dDcY3fe0Dkhvld1927jyNxF1WW6LZZm6zNTfl
   * MrY=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x150\x13\x06\x03U\x04\n\x13\x0cDigiCert Inc1\x190\x17\x06\x03U\x04\x0b\x13\x10www.digicert.com1 0\x1e\x06\x03U\x04\x03\x13\x17DigiCert Global Root G2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xbb7\xcd4\xdc{k\xc9\xb2h\x90\xadJu\xffF\xba!\n\x08\x8d\xf5\x19T\xc9\xfb\x88\xdb\xf3\xae\xf2:\x89\x91<z\xe6\xab\x06\x1ak\xcf\xac-\xe8^\t$D\xbab\x9a~\xd6\xa3\xa8~\xe0Tu \x05\xacP\xb7\x9cc\x1al0\xdc\xda\x1f\x19\xb1\xd7\x1e\xde\xfd\xd7\xe0\xcb\x94\x837\xae\xec\x1fCN\xdd{,\xd2\xbd.\xa5/\xe4\xa9\xb8\xad:\xd4\x99\xa4\xb6%\xe9\x9bk\x00`\x92`\xffO!I\x18\xf7g\x90\xaba\x06\x9c\x8f\xf2\xba\xe9\xb4\xe9\x922k\xb5\xf3W\xe8]\x1b\xcd\x8c\x1d\xab\x95\x04\x95I\xf35-\x96\xe3Im\xddw\xe3\xfbIK\xb4\xacU\x07\xa9\x8f\x95\xb3\xb4#\xbbLmE\xf0\xf6\xa9\xb2\x950\xb4\xfdLU\x8c\'JW\x14|\x82\x9d\xcds\x92\xd3\x16J\x06\x0c\x8cP\xd1\x8f\x1e\t\xbe\x17\xa1\xe6!\xca\xfd\x83\xe5\x10\xbc\x83\xa5\n\xc4g(\xf6s\x14\x14=Fv\xc3\x87\x14\x89!4M\xaf\x0fE\x0c\xa6I\xa1\xba\xbb\x9c\xc5\xb13\x83)\x85\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign O=GlobalSign OU=GlobalSign Root CA - R3
   * Subject: CN=GlobalSign O=GlobalSign OU=GlobalSign Root CA - R3
   * Label: "GlobalSign"
   * Serial: 4835703278459759426209954
   * SHA256 Fingerprint: cb:b5:22:d7:b7:f1:27:ad:6a:01:13:86:5b:df:1c:d4:10:2e:7d:07:59:af:63:5a:7c:f4:72:0d:c9:63:c5:3b
   * -----BEGIN CERTIFICATE-----
   * MIIDXzCCAkegAwIBAgILBAAAAAABIVhTCKIwDQYJKoZIhvcNAQELBQAwTDEgMB4G
   * A1UECxMXR2xvYmFsU2lnbiBSb290IENBIC0gUjMxEzARBgNVBAoTCkdsb2JhbFNp
   * Z24xEzARBgNVBAMTCkdsb2JhbFNpZ24wHhcNMDkwMzE4MTAwMDAwWhcNMjkwMzE4
   * MTAwMDAwWjBMMSAwHgYDVQQLExdHbG9iYWxTaWduIFJvb3QgQ0EgLSBSMzETMBEG
   * A1UEChMKR2xvYmFsU2lnbjETMBEGA1UEAxMKR2xvYmFsU2lnbjCCASIwDQYJKoZI
   * hvcNAQEBBQADggEPADCCAQoCggEBAMwldpB5BngiFvXAg7aEyiie/QV2EcWtiHL8
   * RgJDx7KKnQRfJMsuS+FggkbhUqsMgUdwbN1k0ev1LKMPgj0MK66X17YUhhB5uzsT
   * gHeMCOFJ0mpiLx9e+pZo34knlTifBtc+ycsmWQ1z3rDI6SYOgxXG71uL0gRgykmm
   * KPZpO/bLyCiR5Z2KYVc3rHQU3HTgOu5yLy6c+9C7v/U9AOEGM+iCK65TpjoWc4zd
   * QQ4gOsC0p6Hpsk+QLjJg6VfLuQSSaGjlOCZgdbKfd/+RFO+uIEn8rUAVSNECMWEZ
   * XriX7613t2Saer9fwRPvm2L7DWzgVGkWqQPabumDk3F2xmmFghcCAwEAAaNCMEAw
   * DgYDVR0PAQH/BAQDAgEGMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFI/wS3+o
   * LkUkrk1Q+mOai97i3Ru8MA0GCSqGSIb3DQEBCwUAA4IBAQBLQNvAUKr+yAzv95ZU
   * RUm7lgAJQayzE4aGKAczymvmdLm6AC2upArT9fHxD4q/c2dKg8dEe3jgr25sbwMp
   * jjM5RcOO5LlXbKr8EpbsU8Yt5CRsuZRj+9xTaGdWPoO4zzUhw8lo/s7awlOqzJCK
   * 6fBdRoyV3XpYKBovHd7NADdBj+1EbddTKJd+82cEHhXXipa0095MJ6RMG3NzdvQX
   * mcIfeg7jLQitChws/zyrVQ4PkX4268NXSb7hLi18YIvDQVETI53O9zJrlAGomecs
   * Mx86OyXShkDOOyyGeMlhLxS67ttVb9+E7gUJTb0o2HLO02JQZR7rkpeDMdmztcpH
   * WD9f
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1 0\x1e\x06\x03U\x04\x0b\x13\x17GlobalSign Root CA - R31\x130\x11\x06\x03U\x04\n\x13\nGlobalSign1\x130\x11\x06\x03U\x04\x03\x13\nGlobalSign"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xcc%v\x90y\x06x\"\x16\xf5\xc0\x83\xb6\x84\xca(\x9e\xfd\x05v\x11\xc5\xad\x88r\xfcF\x02C\xc7\xb2\x8a\x9d\x04_$\xcb.K\xe1`\x82F\xe1R\xab\x0c\x81Gpl\xddd\xd1\xeb\xf5,\xa3\x0f\x82=\x0c+\xae\x97\xd7\xb6\x14\x86\x10y\xbb;\x13\x80w\x8c\x08\xe1I\xd2jb/\x1f^\xfa\x96h\xdf\x89\'\x958\x9f\x06\xd7>\xc9\xcb&Y\rs\xde\xb0\xc8\xe9&\x0e\x83\x15\xc6\xef[\x8b\xd2\x04`\xcaI\xa6(\xf6i;\xf6\xcb\xc8(\x91\xe5\x9d\x8aaW7\xact\x14\xdct\xe0:\xeer/.\x9c\xfb\xd0\xbb\xbf\xf5=\x00\xe1\x063\xe8\x82+\xaeS\xa6:\x16s\x8c\xddA\x0e :\xc0\xb4\xa7\xa1\xe9\xb2O\x90.2`\xe9W\xcb\xb9\x04\x92hh\xe58&`u\xb2\x9fw\xff\x91\x14\xef\xae I\xfc\xad@\x15H\xd1\x021a\x19^\xb8\x97\xef\xadw\xb7d\x9az\xbf_\xc1\x13\xef\x9bb\xfb\rl\xe0Ti\x16\xa9\x03\xdan\xe9\x83\x93qv\xc6i\x85\x82\x17\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GlobalSign Root E46 O=GlobalSign nv-sa
   * Subject: CN=GlobalSign Root E46 O=GlobalSign nv-sa
   * Label: "GlobalSign Root E46"
   * Serial: 1552617690338932563915843282459653771421763
   * SHA256 Fingerprint: cb:b9:c4:4d:84:b8:04:3e:10:50:ea:31:a6:9f:51:49:55:d7:bf:d2:e2:c6:b4:93:01:01:9a:d6:1d:9f:50:58
   * -----BEGIN CERTIFICATE-----
   * MIICCzCCAZGgAwIBAgISEdK7ujNu1LzmJGjFDYQdmOhDMAoGCCqGSM49BAMDMEYx
   * CzAJBgNVBAYTAkJFMRkwFwYDVQQKExBHbG9iYWxTaWduIG52LXNhMRwwGgYDVQQD
   * ExNHbG9iYWxTaWduIFJvb3QgRTQ2MB4XDTE5MDMyMDAwMDAwMFoXDTQ2MDMyMDAw
   * MDAwMFowRjELMAkGA1UEBhMCQkUxGTAXBgNVBAoTEEdsb2JhbFNpZ24gbnYtc2Ex
   * HDAaBgNVBAMTE0dsb2JhbFNpZ24gUm9vdCBFNDYwdjAQBgcqhkjOPQIBBgUrgQQA
   * IgNiAAScDrHPt+ieUnd1NPqlRqetMhkytAepJ8qUuwzSChDH2omwlwxwEwkBjtjq
   * R+q+soArzfwoDdusvKSGN+1wCAB16pMLey5SnCNoIwZD7JIvU4Tb+0cUB+hflGdd
   * yXqBPCCjQjBAMA4GA1UdDwEB/wQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB0GA1Ud
   * DgQWBBQxCpCPtsad0kRLgLWi5h+xEk8blTAKBggqhkjOPQQDAwNoADBlAjEA31SQ
   * 7Zvvi5QCkxeCmb6zniz2C5GMn0oUsfZkvLtoURMMA/cVi4RguYv/Uo7njLwcAjA8
   * +RHUjE7AwWHCFUyqqx0LMV87HOIAl0Qx5v5zli/altP+CAezNIm8BZ/3Hobui3A=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02BE1\x190\x17\x06\x03U\x04\n\x13\x10GlobalSign nv-sa1\x1c0\x1a\x06\x03U\x04\x03\x13\x13GlobalSign Root E46"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x9c\x0e\xb1\xcf\xb7\xe8\x9eRwu4\xfa\xa5F\xa7\xad2\x192\xb4\x07\xa9\'\xca\x94\xbb\x0c\xd2\n\x10\xc7\xda\x89\xb0\x97\x0cp\x13\t\x01\x8e\xd8\xeaG\xea\xbe\xb2\x80+\xcd\xfc(\r\xdb\xac\xbc\xa4\x867\xedp\x08\x00u\xea\x93\x0b{.R\x9c#h#\x06C\xec\x92/S\x84\xdb\xfbG\x14\x07\xe8_\x94g]\xc9z\x81< "),
    name_constraints: None
  },

  /*
   * Issuer: CN=UCA Extended Validation Root O=UniTrust
   * Subject: CN=UCA Extended Validation Root O=UniTrust
   * Label: "UCA Extended Validation Root"
   * Serial: 106100277556486529736699587978573607008
   * SHA256 Fingerprint: d4:3a:f9:b3:54:73:75:5c:96:84:fc:06:d7:d8:cb:70:ee:5c:28:e7:73:fb:29:4e:b4:1e:e7:17:22:92:4d:24
   * -----BEGIN CERTIFICATE-----
   * MIIFWjCCA0KgAwIBAgIQT9Irj/VkyDOeTzRYZiNwYDANBgkqhkiG9w0BAQsFADBH
   * MQswCQYDVQQGEwJDTjERMA8GA1UECgwIVW5pVHJ1c3QxJTAjBgNVBAMMHFVDQSBF
   * eHRlbmRlZCBWYWxpZGF0aW9uIFJvb3QwHhcNMTUwMzEzMDAwMDAwWhcNMzgxMjMx
   * MDAwMDAwWjBHMQswCQYDVQQGEwJDTjERMA8GA1UECgwIVW5pVHJ1c3QxJTAjBgNV
   * BAMMHFVDQSBFeHRlbmRlZCBWYWxpZGF0aW9uIFJvb3QwggIiMA0GCSqGSIb3DQEB
   * AQUAA4ICDwAwggIKAoICAQCpCQcoEwKwmeBkqh5DFnpzsZGgdT6o+uM4AHrsiWog
   * D4vFsJszA1qGxliG1cGFu0/GnEBNyr7uaZa4rYEwmnySBesFK5pI0Lh2PpbIILvS
   * sPGP2KxFRv+qZ2C0d35qHzwaUnoEPQc8hQ2E0B92CvdqFN9y4zR8V05WAT558aop
   * O2z6+I9tTcg1367r3CTueUWnhbYFiN6IXSV8l2RnCdm/WhUFhvMJHuxYMjMR83dk
   * sHYf5BA1FxvyDrFspCqjc/wJHx4yGVMR59mzLC52LqGj3n5qiAno8geK+LLNEOfi
   * c0CTuwjRP+H8C5SzJe98ptfRr5//lpr1kXuYC3fUfugH0mK1lTnj8/FtDw5lhIpj
   * VMWAtuCeS31HJqcBCF3RiJ7XwzJE+oJKCmhUfzhTA8ykADNkUVkLo4KRel7sFsLz
   * KuZi2irbWWIQJUoqgQtHB0MGcIfS+pMRKXpITeuUx3BNr2fVUbGAIAEBtHoIppB/
   * TuDvB0GHr2qlXov7z1CymlSvw4m6WC31MJixNnI5fkkE/SmnTHnkBVfblLkWU41G
   * sx2VYVdWf6/wFlthWG82UBEL2KwrlRYaDh8IzTY0ZRBiZtWAXxQgXy0MoHgKaNYs
   * 1+lvK9JKBZP8nm9rZ/+I8U6laUpSNwXqxhaN0sSZ0YIrO7o1dfdRUVjzyAfd5LQD
   * fwIDAQABo0IwQDAdBgNVHQ4EFgQU2XQ65DA9DfcS3H5aBZ8eNJr34RQwDwYDVR0T
   * AQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwDQYJKoZIhvcNAQELBQADggIBADaN
   * l8xCFWQpN5smLNb7rhVpLGsaGvdftvkHTFnq88nIua7Mui563MD1sC3AO6+fcAUR
   * ap8lTwEpcOPlDOHqWnzcSbvBHiqB9RZLcpHIojG5qtr8nR/zXUACE/xOHAbKsxSQ
   * VBcZEhrxH9cMaVr2cXj0lH2RC47skFSOvG+hTKv8dGT9cZr4QQehzZHkPJrgmzI5
   * c6sq1WnIeJEmMX3ixzDx/BR4dxIOE/TdFpS/S2d7cFOFyrC78zhNLJA5wA3CXWvp
   * 4uXViI3WLL+rG761KIcSF3Ru/H38j9CHJrAb+7lsq+KePRXBOy5nAliRn+/4Qh8s
   * t2j1da3Ptfb/EX3C8CSlrdP6oDyp+l3cpaDvRKS+1ujl5BOWF3sGPjLtx7dCvHaj
   * 2GU4Kzg1USEODm8uNBNA4StnDG1KQTAYI1oyVZnJF+A83vbsea0rWBmirSwiGpWO
   * vpaQXUJXxPkUAzUrHC1RVwinOt4/5Mi0A3PCwSaAuwtCH60NryZy2sy+s6ODWA2C
   * xR9GUeOcGMyNm43sSet1UNWMKFnKdDTajAshqx7qG+XH/RU+wBeq+yNuJkbL+vmx
   * cmtpzyKEC2IPrNkZAJSidjzULZrtBJ4tBmIQN1IchXIbJ+XMxjHsN+xjWZsLHXbM
   * fjKaiJUINlK73nZfdklJrX+9ZSCyycErdhh2n1ax
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1\x110\x0f\x06\x03U\x04\n\x0c\x08UniTrust1%0#\x06\x03U\x04\x03\x0c\x1cUCA Extended Validation Root"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa9\t\x07(\x13\x02\xb0\x99\xe0d\xaa\x1eC\x16zs\xb1\x91\xa0u>\xa8\xfa\xe38\x00z\xec\x89j \x0f\x8b\xc5\xb0\x9b3\x03Z\x86\xc6X\x86\xd5\xc1\x85\xbbO\xc6\x9c@M\xca\xbe\xeei\x96\xb8\xad\x810\x9a|\x92\x05\xeb\x05+\x9aH\xd0\xb8v>\x96\xc8 \xbb\xd2\xb0\xf1\x8f\xd8\xacEF\xff\xaag`\xb4w~j\x1f<\x1aRz\x04=\x07<\x85\r\x84\xd0\x1fv\n\xf7j\x14\xdfr\xe34|WNV\x01>y\xf1\xaa);l\xfa\xf8\x8fmM\xc85\xdf\xae\xeb\xdc$\xeeyE\xa7\x85\xb6\x05\x88\xde\x88]%|\x97dg\t\xd9\xbfZ\x15\x05\x86\xf3\t\x1e\xecX23\x11\xf3wd\xb0v\x1f\xe4\x105\x17\x1b\xf2\x0e\xb1l\xa4*\xa3s\xfc\t\x1f\x1e2\x19S\x11\xe7\xd9\xb3,.v.\xa1\xa3\xde~j\x88\t\xe8\xf2\x07\x8a\xf8\xb2\xcd\x10\xe7\xe2s@\x93\xbb\x08\xd1?\xe1\xfc\x0b\x94\xb3%\xef|\xa6\xd7\xd1\xaf\x9f\xff\x96\x9a\xf5\x91{\x98\x0bw\xd4~\xe8\x07\xd2b\xb5\x959\xe3\xf3\xf1m\x0f\x0ee\x84\x8acT\xc5\x80\xb6\xe0\x9eK}G&\xa7\x01\x08]\xd1\x88\x9e\xd7\xc32D\xfa\x82J\nhT\x7f8S\x03\xcc\xa4\x003dQY\x0b\xa3\x82\x91z^\xec\x16\xc2\xf3*\xe6b\xda*\xdbYb\x10%J*\x81\x0bG\x07C\x06p\x87\xd2\xfa\x93\x11)zHM\xeb\x94\xc7pM\xafg\xd5Q\xb1\x80 \x01\x01\xb4z\x08\xa6\x90\x7fN\xe0\xef\x07A\x87\xafj\xa5^\x8b\xfb\xcfP\xb2\x9aT\xaf\xc3\x89\xbaX-\xf50\x98\xb16r9~I\x04\xfd)\xa7Ly\xe4\x05W\xdb\x94\xb9\x16S\x8dF\xb3\x1d\x95aWV\x7f\xaf\xf0\x16[aXo6P\x11\x0b\xd8\xac+\x95\x16\x1a\x0e\x1f\x08\xcd64e\x10bf\xd5\x80_\x14 _-\x0c\xa0x\nh\xd6,\xd7\xe9o+\xd2J\x05\x93\xfc\x9eokg\xff\x88\xf1N\xa5iJR7\x05\xea\xc6\x16\x8d\xd2\xc4\x99\xd1\x82+;\xba5u\xf7QQX\xf3\xc8\x07\xdd\xe4\xb4\x03\x7f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certigna Root CA O=Dhimyotis OU=0002 48146308100036
   * Subject: CN=Certigna Root CA O=Dhimyotis OU=0002 48146308100036
   * Label: "Certigna Root CA"
   * Serial: 269714418870597844693661054334862075617
   * SHA256 Fingerprint: d4:8d:3d:23:ee:db:50:a4:59:e5:51:97:60:1c:27:77:4b:9d:7b:18:c9:4d:5a:05:95:11:a1:02:50:b9:31:68
   * -----BEGIN CERTIFICATE-----
   * MIIGWzCCBEOgAwIBAgIRAMrpG4nxVQMNo+ZBbcTjpuEwDQYJKoZIhvcNAQELBQAw
   * WjELMAkGA1UEBhMCRlIxEjAQBgNVBAoMCURoaW15b3RpczEcMBoGA1UECwwTMDAw
   * MiA0ODE0NjMwODEwMDAzNjEZMBcGA1UEAwwQQ2VydGlnbmEgUm9vdCBDQTAeFw0x
   * MzEwMDEwODMyMjdaFw0zMzEwMDEwODMyMjdaMFoxCzAJBgNVBAYTAkZSMRIwEAYD
   * VQQKDAlEaGlteW90aXMxHDAaBgNVBAsMEzAwMDIgNDgxNDYzMDgxMDAwMzYxGTAX
   * BgNVBAMMEENlcnRpZ25hIFJvb3QgQ0EwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAw
   * ggIKAoICAQDNGDllGlmx6mQWDoyUJJV8g9PFOSbcDO8WV43X2KyjQn+Cyu3NW9sO
   * ty3tRQgXstmzy9YXUnIo245Onoq2C/mehJpNdt4iKVzSs9IGPjA5qXSjklYcoW9M
   * CiBtnyN6tMbaLOQdLNyzKNAT8kxOAkmhVECe5uUFoC2EyP+YbNDrihqECB63aCPu
   * I9Vwzm1RaRDuoXrC0SIxwoKF0vJVdlB8JXrJhFwLrN1CTivngqIkicuQstDuI7pm
   * TLtipPlTWmR7fJj6o0ieD5Wupxj0auwuA0Wv8HT4Ks16XdG+RCYyKfHx9WzMfgIh
   * C59vpD++nVPiz32pLHxYGpfhPTc3GGYo0kDFUYqMwy3OU4gkWGQwFsWq4NYKpkDf
   * ePb1BHxpE4S80dGnBs8B92jAqFe7OmGtBIyT46388NtEbVncSVmurJqZNjBBe3Yz
   * IoejwpKGbvlw7q6Hh5UbxHq9MfPU0uWZ/75I7HX1eBYdpnDBfzwboZL7z8g81sWT
   * Co/1VTp2lc5ZmIoJlXcymoO6LAQ6l73UL77XbJuiyn1tJslV1c/DeVIICZkHJC1k
   * JWumIWmbat10TWuXekG9qxf5kBdIjzb5LdXF2+6qhUVB+s06RbFo5jZMm5BX7CO5
   * hwjCxAnxl4YqKE3idMDaxIzb3+KhF1nOJFl0Mdp//TBt2dzhauH8XwIDAQABo4IB
   * GjCCARYwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYE
   * FBiHVuBud+4kNTxOc5of1uHieX4rMB8GA1UdIwQYMBaAFBiHVuBud+4kNTxOc5of
   * 1uHieX4rMEQGA1UdIAQ9MDswOQYEVR0gADAxMC8GCCsGAQUFBwIBFiNodHRwczov
   * L3d3d3cuY2VydGlnbmEuZnIvYXV0b3JpdGVzLzBtBgNVHR8EZjBkMC+gLaArhilo
   * dHRwOi8vY3JsLmNlcnRpZ25hLmZyL2NlcnRpZ25hcm9vdGNhLmNybDAxoC+gLYYr
   * aHR0cDovL2NybC5kaGlteW90aXMuY29tL2NlcnRpZ25hcm9vdGNhLmNybDANBgkq
   * hkiG9w0BAQsFAAOCAgEAlLieT/DjlQgi581oQfccVdV8AOItOoldaDgvUSILSo3L
   * 6btdPrtcPbEo/uRTVRPPoZAbAh1fZkYJMyjhDSSXcNMQH+pkV5a7XdrnxIxPTGRG
   * HVyH41neQtGbqH6mid2PHMkwgu07nM3A6RngatgCdTer9zQoKJHyBApPNeNgJgH6
   * 0BGM+RFq7q89w1DTj18zeTyGqHNFkIwgtnJzFyO+B2XleJINugHA64wcZr+shncB
   * lA2c5uk5jR+mUYyZDDl34bSb+hxnV29qao6pK0xXeXpXIs/NX2NGjVxZOob4Mkdi
   * o2cNGJHc+6Zr9UhhcyNZjgKnvETq9Emd8VRY+WCv2hikLyhF3HqgiIZd8zvn/yk1
   * gPxkQ5Tm4xxvvq0OKmOZK8l+hfZx6AYDlf7ej0gcWtSS6Cvu5zHbugRqh5jnxV/v
   * faci9wHYTfmJ0A6aBVmknpjZbyvKcL5kwlWj9Omvw5Ip3IgWJJk8jSaYtlu3zM63
   * Nwf9JtmYhST/WSMDmu2dnajkXjjO11INb9I/bbEFa0nOipFGc/T2L/Coc3cOZayh
   * jWZSaX5LaAzHHjcng6WMxwLkFM1JAbBzs/3GkDpv0mztO+7skb6iQ12LAEpmJURw
   * 3kAP+HwV96LOPNdeE4yBFxgX0b3xdxA61GU5wSesVywlVP+i2k+KYTlerj1KjL0=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02FR1\x120\x10\x06\x03U\x04\n\x0c\tDhimyotis1\x1c0\x1a\x06\x03U\x04\x0b\x0c\x130002 481463081000361\x190\x17\x06\x03U\x04\x03\x0c\x10Certigna Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xcd\x189e\x1aY\xb1\xead\x16\x0e\x8c\x94$\x95|\x83\xd3\xc59&\xdc\x0c\xef\x16W\x8d\xd7\xd8\xac\xa3B\x7f\x82\xca\xed\xcd[\xdb\x0e\xb7-\xedE\x08\x17\xb2\xd9\xb3\xcb\xd6\x17Rr(\xdb\x8eN\x9e\x8a\xb6\x0b\xf9\x9e\x84\x9aMv\xde\")\\\xd2\xb3\xd2\x06>09\xa9t\xa3\x92V\x1c\xa1oL\n m\x9f#z\xb4\xc6\xda,\xe4\x1d,\xdc\xb3(\xd0\x13\xf2LN\x02I\xa1T@\x9e\xe6\xe5\x05\xa0-\x84\xc8\xff\x98l\xd0\xeb\x8a\x1a\x84\x08\x1e\xb7h#\xee#\xd5p\xcemQi\x10\xee\xa1z\xc2\xd1\"1\xc2\x82\x85\xd2\xf2UvP|%z\xc9\x84\\\x0b\xac\xddBN+\xe7\x82\xa2$\x89\xcb\x90\xb2\xd0\xee#\xbafL\xbbb\xa4\xf9SZd{|\x98\xfa\xa3H\x9e\x0f\x95\xae\xa7\x18\xf4j\xec.\x03E\xaf\xf0t\xf8*\xcdz]\xd1\xbeD&2)\xf1\xf1\xf5l\xcc~\x02!\x0b\x9fo\xa4?\xbe\x9dS\xe2\xcf}\xa9,|X\x1a\x97\xe1=77\x18f(\xd2@\xc5Q\x8a\x8c\xc3-\xceS\x88$Xd0\x16\xc5\xaa\xe0\xd6\n\xa6@\xdfx\xf6\xf5\x04|i\x13\x84\xbc\xd1\xd1\xa7\x06\xcf\x01\xf7h\xc0\xa8W\xbb:a\xad\x04\x8c\x93\xe3\xad\xfc\xf0\xdbDmY\xdcIY\xae\xac\x9a\x9960A{v3\"\x87\xa3\xc2\x92\x86n\xf9p\xee\xae\x87\x87\x95\x1b\xc4z\xbd1\xf3\xd4\xd2\xe5\x99\xff\xbeH\xecu\xf5x\x16\x1d\xa6p\xc1\x7f<\x1b\xa1\x92\xfb\xcf\xc8<\xd6\xc5\x93\n\x8f\xf5U:v\x95\xceY\x98\x8a\t\x95w2\x9a\x83\xba,\x04:\x97\xbd\xd4/\xbe\xd7l\x9b\xa2\xca}m&\xc9U\xd5\xcf\xc3yR\x08\t\x99\x07$-d%k\xa6!i\x9bj\xddtMk\x97zA\xbd\xab\x17\xf9\x90\x17H\x8f6\xf9-\xd5\xc5\xdb\xee\xaa\x85EA\xfa\xcd:E\xb1h\xe66L\x9b\x90W\xec#\xb9\x87\x08\xc2\xc4\t\xf1\x97\x86*(M\xe2t\xc0\xda\xc4\x8c\xdb\xdf\xe2\xa1\x17Y\xce$Yt1\xda\x7f\xfd0m\xd9\xdc\xe1j\xe1\xfc_\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=GTS Root R1 O=Google Trust Services LLC
   * Subject: CN=GTS Root R1 O=Google Trust Services LLC
   * Label: "GTS Root R1"
   * Serial: 159662320309726417404178440727
   * SHA256 Fingerprint: d9:47:43:2a:bd:e7:b7:fa:90:fc:2e:6b:59:10:1b:12:80:e0:e1:c7:e4:e4:0f:a3:c6:88:7f:ff:57:a7:f4:cf
   * -----BEGIN CERTIFICATE-----
   * MIIFVzCCAz+gAwIBAgINAgPlk28xsBNJiGuiFzANBgkqhkiG9w0BAQwFADBHMQsw
   * CQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZpY2VzIExMQzEU
   * MBIGA1UEAxMLR1RTIFJvb3QgUjEwHhcNMTYwNjIyMDAwMDAwWhcNMzYwNjIyMDAw
   * MDAwWjBHMQswCQYDVQQGEwJVUzEiMCAGA1UEChMZR29vZ2xlIFRydXN0IFNlcnZp
   * Y2VzIExMQzEUMBIGA1UEAxMLR1RTIFJvb3QgUjEwggIiMA0GCSqGSIb3DQEBAQUA
   * A4ICDwAwggIKAoICAQC2EQKLHuOhd5s73L+UPreVp0A8of2C+X0yBoJx9vaMf/vo
   * 27xqLpeXo4xL+Sv2sfnOhB2x+cWX3u+58qPpvBKJXqeqUqv4IyfLpLGcY9vXmX7w
   * Cl7raKb0xlpHDU0QM+NOsROjyBhsS+z8CZDfnWQpJSMHobTSPS5g4M/SCYe7zUjw
   * TcLCeoiKu7rPWRnWr4+wB7CeMfGCwcDfLqZtbBkOtdh+JhpFAz2weaSUKK0Pfybl
   * qAj+lug8aJRT7oM6iCsVlgmy4HqMLnXWnOunVmSPlk9orj2XwoSPwLxAwAtcvfaH
   * szVsrBhQf4TgTM2S0yDpM7xSma8ytSmzJSq0SPly4cpk9+aCEI3oncKKiPo4Zor8
   * Y/kB+Xj9e1x3+naH+uzfsQ55lVe0vSbv1gHR6xYKu44LtcXFilWr06zqkUspzBmk
   * MiVOKvFlRNACzqrOSbTqn3yDsEB750Orp2yjj32JgfpMpf/VjsPOS+C12LOORc92
   * wO1AK/1TD7Cn1TsNsYqiA94xrcx36m97PtbfkSIS5r762DL8EGMUUXLeXdYWk70p
   * aDPvOmbsB4om3xPXV2V4J95eSRQAogB/mqghtqmxlbCluQ0WEdrHbEg8QOB+DVrN
   * VjzRlwW5y0vtOUucxD/SVRNuJLDWcfr0wbrM7Rv1/oFB2ACYPTrIrnqYNxgFlQID
   * AQABo0IwQDAOBgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4E
   * FgQU5K8rJnEaK0gnhS9SZizv8IkTcT4wDQYJKoZIhvcNAQEMBQADggIBAJ+qQibb
   * C5u+/x6Wki4+omVKapi6Ist9wTrYggoGxval3sBOh2Z5ofmmWJyq+bXmYOfg6LEe
   * QkEzCzc9zolwFcq1JKjPa7XSQCGYzyI0zzvFIoTgxQ6KfF2I5DUkzps+GlQebtuy
   * h6f88/qBVRRiClmpIgUxPoLW7ttXNLwzldMXG+gnoot7TiYaelpkttGsN/H9oPM4
   * 7HLwEXWdyzRSjeZ2axfG34arJ45JK3VmgRAhpuo+9K4l/3wV3s6MJT/KYnAK9y8J
   * ZgfIPxz88NtFMN9iiMG1D53Dn0reWVlHxYciNuaCp+0KueIHoI17eko8cdLiA6Ef
   * MgfdG+RCzgwARWGAtQsgWSl4vflVy2PFPEz0tv/bal8xa5meLMFrUKTX5hgUvYU/
   * Z6tGn6D/Qqc6f1zLXbBwHSs09dR2CQzreExZBfMzQsNhFRAbd03OIozUhfJFfbdT
   * 6u9AWpQKXCBfTkBdYiJ23//OYb2MI3jSNwLgjt7RETeJ9r/tSQdirpLsQBqvFAnZ
   * 0E6yove+7u7Y/9waLd64NnHi/Hm3lCXRSHNboTXns5lndcEZOitHTtNCjv0xyBZm
   * 2tIMPNuzjsmhDYAPexZ3FL//2wmUspO8IFgV6dtxQ/PeEMMA3KgqlbbC1j+Qa3bb
   * bP6MvPJwNQzcmRk13NfIRmPVNnGuV/u3gm3c
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\"0 \x06\x03U\x04\n\x13\x19Google Trust Services LLC1\x140\x12\x06\x03U\x04\x03\x13\x0bGTS Root R1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xb6\x11\x02\x8b\x1e\xe3\xa1w\x9b;\xdc\xbf\x94>\xb7\x95\xa7@<\xa1\xfd\x82\xf9}2\x06\x82q\xf6\xf6\x8c\x7f\xfb\xe8\xdb\xbcj.\x97\x97\xa3\x8cK\xf9+\xf6\xb1\xf9\xce\x84\x1d\xb1\xf9\xc5\x97\xde\xef\xb9\xf2\xa3\xe9\xbc\x12\x89^\xa7\xaaR\xab\xf8#\'\xcb\xa4\xb1\x9cc\xdb\xd7\x99~\xf0\n^\xebh\xa6\xf4\xc6ZG\rM\x103\xe3N\xb1\x13\xa3\xc8\x18lK\xec\xfc\t\x90\xdf\x9dd)%#\x07\xa1\xb4\xd2=.`\xe0\xcf\xd2\t\x87\xbb\xcdH\xf0M\xc2\xc2z\x88\x8a\xbb\xba\xcfY\x19\xd6\xaf\x8f\xb0\x07\xb0\x9e1\xf1\x82\xc1\xc0\xdf.\xa6ml\x19\x0e\xb5\xd8~&\x1aE\x03=\xb0y\xa4\x94(\xad\x0f\x7f&\xe5\xa8\x08\xfe\x96\xe8<h\x94S\xee\x83:\x88+\x15\x96\t\xb2\xe0z\x8c.u\xd6\x9c\xeb\xa7Vd\x8f\x96Oh\xae=\x97\xc2\x84\x8f\xc0\xbc@\xc0\x0b\\\xbd\xf6\x87\xb35l\xac\x18P\x7f\x84\xe0L\xcd\x92\xd3 \xe93\xbcR\x99\xaf2\xb5)\xb3%*\xb4H\xf9r\xe1\xcad\xf7\xe6\x82\x10\x8d\xe8\x9d\xc2\x8a\x88\xfa8f\x8a\xfcc\xf9\x01\xf9x\xfd{\\w\xfav\x87\xfa\xec\xdf\xb1\x0ey\x95W\xb4\xbd&\xef\xd6\x01\xd1\xeb\x16\n\xbb\x8e\x0b\xb5\xc5\xc5\x8aU\xab\xd3\xac\xea\x91K)\xcc\x19\xa42%N*\xf1eD\xd0\x02\xce\xaa\xceI\xb4\xea\x9f|\x83\xb0@{\xe7C\xab\xa7l\xa3\x8f}\x89\x81\xfaL\xa5\xff\xd5\x8e\xc3\xceK\xe0\xb5\xd8\xb3\x8eE\xcfv\xc0\xed@+\xfdS\x0f\xb0\xa7\xd5;\r\xb1\x8a\xa2\x03\xde1\xad\xccw\xeao{>\xd6\xdf\x91\"\x12\xe6\xbe\xfa\xd82\xfc\x10c\x14Qr\xde]\xd6\x16\x93\xbd)h3\xef:f\xec\x07\x8a&\xdf\x13\xd7Wex\'\xde^I\x14\x00\xa2\x00\x7f\x9a\xa8!\xb6\xa9\xb1\x95\xb0\xa5\xb9\r\x16\x11\xda\xc7lH<@\xe0~\rZ\xcdV<\xd1\x97\x05\xb9\xcbK\xed9K\x9c\xc4?\xd2U\x13n$\xb0\xd6q\xfa\xf4\xc1\xba\xcc\xed\x1b\xf5\xfe\x81A\xd8\x00\x98=:\xc8\xaez\x987\x18\x05\x95\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=HARICA TLS RSA Root CA 2021 O=Hellenic Academic and Research Institutions CA
   * Subject: CN=HARICA TLS RSA Root CA 2021 O=Hellenic Academic and Research Institutions CA
   * Label: "HARICA TLS RSA Root CA 2021"
   * Serial: 76817823531813593706434026085292783742
   * SHA256 Fingerprint: d9:5d:0e:8e:da:79:52:5b:f9:be:b1:1b:14:d2:10:0d:32:94:98:5f:0c:62:d9:fa:bd:9c:d9:99:ec:cb:7b:1d
   * -----BEGIN CERTIFICATE-----
   * MIIFpDCCA4ygAwIBAgIQOcqTHO9D88aOk8f0ZIk4fjANBgkqhkiG9w0BAQsFADBs
   * MQswCQYDVQQGEwJHUjE3MDUGA1UECgwuSGVsbGVuaWMgQWNhZGVtaWMgYW5kIFJl
   * c2VhcmNoIEluc3RpdHV0aW9ucyBDQTEkMCIGA1UEAwwbSEFSSUNBIFRMUyBSU0Eg
   * Um9vdCBDQSAyMDIxMB4XDTIxMDIxOTEwNTUzOFoXDTQ1MDIxMzEwNTUzN1owbDEL
   * MAkGA1UEBhMCR1IxNzA1BgNVBAoMLkhlbGxlbmljIEFjYWRlbWljIGFuZCBSZXNl
   * YXJjaCBJbnN0aXR1dGlvbnMgQ0ExJDAiBgNVBAMMG0hBUklDQSBUTFMgUlNBIFJv
   * b3QgQ0EgMjAyMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAIvC569l
   * mwVnlskNJLnQDmT8zuIkGCyEf3dRywQRNrhe7Wlxp57kJQmXZ8FHws+RFjZiPTgE
   * 4VGC/6zStGndLuwRo0Xua2s7TL+MjaQenRG56Tj5eg4MmOIjHdFOY9TnuEFE+2uv
   * a9of08WRiFukiZLRgeaMOVig1mlDqa2YUlhu2wr7a89o+uOkXjpFc5gH6l8Cct4M
   * pbOfrqkdtx2z/IpZ525yZa31MJQjB/OCFks1mJxTuy/K5FrZx40d/JiZ+yykgmvw
   * Kh+OC19xXFyuQnspiYHLA6OZyoieC0AJQTPb5lh6/a6ZcMBaD9YThnEvdmn8kN3b
   * LW7R8pv1GmuebxWMevBLKKAiOIAkbDakO/IwkfN4E8/BPzWr8R0RI7VDIp4BkrcY
   * AuUR0YLbFQDMYTfBKnya4dC6s1BG7oKsnTH4+yPiAwBIcKMJJnkVU2DzOFytOOqB
   * AGMUuTNe3QvboEUHGjMJ+E20pwKmafTCWQWIZYVWrkvL4N48fS0ayOn7H6NhStYq
   * E613TBoYm5EPWNgGVMWX+Ko/IIqmhaZ39qb8HOLubpQzKoNQhArlT4b4UEV4AIHr
   * W2jjJo3Me1xR9BQsQL4aYB16cmEdH2MtiKrOokWQCPxrvrNQKlr9qEgYRtaQQJKQ
   * CoReaDH46+0N0x3GfZkYVVYnZS6NRcUk7M7jAgMBAAGjQjBAMA8GA1UdEwEB/wQF
   * MAMBAf8wHQYDVR0OBBYEFApII6ZgpJIKM+qTW8VX6iVNvRLuMA4GA1UdDwEB/wQE
   * AwIBhjANBgkqhkiG9w0BAQsFAAOCAgEAPpBIqm5iFSVmewzVjIuJndftTgfvnNAU
   * X15QvWiWkKQUEapobQk1OUAJ2vQJLDSle1mESSmXdMgHHkdt8s4cUCbjnj1AUz/3
   * f5Z2EMVGpdAgS1D0NTsY9FVqQRtHBmg8uwkIYtlfVUKqrFOFrJVWNlar5AWMxaja
   * H6NpvVMPxP/cyuN+8kyIhkdGGvMA9YCRotxDQpSbIPDRzbLrLFPCU3hKTwSUQZqP
   * JzLB5UkZv/HywouoCjkxKLR9YjYsTewfM7Z+d21+UPCfDtcRj88YxeMn/ibvBZ3P
   * zzfF0HvaO7AWhAw6k9a+F9sPPg4ZeAnHqQJyIkv3N3a6dcSFA1pj1bF1BcK5vZSt
   * jBWZp5N99sXzqnTPBIWUmAD04vnKJGW/4GKvyMX6ssmeVkjaef2WdhW+o45WxLM0
   * /L5H9MG0qPzVMIho7suuyWPEdr6sOBjhXlzPrjoiUevRi7PzKzMHVIf6tLITe7pT
   * BGIBnfHAT+7hOtSLIBD6Alfm78ELt5BGnBkpjNxvoEppaZS3JGWg/6w/zgH7IS79
   * aPib8qXPMThcFarmlwDB31qlpzmq6YR/PFGoOtmUW4y/Twhx5duoXNTSpv4Ao8YW
   * xw/ogM4cKGR0GQjTQuPOAF1/sdwTsOEFy9EgqoZ0njnnkf3/W9b3raYvAwtt41dU
   * 63ZTGI0RmLo=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02GR1705\x06\x03U\x04\n\x0c.Hellenic Academic and Research Institutions CA1$0\"\x06\x03U\x04\x03\x0c\x1bHARICA TLS RSA Root CA 2021"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x8b\xc2\xe7\xafe\x9b\x05g\x96\xc9\r$\xb9\xd0\x0ed\xfc\xce\xe2$\x18,\x84\x7fwQ\xcb\x04\x116\xb8^\xediq\xa7\x9e\xe4%\t\x97g\xc1G\xc2\xcf\x91\x166b=8\x04\xe1Q\x82\xff\xac\xd2\xb4i\xdd.\xec\x11\xa3E\xeekk;L\xbf\x8c\x8d\xa4\x1e\x9d\x11\xb9\xe98\xf9z\x0e\x0c\x98\xe2#\x1d\xd1Nc\xd4\xe7\xb8AD\xfbk\xafk\xda\x1f\xd3\xc5\x91\x88[\xa4\x89\x92\xd1\x81\xe6\x8c9X\xa0\xd6iC\xa9\xad\x98RXn\xdb\n\xfbk\xcfh\xfa\xe3\xa4^:Es\x98\x07\xea_\x02r\xde\x0c\xa5\xb3\x9f\xae\xa9\x1d\xb7\x1d\xb3\xfc\x8aY\xe7nre\xad\xf50\x94#\x07\xf3\x82\x16K5\x98\x9cS\xbb/\xca\xe4Z\xd9\xc7\x8d\x1d\xfc\x98\x99\xfb,\xa4\x82k\xf0*\x1f\x8e\x0b_q\\\\\xaeB{)\x89\x81\xcb\x03\xa3\x99\xca\x88\x9e\x0b@\tA3\xdb\xe6Xz\xfd\xae\x99p\xc0Z\x0f\xd6\x13\x86q/vi\xfc\x90\xdd\xdb-n\xd1\xf2\x9b\xf5\x1ak\x9eo\x15\x8cz\xf0K(\xa0\"8\x80$l6\xa4;\xf20\x91\xf3x\x13\xcf\xc1?5\xab\xf1\x1d\x11#\xb5C\"\x9e\x01\x92\xb7\x18\x02\xe5\x11\xd1\x82\xdb\x15\x00\xcca7\xc1*|\x9a\xe1\xd0\xba\xb3PF\xee\x82\xac\x9d1\xf8\xfb#\xe2\x03\x00Hp\xa3\t&y\x15S`\xf38\\\xad8\xea\x81\x00c\x14\xb93^\xdd\x0b\xdb\xa0E\x07\x1a3\t\xf8M\xb4\xa7\x02\xa6i\xf4\xc2Y\x05\x88e\x85V\xaeK\xcb\xe0\xde<}-\x1a\xc8\xe9\xfb\x1f\xa3aJ\xd6*\x13\xadwL\x1a\x18\x9b\x91\x0fX\xd8\x06T\xc5\x97\xf8\xaa? \x8a\xa6\x85\xa6w\xf6\xa6\xfc\x1c\xe2\xeen\x943*\x83P\x84\n\xe5O\x86\xf8PEx\x00\x81\xeb[h\xe3&\x8d\xcc{\\Q\xf4\x14,@\xbe\x1a`\x1dzra\x1d\x1fc-\x88\xaa\xce\xa2E\x90\x08\xfck\xbe\xb3P*Z\xfd\xa8H\x18F\xd6\x90@\x92\x90\n\x84^h1\xf8\xeb\xed\r\xd3\x1d\xc6}\x99\x18UV\'e.\x8dE\xc5$\xec\xce\xe3\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TeliaSonera Root CA v1 O=TeliaSonera
   * Subject: CN=TeliaSonera Root CA v1 O=TeliaSonera
   * Label: "TeliaSonera Root CA v1"
   * Serial: 199041966741090107964904287217786801558
   * SHA256 Fingerprint: dd:69:36:fe:21:f8:f0:77:c1:23:a1:a5:21:c1:22:24:f7:22:55:b7:3e:03:a7:26:06:93:e8:a2:4b:0f:a3:89
   * -----BEGIN CERTIFICATE-----
   * MIIFODCCAyCgAwIBAgIRAJW+FqD3LkbxezmCcvqLzZYwDQYJKoZIhvcNAQEFBQAw
   * NzEUMBIGA1UECgwLVGVsaWFTb25lcmExHzAdBgNVBAMMFlRlbGlhU29uZXJhIFJv
   * b3QgQ0EgdjEwHhcNMDcxMDE4MTIwMDUwWhcNMzIxMDE4MTIwMDUwWjA3MRQwEgYD
   * VQQKDAtUZWxpYVNvbmVyYTEfMB0GA1UEAwwWVGVsaWFTb25lcmEgUm9vdCBDQSB2
   * MTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAMK+6yfwIaPzaSZVfp3F
   * VRaRXP3vIb9TgHot0pGMYzHw7CTww6XScnwQbfQ3t+XmfHnqjLWCi65ItqwA3GV1
   * 7CpNX8GH9SBlK4GoRz6JI5UwFpB/6FcHSOcZrr9FZ7E3GwYq/t75rH2D+1665I+X
   * Z75Ljo1kB1c4VWk0Nj0TSO9P4tNmHqTPGrdeNjPUtAa9GAH9d4RQAEX1jF3oI7x+
   * /jXh7VB7qTCNGdMJjmhnXb88lxhTuylixcpecsHHltTbLaC0H2kD7OriUPEMPPCs
   * 81Mt8Bz17Ww5OXOAFshSsCPN4D7c3TxHoLs1iuKYaIu+5b9y7tL6pe0S7fyYGKkm
   * dtwoSxAgHNN/Fnct7W+A90m7UwW7XWjH1Mh1Fj+JWov3F0fUTPHSiXk+TT2YqGHe
   * Oh7S+F4D4MHJHIzTjU3TlTazN19jY5szFPAtJmtTfImMMsJu7D0hADnJoWjiUIMu
   * sDor8zagrC/kb2HCUQk5PotTubtn2txTuXZZNp1D5SDgPTJghSJRt8czu90VL6R4
   * pgd7gUY2BIbdeTXHlSw7sKMXNeVzH7RcWe/a6hBle3rQf5+ztCo3O3CLm1u5K7fs
   * slESl1MpWtTwEhDcTwK7EpIvYtQ/aUN8Ddb8WHUBiJ1YFkveupD/RwGJBmr2X7KQ
   * arMCpgKIv7NHfirZ1fpoeDVNAgMBAAGjPzA9MA8GA1UdEwEB/wQFMAMBAf8wCwYD
   * VR0PBAQDAgEGMB0GA1UdDgQWBBTwj1k4ALP1j5qWDNXr+nuqF+gTEjANBgkqhkiG
   * 9w0BAQUFAAOCAgEAvuRcYk4k9AwI//DTDGjkk0kiP0Qnb7tt3oNmzqjMDfz1mgbl
   * dxSR651Be5kqhOX//CHBXfDkH1e3damhXwIm/9fH907eT/j3HEbAek9ALCI18Bmx
   * 0GtnLLCo4MBANzX2hFxc469CeP6nyQ1Q6g2EdvZR74NTxnr/DlZJLo961gzmJ1Tj
   * TQpgcmLNkQfWpb/ImWvtxBnmq0wROMVvMeJuScg/doAmAyYp4Db29iBT4xdwNBed
   * Y2gea+zDTYa4EzAvXUYNR0PVG6pZDrlcjQZIrXSHX8f8MVRBE+LHIQ6e4B4N4cB7
   * Q4WQxYpYxmUKeFfyxiMPAdkgS94P+5KFdSpcc41teyWRyu5FrgZLAMzTsVlQ2jqI
   * OylDRl6XK1TOU2+NSueW+r9xDkKLfP0ooNBIytrEgUy7onOTJsjrDNYmiLbAJM+7
   * vVvrdX3pCI6GMyx5dwlppYn8s3CQh3aP0yK7Qs69cwsgJirQmz1wHiRszYd2qReW
   * t88NkvuOGKmYSdGe/mBEciG5Ge3C9THxOUiIkCR1VBatzvT4aRRkOfujuLpwQMcn
   * HL/EVlP6Y2XQ8xwOFvVrhlhNGNTkDY6lnVuR3HYkUD/GKvvZt5y11ubQ2egZixVx
   * SK236thZiNSQvxaz2emsWWFUyBy6ysHK4bkgTI86k4mloMy/0/Z1pHWWbVY=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x140\x12\x06\x03U\x04\n\x0c\x0bTeliaSonera1\x1f0\x1d\x06\x03U\x04\x03\x0c\x16TeliaSonera Root CA v1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc2\xbe\xeb\'\xf0!\xa3\xf3i&U~\x9d\xc5U\x16\x91\\\xfd\xef!\xbfS\x80z-\xd2\x91\x8cc1\xf0\xec$\xf0\xc3\xa5\xd2r|\x10m\xf47\xb7\xe5\xe6|y\xea\x8c\xb5\x82\x8b\xaeH\xb6\xac\x00\xdceu\xec*M_\xc1\x87\xf5 e+\x81\xa8G>\x89#\x950\x16\x90\x7f\xe8W\x07H\xe7\x19\xae\xbfEg\xb17\x1b\x06*\xfe\xde\xf9\xac}\x83\xfb^\xba\xe4\x8f\x97g\xbeK\x8e\x8dd\x07W8Ui46=\x13H\xefO\xe2\xd3f\x1e\xa4\xcf\x1a\xb7^63\xd4\xb4\x06\xbd\x18\x01\xfdw\x84P\x00E\xf5\x8c]\xe8#\xbc~\xfe5\xe1\xedP{\xa90\x8d\x19\xd3\t\x8ehg]\xbf<\x97\x18S\xbb)b\xc5\xca^r\xc1\xc7\x96\xd4\xdb-\xa0\xb4\x1fi\x03\xec\xea\xe2P\xf1\x0c<\xf0\xac\xf3S-\xf0\x1c\xf5\xedl99s\x80\x16\xc8R\xb0#\xcd\xe0>\xdc\xdd<G\xa0\xbb5\x8a\xe2\x98h\x8b\xbe\xe5\xbfr\xee\xd2\xfa\xa5\xed\x12\xed\xfc\x98\x18\xa9&v\xdc(K\x10 \x1c\xd3\x7f\x16w-\xedo\x80\xf7I\xbbS\x05\xbb]h\xc7\xd4\xc8u\x16?\x89Z\x8b\xf7\x17G\xd4L\xf1\xd2\x89y>M=\x98\xa8a\xde:\x1e\xd2\xf8^\x03\xe0\xc1\xc9\x1c\x8c\xd3\x8dM\xd3\x956\xb37_cc\x9b3\x14\xf0-&kS|\x89\x8c2\xc2n\xec=!\x009\xc9\xa1h\xe2P\x83.\xb0:+\xf36\xa0\xac/\xe4oa\xc2Q\t9>\x8bS\xb9\xbbg\xda\xdcS\xb9vY6\x9dC\xe5 \xe0=2`\x85\"Q\xb7\xc73\xbb\xdd\x15/\xa4x\xa6\x07{\x81F6\x04\x86\xddy5\xc7\x95,;\xb0\xa3\x175\xe5s\x1f\xb4\\Y\xef\xda\xea\x10e{z\xd0\x7f\x9f\xb3\xb4*7;p\x8b\x9b[\xb9+\xb7\xec\xb2Q\x12\x97S)Z\xd4\xf0\x12\x10\xdcO\x02\xbb\x12\x92/b\xd4?iC|\r\xd6\xfcXu\x01\x88\x9dX\x16K\xde\xba\x90\xffG\x01\x89\x06j\xf6_\xb2\x90j\xb3\x02\xa6\x02\x88\xbf\xb3G~*\xd9\xd5\xfahx5M\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=TrustAsia Global Root CA G3 O=TrustAsia Technologies, Inc.
   * Subject: CN=TrustAsia Global Root CA G3 O=TrustAsia Technologies, Inc.
   * Label: "TrustAsia Global Root CA G3"
   * Serial: 576386314500428537169965010905813481816650257167
   * SHA256 Fingerprint: e0:d3:22:6a:eb:11:63:c2:e4:8f:f9:be:3b:50:b4:c6:43:1b:e7:bb:1e:ac:c5:c3:6b:5d:5e:c5:09:03:9a:08
   * -----BEGIN CERTIFICATE-----
   * MIIFpTCCA42gAwIBAgIUZPYOZXdhaqs7tOqFhLuxibhxkw8wDQYJKoZIhvcNAQEM
   * BQAwWjELMAkGA1UEBhMCQ04xJTAjBgNVBAoMHFRydXN0QXNpYSBUZWNobm9sb2dp
   * ZXMsIEluYy4xJDAiBgNVBAMMG1RydXN0QXNpYSBHbG9iYWwgUm9vdCBDQSBHMzAe
   * Fw0yMTA1MjAwMjEwMTlaFw00NjA1MTkwMjEwMTlaMFoxCzAJBgNVBAYTAkNOMSUw
   * IwYDVQQKDBxUcnVzdEFzaWEgVGVjaG5vbG9naWVzLCBJbmMuMSQwIgYDVQQDDBtU
   * cnVzdEFzaWEgR2xvYmFsIFJvb3QgQ0EgRzMwggIiMA0GCSqGSIb3DQEBAQUAA4IC
   * DwAwggIKAoICAQDAMYJhkuSUGwoqZdC+BqmHO1ES6nBBruL7dOoKjbmzTNyPtxNS
   * T1QY4SxzlZHFZjtqz6xjbYdT8PfxObegQ2OwxANdV6nnRM7EoYNl9lA+sX4WuDqK
   * AtCWHwDNBSHvBm3dIZwZQ0WhxeiAysKtQGIXBsaqvPPW5vxQfmZCHzyLpnl5hkA1
   * nyDvP+uLRx+PjsXUjrYsyUQE49RDdT/VP68czH5GX6zfZBCK70bwkPAPLfSIC7Ep
   * qq+FqklYqL9joDiR5rPmd2jE+SoZhLsO4fWvieylL1AgdB4SQXMeJNnKziyhWTXA
   * yB1GJ2Faj/lN03J5Zh6fFZAhLf3ti1ZwA0pJPn9pMRJpxx5cynoTi+jm9WAPzJMs
   * hH/x/Gr8m0ed262IPfN2dTPXS6TIi/n1Q1hPy8gDVI+lhXgEGvNz8teHHUGf59gX
   * zhqcD0r83ERoVGjiQTz+LISGNzzNPy+i2+f3VANfWdP3kXjHi3dqFuVJhZBFcnAv
   * kV34PmVACxmZySYgWmjBNb9Pp1Hx2BErW+Canig7CjoKH8GB5S7wprlppYiU5msT
   * f9FkPz2ccEblooV7WIQn3MSAPmeamseaMQ4w7OYXQJXZRe0Blqq/DPNL0WP3E1jA
   * uPP6Z92bfW1K/zJMtSU7/xxnD4UiWQWRkUF3gdCFTIcQcf+eQxuulXUtgQIDAQAB
   * o2MwYTAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFEDk5PIj7zjKsK5Xf/Ih
   * MBY027ySMB0GA1UdDgQWBBRA5OTyI+84yrCuV3/yITAWNNu8kjAOBgNVHQ8BAf8E
   * BAMCAQYwDQYJKoZIhvcNAQEMBQADggIBACY7UeFNOPMyGLS0XuFlXsSUT9SnYaP4
   * wM8zAQLpw6o1D/GUE3d3NZ4tVlFEbuHGLige/9rsR82XRBf34EzC4Xx8MnpmyFq2
   * XFNFV1pF1AWZLy4jVe5jaN/TG3inEpQGAHUNcoTpLrxaatXeL1nHo+zSh2bbt1S1
   * JKv0Q3jbSwTEb93mPmY+KfJLaHEih6D4sTNjduMNhXJEIlU/HHzp/LgV6FL6qj6j
   * ITk1dImmasI5+njPtqzn59ZW/yOSLlALqbUHM/Q4X6RJpstlcHboCoWASzY9M/eV
   * VHUl2qzEc4Jl6VL1XP04lQJqaTDFHApXB64ipCz5xUG3uOyfT0gA+QEEVcys+TIx
   * xHWVBqB/0Y0n3bOppHKH/lmLmnp0Ft0WpWIp6zqW3IunaFnT63eROfjXy9mPX1on
   * AX1daBli2MjN9LdyR75bl87yraKZk62Uy5P2EgmVtqvXO9A/EcswFi55gORngS1d
   * 7XB4tmBZrOFdRWOPyN9yaFvqHbgB8X7754qz41SgOAngPN5C8sLtLpvzHzW2Ntjj
   * gKGLzZlkD8Kqq7HK9W+eQ42EVJmzbsASZthwEPEGNTNDqJwuuhQxzhB/HIbjj9LV
   * +Hfsm6vxL2PZQl/gZ4FkkfGXL/xuJvYz+NO1+MRiqzFRJQJ6+N1rZdVtTTDIZbpo
   * FGWsJwt0ivKH
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1%0#\x06\x03U\x04\n\x0c\x1cTrustAsia Technologies, Inc.1$0\"\x06\x03U\x04\x03\x0c\x1bTrustAsia Global Root CA G3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xc01\x82a\x92\xe4\x94\x1b\n*e\xd0\xbe\x06\xa9\x87;Q\x12\xeapA\xae\xe2\xfbt\xea\n\x8d\xb9\xb3L\xdc\x8f\xb7\x13ROT\x18\xe1,s\x95\x91\xc5f;j\xcf\xaccm\x87S\xf0\xf7\xf19\xb7\xa0Cc\xb0\xc4\x03]W\xa9\xe7D\xce\xc4\xa1\x83e\xf6P>\xb1~\x16\xb8:\x8a\x02\xd0\x96\x1f\x00\xcd\x05!\xef\x06m\xdd!\x9c\x19CE\xa1\xc5\xe8\x80\xca\xc2\xad@b\x17\x06\xc6\xaa\xbc\xf3\xd6\xe6\xfcP~fB\x1f<\x8b\xa6yy\x86@5\x9f \xef?\xeb\x8bG\x1f\x8f\x8e\xc5\xd4\x8e\xb6,\xc9D\x04\xe3\xd4Cu?\xd5?\xaf\x1c\xcc~F_\xac\xdfd\x10\x8a\xefF\xf0\x90\xf0\x0f-\xf4\x88\x0b\xb1)\xaa\xaf\x85\xaaIX\xa8\xbfc\xa08\x91\xe6\xb3\xe6wh\xc4\xf9*\x19\x84\xbb\x0e\xe1\xf5\xaf\x89\xec\xa5/P t\x1e\x12As\x1e$\xd9\xca\xce,\xa1Y5\xc0\xc8\x1dF\'aZ\x8f\xf9M\xd3ryf\x1e\x9f\x15\x90!-\xfd\xed\x8bVp\x03JI>\x7fi1\x12i\xc7\x1e\\\xcaz\x13\x8b\xe8\xe6\xf5`\x0f\xcc\x93,\x84\x7f\xf1\xfcj\xfc\x9bG\x9d\xdb\xad\x88=\xf3vu3\xd7K\xa4\xc8\x8b\xf9\xf5CXO\xcb\xc8\x03T\x8f\xa5\x85x\x04\x1a\xf3s\xf2\xd7\x87\x1dA\x9f\xe7\xd8\x17\xce\x1a\x9c\x0fJ\xfc\xdcDhTh\xe2A<\xfe,\x84\x867<\xcd?/\xa2\xdb\xe7\xf7T\x03_Y\xd3\xf7\x91x\xc7\x8bwj\x16\xe5I\x85\x90Erp/\x91]\xf8>e@\x0b\x19\x99\xc9& Zh\xc15\xbfO\xa7Q\xf1\xd8\x11+[\xe0\x9a\x9e(;\n:\n\x1f\xc1\x81\xe5.\xf0\xa6\xb9i\xa5\x88\x94\xe6k\x13\x7f\xd1d?=\x9cpF\xe5\xa2\x85{X\x84\'\xdc\xc4\x80>g\x9a\x9a\xc7\x9a1\x0e0\xec\xe6\x17@\x95\xd9E\xed\x01\x96\xaa\xbf\x0c\xf3K\xd1c\xf7\x13X\xc0\xb8\xf3\xfag\xdd\x9b}mJ\xff2L\xb5%;\xff\x1cg\x0f\x85\"Y\x05\x91\x91Aw\x81\xd0\x85L\x87\x10q\xff\x9eC\x1b\xae\x95u-\x81\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CA Disig Root R2 O=Disig a.s.
   * Subject: CN=CA Disig Root R2 O=Disig a.s.
   * Label: "CA Disig Root R2"
   * Serial: 10572350602393338211
   * SHA256 Fingerprint: e2:3d:4a:03:6d:7b:70:e9:f5:95:b1:42:20:79:d2:b9:1e:df:bb:1f:b6:51:a0:63:3e:aa:8a:9d:c5:f8:07:03
   * -----BEGIN CERTIFICATE-----
   * MIIFaTCCA1GgAwIBAgIJAJK4iNuwisFjMA0GCSqGSIb3DQEBCwUAMFIxCzAJBgNV
   * BAYTAlNLMRMwEQYDVQQHEwpCcmF0aXNsYXZhMRMwEQYDVQQKEwpEaXNpZyBhLnMu
   * MRkwFwYDVQQDExBDQSBEaXNpZyBSb290IFIyMB4XDTEyMDcxOTA5MTUzMFoXDTQy
   * MDcxOTA5MTUzMFowUjELMAkGA1UEBhMCU0sxEzARBgNVBAcTCkJyYXRpc2xhdmEx
   * EzARBgNVBAoTCkRpc2lnIGEucy4xGTAXBgNVBAMTEENBIERpc2lnIFJvb3QgUjIw
   * ggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQCio8QACdaFXS1tFPbCw3Oe
   * NcJxVX6B+6tGUODBfEl45qt5WDza/3wcn9iXAng+a0EE6UG9vgMsRfYvZNSrXaNH
   * PWSb6WiaxswbP7q+sos0Ai6YVRn8jG+qX9pMzk0DIaPY0jSTVpbLTAwAFjxfGs3I
   * x2ymrdMxp7zo5eFm1tL7A7RBZckQrg4FY8aAamkw/dLukO8NJ9+flXP04SXabBbe
   * QTg06ov80egEFGEtQX6sx3dOy1FU+16SGBsEWmjGycT6txOgmLcRK7fWV8x8nhfR
   * yyX+hk4kLlYMeE2eARKmK6cBZW58Yh2EhN/qwGu1pSqVg8NTEQxzHQuyRpDRQjrO
   * QG6Vrf/GlK1ul4SOfW+eioANSW1z4nuSHsPzwfPrLgVv2RvPN3YEyLRa5Beny912
   * H9AZdugsBbPWnDTYltxhh5EF5EQIM8HauQhl1K6yNg3ruji6DOWbnuuNZt2Zz9aJ
   * QfYEkoopKW1rOhzndX0CcQ7zwOe9yxndnWCywmZgtrEE7snmhrmaZkCo5xHtgUUD
   * i/ZnWejBBhG93c+AAk9lQHhcR1DIm+YfgXvkRKhbhZri3lrVx/k6RGZL5DJUfORs
   * nLMOPReisjQS1n6yqEm70XooQL6iFh/f5DcfEXP7kAplQ6INfPgGAVUzfbANuPT1
   * rqVCV3w2EYx7XsQDnYx5nQIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4GA1Ud
   * DwEB/wQEAwIBBjAdBgNVHQ4EFgQUtZn4r7CU9eMg1gqtzk5WpC5uQu0wDQYJKoZI
   * hvcNAQELBQADggIBACYGXnDnZTPIgm7ZnBc6G3pmsgH2eDtpXi/q/075KMOYKmFM
   * tCQSin1tERT3nLXK5ryeJ45MGcipvXrA1zYObYVybqjGom32+nNjf7xueQgcnYqf
   * GopTpti72TVVsRHFqQOzVju5hJMiXn7B9hJSi+osZ7z+Nkz1uM/Rs0mSO9MpDpkb
   * lvdhuDvEK7Z4bLQjb/D907JedR+Zlais9trhxTF7+9FGs9K8Z7RiVLoJ92Owk6Ka
   * +elSLotgEqv89WBW7xBci8QaQtyDW2QOy7W81k/BfDxujRNt+3vrMNDcTa/F1bal
   * TFtxyegxvug4BkihGuLq0t4SOVga/4AOgnXmt8kHbA7v/zjxmHHEt38OFdAlab0i
   * nSvtBfZGR6ztwPDUO+Ls7pZbkBNOHlY667DvlruWIxG68kOGdGSVyCh13x01utI3
   * gzhTODY7z2zp+WsO0PsE6E9312UBeIYMej4hYvF/Y3EMyZ9E26gnonW+boE+18Dr
   * G5gPcFw0sorMwIUY6256s/daoQe/qUKS82Ail+QUoQebTnbAjn39pCXHR+3/H3Os
   * zMOl6W8KjptlwlCFtaOgUxLMVYdh84GuEEZhvUQhuMI9dM9+JDX6HAcOmz0iyu8x
   * L4ysEr3vQCj8KWefshNPZiTEUxnpHikV7+ZtsH8tZ/3zbBt1RqPlShfppNcL
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02SK1\x130\x11\x06\x03U\x04\x07\x13\nBratislava1\x130\x11\x06\x03U\x04\n\x13\nDisig a.s.1\x190\x17\x06\x03U\x04\x03\x13\x10CA Disig Root R2"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa2\xa3\xc4\x00\t\xd6\x85]-m\x14\xf6\xc2\xc3s\x9e5\xc2qU~\x81\xfb\xabFP\xe0\xc1|Ix\xe6\xabyX<\xda\xff|\x1c\x9f\xd8\x97\x02x>kA\x04\xe9A\xbd\xbe\x03,E\xf6/d\xd4\xab]\xa3G=d\x9b\xe9h\x9a\xc6\xcc\x1b?\xba\xbe\xb2\x8b4\x02.\x98U\x19\xfc\x8co\xaa_\xdaL\xceM\x03!\xa3\xd8\xd24\x93V\x96\xcbL\x0c\x00\x16<_\x1a\xcd\xc8\xc7l\xa6\xad\xd31\xa7\xbc\xe8\xe5\xe1f\xd6\xd2\xfb\x03\xb4Ae\xc9\x10\xae\x0e\x05c\xc6\x80ji0\xfd\xd2\xee\x90\xef\r\'\xdf\x9f\x95s\xf4\xe1%\xdal\x16\xdeA84\xea\x8b\xfc\xd1\xe8\x04\x14a-A~\xac\xc7wN\xcbQT\xfb^\x92\x18\x1b\x04Zh\xc6\xc9\xc4\xfa\xb7\x13\xa0\x98\xb7\x11+\xb7\xd6W\xcc|\x9e\x17\xd1\xcb%\xfe\x86N$.V\x0cxM\x9e\x01\x12\xa6+\xa7\x01en|b\x1d\x84\x84\xdf\xea\xc0k\xb5\xa5*\x95\x83\xc3S\x11\x0cs\x1d\x0b\xb2F\x90\xd1B:\xce@n\x95\xad\xff\xc6\x94\xadn\x97\x84\x8e}o\x9e\x8a\x80\rIms\xe2{\x92\x1e\xc3\xf3\xc1\xf3\xeb.\x05o\xd9\x1b\xcf7v\x04\xc8\xb4Z\xe4\x17\xa7\xcb\xddv\x1f\xd0\x19v\xe8,\x05\xb3\xd6\x9c4\xd8\x96\xdca\x87\x91\x05\xe4D\x083\xc1\xda\xb9\x08e\xd4\xae\xb26\r\xeb\xba8\xba\x0c\xe5\x9b\x9e\xeb\x8df\xdd\x99\xcf\xd6\x89A\xf6\x04\x92\x8a))mk:\x1c\xe7u}\x02q\x0e\xf3\xc0\xe7\xbd\xcb\x19\xdd\x9d`\xb2\xc2f`\xb6\xb1\x04\xee\xc9\xe6\x86\xb9\x9af@\xa8\xe7\x11\xed\x81E\x03\x8b\xf6gY\xe8\xc1\x06\x11\xbd\xdd\xcf\x80\x02Oe@x\\GP\xc8\x9b\xe6\x1f\x81{\xe4D\xa8[\x85\x9a\xe2\xdeZ\xd5\xc7\xf9:DfK\xe42T|\xe4l\x9c\xb3\x0e=\x17\xa2\xb24\x12\xd6~\xb2\xa8I\xbb\xd1z(@\xbe\xa2\x16\x1f\xdf\xe47\x1f\x11s\xfb\x90\neC\xa2\r|\xf8\x06\x01U3}\xb0\r\xb8\xf4\xf5\xae\xa5BW|6\x11\x8c{^\xc4\x03\x9d\x8cy\x9d\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Amazon Root CA 4 O=Amazon
   * Subject: CN=Amazon Root CA 4 O=Amazon
   * Label: "Amazon Root CA 4"
   * Serial: 143266989758080763974105200630763877849284878
   * SHA256 Fingerprint: e3:5d:28:41:9e:d0:20:25:cf:a6:90:38:cd:62:39:62:45:8d:a5:c6:95:fb:de:a3:c2:2b:0b:fb:25:89:70:92
   * -----BEGIN CERTIFICATE-----
   * MIIB8jCCAXigAwIBAgITBmyf18G7EEwpQ+Vxe3ssyBrBDjAKBggqhkjOPQQDAzA5
   * MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g
   * Um9vdCBDQSA0MB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG
   * A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg
   * Q0EgNDB2MBAGByqGSM49AgEGBSuBBAAiA2IABNKrijdPo1MN/sGKe0uoe0ZLY7Bi
   * 9i0b2whxIdIA6GO9mif78DluXeo9pcmBqqNbIJhFXRbb/egQbeOc4OO9X4Ri83Bk
   * M6DLJC9wuoihKqB1+IGuYgbEgds5bimwHvouXKNCMEAwDwYDVR0TAQH/BAUwAwEB
   * /zAOBgNVHQ8BAf8EBAMCAYYwHQYDVR0OBBYEFNPsxzplbszh2naaVvuc84ZtV+WB
   * MAoGCCqGSM49BAMDA2gAMGUCMDqLIfG9fhGt0O9Yli/W651+kI0rz2ZVwyzjKKlw
   * CkcO8DdZEv8tmZQoTipPNU0zWgIxAOp1AE47xDqUEpHJWEadIRNyp4iciuRMStuW
   * 1KyLa2tJElMzrdfkviT8tQp21KW8EA==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x0f0\r\x06\x03U\x04\n\x13\x06Amazon1\x190\x17\x06\x03U\x04\x03\x13\x10Amazon Root CA 4"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xd2\xab\x8a7O\xa3S\r\xfe\xc1\x8a{K\xa8{FKc\xb0b\xf6-\x1b\xdb\x08q!\xd2\x00\xe8c\xbd\x9a\'\xfb\xf09n]\xea=\xa5\xc9\x81\xaa\xa3[ \x98E]\x16\xdb\xfd\xe8\x10m\xe3\x9c\xe0\xe3\xbd_\x84b\xf3pd3\xa0\xcb$/p\xba\x88\xa1*\xa0u\xf8\x81\xaeb\x06\xc4\x81\xdb9n)\xb0\x1e\xfa.\\"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certigna O=Dhimyotis
   * Subject: CN=Certigna O=Dhimyotis
   * Label: "Certigna"
   * Serial: 18364802974209362175
   * SHA256 Fingerprint: e3:b6:a2:db:2e:d7:ce:48:84:2f:7a:c5:32:41:c7:b7:1d:54:14:4b:fb:40:c1:1f:3f:1d:0b:42:f5:ee:a1:2d
   * -----BEGIN CERTIFICATE-----
   * MIIDqDCCApCgAwIBAgIJAP7c4wEPyUj/MA0GCSqGSIb3DQEBBQUAMDQxCzAJBgNV
   * BAYTAkZSMRIwEAYDVQQKDAlEaGlteW90aXMxETAPBgNVBAMMCENlcnRpZ25hMB4X
   * DTA3MDYyOTE1MTMwNVoXDTI3MDYyOTE1MTMwNVowNDELMAkGA1UEBhMCRlIxEjAQ
   * BgNVBAoMCURoaW15b3RpczERMA8GA1UEAwwIQ2VydGlnbmEwggEiMA0GCSqGSIb3
   * DQEBAQUAA4IBDwAwggEKAoIBAQDIaPHJ1tazNHUmgh7stL7qXOEm7RFHYeGifBZ4
   * QCHkYJ5ayGPhxLGWkv8YbWkj4Sti993iNi+RB7lIzw7sebYs5zRLcAglozyHGxny
   * gQcPOJAZ0xH+hrTy0V4eHpbNgGzOOzGTtvKg0KmVEn2lmsxryIRWijOp5yIVUxbw
   * zBfsV1/pogqYCd7jX5xv3EjjhQsVWqa6n6xI4wmy9/Qy3l40vhx4XUJbzg4ij02Q
   * 130yGLMLLGq/jj8UEYkgDncUtT2UCIf3JR7VsmAA7G8qKCVuKj4YYxclPz5EIBb2
   * JsglrgVKtOdjLPOMFlN+XPsRGgjBRmKfIrjxwo1p3Po6WAbfAgMBAAGjgbwwgbkw
   * DwYDVR0TAQH/BAUwAwEB/zAdBgNVHQ4EFgQUGu3+QTmQtCRZvgHyUtVF9lo53BEw
   * ZAYDVR0jBF0wW4AUGu3+QTmQtCRZvgHyUtVF9lo53BGhOKQ2MDQxCzAJBgNVBAYT
   * AkZSMRIwEAYDVQQKDAlEaGlteW90aXMxETAPBgNVBAMMCENlcnRpZ25hggkA/tzj
   * AQ/JSP8wDgYDVR0PAQH/BAQDAgEGMBEGCWCGSAGG+EIBAQQEAwIABzANBgkqhkiG
   * 9w0BAQUFAAOCAQEAhQMeknH2Qq/ho2Ge6/PAD/Kl1NqV5ta+aDY9fm4fTIrv0Q8h
   * bV6lUmPOEvjvKtpv6zf+EwLHyzs+ImvaYS5/1HI93TDhHkxAGYwP15zRgzB7mFnc
   * fca5DClMoTOi62c6ZYTTluLtdkVwj7Ur3vkj1kluPBS1xp81HlDQwY9qcEQCYsuu
   * HWhBp6pX6FOqB9IG9tUUBguRA3UsbHK1YZWaDYu5Def131TN3ubY1gkIl2PlwS6w
   * t0QmwCbAr1UwnjvVNioZBPRcHv/PLLf/0P2HQBHVESO7SMAhqaQoLf0V+LBOK/Qw
   * WyH8EZE0vkHve52Xdf+XlcCWWC/qu0bXu+TZLg==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02FR1\x120\x10\x06\x03U\x04\n\x0c\tDhimyotis1\x110\x0f\x06\x03U\x04\x03\x0c\x08Certigna"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xc8h\xf1\xc9\xd6\xd6\xb34u&\x82\x1e\xec\xb4\xbe\xea\\\xe1&\xed\x11Ga\xe1\xa2|\x16x@!\xe4`\x9eZ\xc8c\xe1\xc4\xb1\x96\x92\xff\x18mi#\xe1+b\xf7\xdd\xe26/\x91\x07\xb9H\xcf\x0e\xecy\xb6,\xe74Kp\x08%\xa3<\x87\x1b\x19\xf2\x81\x07\x0f8\x90\x19\xd3\x11\xfe\x86\xb4\xf2\xd1^\x1e\x1e\x96\xcd\x80l\xce;1\x93\xb6\xf2\xa0\xd0\xa9\x95\x12}\xa5\x9a\xcck\xc8\x84V\x8a3\xa9\xe7\"\x15S\x16\xf0\xcc\x17\xecW_\xe9\xa2\n\x98\t\xde\xe3_\x9co\xdcH\xe3\x85\x0b\x15Z\xa6\xba\x9f\xacH\xe3\t\xb2\xf7\xf42\xde^4\xbe\x1cx]B[\xce\x0e\"\x8fM\x90\xd7}2\x18\xb3\x0b,j\xbf\x8e?\x14\x11\x89 \x0ew\x14\xb5=\x94\x08\x87\xf7%\x1e\xd5\xb2`\x00\xeco*(%n*>\x18c\x17%?>D \x16\xf6&\xc8%\xae\x05J\xb4\xe7c,\xf3\x8c\x16S~\\\xfb\x11\x1a\x08\xc1Fb\x9f\"\xb8\xf1\xc2\x8di\xdc\xfa:X\x06\xdf\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=D-TRUST BR Root CA 1 2020 O=D-Trust GmbH
   * Subject: CN=D-TRUST BR Root CA 1 2020 O=D-Trust GmbH
   * Label: "D-TRUST BR Root CA 1 2020"
   * Serial: 165870826978392376648679885835942448534
   * SHA256 Fingerprint: e5:9a:aa:81:60:09:c2:2b:ff:5b:25:ba:d3:7d:f3:06:f0:49:79:7c:1f:81:d8:5a:b0:89:e6:57:bd:8f:00:44
   * -----BEGIN CERTIFICATE-----
   * MIIC2zCCAmCgAwIBAgIQfMmPK4TX3+oPyWWa00tNljAKBggqhkjOPQQDAzBIMQsw
   * CQYDVQQGEwJERTEVMBMGA1UEChMMRC1UcnVzdCBHbWJIMSIwIAYDVQQDExlELVRS
   * VVNUIEJSIFJvb3QgQ0EgMSAyMDIwMB4XDTIwMDIxMTA5NDUwMFoXDTM1MDIxMTA5
   * NDQ1OVowSDELMAkGA1UEBhMCREUxFTATBgNVBAoTDEQtVHJ1c3QgR21iSDEiMCAG
   * A1UEAxMZRC1UUlVTVCBCUiBSb290IENBIDEgMjAyMDB2MBAGByqGSM49AgEGBSuB
   * BAAiA2IABMbLxyjR+4T1mu9CFCDhQ2tuda38KwOE1HaTJddZO0Flax7mNCq7dPYS
   * zuht56vkPE4/RAiLzRZxy7+SmfSk1zxQVFKQhYN4lGdnoxwJGT11NIXe7WB9xwy0
   * QVK5buXuQqOCAQ0wggEJMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFHOREKv/
   * VbNafAkl1bK6CKBrqx9tMA4GA1UdDwEB/wQEAwIBBjCBxgYDVR0fBIG+MIG7MD6g
   * PKA6hjhodHRwOi8vY3JsLmQtdHJ1c3QubmV0L2NybC9kLXRydXN0X2JyX3Jvb3Rf
   * Y2FfMV8yMDIwLmNybDB5oHegdYZzbGRhcDovL2RpcmVjdG9yeS5kLXRydXN0Lm5l
   * dC9DTj1ELVRSVVNUJTIwQlIlMjBSb290JTIwQ0ElMjAxJTIwMjAyMCxPPUQtVHJ1
   * c3QlMjBHbWJILEM9REU/Y2VydGlmaWNhdGVyZXZvY2F0aW9ubGlzdDAKBggqhkjO
   * PQQDAwNpADBmAjEAlJAtE/rhY/hhY+ithXhUkZy4kzg+GkHaQBZTQgjKL47xPoFW
   * wKrY7RjEsK70PvomAjEA8yjixtsrmfu3Ubgko6SUeho/5jbiA1czijDLgsfWFBHV
   * dWNbFJWcHwHP2NVypw87
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x13\x0cD-Trust GmbH1\"0 \x06\x03U\x04\x03\x13\x19D-TRUST BR Root CA 1 2020"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xc6\xcb\xc7(\xd1\xfb\x84\xf5\x9a\xefB\x14 \xe1Cknu\xad\xfc+\x03\x84\xd4v\x93%\xd7Y;Aek\x1e\xe64*\xbbt\xf6\x12\xce\xe8m\xe7\xab\xe4<N?D\x08\x8b\xcd\x16q\xcb\xbf\x92\x99\xf4\xa4\xd7<PTR\x90\x85\x83x\x94gg\xa3\x1c\t\x19=u4\x85\xde\xed`}\xc7\x0c\xb4AR\xb9n\xe5\xeeB"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Security Communication ECC RootCA1 O=SECOM Trust Systems CO.,LTD.
   * Subject: CN=Security Communication ECC RootCA1 O=SECOM Trust Systems CO.,LTD.
   * Label: "Security Communication ECC RootCA1"
   * Serial: 15446673492073852651
   * SHA256 Fingerprint: e7:4f:bd:a5:5b:d5:64:c4:73:a3:6b:44:1a:a7:99:c8:a6:8e:07:74:40:e8:28:8b:9f:a1:e5:0e:4b:ba:ca:11
   * -----BEGIN CERTIFICATE-----
   * MIICODCCAb6gAwIBAgIJANZdm7N4gS7rMAoGCCqGSM49BAMDMGExCzAJBgNVBAYT
   * AkpQMSUwIwYDVQQKExxTRUNPTSBUcnVzdCBTeXN0ZW1zIENPLixMVEQuMSswKQYD
   * VQQDEyJTZWN1cml0eSBDb21tdW5pY2F0aW9uIEVDQyBSb290Q0ExMB4XDTE2MDYx
   * NjA1MTUyOFoXDTM4MDExODA1MTUyOFowYTELMAkGA1UEBhMCSlAxJTAjBgNVBAoT
   * HFNFQ09NIFRydXN0IFN5c3RlbXMgQ08uLExURC4xKzApBgNVBAMTIlNlY3VyaXR5
   * IENvbW11bmljYXRpb24gRUNDIFJvb3RDQTEwdjAQBgcqhkjOPQIBBgUrgQQAIgNi
   * AASkpW9gAwPDvTH00xecK4R1rOX9PVdu12O/5gSJko6BnOPpR27KkBLIE+Cnnfdl
   * dB9sELLo5OnvbYUymUSxXv3MdhDYW72ixvnWQuRXdtyQwjWpS4g8EkdtXP9JTxpK
   * ULGjQjBAMB0GA1UdDgQWBBSGHOf+LaVKiwj+KBH6vqNm+GBZLzAOBgNVHQ8BAf8E
   * BAMCAQYwDwYDVR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAwNoADBlAjAVXUI9/Lbu
   * 9zuxNuie9sRGKEkz0FhDKmMpzE2xtHqiuQ04pV1IKv3LsnNdo4gIxwwCMQDAqy0O
   * be0YottT6SXbVQjgUMzfRGEWgqtJsLKB7HOHeLRMsmIbEvoWTSVLY70eN9k=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02JP1%0#\x06\x03U\x04\n\x13\x1cSECOM Trust Systems CO.,LTD.1+0)\x06\x03U\x04\x03\x13\"Security Communication ECC RootCA1"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\xa4\xa5o`\x03\x03\xc3\xbd1\xf4\xd3\x17\x9c+\x84u\xac\xe5\xfd=Wn\xd7c\xbf\xe6\x04\x89\x92\x8e\x81\x9c\xe3\xe9Gn\xca\x90\x12\xc8\x13\xe0\xa7\x9d\xf7et\x1fl\x10\xb2\xe8\xe4\xe9\xefm\x852\x99D\xb1^\xfd\xccv\x10\xd8[\xbd\xa2\xc6\xf9\xd6B\xe4Wv\xdc\x90\xc25\xa9K\x88<\x12Gm\\\xffIO\x1aJP\xb1"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SecureSign Root CA15 O=Cybertrust Japan Co., Ltd.
   * Subject: CN=SecureSign Root CA15 O=Cybertrust Japan Co., Ltd.
   * Label: "SecureSign Root CA15"
   * Serial: 126083514594751269499665114766174399806381178503
   * SHA256 Fingerprint: e7:78:f0:f0:95:fe:84:37:29:cd:1a:00:82:17:9e:53:14:a9:c2:91:44:28:05:e1:fb:1d:8f:b6:b8:88:6c:3a
   * -----BEGIN CERTIFICATE-----
   * MIICIzCCAamgAwIBAgIUFhXHw9hJp75pDIqI7fBw+d23PocwCgYIKoZIzj0EAwMw
   * UTELMAkGA1UEBhMCSlAxIzAhBgNVBAoTGkN5YmVydHJ1c3QgSmFwYW4gQ28uLCBM
   * dGQuMR0wGwYDVQQDExRTZWN1cmVTaWduIFJvb3QgQ0ExNTAeFw0yMDA0MDgwODMy
   * NTZaFw00NTA0MDgwODMyNTZaMFExCzAJBgNVBAYTAkpQMSMwIQYDVQQKExpDeWJl
   * cnRydXN0IEphcGFuIENvLiwgTHRkLjEdMBsGA1UEAxMUU2VjdXJlU2lnbiBSb290
   * IENBMTUwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAAQLUHSNZDKZmbPSYAi4Io5GdCx4
   * wCtELW1fHcmuS1Iggz24FG1Th2CeX2yF2wYUleDHKP+dX+Sq8bOLbe1PL0vJSpSR
   * ZHX+AezB2Ot6lHhWGENfa4HL9rzatAy2KZMIaY+jQjBAMA8GA1UdEwEB/wQFMAMB
   * Af8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBTrQciu/NWeUUj1vYv0hyCTQSvT
   * 9DAKBggqhkjOPQQDAwNoADBlAjEA2S6Jfl5OpBEHvVnCB96rMjhTKkZEBhd6zlHp
   * 4P9mLQlO4E/0BdGF9jVg3PVys0Z9AjBEmEYagoUeYWmJSwdLZrWeqrqgHkHZAXQ6
   * bkU6iYAZezKYVWOr62Nuk22rGwlgMU4=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02JP1#0!\x06\x03U\x04\n\x13\x1aCybertrust Japan Co., Ltd.1\x1d0\x1b\x06\x03U\x04\x03\x13\x14SecureSign Root CA15"),
    subject_public_key_info: Der::from_slice(b"0\x10\x06\x07*\x86H\xce=\x02\x01\x06\x05+\x81\x04\x00\"\x03b\x00\x04\x0bPt\x8dd2\x99\x99\xb3\xd2`\x08\xb8\"\x8eFt,x\xc0+D-m_\x1d\xc9\xaeKR \x83=\xb8\x14mS\x87`\x9e_l\x85\xdb\x06\x14\x95\xe0\xc7(\xff\x9d_\xe4\xaa\xf1\xb3\x8bm\xedO/K\xc9J\x94\x91du\xfe\x01\xec\xc1\xd8\xebz\x94xV\x18C_k\x81\xcb\xf6\xbc\xda\xb4\x0c\xb6)\x93\x08i\x8f"),
    name_constraints: None
  },

  /*
   * Issuer: CN=USERTrust RSA Certification Authority O=The USERTRUST Network
   * Subject: CN=USERTrust RSA Certification Authority O=The USERTRUST Network
   * Label: "USERTrust RSA Certification Authority"
   * Serial: 2645093764781058787591871645665788717
   * SHA256 Fingerprint: e7:93:c9:b0:2f:d8:aa:13:e2:1c:31:22:8a:cc:b0:81:19:64:3b:74:9c:89:89:64:b1:74:6d:46:c3:d4:cb:d2
   * -----BEGIN CERTIFICATE-----
   * MIIF3jCCA8agAwIBAgIQAf1tMPyjylGoG7xkDjUDLTANBgkqhkiG9w0BAQwFADCB
   * iDELMAkGA1UEBhMCVVMxEzARBgNVBAgTCk5ldyBKZXJzZXkxFDASBgNVBAcTC0pl
   * cnNleSBDaXR5MR4wHAYDVQQKExVUaGUgVVNFUlRSVVNUIE5ldHdvcmsxLjAsBgNV
   * BAMTJVVTRVJUcnVzdCBSU0EgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkwHhcNMTAw
   * MjAxMDAwMDAwWhcNMzgwMTE4MjM1OTU5WjCBiDELMAkGA1UEBhMCVVMxEzARBgNV
   * BAgTCk5ldyBKZXJzZXkxFDASBgNVBAcTC0plcnNleSBDaXR5MR4wHAYDVQQKExVU
   * aGUgVVNFUlRSVVNUIE5ldHdvcmsxLjAsBgNVBAMTJVVTRVJUcnVzdCBSU0EgQ2Vy
   * dGlmaWNhdGlvbiBBdXRob3JpdHkwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIK
   * AoICAQCAEmUXNg7D2wiz0KxXDXbtzSfTTK1Qg2HiqiBNCS1kCdzOiZ/MPans9s/B
   * 3PHTsdZ7NygRK0faOca8Ohm0X6a9fZ2jY0K2dvKpOyuR+OJv0OwWIJAJPuLodMkY
   * tJHUYmTbf6MG8YgYapAiPLz+E/CHFHv25B+O1ORRxhFnRghRy4YUVD+8M/5+bJz/
   * Fp0YvVGONaanZshyZ9shZrHUm3gDwFA66Mzw3LyeTP6vBZY1H1dat//O+T23LLb2
   * VN3I5xI6Ta5MirdcmrS3ID3KfyI0rn47aGYBROcBTkZTmzNg95S+UzeQc0PzMsNT
   * 79uq/nROacdrjGCT3sTHDN/hMq7MkztReJVni+49Vv4M0GkPGw/zJSZrM233bkf6
   * c0Plfg6lZrEpfDKEY1WJxA3Bk1QwGROs0303p+tdOmw1XNtB1xLaqUkL39iAigmT
   * Yo61Zs8liM2EuLE/pDkP2QKe6xJMlXzzawWpXhaDzLhn4ugTncxbgtNMs+1b/97l
   * c6wjOy0AvzVVdAlJ2ElYGn+SNuZRkg7zJn0cTRe8yexDJtC/QV9AqURE9JnnV4ee
   * UB9XVKg+/XRjL7FQZQnmWEIuQxpMtPAlR1n6BB6T1CZGSlCBst6+eLf8ZxXhyVeE
   * Hg9j1uliutZfVS7qXMYoCAQlObgOK6nyTJccBz8NUvXt7y+CDwIDAQABo0IwQDAd
   * BgNVHQ4EFgQUU3m/WqorSs9UgOHYm8Cd8rIDZsswDgYDVR0PAQH/BAQDAgEGMA8G
   * A1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQEMBQADggIBAFzUfA3P9wF9QZllDHPF
   * Up/L+M+ZBn8b2kMVn54CVVeWFPFSPCeHlCjtHzoBN6J2/FNQwISbxmtOuowhT6KO
   * VWKR82kV2LyI48SqC/3vqOlLVSoGIG1VeCkZ7l8wXEskEVX/JJpuXior7gtNn3/3
   * ATiUFJVDBwn7YKnuHKsSjKCaXqeYalltiz8I+8jRRa8YFWSQEg9zKC7F4iRO/Fjs
   * 8PRF/iKz6y+O0tlFYQXBl2+odnKPi4w2r78NBc5xjeambx9spnFixdjQg3IM8WcR
   * iQycE0xyNN+81XHfqnHd4blsjDwSXWXavVcStkNr/+XeTWYRUc+ZruwXtuhxkYze
   * Sf7dNXGiFSeUHM9h4ya7b6NnJSFd5t0dCy5oGzuCr+yDZ4XUmFF0sbmZgIn/f3gZ
   * XHlKYC6SQK5MNyosycdiyA5d9zZbyuAlJQG03RoHnHcAP9Dc1ew91Pq7P8yF1m9/
   * qS3fuQL39ZeatTXaw2ewh0qpKJ4jjv9cJ2vhsE/zB+4ALtRZh8tSQZXq9EfX7mRB
   * VXyNWQKV3WKdwrnuWih0hKWbt5DHDAff9Yk2dDLWKMGwsAvgnEzDHNb842m1R0aB
   * L6KCq9NjRHDEjf8tM7qtj3u1cIiuPhnPQCjY/MiQu12ZIvVS5ljFH4gxQ+6IHdfG
   * jjxDah2nGN59PRbxYvnKkKj9
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x130\x11\x06\x03U\x04\x08\x13\nNew Jersey1\x140\x12\x06\x03U\x04\x07\x13\x0bJersey City1\x1e0\x1c\x06\x03U\x04\n\x13\x15The USERTRUST Network1.0,\x06\x03U\x04\x03\x13%USERTrust RSA Certification Authority"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\x80\x12e\x176\x0e\xc3\xdb\x08\xb3\xd0\xacW\rv\xed\xcd\'\xd3L\xadP\x83a\xe2\xaa M\t-d\t\xdc\xce\x89\x9f\xcc=\xa9\xec\xf6\xcf\xc1\xdc\xf1\xd3\xb1\xd6{7(\x11+G\xda9\xc6\xbc:\x19\xb4_\xa6\xbd}\x9d\xa3cB\xb6v\xf2\xa9;+\x91\xf8\xe2o\xd0\xec\x16 \x90\t>\xe2\xe8t\xc9\x18\xb4\x91\xd4bd\xdb\x7f\xa3\x06\xf1\x88\x18j\x90\"<\xbc\xfe\x13\xf0\x87\x14{\xf6\xe4\x1f\x8e\xd4\xe4Q\xc6\x11gF\x08Q\xcb\x86\x14T?\xbc3\xfe~l\x9c\xff\x16\x9d\x18\xbdQ\x8e5\xa6\xa7f\xc8rg\xdb!f\xb1\xd4\x9bx\x03\xc0P:\xe8\xcc\xf0\xdc\xbc\x9eL\xfe\xaf\x05\x965\x1fWZ\xb7\xff\xce\xf9=\xb7,\xb6\xf6T\xdd\xc8\xe7\x12:M\xaeL\x8a\xb7\\\x9a\xb4\xb7 =\xca\x7f\"4\xae~;hf\x01D\xe7\x01NFS\x9b3`\xf7\x94\xbeS7\x90sC\xf32\xc3S\xef\xdb\xaa\xfetNi\xc7k\x8c`\x93\xde\xc4\xc7\x0c\xdf\xe12\xae\xcc\x93;Qx\x95g\x8b\xee=V\xfe\x0c\xd0i\x0f\x1b\x0f\xf3%&k3m\xf7nG\xfasC\xe5~\x0e\xa5f\xb1)|2\x84cU\x89\xc4\r\xc1\x93T0\x19\x13\xac\xd3}7\xa7\xeb]:l5\\\xdbA\xd7\x12\xda\xa9I\x0b\xdf\xd8\x80\x8a\t\x93b\x8e\xb5f\xcf%\x88\xcd\x84\xb8\xb1?\xa49\x0f\xd9\x02\x9e\xeb\x12L\x95|\xf3k\x05\xa9^\x16\x83\xcc\xb8g\xe2\xe8\x13\x9d\xcc[\x82\xd3L\xb3\xed[\xff\xde\xe5s\xac#;-\x00\xbf5Ut\tI\xd8IX\x1a\x7f\x926\xe6Q\x92\x0e\xf3&}\x1cM\x17\xbc\xc9\xecC&\xd0\xbfA_@\xa9DD\xf4\x99\xe7W\x87\x9eP\x1fWT\xa8>\xfdtc/\xb1Pe\t\xe6XB.C\x1aL\xb4\xf0%GY\xfa\x04\x1e\x93\xd4&FJP\x81\xb2\xde\xbex\xb7\xfcg\x15\xe1\xc9W\x84\x1e\x0fc\xd6\xe9b\xba\xd6_U.\xea\\\xc6(\x08\x04%9\xb8\x0e+\xa9\xf2L\x97\x1c\x07?\rR\xf5\xed\xef/\x82\x0f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: O=certSIGN OU=certSIGN ROOT CA
   * Subject: O=certSIGN OU=certSIGN ROOT CA
   * Label: "certSIGN ROOT CA"
   * Serial: 35210227249154
   * SHA256 Fingerprint: ea:a9:62:c4:fa:4a:6b:af:eb:e4:15:19:6d:35:1c:cd:88:8d:4f:53:f3:fa:8a:e6:d7:c4:66:a9:4e:60:42:bb
   * -----BEGIN CERTIFICATE-----
   * MIIDODCCAiCgAwIBAgIGIAYFFnACMA0GCSqGSIb3DQEBBQUAMDsxCzAJBgNVBAYT
   * AlJPMREwDwYDVQQKEwhjZXJ0U0lHTjEZMBcGA1UECxMQY2VydFNJR04gUk9PVCBD
   * QTAeFw0wNjA3MDQxNzIwMDRaFw0zMTA3MDQxNzIwMDRaMDsxCzAJBgNVBAYTAlJP
   * MREwDwYDVQQKEwhjZXJ0U0lHTjEZMBcGA1UECxMQY2VydFNJR04gUk9PVCBDQTCC
   * ASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALczuX7IJUqOtdu0KBuqV5Do
   * 0SLTZLrTk+jUrIZhQGpgV2hUhE28alQCBf/fm5oqrl0Hj0rDKH/v+yv6efHHrfAQ
   * UySQi2bJqIirr1qjAOm+ukbuW3N7LBeCgV5iLKECZbO9xSsAfsT8AzNXDe3i+s5d
   * RdY4zTW2ssHQnIFKquSyAVwdj1+ZxLGt24gh65AIgoDzMKND5pCCrlUoSe1b16kQ
   * OA7+j0xbm0bqQfWwCHTD0IgztnzXdN/chNFDDnU5oSVAKOp4yw4sLjmdjItuFhwv
   * JoIQ4uNllAoEwF73XVv4EOLQunpL+943AAAaWyjj0pxzPjKHmKHJUS/X3qwzs08C
   * AwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAcYwHQYDVR0O
   * BBYEFOCMm9slSbPxfIbWskKHC9BroNnkMA0GCSqGSIb3DQEBBQUAA4IBAQA+0hyJ
   * LjX8+HXd5n9liPRyTMks1zJO890ZeUe9jjtbkw9QSSQTaxQGcu8J06Gh40CEyecY
   * MnQ8SG4Pn0vU9x7Tk4ZkVJdjclDVVc/6IJMCopvDI5NOFlV2oHB5bc0hH88vLbwZ
   * 44gx+FkagQnIl6Z0x2DEW8xXjrJ1/RsCCdtZb3KTafcxQdaIOL+Hsr0Wefmq5L6I
   * Jd1hJyMctTEHBDa0GpC9oHRxUIltvBTjD4au8as+x6AJzKNI0eDbZOeStc+vckNw
   * i/nDhDwTqn6Sm1dTk/pwwpEOMfmbZ13pljheX7NzTogVZ96edhBiIL5VaZVDADlN
   * 9u6wWk5JRFRYX0KD
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02RO1\x110\x0f\x06\x03U\x04\n\x13\x08certSIGN1\x190\x17\x06\x03U\x04\x0b\x13\x10certSIGN ROOT CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xb73\xb9~\xc8%J\x8e\xb5\xdb\xb4(\x1b\xaaW\x90\xe8\xd1\"\xd3d\xba\xd3\x93\xe8\xd4\xac\x86a@j`WhT\x84M\xbcjT\x02\x05\xff\xdf\x9b\x9a*\xae]\x07\x8fJ\xc3(\x7f\xef\xfb+\xfay\xf1\xc7\xad\xf0\x10S$\x90\x8bf\xc9\xa8\x88\xab\xafZ\xa3\x00\xe9\xbe\xbaF\xee[s{,\x17\x82\x81^b,\xa1\x02e\xb3\xbd\xc5+\x00~\xc4\xfc\x033W\r\xed\xe2\xfa\xce]E\xd68\xcd5\xb6\xb2\xc1\xd0\x9c\x81J\xaa\xe4\xb2\x01\\\x1d\x8f_\x99\xc4\xb1\xad\xdb\x88!\xeb\x90\x08\x82\x80\xf30\xa3C\xe6\x90\x82\xaeU(I\xed[\xd7\xa9\x108\x0e\xfe\x8fL[\x9bF\xeaA\xf5\xb0\x08t\xc3\xd0\x883\xb6|\xd7t\xdf\xdc\x84\xd1C\x0eu9\xa1%@(\xeax\xcb\x0e,.9\x9d\x8c\x8bn\x16\x1c/&\x82\x10\xe2\xe3e\x94\n\x04\xc0^\xf7][\xf8\x10\xe2\xd0\xbazK\xfb\xde7\x00\x00\x1a[(\xe3\xd2\x9cs>2\x87\x98\xa1\xc9Q/\xd7\xde\xac3\xb3O\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: O=FNMT-RCM OU=AC RAIZ FNMT-RCM
   * Subject: O=FNMT-RCM OU=AC RAIZ FNMT-RCM
   * Label: "AC RAIZ FNMT-RCM"
   * Serial: 485876308206448804701554682760554759
   * SHA256 Fingerprint: eb:c5:57:0c:29:01:8c:4d:67:b1:aa:12:7b:af:12:f7:03:b4:61:1e:bc:17:b7:da:b5:57:38:94:17:9b:93:fa
   * -----BEGIN CERTIFICATE-----
   * MIIFgzCCA2ugAwIBAgIPXZONMGc2yAYdGsdUhGkHMA0GCSqGSIb3DQEBCwUAMDsx
   * CzAJBgNVBAYTAkVTMREwDwYDVQQKDAhGTk1ULVJDTTEZMBcGA1UECwwQQUMgUkFJ
   * WiBGTk1ULVJDTTAeFw0wODEwMjkxNTU5NTZaFw0zMDAxMDEwMDAwMDBaMDsxCzAJ
   * BgNVBAYTAkVTMREwDwYDVQQKDAhGTk1ULVJDTTEZMBcGA1UECwwQQUMgUkFJWiBG
   * Tk1ULVJDTTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBALpxgHpMhm5/
   * yBNtwMZ9HACXjywMI7sQmkCpGreHiPibVmr75nuOi5KOpyVdWRHbNi63URcfqQgf
   * BBckWKo3Shjf5TnUV/3XwSyRAZHiItQDwFj8d0fsjz50Q7qsNI1NOHZnjrDIbzAz
   * WHFctPVrbtQBULgTfmxKo0nRIBnuvMApGGWn3v7v3QqQIecaZ5JCEJhfTzC8PhxF
   * tBDXaEAUwED653cXeuYLj2VbPNmaUtu1vZ5Gzz3rkQUCwJaydkxNEJY7kvqcfw+Z
   * 374jNUUeAlz+taibmSXaXvMiwzn15Cou08YfxGyqxRxqAQVKL9LFwag0Jl1mpdIC
   * IfkYtwb1TplvqKtMUejPUBjFd8g5CSxJkjKZqLsXF3mwWsXmo8RZZUc1g16p6DUL
   * mbvkzSDGm0oGObVo/CK67lWMK07q87Hj/LaZmtVC+nFNCM+HHmpxffnTtOmlcYF7
   * wk5HlqX2doWjKI/pgG6BU6VtX7hI+cL5NqYuSf+4lsKMB7ObiFj86xsc3i1w4peS
   * MKGJ47xVqCfWS+2QrYv6YyVZLag13cqXM7zlzced0ezvXg5KkAYmY6252TUtB7p2
   * ZSysV4999AeU14ECll2jB0nVetBX+RvnU0Z1qrB5QstocQjpYL05ac70r8NWQMet
   * UqIJ5G+GR4of6ygnXYMgrwTJbFaai0b1AgMBAAGjgYMwgYAwDwYDVR0TAQH/BAUw
   * AwEB/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFPd9xf3E6Jobd2Sn9R2gzL+H
   * YJptMD4GA1UdIAQ3MDUwMwYEVR0gADArMCkGCCsGAQUFBwIBFh1odHRwOi8vd3d3
   * LmNlcnQuZm5tdC5lcy9kcGNzLzANBgkqhkiG9w0BAQsFAAOCAgEAB5BK3/MjTvDD
   * nFFlm5wioooMhfNzKWtN/gHiqQxjAb8EZ6WdmF/9ARP67Jpi6Yb+tmLSbkyU+8B1
   * RXxlDPiyN8+sD8+Nb/kZ94/sHvJwnvDKuO+3/3Y3dlv2bojzr2IyIpMNOmqOFGYM
   * LVN0V2Ue1bLdI4E7pWYjJ2cJj+F3qkPNZVEI7VFY/uY5+ctHhKQV8Xa7pO6kO8Rf
   * 77IzlhEYt8llvhjho6Tc+hj507wTmzl6NLrTQfv6MooqtyuGC2mDOL7Nii4LcK2N
   * JpLuHvUBKwrZ1pebbuCoGRw6IYsMHkCtA+fdZn71uSANA+iW+YJF1DngoABd15jm
   * fZ5nc8OaKveri6E6FO80vFIOiZiaBECEHX5FaZNXzuvO+FB8TxxuBEOb+dY7Ixjp
   * 6o7RTUaN8Tvkasq6+yO3m/qZASlaWFot4/nUbQ4mrcFuNLwy+AwF+mWj2zs3gyLp
   * 1txyM/1d8iC9djwj2ij3+RvrWWTV3F9yfiD8zYm1kGdNYno/Tq0dwzn+evQoFt9B
   * 9kiABdcPUXmsEKvU7ANm5mqwujGSQkBqvjrTcuFqN1W8rB2Vt2lh8kORdOag0wok
   * RqEIr9baRRmW1FMdW4R58MD3R++Lj8UGrp1MYp3/RgT408m2ECVAdf4WqslKYIYv
   * uu8wd+RU4riEmViAqhOLUTpPSPaLtrM=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02ES1\x110\x0f\x06\x03U\x04\n\x0c\x08FNMT-RCM1\x190\x17\x06\x03U\x04\x0b\x0c\x10AC RAIZ FNMT-RCM"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xbaq\x80zL\x86n\x7f\xc8\x13m\xc0\xc6}\x1c\x00\x97\x8f,\x0c#\xbb\x10\x9a@\xa9\x1a\xb7\x87\x88\xf8\x9bVj\xfb\xe6{\x8e\x8b\x92\x8e\xa7%]Y\x11\xdb6.\xb7Q\x17\x1f\xa9\x08\x1f\x04\x17$X\xaa7J\x18\xdf\xe59\xd4W\xfd\xd7\xc1,\x91\x01\x91\xe2\"\xd4\x03\xc0X\xfcwG\xec\x8f>tC\xba\xac4\x8dM8vg\x8e\xb0\xc8o03Xq\\\xb4\xf5kn\xd4\x01P\xb8\x13~lJ\xa3I\xd1 \x19\xee\xbc\xc0)\x18e\xa7\xde\xfe\xef\xdd\n\x90!\xe7\x1ag\x92B\x10\x98_O0\xbc>\x1cE\xb4\x10\xd7h@\x14\xc0@\xfa\xe7w\x17z\xe6\x0b\x8fe[<\xd9\x9aR\xdb\xb5\xbd\x9eF\xcf=\xeb\x91\x05\x02\xc0\x96\xb2vLM\x10\x96;\x92\xfa\x9c\x7f\x0f\x99\xdf\xbe#5E\x1e\x02\\\xfe\xb5\xa8\x9b\x99%\xda^\xf3\"\xc39\xf5\xe4*.\xd3\xc6\x1f\xc4l\xaa\xc5\x1cj\x01\x05J/\xd2\xc5\xc1\xa84&]f\xa5\xd2\x02!\xf9\x18\xb7\x06\xf5N\x99o\xa8\xabLQ\xe8\xcfP\x18\xc5w\xc89\t,I\x922\x99\xa8\xbb\x17\x17y\xb0Z\xc5\xe6\xa3\xc4YeG5\x83^\xa9\xe85\x0b\x99\xbb\xe4\xcd \xc6\x9bJ\x069\xb5h\xfc\"\xba\xeeU\x8c+N\xea\xf3\xb1\xe3\xfc\xb6\x99\x9a\xd5B\xfaqM\x08\xcf\x87\x1ejq}\xf9\xd3\xb4\xe9\xa5q\x81{\xc2NG\x96\xa5\xf6v\x85\xa3(\x8f\xe9\x80n\x81S\xa5m_\xb8H\xf9\xc2\xf96\xa6.I\xff\xb8\x96\xc2\x8c\x07\xb3\x9b\x88X\xfc\xeb\x1b\x1c\xde-p\xe2\x97\x920\xa1\x89\xe3\xbcU\xa8\'\xd6K\xed\x90\xad\x8b\xfac%Y-\xa85\xdd\xca\x973\xbc\xe5\xcd\xc7\x9d\xd1\xec\xef^\x0eJ\x90\x06&c\xad\xb9\xd95-\x07\xbave,\xacW\x8f}\xf4\x07\x94\xd7\x81\x02\x96]\xa3\x07I\xd5z\xd0W\xf9\x1b\xe7SFu\xaa\xb0yB\xcbhq\x08\xe9`\xbd9i\xce\xf4\xaf\xc3V@\xc7\xadR\xa2\t\xe4o\x86G\x8a\x1f\xeb(\']\x83 \xaf\x04\xc9lV\x9a\x8bF\xf5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Buypass Class 3 Root CA O=Buypass AS-983163327
   * Subject: CN=Buypass Class 3 Root CA O=Buypass AS-983163327
   * Label: "Buypass Class 3 Root CA"
   * Serial: 2
   * SHA256 Fingerprint: ed:f7:eb:bc:a2:7a:2a:38:4d:38:7b:7d:40:10:c6:66:e2:ed:b4:84:3e:4c:29:b4:ae:1d:5b:93:32:e6:b2:4d
   * -----BEGIN CERTIFICATE-----
   * MIIFWTCCA0GgAwIBAgIBAjANBgkqhkiG9w0BAQsFADBOMQswCQYDVQQGEwJOTzEd
   * MBsGA1UECgwUQnV5cGFzcyBBUy05ODMxNjMzMjcxIDAeBgNVBAMMF0J1eXBhc3Mg
   * Q2xhc3MgMyBSb290IENBMB4XDTEwMTAyNjA4Mjg1OFoXDTQwMTAyNjA4Mjg1OFow
   * TjELMAkGA1UEBhMCTk8xHTAbBgNVBAoMFEJ1eXBhc3MgQVMtOTgzMTYzMzI3MSAw
   * HgYDVQQDDBdCdXlwYXNzIENsYXNzIDMgUm9vdCBDQTCCAiIwDQYJKoZIhvcNAQEB
   * BQADggIPADCCAgoCggIBAKXaCpUWUOOV8l6ddjEGMnqb8RB2uACatVI2zSRHsJ8Y
   * ZLya9vrVediQYkwiL944PdbgqOkcLNt4EemOaFEVcsfzM4fkoF0LXOBXByow9c3E
   * N3coTRiR5r/VUv1xLXA+58bEiuPwKAv0dpihi4dVsjoT/Lc+JzeOIuOoTyrvYLs9
   * tznDDgFHmV0ST9tD+leh7fmdvhFHJlsTmKtdFoqwNxxXnUX/iJY2v7vKB3tvh2PX
   * 0DJq1l1sDPGzbjniazEuOQAnFN44wOwZZoYS6J1yFhNkUsepNxz9gjDthBgd9K5c
   * /3ATAOux9TN6S9ZV+AWNS2mw9bMoNlwUxFFzTWsL8TQH2xc519woe2v1n/MuwU8X
   * KhDzzMro6/1rqy6any2CbgTUUgGTLT2G/H783+9CHaZr77kgxve9oKeV/afmiSTY
   * zIw0bOIjL9kSGiG5VZFvC5F5GQytQIgLcOJ60g7YaEi7ghM5EFjp2CoHxhLbWNvS
   * O1UQRwUVZ2J+GGOmRj8JDlQyXr8NYnon74Do29lLBlo3WiXQCBJ31G8JUJc9yB3D
   * 34xFMFbG02SrZvPAXpacw8Tvw3xrizp5f7NJzz3iiZ+gMEuFuZyUJHmPfWupRWgP
   * K9Dx2hzLabjKSWJtyNBjYt1gD1iqj6G8BaVmos8bdrKEZLFMOVLAMLrwjEsCsLa3
   * AgMBAAGjQjBAMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFEe4zf/lb+74suwv
   * Tg75JbCOPGvDMA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAACAj
   * QTUEkMJAYmDv4jVM1z+s4jSQuKFvdvoWFqRINyzpkMLyPPgKn9iB5btb2iUspKdV
   * cSQy9sgL8rxq+JOssgfCX5/bzMiKqr5qb+FJEMwx14C7u8jYog5kV+qi9cKpMRXS
   * IGrs/CIBKM+GuIAeqcwRpTzyFrNHnfzSgCHEy9BHcEGhyoMZCCxt8l13nIoUE9Q2
   * HJLw5QY33KbmkJs4j1xrG0aGQ0JfPgEHU1RdZX33inOhmlRaHylDFCfChQ+1iHsa
   * O5S3HWCntZznKWlXWpuTekMwGwPXYshApqr8ZORK15FTAaggiG6cX0S5y2CBNOxv
   * 033aSF/rtJC8LakcC6wc1aJoIIAE1vyxjy+7SjENSoYc6+I2KSb12tjE8nVhz36u
   * dmNKekBlk4f4HoCMhuWG1o8O/FMsYOgWYRqiPkN7zTlgVGr18okmAWiDSKIz6MkE
   * kbIRNBE+6tBDGR8Dk5AM/1E9V/RBbuHLoL7ryWPNbczk+DaqaJ3tvV2XcEQNtg41
   * 3OEMXbugUZTLfhbrES+jkkXITHHZvMmZUldGL1DPvTVp9D0VzgalLA8+9oG6lLvD
   * u79leNKGef9JOxqDDPDeeOzI8k1MGt6CKfjBWtrt7uYnXuhF0J0cUahoq0Tj0Itq
   * 4/g7u9xN12TyUb7mqqta6THuBrxzvxNiCp/HuZc=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02NO1\x1d0\x1b\x06\x03U\x04\n\x0c\x14Buypass AS-9831633271 0\x1e\x06\x03U\x04\x03\x0c\x17Buypass Class 3 Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xa5\xda\n\x95\x16P\xe3\x95\xf2^\x9dv1\x062z\x9b\xf1\x10v\xb8\x00\x9a\xb5R6\xcd$G\xb0\x9f\x18d\xbc\x9a\xf6\xfa\xd5y\xd8\x90bL\"/\xde8=\xd6\xe0\xa8\xe9\x1c,\xdbx\x11\xe9\x8ehQ\x15r\xc7\xf33\x87\xe4\xa0]\x0b\\\xe0W\x07*0\xf5\xcd\xc47w(M\x18\x91\xe6\xbf\xd5R\xfdq-p>\xe7\xc6\xc4\x8a\xe3\xf0(\x0b\xf4v\x98\xa1\x8b\x87U\xb2:\x13\xfc\xb7>\'7\x8e\"\xe3\xa8O*\xef`\xbb=\xb79\xc3\x0e\x01G\x99]\x12O\xdbC\xfaW\xa1\xed\xf9\x9d\xbe\x11G&[\x13\x98\xab]\x16\x8a\xb07\x1cW\x9dE\xff\x88\x966\xbf\xbb\xca\x07{o\x87c\xd7\xd02j\xd6]l\x0c\xf1\xb3n9\xe2k1.9\x00\'\x14\xde8\xc0\xec\x19f\x86\x12\xe8\x9dr\x16\x13dR\xc7\xa97\x1c\xfd\x820\xed\x84\x18\x1d\xf4\xae\\\xffp\x13\x00\xeb\xb1\xf53zK\xd6U\xf8\x05\x8dKi\xb0\xf5\xb3(6\\\x14\xc4QsMk\x0b\xf14\x07\xdb\x179\xd7\xdc({k\xf5\x9f\xf3.\xc1O\x17*\x10\xf3\xcc\xca\xe8\xeb\xfdk\xab.\x9a\x9f-\x82n\x04\xd4R\x01\x93-=\x86\xfc~\xfc\xdf\xefB\x1d\xa6k\xef\xb9 \xc6\xf7\xbd\xa0\xa7\x95\xfd\xa7\xe6\x89$\xd8\xcc\x8c4l\xe2#/\xd9\x12\x1a!\xb9U\x91o\x0b\x91y\x19\x0c\xad@\x88\x0bp\xe2z\xd2\x0e\xd8hH\xbb\x82\x139\x10X\xe9\xd8*\x07\xc6\x12\xdbX\xdb\xd2;U\x10G\x05\x15gb~\x18c\xa6F?\t\x0eT2^\xbf\rbz\'\xef\x80\xe8\xdb\xd9K\x06Z7Z%\xd0\x08\x12w\xd4o\tP\x97=\xc8\x1d\xc3\xdf\x8cE0V\xc6\xd3d\xabf\xf3\xc0^\x96\x9c\xc3\xc4\xef\xc3|k\x8b:y\x7f\xb3I\xcf=\xe2\x89\x9f\xa00K\x85\xb9\x9c\x94$y\x8f}k\xa9Eh\x0f+\xd0\xf1\xda\x1c\xcbi\xb8\xcaIbm\xc8\xd0cb\xdd`\x0fX\xaa\x8f\xa1\xbc\x05\xa5f\xa2\xcf\x1bv\xb2\x84d\xb1L9R\xc00\xba\xf0\x8cK\x02\xb0\xb6\xb7\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=D-TRUST Root Class 3 CA 2 EV 2009 O=D-Trust GmbH
   * Subject: CN=D-TRUST Root Class 3 CA 2 EV 2009 O=D-Trust GmbH
   * Label: "D-TRUST Root Class 3 CA 2 EV 2009"
   * Serial: 623604
   * SHA256 Fingerprint: ee:c5:49:6b:98:8c:e9:86:25:b9:34:09:2e:ec:29:08:be:d0:b0:f3:16:c2:d4:73:0c:84:ea:f1:f3:d3:48:81
   * -----BEGIN CERTIFICATE-----
   * MIIEQzCCAyugAwIBAgIDCYP0MA0GCSqGSIb3DQEBCwUAMFAxCzAJBgNVBAYTAkRF
   * MRUwEwYDVQQKDAxELVRydXN0IEdtYkgxKjAoBgNVBAMMIUQtVFJVU1QgUm9vdCBD
   * bGFzcyAzIENBIDIgRVYgMjAwOTAeFw0wOTExMDUwODUwNDZaFw0yOTExMDUwODUw
   * NDZaMFAxCzAJBgNVBAYTAkRFMRUwEwYDVQQKDAxELVRydXN0IEdtYkgxKjAoBgNV
   * BAMMIUQtVFJVU1QgUm9vdCBDbGFzcyAzIENBIDIgRVYgMjAwOTCCASIwDQYJKoZI
   * hvcNAQEBBQADggEPADCCAQoCggEBAJnxhDRwui+3MKCOvXwEz75ivJn9gpfSegpn
   * ljgJ9hBOlSJzmY3aFS3nBfwZcyK3jpgAvDw9rKFs+9Z5JUut8Mxk2og+KbgPCdM0
   * 3TP1YtHhzRnp7hhPTFiu4h7WDFsVWtg6uMQYZB7jM7K1iXdODL/ZlGsTl28So/6Z
   * qQTMFexgaDbtCHu39b+T7WYxg4zGcTSHThfqr4uRjRxWQa4iN1438h3Z0S0NL2lR
   * p75mpoo6Kr3HGrHhFPC+Oh25z1uxav60sUYgovseO3Dvk5h9jHOW8sXvhXCtKSb8
   * HgQ+HKDYD8tSg2J87otTlZCpV6LqYQXY+U3EJ/pure3511H3a6UCAwEAAaOCASQw
   * ggEgMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYEFNOUikxiEyoZLsyvcop9Ntea
   * HNxnMA4GA1UdDwEB/wQEAwIBBjCB3QYDVR0fBIHVMIHSMIGHoIGEoIGBhn9sZGFw
   * Oi8vZGlyZWN0b3J5LmQtdHJ1c3QubmV0L0NOPUQtVFJVU1QlMjBSb290JTIwQ2xh
   * c3MlMjAzJTIwQ0ElMjAyJTIwRVYlMjAyMDA5LE89RC1UcnVzdCUyMEdtYkgsQz1E
   * RT9jZXJ0aWZpY2F0ZXJldm9jYXRpb25saXN0MEagRKBChkBodHRwOi8vd3d3LmQt
   * dHJ1c3QubmV0L2NybC9kLXRydXN0X3Jvb3RfY2xhc3NfM19jYV8yX2V2XzIwMDku
   * Y3JsMA0GCSqGSIb3DQEBCwUAA4IBAQA07XtaPKSUiO8aEXUHL7P+PPoeUSbrh/Yp
   * 3uDx1MYkCenBz1UbtDDZzhr+BlGmFaQt77JLvyAoJUnRpjZ3NOhk31KxEcdzes05
   * nsKtjHEh8lprr988TlWvsoRlFIm5d8sqMb7Po23Pb0iUMkZv53GMoKaEGTcH8gNF
   * CSuGdXzfX2lXANtu2KZyIktQ1HWYVt+3GP9DQ1CuekR78HlR10M9p9OB0/DJT7na
   * xpeG0ILD5EJt/rDiZE4OJudANCa1CInXCGNjOCd1HjPqbqjdn5lPdE2BiYBL3ZqX
   * KVwvvoFBuYz/6n1gBp7N1z3TLqMVvKjmJuVvw9y4AyHqnxbxLFS1
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\x150\x13\x06\x03U\x04\n\x0c\x0cD-Trust GmbH1*0(\x06\x03U\x04\x03\x0c!D-TRUST Root Class 3 CA 2 EV 2009"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\x99\xf1\x844p\xba/\xb70\xa0\x8e\xbd|\x04\xcf\xbeb\xbc\x99\xfd\x82\x97\xd2z\ng\x968\t\xf6\x10N\x95\"s\x99\x8d\xda\x15-\xe7\x05\xfc\x19s\"\xb7\x8e\x98\x00\xbc<=\xac\xa1l\xfb\xd6y%K\xad\xf0\xccd\xda\x88>)\xb8\x0f\t\xd34\xdd3\xf5b\xd1\xe1\xcd\x19\xe9\xee\x18OLX\xae\xe2\x1e\xd6\x0c[\x15Z\xd8:\xb8\xc4\x18d\x1e\xe33\xb2\xb5\x89wN\x0c\xbf\xd9\x94k\x13\x97o\x12\xa3\xfe\x99\xa9\x04\xcc\x15\xec`h6\xed\x08{\xb7\xf5\xbf\x93\xedf1\x83\x8c\xc6q4\x87N\x17\xea\xaf\x8b\x91\x8d\x1cVA\xae\"7^7\xf2\x1d\xd9\xd1-\r/iQ\xa7\xbef\xa6\x8a:*\xbd\xc7\x1a\xb1\xe1\x14\xf0\xbe:\x1d\xb9\xcf[\xb1j\xfe\xb4\xb1F \xa2\xfb\x1e;p\xef\x93\x98}\x8cs\x96\xf2\xc5\xef\x85p\xad)&\xfc\x1e\x04>\x1c\xa0\xd8\x0f\xcbR\x83b|\xee\x8bS\x95\x90\xa9W\xa2\xeaa\x05\xd8\xf9M\xc4\'\xfan\xad\xed\xf9\xd7Q\xf7k\xa5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Telekom Security TLS RSA Root 2023 O=Deutsche Telekom Security GmbH
   * Subject: CN=Telekom Security TLS RSA Root 2023 O=Deutsche Telekom Security GmbH
   * Label: "Telekom Security TLS RSA Root 2023"
   * Serial: 44676229530606711399881795178081572759
   * SHA256 Fingerprint: ef:c6:5c:ad:bb:59:ad:b6:ef:e8:4d:a2:23:11:b3:56:24:b7:1b:3b:1e:a0:da:8b:66:55:17:4e:c8:97:86:46
   * -----BEGIN CERTIFICATE-----
   * MIIFszCCA5ugAwIBAgIQIZxULej27HF3+k7ow3BXlzANBgkqhkiG9w0BAQwFADBj
   * MQswCQYDVQQGEwJERTEnMCUGA1UECgweRGV1dHNjaGUgVGVsZWtvbSBTZWN1cml0
   * eSBHbWJIMSswKQYDVQQDDCJUZWxla29tIFNlY3VyaXR5IFRMUyBSU0EgUm9vdCAy
   * MDIzMB4XDTIzMDMyODEyMTY0NVoXDTQ4MDMyNzIzNTk1OVowYzELMAkGA1UEBhMC
   * REUxJzAlBgNVBAoMHkRldXRzY2hlIFRlbGVrb20gU2VjdXJpdHkgR21iSDErMCkG
   * A1UEAwwiVGVsZWtvbSBTZWN1cml0eSBUTFMgUlNBIFJvb3QgMjAyMzCCAiIwDQYJ
   * KoZIhvcNAQEBBQADggIPADCCAgoCggIBAO01oYGA88tKaVvC+1GDrib94W7zgRJ9
   * cUD/h3VCKSHtgVIs3xLBGYSJwb3FKNXVS2xE1kzbB5ZKVXrKNoIENqil/Cf2SfHV
   * cp6R+SPWcHu79ZvB7JPPGeplfohwoHP89v+1VmLhc2o0mD6CuKyVU/QBoCcHcqMA
   * U6DksquDOFczJZSfvkgdmOGjup5czQRxUX11eKvzWarE4GC+j4NSuHUaQTXtvPM6
   * Y+mpFEXX5lLRbtLevOP1Czvm4MS9Q2QTps70mDdsipWol8hHD/BeEIvnHRz+sTug
   * BTNoBUGCwQMrAcjnj02r6LX2zWtEtefdi+zqJbQAIldNsLGyMcEWzv/9FIS3R/qy
   * 8XDe24tsNlikfLMR0cN3f1+2JeANxdKz+bi4d9s3cXFH42AYTyS2dTd4uaNir73J
   * co4vzLuu2+QVUhkHM/tqty1LkCiCc/4YizWN26cEar7qwU02OxY2kTLvtkCJkUPg
   * 8qKrBC7m8kwOFjQgrIfBLX7JZkcXFBGk8/ehJImr2BrIoVyxo/eMbcgByU/J7MT8
   * rFEz0ciD0cmfHdRHNCk+y7AO+oMLKFjlKdw/fKifybYKu6boRhYPluV75Gp6SG12
   * mAWl3G0eQh5C2hrgUve1g8Aae3g1LDj1H/1Joy7SWWO/gLCMk3PLNaaZlSJhZQNg
   * +y+TS/qanIA7AgMBAAGjYzBhMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUtqeX
   * gj10hZv3PJ+TmpV5dVKMbUcwDwYDVR0TAQH/BAUwAwEB/zAfBgNVHSMEGDAWgBS2
   * p5eCPXSFm/c8n5OalXl1UoxtRzANBgkqhkiG9w0BAQwFAAOCAgEAqMxhpr51nhVQ
   * pGv7qHBFfLp+sVr8WyP6Cnf4mHGCDG3gXkaqk/QeoMPhk9tLrbKmXauw1GLLXrtm
   * 9S3ul0A8Yute1hTWjOKWi0FpkzXmuZlrYrShF2Y0pmtjxrlO8iLpWA1WQdH6DErw
   * M807u20hOq6OcrXDSvvpfeWxm4bu4uB9tPcy/SKE8YXJN3nptT+/XOR0so8RYgDd
   * GGah2XsjX/GO1WfoVNpbOms2b/mBsTNHM3dA+VKq3dSDz4V4mZqTuXNnQkYRIer+
   * CqkbGmVps4+uFrb2S1ayLfmlyOw7YqPta9BO1UAJpB+Y1zqlklkg5LB9zVtzaL1t
   * xKITDmcZuI1CfmwMmm6gJC3VRRvcxAIU/oVbZZfKTpBQCHpCNfnqwmbU+AGuHrS+
   * w6jv/naaoqYfRvaE7fzbzsQCzndILIyy7MMAo+wsVRjBfhnu4S/yrYObnqsZ38aK
   * L4x35bcF7DvB7L6Gs4a8wPfc5+pbrrLMtTWGS9DiP7bY+A4A7l3j941Y/8+LN+lj
   * X273CXE2whJdV/LItM3z7gLfEdxquVeEHVlNjM7IDiPCtyaaEBRx/pOyiriA8A4Q
   * ntOoUAw3gi/q4Iqd4Sw5/7W0cwDk90imc6y/st53BIe0o82bNSQ3+pCTE4FCxpgm
   * dTdmQRCsu/WU48IxK63nI1bMNSWSs1A=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1\'0%\x06\x03U\x04\n\x0c\x1eDeutsche Telekom Security GmbH1+0)\x06\x03U\x04\x03\x0c\"Telekom Security TLS RSA Root 2023"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xed5\xa1\x81\x80\xf3\xcbJi[\xc2\xfbQ\x83\xae&\xfd\xe1n\xf3\x81\x12}q@\xff\x87uB)!\xed\x81R,\xdf\x12\xc1\x19\x84\x89\xc1\xbd\xc5(\xd5\xd5KlD\xd6L\xdb\x07\x96JUz\xca6\x82\x046\xa8\xa5\xfc\'\xf6I\xf1\xd5r\x9e\x91\xf9#\xd6p{\xbb\xf5\x9b\xc1\xec\x93\xcf\x19\xeae~\x88p\xa0s\xfc\xf6\xff\xb5Vb\xe1sj4\x98>\x82\xb8\xac\x95S\xf4\x01\xa0\'\x07r\xa3\x00S\xa0\xe4\xb2\xab\x838W3%\x94\x9f\xbeH\x1d\x98\xe1\xa3\xba\x9e\\\xcd\x04qQ}ux\xab\xf3Y\xaa\xc4\xe0`\xbe\x8f\x83R\xb8u\x1aA5\xed\xbc\xf3:c\xe9\xa9\x14E\xd7\xe6R\xd1n\xd2\xde\xbc\xe3\xf5\x0b;\xe6\xe0\xc4\xbdCd\x13\xa6\xce\xf4\x987l\x8a\x95\xa8\x97\xc8G\x0f\xf0^\x10\x8b\xe7\x1d\x1c\xfe\xb1;\xa0\x053h\x05A\x82\xc1\x03+\x01\xc8\xe7\x8fM\xab\xe8\xb5\xf6\xcdkD\xb5\xe7\xdd\x8b\xec\xea%\xb4\x00\"WM\xb0\xb1\xb21\xc1\x16\xce\xff\xfd\x14\x84\xb7G\xfa\xb2\xf1p\xde\xdb\x8bl6X\xa4|\xb3\x11\xd1\xc3w\x7f_\xb6%\xe0\r\xc5\xd2\xb3\xf9\xb8\xb8w\xdb7qqG\xe3`\x18O$\xb6u7x\xb9\xa3b\xaf\xbd\xc9r\x8e/\xcc\xbb\xae\xdb\xe4\x15R\x19\x073\xfbj\xb7-K\x90(\x82s\xfe\x18\x8b5\x8d\xdb\xa7\x04j\xbe\xea\xc1M6;\x166\x912\xef\xb6@\x89\x91C\xe0\xf2\xa2\xab\x04.\xe6\xf2L\x0e\x164 \xac\x87\xc1-~\xc9fG\x17\x14\x11\xa4\xf3\xf7\xa1$\x89\xab\xd8\x1a\xc8\xa1\\\xb1\xa3\xf7\x8cm\xc8\x01\xc9O\xc9\xec\xc4\xfc\xacQ3\xd1\xc8\x83\xd1\xc9\x9f\x1d\xd4G4)>\xcb\xb0\x0e\xfa\x83\x0b(X\xe5)\xdc?|\xa8\x9f\xc9\xb6\n\xbb\xa6\xe8F\x16\x0f\x96\xe5{\xe4jzHmv\x98\x05\xa5\xdcm\x1eB\x1eB\xda\x1a\xe0R\xf7\xb5\x83\xc0\x1a{x5,8\xf5\x1f\xfdI\xa3.\xd2Yc\xbf\x80\xb0\x8c\x93s\xcb5\xa6\x99\x95\"ae\x03`\xfb/\x93K\xfa\x9a\x9c\x80;\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=HiPKI Root CA - G1 O=Chunghwa Telecom Co., Ltd.
   * Subject: CN=HiPKI Root CA - G1 O=Chunghwa Telecom Co., Ltd.
   * Label: "HiPKI Root CA - G1"
   * Serial: 60966262342023497858655262305426234976
   * SHA256 Fingerprint: f0:15:ce:3c:c2:39:bf:ef:06:4b:e9:f1:d2:c4:17:e1:a0:26:4a:0a:94:be:1f:0c:8d:12:18:64:eb:69:49:cc
   * -----BEGIN CERTIFICATE-----
   * MIIFajCCA1KgAwIBAgIQLd2szmKXlKFD6LDNdmpeYDANBgkqhkiG9w0BAQsFADBP
   * MQswCQYDVQQGEwJUVzEjMCEGA1UECgwaQ2h1bmdod2EgVGVsZWNvbSBDby4sIEx0
   * ZC4xGzAZBgNVBAMMEkhpUEtJIFJvb3QgQ0EgLSBHMTAeFw0xOTAyMjIwOTQ2MDRa
   * Fw0zNzEyMzExNTU5NTlaME8xCzAJBgNVBAYTAlRXMSMwIQYDVQQKDBpDaHVuZ2h3
   * YSBUZWxlY29tIENvLiwgTHRkLjEbMBkGA1UEAwwSSGlQS0kgUm9vdCBDQSAtIEcx
   * MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEA9B5/UnMyDHPkvRN0o9Qw
   * qNCuS9i233VHZvR85zkEHmpwINJaR3JnVfSl6J3VHiGh8Ge6zCFovkRTv4354twv
   * Vcg3Px+kwJyz5HdcoEb+d/oaoDjq7Zpy3iu9lFc6uux55199QmQ5eiY29yTw1S+6
   * lZgRZq2XNdZ1AYDgr/SEYYwNHl98h5ZeQa/rh+r4XfEuiAU+TCK72h8q3VJGZDnz
   * Qs7ZngyzsHeXZJzA9KMuH5UHsBffMNsAGJZMoYFL3QRtU6M9/Aes1MU3guvklQgZ
   * KILSQjqj2FPseYlgSGDIcpJQ3AOPgz+yQlda22rpEZfdhSi8MEyr48KxRURHH+CK
   * FgeW0iEPU8DtqX7UTuybCeyvQqww1r/REEXgphaypcXTT3OUM3ECoWqj1jOXTyFj
   * HluP2cFeRXF3D4FdXyGarYPM+l7WjSNfGz1BryB1ZlpK9p/7qxj3ccC2HTHsOyDr
   * y+K49a6SsvfhhEvyovKTmiKe0xRvNlS9H15ZFblzqMF8b3ti6RZsR1pl8w4Rm0bZ
   * /W3c1pzAtH2lsN0/Vm+h+fbkEkj9Bn8SV7apI09bA8PgcSojt/ewsTu8mL3WmKgM
   * a/aOEmem8rJY5AIJEzypuxC00jBF8ez3ABHfZfjcK0NVvxaXxA/VLGGEqnKG/uY6
   * fsI/fe78LxQ+5oXdUG+3Se0CAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAdBgNV
   * HQ4EFgQU8ncX+l6o/vY9cdVouslGDDjYr7AwDgYDVR0PAQH/BAQDAgGGMA0GCSqG
   * SIb3DQEBCwUAA4ICAQBQUfB13HAE4/+qddRxosuej6ip0691x1TPOhwEmSKsxBHi
   * 7zNKpiMdDg1H2DfHb680f0+BazVP6XKlMeJ45/dOlBhbQH3PayFUhuaVevvGyuqc
   * SE5XCV0vrPSltJczWNWseanMX/mF+lLFjfiRFOs6DRfQUsJ748JzjkZ4Bjgs6Fza
   * ZsT0pPBWGTMpWmWSBUdGSquEwx4noR8RkpkndZMPvDY7l1ePJlsMu5wP1G4wB9Tc
   * XzZoZjmDlicmisjEOf6aIW/Vcobpf2Lll07QJNBAsNB1CI69aO4I1258EHBGG3zg
   * iLKecoaZAeO/n0kZtCW+VmWuF2PlHt/o/0elv+EmBYTksMCv5wiZqAxeJoBF1Pho
   * L5aPruJKHJwWDBNvOIf2u8g0X5IDUXlwpt/L9ZlNec1OvFefQ05rLisY+GpzjLrF
   * Ne85akEez3GoorKGB1s6yeHvP2UEgEcyRHCVTjFnanRbEEV16rCf0OY1/k6fi8wr
   * kkVbbiVghUbN0aqwdmaTd5a+g744tiROJgvM7XpWGuDpWsZkrUx6AEhEL7lAuxM+
   * vhV4nYWBSipX3tUZQ9rbyltHhoMLP7YNdnhzeSJesYAfz77RP1YQmCuVh6EfnWQU
   * YDksswBVLuT1sw5XxJFBAJw/6KXf6vb/yPCtbVKoF6ubYfwSUTXkJf2vqmqGOQ==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02TW1#0!\x06\x03U\x04\n\x0c\x1aChunghwa Telecom Co., Ltd.1\x1b0\x19\x06\x03U\x04\x03\x0c\x12HiPKI Root CA - G1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xf4\x1e\x7fRs2\x0cs\xe4\xbd\x13t\xa3\xd40\xa8\xd0\xaeK\xd8\xb6\xdfuGf\xf4|\xe79\x04\x1ejp \xd2ZGrgU\xf4\xa5\xe8\x9d\xd5\x1e!\xa1\xf0g\xba\xcc!h\xbeDS\xbf\x8d\xf9\xe2\xdc/U\xc87?\x1f\xa4\xc0\x9c\xb3\xe4w\\\xa0F\xfew\xfa\x1a\xa08\xea\xed\x9ar\xde+\xbd\x94W:\xba\xecy\xe7_}Bd9z&6\xf7$\xf0\xd5/\xba\x95\x98\x11f\xad\x975\xd6u\x01\x80\xe0\xaf\xf4\x84a\x8c\r\x1e_|\x87\x96^A\xaf\xeb\x87\xea\xf8]\xf1.\x88\x05>L\"\xbb\xda\x1f*\xddRFd9\xf3B\xce\xd9\x9e\x0c\xb3\xb0w\x97d\x9c\xc0\xf4\xa3.\x1f\x95\x07\xb0\x17\xdf0\xdb\x00\x18\x96L\xa1\x81K\xdd\x04mS\xa3=\xfc\x07\xac\xd4\xc57\x82\xeb\xe4\x95\x08\x19(\x82\xd2B:\xa3\xd8S\xecy\x89`H`\xc8r\x92P\xdc\x03\x8f\x83?\xb2BWZ\xdbj\xe9\x11\x97\xdd\x85(\xbc0L\xab\xe3\xc2\xb1EDG\x1f\xe0\x8a\x16\x07\x96\xd2!\x0fS\xc0\xed\xa9~\xd4N\xec\x9b\t\xec\xafB\xac0\xd6\xbf\xd1\x10E\xe0\xa6\x16\xb2\xa5\xc5\xd3Os\x943q\x02\xa1j\xa3\xd63\x97O!c\x1e[\x8f\xd9\xc1^Eqw\x0f\x81]_!\x9a\xad\x83\xcc\xfa^\xd6\x8d#_\x1b=A\xaf ufZJ\xf6\x9f\xfb\xab\x18\xf7q\xc0\xb6\x1d1\xec; \xeb\xcb\xe2\xb8\xf5\xae\x92\xb2\xf7\xe1\x84K\xf2\xa2\xf2\x93\x9a\"\x9e\xd3\x14o6T\xbd\x1f^Y\x15\xb9s\xa8\xc1|o{b\xe9\x16lGZe\xf3\x0e\x11\x9bF\xd9\xfdm\xdc\xd6\x9c\xc0\xb4}\xa5\xb0\xdd?Vo\xa1\xf9\xf6\xe4\x12H\xfd\x06\x7f\x12W\xb6\xa9#O[\x03\xc3\xe0q*#\xb7\xf7\xb0\xb1;\xbc\x98\xbd\xd6\x98\xa8\x0ck\xf6\x8e\x12g\xa6\xf2\xb2X\xe4\x02\t\x13<\xa9\xbb\x10\xb4\xd20E\xf1\xec\xf7\x00\x11\xdfe\xf8\xdc+CU\xbf\x16\x97\xc4\x0f\xd5,a\x84\xaar\x86\xfe\xe6:~\xc2?}\xee\xfc/\x14>\xe6\x85\xddPo\xb7I\xed\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=SecureTrust CA O=SecureTrust Corporation
   * Subject: CN=SecureTrust CA O=SecureTrust Corporation
   * Label: "SecureTrust CA"
   * Serial: 17199774589125277788362757014266862032
   * SHA256 Fingerprint: f1:c1:b5:0a:e5:a2:0d:d8:03:0e:c9:f6:bc:24:82:3d:d3:67:b5:25:57:59:b4:e7:1b:61:fc:e9:f7:37:5d:73
   * -----BEGIN CERTIFICATE-----
   * MIIDuDCCAqCgAwIBAgIQDPCOXAgWpa1Cf/DrJxhZ0DANBgkqhkiG9w0BAQUFADBI
   * MQswCQYDVQQGEwJVUzEgMB4GA1UEChMXU2VjdXJlVHJ1c3QgQ29ycG9yYXRpb24x
   * FzAVBgNVBAMTDlNlY3VyZVRydXN0IENBMB4XDTA2MTEwNzE5MzExOFoXDTI5MTIz
   * MTE5NDA1NVowSDELMAkGA1UEBhMCVVMxIDAeBgNVBAoTF1NlY3VyZVRydXN0IENv
   * cnBvcmF0aW9uMRcwFQYDVQQDEw5TZWN1cmVUcnVzdCBDQTCCASIwDQYJKoZIhvcN
   * AQEBBQADggEPADCCAQoCggEBAKukgeWVzfX2FI7CT8rU4niVWJxB4Q2ZQCQXOZEz
   * Zum+4YOvYlyJ0fwkW2Gz4BERQRwdbvC4u/jep4G6pkjGnx29vo6pQT64lO0pGtSO
   * 0gMdA+9tDWccV9cGrcrI9f4Or2YlSASWC12juhbDCE/RRvgUXPLIXgGZbf2IzIao
   * wW8xQmxSPmjL8xk037uHGFaAJsTQ3MBv396gwpEWoGQRS0S8Hvbn+mPeZqx2pHGj
   * 7DaUaHp3pLHnDi+BeuK1cobvomuL8A/b01k/unK8RCSc43Oz969XL0Imnal0ugBS
   * 8kvNU3xHCzaFDmapCJcWNFfBZveA4+1wVMeT4C4oFVmHursCAwEAAaOBnTCBmjAT
   * BgkrBgEEAYI3FAIEBh4EAEMAQTALBgNVHQ8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB
   * /zAdBgNVHQ4EFgQUQjK2FvoE/f5dS3rD/fdMQB1aQ68wNAYDVR0fBC0wKzApoCeg
   * JYYjaHR0cDovL2NybC5zZWN1cmV0cnVzdC5jb20vU1RDQS5jcmwwEAYJKwYBBAGC
   * NxUBBAMCAQAwDQYJKoZIhvcNAQEFBQADggEBADDtT0rhWDpSclu1pqNlGKa7UTt3
   * 6Z3q059c4EVlew3KW+JwULKUBRSuSceNQQcSc5R+DCMh/bwQf2AQWnL1mA6s7Ll/
   * 3XpvXdMc9P+IBWlCqQVxyLesJugutIxq/3HcuLHfmbx8IVQr5Fiiu1cprp6poxkm
   * D5kuCLDv/WnPmRoJjeOnnyvJNjR7JLN4TJUXpAYmHrZkUjZfYGfZnMUFdAvnZyPS
   * CPyI6a6Lf+Ew9Dd+/cYy2i2eRDAwbO4H3tI0/NL/QPZL9GZGBlSm8jIKYyYwa5vR
   * 3ItHuuG51WLQoqD0ZwV4KWMabwTW+MZMo5qxN7SN5ShLHZ4swrhovO0C7jE=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1 0\x1e\x06\x03U\x04\n\x13\x17SecureTrust Corporation1\x170\x15\x06\x03U\x04\x03\x13\x0eSecureTrust CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xab\xa4\x81\xe5\x95\xcd\xf5\xf6\x14\x8e\xc2O\xca\xd4\xe2x\x95X\x9cA\xe1\r\x99@$\x179\x913f\xe9\xbe\xe1\x83\xafb\\\x89\xd1\xfc$[a\xb3\xe0\x11\x11A\x1c\x1dn\xf0\xb8\xbb\xf8\xde\xa7\x81\xba\xa6H\xc6\x9f\x1d\xbd\xbe\x8e\xa9A>\xb8\x94\xed)\x1a\xd4\x8e\xd2\x03\x1d\x03\xefm\rg\x1cW\xd7\x06\xad\xca\xc8\xf5\xfe\x0e\xaff%H\x04\x96\x0b]\xa3\xba\x16\xc3\x08O\xd1F\xf8\x14\\\xf2\xc8^\x01\x99m\xfd\x88\xcc\x86\xa8\xc1o1BlR>h\xcb\xf3\x194\xdf\xbb\x87\x18V\x80&\xc4\xd0\xdc\xc0o\xdf\xde\xa0\xc2\x91\x16\xa0d\x11KD\xbc\x1e\xf6\xe7\xfac\xdef\xacv\xa4q\xa3\xec6\x94hzw\xa4\xb1\xe7\x0e/\x81z\xe2\xb5r\x86\xef\xa2k\x8b\xf0\x0f\xdb\xd3Y?\xbar\xbcD$\x9c\xe3s\xb3\xf7\xafW/B&\x9d\xa9t\xba\x00R\xf2K\xcdS|G\x0b6\x85\x0ef\xa9\x08\x97\x164W\xc1f\xf7\x80\xe3\xedpT\xc7\x93\xe0.(\x15Y\x87\xba\xbb\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Atos TrustedRoot 2011 O=Atos
   * Subject: CN=Atos TrustedRoot 2011 O=Atos
   * Label: "Atos TrustedRoot 2011"
   * Serial: 6643877497813316402
   * SHA256 Fingerprint: f3:56:be:a2:44:b7:a9:1e:b3:5d:53:ca:9a:d7:86:4a:ce:01:8e:2d:35:d5:f8:f9:6d:df:68:a6:f4:1a:a4:74
   * -----BEGIN CERTIFICATE-----
   * MIIDdzCCAl+gAwIBAgIIXDPLYixfszIwDQYJKoZIhvcNAQELBQAwPDEeMBwGA1UE
   * AwwVQXRvcyBUcnVzdGVkUm9vdCAyMDExMQ0wCwYDVQQKDARBdG9zMQswCQYDVQQG
   * EwJERTAeFw0xMTA3MDcxNDU4MzBaFw0zMDEyMzEyMzU5NTlaMDwxHjAcBgNVBAMM
   * FUF0b3MgVHJ1c3RlZFJvb3QgMjAxMTENMAsGA1UECgwEQXRvczELMAkGA1UEBhMC
   * REUwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCVhTuXbyo7LjvPpvMp
   * Nb7PGKw+qtn4TaA+Gke5vJrf8v7MPkfoepbCJI419KkM/IL9bcFyYie96mvr54rM
   * VD6QUM+A1JX76LWC1BTFtqlVJVfbsVD2sGBkWXppzwO3bw2+yj5vdHLqqjAqc2K+
   * SZFhyBH+DgMq92og3AIVDV4VavzjgsG1xZ1kCWyjWZgHJ8cblithdHFsQ/H3NYkQ
   * 4J7sVaE3IqKHBAUsR320HLliKWYoyrfhk/WklAOZuXCFteZI6o1Q/NnezG8HDt0L
   * cp2AMBYHlT8oDv3FdU9T1nSatCQujgKRz3bFmx5VdJx4IbHwLfELn8LVlhgf8FQi
   * eowHAgMBAAGjfTB7MB0GA1UdDgQWBBSnpQaxLKYJYO7Rl+lwrrw7GWzbITAPBgNV
   * HRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFKelBrEspglg7tGX6XCuvDsZbNshMBgG
   * A1UdIAQRMA8wDQYLKwYBBAGwLQMEAQEwDgYDVR0PAQH/BAQDAgGGMA0GCSqGSIb3
   * DQEBCwUAA4IBAQAmdzTblEiGKkGdLD4GkGDEjKwLVLgfuXvTBznk+j57sj1O7Z8j
   * vZfza1zv7v1Apt+hk6EKhqzvINB5Ab149xnYJDE0BAGmuhWawyfc2E8PzBhj/5kP
   * DpFrdRbhIfzYJsdHt6bPWHJxfrrhTZVHO8mvbaG0weyJ9rQPOLXiZNwlz6bb65pc
   * maHFCN795trV1lpFDMS3wrUU77QR/w4VtfX128a961qn8FYiqTxlVMYVqL2Gns2D
   * lmh6cYGJ4Qvh6hEbaAjMaZ7snkGeRDImeuKHCnE96+RapNLbxc3G3mB/ufNPRJLv
   * KrcYPqcZ2Qt9sTdBQrC6YB3y/gkRsPCHe6ed
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x1e0\x1c\x06\x03U\x04\x03\x0c\x15Atos TrustedRoot 20111\r0\x0b\x06\x03U\x04\n\x0c\x04Atos1\x0b0\t\x06\x03U\x04\x06\x13\x02DE"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\x95\x85;\x97o*;.;\xcf\xa6\xf3)5\xbe\xcf\x18\xac>\xaa\xd9\xf8M\xa0>\x1aG\xb9\xbc\x9a\xdf\xf2\xfe\xcc>G\xe8z\x96\xc2$\x8e5\xf4\xa9\x0c\xfc\x82\xfdm\xc1rb\'\xbd\xeak\xeb\xe7\x8a\xccT>\x90P\xcf\x80\xd4\x95\xfb\xe8\xb5\x82\xd4\x14\xc5\xb6\xa9U%W\xdb\xb1P\xf6\xb0`dYzi\xcf\x03\xb7o\r\xbe\xca>otr\xea\xaa0*sb\xbeI\x91a\xc8\x11\xfe\x0e\x03*\xf7j \xdc\x02\x15\r^\x15j\xfc\xe3\x82\xc1\xb5\xc5\x9dd\tl\xa3Y\x98\x07\'\xc7\x1b\x96+atqlC\xf1\xf75\x89\x10\xe0\x9e\xecU\xa17\"\xa2\x87\x04\x05,G}\xb4\x1c\xb9b)f(\xca\xb7\xe1\x93\xf5\xa4\x94\x03\x99\xb9p\x85\xb5\xe6H\xea\x8dP\xfc\xd9\xde\xcco\x07\x0e\xdd\x0br\x9d\x800\x16\x07\x95?(\x0e\xfd\xc5uOS\xd6t\x9a\xb4$.\x8e\x02\x91\xcfv\xc5\x9b\x1eUt\x9cx!\xb1\xf0-\xf1\x0b\x9f\xc2\xd5\x96\x18\x1f\xf0T\"z\x8c\x07\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=BJCA Global Root CA1 O=BEIJING CERTIFICATE AUTHORITY
   * Subject: CN=BJCA Global Root CA1 O=BEIJING CERTIFICATE AUTHORITY
   * Label: "BJCA Global Root CA1"
   * Serial: 113562791157148395269083148143378328608
   * SHA256 Fingerprint: f3:89:6f:88:fe:7c:0a:88:27:66:a7:fa:6a:d2:74:9f:b5:7a:7f:3e:98:fb:76:9c:1f:a7:b0:9c:2c:44:d5:ae
   * -----BEGIN CERTIFICATE-----
   * MIIFdDCCA1ygAwIBAgIQVW9l47TZkGobCdFsPsBsIDANBgkqhkiG9w0BAQsFADBU
   * MQswCQYDVQQGEwJDTjEmMCQGA1UECgwdQkVJSklORyBDRVJUSUZJQ0FURSBBVVRI
   * T1JJVFkxHTAbBgNVBAMMFEJKQ0EgR2xvYmFsIFJvb3QgQ0ExMB4XDTE5MTIxOTAz
   * MTYxN1oXDTQ0MTIxMjAzMTYxN1owVDELMAkGA1UEBhMCQ04xJjAkBgNVBAoMHUJF
   * SUpJTkcgQ0VSVElGSUNBVEUgQVVUSE9SSVRZMR0wGwYDVQQDDBRCSkNBIEdsb2Jh
   * bCBSb290IENBMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAPFmCL3Z
   * xRVhy4QEQaVpN3cdwbB7+sN3SJATcmTRuHyQNZ0YeYjjlwE8R4HyDqKYDZ4/N+AZ
   * spDyRhySsTphzvq3Rp4Dhtczbu33RYx2N95ulpH3134rhxfVizXuhJFyV9xgw8O5
   * 58dnJCNPYwpj9mZ9S1WnP3hkSWkSl+BMDdMJoDIwOvqfwPKcxRIqLhy1BDPapDgR
   * at7GGPZHOiJBhyL8xIkoVNiMpTAK+BcWyqw3/XmnkRd4OJmtWO2y3syJfQOcs4ll
   * 5+M7sSKGjwZteAf9kRJ/sGsciQ35uMt0WwfCyPQ10WRjeulumijWML3mG90Vr4Tq
   * nMfK9Q7q8l0ph49pczm+LiRvRSGsxdRpJQaDrXpIhRMsDQa4bHlW/KNnMoH1V6XK
   * V0Jp6VwkYe/iMBhORJhVb3rCk9gZtt58R4oRTklH2yiUAguUSiz5EtBP6DF+bHq/
   * pj+bOT0CFqMYs2esWz8sgytnOYFcuX6U1WTdno9uruh8W7TXakdI136z1C2OVnZO
   * z2nxbkRs1CTqjSShGL+9V/6pmTW12xB3uD1IutbB5/EjPtffhZ0nPNRAvQoMvfXn
   * jSXWgXSHRtQpdaJCbPdzied9v3pKH9MiyRVVz99vfFXQpIsHETdfg6YmV6YBW37+
   * WGgHqel62bno/1Afq8K0wM7o6v0PvY1NuLxxAgMBAAGjQjBAMB0GA1UdDgQWBBTF
   * 7+3M2I0hxkjk49cULqcWk+WYATAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQE
   * AwIBBjANBgkqhkiG9w0BAQsFAAOCAgEAUoKsITQfI/Ki2Pm4rzc2IInRNwPWaZ+4
   * YRC6ojGYWUfo0Q0lHhVBDOAqVdVXUsv45Mdpox1NcQJeXyFFYEhcCY5JEMEE3Kli
   * awLwQ8hOnThJdMkycFRtwUf8jrQ2ntScvd0g1lPJGKm1Vrl2i5VnZu69mP6u775u
   * +2D2/VnGKhs/I0qUJDAnyIm860Qkmss9vk/Ves6OF8tiwdneHg56/0OGNFK8YT88
   * X7vZdrRTvJez/opMEi4r89fO4aL/3Xtw+zuhTaRjAv04l5U/BXCga99igUOLtFkN
   * SoxUnMW7gZ/NfaXvCyUeOiDbHPwfmGcCCtRzRBPbUYQaVQNW4AB+dAb/OMRyHdOo
   * P2gxXdMJxy6MW2Pg6Nwe0uxhHvLe5e/2mXZgLR6UcnHGCyoyx5JO1UbXHfmpGQrI
   * +pXObSOYqgs4rZpWDW+N8TEAiMEXnM0ZNjX+VVOg4DwzX5Ze4jLp3zO7Bkqp2IRz
   * znfSxqxx4VyjHQy7Ct9f4qNx2No3WqB4K/TUfet27fJhcKVlmtOJNBir+3I+17Q9
   * eVzYH6Eze9mCUAyTF6ps3MKCuwJXNq+YJyo5UOGwifUll35HaBC07HPKs5fRJNz2
   * YqAo07WjuGS3iGJCz51TzZm+ZGiPTx4SSPfSKcOYKMryMguTjClPPGAyzQWWYezy
   * r/6zcCwupvI=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02CN1&0$\x06\x03U\x04\n\x0c\x1dBEIJING CERTIFICATE AUTHORITY1\x1d0\x1b\x06\x03U\x04\x03\x0c\x14BJCA Global Root CA1"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xf1f\x08\xbd\xd9\xc5\x15a\xcb\x84\x04A\xa5i7w\x1d\xc1\xb0{\xfa\xc3wH\x90\x13rd\xd1\xb8|\x905\x9d\x18y\x88\xe3\x97\x01<G\x81\xf2\x0e\xa2\x98\r\x9e?7\xe0\x19\xb2\x90\xf2F\x1c\x92\xb1:a\xce\xfa\xb7F\x9e\x03\x86\xd73n\xed\xf7E\x8cv7\xden\x96\x91\xf7\xd7~+\x87\x17\xd5\x8b5\xee\x84\x91rW\xdc`\xc3\xc3\xb9\xe7\xc7g$#Oc\nc\xf6f}KU\xa7?xdIi\x12\x97\xe0L\r\xd3\t\xa020:\xfa\x9f\xc0\xf2\x9c\xc5\x12*.\x1c\xb5\x043\xda\xa48\x11j\xde\xc6\x18\xf6G:\"A\x87\"\xfc\xc4\x89(T\xd8\x8c\xa50\n\xf8\x17\x16\xca\xac7\xfdy\xa7\x91\x17x8\x99\xadX\xed\xb2\xde\xcc\x89}\x03\x9c\xb3\x89e\xe7\xe3;\xb1\"\x86\x8f\x06mx\x07\xfd\x91\x12\x7f\xb0k\x1c\x89\r\xf9\xb8\xcbt[\x07\xc2\xc8\xf45\xd1dcz\xe9n\x9a(\xd60\xbd\xe6\x1b\xdd\x15\xaf\x84\xea\x9c\xc7\xca\xf5\x0e\xea\xf2])\x87\x8fis9\xbe.$oE!\xac\xc5\xd4i%\x06\x83\xadzH\x85\x13,\r\x06\xb8lyV\xfc\xa3g2\x81\xf5W\xa5\xcaWBi\xe9\\$a\xef\xe20\x18ND\x98Uoz\xc2\x93\xd8\x19\xb6\xde|G\x8a\x11NIG\xdb(\x94\x02\x0b\x94J,\xf9\x12\xd0O\xe81~lz\xbf\xa6?\x9b9=\x02\x16\xa3\x18\xb3g\xac[?,\x83+g9\x81\\\xb9~\x94\xd5d\xdd\x9e\x8fn\xae\xe8|[\xb4\xd7jGH\xd7~\xb3\xd4-\x8eVvN\xcfi\xf1nDl\xd4$\xea\x8d$\xa1\x18\xbf\xbdW\xfe\xa9\x995\xb5\xdb\x10w\xb8=H\xba\xd6\xc1\xe7\xf1#>\xd7\xdf\x85\x9d\'<\xd4@\xbd\n\x0c\xbd\xf5\xe7\x8d%\xd6\x81t\x87F\xd4)u\xa2Bl\xf7s\x89\xe7}\xbfzJ\x1f\xd3\"\xc9\x15U\xcf\xdfo|U\xd0\xa4\x8b\x07\x117_\x83\xa6&W\xa6\x01[~\xfeXh\x07\xa9\xe9z\xd9\xb9\xe8\xffP\x1f\xab\xc2\xb4\xc0\xce\xe8\xea\xfd\x0f\xbd\x8dM\xb8\xbcq\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=ANF Secure Server Root CA O=ANF Autoridad de Certificacion OU=ANF CA Raiz
   * Subject: CN=ANF Secure Server Root CA O=ANF Autoridad de Certificacion OU=ANF CA Raiz
   * Label: "ANF Secure Server Root CA"
   * Serial: 996390341000653745
   * SHA256 Fingerprint: fb:8f:ec:75:91:69:b9:10:6b:1e:51:16:44:c6:18:c5:13:04:37:3f:6c:06:43:08:8d:8b:ef:fd:1b:99:75:99
   * -----BEGIN CERTIFICATE-----
   * MIIF7zCCA9egAwIBAgIIDdPjvGz5a7EwDQYJKoZIhvcNAQELBQAwgYQxEjAQBgNV
   * BAUTCUc2MzI4NzUxMDELMAkGA1UEBhMCRVMxJzAlBgNVBAoTHkFORiBBdXRvcmlk
   * YWQgZGUgQ2VydGlmaWNhY2lvbjEUMBIGA1UECxMLQU5GIENBIFJhaXoxIjAgBgNV
   * BAMTGUFORiBTZWN1cmUgU2VydmVyIFJvb3QgQ0EwHhcNMTkwOTA0MTAwMDM4WhcN
   * MzkwODMwMTAwMDM4WjCBhDESMBAGA1UEBRMJRzYzMjg3NTEwMQswCQYDVQQGEwJF
   * UzEnMCUGA1UEChMeQU5GIEF1dG9yaWRhZCBkZSBDZXJ0aWZpY2FjaW9uMRQwEgYD
   * VQQLEwtBTkYgQ0EgUmFpejEiMCAGA1UEAxMZQU5GIFNlY3VyZSBTZXJ2ZXIgUm9v
   * dCBDQTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBANvrayvmZFSVgpCj
   * cqQZAZ2cC4Ffc0m6p6zzBE57lgvsEeBbphzOG9INgxwruJ4dfkUyYA8H6XdYfp9q
   * yGFOtibBTI3/TO80sh9l2Ll49a2pcbnvT1gdpd50IJeh7WhM3pIXS7yr/2WanvtH
   * 2Vdy8wmhrnZEE26cLUQ5vPnHO6RYPUG9tMJJo8gN0pcvB2VSAKduyK9o7PQUlrZX
   * H1bDOZ8rbeTzPvY1ZNoMHKGESy9LS+IsJJ1tk0DrtSOOMspvRdOoiXsezx76W0OL
   * zc2oD2rKDF65nkeP8Nm2CgtYZRczuSPkdxl9y0oukntPLxB3sY0vaJxizOBQ+OyR
   * p1RMVwnVdmPF6GUe7m1qzwmd+nxPrWAI/VaZDxUse6mAq4xhj0oHdkLePfTdsiQz
   * W7i1o0TJrH93PB0j7IKppuLIBkwC/qxcmZkLLxCKpvR/1Yd0DVlJRfbwcVw5Kda/
   * SiOL9V8BY9KHcyi1Swr1+KuCLH5zJTIdC2MKF4EA/7Z2Xue0sUDKIbvVgFHlSFJn
   * LNJhiQcND85Cd8BEc5xEUKDbEAotlRyBr+Qc5RQe8TZBAQIvfXOn3kLMTOmJDVb3
   * n5HUA8ZsyY/b2BzgQJhdZpmYgG4t/wHFzstGH6wCxkPmrqKEPMVOHj1tyRRM4y5B
   * u8o5vzY8KhmqQYdOpc5LMnndkEl/AgMBAAGjYzBhMB8GA1UdIwQYMBaAFJxf0Gxj
   * o1+TypOYCK2Mh6UsXME3MB0GA1UdDgQWBBScX9BsY6Nfk8qTmAitjIelLFzBNzAO
   * BgNVHQ8BAf8EBAMCAYYwDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0BAQsFAAOC
   * AgEATh65isagmD9uw2nAalxJUqzLK114OMHVVISfk/CHGT0sZonrDUL8zPB1hT+L
   * 9IBdeeUXZ701guLyPI59WzbLWoAAKfLOKyzxj6ptBZNscsdW699QIyjlRRA96Gej
   * rw5VD5AJYu9LWaL2U/HANeQvwSS9eS9OICI7/RogsKQOLHDtdD+4E5UGUcjohybK
   * pFtqFiGS3XNgnhAY3jyB6ugYw3yJ8otQPr0R4hUDqDZ9MwFsSBXXiJCZBMXM5gf0
   * vPSQ7RPi6ovDj6MzD8EpTBNO2hVWcXNyglD2mjN8orGoGjR0ZVzO0eurU+AagNjq
   * OknkJjCb5RyKqKkVMoaZkgoQI1YS4PbOTOK7vtuNknMBZi9iPrJyJ0U27U1W45eZ
   * /zo1PqVUSlJZS2Db7v54EX9K3BR5YLZrZAPbFYPhor72I5dQ8AkzNqdxliXzuUJ9
   * 2zg/LFis6ELhDtjTO0wugumDLmsx2d1Hhk9tl5EuT+IocTUW0fJz/iUrB0ckYyfI
   * +PbZa/wSMVYIwFNCr5zQM378BvAxRAMU8Vjq8moNqRGyg77FGr8H6lnco4g175x2
   * MjxNBiLOFeXdntiP2t7SxDnlF4HPOEfrf4htWRvfn0IUrn7PqLBmZdo3r5+qPeoo
   * tt7VMVgWglvquxl1AnMaykgaIZOQCo6ThKd9OyMYkomgjaw=
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x120\x10\x06\x03U\x04\x05\x13\tG632875101\x0b0\t\x06\x03U\x04\x06\x13\x02ES1\'0%\x06\x03U\x04\n\x13\x1eANF Autoridad de Certificacion1\x140\x12\x06\x03U\x04\x0b\x13\x0bANF CA Raiz1\"0 \x06\x03U\x04\x03\x13\x19ANF Secure Server Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xdb\xebk+\xe6dT\x95\x82\x90\xa3r\xa4\x19\x01\x9d\x9c\x0b\x81_sI\xba\xa7\xac\xf3\x04N{\x96\x0b\xec\x11\xe0[\xa6\x1c\xce\x1b\xd2\r\x83\x1c+\xb8\x9e\x1d~E2`\x0f\x07\xe9wX~\x9fj\xc8aN\xb6&\xc1L\x8d\xffL\xef4\xb2\x1fe\xd8\xb9x\xf5\xad\xa9q\xb9\xefOX\x1d\xa5\xdet \x97\xa1\xedhL\xde\x92\x17K\xbc\xab\xffe\x9a\x9e\xfbG\xd9Wr\xf3\t\xa1\xaevD\x13n\x9c-D9\xbc\xf9\xc7;\xa4X=A\xbd\xb4\xc2I\xa3\xc8\r\xd2\x97/\x07eR\x00\xa7n\xc8\xafh\xec\xf4\x14\x96\xb6W\x1fV\xc39\x9f+m\xe4\xf3>\xf65d\xda\x0c\x1c\xa1\x84K/KK\xe2,$\x9dm\x93@\xeb\xb5#\x8e2\xcaoE\xd3\xa8\x89{\x1e\xcf\x1e\xfa[C\x8b\xcd\xcd\xa8\x0fj\xca\x0c^\xb9\x9eG\x8f\xf0\xd9\xb6\n\x0bXe\x173\xb9#\xe4w\x19}\xcbJ.\x92{O/\x10w\xb1\x8d/h\x9cb\xcc\xe0P\xf8\xec\x91\xa7TLW\t\xd5vc\xc5\xe8e\x1e\xeemj\xcf\t\x9d\xfa|O\xad`\x08\xfdV\x99\x0f\x15,{\xa9\x80\xab\x8ca\x8fJ\x07vB\xde=\xf4\xdd\xb2$3[\xb8\xb5\xa3D\xc9\xac\x7fw<\x1d#\xec\x82\xa9\xa6\xe2\xc8\x06L\x02\xfe\xac\\\x99\x99\x0b/\x10\x8a\xa6\xf4\x7f\xd5\x87t\rYIE\xf6\xf0q\\9)\xd6\xbfJ#\x8b\xf5_\x01c\xd2\x87s(\xb5K\n\xf5\xf8\xab\x82,~s%2\x1d\x0bc\n\x17\x81\x00\xff\xb6v^\xe7\xb4\xb1@\xca!\xbb\xd5\x80Q\xe5HRg,\xd2a\x89\x07\r\x0f\xceBw\xc0Ds\x9cDP\xa0\xdb\x10\n-\x95\x1c\x81\xaf\xe4\x1c\xe5\x14\x1e\xf16A\x01\x02/}s\xa7\xdeB\xccL\xe9\x89\rV\xf7\x9f\x91\xd4\x03\xc6l\xc9\x8f\xdb\xd8\x1c\xe0@\x98]f\x99\x98\x80n-\xff\x01\xc5\xce\xcbF\x1f\xac\x02\xc6C\xe6\xae\xa2\x84<\xc5N\x1e=m\xc9\x14L\xe3.A\xbb\xca9\xbf6<*\x19\xaaA\x87N\xa5\xceK2y\xdd\x90I\x7f\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=T-TeleSec GlobalRoot Class 3 O=T-Systems Enterprise Services GmbH OU=T-Systems Trust Center
   * Subject: CN=T-TeleSec GlobalRoot Class 3 O=T-Systems Enterprise Services GmbH OU=T-Systems Trust Center
   * Label: "T-TeleSec GlobalRoot Class 3"
   * Serial: 1
   * SHA256 Fingerprint: fd:73:da:d3:1c:64:4f:f1:b4:3b:ef:0c:cd:da:96:71:0b:9c:d9:87:5e:ca:7e:31:70:7a:f3:e9:6d:52:2b:bd
   * -----BEGIN CERTIFICATE-----
   * MIIDwzCCAqugAwIBAgIBATANBgkqhkiG9w0BAQsFADCBgjELMAkGA1UEBhMCREUx
   * KzApBgNVBAoMIlQtU3lzdGVtcyBFbnRlcnByaXNlIFNlcnZpY2VzIEdtYkgxHzAd
   * BgNVBAsMFlQtU3lzdGVtcyBUcnVzdCBDZW50ZXIxJTAjBgNVBAMMHFQtVGVsZVNl
   * YyBHbG9iYWxSb290IENsYXNzIDMwHhcNMDgxMDAxMTAyOTU2WhcNMzMxMDAxMjM1
   * OTU5WjCBgjELMAkGA1UEBhMCREUxKzApBgNVBAoMIlQtU3lzdGVtcyBFbnRlcnBy
   * aXNlIFNlcnZpY2VzIEdtYkgxHzAdBgNVBAsMFlQtU3lzdGVtcyBUcnVzdCBDZW50
   * ZXIxJTAjBgNVBAMMHFQtVGVsZVNlYyBHbG9iYWxSb290IENsYXNzIDMwggEiMA0G
   * CSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC9dZPwYiJvJK7genasfb3ZJNW4t/zN
   * 8ELg63iIVl6bmlQdTQyK9tPPcPRStdiTBONGhnFBSivwKixVA9ZIw+A5OO3yXDw/
   * RLyTPWGrTs0NvvAgJ1gORH8EGoel15YUNpDQSXuhdfsaa3Ox+M6pCSzyU9XDFES4
   * hqX2iys52qMzVNn6chr3IhUciJFrf2blw2qAsCTz34ZFiP0Zf3WHHx+xGwpzJFu5
   * ZeAsVMhg02YXP+HMVDNzkQI6pn97djmiH5a2OK61yJN0HZ65tOVgnS9W0eDrXltM
   * EnAMbEQgqxHY9Bn20pxSN+f6tsIxO0rUFJmtxxr1XV/6B7h8DR/Wgx6zAgMBAAGj
   * QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBS1
   * A/d2O2GCahKqGFPrAyGUv/7OyjANBgkqhkiG9w0BAQsFAAOCAQEAVj3vlNW92nOy
   * WL6ukK2YJ5f+AbGwUgC4TeQbIXQbfsDuXmkqJa9c1h3a0nnJ85cp4IaH3gRZD/FZ
   * 1GSFS5mvJQQeyUapl96Cshtwn5z2r3Ex3XsFpSzTucpH9sry9uetuUg/vBa3wW30
   * 6gmv7PO15wWeph6KU1HWk4HMdJP2udqmJQV0eVp+QD6CSyYRMG7hP0HHRwA11fXT
   * 91Q+gT3aSWqas+8QPebrb9HIIkfLzM8BMZLZGOMivgkeGj5asuRrDFR6fUNOuIml
   * e9eiPZaGzPImNC1qkp2aGtAw4l1OBLBfiyB+d8E9lYLRRpo7PHi4b6HQDWSieB4p
   * TpPDpFQUWw==
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02DE1+0)\x06\x03U\x04\n\x0c\"T-Systems Enterprise Services GmbH1\x1f0\x1d\x06\x03U\x04\x0b\x0c\x16T-Systems Trust Center1%0#\x06\x03U\x04\x03\x0c\x1cT-TeleSec GlobalRoot Class 3"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x000\x82\x01\n\x02\x82\x01\x01\x00\xbdu\x93\xf0b\"o$\xae\xe0zv\xac}\xbd\xd9$\xd5\xb8\xb7\xfc\xcd\xf0B\xe0\xebx\x88V^\x9b\x9aT\x1dM\x0c\x8a\xf6\xd3\xcfp\xf4R\xb5\xd8\x93\x04\xe3F\x86qAJ+\xf0*,U\x03\xd6H\xc3\xe098\xed\xf2\\<?D\xbc\x93=a\xabN\xcd\r\xbe\xf0 \'X\x0eD\x7f\x04\x1a\x87\xa5\xd7\x96\x146\x90\xd0I{\xa1u\xfb\x1aks\xb1\xf8\xce\xa9\t,\xf2S\xd5\xc3\x14D\xb8\x86\xa5\xf6\x8b+9\xda\xa33T\xd9\xfar\x1a\xf7\"\x15\x1c\x88\x91k\x7ff\xe5\xc3j\x80\xb0$\xf3\xdf\x86E\x88\xfd\x19\x7fu\x87\x1f\x1f\xb1\x1b\ns$[\xb9e\xe0,T\xc8`\xd3f\x17?\xe1\xccT3s\x91\x02:\xa6\x7f{v9\xa2\x1f\x96\xb68\xae\xb5\xc8\x93t\x1d\x9e\xb9\xb4\xe5`\x9d/V\xd1\xe0\xeb^[L\x12p\x0clD \xab\x11\xd8\xf4\x19\xf6\xd2\x9cR7\xe7\xfa\xb6\xc21;J\xd4\x14\x99\xad\xc7\x1a\xf5]_\xfa\x07\xb8|\r\x1f\xd6\x83\x1e\xb3\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=Certum Trusted Root CA O=Asseco Data Systems S.A. OU=Certum Certification Authority
   * Subject: CN=Certum Trusted Root CA O=Asseco Data Systems S.A. OU=Certum Certification Authority
   * Label: "Certum Trusted Root CA"
   * Serial: 40870380103424195783807378461123655149
   * SHA256 Fingerprint: fe:76:96:57:38:55:77:3e:37:a9:5e:7a:d4:d9:cc:96:c3:01:57:c1:5d:31:76:5b:a9:b1:57:04:e1:ae:78:fd
   * -----BEGIN CERTIFICATE-----
   * MIIFwDCCA6igAwIBAgIQHr9ZULjJgDdMBvfrVU+17TANBgkqhkiG9w0BAQ0FADB6
   * MQswCQYDVQQGEwJQTDEhMB8GA1UEChMYQXNzZWNvIERhdGEgU3lzdGVtcyBTLkEu
   * MScwJQYDVQQLEx5DZXJ0dW0gQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxHzAdBgNV
   * BAMTFkNlcnR1bSBUcnVzdGVkIFJvb3QgQ0EwHhcNMTgwMzE2MTIxMDEzWhcNNDMw
   * MzE2MTIxMDEzWjB6MQswCQYDVQQGEwJQTDEhMB8GA1UEChMYQXNzZWNvIERhdGEg
   * U3lzdGVtcyBTLkEuMScwJQYDVQQLEx5DZXJ0dW0gQ2VydGlmaWNhdGlvbiBBdXRo
   * b3JpdHkxHzAdBgNVBAMTFkNlcnR1bSBUcnVzdGVkIFJvb3QgQ0EwggIiMA0GCSqG
   * SIb3DQEBAQUAA4ICDwAwggIKAoICAQDRLY67tzbqbTeRn06TpwXkKQMlzhyC93yZ
   * n0EGze2jusDbCSzBfN8pfktlL5On1AFrAygYo9idBcEq2EXxkd7fO9CAAozPOA/q
   * p1x4EaTByIVcJdPTsuclzxFUl6s1wB52HO8AU5853BSlLCIls3Jy/I2z5T4IHhQq
   * NwuIPMqw9MjCoa68wb4pZ1Xi/K1ZXP69VyywkI3C7Te2fJmItdUDmj0VDT06qKhF
   * 8JVOJVkdzZhpu9PMMsmN74H+rX2Ju7pgE8pllWeg8xn2A1bUatMn4qGtg/BKEiJ3
   * HAVz4hlxQsDsdUaakFjgao4rpUYwBI4Zshfjvqm6f1bxJAPXsiEodg42MEx51UGa
   * mqi4NboMOvJEGyCI98Ul1z3G4z5D3Yf+xOr1Uz5MZf87Sst4WmsXXw3Hw09Omiqi
   * 7VdNIuJGmj8PkTQkfVXjjJU30xrwCSss0smNtA0Aq2cpKNgB9RkEth2+dv5yXMSF
   * ytKAQd8FqKPVhJBPC/PgP5sZ0jeJP/J7UhyM9uH3PAeXjA6iWYEMspA90+NZRu0P
   * qafegGtaqge2Gcu8V/OXIXoMsSt0Puvap2ctTMSYnjYJdmZm/Bo/6khUHL4wvYBQ
   * v3y1zgD2DGHZ5yQD4OMBgQ692IU0iL2yNqh7XAjlRICMb/gv1SHKHRzQ+8S1h9E6
   * Tsd2tTVItQIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBSM+xx1
   * vALTn04uSNn5YFSqxLNP+jAOBgNVHQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQENBQAD
   * ggIBAEii1QALLtA/vBzVtVRJHlpr9OTy4EA34MwUe7nJ+jW1dReTagVphZzNTxl4
   * WxmB82M+w85bj/UvXgF2Ez8sALnNllI5SW0ETsXpD4YN4fqzX4IS8TrOZgYkNCvo
   * zMrnadyHncI013nR03e4qllY/p0m+jiGPp2Kh2RX5Rc64vmNueMzeMGQ2Ljdt4NR
   * 5MTMI9UGfOZR0800McD2RrsLrfw9EAUqO0qRJe6M1ISHgCq8CYyqOhNf6DR5UMEQ
   * GfnTKB7U0VEwKbOukGfWHwpjscWpxkIxYxeU72nLL/qMFH3EQxiJ2fAyQOaA4kZf
   * 5ePBAFmo+eggvIksDkc0C+pXwlM2/KfUrzHN/gLldfq5Jwn58/U7yn2fqSLLiMmq
   * 0Uc9NneoWWRrJ8/vJ8HjJLWG965+Mk2weWjROeiQWMODvA8s1pfrzgzhIMfatz7D
   * P78v3DSk+yshzWePS/Tj6tQ/50+6uaWTRRxmHyH6ZF5v4HaUMst19W7l9o/HuKTM
   * qJZ9ZPskWkoDbGs4xugDQ5r3V7mzKWmTOPQD8rv7gmsHINFSH5pkAnuYZttcTVoP
   * 0ISVoDwUQwbKytu4QTbaakRnh6+v40URFWkIsr4WOZckbxJF0WddCajJFdr60qZf
   * E2Efv4WstK2tBZQIgx51F9NxO5NQI1mg7TyRVJ12AMXDuDjb
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02PL1!0\x1f\x06\x03U\x04\n\x13\x18Asseco Data Systems S.A.1\'0%\x06\x03U\x04\x0b\x13\x1eCertum Certification Authority1\x1f0\x1d\x06\x03U\x04\x03\x13\x16Certum Trusted Root CA"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xd1-\x8e\xbb\xb76\xeam7\x91\x9fN\x93\xa7\x05\xe4)\x03%\xce\x1c\x82\xf7|\x99\x9fA\x06\xcd\xed\xa3\xba\xc0\xdb\t,\xc1|\xdf)~Ke/\x93\xa7\xd4\x01k\x03(\x18\xa3\xd8\x9d\x05\xc1*\xd8E\xf1\x91\xde\xdf;\xd0\x80\x02\x8c\xcf8\x0f\xea\xa7\\x\x11\xa4\xc1\xc8\x85\\%\xd3\xd3\xb2\xe7%\xcf\x11T\x97\xab5\xc0\x1ev\x1c\xef\x00S\x9f9\xdc\x14\xa5,\"%\xb3rr\xfc\x8d\xb3\xe5>\x08\x1e\x14*7\x0b\x88<\xca\xb0\xf4\xc8\xc2\xa1\xae\xbc\xc1\xbe)gU\xe2\xfc\xadY\\\xfe\xbdW,\xb0\x90\x8d\xc2\xed7\xb6|\x99\x88\xb5\xd5\x03\x9a=\x15\r=:\xa8\xa8E\xf0\x95N%Y\x1d\xcd\x98i\xbb\xd3\xcc2\xc9\x8d\xef\x81\xfe\xad}\x89\xbb\xba`\x13\xcae\x95g\xa0\xf3\x19\xf6\x03V\xd4j\xd3\'\xe2\xa1\xad\x83\xf0J\x12\"w\x1c\x05s\xe2\x19qB\xc0\xecuF\x9a\x90X\xe0j\x8e+\xa5F0\x04\x8e\x19\xb2\x17\xe3\xbe\xa9\xba\x7fV\xf1$\x03\xd7\xb2!(v\x0e60Ly\xd5A\x9a\x9a\xa8\xb85\xba\x0c:\xf2D\x1b \x88\xf7\xc5%\xd7=\xc6\xe3>C\xdd\x87\xfe\xc4\xea\xf5S>Le\xff;J\xcbxZk\x17_\r\xc7\xc3ON\x9a*\xa2\xedWM\"\xe2F\x9a?\x0f\x914$}U\xe3\x8c\x957\xd3\x1a\xf0\t+,\xd2\xc9\x8d\xb4\r\x00\xabg)(\xd8\x01\xf5\x19\x04\xb6\x1d\xbev\xfer\\\xc4\x85\xca\xd2\x80A\xdf\x05\xa8\xa3\xd5\x84\x90O\x0b\xf3\xe0?\x9b\x19\xd27\x89?\xf2{R\x1c\x8c\xf6\xe1\xf7<\x07\x97\x8c\x0e\xa2Y\x81\x0c\xb2\x90=\xd3\xe3YF\xed\x0f\xa9\xa7\xde\x80kZ\xaa\x07\xb6\x19\xcb\xbcW\xf3\x97!z\x0c\xb1+t>\xeb\xda\xa7g-L\xc4\x98\x9e6\tvff\xfc\x1a?\xeaHT\x1c\xbe0\xbd\x80P\xbf|\xb5\xce\x00\xf6\x0ca\xd9\xe7$\x03\xe0\xe3\x01\x81\x0e\xbd\xd8\x854\x88\xbd\xb26\xa8{\\\x08\xe5D\x80\x8co\xf8/\xd5!\xca\x1d\x1c\xd0\xfb\xc4\xb5\x87\xd1:N\xc7v\xb55H\xb5\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

  /*
   * Issuer: CN=CommScope Public Trust RSA Root-02 O=CommScope
   * Subject: CN=CommScope Public Trust RSA Root-02 O=CommScope
   * Label: "CommScope Public Trust RSA Root-02"
   * Serial: 480062499834624527752716769107743131258796508494
   * SHA256 Fingerprint: ff:e9:43:d7:93:42:4b:4f:7c:44:0c:1c:3d:64:8d:53:63:f3:4b:82:dc:87:aa:7a:9f:11:8f:c5:de:e1:01:f1
   * -----BEGIN CERTIFICATE-----
   * MIIFbDCCA1SgAwIBAgIUVBa/O345lXGN0aoApYYNK496BU4wDQYJKoZIhvcNAQEL
   * BQAwTjELMAkGA1UEBhMCVVMxEjAQBgNVBAoMCUNvbW1TY29wZTErMCkGA1UEAwwi
   * Q29tbVNjb3BlIFB1YmxpYyBUcnVzdCBSU0EgUm9vdC0wMjAeFw0yMTA0MjgxNzE2
   * NDNaFw00NjA0MjgxNzE2NDJaME4xCzAJBgNVBAYTAlVTMRIwEAYDVQQKDAlDb21t
   * U2NvcGUxKzApBgNVBAMMIkNvbW1TY29wZSBQdWJsaWMgVHJ1c3QgUlNBIFJvb3Qt
   * MDIwggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAwggIKAoICAQDh+g77aAASyE3VrCLE
   * NQE7xVTlWXZjpX/rwcRqmL0yjReA61260WI9JSMZNRTpf4mnG2I81lDnNJUDMrG0
   * kyI9p+Kx7eZ7Ti6Hmw0zdQreqjXnfuU2mKKuJZ6VszKWpCtYHu8//mI0SFHRtI1C
   * rWDaSWqVcN3SAOLMV2MCe5bdSZdbkk6V0/nLKR8YSvgBKtJjCW4k6YnS5cciTNxz
   * hkcAqg2Ijq6FfUrpuzNPDlJwnZXjfG2WWy09X6GDRl224yW4fKcZgBzqZUPckXk2
   * LHR88mcGyYnJ27/aaL8j7dxrrSiDeS/sOKUNNwFnJ5rpM9kzXzehxfCrPfp4sOcs
   * n/Y+n2Dg70jpkEUeBVF4GiwSLFworA2iI540jwXmojPOEXcT1A6kHkIfhs1w/tku
   * FT0du7jyU1fbzMZ0KZwYszZ1OC4PVKH4kh+Jlk+71O6d6Ts2QrUKOyrUZHk2EOH5
   * kQMreyBUzQ0ZGshBMjTRsJnhkB4BQDa1t/qp5Xd1pCKBXbCL5CcSD1SIxtuFdOa3
   * wNemKfrb3vOTlycEVS8KbzfFPROvCgCpLIscgSjX74Yxqa7ybrjKaixUR9gqiC6v
   * wQcQeKwRoi9C8DfF8rhW3Q5iLc4tVn5V8qdE9isy9COoR+jUKgF4z2rDN6ieZdIs
   * 5fq6M8EGRPbmz6UNp2YINIos8wIDAQABo0IwQDAPBgNVHRMBAf8EBTADAQH/MA4G
   * A1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUR9DnsSL/nSz12Vdgs7GxcJXvYXowDQYJ
   * KoZIhvcNAQELBQADggIBAIZpsU0v6Z9PIpNojuQhmaPORVMbc0RTAIFhzTHjCLqB
   * KCh6krm2qMhDnscTJk3C2OVVnJJdUNjCK9v+5qiXz1I6JMNlZFxHMaNlNRPDk7n3
   * +VGXu6TwYofF1gbTl4MgqX67tiHCpQ2EAOHyJxCDut0DgdXdaMNmEMjRdrSzbyme
   * APnCKfWxkxlSaRosTKCL4BWaMS/TiJVZbuXEs1DIFAhKm4sTg7GkcrI7djNB3Nyq
   * pgdvHSQSn8h2vS/ZjvQs7rfSOBAkNlEv41xdgSGn2rtO/+YHqP65DSdsu3BaVXoT
   * 6fEqSWnHX4dXTEN5bTpl6TBcQe7rd6VzEojov32u5cSoHw2OHG1QAk8mGEPej1WF
   * sQs3BWDJVTkSBKEqz3EWnzZRSb9wO55nnPt7eck5HHisd5FUmrh1CoFSl+NmYWvt
   * PjgelmFV4ZFUjO2MJB+ByRCac5krFk5yAD9UG/iNuovnFNa2RU9g7Jauwy8CTl2d
   * lklyALKrdVwPaFsdZcJfMw8eD/A7hvWwTruc9+olBdytoptLFwG+Qt81IR2tq670
   * v64fG9PiO/yzcnMcmyiQiRM9HcEARwmWmjgb3bHPDcK0RPOWlc4yOo80nOAXx17O
   * rg3bhzjlP1v9mxnhMUF6cKojawHhRUzNlM47ni3niAIi9G7oyOzWPPO5std3eqx7
   * -----END CERTIFICATE-----
   */
  TrustAnchor {
    subject: Der::from_slice(b"1\x0b0\t\x06\x03U\x04\x06\x13\x02US1\x120\x10\x06\x03U\x04\n\x0c\tCommScope1+0)\x06\x03U\x04\x03\x0c\"CommScope Public Trust RSA Root-02"),
    subject_public_key_info: Der::from_slice(b"0\r\x06\t*\x86H\x86\xf7\r\x01\x01\x01\x05\x00\x03\x82\x02\x0f\x000\x82\x02\n\x02\x82\x02\x01\x00\xe1\xfa\x0e\xfbh\x00\x12\xc8M\xd5\xac\"\xc45\x01;\xc5T\xe5Yvc\xa5\x7f\xeb\xc1\xc4j\x98\xbd2\x8d\x17\x80\xeb]\xba\xd1b=%#\x195\x14\xe9\x7f\x89\xa7\x1bb<\xd6P\xe74\x95\x032\xb1\xb4\x93\"=\xa7\xe2\xb1\xed\xe6{N.\x87\x9b\r3u\n\xde\xaa5\xe7~\xe56\x98\xa2\xae%\x9e\x95\xb32\x96\xa4+X\x1e\xef?\xfeb4HQ\xd1\xb4\x8dB\xad`\xdaIj\x95p\xdd\xd2\x00\xe2\xccWc\x02{\x96\xddI\x97[\x92N\x95\xd3\xf9\xcb)\x1f\x18J\xf8\x01*\xd2c\tn$\xe9\x89\xd2\xe5\xc7\"L\xdcs\x86G\x00\xaa\r\x88\x8e\xae\x85}J\xe9\xbb3O\x0eRp\x9d\x95\xe3|m\x96[-=_\xa1\x83F]\xb6\xe3%\xb8|\xa7\x19\x80\x1c\xeaeC\xdc\x91y6,t|\xf2g\x06\xc9\x89\xc9\xdb\xbf\xdah\xbf#\xed\xdck\xad(\x83y/\xec8\xa5\r7\x01g\'\x9a\xe93\xd93_7\xa1\xc5\xf0\xab=\xfax\xb0\xe7,\x9f\xf6>\x9f`\xe0\xefH\xe9\x90E\x1e\x05Qx\x1a,\x12,\\(\xac\r\xa2#\x9e4\x8f\x05\xe6\xa23\xce\x11w\x13\xd4\x0e\xa4\x1eB\x1f\x86\xcdp\xfe\xd9.\x15=\x1d\xbb\xb8\xf2SW\xdb\xcc\xc6t)\x9c\x18\xb36u8.\x0fT\xa1\xf8\x92\x1f\x89\x96O\xbb\xd4\xee\x9d\xe9;6B\xb5\n;*\xd4dy6\x10\xe1\xf9\x91\x03+{ T\xcd\r\x19\x1a\xc8A24\xd1\xb0\x99\xe1\x90\x1e\x01@6\xb5\xb7\xfa\xa9\xe5wu\xa4\"\x81]\xb0\x8b\xe4\'\x12\x0fT\x88\xc6\xdb\x85t\xe6\xb7\xc0\xd7\xa6)\xfa\xdb\xde\xf3\x93\x97\'\x04U/\no7\xc5=\x13\xaf\n\x00\xa9,\x8b\x1c\x81(\xd7\xef\x861\xa9\xae\xf2n\xb8\xcaj,TG\xd8*\x88.\xaf\xc1\x07\x10x\xac\x11\xa2/B\xf07\xc5\xf2\xb8V\xdd\x0eb-\xce-V~U\xf2\xa7D\xf6+2\xf4#\xa8G\xe8\xd4*\x01x\xcfj\xc37\xa8\x9ee\xd2,\xe5\xfa\xba3\xc1\x06D\xf6\xe6\xcf\xa5\r\xa7f\x084\x8a,\xf3\x02\x03\x01\x00\x01"),
    name_constraints: None
  },

];
