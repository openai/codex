#! /bin/bash

# Primary root: root-cert
./mkcert.sh genroot "Root CA" root-key root-cert
# root cert variants: CA:false, key2, DN2, expired
./mkcert.sh genss "Root CA" root-key root-nonca
./mkcert.sh genroot "Root CA" root-key2 root-cert2
./mkcert.sh genroot "Root Cert 2" root-key root-name2
DAYS=-1 ./mkcert.sh genroot "Root CA" root-key root-expired
# cross root and root cross cert
./mkcert.sh genroot "Cross Root" cross-key cross-root
./mkcert.sh genca "Root CA" root-key root-cross-cert cross-key cross-root
# trust variants: +serverAuth -serverAuth +clientAuth -clientAuth
openssl x509 -in root-cert.pem -trustout \
    -addtrust serverAuth -out root+serverAuth.pem
openssl x509 -in root-cert.pem -trustout \
    -addreject serverAuth -out root-serverAuth.pem
openssl x509 -in root-cert.pem -trustout \
    -addtrust clientAuth -out root+clientAuth.pem
openssl x509 -in root-cert.pem -trustout \
    -addreject clientAuth -out root-clientAuth.pem
# trust variants: +anyEKU -anyEKU
openssl x509 -in root-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out root+anyEKU.pem
openssl x509 -in root-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out root-anyEKU.pem
# root-cert2 trust variants: +serverAuth -serverAuth +clientAuth
openssl x509 -in root-cert2.pem -trustout \
    -addtrust serverAuth -out root2+serverAuth.pem
openssl x509 -in root-cert2.pem -trustout \
    -addreject serverAuth -out root2-serverAuth.pem
openssl x509 -in root-cert2.pem -trustout \
    -addtrust clientAuth -out root2+clientAuth.pem
# root-nonca trust variants: +serverAuth +anyEKU
openssl x509 -in root-nonca.pem -trustout \
    -addtrust serverAuth -out nroot+serverAuth.pem
openssl x509 -in root-nonca.pem -trustout \
    -addtrust anyExtendedKeyUsage -out nroot+anyEKU.pem

# Root CA security level variants:
# MD5 self-signature
OPENSSL_SIGALG=md5 \
./mkcert.sh genroot "Root CA" root-key root-cert-md5
# 768-bit key
OPENSSL_KEYBITS=768 \
./mkcert.sh genroot "Root CA" root-key-768 root-cert-768

# primary client-EKU root: croot-cert
./mkcert.sh genroot "Root CA" root-key croot-cert clientAuth
# trust variants: +serverAuth -serverAuth +clientAuth -clientAuth +anyEKU -anyEKU
openssl x509 -in croot-cert.pem -trustout \
    -addtrust serverAuth -out croot+serverAuth.pem
openssl x509 -in croot-cert.pem -trustout \
    -addreject serverAuth -out croot-serverAuth.pem
openssl x509 -in croot-cert.pem -trustout \
    -addtrust clientAuth -out croot+clientAuth.pem
openssl x509 -in croot-cert.pem -trustout \
    -addreject clientAuth -out croot-clientAuth.pem
openssl x509 -in croot-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out croot+anyEKU.pem
openssl x509 -in croot-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out croot-anyEKU.pem

# primary server-EKU root: sroot-cert
./mkcert.sh genroot "Root CA" root-key sroot-cert serverAuth
# trust variants: +serverAuth -serverAuth +clientAuth -clientAuth +anyEKU -anyEKU
openssl x509 -in sroot-cert.pem -trustout \
    -addtrust serverAuth -out sroot+serverAuth.pem
openssl x509 -in sroot-cert.pem -trustout \
    -addreject serverAuth -out sroot-serverAuth.pem
openssl x509 -in sroot-cert.pem -trustout \
    -addtrust clientAuth -out sroot+clientAuth.pem
openssl x509 -in sroot-cert.pem -trustout \
    -addreject clientAuth -out sroot-clientAuth.pem
openssl x509 -in sroot-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out sroot+anyEKU.pem
openssl x509 -in sroot-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out sroot-anyEKU.pem

