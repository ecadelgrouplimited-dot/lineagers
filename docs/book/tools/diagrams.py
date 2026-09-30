"""Generates the documentation's diagrams as theme-aware inline SVG.

    python3 docs/book/tools/diagrams.py        # writes docs/book/src/diagrams/*.html

Each diagram is a <figure> with an inline <svg> (role="img", aria-label) and a caption.
Shapes use CSS classes from docs/book/theme/lineage.css, and text uses currentColor, so
every figure follows the reader's theme. Pages include them with {{#include}}.
"""

import html
import os

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src", "diagrams")


class Diagram:
    def __init__(self, name, width, height, label, caption):
        self.name, self.w, self.h, self.label, self.caption = name, width, height, label, caption
        self.parts = []

    def box(self, x, y, w, h, title, sub=None, kind="b", title_size=None):
        self.parts.append(f'<rect class="{kind}" x="{x}" y="{y}" width="{w}" height="{h}" rx="8"/>')
        cy = y + h / 2 + (-4 if sub else 5)
        size = f' font-size="{title_size}"' if title_size else ""
        self.parts.append(f'<text class="t tb" x="{x + w / 2}" y="{cy}" text-anchor="middle"{size}>{html.escape(title)}</text>')
        if sub:
            self.parts.append(f'<text class="s" x="{x + w / 2}" y="{cy + 17}" text-anchor="middle">{html.escape(sub)}</text>')
        return self

    def pill(self, x, y, w, h, text, kind="b"):
        self.parts.append(f'<rect class="{kind}" x="{x}" y="{y}" width="{w}" height="{h}" rx="{h / 2}"/>')
        self.parts.append(f'<text class="t" x="{x + w / 2}" y="{y + h / 2 + 4.5}" text-anchor="middle">{html.escape(text)}</text>')
        return self

    def arrow(self, *points, label=None, at=None, kind="l", dashed=False, anchor="middle"):
        pts = " ".join(f"{x},{y}" for x, y in points)
        dash = ' stroke-dasharray="5 4"' if dashed else ""
        marker = {"l": "a", "ok": "aok", "bad": "abad", "acc": "aacc"}[kind]
        self.parts.append(f'<polyline class="{kind}" points="{pts}"{dash} marker-end="url(#{self.name}-{marker})"/>')
        if label:
            lx, ly = at or ((points[0][0] + points[-1][0]) / 2, (points[0][1] + points[-1][1]) / 2 - 7)
            self.parts.append(f'<text class="s" x="{lx}" y="{ly}" text-anchor="{anchor}">{html.escape(label)}</text>')
        return self

    def line(self, *points, kind="l", dashed=False):
        pts = " ".join(f"{x},{y}" for x, y in points)
        dash = ' stroke-dasharray="5 4"' if dashed else ""
        self.parts.append(f'<polyline class="{kind}" points="{pts}"{dash}/>')
        return self

    def text(self, x, y, s, cls="s", anchor="middle"):
        self.parts.append(f'<text class="{cls}" x="{x}" y="{y}" text-anchor="{anchor}">{html.escape(s)}</text>')
        return self

    def rect(self, x, y, w, h, kind="b", rx=4):
        self.parts.append(f'<rect class="{kind}" x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}"/>')
        return self

    def render(self):
        n = self.name
        defs = "".join(
            f'<marker id="{n}-{m}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
            f'<path class="{c}" d="M0 0L10 5L0 10z"/></marker>'
            for m, c in (("a", "mk"), ("aok", "mkok"), ("abad", "mkbad"), ("aacc", "mkacc")))
        body = "".join(self.parts)
        return (f'<figure class="lx-fig">'
                f'<svg viewBox="0 0 {self.w} {self.h}" role="img" aria-label="{html.escape(self.label)}">'
                f'<defs>{defs}</defs>{body}</svg>'
                f'<figcaption>{html.escape(self.caption)}</figcaption></figure>\n')


