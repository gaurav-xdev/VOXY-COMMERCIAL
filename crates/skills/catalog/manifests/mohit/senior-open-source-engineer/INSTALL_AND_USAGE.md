# Install and Usage

This skill is portable because it is plain Markdown plus a standard `SKILL.md` entrypoint.

## What you install

Use these folders together:

- `senior-open-source-engineer/`
- `seo/` if you want the short `/seo` alias in folder-name-based skill loaders

If your agent supports aliases in configuration, you can install only `senior-open-source-engineer/` and map `seo` to it yourself.

## Generic installation pattern

Most AI coding agents support one of these models:

1. a `skills/` or prompts directory containing one folder per skill
2. a custom slash command that points at a Markdown file or folder
3. a workspace rule file that references an external prompt folder

For model 1, copy:

```text
skills/
├── senior-open-source-engineer/
└── seo/
```

For model 2 or 3, point the command or rule to:

- `senior-open-source-engineer/SKILL.md`

## Codex CLI

Typical install target:

```text
~/.codex/skills/
├── senior-open-source-engineer/
└── seo/
```

Install:

1. Copy `senior-open-source-engineer/` into `~/.codex/skills/`
2. Copy `seo/` into `~/.codex/skills/` if you want the short alias
3. Start a new Codex session

Use:

```text
/senior-open-source-engineer
Fix the flaky retry test in this repository. Match local conventions, keep the diff minimal, and prepare a PR summary.
```

or:

```text
/seo
Investigate the root cause of this parser bug before coding. Then implement the smallest repository-native fix.
```

## Claude Code

Claude Code setups vary. Use whichever mechanism your install exposes for reusable prompt folders or slash commands.

Recommended approach:

1. Put `senior-open-source-engineer/` in the location Claude Code uses for custom prompts or shared commands.
2. If Claude Code resolves commands from folder names, also place `seo/` beside it.
3. If Claude Code uses a config file for aliases, map `seo` to `senior-open-source-engineer/SKILL.md`.

Use the skill by invoking the command and then giving a concrete task:

```text
/senior-open-source-engineer
Patch this bug as if you are contributing to the upstream repository. Follow contributing docs, infer commit style, and add a regression test.
```

## Cursor

Cursor does not use a single universal `skills/` convention across setups. The most reliable pattern is to paste or reference the skill from your project rules or reusable prompt library.

Practical options:

1. Add `senior-open-source-engineer/SKILL.md` as a reusable prompt.
2. Add a project rule that tells Cursor to follow the framework and linked rule files for repository contribution tasks.
3. Store the folder in your personal prompt library and create a shortcut named `seo`.

Best usage pattern:

- load `SKILL.md`
- attach the repository task
- tell Cursor to also consult the linked `rules/`, `prompts/`, and `templates/` files during the task

## Cline and Roo Code

These tools generally work well with folder-based prompt assets in the workspace.

Recommended setup:

1. Put `senior-open-source-engineer/` in a shared prompts or skills directory in your workspace.
2. Put `seo/` next to it if command names are folder-derived.
3. Invoke the prompt or slash command, then provide the repository task.

If the tool does not support folder commands directly, use:

- `senior-open-source-engineer/SKILL.md` as the main prompt
- the `rules/` directory as supplemental context

## Gemini CLI

Gemini CLI setups also vary by wrapper. Use the same portable pattern:

1. register `senior-open-source-engineer/SKILL.md` as a reusable command or prompt
2. register `seo` as an alias if supported
3. keep the full folder available so linked references still resolve

## Amp and similar coding agents

If the tool accepts reusable prompt folders, install the whole directory.

If it only accepts single prompt files, use:

- `SKILL.md` as the entrypoint

and keep the rest of the folder beside it so file references remain valid.

## How to use it well

This skill works best when you give it a concrete engineering task, not a vague instruction.

Good:

```text
/seo
Fix the nil dereference in the webhook dispatcher. First identify the owning layer, then patch the smallest correct location, add a regression test, and draft the PR body using repository conventions.
```

Better:

```text
/seo
Issue: webhook retries can be scheduled twice when the backoff state is empty.
Task:
- restate the bug
- inspect CONTRIBUTING, tests, and recent related commits
- trace the scheduling path
- implement the smallest fix
- add the narrowest failing-then-passing regression test
- prepare commit message, PR text, and ENGINEERING_LOG entry
```

Weak:

```text
/seo
Improve this codebase.
```

## Recommended workflow in any agent

1. Invoke `/senior-open-source-engineer` or `/seo`
2. Give the issue or task
3. Require the agent to inspect repository conventions before coding
4. Require a minimal diff
5. Require repository-native tests
6. Require self-review before final output
7. Require PR text and engineering log entry if you want contribution artifacts

## Minimal command cookbook

### Bug fix

```text
/seo
Fix this bug as an upstream-quality open-source contribution. Learn the repository conventions first, trace ownership, patch the narrowest layer, add a matching regression test, and prepare PR-ready output.
```

### Scoped feature

```text
/seo
Implement this small feature in the style of the repository. Reuse existing abstractions, avoid unrelated refactors, add tests in local style, and explain architectural placement.
```

### Review only

```text
/seo
Review this proposed change like a maintainer. Focus on correctness, architecture fit, repository consistency, and missing tests.
```

### Investigation only

```text
/seo
Investigate this issue without coding yet. Restate the bug, inspect repository conventions, trace execution flow, identify owning abstraction, and propose the smallest correct fix.
```

## Important limitation

There is no single cross-agent universal alias standard for `/seo`.

That is why this package includes:

- the full `senior-open-source-engineer/` skill
- a separate `seo/` alias folder for tools that derive commands from folder names

If your tool does not support slash commands at all, use `SKILL.md` as a reusable prompt and keep the rest of the folder next to it.
