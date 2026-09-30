"""{{name}}: a customer-support agent under a Lineage guard.

The agent answers refund requests. It may look orders up freely, issue refunds only with a
human's approval, and never touch account settings. A scripted model drives it, so it runs
without an API key; swap in your LLM in model.py.

    python3 agent.py                  # approve refunds at the guard console
    python3 agent.py --auto-approve   # approve automatically (demo only)
"""

import json
import os
import sys
import threading
import time

from lineage_guard import AgentClient, SetupError, connect_admin
from model import ScriptedModel

HERE = os.path.dirname(os.path.abspath(__file__))
AGENT_ID = os.environ.get("AGENT_ID", "{{name}}")

ORDERS = {
    "A-100": {"customer": "dana@example.com", "total": 40, "status": "delivered 9 days late"},
    "A-101": {"customer": "lee@example.com", "total": 18, "status": "delivered on time"},
}


def lookup_order(order_id):
    if order_id not in ORDERS:
        raise KeyError(f"no order {order_id}")
    return ORDERS[order_id]


def issue_refund(order_id, amount):
    return {"order_id": order_id, "refunded": amount}


TOOLS = {"lookup_order": lookup_order, "issue_refund": issue_refund}


def connect():
    """Creates the agent on first run; later runs reuse it, with its budget and scars."""
    admin = connect_admin()
    token_file = os.path.join(HERE, ".agent-token")
    if os.path.exists(token_file):
        with open(token_file) as f:
            return admin, AgentClient(admin.url, AGENT_ID, f.read().strip())
    with open(os.path.join(HERE, "policy.json")) as f:
        created = admin.create_agent(AGENT_ID, json.load(f))
    with open(token_file, "w") as f:
        f.write(created["token"])
    os.chmod(token_file, 0o600)
    return admin, AgentClient(admin.url, AGENT_ID, created["token"])


def step(agent, tool, args):
    """One guarded tool call. Returns the text the model sees."""
    decision = agent.request(tool, args)
    if decision["decision"] == "pending_approval":
        print(f"  {tool}: waiting for approval ({decision['action_id']})")
        status = agent.wait_for_approval(decision["action_id"], timeout=600, poll=1.0)
        if status != "allowed":
            return f"Denied: a human rejected {tool}. Tell the customer it needs manual review."
    elif decision["decision"] == "denied":
        return f"Denied by the policy guard: {decision['reason']}. Do not retry."
    try:
        result = TOOLS[tool](**args)
    except Exception as e:
        agent.report(decision["action_id"], "failure", str(e))
        return f"Error: {e}"
    agent.report(decision["action_id"], "success")
    return json.dumps(result)


def run(agent, model, request):
    messages = [{"role": "user", "content": request}]
    while True:
        turn = agent.request("llm_turn", {})
        if turn["decision"] != "allowed":
            return f"stopped: {turn['reason']}"
        reply = model(messages)
        agent.report(turn["action_id"], "success")
        if reply["tool"] is None:
            return reply["text"]
        result = step(agent, reply["tool"], reply["args"])
        print(f"  {reply['tool']}({reply['args']}) -> {result}")
        messages.append({"role": "tool", "content": result})
        if not agent.status()["alive"]:
            return "the agent was terminated"


def auto_approve(admin):
    while True:
        for item in admin.pending(AGENT_ID):
            admin.approve(AGENT_ID, item["action"]["action_id"], "auto-approver", note="demo")
        time.sleep(0.3)


def main():
    try:
        admin, agent = connect()
    except SetupError as e:
        sys.exit(str(e))
    if "--auto-approve" in sys.argv:
        threading.Thread(target=auto_approve, args=(admin,), daemon=True).start()
    else:
        print(f"Approve refunds at {admin.url}/ (agent {AGENT_ID}).")

    request = "Order A-100 arrived very late. Can I get my money back? Also please change my account email."
    print(f"customer: {request}\n")
    answer = run(agent, ScriptedModel(), request)
    status = agent.status()
    print(f"\nagent: {answer}")
    print(f"\nspent {status['spent']}/{status['budget']}, scars {status['scar_score']}/{status['scar_limit']}, alive {status['alive']}")


if __name__ == "__main__":
    main()
