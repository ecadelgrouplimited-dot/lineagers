// Guard Console. Everything shown here can come from an agent (tool inputs, reasons), so
// it is rendered with textContent only, never as HTML.
"use strict";

const LOG_ROWS = 60;
const POLL_MS = 2000;
const store = {
  get(key) { try { return sessionStorage.getItem(key); } catch { return null; } },
  set(key, value) { try { sessionStorage.setItem(key, value); } catch {} },
  clear() { try { sessionStorage.clear(); } catch {} },
};

let token = store.get("guard-token");
let approver = store.get("guard-approver");
let selected = null;
let timer = null;

const $ = (id) => document.getElementById(id);

function el(tag, text, className) {
  const node = document.createElement(tag);
  if (text !== undefined && text !== null) node.textContent = String(text);
  if (className) node.className = className;
  return node;
}

async function api(method, path, body) {
  const response = await fetch(path, {
    method,
    headers: { Authorization: `Bearer ${token}`, ...(body ? { "Content-Type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    const error = new Error(data.error || `HTTP ${response.status}`);
    error.status = response.status;
    throw error;
  }
  return data;
}

function showSignIn(message) {
  clearInterval(timer);
  $("app").hidden = true;
  $("sign-out").hidden = true;
  $("sign-in").hidden = false;
  $("approver").value = approver || "";
  $("sign-in-error").hidden = !message;
  $("sign-in-error").textContent = message || "";
}

async function signIn(event) {
  event.preventDefault();
  token = $("token").value.trim();
  approver = $("approver").value.trim();
  try {
    await api("GET", "/v1/agents");
  } catch (e) {
    showSignIn(e.status === 401 || e.status === 403 ? "That token was not accepted." : e.message);
    return;
  }
  store.set("guard-token", token);
  store.set("guard-approver", approver);
  $("token").value = "";
  start();
}

function start() {
  $("sign-in").hidden = true;
  $("app").hidden = false;
  $("sign-out").hidden = false;
  refresh();
  clearInterval(timer);
  timer = setInterval(refresh, POLL_MS);
}

async function refresh() {
  try {
    const { agents } = await api("GET", "/v1/agents");
    renderAgents(agents);
    if (selected) await renderDetail(selected);
  } catch (e) {
    if (e.status === 401) showSignIn("Session expired. Sign in again.");
  }
}

function renderAgents(agents) {
  const body = $("agents").querySelector("tbody");
  body.replaceChildren();
  $("no-agents").hidden = agents.length > 0;
  for (const agent of agents) {
    const row = el("tr");
    if (agent.agent_id === selected) row.className = "selected";
    row.addEventListener("click", () => { selected = agent.agent_id; $("verify-result").hidden = true; refresh(); });
    row.append(el("td", agent.agent_id));

    if (agent.state === "quarantined") {
      const cell = el("td");
      cell.append(el("span", "quarantined", "badge quarantined"));
      cell.title = agent.error;
      row.append(cell, el("td", "-"), el("td", "-"), el("td", "-"));
    } else {
      const s = agent.status;
      const state = el("td");
      state.append(el("span", s.alive ? "alive" : "terminated", `badge ${s.alive ? "alive" : "terminated"}`));
      const budget = el("td");
      const bar = el("span", null, "bar");
      const fill = el("span");
      fill.style.width = `${Math.min(100, (100 * s.spent) / Math.max(1, s.budget))}%`;
      bar.append(fill);
      budget.append(bar, document.createTextNode(`${s.spent} / ${s.budget}`));
      const pending = el("td", s.actions_pending || "", s.actions_pending ? "pending-count" : "");
      row.append(state, budget, el("td", `${s.scar_score} / ${s.scar_limit}`), pending);
    }
    body.append(row);
  }
}

async function renderDetail(agentId) {
  $("detail").hidden = false;
  $("detail-title").textContent = agentId;
  let status;
  try {
    status = await api("GET", `/v1/agents/${encodeURIComponent(agentId)}`);
  } catch (e) {
    $("detail-stats").replaceChildren(el("div", e.message, "error"));
    return;
  }
  $("terminate").disabled = !status.alive;

  const stats = [
    ["State", status.alive ? "alive" : `terminated: ${status.termination_reason}`],
    ["Spent", `${status.spent} of ${status.budget}`],
    ["Scars", `${status.scar_score} of ${status.scar_limit}`],
    ["Allowed", status.actions_allowed],
    ["Denied", status.actions_denied],
    ["Log head", `#${status.head.seq}`],
  ];
  $("detail-stats").replaceChildren(...stats.map(([k, v]) => {
    const box = el("div");
    box.append(el("dt", k), el("dd", v));
    return box;
  }));
  $("detail-scars").replaceChildren(...status.scars.map((s) => el("li", `${s.severity}: ${s.reason}`)));

  const after = status.head.seq >= LOG_ROWS ? `after=${status.head.seq - LOG_ROWS}&` : "";
  const [{ pending }, log] = await Promise.all([
    api("GET", `/v1/agents/${encodeURIComponent(agentId)}/actions/pending`),
    api("GET", `/v1/agents/${encodeURIComponent(agentId)}/log?${after}limit=${LOG_ROWS}`),
  ]);
  renderPending(agentId, pending);
  renderLog(log);
}

function renderPending(agentId, pending) {
  const container = $("pending");
  $("no-pending").hidden = pending.length > 0;
  const template = $("pending-item");
  container.replaceChildren(...pending.map((item) => {
    const node = template.content.firstElementChild.cloneNode(true);
    const action = item.action;
    node.querySelector(".tool").textContent = `${action.action_id}: ${action.tool} (cost ${action.cost})`;
    node.querySelector(".when").textContent = new Date(item.requested_at).toLocaleTimeString();
    node.querySelector(".input").textContent = JSON.stringify(action.input, null, 2);
    const path = `/v1/agents/${encodeURIComponent(agentId)}/actions/${encodeURIComponent(action.action_id)}`;
    const act = (fn) => async (event) => {
      node.querySelectorAll("button").forEach((b) => { b.disabled = true; });
      try { await fn(); } catch (e) { alert(e.message); }
      refresh();
    };
    node.querySelector(".approve").addEventListener("click", act(() => api("POST", `${path}/approve`, { approver })));
    node.querySelector(".reject").addEventListener("click", act(() => {
      const reason = prompt("Reason for rejecting?", "not approved");
      if (reason === null) return Promise.resolve();
      return api("POST", `${path}/reject`, { approver, reason });
    }));
    node.querySelector(".reject-scar").addEventListener("click", act(() => {
      const reason = prompt("Reason? The agent will be scarred (moderate).", "unsafe request");
      if (reason === null) return Promise.resolve();
      return api("POST", `${path}/reject`, { approver, reason, scar: "moderate" });
    }));
    return node;
  }));
}

function describe(record) {
  const p = record.payload || {};
  switch (record.kind) {
    case "action_requested": return `${p.action_id} ${p.tool} ${JSON.stringify(p.input)}`;
    case "action_allowed": return `${p.action_id}${p.approved_by ? ` approved by ${p.approved_by}` : ""}${p.note ? `: ${p.note}` : ""}`;
    case "action_denied": return `${p.action_id} ${p.reason ? p.reason.code : ""}`;
    case "action_rejected": return `${p.action_id} by ${p.by}: ${p.reason}`;
    case "outcome": return `${p.action_id} ${p.status}${p.detail ? `: ${p.detail}` : ""}`;
    case "scar": return `${p.severity}: ${p.reason}`;
    case "terminated": return `${p.reason} (by ${p.by})`;
    case "policy": return `budget ${p.policy.budget}, tools ${Object.keys(p.policy.tools).join(", ")}`;
    default: return JSON.stringify(p);
  }
}

function renderLog(log) {
  $("log-head").textContent = `head ${log.head.seq}:${log.head.hash.slice(0, 12)}…`;
  const body = $("log").querySelector("tbody");
  body.replaceChildren(...log.records.slice().reverse().map((r) => {
    const row = el("tr", null, `kind-${r.kind}`);
    row.append(el("td", r.seq), el("td", new Date(r.timestamp).toLocaleTimeString()), el("td", r.kind), el("td", describe(r)));
    return row;
  }));
}

async function verify() {
  const out = $("verify-result");
  out.hidden = false;
  out.className = "";
  out.textContent = "Verifying…";
  try {
    const report = await api("GET", `/v1/agents/${encodeURIComponent(selected)}/verify`);
    if (report.ok) {
      out.className = "ok";
      out.textContent = `Verified: ${report.records} records, signatures valid. Checkpoint ${report.head.seq}:${report.head.hash}`;
    } else {
      out.className = "error";
      out.textContent = `FAILED: ${report.failure.reason} (line ${report.failure.line}, seq ${report.failure.seq})`;
    }
  } catch (e) {
    out.className = "error";
    out.textContent = e.message;
  }
}

async function terminate() {
  const reason = prompt(`Terminate ${selected}? This is permanent.\n\nReason:`);
  if (!reason) return;
  try {
    await api("POST", `/v1/agents/${encodeURIComponent(selected)}/terminate`, { reason, by: approver });
  } catch (e) {
    alert(e.message);
  }
  refresh();
}

document.addEventListener("DOMContentLoaded", async () => {
  $("sign-in-form").addEventListener("submit", signIn);
  $("sign-out").addEventListener("click", () => { store.clear(); token = null; selected = null; showSignIn(); });
  $("verify").addEventListener("click", verify);
  $("terminate").addEventListener("click", terminate);
  try {
    const { public_key } = await (await fetch("/v1/public-key")).json();
    $("server-key").textContent = `public key ${public_key.slice(0, 16)}…`;
    $("server-key").title = public_key;
  } catch {}
  if (token) start(); else showSignIn();
});
