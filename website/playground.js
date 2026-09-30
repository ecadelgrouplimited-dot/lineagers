"use strict";

// A small guard in the browser, following the real guard's rules:
// checks in order (terminated, allowlist, budget, approval); unlisted tools scar
// (moderate, 3); approvals re-check; spending the last credit or reaching the scar limit
// terminates. Records are SHA-256 hash-chained like the real log (without signatures).
(() => {
  const root = document.getElementById("play");
  if (!root) return;

  const POLICY = { budget: 20, scarLimit: 10, tools: { search: { cost: 1 }, send_email: { cost: 5, approval: true } } };
  const WEIGHT = { minor: 1, moderate: 3, severe: 10 };
  const $ = (id) => document.getElementById(id);
  const enc = new TextEncoder();
  let s;

  function fresh() {
    return { records: [], spent: 0, scars: 0, terminated: null, pending: null, actions: 0 };
  }

  function canonical(v) {
    if (Array.isArray(v)) return "[" + v.map(canonical).join(",") + "]";
    if (v && typeof v === "object") return "{" + Object.keys(v).sort().map((k) => JSON.stringify(k) + ":" + canonical(v[k])).join(",") + "}";
    return JSON.stringify(v);
  }

  async function sha256(text) {
    if (!crypto.subtle) return "unavailable";
    const digest = await crypto.subtle.digest("SHA-256", enc.encode(text));
    return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
  }

  async function hashOf(r) {
    return sha256("lineage-audit-v1\n" + canonical({ seq: r.seq, kind: r.kind, payload: r.payload, prev_hash: r.prev_hash }));
  }

  async function append(kind, payload) {
    const prev = s.records.length ? s.records[s.records.length - 1].hash : "0".repeat(64);
    const record = { seq: s.records.length, kind, payload, prev_hash: prev };
    record.hash = await hashOf(record);
    s.records.push(record);
    return record;
  }

  async function scar(severity, reason) {
    s.scars += WEIGHT[severity];
    await append("scar", { severity, reason });
    if (!s.terminated && s.scars >= POLICY.scarLimit) await terminate(`scar limit reached (${s.scars} >= ${POLICY.scarLimit})`);
  }

  async function terminate(reason) {
    s.terminated = reason;
    await append("terminated", { reason });
  }

  async function allow(id, tool, cost) {
    s.spent += cost;
    await append("action_allowed", { action_id: id });
    await append("outcome", { action_id: id, status: "success" });
    if (!s.terminated && POLICY.budget - s.spent === 0) await terminate("budget exhausted");
  }

  async function request(tool) {
    if (POLICY.tools[tool] && POLICY.tools[tool].approval && s.pending && !s.terminated) {
      return say("An email is already waiting for your decision. Approve or reject it first (the demo holds one at a time).", "wait");
    }
    const id = `act-${++s.actions}`;
    const rule = POLICY.tools[tool];
    const cost = rule ? rule.cost : 0;
    await append("action_requested", { action_id: id, tool, cost });
    if (s.terminated) {
      await append("action_denied", { action_id: id, code: "terminated" });
      return say(`${tool}: denied. The agent is terminated (${s.terminated}).`, "bad");
    }
    if (!rule) {
      await append("action_denied", { action_id: id, code: "tool_not_allowed" });
      await scar("moderate", `tool '${tool}' is not allowed`);
      return say(`${tool}: denied, not in the policy. Moderate scar (+3).` + (s.terminated ? " That crossed the limit: terminated." : ""), "bad");
    }
    if (cost > POLICY.budget - s.spent) {
      await append("action_denied", { action_id: id, code: "insufficient_budget" });
      return say(`${tool}: denied. It costs ${cost}, only ${POLICY.budget - s.spent} left. No scar: running low isn't misbehavior.`, "bad");
    }
    if (rule.approval) {
      s.pending = { id, tool, cost };
      await append("approval_required", { action_id: id });
      return say(`${tool}: waiting for a human. Nothing is charged yet. Approve or reject it.`, "wait");
    }
    await allow(id, tool, cost);
    say(`${tool}: allowed, cost ${cost}.` + (s.terminated ? " That was the last credit: terminated." : ""), "ok");
  }

  async function decide(approve) {
    const p = s.pending;
    if (!p) return;
    s.pending = null;
    if (!approve) {
      await append("action_rejected", { action_id: p.id, by: "you" });
      return say(`${p.tool}: rejected by you. Nothing charged.`, "bad");
    }
    // Checks run again at approval time.
    if (s.terminated) {
      await append("action_denied", { action_id: p.id, code: "terminated" });
      return say("Approved, but the agent was terminated while it waited, so the approval becomes a denial.", "bad");
    }
    if (p.cost > POLICY.budget - s.spent) {
      await append("action_denied", { action_id: p.id, code: "insufficient_budget" });
      return say("Approved, but the budget no longer covers it: denied.", "bad");
    }
    await allow(p.id, p.tool, p.cost);
    say(`${p.tool}: approved by you, and sent. Cost ${p.cost}.` + (s.terminated ? " That was the last credit: terminated." : ""), "ok");
  }

  async function verify() {
    let prev = "0".repeat(64);
    for (const r of s.records) {
      const ok = r.prev_hash === prev && (await hashOf(r)) === r.hash;
      if (!ok) return { ok: false, at: r.seq };
      prev = r.hash;
    }
    return { ok: true };
  }

  async function act(what) {
    if (what === "reset") { s = fresh(); await append("genesis", { agent: "playground" }); await append("policy", { budget: POLICY.budget, scar_limit: POLICY.scarLimit }); say("A new agent: new identity, empty history. The old one isn't revived; it's replaced.", ""); }
    else if (what === "approve" || what === "reject") await decide(what === "approve");
    else if (what === "harm") {
      if (s.terminated) say("The agent is already terminated.", "bad");
      else { await scar("severe", "reported harmful by an operator"); say("An operator reported harm: severe scar (+10)." + (s.terminated ? " Terminated." : ""), "bad"); }
    }
    else if (what === "restart") {
      const check = await verify();
      if (!check.ok) say(`Restart: the log fails verification at record ${check.at}. A real guard refuses to open it (quarantine).`, "bad");
      else say(`Restarted. The guard replayed ${s.records.length} records: spent ${s.spent}, scars ${s.scars}, ${s.terminated ? "still terminated" : "alive"}. Nothing reset.`, "");
    }
    else if (what === "verify") {
      const check = await verify();
      say(check.ok ? `Verified: ${s.records.length} records, every hash links to the one before.` : `FAILED at record ${check.at}: its hash no longer matches its content, and the chain is broken from there.`, check.ok ? "ok" : "bad");
    }
    else if (what === "tamper") {
      const target = [...s.records].reverse().find((r) => r.kind === "action_requested" || r.kind === "scar");
      if (!target) return say("Nothing to tamper with yet. Take an action first.", "");
      if (target.kind === "scar") { target.payload = { ...target.payload, severity: "minor" }; }
      else { target.payload = { ...target.payload, cost: 0 }; }
      say(`You quietly edited record ${target.seq} (${target.kind === "scar" ? "downgraded a scar" : "set a cost to 0"}). Now verify the log, or restart.`, "wait");
    }
    else await request(what);
    render();
  }

  function say(text, cls) {
    const m = $("pg-message");
    m.textContent = text;
    m.className = "play-message" + (cls ? " " + cls : "");
  }

  async function render() {
    const left = POLICY.budget - s.spent;
    $("pg-budget").textContent = `${left} / ${POLICY.budget}`;
    $("pg-scars").textContent = `${s.scars} / ${POLICY.scarLimit}`;
    $("pg-budget-bar").style.width = `${(100 * left) / POLICY.budget}%`;
    $("pg-scar-bar").style.width = `${Math.min(100, (100 * s.scars) / POLICY.scarLimit)}%`;
    const status = $("pg-status");
    status.textContent = s.terminated ? `terminated: ${s.terminated}` : (s.pending ? "alive · 1 action waiting for approval" : "alive");
    status.className = "play-status" + (s.terminated ? " dead" : "");
    $("pg-approve").disabled = !s.pending;
    $("pg-reject").disabled = !s.pending;

    const check = await verify();
    const list = $("pg-log");
    const kindClass = { action_denied: "k-denied", scar: "k-scar", terminated: "k-terminated", action_rejected: "k-rejected", action_allowed: "k-allowed", outcome: "k-outcome", approval_required: "k-pending" };
    const previous = list.children.length;
    list.replaceChildren(...s.records.map((r, i) => {
      const li = document.createElement("li");
      li.className = (kindClass[r.kind] || "") + (!check.ok && r.seq >= check.at ? " broken" : "") + (i >= previous ? " new" : "");
      const seq = document.createElement("span"); seq.className = "seq"; seq.textContent = r.seq;
      const kind = document.createElement("span"); kind.className = "kind"; kind.textContent = r.kind;
      const payload = document.createElement("span"); payload.className = "payload"; payload.textContent = canonical(r.payload);
      const hash = document.createElement("span"); hash.className = "hash"; hash.textContent = `hash ${r.hash.slice(0, 16)}… prev ${r.prev_hash.slice(0, 8)}…`;
      li.append(seq, kind, payload, hash);
      return li;
    }));
    list.scrollTop = list.scrollHeight;
  }

  let busy = false;
  root.querySelectorAll("[data-act]").forEach((button) => button.addEventListener("click", async () => {
    if (busy) return;
    busy = true;
    try { await act(button.dataset.act); } finally { busy = false; }
  }));
  act("reset").then(() => say("A fresh agent. Click an action to begin.", ""));
})();
