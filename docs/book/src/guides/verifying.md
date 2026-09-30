# Verifying logs as an auditor

You don't have to trust the operator of a guard server to check what its agents did. You need three things:

1. **The log file**, `agents/<id>.jsonl`, or its records from `GET /v1/agents/:id/log`.
2. **The public key.** Get it from `GET /v1/public-key`, or from the operator through a channel you trust.
3. **Ideally, a checkpoint** (`seq:hash`) you obtained earlier, independently of the log.

```sh
lineage audit verify support-bot.jsonl \
  --public-key 13f5fbff904c6f5306fbdeb2fbfc97f5d98df2e2716852bdf5d771715deacaef \
  --checkpoint 69:986e3da74e6c81ae62956fedd1151d7ab85fae0568110a2748eaa76d557170a8
```

```text
OK  70 records, log support-bot
    head       69:986e3da74e6c81ae62956fedd1151d7ab85fae0568110a2748eaa76d557170a8
    public key 13f5fbff904c6f5306fbdeb2fbfc97f5d98df2e2716852bdf5d771715deacaef
```

If anything was changed, it fails and points at the first bad record. The exit code is 1:

```text
FAILED  line 12 (seq 11): hash does not match record content
```

## What each check proves

| You provide | A passing result proves |
|---|---|
| Nothing but the file | The file is internally consistent. It says nothing about who wrote it: anyone can make a self-consistent log with their own key. The CLI warns about this |
| `--public-key` | Every record was signed by that key, in this order, with nothing inserted, removed, or changed |
| `--public-key` and `--checkpoint` | All of the above, **and** the history up to the checkpoint is exactly what it was when you got the checkpoint: not truncated, not rewritten |

## Reading the log

```sh
lineage audit show support-bot.jsonl          # a table
lineage audit show support-bot.jsonl --json   # one JSON record per line
```

Each guard log starts with `genesis` and `policy`, followed by one record per step: `action_requested`, `approval_required`, `action_allowed` (with `approved_by` and `note` when a human or reviewer approved), `action_denied`, `action_rejected`, `outcome`, `scar`, and finally `terminated` if the agent was stopped. The fields of each kind are listed in [Log format](../reference/log-format.md).

## Machine-readable reports

```sh
lineage audit verify agent.jsonl --public-key <key> --json
```

```json
{
  "ok": true,
  "records": 70,
  "log_id": "support-bot",
  "public_key": "13f5fb…",
  "key_trusted": true,
  "head": { "seq": 69, "hash": "986e3d…" },
  "failure": null
}
```

## Verifying without Lineage

The format is simple and fully specified, so you can write a verifier in any language with SHA-256 and Ed25519. See [Log format](../reference/log-format.md).
