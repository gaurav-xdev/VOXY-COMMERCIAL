# Senior Open Source Engineer Skill

Production-quality reusable skill framework for AI coding agents that need to contribute to mature open-source repositories like experienced senior engineers.

This project is not a single prompt. It is a structured engineering framework designed to make an agent:

- understand the repository before changing it
- keep diffs minimal
- follow local conventions
- place logic in the correct ownership layer
- write tests in the repository's native style
- prepare maintainer-friendly commits and PRs
- document engineering knowledge after each contribution

## What this repository contains

This repository ships two installable skill folders:

- `senior-open-source-engineer/`: the full framework
- `seo/`: a short alias entrypoint for agents that resolve slash commands from folder names

The main skill is intended for tools such as:

- Codex CLI
- Claude Code
- Gemini CLI
- Cursor
- Roo Code
- Cline
- Amp
- similar AI coding agents that support reusable prompt folders, skills, slash commands, or imported Markdown workflows

## Why this exists

Most AI coding prompts optimize for speed and code generation volume. Mature open-source repositories do not reward that behavior.

Maintainers usually want:

- a small diff
- correct architectural placement
- repository-native style
- existing abstractions reused instead of replaced
- tests that match the project's existing patterns
- a PR that is easy to review

This framework pushes an agent toward that contribution style.

## Core design goals

The framework optimizes for:

- mergeable pull requests
- engineering reasoning
- repository adaptation
- minimal diffs
- maintainability
- backwards compatibility
- production-quality code
- durable documentation of architecture and lessons learned

It does not optimize for writing lots of code.

## Repository layout

```text
senior-open-source-engineer-skill/
├── README.md
└── skills/
    ├── senior-open-source-engineer/
    │   ├── README.md
    │   ├── SKILL.md
    │   ├── CHECKLIST.md
    │   ├── ENGINEERING_LOG_TEMPLATE.md
    │   ├── IMPROVEMENTS.md
    │   ├── INSTALL_AND_USAGE.md
    │   ├── prompts/
    │   ├── rules/
    │   └── templates/
    └── seo/
        ├── README.md
        └── SKILL.md
```

## What the main skill enforces

The `senior-open-source-engineer` skill requires the agent to do the following before implementation:

1. evaluate the issue for competition, merge likelihood, and engineering value
2. read the full issue thread — every comment, not just the description
3. follow links in issue comments (related issues, discussions, docs)
4. check for linked PRs — skip if a merged PR exists
5. if unmerged PRs exist, read their diffs and all review comments
6. restate the issue clearly incorporating full thread context
7. define expected behavior
8. inspect repository structure
9. inspect `CONTRIBUTING.md`
10. inspect `README.md` and docs
11. detect build system
12. detect package manager
13. detect formatter
14. detect linter
15. detect test framework
16. detect CI
17. detect commit conventions
18. detect PR templates
19. detect issue templates
20. detect naming and code style
21. inspect architecture notes or ADRs
22. search similar issues or merged PRs
23. search related files
24. trace the owning execution path
18. search similar issues or merged PRs
19. search related files
20. trace the owning execution path

Only after that does it allow coding.

## Skill contents

### `skills/senior-open-source-engineer/SKILL.md`

Primary operating document. It defines the workflow, decision rules, implementation expectations, testing bar, review bar, PR preparation, and engineering log requirements.

### `skills/senior-open-source-engineer/CHECKLIST.md`

Per-contribution checklist to ensure the agent covers problem framing, repository adaptation, implementation discipline, testing, review, git, and engineering notes.

### `skills/senior-open-source-engineer/ENGINEERING_LOG_TEMPLATE.md`

Template for durable engineering journaling. The skill expects the agent to create or append to a local `~/.claude/engineering-log/ENGINEERING_LOG.md` file that is never committed to the target repository.

### `skills/senior-open-source-engineer/rules/`

Deeper guidance split by phase:

