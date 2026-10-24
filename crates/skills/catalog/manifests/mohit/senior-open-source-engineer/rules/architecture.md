# Architecture Investigation Rules

Learn the architecture before proposing a fix.

## Goals

- understand ownership
- trace behavior through the real execution path
- separate symptom location from root-cause location
- capture durable architectural knowledge for future changes

## Start with ownership, not files

When you find a relevant file, do not assume the fix belongs there. First answer:

- Is this file an entry point, coordinator, adapter, domain owner, helper, or renderer?
- Does it own the behavior, or does it merely expose it?
- Where does data become validated, transformed, persisted, or displayed?
- Where are errors translated or suppressed?

If you cannot classify the file's role, you do not yet understand the subsystem.

## Trace the flow end to end

For any non-trivial change, trace the request or event through these layers when they exist:

1. entry point
2. parsing or validation
3. orchestration
4. domain logic
5. persistence or network boundary
6. presentation or serialization

Note the contract at each boundary:

- input shape
- output shape
- failure behavior
- side effects
- invariants

## Separate symptom from source

Common traps:

- UI shows bad output, but normalization is wrong upstream.
- API returns wrong status, but domain logic is swallowing the real error.
- A retry loop misbehaves, but the bug is in state transition ownership.
- A flaky test reveals shared mutable state in fixtures or caches.

Fix the source when possible. Patch the symptom only when the symptom layer truly owns the contract.

## Build an ownership statement

Before implementation, write a one-sentence statement:

`This behavior belongs in <module or layer> because <architectural reason>.`

Good reasons:

- this layer defines the contract
- this module already owns normalization
- this boundary translates external errors
- adjacent behavior already lives here
- moving it elsewhere would duplicate business rules

Weak reasons:

- this file was easy to find
- the failing test imports this module
- changing a deeper layer feels risky
- the caller already has the data available

## Look for architecture signals

Prioritize these clues:

- ADRs and design docs
- package boundaries
- service or handler interfaces
- module naming patterns
- shared helpers used in similar code paths
- tests that exercise the same behavior
- recent merged PRs touching the same subsystem

When signals conflict, prefer the local subsystem's established pattern over a repo-wide generalization.

## Ask the hard questions

During investigation, answer:

- Why does the bug happen now?
- What assumption broke?
- Why is the current layer the wrong or right place to fix it?
- Which module would a maintainer expect to own this logic?
- Which other modules depend on the current behavior?
- What coupling could this change introduce?
- Are there hidden invariants in tests, serialization, migrations, or CI?

## Record architecture knowledge

For each contribution, capture:

- entry points involved
- owning abstraction
- upstream dependencies
- downstream consumers
- key invariants
- unexpected subsystem behavior

Write those facts into the local `ENGINEERING_LOG.md`. The log should teach the next engineer how this area works.
