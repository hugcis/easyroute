# GitHub Workflows Documentation

This directory contains GitHub Actions workflows and configurations for the EasyRoute project.

## Workflows

### 1. CI (`ci.yml`)
**Trigger**: Push to main/master branches, Pull requests

**Jobs**:
- **Test Suite**: Runs comprehensive tests with PostgreSQL (PostGIS) and Redis services
  - Code formatting check (`cargo fmt`)
  - Linting with Clippy (`cargo clippy`)
  - Database migration
  - Unit and integration tests

- **Cargo Check**: Fast compilation check without running tests

- **Security Audit**: Runs `cargo audit` to check for known vulnerabilities

- **Code Coverage**: Generates test coverage reports using `cargo-tarpaulin` and uploads to Codecov

**Services**:
- PostgreSQL 15 with PostGIS 3.3
- Redis 7

### 2. Pull Request (`pr.yml`)
**Trigger**: Pull request events (opened, synchronized, reopened)

**Jobs**:
- **Quick Check**: Fast pre-test validation
  - Code formatting
  - Clippy linting
  - Build check

- **Test**: Full test suite (runs after quick-check passes)
  - Database migrations
  - All tests

This workflow is optimized for speed with job dependencies to fail fast.

### 3. Release (`release.yml`)
**Trigger**: Git tags matching `v*.*.*` (e.g., `v1.0.0`)

**Jobs**:
- **Build and Release**: Creates release binaries for multiple platforms
  - Linux (x86_64, ARM64)
  - macOS (x86_64, ARM64)

- **Create Release**: Creates a GitHub release with all built binaries

**Usage**:
```bash
git tag v1.0.0
git push origin v1.0.0
```

### 4. Scheduled Tasks (`scheduled.yml`)
**Trigger**: Daily at 2 AM UTC (also manually via workflow_dispatch)

**Jobs**:
- **Daily Security Audit**: Runs `cargo audit` and creates an issue if vulnerabilities are found

- **Dependency Check**: Checks for outdated dependencies using `cargo-outdated`

## Dependabot Configuration (`dependabot.yml`)

Automatically creates pull requests for:
- **Cargo dependencies**: Weekly updates for Rust crates
- **GitHub Actions**: Weekly updates for action versions

**Configuration**:
- Max 10 PRs for Cargo dependencies
- Max 5 PRs for GitHub Actions
- Auto-labels: `dependencies`, `rust`, `github-actions`
- Reviewer: `hugcis`

## Required Secrets

The workflows use the following secrets (some are automatically provided by GitHub):

### Automatic (No configuration needed)
- `GITHUB_TOKEN`: Automatically provided by GitHub Actions

### Optional (for enhanced features)
- `CODECOV_TOKEN`: For uploading coverage reports to Codecov (optional, workflow continues without it)

## Workflow Badges

Add these badges to your main README.md:

```markdown
[![CI](https://github.com/hugcis/easyroute/workflows/CI/badge.svg)](https://github.com/hugcis/easyroute/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/hugcis/easyroute/branch/main/graph/badge.svg)](https://codecov.io/gh/hugcis/easyroute)
```

## Caching Strategy

All workflows use optimized Rust caching via `Swatinem/rust-cache@v2`:
- **Automatic caching** of Cargo registry, git dependencies, and build artifacts
- **Incremental compilation** artifacts preserved between runs
- **Shared caches** across jobs with different `shared-key` values
- **Smart invalidation** based on Cargo.lock and toolchain changes
- **Compression** optimized for Rust projects

Benefits over manual caching:
- 3-5x faster cache restoration
- Automatic cleanup of outdated cache entries
- Handles all Rust-specific cache directories
- Works seamlessly with workspace projects

Cache keys used:
- `test` - Main test suite
- `check` - Cargo check job
- `coverage` - Code coverage job
- `pr-quick` - PR quick checks
- `pr-test` - PR test suite
- `audit` - Security audit

## Environment Variables

Common environment variables across workflows:
- `DATABASE_URL`: PostgreSQL connection string
- `REDIS_URL`: Redis connection string
- `MAPBOX_API_KEY`: Set to `test_api_key_for_ci` (tests should mock external APIs)
- `RUST_LOG`: Logging configuration
- `CARGO_TERM_COLOR`: Always enabled for better output
- `RUST_BACKTRACE`: Enabled for better error debugging

## Running Workflows Locally

To test workflows locally, consider using [act](https://github.com/nektos/act):

```bash
# Install act
brew install act  # macOS
# or
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash  # Linux

# Run the CI workflow
act -j test

# Run with specific event
act pull_request
```

## Troubleshooting

### Migration Failures
If database migrations fail in CI:
1. Ensure migrations are committed to the repository
2. Check that `sqlx-cli` version matches local development
3. Verify PostgreSQL service is healthy before migrations run

### Test Failures
If tests fail only in CI:
1. Check that all required services (PostgreSQL, Redis) are running
2. Verify environment variables are set correctly
3. Ensure tests don't rely on local-only resources

### Cache Issues
If builds are slower than expected:
1. Check cache hit rate in workflow logs
2. Verify `Cargo.lock` is committed
3. Consider clearing cache manually via GitHub UI (Settings > Actions > Caches)

## Contributing

When adding new workflows:
1. Test locally with `act` if possible
2. Use caching appropriately
3. Add documentation to this README
4. Consider job dependencies to fail fast
5. Use descriptive job and step names