def architecture():
    d = Diagram("arch", 860, 330, "Agents ask the Lineage guard before calling tools; operators approve risky actions; every decision is written to a signed audit log that auditors verify with the public key.",
                "The guard sits between an agent and its tools. Operators approve what the policy holds back. Everything lands in a signed log that anyone can verify.")
    d.box(20, 40, 180, 76, "Your agent", "Claude, DeepSeek, any code")
    d.box(340, 40, 180, 76, "Lineage guard", "policy · budget · scars", kind="ba")
    d.box(660, 40, 180, 76, "Tools", "shell · email · payments")
    d.box(20, 220, 180, 76, "Operators", "approve · reject · kill")
    d.box(340, 220, 180, 76, "Signed audit log", "hash-chained · Ed25519")
    d.box(660, 220, 180, 76, "Auditors", "verify offline")
    d.arrow((200, 66), (334, 66), label="1. may I?")
    d.arrow((334, 92), (206, 92), label="2. allowed / denied / wait", at=(270, 110))
    d.arrow((520, 78), (654, 78), label="3. only if allowed", kind="ok")
    d.arrow((110, 216), (110, 170), (400, 170), (400, 122), label="approve / reject", at=(250, 162))
    d.arrow((460, 116), (460, 214), label="every step", at=(506, 170), anchor="middle")
    d.arrow((520, 258), (654, 258), label="public key + checkpoint")
    return d


def action_lifecycle():
    d = Diagram("life", 860, 250, "An action is requested, then either denied, held pending approval, or allowed; pending actions are approved (re-checked) or rejected; allowed actions end with a reported outcome.",
                "The states an action moves through. Every transition is one signed record in the agent's log.")
    d.pill(20, 100, 130, 40, "requested")
    d.pill(250, 30, 170, 40, "pending_approval", kind="bacc")
    d.pill(250, 170, 130, 40, "denied", kind="bbad")
    d.pill(520, 100, 130, 40, "allowed", kind="bok")
    d.pill(720, 100, 120, 40, "completed", kind="bok")
    d.arrow((150, 112), (246, 56), label="needs a human", at=(170, 72), anchor="start")
    d.arrow((150, 128), (246, 186), label="a check fails", at=(160, 176), anchor="start")
    d.arrow((150, 120), (516, 120), label="checks pass", at=(460, 112))
    d.arrow((420, 50), (560, 50), (580, 96), label="approved + re-checked", at=(470, 42), kind="ok")
    d.arrow((335, 70), (335, 166), label="rejected", at=(343, 150), kind="bad", anchor="start")
    d.arrow((650, 120), (716, 120), label="outcome", at=(683, 110))
    d.text(780, 162, "success · failure · harmful")
    return d


def checks():
    d = Diagram("checks", 860, 250, "The guard checks, in order: is the agent terminated, is the tool allowed, is the tool's call cap reached, is the rate limit reached, is there enough budget, does it need approval. The first failure denies the request.",
                "Checks run in this order and the first failure wins. The scar each denial leaves is shown underneath.")
    steps = [("alive?", "terminated", "no scar"), ("tool allowed?", "tool_not_allowed", "moderate"), ("call cap?", "tool_call_limit", "minor"),
             ("rate limit?", "rate_limited", "minor"), ("budget?", "insufficient_budget", "no scar")]
    x = 20
    for i, (q, code, scar) in enumerate(steps):
        d.box(x, 40, 124, 56, q)
        d.arrow((x + 62, 96), (x + 62, 150), kind="bad")
        d.text(x + 62, 170, code, cls="s mono")
        d.text(x + 62, 188, scar, cls="s")
        d.arrow((x + 124, 68), (x + 138, 68))
        x += 140
    d.box(720, 40, 120, 56, "approval?", kind="ba")
    d.arrow((780, 96), (780, 150), kind="acc")
    d.text(780, 170, "pending_approval", cls="s mono")
    d.text(780, 188, "waits for a human", cls="s")
    d.text(430, 226, "all pass → allowed, budget charged", cls="t okt")
    return d


