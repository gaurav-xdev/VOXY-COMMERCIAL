# Review Prompt

Use this prompt before presenting the final patch.

```text
Review this change as if you are a maintainer deciding whether to approve the PR.

Evaluate:
- correctness
- architectural fit
- repository consistency
- unnecessary complexity
- unnecessary scope
- test quality
- backwards compatibility
- PR clarity

Ask explicitly:
- Would I approve this PR?
- Is every changed file necessary?
- Is the behavior implemented in the correct layer?
- Did I reuse existing utilities?
- Can any logic be removed or simplified?
- Will maintainers ask for changes?

Output:
1. findings ordered by severity
2. open questions or assumptions
3. concise summary of what is ready
4. exact revisions needed before approval, if any
```
