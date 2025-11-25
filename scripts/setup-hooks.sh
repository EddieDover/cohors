#!/bin/bash
# Setup git hooks for the Praeda project

set -e

echo "📦 Setting up git hooks..."

# Configure git to use .githooks directory
git config core.hooksPath .githooks

# Verify setup
HOOKS_PATH=$(git config core.hooksPath)
if [ "$HOOKS_PATH" = ".githooks" ]; then
    echo "✅ Git hooks configured successfully!"
    echo ""
    echo "The following hooks are now active:"
    echo "  • commit-msg: Enforces Conventional Commits format"
    echo "  • pre-commit: Runs clippy and tests before each commit"
    echo ""
    echo "For more information, see .githooks/README.md"
else
    echo "❌ Failed to configure git hooks"
    exit 1
fi