# Primary intermediate ca: ca-cert
./mkcert.sh genca "CA" ca-key ca-cert root-key root-cert
# ca variants: CA:false, no bc, key2, DN2, issuer2, expired
./mkcert.sh genee "CA" ca-key ca-nonca root-key root-cert
./mkcert.sh gen_nonbc_ca "CA" ca-key ca-nonbc root-key root-cert
./mkcert.sh genca "CA" ca-key2 ca-cert2 root-key root-cert
./mkcert.sh genca "CA2" ca-key ca-name2 root-key root-cert
./mkcert.sh genca "CA" ca-key ca-root2 root-key2 root-cert2
DAYS=-1 ./mkcert.sh genca "CA" ca-key ca-expired root-key root-cert
# trust variants: +serverAuth, -serverAuth, +clientAuth, -clientAuth
openssl x509 -in ca-cert.pem -trustout \
    -addtrust serverAuth -out ca+serverAuth.pem
openssl x509 -in ca-cert.pem -trustout \
    -addreject serverAuth -out ca-serverAuth.pem
openssl x509 -in ca-cert.pem -trustout \
    -addtrust clientAuth -out ca+clientAuth.pem
openssl x509 -in ca-cert.pem -trustout \
    -addreject clientAuth -out ca-clientAuth.pem
# trust variants: +anyEKU, -anyEKU
openssl x509 -in ca-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out ca+anyEKU.pem
openssl x509 -in ca-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out ca-anyEKU.pem
# ca-nonca trust variants: +serverAuth, +anyEKU
openssl x509 -in ca-nonca.pem -trustout \
    -addtrust serverAuth -out nca+serverAuth.pem
openssl x509 -in ca-nonca.pem -trustout \
    -addtrust anyExtendedKeyUsage -out nca+anyEKU.pem

# Intermediate CA security variants:
# MD5 issuer signature,
OPENSSL_SIGALG=md5 \
./mkcert.sh genca "CA" ca-key ca-cert-md5 root-key root-cert
openssl x509 -in ca-cert-md5.pem -trustout \
    -addtrust anyExtendedKeyUsage -out ca-cert-md5-any.pem
# Issuer has 768-bit key
./mkcert.sh genca "CA" ca-key ca-cert-768i root-key-768 root-cert-768
# CA has 768-bit key
OPENSSL_KEYBITS=768 \
./mkcert.sh genca "CA" ca-key-768 ca-cert-768 root-key root-cert
# EC cert with explicit curve
./mkcert.sh genca "CA" ca-key-ec-explicit ca-cert-ec-explicit root-key root-cert
# EC cert with named curve
./mkcert.sh genca "CA" ca-key-ec-named ca-cert-ec-named root-key root-cert

# client intermediate ca: cca-cert
./mkcert.sh genca -p clientAuth "CA" ca-key cca-cert root-key root-cert
# trust variants: +serverAuth, -serverAuth, +clientAuth, -clientAuth, +anyEKU, -anyEKU
openssl x509 -in cca-cert.pem -trustout \
    -addtrust serverAuth -out cca+serverAuth.pem
openssl x509 -in cca-cert.pem -trustout \
    -addreject serverAuth -out cca-serverAuth.pem
openssl x509 -in cca-cert.pem -trustout \
    -addtrust clientAuth -out cca+clientAuth.pem
openssl x509 -in cca-cert.pem -trustout \
    -addreject clientAuth -out cca-clientAuth.pem
openssl x509 -in cca-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out cca+anyEKU.pem
openssl x509 -in cca-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out cca-anyEKU.pem

# server intermediate ca: sca-cert
./mkcert.sh genca -p serverAuth "CA" ca-key sca-cert root-key root-cert
# trust variants: +serverAuth, -serverAuth, +clientAuth, -clientAuth, +anyEKU, -anyEKU
openssl x509 -in sca-cert.pem -trustout \
    -addtrust serverAuth -out sca+serverAuth.pem
openssl x509 -in sca-cert.pem -trustout \
    -addreject serverAuth -out sca-serverAuth.pem
openssl x509 -in sca-cert.pem -trustout \
    -addtrust clientAuth -out sca+clientAuth.pem
openssl x509 -in sca-cert.pem -trustout \
    -addreject clientAuth -out sca-clientAuth.pem
openssl x509 -in sca-cert.pem -trustout \
    -addtrust anyExtendedKeyUsage -out sca+anyEKU.pem
openssl x509 -in sca-cert.pem -trustout \
    -addreject anyExtendedKeyUsage -out sca-anyEKU.pem

