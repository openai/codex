#!/bin/bash

CURLRC=~/testcase_curlrc

# Set up the routing needed for the simulation
/setup.sh

# The following variables are available for use:
# - ROLE contains the role of this execution context, client or server
# - SERVER_PARAMS contains user-supplied command line parameters
# - CLIENT_PARAMS contains user-supplied command line parameters

generate_outputs_http3() {
    for i in $REQUESTS
    do
        OUTFILE=$(basename $i)
        echo -e "--http3-only\n-o /downloads/$OUTFILE\n--url $i" >> $CURLRC
        echo "--next" >> $CURLRC
    done
    # Remove the last --next
    head -n -1 $CURLRC > $CURLRC.tmp
    mv $CURLRC.tmp $CURLRC 
}

dump_curlrc() {
    echo "Using curlrc:"
    cat $CURLRC
}

if [ "$ROLE" == "client" ]; then
    # Wait for the simulator to start up.
    echo "Waiting for simulator"
    /wait-for-it.sh sim:57832 -s -t 30
    echo "TESTCASE is $TESTCASE"
    rm -f $CURLRC 

    case "$TESTCASE" in
    "http3")
        echo -e "--verbose\n--parallel" >> $CURLRC
        generate_outputs_http3
        dump_curlrc
        SSL_CERT_FILE=/certs/ca.pem curl --config $CURLRC || exit 1
        exit 0
        ;;
    "handshake"|"transfer"|"retry"|"ipv6")
        HOSTNAME=none
        for req in $REQUESTS
        do
            OUTFILE=$(basename $req)
            if [ "$HOSTNAME" == "none" ]
            then
                HOSTNAME=$(printf "%s\n" "$req" | sed -ne 's,^https://\([^/:]*\).*,\1,p')
                HOSTPORT=$(printf "%s\n" "$req" | sed -ne 's,^https://[^:/]*:\([^/]*\).*,\1,p')
            fi
            echo -n "$OUTFILE " >> ./reqfile.txt
        done
        SSLKEYLOGFILE=/logs/keys.log SSL_CERT_FILE=/certs/ca.pem SSL_CERT_DIR=/certs quic-hq-interop $HOSTNAME $HOSTPORT ./reqfile.txt || exit 1
        exit 0
        ;;
    "resumption")
        for req in $REQUESTS
        do
            OUTFILE=$(basename $req)
            echo -n "$OUTFILE " > ./reqfile.txt
            HOSTNAME=$(printf "%s\n" "$req" | sed -ne 's,^https://\([^/:]*\).*,\1,p')
            HOSTPORT=$(printf "%s\n" "$req" | sed -ne 's,^https://[^:/]*:\([^/]*\).*,\1,p')
            SSL_SESSION_FILE=./session.db SSLKEYLOGFILE=/logs/keys.log SSL_CERT_FILE=/certs/ca.pem SSL_CERT_DIR=/certs quic-hq-interop $HOSTNAME $HOSTPORT ./reqfile.txt || exit 1
        done
        exit 0
        ;;
    "chacha20")
        for req in $REQUESTS
        do
            OUTFILE=$(basename $req)
            printf "%s " "$OUTFILE" >> ./reqfile.txt
            HOSTNAME=$(printf "%s\n" "$req" | sed -ne 's,^https://\([^/:]*\).*,\1,p')
            HOSTPORT=$(printf "%s\n" "$req" | sed -ne 's,^https://[^:/]*:\([^/]*\).*,\1,p')
        done
        SSL_CIPHER_SUITES=TLS_CHACHA20_POLY1305_SHA256 SSL_SESSION_FILE=./session.db SSLKEYLOGFILE=/logs/keys.log SSL_CERT_FILE=/certs/ca.pem SSL_CERT_DIR=/certs quic-hq-interop $HOSTNAME $HOSTPORT ./reqfile.txt || exit 1
        exit 0
        ;;
    *)
        echo "UNSUPPORTED TESTCASE $TESTCASE"
        exit 127
        ;;
    esac
elif [ "$ROLE" == "server" ]; then
    echo "TESTCASE is $TESTCASE"
    rm -f $CURLRC 
    case "$TESTCASE" in
    "handshake"|"transfer"|"ipv6")
        NO_ADDR_VALIDATE=yes SSLKEYLOGFILE=/logs/keys.log FILEPREFIX=/www quic-hq-interop-server 443 /certs/cert.pem /certs/priv.key
        ;;
    "retry")
        SSLKEYLOGFILE=/logs/keys.log FILEPREFIX=/www quic-hq-interop-server 443 /certs/cert.pem /certs/priv.key
        ;;
    "resumption")
        NO_ADDR_VALIDATE=yes SSLKEYLOGFILE=/logs/keys.log FILEPREFIX=/www quic-hq-interop-server 443 /certs/cert.pem /certs/priv.key
        ;;
    "http3")
        FILEPREFIX=/www/ SSLKEYLOGFILE=/logs/keys.log ossl-nghttp3-demo-server 443 /certs/cert.pem /certs/priv.key
        ;;
    "chacha20")
        SSL_CIPHER_SUITES=TLS_CHACHA20_POLY1305_SHA256 SSLKEYLOGFILE=/logs/keys.log FILEPREFIX=/www quic-hq-interop-server 443 /certs/cert.pem /certs/priv.key
        ;;
    *)
        echo "UNSUPPORTED TESTCASE $TESTCASE"
        exit 127
        ;;
    esac
else
    echo "Unknown ROLE $ROLE"
    exit 127
fi

