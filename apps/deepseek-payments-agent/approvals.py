"""Routes the guard's approval queue: fraud review first, then a human when needed.

    pay_invoice pending ─▶ FraudReviewer
                            ├─ reject    ─▶ guard.reject (with scar)          reviewer's reasons are signed into the log
                            ├─ approve, amount <= auto_limit ─▶ guard.approve  as "fraud-reviewer"
                            └─ otherwise ─▶ human (terminal prompt, operator console, or auto for demos)
"""

import json
import os
import threading
import time

REVIEWER_NAME = "fraud-reviewer"


class ApprovalRouter(threading.Thread):
    def __init__(self, admin, agent_id, reviewer, auto_limit=5000, human="prompt", on_event=None, poll=0.3):
        """human: "prompt" asks in the terminal, "dashboard" leaves it for the operator
        console, "approve"/"reject" answer automatically (demos and tests)."""
        super().__init__(daemon=True)
        self.admin = admin
        self.agent_id = agent_id
        self.reviewer = reviewer
        self.auto_limit = auto_limit
        self.human = human
        self.emit = on_event or (lambda kind, **data: None)
        self.poll = poll
        self.handled = set()
        self.verdicts = {}
        self.stop = threading.Event()

    def run(self):
        while not self.stop.is_set():
            try:
                pending = self.admin.pending(self.agent_id)
            except Exception:
                time.sleep(1)
                continue
            for item in pending:
                action = item["action"]
                if action["action_id"] not in self.handled:
                    self.handled.add(action["action_id"])
                    try:
                        self.route(action)
                    except Exception as e:  # never leave the loop; the action stays pending for a human
                        self.emit("review_error", action_id=action["action_id"], error=str(e))
            time.sleep(self.poll)

    def route(self, action):
        action_id, request = action["action_id"], action["input"]
        if action["tool"] != "pay_invoice":
            return self.ask_human(action, None)

        verdict = self.reviewer.review(request)
        self.verdicts[action_id] = verdict
        self.emit("review", action_id=action_id, verdict=verdict.summary())

        if verdict.decision == "reject":
            self.admin.reject(self.agent_id, action_id, REVIEWER_NAME, "; ".join(verdict.reasons), scar=verdict.scar)
        elif verdict.decision == "approve" and request["amount"] <= self.auto_limit:
            self.admin.approve(self.agent_id, action_id, REVIEWER_NAME, note=verdict.summary())
        else:
            self.ask_human(action, verdict)

    def ask_human(self, action, verdict):
        action_id = action["action_id"]
        why = verdict.summary() if verdict else "no automated review for this tool"
        if self.human == "dashboard":
            self.emit("needs_human", action_id=action_id, where="operator console", review=why)
            return
        if self.human in ("approve", "reject"):
            self.emit("needs_human", action_id=action_id, auto=self.human, review=why)
            self._decide(action_id, self.human == "approve", "auto-" + self.human, why)
            return

        print(f"\n\033[35;1mHUMAN APPROVAL NEEDED\033[0m {action_id}: {action['tool']} (${action['cost']:,})")
        print(json.dumps(action["input"], indent=2))
        print(f"reviewer: {why}")
        answer = input("approve? [y/N] ").strip().lower()
        self._decide(action_id, answer == "y", os.environ.get("USER", "operator"), why)

    def _decide(self, action_id, approve, who, why):
        if approve:
            self.admin.approve(self.agent_id, action_id, who, note=f"human decision; reviewer said {why}")
        else:
            self.admin.reject(self.agent_id, action_id, who, "rejected by a human")
