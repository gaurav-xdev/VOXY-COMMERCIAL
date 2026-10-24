# Commit Message Template

Infer the style from the repository before choosing a format.

## Detection order

1. Inspect recent commit subjects.
2. Check contribution docs and release tooling.
3. Look for scopes, prefixes, issue references, and footer conventions.
4. Match the established style exactly.

## Conventional Commit pattern

```text
<type>(<scope>): <imperative summary>

<optional body>

<optional footer>
```

Example:

```text
fix(queue): normalize retry state before scheduling
```

## Plain imperative pattern

```text
<Imperative summary>

<optional body>
```

Example:

```text
Normalize retry state before scheduling webhook jobs
```

## Repository-specific issue-tag pattern

```text
<prefix or issue tag> <imperative summary>
```

Example:

```text
[scheduler] Fix retry normalization for delayed jobs
```

## Rules

- Keep the subject concise.
- Use imperative mood.
- Mention the real subsystem when the repository commonly scopes subjects.
- Do not claim broad refactors if the diff is narrow.
- Add a body only when context is needed for reviewers or release tooling.
