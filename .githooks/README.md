# Git Hooks for VM Project

This directory contains Git hooks to maintain code quality.

## Available Hooks

### `pre-commit` (Default)

Comprehensive checks that run before each commit:
- Code formatting (cargo fmt)
- Clippy linting
- Compilation check
- Unit tests
- Documentation tests
- Large file detection
- Sensitive information scanning
- TODO/FIXME tracking

**Runtime**: 1-3 minutes depending on changes

### `pre-commit-fast` (Alternative)

Lightweight checks for frequent commits:
- Format check on changed files only
- Clippy on affected packages
- Quick compilation check

**Runtime**: 10-30 seconds

## Setup

### Use Standard Pre-commit Hook (Recommended)

The hook is already configured if you ran `scripts/setup_dev_env.sh`:

```bash
# Verify it's installed
ls -la .git/hooks/pre-commit

# Should show a symlink to ../../.githooks/pre-commit
```

### Use Fast Pre-commit Hook

For faster commits during development:

```bash
# Switch to fast hook
ln -sf ../../.githooks/pre-commit-fast .git/hooks/pre-commit

# Switch back to full hook
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
```

### Disable Hooks Temporarily

To skip hooks for a single commit:

```bash
git commit --no-verify -m "WIP: work in progress"
```

### Disable Hooks Permanently

Not recommended, but if needed:

```bash
# Remove the hook
rm .git/hooks/pre-commit

# Or tell git to ignore hooks globally
git config --global core.hooksPath /dev/null
```

## Running Hooks Manually

```bash
# Run the standard hook
.git/hooks/pre-commit

# Run the fast hook
.git/hooks/pre-commit-fast
```

## Customizing Hooks

Edit the hook files directly:
- `.githooks/pre-commit` - Standard hook
- `.githooks/pre-commit-fast` - Fast hook

Changes take effect immediately for the next commit.

## Troubleshooting

### Hook Permission Denied

```bash
chmod +x .githooks/pre-commit
chmod +x .githooks/pre-commit-fast
```

### Hook Not Running

Check if the symlink exists:
```bash
ls -la .git/hooks/pre-commit
```

If missing, recreate it:
```bash
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
```

### Hook Too Slow

1. Use the fast hook: `ln -sf ../../.githooks/pre-commit-fast .git/hooks/pre-commit`
2. Skip temporarily: `git commit --no-verify -m "message"`
3. Adjust the hook to skip certain checks

## Best Practices

1. **Keep hooks fast**: They run before every commit
2. **Fix issues immediately**: Don't accumulate warnings
3. **Use fast hook during active development**: Switch to full hook for PRs
4. **Don't bypass hooks often**: Only use --no-verify for genuine WIP commits
5. **Run full checks before PRs**: Use CI/CD for comprehensive testing

## CI/CD Integration

These hooks complement CI/CD checks:
- **Local hooks**: Quick feedback, prevent bad commits
- **CI/CD**: Comprehensive checks, run on all PRs

Both should pass before merging.
