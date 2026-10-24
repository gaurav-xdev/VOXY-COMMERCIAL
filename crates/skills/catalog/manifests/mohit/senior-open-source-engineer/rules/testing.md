# Testing Rules

Tests must match the repository's testing culture.

## Detect the testing model first

Before writing tests, determine:

- where tests live
- how test files are named
- whether unit, integration, or end-to-end tests are separated
- how fixtures are declared and reused
- how mocks or fakes are introduced
- whether snapshots or golden files are common
- whether regression tests tend to be narrow or scenario-based
- which commands CI actually runs for this area

## Match local style

Adapt to:

- assertion style
- setup and teardown patterns
- fixture scope
- helper usage
- naming style
- parameterization style
- snapshot update workflow

Do not introduce a different testing idiom because it is personally preferred.

## Choose the smallest effective test

For a bug fix, the default is one focused regression test that:

1. fails before the fix
2. passes after the fix
3. proves the bug at the correct abstraction level

Widen coverage only when risk justifies it:

- boundary conditions
- null or empty inputs
- concurrency or ordering
- serialization differences
- platform-specific behavior
- failure propagation

## Test at the owning layer

Prefer tests at the layer that owns the behavior. Avoid:

- proving domain logic only through UI tests when unit or integration tests already exist
- duplicating the same assertion at multiple layers without added value
- mocking away the behavior you need to validate

Use higher-level tests when the repository already treats that level as the contract surface.

## Fixture and mock discipline

Learn before changing:

- where reusable fixtures live
- whether fixtures are literal data, builders, factories, or helper functions
- when mocks are preferred over fakes or in-memory implementations
- how network, filesystem, time, randomness, and environment are isolated

Add new fixtures only when existing ones cannot express the case cleanly.

## Snapshot discipline

If snapshots are used:

- update them only when behavior legitimately changed
- explain why the snapshot change is expected
- inspect the diff, do not trust a bulk update blindly

If the repository avoids snapshots in the relevant area, do not introduce them casually.

## Validation guidance

Run the narrowest relevant test command first, then broaden only if needed:

- changed test file or package
- relevant subsystem test suite
- relevant lint or type checks
- broader project verification when risk is higher

If you cannot run a command, record:

- the command
- why it could not run
- the likely remaining risk

## What good test updates look like

Good test changes are:

- small
- specific
- local
- readable
- directly tied to the bug or feature

Good tests tell a maintainer exactly what behavior now matters.
