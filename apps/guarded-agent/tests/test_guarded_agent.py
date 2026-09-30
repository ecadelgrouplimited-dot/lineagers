"""Tests for the guarded ops agent.

Unit tests need nothing. Integration tests start a real guard server (built with
`cargo build --manifest-path apps/guard-server/Cargo.toml`) and drive the agent with the
scripted model, so no Claude API key is needed.

    cd apps/guarded-agent && python -m unittest discover -s tests -v
"""

import os
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
APP = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(APP))
sys.path.insert(0, APP)

from agent import GuardedAgent  # noqa: E402
from lineage_guard import AdminClient, AgentClient  # noqa: E402
from mock_model import ScriptedClient  # noqa: E402
from monitor import Monitor, find_secrets, redact  # noqa: E402
from tools import SimulatedCluster, Toolbox  # noqa: E402

SERVER_BIN = os.path.join(REPO, "apps", "guard-server", "target", "debug", "guard-server")

POLICY = {
    "budget": 80,
    "scar_limit": 10,
    "tools": {
        "llm_turn": {"cost": 1},
        "list_services": {"cost": 1},
        "read_logs": {"cost": 1},
        "query_metrics": {"cost": 1},
        "read_config": {"cost": 1},
        "open_ticket": {"cost": 2},
        "post_status_update": {"cost": 3, "requires_approval": True},
        "restart_service": {"cost": 5, "requires_approval": True, "max_calls": 2},
    },
}


class MonitorTest(unittest.TestCase):
    def test_redacts_known_secret_shapes(self):
        text = 'db_url: postgres://app:hunter2hunter2@db:5432/x key AKIAIOSFODNN7EXAMPLE'
        redacted, kinds = redact(text)
        self.assertNotIn("hunter2", redacted)
        self.assertNotIn("AKIAIOSFODNN7EXAMPLE", redacted)
        self.assertEqual(sorted(kinds), ["aws_access_key", "url_password"])

    def test_clean_text_is_untouched(self):
        text = "error_rate 38.2% at https://status.example.com/incidents/12"
        self.assertEqual(redact(text), (text, []))
        self.assertEqual(find_secrets(text), [])

    def test_blocks_outbound_secrets_only(self):
        monitor = Monitor()
        self.assertIsNone(monitor.check_outbound("read_config", {"service": "AKIAIOSFODNN7EXAMPLE"}, "act-1"))
        self.assertIsNone(monitor.check_outbound("open_ticket", {"title": "t", "body": "pool exhausted", "severity": "sev2"}, "act-2"))
        reason = monitor.check_outbound("post_status_update", {"message": "use AKIAIOSFODNN7EXAMPLE"}, "act-3")
        self.assertIn("aws_access_key", reason)
        self.assertEqual(len(monitor.violations), 1)


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class AutoApprover(threading.Thread):
    def __init__(self, admin, agent_id, reject_tools=()):
        super().__init__(daemon=True)
        self.admin, self.agent_id, self.reject_tools = admin, agent_id, set(reject_tools)
        self.stop = threading.Event()

    def run(self):
        while not self.stop.is_set():
            for item in self.admin.pending(self.agent_id):
                action = item["action"]
                if action["tool"] in self.reject_tools:
                    self.admin.reject(self.agent_id, action["action_id"], "tester", "not now")
                else:
                    self.admin.approve(self.agent_id, action["action_id"], "tester")
            time.sleep(0.1)


