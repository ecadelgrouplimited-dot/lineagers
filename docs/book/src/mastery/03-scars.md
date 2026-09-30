# Level 3: Scars and termination

<div class="lx-lesson">
<p><strong>You'll learn</strong> what leaves a scar, how scars add up, and the kill switch.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_03_scars</code></p>
</div>

## The idea

{{#include ../diagrams/scars.html}}

Misbehavior leaves permanent scars, weighted by severity: minor 1, moderate 3, severe 10. When the total reaches the policy's `scar_limit`, the agent is terminated for good. Operators, and monitors acting for them, can add scars directly, and can terminate an agent at any time.

## The code

```rust
{{#include ../../../../examples/mastery_03_scars.rs}}
```

## Run it

```text
read_file failed                               scars  1/10  alive true
asked for delete_file (not allowed)            scars  4/10  alive true
third http_get (cap is 2)                      scars  5/10  alive true
monitor reported harm                          scars 15/10  alive false

termination reason: scar limit reached (15 >= 10)
scars, permanently on record:
  Minor: action failed: file not found
  Moderate: tool 'delete_file' is not allowed
  Minor: tool 'http_get' reached its limit of 2 calls
  Severe: uploaded a customer file to a paste site

kill switch: other agent alive = false; terminating again: agent is already terminated: incident INC-42: suspended while we investigate
```

## What happened

| Event | Scar | Why |
|---|---|---|
| A tool failed | minor (1) | A flailing agent should eventually stop |
| An unlisted tool was requested | moderate (3) | The agent was confused or compromised |
| A tool's `max_calls` cap was exceeded | minor (1) | A limit you set on purpose was hit |
| A monitor reported harm | severe (10) | Something went wrong in the world |

The kill switch is `terminate`. It records who did it and why, and it can't be done twice: termination is final.

## Try this

Raise `scar_limit` to 20, and find the smallest sequence of events that still terminates the agent.

[Next: Humans in the loop →](04-approvals.md)
