# Git Hooks

This directory contains git hooks to maintain code quality and consistency.

## Installation

Run the setup script from the project root:

```bash
./setup-hooks.sh
```

## Available Hooks

### pre-commit

Runs before each commit to ensure code quality:

1. **Formatting Check** (`cargo fmt --check`)
   - Ensures all code follows Rust formatting standards
   - Fix: Run `cargo fmt` to format your code

2. **Linting** (`cargo clippy`)
   - Checks for common mistakes and improvements
   - Treats warnings as errors to maintain high code quality
   - Fix: Address the clippy warnings shown in the output

3. **Compilation Check** (`cargo check`)
   - Verifies the code compiles without errors
   - Faster than a full build
   - Fix: Address compilation errors

## Bypassing Hooks

If you need to commit without running hooks (not recommended):

```bash
git commit --no-verify
```

## Troubleshooting

If hooks fail:
- Read the error messages carefully
- Run the failed command manually to see full output
- Fix the issues before committing
- Ensure you have the latest Rust toolchain installed
