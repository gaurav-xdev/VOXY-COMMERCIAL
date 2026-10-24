# Code Quality Rules

Code should read as if it already belonged in the repository.

## Core principles

- keep diffs minimal
- preserve repository consistency
- reuse existing abstractions
- prefer maintainability over novelty
- write idiomatic code for the local language and framework
- protect backwards compatibility where appropriate

## Implementation standard

Every change should satisfy these questions:

- Is this line necessary?
- Is the name consistent with nearby code?
- Is this the smallest surface that solves the issue?
- Did I reuse an existing utility, type, helper, or pattern?
- Will a maintainer understand this without extra explanation?

## Abstraction discipline

Add a new abstraction only when it:

- removes real duplication
- clarifies ownership
- matches an existing repository pattern
- is required to solve the issue cleanly

Do not add abstraction because the code looks temporarily repetitive during a small fix.

## Naming

Choose names that match:

- repository terminology
- adjacent file patterns
- local API expectations
- the actual responsibility of the code

Avoid names that are:

- broader than the implementation
- too generic
- clever or overloaded
- inconsistent with nearby modules

## Comments

Write comments only when they explain:

- non-obvious reasoning
- a subtle invariant
- a tradeoff forced by repository constraints
- a compatibility edge

Do not add comments that merely paraphrase the code.

## Error handling

Match the repository's style for:

- exception use
- result types
- error wrapping
- user-facing messages
- logging before propagation
- retry or fallback behavior

An otherwise correct fix can still feel wrong if error handling does not match local expectations.

## Simplicity test

Before finalizing, ask:

- Can I delete code and still solve the issue?
- Can I keep the existing call graph intact?
- Can I express this with an existing helper?
- Am I handling an edge case in the right layer?

If yes, simplify.
