# Why Lineage

## The problem

AI agents have moved from answering questions to taking actions: running commands, sending messages, moving money, changing production systems. Three things about them don't change with a better model:

- **They act on text they read.** Anything an agent reads can instruct it: a log line, an email, a web page, a ticket, a tool's output. That's prompt injection. Better training makes it less likely, but it doesn't make it impossible.
- **They don't know when to stop.** An agent in a loop will call tools and spend money until something outside it says no.
- **They leave no trustworthy record.** Application logs can be edited, trimmed, or lost. When a customer, an auditor, or your own incident review asks what the agent did and who approved it, "the logs say so" isn't proof.

Every team that deploys agents ends up rebuilding the same controls by hand: an allowlist here, a spending cap there, an approval step in one tool, a log table in the database. They're scattered, and a bug in any of them fails open.

## What Lineage gives you

One small component, in the same place for every agent:

| Control | What it replaces |
|---|---|
| **Policy**: allowlisted tools, costs, limits, approvals, fixed at creation | Ad-hoc checks inside each tool |
| **Budget** that never refills | Hoping the loop terminates |
| **Human approval** bound to the exact input | Slack messages and "I think someone said yes" |
| **Scars and termination** | An agent that keeps trying after its tenth violation |
| **Signed, hash-chained log**, verifiable offline | Rows in a table that anyone with database access can edit |

The controls live **outside the model**. The model can be tricked; the guard can't be argued with. Its decisions come from a policy the agent can't change, and its history from a log the agent can't rewrite.

## Why teams adopt it

| If you're… | Lineage gives you |
|---|---|
| **An engineering team** shipping an agent | The controls you'd otherwise write yourself, already tested. One guard call before each tool, and examples for Claude, DeepSeek, and any other model |
| **Security** | Least privilege for agents; a tripwire when an agent asks for something it shouldn't (every attempt is recorded and scarred); a kill switch; approvals that a compromised agent can't forge or replay |
| **Finance and operations** | Spending authority as a hard number. An agent with a $25,000 budget can't spend $25,001, and every payment above a threshold waits for a person |
| **Risk, compliance, and legal** | A record of every automated decision, and of every human approval with its reason, that can be independently verified. Use it as evidence for your own controls and audits |
| **Leadership** | A way to say yes to agent projects with bounded downside: a fixed scope, a fixed budget, human sign-off where it matters, and proof afterwards |

## When to use it

Use Lineage when an agent's mistake **costs something**: money, customer trust, data, uptime. Use it when you'll need to **show** what happened, or when **more than one party** needs to trust the record: your team and a customer, a vendor and an auditor.

## When not to

- **Your system needs undo.** Lineage deliberately has no rollback, no refunds, and no resurrection. If a workflow needs to erase history, it doesn't fit.
- **You need a sandbox.** Lineage decides what an agent may do; it doesn't isolate code execution. Use it together with containers or VMs, not instead of them.
- **There are no actions to guard.** A chatbot that only answers questions doesn't need a policy gate.

## What it costs to adopt

- **In Rust:** add a dependency and call `guard.request` before each tool.
- **In any language:** run one static binary, and call one HTTP endpoint before each tool.
- **To start from a working project:** run `lineage new my-agent`. See [Project setup](building/new-project.md).
- **To learn it hands-on:** [Lineage Mastery](mastery/index.md) goes from your first guarded action to production in ten runnable levels.
