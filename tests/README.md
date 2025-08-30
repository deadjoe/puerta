# Puerta Test Suite

This directory contains the comprehensive test suite for the Puerta load balancer, supporting both MongoDB Sharded Cluster and Redis Cluster load balancing validation.

## Directory Structure

```
tests/
├── mongodb/                         # MongoDB load balancer tests
│   ├── README.md                   # MongoDB test documentation
│   ├── test_mongodb_lb_basic.sh           # Basic functionality test
│   ├── test_mongodb_lb_quick.sh           # Quick verification test
│   └── test_mongodb_lb_comprehensive.sh   # Comprehensive test suite
├── redis/                          # Redis cluster load balancer tests
│   ├── README.md                   # Redis test documentation
│   ├── test_redis_lb_basic.sh             # Basic functionality test
│   ├── test_redis_lb_quick.sh             # Quick verification test
│   ├── test_redis_lb_comprehensive.sh     # Comprehensive test suite
│   └── test_redis_routing_logic.sh        # Slot routing logic validation
└── test.sh                         # Unified test runner
```

## Quick Start

### Using the Convenient Test Runner

```bash
# Run from the tests directory
cd tests
./test.sh <database> <test_type>

# MongoDB Tests
./test.sh mongo basic     # MongoDB basic functionality test
./test.sh mongo quick     # MongoDB quick verification test  
./test.sh mongo full      # MongoDB comprehensive test suite

# Redis Tests
./test.sh redis basic     # Redis basic functionality test
./test.sh redis quick     # Redis quick verification test
./test.sh redis full      # Redis comprehensive test suite
./test.sh redis routing   # Redis slot routing logic validation

# Note: Puerta operates in single mode (MongoDB OR Redis) at any given time.
# Choose tests based on the currently running mode.
```

### Running Tests Directly

```bash
# MongoDB Tests
./tests/mongodb/test_mongodb_lb_basic.sh           # Basic functionality test (~15s)
./tests/mongodb/test_mongodb_lb_quick.sh           # Quick verification test (~30s)
./tests/mongodb/test_mongodb_lb_comprehensive.sh   # Comprehensive test suite (~2-3min)

# Redis Tests
./tests/redis/test_redis_lb_basic.sh               # Basic functionality test (~15s)
./tests/redis/test_redis_lb_quick.sh               # Quick verification test (~30s)
./tests/redis/test_redis_lb_comprehensive.sh       # Comprehensive test suite (~2-3min)
./tests/redis/test_redis_routing_logic.sh          # Slot routing logic validation (~10s)
```

## Test Types

### MongoDB Test Suite
Validates MongoDB load balancer functionality:
- Basic connectivity and routing verification
- Session affinity testing
- Load balancing efficiency validation
- Concurrent connection handling
- Database operation verification
- Performance benchmarking
- Error handling and recovery capabilities

### Redis Test Suite
Validates Redis cluster load balancer functionality:
- Basic connectivity and RESP protocol support
- Slot-based routing and hash tag functionality
- CRC16 slot calculation verification
- Redis cluster topology awareness
- Multiple Redis data type support
- Concurrent connection and performance testing
- MOVED/ASK redirection handling
- Routing consistency validation
- Connection resilience and error handling

## Usage Recommendations

### Development Phase
```bash
# MongoDB quick validation
./test.sh mongo basic

# Redis quick validation  
./test.sh redis basic

# Redis routing logic validation (recommended for development)
./test.sh redis routing
```

### Pre-commit Validation
```bash
# MongoDB functionality verification
./test.sh mongo quick

# Redis functionality verification
./test.sh redis quick
```

### Pre-deployment Validation
```bash
# Comprehensive functionality and performance testing
./test.sh mongo full    # MongoDB comprehensive test
./test.sh redis full    # Redis comprehensive test
```

## Test Configuration

### MongoDB Test Configuration
```bash
# Load balancer configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")
```

### Redis Test Configuration
```bash
# Load balancer configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="6379"
REDIS_NODES=("127.0.0.1:7001" "127.0.0.1:7002" "127.0.0.1:7003" "127.0.0.1:7004" "127.0.0.1:7005" "127.0.0.1:7006")
```

## Adding New Tests

1. Create test scripts in the appropriate subdirectory
2. Follow existing naming convention: `test_<component>_<type>.sh`
3. Add appropriate documentation and comments
4. Update relevant README files
5. Ensure tests are executable: `chmod +x your_test.sh`

## Output Files

Tests generate the following output files:
- Real-time console output
- Detailed log files (`/tmp/puerta_test_*.log`)
- JSON format reports (`/tmp/puerta_test_report_*.json`)

## Prerequisites

### MongoDB Test Prerequisites
- MongoDB Sharded Cluster running and accessible
- Puerta load balancer started in MongoDB mode (port 27016)
- `mongosh` MongoDB Shell installed
- `bc` calculator tool installed

### Redis Test Prerequisites  
- Redis Cluster nodes running (ports 7001-7006) or single Redis instance
- Puerta load balancer started in Redis mode (port 6379)
- `redis-cli` Redis command-line tool installed
- `bc` calculator tool installed

## Performance Metrics Reference

### MongoDB Tests
- Basic test: ~15 seconds
- Quick test: ~30 seconds  
- Comprehensive test: ~2-3 minutes
- Expected performance: 50-100 operations/second

### Redis Tests
- Basic test: ~15 seconds
- Quick test: ~30 seconds
- Comprehensive test: ~2-3 minutes
- Routing logic validation: ~10 seconds
- Expected performance: 100-150 operations/second
- Stress test capability: 4000+ operations in 30 seconds
- Slot calculation performance: ~200 calculations/second

## Troubleshooting

If you encounter issues, please check:

1. **Test-specific documentation**: Refer to detailed README files in each test subdirectory (`tests/mongodb/README.md`, `tests/redis/README.md`)
2. **Test log files**: Check error information in test logs (`/tmp/puerta_*_test_*.log`)
3. **Prerequisites**: Ensure all prerequisites are met
4. **Backend cluster status**: Verify the respective backend database cluster is running and accessible
5. **Puerta configuration**: Confirm Puerta is running in the correct mode with proper configuration
6. **Network connectivity**: Ensure network connectivity between Puerta and backend services

## Test Development Guidelines

### Writing New Tests
- Use clear, descriptive test names
- Include setup and cleanup procedures
- Add comprehensive error handling
- Document expected behavior and edge cases
- Follow the existing code style and patterns

### Test Best Practices
- Test one feature per script when possible
- Include both positive and negative test cases
- Add timeout mechanisms for long-running operations
- Generate meaningful log output for debugging
- Clean up test data after completion

## Integration with CI/CD

These tests are designed for integration with continuous integration pipelines:

```bash
# Example CI pipeline integration
./test.sh mongo basic && ./test.sh redis routing
```

For comprehensive validation in production environments:

```bash
# Full validation suite
./test.sh mongo full && ./test.sh redis full
```