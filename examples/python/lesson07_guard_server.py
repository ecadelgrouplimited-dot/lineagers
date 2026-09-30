"""Lineage Mastery, level 7: the guard server, from Python.

Agents in any language use the guard over HTTP. Here a support agent's tools are wrapped
with @agent.tool: each call asks the guard first, waits for a human when the policy says
so, and reports success or failure afterwards.

    # terminal 1 (repository root)
    cargo run --release --manifest-path apps/guard-server/Cargo.toml
    # terminal 2
    python3 examples/python/lesson07_guard_server.py                 # approve in the console
    python3 examples/python/lesson07_guard_server.py --auto-approve  # or let a script approve
"""

import os
import sys
import threading
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "apps", "guard-server", "clients", "python"))
from lineage_guard import ActionDenied, AgentClient, SetupError, connect_admin  # noqa: E402

POLICY = {
    "budget": 50,
    "scar_limit": 10,
    "tools": {
        "lookup_order": {"cost": 1},
        "issue_refund": {"cost": 5, "requires_approval": True, "max_calls": 3},
    },
}


def auto_approver(admin, agent_id):
    """Stands in for a person clicking Approve in the console."""
    for _ in range(300):
        for item in admin.pending(agent_id):
            action = item["action"]
            print(f"   [approver] {action['tool']} {action['input']['kwargs']} -> approve")
            admin.approve(agent_id, action["action_id"], "lesson-approver", note="refund within policy")
        time.sleep(0.2)


def main():
    try:
        admin = connect_admin()
    except SetupError as e:
        sys.exit(str(e))

    agent_id = f"support-{int(time.time())}"
    created = admin.create_agent(agent_id, POLICY)
    agent = AgentClient(admin.url, agent_id, created["token"])
    print(f"created {agent_id} with a budget of {POLICY['budget']}\n")

    @agent.tool("lookup_order")
    def lookup_order(order_id):
        return {"order_id": order_id, "status": "delivered late", "total": 40}

    @agent.tool("issue_refund")
    def issue_refund(order_id, amount):
        return f"refunded ${amount} on {order_id}"

    print("1. a free-to-use tool")
    print("  ", lookup_order(order_id="A-100"))

    print("\n2. a tool the policy doesn't list")
    try:
        agent.authorize("close_account", {"customer": 7})
    except ActionDenied as e:
        print("   denied:", e.reason)

    print("\n3. a tool that needs a human")
    if "--auto-approve" in sys.argv:
        threading.Thread(target=auto_approver, args=(admin, agent_id), daemon=True).start()
    else:
        print(f"   open {admin.url}/ , sign in with the admin token (cat guard-data/keys/admin.token),")
        print(f"   pick {agent_id}, and approve the refund. Waiting...")
    print("  ", issue_refund(order_id="A-100", amount=40))

    status = agent.status()
    print(f"\nspent {status['spent']}/{status['budget']}, scars {status['scar_score']}, "
          f"{status['actions_allowed']} allowed, {status['actions_denied']} denied")
    print(f"log verifies: {admin.verify(agent_id)['ok']}")


if __name__ == "__main__":
    main()
