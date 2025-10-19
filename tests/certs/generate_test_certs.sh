#!/bin/bash
# Generate self-signed certificates for TLS testing

set -e

# Directory to store generated certificates
CERT_DIR="."

# Generate CA private key
openssl genrsa -out "$CERT_DIR/ca-key.pem" 2048

# Generate CA certificate
openssl req -new -x509 -days 3650 -key "$CERT_DIR/ca-key.pem" \
    -out "$CERT_DIR/ca-cert.pem" \
    -subj "/C=US/ST=Test/L=Test/O=Test CA/CN=Test CA"

# Generate server private key
openssl genrsa -out "$CERT_DIR/server-key.pem" 2048

# Generate server certificate signing request
openssl req -new -key "$CERT_DIR/server-key.pem" \
    -out "$CERT_DIR/server.csr" \
    -subj "/C=US/ST=Test/L=Test/O=Test Server/CN=localhost"

# Create extensions file for SAN
cat > "$CERT_DIR/server-ext.cnf" << EOF
subjectAltName = DNS:localhost,IP:127.0.0.1
EOF

# Sign server certificate with CA
openssl x509 -req -days 3650 \
    -in "$CERT_DIR/server.csr" \
    -CA "$CERT_DIR/ca-cert.pem" \
    -CAkey "$CERT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$CERT_DIR/server-cert.pem" \
    -extfile "$CERT_DIR/server-ext.cnf"

# Generate client private key (for mutual TLS)
openssl genrsa -out "$CERT_DIR/client-key.pem" 2048

# Generate client certificate signing request
openssl req -new -key "$CERT_DIR/client-key.pem" \
    -out "$CERT_DIR/client.csr" \
    -subj "/C=US/ST=Test/L=Test/O=Test Client/CN=Test Client"

# Sign client certificate with CA
openssl x509 -req -days 3650 \
    -in "$CERT_DIR/client.csr" \
    -CA "$CERT_DIR/ca-cert.pem" \
    -CAkey "$CERT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$CERT_DIR/client-cert.pem"

# Clean up CSR files and extensions
rm -f "$CERT_DIR/server.csr" "$CERT_DIR/client.csr" "$CERT_DIR/server-ext.cnf"
rm -f "$CERT_DIR/ca-cert.srl"
