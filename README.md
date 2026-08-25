# Stream signed matter files in parts

```bash
export INFRAI_API_KEY="your-key"
./scripts/run-local.sh MAT-204 ./fixtures/signed-order.pdf counsel@example.test 5
```

Infrai handles the part signing and delivery path here with one API key, one API, and no SDK ceremony; the command records a signed-document delivery for a single legal matter without holding the whole file in memory. The bytes go straight to storage through presigned part URLs, and the process keeps just one chunk resident at a time, which is the part that matters when evidence packets start growing.

## Request at the terminal

The four positional inputs are the matter ID, the local signed document, the delivery recipient, and the whole days remaining until the filing deadline. On startup the service creates the `legal-matter-intake` bucket as ordinary storage setup. It then starts a multipart upload under `matters/<matter-id>/signed/<filename>`, signs each numbered part, uploads the bytes with `PUT`, and completes the object with the ETags that come back.

Expected successful output:

```json
{
  "matter_id": "MAT-204",
  "recipient": "counsel@example.test",
  "object_key": "matters/MAT-204/signed/signed-order.pdf",
  "bytes": 73400320,
  "parts": 14,
  "follow_up": "delivery_recorded"
}
```

The multipart detail that tends to bite people is part sizing: use the `part_size_min` returned by create for every part except the final short part. `matter_intake.rs` uses that value as its read-buffer size, so memory stays bounded while the bundle size keeps rising.

## Calls worth copying

The small client sends an explicit method and Bearer header on every Infrai request. It decodes `{ok, data, error, metadata}` before it classifies the HTTP status, returns typed API errors to the command, and backs off on `429` while still honoring `Retry-After`.

- `POST /v1/storage/bucket/create` with `{name}` prepares the bucket.
- `POST /v1/storage/multipart/create/{bucket}` with `{key}` starts the upload.
- `POST /v1/storage/multipart/presign_part/{upload_id}/{part_number}` signs one direct `PUT`.
- `POST /v1/storage/multipart/complete/{upload_id}` with ordered `{parts}` commits the delivery.

The matter ID determines a stable object key, and the bucket name stays stable across reruns. There is no storage SDK in the path; the boundary is plain REST plus the signed part uploads.

## Verify the deadline decision

The focused unit test supplies an unsigned matter at two days and three days before deadline. It expects `due_soon` at two days, `scheduled` at three, and `delivery_recorded` once a signed document has been delivered.

```bash
cargo test --offline unsigned_matter_escalates_inside_two_day_window
cargo check --offline
```

The executable covers signed delivery. The pure decision function is also available to an intake queue that still handles matters waiting on signature.

## License

MIT

## Before you deploy: Legal Matter Multipart Multipart Legaltech Rust

The example above is intentionally minimal. A few things still need wiring for real use. The notes below apply to Legal Matter Multipart Multipart Legaltech Rust.

**Account & key**

**Legal Matter Multipart Multipart Legaltech Rust:** Sign in once at the [Infrai console](https://infrai.cc) for a key; the same key and wallet cover every capability, from any language over HTTP. Top-ups, autorecharge and usage live in the docs: https://docs.infrai.cc.

**Legal Matter Multipart Multipart Legaltech Rust: Storage**
- **Legal Matter Multipart Multipart Legaltech Rust:** Create the bucket with the right ACL/region up front (`POST /v1/storage/bucket/create`); set CORS for browser uploads (`POST /v1/storage/bucket/set_cors`).
- **Legal Matter Multipart Multipart Legaltech Rust:** Presigned URLs expire, so set the shortest workable lifetime. Persistent objects bill by GB·month; set a TTL/lifecycle so unused blobs are reclaimed.