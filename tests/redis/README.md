# Puerta Redis Cluster Load Balancer Tests

This directory contains comprehensive tests for the Puerta Redis cluster load balancer functionality.

## Test Scripts

### 1. Basic Test (`test_redis_lb_basic.sh`)
**Duration:** ~15-30 seconds  
**Purpose:** Basic connectivity and functionality verification

Tests:
- Load balancer connectivity (PING)
- Basic Redis operations (SET/GET)
- Multiple key operations with slot routing
- Hash tag functionality for consistent routing
- Concurrent connections handling
- Cluster information access

Usage:
```bash
./test_redis_lb_basic.sh
```

### 2. Quick Test (`test_redis_lb_quick.sh`)
**Duration:** ~30-60 seconds  
**Purpose:** Rapid verification with performance metrics

Tests:
- Basic connectivity and operations
- Rapid-fire operations (20 keys)
- Concurrent operations (10 parallel)
- Multiple Redis data types (Hash, List, Set)
- Cluster awareness
- Performance metrics calculation

Usage:
```bash
./test_redis_lb_quick.sh
```

### 3. Comprehensive Test (`test_redis_lb_comprehensive.sh`)
**Duration:** ~2-3 minutes  
**Purpose:** Complete test suite with detailed analysis

Tests:
- All basic and quick test functionality
- Performance benchmarking (100 SET/GET operations)
- Redis-specific data types (Sets, Sorted Sets, Expiration)
- Error handling scenarios
- Connection resilience testing
- Stress testing (30-second duration)
- Detailed JSON report generation

Usage:
```bash
./test_redis_lb_comprehensive.sh
```

## Prerequisites

### Redis Cluster Setup
Before running tests, ensure you have a Redis cluster running with the following nodes:
- Master nodes: localhost:7001, localhost:7002, localhost:7003
- Replica nodes: localhost:7004, localhost:7005, localhost:7006

### Puerta Load Balancer
The Puerta load balancer should be:
- Running on localhost:6379 (default Redis port)
- Configured to proxy to the Redis cluster
- Using the `redis.toml` configuration

## Expected Test Environment

```
Redis Cluster Topology:
├── Master: localhost:7001 (slots 0-5460)
├── Master: localhost:7002 (slots 5461-10922)  
├── Master: localhost:7003 (slots 10923-16383)
├── Replica: localhost:7004 → localhost:7001
├── Replica: localhost:7005 → localhost:7002
└── Replica: localhost:7006 → localhost:7003

Puerta Load Balancer:
└── localhost:6379 → Redis Cluster
```

## Key Test Features

### Slot Routing Verification
- Tests CRC16 slot calculation
- Verifies proper key routing to correct nodes
- Hash tag support for multi-key operations

### Load Balancing
- Concurrent connection handling
- Connection distribution across cluster nodes
- Performance under load

### Redis Protocol Support
- All standard Redis data types
- Command parsing and response handling
- Error propagation and handling

### Fault Tolerance
- Connection resilience testing
- Retry mechanism verification
- Graceful error handling

## Test Output

Each test script provides:
- Real-time progress indicators
- Success/failure status for each test
- Performance metrics where applicable
- Summary report with recommendations

The comprehensive test additionally generates:
- Detailed JSON report in `/tmp/puerta_redis_test_report_*.json`
- Complete execution log in `/tmp/puerta_redis_test_*.log`

## Integration with Main Test Runner

These Redis tests integrate with the main test runner:

```bash
# From project root
./test.sh redis-basic     # Run basic Redis test
./test.sh redis-quick     # Run quick Redis test  
./test.sh redis-full      # Run comprehensive Redis test
./test.sh all            # Run all tests (MongoDB + Redis)
```

## Troubleshooting

### Common Issues

1. **Connection refused**: Ensure Redis cluster is running on expected ports
2. **CLUSTERDOWN errors**: Verify cluster initialization completed successfully
3. **Slot not served**: Check that all 16384 slots are assigned to cluster nodes
4. **Load balancer not found**: Confirm Puerta is running on port 6379

### Debugging Commands

```bash
# Check Redis cluster status
redis-cli -p 7001 cluster info
redis-cli -p 7001 cluster nodes

# Test direct cluster connectivity
redis-cli -p 7001 ping
redis-cli -p 7002 ping
redis-cli -p 7003 ping

# Test Puerta load balancer
redis-cli -p 6379 ping
redis-cli -p 6379 info
```

## Performance Expectations

Typical performance benchmarks on local development environment:

- **Basic operations**: 50-100 ops/sec
- **Concurrent operations**: 80-90% success rate
- **Stress test**: 100-200 ops/sec sustained
- **Connection setup**: <100ms per connection

Performance may vary based on:
- Hardware specifications
- Network latency
- Redis cluster configuration
- System load