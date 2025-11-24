# configuration file for util/mkerr.pl

# The INPUT HEADER is scanned for declarations
# LIBNAME       PUBLIC HEADER                   ERROR-TABLE FILE                        INTERNAL HEADER (if relevant)
L ERR           NONE                            NONE                                    
L FUNC          NONE                            NONE                                    
L BN            include/openssl/bnerr.h         crypto/bn/bn_err.c                      include/crypto/bnerr.h
L RSA           include/openssl/rsaerr.h        crypto/rsa/rsa_err.c                    include/crypto/rsaerr.h
L DH            include/openssl/dherr.h         crypto/dh/dh_err.c                      include/crypto/dherr.h
L EVP           include/openssl/evperr.h        crypto/evp/evp_err.c                    include/crypto/evperr.h
L BUF           include/openssl/buffererr.h     crypto/buffer/buf_err.c                 include/crypto/buffererr.h
L OBJ           include/openssl/objectserr.h    crypto/objects/obj_err.c                include/crypto/objectserr.h
L PEM           include/openssl/pemerr.h        crypto/pem/pem_err.c                    include/crypto/pemerr.h
L DSA           include/openssl/dsaerr.h        crypto/dsa/dsa_err.c                    include/crypto/dsaerr.h
L X509          include/openssl/x509err.h       crypto/x509/x509_err.c                  include/crypto/x509err.h
L ASN1          include/openssl/asn1err.h       crypto/asn1/asn1_err.c                  include/crypto/asn1err.h
L CONF          include/openssl/conferr.h       crypto/conf/conf_err.c                  include/crypto/conferr.h
L CRYPTO        include/openssl/cryptoerr.h     crypto/cpt_err.c                        include/crypto/cryptoerr.h
L EC            include/openssl/ecerr.h         crypto/ec/ec_err.c                      include/crypto/ecerr.h
L SSL           include/openssl/sslerr.h        crypto/ssl_err.c                        crypto/sslerr.h
L BIO           include/openssl/bioerr.h        crypto/bio/bio_err.c                    include/crypto/bioerr.h
L PKCS7         include/openssl/pkcs7err.h      crypto/pkcs7/pkcs7err.c                 include/crypto/pkcs7err.h
L X509V3        include/openssl/x509v3err.h     crypto/x509/v3err.c                     include/crypto/x509v3err.h
L PKCS12        include/openssl/pkcs12err.h     crypto/pkcs12/pk12err.c                 include/crypto/pkcs12err.h
L RAND          include/openssl/randerr.h       crypto/rand/rand_err.c                  include/crypto/randerr.h
L DSO           NONE                            crypto/dso/dso_err.c                    include/internal/dsoerr.h
L ENGINE        include/openssl/engineerr.h     crypto/engine/eng_err.c                 include/crypto/engineerr.h
L OCSP          include/openssl/ocsperr.h       crypto/ocsp/ocsp_err.c                  include/crypto/ocsperr.h
L UI            include/openssl/uierr.h         crypto/ui/ui_err.c                      include/crypto/uierr.h
L COMP          include/openssl/comperr.h       crypto/comp/comp_err.c                  include/crypto/comperr.h
L TS            include/openssl/tserr.h         crypto/ts/ts_err.c                      include/crypto/tserr.h
L CMS           include/openssl/cmserr.h        crypto/cms/cms_err.c                    include/crypto/cmserr.h
L CRMF          include/openssl/crmferr.h       crypto/crmf/crmf_err.c                  include/crypto/crmferr.h
L CMP           include/openssl/cmperr.h        crypto/cmp/cmp_err.c                    include/crypto/cmperr.h
L CT            include/openssl/cterr.h         crypto/ct/ct_err.c                      include/crypto/cterr.h
L ASYNC         include/openssl/asyncerr.h      crypto/async/async_err.c                include/crypto/asyncerr.h
# KDF is only here for conservation purposes
L KDF           NONE                            NONE                                    NONE
L SM2           NONE                            crypto/sm2/sm2_err.c                    include/crypto/sm2err.h
L OSSL_STORE    include/openssl/storeerr.h      crypto/store/store_err.c                include/crypto/storeerr.h
L ESS           include/openssl/esserr.h        crypto/ess/ess_err.c                    include/crypto/esserr.h
L PROP          NONE                            crypto/property/property_err.c          include/internal/propertyerr.h
L PROV          include/openssl/proverr.h       providers/common/provider_err.c         providers/common/include/prov/proverr.h
L OSSL_ENCODER  include/openssl/encodererr.h    crypto/encode_decode/encoder_err.c      include/crypto/encodererr.h
L OSSL_DECODER  include/openssl/decodererr.h    crypto/encode_decode/decoder_err.c      include/crypto/decodererr.h
L HTTP          include/openssl/httperr.h       crypto/http/http_err.c                  include/crypto/httperr.h

