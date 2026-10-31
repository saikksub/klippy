#!/bin/sh
# Creates a self-signed "Klippy Development" code-signing identity in the
# login keychain. Signing every local build with it keeps Klippy's designated
# requirement stable (ad-hoc signatures change every build). Keychain still
# asks again after each rebuild: without an Apple Team ID, an item's partition
# list can only name a build's code hash.
set -eu

NAME="Klippy Development"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
    echo "\"$NAME\" already exists."
    exit 0
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

# macOS's own LibreSSL: its PKCS#12 output is what `security import` reads.
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -config "$TMP/cert.cnf" -keyout "$TMP/key.pem" -out "$TMP/cert.pem" 2>/dev/null
/usr/bin/openssl pkcs12 -export -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -name "$NAME" -passout pass:klippy -out "$TMP/identity.p12"

# Only codesign may use the private key.
security import "$TMP/identity.p12" -k "$KEYCHAIN" -P klippy -T /usr/bin/codesign
# Trust it for code signing (macOS asks for your password once).
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$TMP/cert.pem"

echo "Created \"$NAME\". Run 'make app' to sign with it."
