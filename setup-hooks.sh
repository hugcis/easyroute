#!/bin/bash

# Setup script to install git hooks for the project

set -e

echo "Setting up git hooks for EasyRoute..."

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo "Error: Not a git repository. Please run this script from the project root."
    exit 1
fi

# Create .git/hooks directory if it doesn't exist
mkdir -p .git/hooks

# Copy pre-commit hook
echo "Installing pre-commit hook..."
cp hooks/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

echo "✅ Git hooks installed successfully!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: Runs cargo fmt, clippy, and check before each commit"
echo ""
echo "To temporarily skip hooks, use: git commit --no-verify"
