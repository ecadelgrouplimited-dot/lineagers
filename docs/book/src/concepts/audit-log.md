# The audit log

Every agent's history is an audit log: a JSON Lines file where each record is chained to the previous one by its SHA-256 hash and signed with Ed25519. The guard writes through `lineage::audit::AuditLog`, which you can also use directly for any history that must be tamper-evident.

```json
{"seq":14,"timestamp":"2026-09-30T08:41:12.508772Z","actor":"ap-clerk","kind":"action_requested",
 "payload":{"action_id":"act-13","tool":"pay_invoice","cost":1250,"input":{"invoice_id":"INV-1001", "...": "..."}},
 "prev_hash":"3f9a…","hash":"b41c…","signature":"9e0d…"}
```

## What it guarantees

When a log verifies against a trusted public key:

| Tampering | Detected by |
|---|---|
| Any field of any record changed | The record's hash no longer matches its content |
| Content changed *and* the hash recomputed | The signature is invalid without the private key |
| A record deleted, inserted, or reordered | `seq` and `prev_hash` no longer line up |
| A whole log forged with a different key | The log declares a key the verifier doesn't trust |
| Records cut off the end | Only against a **checkpoint** published earlier (see below) |

A log that fails verification is never appended to: `AuditLog::open` refuses it, and the guard server quarantines that agent while the others keep running.

## Checkpoints

A truncated log is still internally consistent: its remaining records verify. To catch truncation, and to catch the signing key's holder rewriting history, publish **checkpoints**, the `seq:hash` of the latest record, somewhere the writer can't change. Good places are a ticket, a chat message, another system's database, or a transparency log. Verifying against a checkpoint proves that the history up to that point is unchanged:

```sh
lineage audit verify agent.jsonl --public-key 13f5fb… --checkpoint 69:986e3da7…
```

Get the current checkpoint from `guard.head()`, `AuditLog::head()`, `GET /v1/agents/:id/verify` or `/log`, or the last line of `lineage audit verify`.

## Keys

- The **private key** (`audit.key`, hex, mode 0600) signs. Whoever holds it can write valid records, so keep it on the guard's host and away from agents.
- The **public key** verifies. Publish it: `lineage audit pubkey audit.key`, or `GET /v1/public-key`.
- Each log's first (`genesis`) record names its public key. Verifying *without* a trusted key only proves the log is self-consistent; the CLI warns you when that's all you've checked.

## Using the log directly

```rust
use lineage::audit::{AuditKey, AuditLog, VerifyOptions, verify_file};
use serde_json::json;

let key = AuditKey::load_or_create("audit.key")?;
let mut log = AuditLog::open_or_create("deploys.jsonl", key, "deploys")?;
log.append("ci-bot", "deploy", json!({"service": "api", "version": "1.4.2"}))?;
let checkpoint = log.head();   // publish this

let report = verify_file("deploys.jsonl", &VerifyOptions {
    public_key: Some(log.public_key_hex()),
    checkpoint: Some(checkpoint),
});
assert!(report.ok);
```

`append` returns only after the record is flushed to disk. One process may write a log at a time; a second writer gets `AuditError::Locked`.

The exact record format, including how hashes and signatures are computed, is specified in [Log format](../reference/log-format.md), so you can write an independent verifier in any language.
