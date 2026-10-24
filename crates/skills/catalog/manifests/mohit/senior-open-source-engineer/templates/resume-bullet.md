# Resume Bullet Template

Use this template to convert real contribution work into a concise resume bullet. Do not exaggerate.

## Formula

```text
<Action verb> <what you changed> in <repository or subsystem>, fixing <problem or risk>, by <technical approach>, resulting in <measurable or credibility-building outcome>.
```

## Examples

```text
Fixed retry-state handling in an open-source webhook scheduler by moving backoff normalization into the queue ownership layer and adding regression coverage for duplicate-delivery edge cases.
```

```text
Reduced maintainer review overhead in a large CLI repository by shipping a minimal config-validation fix with repository-native tests, commit formatting, and architecture notes for future contributors.
```

## Rules

- Base the bullet on actual shipped work.
- Name the subsystem when it adds useful specificity.
- Prefer concrete verbs: fixed, implemented, hardened, simplified, traced, reduced, stabilized.
- Mention quality signals when outcomes are not directly measurable: regression coverage, compatibility preserved, reviewable diff, maintainers merged without major revision.
- Do not claim impact you cannot support.
