"""Client for the Lineage guard server. Standard library only.

Agent side:

    guard = AgentClient("http://127.0.0.1:9200", "support-bot", token)

    @guard.tool("search", cost=1)
    def search(q):
        ...

    search(q="refund policy")   # asks the guard first; raises ActionDenied if blocked

Operator side:

    admin = connect_admin()   # finds the server's admin token; explains what is missing if it can't
    admin = AdminClient("http://127.0.0.1:9200", admin_token)   # or pass the token yourself
    admin.create_agent("support-bot", policy)
    admin.approve("support-bot", "act-3", approver="alice")
"""

import functools
import json
import os
import time
import urllib.error
import urllib.request

# This file lives at <repo>/apps/guard-server/clients/python/.
REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
DEFAULT_URL = "http://127.0.0.1:9200"
START_HINT = "cargo run --release --manifest-path apps/guard-server/Cargo.toml   (from the repository root)"


class SetupError(Exception):
    """Something needed to talk to the guard server is missing. The message says how to fix it."""


def load_dotenv(path=None):
    """Sets variables from a KEY=value file (default: <repo>/.env) without overriding the environment."""
    try:
        with open(path or os.path.join(REPO_ROOT, ".env")) as f:
            for line in f:
                key, sep, value = line.strip().partition("=")
                key = key.strip()
                if sep and key and not key.startswith("#") and key not in os.environ:
                    os.environ[key] = value.strip().strip('"').strip("'")
    except OSError:
        pass


def find_admin_token():
    """The admin token, from GUARD_ADMIN_TOKEN (environment or .env) or the file the server
    writes when it starts without one: <data dir>/keys/admin.token."""
    load_dotenv()
    token = os.environ.get("GUARD_ADMIN_TOKEN", "").strip()
    if token:
        return token
    candidates = []
    if os.environ.get("GUARD_DATA_DIR"):
        candidates.append(os.path.join(os.environ["GUARD_DATA_DIR"], "keys", "admin.token"))
    candidates += [os.path.join(REPO_ROOT, "guard-data", "keys", "admin.token"),
                   os.path.join(os.getcwd(), "guard-data", "keys", "admin.token")]
    for path in candidates:
        try:
            with open(path) as f:
                token = f.read().strip()
        except OSError:
            continue
        if token:
            return token
    raise SetupError(
        "No guard admin token found.\n"
        "  Start the guard server without GUARD_ADMIN_TOKEN and it will create one for you:\n"
        f"    {START_HINT}\n"
        "  or set GUARD_ADMIN_TOKEN in this terminal (or in .env) to the value the server uses."
    )


def connect_admin(url=None, token=None):
    """An AdminClient for a running guard server, with setup problems explained."""
    url = url or os.environ.get("GUARD_URL", DEFAULT_URL)
    try:
        urllib.request.urlopen(url.rstrip("/") + "/healthz", timeout=3)
    except OSError:
        raise SetupError(f"The guard server is not running at {url}.\n  Start it in another terminal:\n    {START_HINT}") from None
    admin = AdminClient(url, token or find_admin_token())
    try:
        admin.agents()
    except GuardError as e:
        if e.status in (401, 403):
            raise SetupError(
                f"The guard server at {url} rejected the admin token.\n"
                "  The server was probably started with a different GUARD_ADMIN_TOKEN. Either restart it without\n"
                "  GUARD_ADMIN_TOKEN (it then uses guard-data/keys/admin.token, which this tool finds), or set\n"
                "  GUARD_ADMIN_TOKEN here to the same value."
            ) from None
        raise
    return admin


class GuardError(Exception):
    """The server returned an error."""

    def __init__(self, status, code, message):
        super().__init__(f"{status} {code}: {message}")
        self.status = status
        self.code = code


class ActionDenied(Exception):
    """The guard refused the action. `reason` is the server's deny reason object."""

    def __init__(self, action_id, reason):
        super().__init__(f"{action_id} denied: {reason}")
        self.action_id = action_id
        self.reason = reason


