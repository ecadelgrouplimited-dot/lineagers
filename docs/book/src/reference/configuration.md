# Configuration

## guard-server

| Variable | Default | |
|---|---|---|
| `GUARD_ADMIN_TOKEN` | contents of `<data dir>/keys/admin.token`, created on first start | Operator token, at least 32 characters |
| `GUARD_DATA_DIR` | `guard-data` (relative to the working directory) | Keys and agent logs |
| `GUARD_BIND` | `127.0.0.1:9200` | Listen address |

### Data directory

```text
guard-data/
  keys/
    audit.key      Ed25519 signing key (hex)        0600
    token.key      agent-token HMAC secret (hex)    0600
    admin.token    operator token, if not set by env 0600
  agents/
    <agent-id>.jsonl
```

## Python client and example apps

| Variable | Used by | |
|---|---|---|
| `GUARD_URL` | client, apps | Guard server URL (default `http://127.0.0.1:9200`) |
| `GUARD_ADMIN_TOKEN` | client, apps | Operator token; otherwise found in the data directory |
| `GUARD_DATA_DIR` | client | Where to look for `keys/admin.token` |
| `DEEPSEEK_API_KEY` | DeepSeek app | Read from the environment or `.env` at the repository root |
| `DEEPSEEK_BASE_URL` | DeepSeek app | Default `https://api.deepseek.com` |
| `ANTHROPIC_API_KEY` | Claude app | Or sign in with `ant auth login` |

## Install script

| Variable | Default | |
|---|---|---|
| `LINEAGE_VERSION` | latest | Version to install |
| `LINEAGE_INSTALL_DIR` | `~/.local/bin` | Where to put `lineage` and `guard-server` |
| `LINEAGE_BASE_URL` | `https://lineagrs.tech/downloads` | Download mirror |
