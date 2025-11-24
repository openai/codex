#!/bin/sh

opensslcmd() {
    LD_LIBRARY_PATH=../.. ../../apps/openssl $@
}

# report the openssl version
opensslcmd version

echo "Creating private keys and certs..."

#####

# root CA private key
opensslcmd genpkey \
           -algorithm EC \
           -pkeyopt ec_paramgen_curve:secp521r1 \
           -pkeyopt ec_param_enc:named_curve \
           -out root-key.pem

# root CA certificate (self-signed)
opensslcmd req \
           -config ca.cnf \
           -x509 \
           -days 3650 \
           -key root-key.pem \
           -subj /CN=TestRootCA \
           -out root-cert.pem
#####

# intermediate CA private key
opensslcmd genpkey \
           -algorithm EC \
           -pkeyopt ec_paramgen_curve:secp384r1 \
           -pkeyopt ec_param_enc:named_curve \
           -out intermediate-key.pem

# intermediate CA certificate-signing-request
opensslcmd req \
           -config ca.cnf \
           -new \
           -key intermediate-key.pem \
           -subj /CN=TestIntermediateCA \
           -out intermediate-csr.pem

# intermediate CA certificate (signed by root CA)
opensslcmd req \
           -config ca.cnf \
           -x509 \
           -days 1825 \
           -CA root-cert.pem \
           -CAkey root-key.pem \
           -in intermediate-csr.pem \
           -copy_extensions copyall \
           -out intermediate-cert.pem
#####

# server key
opensslcmd genpkey \
           -algorithm EC \
           -pkeyopt ec_paramgen_curve:prime256v1 \
           -pkeyopt ec_param_enc:named_curve \
           -out server-key.pem

# server certificate-signing-request
opensslcmd req \
           -config ca.cnf \
	   -extensions usr_cert \
           -new \
           -key server-key.pem \
           -subj /CN=TestServerCA \
           -out server-csr.pem

# server certificate (signed by intermediate CA)
opensslcmd req \
           -config ca.cnf \
	   -extensions usr_cert \
           -x509 \
           -days 365 \
           -CA intermediate-cert.pem \
           -CAkey intermediate-key.pem \
           -in server-csr.pem \
           -copy_extensions copyall \
           -out server-cert.pem
#####

rm -f index.txt index.txt.attr
echo -n > index.txt
opensslcmd ca \
	   -config ca.cnf \
	   -valid server-cert.pem \
	   -keyfile intermediate-key.pem \
	   -cert intermediate-cert.pem
rm -f index.txt.old
#####

cat server-cert.pem server-key.pem intermediate-cert.pem > server.pem
cat intermediate-cert.pem intermediate-key.pem > ocsp.pem

echo "Done."
