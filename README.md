<img src="./logo.png" alt="Puerta banner image" width="25%">

# Puerta

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

**Note**: This project currently includes local dependencies on Pingora framework components. For a working build, you'll need to clone the Pingora repository and update the Cargo.toml path dependencies accordingly.

```bash
git clone https://github.com/deadjoe/puerta
cd puerta

# Clone Pingora framework dependencies
git clone https://github.com/cloudflare/pingora examples/pingora

# Update Cargo.toml to uncomment and fix path dependencies
# Then build
cargo build --release
```

## Configuration

Puerta uses TOML configuration files. Example configurations are provided in the `config/` directory.

### MongoDB Mode Configuration

```toml
mode = "mongodb"
listen_addr = "0.0.0.0:27017"
max_connections = 10000

[mongodb]
mongos_endpoints = [
    "mongodb1.example.com:27017",
    "mongodb2.example.com:27017",
    "mongodb3.example.com:27017"
]
session_affinity_enabled = true
session_timeout_sec = 1800
health_check_interval_sec = 30
```

### Redis Mode Configuration

```toml
mode = "redis"
listen_addr = "0.0.0.0:6379"
max_connections = 10000

[redis]
cluster_nodes = [
    "redis1.example.com:6379",
    "redis2.example.com:6379", 
    "redis3.example.com:6379"
]
slot_refresh_interval_sec = 60
max_redirects = 3
connection_timeout_ms = 5000
```

## Usage

### Running Puerta

```bash
# Start with MongoDB configuration
./target/release/puerta --config config/mongodb.toml

# Start with Redis configuration  
./target/release/puerta --config config/redis.toml

# Enable debug logging
RUST_LOG=debug ./target/release/puerta --config config/mongodb.toml
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

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

Current test coverage: 106 tests passing

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with appropriate tests
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

- **Cloudflare Pingora Team**: For the excellent Pingora proxy framework
- **RCProxy Project**: For Redis cluster proxy architecture patterns
- **MongoDB Inc.**: For MongoDB protocol documentation
- **Redis Labs**: For Redis Cluster specification