# Primary leaf cert: ee-cert with default purpose: serverAuth
./mkcert.sh genee server.example ee-key ee-cert ca-key ca-cert
# ee variants: expired, issuer-key2, issuer-name2, bad-pathlen
./mkcert.sh genee server.example ee-key ee-expired ca-key ca-cert -days -1
./mkcert.sh genee server.example ee-key ee-cert2 ca-key2 ca-cert2
./mkcert.sh genee server.example ee-key ee-name2 ca-key ca-name2
./mkcert.sh genee server.example ee-key ee-pathlen ca-key ca-cert \
    -extfile <(echo "basicConstraints=CA:false,pathlen:0") # bash needed here
# purpose variants: clientAuth
./mkcert.sh genee -p clientAuth server.example ee-key ee-client ca-key ca-cert
# trust variants: +serverAuth, -serverAuth, +clientAuth, -clientAuth
openssl x509 -in ee-cert.pem -trustout \
    -addtrust serverAuth -out ee+serverAuth.pem
openssl x509 -in ee-cert.pem -trustout \
    -addreject serverAuth -out ee-serverAuth.pem
openssl x509 -in ee-client.pem -trustout \
    -addtrust clientAuth -out ee+clientAuth.pem
openssl x509 -in ee-client.pem -trustout \
    -addreject clientAuth -out ee-clientAuth.pem

# time stamping certificates
./mkcert.sh genee -p critical,timeStamping -k critical,digitalSignature server.example ee-key ee-timestampsign-CABforum ca-key ca-cert
./mkcert.sh genee -p timeStamping -k critical,digitalSignature server.example ee-key ee-timestampsign-CABforum-noncritxku ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping,serverAuth -k critical,digitalSignature server.example ee-key ee-timestampsign-CABforum-serverauth ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping,2.5.29.37.0 -k critical,digitalSignature server.example ee-key ee-timestampsign-CABforum-anyextkeyusage ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping -k critical,digitalSignature,cRLSign server.example ee-key ee-timestampsign-CABforum-crlsign ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping -k critical,digitalSignature,keyCertSign server.example ee-key ee-timestampsign-CABforum-keycertsign ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping server.example ee-key ee-timestampsign-rfc3161 ca-key ca-cert
./mkcert.sh genee -p timeStamping server.example ee-key ee-timestampsign-rfc3161-noncritxku ca-key ca-cert
./mkcert.sh genee -p critical,timeStamping -k digitalSignature server.example ee-key ee-timestampsign-rfc3161-digsig ca-key ca-cert

# code signing certificate
./mkcert.sh genee -p codeSigning -k critical,digitalSignature server.example ee-key ee-codesign ca-key ca-cert
./mkcert.sh genee -p codeSigning,serverAuth -k critical,digitalSignature server.example ee-key ee-codesign-serverauth ca-key ca-cert
./mkcert.sh genee -p codeSigning,2.5.29.37.0 -k critical,digitalSignature server.example ee-key ee-codesign-anyextkeyusage ca-key ca-cert
./mkcert.sh genee -p codeSigning -k critical,digitalSignature,cRLSign server.example ee-key ee-codesign-crlsign ca-key ca-cert
./mkcert.sh genee -p codeSigning -k critical,digitalSignature,keyCertSign server.example ee-key ee-codesign-keycertsign ca-key ca-cert
./mkcert.sh genee -p codeSigning -k digitalSignature server.example ee-key ee-codesign-noncritical ca-key ca-cert

# Leaf cert security level variants
# MD5 issuer signature
OPENSSL_SIGALG=md5 \
./mkcert.sh genee server.example ee-key ee-cert-md5 ca-key ca-cert
# 768-bit issuer key
./mkcert.sh genee server.example ee-key ee-cert-768i ca-key-768 ca-cert-768
# 768-bit leaf key
OPENSSL_KEYBITS=768 \
./mkcert.sh genee server.example ee-key-768 ee-cert-768 ca-key ca-cert
# EC cert with explicit curve signed by named curve ca
./mkcert.sh genee server.example ee-key-ec-explicit ee-cert-ec-explicit ca-key-ec-named ca-cert-ec-named
# EC cert with named curve signed by explicit curve ca
./mkcert.sh genee server.example ee-key-ec-named-explicit \
    ee-cert-ec-named-explicit ca-key-ec-explicit ca-cert-ec-explicit
