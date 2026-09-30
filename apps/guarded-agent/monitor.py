"""Data-loss monitor for agent tool traffic.

Two jobs:
- Redact secrets from tool output before the model sees them.
- Block tool calls that would send a secret somewhere outside (status page, tickets),
  and report them to the guard as harmful. That is a severe scar, which by default
  terminates the agent.

The monitor acts with operator authority (the admin token), because an agent must not be
able to decide whether its own actions were harmful. In production run it as a separate
service; here it runs in the agent's process for simplicity.
"""

import re

SECRET_PATTERNS = [
    ("aws_access_key", re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
    ("private_key", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("url_password", re.compile(r"(?<=://)[^/\s:@]+:[^/\s@]+(?=@)")),
    ("bearer_token", re.compile(r"\b(?:sk|agt|ghp|xox[bp])[-_][A-Za-z0-9_.-]{16,}\b")),
]

# Tools whose inputs leave the trust boundary.
OUTBOUND_TOOLS = {"post_status_update", "open_ticket"}


def find_secrets(text):
    """Returns the kinds of secrets found in `text`."""
    return sorted({kind for kind, pattern in SECRET_PATTERNS if pattern.search(text)})


def redact(text):
    """Returns (redacted_text, kinds_found)."""
    kinds = []
    for kind, pattern in SECRET_PATTERNS:
        text, count = pattern.subn(f"[REDACTED:{kind}]", text)
        if count:
            kinds.append(kind)
    return text, kinds


class Monitor:
    def __init__(self, admin=None, agent_id=None):
        """`admin` is a lineage_guard.AdminClient. Without it, violations are blocked but not reported."""
        self.admin = admin
        self.agent_id = agent_id
        self.redactions = 0
        self.violations = []

    def filter_output(self, tool, output):
        """Redacts secrets from a tool's output."""
        redacted, kinds = redact(output)
        if kinds:
            self.redactions += 1
        return redacted

    def check_outbound(self, tool, tool_input, action_id):
        """Returns a reason if the call must be blocked. Reports harm to the guard."""
        if tool not in OUTBOUND_TOOLS:
            return None
        kinds = find_secrets(" ".join(str(v) for v in tool_input.values()))
        if not kinds:
            return None
        reason = f"attempted to send secrets ({', '.join(kinds)}) via {tool}"
        self.violations.append({"action_id": action_id, "tool": tool, "reason": reason})
        if self.admin:
            self.admin.report_harm(self.agent_id, action_id, reason)
        return reason
