//! # Agent Guard
//!
//! A policy gate for autonomous agents (LLM tool loops, bots, workers). Before an agent
//! acts it asks the guard; the guard allows, denies, or holds the action for human
//! approval, and every step lands in a signed [`AuditLog`].
//!
//! ## What This Enforces
//! - Tools not on the policy's allowlist are denied and scar the agent
//! - A finite budget: spent credits are never refunded
//! - Optional rate limit against runaway loops
//! - Scars accumulate from violations and bad outcomes; past the limit the agent is terminated
//! - Termination is permanent: every later request is denied
//!
//! ## Why State Comes From The Log
//! The guard keeps no state file of its own. [`Guard::open`] verifies the audit log and
//! replays it, so restarting a process cannot refund budget, heal scars, or revive a
//! terminated agent. The policy is written into the log at creation and cannot be
//! loosened afterwards: create a new agent instead.
//!
//! ## Example
//!
//! ```no_run
//! use lineage::audit::AuditKey;
//! use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
//! use serde_json::json;
//!
//! let policy = Policy::new(100)
//!     .allow("search", ToolRule::cost(1))
//!     .allow("send_email", ToolRule::cost(5).with_approval());
//!
//! let key = AuditKey::load_or_create("audit.key").unwrap();
//! let mut guard = Guard::create("agents/support-bot.jsonl", key, "support-bot", policy).unwrap();
//!
//! match guard.request("search", json!({"q": "refund policy"}), None).unwrap() {
//!     Decision::Allowed { action_id, .. } => {
//!         // ... run the tool ...
//!         guard.report(&action_id, Outcome::success("3 results")).unwrap();
//!     }
//!     Decision::Denied { reason, .. } => eprintln!("blocked: {}", reason),
//!     Decision::PendingApproval { action_id } => eprintln!("waiting for a human: {}", action_id),
//! }
//! ```

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::audit::{self, AuditError, AuditKey, AuditLog, Checkpoint, Record};

/// Record kinds written by the guard.
pub mod kinds {
    pub const POLICY: &str = "policy";
    pub const ACTION_REQUESTED: &str = "action_requested";
    pub const ACTION_ALLOWED: &str = "action_allowed";
    pub const ACTION_DENIED: &str = "action_denied";
    pub const APPROVAL_REQUIRED: &str = "approval_required";
    pub const ACTION_REJECTED: &str = "action_rejected";
    pub const OUTCOME: &str = "outcome";
    pub const SCAR: &str = "scar";
    pub const TERMINATED: &str = "terminated";
}

/// Rules for one tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRule {
    /// Minimum credits charged per call. A request may declare a higher cost, never lower.
    pub cost: u64,
    /// Hold every call until a human approves it.
    #[serde(default)]
    pub requires_approval: bool,
    /// Lifetime cap on allowed calls of this tool.
    #[serde(default)]
    pub max_calls: Option<u64>,
}

impl ToolRule {
    pub fn cost(cost: u64) -> Self {
        ToolRule { cost, requires_approval: false, max_calls: None }
    }

    pub fn with_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }

    pub fn with_max_calls(mut self, max_calls: u64) -> Self {
        self.max_calls = Some(max_calls);
        self
    }
}

/// At most `max_actions` allowed actions in any `window_secs` window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimit {
    pub max_actions: u32,
    pub window_secs: u64,
}

/// What an agent may do. Fixed for the agent's lifetime once written to its log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    /// Total credits the agent may ever spend.
    pub budget: u64,
    /// Allowlisted tools. Anything else is handled by `default_rule`.
    pub tools: BTreeMap<String, ToolRule>,
    /// Rule for tools not in `tools`. `None` denies them (and scars the agent).
    #[serde(default)]
    pub default_rule: Option<ToolRule>,
    #[serde(default)]
    pub rate_limit: Option<RateLimit>,
    /// Terminate once the sum of scar weights reaches this. See [`Severity::weight`].
    pub scar_limit: u32,
}

impl Policy {
    /// A deny-by-default policy with the given budget and a scar limit of 10.
    pub fn new(budget: u64) -> Self {
        Policy { budget, tools: BTreeMap::new(), default_rule: None, rate_limit: None, scar_limit: 10 }
    }

    pub fn allow(mut self, tool: &str, rule: ToolRule) -> Self {
        self.tools.insert(tool.to_string(), rule);
        self
    }

