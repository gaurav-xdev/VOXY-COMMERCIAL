# senior-open-source-engineer

`senior-open-source-engineer` is a reusable coding skill for AI agents that need to contribute to mature open-source repositories with the habits of a strong senior engineer.

It is built for repository comprehension first, implementation second. The skill pushes an agent to learn architecture, adapt to project conventions, keep diffs small, write reviewable code, match existing testing style, and leave behind durable engineering knowledge.

## When to use it

Use this skill when the work should look like a careful maintainer contribution rather than a quick prototype:

- fixing bugs in established repositories
- implementing narrow features in mature codebases
- preparing repository-appropriate commits and PR descriptions
- learning a new repository before making changes
- contributing to public open-source projects where maintainer trust matters
- documenting architectural knowledge and lessons learned after a change

Do not use it for greenfield brainstorming, exploratory prototypes, code golf, or "generate as much code as possible" tasks.

## Supported coding agents

The skill is written in plain Markdown so it can be used by coding agents that load prompt folders or slash-command skills, including:

- Codex CLI
- Claude Code
- Gemini CLI
- Cursor
- Roo Code
- Amp
- Cline
- similar folder-based or prompt-based coding agents

## Install

1. Copy the `senior-open-source-engineer/` folder into your agent's `skills/` directory.
2. Invoke it with `/senior-open-source-engineer` in agents that map commands from folder names.
3. For a short `/seo` command in folder-name-based agents, also copy or symlink the included `seo/` alias folder next to it.

Example target layout:

```text
skills/
├── senior-open-source-engineer/
└── seo/
```

If your agent supports custom aliases in its own configuration, map `seo` to `senior-open-source-engineer`.

## Example usage

```text
/senior-open-source-engineer
Fix the race condition in the webhook retry scheduler. Match repository style, add the smallest regression test that proves the bug, and prepare a PR summary.
```

```text
/seo
Investigate why the CLI exits with status 0 on invalid config. Explain root cause, patch the correct ownership layer, and update the local engineering log.
```

## Expected workflow

1. Evaluate the issue for competition, merge likelihood, and engineering value.
2. Read the full issue thread — every comment, every link, every linked PR.
3. If a merged PR exists, skip the issue. If unmerged PRs exist, study why they failed.
4. Read the issue or task carefully and restate it.
5. Discover repository conventions before writing code.
6. Trace the relevant execution path and identify the owning abstraction.
7. Search for existing helpers, patterns, and related fixes.
8. Implement the narrowest change that solves the problem.
9. Add tests in the repository's established style.
10. Run self-review as if approving the PR.
11. Produce a repository-appropriate commit message and PR description.
12. Append an entry to the local engineering log (never committed to the target repo).

## Contents

- `SKILL.md`: primary operating instructions (10-phase workflow)
- `CHECKLIST.md`: per-contribution checklist covering issue selection through git preparation
- `ENGINEERING_LOG_TEMPLATE.md`: local-only contribution journal template (never pushed to target repos)
- `rules/`: deeper guidance on issue selection, issue research, architecture, adaptation, quality, testing, review, and git
- `prompts/`: specialized operator prompts for investigation, implementation, testing, review, and journaling
- `templates/`: reusable templates for commit messages, issue summaries, resume bullets, and STAR stories

## Design goals

This skill optimizes for:

- mergeable pull requests
- minimal and reviewable diffs
- repository consistency
- maintainability
- clear reasoning
- production-quality code
- strong testing discipline
- durable engineering notes

It does not optimize for code volume.
