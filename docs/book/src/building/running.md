# Running a project

## What runs where

{{#include ../diagrams/running.html}}

A Python (or any-language) project has up to three moving parts:

1. **`guard-server`**, in its own terminal or as a service. It owns `guard-data/`: the keys and every agent's signed log.
2. **Your agent.** It asks the server before every action.
3. **A browser**, only when a person approves, rejects, or audits: the console at `http://127.0.0.1:9200/`.

A Rust project using the guard in-process is just one process, with `lineage-data/` next to it.

## The everyday loop

```sh
# terminal 1: start once, leave running
guard-server

# terminal 2: run, change, run again
cd my-agent
python3 agent.py
python3 -m unittest discover -s tests
```

Look at what happened:

```sh
# in the console: http://127.0.0.1:9200/  (sign in: cat guard-data/keys/admin.token)
# or from the command line, on the log file itself:
lineage audit show guard-data/agents/my-agent.jsonl
lineage audit verify guard-data/agents/my-agent.jsonl --public-key "$(curl -s localhost:9200/v1/public-key | jq -r .public_key)"
```

## How the pieces find each other

| The agent needs | It looks in, in order |
|---|---|
| The server's URL | `GUARD_URL`, then `http://127.0.0.1:9200` |
| The admin token (to create the agent, and approve in scripts) | `GUARD_ADMIN_TOKEN` in the environment, then `.env`, then `keys/admin.token` in `$GUARD_DATA_DIR` or `./guard-data` |
| Its own agent token | Returned when the agent is created; the Python template saves it to `.agent-token` |
| An LLM API key | Your provider's usual variable (`ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`, …) |

Run the agent from the directory where the server runs, or set `GUARD_DATA_DIR` or `GUARD_ADMIN_TOKEN`.

## Starting over

An agent can't be reset; that's the point. To start fresh during development:

- **Python template:** delete `.agent-token`, and set a new `AGENT_ID` (or delete that agent's log from `guard-data/agents/` while the server is stopped).
- **Rust template:** delete `lineage-data/`.

In production, "starting over" always means creating a new agent. The old one's history stays.

## Troubleshooting

| You see | Cause and fix |
|---|---|
| `The guard server is not running at http://127.0.0.1:9200` | Start `guard-server`, or set `GUARD_URL` |
| `No guard admin token found` | Start the server without `GUARD_ADMIN_TOKEN`, so it creates `guard-data/keys/admin.token`, and run from that directory; or set `GUARD_ADMIN_TOKEN` to the server's value |
| `rejected the admin token` | The server was started with a different `GUARD_ADMIN_TOKEN`. Use the same value, or restart it without one |
| `cannot listen on 127.0.0.1:9200 … is another guard-server running?` | One is already running; use it, or set `GUARD_BIND` |
| `agent_exists` (409) | That agent ID is taken. Reuse it with its token, or choose a new ID |
| Agent shows as `quarantined` | Its log failed verification at startup. Don't edit logs; investigate it as an incident |
| Every request is `terminated` | The agent crossed its scar limit or budget. Look at `scars` and `termination_reason` in its status |
| `failed to run custom build command for yeslogic-fontconfig-sys` | Only when building the repository's examples: `sudo apt-get install libfontconfig1-dev pkg-config` |

Next: [Building an app](apps.md).