# EC cert with named curve signed by named curve ca
./mkcert.sh genee server.example ee-key-ec-named-named \
    ee-cert-ec-named-named ca-key-ec-named ca-cert-ec-named
# 1024-bit leaf key
OPENSSL_KEYBITS=1024 \
./mkcert.sh genee server.example ee-key-1024 ee-cert-1024 ca-key ca-cert
# 3072-bit leaf key
OPENSSL_KEYBITS=3072 \
./mkcert.sh genee server.example ee-key-3072 ee-cert-3072 ca-key ca-cert
# 4096-bit leaf key
OPENSSL_KEYBITS=4096 \
./mkcert.sh genee server.example ee-key-4096 ee-cert-4096 ca-key ca-cert
# 8192-bit leaf key
OPENSSL_KEYBITS=8192 \
./mkcert.sh genee server.example ee-key-8192 ee-cert-8192 ca-key ca-cert

# self-signed end-entity cert with explicit keyUsage not including KeyCertSign
openssl req -new -x509 -key ee-key.pem -subj /CN=ee-self-signed -out ee-self-signed.pem -addext keyUsage=digitalSignature -days 36525

# self-signed end-entity cert signed with RSA-PSS
openssl req -new -x509 -key ee-key.pem -subj /CN=ee-self-signed-pss -out ee-self-signed-pss.pem -days 36525 \
    -sha256 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:digest

# Proxy certificates, off of ee-client
# Start with some good ones
./mkcert.sh req pc1-key "0.CN = server.example" "1.CN = proxy 1" | \
    ./mkcert.sh genpc pc1-key pc1-cert ee-key ee-client \
                "language = id-ppl-anyLanguage" "pathlen = 1" "policy = text:AB"
./mkcert.sh req pc2-key "0.CN = server.example" "1.CN = proxy 1" "2.CN = proxy 2" | \
    ./mkcert.sh genpc pc2-key pc2-cert pc1-key pc1-cert \
                "language = id-ppl-anyLanguage" "pathlen = 0" "policy = text:AB"
# And now a couple of bad ones
# pc3: incorrect CN
./mkcert.sh req bad-pc3-key "0.CN = server.example" "1.CN = proxy 3" | \
    ./mkcert.sh genpc bad-pc3-key bad-pc3-cert pc1-key pc1-cert \
                "language = id-ppl-anyLanguage" "pathlen = 0" "policy = text:AB"
# pc4: incorrect pathlen
./mkcert.sh req bad-pc4-key "0.CN = server.example" "1.CN = proxy 1" "2.CN = proxy 4" | \
    ./mkcert.sh genpc bad-pc4-key bad-pc4-cert pc1-key pc1-cert \
                "language = id-ppl-anyLanguage" "pathlen = 1" "policy = text:AB"
# pc5: no policy
./mkcert.sh req pc5-key "0.CN = server.example" "1.CN = proxy 1" "2.CN = proxy 5" | \
    ./mkcert.sh genpc pc5-key pc5-cert pc1-key pc1-cert \
                "language = id-ppl-anyLanguage" "pathlen = 0"
# pc6: incorrect CN (made into a component of a multivalue RDN)
./mkcert.sh req bad-pc6-key "0.CN = server.example" "1.CN = proxy 1" "2.+CN = proxy 6" | \
    ./mkcert.sh genpc bad-pc6-key bad-pc6-cert pc1-key pc1-cert \
                "language = id-ppl-anyLanguage" "pathlen = 0" "policy = text:AB"

# Name constraints test certificates.

# NC CA1 only permits the host www.good.org and *.good.com email address
# good@good.org and *@good.com and IP addresses 127.0.0.1 and
# 192.168.0.0/16

NC="permitted;DNS:www.good.org, permitted;DNS:good.com,"
NC="$NC permitted;email:good@good.org, permitted;email:good.com,"
NC="$NC permitted;IP:127.0.0.1/255.255.255.255, permitted;IP:192.168.0.0/255.255.0.0"

NC=$NC ./mkcert.sh genca "Test NC CA 1" ncca1-key ncca1-cert root-key root-cert

# NC CA2 allows anything apart from hosts www.bad.org and *.bad.com
# and email addresses bad@bad.org and *@bad.com

NC="excluded;DNS:www.bad.org, excluded;DNS:bad.com,"
NC="$NC excluded;email:bad@bad.org, excluded;email:bad.com, "
NC="$NC excluded;IP:10.0.0.0/255.0.0.0"