    pub fn allow_unlisted(mut self, rule: ToolRule) -> Self {
        self.default_rule = Some(rule);
        self
    }

    pub fn rate_limit(mut self, max_actions: u32, window_secs: u64) -> Self {
        self.rate_limit = Some(RateLimit { max_actions, window_secs });
        self
    }

    pub fn scar_limit(mut self, scar_limit: u32) -> Self {
        self.scar_limit = scar_limit;
        self
    }

    fn rule_for(&self, tool: &str) -> Option<&ToolRule> {
        self.tools.get(tool).or(self.default_rule.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Minor,
    Moderate,
    Severe,
    /// Terminates the agent immediately.
    Fatal,
}

impl Severity {
    pub fn weight(self) -> u32 {
        match self {
            Severity::Minor => 1,
            Severity::Moderate => 3,
            Severity::Severe => 10,
            Severity::Fatal => u32::MAX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scar {
    pub seq: u64,
    pub severity: Severity,
    pub reason: String,
    pub action_id: Option<String>,
}

/// Why an action was denied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum DenyReason {
    Terminated { reason: String },
    ToolNotAllowed { tool: String },
    InsufficientBudget { cost: u64, remaining: u64 },
    ToolCallLimit { tool: String, max_calls: u64 },
    RateLimited { max_actions: u32, window_secs: u64 },
    Rejected { by: String, reason: String },
}

impl fmt::Display for DenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DenyReason::Terminated { reason } => write!(f, "agent is terminated: {}", reason),
            DenyReason::ToolNotAllowed { tool } => write!(f, "tool '{}' is not allowed", tool),
            DenyReason::InsufficientBudget { cost, remaining } => {
                write!(f, "cost {} exceeds remaining budget {}", cost, remaining)
            }
            DenyReason::ToolCallLimit { tool, max_calls } => write!(f, "tool '{}' reached its limit of {} calls", tool, max_calls),
            DenyReason::RateLimited { max_actions, window_secs } => {
                write!(f, "rate limit of {} actions per {}s exceeded", max_actions, window_secs)
            }
            DenyReason::Rejected { by, reason } => write!(f, "rejected by {}: {}", by, reason),
        }
    }
}

impl DenyReason {
    /// Scar inflicted on the agent for attempting this, if any.
    fn scar(&self) -> Option<Severity> {
        match self {
            DenyReason::ToolNotAllowed { .. } => Some(Severity::Moderate),
            DenyReason::ToolCallLimit { .. } | DenyReason::RateLimited { .. } => Some(Severity::Minor),
            DenyReason::Terminated { .. } | DenyReason::InsufficientBudget { .. } | DenyReason::Rejected { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allowed { action_id: String, cost: u64, remaining: u64 },
    Denied { action_id: String, reason: DenyReason },
    PendingApproval { action_id: String },
}

impl Decision {
    pub fn action_id(&self) -> &str {
        match self {
            Decision::Allowed { action_id, .. }
            | Decision::Denied { action_id, .. }
            | Decision::PendingApproval { action_id } => action_id,
        }
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self, Decision::Allowed { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    Success,
    /// The action failed. Minor scar.
    Failure,
    /// The action caused harm (flagged by a reviewer, evaluator, or monitor). Severe scar.
    Harmful,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub status: OutcomeStatus,
    #[serde(default)]
    pub detail: String,
}

impl Outcome {
    pub fn success(detail: impl Into<String>) -> Self {
        Outcome { status: OutcomeStatus::Success, detail: detail.into() }
    }

    pub fn failure(detail: impl Into<String>) -> Self {
        Outcome { status: OutcomeStatus::Failure, detail: detail.into() }
    }

    pub fn harmful(detail: impl Into<String>) -> Self {
        Outcome { status: OutcomeStatus::Harmful, detail: detail.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Requested,
    PendingApproval,
    Allowed,
    Denied,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub action_id: String,
    pub tool: String,
    /// The input exactly as requested. An approval covers this input and nothing else, so a
    /// tool backend can check that what it is asked to do is what was approved.
    pub input: Value,
    pub cost: u64,
    pub status: ActionStatus,
    pub outcome: Option<OutcomeStatus>,
}

#[derive(Debug)]
pub enum GuardError {
    Audit(AuditError),
    /// The log is not a valid guard log (no policy, unknown references, ...).
    InvalidLog(String),
    UnknownAction(String),
    /// The action is not in a state that permits this operation.
    InvalidState { action_id: String, status: ActionStatus },
    AlreadyTerminated(String),
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuardError::Audit(e) => write!(f, "{}", e),
            GuardError::InvalidLog(e) => write!(f, "invalid guard log: {}", e),
            GuardError::UnknownAction(id) => write!(f, "unknown action: {}", id),
            GuardError::InvalidState { action_id, status } => {
                write!(f, "action {} is {:?}; operation not permitted", action_id, status)
            }
            GuardError::AlreadyTerminated(reason) => write!(f, "agent is already terminated: {}", reason),
        }
    }
}

impl std::error::Error for GuardError {}

impl From<AuditError> for GuardError {
    fn from(e: AuditError) -> Self {
        GuardError::Audit(e)
    }
}

/// Snapshot of an agent's standing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub alive: bool,
    pub termination_reason: Option<String>,
    pub budget: u64,
    pub spent: u64,
    pub remaining: u64,
    pub scar_score: u32,
    pub scar_limit: u32,
    pub scars: Vec<Scar>,
    pub actions_allowed: u64,
    pub actions_denied: u64,
    pub actions_pending: u64,
    pub head: Checkpoint,
    pub public_key: String,
}

/// Guard state, derived only by applying log records in order.
#[derive(Debug, Default)]
struct State {
    policy: Option<Policy>,
    spent: u64,
    scars: Vec<Scar>,
    scar_score: u32,
    terminated: Option<String>,
    actions: HashMap<String, Action>,
    action_count: u64,
    tool_calls: HashMap<String, u64>,
    recent_allowed: VecDeque<DateTime<Utc>>,
    allowed: u64,
    denied: u64,
}

impl State {
    fn policy(&self) -> &Policy {
        self.policy.as_ref().expect("guard state always has a policy after the policy record")
    }

    fn remaining(&self) -> u64 {
        self.policy().budget.saturating_sub(self.spent)
    }

    fn action_mut(&mut self, payload: &Value) -> Result<&mut Action, GuardError> {
        let id = payload["action_id"].as_str().ok_or_else(|| GuardError::InvalidLog("record has no action_id".into()))?;
        self.actions.get_mut(id).ok_or_else(|| GuardError::UnknownAction(id.to_string()))
    }

    fn apply(&mut self, record: &Record) -> Result<(), GuardError> {
        let invalid = |what: &str| GuardError::InvalidLog(format!("seq {}: {}", record.seq, what));
        if record.kind != kinds::POLICY && record.kind != audit::GENESIS_KIND && self.policy.is_none() {
            return Err(invalid("record before policy"));
        }
        let payload = &record.payload;
        match record.kind.as_str() {
            audit::GENESIS_KIND => {}
            kinds::POLICY => {
                if self.policy.is_some() {
                    return Err(invalid("policy may only be set once"));
                }
                let policy = serde_json::from_value(payload["policy"].clone()).map_err(|e| invalid(&e.to_string()))?;
                self.policy = Some(policy);
            }
            kinds::ACTION_REQUESTED => {
                let action: Action = Action {
                    action_id: payload["action_id"].as_str().ok_or_else(|| invalid("missing action_id"))?.to_string(),
                    tool: payload["tool"].as_str().ok_or_else(|| invalid("missing tool"))?.to_string(),
                    input: payload["input"].clone(),
                    cost: payload["cost"].as_u64().ok_or_else(|| invalid("missing cost"))?,
                    status: ActionStatus::Requested,
                    outcome: None,
                };
                self.action_count += 1;
                self.actions.insert(action.action_id.clone(), action);
            }
            kinds::APPROVAL_REQUIRED => {
                self.action_mut(payload)?.status = ActionStatus::PendingApproval;
            }
            kinds::ACTION_ALLOWED => {
                let timestamp: DateTime<Utc> = record.timestamp.parse().map_err(|_| invalid("bad timestamp"))?;
                let action = self.action_mut(payload)?;
                action.status = ActionStatus::Allowed;
                let (tool, cost) = (action.tool.clone(), action.cost);
                self.spent += cost;
                *self.tool_calls.entry(tool).or_insert(0) += 1;
                self.recent_allowed.push_back(timestamp);
                self.allowed += 1;
            }
            kinds::ACTION_DENIED | kinds::ACTION_REJECTED => {
                self.action_mut(payload)?.status = ActionStatus::Denied;
                self.denied += 1;
            }
            kinds::OUTCOME => {
                let status: OutcomeStatus = serde_json::from_value(payload["status"].clone()).map_err(|e| invalid(&e.to_string()))?;
                let action = self.action_mut(payload)?;
                action.status = ActionStatus::Completed;
                action.outcome = Some(status);
            }
            kinds::SCAR => {
                let severity: Severity = serde_json::from_value(payload["severity"].clone()).map_err(|e| invalid(&e.to_string()))?;
                self.scar_score = self.scar_score.saturating_add(severity.weight());
                self.scars.push(Scar {
                    seq: record.seq,
                    severity,
                    reason: payload["reason"].as_str().unwrap_or_default().to_string(),
                    action_id: payload["action_id"].as_str().map(str::to_string),
                });
            }
            kinds::TERMINATED => {
                self.terminated = Some(payload["reason"].as_str().unwrap_or_default().to_string());
            }
            other => return Err(invalid(&format!("unknown record kind '{}'", other))),
        }
        Ok(())
    }

    /// Pre-flight checks for a request. `None` means the action may proceed.
    fn check(&mut self, tool: &str, cost: u64, now: DateTime<Utc>) -> Option<DenyReason> {
        if let Some(reason) = &self.terminated {
            return Some(DenyReason::Terminated { reason: reason.clone() });
        }
        let policy = self.policy().clone();
        let Some(rule) = policy.rule_for(tool) else {
            return Some(DenyReason::ToolNotAllowed { tool: tool.to_string() });
        };
        if let Some(max_calls) = rule.max_calls
            && self.tool_calls.get(tool).copied().unwrap_or(0) >= max_calls
        {
            return Some(DenyReason::ToolCallLimit { tool: tool.to_string(), max_calls });
        }
        if let Some(limit) = &policy.rate_limit {
            let window_start = now - Duration::seconds(limit.window_secs as i64);
            while self.recent_allowed.front().is_some_and(|t| *t <= window_start) {
                self.recent_allowed.pop_front();
            }
            if self.recent_allowed.len() >= limit.max_actions as usize {
                return Some(DenyReason::RateLimited { max_actions: limit.max_actions, window_secs: limit.window_secs });
            }
        }
        let remaining = self.remaining();
        if cost > remaining {
            return Some(DenyReason::InsufficientBudget { cost, remaining });
        }
        None
    }
}

/// A policy gate for one agent, backed by a signed audit log.
pub struct Guard {
    log: AuditLog,
    state: State,
    agent_id: String,
}

impl Guard {
    /// Creates a new agent with a fixed policy. Fails if the log already exists.
    pub fn create(path: impl AsRef<Path>, key: AuditKey, agent_id: &str, policy: Policy) -> Result<Self, GuardError> {
        let log = AuditLog::create(path, key, agent_id)?;
        let mut guard = Guard { log, state: State::default(), agent_id: agent_id.to_string() };
        guard.commit(kinds::POLICY, json!({ "policy": policy }))?;
        Ok(guard)
    }

    /// Opens an existing agent: verifies its log and replays it to rebuild state.
    pub fn open(path: impl AsRef<Path>, key: AuditKey) -> Result<Self, GuardError> {
        let log = AuditLog::open(path, key)?;
        let mut state = State::default();
        for record in log.records()? {
            state.apply(&record)?;
        }
        if state.policy.is_none() {
            return Err(GuardError::InvalidLog("log has no policy record".into()));
        }
        let agent_id = log.log_id().to_string();
        let mut guard = Guard { log, state, agent_id };
        guard.terminate_if_record_missing()?;
        Ok(guard)
    }

    /// An honest guard writes `terminated` in the same step that crosses the scar limit
    /// or spends the last credit. A log that shows either without that record has had its
    /// tail cut off; terminate now instead of letting the agent act again.
    fn terminate_if_record_missing(&mut self) -> Result<(), GuardError> {
        if !self.is_alive() {
            return Ok(());
        }
        let policy = self.state.policy();
        let reason = if !self.state.scars.is_empty() && self.state.scar_score >= policy.scar_limit {
            Some(format!("scar limit reached ({} >= {}); termination record missing from the log", self.state.scar_score, policy.scar_limit))
        } else if self.state.allowed > 0 && self.state.remaining() == 0 {
            Some("budget exhausted; termination record missing from the log".to_string())
        } else {
            None
        };
        match reason {
            Some(reason) => self.commit(kinds::TERMINATED, json!({ "reason": reason, "by": "guard (replay)" })),
            None => Ok(()),
        }
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn policy(&self) -> &Policy {
        self.state.policy()
    }

    pub fn is_alive(&self) -> bool {
        self.state.terminated.is_none()
    }

    pub fn head(&self) -> Checkpoint {
        self.log.head()
    }

    pub fn records(&self) -> Result<Vec<Record>, GuardError> {
        Ok(self.log.records()?)
    }

    pub fn action(&self, action_id: &str) -> Option<&Action> {
        self.state.actions.get(action_id)
    }

    /// Asks permission to call `tool`. `cost` may raise the charge above the tool's
    /// minimum (e.g. tokens used), never lower it. `input` is recorded verbatim, so
    /// redact secrets before passing them.
    pub fn request(&mut self, tool: &str, input: Value, cost: Option<u64>) -> Result<Decision, GuardError> {
        let rule_cost = self.state.policy().rule_for(tool).map(|r| r.cost).unwrap_or(0);
        let cost = cost.unwrap_or(0).max(rule_cost);
        let action_id = format!("act-{}", self.state.action_count + 1);

        self.commit(kinds::ACTION_REQUESTED, json!({ "action_id": action_id, "tool": tool, "input": input, "cost": cost }))?;

        if let Some(reason) = self.state.check(tool, cost, Utc::now()) {
            self.deny(&action_id, reason.clone())?;
            return Ok(Decision::Denied { action_id, reason });
        }

        if self.state.policy().rule_for(tool).is_some_and(|r| r.requires_approval) {
            self.commit(kinds::APPROVAL_REQUIRED, json!({ "action_id": action_id }))?;
            return Ok(Decision::PendingApproval { action_id });
        }

        self.allow(&action_id)
    }

    /// Approves a pending action. Checks are re-run, since budget or standing may have
    /// changed while it waited.
    pub fn approve(&mut self, action_id: &str, approver: &str) -> Result<Decision, GuardError> {
        self.approve_with_note(action_id, approver, None)
    }

    /// Like [`Guard::approve`], recording why (e.g. a reviewer's verdict) in the signed log.
    pub fn approve_with_note(&mut self, action_id: &str, approver: &str, note: Option<&str>) -> Result<Decision, GuardError> {
        let action = self.pending(action_id)?;
        let (tool, cost) = (action.tool.clone(), action.cost);
        if let Some(reason) = self.state.check(&tool, cost, Utc::now()) {
            self.deny(action_id, reason.clone())?;
            return Ok(Decision::Denied { action_id: action_id.to_string(), reason });
        }
        self.allow_by(action_id, Some(approver), note)
    }

    /// Rejects a pending action. `scar` optionally punishes the agent for asking.
    pub fn reject(&mut self, action_id: &str, approver: &str, reason: &str, scar: Option<Severity>) -> Result<Decision, GuardError> {
        self.pending(action_id)?;
        self.commit(kinds::ACTION_REJECTED, json!({ "action_id": action_id, "by": approver, "reason": reason }))?;
        if let Some(severity) = scar {
            self.scar(severity, &format!("rejected by {}: {}", approver, reason), Some(action_id))?;
        }
        Ok(Decision::Denied {
            action_id: action_id.to_string(),
            reason: DenyReason::Rejected { by: approver.to_string(), reason: reason.to_string() },
        })
    }

    /// Reports how an allowed action went. Failures and harm scar the agent.
    pub fn report(&mut self, action_id: &str, outcome: Outcome) -> Result<(), GuardError> {
        let action = self.state.actions.get(action_id).ok_or_else(|| GuardError::UnknownAction(action_id.to_string()))?;
        if action.status != ActionStatus::Allowed {
            return Err(GuardError::InvalidState { action_id: action_id.to_string(), status: action.status });
        }
        self.commit(kinds::OUTCOME, json!({ "action_id": action_id, "status": outcome.status, "detail": outcome.detail }))?;
        match outcome.status {
            OutcomeStatus::Success => {}
            OutcomeStatus::Failure => self.scar(Severity::Minor, &format!("action failed: {}", outcome.detail), Some(action_id))?,
            OutcomeStatus::Harmful => self.scar(Severity::Severe, &format!("action caused harm: {}", outcome.detail), Some(action_id))?,
        }
        Ok(())
    }

    /// Records a scar from an external monitor or reviewer.
    pub fn scar(&mut self, severity: Severity, reason: &str, action_id: Option<&str>) -> Result<(), GuardError> {
        self.commit(kinds::SCAR, json!({ "severity": severity, "reason": reason, "action_id": action_id }))?;
        if self.is_alive() && self.state.scar_score >= self.state.policy().scar_limit {
            let reason = format!("scar limit reached ({} >= {})", self.state.scar_score, self.state.policy().scar_limit);
            self.commit(kinds::TERMINATED, json!({ "reason": reason, "by": "guard" }))?;
        }
        Ok(())
    }

    /// Kill switch. Permanent.
    pub fn terminate(&mut self, reason: &str, by: &str) -> Result<(), GuardError> {
        if let Some(existing) = &self.state.terminated {
            return Err(GuardError::AlreadyTerminated(existing.clone()));
        }
        self.commit(kinds::TERMINATED, json!({ "reason": reason, "by": by }))
    }

    pub fn status(&self) -> AgentStatus {
        let policy = self.state.policy();
        AgentStatus {
            agent_id: self.agent_id.clone(),
            alive: self.is_alive(),
            termination_reason: self.state.terminated.clone(),
            budget: policy.budget,
            spent: self.state.spent,
            remaining: self.state.remaining(),
            scar_score: self.state.scar_score,
            scar_limit: policy.scar_limit,
            scars: self.state.scars.clone(),
            actions_allowed: self.state.allowed,
            actions_denied: self.state.denied,
            actions_pending: self.state.actions.values().filter(|a| a.status == ActionStatus::PendingApproval).count() as u64,
            head: self.log.head(),
            public_key: self.log.public_key_hex(),
        }
    }

    fn pending(&self, action_id: &str) -> Result<&Action, GuardError> {
        let action = self.state.actions.get(action_id).ok_or_else(|| GuardError::UnknownAction(action_id.to_string()))?;
        if action.status != ActionStatus::PendingApproval {
            return Err(GuardError::InvalidState { action_id: action_id.to_string(), status: action.status });
        }
        Ok(action)
    }

    fn allow(&mut self, action_id: &str) -> Result<Decision, GuardError> {
        self.allow_by(action_id, None, None)
    }

    fn allow_by(&mut self, action_id: &str, approver: Option<&str>, note: Option<&str>) -> Result<Decision, GuardError> {
        let mut payload = json!({ "action_id": action_id, "approved_by": approver });
        if let Some(note) = note {
            payload["note"] = json!(note);
        }
        self.commit(kinds::ACTION_ALLOWED, payload)?;
        let cost = self.state.actions[action_id].cost;
        let remaining = self.state.remaining();
        if remaining == 0 && self.is_alive() {
            self.commit(kinds::TERMINATED, json!({ "reason": "budget exhausted", "by": "guard" }))?;
        }
        Ok(Decision::Allowed { action_id: action_id.to_string(), cost, remaining })
    }

    fn deny(&mut self, action_id: &str, reason: DenyReason) -> Result<(), GuardError> {
        self.commit(kinds::ACTION_DENIED, json!({ "action_id": action_id, "reason": reason }))?;
        if let Some(severity) = reason.scar() {
            self.scar(severity, &reason.to_string(), Some(action_id))?;
        }
        Ok(())
    }

    /// The only way state changes: append to the log, then apply the same record.
    fn commit(&mut self, kind: &str, payload: Value) -> Result<(), GuardError> {
        let record = self.log.append(&self.agent_id, kind, payload)?;
        self.state.apply(&record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut bytes = [0u8; 8];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let dir = std::env::temp_dir().join(format!("lineage-guard-{}-{}", name, hex::encode(bytes)));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn setup(name: &str, policy: Policy) -> (TempDir, Guard) {
        let dir = TempDir::new(name);
        let key = AuditKey::load_or_create(dir.0.join("audit.key")).unwrap();
        let guard = Guard::create(dir.0.join("agent.jsonl"), key, "agent-1", policy).unwrap();
        (dir, guard)
    }

    fn reopen(dir: &TempDir, guard: Guard) -> Guard {
        drop(guard);
        Guard::open(dir.0.join("agent.jsonl"), AuditKey::load(dir.0.join("audit.key")).unwrap()).unwrap()
    }

    fn basic_policy() -> Policy {
        Policy::new(20).allow("search", ToolRule::cost(2)).allow("email", ToolRule::cost(5).with_approval())
    }

    #[test]
    fn test_allowed_action_spends_budget() {
        let (_dir, mut guard) = setup("allow", basic_policy());
        let decision = guard.request("search", json!({"q": "x"}), None).unwrap();
        assert_eq!(decision, Decision::Allowed { action_id: "act-1".into(), cost: 2, remaining: 18 });
        guard.report("act-1", Outcome::success("ok")).unwrap();

        let status = guard.status();
        assert_eq!(status.spent, 2);
        assert_eq!(status.actions_allowed, 1);
        assert_eq!(status.scar_score, 0);
    }

    #[test]
    fn test_declared_cost_cannot_undercut_rule() {
        let (_dir, mut guard) = setup("cost", basic_policy());
        assert_eq!(guard.request("search", json!({}), Some(0)).unwrap(), Decision::Allowed { action_id: "act-1".into(), cost: 2, remaining: 18 });
        assert_eq!(guard.request("search", json!({}), Some(7)).unwrap(), Decision::Allowed { action_id: "act-2".into(), cost: 7, remaining: 11 });
    }

    #[test]
    fn test_unlisted_tool_is_denied_and_scars() {
        let (_dir, mut guard) = setup("unlisted", basic_policy());
        let decision = guard.request("shell", json!({"cmd": "rm -rf /"}), None).unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::ToolNotAllowed { .. }, .. }));
        assert_eq!(guard.status().scar_score, Severity::Moderate.weight());
        assert_eq!(guard.status().spent, 0);
    }

    #[test]
    fn test_repeated_violations_terminate_agent() {
        let (_dir, mut guard) = setup("violations", basic_policy().scar_limit(6));
        guard.request("shell", json!({}), None).unwrap();
        assert!(guard.is_alive());
        guard.request("shell", json!({}), None).unwrap();
        assert!(!guard.is_alive());

        let decision = guard.request("search", json!({}), None).unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::Terminated { .. }, .. }));
    }

    #[test]
    fn test_budget_exhaustion() {
        let (_dir, mut guard) = setup("budget", Policy::new(5).allow("search", ToolRule::cost(2)));
        guard.request("search", json!({}), None).unwrap();
        guard.request("search", json!({}), None).unwrap();
        let decision = guard.request("search", json!({}), None).unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::InsufficientBudget { cost: 2, remaining: 1 }, .. }));
        assert!(guard.is_alive());

