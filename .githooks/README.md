# Git Hooks for Praeda

This directory contains git hooks that enforce code quality and commit standards.

## Hooks

### `commit-msg` Hook
Enforces [Conventional Commits](https://www.conventionalcommits.org/) format for all commit messages.

**Allowed commit types:**
- `feat:` A new feature
- `fix:` A bug fix
- `refactor:` Code refactoring without feature or bug fix
- `docs:` Documentation changes
- `test:` Adding or updating tests
- `chore:` Maintenance tasks (dependencies, build, etc.)
- `perf:` Performance improvements
- `style:` Code style changes (formatting, etc.)
- `ci:` CI/CD configuration changes
- `build:` Build system changes
- `revert:` Reverting a previous commit

**Examples:**
```
feat: add user authentication
fix(auth): handle expired tokens
docs: update API documentation
chore(deps): bump clap to 4.5
refactor: simplify metadata handling
```

### `pre-commit` Hook
Runs code quality checks before committing:
- **Clippy** - Rust linter to catch common mistakes
- **Tests** - Runs all library tests

This ensures only high-quality code is committed.

## Setup

The hooks are already configured for this repository. When you clone it, git will automatically use these hooks.

### Manual Setup (if needed)

If the hooks aren't working, run:

```bash
git config core.hooksPath .githooks
```

## Bypassing Hooks (Use with caution!)

If you need to bypass hooks in an emergency:

```bash
# Skip pre-commit hook
git commit --no-verify

# For commit-msg hook, there's no simple bypass - the message must be valid
```

**Note:** Bypassing hooks is discouraged and should only be done in exceptional circumstances.