NC=$NC ./mkcert.sh genca "Test NC CA 2" ncca2-key ncca2-cert root-key root-cert

# Name constraints subordinate CA. Adds www.good.net (which should be
# disallowed because parent CA doesn't permit it) adds ok.good.com
# (which should be allowed because parent allows *.good.com
# and now excludes bad.ok.good.com (allowed in permitted subtrees
# but explicitly excluded).

NC="permitted;DNS:www.good.net, permitted;DNS:ok.good.com, "
NC="$NC excluded;DNS:bad.ok.good.com"
NC=$NC ./mkcert.sh genca "Test NC sub CA" ncca3-key ncca3-cert \
        ncca1-key ncca1-cert

# all subjectAltNames allowed by CA1.  Some CNs are not!

./mkcert.sh req alt1-key "O = Good NC Test Certificate 1" \
    "1.CN=www.example.net" "2.CN=Joe Bloggs" | \
    ./mkcert.sh geneealt alt1-key alt1-cert ncca1-key ncca1-cert \
    "DNS.1 = www.good.org" "DNS.2 = any.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# all DNS-like CNs allowed by CA1, no DNS SANs.

./mkcert.sh req goodcn1-key "O = Good NC Test Certificate 1" \
    "1.CN=www.good.org" "2.CN=any.good.com" \
    "3.CN=not..dns" "4.CN=not@dns" "5.CN=not-.dns" "6.CN=not.dns." | \
    ./mkcert.sh geneealt goodcn1-key goodcn1-cert ncca1-key ncca1-cert \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# all DNS-like CNs allowed by CA1, no SANs

./mkcert.sh req goodcn2-key "O = Good NC Test Certificate 1" \
    "CN=www.good.org" | \
    ./mkcert.sh geneeconfig goodcn2-key goodcn2-cert ncca1-key ncca1-cert

# Some DNS-like CNs not permitted by CA1, no DNS SANs.

./mkcert.sh req badcn1-key "O = Good NC Test Certificate 1" \
    "1.CN=www.good.org" "3.CN=bad.net" | \
    ./mkcert.sh geneealt badcn1-key badcn1-cert ncca1-key ncca1-cert \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# no subjectAltNames excluded by CA2.

./mkcert.sh req alt2-key "O = Good NC Test Certificate 2" | \
    ./mkcert.sh geneealt alt2-key alt2-cert ncca2-key ncca2-cert \
    "DNS.1 = www.anything.org" "DNS.2 = any.other.com" \
    "email.1 = other@bad.org" "email.2 = any@something.com"

# hostname other.good.org which is not allowed by CA1.

./mkcert.sh req badalt1-key "O = Bad NC Test Certificate 1" | \
    ./mkcert.sh geneealt badalt1-key badalt1-cert ncca1-key ncca1-cert \
    "DNS.1 = other.good.org" "DNS.2 = any.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com"

# any.bad.com is excluded by CA2.

./mkcert.sh req badalt2-key 'O = Bad NC Test Certificate 2' | \
    ./mkcert.sh geneealt badalt2-key badalt2-cert ncca2-key ncca2-cert \
    "DNS.1 = www.good.org" "DNS.2 = any.bad.com" \
    "email.1 = good@good.org" "email.2 = any@good.com"

# other@good.org not permitted by CA1

./mkcert.sh req badalt3-key "O = Bad NC Test Certificate 3" | \
    ./mkcert.sh geneealt badalt3-key badalt1-cert ncca1-key ncca1-cert \
    "DNS.1 = www.good.org" "DNS.2 = any.good.com" \
    "email.1 = other@good.org" "email.2 = any@good.com"

# all subject alt names OK but subject email address not allowed by CA1.

./mkcert.sh req badalt4-key 'O = Bad NC Test Certificate 4' \
    "emailAddress = any@other.com" | \
    ./mkcert.sh geneealt badalt4-key badalt4-cert ncca1-key ncca1-cert \
    "DNS.1 = www.good.org" "DNS.2 = any.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com"

# IP address not allowed by CA1
./mkcert.sh req badalt5-key "O = Bad NC Test Certificate 5" | \
    ./mkcert.sh geneealt badalt5-key badalt5-cert ncca1-key ncca1-cert \
    "DNS.1 = www.good.org" "DNS.2 = any.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.2"

