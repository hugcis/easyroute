# Git Commit Guidelines

Conventions for creating clean, reviewable commits in this repository.

## Commit Message Format

```
<type>(<scope>): <summary>

<optional body — explain why, not what>

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

**Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`
**Scopes**: `route-generator`, `poi-service`, `cache`, `api`, `db`, `config`, `ci`, `devx`

Examples:
```
feat(route-generator): add adaptive waypoint count per retry attempt
fix(cache): correct TTL calculation for POI region cache
docs: update CLAUDE.md with comprehensive project documentation
chore: add MIT license
```

## Atomic Commit Principles

Each commit should represent **one logical change** that can be reviewed independently.

**Group together**: files that implement the same concern — a new endpoint means its handler, query, route registration, and tests belong in one commit.

**Split apart**:
- Infrastructure/config changes vs feature code
- New dependencies vs the code that uses them (when the dep is large/notable)
- Documentation updates vs code changes
- Test-only changes vs implementation (unless the test is specifically for the new code)

## Commit Ordering

Commits should build on each other logically. Prefer this order:
1. Dependencies and infrastructure (licenses, CI, Docker)
2. Core logic changes (services, algorithms)
3. API surface changes (endpoints, routes)
4. Developer experience (tooling, scripts)
5. Documentation

## Pre-Commit Checks

This repo has a pre-commit hook that runs:
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`
4. `cargo build` (compile check)

Fix any issues **before** starting the commit sequence — otherwise the hook will reject mid-way through and you'll need to amend or re-commit. Run `cargo fmt` and fix clippy warnings across the entire working tree first.

## Workflow: Staging Atomic Commits from Mixed Changes

When the working tree has many unrelated changes:

1. `git reset HEAD` — unstage everything to start clean
2. `git diff --stat` — survey all changes
3. Read each diff to understand logical groupings
4. For each commit: `git add <files...>` then `git commit`
5. `git log --oneline` — verify the final history reads clearly
