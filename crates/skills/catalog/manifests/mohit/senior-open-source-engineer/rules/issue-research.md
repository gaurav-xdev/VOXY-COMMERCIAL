# Issue Research Rules

Understand the full context of an issue before writing any code. The issue description is the starting point, not the whole picture.

## Read the entire comment thread

Do not stop at the issue body. Read every comment, in order, from oldest to newest.

Extract:

- clarifications from the reporter
- additional reproduction steps or edge cases discovered later
- maintainer opinions on preferred approach
- constraints or requirements added after the initial report
- disagreements between commenters and how they were resolved
- any "this is related to X" links

If a maintainer has stated a preferred direction, follow it unless you have strong evidence it will not work.

## Follow links in comments

Comments often contain critical context that is not in the issue body:

- **Related issues** — read them. They may reveal scope, prior decisions, or known constraints.
- **External discussions** — forum threads, mailing list archives, Discord/Slack links, RFC documents. Read what is accessible.
- **Stack Overflow or blog posts** — these may explain the underlying problem domain.
- **Documentation links** — specs, API docs, or design documents that define expected behavior.
- **Code permalinks** — follow them to understand which code paths commenters are referencing, but verify the lines are still current (the code may have changed since the link was posted).

Do not skip links because they seem tangential. A link posted by a maintainer is almost never tangential.

## Analyze existing PRs on the issue

Check whether any pull requests are linked to the issue. This is critical.

### If a merged PR exists

**Stop.** The issue is resolved. Do not submit a duplicate fix.

Verify by:

- checking the issue's linked PRs section
- searching for the issue number in recent commits
- checking if the issue is closed with a "fixed by" reference

If the issue is still open but a PR was merged that appears to address it, note this and move on.

### If an open (unmerged) PR exists

This is intelligence, not a blocker. Read it thoroughly:

1. **Read the PR diff** — understand what approach the author took.
2. **Read every review comment** — understand what reviewers liked and disliked.
3. **Read the PR description** — understand the author's reasoning.
4. **Check CI status** — did tests pass? Did linters flag issues?
5. **Identify why it stalled** — common reasons:
   - reviewer requested changes that were never addressed
   - the approach was rejected in favor of a different design
   - the author went inactive
   - merge conflicts accumulated
   - the PR was too large or mixed concerns
   - tests were missing or insufficient

Use this information to build a better solution:

- If the approach was sound but incomplete, consider completing the work (with attribution).
- If the approach was rejected, understand the rejection reason and design around it.
- If the PR stalled on review feedback, address that feedback in your approach from the start.
- If the PR was too large, consider splitting the work into smaller, more mergeable pieces.

### If multiple unmerged PRs exist

Read all of them. Look for patterns:

- Do they all fail on the same review feedback? That feedback is a hard requirement.
- Do they take different approaches? Compare tradeoffs.
- Did the maintainer express preference for one approach over others?
- Is there a PR that was close to merging? Understand what was missing.

If multiple contributors have failed to merge a fix, the issue may have hidden requirements or the maintainers may not have clear acceptance criteria. Proceed with caution or pick a different issue.

## Build context before coding

After completing the research, you should be able to answer:

- What exactly is the problem? (from the full thread, not just the title)
- What has already been tried? (from existing PRs)
- What did maintainers accept or reject? (from review comments)
- What constraints exist that are not in the issue body? (from comments and linked discussions)
- What approach has the best chance of being merged? (synthesized from all evidence)

If you cannot answer these, keep reading. Do not start coding with incomplete context.

## When to abandon an issue

After research, you may discover the issue is not worth pursuing:

- The issue is already solved by a merged PR but not yet closed.
- Multiple high-quality PRs have been rejected without clear feedback — the maintainers may not know what they want.
- The issue requires changes the maintainers have explicitly refused.
- The issue depends on a larger architectural change that is not in progress.
- The maintainer conversation reveals political or personal dynamics that make a clean merge unlikely.
- The solution requires access or knowledge you cannot reasonably obtain.

Abandoning early is a sign of good judgment, not failure. Record what you learned in your engineering log and move to a better issue.

## Research checklist

Before proceeding to implementation:

- [ ] Read every comment on the issue
- [ ] Followed all links in comments and extracted relevant context
- [ ] Checked for linked PRs (merged or open)
- [ ] If merged PR exists: stopped and moved on
- [ ] If unmerged PRs exist: read the diffs and all review comments
- [ ] Identified why unmerged PRs failed or stalled
- [ ] Synthesized a preferred approach based on all evidence
- [ ] Confirmed the issue is still relevant and unresolved
