# Lineage in pictures

The whole system on one page. Each picture shows one mechanism, and each is explained in depth elsewhere in the docs.

## Where Lineage sits

{{#include diagrams/arch.html}}

An agent never calls a tool directly. It asks the guard, and the guard answers from a policy the agent can't change. Everything that happens, whether asked, allowed, denied, approved, failed or harmful, is appended to the agent's signed log. [How Lineage works →](concepts/overview.md)

## What changes when you add it

{{#include diagrams/compare.html}}

The model is the same, and so is the injected text. The difference is that the model's requests now pass through something that says no, holds risky actions for a person, and keeps evidence. [Why Lineage →](why.md)

## The checks, in order

{{#include diagrams/checks.html}}

A request passes six checks. The first failure denies it; some denials leave a scar, because asking for a forbidden tool is itself a signal. [Decisions and scars →](concepts/decisions.md)

## The life of an action

{{#include diagrams/life.html}}

Pending actions cost nothing until approved, and they're checked again at approval time. An action approved an hour later, after its agent was terminated, is denied. [Approvals →](concepts/approvals.md)

## Scars add up

{{#include diagrams/scars.html}}

Scars never heal. The weights (minor 1, moderate 3, severe 10) add toward the policy's `scar_limit`. [Policies →](concepts/policies.md)

## Why the history can't be rewritten

{{#include diagrams/chain.html}}

Every record carries the hash of the one before it and an Ed25519 signature. Changing any byte breaks that record's hash; recomputing the hash breaks the signature; deleting a record breaks the chain. Published checkpoints also catch the tail being cut off. [The audit log →](concepts/audit-log.md)

## Why restarts don't help a misbehaving agent

{{#include diagrams/replay.html}}

There is no state file to reset. The log is the state, and a tampered log is refused. [Level 5 of Lineage Mastery →](mastery/05-restarts.md)