# SSL/TLS alerts
R SSL_R_SSLV3_ALERT_UNEXPECTED_MESSAGE          1010
R SSL_R_SSLV3_ALERT_BAD_RECORD_MAC              1020
R SSL_R_TLSV1_ALERT_DECRYPTION_FAILED           1021
R SSL_R_TLSV1_ALERT_RECORD_OVERFLOW             1022
R SSL_R_SSLV3_ALERT_DECOMPRESSION_FAILURE       1030
R SSL_R_SSLV3_ALERT_HANDSHAKE_FAILURE           1040
R SSL_R_SSLV3_ALERT_NO_CERTIFICATE              1041
R SSL_R_SSLV3_ALERT_BAD_CERTIFICATE             1042
R SSL_R_SSLV3_ALERT_UNSUPPORTED_CERTIFICATE     1043
R SSL_R_SSLV3_ALERT_CERTIFICATE_REVOKED         1044
R SSL_R_SSLV3_ALERT_CERTIFICATE_EXPIRED         1045
R SSL_R_SSLV3_ALERT_CERTIFICATE_UNKNOWN         1046
R SSL_R_SSLV3_ALERT_ILLEGAL_PARAMETER           1047
R SSL_R_TLSV1_ALERT_UNKNOWN_CA                  1048
R SSL_R_TLSV1_ALERT_ACCESS_DENIED               1049
R SSL_R_TLSV1_ALERT_DECODE_ERROR                1050
R SSL_R_TLSV1_ALERT_DECRYPT_ERROR               1051
R SSL_R_TLSV1_ALERT_EXPORT_RESTRICTION          1060
R SSL_R_TLSV1_ALERT_PROTOCOL_VERSION            1070
R SSL_R_TLSV1_ALERT_INSUFFICIENT_SECURITY       1071
R SSL_R_TLSV1_ALERT_INTERNAL_ERROR              1080
R SSL_R_TLSV1_ALERT_INAPPROPRIATE_FALLBACK      1086
R SSL_R_TLSV1_ALERT_USER_CANCELLED              1090
R SSL_R_TLSV1_ALERT_NO_RENEGOTIATION            1100
R SSL_R_TLSV13_ALERT_MISSING_EXTENSION          1109
R SSL_R_TLSV1_UNSUPPORTED_EXTENSION             1110
R SSL_R_TLSV1_CERTIFICATE_UNOBTAINABLE          1111
R SSL_R_TLSV1_UNRECOGNIZED_NAME                 1112
R SSL_R_TLSV1_BAD_CERTIFICATE_STATUS_RESPONSE   1113
R SSL_R_TLSV1_BAD_CERTIFICATE_HASH_VALUE        1114
R SSL_R_TLSV1_ALERT_UNKNOWN_PSK_IDENTITY        1115
R SSL_R_TLSV13_ALERT_CERTIFICATE_REQUIRED       1116
R SSL_R_TLSV1_ALERT_NO_APPLICATION_PROTOCOL     1120