def scars():
    d = Diagram("scars", 860, 210, "Scars accumulate toward the limit: a failure adds 1, an unlisted tool adds 3, a hit rate limit adds 1, and a harmful outcome adds 10, which crosses the limit of 10 and terminates the agent.",
                "Scar weights add up toward the policy's scar_limit. Crossing it terminates the agent, permanently.")
    unit, x0, y = 64, 40, 70
    parts = [(1, "failure", "b"), (3, "unlisted tool", "bacc"), (1, "rate limit", "b"), (10, "harmful", "bbad")]
    x = x0
    for weight, what, kind in parts:
        width = min(weight, 6) * unit
        d.rect(x, y, width, 40, kind=kind, rx=4)
        d.text(x + width / 2, y + 25, f"+{weight}", cls="t tb", anchor="middle")
        d.text(x + width / 2, y + 60, what, cls="s", anchor="middle")
        x += width
    d.text(x - 20, y + 25, "…", cls="t tb", anchor="middle")
    limit_x = x0 + 10 * unit
    d.line((limit_x, 44), (limit_x, 124), kind="bad", dashed=True)
    d.text(limit_x, 36, "scar_limit = 10", cls="t badt", anchor="middle")
    d.text(x0, 60, "0", anchor="middle")
    d.text(430, 186, "score 15 ≥ 10 → terminated: every later request is denied", cls="t badt", anchor="middle")
    return d


def hash_chain():
    d = Diagram("chain", 860, 250, "Each record stores the previous record's hash and its own hash and signature; changing one record breaks its own hash and every link after it.",
                "Each record carries the previous record's hash and is signed. Editing one breaks its hash, and every link after it.")
    labels = [("0 genesis", "public key"), ("1 policy", "budget 100"), ("2 requested", "pay $120"), ("3 allowed", "by alice"), ("4 outcome", "success")]
    x = 20
    for i, (title, sub) in enumerate(labels):
        kind = "bbad" if i == 2 else "b"
        d.box(x, 50, 148, 80, title, sub, kind=kind)
        d.text(x + 74, 150, "hash · signature", cls="s mono")
        if i < len(labels) - 1:
            d.arrow((x + 148, 90), (x + 170, 90), kind="bad" if i >= 1 else "l")
        x += 170
    d.text(20 + 2 * 170 + 74, 186, "edited: $120 → $12", cls="t badt")
    d.text(20 + 2 * 170 + 74, 206, "hash no longer matches content", cls="s")
    d.text(700, 186, "chain broken from here on", cls="t badt")
    d.text(94, 186, "prev_hash links every record", cls="s")
    d.text(94, 206, "to the one before", cls="s")
    return d


def replay():
    d = Diagram("replay", 860, 190, "On restart, Guard::open verifies the log and replays every record to rebuild the agent's state: spent budget, scars, pending approvals, and whether it is alive.",
                "A restart doesn't reset anything. Guard::open verifies the log, then replays it into the same state it had before.")
    d.box(20, 50, 190, 80, "agent.jsonl", "the only state there is")
    d.box(300, 50, 200, 80, "Guard::open", "verify, then replay", kind="ba")
    d.box(590, 30, 250, 120, "")
    for i, s in enumerate(["spent budget: 6 / 20", "scars: 6 / 6", "pending approvals: 1", "alive: no"]):
        d.text(610, 58 + i * 24, s, cls="t", anchor="start")
    d.arrow((210, 90), (294, 90), label="read")
    d.arrow((500, 90), (584, 90), label="rebuild")
    d.text(430, 172, "tampered log → refused (quarantined on the server)", cls="s badt")
    return d


