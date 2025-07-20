.PHONY: help build test clean fmt clippy deny coverage bench install dev docker

# Default target
help:
	@echo "Available targets:"
	@echo "  build     - Build the project"
	@echo "  test      - Run all tests"
	@echo "  clean     - Clean build artifacts"
	@echo "  fmt       - Format code with rustfmt"
	@echo "  clippy    - Run clippy lints"
	@echo "  deny      - Run cargo deny checks"
	@echo "  coverage  - Generate code coverage report"
	@echo "  bench     - Run benchmarks"
	@echo "  install   - Install puerta binary"
	@echo "  dev       - Run in development mode"
	@echo "  docker    - Build Docker image"

# Build targets
build:
	cargo build --release

test:
	cargo test --all-features
	cargo test --doc

clean:
	cargo clean

# Code quality targets
fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

deny:
	cargo deny check

coverage:
	cargo tarpaulin --verbose --all-features --workspace --timeout 120 --out html

# Performance targets
bench:
	cargo bench

# Installation and development
install:
	cargo install --path .

dev:
	cargo run -- --config config/dev.toml

# Docker
docker:
	docker build -t puerta:latest .

# Quality check pipeline
check: fmt clippy deny test

# Full CI pipeline
ci: check coverage bench