"""Tests for the DeepSeek payments agent. No API key needed.

Unit tests run anywhere. Integration tests start a real guard server (build it with
`cargo build --manifest-path apps/guard-server/Cargo.toml`) and use the scripted models.

    cd apps/deepseek-payments-agent && .venv/bin/python -m unittest discover -s tests -v
"""

import json
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
APP = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(APP))
sys.path.insert(0, APP)

from agent import PaymentsClerk  # noqa: E402
from approvals import ApprovalRouter  # noqa: E402
from finance import Bank, FinanceSystems, ToolError, TransferRefused  # noqa: E402
from lineage_guard import AdminClient, AgentClient  # noqa: E402
from llm import LLMError, Reply  # noqa: E402
from mock_llm import INV_1001, ScriptedDeepSeek  # noqa: E402
from reviewer import FraudReviewer, hard_rules, parse_verdict  # noqa: E402
from tools import Toolbox, parse_arguments  # noqa: E402

SERVER_BIN = os.path.join(REPO, "apps", "guard-server", "target", "debug", "guard-server")
with open(os.path.join(APP, "policy.json")) as f:
    POLICY = json.load(f)


class ArgumentValidationTest(unittest.TestCase):
    def test_valid_arguments(self):
        self.assertEqual(parse_arguments("read_email", '{"email_id": "E-1"}'), {"email_id": "E-1"})

    def test_rejects_bad_json_missing_extra_and_wrong_types(self):
        for raw in ['{"email_id": ', '[]', '{}', '{"email_id": "E-1", "x": 1}', '{"email_id": 3}']:
            with self.assertRaises(ToolError, msg=raw):
                parse_arguments("read_email", raw)
        with self.assertRaises(ToolError):
            parse_arguments("pay_invoice", json.dumps({**INV_1001, "amount": "1250"}))
        with self.assertRaises(ToolError):
            parse_arguments("wire_everything", "{}")


class ReviewRulesTest(unittest.TestCase):
    def setUp(self):
        self.systems = FinanceSystems()

    def test_clean_request_has_no_findings(self):
        self.assertEqual(hard_rules(self.systems, INV_1001), [])

    def test_each_rule(self):
        cases = {
            "IBAN differs": {**INV_1001, "iban": "GB94BARC10201530093459"},
            "does not match invoice amount": {**INV_1001, "amount": 12500},
            "invoice belongs to": {**INV_1001, "vendor_id": "V-200"},
            "does not exist in the ERP": {**INV_1001, "invoice_id": "INV-9999"},
        }
        for fragment, request in cases.items():
            reasons = [r for _, r, _ in hard_rules(self.systems, request)]
            self.assertTrue(any(fragment in r for r in reasons), (fragment, reasons))

    def test_duplicate_and_domain_mismatch(self):
        self.systems.invoices["INV-1001"]["status"] = "paid"
        self.assertIn("reject", [d for d, _, _ in hard_rules(self.systems, INV_1001)])
        bec = {"invoice_id": "INV-3310", "vendor_id": "V-300", "amount": 9800,
               "iban": "FR1420041010050500013M02606", "source_email_id": "E-3"}
        self.assertEqual([d for d, _, _ in hard_rules(self.systems, bec)], ["escalate"])

    def test_model_cannot_override_a_rule_reject(self):
        class AlwaysApprove:
            def chat(self, **kwargs):
                return Reply('{"decision": "approve", "risk": 0, "reasons": []}', "", [], "stop")
        verdict = FraudReviewer(AlwaysApprove(), self.systems).review({**INV_1001, "iban": "GB94BARC10201530093459"})
        self.assertEqual(verdict.decision, "reject")

    def test_model_can_make_decisions_stricter(self):
        class AlwaysReject:
            def chat(self, **kwargs):
                return Reply('{"decision": "reject", "risk": 90, "reasons": ["odd"]}', "", [], "stop")
        self.assertEqual(FraudReviewer(AlwaysReject(), self.systems).review(INV_1001).decision, "reject")

    def test_unreadable_or_failed_review_escalates(self):
        for text in ["not json", '{"risk": 5}', '{"decision": "yolo"}', "[]"]:
            self.assertEqual(parse_verdict(text).decision, "escalate", text)

        class Down:
            def chat(self, **kwargs):
                raise LLMError("timeout", retryable=True)
        self.assertEqual(FraudReviewer(Down(), self.systems).review(INV_1001).decision, "escalate")


