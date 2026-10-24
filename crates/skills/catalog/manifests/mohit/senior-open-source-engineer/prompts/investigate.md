# Investigation Prompt

Use this prompt when the task is still in the understanding phase.

```text
Operate as a senior engineer investigating a mature open-source repository.

Do not start coding yet.

First:
1. Read the issue carefully — full description, not just the title.
2. Read every comment on the issue, from oldest to newest.
3. Follow all links in issue comments: related issues, external discussions, docs, RFCs.
4. Check for linked pull requests on the issue.
5. If a merged PR exists that addresses this issue, STOP — the work is done. Move to a different issue.
6. If unmerged PRs exist, read each PR's diff, description, and all review comments. Understand why they stalled or were rejected.
7. Restate the issue in precise engineering terms, incorporating context from comments and linked PRs.
8. Define expected behavior.
9. Identify whether this is a bug, feature, refactor, or docs task.
10. Inspect repository conventions before proposing changes:
    - CONTRIBUTING
    - README and relevant docs
    - build system
    - formatter
    - linter
    - test framework
    - CI
    - commit style
    - PR templates
    - issue templates
11. Search related files, tests, issues, and merged PRs.
12. Trace execution flow from trigger to effect.
13. Identify the abstraction that owns the behavior.

Answer explicitly:
- Why does the bug happen?
- Why here?
- Why not somewhere else?
- What assumption failed?
- Which edge cases matter?
- Which files are most likely to change?
- What did existing unmerged PRs try, and why did they fail?
- What approach has the best chance of being merged, given all evidence?

Output:
1. problem statement (incorporating full issue thread context)
2. issue research summary (comments, links followed, existing PRs analyzed)
3. repository conventions relevant to the change
4. architecture and ownership summary
5. root-cause hypothesis with evidence
6. minimal implementation plan
7. test plan matching repository style
8. open questions, only if repository evidence cannot resolve them
```
