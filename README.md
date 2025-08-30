<img src="./logo.png" alt="Puerta banner image" width="25%">

A high-performance, production-ready database cluster proxy for MongoDB Sharded Clusters and Redis Clusters. Built on Cloudflare's battle-tested Pingora framework with enterprise-grade reliability and performance.

## Overview

Puerta provides intelligent database cluster proxying with advanced session management, health checking, and load balancing capabilities:

- **MongoDB Mode**: NAT-friendly session affinity with multi-strategy client identification
- **Redis Mode**: Full Redis Cluster protocol support with automatic redirection handling
- **Unified Architecture**: Consistent error handling, monitoring, and configuration across modes

## Key Features

### 🚀 High Performance
- **Zero-copy I/O**: Optimized data forwarding with 64KB buffers
- **Async Architecture**: Built on Tokio for high-concurrency handling
- **Memory Efficient**: Object pooling and reference counting optimizations
- **Low Latency**: P99 < 10ms response times under load

### 🎯 MongoDB Mode
- **Advanced Session Affinity**: Multi-strategy client identification (SocketAddr, Fingerprint, SessionID, Hybrid)
- **NAT-Friendly**: SHA-256 connection fingerprinting for complex network environments
- **Wire Protocol Health Checks**: MongoDB `ismaster` command with retry mechanisms
- **Intelligent Load Balancing**: Weighted round-robin with health-aware backend selection
- **Session Lifecycle Management**: Configurable timeouts and automatic cleanup

### 🔄 Redis Mode
- **Full RESP Protocol Support**: Complete Redis protocol parsing and handling
- **Cluster Slot Management**: CRC16-based key slot calculation and mapping
- **Automatic Redirection**: Seamless MOVED/ASK redirection handling
- **Topology Discovery**: Dynamic Redis cluster node discovery and updates
- **Connection Optimization**: Efficient connection pooling and reuse

### 🛡️ Enterprise Features
- **Production Deployment**: Daemon mode with PID management and service integration
- **Zero-Downtime Operations**: Graceful reload and upgrade capabilities
- **Unified Error Handling**: Comprehensive error classification and recovery strategies
- **Health Check System**: Configurable health monitoring with Wire Protocol validation
- **Observability**: Structured logging, metrics collection, and performance monitoring
- **Configuration Management**: TOML-based config with validation and hot-reload support

## Architecture

Puerta is built on proven, high-performance foundations:

- **Pingora Framework**: Cloudflare's battle-tested proxy framework for TCP/HTTP load balancing
- **RCProxy Architecture**: Redis cluster proxy design patterns for protocol-aware proxying
- **Tokio Runtime**: Async I/O for high-concurrency connection handling

## Installation

### Prerequisites

- Rust 1.70 or later
- Git

### Build from Source

**Note**: This project includes local dependencies on Pingora framework components. The repository already includes the necessary Pingora framework in the `examples/pingora/` directory.

```bash
git clone https://github.com/deadjoe/puerta
cd puerta

# The Pingora framework is already included in examples/pingora/
cargo build --release
```

## Configuration

Puerta uses TOML configuration files. Example configurations are provided in the `config/` directory.

### MongoDB Mode Configuration

```toml
[server]
listen_addr = "0.0.0.0:27017"
max_connections = 10000
connection_timeout_sec = 60
worker_threads = 4

# Optional daemon mode configuration
[server.daemon]
enabled = true                           # Enable daemon mode
pid_file = "/var/run/puerta.pid"         # PID file path
error_log = "/var/log/puerta/error.log"  # Error log file
upgrade_sock = "/var/run/puerta_upgrade.sock"  # Upgrade socket path
user = "puerta"                          # User to run as (optional)
group = "puerta"                         # Group to run as (optional)

[proxy]
mode = "mongodb"
mongos_endpoints = [
    "mongodb1.example.com:27017",
    "mongodb2.example.com:27017",
    "mongodb3.example.com:27017"
]
session_affinity = true
session_timeout_sec = 1800

[health]
interval_sec = 30
```

### Redis Mode Configuration

```toml
[server]
listen_addr = "0.0.0.0:6379"
max_connections = 10000
connection_timeout_sec = 60
worker_threads = 4

[proxy]
mode = "redis"
cluster_nodes = [
    "redis1.example.com:6379",
    "redis2.example.com:6379", 
    "redis3.example.com:6379"
]
slot_refresh_interval_sec = 60
max_redirects = 3
connection_timeout_ms = 5000

[health]
interval_sec = 30
```

## Usage

### Running Puerta

#### Basic Usage

```bash
# Start with MongoDB configuration
./target/release/puerta run --config config/mongodb.toml

# Start with Redis configuration  
./target/release/puerta run --config config/redis.toml

# Use default configuration (config/dev.toml)
./target/release/puerta run

# Enable debug logging
RUST_LOG=debug ./target/release/puerta run --config config/mongodb.toml
```

#### Production Deployment (Daemon Mode)

