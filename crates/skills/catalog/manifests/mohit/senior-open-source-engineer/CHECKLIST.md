# Engineering Checklist

Run this checklist for every contribution.

## Issue selection

- [ ] I verified the issue has no merged PR — the problem is not already solved.
- [ ] I checked assignees and recent comments — no one else has claimed the issue.
- [ ] I verified the repository is actively maintained (recent commits, PRs being merged).
- [ ] This issue demonstrates meaningful engineering skill, not just docs or formatting.
- [ ] A senior engineer at Apple, Google, Microsoft, or Amazon would consider this contribution substantive.

## Issue research

- [ ] I read every comment on the issue, not just the description.
- [ ] I followed links in issue comments (related issues, discussions, docs, RFCs).
- [ ] I checked for linked pull requests on the issue.
- [ ] If unmerged PRs exist, I read their diffs and all review comments.
- [ ] I understand why any existing PRs stalled or were rejected.
- [ ] I synthesized a preferred approach based on all available evidence.

## Problem framing

- [ ] I can restate the issue in precise engineering terms.
- [ ] I can explain expected behavior without relying on vague language.
- [ ] I know whether this is a bug fix, feature, refactor, or docs change.
- [ ] I identified the subsystem that owns the behavior.
- [ ] I can explain why the bug happens.
- [ ] I can explain why the fix belongs in the chosen layer.

## Repository adaptation

- [ ] I read `CONTRIBUTING.md` if it exists.
- [ ] I read the main `README` and the relevant docs.
- [ ] I detected the build system and package manager.
- [ ] I detected the formatter, linter, and test framework.
- [ ] I inspected CI configuration for relevant commands.
- [ ] I checked commit style from recent history.
- [ ] I checked for PR and issue templates.
- [ ] I identified local naming, logging, and error handling style.
- [ ] I looked for ADRs, architecture notes, or subsystem documentation.

## Investigation

- [ ] I searched related files, tests, issues, and merged PRs.
- [ ] I traced execution from trigger to effect.
- [ ] I identified the owning abstraction instead of patching the first visible symptom.
- [ ] I looked for existing utilities or helpers before adding a new one.
- [ ] I recorded edge cases that could regress.

## Implementation

- [ ] The change is the smallest diff that solves the issue.
- [ ] I avoided unrelated refactoring.
- [ ] I preserved existing abstractions where possible.
- [ ] I reused repository utilities, helpers, and types.
- [ ] I matched local naming and structure.
- [ ] Every changed line has a clear purpose.
- [ ] Any new comment explains non-obvious reasoning rather than restating code.

## Testing

- [ ] I identified how this repository structures tests.
- [ ] I added or updated the narrowest regression test that proves the fix.
- [ ] I matched local fixture, mock, and snapshot conventions.
- [ ] I ran the most relevant tests I could run.
- [ ] If I could not run tests, I documented why and noted residual risk.

## Self-review

- [ ] I reviewed the patch as if I were the maintainer.
- [ ] I removed unnecessary complexity.
- [ ] I verified the behavior lives in the right layer.
- [ ] I verified the tests prove the intended behavior.
- [ ] I checked for backwards compatibility concerns.
- [ ] I checked whether maintainers would ask why each changed file changed.

## Git and PR preparation

- [ ] I inferred commit style from the repository instead of guessing.
- [ ] I prepared a concise, repository-appropriate commit message.
- [ ] I followed the repository PR template exactly if one exists.
- [ ] I described the problem, root cause, solution, and testing.
- [ ] I kept the PR description factual and free of boilerplate.

## Engineering journal

- [ ] I appended to the local `~/.claude/engineering-log/ENGINEERING_LOG.md` (created it if it did not exist).
- [ ] I recorded why this issue was worth picking (contribution value and competition analysis).
- [ ] I captured architecture learned, alternatives considered, and lessons learned.
- [ ] I added a resume bullet and STAR story grounded in the actual work.
- [ ] I did not commit or push the engineering log to the target repository.