def project_layouts():
    d = Diagram("layouts", 860, 330, "Two ways to build: a Rust program using the guard in-process, with its log and key in lineage-data; or a Python agent calling a guard server over HTTP, with the server's keys and logs in guard-data.",
                "In-process (Rust) or over HTTP (any language). Same guard, same log format, same guarantees.")
    d.text(210, 26, "In-process: lineage new my-bot --template rust", cls="t tb")
    d.box(40, 50, 340, 110, "")
    d.text(60, 78, "cargo run", cls="t mono", anchor="start")
    d.box(70, 92, 130, 52, "your code", "src/main.rs")
    d.box(220, 92, 140, 52, "Guard", "lineage-rs", kind="ba")
    d.arrow((200, 118), (216, 118))
    d.box(40, 200, 340, 100, "")
    d.text(60, 226, "lineage-data/", cls="t mono", anchor="start")
    for i, (name, what) in enumerate([("audit.key", "signing key"), ("my-bot.jsonl", "signed log")]):
        d.text(80, 252 + i * 22, name, cls="s mono", anchor="start")
        d.text(210, 252 + i * 22, what, cls="s", anchor="start")
    d.arrow((290, 144), (290, 196), label="writes", at=(330, 176))
    d.text(650, 26, "Over HTTP: lineage new my-bot --template python", cls="t tb")
    d.box(470, 50, 170, 110, "agent.py", "policy.json · model.py")
    d.box(680, 50, 160, 110, "guard-server", ":9200 + console", kind="ba")
    d.arrow((640, 92), (676, 92), label="HTTP")
    d.arrow((676, 126), (644, 126))
    d.box(560, 200, 280, 100, "")
    d.text(580, 226, "guard-data/", cls="t mono", anchor="start")
    for i, (name, what) in enumerate([("keys/", "audit, token, admin"), ("agents/", "my-bot.jsonl, …")]):
        d.text(600, 252 + i * 22, name, cls="s mono", anchor="start")
        d.text(680, 252 + i * 22, what, cls="s", anchor="start")
    d.arrow((760, 160), (760, 196), label="writes", at=(800, 184))
    return d


def running():
    d = Diagram("running", 860, 270, "Running a project: the agent (terminal 2) and a browser both talk to guard-server (terminal 1) on port 9200; the server keeps its keys and signed logs in guard-data.",
                "Three things run: your agent, the guard server, and (when a human approves) a browser. Only the server touches guard-data/.")
    d.box(20, 50, 230, 80, "Terminal 2: your agent", "python3 agent.py")
    d.box(315, 50, 230, 80, "Terminal 1: guard-server", "127.0.0.1:9200", kind="ba")
    d.box(610, 50, 230, 80, "Browser: console", "http://127.0.0.1:9200/")
    d.box(315, 190, 230, 64, "guard-data/", "keys and signed logs")
    d.arrow((250, 90), (309, 90), label="HTTP", at=(280, 82))
    d.arrow((610, 90), (551, 90), label="approve", at=(580, 82), kind="acc")
    d.arrow((430, 130), (430, 184), label="writes", at=(470, 162))
    d.text(135, 152, "finds the admin token itself:", cls="s", anchor="middle")
    d.text(135, 170, "GUARD_ADMIN_TOKEN, .env, or", cls="s", anchor="middle")
    d.text(135, 188, "guard-data/keys/admin.token", cls="s mono", anchor="middle")
    return d


def app_anatomy():
    d = Diagram("anatomy", 860, 320, "Anatomy of a guarded app: the model proposes tool calls; the loop asks the guard; a reviewer checks what the policy holds back; the tool backend re-checks the approval; a monitor filters tool output and reports harm; everything is logged.",
                "The pieces of a production agent, and where each check happens.")
    xs, w = [20, 197, 374, 551, 728], 112
    names = [("Model", "LLM or script"), ("Agent loop", "your code"), ("Guard", "policy decides"), ("Reviewer", "rules, model, human"), ("Backend", "re-checks")]
    for x, (title, sub) in zip(xs, names):
        d.box(x, 60, w, 70, title, sub, kind="ba" if title == "Guard" else "b")
    labels = ["tool call", "request", "pending", "allowed"]
    for i, label in enumerate(labels):
        a, b = xs[i] + w, xs[i + 1]
        d.arrow((a, 84), (b - 4, 84))
        d.text((a + b) / 2, 50, label, anchor="middle")
    d.arrow((xs[1], 110), (xs[0] + w + 4, 110))
    d.text((xs[0] + w + xs[1]) / 2, 150, "result or denial", anchor="middle")
    d.box(197, 210, 112, 70, "Monitor", "redact, report harm")
    d.box(374, 210, 112, 70, "Signed log", "every step")
    d.box(551, 210, 289, 70, "Tests", "scripted model + real guard server")
    d.arrow((253, 130), (253, 204), label="tool output", at=(206, 172), anchor="end")
    d.arrow((430, 130), (430, 204))
    d.arrow((309, 245), (368, 245), label="harm", at=(338, 237))
    return d


