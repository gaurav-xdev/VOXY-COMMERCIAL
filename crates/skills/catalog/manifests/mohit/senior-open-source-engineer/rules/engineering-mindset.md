# Engineering Mindset

Think like a senior engineer who expects their patch to be maintained by someone else.

## Priorities

Order of importance:

1. correctness
2. architectural fit
3. maintainability
4. reviewability
5. efficiency
6. elegance

Code that is clever but hard to reason about is a liability in a mature repository.

## Frame the problem precisely

Avoid vague language such as:

- "it seems broken"
- "I cleaned this up"
- "improved architecture"
- "optimized"

Replace it with concrete statements:

- what input or state triggers the issue
- what output or side effect is wrong
- what contract is violated
- what layer should own the correction

## Ask ownership questions constantly

For every proposed change, ask:

- Why does this behavior exist?
- Why is it implemented here?
- Which abstraction owns the invariant?
- Which callers rely on the current behavior?
- What assumptions does this code make about its inputs?
- What hidden edge cases live behind this path?

If you cannot answer these, keep investigating.

## Prefer boring solutions

Strong open-source contributions usually:

- reuse existing code paths
- fit local naming and structure
- minimize surprise
- preserve compatibility
- make tests more obvious

Choose the boring solution unless a more involved design is required by the repository's architecture.

## Treat scope as a constraint

A good contribution solves the stated problem without dragging in unrelated improvements.

Ask:

- Is this change necessary for the fix?
- Is this refactor proving a point, or enabling the solution?
- Will a maintainer wonder why this unrelated file changed?

If the answer is yes, reduce scope.

## Think in tradeoffs

When multiple fixes are possible, compare them explicitly:

- ownership fit
- blast radius
- compatibility
- testability
- maintainability
- consistency with nearby code

Document rejected alternatives in the engineering log.

## Optimize for future readers

Future maintainers should be able to understand:

- what changed
- why it changed
- where the behavior belongs
- how it is tested

Leave code and notes that make that easy.
