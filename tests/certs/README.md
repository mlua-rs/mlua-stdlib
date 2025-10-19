# TLS Test Certificates

This directory contains self-signed certificates used for testing TLS functionality.

## Contents

- `ca-cert.pem` - Certificate Authority certificate
- `ca-key.pem` - Certificate Authority private key
- `server-cert.pem` - Server certificate (signed by CA)
- `server-key.pem` - Server private key
- `client-cert.pem` - Client certificate for mutual TLS (signed by CA)
- `client-key.pem` - Client private key

## Regenerating Certificates

To regenerate these test certificates, run:

```sh
./tests/certs/generate_test_certs.sh
```

The script will create all necessary certificates with a 10-year validity period.

## Security Note

**These certificates are for testing purposes only and should never be used in production!**
