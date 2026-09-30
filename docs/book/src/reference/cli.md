# CLI

```text
lineage [COMMAND]

Commands:
  demo   Walk through the core principles (default)
  audit  Tamper-evident audit logs
```

## `lineage audit keygen <PATH>`

Generates an Ed25519 signing key and writes its secret, hex-encoded, to a new file with mode 0600. It refuses to overwrite an existing file, and prints the public key.

```sh
lineage audit keygen audit.key
# secret key written to audit.key
# public key: 5e84811112e054941689cb7a2744a29396db3183831789c386f210f2f41e9bd9
```

## `lineage audit pubkey <PATH>`

Prints the public key of a signing key file. This is what you give to anyone who will verify your logs.

## `lineage audit append <LOG> --key <KEY> --actor <ACTOR> --kind <KIND> [--payload <JSON>]`

Appends a signed record, creating the log if needed (its ID is the file name without extension), and prints the new head as `seq:hash`. Use it to record events from shell scripts, CI jobs, or cron:

```sh
lineage audit append deploys.jsonl --key audit.key --actor ci --kind deploy \
  --payload '{"service": "api", "version": "1.4.2"}'
# 3:9f952ddf6df5e24e68ea51262b5e41d6748ef84f9d9f374a0bdbcab75dd5e4a8
```

It refuses to append to a log that fails verification, or that was created with a different key.

## `lineage audit verify <LOG> [--public-key <HEX>] [--checkpoint <SEQ:HASH>] [--json]`

Verifies the hash chain and every signature.

| Option | |
|---|---|
| `--public-key` | The key the log must be signed with. Without it, the log is only checked against the key it declares itself, and the output warns that this proves nothing about who wrote it |
| `--checkpoint` | A previously published `seq:hash`; detects truncation and rewrites |
| `--json` | Print the full report as JSON |

**Exit codes:** 0 if valid, 1 if verification failed, 2 on usage or I/O errors.

## `lineage audit show <LOG> [--json]`

Prints the records as a table (`seq`, timestamp, actor, kind, payload), or as JSON Lines with `--json`. It doesn't verify; use `verify` for that.

## `lineage demo`

Walks through the original Lineage model: identity, a behavior loop, energy, scars, and death. It's also what `lineage` does with no arguments.
