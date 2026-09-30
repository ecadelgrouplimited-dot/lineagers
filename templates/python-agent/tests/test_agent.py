"""Runs the agent end to end against a running guard server.

    python3 -m unittest discover -s tests -v
"""

import os
import sys
import threading
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import agent as app  # noqa: E402
from lineage_guard import AgentClient, SetupError, connect_admin  # noqa: E402
from model import ScriptedModel  # noqa: E402


class AgentTest(unittest.TestCase):
    def setUp(self):
        try:
            self.admin = connect_admin()
        except SetupError as e:
            self.skipTest(str(e))
        self.agent_id = f"test-{int(time.time() * 1000)}"
        app.AGENT_ID = self.agent_id
        with open(os.path.join(app.HERE, "policy.json")) as f:
            import json
            created = self.admin.create_agent(self.agent_id, json.load(f))
        self.agent = AgentClient(self.admin.url, self.agent_id, created["token"])
        threading.Thread(target=app.auto_approve, args=(self.admin,), daemon=True).start()

    def test_refund_is_issued_and_account_change_denied(self):
        answer = app.run(self.agent, ScriptedModel(), "refund please")
        self.assertIn("refunded", answer)
        status = self.agent.status()
        self.assertTrue(status["alive"])
        self.assertEqual(status["spent"], 4 + 5)          # four model turns + one refund
        self.assertEqual(status["scar_score"], 3)          # update_account_email is not allowed
        self.assertTrue(self.admin.verify(self.agent_id)["ok"])


if __name__ == "__main__":
    unittest.main()
