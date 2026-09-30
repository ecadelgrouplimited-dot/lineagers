# Rust API

The full API reference is generated from the source on **[docs.rs/lineage-rs](https://docs.rs/lineage-rs)**. This page is a map of the parts you'll use most.

## `lineage::guard`

| Item | |
|---|---|
| `Guard::create(path, key, agent_id, policy)` | New agent; fails if the log exists |
| `Guard::open(path, key)` | Verify and replay an existing agent |
| `guard.request(tool, input, cost) -> Decision` | Ask permission. `cost: Option<u64>` can raise the charge, never lower it |
| `guard.approve(action_id, approver)` / `approve_with_note(action_id, approver, note)` | Approve a pending action; checks re-run |
| `guard.reject(action_id, approver, reason, scar: Option<Severity>)` | Reject a pending action |
| `guard.report(action_id, Outcome)` | `Outcome::success(..)`, `failure(..)`, `harmful(..)` |
| `guard.scar(severity, reason, action_id)` | Scar from an external monitor |
| `guard.terminate(reason, by)` | Kill switch; permanent |
| `guard.status() -> AgentStatus` | Budget, scars, counts, head, public key |
| `guard.action(id) -> Option<&Action>` | Includes the recorded `input` |
| `guard.head() -> Checkpoint`, `guard.records()` | For publishing checkpoints and auditing |
| `Policy::new(budget).allow(tool, rule).allow_unlisted(rule).rate_limit(n, secs).scar_limit(n)` | Builder |
| `ToolRule::cost(n).with_approval().with_max_calls(n)` | Builder |
| `Decision`, `DenyReason`, `Severity`, `OutcomeStatus`, `ActionStatus`, `GuardError` | |

## `lineage::audit`

| Item | |
|---|---|
| `AuditKey::generate()`, `load(path)`, `load_or_create(path)`, `save(path)`, `from_secret_hex(hex)` | Signing keys; not `Clone` |
| `key.public_key_hex()` | |
| `AuditLog::create(path, key, log_id)`, `open(path, key)`, `open_or_create(...)` | `open` verifies first and refuses corrupt logs |
| `log.append(actor, kind, payload) -> Record` | Flushed to disk before it returns |
| `log.head() -> Checkpoint`, `log.records()` | |
| `audit::verify_file(path, &VerifyOptions) -> VerifyReport` | `VerifyOptions { public_key, checkpoint }` |
| `audit::verify_records_with(&records, &options)` | Verify records you fetched over the API |
| `audit::read_records(path)` | Parse without verifying |
| `AuditError` | `Io`, `Key`, `Corrupt`, `Locked`, `AlreadyExists`, `KeyMismatch` |

## Errors

`GuardError` wraps `AuditError` and adds `InvalidLog`, `UnknownAction`, `InvalidState { action_id, status }`, and `AlreadyTerminated`. Both implement `std::error::Error`, so `?` works in functions returning `Box<dyn Error>`.

## Threads and processes

A `Guard` or `AuditLog` holds an exclusive OS lock on its file. Share one across threads behind a `Mutex`, and use one process per log. The guard server is the multi-process answer: one server, many agents, and HTTP for everyone else.