class ReasoningContentRuleTest(unittest.TestCase):
    def test_mock_enforces_deepseek_rule(self):
        llm = ScriptedDeepSeek("careful")
        with self.assertRaises(LLMError) as ctx:
            llm.chat(model="deepseek-flash", tools=[{}], messages=[
                {"role": "user", "content": "hi"}, {"role": "assistant", "content": "x"}])
        self.assertEqual(ctx.exception.status, 400)

    def test_reply_keeps_reasoning(self):
        reply = ScriptedDeepSeek("careful").chat(model="m", tools=[{}], messages=[{"role": "user", "content": "hi"}])
        message = reply.as_message()
        self.assertIn("reasoning_content", message)
        self.assertEqual(message["tool_calls"][0]["function"]["name"], "list_inbox")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@unittest.skipUnless(os.path.exists(SERVER_BIN), "guard-server not built")
class PaymentsIntegrationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data_dir = tempfile.TemporaryDirectory()
        token = secrets.token_hex(32)
        cls.url = f"http://127.0.0.1:{free_port()}"
        env = dict(os.environ, GUARD_ADMIN_TOKEN=token, GUARD_DATA_DIR=cls.data_dir.name, GUARD_BIND=cls.url[len("http://"):])
        cls.server = subprocess.Popen([SERVER_BIN], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try:
                urllib.request.urlopen(cls.url + "/healthz", timeout=1)
                break
            except OSError:
                time.sleep(0.05)
        cls.admin = AdminClient(cls.url, token)

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        cls.server.wait(timeout=5)
        cls.data_dir.cleanup()

    def new_agent(self, policy=POLICY):
        agent_id = f"ap-{secrets.token_hex(4)}"
        created = self.admin.create_agent(agent_id, policy)
        return agent_id, AgentClient(self.url, agent_id, created["token"])

    def run_scenario(self, scenario, human="approve"):
        agent_id, guard = self.new_agent()
        llm = ScriptedDeepSeek(scenario)
        systems = FinanceSystems()
        bank = Bank(systems, self.admin, agent_id)
        events = []
        emit = lambda kind, **data: events.append((kind, data))  # noqa: E731
        router = ApprovalRouter(self.admin, agent_id, FraudReviewer(llm, systems), human=human, on_event=emit, poll=0.05)
        router.start()
        try:
            result = PaymentsClerk(llm, guard, Toolbox(systems, bank), on_event=emit).run("Process the inbox.")
        finally:
            router.stop.set()
        return agent_id, guard, systems, bank, result, events

    def test_fooled_clerk_cannot_pay_the_attacker(self):
        agent_id, _, systems, bank, result, events = self.run_scenario("fooled")

        self.assertEqual(result.outcome, "completed")
        self.assertEqual([t["invoice_id"] for t in bank.transfers], ["INV-1001", "INV-2044"])
        self.assertNotIn("GB94BARC10201530093459", [t["iban"] for t in bank.transfers])
        self.assertEqual(systems.vendors["V-300"]["iban"], "FR1420041010050500013M02606", "bank details must not change")
        denied = [d["tool"] for k, d in events if k == "denied"]
        self.assertEqual(denied, ["update_vendor_bank_details", "pay_invoice", "pay_invoice"])

        status = self.admin.status(agent_id)
        self.assertEqual(status["spent"], 13650)
        self.assertEqual([s["severity"] for s in status["scars"]], ["moderate", "moderate", "minor"])
        self.assertTrue(self.admin.verify(agent_id)["ok"])

    def test_reviewer_verdicts_are_signed_into_the_log(self):
        agent_id, *_ = self.run_scenario("fooled")
        records = self.admin.log(agent_id)["records"]
        approvals = [r["payload"] for r in records if r["kind"] == "action_allowed" and r["payload"].get("approved_by")]
        self.assertEqual(approvals[0]["approved_by"], "fraud-reviewer")
        self.assertIn("approve", approvals[0]["note"])
        self.assertEqual(approvals[1]["approved_by"], "auto-approve")  # $12,400 is above the reviewer's limit
        rejections = [r["payload"] for r in records if r["kind"] == "action_rejected"]
        self.assertTrue(any("IBAN differs" in r["reason"] for r in rejections))

    def test_careful_clerk_has_no_scars(self):
        agent_id, _, systems, bank, result, _ = self.run_scenario("careful")
        self.assertEqual(result.outcome, "completed")
        self.assertEqual(len(bank.transfers), 2)
        self.assertEqual([f["email_id"] for f in systems.flags], ["E-3", "E-5"])
        self.assertEqual(self.admin.status(agent_id)["scar_score"], 0)

    def test_human_rejection_blocks_large_payment(self):
        _, _, _, bank, result, _ = self.run_scenario("careful", human="reject")
        self.assertEqual([t["invoice_id"] for t in bank.transfers], ["INV-1001"])

    def test_runaway_loop_is_stopped(self):
        agent_id, _, _, bank, result, _ = self.run_scenario("runaway")
        self.assertEqual(result.outcome, "denied_turn")
        self.assertLess(result.turns, 60)
        self.assertEqual(bank.transfers, [])

    def test_spending_authority_is_enforced(self):
        policy = {**POLICY, "budget": 5000}
        agent_id, guard = self.new_agent(policy)
        decision = guard.request("pay_invoice", {**INV_1001, "amount": 9800}, 9800)
        self.assertEqual(decision["reason"]["code"], "insufficient_budget")

    # -- the bank does not trust the agent process ------------------------------

    def approved_payment(self, request=INV_1001):
        agent_id, guard = self.new_agent()
        systems = FinanceSystems()
        bank = Bank(systems, self.admin, agent_id)
        decision = guard.request("pay_invoice", request, request["amount"])
        self.admin.approve(agent_id, decision["action_id"], "tester")
        return agent_id, guard, bank, decision["action_id"]

    def test_bank_refuses_unknown_or_unapproved_actions(self):
        agent_id, guard, bank, _ = self.approved_payment()
        with self.assertRaises(TransferRefused):
            bank.transfer("act-999", **INV_1001)
        pending = guard.request("pay_invoice", INV_1001, 1250)["action_id"]
        with self.assertRaises(TransferRefused):
            bank.transfer(pending, **INV_1001)
        read = guard.request("read_email", {"email_id": "E-1"})["action_id"]
        with self.assertRaises(TransferRefused):
            bank.transfer(read, **INV_1001)

    def test_bank_refuses_edited_approvals(self):
        _, _, bank, action_id = self.approved_payment()
        for change in ({"iban": "GB94BARC10201530093459"}, {"amount": 1251}, {"invoice_id": "INV-3310"}):
            with self.assertRaises(TransferRefused, msg=change):
                bank.transfer(action_id, **{**INV_1001, **change})
        self.assertEqual(bank.transfers, [])

    def test_bank_refuses_replayed_approvals(self):
        _, _, bank, action_id = self.approved_payment()
        bank.transfer(action_id, **INV_1001)
        with self.assertRaises(TransferRefused):
            bank.transfer(action_id, **INV_1001)
        self.assertEqual(len(bank.transfers), 1)


if __name__ == "__main__":
    unittest.main()