- `issue-selection.md`
- `issue-research.md`
- `architecture.md`
- `repository-adaptation.md`
- `engineering-mindset.md`
- `testing.md`
- `review.md`
- `git.md`
- `code-quality.md`

### `skills/senior-open-source-engineer/prompts/`

Specialized reusable task prompts:

- `investigate.md`
- `implement.md`
- `testing.md`
- `review.md`
- `journal.md`

### `skills/senior-open-source-engineer/templates/`

Reusable output templates:

- `resume-bullet.md`
- `star-story.md`
- `issue-summary.md`
- `commit-message.md`

### `skills/seo/`

Alias wrapper for shorter invocation in folder-derived slash-command systems.

## Installation

There is no universal installation mechanism across all AI coding agents. This repository is therefore packaged in the most portable format: plain folders with `SKILL.md` entrypoints.

### Quick install

Copy the two folders under `skills/` into your agent's skill or prompt directory:

```text
skills/
├── senior-open-source-engineer/
└── seo/
```

If your tool does not support aliases, you may install only `senior-open-source-engineer/`.

If your tool does not support folder-based skills, use:

- `skills/senior-open-source-engineer/SKILL.md`

as the main reusable prompt, while keeping the rest of the folder beside it so linked references still work.

## Agent-specific setup

### Codex CLI

Recommended target:

```text
~/.codex/skills/
├── senior-open-source-engineer/
└── seo/
```

Install:

1. copy `skills/senior-open-source-engineer/` to `~/.codex/skills/`
2. copy `skills/seo/` to `~/.codex/skills/`
3. start a new Codex session

Use:

```text
/senior-open-source-engineer
Fix this upstream bug. Learn repository conventions first, trace the owning abstraction, implement the smallest correct fix, add a repository-native regression test, and prepare PR-ready output.
```

Short alias:

```text
/seo
Investigate this issue before coding. Explain root cause, patch the right layer, and update the local engineering log.
```

### Claude Code

Claude Code installations vary, but the portable pattern is the same:

1. place `senior-open-source-engineer/` in the location used for reusable prompts or slash commands
2. place `seo/` beside it if command names are derived from folder names
3. if Claude Code supports aliases in config, map `seo` to `senior-open-source-engineer/SKILL.md`

Recommended usage:

```text
/senior-open-source-engineer
Treat this as a maintainer-facing open-source contribution. Follow contributing docs, match repository style, avoid unrelated refactors, and prepare a concise PR summary.
```

### Cursor

Cursor setups are less standardized for portable skills. Use one of these approaches:

1. add `skills/senior-open-source-engineer/SKILL.md` as a reusable prompt
2. add the framework as a project rule or shared ruleset
3. store the folder in your prompt library and create a personal shortcut named `seo`

Recommended pattern:

- load `SKILL.md`
- keep the full folder available so references to `rules/`, `prompts/`, and `templates/` remain usable
- tell Cursor to consult the linked files during the task, not just the entrypoint

### Cline and Roo Code

These tools generally work well with workspace-local prompt or skill folders.

Recommended setup:

1. copy `senior-open-source-engineer/` into a shared prompts or skills directory
2. copy `seo/` beside it if slash commands are folder-based
3. invoke the prompt and then provide a concrete engineering task

If the tool only supports a single reusable prompt file, use `SKILL.md` as the entrypoint.

### Gemini CLI

Gemini CLI wrappers vary, so use the generic pattern:

1. register `skills/senior-open-source-engineer/SKILL.md` as a reusable prompt or command
2. register `seo` as an alias if your wrapper supports it
3. keep the full folder present so linked documents are available

### Amp and similar agents

If the agent accepts prompt folders, install the entire folder.

If it accepts only a single prompt file, use:

- `skills/senior-open-source-engineer/SKILL.md`

and keep the rest of the folder alongside it.

## How to use the skill well

This framework performs best when the task is concrete.

### Good usage

