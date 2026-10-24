# Git and PR Rules

Use git history as a repository convention source, not just a transport mechanism.

## Commit style detection

Never invent a commit format. Inspect recent history and detect:

- Conventional Commits
- Angular-style prefixes
- issue references in subjects
- imperative plain-English subjects
- scope formatting
- footer conventions

Match the repository's existing style.

## Commit granularity

Prefer small focused commits that preserve review clarity.

Good commit boundaries:

- one bug fix
- one supporting test update
- one necessary preparatory refactor tightly coupled to the fix

Avoid bundling:

- cleanup
- renames
- formatting churn
- unrelated test rewrites

## Branch and history discipline

When working in a dirty tree:

- do not revert unrelated changes
- do not absorb unrelated edits into the patch
- understand adjacent user changes before editing shared files

Preserve user work unless explicitly asked to rewrite or discard it.

## PR preparation

Before drafting PR text:

- inspect for repository PR templates
- detect multiple templates and choose the right one
- review recent merged PRs for tone and structure
- keep the description aligned with the repository's norms

## PR content expectations

A strong PR description usually covers:

1. what was wrong
2. why it happened
3. what changed
4. how it was tested

Add:

- screenshots only when the repository expects them
- migration notes only when behavior or configuration changes
- follow-up ideas only when they are clearly out of scope for this PR

## Safe git habits

- prefer non-destructive commands
- avoid resetting or overwriting user work
- do not rewrite history unless explicitly asked
- verify staged or changed files match the intended scope

The best git outcome is a small, intentional, easy-to-review patch.