        // Spending the last credit terminates the agent.
        assert!(!guard.request("search", json!({}), Some(1)).unwrap().is_allowed());
        let (_dir, mut exact) = setup("budget-exact", Policy::new(4).allow("search", ToolRule::cost(2)));
        exact.request("search", json!({}), None).unwrap();
        assert!(exact.request("search", json!({}), None).unwrap().is_allowed());
        assert!(!exact.is_alive());
        assert_eq!(exact.status().termination_reason.as_deref(), Some("budget exhausted"));
    }

    #[test]
    fn test_approval_flow() {
        let (_dir, mut guard) = setup("approval", basic_policy());
        let decision = guard.request("email", json!({"to": "ceo@example.com"}), None).unwrap();
        assert_eq!(decision, Decision::PendingApproval { action_id: "act-1".into() });
        assert_eq!(guard.status().spent, 0);
        assert!(matches!(guard.report("act-1", Outcome::success("")), Err(GuardError::InvalidState { .. })));

        let approved = guard.approve("act-1", "alice").unwrap();
        assert_eq!(approved, Decision::Allowed { action_id: "act-1".into(), cost: 5, remaining: 15 });
        assert!(matches!(guard.approve("act-1", "alice"), Err(GuardError::InvalidState { .. })));

        guard.request("email", json!({}), None).unwrap();
        let rejected = guard.reject("act-2", "bob", "phishing", Some(Severity::Moderate)).unwrap();
        assert!(matches!(rejected, Decision::Denied { reason: DenyReason::Rejected { .. }, .. }));
        assert_eq!(guard.status().scar_score, 3);
    }

    #[test]
    fn test_approval_rechecks_after_termination() {
        let (_dir, mut guard) = setup("approval-kill", basic_policy());
        guard.request("email", json!({}), None).unwrap();
        guard.terminate("incident", "oncall").unwrap();
        let decision = guard.approve("act-1", "alice").unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::Terminated { .. }, .. }));
    }

    #[test]
    fn test_harmful_outcome_scars_severely() {
        let (_dir, mut guard) = setup("harm", basic_policy());
        guard.request("search", json!({}), None).unwrap();
        guard.report("act-1", Outcome::harmful("leaked customer data")).unwrap();
        assert!(!guard.is_alive());
        assert!(guard.status().termination_reason.unwrap().contains("scar limit"));
        assert!(matches!(guard.report("act-1", Outcome::success("")), Err(GuardError::InvalidState { .. })));
    }

    #[test]
    fn test_action_keeps_its_input() {
        let (dir, mut guard) = setup("input", basic_policy());
        guard.request("email", json!({"to": "a@example.com", "amount": 5}), None).unwrap();
        let guard = reopen(&dir, guard);
        assert_eq!(guard.action("act-1").unwrap().input, json!({"to": "a@example.com", "amount": 5}));
    }

    #[test]
    fn test_tool_call_limit() {
        let (_dir, mut guard) = setup("maxcalls", Policy::new(100).allow("deploy", ToolRule::cost(1).with_max_calls(1)));
        assert!(guard.request("deploy", json!({}), None).unwrap().is_allowed());
        let decision = guard.request("deploy", json!({}), None).unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::ToolCallLimit { .. }, .. }));
    }

    #[test]
    fn test_rate_limit() {
        let (_dir, mut guard) = setup("rate", Policy::new(100).allow("search", ToolRule::cost(1)).rate_limit(3, 60));
        for _ in 0..3 {
            assert!(guard.request("search", json!({}), None).unwrap().is_allowed());
        }
        let decision = guard.request("search", json!({}), None).unwrap();
        assert!(matches!(decision, Decision::Denied { reason: DenyReason::RateLimited { .. }, .. }));
    }

    #[test]
    fn test_restart_cannot_refund_or_revive() {
        let (dir, mut guard) = setup("replay", basic_policy().scar_limit(6));
        guard.request("search", json!({}), None).unwrap();
        guard.report("act-1", Outcome::failure("timeout")).unwrap();
        guard.request("email", json!({}), None).unwrap();
        let before = guard.status();

        let mut guard = reopen(&dir, guard);
        let after = guard.status();
        assert_eq!(after.spent, before.spent);
        assert_eq!(after.scar_score, before.scar_score);
        assert_eq!(after.actions_pending, 1);
        assert_eq!(after.head, before.head);

        // Pending approvals survive restarts, and ids keep counting.
        assert!(guard.approve("act-2", "alice").unwrap().is_allowed());
        assert_eq!(guard.request("search", json!({}), None).unwrap().action_id(), "act-3");

        guard.terminate("manual", "oncall").unwrap();
        let mut guard = reopen(&dir, guard);
        assert!(!guard.is_alive());
        assert!(matches!(guard.terminate("again", "oncall"), Err(GuardError::AlreadyTerminated(_))));
        assert!(!guard.request("search", json!({}), None).unwrap().is_allowed());
    }

    #[test]
    fn test_deleting_the_termination_record_does_not_revive() {
        let (dir, mut guard) = setup("untruncate", basic_policy().scar_limit(3));
        guard.request("shell", json!({}), None).unwrap();
        assert!(!guard.is_alive());
        drop(guard);

        // Cut the `terminated` record off the end: the shorter chain still verifies.
        let path = dir.0.join("agent.jsonl");
        let text = fs::read_to_string(&path).unwrap();
        let kept: Vec<&str> = text.lines().filter(|l| !l.contains("\"kind\":\"terminated\"")).collect();
        fs::write(&path, kept.join("\n") + "\n").unwrap();

        let mut guard = Guard::open(&path, AuditKey::load(dir.0.join("audit.key")).unwrap()).unwrap();
        assert!(!guard.is_alive());
        assert!(guard.status().termination_reason.unwrap().contains("termination record missing"));
        assert!(!guard.request("search", json!({}), None).unwrap().is_allowed());
    }

    #[test]
    fn test_policy_cannot_be_changed_by_tampering() {
        let (dir, guard) = setup("tamper-policy", basic_policy());
        drop(guard);
        let path = dir.0.join("agent.jsonl");
        let text = fs::read_to_string(&path).unwrap().replace("\"budget\":20", "\"budget\":999999");
        fs::write(&path, text).unwrap();

        let result = Guard::open(&path, AuditKey::load(dir.0.join("audit.key")).unwrap());
        assert!(matches!(result, Err(GuardError::Audit(AuditError::Corrupt(_)))));
    }
}
