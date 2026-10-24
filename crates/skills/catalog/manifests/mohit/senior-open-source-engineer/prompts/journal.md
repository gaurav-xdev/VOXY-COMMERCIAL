# Engineering Journal Prompt

Use this prompt when writing or appending to the local `~/.claude/engineering-log/ENGINEERING_LOG.md`.

This file is your personal contribution tracker. Never commit or push it to any target repository.

```text
Write an engineering journal entry for this contribution using ENGINEERING_LOG_TEMPLATE.md.

Requirements:
- append to the local file at ~/.claude/engineering-log/ENGINEERING_LOG.md
- create the file and directory if they do not exist
- be factual, not promotional
- capture why this issue was selected (contribution value and competition analysis)
- capture architecture learned, not just actions taken
- explain root cause and why the chosen solution fits the repository
- list files inspected and files modified
- record tests actually run
- note alternatives considered and why they were rejected
- set merge outcome to "pending" (update later when the PR is resolved)
- extract one realistic resume bullet
- extract one realistic STAR interview story

Output the entry in append-ready Markdown.
```
