# Documentation

This directory contains all project documentation organized by type and audience.

## Directory Structure

### `architecture/`
System architecture and design documents.
- Domain-driven design patterns
- Component interactions
- Data flow diagrams
- Performance considerations

### `user-guides/`
Documentation for end users.
- [User Guide](./user-guides/USER_GUIDE.md) - Complete manual for CLI and GUI
- [Multi-OS Support](./user-guides/MULTI_OS_SUPPORT.md) - Supported operating systems and setup
- [QEMU Comparison](./user-guides/QEMU_FEATURE_COMPARISON.md) - Feature comparison with QEMU

### `api/`
API documentation for developers.
- Module-specific documentation
- API references
- Usage examples
- Integration guides

### `development/`
Development guides and reports.
- Development setup
- Contribution guidelines
- Architecture analysis
- Performance reports
- [Production Guide](./development/AMD64_ON_ARM_GUI_PRODUCTION_READY.md)

## Quick Links

### For Users
- [Getting Started](../README.md#quick-start)
- [User Guide](./user-guides/USER_GUIDE.md)
- [Supported Operating Systems](./user-guides/MULTI_OS_SUPPORT.md)

### For Developers
- [Architecture](./architecture/)
- [API Documentation](./api/)
- [Development Guide](./development/)
- [CONTRIBUTING.md](./development/CONTRIBUTING.md)

## Documentation Standards

All documentation should:
1. Be written in Markdown
2. Use clear, concise language
3. Include code examples where applicable
4. Be kept up-to-date with code changes
5. Follow the existing style guide

## Adding Documentation

When adding new documentation:

1. Choose the appropriate directory based on audience
2. Use descriptive filenames with kebab-case
3. Update this README with a brief description
4. Link from relevant sections
5. Ensure all links are working

## Building Documentation

To build and view API documentation locally:

```bash
# Build documentation for all crates
cargo doc --all --no-deps --open

# Build with private items
cargo doc --all --no-deps --document-private-items --open
```

## Questions?

For questions about documentation structure or adding new docs, please refer to [CONTRIBUTING.md](./development/CONTRIBUTING.md).
