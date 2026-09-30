# Level 6: Proving what happened

<div class="lx-lesson">
<p><strong>You'll learn</strong> how anyone can verify an agent's history, and how tampering is caught.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_06_audit</code></p>
</div>

## The idea

{{#include ../diagrams/chain.html}}

Each record includes the previous record's hash, and is signed with the agent's Ed25519 key. With only the **public key**, anyone can check that no record was changed, removed or reordered. With a **checkpoint** (`seq:hash`) published earlier, somewhere the writer can't change, they can also prove nothing was cut off the end.

## The code

```rust
{{#include ../../../../examples/mastery_06_audit.rs}}
```

## Run it

```text
public key  95a2fd0cbac87cfa44b3e7660c61feb61b1dfb9ee63f5c90cf6f9781fdea3925
checkpoint  10:948cc8797ab8c691734c3ee59434cfaf3f450dc7b7f7b4e3a40955bedc096e36

original log                           OK      (11 records)
amount 310 edited to 31                FAILED  line 9 (seq 8): hash does not match record content
record 5 deleted                       FAILED  line 6 (seq 6): expected seq 5, found 6
last 3 records cut (key only)          OK      (8 records)
last 3 records cut (with checkpoint)   FAILED  log ends at seq 7 before checkpoint (truncated)
```

## What happened

| Tampering | Caught by |
|---|---|
| A payment amount changed | The record's hash no longer matches its content |
| A record deleted | The sequence numbers and `prev_hash` links no longer line up |
| The tail cut off | Only the checkpoint. A shorter chain is still internally valid |

That last row is why you **publish checkpoints**, for example to a ticket, another system, or a daily email. In production the guard server gives you one from `GET /v1/agents/:id/verify`.

## Try this

Verify the log from the command line, as an auditor would:

```sh
lineage audit verify mastery-data/06/agent.jsonl \
  --public-key "$(lineage audit pubkey mastery-data/06/audit.key)"
```

Then write your own verifier from the [Log format](../reference/log-format.md) spec. It's about forty lines of Python.

[Next: The guard server →](07-guard-server.md)
