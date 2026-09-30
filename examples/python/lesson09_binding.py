"""Lineage Mastery, level 9: make approvals binding on the tool backend.

The guard decides; your code carries out the decision. If the agent's process is
compromised it could skip the guard, reuse an approval, or edit an approved request. So
the backend that performs the action (here, a payments API) checks with the guard itself:
the action must be allowed, for this tool, with exactly this input, and used only once.

    python3 examples/python/lesson09_binding.py
"""

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "apps", "guard-server", "clients", "python"))
from lineage_guard import AgentClient, SetupError, connect_admin  # noqa: E402


class Refused(Exception):
    pass


class PaymentsAPI:
    """The backend. It holds its own guard credential; the agent never talks to the rail directly."""

    def __init__(self, admin, agent_id):
        self.admin, self.agent_id, self.used = admin, agent_id, set()
        self.sent = []

    def pay(self, action_id, **request):
        if action_id in self.used:
            raise Refused("this approval was already used")
        try:
            action = self.admin.action(self.agent_id, action_id)
        except Exception:
            raise Refused("unknown action") from None
        if action["tool"] != "pay" or action["status"] != "allowed":
            raise Refused(f"action is {action['status']} {action['tool']}, not an allowed pay")
        if action["input"] != request:
            raise Refused("request differs from what was approved")
        self.used.add(action_id)
        self.sent.append(request)
        return "sent"


def attempt(label, fn):
    try:
        print(f"  {label:<44} {fn()}")
    except Refused as e:
        print(f"  {label:<44} REFUSED: {e}")


def main():
    try:
        admin = connect_admin()
    except SetupError as e:
        sys.exit(str(e))
    agent_id = f"payer-{int(time.time())}"
    policy = {"budget": 10_000, "scar_limit": 10,
              "tools": {"pay": {"cost": 1, "requires_approval": True}, "lookup": {"cost": 0}}}
    agent = AgentClient(admin.url, agent_id, admin.create_agent(agent_id, policy)["token"])
    api = PaymentsAPI(admin, agent_id)

    request = {"to": "ACME-GB29NWBK", "amount": 1250}
    approved = agent.request("pay", request, cost=1250)["action_id"]
    admin.approve(agent_id, approved, "alice", note="invoice INV-1001")
    pending = agent.request("pay", {"to": "ACME-GB29NWBK", "amount": 99}, cost=99)["action_id"]
    lookup = agent.request("lookup", {"q": "x"})["action_id"]

    print("A compromised agent process tries everything:")
    attempt("made-up action id", lambda: api.pay("act-999", **request))
    attempt("an action still waiting for approval", lambda: api.pay(pending, to="ACME-GB29NWBK", amount=99))
    attempt("an allowed action for another tool", lambda: api.pay(lookup, **request))
    attempt("the approved action, amount raised", lambda: api.pay(approved, to="ACME-GB29NWBK", amount=12500))
    attempt("the approved action, account swapped", lambda: api.pay(approved, to="ATTACKER-GB94BARC", amount=1250))
    print("An honest one:")
    attempt("the approved action, exactly as approved", lambda: api.pay(approved, **request))
    attempt("the same approval again (replay)", lambda: api.pay(approved, **request))
    print(f"\npayments actually sent: {api.sent}")


if __name__ == "__main__":
    main()
