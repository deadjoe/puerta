# Development Setup Guide

This guide explains how to set up a development environment for Puerta that includes the required Pingora and RCProxy dependencies.

## Prerequisites

- Rust 1.70+ with cargo
- Git
- Make (for convenience commands)

## Setting up Dependencies

The Puerta project depends on Cloudflare's Pingora framework and uses RCProxy patterns for Redis cluster handling. Since these are external dependencies, they need to be cloned and configured for local development.

### 1. Clone Dependencies

```bash
# Clone Pingora framework
git clone https://github.com/cloudflare/pingora.git examples/pingora
cd examples/pingora
git checkout v0.1.1  # Use stable version
cd ../..

# Clone RCProxy for Redis cluster patterns
git clone https://github.com/dashbitco/rcproxy.git examples/rcproxy
cd examples/rcproxy
# Use main branch for latest Redis cluster handling
cd ../..
```

### 2. Enable Local Dependencies

Uncomment the path dependencies in `Cargo.toml`:

```toml
[dependencies]
# Uncomment these lines for local development:
pingora = { path = "examples/pingora" }
pingora-core = { path = "examples/pingora/pingora-core" }
pingora-load-balancing = { path = "examples/pingora/pingora-load-balancing" }
```

### 3. Development Commands

```bash
# Run all tests
make test

# Run code quality checks
make check

# Format code
make fmt

# Run clippy lints
make lint

# Run security audit
make audit

# Build release version
make build-release

# Run benchmarks
make bench
```

## Project Structure

```
puerta/
├── src/                    # Core implementation
│   ├── lib.rs             # Main library
│   ├── modes/             # MongoDB and Redis proxy modes
│   │   ├── mongodb/       # MongoDB TCP proxy using Pingora
│   │   └── redis/         # Redis cluster proxy using RCProxy patterns
│   ├── config/            # Configuration management
│   ├── health/            # Health checking
│   └── utils/             # Utility functions
├── examples/              # External dependencies (dev only)
│   ├── pingora/          # Cloudflare Pingora framework
│   └── rcproxy/          # Redis cluster proxy reference
├── benches/              # Performance benchmarks
├── tests/                # Integration tests
└── target/               # Build artifacts
```

## Architecture Overview

### MongoDB Mode
- **Purpose**: Session-aware TCP load balancing for MongoDB Sharded Clusters
- **Approach**: Transparent TCP forwarding to mongos instances
- **Technology**: Pingora TCP proxy with session affinity
- **Protocol**: Transparent (no MongoDB Wire Protocol parsing needed)

### Redis Mode  
- **Purpose**: Protocol-aware proxy for Redis Cluster
- **Approach**: RESP protocol parsing with MOVED/ASK redirection handling
- **Technology**: RCProxy-style Redis cluster patterns on Pingora
- **Protocol**: RESP protocol aware with slot-based routing

## Testing

The project maintains comprehensive test coverage:

```bash
# Run unit tests
cargo test

# Run with coverage
cargo tarpaulin --out Html

# Run integration tests
cargo test --test integration

# Run specific module tests
cargo test mongodb
cargo test redis
```

## Performance Testing

```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo flamegraph --bin puerta

# Load testing (requires external tools)
# See docs/load-testing.md for details
```

## Troubleshooting

### Compilation Errors

If you see errors about missing Pingora modules:

1. Ensure you've cloned the dependencies to `examples/`
2. Uncomment the path dependencies in `Cargo.toml`
3. Check that the Pingora version matches the expected API

### Test Failures

Some tests may be sensitive to system networking configuration:

1. Connection timeout tests may behave differently on various systems
2. Health check tests require actual network connectivity
3. Use `RUST_LOG=debug` for detailed test output

### Performance Issues

For optimal performance in development:

1. Use `cargo build --release` for performance testing
2. Set appropriate `RUST_LOG` levels (warn/error for production)
3. Monitor system resources during load testing

## Contributing

When making changes:

1. Run `make check` before committing
2. Ensure test coverage remains above 70%
3. Update documentation for API changes
4. Follow existing code patterns and conventions

## Repository State

This development setup is for local work only. The main repository excludes the `examples/` directory to maintain clean distribution. The production build uses crates.io versions of dependencies when available.