# Justfile for puerta development
# Alternative to Makefile with better cross-platform support

# Default recipe
default:
    @just --list

# Development setup
setup:
    @echo "Setting up development environment..."
    @echo "Cloning Pingora framework..."
    git clone https://github.com/cloudflare/pingora.git examples/pingora || echo "Pingora already exists"
    @echo "Cloning RCProxy for Redis patterns..."
    git clone https://github.com/dashbitco/rcproxy.git examples/rcproxy || echo "RCProxy already exists"
    @echo "Setup complete! Edit Cargo.toml to uncomment path dependencies."

# Clean setup
clean-deps:
    rm -rf examples/pingora examples/rcproxy

# Test commands
test:
    cargo test

test-verbose:
    cargo test -- --nocapture

test-coverage:
    cargo tarpaulin --out Html

# Code quality
fmt:
    cargo fmt

lint:
    cargo clippy -- -D warnings

check: fmt lint test

# Security audit
audit:
    cargo audit

# Build commands
build:
    cargo build

build-release:
    cargo build --release

# Benchmarks
bench:
    cargo bench

# Documentation
doc:
    cargo doc --open

# Clean
clean:
    cargo clean

# Full development cycle
dev-check: setup check test-coverage
    @echo "Development environment ready!"