# No DNS-ID SANs and subject CN not allowed by CA1.
./mkcert.sh req badalt6-key "O = Bad NC Test Certificate 6" \
    "1.CN=other.good.org" "2.CN=Joe Bloggs" "3.CN=any.good.com" | \
    ./mkcert.sh geneealt badalt6-key badalt6-cert ncca1-key ncca1-cert \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# No DNS-ID SANS and subject CN not allowed by CA1, BMPSTRING
REQMASK=MASK:0x800 ./mkcert.sh req badalt7-key "O = Bad NC Test Certificate 7" \
    "1.CN=other.good.org" "2.CN=Joe Bloggs" "3.CN=any.good.com" | \
    ./mkcert.sh geneealt badalt7-key badalt7-cert ncca1-key ncca1-cert \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# all subjectAltNames allowed by chain

./mkcert.sh req alt3-key "O = Good NC Test Certificate 3" \
    "1.CN=www.ok.good.com" "2.CN=Joe Bloggs" | \
    ./mkcert.sh geneealt alt3-key alt3-cert ncca3-key ncca3-cert \
    "DNS.1 = www.ok.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# www.good.net allowed by parent CA but not parent of parent

./mkcert.sh req badalt8-key "O = Bad NC Test Certificate 8" \
    "1.CN=www.good.com" "2.CN=Joe Bloggs" | \
    ./mkcert.sh geneealt badalt8-key badalt8-cert ncca3-key ncca3-cert \
    "DNS.1 = www.ok.good.com" "DNS.2 = www.good.net" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# other.good.com not allowed by parent CA but allowed by parent of parent

./mkcert.sh req badalt9-key "O = Bad NC Test Certificate 9" \
    "1.CN=www.good.com" "2.CN=Joe Bloggs" | \
    ./mkcert.sh geneealt badalt9-key badalt9-cert ncca3-key ncca3-cert \
    "DNS.1 = www.good.com" "DNS.2 = other.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# www.bad.net excluded by parent CA.

./mkcert.sh req badalt10-key "O = Bad NC Test Certificate 10" \
    "1.CN=www.ok.good.com" "2.CN=Joe Bloggs" | \
    ./mkcert.sh geneealt badalt10-key badalt10-cert ncca3-key ncca3-cert \
    "DNS.1 = www.ok.good.com" "DNS.2 = bad.ok.good.com" \
    "email.1 = good@good.org" "email.2 = any@good.com" \
    "IP = 127.0.0.1" "IP = 192.168.0.1"

# NC CA4 only permits URIs matching good.org.

NC="permitted;URI:good.org"
NC=$NC ./mkcert.sh genca "Test NC CA 4" ncca4-key ncca4-cert root-key root-cert

# A certificate with an URI SAN
./mkcert.sh req alt1-key "O = Good NC Test Certificate 1" \
    "CN=Joe Bloggs" | \
    ./mkcert.sh geneealt nc-uri-key nc-uri-cert ncca4-key ncca4-cert \
    "URI.1 = foo://%40something@good.org" \
    "URI.2 = bar://other@good.org/baz/quux"

# Certs for CVE-2022-4203 testcase

NC="excluded;otherName:SRVName;UTF8STRING:foo@example.org" ./mkcert.sh genca \
    "Test NC CA othername" nccaothername-key nccaothername-cert \
    root-key root-cert

./mkcert.sh req alt-email-key "O = NC email in othername Test Certificate" | \
    ./mkcert.sh geneealt bad-othername-key bad-othername-cert \
    nccaothername-key nccaothername-cert \
    "otherName.1 = SRVName;UTF8STRING:foo@example.org"

# RSA-PSS signatures
# SHA1
./mkcert.sh genee PSS-SHA1 ee-key ee-pss-sha1-cert ca-key ca-cert \
    -sha1 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:digest
# EE SHA256
./mkcert.sh genee PSS-SHA256 ee-key ee-pss-sha256-cert ca-key ca-cert \
            -sha256 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:digest
# CA-PSS
./mkcert.sh genca "CA-PSS" ca-pss-key ca-pss-cert root-key root-cert \
            -sha256 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:-1
./mkcert.sh genee "EE-PSS" ee-key ee-pss-cert ca-pss-key ca-pss-cert \
            -sha256 -sigopt rsa_padding_mode:pss -sigopt rsa_pss_saltlen:-1
