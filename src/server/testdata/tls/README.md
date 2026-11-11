# Test certificates for `server::listen` (roadmap 107)

**Throwaway keys, for the tests only. Never use them anywhere else.**

- `ca.pem`: a test CA. Its key was deleted after signing the two leaves.
- `a.pem` / `a.key`, `b.pem` / `b.key`: two `localhost` (and `127.0.0.1`) server certificates
  signed by it, P-256 keys in PKCS#8, valid for 100 years. The tests swap from `a` to `b` to
  check the reload.

The leaves can't be self-signed: rustls refuses a CA certificate as a server's
(`CaUsedAsEndEntity`), so they carry `basicConstraints=CA:FALSE` and
`extendedKeyUsage=serverAuth`. To regenerate them, from this directory:

```bash
W=$(mktemp -d)
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$W/ca.key"
openssl req -x509 -new -key "$W/ca.key" -subj "/CN=Moonlight test CA (throwaway)" -days 36500 \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign,cRLSign" \
  -out ca.pem
printf '%s\n' "basicConstraints=critical,CA:FALSE" "keyUsage=critical,digitalSignature" \
  "extendedKeyUsage=serverAuth" "subjectAltName=DNS:localhost,IP:127.0.0.1" > "$W/ext"
for n in a b; do
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$n.key"
  openssl req -new -key "$n.key" -subj "/CN=localhost $n" -out "$W/$n.csr"
  openssl x509 -req -in "$W/$n.csr" -CA ca.pem -CAkey "$W/ca.key" -CAcreateserial -days 36500 \
    -extfile "$W/ext" -out "$n.pem"
done
rm -rf "$W" ca.srl
```
