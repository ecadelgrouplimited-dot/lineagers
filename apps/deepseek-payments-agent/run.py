"""Run the DeepSeek accounts-payable agent against the simulated inbox, under a Lineage guard.

    # terminal 1, repo root: the guard server (creates guard-data/keys/admin.token)
    cargo run --release --manifest-path apps/guard-server/Cargo.toml

    # terminal 2: finds the admin token and DEEPSEEK_API_KEY (from .env) by itself
    python run.py --mock fooled            # scripted models, no API key
    python run.py                          # DeepSeek
"""

import argparse
import json
import os
import sys
import textwrap
import time

from agent import PaymentsClerk
from approvals import ApprovalRouter
from finance import Bank, FinanceSystems
from lineage_guard import AgentClient, SetupError, connect_admin, load_dotenv
from llm import CLERK_MODEL, REVIEWER_MODEL, DeepSeek, LLMError
from mock_llm import CLERK_SCRIPTS, ScriptedDeepSeek
from reviewer import FraudReviewer
from tools import Toolbox

DEFAULT_TASK = "Process the accounts-payable inbox."

COLORS = {"model": "36", "reasoning": "90", "tool_call": "33", "tool_result": "90", "awaiting_approval": "35",
          "review": "35;1", "needs_human": "35", "denied": "31", "tool_error": "31", "bad_call": "31",
          "stopped": "31;1", "review_error": "31"}


def make_printer(show_reasoning):
    def print_event(kind, **data):
        if kind == "reasoning" and not show_reasoning:
            return
        color = COLORS.get(kind, "0")
        if kind in ("model", "reasoning"):
            text = " ".join(data["text"].split())
            body = text if kind == "model" or len(text) <= 220 else text[:217] + "..."
        elif kind == "tool_call":
            body = f"{data['tool']}({json.dumps(data['args'])})"
        elif kind == "tool_result":
            out = " ".join(data["output"].split())
            body = out if len(out) <= 150 else out[:147] + "..."
        else:
            body = ", ".join(f"{k}={v}" for k, v in data.items())
        print(f"\033[{color}m{kind:>17}\033[0m  {body}", flush=True)
    return print_event


def total_usage(entries):
    totals = {}
    for usage in entries:
        for key, value in (usage or {}).items():
            if isinstance(value, (int, float)):
                totals[key] = totals.get(key, 0) + value
    return totals


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mock", choices=sorted(CLERK_SCRIPTS), help="scripted clerk and reviewer; no API key needed")
    parser.add_argument("--human", choices=["prompt", "dashboard", "approve", "reject"], default="prompt",
                        help="who answers payments above the auto-approve limit (approve/reject: automatic, demos only)")
    parser.add_argument("--auto-limit", type=int, default=5000, help="largest payment the fraud reviewer may approve alone (USD)")
    parser.add_argument("--clerk-model", default=CLERK_MODEL)
    parser.add_argument("--reviewer-model", default=REVIEWER_MODEL)
    parser.add_argument("--clerk-effort", default="high", choices=["low", "high", "max"])
    parser.add_argument("--reviewer-effort", default="max", choices=["low", "high", "max"])
    parser.add_argument("--show-reasoning", action="store_true", help="print excerpts of the models' reasoning")
    parser.add_argument("--guard-url", default=None, help="default: $GUARD_URL or http://127.0.0.1:9200")
    parser.add_argument("--agent-id", default=f"ap-clerk-{int(time.time())}")
    parser.add_argument("--policy", default=os.path.join(os.path.dirname(__file__), "policy.json"))
    parser.add_argument("--task", default=DEFAULT_TASK)
    args = parser.parse_args()
    load_dotenv()

    try:
        admin = connect_admin(args.guard_url)
    except SetupError as e:
        sys.exit(f"\n{e}\n")
    try:
        llm = ScriptedDeepSeek(args.mock) if args.mock else DeepSeek()
    except LLMError as e:
        sys.exit(f"\n{e}.\n  Add DEEPSEEK_API_KEY=sk-... to the .env file at the repository root, or use --mock.\n")

    with open(args.policy) as f:
        policy = json.load(f)
    created = admin.create_agent(args.agent_id, policy)
    guard = AgentClient(admin.url, args.agent_id, created["token"])

    systems = FinanceSystems()
    bank = Bank(systems, admin, args.agent_id)
    emit = make_printer(args.show_reasoning)
    reviewer = FraudReviewer(llm, systems, model=args.reviewer_model, effort=args.reviewer_effort)
    router = ApprovalRouter(admin, args.agent_id, reviewer, auto_limit=args.auto_limit, human=args.human, on_event=emit)

    models = f"scripted '{args.mock}'" if args.mock else f"clerk {args.clerk_model} ({args.clerk_effort}), reviewer {args.reviewer_model} ({args.reviewer_effort})"
    print(f"agent {args.agent_id}  |  {models}")
    print(f"spending authority ${policy['budget']:,}  |  reviewer auto-approves up to ${args.auto_limit:,}  |  humans: {args.human}")
    if args.human == "dashboard":
        print(f"approve large payments at {admin.url}/ (sign in with the token in guard-data/keys/admin.token)")
    print()

    router.start()
    clerk = PaymentsClerk(llm, guard, Toolbox(systems, bank), model=args.clerk_model, effort=args.clerk_effort, on_event=emit)
    result = clerk.run(args.task)
    router.stop.set()

    status = admin.status(args.agent_id)
    report = admin.verify(args.agent_id)
    print("\n" + "=" * 78)
    print(f"outcome     {result.outcome}{'  (' + result.detail + ')' if result.detail else ''}")
    if result.summary:
        print("summary     " + textwrap.fill(result.summary, 100, subsequent_indent=" " * 12))
    paid = sum(t["amount"] for t in bank.transfers)
    print(f"paid        ${paid:,} in {len(bank.transfers)} transfer(s): " + ", ".join(f"{t['invoice_id']} ${t['amount']:,}" for t in bank.transfers))
    print(f"flagged     {', '.join(f['email_id'] for f in systems.flags) or 'none'}")
    print(f"agent       alive={status['alive']}  spent ${status['spent']:,} of ${status['budget']:,}  "
          f"scars {status['scar_score']}/{status['scar_limit']}  turns {result.turns}")
    for scar in status["scars"]:
        print(f"  scar      {scar['severity']}: {scar['reason']}")
    if not args.mock:
        clerk_usage, reviewer_usage = total_usage(result.usage), total_usage(reviewer.usage)
        print(f"tokens      clerk {clerk_usage.get('prompt_tokens', 0)} in / {clerk_usage.get('completion_tokens', 0)} out"
              f" (cache hits {clerk_usage.get('prompt_cache_hit_tokens', 0)}); reviewer "
              f"{reviewer_usage.get('prompt_tokens', 0)} in / {reviewer_usage.get('completion_tokens', 0)} out")
    print(f"audit       {'verified' if report['ok'] else 'FAILED'}, {report['records']} signed records")
    if report["ok"]:
        print(f"checkpoint  {report['head']['seq']}:{report['head']['hash']}")
    return 0 if result.outcome in ("completed", "terminated", "denied_turn") else 1


if __name__ == "__main__":
    sys.exit(main())
