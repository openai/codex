#! /bin/bash
#
# Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
# Copyright (c) 2016 Viktor Dukhovni <openssl-users@dukhovni.org>.
# All rights reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# This file is dual-licensed and is also available under other terms.
# Please contact the author.

# 100 years should be enough for now
if [ -z "$DAYS" ]; then
    DAYS=36525
fi

if [ -z "$OPENSSL_SIGALG" ]; then
    OPENSSL_SIGALG=sha256
fi

if [ -z "$REQMASK" ]; then
    REQMASK=utf8only
fi

stderr_onerror() {
    (
        err=$("$@" >&3 2>&1) || {
            printf "%s\n" "$err" >&2
            exit 1
        }
    ) 3>&1
}

key() {
    local key=$1; shift

    local alg=rsa
    if [ -n "$OPENSSL_KEYALG" ]; then
        alg=$OPENSSL_KEYALG
    fi

    local bits=2048
    if [ -n "$OPENSSL_KEYBITS" ]; then
        bits=$OPENSSL_KEYBITS
    fi

    if [ ! -f "${key}.pem" ]; then
        args=(-algorithm "$alg")
        case $alg in
        rsa) args=("${args[@]}" -pkeyopt rsa_keygen_bits:$bits );;
        ec)  args=("${args[@]}" -pkeyopt "ec_paramgen_curve:$bits")
               args=("${args[@]}" -pkeyopt ec_param_enc:named_curve);;
        dsa)  args=(-paramfile "$bits");;
        ed25519)  ;;
        ed448)  ;;
        *) printf "Unsupported key algorithm: %s\n" "$alg" >&2; return 1;;
        esac
        stderr_onerror \
            openssl genpkey "${args[@]}" -out "${key}.pem"
    fi
}

# Usage: $0 req keyname dn1 dn2 ...
req() {
    local key=$1; shift

    key "$key"
    local errs

    stderr_onerror \
        openssl req -new -"${OPENSSL_SIGALG}" -key "${key}.pem" \
            -config <(printf "string_mask=%s\n[req]\n%s\n%s\n[dn]\n" \
              "$REQMASK" "prompt = no" "distinguished_name = dn"
                      for dn in "$@"; do echo "$dn"; done)
}

req_nocn() {
    local key=$1; shift

    key "$key"
    stderr_onerror \
        openssl req -new -"${OPENSSL_SIGALG}" -subj / -key "${key}.pem" \
            -config <(printf "[req]\n%s\n[dn]\nCN_default =\n" \
		      "distinguished_name = dn")
}

cert() {
    local cert=$1; shift
    local exts=$1; shift

    stderr_onerror \
        openssl x509 -req -"${OPENSSL_SIGALG}" -out "${cert}.pem" \
            -extfile <(printf "%s\n" "$exts") "$@"
}

genroot() {
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local bcon="basicConstraints = critical,CA:true"
    local ku="keyUsage = keyCertSign,cRLSign"
    local skid="subjectKeyIdentifier = hash"
    local akid="authorityKeyIdentifier = keyid"

    exts=$(printf "%s\n%s\n%s\n" "$bcon" "$ku" "$skid" "$akid")
    for eku in "$@"
    do
        exts=$(printf "%s\nextendedKeyUsage = %s\n" "$exts" "$eku")
    done
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
       cert "$cert" "$exts" -signkey "${key}.pem" -set_serial 1 -days "${DAYS}"
}

genca() {
    local OPTIND=1
    local purpose=

    while getopts p:c: o
    do
        case $o in
        p) purpose="$OPTARG";;
        c) certpol="$OPTARG";;
        *) echo "Usage: $0 genca [-p EKU][-c policyoid] cn keyname certname cakeyname cacertname" >&2
           return 1;;
        esac
    done

    shift $((OPTIND - 1))
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local cacert=$1; shift
    local bcon="basicConstraints = critical,CA:true"
    local ku="keyUsage = keyCertSign,cRLSign"
    local skid="subjectKeyIdentifier = hash"
    local akid="authorityKeyIdentifier = keyid"

    exts=$(printf "%s\n%s\n%s\n" "$bcon" "$ku" "$skid" "$akid")
    if [ -n "$purpose" ]; then
        exts=$(printf "%s\nextendedKeyUsage = %s\n" "$exts" "$purpose")
    fi
    if [ -n "$NC" ]; then
        exts=$(printf "%s\nnameConstraints = %s\n" "$exts" "$NC")
    fi
    if [ -n "$certpol" ]; then
        exts=$(printf "%s\ncertificatePolicies = %s\n" "$exts" "$certpol")
    fi

    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
        cert "$cert" "$exts" -CA "${cacert}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
}

gen_nonbc_ca() {
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local cacert=$1; shift
    local skid="subjectKeyIdentifier = hash"
    local akid="authorityKeyIdentifier = keyid"

    exts=$(printf "%s\n%s\n%s\n" "$skid" "$akid")
    exts=$(printf "%s\nkeyUsage = %s\n" "$exts" "keyCertSign, cRLSign")
    for eku in "$@"
    do
        exts=$(printf "%s\nextendedKeyUsage = %s\n" "$exts" "$eku")
    done
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
        cert "$cert" "$exts" -CA "${cacert}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}"
}