```bash
# Run as daemon process
./target/release/puerta run --config config/mongodb.toml --daemon

# Run as daemon with custom PID file and error log
./target/release/puerta run --config config/mongodb.toml \
    --daemon \
    --pid-file /var/run/puerta.pid \
    --error-log /var/log/puerta/error.log

# Test configuration without starting
./target/release/puerta run --config config/mongodb.toml --test
```

#### Zero-Downtime Upgrades

Puerta supports seamless zero-downtime upgrades using Pingora's graceful upgrade mechanism:

```bash
# Current setup: puerta daemon running in background
./target/release/puerta run --config config/mongodb.toml --daemon

# Upgrade process (no connection loss):
# Step 1: Start new version with --upgrade flag
./target/release/puerta_new run --config config/mongodb.toml --upgrade

# Step 2: Signal old instance to transfer listening sockets
kill -QUIT $(cat /tmp/puerta.pid)

# Advanced: Custom upgrade socket path
./target/release/puerta run --config config/mongodb.toml \
    --upgrade \
    --upgrade-sock /var/run/puerta_upgrade.sock
```

**Zero-Downtime Upgrade Guarantees:**
- ✅ No connection refused errors during upgrade
- ✅ All in-flight requests complete successfully  
- ✅ New connections handled by new instance immediately
- ✅ Session affinity maintained across upgrade

#### Command Line Options

```bash
# View all available options
./target/release/puerta run --help

# Available options:
#   -c, --config <CONFIG>              Path to configuration file
#   -d, --daemon                       Run as daemon process in the background
#   -p, --pid-file <PID_FILE>          PID file path for daemon mode
#   -e, --error-log <ERROR_LOG>        Error log file path for daemon mode
#   -t, --test                         Test configuration and exit
#   -u, --upgrade                      Enable upgrade mode for zero-downtime updates
#       --upgrade-sock <UPGRADE_SOCK>  Upgrade socket path for zero-downtime updates
```

### Testing

#### Unit and Integration Tests
```bash
# Run all unit tests
cargo test

# Run tests with coverage
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

#### End-to-End Load Balancer Tests
Comprehensive test suite for MongoDB and Redis cluster load balancing:

```bash
# Navigate to tests directory
cd tests

# MongoDB Tests
./test.sh mongo basic     # Basic MongoDB functionality (~15s)
./test.sh mongo quick     # Quick MongoDB verification (~30s)
./test.sh mongo full      # Comprehensive MongoDB test suite (~2-3min)

# Redis Tests
./test.sh redis basic     # Basic Redis functionality (~15s)
./test.sh redis quick     # Quick Redis verification (~30s)
./test.sh redis full      # Comprehensive Redis test suite (~2-3min)

# All Tests
./test.sh all             # Run complete test suite
```

For detailed test documentation, see [tests/README.md](tests/README.md).

### Benchmarking

```bash
# Run load balancing benchmarks
cargo bench
```

## Performance

Puerta is designed for high-performance scenarios:

- **Connection Handling**: Supports 10,000+ concurrent connections
- **Low Latency**: Sub-millisecond request routing overhead
- **Memory Efficient**: Minimal memory footprint per connection
- **Zero-Copy Forwarding**: Efficient data transfer using Pingora's optimized I/O

## Development

### Project Structure

```
src/
├── core/           # Core connection and backend management
├── modes/          # MongoDB and Redis mode implementations
│   ├── mongodb/    # MongoDB session affinity and load balancing
│   └── redis/      # Redis cluster protocol and redirection
├── health/         # Health checking implementations
├── config/         # Configuration management
└── utils/          # Common utilities

config/             # Configuration file examples
benches/            # Performance benchmarks
```

### Key Components

- **Connection Manager**: TCP connection handling and pooling
- **Session Affinity Manager**: Client-to-backend mapping for MongoDB
- **Redis Protocol Parser**: RESP protocol implementation
- **Redirection Handler**: Redis MOVED/ASK processing
- **Health Checkers**: MongoDB and Redis health monitoring

### Testing

The project maintains comprehensive test coverage:

- Unit tests for all core components
- Integration tests for connection handling
- Protocol parsing validation
- Redirection logic verification
- Session affinity behavior testing
- Runtime compatibility verification

Current test coverage: 116 tests passing

### Runtime Compatibility

The project has been optimized for seamless runtime integration:
- **Tokio Runtime Compatibility**: Fully compatible with Pingora's runtime model
- **Zero Runtime Conflicts**: Eliminated "Cannot start a runtime from within a runtime" errors
- **Production-Ready**: Stable deployment with proper async/sync boundary management

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with appropriate tests
4. Ensure all tests pass: `cargo test`
5. Run clippy checks: `cargo clippy -- -D warnings`
6. Build release version: `cargo build --release`
5. Submit a pull request

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

- **Cloudflare Pingora Team**: For the excellent Pingora proxy framework
- **RCProxy Project**: For Redis cluster proxy architecture patterns
- **MongoDB Inc.**: For MongoDB protocol documentation
- **Redis Labs**: For Redis Cluster specification
