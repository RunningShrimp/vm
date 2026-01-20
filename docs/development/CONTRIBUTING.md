# Contributing to VM Project

Thank you for your interest in contributing! This document provides guidelines for contributing to the VM project.

## Getting Started

### Prerequisites

- Rust 1.92 or later
- Platform-specific tools:
  - **Linux**: KVM headers (`sudo apt install linux-headers-$(uname -r)` on Ubuntu)
  - **macOS**: Xcode command-line tools (`xcode-select --install`)
  - **Windows**: Windows SDK and Visual Studio Build Tools

### Setting Up the Development Environment

```bash
# Clone the repository
git clone <repository-url>
cd vm

# Install pre-commit hooks (if configured)
cd .githooks && ./install.sh

# Build the workspace
cargo build --all

# Run tests
cargo test --all

# Format code
cargo fmt --all

# Check for linting issues
cargo clippy --all -- -D warnings
```

## Development Workflow

### Branching Strategy

We follow a simplified Git workflow:

1. `main` - Stable production code
2. Feature branches from `main`: `feature/description`
3. Bugfix branches from `main`: `fix/description`
4. Experimental branches: `exp/experiment-name`

### Commit Messages

Follow the Conventional Commits specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Example:
```
feat(vm-mem): add NUMA-aware memory allocation

Implement NUMA topology detection and optimize memory
allocation for multi-socket systems.

Closes #123
```

### Code Review Process

1. Create a feature branch
2. Make your changes
3. Ensure all tests pass
4. Submit a pull request with:
   - Clear description of changes
   - Related issue numbers
   - Testing performed
5. Address review feedback
6. Wait for approval and merge

## Code Standards

### Rust Code Style

We follow strict Rust code quality standards:

- **No `unwrap()`** in production code - use proper error handling
- **No `unsafe`** without clear justification and safety comments
- **Documentation** on all public APIs (`///` comments)
- **Type safety** - prefer type-safe APIs over dynamic checks
- **No `as any`, `@ts-ignore`** - type errors must be resolved

### Linting

All workspace members enforce strict linting via workspace lints:

```toml
[workspace.lints.rust]
warnings = "deny"
future_incompatible = "deny"
nonstandard_style = "deny"

[workspace.lints.clippy]
all = "deny"
pedantic = "deny"
cargo = "deny"
```

Before submitting, run:

```bash
# Check all lints
cargo clippy --all -- -D warnings

# Format code
cargo fmt --all -- --check
```

### Testing

- Unit tests for all modules
- Integration tests for major features
- Property-based tests where applicable (proptest)
- Benchmark tests for performance-critical code

Run tests:
```bash
# All tests
cargo test --all

# With output
cargo test --all -- --nocapture

# Release mode tests
cargo test --all --release

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin --all --out Html
```

## Architecture Guidelines

### Domain-Driven Design

This project uses DDD principles:
- **Aggregates**: Cluster of domain objects treated as a unit
- **Domain Events**: Events that capture something that happened in the domain
- **Repositories**: Abstractions for persistence
- **Services**: Stateless operations on domain objects

### Module Organization

- Keep modules focused and cohesive
- Minimize inter-module dependencies
- Use clear module boundaries
- Prefer composition over inheritance

### Error Handling

- Use `thiserror` for defining error types
- Use `anyhow` for application-level error propagation
- Provide context for errors (`.context()` from anyhow)
- Never panic in production code

## Documentation

### Code Documentation

- All public functions and types must have documentation comments
- Include examples in documentation
- Document invariants, constraints, and usage patterns
- Keep documentation in sync with code changes

### Project Documentation

- Update relevant documentation in `docs/`
- Add examples to `examples/` directories
- Update `README.md` for user-facing changes
- Document breaking changes in migration guides

## Performance Guidelines

### Critical Paths

- Profile before optimizing
- Benchmark performance-critical code
- Use criterion for benchmarks
- Document performance characteristics

### Memory Management

- Minimize allocations in hot paths
- Use stack allocation where possible
- Consider lock-free data structures for high-contention areas
- Leverage SIMD for data-parallel operations

## Testing Guidelines

### Test Organization

```
vm-module/
├── src/
│   └── lib.rs
├── tests/
│   ├── integration_tests.rs  # Integration tests
│   └── property_tests.rs      # Property-based tests
├── examples/
│   └── example.rs
└── benches/
    └── bench.rs
```

### Test Coverage

- Aim for >80% coverage for core modules
- Test error paths
- Test edge cases and boundaries
- Use property-based tests for complex invariants

## Issue Reporting

### Bug Reports

Include:
- Description of the bug
- Steps to reproduce
- Expected behavior
- Actual behavior
- Environment (OS, Rust version, etc.)
- Logs or error messages
- Minimal reproduction example if possible

### Feature Requests

Include:
- Problem description
- Proposed solution
- Alternatives considered
- Additional context
- Use cases and benefits

## Community Guidelines

### Code of Conduct

- Be respectful and constructive
- Welcome newcomers and help them learn
- Focus on what is best for the community
- Show empathy toward other community members

### Communication

- Use clear, professional language
- Provide context when asking questions
- Be patient with responses
- Assume good intentions

## Release Process

Releases are managed by maintainers:

1. Version bump in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Tag release
4. Push to main
5. Build and publish to crates.io (if applicable)
6. Create GitHub release

## Getting Help

- **Documentation**: Check `docs/` directory
- **Issues**: Search existing issues before creating new ones
- **Discussions**: Use GitHub Discussions for questions
- **Chat**: (Add link to community chat if available)

## Recognition

Contributors are acknowledged in:
- `CONTRIBUTORS.md`
- Release notes
- Project documentation

Thank you for contributing!