# Usage: $0 genpc keyname certname eekeyname eecertname pcext1 pcext2 ...
#
# Note: takes csr on stdin, so must be used with $0 req like this:
#
# $0 req keyname dn | $0 genpc keyname certname eekeyname eecertname pcext ...
genpc() {
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer:always" \
	    "basicConstraints = CA:false" \
	    "proxyCertInfo = critical, @pcexts";
           echo "[pcexts]";
           for x in "$@"; do echo $x; done)
    cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	 -set_serial 2 -days "${DAYS}"
}

geneeconfig() {
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift
    local conf=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n" \
        "subjectKeyIdentifier = hash" \
        "authorityKeyIdentifier = keyid" \
        "basicConstraints = CA:false"; \
        echo "$conf")

    cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
        -set_serial 2 -days "${DAYS}"
}

# Usage: $0 geneealt keyname certname cakeyname cacertname alt1 alt2 ...
#
# Note: takes csr on stdin, so must be used with $0 req like this:
#
# $0 req keyname dn | $0 geneealt keyname certname cakeyname cacertname alt ...
geneealt() {
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift

    conf=$(echo "subjectAltName = @alts"
           echo "[alts]";
           for x in "$@"; do echo "$x"; done)

    geneeconfig $key $cert $cakey $ca "$conf"
}

genee() {
    local OPTIND=1
    local purpose=serverAuth
    local ku=

    while getopts p:k: o
    do
        case $o in
        p) purpose="$OPTARG";;
        k) ku="keyUsage = $OPTARG";;
        *) echo "Usage: $0 genee [-k KU] [-p EKU] cn keyname certname cakeyname cacertname" >&2
           return 1;;
        esac
    done

    shift $((OPTIND - 1))
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
            "$ku" \
	    "extendedKeyUsage = $purpose" \
	    "subjectAltName = @alts" "DNS=${cn}")
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
	cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
}

geneeextra() {
    local OPTIND=1
    local purpose=serverAuth

    while getopts p: o
    do
        case $o in
        p) purpose="$OPTARG";;
        *) echo "Usage: $0 geneeextra [-p EKU] cn keyname certname cakeyname cacertname extraext" >&2
           return 1;;
        esac
    done

    shift $((OPTIND - 1))
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift
    local extraext=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
	    "extendedKeyUsage = $purpose" \
	    "subjectAltName = @alts"\
	    "$extraext" "DNS=${cn}")
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
	cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
}

geneenocsr() {
    local OPTIND=1
    local purpose=serverAuth

    while getopts p: o
    do
        case $o in
        p) purpose="$OPTARG";;
        *) echo "Usage: $0 geneenocsr [-p EKU] cn certname cakeyname cacertname" >&2
           return 1;;
        esac
    done

    shift $((OPTIND - 1))
    local cn=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
	    "extendedKeyUsage = $purpose" \
	    "subjectAltName = @alts" "DNS=${cn}")
	cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
}

genss() {
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier   = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
	    "extendedKeyUsage = serverAuth" \
	    "subjectAltName = @alts" "DNS=${cn}")
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
        cert "$cert" "$exts" -signkey "${key}.pem" \
            -set_serial 1 -days "${DAYS}" "$@"
}

gennocn() {
    local key=$1; shift
    local cert=$1; shift

    csr=$(req_nocn "$key") || return 1
    echo "$csr" |
	cert "$cert" "" -signkey "${key}.pem" -set_serial 1 -days -1 "$@"
}

genct() {
    local OPTIND=1
    local purpose=serverAuth

    while getopts p: o
    do
        case $o in
        p) purpose="$OPTARG";;
        *) echo "Usage: $0 genct [-p EKU] cn keyname certname cakeyname cacertname ctlogkey" >&2
           return 1;;
        esac
    done

    shift $((OPTIND - 1))
    local cn=$1; shift
    local key=$1; shift
    local cert=$1; shift
    local cakey=$1; shift
    local ca=$1; shift
    local logkey=$1; shift

    exts=$(printf "%s\n%s\n%s\n%s\n%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
	    "extendedKeyUsage = $purpose" \
            "1.3.6.1.4.1.11129.2.4.3 = critical,ASN1:NULL"\
	    "subjectAltName = @alts" "DNS=${cn}")
    csr=$(req "$key" "CN = $cn") || return 1
    echo "$csr" |
	cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
    cat ${cert}.pem ${ca}.pem > ${cert}-chain.pem
    go run github.com/google/certificate-transparency-go/ctutil/sctgen \
       --log_private_key ${logkey}.pem \
       --timestamp="2020-01-01T00:00:00Z" \
       --cert_chain ${cert}-chain.pem \
       --tls_out ${cert}.tlssct
    rm ${cert}-chain.pem
    filesize=$(wc -c <${cert}.tlssct)
    exts=$(printf "%s\n%s\n%s\n%s\n%s%04X%04X%s\n%s\n[alts]\n%s\n" \
	    "subjectKeyIdentifier = hash" \
	    "authorityKeyIdentifier = keyid, issuer" \
	    "basicConstraints = CA:false" \
	    "extendedKeyUsage = $purpose" \
	    "1.3.6.1.4.1.11129.2.4.2 = ASN1:FORMAT:HEX,OCT:" $((filesize+2)) $filesize `xxd -p ${cert}.tlssct | tr -d '\n'` \
	    "subjectAltName = @alts" "DNS=${cn}")
    echo "$csr" |
	cert "$cert" "$exts" -CA "${ca}.pem" -CAkey "${cakey}.pem" \
	    -set_serial 2 -days "${DAYS}" "$@"
}

"$@"
