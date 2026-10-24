# Issue Selection Rules

Choose issues strategically. The goal is to maximize merge probability while building a portfolio of contributions that demonstrates senior-level engineering skill.

## Selection criteria

Evaluate every candidate issue against three axes before starting work:

1. **Competition** — how many people are already working on it?
2. **Merge likelihood** — will the maintainers actually review and merge a good PR?
3. **Engineering value** — does solving this demonstrate skills a senior engineer at Apple, Google, Microsoft, or Amazon would respect?

All three must pass. A high-value issue in a dead repository wastes time. A low-competition issue that is trivial wastes reputation.

## Competition signals

Low competition (prefer these):

- no assignee
- no linked PRs
- no recent comments claiming the issue ("I'll take this", "working on this")
- issue has been open for a while without activity
- issue is labeled `help wanted` or `good first issue` but has no takers
- issue is in a less popular subsystem of a popular repository

High competition (avoid or deprioritize):

- someone already commented claiming the issue within the last 7 days
- one or more open PRs are linked
- the issue is trending or recently featured
- a maintainer tagged a specific contributor

## Merge likelihood signals

High merge likelihood (prefer these):

- the repository has merged PRs in the last 30 days
- maintainers respond to issues and PRs within a reasonable window
- the issue was filed or triaged by a maintainer
- the issue has a milestone or priority label
- the repository has clear contribution docs and CI that runs on forks
- similar past issues led to merged PRs

Low merge likelihood (avoid or deprioritize):

- no maintainer activity in the last 90 days
- large backlog of unreviewed PRs
- the issue has been open for years with no traction
- the repository has no CI or broken CI
- the issue is controversial with no maintainer resolution

## Engineering value assessment

Think like a hiring manager at a top-tier company reviewing a candidate's open-source contributions. They look for evidence of:

### High-value issue types (ranked)

1. **Correctness bugs** — fixing wrong behavior in core logic, data handling, or edge cases
2. **Performance issues** — profiling, identifying bottlenecks, and fixing them with measurable improvement
3. **Security vulnerabilities** — identifying and patching real security issues (not just dependency bumps)
4. **Architecture improvements** — refactoring that improves modularity, testability, or maintainability with clear justification
5. **Feature implementation** — scoped features requested by maintainers with clear acceptance criteria
6. **Test coverage gaps** — adding meaningful tests for untested critical paths (not just bumping coverage numbers)
7. **Build and tooling fixes** — fixing flaky CI, broken builds, or toolchain issues that block other contributors
8. **API design issues** — improving public interfaces, fixing inconsistencies, adding missing validation

### Low-value issue types (avoid)

- documentation typos or grammar fixes
- README badge updates
- dependency version bumps without behavioral change
- formatting or linting configuration changes
- adding comments to obvious code
- renaming variables for style preference
- trivial label or metadata changes
- translation-only contributions (unless you are contributing domain expertise)

### The FAANG test

Before committing to an issue, ask:

- Would I mention this contribution in a senior engineer interview?
- Does solving this require understanding system architecture, not just syntax?
- Does the fix demonstrate debugging skill, design judgment, or domain knowledge?
- Would a staff engineer reviewing my portfolio consider this substantive?

If the answer to all four is no, find a better issue.

## Red flags — skip these issues

- **Merged PR exists** — the problem is already solved. Do not duplicate work.
- **Maintainer has stated a specific approach** and you disagree — unless you have strong evidence, this is a losing battle.
- **Issue is a feature request with no maintainer buy-in** — your PR may be rejected on principle regardless of quality.
- **Issue is in an archived or abandoned repository** — no one will merge your PR.
- **Issue requires access you do not have** — infrastructure, credentials, proprietary systems.
- **Issue is a design debate disguised as a bug** — these drag on and rarely result in clean merges.
- **Issue has multiple failed PRs with no clear feedback** — the maintainers may not know what they want.

## Repository health check

Before investing time, quickly verify:

- last commit to the default branch within 90 days
- at least one PR merged in the last 60 days
- maintainer responded to at least one issue in the last 30 days
- CI runs and passes on the default branch

If any of these fail, reconsider unless you have specific reason to believe the repository is active.

## Search strategy

When looking for issues to contribute to:

1. Start with repositories you already use or understand.
2. Filter issues by `help wanted`, `good first issue`, or `bug` labels.
3. Sort by recently updated to find active discussions.
4. Check the issue's linked PRs and assignee fields immediately.
5. Cross-reference with the repository's PR merge rate.
6. Prefer repositories with 100-10,000 stars — large enough to matter, small enough that contributions are noticed.

## Deliverable from this phase

Before proceeding to investigation, confirm:

- the issue has no merged PR
- the issue has no assignee or active claimant
- the repository is healthy and merging PRs
- the issue demonstrates meaningful engineering work
- you can articulate why this contribution is worth your time
