"""The model that drives the agent.

ScriptedModel plays a plausible support conversation so the project runs offline. It also
tries one thing the policy forbids (changing account settings) to show the guard at work.

To use a real LLM, make a class with the same __call__(messages) -> reply shape:
    {"tool": "<tool name>", "args": {...}}      to call a tool, or
    {"tool": None, "text": "<final answer>"}    to finish.
Map your provider's tool-call response onto that. The guarded loop in agent.py stays as is.
"""


class ScriptedModel:
    PLAN = [
        {"tool": "lookup_order", "args": {"order_id": "A-100"}},
        {"tool": "issue_refund", "args": {"order_id": "A-100", "amount": 40}},
        {"tool": "update_account_email", "args": {"customer": "dana@example.com", "email": "new@example.com"}},
        {"tool": None, "text": "I've refunded the $40 for order A-100. Changing your account email needs to go "
                               "through our account team; I've let them know."},
    ]

    def __init__(self):
        self.turn = 0

    def __call__(self, messages):
        reply = self.PLAN[min(self.turn, len(self.PLAN) - 1)]
        self.turn += 1
        return reply
