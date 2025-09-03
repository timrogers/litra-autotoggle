# Contributing to litra-autotoggle

Thank you for your interest in contributing to `litra-autotoggle`! This guide will help you get started with development and contributing to the project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Environment Setup](#development-environment-setup)
- [Building the Project](#building-the-project)
- [Code Style and Linting](#code-style-and-linting)
- [Testing](#testing)
- [Contributing Workflow](#contributing-workflow)
- [Platform-Specific Considerations](#platform-specific-considerations)
- [Release Process](#release-process)
- [Getting Help](#getting-help)

## Getting Started

`litra-autotoggle` is a Rust command-line application that automatically controls Logitech Litra devices based on webcam activity. The project supports macOS and Linux platforms.

### Prerequisites

- **Rust 1.89.0** (managed via rustup)
- **Git** for version control
- **Platform-specific dependencies**:
  - **Linux**: `libudev-dev` package
  - **macOS**: Xcode command line tools

## Development Environment Setup

### 1. Install Rust

If you don't have Rust installed, install it via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. Set the Correct Rust Version

This project uses Rust 1.89.0. Set this version:

```bash
rustup override set 1.89.0
rustup component add clippy rustfmt
```

### 3. Install Platform Dependencies

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get update && sudo apt-get install -y libudev-dev
```

#### Linux (Fedora/CentOS/RHEL)
```bash
sudo dnf install systemd-devel
# or
sudo yum install systemd-devel
```

#### macOS
Install Xcode command line tools:
```bash
xcode-select --install
```

### 4. Clone the Repository

```bash
git clone https://github.com/timrogers/litra-autotoggle.git
cd litra-autotoggle
```

### 5. Set Up Pre-commit Hooks (Optional but Recommended)

Install and set up pre-commit hooks for automated code quality checks:

```bash
pip install pre-commit
pre-commit install
```

This will automatically run code formatting, linting, and other checks before each commit.

## Building the Project

### Development Build

```bash
cargo build
```

### Release Build

```bash
cargo build --release
```

### Cross-platform Builds

The project supports multiple target platforms:

```bash
# Linux x86_64
cargo build --target x86_64-unknown-linux-gnu

# Linux ARM64
cargo build --target aarch64-unknown-linux-gnu

# macOS Intel
cargo build --target x86_64-apple-darwin

# macOS Apple Silicon
cargo build --target aarch64-apple-darwin
```

You may need to install additional targets:
```bash
rustup target add aarch64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

## Code Style and Linting

The project uses several tools to maintain code quality:

### Rust Formatting

Format your code using `rustfmt`:

```bash
cargo fmt
```

### Linting with Clippy

Run Clippy for additional linting:

```bash
cargo clippy --workspace --all-features --all-targets -- -D warnings
```

### Pre-commit Checks

Run all pre-commit checks manually:

```bash
pre-commit run --all-files
```

This includes:
- Rust formatting (`rustfmt`)
- Clippy linting
- General file checks (trailing whitespace, file endings, etc.)
- Spell checking with `codespell`

## Testing

Currently, the project does not have automated unit tests, but you can manually test the application:

### Manual Testing

1. **Build the project**:
   ```bash
   cargo build
   ```

2. **Test the help command**:
   ```bash
   ./target/debug/litra-autotoggle --help
   ```

3. **Test with a Litra device** (if available):
   ```bash
   ./target/debug/litra-autotoggle --verbose
   ```

### Integration Testing

Test the application with actual hardware:
- Connect a supported Litra device via USB
- Run the application and test webcam on/off events
- Verify the Litra device responds correctly

## Contributing Workflow

### 1. Create an Issue

Before starting work, create an issue describing:
- The problem you're solving
- Your proposed solution
- Any breaking changes

### 2. Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/timrogers/litra-autotoggle.git
   ```

### 3. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
```

### 4. Make Your Changes

- Keep commits atomic and well-described
- Follow the existing code style
- Update documentation if needed
- Add appropriate error handling

### 5. Test Your Changes

- Build and test locally
- Run pre-commit checks
- Test with actual hardware if possible

### 6. Submit a Pull Request

1. Push your branch to your fork
2. Create a pull request against the `main` branch
3. Include a clear description of your changes
4. Reference any related issues

### Pull Request Guidelines

- Use a clear, descriptive title
- Include a detailed description of changes
- Keep the scope focused and manageable
- Ensure CI checks pass
- Be responsive to review feedback

## Platform-Specific Considerations

### macOS

- The application requires camera access permissions
- Code signing and notarization are handled in CI for releases
- Universal binaries are created for both Intel and Apple Silicon

### Linux

- Requires `udev` rules for device access (see [`99-litra.rules`](99-litra.rules))
- Different distributions may require different dependency packages
- Video device monitoring works differently than macOS

### Cross-platform Code

When adding features:
- Use conditional compilation for platform-specific code: `#[cfg(target_os = "macos")]`
- Ensure both platforms are supported unless the feature is inherently platform-specific
- Test on both platforms when possible

## Release Process

The release process is automated through GitHub Actions:

### Version Bumping

1. Update the version in `Cargo.toml`
2. Update `Cargo.lock` by running `cargo check`
3. Create a commit with the version bump
4. Create and push a git tag: `git tag v0.x.x && git push origin v0.x.x`

### Automated Release Steps

The CI pipeline will automatically:
1. Build binaries for all supported platforms
2. Sign and notarize macOS binaries
3. Create a GitHub release with binary assets
4. Publish to [crates.io](https://crates.io)

### Manual Release Steps

If needed, you can publish manually:

```bash
# Dry run first
cargo publish --dry-run

# Publish to crates.io
cargo publish
```

## Getting Help

- **Issues**: Create a GitHub issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions and general discussion
- **Code Review**: Pull request reviews are the primary way to get code feedback

### Development Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
- [Litra Library Documentation](https://docs.rs/litra/)

## Code of Conduct

Please be respectful and constructive in all interactions. Follow the general principles of open source collaboration:

- Be welcoming to newcomers
- Be respectful of differing viewpoints
- Focus on constructive feedback
- Help others learn and grow

Thank you for contributing to `litra-autotoggle`!