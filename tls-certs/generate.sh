#!/usr/bin/env bash

# requirements: openssl installed
# Public/private key algorithm: EC P-256
# Signature signing algorithm: SHA-256

TARGET_DIR=tls-certs
CA_KEY_PAIR_PATH=${TARGET_DIR}/rootCA.key
CA_CERT_PATH=${TARGET_DIR}/rootCA.crt
SERVER_KEY_PAIR_PATH=${TARGET_DIR}/server.key
SERVER_KEY_PAIR_PKCS8_PATH=${TARGET_DIR}/server.pkcs8.key
SERVER_CRS_PATH=${TARGET_DIR}/server.crs
SERVER_CERT_PATH=${TARGET_DIR}/server.crt
SERVER_COMMON_NAME=rcmd-server
CLIENT_KEY_PAIR_PATH=${TARGET_DIR}/client.key
CLIENT_KEY_PAIR_PKCS8_PATH=${TARGET_DIR}/client.pkcs8.key
CLIENT_KEY_PAIR_CERT_PATH=${TARGET_DIR}/clientKeyCert.pem
CLIENT_CRS_PATH=${TARGET_DIR}/client.crs
CLIENT_CERT_PATH=${TARGET_DIR}/client.crt
CLIENT_COMMON_NAME=rcmd-client


# == root CA
# generate CA private/public key pair
openssl ecparam -genkey -name prime256v1 -noout -out $CA_KEY_PAIR_PATH
# generate CA root certificate (self-signed)
openssl req -x509 -new -nodes -key $CA_KEY_PAIR_PATH -sha256 -days 365 -out $CA_CERT_PATH \
    -subj "/O=Jannes Root CA" 

# == server cert
# generate private/public key pair
openssl ecparam -genkey -name prime256v1 -noout -out $SERVER_KEY_PAIR_PATH
# generate CSR (certificate signing request)
openssl req -new -key $SERVER_KEY_PAIR_PATH -out $SERVER_CRS_PATH \
    -subj "/O=Jannes Server/CN=${SERVER_COMMON_NAME}" 
# generate TLS cert from CSR, key, CA root key
openssl x509 -req -in $SERVER_CRS_PATH \
    -CA $CA_CERT_PATH -CAkey $CA_KEY_PAIR_PATH -CAcreateserial \
    -extfile <(printf "subjectAltName=DNS:localhost,DNS:${SERVER_COMMON_NAME}") \
    -out $SERVER_CERT_PATH -days 365 -sha256

# == client cert
# generate private/public key pair
openssl ecparam -genkey -name prime256v1 -noout -out $CLIENT_KEY_PAIR_PATH
# generate CSR (certificate signing request)
openssl req -new -key $CLIENT_KEY_PAIR_PATH -out $CLIENT_CRS_PATH \
    -subj "/O=Jannes Client/CN=${CLIENT_COMMON_NAME}" 
# generate TLS cert from CSR, key, CA root key
openssl x509 -req -in $CLIENT_CRS_PATH \
    -CA $CA_CERT_PATH -CAkey $CA_KEY_PAIR_PATH -CAcreateserial \
    -extfile <(printf "subjectAltName=DNS:localhost,DNS:${CLIENT_COMMON_NAME}") \
    -out $CLIENT_CERT_PATH -days 365 -sha256

# == convert server key and client key/cert into expected formats by server/client
# convert server key pair to PKCS8 for import by rocket
openssl pkcs8 -topk8 -nocrypt -in $SERVER_KEY_PAIR_PATH -out $SERVER_KEY_PAIR_PKCS8_PATH
# convert client key pair to PKCS8 for constructing identity pem
openssl pkcs8 -topk8 -nocrypt -in $CLIENT_KEY_PAIR_PATH -out $CLIENT_KEY_PAIR_PKCS8_PATH
# create client identity pem from concatenated cert and key for import by reqwest
cat $CLIENT_KEY_PAIR_PKCS8_PATH $CLIENT_CERT_PATH > $CLIENT_KEY_PAIR_CERT_PATH