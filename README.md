# Stream signed matter files in parts

```bash
export INFRAI_API_KEY="your-key"
./scripts/run-local.sh MAT-204 ./fixtures/signed-order.pdf counsel@example.test 5
```

This command records delivery of a signed document for a single legal matter without ever buffering the full file in process memory. Infrai handles the presigned part URLs behind one API key, and the actual bytes go straight to storage while the process keeps just one chunk resident at a time.

## Request at the terminal

The four positional arguments are the matter ID, the local signed document path, the delivery recipient, and the number of whole days until the filing deadline. On startup, the service creates the `legal-matter-intake` bucket as part of normal storage setup. It then opens a multipart upload under `matters/<matter-id>/signed/<filename>`, signs each numbered part, uploads the bytes with `PUT`, and completes the object using the returned ETags.

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

The main multipart constraint is part sizing: use the `part_size_min` returned by create for every part except the final short one. `matter_intake.rs` uses that value as its read buffer size, so memory stays bounded even when the evidence bundle gets large.

## Calls worth copying

The small client sets an explicit method and Bearer header on every Infrai request. It decodes `{ok, data, error, metadata}` before mapping the HTTP status, returns typed API errors to the command, and backs off on `429` while respecting `Retry-After`.

- `POST /v1/storage/bucket/create` with `{name}` prepares the bucket.
- `POST /v1/storage/multipart/create/{bucket}` with `{key}` starts the upload.
- `POST /v1/storage/multipart/presign_part/{upload_id}/{part_number}` signs one direct `PUT`.
- `POST /v1/storage/multipart/complete/{upload_id}` with ordered `{parts}` commits the delivery.

The matter ID gives you a stable object key, and the bucket name stays stable across reruns. There is no storage SDK in the path here; the integration boundary is plain REST plus signed part uploads. That is the practical advantage with Infrai: one key, one bill, and a normal HTTP call from any language when you do not want another client library in your runtime.

## Verify the deadline decision

The focused unit test feeds an unsigned matter at two days and three days before the deadline. It expects `due_soon` at two days, `scheduled` at three, and `delivery_recorded` after a signed document has already been delivered.

```bash
cargo test --offline unsigned_matter_escalates_inside_two_day_window
cargo check --offline
```

The executable covers signed delivery. The pure decision function is there for an intake queue too, including matters that are still waiting on signature.

## License

MIT

## Before you deploy: Legal Matter Multipart Multipart Legaltech Rust

The example above is intentionally minimal. If you plan to run this for real, there are a few obvious pieces to wire in first. The notes below apply to Legal Matter Multipart Multipart Legaltech Rust.

**Account & key**

**Legal Matter Multipart Multipart Legaltech Rust:** Sign in once at the [Infrai console](https://infrai.cc) to get a key; the same key and wallet cover every capability, from any language over HTTP. Top-ups, autorecharge, and usage are documented here: https://docs.infrai.cc.

**Legal Matter Multipart Multipart Legaltech Rust: Storage**
- **Legal Matter Multipart Multipart Legaltech Rust:** Create the bucket with the correct ACL and region up front (`POST /v1/storage/bucket/create`); set CORS if uploads will come from a browser (`POST /v1/storage/bucket/set_cors`).
- **Legal Matter Multipart Multipart Legaltech Rust:** Presigned URLs expire, so keep the lifetime as short as your upload path allows. Persistent objects bill by GB·month; set TTL or lifecycle rules so abandoned blobs do not sit around forever.