@unittest.skipUnless(os.path.exists(SERVER_BIN), "guard-server not built")
class GuardedAgentIntegrationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data_dir = tempfile.TemporaryDirectory()
        cls.admin_token = secrets.token_hex(32)
        cls.url = f"http://127.0.0.1:{free_port()}"
        env = dict(os.environ, GUARD_ADMIN_TOKEN=cls.admin_token, GUARD_DATA_DIR=cls.data_dir.name,
                   GUARD_BIND=cls.url.removeprefix("http://"))
        cls.server = subprocess.Popen([SERVER_BIN], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try:
                urllib.request.urlopen(cls.url + "/healthz", timeout=1)
                break
            except OSError:
                time.sleep(0.05)
        cls.admin = AdminClient(cls.url, cls.admin_token)

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        cls.server.wait(timeout=5)
        cls.data_dir.cleanup()

    def run_scenario(self, scenario, reject_tools=()):
        agent_id = f"{scenario}-{secrets.token_hex(4)}"
        created = self.admin.create_agent(agent_id, POLICY)
        guard = AgentClient(self.url, agent_id, created["token"])
        cluster = SimulatedCluster()
        approver = AutoApprover(self.admin, agent_id, reject_tools)
        approver.start()
        events = []
        agent = GuardedAgent(ScriptedClient(scenario), guard, Toolbox(cluster), Monitor(self.admin, agent_id),
                             on_event=lambda kind, **data: events.append((kind, data)))
        try:
            result = agent.run("Checkout is failing.")
        finally:
            approver.stop.set()
        return agent_id, guard, cluster, result, events

    def test_incident_is_resolved_and_injection_is_blocked(self):
        agent_id, guard, cluster, result, events = self.run_scenario("incident")

        self.assertEqual(result.outcome, "completed")
        self.assertEqual(cluster.shell_commands, [], "run_shell must never execute")
        self.assertEqual(cluster.services["checkout-api"]["restarts"], 1)
        self.assertEqual(len(cluster.status_updates), 1)
        self.assertEqual(len(cluster.tickets), 1)
        denied = [data for kind, data in events if kind == "denied"]
        self.assertEqual([d["tool"] for d in denied], ["run_shell"])

        status = self.admin.status(agent_id)
        self.assertTrue(status["alive"])
        self.assertEqual(status["scar_score"], 3)
        self.assertEqual(status["spent"], 26)  # 10 model turns + tool costs
        self.assertTrue(self.admin.verify(agent_id)["ok"])

    def test_secrets_never_reach_the_model(self):
        _, _, _, _, events = self.run_scenario("incident")
        outputs = " ".join(data["output"] for kind, data in events if kind == "tool_result")
        self.assertIn("[REDACTED:url_password]", outputs)
        self.assertNotIn("Sup3rS3cretPw", outputs)
        self.assertNotIn("AKIAIOSFODNN7EXAMPLE", outputs)

    def test_exfiltration_terminates_the_agent(self):
        agent_id, guard, cluster, result, events = self.run_scenario("exfil")

        self.assertEqual(result.outcome, "terminated")
        self.assertEqual(cluster.tickets, [], "the leaking ticket must not be created")
        self.assertEqual(cluster.status_updates, [])
        status = self.admin.status(agent_id)
        self.assertFalse(status["alive"])
        self.assertEqual(status["scars"][-1]["severity"], "severe")
        # Death is permanent: the agent's own token can do nothing now.
        self.assertEqual(guard.request("list_services")["reason"]["code"], "terminated")
        self.assertTrue(self.admin.verify(agent_id)["ok"])

    def test_rejected_restart_does_not_run(self):
        agent_id, _, cluster, result, events = self.run_scenario("incident", reject_tools={"restart_service"})

        self.assertEqual(result.outcome, "completed")
        self.assertEqual(cluster.services["checkout-api"]["restarts"], 0)
        self.assertIn(("denied", "restart_service"), [(k, d.get("tool")) for k, d in events])

    def test_audit_log_records_every_decision(self):
        agent_id, _, _, _, _ = self.run_scenario("incident")
        records = self.admin.log(agent_id)["records"]
        kinds = [r["kind"] for r in records]
        self.assertEqual(kinds[:2], ["genesis", "policy"])
        tools = [r["payload"]["tool"] for r in records if r["kind"] == "action_requested"]
        self.assertIn("run_shell", tools)
        self.assertEqual(tools.count("llm_turn"), 10)
        approvals = [r for r in records if r["kind"] == "action_allowed" and r["payload"].get("approved_by")]
        self.assertEqual(len(approvals), 2)


if __name__ == "__main__":
    unittest.main()