def mastery_path():
    d = Diagram("path", 860, 190, "The Lineage Mastery path: levels 1 to 6 in Rust (first action, budgets, scars, approvals, restarts, audit), levels 7 to 9 in Python against the guard server (server, LLM loop, binding approvals), and level 10, production.",
                "Ten levels. Each one is a program you run; each builds on the one before.")
    levels = ["first action", "budgets", "scars", "approvals", "restarts", "audit", "server", "LLM loop", "binding", "production"]
    for i, name in enumerate(levels):
        x = 20 + i * 84
        kind = "ba" if i == 9 else ("bok" if i >= 6 else "b")
        d.rect(x, 60, 72, 56, kind=kind, rx=8)
        d.text(x + 36, 84, str(i + 1), cls="t tb")
        d.text(x + 36, 104, name, cls="s")
        if i < 9:
            d.arrow((x + 72, 88), (x + 82, 88))
    d.text(270, 40, "Rust, in-process: cargo run --example mastery_0N_…", cls="s")
    d.text(650, 40, "Python + guard server", cls="s")
    d.line((20, 136), (516, 136))
    d.line((524, 136), (768, 136), kind="ok")
    d.text(270, 160, "levels 1–6", cls="s")
    d.text(646, 160, "levels 7–9", cls="s")
    d.text(806, 160, "deploy", cls="s")
    return d


def without_with():
    d = Diagram("compare", 860, 300, "Without Lineage, a prompt-injected agent calls the shell and the payment API directly and the only record is an editable log. With Lineage, the same requests go through the guard: the shell call is denied, the payment waits for approval, and every step is signed.",
                "The same prompt-injected agent, without and with the guard.")
    d.text(210, 26, "Without Lineage", cls="t tb")
    d.box(30, 50, 150, 60, "agent", "reads injected text")
    d.box(250, 40, 150, 40, "run_shell ✓", kind="bbad")
    d.box(250, 100, 150, 40, "pay $9,800 ✓", kind="bbad")
    d.arrow((180, 70), (246, 60), kind="bad")
    d.arrow((180, 90), (246, 120), kind="bad")
    d.box(30, 180, 370, 70, "app.log", "editable, unsigned, can be deleted")
    d.text(650, 26, "With Lineage", cls="t tb")
    d.box(460, 50, 120, 60, "agent", "same text")
    d.box(610, 50, 90, 60, "guard", kind="ba")
    d.box(730, 30, 110, 40, "run_shell ✗", kind="bok")
    d.box(730, 90, 110, 40, "pay: waits", kind="bacc")
    d.arrow((580, 80), (606, 80))
    d.arrow((700, 66), (726, 52), kind="ok")
    d.arrow((700, 94), (726, 108), kind="acc")
    d.box(460, 180, 380, 70, "signed log", "request, denial, scar, approval: provable")
    d.arrow((655, 110), (655, 176))
    return d


ALL = [architecture, action_lifecycle, checks, scars, hash_chain, replay, project_layouts, running, app_anatomy, mastery_path, without_with]

if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for make in ALL:
        d = make()
        with open(os.path.join(OUT, f"{d.name}.html"), "w") as f:
            f.write(d.render())
        print("wrote", os.path.join("docs/book/src/diagrams", f"{d.name}.html"))
