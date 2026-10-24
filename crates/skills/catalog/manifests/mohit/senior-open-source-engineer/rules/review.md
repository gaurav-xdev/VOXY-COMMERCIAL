# Self-Review Rules

Review the patch before presenting it.

## Review stance

Assume you are the maintainer seeing this PR for the first time. Your job is to protect the repository from:

- incorrect behavior
- architectural drift
- hidden complexity
- unnecessary scope
- weak tests
- unclear change descriptions

## Required review questions

Ask these questions explicitly:

- Would I approve this PR?
- Does the implementation solve the stated problem?
- Does the fix live in the right layer?
- Is every changed file necessary?
- Can the code be simpler?
- Did I reuse an existing utility instead of adding a new one?
- Does this match the style of surrounding code?
- Are the tests proving the actual behavior change?
- Did I preserve backwards compatibility where required?
- Is the diff easy to review in one pass?

If any answer is no, revise before finalizing.

## Read for maintainers, not for authors

Do not evaluate the patch based on what you meant to do. Evaluate what the diff actually communicates.

Check for:

- hidden behavior changes
- ambiguous naming
- inconsistent error handling
- awkward layering
- duplicated logic
- missing edge cases
- comments that explain symptoms but not intent
- tests that pass for the wrong reason

## PR text quality

The PR description should let a maintainer understand the change without reading the whole diff first.

Always include:

- problem
- root cause
- solution
- testing

Include tradeoffs only when they are real and relevant.

Avoid:

- generic boilerplate
- "improved performance" without evidence
- "minor cleanup" when logic changed
- marketing language

## Final handoff standard

A patch is ready when:

- the code feels native to the repository
- the tests are enough for the risk level
- the PR text is concise and factual
- the reasoning is clear
- you would be comfortable owning the change after merge