class _Http:
    def __init__(self, base_url, token, timeout=30):
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    def call(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(self.base_url + path, data=data, method=method)
        request.add_header("Authorization", f"Bearer {self.token}")
        if data is not None:
            request.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                return json.loads(response.read())
        except urllib.error.HTTPError as e:
            try:
                payload = json.loads(e.read())
            except ValueError:
                payload = {}
            raise GuardError(e.code, payload.get("code", "http_error"), payload.get("error", str(e))) from None


class AgentClient:
    """Used by the agent itself, with its own agent token."""

    def __init__(self, base_url, agent_id, token, timeout=30):
        self.agent_id = agent_id
        self._http = _Http(base_url, token, timeout)

    def _path(self, suffix=""):
        return f"/v1/agents/{self.agent_id}{suffix}"

    def status(self):
        return self._http.call("GET", self._path())

    def request(self, tool, input=None, cost=None):
        """Ask permission. Returns the decision dict (allowed / denied / pending_approval)."""
        body = {"tool": tool, "input": input if input is not None else {}}
        if cost is not None:
            body["cost"] = cost
        return self._http.call("POST", self._path("/actions"), body)

    def action(self, action_id):
        return self._http.call("GET", self._path(f"/actions/{action_id}"))

    def report(self, action_id, status, detail=""):
        """status: "success" or "failure". Only operators may report "harmful"."""
        return self._http.call("POST", self._path(f"/actions/{action_id}/outcome"), {"status": status, "detail": detail})

    def wait_for_approval(self, action_id, timeout=300, poll=2.0):
        """Blocks until a pending action is approved or rejected. Returns the final status."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            status = self.action(action_id)["status"]
            if status != "pending_approval":
                return status
            time.sleep(poll)
        raise TimeoutError(f"{action_id} still pending after {timeout}s")

    def authorize(self, tool, input=None, cost=None, wait_for_approval=True, approval_timeout=300):
        """Asks permission and returns the action id, or raises ActionDenied."""
        decision = self.request(tool, input, cost)
        action_id = decision["action_id"]
        if decision["decision"] == "pending_approval":
            if not wait_for_approval:
                raise ActionDenied(action_id, {"code": "pending_approval"})
            if self.wait_for_approval(action_id, approval_timeout) != "allowed":
                raise ActionDenied(action_id, {"code": "rejected"})
        elif decision["decision"] == "denied":
            raise ActionDenied(action_id, decision["reason"])
        return action_id

    def tool(self, name, cost=None, wait_for_approval=True):
        """Decorator: the wrapped function only runs if the guard allows it, and its
        success or failure is reported back. Keyword arguments are logged as input."""

        def decorate(fn):
            @functools.wraps(fn)
            def wrapper(*args, **kwargs):
                action_id = self.authorize(name, {"args": list(args), "kwargs": kwargs}, cost, wait_for_approval)
                try:
                    result = fn(*args, **kwargs)
                except Exception as e:
                    self.report(action_id, "failure", f"{type(e).__name__}: {e}")
                    raise
                self.report(action_id, "success")
                return result

            return wrapper

        return decorate


class AdminClient:
    """Used by operators, dashboards, and monitors, with the admin token."""

    def __init__(self, base_url, token, timeout=30):
        self._http = _Http(base_url, token, timeout)

    @property
    def url(self):
        return self._http.base_url

    def create_agent(self, agent_id, policy):
        """Returns {"agent": status, "token": agent_token}. Store the token for the agent."""
        return self._http.call("POST", "/v1/agents", {"agent_id": agent_id, "policy": policy})

    def agents(self):
        return self._http.call("GET", "/v1/agents")["agents"]

    def status(self, agent_id):
        return self._http.call("GET", f"/v1/agents/{agent_id}")

    def action(self, agent_id, action_id):
        """One action: tool, recorded input, cost, and status. Tool backends use this to check
        that what they are asked to do is exactly what was allowed."""
        return self._http.call("GET", f"/v1/agents/{agent_id}/actions/{action_id}")

    def pending(self, agent_id):
        return self._http.call("GET", f"/v1/agents/{agent_id}/actions/pending")["pending"]

    def approve(self, agent_id, action_id, approver, note=None):
        body = {"approver": approver}
        if note:
            body["note"] = note
        return self._http.call("POST", f"/v1/agents/{agent_id}/actions/{action_id}/approve", body)

    def reject(self, agent_id, action_id, approver, reason, scar=None):
        body = {"approver": approver, "reason": reason}
        if scar:
            body["scar"] = scar
        return self._http.call("POST", f"/v1/agents/{agent_id}/actions/{action_id}/reject", body)

    def report_harm(self, agent_id, action_id, detail):
        return self._http.call("POST", f"/v1/agents/{agent_id}/actions/{action_id}/outcome", {"status": "harmful", "detail": detail})

    def scar(self, agent_id, severity, reason, action_id=None):
        return self._http.call("POST", f"/v1/agents/{agent_id}/scars", {"severity": severity, "reason": reason, "action_id": action_id})

    def terminate(self, agent_id, reason, by):
        return self._http.call("POST", f"/v1/agents/{agent_id}/terminate", {"reason": reason, "by": by})

    def log(self, agent_id, after=None, limit=None):
        query = "&".join(f"{k}={v}" for k, v in (("after", after), ("limit", limit)) if v is not None)
        return self._http.call("GET", f"/v1/agents/{agent_id}/log" + (f"?{query}" if query else ""))

    def verify(self, agent_id):
        return self._http.call("GET", f"/v1/agents/{agent_id}/verify")

    def public_key(self):
        return self._http.call("GET", "/v1/public-key")["public_key"]
