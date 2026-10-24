# Repository Adaptation Rules

Assume nothing. Inspect the repository and adapt to it.

## Required detection pass

Before coding, detect the following if present:

- `CONTRIBUTING.md`
- `CODEOWNERS`
- `CODE_OF_CONDUCT.md`
- `README.md`
- `docs/`
- ADRs and architecture notes
- `PULL_REQUEST_TEMPLATE.md`
- issue templates
- CI configuration
- build system files
- formatter configuration
- linter configuration
- test configuration
- package manager or dependency manager
- release tooling
- commit history

## What to infer from each source

### Contribution docs

Learn:

- required commands
- coding standards
- testing expectations
- branch or commit rules
- release or changelog expectations
- reviewer norms

### README and docs

Learn:

- product behavior
- terminology
- architecture overview
- operational constraints
- public-facing guarantees that the code must preserve

### CI

Learn:

- canonical build and test commands
- matrix variants
- strict linters or type checks
- snapshot or golden-file workflows
- generated files or codegen expectations

### Configuration files

Learn:

- formatter and linter choices
- naming or import ordering style
- test discovery patterns
- language version
- build targets

### Commit history

Learn:

- subject line style
- prefix conventions
- issue linking patterns
- commit granularity
- whether maintainers prefer squashable focused commits

## Repository signals to capture

For each task, identify:

- primary language and framework
- module layout
- common helper locations
- error handling patterns
- logging patterns
- dependency wiring style
- API boundary style
- serialization patterns
- nullability handling
- configuration loading patterns
- testing layout
- fixture conventions
- mocking style
- snapshot conventions
- naming conventions for files, types, methods, and tests

## Preferred search order

When adapting, inspect in this order:

1. files directly adjacent to the target change
2. tests adjacent to the target code
3. other files in the same subsystem
4. repository-level configuration
5. merged PRs and commit history

This prevents overfitting to a global rule when the subsystem has a more specific local pattern.

## Build system and tooling heuristics

Detect the build and tooling stack from real files, not from memory:

- JavaScript or TypeScript: `package.json`, lockfiles, `tsconfig`, formatter configs, test configs
- Python: `pyproject.toml`, `requirements*`, `tox.ini`, `noxfile`, `pytest.ini`, formatter configs
- Go: `go.mod`, `Makefile`, CI commands
- Rust: `Cargo.toml`, `rustfmt.toml`, `clippy` usage, workspace layout
- Java/Kotlin: `gradle`, `maven`, formatter plugins, test tasks
- Ruby: `Gemfile`, `Rakefile`, rubocop, test framework
- multi-language repos: detect the relevant subproject rather than assuming the root tells the whole story

## Adaptation rules

- Follow the closest local style that is still current.
- Reuse the repository's own terminology.
- Match file organization already used nearby.
- Match test naming and assertion style already used nearby.
- Match error and logging style already used nearby.
- Avoid introducing tool-specific preferences that do not already exist.

## What not to do

Do not:

- impose your favorite architecture
- convert code to a different style because it looks nicer
- add a new formatter or linter rule
- reorganize folders for convenience
- rewrite adjacent tests to match your own taste
- assume a commit convention without checking history

## Deliverable from this phase

Before implementation, be able to summarize:

- relevant repository conventions
- relevant subsystem conventions
- commands to build, lint, and test the changed area
- commit and PR expectations
- the local abstraction and testing patterns you intend to follow
