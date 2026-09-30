# Log format

This page specifies the audit log precisely enough to write an independent verifier. Version: `lineage-audit-v1`.

## File

A log is a UTF-8 text file with one JSON object (a **record**) per line, in order. Nothing else is in the file.

## Record

| Field | Type | |
|---|---|---|
| `seq` | integer | Position in the log, starting at 0. Must equal the line number minus one |
| `timestamp` | string | RFC 3339, UTC, microsecond precision, ending in `Z`, e.g. `2026-09-30T08:27:35.423031Z` |
| `actor` | string | Who caused the record (for guard logs, the agent ID) |
| `kind` | string | Record type |
| `payload` | any JSON | Record data |
| `prev_hash` | string | `hash` of the previous record, as 64 lowercase hex characters; 64 zeros for the first record |
| `hash` | string | SHA-256 of the record's content (below), as 64 lowercase hex characters |
| `signature` | string | Ed25519 signature of the 32 raw hash bytes, as 128 lowercase hex characters |

## Hash

```text
content = {"seq", "timestamp", "actor", "kind", "payload", "prev_hash"}   (the record minus hash and signature)
hash    = SHA-256( "lineage-audit-v1\n" || canonical_json(content) )
```

`canonical_json` is compact JSON with **object keys sorted** by byte-wise comparison of their UTF-8 encodings, at every depth, and no whitespace. Strings are encoded as standard JSON: `"` and `\` escaped, control characters escaped (`\n`, `\t`, … or `\u00XX`), everything else, including non-ASCII and `/`, left as raw UTF-8. Integers are written in decimal. Floating-point numbers use the shortest representation that round-trips; the guard itself only writes integers and strings.

## Signature

`signature = Ed25519-sign(private_key, hash_bytes)`, where `hash_bytes` are the 32 raw bytes of the hash, not its hex string.

## Genesis

The first record (`seq` 0) has `kind` `"genesis"`, `prev_hash` of 64 zeros, and payload:

```json
{"log_id": "<log id>", "public_key": "<64 hex characters: the Ed25519 public key>"}
```

No other record may have kind `genesis`.

## Verification

A log is valid against a trusted public key if:

1. The genesis record's `public_key` equals the trusted key (case-insensitive hex).
2. For every record `i` (0-based line index):
   - `seq == i`;
   - `prev_hash` equals the previous record's `hash` (64 zeros for `i = 0`);
   - `kind` is not `genesis` unless `i = 0`;
   - `hash` equals the recomputed hash of its content;
   - `signature` is a valid Ed25519 signature of the hash bytes under the trusted key.
3. If a checkpoint `(seq, hash)` is given, the record at that `seq` exists and has that `hash`.

## Reference verifier (Python)

Tested against logs written by Lineage: it accepts valid ones, and rejects edited, truncated, and wrongly-keyed ones with the same results as `lineage audit verify`.

```python
"""Independent verifier for Lineage audit logs. Needs: pip install cryptography"""
import hashlib, json, sys
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

DOMAIN = b"lineage-audit-v1\n"
ZERO = "0" * 64

def canonical(value):
    # Sorted keys, no whitespace, non-ASCII kept as UTF-8: matches the Rust encoder.
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)

def verify(path, trusted_key_hex, checkpoint=None):
    key = Ed25519PublicKey.from_public_bytes(bytes.fromhex(trusted_key_hex))
    prev, records = ZERO, [json.loads(line) for line in open(path, encoding="utf-8")]
    if records[0]["kind"] != "genesis" or records[0]["payload"]["public_key"] != trusted_key_hex.lower():
        return f"line 1: not a genesis record for the trusted key"
    for i, r in enumerate(records):
        if r["seq"] != i or r["prev_hash"] != prev or (i > 0 and r["kind"] == "genesis"):
            return f"line {i + 1}: broken chain"
        content = {k: r[k] for k in ("seq", "timestamp", "actor", "kind", "payload", "prev_hash")}
        digest = hashlib.sha256(DOMAIN + canonical(content).encode("utf-8")).digest()
        if digest.hex() != r["hash"]:
            return f"line {i + 1}: hash does not match record content"
        try:
            key.verify(bytes.fromhex(r["signature"]), digest)
        except Exception:
            return f"line {i + 1}: signature is invalid"
        prev = r["hash"]
    if checkpoint:
        seq, h = checkpoint.split(":")
        if int(seq) >= len(records) or records[int(seq)]["hash"] != h:
            return f"checkpoint {seq} not found (truncated or rewritten)"
    return f"OK {len(records)} records"

if __name__ == "__main__":
    print(verify(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None))
```

```sh
pip install cryptography
python3 verify_lineage_log.py agent.jsonl <public-key-hex> [<seq>:<hash>]
```

## Guard record kinds

Guard logs use these kinds after `genesis`:

| `kind` | Payload |
|---|---|
| `policy` | `{"policy": <policy>}`. Always `seq` 1 |
| `action_requested` | `{"action_id", "tool", "input", "cost"}` |
| `approval_required` | `{"action_id"}` |
| `action_allowed` | `{"action_id", "approved_by": string or null, "note"?: string}` |
| `action_denied` | `{"action_id", "reason": <deny reason>}` |
| `action_rejected` | `{"action_id", "by", "reason"}` |
| `outcome` | `{"action_id", "status": "success" or "failure" or "harmful", "detail"}` |
| `scar` | `{"severity", "reason", "action_id": string or null}` |
| `terminated` | `{"reason", "by"}` |

A deny reason is `{"code": ..., ...fields}`. The codes are listed in [Decisions](../concepts/decisions.md#deny-reasons).