# Should not have been possible to produce, see issue #13968:
#./mkcert.sh genee "EE-PSS-wrong1.5" ee-key ee-pss-wrong1.5-cert ca-pss-key ca-pss-cert -sha256

OPENSSL_KEYALG=ec OPENSSL_KEYBITS=brainpoolP256r1 ./mkcert.sh genee \
    "Server ECDSA brainpoolP256r1 cert" server-ecdsa-brainpoolP256r1-key \
    server-ecdsa-brainpoolP256r1-cert rootkey rootcert

openssl req -new -noenc -subj "/CN=localhost" \
    -newkey rsa-pss -keyout server-pss-restrict-key.pem \
    -pkeyopt rsa_pss_keygen_md:sha256 -pkeyopt rsa_pss_keygen_saltlen:32 | \
    ./mkcert.sh geneenocsr "Server RSA-PSS restricted cert" \
    server-pss-restrict-cert rootkey rootcert

openssl req -new -noenc -subj "/CN=Client-RSA-PSS" \
    -newkey rsa-pss -keyout client-pss-restrict-key.pem \
    -pkeyopt rsa_pss_keygen_md:sha256 -pkeyopt rsa_pss_keygen_saltlen:32 | \
    ./mkcert.sh geneenocsr -p clientAuth "Client RSA-PSS restricted cert" \
    client-pss-restrict-cert rootkey rootcert

# CT entry
./mkcert.sh genct server.example embeddedSCTs1-key embeddedSCTs1 embeddedSCTs1_issuer-key embeddedSCTs1_issuer ct-server-key

OPENSSL_SIGALG= OPENSSL_KEYALG=ed448 ./mkcert.sh genroot "Root Ed448" \
    root-ed448-key root-ed448-cert
OPENSSL_SIGALG=ED448 OPENSSL_KEYALG=ed448 ./mkcert.sh genee ed448 \
    server-ed448-key server-ed448-cert root-ed448-key root-ed448-cert

# non-critical unknown extension
./mkcert.sh geneeextra server.example ee-key ee-cert-noncrit-unknown-ext ca-key ca-cert "1.2.3.4=DER:05:00"

# critical unknown extension
./mkcert.sh geneeextra server.example ee-key ee-cert-crit-unknown-ext ca-key ca-cert "1.2.3.4=critical,DER:05:00"

# critical id-pkix-ocsp-no-check extension
./mkcert.sh geneeextra server.example ee-key ee-cert-ocsp-nocheck ca-key ca-cert "1.3.6.1.5.5.7.48.1.5=critical,DER:05:00"

# certificatePolicies extension
./mkcert.sh genca -c "1.3.6.1.4.1.16604.998855.1" "CA" ca-key ca-pol-cert root-key root-cert
./mkcert.sh geneeextra server.example ee-key ee-cert-policies ca-key ca-cert "certificatePolicies=1.3.6.1.4.1.16604.998855.1"
# We can create a cert with a duplicate policy oid - but its actually invalid!
./mkcert.sh geneeextra server.example ee-key ee-cert-policies-bad ca-key ca-cert "certificatePolicies=1.3.6.1.4.1.16604.998855.1,1.3.6.1.4.1.16604.998855.1"

# EC cert signed by curve ca with SHA3-224, SHA3-256, SHA3-384, SHA3-512
OPENSSL_SIGALG="sha3-224" ./mkcert.sh genee server.example ee-key-ec-named-named ee-cert-ec-sha3-224 ca-key-ec-named ca-cert-ec-named
OPENSSL_SIGALG="sha3-256" ./mkcert.sh genee server.example ee-key-ec-named-named ee-cert-ec-sha3-256 ca-key-ec-named ca-cert-ec-named
OPENSSL_SIGALG="sha3-384" ./mkcert.sh genee server.example ee-key-ec-named-named ee-cert-ec-sha3-384 ca-key-ec-named ca-cert-ec-named
OPENSSL_SIGALG="sha3-512" ./mkcert.sh genee server.example ee-key-ec-named-named ee-cert-ec-sha3-512 ca-key-ec-named ca-cert-ec-named

# EC cert seigned RSA intermediate CA
OPENSSL_KEYALG=ec OPENSSL_KEYBITS=prime256v1 ./mkcert.sh genee \
    "P-256 cert EE issuer" p256-ee-rsa-ca-key \
    p256-ee-rsa-ca-cert ca-key ca-cert
