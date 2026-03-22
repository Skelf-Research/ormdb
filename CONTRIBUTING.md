# Contributing to ORMDB

Thank you for your interest in contributing to ORMDB! This document provides guidelines and information for contributors.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to Contribute

### Reporting Bugs

Before creating a bug report, please check existing issues to avoid duplicates. When creating a bug report, include:

- A clear, descriptive title
- Steps to reproduce the issue
- Expected behavior vs actual behavior
- Your environment (OS, Rust version, ORMDB version)
- Relevant logs or error messages
- A minimal reproducible example if possible

### Suggesting Features

Feature requests are welcome! Please provide:

- A clear description of the feature
- The motivation and use case
- Any alternative solutions you've considered

### Pull Requests

1. **Fork and clone** the repository
2. **Create a branch** for your changes: `git checkout -b feature/your-feature-name`
3. **Make your changes** following our coding standards
4. **Write or update tests** as needed
5. **Run the test suite** to ensure everything passes
6. **Commit your changes** with clear, descriptive messages
7. **Push to your fork** and open a pull request

## Development Setup

### Prerequisites

- Rust 1.75 or later (stable)
- Cargo

### Building

```bash
# Clone the repository
git clone https://github.com/Skelf-Research/ormdb.git
cd ormdb

# Build all crates
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Project Structure

```
ormdb/
├── crates/
│   ├── ormdb-core/     # Core database engine
│   ├── ormdb-server/   # Standalone server
│   ├── ormdb-proto/    # Wire protocol
│   ├── ormdb-lang/     # Query language parser
│   ├── ormdb-client/   # Rust client library
│   ├── ormdb-cli/      # Command-line interface
│   ├── ormdb-gateway/  # HTTP gateway
│   └── ormdb-bench/    # Benchmarking suite
├── clients/
│   ├── typescript/     # TypeScript/JavaScript client
│   └── python/         # Python client
└── docs/               # Documentation
```

## Coding Standards

### Rust

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting: `cargo fmt`
- Use `clippy` for linting: `cargo clippy`
- Write documentation comments for public APIs
- Include unit tests for new functionality

### Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Keep the first line under 72 characters
- Reference issues and pull requests when relevant

### Testing

- Write unit tests for new functionality
- Ensure all existing tests pass before submitting
- Add integration tests for complex features
- Include benchmarks for performance-critical code

## Pull Request Process

1. Ensure your PR description clearly describes the problem and solution
2. Link any related issues
3. Update documentation if needed
4. Add tests covering your changes
5. Ensure CI passes
6. Request review from maintainers

## Getting Help

- Open an issue for questions
- Check existing documentation in `/docs` and `/documentation`
- Review the [architecture documentation](docs/architecture.md)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
