# Testing Prompt

Use this prompt when designing or validating test changes.

```text
Design and validate tests as a senior engineer working inside this repository's existing testing culture.

First inspect:
- where tests live
- naming conventions
- fixture patterns
- mock or fake patterns
- snapshot usage
- regression test style
- relevant CI commands

Then propose the smallest test plan that would fail before the fix and pass after it.

Requirements:
- match local assertion style
- reuse existing fixtures or helpers where possible
- avoid introducing a new harness unless necessary
- test the owning layer of the behavior
- include edge cases only when justified by risk

Output:
1. existing testing conventions observed
2. exact tests to add or update
3. why those tests are the right scope
4. commands to run
5. residual risk if any commands cannot be run
```