```text
/seo
Fix the nil dereference in the webhook dispatcher. First inspect CONTRIBUTING, local tests, and recent related commits. Then identify the owning layer, implement the smallest correct fix, add the narrowest regression test, and draft the PR body.
```

### Better usage

```text
/seo
Issue: retry scheduling can duplicate jobs when the backoff state is empty.
Task:
- restate the bug
- inspect repository conventions
- trace the scheduling path
- identify the owning abstraction
- implement the smallest correct fix
- add a failing-then-passing regression test
- prepare commit message, PR text, and ENGINEERING_LOG entry
```

### Weak usage

```text
/seo
Improve this codebase.
```

The skill is strong, but it cannot infer product intent from vague tasks better than the repository itself can express.

## Recommended workflow

For any repository contribution:

1. invoke `/senior-open-source-engineer` or `/seo`
2. give the concrete issue or task
3. require the agent to inspect repository conventions before coding
4. require a minimal diff
5. require repository-native tests
6. require self-review before final output
7. require commit, PR, and engineering-log artifacts if you need contribution-ready output

## Example task recipes

### Investigation only

```text
/seo
Investigate this issue without coding yet. Restate the problem, inspect repository conventions, trace the execution path, identify the owning abstraction, and propose the smallest correct fix.
```

### Bug fix

```text
/seo
Fix this bug as an upstream-quality contribution. Learn the repository before editing, patch the smallest correct location, add a matching regression test, and prepare PR-ready output.
```

### Scoped feature

```text
/seo
Implement this feature in the style of the repository. Reuse existing abstractions, avoid unrelated refactors, add tests in local style, and explain why the chosen layer owns the behavior.
```

### Review only

```text
/seo
Review this proposed change like a maintainer. Focus on correctness, architecture fit, repository consistency, unnecessary complexity, and missing tests.
```

### Architecture learning

```text
/seo
Map the architecture of this subsystem before making changes. Explain entry points, ownership boundaries, side effects, contracts, and likely regression risks.
```

## How the `/seo` alias works

There is no universal cross-agent alias standard.

That is why this repository includes a second installable folder:

- `skills/seo/`

This alias skill simply redirects the agent to the full `senior-open-source-engineer` framework.

Use `seo/` when:

- your agent derives slash commands from folder names
- you want a shorter command

Do not rely on `seo/` if your agent already supports custom aliases in configuration. In that case, configure the alias directly.

## What makes this better than a single prompt

This project separates concerns by engineering phase instead of packing everything into one long instruction block.

Benefits:

- lower context overhead per task
- clearer operating stages
- easier maintenance
- easier extension
- better reuse across multiple agents

The framework contains:

- one main workflow document
- one contribution checklist
- one engineering log template
- six rules documents
- five specialized prompts
- four reusable templates

## Limitations

- there is no universal slash-command standard across all AI coding agents
- some tools only support single-file prompts, not skill folders
- some agents may not follow relative file references unless explicitly told to consult linked files
- the quality of repository adaptation still depends on the model actually reading the repository

This repository addresses those limits by staying plain, portable, and agent-agnostic.

## Suggested first use

If you want to test the framework quickly in a real repository:

```text
/seo
Investigate this bug as if you were preparing an upstream PR. Read repository conventions first, identify root cause and ownership, implement the narrowest fix, add matching regression coverage, and produce a concise PR summary.
```

## Included documentation

Inside `skills/senior-open-source-engineer/`:

- `README.md`: skill-local overview
- `INSTALL_AND_USAGE.md`: deeper install guidance
- `IMPROVEMENTS.md`: notes on how the packaged framework improves on earlier versions
- `CHECKLIST.md`: execution checklist
- `ENGINEERING_LOG_TEMPLATE.md`: append-ready journal template

## Contributing

If you extend this framework, keep the core design intact:

- do not optimize for code volume
- do not dilute repository adaptation
- do not encourage generic PR boilerplate
- do not replace the minimal-diff bias with broad refactoring guidance

Improvements should make the skill more repository-native, more portable, or easier to maintain.
