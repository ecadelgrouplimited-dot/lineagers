"""Run the ops agent against the simulated incident, under a Lineage guard.

    # terminal 1
    export GUARD_ADMIN_TOKEN=$(openssl rand -hex 32)
    cargo run --release --manifest-path apps/guard-server/Cargo.toml

    # terminal 2 (same GUARD_ADMIN_TOKEN)
    python run.py --mock incident          # scripted model, no API key needed
    python run.py                          # Claude Opus 5.5 (needs Claude API credentials)
"""

import argparse
import json
import os
import sys
import threading
import time

from agent import MODEL, GuardedAgent
from lineage_guard import AdminClient, AgentClient
from mock_model import SCRIPTS, ScriptedClient
from monitor import Monitor
from tools import Toolbox

DEFAULT_TASK = ("Checkout is failing for customers. Investigate, mitigate if you can, keep customers informed, "
                "and make sure engineering has a ticket for the root cause.")

COLORS = {"model": "36", "tool_call": "33", "tool_result": "90", "approval_needed": "35", "approved": "32",
          "denied": "31", "blocked": "31", "tool_error": "31", "stopped": "31;1"}


def print_event(kind, **data):
    color = COLORS.get(kind, "0")
    if kind == "model":
        body = data["text"]
    elif kind == "tool_call":
        body = f"{data['tool']}({json.dumps(data['input'])})"
    elif kind == "tool_result":
        out = data["output"].replace("\n", " ")
        body = out if len(out) <= 160 else out[:157] + "..."
    else:
        body = ", ".join(f"{k}={v}" for k, v in data.items())
    print(f"\033[{color}m{kind:>15}\033[0m  {body}", flush=True)


class Approver(threading.Thread):
    """Answers the approval queue for one agent, standing in for a human at a dashboard."""

    def __init__(self, admin, agent_id, mode):
        super().__init__(daemon=True)
        self.admin, self.agent_id, self.mode = admin, agent_id, mode
        self.seen = set()

    def run(self):
        while True:
            try:
                pending = self.admin.pending(self.agent_id)
            except Exception:
                time.sleep(1)
                continue
            for item in pending:
                action = item["action"]
                if action["action_id"] in self.seen:
                    continue
                self.seen.add(action["action_id"])
                self.decide(action)
            time.sleep(0.5)

    def decide(self, action):
        if self.mode == "auto":
            self.admin.approve(self.agent_id, action["action_id"], approver="auto-approver")
            return
        print(f"\n\033[35;1mAPPROVAL REQUIRED\033[0m {action['action_id']}: {action['tool']}")
        print(json.dumps(action["input"], indent=2))
        answer = input("approve? [y/N/r=reject with scar] ").strip().lower()
        if answer == "y":
            self.admin.approve(self.agent_id, action["action_id"], approver=os.environ.get("USER", "operator"))
        else:
            scar = "moderate" if answer == "r" else None
            self.admin.reject(self.agent_id, action["action_id"], approver=os.environ.get("USER", "operator"),
                              reason="rejected at the terminal", scar=scar)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mock", choices=sorted(SCRIPTS), help="use a scripted model instead of Claude")
    parser.add_argument("--approvals", choices=["prompt", "dashboard", "auto"], default="prompt",
                        help="prompt: ask here; dashboard: approve at the guard server's console; auto: approve everything (demos only)")
    parser.add_argument("--guard-url", default=os.environ.get("GUARD_URL", "http://127.0.0.1:9200"))
    parser.add_argument("--agent-id", default=f"ops-agent-{int(time.time())}")
    parser.add_argument("--policy", default=os.path.join(os.path.dirname(__file__), "policy.json"))
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--effort", default="high", choices=["low", "medium", "high", "xhigh", "max"])
    parser.add_argument("--task", default=DEFAULT_TASK)
    args = parser.parse_args()

    admin_token = os.environ.get("GUARD_ADMIN_TOKEN")
    if not admin_token:
        sys.exit("GUARD_ADMIN_TOKEN is not set (use the same value the guard server was started with)")

    admin = AdminClient(args.guard_url, admin_token)
    with open(args.policy) as f:
        policy = json.load(f)
    created = admin.create_agent(args.agent_id, policy)
    guard = AgentClient(args.guard_url, args.agent_id, created["token"])

    if args.mock:
        import anthropic  # noqa: F401  (the scripted client builds SDK types)
        client = ScriptedClient(args.mock)
        model_label = f"scripted '{args.mock}' model"
    else:
        import anthropic
        client = anthropic.Anthropic()
        model_label = f"{args.model} (effort {args.effort})"

    print(f"agent {args.agent_id}  |  model: {model_label}  |  budget {policy['budget']}  |  approvals: {args.approvals}")
    if args.approvals == "dashboard":
        print(f"approve actions at {args.guard_url}/  (sign in with the admin token)")
    elif args.approvals == "auto":
        print("\033[33mauto-approving every request: demo mode only\033[0m")
    print()

    if args.approvals != "dashboard":
        Approver(admin, args.agent_id, args.approvals).start()

    agent = GuardedAgent(client, guard, Toolbox(), Monitor(admin, args.agent_id),
                         model=args.model, effort=args.effort, on_event=print_event)
    result = agent.run(args.task)

    status = admin.status(args.agent_id)
    report = admin.verify(args.agent_id)
    print("\n" + "=" * 72)
    print(f"outcome     {result.outcome}{'  (' + result.detail + ')' if result.detail else ''}")
    print(f"agent       alive={status['alive']}  spent {status['spent']}/{status['budget']}  "
          f"scars {status['scar_score']}/{status['scar_limit']}  turns {result.turns}")
    for scar in status["scars"]:
        print(f"  scar      {scar['severity']}: {scar['reason']}")
    if not args.mock:
        u = result.usage
        print(f"tokens      in {u['input_tokens']}  out {u['output_tokens']}  cache read {u['cache_read_input_tokens']}")
    print(f"audit       {'verified' if report['ok'] else 'FAILED'}, {report['records']} signed records")
    if report["ok"]:
        head = report["head"]
        print(f"checkpoint  {head['seq']}:{head['hash']}")
    return 0 if result.outcome in ("completed", "terminated") else 1


if __name__ == "__main__":
    sys.exit(main())
