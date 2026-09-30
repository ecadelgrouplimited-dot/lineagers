# Level 5: Consequences survive restarts

<div class="lx-lesson">
<p><strong>You'll learn</strong> why restarting a process never resets an agent.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_05_persistence</code>, three times in a row</p>
</div>

## The idea

{{#include ../diagrams/replay.html}}

The guard keeps no state of its own. `Guard::open` verifies the agent's log, and replays it: spent budget, scars, pending approvals and termination all come back exactly as they were. The only way to "start over" is to create a *new* agent, with a new identity and an empty history.

## The code

```rust
{{#include ../../../../examples/mastery_05_persistence.rs}}
```

## Run it (three times)

```text
(first run: creating the agent)
start of run: spent 0/20, scars 0/6, alive true
  work      allowed
  wipe_disk denied: tool 'wipe_disk' is not allowed
end of run:   spent 3/20, scars 3/6, alive true  (8 records in the log)
```
```text
start of run: spent 3/20, scars 3/6, alive true
  work      allowed
  wipe_disk denied: tool 'wipe_disk' is not allowed
end of run:   spent 6/20, scars 6/6, alive false  (15 records in the log)

The agent is terminated: scar limit reached (6 >= 6). Restarting won't change that.
```
```text
start of run: spent 6/20, scars 6/6, alive false
  work      denied: agent is terminated: scar limit reached (6 >= 6)
  wipe_disk denied: agent is terminated: scar limit reached (6 >= 6)
end of run:   spent 6/20, scars 6/6, alive false  (19 records in the log)
```

## What happened

Each run picked up exactly where the last one ended. The second run crossed the scar limit, and the third could do nothing, even though its process had never seen a scar. The log grew anyway, because denied attempts are part of the record.

## Try this

After the agent is terminated, try to bring it back by editing `mastery-data/05/agent.jsonl`:

- **Change a record**, say a `"cost":3` to `"cost":0`, and run again. `Guard::open` refuses the log:
  ```text
  Error: Audit(Corrupt(VerifyFailure { line: Some(2), seq: Some(1), reason: "hash does not match record content" }))
  ```
- **Delete the last line**, the `terminated` record. What's left is a valid, shorter chain. But the guard sees the scar limit already reached, with no termination recorded, so it terminates the agent again before it can act:
  ```text
  work      denied: agent is terminated: scar limit reached (6 >= 6); termination record missing from the log
  ```
- **Cut off more**, the scars as well, and you're truncating history. Only a published checkpoint catches that; see [level 6](06-audit.md).

Run with `-- --reset` to start a new agent.

[Next: Proving what happened →](06-audit.md)
