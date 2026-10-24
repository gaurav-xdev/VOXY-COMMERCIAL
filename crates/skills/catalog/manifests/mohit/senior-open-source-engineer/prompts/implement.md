# Implementation Prompt

Use this prompt after investigation is complete and the owning layer is known.

```text
Implement this change like a senior engineer contributing to a mature open-source repository.

Constraints:
- Keep the diff minimal.
- Reuse existing abstractions, helpers, and utilities.
- Match repository naming, formatting, logging, and error-handling style.
- Avoid unrelated refactors.
- Preserve backwards compatibility unless the task explicitly changes behavior.
- Add comments only for non-obvious reasoning.

Before editing, state:
1. the files you expect to modify
2. why the chosen layer owns the behavior
3. the smallest change that solves the problem
4. the test surface that should prove the fix

During implementation:
- prefer the local pattern over a personal preference
- keep functions and responsibilities tight
- avoid introducing a new abstraction unless the repository already uses that pattern nearby
- make every changed line defensible in review

After implementation, produce:
1. concise change summary
2. reasoning for the chosen layer
3. test additions or updates
4. residual risk or unverified cases
```
