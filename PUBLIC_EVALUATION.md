# STATICZERO™ public evaluation

The public evaluation is available to people and machine clients without an
account, installation, upload or customer data.

## Browser

Open https://evaluate.staticzero.online/ and select **Run the proof**. The page
displays the measured result, creates a signed receipt, and verifies that
receipt with the published public key.

## Machine client

Discover the stable public contract:

```text
GET https://evaluate.staticzero.online/api
GET https://evaluate.staticzero.online/openapi.json
GET https://evaluate.staticzero.online/api/public-key
GET https://evaluate.staticzero.online/api/verification-spec
```

Start a fixed, synthetic proof run:

```sh
curl -fsS -X POST https://evaluate.staticzero.online/api/run \
  -H 'Accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{}'
```

To request server-assisted receipt verification, send the returned receipt as
the JSON value of `receipt`:

```text
POST https://evaluate.staticzero.online/api/verify
Content-Type: application/json

{"receipt": { ... }}
```

The verification specification and public JWK enable independent ES256
verification. `GET /api/run` never starts execution. Capacity limits return
HTTP 429 with `Retry-After`; clients must wait before retrying.

The workload is fixed and synthetic. The receipt is payload-blind and exposes
no proprietary implementation, signing key or customer data.
